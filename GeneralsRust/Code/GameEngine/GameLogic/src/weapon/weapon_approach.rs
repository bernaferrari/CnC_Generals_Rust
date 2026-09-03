//! C++ `Weapon::computeApproachTarget` (Weapon.cpp:1977-2102).

use crate::ai::the_ai;
use crate::common::{Coord3D, KindOf};
use crate::helpers::TheTerrainLogic;
use crate::path::{PATHFIND_CELL_SIZE_F, SURFACE_AIR, SURFACE_GROUND};

use super::helpers::{ObjectId, dual_world_registry_unavailable};
use super::weapon_instance::Weapon;

const ATTACK_RANGE_APPROACH_FUDGE: f32 = 0.9;
const APPROACH_FUDGE: f32 = 0.001;

impl Weapon {
    /// Compute a stand-off / backup goal for attacking `target` or `pos`.
    ///
    /// Returns `true` when the source is already close enough (C++ "we're close enough").
    /// Otherwise writes `approach_target_pos` and returns `false`.
    pub fn compute_approach_target(
        &self,
        source: ObjectId,
        target: Option<ObjectId>,
        pos: Option<&Coord3D>,
        angle_offset: f32,
        approach_target_pos: &mut Coord3D,
    ) -> bool {
        if dual_world_registry_unavailable() {
            *approach_target_pos = Coord3D::new(0.0, 0.0, 0.0);
            return false;
        }

        let Some(source_snap) = object_approach_snap(source) else {
            *approach_target_pos = Coord3D::new(0.0, 0.0, 0.0);
            return false;
        };

        let (target_pos, target_radius, dir) = if let Some(target_id) = target {
            let Some(target_snap) = object_approach_snap(target_id) else {
                *approach_target_pos = Coord3D::new(0.0, 0.0, 0.0);
                return false;
            };
            let dir = bounding_sphere_2d_vector(&target_snap, &source_snap);
            (target_snap.pos, target_snap.radius, dir)
        } else if let Some(pos) = pos {
            let dir_from_source = bounding_sphere_2d_vector_to_pos(&source_snap, pos);
            let dir = Coord3D::new(-dir_from_source.x, -dir_from_source.y, -dir_from_source.z);
            (*pos, 0.0, dir)
        } else {
            *approach_target_pos = Coord3D::new(0.0, 0.0, 0.0);
            return false;
        };

        let dist = dir.length();
        let min_attack_range = self.template.get_minimum_attack_range();
        if min_attack_range > PATHFIND_CELL_SIZE_F && dist < min_attack_range {
            let mut dir = Coord3D::new(
                source_snap.pos.x - target_pos.x,
                source_snap.pos.y - target_pos.y,
                0.0,
            );
            dir = if dir.length() > f32::EPSILON {
                dir.normalize()
            } else {
                Coord3D::new(1.0, 0.0, 0.0)
            };

            if source_snap.above_terrain {
                let angle = (-dir.y).atan2(-dir.x);
                let mut rel = source_snap.orientation - angle;
                let two_pi = std::f32::consts::TAU;
                if rel > two_pi {
                    rel -= two_pi;
                }
                if rel < -two_pi {
                    rel += two_pi;
                }
                if rel.abs() < std::f32::consts::FRAC_PI_2 {
                    dir.x = -dir.x;
                    dir.y = -dir.y;
                    dir.z = -dir.z;
                }
            }

            apply_angle_offset(&mut dir, angle_offset);

            let mut attack_range = (self.get_attack_range(source) + min_attack_range) * 0.5;
            attack_range += target_radius;
            attack_range += source_snap.radius;
            *approach_target_pos = Coord3D::new(
                attack_range * dir.x + target_pos.x,
                attack_range * dir.y + target_pos.y,
                attack_range * dir.z + target_pos.z,
            );
            clip_to_terrain_extent(approach_target_pos);
            return false;
        }

        if dist < APPROACH_FUDGE {
            *approach_target_pos = source_snap.pos;
            return true;
        }

        if self.is_contact_weapon() {
            *approach_target_pos = target_pos;
            return false;
        }

        let mut dir = Coord3D::new(dir.x / dist, dir.y / dist, dir.z / dist);
        apply_angle_offset(&mut dir, angle_offset);

        let attack_range = self.get_attack_range(source) * ATTACK_RANGE_APPROACH_FUDGE;
        *approach_target_pos = Coord3D::new(
            attack_range * dir.x + target_pos.x,
            attack_range * dir.y + target_pos.y,
            attack_range * dir.z + target_pos.z,
        );

        if source_snap.adjusts_destination {
            let ai_store = the_ai(); if let Ok(ai) = ai_store.read() {
                if let Some(pathfinder) = ai.pathfinder() {
                    if let Ok(pf) = pathfinder.read() {
                        let surfaces = if source_snap.airborne {
                            SURFACE_AIR
                        } else {
                            SURFACE_GROUND
                        };
                        let _ = pf.adjust_target_destination_for(
                            surfaces,
                            source_snap.is_crusher,
                            source_snap.radius,
                            Some(source),
                            approach_target_pos,
                            |goal| {
                                self.is_source_object_with_goal_position_within_attack_range(
                                    source, goal, target, pos,
                                )
                            },
                        );
                    }
                }
            }
        }

        false
    }
}

struct ApproachSnap {
    pos: Coord3D,
    radius: f32,
    orientation: f32,
    above_terrain: bool,
    airborne: bool,
    adjusts_destination: bool,
    is_crusher: bool,
}

fn object_approach_snap(id: ObjectId) -> Option<ApproachSnap> {
    crate::object::registry::OBJECT_REGISTRY.with_object(id, |guard| {
        let adjusts = guard
            .get_ai()
            .and_then(|ai| {
                ai.lock()
                    .ok()
                    .map(|ai_guard| ai_guard.is_aircraft_that_adjusts_destination())
            })
            .unwrap_or(false);
        ApproachSnap {
            pos: *guard.get_position(),
            radius: guard.get_geometry_info().get_bounding_circle_radius(),
            orientation: guard.get_orientation(),
            above_terrain: guard.is_above_terrain(),
            airborne: guard.is_airborne_target() || guard.is_kind_of(KindOf::Aircraft),
            adjusts_destination: adjusts,
            is_crusher: guard.get_crusher_level() > 0,
        }
    })
}

fn bounding_sphere_2d_vector(from: &ApproachSnap, to: &ApproachSnap) -> Coord3D {
    let dx = to.pos.x - from.pos.x;
    let dy = to.pos.y - from.pos.y;
    let center = (dx * dx + dy * dy).sqrt();
    if center <= f32::EPSILON {
        return Coord3D::new(0.0, 0.0, 0.0);
    }
    let boundary = (center - from.radius - to.radius).max(0.0);
    let scale = boundary / center;
    Coord3D::new(dx * scale, dy * scale, 0.0)
}

fn bounding_sphere_2d_vector_to_pos(from: &ApproachSnap, pos: &Coord3D) -> Coord3D {
    let dx = pos.x - from.pos.x;
    let dy = pos.y - from.pos.y;
    let center = (dx * dx + dy * dy).sqrt();
    if center <= f32::EPSILON {
        return Coord3D::new(0.0, 0.0, 0.0);
    }
    let boundary = (center - from.radius).max(0.0);
    let scale = boundary / center;
    Coord3D::new(dx * scale, dy * scale, 0.0)
}

fn apply_angle_offset(dir: &mut Coord3D, angle_offset: f32) {
    if angle_offset == 0.0 {
        return;
    }
    let angle = dir.y.atan2(dir.x) + angle_offset;
    dir.x = angle.cos();
    dir.y = angle.sin();
}

fn clip_to_terrain_extent(pos: &mut Coord3D) {
    let Some(terrain) = TheTerrainLogic::get() else {
        return;
    };
    let bounds = terrain.get_extent();
    let pad = PATHFIND_CELL_SIZE_F;
    if pos.x < bounds.lo.x + pad {
        pos.x = bounds.lo.x + pad;
    }
    if pos.y < bounds.lo.y + pad {
        pos.y = bounds.lo.y + pad;
    }
    if pos.x > bounds.hi.x - pad {
        pos.x = bounds.hi.x - pad;
    }
    if pos.y > bounds.hi.y - pad {
        pos.y = bounds.hi.y - pad;
    }
}
