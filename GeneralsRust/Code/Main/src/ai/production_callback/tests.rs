use super::*;

fn production_world(unit_name: &str, dozer: bool) -> (GameLogic, ObjectId) {
    production_world_with_exit(unit_name, dozer, None)
}

fn production_world_with_exit(
    unit_name: &str,
    dozer: bool,
    exit: Option<crate::game_logic::thing::ProductionExitMetadata>,
) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Production AI", false);
    player.resources.supplies = 10_000;
    world.add_player(player);
    let mut factory = ThingTemplate::new("DeliveryFactory");
    factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks);
    factory.production_exit_metadata = exit;
    world.templates.insert(factory.name.clone(), factory);
    let mut unit = ThingTemplate::new(unit_name);
    unit.add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    unit.add_kind_of(if dozer {
        KindOf::Dozer
    } else {
        KindOf::Infantry
    });
    unit.lock_weapon_slot = Some(0);
    unit.locomotor_name = Some("BasicHumanLocomotor".into());
    world.templates.insert(unit.name.clone(), unit);
    let factory = world
        .create_object_for_player("DeliveryFactory", 1, Vec3::ZERO)
        .unwrap();
    world
        .ai_manager
        .add_ai_player(1, Team::USA, AIDifficulty::Medium);
    (world, factory)
}

fn spawn_finished_output(world: &mut GameLogic, factory: ObjectId, name: &str) -> ObjectId {
    let result = world
        .apply_production_authority_op(ProductionAuthorityOp::SpawnUnit {
            template: name.into(),
            team: Team::USA,
            owner_player_id: Some(1),
            spawn_pos: Vec3::new(12.0, 0.0, 0.0),
        })
        .expect("production unit spawn");
    let ProductionAuthorityResult::Spawned(Some(unit)) = result else {
        panic!("production output");
    };
    world.host_object_mut(unit).unwrap().producer_id = Some(factory);
    world.record_production_spawn_completion(
        unit,
        factory,
        name.into(),
        [12.0, 0.0, 0.0],
        Some([24.0, 0.0, 0.0]),
    );
    unit
}

fn queued_order(name: &str, factory: ObjectId, count: u32) -> AIWorkOrder {
    let mut order = AIWorkOrder::new(name.into(), count, 100);
    order.factory_id = Some(factory);
    order.queued_count = 1;
    order
}

#[test]
fn factory_exit_callback_observes_completed_owned_order_before_authored_create() {
    // ProductionUpdate.cpp:798-825: exit -> Player/AI -> Create. A successful
    // enqueue does not count, and only the first incomplete matching order does.
    let name = "AmericaInfantryRanger";
    let (mut world, factory) = production_world(name, false);
    let home = Vec3::new(70.0, 0.0, 80.0);
    let team_id = {
        let mut teams = world.team_factory.lock().unwrap();
        let mut prototype = gamelogic::team::TeamPrototype::new("DeliveredTeam".into());
        prototype.set_home_location(gamelogic::common::Coord3D {
            x: home.x,
            y: home.z,
            z: home.y,
        });
        teams.replace_team_prototype(prototype);
        teams
            .create_team("DeliveredTeam")
            .unwrap()
            .read()
            .unwrap()
            .get_id()
    };
    let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
    let mut already_complete = queued_order(name, factory, 1);
    already_complete.num_completed = 1;
    let mut queued = AITeamQueue::new(
        "DeliveredTeam".into(),
        vec![
            already_complete,
            queued_order("OtherUnit", factory, 1),
            queued_order(name, factory, 2),
            queued_order(name, factory, 1),
        ],
        false,
        0,
    );
    queued.team_id = Some(team_id);
    queued.reinforcement = true;
    ai.team_queue.push_back(queued);
    ai.next_team_queue_time = 99.0;
    ai.next_team_time = 88.0;
    // Restore the queued continuation before real production, as Core's
    // reference callback test does through Xfer. Preserve this host schema.
    let bytes = serde_json::to_vec(&ai.capture_queue_persist()).unwrap();
    ai.apply_queue_persist(serde_json::from_slice(&bytes).unwrap());

    let unit = spawn_finished_output(&mut world, factory, name);
    assert_eq!(
        world.host_object(unit).unwrap().weapon_lock_type,
        WeaponLockType::NotLocked,
        "production allocation must not fire onBuildComplete before factory delivery"
    );
    let mut callbacks = 0;
    assert_eq!(
        world
            .host_apply_production_spawn_ready_completions_with_build_complete(
                |world, delivered| {
                    callbacks += 1;
                    assert_eq!(delivered, unit);
                    assert!(
                        world.host_object(unit).is_some(),
                        "unit stays in canonical store during callback"
                    );
                    let ai = &world.ai_manager.ai_players[&1];
                    let team = &ai.team_queue[0];
                    assert_eq!(
                        team.work_orders
                            .iter()
                            .map(|w| w.num_completed)
                            .collect::<Vec<_>>(),
                        vec![1, 0, 1, 0]
                    );
                    assert_eq!(team.work_orders[2].factory_id, None);
                    assert_eq!(team.work_orders[2].queued_count, 0);
                    assert_eq!(team.reinforcement_id, Some(unit));
                    assert_eq!(ai.next_team_queue_time, 0.0);
                    assert_eq!(
                        ai.next_team_time, 88.0,
                        "delivery resets team delay, not team selection timer"
                    );
                    let output = world.host_object(unit).unwrap();
                    assert_eq!(output.team_instance_name, "DeliveredTeam");
                    assert_eq!(
                        output.movement.path.last(),
                        Some(&home),
                        "AI home path follows already installed factory exit"
                    );
                    world.apply_create_modules_on_build_complete(unit);
                }
            )
            .expect("spawn-ready completion"),
        1
    );
    assert_eq!(callbacks, 1);
    assert_eq!(
        world.host_object(unit).unwrap().weapon_lock_type,
        WeaponLockType::LockedPermanently
    );
    let after = world.ai_manager.ai_players[&1].capture_queue_persist();
    let bytes = serde_json::to_vec(&after).unwrap();
    let restored: crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist =
        serde_json::from_slice(&bytes).unwrap();
    world
        .ai_manager
        .ai_players
        .get_mut(&1)
        .unwrap()
        .apply_queue_persist(restored);
    let ai = &world.ai_manager.ai_players[&1];
    assert_eq!(ai.team_queue[0].reinforcement_id, Some(unit));
    assert_eq!(ai.team_queue[0].work_orders[2].num_completed, 1);
    assert_eq!(
        world
            .host_apply_production_spawn_ready_completions()
            .expect("spawn-ready completion"),
        0,
        "restoring progress does not redeliver previously completed production"
    );
}

#[test]
fn unqueued_factory_dozer_delivery_uses_cpp_repair_or_next_frame_build_shortcut() {
    for repair in [false, true] {
        let (mut world, factory) = production_world("AmericaVehicleDozer", true);
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.dozer_queued_for_repair = repair;
        ai.next_building_time = 80.0;
        ai.next_team_queue_time = 99.0;
        let unit = spawn_finished_output(&mut world, factory, "AmericaVehicleDozer");
        assert_eq!(
            world
                .host_apply_production_spawn_ready_completions()
                .expect("spawn-ready completion"),
            1
        );
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(ai.next_team_queue_time, 0.0);
        assert!(!ai.dozer_queued_for_repair);
        if repair {
            assert_eq!(ai.repair_dozer, Some(unit));
            assert_eq!(ai.next_building_time, 80.0);
        } else {
            assert_eq!(ai.repair_dozer, None);
            assert_eq!(
                ai.next_building_time,
                (world.get_frame() as f32 + 1.0) / LOGIC_FRAMES_PER_SECOND
            );
        }
    }
}

#[test]
fn same_id_factory_deliveries_update_only_the_driving_world_queue() {
    let name = "AmericaInfantryRanger";
    let (mut a, af) = production_world(name, false);
    let (mut b, bf) = production_world(name, false);
    assert_eq!(af, bf);
    for (world, factory, required) in [(&mut a, af, 2), (&mut b, bf, 3)] {
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .team_queue
            .push_back(AITeamQueue::new(
                "LocalDelivery".into(),
                vec![queued_order(name, factory, required)],
                false,
                0,
            ));
    }
    let au = spawn_finished_output(&mut a, af, name);
    let bu = spawn_finished_output(&mut b, bf, name);
    assert_eq!(au, bu);
    // Both same-ID callbacks are pending before either receiving owner drains.
    assert_eq!(
        a.host_apply_production_spawn_ready_completions()
            .expect("spawn-ready completion"),
        1
    );
    assert_eq!(
        a.ai_manager.ai_players[&1].team_queue[0].work_orders[0].num_completed,
        1
    );
    assert_eq!(
        b.ai_manager.ai_players[&1].team_queue[0].work_orders[0].num_completed,
        0
    );
    assert_eq!(
        b.host_apply_production_spawn_ready_completions()
            .expect("spawn-ready completion"),
        1
    );
    assert_eq!(
        b.ai_manager.ai_players[&1].team_queue[0].work_orders[0].num_completed,
        1
    );
    assert_eq!(
        a.ai_manager.ai_players[&1].team_queue[0].work_orders[0].num_completed,
        1
    );
}

#[test]
fn missing_factory_delivery_keeps_owned_ai_timers_and_orders_unchanged() {
    let name = "AmericaInfantryRanger";
    let (mut world, factory) = production_world(name, false);
    let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
    ai.next_team_queue_time = 99.0;
    ai.team_queue.push_back(AITeamQueue::new(
        "NullFactory".into(),
        vec![queued_order(name, factory, 1)],
        false,
        0,
    ));
    let unit = world.create_object_for_player(name, 1, Vec3::ZERO).unwrap();
    world
        .notify_owned_ai_unit_produced(ObjectId(u32::MAX), unit)
        .expect("missing factory is a no-op");
    let ai = &world.ai_manager.ai_players[&1];
    assert_eq!(ai.next_team_queue_time, 99.0);
    assert_eq!(ai.team_queue[0].work_orders[0].num_completed, 0);
    assert_eq!(ai.team_queue[0].work_orders[0].factory_id, Some(factory));
}

#[test]
fn supply_spawn_notifies_ai_before_exit_without_replaying_create_or_draining_paid_output() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "supply_spawn_notifies_ai_before_exit_without_replaying_create_or_draining_paid_output",
        || {
            let name = "AmericaVehicleChinook";
            let mut world = GameLogic::new();
            world.admit_object_ini_sources([(
                "SupplyCallback.ini".into(),
                "Object CallbackSupplyCenter\n KindOf = STRUCTURE FS_SUPPLY_CENTER SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = SpawnBehavior SpawnTag\n OneShot = Yes\n SpawnNumber = 1\n SpawnTemplateName = AmericaVehicleChinook\n End\n Behavior = SupplyCenterProductionExitUpdate ExitTag\n UnitCreatePoint = X:0 Y:0 Z:0\n NaturalRallyPoint = X:30 Y:0 Z:0\n End\nEnd\nObject AmericaVehicleChinook\n KindOf = HARVESTER VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = SupplyTruckAIUpdate TruckAI\n End\nEnd\n".into(),
            )]).expect("authored supply fixture");
            let mut player = Player::new(1, Team::USA, "Production AI", false);
            player.resources.supplies = 10_000;
            world.add_player(player);
            let mut factory = ThingTemplate::new("DeliveryFactory");
            factory.add_kind_of(KindOf::Structure).add_kind_of(KindOf::FSBarracks);
            world.templates.insert(factory.name.clone(), factory);
            let unit = world.templates.get_mut(name).unwrap();
            unit.lock_weapon_slot = Some(0);
            unit.locomotor_name = Some("BasicHumanLocomotor".into());
            let paid_factory = world.create_object_for_player("DeliveryFactory", 1, Vec3::ZERO).unwrap();
            world.ai_manager.add_ai_player(1, Team::USA, AIDifficulty::Medium);
            let center_id = world.create_object_for_player("CallbackSupplyCenter", 1, Vec3::ZERO).unwrap();
            let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
            ai.team_queue.push_back(AITeamQueue::new(
                "PendingPaid".into(),
                vec![queued_order(name, paid_factory, 1)],
                false,
                0,
            ));
            ai.team_queue.push_back(AITeamQueue::new(
                "StarterSpawn".into(),
                vec![queued_order(name, center_id, 2)],
                false,
                0,
            ));
            // Keep a real paid delivery pending to prove a free-spawn handoff
            // does not consume or misclassify another factory's event.
            let paid = spawn_finished_output(&mut world, paid_factory, name);
            for _ in 0..32 {
                world.frame += 1;
                world.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
            }
            let spawned = world.host_objects().registered_values()
                .find(|unit| unit.producer_id == Some(center_id))
                .map(|unit| unit.id).expect("actual free SpawnBehavior collector");
            assert_ne!(spawned, paid);
            assert_eq!(world.ai_manager.ai_players[&1].team_queue[0].work_orders[0].num_completed, 0);
            assert_eq!(world.ai_manager.ai_players[&1].team_queue[1].work_orders[0].num_completed, 1);
            assert_eq!(world.host_object(spawned).unwrap().weapon_lock_type, WeaponLockType::LockedPermanently);
            // CPP AIFollowPathState::onEnter copies the authored point, but
            // Pathfinder::prependCells installs adjustCoordToCell output
            // (AIPathfind.cpp:9005-9017), not the raw model coordinate.
            let output = world.host_object(spawned).unwrap();
            let grid = &world.pathfinding_system.grid;
            let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
                output.selection_radius,
                grid.grid_size(),
            );
            let expected_exit = ({
                let ground_source = crate::game_logic::pathfinding::PathfindingGround::Flat(0.0);
                grid.snap_position(Vec3::new(30.0, 0.0, 0.0), center, &ground_source)
            });
            assert_eq!(
                output.movement.path.last(),
                Some(&expected_exit),
                "the actual snapped exit follows the earlier AI callback"
            );
            assert_eq!(output.ignored_obstacle_id, Some(center_id));
            for _ in 0..32 {
                world.frame += 1;
                world.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
            }
            assert_eq!(world.host_objects().registered_values()
                .filter(|unit| unit.producer_id == Some(center_id)).count(), 1);
            assert_eq!(
                world
                    .host_apply_production_spawn_ready_completions()
                    .expect("spawn-ready completion"),
                1,
                "the pending paid output remains for its own original callback point"
            );
            assert_eq!(
                world.ai_manager.ai_players[&1].team_queue[0].work_orders[0].num_completed,
                1
            );
            assert_eq!(
                world.ai_manager.ai_players[&1].team_queue[1].work_orders[0].num_completed,
                1
            );
            assert_eq!(
                world.host_object(paid).unwrap().weapon_lock_type,
                WeaponLockType::LockedPermanently,
                "paid production retains its real post-exit Create callback"
            );
        },
    );
}

#[test]
fn owned_structure_completion_resets_cpp_delays_and_only_exact_pad() {
    let (mut world, structure) = production_world("UnusedOutput", false);
    let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
    ai.building_queue.clear();
    ai.next_building_time = 80.0;
    ai.next_team_queue_time = 90.0;
    let mut bound = AIBuildingInfo::new("DeliveryFactory".into(), Vec3::ZERO, 2);
    bound.object_id = Some(structure);
    ai.building_queue.push(bound);
    ai.building_queue
        .push(AIBuildingInfo::new("DeliveryFactory".into(), Vec3::ZERO, 2));
    world.notify_owned_ai_structure_produced(structure);
    let ai = &world.ai_manager.ai_players[&1];
    assert_eq!(ai.next_building_time, 0.0);
    assert_eq!(ai.next_team_queue_time, 0.0);
    assert!(ai.building_queue[0].is_built);
    assert!(!ai.building_queue[1].is_built);
}

#[test]
fn same_id_structure_completion_keeps_foreign_ai_delays_and_queue_unchanged() {
    let (mut owner, id) = production_world("UnusedOutput", false);
    let (mut foreign, foreign_id) = production_world("UnusedOutput", false);
    assert_eq!(id, foreign_id);
    for world in [&mut owner, &mut foreign] {
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.building_queue.clear();
        ai.next_building_time = 80.0;
        ai.next_team_queue_time = 90.0;
        let mut pad = AIBuildingInfo::new("DeliveryFactory".into(), Vec3::ZERO, 2);
        pad.object_id = Some(id);
        ai.building_queue.push(pad);
    }
    owner.notify_owned_ai_structure_produced(id);
    let ai = &foreign.ai_manager.ai_players[&1];
    assert_eq!(ai.next_building_time, 80.0);
    assert_eq!(ai.next_team_queue_time, 90.0);
    assert!(!ai.building_queue[0].is_built);
    foreign.notify_owned_ai_structure_produced(ObjectId(u32::MAX));
    assert_eq!(foreign.ai_manager.ai_players[&1].next_building_time, 80.0);
}

#[test]
fn production_completion_receipts_follow_actual_owner_fifo_across_peer_reset_and_restore() {
    use crate::game_logic::buildings::ProductionKind;
    use crate::game_logic::host_production_log::HostProductionEvent;
    use crate::game_logic::thing::{ProductionExitMetadata, ProductionExitStyle};
    use crate::game_logic::{BuildingData, BuildingType, ProductionItem, Resources};

    fn finish(world: &mut GameLogic, factory: ObjectId, name: &str) -> ObjectId {
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: name.into(),
            progress: 0.0,
            total_time: 0.0,
            construction_frames: 0,
            cost: Resources::default(),
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        world.host_object_mut(factory).unwrap().building_data = Some(building);
        let previous = world
            .objects
            .read()
            .registered_keys()
            .copied()
            .collect::<Vec<_>>();
        world.update_production(1.0 / 30.0);
        assert!(
            world
                .host_object(factory)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue
                .is_empty()
        );
        let spawned = world
            .objects
            .registered_values()
            .find(|object| object.producer_id == Some(factory) && !previous.contains(&object.id))
            .unwrap()
            .id;
        assert!(world.host_object(spawned).unwrap().is_alive());
        world.production_events.complete_boundary();
        spawned
    }

    let name = "ReceiptInfantry";
    let metadata = ProductionExitMetadata {
        style: ProductionExitStyle::Default,
        unit_create_point: [0.0; 3],
        natural_rally_point: [0.0; 3],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    };
    // Admit the real producer with its authored exit before its first birth.
    let (mut owner, factory) = production_world_with_exit(name, false, Some(metadata));
    let first = finish(&mut owner, factory, name);
    let (mut peer, peer_factory) = production_world(name, false);
    assert_eq!(factory, peer_factory);
    let peer_spawn = finish(&mut peer, peer_factory, name);
    assert_eq!(first, peer_spawn);
    assert_eq!(peer.production_events.take_completed().len(), 1);
    peer.reset();
    // Produce another actual completion so committed restore must discard a
    // real stale output rather than merely clearing an already empty buffer.
    let (mut peer, peer_factory) = production_world(name, false);
    finish(&mut peer, peer_factory, name);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    peer.templates = owner.templates.clone();
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    assert!(peer.production_events.take_completed().is_empty());
    let second_before = owner
        .objects
        .read()
        .registered_keys()
        .copied()
        .collect::<Vec<_>>();
    finish(&mut owner, factory, name);
    let second = owner
        .objects
        .registered_keys()
        .copied()
        .find(|id| !second_before.contains(id))
        .unwrap();
    let completed = owner.production_events.take_completed();
    assert_eq!(
        completed,
        vec![
            HostProductionEvent::Complete {
                producer: factory,
                template_name: name.into(),
                spawned: first
            },
            HostProductionEvent::Complete {
                producer: factory,
                template_name: name.into(),
                spawned: second
            },
        ]
    );
    assert!(owner.production_events.take_completed().is_empty());
}

#[test]
fn produced_collector_visits_saved_build_list_in_order_without_touching_peer() {

    fn setup() -> (GameLogic, ObjectId, Vec<ObjectId>) {
        let (mut world, factory) = production_world("LedgerTruck", false);
        world.templates.get_mut("LedgerTruck").unwrap().add_kind_of(KindOf::Harvester);
        let mut centers = Vec::new();
        for x in [100.0, 200.0, 300.0] {
            centers.push(world.create_object_for_player(
                "DeliveryFactory", 1, Vec3::new(x, 0.0, 0.0),
            ).unwrap());
        }
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.building_queue.clear();
        for (object, desired, current) in [
            (Some(centers[0]), 2, 0),
            (None, 2, 0),
            (Some(centers[1]), 1, 1),
            (Some(centers[2]), 3, -1),
        ] {
            let mut row = AIBuildingInfo::new("DeliveryFactory".into(), Vec3::ZERO, 1);
            row.object_id = object;
            row.supply_building = true;
            row.desired_gatherers = desired;
            row.current_gatherers = current;
            ai.building_queue.push(row);
        }
        let mut order = queued_order("LedgerTruck", factory, 1);
        order.is_resource_gatherer = true;
        ai.team_queue.push_back(AITeamQueue::new(
            "Supply truck".into(), vec![order], true, 0,
        ));
        (world, factory, centers)
    }

    let (source, factory, centers) = setup();
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, restored_factory, restored_centers) = setup();
    let (peer, peer_factory, peer_centers) = setup();
    assert_eq!((factory, &centers), (restored_factory, &restored_centers));
    assert_eq!((factory, &centers), (peer_factory, &peer_centers));
    builder.restore_from_snapshot(&saved, &mut restored).unwrap();
    // C++ finds an object by ID even if it has begun dying.
    restored.host_object_mut(centers[0]).unwrap().health.current = 0.0;
    let truck = restored.create_object_for_player(
        "LedgerTruck", 1, Vec3::new(12.0, 0.0, 0.0),
    ).unwrap();
    let mut visits = Vec::new();
    restored.deliver_owned_ai_unit_with_dock(
        factory, truck, &mut Default::default(),
        |world, truck, center| {
            let counts = world.ai_manager.ai_players[&1].building_queue.iter()
                .map(|row| row.current_gatherers).collect::<Vec<_>>();
            visits.push((center, counts));
            assert!(world.unit_command_ai_dock_supply_center(truck, center));
            assert_eq!(world.host_object(truck).unwrap().preferred_dock_id, Some(center));
        },
    ).unwrap();
    assert_eq!(visits, [
        (centers[0], vec![1, 0, 1, -1]),
        (centers[2], vec![1, 0, 1, 0]),
    ]);
    let rows = &restored.ai_manager.ai_players[&1].building_queue;
    assert_eq!(rows.iter().map(|r| r.current_gatherers).collect::<Vec<_>>(), [1, 0, 1, 0]);
    let truck = restored.host_object(truck).unwrap();
    assert_eq!(truck.preferred_dock_id, Some(centers[2]));
    assert_eq!(truck.target, Some(centers[2]));
    assert_eq!(truck.ai_state(), AIState::ReturningResources);
    let after = builder.create_world_snapshot(&restored).unwrap();
    let (mut loaded_again, _, _) = setup();
    builder.restore_from_snapshot(&after, &mut loaded_again).unwrap();
    assert_eq!(
        loaded_again.ai_manager.ai_players[&1].building_queue.iter()
            .map(|r| r.current_gatherers).collect::<Vec<_>>(),
        [1, 0, 1, 0],
    );
    assert_eq!(
        peer.ai_manager.ai_players[&1].building_queue.iter()
            .map(|r| r.current_gatherers).collect::<Vec<_>>(),
        [0, 0, 1, -1],
    );
}
