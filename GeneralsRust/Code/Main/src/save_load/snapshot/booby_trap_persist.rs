//! Persist live `HostBoobyTrapRegistry` and `OBJECT_STATUS_BOOBY_TRAPPED`.
//!
//! C++ `StickyBombUpdate::xfer` (`StickyBombUpdate.cpp:280-299`) writes
//! `m_targetID` / `m_dieFrame` / `m_nextPingFrame`. Leftover
//! `StickyBombUpdate` already matches that table, and leftover `Object::xfer`
//! already writes `BOOBY_TRAPPED` in the named status mask. Live host plants
//! live in `HostBoobyTrapRegistry` plus `Object.status.booby_trapped`, neither
//! of which was in `WorldSnapshot`, so quickload wiped planted traps.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL / SUBD / HSQD) so older decoders ignore the extra
//! bytes. No world snapshot version bump.

use crate::game_logic::host_booby_trap::HostBoobyTrapRegistry;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const BTRY_MAGIC: &[u8; 4] = b"BTRY";
const BTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BoobyTrapPersistPayload {
    registry: HostBoobyTrapRegistry,
    objects_spawned: u32,
    /// Object ids that carried `OBJECT_STATUS_BOOBY_TRAPPED`.
    trapped_ids: Vec<u32>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.active_count() == 0
        && payload.registry.plants_total == 0
        && payload.registry.detonations_total == 0
        && payload.objects_spawned == 0
        && payload.trapped_ids.is_empty()
    {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(BTRY_MAGIC);
    append_u32(bytes, BTRY_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_btry_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != BTRY_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown BTRY suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "BTRY payload truncated".to_string(),
        ));
    }
    let payload: BoobyTrapPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("BTRY payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> BoobyTrapPersistPayload {
    let registry = game_logic.booby_trap_residual().clone();
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut trapped_ids = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if object.status.booby_trapped {
            trapped_ids.push(id.0);
        }
    }
    BoobyTrapPersistPayload {
        registry,
        objects_spawned: game_logic.booby_trap_objects_spawned(),
        trapped_ids,
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: BoobyTrapPersistPayload) {
    let mut trapped: std::collections::HashSet<u32> = payload.trapped_ids.into_iter().collect();
    for plant in payload.registry.plants() {
        trapped.insert(plant.structure_id.0);
    }
    game_logic.restore_booby_traps(payload.registry, payload.objects_spawned);
    for id in trapped {
        let Some(object) = game_logic.host_object_mut(ObjectId(id)) else {
            continue;
        };
        object.set_status_booby_trapped(true);
    }
}

fn find_btry_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == BTRY_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("BTRY u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_btry_suffix(b"no-magic-here").is_none());
    }

    #[test]
    fn snapshot_round_trips_booby_registry_and_status() {
        let mut source = GameLogic::new();
        source
            .templates
            .insert("GLABarracks".to_string(), ThingTemplate::new("GLABarracks"));
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let structure_id = source
            .create_object("GLABarracks", Team::GLA, Vec3::new(40.0, 0.0, 12.0))
            .expect("barracks");
        let mut registry = HostBoobyTrapRegistry::new();
        registry.install(
            structure_id,
            ObjectId(7),
            Team::GLA,
            30,
            8.0,
            Some(ObjectId(99)),
        );
        source.restore_booby_traps(registry, 1);
        source
            .host_object_mut(structure_id)
            .expect("structure")
            .set_status_booby_trapped(true);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_btry_suffix(&snapshot.lifecycle_tail).is_some(),
            "BTRY suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        assert!(
            restored
                .booby_trap_residual()
                .is_booby_trapped(structure_id),
            "planted booby registry must survive load"
        );
        let plant = restored
            .booby_trap_residual()
            .plant(structure_id)
            .expect("plant");
        assert_eq!(plant.planter_id, ObjectId(7));
        assert_eq!(plant.planter_team, Team::GLA);
        assert_eq!(plant.charge_object_id, Some(ObjectId(99)));
        assert_eq!(restored.booby_trap_objects_spawned(), 1);
        let loaded = restored
            .host_object(structure_id)
            .expect("restored barracks");
        assert!(
            loaded.status.booby_trapped,
            "OBJECT_STATUS_BOOBY_TRAPPED must survive load"
        );
    }
}
