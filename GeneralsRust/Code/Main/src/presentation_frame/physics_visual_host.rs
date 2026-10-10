//! Host freeze + per-draw calc for C++ `Drawable::applyPhysicsXform`.
//!
//! Facts are collected at the presentation-frame boundary (previous logic
//! frame) and the calc mutates persistent loco state on each present.

use super::physics_visual_host_inputs::{
    ObjectVisualIni, host_geometry_radii, host_kindof_token, object_visual_ini,
    terrain_normal_zup_from_height_samples,
};
use crate::game_logic::{
    GameLogic, KindOf, LocomotorAppearance, Object, ObjectId, PhysicsTurningType,
};
use game_client::physics_visual::{
    ClientVisualRng, LiveClientRng, LocomotorVisualParams, OverlapVisualTarget,
    PhysicsVisualAppearance, PhysicsVisualBody, PhysicsVisualInput, PhysicsVisualLocoState,
    calc_physics_visual_xform, glam_yup_physics_visual_local,
};
use game_engine::common::ini::get_global_data;
use glam::Mat4;
use std::collections::HashMap;

/// C++ `isSignificantlyAboveTerrain` with default gravity -1 → threshold 9.
const SIGNIFICANTLY_ABOVE: f32 = 9.0;

/// Frozen per-object facts for one presentation frame (C++ Z-up inside calc).
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// Immutable facts captured at a host presentation boundary. The ordinal is
/// allocated by its game owner; constructing another game cannot publish it.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrozenHostPhysicsVisuals {
    pub(super) origin: std::sync::Arc<()>,
    pub ordinal: u64,
    pub world_epoch: u64,
    pub frozen: bool,
    pub objects: HashMap<ObjectId, FrozenHostPhysicsObject>,
}

impl PartialEq for FrozenHostPhysicsVisuals {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.origin, &other.origin)
            && self.ordinal == other.ordinal
            && self.world_epoch == other.world_epoch
            && self.frozen == other.frozen
            && self.objects == other.objects
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrozenHostPhysicsObject {
    pub generation: u64,
    pub facts: HostPhysicsVisualFacts,
}

#[derive(Debug, Default)]
pub(crate) struct HostPhysicsVisualState {
    pub(super) origin: std::sync::Arc<()>,
    next_ordinal: u64,
    active_ordinal: u64,
    world_epoch: u64,
    loco: HashMap<ObjectId, (u64, PhysicsVisualLocoState)>,
    pub(super) schedule: super::host_draw_schedule::HostDrawSchedule,
}

impl HostPhysicsVisualState {
    pub(crate) fn freeze(&mut self, logic: &GameLogic) -> FrozenHostPhysicsVisuals {
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .expect("host physics frame ordinal exhausted");
        self.loco.retain(|id, (generation, _)| {
            logic
                .host_objects()
                .get(id)
                .is_some_and(|object| object.visual_object_generation == *generation)
        });
        let script_frozen = logic.is_script_time_frozen();
        let camera_frozen = logic.is_script_camera_time_frozen();
        // Drawable::applyPhysicsXform gates each drawable on this global option.
        // The host freezes presentation facts per frame, so sample on the first
        // eligible object and reuse that value for the rest of the frame. Keep
        // this lazy: frames with no visible physics drawable do not read it.
        let mut show_client_physics = None;
        FrozenHostPhysicsVisuals {
            origin: std::sync::Arc::clone(&self.origin),
            ordinal: self.next_ordinal,
            world_epoch: logic.host_visual_world_epoch(),
            frozen: script_frozen || camera_frozen,
            objects: logic
                .host_objects()
                .iter()
                .filter_map(|(id, object)| {
                    if object.drawable_hidden {
                        return None;
                    }
                    let facts = collect_facts(
                        object,
                        logic.host_objects(),
                        script_frozen,
                        camera_frozen,
                        &mut show_client_physics,
                        |pos| logic.terrain_height_at(pos),
                    )?;
                    Some((
                        *id,
                        FrozenHostPhysicsObject {
                            generation: object.visual_object_generation,
                            facts,
                        },
                    ))
                })
                .collect(),
        }
    }

    fn begin(&mut self, frame: &FrozenHostPhysicsVisuals) -> bool {
        // The live renderer only installs its latest completed frame. An expired
        // snapshot cannot rewind retained visual state or consume client RNG.
        if !std::sync::Arc::ptr_eq(&self.origin, &frame.origin) {
            return false;
        }
        if frame.ordinal < self.next_ordinal || frame.ordinal < self.active_ordinal {
            return false;
        }
        if frame.ordinal == self.active_ordinal {
            return true;
        }
        self.active_ordinal = frame.ordinal;
        if self.world_epoch != frame.world_epoch {
            self.loco.clear();
        }
        self.world_epoch = frame.world_epoch;
        self.schedule
            .begin_presented_frame(super::host_draw_schedule::HostPresentVisualInput {
                visual_dt_ms: if frame.frozen {
                    0
                } else {
                    super::host_draw_schedule::HOST_VISUAL_FRAME_MS
                },
                frozen: frame.frozen,
            });
        true
    }

    pub(crate) fn complete_input(
        &mut self,
        input: &mut super::UnitRenderInput,
        frame: &super::PresentationFrame,
    ) {
        input.physics_visual_local =
            self.local_matrix(&frame.host_physics_visuals, input.id, &mut LiveClientRng);
    }

    pub(super) fn local_matrix(
        &mut self,
        frame: &FrozenHostPhysicsVisuals,
        id: ObjectId,
        rng: &mut impl ClientVisualRng,
    ) -> Option<Mat4> {
        if !self.begin(frame) {
            return None;
        }
        let object = frame.objects.get(&id)?;
        let facts = object.facts;
        let gates = PhysicsVisualInput {
            has_object: facts.body.has_object,
            object_disabled_held: facts.object_disabled_held,
            show_client_physics: facts.show_client_physics,
            tactical_view_time_frozen: facts.tactical_view_time_frozen,
            camera_movement_finished: facts.camera_movement_finished,
            script_time_frozen_debug: facts.script_time_frozen_debug,
            script_time_frozen_script: facts.script_time_frozen_script,
            calculated_xform: None,
        };
        if !gates.permits_application() {
            return None;
        }
        if let Some(matrix) = self.schedule.cached_applied_matrix(id) {
            return Some(matrix);
        }
        if !self.schedule.should_calc_loco(id) {
            return None;
        }
        let mut loco = self
            .loco
            .get(&id)
            .filter(|(generation, _)| *generation == object.generation)
            .map(|(_, loco)| *loco)
            .unwrap_or_default();
        let xform = calc_physics_visual_xform(
            facts.appearance,
            &mut loco,
            &facts.params,
            &facts.body,
            rng,
        )?;
        self.loco.insert(id, (object.generation, loco));
        let local = glam_yup_physics_visual_local(xform);
        self.schedule.note_loco_applied(id, local);
        Some(local)
    }

    #[cfg(test)]
    pub(super) fn loco_state(&self, id: ObjectId) -> PhysicsVisualLocoState {
        self.loco
            .get(&id)
            .map(|(_, loco)| *loco)
            .unwrap_or_default()
    }
}

pub(super) fn collect_facts(
    obj: &Object,
    objects: &std::collections::HashMap<ObjectId, Object>,
    script_time_frozen: bool,
    script_camera_time_frozen: bool,
    show_client_physics: &mut Option<bool>,
    sample_height: impl Fn(glam::Vec3) -> Option<f32>,
) -> Option<HostPhysicsVisualFacts> {
    let appearance = map_appearance(obj.loco_appearance);
    if !appearance.has_physics_xform() {
        return None;
    }
    let params = params_for_object(obj);
    let body = body_for_object(obj, objects, sample_height, object_visual_ini);
    let show_client_physics = cached_show_client_physics(show_client_physics, || {
        get_global_data()
            .map(|data| data.read().show_client_physics)
            .unwrap_or(true)
    });
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

pub(super) fn cached_show_client_physics(
    cached: &mut Option<bool>,
    read: impl FnOnce() -> bool,
) -> bool {
    *cached.get_or_insert_with(read)
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

/// Shared body sampling uses explicit immutable content lookup. Production
/// supplies the current asset adapter; tests supply local authored definitions.
pub(super) fn body_for_object(
    obj: &Object,
    objects: &std::collections::HashMap<ObjectId, Object>,
    sample_height: impl Fn(glam::Vec3) -> Option<f32>,
    definition: impl Fn(&str) -> ObjectVisualIni,
) -> PhysicsVisualBody {
    let pos = obj.get_position();
    let vel = obj.movement.velocity;
    let accel = obj.previous_acceleration();
    let dir = obj.unit_direction_vector_2d();
    let self_ini = definition(&obj.template_name);
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
            let other_ini = definition(&other.template_name);
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
