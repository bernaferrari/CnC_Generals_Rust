//! Persist live Dragon Tank FireWall burn registry.
//!
//! C++ FireWallSegment objects persist via `DeletionUpdate::xfer` `m_dieFrame`
//! plus `FireWeaponUpdate::xfer` weapon + `m_initialDelayFrame`. Leftover
//! `deletion_update.rs` / `fire_weapon_update.rs` already match those tables.
//! Live stores activate/expires/next_tick/segments on `GameLogic.fire_walls`.
//! Those records were live-only — a mid-wall save dropped remaining FLAME.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore replaces the live registry and never re-runs `activate`.

use crate::game_logic::GameLogic;
use crate::game_logic::host_firewall::HostFireWallRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const FWAL_MAGIC: &[u8; 4] = b"FWAL";
const FWAL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FireWallPersistPayload {
    registry: HostFireWallRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.active_count() == 0 && payload.registry.activations == 0 {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(FWAL_MAGIC);
    append_u32(bytes, FWAL_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    game_logic.fire_walls.clear();
    let Some(suffix) = find_fwal_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != FWAL_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown FWAL suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "FWAL payload truncated".to_string(),
        ));
    }
    let payload: FireWallPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("FWAL payload decode: {err}")))?;
    game_logic.fire_walls = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> FireWallPersistPayload {
    FireWallPersistPayload {
        registry: game_logic.fire_walls.clone(),
    }
}

fn find_fwal_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == FWAL_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("FWAL u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{ObjectId, Team};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_firewall() {
        let mut source = GameLogic::new();
        source.fire_walls.activate(
            ObjectId(1),
            Team::China,
            Vec3::ZERO,
            Vec3::new(80.0, 0.0, 0.0),
            5,
            false,
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_fwal_suffix(&snapshot.lifecycle_tail).is_some(),
            "FWAL suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        assert_eq!(restored.fire_walls.active_count(), 1);
        let wall = &restored.fire_walls.active_walls()[0];
        assert_eq!(wall.activate_frame, 5);
        assert_eq!(
            wall.expires_frame,
            5 + crate::game_logic::FIREWALL_DURATION_FRAMES
        );
        assert!(!wall.segments.is_empty());
    }

    #[test]
    fn absent_suffix_clears_stale_firewall() {
        let mut logic = GameLogic::new();
        logic.fire_walls.activate(
            ObjectId(1),
            Team::China,
            Vec3::ZERO,
            Vec3::new(40.0, 0.0, 0.0),
            0,
            false,
        );
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.fire_walls.active_count(), 0);
    }
}
