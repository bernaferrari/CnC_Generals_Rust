//! Main-only TODO groups: status / model / continuous-fire / subdual / overlord.
//!
//! Inventory of existing Object fields. Not Entity Xfer authority.
//! C++: Object.cpp status/model xfer, FiringTracker.cpp:340-359,
//! ActiveBody.cpp subdual, OverlordContain.cpp portable addon residual.

use super::Object;
use super::entity_lifecycle_inventory::{decode_payload, push_present};
use super::entity_lifecycle_tags::*;
use gamelogic::world::entities::{EntityLifecycleCodecError, EntityModuleState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct StatusBitsResidual {
    pub bits: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct ModelConditionResidual {
    pub bits: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct ContinuousFireResidual {
    pub consecutive: u32,
    pub level: u8,
    pub one_shots: u32,
    pub two_shots: u32,
    pub coast_frames: u32,
    pub coast_until_frame: u32,
    pub victim: u32,
    pub auto_reload_when_idle_frames: u32,
    pub frame_to_force_reload: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct SubdualDamageResidual {
    pub damage: f32,
    pub heal_rate_frames: u32,
    pub heal_amount: f32,
    pub heal_countdown: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct OverlordAddonResidual {
    pub gattling: bool,
    pub propaganda: bool,
    pub helix_transport: bool,
    pub bunker_capacity: Option<usize>,
}

impl StatusBitsResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.object_status_bits != 0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            bits: object.object_status_bits,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.object_status_bits = self.bits;
    }
}

impl ModelConditionResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.model_condition_bits != 0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            bits: object.model_condition_bits,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.model_condition_bits = self.bits;
    }
}

impl ContinuousFireResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.continuous_fire_consecutive != 0
            || object.continuous_fire_level != 0
            || object.continuous_fire_coast_until_frame != 0
            || object.continuous_fire_victim != 0
            || object.frame_to_force_reload != 0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            consecutive: object.continuous_fire_consecutive,
            level: object.continuous_fire_level,
            one_shots: object.continuous_fire_one_shots,
            two_shots: object.continuous_fire_two_shots,
            coast_frames: object.continuous_fire_coast_frames,
            coast_until_frame: object.continuous_fire_coast_until_frame,
            victim: object.continuous_fire_victim,
            auto_reload_when_idle_frames: object.auto_reload_when_idle_frames,
            frame_to_force_reload: object.frame_to_force_reload,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.continuous_fire_consecutive = self.consecutive;
        object.continuous_fire_level = self.level;
        object.continuous_fire_one_shots = self.one_shots;
        object.continuous_fire_two_shots = self.two_shots;
        object.continuous_fire_coast_frames = self.coast_frames;
        object.continuous_fire_coast_until_frame = self.coast_until_frame;
        object.continuous_fire_victim = self.victim;
        object.auto_reload_when_idle_frames = self.auto_reload_when_idle_frames;
        object.frame_to_force_reload = self.frame_to_force_reload;
    }
}

impl SubdualDamageResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.subdual_damage != 0.0
            || object.subdual_heal_rate_frames != 0
            || object.subdual_heal_amount != 0.0
            || object.subdual_heal_countdown != 0
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            damage: object.subdual_damage,
            heal_rate_frames: object.subdual_heal_rate_frames,
            heal_amount: object.subdual_heal_amount,
            heal_countdown: object.subdual_heal_countdown,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.subdual_damage = self.damage;
        object.subdual_heal_rate_frames = self.heal_rate_frames;
        object.subdual_heal_amount = self.heal_amount;
        object.subdual_heal_countdown = self.heal_countdown;
    }
}

impl OverlordAddonResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.has_overlord_gattling_addon
            || object.has_overlord_propaganda_addon
            || object.is_helix_transport
            || object.overlord_bunker_capacity.is_some()
    }
    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            gattling: object.has_overlord_gattling_addon,
            propaganda: object.has_overlord_propaganda_addon,
            helix_transport: object.is_helix_transport,
            bunker_capacity: object.overlord_bunker_capacity,
        }
    }
    pub(crate) fn apply(self, object: &mut Object) {
        object.has_overlord_gattling_addon = self.gattling;
        object.has_overlord_propaganda_addon = self.propaganda;
        object.is_helix_transport = self.helix_transport;
        object.overlord_bunker_capacity = self.bunker_capacity;
    }
}

pub(crate) fn collect(
    object: &Object,
    out: &mut Vec<EntityModuleState>,
) -> Result<(), EntityLifecycleCodecError> {
    push_present(
        out,
        TAG_STATUS_BITS,
        StatusBitsResidual::present(object),
        &StatusBitsResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_MODEL_CONDITION,
        ModelConditionResidual::present(object),
        &ModelConditionResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_CONTINUOUS_FIRE,
        ContinuousFireResidual::present(object),
        &ContinuousFireResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_SUBDUAL_DAMAGE,
        SubdualDamageResidual::present(object),
        &SubdualDamageResidual::from_object(object),
    )?;
    push_present(
        out,
        TAG_OVERLORD_ADDON,
        OverlordAddonResidual::present(object),
        &OverlordAddonResidual::from_object(object),
    )
}

pub(crate) fn apply(
    object: &mut Object,
    module: &EntityModuleState,
) -> Result<bool, EntityLifecycleCodecError> {
    let payload = module.payload.as_slice();
    match module.tag.as_str() {
        TAG_STATUS_BITS => decode_payload::<StatusBitsResidual>(payload)?.apply(object),
        TAG_MODEL_CONDITION => decode_payload::<ModelConditionResidual>(payload)?.apply(object),
        TAG_CONTINUOUS_FIRE => decode_payload::<ContinuousFireResidual>(payload)?.apply(object),
        TAG_SUBDUAL_DAMAGE => decode_payload::<SubdualDamageResidual>(payload)?.apply(object),
        TAG_OVERLORD_ADDON => decode_payload::<OverlordAddonResidual>(payload)?.apply(object),
        _ => return Ok(false),
    }
    Ok(true)
}
