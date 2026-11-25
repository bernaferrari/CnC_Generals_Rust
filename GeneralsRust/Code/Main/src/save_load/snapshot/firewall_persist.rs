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

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::host_firewall::HostFireWallRegistry;
use crate::game_logic::GameLogic;
use crate::save_load::SaveLoadResult;
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
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(FWAL_MAGIC);
    append_u32(bytes, FWAL_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    game_logic.fire_walls.clear();
    let Some(record) = records.get(FWAL_MAGIC) else {
        return Ok(());
    };
    let payload: FireWallPersistPayload = record.decode(FWAL_VERSION)?;
    game_logic.fire_walls = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> FireWallPersistPayload {
    FireWallPersistPayload {
        registry: game_logic.fire_walls.clone(),
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
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
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(FWAL_MAGIC)
                .is_some(),
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
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert_eq!(logic.fire_walls.active_count(), 0);
    }
}
