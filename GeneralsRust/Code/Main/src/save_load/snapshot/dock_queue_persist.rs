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
//! Restore replaces only the destination GameLogic's owner store.
//! SnapshotBuilder round-trip coverage below exercises this host suffix; native
//! byte-for-byte DockUpdate::xfer compatibility is still unverified.

use crate::game_logic::host_supply_gather::HostDockApproachQueue;
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
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(DCKQ_MAGIC);
    append_u32(bytes, DCKQ_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    // Always clear this world's previous session first (C++ module state is
    // per-object); never touch another GameLogic's queues.
    game_logic.reset_host_dock_approach_queues();
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
    let payload: DockQueuePersistPayload = bincode_legacy::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("DCKQ payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> DockQueuePersistPayload {
    let queues = game_logic
        .host_dock_approach_queue_snapshot()
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
    game_logic.restore_host_dock_approach_queues(restored);

    for entry in payload.active {
        let dock_id = ObjectId(entry.dock_id);
        let active_docker = (entry.active_docker != 0).then_some(ObjectId(entry.active_docker));
        let Some(object) = game_logic.host_object_mut(dock_id) else {
            continue;
        };
        object.dock_active_docker = active_docker;
        if let Some(docker_id) = active_docker {
            game_logic.track_restored_active_docker(dock_id, docker_id);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applying_one_worlds_dock_queue_tail_does_not_clear_another_world() {
        let dock_id = ObjectId(1);
        let mut queue_a = HostDockApproachQueue::new(3);
        queue_a.owners[0] = Some(ObjectId(2));
        queue_a.owners[1] = Some(ObjectId(3));
        queue_a.reached[1] = true;
        let mut queue_b = HostDockApproachQueue::new(3);
        queue_b.owners[0] = Some(ObjectId(2));
        queue_b.reached[0] = true;

        let mut world_a = GameLogic::new();
        let mut world_b = GameLogic::new();
        world_a.restore_host_dock_approach_queues(vec![(dock_id, queue_a)]);
        world_b.restore_host_dock_approach_queues(vec![(dock_id, queue_b)]);

        let mut tail_a = Vec::new();
        append_to_lifecycle_tail(&mut tail_a, &world_a);
        apply_from_lifecycle_tail(&tail_a, &mut world_a).expect("restore A's queue");

        let restored_a = world_a.host_dock_approach_queue_snapshot();
        assert_eq!(restored_a.len(), 1);
        assert_eq!(restored_a[0].1.owners[0], Some(ObjectId(2)));
        assert_eq!(restored_a[0].1.owners[1], Some(ObjectId(3)));
        assert!(restored_a[0].1.reached[1]);

        let still_b = world_b.host_dock_approach_queue_snapshot();
        assert_eq!(still_b.len(), 1);
        assert_eq!(still_b[0].1.owners[0], Some(ObjectId(2)));
        assert!(still_b[0].1.reached[0]);
    }

    #[test]
    fn snapshot_builder_round_trips_production_dock_queue_and_isolates_same_ids() {
        use crate::game_logic::{AIState, DockKind, KindOf, Team, ThingTemplate};

        fn make_world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
            let mut world = GameLogic::new();
            world.force_map_loaded_for_path_test(true);

            let mut dock_template = ThingTemplate::new("SnapshotSupplyWarehouse");
            dock_template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::SupplySource)
                .set_health(1_000.0);
            dock_template.dock_kind = DockKind::SupplyWarehouse;
            world
                .templates
                .insert(dock_template.name.clone(), dock_template);

            let mut docker_template = ThingTemplate::new("SnapshotSupplyTruck");
            docker_template
                .add_kind_of(KindOf::Vehicle)
                .set_health(200.0);
            world
                .templates
                .insert(docker_template.name.clone(), docker_template);

            let dock = world
                .create_object("SnapshotSupplyWarehouse", Team::USA, Vec3::ZERO)
                .expect("warehouse");
            let active = world
                .create_object("SnapshotSupplyTruck", Team::USA, Vec3::ZERO)
                .expect("active docker");
            let waiting = world
                .create_object("SnapshotSupplyTruck", Team::USA, Vec3::ZERO)
                .expect("waiting docker");
            for docker in [active, waiting] {
                let object = world.host_object_mut(docker).expect("docker");
                // This is a dock-session target, not a player AttackObject
                // command: `set_target` enters Attacking, so seed the same
                // target/session state used by the production gather path.
                object.target = Some(dock);
                object.set_ai_state(AIState::Gathering);
            }
            (world, dock, active, waiting)
        }

        let (mut source, dock, active, waiting) = make_world();
        let (mut other, other_dock, other_active, other_waiting) = make_world();
        assert_eq!(
            (dock, active, waiting),
            (other_dock, other_active, other_waiting)
        );

        // Populate both worlds through the live docking entry point, interleaved
        // with identical object IDs. The waiting docker is already at its
        // approach point, so the real queue records reached + clearance time.
        source.set_current_frame(41);
        other.set_current_frame(17);
        assert!(source.try_claim_dock(dock, active));
        assert!(other.try_claim_dock(other_dock, other_active));
        assert!(!source.try_claim_dock(dock, waiting));
        assert!(!other.try_claim_dock(other_dock, other_waiting));

        let queue_for = |world: &GameLogic| {
            world
                .host_dock_approach_queue_snapshot()
                .into_iter()
                .find(|(id, _)| *id == dock)
                .map(|(_, queue)| queue)
                .expect("production queue")
        };
        let source_queue = queue_for(&source);
        let waiting_slot = source_queue.index_of(waiting).expect("waiting reservation") as usize;
        assert!(source_queue.reached[waiting_slot]);
        assert_eq!(source_queue.wait_started.get(&waiting), Some(&41));
        assert_eq!(
            source.host_object(dock).unwrap().dock_active_docker,
            Some(active)
        );

        let other_queue_before = queue_for(&other);
        let other_owners_before = other_queue_before.owners.clone();
        let other_reached_before = other_queue_before.reached.clone();
        let other_wait_started_before = other_queue_before.wait_started.clone();
        let other_wait_slot = other_queue_before
            .index_of(other_waiting)
            .expect("other waiting reservation") as usize;
        assert!(other_queue_before.reached[other_wait_slot]);
        assert_eq!(
            other_queue_before.wait_started.get(&other_waiting),
            Some(&17)
        );
        assert_eq!(
            other.host_object(other_dock).unwrap().dock_active_docker,
            Some(other_active)
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("capture");
        assert!(find_dckq_suffix(&snapshot.lifecycle_tail).is_some());

        // Templates are the authoritative static definitions used when saved
        // object/module runtime state is rebuilt. SnapshotBuilder deliberately
        // does not serialize them, so install those definitions before restore.
        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(
            restored
                .host_object(dock)
                .unwrap()
                .thing()
                .template
                .dock_kind,
            DockKind::SupplyWarehouse,
            "restore must use the authoritative warehouse template"
        );
        assert_eq!(
            restored.host_object(dock).unwrap().dock_active_docker,
            Some(active)
        );
        let restored_queue = queue_for(&restored);
        let restored_slot = restored_queue
            .index_of(waiting)
            .expect("restored waiting reservation") as usize;
        assert!(restored_queue.reached[restored_slot]);
        assert_eq!(restored_queue.wait_started.get(&waiting), Some(&41));
        assert_eq!(restored_queue.number_approach_positions, 9);
        assert_eq!(restored_queue.number_approach_position_bones, 0);

        // Exercise the restored state through the production entry point again:
        // the current active docker still blocks the waiter, and its timeout
        // age continues from the captured frame instead of restarting.
        restored.set_current_frame(42);
        assert!(!restored.try_claim_dock(dock, waiting));
        let continued_queue = queue_for(&restored);
        assert_eq!(continued_queue.wait_started.get(&waiting), Some(&41));
        assert!(continued_queue.reached[restored_slot]);

        // Capturing/restoring A must not mutate the other world with identical
        // dock/docker IDs. Compare its full queue payload and active module
        // state to the pre-restore snapshot.
        let other_queue_after = queue_for(&other);
        let after_slot = other_queue_after
            .index_of(other_waiting)
            .expect("other reservation remains") as usize;
        assert_eq!(other_queue_after.owners, other_owners_before);
        assert_eq!(other_queue_after.reached, other_reached_before);
        assert_eq!(other_queue_after.wait_started, other_wait_started_before);
        assert!(other_queue_after.reached[after_slot]);
        assert_eq!(
            other_queue_after.wait_started.get(&other_waiting),
            Some(&17)
        );
        assert_eq!(
            other.host_object(other_dock).unwrap().dock_active_docker,
            Some(other_active)
        );
    }
}
