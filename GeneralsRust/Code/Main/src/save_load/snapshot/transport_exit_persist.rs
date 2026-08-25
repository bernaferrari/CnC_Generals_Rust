//! Persist live TransportContain exit-busy frames.
//!
//! C++ `TransportContain::xfer` v1 (`TransportContain.cpp:654-674`) writes
//! OpenContain base, `m_payloadCreated`, `m_extraSlotsInUse`, and
//! `m_frameExitNotBusy`. Leftover `transport_contain.rs` / `rider_change_contain.rs`
//! already match that table. Live stores the exit-busy clock on
//! `Object.frame_exit_not_busy` (`exitObjectViaDoor` sets
//! `getFrame() + ExitDelay`; `isExitBusy` is true while
//! `getFrame() < frame_exit_not_busy`). Object snapshots never wrote the
//! field — a mid-unload save re-armed the next rider immediately.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes the clock only; it never re-runs `exitObjectViaDoor`.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const TXEB_MAGIC: &[u8; 4] = b"TXEB";
const TXEB_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TransportExitPersistPayload {
    objects: Vec<TransportExitPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransportExitPersist {
    object_id: u32,
    frame_exit_not_busy: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(TXEB_MAGIC);
    append_u32(bytes, TXEB_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: reconstructed transports start idle; a reused GameLogic
    // must not keep the previous session's exit delay.
    reset_exit_busy(game_logic);
    let Some(suffix) = find_txeb_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != TXEB_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown TXEB suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "TXEB payload truncated".to_string(),
        ));
    }
    let payload: TransportExitPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("TXEB payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> TransportExitPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if object.frame_exit_not_busy == 0 {
            continue;
        }
        objects.push(TransportExitPersist {
            object_id: id.0,
            frame_exit_not_busy: object.frame_exit_not_busy,
        });
    }
    TransportExitPersistPayload { objects }
}

fn reset_exit_busy(game_logic: &mut GameLogic) {
    let ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    for id in ids {
        if let Some(object) = game_logic.host_object_mut(id) {
            object.frame_exit_not_busy = 0;
        }
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: TransportExitPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.frame_exit_not_busy = entry.frame_exit_not_busy;
    }
}

fn find_txeb_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == TXEB_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("TXEB u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_transport_exit_delay() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaVehicleHumvee".to_string(),
            ThingTemplate::new("AmericaVehicleHumvee"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let transport = source
            .create_object(
                "AmericaVehicleHumvee",
                Team::USA,
                Vec3::new(20.0, 0.0, 16.0),
            )
            .expect("transport");
        {
            let object = source.host_object_mut(transport).expect("transport obj");
            object.frame_exit_not_busy = 88;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_txeb_suffix(&snapshot.lifecycle_tail).is_some(),
            "TXEB suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(transport).expect("restored transport");
        assert_eq!(loaded.frame_exit_not_busy, 88);
        assert!(
            loaded.is_transport_exit_busy(87),
            "load must keep the remaining ExitDelay window"
        );
        assert!(!loaded.is_transport_exit_busy(88));
    }

    #[test]
    fn absent_suffix_clears_stale_exit_busy() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaVehicleHumvee".to_string(),
            ThingTemplate::new("AmericaVehicleHumvee"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let transport = logic
            .create_object("AmericaVehicleHumvee", Team::USA, Vec3::ZERO)
            .expect("transport");
        {
            let object = logic.host_object_mut(transport).expect("transport obj");
            object.frame_exit_not_busy = 44;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(transport).expect("transport");
        assert_eq!(object.frame_exit_not_busy, 0);
    }
}
