//! Persist live Helix / HistoricBonus Firestorm DoT geometry.
//!
//! C++ `FirestormDynamicGeometryInfoUpdate::xfer`
//! (`FirestormDynamicGeometryInfoUpdate.cpp:264-286`) writes v1,
//! `DynamicGeometryInfoUpdate` countdown/started/radii/direction, particle
//! ids, `m_effectsFired`, `m_scorchPlaced`, and `m_lastDamageFrame`. Leftover
//! `FirestormDynamicGeometryInfoUpdate::xfer` already matches that table.
//! Live host stores mid-storm state on `GameLogic.helix_napalm`
//! (`HostHelixFirestormZone`: activate/expires, radius, switched_directions,
//! scorch_placed, next_tick_frame). Those records were live-only — a mid-
//! firestorm save dropped remaining DoT ticks, expand/reverse, and scorch.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::host_helix_napalm::HostHelixNapalmRegistry;
use crate::game_logic::GameLogic;
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const FSGM_MAGIC: &[u8; 4] = b"FSGM";
const FSGM_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HelixNapalmPersistPayload {
    registry: HostHelixNapalmRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.active_count() == 0
        && payload.registry.drops == 0
        && payload.registry.zones_spawned == 0
    {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(FSGM_MAGIC);
    append_u32(bytes, FSGM_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load firestorms.
    game_logic.helix_napalm.clear();
    let Some(record) = records.get(FSGM_MAGIC) else {
        return Ok(());
    };
    let payload: HelixNapalmPersistPayload = record.decode(FSGM_VERSION)?;
    game_logic.helix_napalm = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> HelixNapalmPersistPayload {
    HelixNapalmPersistPayload {
        registry: game_logic.helix_napalm.clone(),
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
    fn snapshot_round_trips_firestorm_dot_geometry() {
        let mut source = GameLogic::new();
        source.helix_napalm.record_drop_and_spawn_firestorm(
            ObjectId(7),
            Team::China,
            Vec3::new(120.0, 0.0, 40.0),
            30,
            false,
            2,
            150.0,
        );
        {
            let zone = &mut source.helix_napalm.active_zones_mut()[0];
            zone.radius = 45.0;
            zone.next_tick_frame = 60;
            zone.switched_directions = true;
            zone.scorch_placed = false;
            zone.total_damage_applied = 25.0;
            zone.damage_applications = 1;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(FSGM_MAGIC)
                .is_some(),
            "FSGM suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.helix_napalm.record_drop_and_spawn_firestorm(
            ObjectId(99),
            Team::USA,
            Vec3::ZERO,
            1,
            true,
            0,
            0.0,
        );
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let reg = restored.helix_napalm();
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.drops, 1);
        assert_eq!(reg.zones_spawned, 1);
        let zone = &reg.active_zones()[0];
        assert_eq!(zone.source_object, ObjectId(7));
        assert_eq!(zone.source_team, Team::China);
        assert_eq!(zone.activate_frame, 30);
        assert_eq!(zone.next_tick_frame, 60);
        assert!(zone.switched_directions);
        assert!(!zone.scorch_placed);
        assert!((zone.radius - 45.0).abs() < 0.01);
        assert!((zone.position.x - 120.0).abs() < 0.01);
        assert!((zone.total_damage_applied - 25.0).abs() < 0.01);
    }

    #[test]
    fn absent_suffix_clears_stale_firestorms() {
        let mut logic = GameLogic::new();
        logic.helix_napalm.record_drop_and_spawn_firestorm(
            ObjectId(1),
            Team::China,
            Vec3::ZERO,
            0,
            false,
            0,
            0.0,
        );
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert_eq!(logic.helix_napalm.active_count(), 0);
    }
}
