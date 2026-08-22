//! Persist live PointDefenseLaser shot cooldown.
//!
//! C++ `PointDefenseLaserUpdate::xfer` v1
//! (`PointDefenseLaserUpdate.cpp:363-386`) writes UpdateModule base plus
//! `m_bestTargetID`, `m_inRange`, `m_nextScanFrames`,
//! `m_nextShotAvailableInFrames`. Leftover `PointDefenseLaserUpdate::xfer`
//! matches that table. Live intercept uses
//! `GameLogic.point_defense_next_ready_frame` (absolute ready frame, the
//! live stand-in for remaining DelayBetweenShots). Construct / reset clear
//! the map — a mid-DelayBetweenShots Paladin save re-armed the laser.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore always replaces the live map so a load cannot leak the previous
//! session's cooldown.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const PDLS_MAGIC: &[u8; 4] = b"PDLS";
const PDLS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PointDefensePersistPayload {
    ready: Vec<PointDefenseReadyPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointDefenseReadyPersist {
    object_id: u32,
    next_ready_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.ready.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(PDLS_MAGIC);
    append_u32(bytes, PDLS_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(
    bytes: &[u8],
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous cooldown.
    game_logic.point_defense_next_ready_frame.clear();
    let Some(suffix) = find_pdls_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != PDLS_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown PDLS suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "PDLS payload truncated".to_string(),
        ));
    }
    let payload: PointDefensePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("PDLS payload decode: {err}")))?;
    for entry in payload.ready {
        game_logic
            .point_defense_next_ready_frame
            .insert(ObjectId(entry.object_id), entry.next_ready_frame);
    }
    Ok(())
}

fn capture(game_logic: &GameLogic) -> PointDefensePersistPayload {
    let mut ready: Vec<PointDefenseReadyPersist> = game_logic
        .point_defense_next_ready_frame
        .iter()
        .map(|(id, frame)| PointDefenseReadyPersist {
            object_id: id.0,
            next_ready_frame: *frame,
        })
        .collect();
    ready.sort_by_key(|entry| entry.object_id);
    PointDefensePersistPayload { ready }
}

fn find_pdls_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == PDLS_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("PDLS u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips_paladin_shot_cooldown() {
        let mut source = GameLogic::new();
        source
            .point_defense_next_ready_frame
            .insert(ObjectId(5), 77);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_pdls_suffix(&snapshot.lifecycle_tail).is_some(),
            "PDLS suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored
            .point_defense_next_ready_frame
            .insert(ObjectId(99), 1);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(
            restored.point_defense_next_ready_frame.get(&ObjectId(5)),
            Some(&77)
        );
        assert!(
            !restored
                .point_defense_next_ready_frame
                .contains_key(&ObjectId(99))
        );
    }

    #[test]
    fn absent_suffix_clears_stale_point_defense() {
        let mut logic = GameLogic::new();
        logic
            .point_defense_next_ready_frame
            .insert(ObjectId(3), 12);
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.point_defense_next_ready_frame.is_empty());
    }
}
