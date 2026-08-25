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

use crate::game_logic::GameLogic;
use crate::game_logic::host_inferno_cannon::HostInfernoFireZoneRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
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
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(INFR_MAGIC);
    append_u32(bytes, INFR_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    game_logic.inferno_fire_zones.clear();
    let Some(suffix) = find_infr_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != INFR_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown INFR suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "INFR payload truncated".to_string(),
        ));
    }
    let payload: InfernoFirePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("INFR payload decode: {err}")))?;
    game_logic.inferno_fire_zones = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> InfernoFirePersistPayload {
    InfernoFirePersistPayload {
        registry: game_logic.inferno_fire_zones.clone(),
    }
}

fn find_infr_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == INFR_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("INFR u32 truncated".to_string()));
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
            find_infr_suffix(&snapshot.lifecycle_tail).is_some(),
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
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.inferno_fire_zones.active_count(), 0);
    }
}
