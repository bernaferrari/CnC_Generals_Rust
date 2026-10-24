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
use bincode_legacy::Options;
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
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(SUBD_MAGIC);
    append_u32(bytes, SUBD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

fn decode_from_lifecycle_tail(bytes: &[u8]) -> SaveLoadResult<Option<SubdualPersistPayload>> {
    let Some(suffix) = find_subd_suffix(bytes)? else {
        return Ok(None);
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    let payload_len = take_u32(&mut rest)? as usize;
    if version != SUBD_VERSION || payload_len != rest.len() || payload_len > 16 * 1024 * 1024 {
        return Err(SaveLoadError::Corrupted(
            "invalid SUBD version/length".into(),
        ));
    }
    // The fixed-int wire is one Vec length (u64), then three four-byte
    // fields per entry. Validate that count before serde can reserve memory.
    let count = rest
        .get(..8)
        .ok_or_else(|| SaveLoadError::Corrupted("SUBD missing object count".into()))?;
    let count = usize::try_from(u64::from_le_bytes(count.try_into().unwrap()))
        .map_err(|_| SaveLoadError::Corrupted("SUBD object count overflow".into()))?;
    if count
        .checked_mul(12)
        .and_then(|length| length.checked_add(8))
        != Some(rest.len())
    {
        return Err(SaveLoadError::Corrupted(
            "SUBD object count/length mismatch".into(),
        ));
    }
    let payload: SubdualPersistPayload = bincode_legacy::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .deserialize(rest)
        .map_err(|error| SaveLoadError::Corrupted(format!("SUBD payload decode: {error}")))?;
    let mut seen = std::collections::HashSet::new();
    for entry in &payload.objects {
        // Original internalAddSubdualDamage clamps only the upper cap; finite
        // negative residual pools and every unsigned countdown are valid.
        if !entry.current_subdual_damage.is_finite() || !seen.insert(entry.object_id) {
            return Err(SaveLoadError::Corrupted(
                "nonfinite/duplicate SUBD owner".into(),
            ));
        }
    }
    Ok(Some(payload))
}
pub(super) fn preflight_from_lifecycle_tail(
    bytes: &[u8],
    snapshot: &super::WorldSnapshot,
) -> SaveLoadResult<()> {
    if let Some(payload) = decode_from_lifecycle_tail(bytes)? {
        for entry in payload.objects {
            if !snapshot.objects.contains_key(&ObjectId(entry.object_id)) {
                return Err(SaveLoadError::Corrupted(
                    "SUBD references no saved receiving actor".into(),
                ));
            }
        }
    }
    Ok(())
}
pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(payload) = decode_from_lifecycle_tail(bytes)? else {
        return Ok(());
    };
    // Complete owner validation before assigning any pool/countdown.
    if payload
        .objects
        .iter()
        .any(|entry| game_logic.host_object(ObjectId(entry.object_id)).is_none())
    {
        return Err(SaveLoadError::Corrupted(
            "SUBD references no receiving actor".into(),
        ));
    }
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> SubdualPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
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

fn find_subd_suffix(bytes: &[u8]) -> SaveLoadResult<Option<&[u8]>> {
    // Preserve the old absent-owner adapter on arbitrary detached opaque tails.
    // This negative presence proof never selects a record or reads its fields.
    // Every possible owner still must pass the shared structural grammar.
    if !bytes.windows(4).any(|window| window == SUBD_MAGIC) {
        return Ok(None);
    }
    super::ai_team_persist::framing::find_record(bytes, SUBD_MAGIC)
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

    fn framed_payload(payload: &SubdualPersistPayload) -> Vec<u8> {
        let encoded = bincode_legacy::serialize(payload).unwrap();
        let mut bytes = SUBD_MAGIC.to_vec();
        append_u32(&mut bytes, SUBD_VERSION);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        bytes
    }
    #[test]
    fn subd_magic_inside_framed_timer_name_is_not_an_owner() {
        let mut bytes = b"SPCD".to_vec();
        append_u32(&mut bytes, 2); // supported legacy SPCD with three tables
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 4);
        bytes.extend_from_slice(b"SUBD");
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 0);
        assert!(decode_from_lifecycle_tail(&bytes).unwrap().is_none());
        let actual = framed_payload(&SubdualPersistPayload {
            objects: vec![ObjectSubdualPersist {
                object_id: 9,
                current_subdual_damage: -3.5,
                healing_step_countdown: u32::MAX,
            }],
        });
        bytes.extend_from_slice(&actual);
        let decoded = decode_from_lifecycle_tail(&bytes).unwrap().unwrap();
        assert_eq!(decoded.objects[0].current_subdual_damage, -3.5);
        assert_eq!(decoded.objects[0].healing_step_countdown, u32::MAX);
        bytes.extend_from_slice(&actual);
        assert!(
            decode_from_lifecycle_tail(&bytes).is_err(),
            "duplicate actual owner rejects"
        );
    }
    #[test]
    fn malformed_subd_rejects_duplicates_and_nonfinite_but_not_negative_history() {
        let entry = || ObjectSubdualPersist {
            object_id: 7,
            current_subdual_damage: -2.0,
            healing_step_countdown: 0,
        };
        assert!(
            decode_from_lifecycle_tail(&framed_payload(&SubdualPersistPayload {
                objects: vec![entry()],
            }))
            .unwrap()
            .is_some()
        );
        assert!(
            decode_from_lifecycle_tail(&framed_payload(&SubdualPersistPayload {
                objects: vec![entry(), entry()],
            }))
            .is_err()
        );
        let mut bad = entry();
        bad.current_subdual_damage = f32::NAN;
        assert!(
            decode_from_lifecycle_tail(&framed_payload(&SubdualPersistPayload {
                objects: vec![bad],
            }))
            .is_err()
        );
    }

    #[test]
    fn foreign_subdual_actor_rejects_before_changing_the_receiving_pool() {
        let mut world = GameLogic::new();
        world.templates.insert(
            "ReceivingSubdual".into(),
            ThingTemplate::new("ReceivingSubdual"),
        );
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let id = world
            .create_object("ReceivingSubdual", Team::USA, Vec3::ZERO)
            .unwrap();
        {
            let actor = world.host_object_mut(id).unwrap();
            actor.subdual_damage = 12.0;
            actor.subdual_heal_countdown = 3;
        }
        let snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&world)
            .unwrap();
        let payload = SubdualPersistPayload {
            objects: vec![
                ObjectSubdualPersist {
                    object_id: id.0,
                    current_subdual_damage: 99.0,
                    healing_step_countdown: 7,
                },
                ObjectSubdualPersist {
                    object_id: u32::MAX,
                    current_subdual_damage: -2.0,
                    healing_step_countdown: u32::MAX,
                },
            ],
        };
        let bytes = framed_payload(&payload);
        assert!(preflight_from_lifecycle_tail(&bytes, &snapshot).is_err());
        assert!(apply_from_lifecycle_tail(&bytes, &mut world).is_err());
        let actor = world.host_object(id).unwrap();
        assert_eq!(
            (actor.subdual_damage, actor.subdual_heal_countdown),
            (12.0, 3)
        );
        // The advertised table count must never drive an unbounded allocation.
        let mut forged = framed_payload(&SubdualPersistPayload::default());
        forged[12..20].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_from_lifecycle_tail(&forged).is_err());
    }

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_subd_suffix(b"no-magic-here").unwrap().is_none());
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
            let (factory, health_events) = source
                .host_object_and_health_events_mut(factory_id)
                .expect("factory");
            factory.health.maximum = 2000.0;
            factory.health.current = 2000.0;
            factory.subdual_damage_cap = 2000.0;
            factory.subdual_heal_rate_frames = 15;
            factory.subdual_heal_amount = 50.0;
            factory.subdual_damage = 2000.0;
            factory.subdual_heal_countdown = 11;
            factory.set_disabled_subdued(
                true,
                health_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
            );
            assert!(factory.is_subdued_disabled());
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_subd_suffix(&snapshot.lifecycle_tail)
                .unwrap()
                .is_some(),
            "SUBD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::USA, "USA", true));
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
