//! Persist live temporary stealth grant expiry.
//!
//! C++ `StealthUpdate::xfer` v2 writes `m_framesGranted`. Supply-center
//! dock/exit call `receiveGrant(TRUE, GrantTemporaryStealth)` (retail
//! 20000ms = 600 frames). Leftover `stealth_update.rs` already matches
//! that table. Live stores the remaining grant as
//! `Object.temporary_stealth_expires_frame` (absolute host frame).
//! Object snapshots never wrote it — load zeroed the clock so a camo
//! worker stayed stealthed forever (`temporary_stealth_grant_should_expire`
//! requires expires_frame > 0).
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes the clock only; it never re-runs `apply_temporary_stealth_grant`.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const STLG_MAGIC: &[u8; 4] = b"STLG";
const STLG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StealthGrantPersistPayload {
    objects: Vec<StealthGrantPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StealthGrantPersist {
    object_id: u32,
    temporary_stealth_expires_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(STLG_MAGIC);
    append_u32(bytes, STLG_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    reset_stealth_grant(game_logic);
    let Some(suffix) = find_stlg_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != STLG_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown STLG suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "STLG payload truncated".to_string(),
        ));
    }
    let payload: StealthGrantPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("STLG payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> StealthGrantPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if object.temporary_stealth_expires_frame == 0 {
            continue;
        }
        objects.push(StealthGrantPersist {
            object_id: id.0,
            temporary_stealth_expires_frame: object.temporary_stealth_expires_frame,
        });
    }
    StealthGrantPersistPayload { objects }
}

fn reset_stealth_grant(game_logic: &mut GameLogic) {
    let ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    for id in ids {
        if let Some(object) = game_logic.host_object_mut(id) {
            object.temporary_stealth_expires_frame = 0;
        }
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: StealthGrantPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.temporary_stealth_expires_frame = entry.temporary_stealth_expires_frame;
    }
}

fn find_stlg_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == STLG_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("STLG u32 truncated".to_string()));
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
    fn snapshot_round_trips_temporary_stealth_grant() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "GLAInfantryWorker".to_string(),
            ThingTemplate::new("GLAInfantryWorker"),
        );
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let id = source
            .create_object("GLAInfantryWorker", Team::GLA, Vec3::new(6.0, 0.0, 4.0))
            .expect("worker");
        {
            let object = source.host_object_mut(id).expect("worker obj");
            object.temporary_stealth_expires_frame = 600;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_stlg_suffix(&snapshot.lifecycle_tail).is_some(),
            "STLG suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(id).expect("restored worker");
        assert_eq!(loaded.temporary_stealth_expires_frame, 600);
        assert!(
            !crate::game_logic::Object::temporary_stealth_grant_should_expire(
                loaded.temporary_stealth_expires_frame,
                599,
                false,
            )
        );
        assert!(
            crate::game_logic::Object::temporary_stealth_grant_should_expire(
                loaded.temporary_stealth_expires_frame,
                600,
                false,
            )
        );
        assert!(
            crate::game_logic::Object::temporary_stealth_grant_should_expire(
                loaded.temporary_stealth_expires_frame,
                100,
                true,
            )
        );
    }

    #[test]
    fn absent_suffix_clears_stale_stealth_grant() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "GLAInfantryWorker".to_string(),
            ThingTemplate::new("GLAInfantryWorker"),
        );
        logic.add_player(Player::new(0, Team::GLA, "GLA", true));
        let id = logic
            .create_object("GLAInfantryWorker", Team::GLA, Vec3::ZERO)
            .expect("worker");
        {
            let object = logic.host_object_mut(id).expect("worker");
            object.temporary_stealth_expires_frame = 600;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("worker");
        assert_eq!(object.temporary_stealth_expires_frame, 0);
        assert!(
            !crate::game_logic::Object::temporary_stealth_grant_should_expire(
                object.temporary_stealth_expires_frame,
                50,
                true,
            ),
            "zero expiry must not keep the grant after a player order"
        );
    }
}
