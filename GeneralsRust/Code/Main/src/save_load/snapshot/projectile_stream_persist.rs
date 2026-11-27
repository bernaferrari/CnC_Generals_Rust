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

use super::lifecycle_records::LifecycleRecords;
use generals_sim::combat::projectile_stream::ProjectileStreamRegistry;
use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const PJST_MAGIC: &[u8; 4] = b"PJST";
const PJST_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProjectileStreamPersistPayload {
    registry: ProjectileStreamRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(PJST_MAGIC);
    append_u32(bytes, PJST_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous trail.
    game_logic.projectile_streams.clear();
    let Some(record) = records.get(PJST_MAGIC) else {
        return Ok(());
    };
    match record.version() {
        2 => {
            let payload: ProjectileStreamPersistPayload = record.decode_payload()?;
            game_logic.projectile_streams = payload.registry;
        }
        1 => {
            #[derive(Deserialize)]
            struct V1Payload {
                registry: generals_sim::combat::projectile_stream::ProjectileStreamRegistryV1Compat,
            }
            let payload: V1Payload = record.decode_payload()?;
            game_logic
                .projectile_streams
                .restore_from_v1_shooter_map(payload.registry.into_shooter_map());
        }
        other => {
            return Err(SaveLoadError::Corrupted(format!(
                "unknown PJST suffix version {other}"
            )));
        }
    }
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ProjectileStreamPersistPayload {
    ProjectileStreamPersistPayload {
        registry: game_logic.projectile_streams.clone(),
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
pub(crate) fn apply_from_lifecycle_tail(bytes: &[u8], world: &mut GameLogic) -> SaveLoadResult<()> {
    apply(&LifecycleRecords::parse(bytes)?, world)
}

#[cfg(test)]
mod tests {
    use super::*;
    use generals_sim::combat::projectile_stream::STREAM_HOLE;
    use crate::game_logic::ObjectId;
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_flame_trail() {
        // Drive PJST append/apply directly so this residual does not depend on
        // the full WorldSnapshot CommonServices/BaseAI capture path.
        let mut source = GameLogic::new();
        source.projectile_streams.add_projectile(
            ObjectId(7),
            0,
            "DragonTankFlameStream",
            Vec3::new(10.0, 0.0, 4.0),
            Some(ObjectId(11)),
            None,
            40,
        );
        source.projectile_streams.add_projectile(
            ObjectId(7),
            0,
            "DragonTankFlameStream",
            Vec3::new(12.0, 0.0, 5.0),
            Some(ObjectId(12)),
            None,
            41,
        );

        let mut tail = Vec::new();
        append_to_lifecycle_tail(&mut tail, &source);
        assert!(
            LifecycleRecords::parse(&tail)
                .unwrap()
                .get(PJST_MAGIC)
                .is_some(),
            "PJST suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.projectile_streams.add_projectile(
            ObjectId(99),
            0,
            "ToxinStream",
            Vec3::ZERO,
            None,
            None,
            1,
        );
        apply(&LifecycleRecords::parse(&tail).unwrap(), &mut restored).expect("restore");

        let streams = restored.projectile_streams.snapshot();
        assert_eq!(streams.len(), 1);
        let (key, state) = streams[0];
        assert_eq!(key.shooter, ObjectId(7));
        assert_eq!(key.slot, 0);
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
                .any(|(key, _)| key.shooter == ObjectId(99)),
            "stale pre-load trail must not leak"
        );
    }

    #[test]
    fn absent_suffix_clears_stale_trails() {
        let mut logic = GameLogic::new();
        logic.projectile_streams.add_projectile(
            ObjectId(1),
             0,
            "DragonTankFlameStream",
            Vec3::ZERO,
            None,
            None,
            4,
        );
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert!(logic.projectile_streams.is_empty());
    }
}
