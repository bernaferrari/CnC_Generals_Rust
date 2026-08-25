//! Persistent Object::xfer residuals that were Main-only flattened fields.
//!
//! C++ citations: Object.cpp:3995-4364 (v9 object fields + tagged module
//! snapshots). Also ActiveBody.cpp:1513-1573 crush/indestructible,
//! PhysicsUpdate.cpp:1830-1875 stun/overlap, RailroadGuideAIUpdate.cpp:1503-1576.

use super::Object;
use crate::command_system::SpecialPowerType;
use crate::game_logic::ObjectId;
use crate::game_logic::host_railroad::{HostRailroadCar, railroad_car, restore_railroad_car};
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct ActiveBodyCrushResidual {
    pub front_crushed: bool,
    pub back_crushed: bool,
    pub indestructible: bool,
    pub last_damage_source: Option<ObjectId>,
    pub last_damage_timestamp: Option<u32>,
    pub last_healing_timestamp: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct PhysicsBehaviorResidual {
    pub shock_stun_frames: u32,
    pub shock_yaw_rate: f32,
    pub shock_pitch_rate: f32,
    pub shock_roll_rate: f32,
    pub shock_allow_bounce: bool,
    pub shock_was_airborne: bool,
    pub shock_grounded_once: bool,
    pub shock_up_z: f32,
    pub current_overlap: Option<ObjectId>,
    pub previous_overlap: Option<ObjectId>,
    pub ignore_collisions_with: Option<ObjectId>,
    pub last_collidee: Option<ObjectId>,
    pub motive_frames_remaining: u32,
    pub extra_friction: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RailroadBehaviorResidual {
    pub car: HostRailroadCar,
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

impl ActiveBodyCrushResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.front_crushed
            || object.back_crushed
            || object.indestructible
            || object.last_damage_source.is_some()
            || object.last_damage_timestamp.is_some()
            || object.last_healing_timestamp.is_some()
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            front_crushed: object.front_crushed,
            back_crushed: object.back_crushed,
            indestructible: object.indestructible,
            last_damage_source: object.last_damage_source,
            last_damage_timestamp: object.last_damage_timestamp,
            last_healing_timestamp: object.last_healing_timestamp,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.front_crushed = self.front_crushed;
        object.back_crushed = self.back_crushed;
        object.indestructible = self.indestructible;
        object.last_damage_source = self.last_damage_source;
        object.last_damage_timestamp = self.last_damage_timestamp;
        object.last_healing_timestamp = self.last_healing_timestamp;
        object.apply_crush_die_model_conditions();
    }
}

impl PhysicsBehaviorResidual {
    pub(crate) fn present(object: &Object) -> bool {
        object.shock_stun_frames != 0
            || object.shock_yaw_rate != 0.0
            || object.shock_pitch_rate != 0.0
            || object.shock_roll_rate != 0.0
            || object.shock_allow_bounce
            || object.shock_was_airborne
            || object.shock_grounded_once
            || object.physics_current_overlap.is_some()
            || object.physics_previous_overlap.is_some()
            || object.ignore_collisions_with.is_some()
            || object.last_collidee.is_some()
            || object.motive_frames_remaining != 0
            || object.extra_friction != 0.0
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            shock_stun_frames: object.shock_stun_frames,
            shock_yaw_rate: object.shock_yaw_rate,
            shock_pitch_rate: object.shock_pitch_rate,
            shock_roll_rate: object.shock_roll_rate,
            shock_allow_bounce: object.shock_allow_bounce,
            shock_was_airborne: object.shock_was_airborne,
            shock_grounded_once: object.shock_grounded_once,
            shock_up_z: object.shock_up_z,
            current_overlap: object.physics_current_overlap,
            previous_overlap: object.physics_previous_overlap,
            ignore_collisions_with: object.ignore_collisions_with,
            last_collidee: object.last_collidee,
            motive_frames_remaining: object.motive_frames_remaining,
            extra_friction: object.extra_friction,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.shock_stun_frames = self.shock_stun_frames;
        object.shock_yaw_rate = self.shock_yaw_rate;
        object.shock_pitch_rate = self.shock_pitch_rate;
        object.shock_roll_rate = self.shock_roll_rate;
        object.shock_allow_bounce = self.shock_allow_bounce;
        object.shock_was_airborne = self.shock_was_airborne;
        object.shock_grounded_once = self.shock_grounded_once;
        object.shock_up_z = self.shock_up_z;
        object.physics_current_overlap = self.current_overlap;
        object.physics_previous_overlap = self.previous_overlap;
        object.ignore_collisions_with = self.ignore_collisions_with;
        object.last_collidee = self.last_collidee;
        object.motive_frames_remaining = self.motive_frames_remaining;
        object.extra_friction = self.extra_friction;
        object.refresh_model_condition_bits();
    }
}

impl RailroadBehaviorResidual {
    pub(crate) fn present(object: &Object) -> bool {
        railroad_car(object.id).is_some()
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            car: railroad_car(object.id)
                .unwrap_or_else(|| HostRailroadCar::new_carriage(object.id)),
        }
    }

    pub(crate) fn apply(self, _object: &mut Object) {
        restore_railroad_car(self.car);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_enum_table_residual::{
        MC_BIT_STUNNED_FLAILING, host_model_condition_has,
    };
    use crate::game_logic::host_neutron_missile_slow_death::MC_BIT_FRONTCRUSHED;
    use crate::game_logic::host_railroad::{
        HostConductorState, HostRailroadCar, railroad_car, railroad_registry_reset,
        restore_railroad_car,
    };
    use crate::game_logic::object::Object;
    use crate::game_logic::{ObjectId, Team, ThingTemplate};

    fn test_object() -> Object {
        Object::new(ThingTemplate::new("SaveACar"), ObjectId(21), Team::USA)
    }

    #[test]
    fn active_body_crush_and_indestructible_round_trip() {
        let mut src = test_object();
        src.front_crushed = true;
        src.indestructible = true;
        src.last_damage_source = Some(ObjectId(9));
        src.last_damage_timestamp = Some(44);
        src.apply_crush_die_model_conditions();
        let envelope = src.entity_lifecycle_envelope();
        assert!(
            envelope
                .module_states
                .iter()
                .any(|m| m.tag == super::super::entity_lifecycle_tags::TAG_ACTIVE_BODY)
        );

        let mut dst = test_object();
        dst.entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        assert!(dst.front_crushed);
        assert!(!dst.back_crushed);
        assert!(dst.indestructible);
        assert_eq!(dst.last_damage_source, Some(ObjectId(9)));
        assert_eq!(dst.last_damage_timestamp, Some(44));
        assert!(host_model_condition_has(
            dst.model_condition_bits,
            MC_BIT_FRONTCRUSHED
        ));
    }

    #[test]
    fn physics_stun_and_overlap_round_trip() {
        let mut src = test_object();
        src.shock_stun_frames = 40;
        src.shock_yaw_rate = 0.3;
        src.shock_pitch_rate = -0.1;
        src.physics_current_overlap = Some(ObjectId(7));
        src.physics_previous_overlap = Some(ObjectId(8));
        src.ignore_collisions_with = Some(ObjectId(3));
        src.motive_frames_remaining = 5;
        src.extra_friction = -0.02;
        src.refresh_model_condition_bits();
        let envelope = src.entity_lifecycle_envelope();
        assert!(
            envelope
                .module_states
                .iter()
                .any(|m| m.tag == super::super::entity_lifecycle_tags::TAG_PHYSICS_BEHAVIOR)
        );

        let mut dst = test_object();
        dst.entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        assert_eq!(dst.shock_stun_frames, 40);
        assert!((dst.shock_yaw_rate - 0.3).abs() < 1e-6);
        assert_eq!(dst.physics_current_overlap, Some(ObjectId(7)));
        assert_eq!(dst.physics_previous_overlap, Some(ObjectId(8)));
        assert_eq!(dst.ignore_collisions_with, Some(ObjectId(3)));
        assert_eq!(dst.motive_frames_remaining, 5);
        assert!((dst.extra_friction + 0.02).abs() < 1e-6);
        assert!(host_model_condition_has(
            dst.model_condition_bits,
            MC_BIT_STUNNED_FLAILING
        ));
    }

    #[test]
    fn railroad_conductor_round_trip() {
        railroad_registry_reset();
        let src = test_object();
        let mut car = HostRailroadCar::new_locomotive(src.id);
        car.conductor_state = HostConductorState::WaitAtStation;
        car.speed = 2.5;
        car.track_distance = 120.0;
        car.wait_at_station_timer = 40;
        car.trailer_id = Some(ObjectId(22));
        car.has_ever_been_hitched = true;
        car.carriages_created = true;
        car.track_data_loaded = true;
        car.held = true;
        restore_railroad_car(car);

        let envelope = src.entity_lifecycle_envelope();
        assert!(
            envelope
                .module_states
                .iter()
                .any(|m| m.tag == super::super::entity_lifecycle_tags::TAG_RAILROAD)
        );

        railroad_registry_reset();
        assert!(railroad_car(src.id).is_none());
        let mut dst = test_object();
        dst.entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        let restored = railroad_car(src.id).expect("restored car");
        assert_eq!(restored.conductor_state, HostConductorState::WaitAtStation);
        assert!((restored.speed - 2.5).abs() < 1e-6);
        assert!((restored.track_distance - 120.0).abs() < 1e-6);
        assert_eq!(restored.wait_at_station_timer, 40);
        assert_eq!(restored.trailer_id, Some(ObjectId(22)));
        assert!(restored.held);
        assert!(restored.carriages_created);
        railroad_registry_reset();
    }
}
