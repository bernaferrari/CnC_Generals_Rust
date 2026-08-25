//! Persist live ProjectileStream trail rings.
//!
//! C++ `ProjectileStreamUpdate::xfer` v2
//! (`ProjectileStreamUpdate.cpp:209-238`) writes UpdateModule base,
//! `m_projectileIDs[MAX_PROJECTILE_STREAM]`, `m_nextFreeIndex`,
//! `m_firstValidIndex`, `m_owningObject`, and v2 `m_targetObject` +
//! `m_targetPosition`. Leftover `ProjectileStreamUpdate::xfer` already
//! matches that table. Live stores the same residual on
//! `GameLogic.projectile_streams` (`ProjectileStreamState`: ring of
//! points, target id/pos). Construct / reset clear the registry — a
//! mid-flame save dropped the ribbon.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore replaces the live registry and never calls `add_projectile`, so
//! a load cannot insert a retarget hole or re-create the trail.

use crate::game_logic::GameLogic;
use crate::game_logic::host_projectile_stream::ProjectileStreamRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const PJST_MAGIC: &[u8; 4] = b"PJST";
const PJST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProjectileStreamPersistPayload {
    registry: ProjectileStreamRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(PJST_MAGIC);
    append_u32(bytes, PJST_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous trail.
    game_logic.projectile_streams.clear();
    let Some(suffix) = find_pjst_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != PJST_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown PJST suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "PJST payload truncated".to_string(),
        ));
    }
    let payload: ProjectileStreamPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("PJST payload decode: {err}")))?;
    game_logic.projectile_streams = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ProjectileStreamPersistPayload {
    ProjectileStreamPersistPayload {
        registry: game_logic.projectile_streams.clone(),
    }
}

fn find_pjst_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == PJST_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("PJST u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_projectile_stream::STREAM_HOLE;
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_flame_trail() {
        let mut source = GameLogic::new();
        source.projectile_streams.add_projectile(
            ObjectId(7),
            "DragonTankFlameStream",
            Vec3::new(10.0, 0.0, 4.0),
            Some(ObjectId(11)),
            None,
            40,
        );
        source.projectile_streams.add_projectile(
            ObjectId(7),
            "DragonTankFlameStream",
            Vec3::new(12.0, 0.0, 5.0),
            Some(ObjectId(12)),
            None,
            41,
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_pjst_suffix(&snapshot.lifecycle_tail).is_some(),
            "PJST suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.projectile_streams.add_projectile(
            ObjectId(99),
            "ToxinStream",
            Vec3::ZERO,
            None,
            None,
            1,
        );
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let streams = restored.projectile_streams.snapshot();
        assert_eq!(streams.len(), 1);
        let (shooter, state) = streams[0];
        assert_eq!(shooter, ObjectId(7));
        assert_eq!(state.stream_name, "DragonTankFlameStream");
        assert_eq!(state.target_id, Some(ObjectId(12)));
        assert_eq!(state.last_frame, 41);
        assert_eq!(state.points.len(), 3);
        assert_eq!(state.points[1], STREAM_HOLE);
        assert!((state.points[0].x - 10.0).abs() < 0.01);
        assert!((state.points[2].x - 12.0).abs() < 0.01);
        assert!(
            !restored
                .projectile_streams
                .snapshot()
                .iter()
                .any(|(id, _)| *id == ObjectId(99)),
            "stale pre-load trail must not leak"
        );
    }

    #[test]
    fn absent_suffix_clears_stale_trails() {
        let mut logic = GameLogic::new();
        logic.projectile_streams.add_projectile(
            ObjectId(1),
            "DragonTankFlameStream",
            Vec3::ZERO,
            None,
            None,
            4,
        );
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.projectile_streams.is_empty());
    }
}
