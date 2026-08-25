//! Persist C++ `DozerAIUpdate` REPAIR task slot.
//!
//! C++ `DozerAIUpdate::xfer` (`DozerAIUpdate.cpp:2432-2477`) writes every
//! `m_task[DOZER_NUM_TASKS]` slot — BUILD and REPAIR target + order frame —
//! plus `m_currentTask`, `m_dockPoint[][]`, and `m_buildSubTask`. After load
//! the dozer resumes the repair dock.
//!
//! Live `snapshot_builder_tasks` only writes BUILD (`builder_id` /
//! `dozer_task_build_target` / `dozer_task_build_order_frame`). Repair-only
//! dozers are skipped entirely, so a mid-repair save left the dozer idle
//! and the barracks damaged.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No world snapshot version bump.

use crate::game_logic::{AIState, GameLogic, KindOf, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const DZRP_MAGIC: &[u8; 4] = b"DZRP";
const DZRP_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DozerRepairPersistPayload {
    objects: Vec<ObjectDozerRepairPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectDozerRepairPersist {
    object_id: u32,
    /// C++ `m_task[DOZER_TASK_REPAIR].m_targetObjectID` (0 = none).
    repair_target: u32,
    /// C++ `m_task[DOZER_TASK_REPAIR].m_taskOrderFrame`.
    repair_order_frame: u32,
    /// C++ `m_dockPoint[DOZER_TASK_REPAIR][DOZER_DOCK_POINT_ACTION]` xyz.
    dock_action: Option<[f32; 3]>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(DZRP_MAGIC);
    append_u32(bytes, DZRP_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_dzrp_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != DZRP_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown DZRP suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "DZRP payload truncated".to_string(),
        ));
    }
    let payload: DozerRepairPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("DZRP payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> DozerRepairPersistPayload {
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut objects = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if object.dozer_task_repair_target.is_none() && object.dozer_task_repair_order_frame == 0 {
            continue;
        }
        objects.push(ObjectDozerRepairPersist {
            object_id: id.0,
            repair_target: object
                .dozer_task_repair_target
                .map(|tid| tid.0)
                .unwrap_or(0),
            repair_order_frame: object.dozer_task_repair_order_frame,
            dock_action: object.dozer_dock_action.map(|p| [p.x, p.y, p.z]),
        });
    }
    DozerRepairPersistPayload { objects }
}

fn apply_payload(game_logic: &mut GameLogic, payload: DozerRepairPersistPayload) {
    for entry in payload.objects {
        let repair_target = if entry.repair_target == 0 {
            None
        } else {
            Some(ObjectId(entry.repair_target))
        };
        let dock = entry.dock_action.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
        {
            let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
                continue;
            };
            object.dozer_task_repair_target = repair_target;
            object.dozer_task_repair_order_frame = entry.repair_order_frame;
            if dock.is_some() {
                object.dozer_dock_action = dock;
            }
        }
        resume_repair_if_idle(game_logic, ObjectId(entry.object_id), repair_target);
    }
}

/// C++ post-load `m_currentTask == DOZER_TASK_REPAIR` resumes the dock.
/// Live idle-resume only restarts BUILD; a parked REPAIR slot must re-enter
/// `AIState::Repairing` so the next support tick heals the building.
fn resume_repair_if_idle(
    game_logic: &mut GameLogic,
    dozer_id: ObjectId,
    repair_target: Option<ObjectId>,
) {
    let Some(tid) = repair_target else {
        return;
    };
    let keep = game_logic.host_object(tid).is_some_and(|target| {
        target.is_alive()
            && target.is_kind_of(KindOf::Structure)
            && !target.status.under_construction
            && target.health.current + 0.01 < target.health.maximum
    });
    if !keep {
        return;
    }
    let Some(object) = game_logic.host_object_mut(dozer_id) else {
        return;
    };
    if matches!(object.ai_state, AIState::Repairing) {
        if object.target.is_none() {
            object.target = Some(tid);
        }
        return;
    }
    if !matches!(object.ai_state, AIState::Idle) {
        return;
    }
    object.target = Some(tid);
    object.set_ai_state(AIState::Repairing);
    object.idle_since_frame = 0;
}

fn find_dzrp_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == DZRP_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("DZRP u32 truncated".to_string()));
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
        assert!(find_dzrp_suffix(b"no-magic-here").is_none());
        let mut logic = GameLogic::new();
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
    }

    #[test]
    fn snapshot_round_trips_dozer_repair_task() {
        let mut source = GameLogic::new();
        let mut dozer_tmpl = ThingTemplate::new("AmericaVehicleDozer");
        dozer_tmpl.add_kind_of(KindOf::Dozer);
        source
            .templates
            .insert("AmericaVehicleDozer".to_string(), dozer_tmpl);
        let mut barracks = ThingTemplate::new("AmericaBarracks");
        barracks.add_kind_of(KindOf::Structure);
        barracks.set_health(500.0);
        source
            .templates
            .insert("AmericaBarracks".to_string(), barracks);
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let dozer = source
            .create_object("AmericaVehicleDozer", Team::USA, Vec3::ZERO)
            .expect("dozer");
        let damaged = source
            .create_object("AmericaBarracks", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("barracks");
        {
            let building = source.host_object_mut(damaged).expect("building");
            building.health.current = 200.0;
            building.set_status_under_construction(false);
        }
        {
            let unit = source.host_object_mut(dozer).expect("dozer obj");
            unit.dozer_task_repair_target = Some(damaged);
            unit.dozer_task_repair_order_frame = 88;
            unit.dozer_dock_action = Some(Vec3::new(16.0, 0.0, 0.0));
            unit.set_ai_state(AIState::Idle);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_dzrp_suffix(&snapshot.lifecycle_tail).is_some(),
            "DZRP suffix must be appended to lifecycle tail"
        );
        assert!(
            snapshot
                .builder_tasks
                .iter()
                .all(|entry| entry.object_id != dozer
                    || entry.dozer_task_build_target.is_some()
                    || entry.builder_id.is_some()),
            "repair-only dozers stay off the BUILD-only builder_tasks tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.host_object(dozer).expect("restored dozer");
        assert_eq!(loaded.dozer_task_repair_target, Some(damaged));
        assert_eq!(loaded.dozer_task_repair_order_frame, 88);
        let dock = loaded.dozer_dock_action.expect("dock");
        assert!((dock.x - 16.0).abs() < 0.01);
        assert_eq!(loaded.target, Some(damaged));
        assert_eq!(loaded.ai_state, AIState::Repairing);
        let building = restored.host_object(damaged).expect("restored barracks");
        assert!((building.health.current - 200.0).abs() < 0.01);
    }
}
