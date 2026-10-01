//! C++ PhysicsBehavior::onCollide (PhysicsUpdate.cpp:1141-1400).
//! Projectile early-out, ground/containment, unmanned steal, crush skip,
//! vehicle-into-building crash weapons, overlap bounce-apart.

use super::physics_crush::check_for_overlap_collision;
use super::{
    FLAG_ALLOW_COLLIDE_FORCE, INVALID_VEL_MAG, PhysicsBehaviorHandle, PhysicsBehaviorModuleData,
};
use crate::common::{
    AsciiString, Coord3D, DisabledType, KindOf, LOGICFRAMES_PER_SECOND, ObjectID,
    ObjectStatusTypes, Real,
};
use crate::helpers::{TheGameLogic, TheWeaponStore};
use crate::modules::{AIUpdateInterface, PhysicsBehavior};
use crate::object::Object as GameObject;
use crate::object::behavior::dumb_projectile_behavior::dispatch_dumb_projectile_handle_collision;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::global_data;

const MIN_STIFF: Real = 0.01;
const MAX_STIFF: Real = 0.99;

struct SelfView {
    contained_by: Option<ObjectID>,
    pos: Coord3D,
    parachuting: bool,
    infantry: bool,
    vehicle: bool,
    name: AsciiString,
    team: Option<crate::team::TeamID>,
    /// `None` when the object has no AI.
    ignored_obstacle: Option<ObjectID>,
    dead: bool,
    destroyed: bool,
    above: bool,
    center: Coord3D,
    sphere_r: Real,
    circle_r: Real,
    dir: (Real, Real),
}

struct OtherView {
    contained_by: Option<ObjectID>,
    parachuting: bool,
    unmanned: bool,
    immobile: bool,
    structure: bool,
    /// `None` when the object has no AI.
    ignored_obstacle: Option<ObjectID>,
    /// `None` when the object has no physics.
    physics_ignore: Option<ObjectID>,
    center: Coord3D,
    sphere_r: Real,
    circle_r: Real,
}

fn self_view(obj: &GameObject) -> SelfView {
    let pos = *obj.get_position();
    let geom = obj.get_geometry_info();
    SelfView {
        contained_by: obj.get_contained_by(),
        pos,
        parachuting: obj.test_status(ObjectStatusTypes::Parachuting),
        infantry: obj.is_kind_of(KindOf::Infantry),
        vehicle: obj.is_kind_of(KindOf::Vehicle),
        name: obj.get_name().clone(),
        team: obj.get_team(),
        ignored_obstacle: obj
            .get_ai()
            .map(AIUpdateInterface::get_ignored_obstacle_id),
        dead: obj.is_effectively_dead(),
        destroyed: obj.is_destroyed(),
        above: obj.is_above_terrain(),
        center: geom.get_center_position(&pos),
        sphere_r: geom.get_bounding_sphere_radius(),
        circle_r: geom.get_bounding_circle_radius(),
        dir: obj.get_unit_direction_vector_2d(),
    }
}

fn other_view(obj: &GameObject) -> OtherView {
    let pos = *obj.get_position();
    let geom = obj.get_geometry_info();
    OtherView {
        contained_by: obj.get_contained_by(),
        parachuting: obj.test_status(ObjectStatusTypes::Parachuting),
        unmanned: obj.is_disabled_by_type(DisabledType::DisabledUnmanned),
        immobile: obj.is_kind_of(KindOf::Immobile),
        structure: obj.is_kind_of(KindOf::Structure),
        ignored_obstacle: obj
            .get_ai()
            .map(AIUpdateInterface::get_ignored_obstacle_id),
        physics_ignore: obj
            .get_physics()
            .map(PhysicsBehavior::get_ignore_collisions_with),
        center: geom.get_center_position(&pos),
        sphere_r: geom.get_bounding_sphere_radius(),
        circle_r: geom.get_bounding_circle_radius(),
    }
}

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

    let Some(me) = OBJECT_REGISTRY.with_object(object_id, self_view) else {
        return;
    };

    // other == null means collide with ground.
    if other_id == crate::common::INVALID_ID {
        if let Some(container_id) = me.contained_by {
            let pos = me.pos;
            let normal = Coord3D::new(0.0, 0.0, -1.0);
            // Self checkout has ended. Touch the container alone.
            let _ = OBJECT_REGISTRY.with_object_mut(container_id, |container| {
                container.on_collide(None, &pos, &normal);
            });
        }
        return;
    }

    let Some(them) = OBJECT_REGISTRY.with_object(other_id, other_view) else {
        return;
    };

    if them.contained_by == Some(object_id) || me.contained_by == Some(other_id) {
        return;
    }

    if me.parachuting && them.parachuting {
        return;
    }

    if handle.is_ignoring_collisions_with(other_id) {
        return;
    }

    if me.ignored_obstacle == Some(other_id) {
        // Infantry walking into an unmanned vehicle: recrew it.
        if me.infantry && them.unmanned {
            let _ = OBJECT_REGISTRY.with_object_mut(other_id, |other| {
                other.clear_disabled(DisabledType::DisabledUnmanned);
                other.set_captured(true);
                other.defect(me.team, 0);
            });
            let _ = crate::scripting::engine::transfer_object_name(&me.name, other_id);
            let _ = TheGameLogic::destroy_object_by_id(object_id);
        }
        return;
    }

    if them.ignored_obstacle == Some(object_id) {
        return;
    }
    if let Some(ignore) = them.physics_ignore {
        if ignore == object_id {
            return;
        }
    } else if !them.immobile {
        return;
    }

    // Crush reads the crusher and mutates the crushee together. Fields cannot
    // be copied first: `can_crush_or_squish` needs both live objects. The two
    // ids are checked out only for that call, then both are released.
    let overlap = OBJECT_REGISTRY.with_object(object_id, |obj| {
        OBJECT_REGISTRY.with_object_mut(other_id, |other| {
            check_for_overlap_collision(handle, obj, other)
        })
    });
    match overlap {
        Some(Some(false)) => {}
        _ => return,
    }

    // Crushee may have taken damage. Re-read it alone before bounce math.
    let Some(them) = OBJECT_REGISTRY.with_object(other_id, other_view) else {
        return;
    };

    // C++ may refuse bounce via AI::processCollision. This port has no
    // process_collision hook, so dead/parachuting vs immobile still bounce.
    let _ = me.dead;

    let mut delta = Coord3D::new(
        them.center.x - me.center.x,
        them.center.y - me.center.y,
        them.center.z - me.center.z,
    );
    let (us_radius, them_radius, dist_sqr) = if me.above {
        (
            me.sphere_r,
            them.sphere_r,
            delta.x * delta.x + delta.y * delta.y + delta.z * delta.z,
        )
    } else {
        delta.z = 0.0;
        (
            me.circle_r,
            them.circle_r,
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

    let cargo_extra = handle.lookup_cargo_mass();
    let mut factor;
    if them.immobile && !me.destroyed {
        if me.parachuting {
            let mut bounce_id = object_id;
            let mut walk = me.contained_by;
            while let Some(container_id) = walk {
                bounce_id = container_id;
                walk = OBJECT_REGISTRY
                    .with_object(container_id, |container| container.get_contained_by())
                    .flatten();
            }
            let bounce_out = us_radius * 0.1;
            let _ = OBJECT_REGISTRY.with_object_mut(bounce_id, |bounce| {
                let mut tmp = *bounce.get_position();
                tmp.x -= bounce_out * delta.x / dist;
                tmp.y -= bounce_out * delta.y / dist;
                let _ = bounce.set_position(&tmp);
                if bounce_id != object_id {
                    if let Some(phys) = bounce.get_physics_mut() {
                        phys.scrub_velocity_2d(0.0);
                    }
                }
            });
            if bounce_id == object_id {
                // Handle is this object's physics. Do not checkout it again.
                PhysicsBehavior::scrub_velocity_2d(handle, 0.0);
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
        factor = -mag * (handle.state.mass + cargo_extra) * stiffness;

        let rubble_h = global_data::read_safe()
            .map(|data| data.default_structure_rubble_height)
            .unwrap_or(1.0);
        if delta.z < 0.0 && me.pos.z >= rubble_h {
            if them.structure {
                if me.vehicle {
                    let _ = OBJECT_REGISTRY.with_object(object_id, |obj| {
                        fire_crash_weapon(
                            module_data
                                .vehicle_crashes_into_building_weapon_template
                                .as_str(),
                            obj,
                        );
                    });
                }
                let _ = TheGameLogic::destroy_object_by_id(object_id);
                return;
            } else if me.vehicle {
                let _ = OBJECT_REGISTRY.with_object(object_id, |obj| {
                    fire_crash_weapon(
                        module_data
                            .vehicle_crashes_into_non_building_weapon_template
                            .as_str(),
                        obj,
                    );
                });
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
        handle.apply_force_with_facing(&force, Some(me.dir), cargo_extra);
    }
}

fn fire_crash_weapon(template_name: &str, source: &GameObject) {
    if template_name.is_empty() {
        return;
    }
    let pos = *source.get_position();
    let _ = TheWeaponStore::create_and_fire_temp_weapon(template_name, source, &pos);
}
