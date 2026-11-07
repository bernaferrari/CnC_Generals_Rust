use super::*;
use crate::game_logic::{AIState, DockKind, GameLogic, KindOf, Team, ThingTemplate};

fn world_with_warehouse_and_workers() -> (GameLogic, ObjectId, Vec<ObjectId>) {
    let mut world = GameLogic::new();
    let mut dock = ThingTemplate::new("TestQueueWarehouse");
    dock.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplySource)
        .set_health(1000.0);
    dock.dock_kind = DockKind::SupplyWarehouse;
    let mut worker = ThingTemplate::new("TestQueueWorker");
    worker
        .add_kind_of(KindOf::Harvester)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Infantry)
        .set_health(250.0);
    world.templates.insert(dock.name.clone(), dock);
    world.templates.insert(worker.name.clone(), worker);

    let dock_id = world
        .create_object("TestQueueWarehouse", Team::USA, glam::Vec3::ZERO)
        .expect("warehouse");
    assert_eq!(dock_id, ObjectId(1));
    world
        .host_object_mut(dock_id)
        .expect("warehouse object")
        .stored_resources
        .supplies = 1000;
    let workers = (0..10)
        .map(|index| {
            world
                .create_object(
                    "TestQueueWorker",
                    Team::USA,
                    glam::Vec3::new(500.0 + index as f32 * 20.0, 0.0, 0.0),
                )
                .expect("worker")
        })
        .collect();
    (world, dock_id, workers)
}

fn make_gathering(world: &mut GameLogic, dock: ObjectId, docker: ObjectId) {
    let frame = world.frame;
    let worker = world.host_object_mut(docker).expect("worker");
    worker.target = Some(dock);
    worker.set_ai_state(
        AIState::Gathering,
        frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
}

#[test]
fn same_dock_id_queues_are_owned_by_their_game_logic() {
    let (mut world_a, dock_a, docker_ids_a) = world_with_warehouse_and_workers();
    let (mut world_b, dock_b, docker_ids_b) = world_with_warehouse_and_workers();
    assert_eq!(dock_a, dock_b);
    assert_eq!(docker_ids_a, docker_ids_b);

    let b_query = docker_ids_b[9];
    assert_eq!(
        world_b.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            b_query,
        ),
        Some(dock_b),
        "B initially sees its unoccupied warehouse through the production supply query"
    );

    // Fill A's nine authored SupplyWarehouseDockUpdate approach slots
    // through Main's production claim path. Keep each duplicate ID live
    // and gathering in B so its queue's liveness check cannot evict A.
    for &docker in &docker_ids_a[..9] {
        make_gathering(&mut world_a, dock_a, docker);
        assert_eq!(
            world_a
                .try_claim_dock_for_test(dock_a, docker)
                .expect("result"),
            false
        );
    }
    make_gathering(&mut world_a, dock_a, docker_ids_a[9]);
    assert_eq!(
        world_a.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            docker_ids_a[9],
        ),
        None,
        "A's tenth docker must observe A's nine occupied approach slots"
    );

    // The same live worker IDs in B must not make A's nine reservations
    // visible to B's production warehouse-acquisition query.
    for &docker in &docker_ids_b {
        make_gathering(&mut world_b, dock_b, docker);
    }
    assert!(
        world_b.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            b_query,
        ) == Some(dock_b),
        "B must still see its own free warehouse after A fills A's queue"
    );
}

#[test]
fn later_construction_and_reset_of_one_world_leave_the_other_queue_alone() {
    let (mut world_a, dock_a, docker_ids_a) = world_with_warehouse_and_workers();
    for &docker in &docker_ids_a[..9] {
        make_gathering(&mut world_a, dock_a, docker);
        assert_eq!(
            world_a
                .try_claim_dock_for_test(dock_a, docker)
                .expect("result"),
            false
        );
    }
    make_gathering(&mut world_a, dock_a, docker_ids_a[9]);
    let query_a = |world: &mut GameLogic, docker| {
        world.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            docker,
        )
    };
    assert_eq!(query_a(&mut world_a, docker_ids_a[9]), None);

    // Construct B only after A owns all slots. Constructing a candidate
    // world must not reset an already live owner's module state.
    let (mut world_b, dock_b, docker_ids_b) = world_with_warehouse_and_workers();
    assert_eq!(dock_a, dock_b);
    assert_eq!(query_a(&mut world_a, docker_ids_a[9]), None);

    for &docker in &docker_ids_b[..9] {
        make_gathering(&mut world_b, dock_b, docker);
        assert_eq!(
            world_b
                .try_claim_dock_for_test(dock_b, docker)
                .expect("result"),
            false
        );
    }
    make_gathering(&mut world_b, dock_b, docker_ids_b[9]);
    assert_eq!(query_a(&mut world_b, docker_ids_b[9]), None);

    world_a.reset();
    assert_eq!(query_a(&mut world_b, docker_ids_b[9]), None);
}

#[test]
fn leaving_and_reentering_dock_state_before_observation_cancels_old_slot() {
    let (mut world, dock, docker_ids) = world_with_warehouse_and_workers();
    for &docker in &docker_ids[..9] {
        make_gathering(&mut world, dock, docker);
        assert_eq!(
            world.try_claim_dock_for_test(dock, docker).expect("result"),
            false
        );
    }
    let waiter = docker_ids[9];
    make_gathering(&mut world, dock, waiter);
    assert_eq!(
        world.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            waiter,
        ),
        None,
        "the old live-session reservations initially fill the warehouse"
    );

    // No queue observer runs between these writes. A callback epoch must
    // retain the exit event even though the current state is live again.
    let frame = world.frame;
    {
        let owner = world.host_object_mut(docker_ids[0]).expect("first owner");
        owner.set_ai_state(
            AIState::Moving,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        owner.set_ai_state(
            AIState::Gathering,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    assert_eq!(
        world.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            waiter,
        ),
        Some(dock),
        "the canceled reservation frees one approach slot despite reentry"
    );
}

#[test]
fn query_clone_is_inert_and_readmission_requires_completed_retirement() {
    let (mut world, dock, docker_ids) = world_with_warehouse_and_workers();
    let owner_id = docker_ids[0];
    let waiter_id = docker_ids[9];
    for &docker in &docker_ids[..9] {
        make_gathering(&mut world, dock, docker);
        assert_eq!(
            world.try_claim_dock_for_test(dock, docker).expect("result"),
            false
        );
    }
    make_gathering(&mut world, dock, waiter_id);

    let replacement = world.host_object(owner_id).expect("owner").clone();
    let mut query_clone = world.host_object(owner_id).expect("owner").clone();
    query_clone.set_ai_state(
        AIState::Moving,
        world.frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    query_clone.set_ai_state(
        AIState::Gathering,
        world.frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(
        world.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            waiter_id,
        ),
        None,
        "mutating a detached query clone cannot cancel the live Object's slot"
    );

    // A live lifetime cannot be silently replaced. Rejection preserves
    // its dock slot until normal onDelete/physical retirement.
    assert!(world.try_admit_host_object(replacement.clone()).is_err());
    assert_eq!(
        world.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            waiter_id,
        ),
        None
    );
    world.destroy_object(owner_id);
    world.process_destroy_list();
    world.try_admit_host_object(replacement).unwrap();
    assert_eq!(
        world.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            waiter_id,
        ),
        Some(dock)
    );
}

#[test]
fn destroyed_and_recreated_docker_does_not_keep_its_old_slot() {
    let (mut world, dock, docker_ids) = world_with_warehouse_and_workers();
    for &docker in &docker_ids[..9] {
        make_gathering(&mut world, dock, docker);
        assert_eq!(
            world.try_claim_dock_for_test(dock, docker).expect("result"),
            false
        );
    }
    let destroyed_id = docker_ids[0];
    let replacement = world.host_object(destroyed_id).unwrap().clone();
    let waiter = docker_ids[9];
    make_gathering(&mut world, dock, waiter);
    let query_waiter = |world: &mut GameLogic| {
        world.find_nearest_harvestable_supply_within(
            Team::USA,
            glam::Vec3::new(700.0, 0.0, 0.0),
            None,
            waiter,
        )
    };
    assert_eq!(query_waiter(&mut world), None);

    world.destroy_object(destroyed_id);
    world.process_destroy_list();
    assert!(world.host_object(destroyed_id).is_none());
    assert_eq!(query_waiter(&mut world), Some(dock));

    world.admit_host_object(replacement);
    assert_eq!(query_waiter(&mut world), Some(dock));
}

#[test]
fn destroyed_dock_drops_its_object_owned_queue() {
    let (mut world, dock, docker_ids) = world_with_warehouse_and_workers();
    let owner = docker_ids[0];
    make_gathering(&mut world, dock, owner);
    assert_eq!(
        world.try_claim_dock_for_test(dock, owner).expect("result"),
        false
    );
    assert!(world.host_dock_approach_queues.has_dock(dock));

    world.destroy_object(dock);
    world.process_destroy_list();
    assert!(world.host_object(dock).is_none());
    assert!(
        !world.host_dock_approach_queues.has_dock(dock),
        "the DockUpdate mirror must be removed with its owning Object"
    );
}

#[test]
fn setter_only_dock_retask_releases_active_docker_side_effects_per_world() {
    let (mut world_a, dock_a, workers_a) = world_with_warehouse_and_workers();
    let (mut world_b, dock_b, workers_b) = world_with_warehouse_and_workers();
    assert_eq!(dock_a, dock_b);
    assert_eq!(workers_a, workers_b);

    let active_a = workers_a[0];
    let active_b = workers_b[0];
    for (world, active, dock) in [
        (&mut world_a, active_a, dock_a),
        (&mut world_b, active_b, dock_b),
    ] {
        world.set_fixture_object_position(active, glam::Vec3::ZERO);
        make_gathering(world, dock, active);
        assert_eq!(
            world.try_claim_dock_for_test(dock, active).expect("result"),
            true
        );
        assert_eq!(
            world.host_object(dock).unwrap().dock_active_docker,
            Some(active)
        );
        let slots: Vec<_> = world
            .host_dock_approach_queues
            .snapshot()
            .into_iter()
            .find(|(id, _)| *id == dock)
            .expect("live DockUpdate slot state")
            .1
            .owners
            .into_iter()
            .flatten()
            .collect();
        assert!(
            !slots.contains(&active),
            "C++ onEnterReached releases the approach slot while DockUpdate retains activeDocker"
        );
    }

    let active_model =
        1u128 << crate::game_logic::host_enum_table_residual::docking_active_model_bit();
    let beginning_model =
        1u128 << crate::game_logic::host_enum_table_residual::docking_beginning_model_bit();
    let ending_model =
        1u128 << crate::game_logic::host_enum_table_residual::docking_ending_model_bit();
    let docking_model = 1u128
        << crate::game_logic::host_enum_table_residual::model_condition_bit_name_index("DOCKING")
            .expect("DOCKING model condition") as u32;
    assert_ne!(
        world_b.host_object(active_b).unwrap().model_condition_bits & active_model,
        0
    );

    // C++ AIDockMachine::halt calls DockUpdate::cancelDock on a session
    // exit. Exercise only the Object callback here; no GameLogic cancel
    // helper is called before the next normal dock update/query.
    let frame = world_a.frame;
    world_a
        .host_object_mut(active_a)
        .expect("active docker")
        .set_ai_state(
            AIState::Moving,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    assert_eq!(
        world_a.host_object(dock_a).unwrap().dock_active_docker,
        Some(active_a),
        "Object holder effects remain deferred until a mutable GameLogic boundary"
    );
    let (snapshot_queues, snapshot_active) = world_a.host_dock_approach_snapshot();
    assert!(
        !snapshot_active.contains(&(dock_a, active_a)),
        "save projection must not pair reconciled queue slots with a stale active holder"
    );
    assert!(
        snapshot_queues
            .iter()
            .find(|(id, _)| *id == dock_a)
            .map_or(true, |(_, queue)| {
                !queue.owned_dockers().any(|id| id == active_a)
            })
    );
    let waiter_a = workers_a[1];
    make_gathering(&mut world_a, dock_a, waiter_a);
    // The normal mutable support-state update flushes callback epochs
    // even if no new docker queries this module this frame.
    world_a.update_support_states(&[], 1.0 / 30.0);

    let dock_after_a = world_a.host_object(dock_a).unwrap();
    let active_after_a = world_a.host_object(active_a).unwrap();
    assert_eq!(dock_after_a.dock_active_docker, None);
    assert_eq!(dock_after_a.model_condition_bits & active_model, 0);
    assert_eq!(dock_after_a.model_condition_bits & beginning_model, 0);
    assert_eq!(dock_after_a.model_condition_bits & docking_model, 0);
    assert_ne!(dock_after_a.model_condition_bits & ending_model, 0);
    assert_eq!(active_after_a.model_condition_bits & active_model, 0);
    assert_eq!(active_after_a.model_condition_bits & beginning_model, 0);
    assert_eq!(active_after_a.model_condition_bits & docking_model, 0);
    assert_ne!(active_after_a.model_condition_bits & ending_model, 0);

    assert_eq!(
        world_a
            .try_claim_dock_for_test(dock_a, waiter_a)
            .expect("result"),
        false
    );

    let dock_after_b = world_b.host_object(dock_b).unwrap();
    let active_after_b = world_b.host_object(active_b).unwrap();
    assert_eq!(dock_after_b.dock_active_docker, Some(active_b));
    assert_ne!(dock_after_b.model_condition_bits & active_model, 0);
    assert_eq!(dock_after_b.model_condition_bits & ending_model, 0);
    assert_ne!(active_after_b.model_condition_bits & active_model, 0);
    assert_eq!(active_after_b.model_condition_bits & ending_model, 0);
}

#[test]
fn seeking_repair_path_keeps_existing_dock_reservation_hq_z0490() {
    let mut world = GameLogic::new();
    world.force_map_loaded_for_path_test(true);
    let mut pad = ThingTemplate::new("TestRepairQueuePad");
    pad.repair_dock_frames_for_full_heal = Some(150.0);
    pad.add_kind_of(KindOf::RepairPad)
        .add_kind_of(KindOf::Structure)
        .set_health(2000.0);
    let mut vehicle = ThingTemplate::new("TestRepairQueueVehicle");
    vehicle.add_kind_of(KindOf::Vehicle).set_health(500.0);
    world.templates.insert(pad.name.clone(), pad);
    world.templates.insert(vehicle.name.clone(), vehicle);
    let dock = world
        .create_object("TestRepairQueuePad", Team::USA, glam::Vec3::ZERO)
        .expect("repair pad");
    let first = world
        .create_object(
            "TestRepairQueueVehicle",
            Team::USA,
            glam::Vec3::new(500.0, 0.0, 0.0),
        )
        .expect("first repairer");
    let second = world
        .create_object(
            "TestRepairQueueVehicle",
            Team::USA,
            glam::Vec3::new(520.0, 0.0, 0.0),
        )
        .expect("second repairer");
    let frame = world.frame;
    for id in [first, second] {
        let (object, attack_events) = world
            .host_object_and_attack_events_mut(id)
            .expect("repairer object");
        object.set_target(
            Some(dock),
            frame,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        object.set_ai_state(
            AIState::SeekingRepair,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }

    let first_epoch = world.host_object(first).unwrap().dock_cancel_epoch;
    assert_eq!(
        world.try_claim_dock_for_test(dock, first).expect("result"),
        false
    );
    assert_eq!(
        world.host_object(first).unwrap().ai_state(),
        AIState::SeekingRepair,
        "the path adapter restores the outer repair state after assigning movement"
    );
    assert_eq!(
        world.host_object(first).unwrap().dock_cancel_epoch,
        first_epoch,
        "nested approach movement must preserve the outer dock session"
    );

    assert_eq!(
        world.try_claim_dock_for_test(dock, second).expect("result"),
        false
    );
    let queues = &world.host_dock_approach_queues;
    let owners: Vec<_> = queues
        .snapshot()
        .into_iter()
        .find(|(id, _)| *id == dock)
        .expect("repair dock queue")
        .1
        .owners
        .into_iter()
        .flatten()
        .collect();
    assert!(
        owners.contains(&first),
        "first repairer lost its live reservation: {owners:?}"
    );
    assert!(
        owners.contains(&second),
        "second repairer should take a different slot"
    );
}
