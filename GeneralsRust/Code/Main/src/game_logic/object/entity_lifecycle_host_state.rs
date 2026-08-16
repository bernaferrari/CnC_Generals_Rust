//! Main-only TODO groups: radar / door / rebuild / supply / shock / motive.
//!
//! Inventory of existing Object fields. Not Entity Xfer authority.
//! C++: RadarUpdate.cpp:117-136, RebuildHoleBehavior.cpp:382-433,
//! SupplyTruckAIUpdate.cpp:242-255, PhysicsUpdate.cpp:1812 (IS_STUNNED / motive).

use super::Object;
use super::entity_lifecycle_inventory::{decode_payload, push_present};
use super::entity_lifecycle_tags::*;
use crate::game_logic::{ObjectId, SupplyTruckState};
use gamelogic::world::entities::{EntityLifecycleCodecError, EntityModuleState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct RadarExtendResidual {
    pub done_frame: u32,
    pub complete: bool,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct ProductionDoorResidual {
    pub phase: u8,
    pub phase_end_frame: u32,
    pub hold_open: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct RebuildHoleResidual {
    pub is_rebuild_hole: bool,
    pub template_name: Option<String>,
    pub ready_frame: u32,
    pub spawner_id: Option<ObjectId>,
    pub worker_id: Option<ObjectId>,
    pub reconstructing_id: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct SupplyTruckResidual {
    pub state: SupplyTruckState,
    pub force_pending: bool,
    pub next_dock_action_frame: u32,
    pub preferred_dock_id: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct ShockStunResidual {
    pub frames: u32,
    pub yaw_rate: f32,
    pub pitch_rate: f32,
    pub roll_rate: f32,
    pub allow_bounce: bool,
    pub was_airborne: bool,
    pub grounded_once: bool,
    pub up_z: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct PhysicsMotiveResidual {
    pub frames_remaining: u32,
}

impl RadarExtendResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.radar_extend_done_frame != 0 || object.radar_extend_complete || object.radar_active
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            done_frame: object.radar_extend_done_frame,
            complete: object.radar_extend_complete,
            active: object.radar_active,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.radar_extend_done_frame = self.done_frame;
        object.radar_extend_complete = self.complete;
        object.radar_active = self.active;
    }
}

impl ProductionDoorResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.production_door_phase != 0
            || object.production_door_phase_end_frame != 0
            || object.production_door_hold_open
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            phase: object.production_door_phase,
            phase_end_frame: object.production_door_phase_end_frame,
            hold_open: object.production_door_hold_open,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.production_door_phase = self.phase;
        object.production_door_phase_end_frame = self.phase_end_frame;
        object.production_door_hold_open = self.hold_open;
    }
}

impl RebuildHoleResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.is_rebuild_hole
            || object.rebuild_template_name.is_some()
            || object.rebuild_ready_frame != 0
            || object.rebuild_spawner_id.is_some()
            || object.rebuild_worker_id.is_some()
            || object.rebuild_reconstructing_id.is_some()
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            is_rebuild_hole: object.is_rebuild_hole,
            template_name: object.rebuild_template_name.clone(),
            ready_frame: object.rebuild_ready_frame,
            spawner_id: object.rebuild_spawner_id,
            worker_id: object.rebuild_worker_id,
            reconstructing_id: object.rebuild_reconstructing_id,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.is_rebuild_hole = self.is_rebuild_hole;
        object.rebuild_template_name = self.template_name;
        object.rebuild_ready_frame = self.ready_frame;
        object.rebuild_spawner_id = self.spawner_id;
        object.rebuild_worker_id = self.worker_id;
        object.rebuild_reconstructing_id = self.reconstructing_id;
    }
}

impl SupplyTruckResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.supply_truck_state != SupplyTruckState::Idle
            || object.supply_truck_force_pending
            || object.supply_truck_next_dock_action_frame != 0
            || object.preferred_dock_id.is_some()
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            state: object.supply_truck_state,
            force_pending: object.supply_truck_force_pending,
            next_dock_action_frame: object.supply_truck_next_dock_action_frame,
            preferred_dock_id: object.preferred_dock_id,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.supply_truck_state = self.state;
        object.supply_truck_force_pending = self.force_pending;
        object.supply_truck_next_dock_action_frame = self.next_dock_action_frame;
        object.preferred_dock_id = self.preferred_dock_id;
    }
}

impl ShockStunResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.shock_stun_frames != 0
            || object.shock_yaw_rate != 0.0
            || object.shock_pitch_rate != 0.0
            || object.shock_roll_rate != 0.0
            || object.shock_allow_bounce
            || object.shock_was_airborne
            || object.shock_grounded_once
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            frames: object.shock_stun_frames,
            yaw_rate: object.shock_yaw_rate,
            pitch_rate: object.shock_pitch_rate,
            roll_rate: object.shock_roll_rate,
            allow_bounce: object.shock_allow_bounce,
            was_airborne: object.shock_was_airborne,
            grounded_once: object.shock_grounded_once,
            up_z: object.shock_up_z,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.shock_stun_frames = self.frames;
        object.shock_yaw_rate = self.yaw_rate;
        object.shock_pitch_rate = self.pitch_rate;
        object.shock_roll_rate = self.roll_rate;
        object.shock_allow_bounce = self.allow_bounce;
        object.shock_was_airborne = self.was_airborne;
        object.shock_grounded_once = self.grounded_once;
        object.shock_up_z = self.up_z;
    }
}

impl PhysicsMotiveResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.motive_frames_remaining != 0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            frames_remaining: object.motive_frames_remaining,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.motive_frames_remaining = self.frames_remaining;
    }
}

pub(crate) fn collect(
    object: &Object,
    out: &mut Vec<EntityModuleState>,
) -> Result<(), EntityLifecycleCodecError> {
    push_present(
        out,
        TAG_RADAR_EXTEND,
        RadarExtendResidual::present(object),
        &RadarExtendResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_PRODUCTION_DOOR,
        ProductionDoorResidual::present(object),
        &ProductionDoorResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_REBUILD_HOLE,
        RebuildHoleResidual::present(object),
        &RebuildHoleResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_SUPPLY_TRUCK,
        SupplyTruckResidual::present(object),
        &SupplyTruckResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_SHOCK_STUN,
        ShockStunResidual::present(object),
        &ShockStunResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_PHYSICS_MOTIVE,
        PhysicsMotiveResidual::present(object),
        &PhysicsMotiveResidual::from_object(object),
    )
}

pub(crate) fn apply(
    object: &mut Object,
    module: &EntityModuleState,
) -> Result<bool, EntityLifecycleCodecError> {
    let payload = module.payload.as_slice();
    match module.tag.as_str() {
        TAG_RADAR_EXTEND => decode_payload::<RadarExtendResidual>(payload)?.apply(object),
        TAG_PRODUCTION_DOOR => decode_payload::<ProductionDoorResidual>(payload)?.apply(object),
        TAG_REBUILD_HOLE => decode_payload::<RebuildHoleResidual>(payload)?.apply(object),
        TAG_SUPPLY_TRUCK => decode_payload::<SupplyTruckResidual>(payload)?.apply(object),
        TAG_SHOCK_STUN => decode_payload::<ShockStunResidual>(payload)?.apply(object),
        TAG_PHYSICS_MOTIVE => decode_payload::<PhysicsMotiveResidual>(payload)?.apply(object),
        _ => return Ok(false),
    }
    Ok(true)
}
