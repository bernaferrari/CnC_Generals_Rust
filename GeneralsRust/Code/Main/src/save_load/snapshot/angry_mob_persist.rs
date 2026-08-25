//! Persist live Angry Mob SpawnBehavior roster.
//!
//! C++ `SpawnBehavior::xfer` v2 (`SpawnBehavior.cpp:1058-1127`) writes
//! `m_initialBurstTimesInited`, spawn template, `m_oneShotCountdown`,
//! `m_framesToWait`, `m_firstBatchCount`, `m_replacementTimes`, `m_spawnIDs`,
//! `m_active`, `m_aggregateHealth`, `m_spawnCount`. Leftover
//! `SpawnBehavior::xfer` already matches that table. Live Angry Mob is a
//! separate `GameLogic.angry_mobs` registry (`HostAngryMobState`:
//! `member_ids`, `replacement_times`, `member_count` as `m_spawnCount`).
//! Those records were live-only — a mid-roster save dropped dead-member
//! replacement delays and the slave list.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore always clears the live registry first so a load cannot leak the
//! previous session's roster.

use crate::game_logic::GameLogic;
use crate::game_logic::host_angry_mob::HostAngryMobRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const AMOB_MAGIC: &[u8; 4] = b"AMOB";
const AMOB_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AngryMobPersistPayload {
    registry: HostAngryMobRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.active_count() == 0 && payload.registry.members_spawned == 0 {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
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
    let payload: AngryMobPersistPayload = bincode::deserialize(&rest[..payload_len])
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
    fn snapshot_round_trips_angry_mob_roster() {
        let mut source = GameLogic::new();
        let nexus = ObjectId(7);
        source
            .angry_mobs
            .sync_mobs(&[(nexus, Team::GLA, Vec3::new(40.0, 0.0, 12.0))], 30);
        {
            let mob = &mut source.angry_mobs.active_mobs_mut()[0];
            mob.member_ids = vec![ObjectId(11), ObjectId(12), ObjectId(13)];
            mob.member_count = 3;
            mob.replacement_times = vec![930, 1830];
            mob.next_template_index = 3;
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
            .sync_mobs(&[(ObjectId(99), Team::USA, Vec3::ZERO)], 1);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let reg = restored.angry_mobs();
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.members_spawned, 5);
        let mob = &reg.active_mobs()[0];
        assert_eq!(mob.object_id, nexus);
        assert_eq!(mob.team, Team::GLA);
        assert_eq!(
            mob.member_ids,
            vec![ObjectId(11), ObjectId(12), ObjectId(13)]
        );
        assert_eq!(mob.member_count, 3);
        assert_eq!(mob.replacement_times, vec![930, 1830]);
        assert_eq!(mob.next_template_index, 3);
    }

    #[test]
    fn absent_suffix_clears_stale_angry_mobs() {
        let mut logic = GameLogic::new();
        logic
            .angry_mobs
            .sync_mobs(&[(ObjectId(1), Team::GLA, Vec3::ZERO)], 0);
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert_eq!(logic.angry_mobs.active_count(), 0);
    }
}
