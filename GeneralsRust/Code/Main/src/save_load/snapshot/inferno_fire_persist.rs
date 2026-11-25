//! Persist live Inferno Cannon FireFieldSmall burn zones.
//!
//! C++ FireFieldSmall objects persist via `DeletionUpdate::xfer` `m_dieFrame`
//! and `FireWeaponUpdate::xfer` weapon/`m_initialDelayFrame`. Leftover
//! `deletion_update.rs` / `fire_weapon_update.rs` already match those tables.
//! Live stores expires/next_tick/BlackNapalm on `GameLogic.inferno_fire_zones`.
//! Those records were live-only — a mid-burn save dropped remaining DoT.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore replaces the live registry and never re-runs `spawn_zone`.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::host_inferno_cannon::HostInfernoFireZoneRegistry;
use crate::game_logic::GameLogic;
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const INFR_MAGIC: &[u8; 4] = b"INFR";
const INFR_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct InfernoFirePersistPayload {
    registry: HostInfernoFireZoneRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.active_count() == 0 && payload.registry.zones_spawned == 0 {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(INFR_MAGIC);
    append_u32(bytes, INFR_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    game_logic.inferno_fire_zones.clear();
    let Some(record) = records.get(INFR_MAGIC) else {
        return Ok(());
    };
    let payload: InfernoFirePersistPayload = record.decode(INFR_VERSION)?;
    game_logic.inferno_fire_zones = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> InfernoFirePersistPayload {
    InfernoFirePersistPayload {
        registry: game_logic.inferno_fire_zones.clone(),
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
    fn snapshot_round_trips_inferno_zones() {
        let mut source = GameLogic::new();
        source.inferno_fire_zones.spawn_zone(
            ObjectId(1),
            Team::China,
            Vec3::new(40.0, 0.0, 12.0),
            10,
            true,
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(INFR_MAGIC)
                .is_some(),
            "INFR suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        assert_eq!(restored.inferno_fire_zones.active_count(), 1);
        let zone = &restored.inferno_fire_zones.active_zones()[0];
        assert!(zone.upgraded);
        assert_eq!(
            zone.expires_frame,
            10 + crate::game_logic::INFERNO_FIRE_DURATION_FRAMES
        );
    }

    #[test]
    fn absent_suffix_clears_stale_inferno_zones() {
        let mut logic = GameLogic::new();
        logic
            .inferno_fire_zones
            .spawn_zone(ObjectId(1), Team::China, Vec3::ZERO, 0, false);
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert_eq!(logic.inferno_fire_zones.active_count(), 0);
    }
}
