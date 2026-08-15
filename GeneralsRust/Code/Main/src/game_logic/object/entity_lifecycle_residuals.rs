//! Persistent Object::xfer residuals that were Main-only flattened fields.
//!
//! C++ citations: Object.cpp:3995-4364 (v9 object fields + tagged module
//! snapshots). Capture/hacker channels are already inventoried; this file
//! covers cooldown maps, weapon lock, emoticon/surrender, FireWeaponWhenDead,
//! and CreateObjectDie health transfer.

use super::Object;
use crate::command_system::SpecialPowerType;
use crate::game_logic::object::WeaponLockType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct FireWeaponWhenDeadResidual {
    pub fired: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct CreateObjectDieTransferResidual {
    pub transfer_damage: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct SpecialPowerCooldownResidual {
    pub ready: bool,
    pub cooldown: f32,
    pub remaining: f32,
    pub per_power: HashMap<SpecialPowerType, f32>,
    pub override_destination: Option<Vec3>,
    pub override_type: Option<SpecialPowerType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct WeaponLockResidual {
    pub lock_type: WeaponLockType,
    pub slot: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct EmoticonSurrenderResidual {
    pub emoticon_name: String,
    pub frames_left: i32,
    pub surrendered: bool,
}

impl FireWeaponWhenDeadResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.fire_weapon_when_dead_fired
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            fired: object.fire_weapon_when_dead_fired,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.fire_weapon_when_dead_fired = self.fired;
    }
}

impl CreateObjectDieTransferResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.create_object_die_transfer_damage != 0.0
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            transfer_damage: object.create_object_die_transfer_damage,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.create_object_die_transfer_damage = self.transfer_damage;
    }
}

impl SpecialPowerCooldownResidual {
    pub(crate) fn present(object: &Object) -> bool {
        !object.special_power_ready
            || object.special_power_cooldown_remaining != 0.0
            || !object.special_power_cooldowns.is_empty()
            || object.special_power_override_destination.is_some()
            || object.special_power_override_type.is_some()
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            ready: object.special_power_ready,
            cooldown: object.special_power_cooldown,
            remaining: object.special_power_cooldown_remaining,
            per_power: object.special_power_cooldowns.clone(),
            override_destination: object.special_power_override_destination,
            override_type: object.special_power_override_type.clone(),
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.special_power_ready = self.ready;
        object.special_power_cooldown = self.cooldown;
        object.special_power_cooldown_remaining = self.remaining;
        object.special_power_cooldowns = self.per_power;
        object.special_power_override_destination = self.override_destination;
        object.special_power_override_type = self.override_type;
    }
}

impl WeaponLockResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.weapon_lock_type != WeaponLockType::NotLocked || object.weapon_lock_slot != 0
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            lock_type: object.weapon_lock_type,
            slot: object.weapon_lock_slot,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.weapon_lock_type = self.lock_type;
        object.weapon_lock_slot = self.slot;
    }
}

impl EmoticonSurrenderResidual {
    pub(crate) fn present(object: &Object) -> bool {
        !object.emoticon_name.is_empty()
            || object.emoticon_frames_left != 0
            || object.is_surrendered
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            emoticon_name: object.emoticon_name.clone(),
            frames_left: object.emoticon_frames_left,
            surrendered: object.is_surrendered,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.emoticon_name = self.emoticon_name;
        object.emoticon_frames_left = self.frames_left;
        object.is_surrendered = self.surrendered;
    }
}
