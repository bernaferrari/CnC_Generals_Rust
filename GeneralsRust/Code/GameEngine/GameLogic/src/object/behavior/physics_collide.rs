//! C++ PhysicsBehavior::onCollide (PhysicsUpdate.cpp:1141-1400).
//! Projectile early-out, ground/containment, unmanned steal, crush skip,
//! vehicle-into-building crash weapons, overlap bounce-apart.

use super::physics_crush::check_for_overlap_collision;
use super::{
    FLAG_ALLOW_COLLIDE_FORCE, INVALID_VEL_MAG, PhysicsBehaviorHandle, PhysicsBehaviorModuleData,
    find_object,
};
use crate::common::{
    Coord3D, DisabledType, KindOf, LOGICFRAMES_PER_SECOND, ObjectID, ObjectStatusTypes, Real,
};
use crate::helpers::{TheGameLogic, TheWeaponStore};
use crate::modules::PhysicsBehaviorExt;
use crate::object::Object as GameObject;
use crate::object::behavior::dumb_projectile_behavior::dispatch_dumb_projectile_handle_collision;
use game_engine::common::global_data;

const MIN_STIFF: Real = 0.01;
const MAX_STIFF: Real = 0.99;

/// C++ PhysicsBehavior::onCollide.
pub(super) fn on_collide(
    handle: &mut PhysicsBehaviorHandle,
    object_id: ObjectID,
    other_id: ObjectID,
    module_data: &PhysicsBehaviorModuleData,
) {
    // Projectiles always get a chance to handle their own collisions first.
    let other_opt = if other_id == crate::common::INVALID_ID {
        None
    } else {
        Some(other_id)
    };
    if dispatch_dumb_projectile_handle_collision(object_id, other_opt) {
        return;
    }

    let Some(obj_arc) = find_object(object_id) else {
        return;
    };
    let Ok(obj) = obj_arc.try_read() else {
        return;
    };

    let obj_contained_by = obj.get_contained_by();

    // other == null means collide with ground.
    if other_id == crate::common::INVALID_ID {
        if let Some(container_id) = obj_contained_by {
            let pos = *obj.get_position();
            let normal = Coord3D::new(0.0, 0.0, -1.0);
            drop(obj);
            if let Some(container) = find_object(container_id) {
                if let Ok(mut container) = container.try_write() {
                    container.on_collide(None, &pos, &normal);
                }
            }
        }
        return;
    }

    let Some(other_arc) = find_object(other_id) else {
        return;
    };
    let Ok(other) = other_arc.try_read() else {
        return;
    };

    let other_contained_by = other.get_contained_by();
    if other_contained_by == Some(object_id) || obj_contained_by == Some(other_id) {
        return;
    }

    if obj.test_status(ObjectStatusTypes::Parachuting)
        && other.test_status(ObjectStatusTypes::Parachuting)
    {
        return;
    }

    if handle.is_ignoring_collisions_with(other_id) {
        return;
    }

    if let Some(ai) = obj.get_ai() {
        if let Ok(ai) = ai.try_lock() {
            if ai.get_ignored_obstacle_id() == other_id {
                // Infantry walking into an unmanned vehicle: recrew it.
                if obj.is_kind_of(KindOf::Infantry)
                    && other.is_disabled_by_type(DisabledType::DisabledUnmanned)
                {
                    let infantry_name = obj.get_name().clone();
                    let infantry_team = obj.get_team();
                    drop(ai);
                    drop(obj);
                    drop(other);
                    if let Ok(mut other) = other_arc.try_write() {
                        other.clear_disabled(DisabledType::DisabledUnmanned);
                        other.set_captured(true);
                        other.defect(infantry_team, 0);
                    }
                    let _ =
                        crate::scripting::engine::transfer_object_name(&infantry_name, other_id);
                    let _ = TheGameLogic::destroy_object_by_id(object_id);
                }
                return;
            }
        }
    }

    if let Some(ai_other) = other.get_ai() {
        if let Ok(ai_other) = ai_other.try_lock() {
            if ai_other.get_ignored_obstacle_id() == object_id {
                return;
            }
        }
    }
    if let Some(other_physics) = other.get_physics() {
        if let Ok(phys) = other_physics.try_lock() {
            if phys.get_ignore_collisions_with() == object_id {
                return;
            }
        }
    } else if !other.is_kind_of(KindOf::Immobile) {
        return;
    }

    // Crush / overlap skip bounce. Need write on crushee for damage.
    drop(other);
    {
        let Ok(mut other) = other_arc.try_write() else {
            return;
        };
        if check_for_overlap_collision(handle, &obj, &mut other) {
            return;
        }
    }
    let Ok(other) = other_arc.try_read() else {
        return;
    };

    let other_immobile = other.is_kind_of(KindOf::Immobile);

    // AI processCollision may refuse bounce. Dead/parachuting vs immobile still bounce.
    // The dyn AIUpdateInterface has no process_collision; default is allow force
    // (C++ returns true unless the locomotor refuses). Ignored-obstacle is handled above.
    if obj.get_ai().is_some()
        && !((obj.is_effectively_dead() || obj.test_status(ObjectStatusTypes::Parachuting))
            && other_immobile)
    {
        // Continue — apply bounce unless collide force is off.
    }

    let us_center = obj
        .get_geometry_info()
        .get_center_position(obj.get_position());
    let them_center = other
        .get_geometry_info()
        .get_center_position(other.get_position());
    let mut delta = Coord3D::new(
        them_center.x - us_center.x,
        them_center.y - us_center.y,
        them_center.z - us_center.z,
    );

    let (us_radius, them_radius, dist_sqr) = if obj.is_above_terrain() {
        (
            obj.get_geometry_info().get_bounding_sphere_radius(),
            other.get_geometry_info().get_bounding_sphere_radius(),
            delta.x * delta.x + delta.y * delta.y + delta.z * delta.z,
        )
    } else {
        delta.z = 0.0;
        (
            obj.get_geometry_info().get_bounding_circle_radius(),
            other.get_geometry_info().get_bounding_circle_radius(),
            delta.x * delta.x + delta.y * delta.y,
        )
    };
    let radius_sum = us_radius + them_radius;
    if dist_sqr > radius_sum * radius_sum {
        return;
    }

    handle.state.last_collidee = other_id;

    let mut dist = dist_sqr.sqrt();
    let mut overlap = us_radius + them_radius - dist;
    if dist < 1.0 {
        dist = 1.0;
    }

    if !handle.state.has_flag(FLAG_ALLOW_COLLIDE_FORCE) {
        return;
    }

    let mut factor;
    if other_immobile && !obj.is_destroyed() {
        if obj.test_status(ObjectStatusTypes::Parachuting) {
            let mut bounce_id = object_id;
            let mut walk = obj.get_contained_by();
            while let Some(container_id) = walk {
                bounce_id = container_id;
                walk = find_object(container_id)
                    .and_then(|c| c.try_read().ok().and_then(|g| g.get_contained_by()));
            }
            let bounce_out = us_radius * 0.1;
            drop(obj);
            drop(other);
            if let Some(bounce_arc) = find_object(bounce_id) {
                if let Ok(mut bounce) = bounce_arc.try_write() {
                    let mut tmp = *bounce.get_position();
                    tmp.x -= bounce_out * delta.x / dist;
                    tmp.y -= bounce_out * delta.y / dist;
                    let _ = bounce.set_position(&tmp);
                    if let Some(phys) = bounce.get_physics() {
                        phys.scrub_velocity_2d(0.0);
                    }
                }
            }
            return;
        }

        let stiffness = global_data::read_safe()
            .map(|data| data.structure_stiffness)
            .unwrap_or(0.5)
            .clamp(MIN_STIFF, MAX_STIFF);
        let mut mag = handle.velocity_magnitude();
        let min_bounce = 1.0 / (LOGICFRAMES_PER_SECOND as Real * 5.0);
        if mag < min_bounce {
            mag = min_bounce;
        }
        factor = -mag * handle.mass_with_cargo(Some(&obj)) * stiffness;

        let rubble_h = global_data::read_safe()
            .map(|data| data.default_structure_rubble_height)
            .unwrap_or(1.0);
        if delta.z < 0.0 && obj.get_position().z >= rubble_h {
            if other.is_kind_of(KindOf::Structure) {
                if obj.is_kind_of(KindOf::Vehicle) {
                    fire_crash_weapon(
                        module_data
                            .vehicle_crashes_into_building_weapon_template
                            .as_str(),
                        &obj,
                    );
                }
                drop(obj);
                drop(other);
                let _ = TheGameLogic::destroy_object_by_id(object_id);
                return;
            } else if obj.is_kind_of(KindOf::Vehicle) {
                fire_crash_weapon(
                    module_data
                        .vehicle_crashes_into_non_building_weapon_template
                        .as_str(),
                    &obj,
                );
            }
        }

        handle.state.vel = Coord3D::ZERO;
        handle.state.vel_mag = INVALID_VEL_MAG;
    } else {
        if overlap > 5.0 {
            overlap = 5.0;
        }
        factor = -overlap;
    }

    let force = Coord3D::new(
        factor * delta.x / dist,
        factor * delta.y / dist,
        factor * delta.z / dist,
    );
    if force.x.is_finite() && force.y.is_finite() && force.z.is_finite() {
        handle.apply_force_with_obj(&force, Some(&obj));
    }
}

fn fire_crash_weapon(template_name: &str, source: &GameObject) {
    if template_name.is_empty() {
        return;
    }
    let pos = *source.get_position();
    let _ = TheWeaponStore::create_and_fire_temp_weapon(template_name, source, &pos);
}
