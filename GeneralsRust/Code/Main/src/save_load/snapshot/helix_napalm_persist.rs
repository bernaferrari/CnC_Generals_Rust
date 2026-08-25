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

use crate::game_logic::GameLogic;
use crate::game_logic::host_helix_napalm::HostHelixNapalmRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
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
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(FSGM_MAGIC);
    append_u32(bytes, FSGM_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load firestorms.
    game_logic.helix_napalm.clear();
    let Some(suffix) = find_fsgm_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != FSGM_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown FSGM suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "FSGM payload truncated".to_string(),
        ));
    }
    let payload: HelixNapalmPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("FSGM payload decode: {err}")))?;
    game_logic.helix_napalm = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> HelixNapalmPersistPayload {
    HelixNapalmPersistPayload {
        registry: game_logic.helix_napalm.clone(),
    }
}

fn find_fsgm_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == FSGM_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("FSGM u32 truncated".to_string()));
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
            find_fsgm_suffix(&snapshot.lifecycle_tail).is_some(),
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
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.helix_napalm.active_count(), 0);
    }
}
