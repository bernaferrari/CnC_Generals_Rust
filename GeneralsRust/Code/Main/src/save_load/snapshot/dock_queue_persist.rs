//! Persist C++ `DockUpdate::xfer` approach owners / reached / active docker.
//!
//! C++ `DockUpdate::xfer` (`DockUpdate.cpp`) writes `m_approachPositionOwners`,
//! `m_approachPositionReached`, and `m_activeDocker`. Leftover `dock_update.rs`
//! already matches that table. Live host stores the same slots in the
//! process-global `HostDockApproachQueue` map plus `Object::dock_active_docker`.
//! Those were session-only — a mid-queue save reset waiters and the active
//! docker after load.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No world snapshot version bump.
//! Restore always clears the process-global map first so a load cannot leak
//! the previous session's reservations.

use crate::game_logic::host_supply_gather::{
    HostDockApproachQueue, restore_live_dock_queues, snapshot_live_dock_queues,
};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const DCKQ_MAGIC: &[u8; 4] = b"DCKQ";
const DCKQ_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DockQueuePersistPayload {
    queues: Vec<DockQueuePersist>,
    active: Vec<DockActivePersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DockQueuePersist {
    dock_id: u32,
    number_approach_positions: i32,
    number_approach_position_bones: i32,
    waiting_bones: Vec<[f32; 3]>,
    owners: Vec<u32>,
    reached: Vec<bool>,
    wait_started: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DockActivePersist {
    dock_id: u32,
    /// C++ `m_activeDocker`; 0 = `INVALID_ID`.
    active_docker: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.queues.is_empty() && payload.active.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(DCKQ_MAGIC);
    append_u32(bytes, DCKQ_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Always drop the previous session first (C++ module state is per-object).
    restore_live_dock_queues(Vec::new());
    let Some(suffix) = find_dckq_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != DCKQ_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown DCKQ suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "DCKQ payload truncated".to_string(),
        ));
    }
    let payload: DockQueuePersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("DCKQ payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> DockQueuePersistPayload {
    let queues = snapshot_live_dock_queues()
        .into_iter()
        .map(|(dock_id, queue)| DockQueuePersist {
            dock_id: dock_id.0,
            number_approach_positions: queue.number_approach_positions,
            number_approach_position_bones: queue.number_approach_position_bones,
            waiting_bones: queue
                .waiting_bones
                .iter()
                .map(|bone| [bone.x, bone.y, bone.z])
                .collect(),
            owners: queue
                .owners
                .iter()
                .map(|owner| owner.map(|id| id.0).unwrap_or(0))
                .collect(),
            reached: queue.reached.clone(),
            wait_started: {
                let mut waits: Vec<(u32, u32)> = queue
                    .wait_started
                    .iter()
                    .map(|(id, frame)| (id.0, *frame))
                    .collect();
                waits.sort_by_key(|(id, _)| *id);
                waits
            },
        })
        .collect();

    let mut dock_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    dock_ids.sort();
    let mut active = Vec::new();
    for id in dock_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let Some(docker) = object.dock_active_docker else {
            continue;
        };
        active.push(DockActivePersist {
            dock_id: id.0,
            active_docker: docker.0,
        });
    }
    DockQueuePersistPayload { queues, active }
}

fn apply_payload(game_logic: &mut GameLogic, payload: DockQueuePersistPayload) {
    let mut restored = Vec::with_capacity(payload.queues.len());
    for entry in payload.queues {
        let slot_count = entry.owners.len().max(entry.reached.len());
        let mut queue = HostDockApproachQueue::new(entry.number_approach_positions);
        queue.number_approach_positions = entry.number_approach_positions;
        queue.number_approach_position_bones = entry.number_approach_position_bones;
        queue.waiting_bones = entry
            .waiting_bones
            .iter()
            .map(|xyz| Vec3::new(xyz[0], xyz[1], xyz[2]))
            .collect();
        queue.owners = entry
            .owners
            .into_iter()
            .map(|id| (id != 0).then_some(ObjectId(id)))
            .collect();
        queue.reached = entry.reached;
        if queue.owners.len() < slot_count {
            queue.owners.resize(slot_count, None);
        }
        if queue.reached.len() < slot_count {
            queue.reached.resize(slot_count, false);
        }
        queue.wait_started = entry
            .wait_started
            .into_iter()
            .filter(|(id, _)| *id != 0)
            .map(|(id, frame)| (ObjectId(id), frame))
            .collect::<HashMap<_, _>>();
        restored.push((ObjectId(entry.dock_id), queue));
    }
    restore_live_dock_queues(restored);

    for entry in payload.active {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.dock_id)) else {
            continue;
        };
        object.dock_active_docker =
            (entry.active_docker != 0).then_some(ObjectId(entry.active_docker));
    }
}

fn find_dckq_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == DCKQ_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("DCKQ u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}
