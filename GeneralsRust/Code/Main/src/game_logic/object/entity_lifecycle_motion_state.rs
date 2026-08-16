//! Locomotor + Turret inventory. Not a complete C++ Locomotor xfer
//! (Main lacks donut/maxSpeed/maxAccel/maxTurnRate/closeEnoughDist).
//! C++: Locomotor.cpp:722-749, TurretAI.cpp:319-353.

use super::entity_lifecycle_inventory::{decode_payload, push_present};
use super::entity_lifecycle_tags::*;
use super::{LocomotorAppearance, LocomotorBehaviorZ, Object, TurretSubState};
use crate::game_logic::ObjectId;
use gamelogic::world::entities::{EntityLifecycleCodecError, EntityModuleState};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct LocomotorResidual {
    pub surfaces: u32,
    pub upgrade: bool,
    pub maintain_pos: Option<Vec3>,
    pub maintain_pos_valid: bool,
    pub braking_factor: f32,
    pub max_lift: f32,
    pub braking: f32,
    pub preferred_height: f32,
    pub preferred_height_damping: f32,
    pub wander_angle_offset: f32,
    pub wander_offset_increment: f32,
    pub is_braking: bool,
    pub ultra_accurate: bool,
    pub moving_backwards: bool,
    pub over_water: bool,
    pub appearance: LocomotorAppearance,
    pub behavior_z: LocomotorBehaviorZ,
    pub can_move_backward: bool,
    pub no_slow_down_as_approaching_dest: bool,
    pub turn_pivot_offset: f32,
    pub wander_width_factor: f32,
    pub wander_offset_increasing: bool,
    pub downhill_only: bool,
    pub precise_z_pos: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct TurretResidual {
    pub enabled: bool,
    pub angle_deg: f32,
    pub pitch_deg: f32,
    pub substate: TurretSubState,
    pub rotating: bool,
    pub holding: bool,
    pub hold_until_frame: u32,
    pub idle_scanning: bool,
    pub idle_scan_next_frame: u32,
    pub idle_scan_desired_angle_deg: f32,
    pub idle_scan_index: u32,
    pub idle_recentering: bool,
    pub mood_target: bool,
    pub target_id: Option<ObjectId>,
    pub force_attacking: bool,
    pub turn_rate_rad: f32,
    pub natural_angle_deg: f32,
    pub natural_pitch_deg: f32,
    pub recenter_frames: u32,
}

impl LocomotorResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.locomotor_upgrade
            || object.is_braking
            || object.maintain_pos_valid
            || object.maintain_pos.is_some()
            || object.moving_backwards
            || object.ultra_accurate
            || object.over_water
            || object.locomotor_surfaces != 0
            || object.loco_preferred_height != 0.0
            || object.wander_width_factor != 0.0
            || object.can_move_backward
            || object.no_slow_down_as_approaching_dest
            || object.precise_z_pos
            || object.downhill_only
            || (object.braking_factor - 1.0).abs() > f32::EPSILON
            || object.max_lift != 0.0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            surfaces: object.locomotor_surfaces,
            upgrade: object.locomotor_upgrade,
            maintain_pos: object.maintain_pos,
            maintain_pos_valid: object.maintain_pos_valid,
            braking_factor: object.braking_factor,
            max_lift: object.max_lift,
            braking: object.braking,
            preferred_height: object.loco_preferred_height,
            preferred_height_damping: object.loco_preferred_height_damping,
            wander_angle_offset: object.wander_angle_offset,
            wander_offset_increment: object.wander_offset_increment,
            is_braking: object.is_braking,
            ultra_accurate: object.ultra_accurate,
            moving_backwards: object.moving_backwards,
            over_water: object.over_water,
            appearance: object.loco_appearance,
            behavior_z: object.loco_behavior_z,
            can_move_backward: object.can_move_backward,
            no_slow_down_as_approaching_dest: object.no_slow_down_as_approaching_dest,
            turn_pivot_offset: object.turn_pivot_offset,
            wander_width_factor: object.wander_width_factor,
            wander_offset_increasing: object.wander_offset_increasing,
            downhill_only: object.downhill_only,
            precise_z_pos: object.precise_z_pos,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.locomotor_surfaces = self.surfaces;
        object.locomotor_upgrade = self.upgrade;
        object.maintain_pos = self.maintain_pos;
        object.maintain_pos_valid = self.maintain_pos_valid;
        object.braking_factor = self.braking_factor;
        object.max_lift = self.max_lift;
        object.braking = self.braking;
        object.loco_preferred_height = self.preferred_height;
        object.loco_preferred_height_damping = self.preferred_height_damping;
        object.wander_angle_offset = self.wander_angle_offset;
        object.wander_offset_increment = self.wander_offset_increment;
        object.is_braking = self.is_braking;
        object.ultra_accurate = self.ultra_accurate;
        object.moving_backwards = self.moving_backwards;
        object.over_water = self.over_water;
        object.loco_appearance = self.appearance;
        object.loco_behavior_z = self.behavior_z;
        object.can_move_backward = self.can_move_backward;
        object.no_slow_down_as_approaching_dest = self.no_slow_down_as_approaching_dest;
        object.turn_pivot_offset = self.turn_pivot_offset;
        object.wander_width_factor = self.wander_width_factor;
        object.wander_offset_increasing = self.wander_offset_increasing;
        object.downhill_only = self.downhill_only;
        object.precise_z_pos = self.precise_z_pos;
    }
}

impl TurretResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.turret_enabled
            || object.turret_target_id.is_some()
            || object.turret_substate != TurretSubState::Idle
            || object.turret_rotating
            || object.turret_holding
            || object.turret_idle_scanning
            || object.turret_mood_target
            || object.turret_force_attacking
            || object.turret_idle_recentering
            || object.turret_hold_until_frame != 0
            || object.turret_idle_scan_next_frame != 0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            enabled: object.turret_enabled,
            angle_deg: object.turret_angle_deg,
            pitch_deg: object.turret_pitch_deg,
            substate: object.turret_substate,
            rotating: object.turret_rotating,
            holding: object.turret_holding,
            hold_until_frame: object.turret_hold_until_frame,
            idle_scanning: object.turret_idle_scanning,
            idle_scan_next_frame: object.turret_idle_scan_next_frame,
            idle_scan_desired_angle_deg: object.turret_idle_scan_desired_angle_deg,
            idle_scan_index: object.turret_idle_scan_index,
            idle_recentering: object.turret_idle_recentering,
            mood_target: object.turret_mood_target,
            target_id: object.turret_target_id,
            force_attacking: object.turret_force_attacking,
            turn_rate_rad: object.turret_turn_rate_rad,
            natural_angle_deg: object.turret_natural_angle_deg,
            natural_pitch_deg: object.turret_natural_pitch_deg,
            recenter_frames: object.turret_recenter_frames,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.turret_enabled = self.enabled;
        object.turret_angle_deg = self.angle_deg;
        object.turret_pitch_deg = self.pitch_deg;
        object.turret_substate = self.substate;
        object.turret_rotating = self.rotating;
        object.turret_holding = self.holding;
        object.turret_hold_until_frame = self.hold_until_frame;
        object.turret_idle_scanning = self.idle_scanning;
        object.turret_idle_scan_next_frame = self.idle_scan_next_frame;
        object.turret_idle_scan_desired_angle_deg = self.idle_scan_desired_angle_deg;
        object.turret_idle_scan_index = self.idle_scan_index;
        object.turret_idle_recentering = self.idle_recentering;
        object.turret_mood_target = self.mood_target;
        object.turret_target_id = self.target_id;
        object.turret_force_attacking = self.force_attacking;
        object.turret_turn_rate_rad = self.turn_rate_rad;
        object.turret_natural_angle_deg = self.natural_angle_deg;
        object.turret_natural_pitch_deg = self.natural_pitch_deg;
        object.turret_recenter_frames = self.recenter_frames;
    }
}

pub(crate) fn collect_locomotor(
    object: &Object,
    out: &mut Vec<EntityModuleState>,
) -> Result<(), EntityLifecycleCodecError> {
    push_present(
        out,
        TAG_LOCOMOTOR,
        LocomotorResidual::present(object),
        &LocomotorResidual::from_object(object),
    )
}

pub(crate) fn collect_turret(
    object: &Object,
    out: &mut Vec<EntityModuleState>,
) -> Result<(), EntityLifecycleCodecError> {
    push_present(
        out,
        TAG_TURRET,
        TurretResidual::present(object),
        &TurretResidual::from_object(object),
    )
}

pub(crate) fn apply(
    object: &mut Object,
    module: &EntityModuleState,
) -> Result<bool, EntityLifecycleCodecError> {
    let payload = module.payload.as_slice();
    match module.tag.as_str() {
        TAG_LOCOMOTOR => decode_payload::<LocomotorResidual>(payload)?.apply(object),
        TAG_TURRET => decode_payload::<TurretResidual>(payload)?.apply(object),
        _ => return Ok(false),
    }
    Ok(true)
}
