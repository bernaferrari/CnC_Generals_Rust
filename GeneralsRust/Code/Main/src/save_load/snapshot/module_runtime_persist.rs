//! Persist Countermeasures / CheckpointUpdate / EnemyNearUpdate runtime.
//!
//! Leftover module xfer already matches C++:
//! - `CountermeasuresBehavior::xfer` v2 (`countermeasures_behavior.rs`)
//! - `CheckpointUpdate::xfer` (`checkpoint_update.rs`)
//! - `EnemyNearUpdate::xfer` (`enemy_near_update.rs`)
//!
//! Live snapshot/restore omitted those tables. `ensure()` rebuilt a full flare
//! load on first post-load use; checkpoint/enemy-near reconstruct as Default
//! (`open: false`, `enemy_near: false`) so an open gate snaps shut and a wall
//! drops ENEMYNEAR until the next scan.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload so
//! older decoders ignore the extra bytes. No WorldSnapshot version bump.

use crate::game_logic::host_checkpoint_update::HostCheckpointUpdateData;
use crate::game_logic::host_countermeasures::{
    HostCountermeasuresState, PendingCountermeasureFlareSpawn,
};
use crate::game_logic::host_enemy_near::{ENEMY_NEAR_MODEL_CONDITION, HostEnemyNearData};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const MSRT_MAGIC: &[u8; 4] = b"MSRT";
const MSRT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModuleRuntimePersistPayload {
    countermeasures: Vec<CountermeasuresPersist>,
    checkpoints: Vec<CheckpointPersist>,
    enemy_near: Vec<EnemyNearPersist>,
    pending_flares: Vec<PendingCountermeasureFlareSpawn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CountermeasuresPersist {
    object_id: u32,
    state: HostCountermeasuresState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointPersist {
    object_id: u32,
    data: HostCheckpointUpdateData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnemyNearPersist {
    object_id: u32,
    data: HostEnemyNearData,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.countermeasures.is_empty()
        && payload.checkpoints.is_empty()
        && payload.enemy_near.is_empty()
        && payload.pending_flares.is_empty()
    {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(MSRT_MAGIC);
    append_u32(bytes, MSRT_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Fail-closed: a reused GameLogic must not keep pre-load flare states.
    game_logic.countermeasures.clear();
    let Some(suffix) = find_msrt_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != MSRT_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown MSRT suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "MSRT payload truncated".to_string(),
        ));
    }
    let payload: ModuleRuntimePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("MSRT payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ModuleRuntimePersistPayload {
    let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    ids.sort();

    let mut countermeasures = Vec::new();
    let mut cm_ids = game_logic.countermeasures.aircraft_ids();
    cm_ids.sort_by_key(|id| id.0);
    for id in cm_ids {
        if let Some(state) = game_logic.countermeasures.get(id) {
            countermeasures.push(CountermeasuresPersist {
                object_id: id.0,
                state: state.clone(),
            });
        }
    }

    let mut checkpoints = Vec::new();
    let mut enemy_near = Vec::new();
    for id in ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if let Some(data) = object.checkpoint_update.clone() {
            checkpoints.push(CheckpointPersist {
                object_id: id.0,
                data,
            });
        }
        if let Some(data) = object.enemy_near.clone() {
            enemy_near.push(EnemyNearPersist {
                object_id: id.0,
                data,
            });
        }
    }

    ModuleRuntimePersistPayload {
        countermeasures,
        checkpoints,
        enemy_near,
        pending_flares: game_logic.countermeasures.pending_flare_spawns().to_vec(),
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ModuleRuntimePersistPayload) {
    game_logic.countermeasures.clear();
    for entry in payload.countermeasures {
        game_logic
            .countermeasures
            .restore_state(ObjectId(entry.object_id), entry.state);
    }
    game_logic
        .countermeasures
        .restore_pending_flare_spawns(payload.pending_flares);

    for entry in payload.checkpoints {
        if let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) {
            apply_checkpoint_door_bits(object, &entry.data);
            object.checkpoint_update = Some(entry.data);
        }
    }
    for entry in payload.enemy_near {
        if let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) {
            apply_enemy_near_bits(object, &entry.data);
            object.enemy_near = Some(entry.data);
        }
    }
}

fn apply_checkpoint_door_bits(
    object: &mut crate::game_logic::Object,
    data: &HostCheckpointUpdateData,
) {
    use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
    if let Some(bit) = model_condition_bit_name_index("DOOR_1_OPENING") {
        object.model_condition_bits &= !(1u128 << bit);
    }
    if let Some(bit) = model_condition_bit_name_index("DOOR_1_CLOSING") {
        object.model_condition_bits &= !(1u128 << bit);
    }
    if let Some(name) = data.door_anim.model_condition() {
        if let Some(bit) = model_condition_bit_name_index(name) {
            object.model_condition_bits |= 1u128 << bit;
        }
    }
    object.record_host_model_condition();
}

fn apply_enemy_near_bits(object: &mut crate::game_logic::Object, data: &HostEnemyNearData) {
    use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
    let Some(bit) = model_condition_bit_name_index(ENEMY_NEAR_MODEL_CONDITION) else {
        return;
    };
    if data.model_enemy_near || data.enemy_near {
        object.model_condition_bits |= 1u128 << bit;
    } else {
        object.model_condition_bits &= !(1u128 << bit);
    }
    object.record_host_model_condition();
}

fn find_msrt_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == MSRT_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("MSRT u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_checkpoint_update::CheckpointDoorAnim;
    use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
    use crate::game_logic::{GameLogic, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_msrt_suffix(b"no-magic-here").is_none());
        let mut logic = GameLogic::new();
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.countermeasures.aircraft_ids().is_empty());
    }

    #[test]
    fn snapshot_round_trips_flare_gate_and_wall_latch() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaJetRaptor".to_string(),
            ThingTemplate::new("AmericaJetRaptor"),
        );
        source.templates.insert(
            "AmericaCheckpoint".to_string(),
            ThingTemplate::new("AmericaCheckpoint"),
        );
        source.templates.insert(
            "AmericaWallSegment".to_string(),
            ThingTemplate::new("AmericaWallSegment"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));

        let raptor = source
            .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 20.0, 0.0))
            .expect("raptor");
        let gate = source
            .create_object("AmericaCheckpoint", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .expect("gate");
        let wall = source
            .create_object("AmericaWallSegment", Team::USA, Vec3::new(80.0, 0.0, 0.0))
            .expect("wall");

        let mut cm = HostCountermeasuresState::full_load();
        cm.available = 12;
        cm.active = 4;
        cm.incoming_missiles = 3;
        cm.diverted_missiles = 2;
        cm.volleys_fired = 2;
        cm.reaction_frame = 44;
        cm.reaction_armed = true;
        cm.next_volley_frame = 74;
        cm.flare_ids = vec![ObjectId(501), ObjectId(502), ObjectId(503), ObjectId(504)];
        source.countermeasures.restore_state(raptor, cm);
        source.countermeasures.restore_pending_flare_spawns(vec![
            PendingCountermeasureFlareSpawn {
                aircraft_id: raptor,
                frame: 40,
                volley_index: 1,
            },
        ]);

        {
            let object = source.host_object_mut(gate).expect("gate");
            let mut data = object
                .checkpoint_update
                .clone()
                .unwrap_or_else(HostCheckpointUpdateData::default);
            data.enemy_near = false;
            data.ally_near = true;
            data.scan_delay = 17;
            data.open = true;
            data.door_anim = CheckpointDoorAnim::Opening;
            data.path_radius = 3.5;
            object.checkpoint_update = Some(data);
        }
        {
            let object = source.host_object_mut(wall).expect("wall");
            let mut data = object
                .enemy_near
                .clone()
                .unwrap_or_else(HostEnemyNearData::default);
            data.enemy_near = true;
            data.model_enemy_near = true;
            data.scan_delay = 11;
            object.enemy_near = Some(data);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_msrt_suffix(&snapshot.lifecycle_tail).is_some(),
            "MSRT suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded_cm = restored
            .countermeasures
            .get(raptor)
            .expect("restored flare registry");
        assert_eq!(loaded_cm.available, 12);
        assert_eq!(loaded_cm.active, 4);
        assert_eq!(loaded_cm.incoming_missiles, 3);
        assert_eq!(loaded_cm.diverted_missiles, 2);
        assert_eq!(loaded_cm.volleys_fired, 2);
        assert_eq!(loaded_cm.reaction_frame, 44);
        assert!(loaded_cm.reaction_armed);
        assert_eq!(loaded_cm.next_volley_frame, 74);
        assert_eq!(
            loaded_cm.flare_ids,
            vec![ObjectId(501), ObjectId(502), ObjectId(503), ObjectId(504)]
        );
        let pending = restored.countermeasures.pending_flare_spawns();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].aircraft_id, raptor);
        assert_eq!(pending[0].frame, 40);
        assert_eq!(pending[0].volley_index, 1);
        // Post-load ensure must not rebuild a full 20-flare load.
        assert_eq!(restored.countermeasures.ensure(raptor).available, 12);

        let loaded_gate = restored.host_object(gate).expect("restored gate");
        let cp = loaded_gate
            .checkpoint_update
            .as_ref()
            .expect("checkpoint latch");
        assert!(!cp.enemy_near);
        assert!(cp.ally_near);
        assert_eq!(cp.scan_delay, 17);
        assert!(cp.open, "open gate must not snap shut after load");
        assert_eq!(cp.door_anim, CheckpointDoorAnim::Opening);
        assert!((cp.path_radius - 3.5).abs() < f32::EPSILON);
        if let Some(bit) = model_condition_bit_name_index("DOOR_1_OPENING") {
            assert_ne!(
                loaded_gate.model_condition_bits & (1u128 << bit),
                0,
                "open gate must keep DOOR_1_OPENING"
            );
        }

        let loaded_wall = restored.host_object(wall).expect("restored wall");
        let en = loaded_wall.enemy_near.as_ref().expect("enemy-near latch");
        assert!(en.enemy_near);
        assert!(en.model_enemy_near);
        assert_eq!(en.scan_delay, 11);
        if let Some(bit) = model_condition_bit_name_index(ENEMY_NEAR_MODEL_CONDITION) {
            assert_ne!(
                loaded_wall.model_condition_bits & (1u128 << bit),
                0,
                "wall must keep ENEMYNEAR pose after load"
            );
        }
    }
}
