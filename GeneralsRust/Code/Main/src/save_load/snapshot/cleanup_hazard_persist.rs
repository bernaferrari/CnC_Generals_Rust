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

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::host_cleanup_area::HostCleanupAreaOrder;
use crate::game_logic::GameLogic;
use crate::save_load::SaveLoadResult;
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
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(CLHA_MAGIC);
    append_u32(bytes, CLHA_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous dest.
    game_logic.cleanup_areas.restore_orders(Vec::new());
    let Some(record) = records.get(CLHA_MAGIC) else {
        return Ok(());
    };
    let payload: CleanupHazardPersistPayload = record.decode(CLHA_VERSION)?;
    game_logic.cleanup_areas.restore_orders(payload.orders);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> CleanupHazardPersistPayload {
    CleanupHazardPersistPayload {
        orders: game_logic.cleanup_areas.orders().to_vec(),
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
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
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(CLHA_MAGIC)
                .is_some(),
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
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert!(logic.cleanup_areas.orders().is_empty());
    }
}
