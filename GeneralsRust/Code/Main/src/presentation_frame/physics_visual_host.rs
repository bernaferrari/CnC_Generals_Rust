//! Host freeze + per-draw calc for C++ `Drawable::applyPhysicsXform`.
//!
//! Facts are collected at the presentation-frame boundary (previous logic
//! frame) and the calc mutates persistent loco state on each present.

use super::physics_visual_host_inputs::{
    host_geometry_radii, host_kindof_token, object_visual_ini,
    terrain_normal_zup_from_height_samples,
};
use crate::game_logic::{
    GameLogic, KindOf, LocomotorAppearance, Object, ObjectId, PhysicsTurningType,
};
use game_client::physics_visual::{
    LiveClientRng, LocomotorVisualParams, OverlapVisualTarget, PhysicsVisualAppearance,
    PhysicsVisualBody, PhysicsVisualInput, PhysicsVisualLocoState, calc_physics_visual_xform,
    glam_yup_physics_visual_local,
};
use game_engine::common::ini::get_global_data;
use glam::Mat4;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;

/// C++ `isSignificantlyAboveTerrain` with default gravity -1 → threshold 9.
const SIGNIFICANTLY_ABOVE: f32 = 9.0;

static FACTS: Lazy<Mutex<HashMap<u32, HostPhysicsVisualFacts>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static LOCO: Lazy<Mutex<HashMap<u32, PhysicsVisualLocoState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Frozen per-object facts for one presentation frame (C++ Z-up inside calc).
#[derive(Debug, Clone, Copy)]
pub struct HostPhysicsVisualFacts {
    pub appearance: PhysicsVisualAppearance,
    pub params: LocomotorVisualParams,
    pub body: PhysicsVisualBody,
    pub object_disabled_held: bool,
    pub show_client_physics: bool,
    pub tactical_view_time_frozen: bool,
    pub camera_movement_finished: bool,
    pub script_time_frozen_debug: bool,
    pub script_time_frozen_script: bool,
}

pub fn freeze_for_object(
    obj: &Object,
    objects: &std::collections::HashMap<ObjectId, Object>,
    script_time_frozen: bool,
    script_camera_time_frozen: bool,
    logic: &GameLogic,
) {
    let Some(facts) = collect_facts(
        obj,
        objects,
        script_time_frozen,
        script_camera_time_frozen,
        |pos| logic.terrain_height_at(pos),
    ) else {
        FACTS.lock().remove(&obj.id.0);
        return;
    };
    FACTS.lock().insert(obj.id.0, facts);
}

/// Run gates + calc + Y-up post-multiply on a host world matrix.
#[must_use]
pub fn apply_to_world_matrix(id: ObjectId, base: Mat4) -> Mat4 {
    let Some(facts) = FACTS.lock().get(&id.0).copied() else {
        return base;
    };
    let input_gates = PhysicsVisualInput {
        has_object: facts.body.has_object,
        object_disabled_held: facts.object_disabled_held,
        show_client_physics: facts.show_client_physics,
        tactical_view_time_frozen: facts.tactical_view_time_frozen,
        camera_movement_finished: facts.camera_movement_finished,
        script_time_frozen_debug: facts.script_time_frozen_debug,
        script_time_frozen_script: facts.script_time_frozen_script,
        calculated_xform: None,
    };
    if !input_gates.permits_application() {
        return base;
    }
    if let Some(cached) = super::host_draw_schedule::cached_applied_matrix(id) {
        return cached;
    }
    if !super::host_draw_schedule::should_calc_loco(id) {
        return base;
    }

    let mut loco = LOCO
        .lock()
        .entry(id.0)
        .or_insert_with(PhysicsVisualLocoState::default)
        .clone();
    let mut rng = LiveClientRng;
    let Some(xform) = calc_physics_visual_xform(
        facts.appearance,
        &mut loco,
        &facts.params,
        &facts.body,
        &mut rng,
    ) else {
        return base;
    };
    LOCO.lock().insert(id.0, loco);
    let applied = base * glam_yup_physics_visual_local(xform);
    super::host_draw_schedule::note_loco_applied(id, applied);
    applied
}

#[cfg(test)]
pub fn reset_host_physics_visual_state() {
    FACTS.lock().clear();
    LOCO.lock().clear();
    super::host_draw_schedule::reset_host_present_schedule();
}

#[cfg(test)]
pub fn loco_state(id: ObjectId) -> PhysicsVisualLocoState {
    LOCO.lock().get(&id.0).copied().unwrap_or_default()
}

#[cfg(test)]
pub fn insert_facts_for_test(id: ObjectId, facts: HostPhysicsVisualFacts) {
    FACTS.lock().insert(id.0, facts);
}

fn collect_facts(
    obj: &Object,
    objects: &std::collections::HashMap<ObjectId, Object>,
    script_time_frozen: bool,
    script_camera_time_frozen: bool,
    sample_height: impl Fn(glam::Vec3) -> Option<f32>,
) -> Option<HostPhysicsVisualFacts> {
    let appearance = map_appearance(obj.loco_appearance);
    if !appearance.has_physics_xform() {
        return None;
    }
    let params = params_for_object(obj);
    let body = body_for_object(obj, objects, sample_height);
    let show_client_physics = get_global_data()
        .map(|data| data.read().show_client_physics)
        .unwrap_or(true);
    Some(HostPhysicsVisualFacts {
        appearance,
        params,
        body,
        object_disabled_held: obj.contained_by.is_some(),
        show_client_physics,
        tactical_view_time_frozen: script_camera_time_frozen,
        camera_movement_finished: true,
        script_time_frozen_debug: false,
        script_time_frozen_script: script_time_frozen,
    })
}

fn map_appearance(appearance: LocomotorAppearance) -> PhysicsVisualAppearance {
    match appearance {
        LocomotorAppearance::Other => PhysicsVisualAppearance::Other,
        LocomotorAppearance::LegsTwo => PhysicsVisualAppearance::LegsTwo,
        LocomotorAppearance::WheelsFour => PhysicsVisualAppearance::WheelsFour,
        LocomotorAppearance::Treads => PhysicsVisualAppearance::Treads,
        LocomotorAppearance::Hover => PhysicsVisualAppearance::Hover,
        LocomotorAppearance::Wings => PhysicsVisualAppearance::Wings,
        LocomotorAppearance::Thrust => PhysicsVisualAppearance::Thrust,
        LocomotorAppearance::Motorcycle => PhysicsVisualAppearance::Motorcycle,
        LocomotorAppearance::Climber => PhysicsVisualAppearance::Climber,
    }
}

fn params_for_object(obj: &Object) -> LocomotorVisualParams {
    let Some(name) =
        crate::game_logic::locomotor_bootstrap::locomotor_name_for_unit(&obj.template_name)
    else {
        return LocomotorVisualParams::default();
    };
    let store = game_engine::common::ini::ini_locomotor::get_locomotor_store();
    let Some(template) = store.find_template(name) else {
        return LocomotorVisualParams::default();
    };
    LocomotorVisualParams {
        accel_pitch_limit: template.accel_pitch_limit,
        decel_pitch_limit: template.decel_pitch_limit,
        bounce_kick: template.bounce_kick,
        pitch_stiffness: template.pitch_stiffness,
        roll_stiffness: template.roll_stiffness,
        pitch_damping: template.pitch_damping,
        roll_damping: template.roll_damping,
        pitch_by_z_vel_coef: template.pitch_by_z_vel_coef,
        thrust_roll: template.thrust_roll,
        wobble_rate: template.wobble_rate,
        min_wobble: template.min_wobble,
        max_wobble: template.max_wobble,
        forward_vel_coef: template.forward_vel_coef,
        lateral_vel_coef: template.lateral_vel_coef,
        forward_accel_coef: template.forward_accel_coef,
        lateral_accel_coef: template.lateral_accel_coef,
        uniform_axial_damping: template.uniform_axial_damping,
        has_suspension: template.has_suspension,
        max_wheel_extension: template.maximum_wheel_extension,
        wheel_turn_angle: template.wheel_turn_angle,
        rudder_correction_degree: template.rudder_correction_degree,
        rudder_correction_rate: template.rudder_correction_rate,
        elevator_correction_degree: template.elevator_correction_degree,
        elevator_correction_rate: template.elevator_correction_rate,
    }
}

fn body_for_object(
    obj: &Object,
    objects: &std::collections::HashMap<ObjectId, Object>,
    sample_height: impl Fn(glam::Vec3) -> Option<f32>,
) -> PhysicsVisualBody {
    let pos = obj.get_position();
    let vel = obj.movement.velocity;
    let accel = obj.previous_acceleration();
    let dir = obj.unit_direction_vector_2d();
    let self_ini = object_visual_ini(&obj.template_name);
    let (major_radius, minor_radius, bounding_circle_radius, _) =
        host_geometry_radii(obj, &self_ini);
    let height = pos.y - obj.ground_height;
    let (terrain_normal_x, terrain_normal_y, terrain_normal_z) =
        terrain_normal_zup_from_height_samples(sample_height, pos);
    let overlap = obj
        .physics_current_overlap
        .and_then(|id| objects.get(&id))
        .map(|other| {
            let other_pos = other.get_position();
            let other_ini = object_visual_ini(&other.template_name);
            let (_, _, other_circle, other_height) = host_geometry_radii(other, &other_ini);
            OverlapVisualTarget {
                is_shrubbery: host_kindof_token(&other_ini, "SHRUBBERY"),
                is_low_overlappable: host_kindof_token(&other_ini, "LOW_OVERLAPPABLE"),
                is_infantry: other.is_kind_of(KindOf::Infantry),
                front_crushed: other.front_crushed,
                back_crushed: other.back_crushed,
                pos_x: other_pos.x,
                pos_y: other_pos.z,
                bounding_circle_radius: other_circle,
                max_height_above_position: other_height,
            }
        });
    PhysicsVisualBody {
        has_object: true,
        has_ai: true,
        has_physics: true,
        dir_x: dir.x,
        dir_y: dir.y,
        vel_x: vel.x,
        vel_y: vel.z,
        vel_z: vel.y,
        accel_x: accel.x,
        accel_y: accel.z,
        accel_z: accel.y,
        velocity_magnitude: vel.length(),
        forward_speed_2d: obj.forward_speed_2d(),
        is_motive: obj.motive_frames_remaining > 0,
        turning: match obj.physics_turning {
            PhysicsTurningType::TurnNegative => -1,
            PhysicsTurningType::TurnNone => 0,
            PhysicsTurningType::TurnPositive => 1,
        },
        cur_locomotor_speed: obj.movement.max_speed,
        pos_x: pos.x,
        pos_y: pos.z,
        pos_z: pos.y,
        terrain_height: obj.ground_height,
        terrain_normal_x,
        terrain_normal_y,
        terrain_normal_z,
        significantly_above_terrain: height > SIGNIFICANTLY_ABOVE,
        major_radius,
        minor_radius,
        bounding_circle_radius,
        current_overlap: overlap,
        previous_overlap_valid: obj.physics_previous_overlap.is_some(),
    }
}

#[cfg(test)]
pub fn body_for_object_with_height_samples(
    obj: &Object,
    objects: &std::collections::HashMap<ObjectId, Object>,
    sample_height: impl Fn(glam::Vec3) -> Option<f32>,
) -> PhysicsVisualBody {
    body_for_object(obj, objects, sample_height)
}
