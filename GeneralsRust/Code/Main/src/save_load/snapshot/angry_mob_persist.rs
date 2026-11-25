//! Persist residual Angry Mob combat cadence. SpawnBehavior children and
//! replacement deadlines are persisted by the canonical lifecycle roster.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::host_angry_mob::HostAngryMobRegistry;
use crate::game_logic::GameLogic;
use crate::save_load::SaveLoadResult;
use serde::{Deserialize, Serialize};

const AMOB_MAGIC: &[u8; 4] = b"AMOB";
const AMOB_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AngryMobPersistPayload {
    registry: HostAngryMobRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.active_count() == 0 && payload.registry.members_spawned == 0 {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(AMOB_MAGIC);
    append_u32(bytes, AMOB_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn apply(
    records: &LifecycleRecords<'_>,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous roster.
    game_logic.angry_mobs.clear();
    let Some(record) = records.get(AMOB_MAGIC) else {
        return Ok(());
    };
    let payload: AngryMobPersistPayload = record.decode(AMOB_VERSION)?;
    game_logic.angry_mobs = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> AngryMobPersistPayload {
    AngryMobPersistPayload {
        registry: game_logic.angry_mobs.clone(),
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
    fn snapshot_round_trips_angry_mob_combat_cadence() {
        let mut source = GameLogic::new();
        let nexus = ObjectId(7);
        source
            .angry_mobs
            .sync_mobs(&[(nexus, Team::GLA, Vec3::new(40.0, 0.0, 12.0), 1)], 30);
        {
            let mob = &mut source.angry_mobs.active_mobs_mut()[0];
            mob.next_tick_frame = 42;
            mob.total_damage_applied = 17.0;
        }
        source.angry_mobs.members_spawned = 5;

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(AMOB_MAGIC)
                .is_some(),
            "AMOB suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored
            .angry_mobs
            .sync_mobs(&[(ObjectId(99), Team::USA, Vec3::ZERO, 1)], 1);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let reg = restored.angry_mobs();
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.members_spawned, 5);
        let mob = &reg.active_mobs()[0];
        assert_eq!(mob.object_id, nexus);
        assert_eq!(mob.team, Team::GLA);
        assert_eq!(mob.next_tick_frame, 42);
        assert_eq!(mob.total_damage_applied, 17.0);
        // Runtime generations are rebound on the first receiving-world sync.
        restored.angry_mobs.sync_mobs(
            &[(nexus, Team::GLA, Vec3::new(40.0, 0.0, 12.0), 2)],
            31,
        );
        let mob = &restored.angry_mobs().active_mobs()[0];
        assert_eq!(mob.next_tick_frame, 42);
        assert_eq!(mob.total_damage_applied, 17.0);
    }

    #[test]
    fn pre_cutover_registry_payload_is_rejected() {
        let mut legacy = b"AMOB".to_vec();
        append_u32(&mut legacy, 1);
        append_u32(&mut legacy, 0);
        let error = apply(
            &LifecycleRecords::parse(&legacy).unwrap(),
            &mut GameLogic::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown AMOB suffix version 1"));
    }

    #[test]
    fn absent_suffix_clears_stale_angry_mobs() {
        let mut logic = GameLogic::new();
        logic
            .angry_mobs
            .sync_mobs(&[(ObjectId(1), Team::GLA, Vec3::ZERO, 1)], 0);
        apply(&LifecycleRecords::parse(&[]).unwrap(), &mut logic).expect("apply");
        assert_eq!(logic.angry_mobs.active_count(), 0);
    }
}
