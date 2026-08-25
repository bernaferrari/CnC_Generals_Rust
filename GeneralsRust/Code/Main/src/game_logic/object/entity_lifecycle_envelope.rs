//! Main Object producer/applier and opt-in save/load block for the envelope.

use super::Object;
use super::entity_lifecycle_apply::apply_module_states;
use super::entity_lifecycle_inventory::collect_module_states;
use gamelogic::world::entities::{
    ENTITY_LIFECYCLE_ENVELOPE_VERSION, EntityLifecycleCodecError, EntityLifecycleEnvelope,
};

impl Object {
    /// Inventory residual groups the Main object owns today. Additive only.
    pub fn entity_lifecycle_envelope(&self) -> EntityLifecycleEnvelope {
        EntityLifecycleEnvelope {
            version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
            entity_id: self.id.0,
            destroyed: self.status.destroyed,
            destroyed_at_frame: 0,
            module_states: collect_module_states(self).unwrap_or_default(),
        }
    }

    /// Restore only groups this Main object actually owns. Unknown tags skip.
    pub fn entity_apply_lifecycle_envelope(
        &mut self,
        envelope: &EntityLifecycleEnvelope,
    ) -> Result<(), EntityLifecycleCodecError> {
        self.status.destroyed = envelope.destroyed;
        apply_module_states(self, &envelope.module_states)
    }
}

/// Opt-in additive snapshot block. Empty / absent bytes load as no envelopes.
pub fn encode_lifecycle_snapshot_block(envelopes: &[EntityLifecycleEnvelope]) -> Vec<u8> {
    let mut out = Vec::new();
    let count = u32::try_from(envelopes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for envelope in envelopes.iter().take(count as usize) {
        let encoded = envelope.encode();
        let len = u32::try_from(encoded.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&encoded[..len as usize]);
    }
    out
}

/// Decode an opt-in block. Absence is a successful empty inventory.
pub fn decode_lifecycle_snapshot_block(
    bytes: &[u8],
) -> Result<Vec<EntityLifecycleEnvelope>, EntityLifecycleCodecError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bytes.len() < 4 {
        return Err(EntityLifecycleCodecError::UnexpectedEof {
            context: "snapshot_count",
        });
    }
    let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let mut rest = &bytes[4..];
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if rest.len() < 4 {
            return Err(EntityLifecycleCodecError::UnexpectedEof {
                context: "snapshot_len",
            });
        }
        let len = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        rest = &rest[4..];
        if rest.len() < len {
            return Err(EntityLifecycleCodecError::UnexpectedEof {
                context: "snapshot_envelope",
            });
        }
        out.push(EntityLifecycleEnvelope::decode(&rest[..len])?);
        rest = &rest[len..];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::entity_lifecycle_tags::{
        INVENTORY_TAGS, TAG_CAPTURE_CHANNEL, TAG_HACKER_DISABLE_CHANNEL, TAG_LIFETIME,
        TAG_POISONED, TAG_SLOW_DEATH, TAG_TOPPLE,
    };
    use super::*;
    use crate::game_logic::host_lifetime_update::HostLifetimeUpdateData;
    use crate::game_logic::host_poisoned_behavior::HostPoisonedBehaviorData;
    use crate::game_logic::host_slow_death::{HostSlowDeathData, HostSlowDeathPhase};
    use crate::game_logic::host_topple::{HostToppleData, HostToppleState};
    use crate::game_logic::object::{
        CaptureChannelPhase, CaptureChannelState, HackerDisableChannelPhase,
        HackerDisableChannelState, Object,
    };
    use crate::game_logic::{ObjectId, Team, ThingTemplate};
    use gamelogic::world::entities::EntityModuleState;

    fn test_object() -> Object {
        Object::new(ThingTemplate::new("TestUnit"), ObjectId(7), Team::USA)
    }

    #[test]
    fn residual_roundtrip_is_lossless_for_owned_groups() {
        let mut src = test_object();
        src.status.destroyed = true;
        src.slow_death = Some(HostSlowDeathData {
            phase: HostSlowDeathPhase::Sinking,
            begin_frame: 4,
            sink_at_frame: 8,
            destroy_at_frame: 20,
            sink_rate_per_frame: 0.25,
            sink_offset: -1.5,
            destruction_altitude: -10.0,
            fling_vx: 1.0,
            fling_vz: 0.0,
            fling_vy: 2.0,
            fling_applied: true,
            ..HostSlowDeathData::default()
        });
        src.lifetime_update = Some(HostLifetimeUpdateData {
            expire_at_frame: 90,
            active: true,
        });
        src.poisoned_behavior = Some(HostPoisonedBehaviorData {
            poison_damage_frame: 11,
            poison_overall_stop_frame: 40,
            poison_damage_amount: 3.5,
            tint_poisoned: true,
            tick_count: 2,
            total_dot_damage: 7.0,
            needs_frame_sync: false,
            ..HostPoisonedBehaviorData::default()
        });
        src.topple_data = Some(HostToppleData {
            state: HostToppleState::Falling,
            dir_x: 1.0,
            dir_y: 0.0,
            angular_velocity: 0.2,
            angular_acceleration: 0.01,
            angular_accumulation: 0.4,
            options: 0,
            kill_when_toppled: true,
            kill_when_start_toppled: false,
            lean_radians: 0.3,
            ..Default::default()
        });
        src.capture_channel = Some(CaptureChannelState::new(
            CaptureChannelPhase::Preparing,
            1500,
        ));
        src.hacker_disable_channel = Some(HackerDisableChannelState::new(
            ObjectId(99),
            HackerDisableChannelPhase::Approaching,
            800,
        ));

        let envelope = src.entity_lifecycle_envelope();
        assert!(envelope.destroyed);
        let tags: Vec<&str> = envelope
            .module_states
            .iter()
            .map(|m| m.tag.as_str())
            .collect();
        assert_eq!(
            tags,
            vec![
                TAG_CAPTURE_CHANNEL,
                TAG_HACKER_DISABLE_CHANNEL,
                TAG_TOPPLE,
                TAG_POISONED,
                TAG_LIFETIME,
                TAG_SLOW_DEATH,
            ]
        );

        let mut dst = test_object();
        dst.entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        let again = dst.entity_lifecycle_envelope();
        assert_eq!(again.module_states, envelope.module_states);
        assert!(dst.status.destroyed);
        assert_eq!(
            dst.slow_death.as_ref().map(|s| s.destroy_at_frame),
            Some(20)
        );
        assert_eq!(
            dst.lifetime_update.as_ref().map(|l| l.expire_at_frame),
            Some(90)
        );
    }

    #[test]
    fn unknown_tag_is_skipped_and_known_groups_still_apply() {
        let mut env = test_object().entity_lifecycle_envelope();
        env.module_states.push(EntityModuleState {
            tag: "FutureUnknownModule".to_string(),
            payload: vec![1, 2, 3, 4],
        });
        env.module_states.push(EntityModuleState {
            tag: TAG_LIFETIME.to_string(),
            payload: super::super::entity_lifecycle_inventory::encode_payload(
                &HostLifetimeUpdateData {
                    expire_at_frame: 5,
                    active: true,
                },
            )
            .expect("encode"),
        });
        let mut dst = test_object();
        dst.entity_apply_lifecycle_envelope(&env).expect("apply");
        let life = dst.lifetime_update.expect("lifetime restored");
        assert_eq!(life.expire_at_frame, 5);
        assert!(life.active);
    }

    #[test]
    fn save_absence_loads_empty_inventory() {
        assert!(
            decode_lifecycle_snapshot_block(&[])
                .expect("absent")
                .is_empty()
        );
        let encoded = encode_lifecycle_snapshot_block(&[test_object().entity_lifecycle_envelope()]);
        let decoded = decode_lifecycle_snapshot_block(&encoded).expect("present");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].entity_id, 7);
    }

    #[test]
    fn inventory_tag_table_is_declaration_order() {
        assert_eq!(INVENTORY_TAGS[0], "UpgradeDie");
        assert_eq!(INVENTORY_TAGS[3], "CaptureChannel");
        assert_eq!(INVENTORY_TAGS[19], "SlowDeath");
        assert_eq!(INVENTORY_TAGS[49], "CommandButtonHuntUpdate");
        assert_eq!(
            *INVENTORY_TAGS.last().expect("tags"),
            "ProjectileFlightResiduals"
        );
    }

    #[test]
    fn persistent_gap_clusters_round_trip() {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::object::WeaponLockType;
        use glam::Vec3;

        let mut src = test_object();
        src.fire_weapon_when_dead_fired = true;
        src.create_object_die_transfer_damage = 17.5;
        src.special_power_ready = false;
        src.special_power_cooldown = 30.0;
        src.special_power_cooldown_remaining = 12.0;
        src.special_power_cooldowns
            .insert(SpecialPowerType::DaisyCutter, 8.0);
        src.special_power_override_destination = Some(Vec3::new(1.0, 2.0, 3.0));
        src.weapon_lock_type = WeaponLockType::LockedPermanently;
        src.weapon_lock_slot = 1;
        src.emoticon_name = "Cheer".to_string();
        src.emoticon_frames_left = 45;
        src.is_surrendered = true;
        src.carpet_bomb_payload = true;
        src.booby_trap_attached_to = Some(ObjectId(22));
        src.stealth_jet_missile_projectile = true;
        src.stealth_jet_missile_travelled = 40.0;

        let envelope = src.entity_lifecycle_envelope();
        let tags: Vec<&str> = envelope
            .module_states
            .iter()
            .map(|m| m.tag.as_str())
            .collect();
        assert!(tags.contains(&"FireWeaponWhenDead"));
        assert!(tags.contains(&"CreateObjectDieTransfer"));
        assert!(tags.contains(&"SpecialPowerCooldown"));
        assert!(tags.contains(&"WeaponLock"));
        assert!(tags.contains(&"EmoticonSurrender"));
        assert!(tags.contains(&"ProjectileFlightResiduals"));

        let mut dst = test_object();
        dst.entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        assert!(dst.fire_weapon_when_dead_fired);
        assert_eq!(dst.create_object_die_transfer_damage, 17.5);
        assert!(!dst.special_power_ready);
        assert_eq!(
            dst.special_power_cooldowns
                .get(&SpecialPowerType::DaisyCutter)
                .copied(),
            Some(8.0)
        );
        assert_eq!(dst.weapon_lock_type, WeaponLockType::LockedPermanently);
        assert_eq!(dst.emoticon_frames_left, 45);
        assert!(dst.is_surrendered);
        assert!(dst.carpet_bomb_payload);
        assert_eq!(dst.booby_trap_attached_to, Some(ObjectId(22)));
        assert!(dst.stealth_jet_missile_projectile);
        assert_eq!(dst.stealth_jet_missile_travelled, 40.0);
    }
}
