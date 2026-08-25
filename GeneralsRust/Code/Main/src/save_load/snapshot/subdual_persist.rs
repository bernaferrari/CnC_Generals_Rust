//! Persist C++ `ActiveBody::m_currentSubdualDamage` and
//! `SubdualDamageHelper::m_healingStepCountdown` on the existing v9
//! `lifecycle_tail` blob.
//!
//! C++ `ActiveBody::xfer` (`ActiveBody.cpp:1523`) writes the running
//! subdual pool; `SubdualDamageHelper::xfer` (`SubdualDamageHelper.cpp:91`)
//! writes the heal-step countdown. Live host already accumulates and heals
//! those fields, but `ObjectSnapshot` only stored `disabled_subdued`. After
//! load the bit stayed set forever because heal had no pool/countdown to
//! drain. Leftover ActiveBody / SubdualDamageHelper xfer already matches;
//! this suffix wires the same two values onto the live host path.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL) so older decoders ignore the extra bytes. No
//! world snapshot version bump.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const SUBD_MAGIC: &[u8; 4] = b"SUBD";
const SUBD_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SubdualPersistPayload {
    objects: Vec<ObjectSubdualPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectSubdualPersist {
    object_id: u32,
    /// C++ `ActiveBody::m_currentSubdualDamage`.
    current_subdual_damage: f32,
    /// C++ `SubdualDamageHelper::m_healingStepCountdown`.
    healing_step_countdown: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(SUBD_MAGIC);
    append_u32(bytes, SUBD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_subd_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != SUBD_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown SUBD suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "SUBD payload truncated".to_string(),
        ));
    }
    let payload: SubdualPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("SUBD payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> SubdualPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if object.subdual_damage == 0.0 && object.subdual_heal_countdown == 0 {
            continue;
        }
        objects.push(ObjectSubdualPersist {
            object_id: id.0,
            current_subdual_damage: object.subdual_damage,
            healing_step_countdown: object.subdual_heal_countdown,
        });
    }
    SubdualPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: SubdualPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.subdual_damage = entry.current_subdual_damage;
        object.subdual_heal_countdown = entry.healing_step_countdown;
    }
}

fn find_subd_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == SUBD_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("SUBD u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_subd_suffix(b"no-magic-here").is_none());
    }

    #[test]
    fn snapshot_round_trips_subdual_pool_and_heal_countdown() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaWarFactory".to_string(),
            ThingTemplate::new("AmericaWarFactory"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let factory_id = source
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(20.0, 0.0, 16.0))
            .expect("factory");
        {
            let factory = source.host_object_mut(factory_id).expect("factory");
            factory.health.maximum = 2000.0;
            factory.health.current = 2000.0;
            factory.subdual_damage_cap = 2000.0;
            factory.subdual_heal_rate_frames = 15;
            factory.subdual_heal_amount = 50.0;
            factory.subdual_damage = 2000.0;
            factory.subdual_heal_countdown = 11;
            factory.set_disabled_subdued(true);
            assert!(factory.is_subdued_disabled());
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_subd_suffix(&snapshot.lifecycle_tail).is_some(),
            "SUBD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(factory_id).expect("restored factory");
        assert!(
            (loaded.subdual_damage - 2000.0).abs() < 1e-3,
            "m_currentSubdualDamage must survive load, got {}",
            loaded.subdual_damage
        );
        assert_eq!(
            loaded.subdual_heal_countdown, 11,
            "m_healingStepCountdown must survive load"
        );
        assert!(
            loaded.is_subdued_disabled(),
            "DISABLED_SUBDUED bit still restored from ObjectSnapshot"
        );
    }
}
