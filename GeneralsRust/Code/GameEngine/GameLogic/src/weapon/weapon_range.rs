//! C++ `Weapon::isWithinAttackRange` (Weapon.cpp:2135-2207).

use crate::common::GeometryInfo;
use crate::common::{Coord3D, KindOf};
use crate::helpers::ThePartitionManager;
use crate::object::registry::OBJECT_REGISTRY;
use crate::terrain::BridgeAttackInfo;

use super::helpers::{ObjectId, dual_world_registry_unavailable};
use super::masks_enums::{WeaponBonus, WeaponBonusConditionFlags};
use super::weapon_instance::Weapon;

impl Weapon {
    pub fn is_within_attack_range(
        &self,
        source_obj: ObjectId,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        let Some((source_pos, source_geom)) = self.range_source(source_obj) else {
            return false;
        };
        let source_radius = source_geom.get_bounding_circle_radius();
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.is_within_attack_range_from_source(
            &source_pos,
            source_radius,
            &source_geom,
            &bonus,
            target_obj,
            target_pos,
        )
    }

    pub fn is_within_attack_range_from_source(
        &self,
        source_pos: &Coord3D,
        source_radius: f32,
        source_geom: &GeometryInfo,
        bonus: &WeaponBonus,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        if let Some(pos) = target_pos {
            let max_range = self.template.get_attack_range(bonus);
            let min_range = self.template.get_minimum_attack_range();
            let attack_range_sqr = max_range * max_range;
            let min_range_sqr = min_range * min_range;
            let dist_sqr = boundary_dist_sqr(&source_pos, source_radius, pos, 0.0);
            // C++ Weapon.cpp:2140-2141 (RATIONALIZE_ATTACK_RANGE): no -0.5 fudge.
            if dist_sqr < min_range_sqr {
                return false;
            }
            return dist_sqr <= attack_range_sqr;
        }

        let Some(target_id) = target_obj else {
            return false;
        };

        let Some((target_pos, target_radius, is_bridge, is_structure)) =
            self.range_target(target_id)
        else {
            return false;
        };

        self.is_within_attack_range_from_target_data(
            &source_pos,
            source_radius,
            &source_geom,
            &bonus,
            target_id,
            target_pos,
            target_radius,
            is_bridge,
            is_structure,
            self.caller_held_source
                .as_ref()
                .map(|source| (source.id, source.position))
                .as_slice(),
        )
    }

    /// C++ `Weapon::isWithinAttackRange(source, target)` with both Objects
    /// already borrowed by the synchronous caller.
    pub(crate) fn is_within_attack_range_for_objects(
        &self,
        source: &crate::object::Object,
        target: Option<&crate::object::Object>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        let source_pos = *source.get_position();
        let source_geom = *source.get_geometry_info();
        let bonus = self.compute_bonus_for_object(source);
        if target_pos.is_some() {
            return self.is_within_attack_range_from_source(
                &source_pos,
                source_geom.get_bounding_circle_radius(),
                &source_geom,
                &bonus,
                None,
                target_pos,
            );
        }
        if let Some(target) = target {
            self.is_within_attack_range_from_target_data(
                &source_pos,
                source_geom.get_bounding_circle_radius(),
                &source_geom,
                &bonus,
                target.get_id(),
                *target.get_position(),
                target.get_geometry_info().get_bounding_circle_radius(),
                target.is_kind_of(KindOf::Bridge),
                target.is_kind_of(KindOf::Structure),
                &[
                    (source.get_id(), source_pos),
                    (target.get_id(), *target.get_position()),
                ],
            )
        } else {
            self.is_within_attack_range_from_source(
                &source_pos,
                source_geom.get_bounding_circle_radius(),
                &source_geom,
                &bonus,
                None,
                target_pos,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn is_within_attack_range_from_target_data(
        &self,
        source_pos: &Coord3D,
        source_radius: f32,
        source_geom: &GeometryInfo,
        bonus: &WeaponBonus,
        target_id: ObjectId,
        target_pos: Coord3D,
        target_radius: f32,
        is_bridge: bool,
        is_structure: bool,
        borrowed_positions: &[(ObjectId, Coord3D)],
    ) -> bool {
        let max_range = self.template.get_attack_range(bonus);
        let min_range = self.template.get_minimum_attack_range();
        let attack_range_sqr = max_range * max_range;
        let min_range_sqr = min_range * min_range;

        let dist_sqr = if is_bridge {
            let mut info = BridgeAttackInfo::new();
            if let Ok(guard) = crate::terrain::get_terrain_logic().try_read() {
                guard.get_bridge_attack_points(target_id, &mut info);
            }
            let d1 = boundary_dist_sqr(source_pos, source_radius, &info.attack_point1, 0.0);
            if d1 <= attack_range_sqr {
                d1
            } else {
                boundary_dist_sqr(source_pos, source_radius, &info.attack_point2, 0.0)
            }
        } else {
            boundary_dist_sqr(source_pos, source_radius, &target_pos, target_radius)
        };

        // C++ Weapon.cpp:2175-2176 (RATIONALIZE_ATTACK_RANGE): contact distance,
        // no -0.5 fudge.
        if dist_sqr < min_range_sqr {
            return false;
        }
        if dist_sqr > attack_range_sqr {
            return false;
        }

        if self.is_contact_weapon() && is_structure {
            let Some(partition) = ThePartitionManager::get() else {
                return false;
            };
            let hits = partition.iterate_potential_collisions_with_borrowed_positions(
                source_pos,
                source_geom,
                0.0,
                borrowed_positions,
            );
            return hits.iter().any(|&id| id == target_id);
        }

        true
    }

    /// C++ Weapon.cpp:2211–2238: no minimum range means no distance query.
    pub fn is_too_close(
        &self,
        source_obj: ObjectId,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        let min_range = self.template.get_minimum_attack_range();
        if min_range == 0.0 {
            return false;
        }
        let Some((source_pos, source_geom)) = self.range_source(source_obj) else {
            return false;
        };
        let (target_pos, target_radius) = if let Some(target_id) = target_obj {
            let Some((position, radius, _, _)) = self.range_target(target_id) else {
                return false;
            };
            (position, radius)
        } else if let Some(pos) = target_pos {
            (*pos, 0.0)
        } else {
            return false;
        };
        boundary_dist_sqr(
            &source_pos,
            source_geom.get_bounding_circle_radius(),
            &target_pos,
            target_radius,
        ) < min_range * min_range
    }

    fn range_source(&self, id: ObjectId) -> Option<(Coord3D, GeometryInfo)> {
        if let Some(source) = self.caller_source(id) {
            return Some((source.position, source.geometry));
        }
        if dual_world_registry_unavailable() {
            return None;
        }
        OBJECT_REGISTRY.with_object(id, |source| {
            (*source.get_position(), *source.get_geometry_info())
        })
    }

    fn range_target(&self, id: ObjectId) -> Option<(Coord3D, f32, bool, bool)> {
        // Self-targets are legal queries in C++; the same borrowed Object must
        // not be reacquired as a target while the caller holds its write guard.
        if let Some(source) = self.caller_source(id) {
            return Some((
                source.position,
                source.geometry.get_bounding_circle_radius(),
                source.is_bridge,
                source.is_structure,
            ));
        }
        OBJECT_REGISTRY.with_object(id, |target| {
            (
                *target.get_position(),
                target.get_geometry_info().get_bounding_circle_radius(),
                target.is_kind_of(KindOf::Bridge),
                target.is_kind_of(KindOf::Structure),
            )
        })
    }
}

fn boundary_dist_sqr(a: &Coord3D, a_r: f32, b: &Coord3D, b_r: f32) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let center = (dx * dx + dy * dy).sqrt();
    let boundary = (center - a_r - b_r).max(0.0);
    boundary * boundary
}
