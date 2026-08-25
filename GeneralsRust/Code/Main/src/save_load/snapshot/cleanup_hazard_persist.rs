//! Persist live CleanupHazard area orders.
//!
//! C++ `CleanupHazardUpdate::xfer` v1 (`CleanupHazardUpdate.cpp:309-341`)
//! writes UpdateModule base plus `m_bestTargetID`, `m_inRange`,
//! `m_nextScanFrames`, `m_nextShotAvailableInFrames`, `m_pos`, `m_moveRange`.
//! Leftover `CleanupHazardUpdate::xfer` matches that table. Live stores the
//! same order on `GameLogic.cleanup_areas` as `HostCleanupAreaOrder`
//! (caster, dest, move_range, next_shot_frame). Construct / reset clear
//! the registry — a mid-CleanupArea save dropped the ambulance dest.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore always replaces live orders so a load cannot leak the previous
//! session's dest.

use crate::game_logic::GameLogic;
use crate::game_logic::host_cleanup_area::HostCleanupAreaOrder;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const CLHA_MAGIC: &[u8; 4] = b"CLHA";
const CLHA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CleanupHazardPersistPayload {
    orders: Vec<HostCleanupAreaOrder>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.orders.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(CLHA_MAGIC);
    append_u32(bytes, CLHA_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous dest.
    game_logic.cleanup_areas.restore_orders(Vec::new());
    let Some(suffix) = find_clha_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != CLHA_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown CLHA suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "CLHA payload truncated".to_string(),
        ));
    }
    let payload: CleanupHazardPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("CLHA payload decode: {err}")))?;
    game_logic.cleanup_areas.restore_orders(payload.orders);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> CleanupHazardPersistPayload {
    CleanupHazardPersistPayload {
        orders: game_logic.cleanup_areas.orders().to_vec(),
    }
}

fn find_clha_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == CLHA_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("CLHA u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_cleanup_area_dest() {
        let mut source = GameLogic::new();
        source
            .cleanup_areas
            .set_cleanup_area_parameters(HostCleanupAreaOrder {
                caster_id: ObjectId(7),
                player_id: 1,
                location: Vec3::new(120.0, 0.0, 40.0),
                move_range: 300.0,
                next_shot_frame: 88,
            });

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_clha_suffix(&snapshot.lifecycle_tail).is_some(),
            "CLHA suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored
            .cleanup_areas
            .set_cleanup_area_parameters(HostCleanupAreaOrder {
                caster_id: ObjectId(99),
                player_id: 0,
                location: Vec3::ZERO,
                move_range: 1.0,
                next_shot_frame: 1,
            });
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let orders = restored.cleanup_areas.orders();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].caster_id, ObjectId(7));
        assert_eq!(orders[0].player_id, 1);
        assert!((orders[0].location.x - 120.0).abs() < 0.01);
        assert!((orders[0].location.z - 40.0).abs() < 0.01);
        assert!((orders[0].move_range - 300.0).abs() < 0.01);
        assert_eq!(orders[0].next_shot_frame, 88);
    }

    #[test]
    fn absent_suffix_clears_stale_cleanup_orders() {
        let mut logic = GameLogic::new();
        logic
            .cleanup_areas
            .set_cleanup_area_parameters(HostCleanupAreaOrder {
                caster_id: ObjectId(1),
                player_id: 0,
                location: Vec3::ZERO,
                move_range: 50.0,
                next_shot_frame: 4,
            });
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.cleanup_areas.orders().is_empty());
    }
}
