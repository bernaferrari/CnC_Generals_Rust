//! C++ `Weapon::isWithinAttackRange` (Weapon.cpp:2135-2207).

use crate::common::{Coord3D, KindOf};
use crate::helpers::ThePartitionManager;
use crate::object::registry::OBJECT_REGISTRY;
use crate::terrain::BridgeAttackInfo;

use super::helpers::{ObjectId, dual_world_registry_unavailable};
use super::masks_enums::WeaponBonusConditionFlags;
use super::weapon_instance::Weapon;

impl Weapon {
    pub fn is_within_attack_range(
        &self,
        source_obj: ObjectId,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some((source_pos, source_radius, source_geom)) =
            OBJECT_REGISTRY.with_object(source_obj, |guard| {
                (
                    *guard.get_position(),
                    guard.get_geometry_info().get_bounding_circle_radius(),
                    guard.get_geometry_info().clone(),
                )
            })
        else {
            return false;
        };

        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        let max_range = self.template.get_attack_range(&bonus);
        let min_range = self.template.get_minimum_attack_range();
        let attack_range_sqr = max_range * max_range;
        let min_range_sqr = min_range * min_range;

        if let Some(pos) = target_pos {
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

        let Some((target_pos, target_radius, is_bridge, is_structure)) = OBJECT_REGISTRY
            .with_object(target_id, |guard| {
                (
                    *guard.get_position(),
                    guard.get_geometry_info().get_bounding_circle_radius(),
                    guard.is_kind_of(KindOf::Bridge),
                    guard.is_kind_of(KindOf::Structure),
                )
            })
        else {
            return false;
        };

        let dist_sqr = if is_bridge {
            let mut info = BridgeAttackInfo::new();
            if let Ok(guard) = crate::terrain::get_terrain_logic().try_read() {
                guard.get_bridge_attack_points(target_id, &mut info);
            }
            let d1 = boundary_dist_sqr(&source_pos, source_radius, &info.attack_point1, 0.0);
            if d1 <= attack_range_sqr {
                d1
            } else {
                boundary_dist_sqr(&source_pos, source_radius, &info.attack_point2, 0.0)
            }
        } else {
            boundary_dist_sqr(&source_pos, source_radius, &target_pos, target_radius)
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
            let hits = partition.iterate_potential_collisions(&source_pos, &source_geom, 0.0);
            return hits.iter().any(|&id| id == target_id);
        }

        true
    }
}

fn boundary_dist_sqr(a: &Coord3D, a_r: f32, b: &Coord3D, b_r: f32) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let center = (dx * dx + dy * dy).sqrt();
    let boundary = (center - a_r - b_r).max(0.0);
    boundary * boundary
}
