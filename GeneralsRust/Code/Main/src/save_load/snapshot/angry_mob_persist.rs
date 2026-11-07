//! Persist residual Angry Mob combat cadence. SpawnBehavior children and
//! replacement deadlines are persisted by the canonical lifecycle roster.

use crate::game_logic::host_angry_mob::HostAngryMobRegistry;
use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
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

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep the previous roster.
    game_logic.angry_mobs.clear();
    let Some(suffix) = find_amob_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != AMOB_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown AMOB suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "AMOB payload truncated".to_string(),
        ));
    }
    let payload: AngryMobPersistPayload = bincode_legacy::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("AMOB payload decode: {err}")))?;
    game_logic.angry_mobs = payload.registry;
    Ok(())
}

fn capture(game_logic: &GameLogic) -> AngryMobPersistPayload {
    AngryMobPersistPayload {
        registry: game_logic.angry_mobs.clone(),
    }
}

fn find_amob_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == AMOB_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("AMOB u32 truncated".to_string()));
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
            find_amob_suffix(&snapshot.lifecycle_tail).is_some(),
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
        let error = apply_from_lifecycle_tail(&legacy, &mut GameLogic::new()).unwrap_err();
        assert!(error.to_string().contains("unknown AMOB suffix version 1"));
    }

    #[test]
    fn absent_suffix_clears_stale_angry_mobs() {
        let mut logic = GameLogic::new();
        logic
            .angry_mobs
            .sync_mobs(&[(ObjectId(1), Team::GLA, Vec3::ZERO, 1)], 0);
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.angry_mobs.active_count(), 0);
    }
}
