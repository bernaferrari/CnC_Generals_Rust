//! Persist live PointDefenseLaser shot cooldown.
//!
//! C++ `PointDefenseLaserUpdate::xfer` v1
//! (`PointDefenseLaserUpdate.cpp:363-386`) writes UpdateModule base plus
//! `m_bestTargetID`, `m_inRange`, `m_nextScanFrames`,
//! `m_nextShotAvailableInFrames`. Leftover `PointDefenseLaserUpdate::xfer`
//! matches that table. Live intercept uses
//! `GameLogic.point_defense_next_ready_frame` (module 0) and
//! `point_defense_next_ready_frame_1` (Avenger leftover module 1). Both are
//! absolute ready frames, the live stand-in for remaining DelayBetweenShots.
//! Construct / reset clear the maps — a mid-DelayBetweenShots Paladin save
//! re-armed the laser; Avenger laser-two would re-arm without the second map.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore always replaces both live maps so a load cannot leak the previous
//! session's cooldown. PDLS v1 loads module 0 only; v2 restores both streams.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const PDLS_MAGIC: &[u8; 4] = b"PDLS";
const PDLS_VERSION: u32 = 2;
const PDLS_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PointDefensePersistPayload {
    ready: Vec<PointDefenseReadyPersist>,
    ready_1: Vec<PointDefenseReadyPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PointDefensePersistPayloadV1 {
    ready: Vec<PointDefenseReadyPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointDefenseReadyPersist {
    object_id: u32,
    next_ready_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.ready.is_empty() && payload.ready_1.is_empty() {
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

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous cooldown.
    game_logic.point_defense_next_ready_frame.clear();
    game_logic.point_defense_next_ready_frame_1.clear();
    let Some(suffix) = find_pdls_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "PDLS payload truncated".to_string(),
        ));
    }
    let encoded = &rest[..payload_len];
    let (ready, ready_1) = match version {
        PDLS_VERSION_V1 => {
            let payload: PointDefensePersistPayloadV1 = bincode::deserialize(encoded)
                .map_err(|err| SaveLoadError::Corrupted(format!("PDLS payload decode: {err}")))?;
            (payload.ready, Vec::new())
        }
        PDLS_VERSION => {
            let payload: PointDefensePersistPayload = bincode::deserialize(encoded)
                .map_err(|err| SaveLoadError::Corrupted(format!("PDLS payload decode: {err}")))?;
            (payload.ready, payload.ready_1)
        }
        other => {
            return Err(SaveLoadError::Corrupted(format!(
                "unknown PDLS suffix version {other}"
            )));
        }
    };
    for entry in ready {
        game_logic
            .point_defense_next_ready_frame
            .insert(ObjectId(entry.object_id), entry.next_ready_frame);
    }
    for entry in ready_1 {
        game_logic
            .point_defense_next_ready_frame_1
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
    let mut ready_1: Vec<PointDefenseReadyPersist> = game_logic
        .point_defense_next_ready_frame_1
        .iter()
        .map(|(id, frame)| PointDefenseReadyPersist {
            object_id: id.0,
            next_ready_frame: *frame,
        })
        .collect();
    ready_1.sort_by_key(|entry| entry.object_id);
    PointDefensePersistPayload { ready, ready_1 }
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
    fn snapshot_round_trips_avenger_second_laser_clock() {
        let mut source = GameLogic::new();
        source
            .point_defense_next_ready_frame
            .insert(ObjectId(8), 40);
        source
            .point_defense_next_ready_frame_1
            .insert(ObjectId(8), 55);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");

        let mut restored = GameLogic::new();
        restored
            .point_defense_next_ready_frame_1
            .insert(ObjectId(3), 9);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(
            restored.point_defense_next_ready_frame.get(&ObjectId(8)),
            Some(&40)
        );
        assert_eq!(
            restored.point_defense_next_ready_frame_1.get(&ObjectId(8)),
            Some(&55)
        );
        assert!(
            !restored
                .point_defense_next_ready_frame_1
                .contains_key(&ObjectId(3))
        );
    }

    #[test]
    fn absent_suffix_clears_stale_point_defense() {
        let mut logic = GameLogic::new();
        logic.point_defense_next_ready_frame.insert(ObjectId(3), 12);
        logic
            .point_defense_next_ready_frame_1
            .insert(ObjectId(3), 19);
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.point_defense_next_ready_frame.is_empty());
        assert!(logic.point_defense_next_ready_frame_1.is_empty());
    }
}
