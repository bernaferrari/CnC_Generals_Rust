//! Persist C++ `ProductionUpdate` door + construction-complete residuals.
//!
//! C++ `ProductionUpdate::xfer` (`ProductionUpdate.cpp:1203-1376`) writes
//! `m_constructionCompleteFrame` and the raw `m_doors[DOOR_COUNT_MAX]`
//! `DoorInfo` array (`m_doorOpenedFrame` / `m_doorWaitOpenFrame` /
//! `m_doorClosedFrame` / `m_holdOpen`). After load a mid-open War Factory
//! door finishes opening and the waiting unit exits.
//!
//! Live host stores that table on `Object` as `production_door_phases[4]`,
//! `production_door_phase_end_frames[4]`, `production_door_active_index`,
//! `production_door_hold_open`, and `construction_complete_clear_frame`.
//! `ProductionModuleSnapshot` only covers queue / quantity / exit delay, so
//! a mid-open save snapped the door idle and stalled the unit.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL / SUBD / HSQD / BTRY / CBPD) so older decoders
//! ignore the extra bytes. No world snapshot version bump.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const PDRP_MAGIC: &[u8; 4] = b"PDRP";
const PDRP_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProductionDoorPersistPayload {
    objects: Vec<ObjectProductionDoorPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectProductionDoorPersist {
    object_id: u32,
    /// C++ `DoorInfo` phase per hangar (0 idle, 1 opening, 2 wait, 4 closing).
    phases: [u8; 4],
    /// Absolute frame when each door's current residual phase ends.
    phase_end_frames: [u32; 4],
    /// Reserved `ExitDoorType` index (0=DOOR_1 .. 3=DOOR_4).
    active_index: u8,
    /// C++ `DoorInfo::m_holdOpen`.
    hold_open: bool,
    /// Absolute frame when CONSTRUCTION_COMPLETE should clear (0 = inactive).
    construction_complete_clear_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(PDRP_MAGIC);
    append_u32(bytes, PDRP_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_pdrp_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != PDRP_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown PDRP suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "PDRP payload truncated".to_string(),
        ));
    }
    let payload: ProductionDoorPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("PDRP payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ProductionDoorPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let has_doors = object.production_door_phases.iter().any(|&p| p != 0)
            || object.production_door_phase != 0
            || object.production_door_hold_open
            || object.production_door_active_index != 0
            || object.construction_complete_clear_frame != 0;
        if !has_doors {
            continue;
        }
        let mut phases = object.production_door_phases;
        let mut ends = object.production_door_phase_end_frames;
        if phases[0] == 0 && object.production_door_phase != 0 {
            phases[0] = object.production_door_phase;
            ends[0] = object.production_door_phase_end_frame;
        }
        objects.push(ObjectProductionDoorPersist {
            object_id: id.0,
            phases,
            phase_end_frames: ends,
            active_index: object.production_door_active_index,
            hold_open: object.production_door_hold_open,
            construction_complete_clear_frame: object.construction_complete_clear_frame,
        });
    }
    ProductionDoorPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ProductionDoorPersistPayload) {
    for entry in payload.objects {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.production_door_phases = entry.phases;
        object.production_door_phase_end_frames = entry.phase_end_frames;
        object.production_door_active_index = entry.active_index;
        object.production_door_hold_open = entry.hold_open;
        object.construction_complete_clear_frame = entry.construction_complete_clear_frame;
        let active = (entry.active_index as usize).min(3);
        let phase = entry.phases[active];
        let end = entry.phase_end_frames[active];
        object.apply_production_door_phase_residual(phase);
        object.production_door_phases = entry.phases;
        object.production_door_phase_end_frames = entry.phase_end_frames;
        object.production_door_phase_end_frames[active] = end;
        object.production_door_phase = object.production_door_phases[0];
        object.production_door_phase_end_frame = object.production_door_phase_end_frames[0];
        if entry.construction_complete_clear_frame > 0 {
            use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
            object.model_condition_bits |= 1u128 << construction_complete_model_bit();
        }
    }
}

fn find_pdrp_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == PDRP_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("PDRP u32 truncated".to_string()));
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
        assert!(find_pdrp_suffix(b"no-magic-here").is_none());
        let mut logic = GameLogic::new();
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
    }

    #[test]
    fn snapshot_round_trips_war_factory_door_phase() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaWarFactory".to_string(),
            ThingTemplate::new("AmericaWarFactory"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let factory_id = source
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(20.0, 0.0, 16.0))
            .expect("factory");
        {
            let factory = source.host_object_mut(factory_id).expect("factory");
            factory.production_door_phases = [1, 0, 0, 0];
            factory.production_door_phase_end_frames = [140, 0, 0, 0];
            factory.production_door_active_index = 0;
            factory.production_door_hold_open = true;
            factory.production_door_phase = 1;
            factory.production_door_phase_end_frame = 140;
            factory.construction_complete_clear_frame = 90;
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_pdrp_suffix(&snapshot.lifecycle_tail).is_some(),
            "PDRP suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(factory_id).expect("restored factory");
        assert_eq!(loaded.production_door_phases, [1, 0, 0, 0]);
        assert_eq!(loaded.production_door_phase_end_frames, [140, 0, 0, 0]);
        assert_eq!(loaded.production_door_active_index, 0);
        assert!(loaded.production_door_hold_open);
        assert_eq!(loaded.production_door_phase, 1);
        assert_eq!(loaded.production_door_phase_end_frame, 140);
        assert_eq!(loaded.construction_complete_clear_frame, 90);

        let mut factory = restored.host_object_mut(factory_id).expect("tick factory");
        // Hold-open keeps WAITING_OPEN; the door must not snap idle.
        assert!(!factory.tick_production_door(140));
        assert_eq!(factory.production_door_phase, 2);
        assert_eq!(factory.production_door_phases[0], 2);
    }
}
