//! Actual installed OTHER locomotion, Physics and waypoint ownership controls.
use super::*;
use crate::common::{AsciiString, LocomotorSetType};
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate};
use crate::modules::{AIUpdateInterface, ai_world::AiWorld};
use crate::object::behavior::physics_update::PhysicsBehaviorModuleData;
use crate::object::body::active_body::ActiveBodyModuleData;
use crate::object::update::AIUpdateModuleData;

fn new_game(name: &str) -> NativeMatch {
    let mut base = DefaultThingTemplate::new(name.into());
    base.add_kind_of(KindOf::Infantry);
    let locomotor_name = format!("{name}Loco");
    let mut locomotor = LocomotorTemplate::new(locomotor_name.clone());
    // Keep the original factory controls' OTHER defaults, not infantry TWO_LEGS.
    locomotor.surfaces = crate::locomotor::SURFACE_GROUND;
    locomotor.max_speed = 3.0;
    locomotor.acceleration = 0.1;
    locomotor.max_turn_rate = 0.1;
    LOCOMOTOR_STORE.register_template(locomotor);
    let mut ai_data = AIUpdateModuleData::default();
    ai_data.set_locomotor_set_entries(
        LocomotorSetType::Normal,
        vec![AsciiString::from(locomotor_name.as_str())],
    );
    let mut body = ActiveBodyModuleData::default();
    body.max_health = 100.0;
    body.initial_health = 100.0;
    let template = Arc::new(EntryTemplate {
        base,
        modules: vec![
            TemplateModuleInfo {
                name: "ActiveBody".into(),
                module_tag: "WaypointBody".into(),
                data: Arc::new(body),
                interface_mask: ModuleInterfaceType::BODY,
            },
            TemplateModuleInfo {
                name: "AIUpdateInterface".into(),
                module_tag: "WaypointAI".into(),
                data: Arc::new(ai_data),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
            TemplateModuleInfo {
                name: "PhysicsBehavior".into(),
                module_tag: "WaypointPhysics".into(),
                data: Arc::new(PhysicsBehaviorModuleData::default()),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
        ],
    });
    let mut game = NativeMatch::with_source_template(290.0, container_template(), false, template);
    Object::admit_update_schedule_from_handle(&game.source, &mut game.scheduler).unwrap();
    let owner = Arc::clone(&game.source);
    game.operate(0, |world| {
        world
            .set_object_position(&mut owner.write().unwrap(), &Coord3D::new(30.0, 30.0, 0.0))
            .unwrap();
    });
    let interface = ai(&owner);
    game.operate(0, |world| {
        let mut interface = interface.lock().unwrap();
        interface
            .execute_command_with_world(
                &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromPlayer),
                world,
            )
            .unwrap();
        interface
            .execute_command_with_world(
                &AiCommandParams::new(AiCommandType::Busy, CommandSourceType::FromPlayer),
                world,
            )
            .unwrap();
        assert_eq!(
            interface.get_current_state_id(),
            Some(crate::modules::AIStateType::Busy as u32)
        );
    });
    game
}

fn proxy(
    owner: &Arc<RwLock<Object>>,
    name: &str,
) -> game_engine::common::thing::update_module::UpdateModulePtr {
    let mut entries = owner.read().unwrap().named_update_registrations(name);
    assert_eq!(entries.len(), 1, "one actual installed {name}");
    entries.pop().unwrap()
}

fn ai(owner: &Arc<RwLock<Object>>) -> Arc<std::sync::Mutex<dyn AIUpdateInterface>> {
    owner.read().unwrap().get_ai_update_interface().unwrap()
}

fn ai_snapshot(game: &NativeMatch, locomotor_name: &str) -> Vec<u8> {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let locomotor = LOCOMOTOR_STORE.get_template(locomotor_name).unwrap();
    let mut lookup = |name: &str| Ok((name == locomotor_name).then(|| Arc::clone(&locomotor)));
    let mut bytes = Cursor::new(Vec::new());
    ai(&game.source)
        .lock()
        .unwrap()
        .xfer_ai_update_state_with_locomotors(&mut XferSave::new(&mut bytes, 1), &mut lookup)
        .unwrap();
    bytes.into_inner()
}

fn path_phase(game: &mut NativeMatch, frame: u32) {
    let owner = Arc::clone(&game.source);
    let interface = ai(&owner);
    game.operate_with_current(frame, Some(proxy(&owner, "AIUpdateInterface")), |world| {
        interface
            .lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
            .unwrap();
    });
}

fn physics_phase(game: &mut NativeMatch, frame: u32) {
    let owner = Arc::clone(&game.source);
    game.operate_with_current(frame, Some(proxy(&owner, "PhysicsBehavior")), |world| {
        world.update_physics_phase(&owner).unwrap();
    });
}

fn install_waypoints(game: &mut NativeMatch) {
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 32 * 32];
    map.boundaries = vec![crate::common::ICoord2D::new(32, 32)];
    map.waypoints = [(9_801, 180.0), (9_802, 240.0)]
        .into_iter()
        .map(|(id, x)| crate::system::map_loader::MapWaypoint {
            id,
            name: format!("OwnedWaypoint{id}"),
            location: Coord3D::new(x, 30.0, 0.0),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        })
        .collect();
    map.waypoint_links = vec![(9_801, 9_802)];
    assert!(game.terrain.load_map_geometry(map).is_empty());
    game.pathfinder.rebuild_from_terrain(&game.terrain);
    assert!(game.terrain.get_waypoint_by_id(9_801).is_some());
    assert!(game.terrain.get_waypoint_by_id(9_802).is_some());
}

pub(crate) fn individual_waypoints_use_real_queue_physics_and_inline_second_goal() {
    waypoint_chain_control(false);
}

#[test]
fn team_waypoints_use_supplied_group_real_queue_and_physics() {
    waypoint_chain_control(true);
}

fn admit_source_group(game: &mut NativeMatch) {
    let mut group = crate::ai::AiGroup::new(71);
    group.add(game.source.read().unwrap().get_id());
    game.source
        .write()
        .unwrap()
        .enter_runtime_group(&group)
        .unwrap();
    game.groups.push(Arc::new(RwLock::new(group)));
}

fn waypoint_chain_control(as_team: bool) {
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut game = new_game("OwnedWaypointChain");
    install_waypoints(&mut game);
    if as_team {
        admit_source_group(&mut game);
    }
    let peer = new_game("OwnedWaypointChainPeer");
    assert_eq!(
        game.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let peer_before = *peer.source.read().unwrap().get_position();
    let peer_ai_before = ai_snapshot(&peer, "OwnedWaypointChainPeerLoco");
    let interface = ai(&game.source);
    let mut command = AiCommandParams::new(
        if as_team {
            AiCommandType::FollowWaypointPathAsTeam
        } else {
            AiCommandType::FollowWaypointPath
        },
        CommandSourceType::FromPlayer,
    );
    command.waypoint = Some(9_801);
    game.operate(0, |world| {
        let mut interface = interface.lock().unwrap();
        interface
            .execute_command_with_world(
                &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromAI),
                world,
            )
            .unwrap();
        interface
            .execute_command_with_world(&command, world)
            .unwrap();
    });
    {
        let mut interface = interface.lock().unwrap();
        let native = interface
            .unit_ai_for_test()
            .unwrap()
            .native_waypoint_observation_for_test();
        assert_eq!(native.requested_destination, Coord3D::new(180.0, 30.0, 0.0));
        assert!(native.waiting_for_path);
    }
    // Queue rebuilding delays delivery only while zones remain dirty. Command
    // goal snapping may already have rebuilt them through the real pathfinder.
    let first_processed = game.operate(0, |world| world.process_enter_path_queue(10).unwrap());
    assert!(
        first_processed <= 1,
        "one actual queued owner, got {first_processed}"
    );
    if first_processed == 1 {
        let mut interface = interface.lock().unwrap();
        let observation = interface
            .unit_ai_for_test()
            .unwrap()
            .native_waypoint_observation_for_test();
        assert!(
            observation.has_path,
            "serviced queue result must install actual AiPath"
        );
        assert!(!observation.waiting_for_path);
    }
    let mut processed_paths = first_processed;
    let mut saw_path = false;
    let mut saw_second_goal = false;
    let mut completed = false;
    let mut restored_moving_waypoint = false;
    let locomotor = LOCOMOTOR_STORE
        .get_template("OwnedWaypointChainLoco")
        .unwrap();
    let mut lookup =
        |name: &str| Ok((name == "OwnedWaypointChainLoco").then(|| Arc::clone(&locomotor)));
    for frame in 1..=2_000 {
        let processed = game.operate(frame, |world| world.process_enter_path_queue(10).unwrap());
        assert!(processed <= 1, "one actual queued owner, got {processed}");
        processed_paths += processed;
        let owner = Arc::clone(&game.source);
        game.operate_with_current(frame, Some(proxy(&owner, "AIUpdateInterface")), |world| {
            interface
                .lock()
                .unwrap()
                .update_waypoint_phase_with_world(world)
                .unwrap();
        });
        let has_path;
        {
            let mut interface = interface.lock().unwrap();
            let native = interface
                .unit_ai_for_test()
                .unwrap()
                .native_waypoint_observation_for_test();
            has_path = native.has_path;
            saw_path |= has_path;
            saw_second_goal |= native.current_waypoint == Some(9_802)
                && native.parent_goal == Some(Coord3D::new(240.0, 30.0, 0.0));
            completed = native.completed_waypoint == Some(9_802);
        }
        if !completed && has_path {
            path_phase(&mut game, frame);
        }
        physics_phase(&mut game, frame);
        if has_path
            && !completed
            && !restored_moving_waypoint
            && game.source.read().unwrap().get_position().x > 30.0
        {
            let mut interface = interface.lock().unwrap();
            let mut saved = Cursor::new(Vec::new());
            interface
                .xfer_ai_update_state_with_locomotors(
                    &mut XferSave::new(&mut saved, 1),
                    &mut lookup,
                )
                .unwrap();
            let saved = saved.into_inner();
            interface
                .xfer_ai_update_state_with_locomotors(
                    &mut XferLoad::new(Cursor::new(saved.clone()), 1),
                    &mut lookup,
                )
                .unwrap();
            // Loaded FSM IDs remain unresolved until the next supplied-world
            // phase. A save in this interval must retain both IDs verbatim.
            let mut immediately_resaved = Cursor::new(Vec::new());
            interface
                .xfer_ai_update_state_with_locomotors(
                    &mut XferSave::new(&mut immediately_resaved, 1),
                    &mut lookup,
                )
                .unwrap();
            assert_eq!(
                saved,
                immediately_resaved.into_inner(),
                "mid-movement AI/FSM restore must preserve serialized waypoint IDs before resolution"
            );
            restored_moving_waypoint = true;
        }
        if frame == 10 {
            assert!(
                game.source.read().unwrap().get_position().x > 30.0,
                "ten ordinary AI/Physics phases must advance the actual factory owner"
            );
            let mut interface = interface.lock().unwrap();
            let observation = interface
                .unit_ai_for_test()
                .unwrap()
                .native_waypoint_observation_for_test();
            assert_eq!(
                observation.requested_destination,
                Coord3D::new(180.0, 30.0, 0.0),
                "first waypoint goal must remain live during initial movement"
            );
        }
        if completed {
            break;
        }
    }
    assert!(
        processed_paths >= 1,
        "actual path queue must deliver a request"
    );
    assert!(
        saw_path,
        "real queued result must install into canonical AiPath"
    );
    assert!(
        saw_second_goal,
        "the live parent FSM must observe the second waypoint goal"
    );
    assert!(
        restored_moving_waypoint,
        "save/load must interrupt actual waypoint movement"
    );
    assert!(
        completed,
        "actual Physics movement must complete the final waypoint"
    );
    assert!(game.source.read().unwrap().get_position().x >= 235.0);
    assert_eq!(*peer.source.read().unwrap().get_position(), peer_before);
    assert_eq!(
        ai_snapshot(&peer, "OwnedWaypointChainPeerLoco"),
        peer_ai_before,
        "same-ID other world's complete serialized AI must remain unchanged"
    );
    let peer_ai = ai(&peer.source);
    let mut peer_ai = peer_ai.lock().unwrap();
    let peer_native = peer_ai
        .unit_ai_for_test()
        .unwrap()
        .native_waypoint_observation_for_test();
    assert!(!peer_native.has_path);
    assert!(peer_native.current_waypoint.is_none());
    assert!(peer_native.completed_waypoint.is_none());
}

pub(crate) fn other_path_moves_its_real_owner_and_continues_after_canonical_ai_xfer() {
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut first = new_game("OwnedPathXfer");
    let second = new_game("OwnedPathPeer");
    let first_id = first.source.read().unwrap().get_id();
    assert_eq!(first_id, second.source.read().unwrap().get_id());
    assert!(!Arc::ptr_eq(&first.source, &second.source));
    let peer_before = *second.source.read().unwrap().get_position();
    let peer_ai_before = ai_snapshot(&second, "OwnedPathPeerLoco");
    let interface = ai(&first.source);
    let points = [
        Coord3D::new(90.0, 30.0, 0.0),
        Coord3D::new(150.0, 30.0, 0.0),
        Coord3D::new(210.0, 30.0, 0.0),
    ];
    {
        let mut interface = interface.lock().unwrap();
        interface
            .unit_ai_for_test()
            .unwrap()
            .install_native_path_for_test(&points);
        interface.set_desired_speed(crate::modules::FAST_AS_POSSIBLE);
        interface.set_locomotor_goal_position_on_path();
    }
    let distance = first.operate(0, |world| {
        let mut interface = interface.lock().unwrap();
        let distance = interface
            .get_locomotor_distance_to_goal_with_world(world)
            .unwrap();
        let runtime_distance = interface
            .unit_ai_for_test()
            .unwrap()
            .native_path_distance_for_test(world)
            .unwrap();
        assert_eq!(
            distance, runtime_distance,
            "cached AI interface and installed native runtime must observe the same owned AiPath"
        );
        distance
    });
    let mut close_enough = 0.0;
    interface
        .lock()
        .unwrap()
        .with_cur_locomotor(&mut |locomotor| {
            close_enough = locomotor.get_close_enough_dist();
        });
    assert!(
        distance.is_finite() && distance > close_enough,
        "remaining actual factory-owner AiPath distance: {distance}"
    );
    let before = *first.source.read().unwrap().get_position();
    path_phase(&mut first, 1);
    physics_phase(&mut first, 1);
    let first_tick = *first.source.read().unwrap().get_position();
    assert!(
        first_tick.x > before.x,
        "actual Physics must advance its factory owner"
    );
    let mut bytes = Cursor::new(Vec::new());
    let locomotor = LOCOMOTOR_STORE.get_template("OwnedPathXferLoco").unwrap();
    let mut lookup = |name: &str| Ok((name == "OwnedPathXferLoco").then(|| Arc::clone(&locomotor)));
    {
        let mut interface = interface.lock().unwrap();
        interface
            .xfer_ai_update_state_with_locomotors(&mut XferSave::new(&mut bytes, 1), &mut lookup)
            .unwrap();
        interface
            .unit_ai_for_test()
            .unwrap()
            .install_native_path_for_test(&[
                Coord3D::new(30.0, 90.0, 0.0),
                Coord3D::new(30.0, 150.0, 0.0),
            ]);
        assert_ne!(interface.get_path_destination(), Some(points[2]));
        interface
            .xfer_ai_update_state_with_locomotors(
                &mut XferLoad::new(Cursor::new(bytes.into_inner()), 1),
                &mut lookup,
            )
            .unwrap();
        assert_eq!(interface.get_path_destination(), Some(points[2]));
    }
    path_phase(&mut first, 2);
    physics_phase(&mut first, 2);
    assert!(first.source.read().unwrap().get_position().x > first_tick.x);
    assert_eq!(*second.source.read().unwrap().get_position(), peer_before);
    assert_eq!(
        ai_snapshot(&second, "OwnedPathPeerLoco"),
        peer_ai_before,
        "movement and restore must leave the same-ID peer's canonical AI untouched"
    );
    let peer_ai = ai(&second.source);
    assert_eq!(peer_ai.lock().unwrap().get_path_destination(), None);
}

pub(crate) fn other_turn_is_not_repeated_by_actual_physics_integration() {
    let mut game = new_game("OwnedTurnPhysics");
    let interface = ai(&game.source);
    {
        let mut interface = interface.lock().unwrap();
        interface
            .unit_ai_for_test()
            .unwrap()
            .install_native_path_for_test(&[
                Coord3D::new(30.0, 30.0, 0.0),
                Coord3D::new(150.0, 150.0, 0.0),
            ]);
        interface.set_desired_speed(crate::modules::FAST_AS_POSSIBLE);
        interface.set_locomotor_goal_position_on_path();
    }
    let ai_proxy = proxy(&game.source, "AIUpdateInterface");
    let ai_update = game
        .scheduler
        .next_owned_scheduled_update(1)
        .unwrap()
        .expect("actual due Initial AI heap entry");
    assert_eq!(
        ai_update.phase,
        game_engine::common::thing::update_module::SleepyUpdatePhase::Initial
    );
    assert!(Arc::ptr_eq(&ai_update.module, &ai_proxy));
    assert!(ai_update.should_process);
    game.operate_with_current(1, Some(Arc::clone(&ai_update.module)), |world| {
        interface
            .lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
            .unwrap();
    });
    // This bounded active-path body stays awake. It is not a complete AI tick.
    game.scheduler.finish_owned_scheduled_update(
        1,
        ai_update,
        game_engine::common::thing::update_module::UpdateSleepTime::None,
    );
    let angle = game.source.read().unwrap().get_orientation();
    assert!(
        (angle - 0.1).abs() < 1.0e-5,
        "authored rotation per logic frame: {angle}"
    );
    let physics_proxy = proxy(&game.source, "PhysicsBehavior");
    let update = game
        .scheduler
        .next_owned_scheduled_update(1)
        .unwrap()
        .expect("actual due Physics heap entry");
    assert_eq!(
        update.phase,
        game_engine::common::thing::update_module::SleepyUpdatePhase::Physics
    );
    assert!(Arc::ptr_eq(&update.module, &physics_proxy));
    game.operate_with_current(1, Some(Arc::clone(&update.module)), |world| {
        world.run_scheduled_physics_update(update).unwrap();
    });
    assert_eq!(
        game.scheduler.sleepy_entry_for(&physics_proxy),
        Some((1, 2))
    );
    assert!((game.source.read().unwrap().get_orientation() - angle).abs() < 1.0e-5);
}

#[test]
fn individual_exact_waypoints_move_through_owned_internal_move_and_physics() {
    exact_waypoint_control(false);
}

#[test]
fn team_exact_waypoints_move_through_supplied_group_internal_move_and_physics() {
    exact_waypoint_control(true);
}

fn exact_waypoint_control(as_team: bool) {
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut game = new_game("OwnedExactWaypoint");
    install_waypoints(&mut game);
    if as_team {
        admit_source_group(&mut game);
    }
    let mut peer = new_game("OwnedExactWaypointPeer");
    if as_team {
        admit_source_group(&mut peer);
    }
    let peer_before = ai_snapshot(&peer, "OwnedExactWaypointPeerLoco");
    let interface = ai(&game.source);
    let mut command = AiCommandParams::new(
        if as_team {
            AiCommandType::FollowWaypointPathAsTeamExact
        } else {
            AiCommandType::FollowWaypointPathExact
        },
        CommandSourceType::FromPlayer,
    );
    command.waypoint = Some(9_801);
    game.operate(0, |world| {
        interface
            .lock()
            .unwrap()
            .execute_command_with_world(&command, world)
            .unwrap()
    });
    assert_eq!(
        interface.lock().unwrap().get_current_state_id(),
        Some(if as_team {
            crate::modules::AIStateType::FollowWaypointPathAsTeamExact
        } else {
            crate::modules::AIStateType::FollowWaypointPathAsIndividualsExact
        } as u32)
    );
    let locomotor = LOCOMOTOR_STORE
        .get_template("OwnedExactWaypointLoco")
        .unwrap();
    let mut lookup =
        |name: &str| Ok((name == "OwnedExactWaypointLoco").then(|| Arc::clone(&locomotor)));
    let mut restored_moving_exact = false;
    let mut completed = false;
    for frame in 1..=2_000 {
        let owner = Arc::clone(&game.source);
        game.operate_with_current(frame, Some(proxy(&owner, "AIUpdateInterface")), |world| {
            interface
                .lock()
                .unwrap()
                .update_waypoint_phase_with_world(world)
                .unwrap();
        });
        {
            let mut interface = interface.lock().unwrap();
            let observation = interface
                .unit_ai_for_test()
                .unwrap()
                .native_waypoint_observation_for_test();
            completed = observation.completed_waypoint == Some(9_801);
        }
        if completed {
            break;
        }
        path_phase(&mut game, frame);
        physics_phase(&mut game, frame);
        if !restored_moving_exact && game.source.read().unwrap().get_position().x > 30.0 {
            let mut interface = interface.lock().unwrap();
            let mut saved = Cursor::new(Vec::new());
            interface
                .xfer_ai_update_state_with_locomotors(
                    &mut XferSave::new(&mut saved, 1),
                    &mut lookup,
                )
                .unwrap();
            let saved = saved.into_inner();
            interface
                .xfer_ai_update_state_with_locomotors(
                    &mut XferLoad::new(Cursor::new(saved.clone()), 1),
                    &mut lookup,
                )
                .unwrap();
            let mut resaved = Cursor::new(Vec::new());
            interface
                .xfer_ai_update_state_with_locomotors(
                    &mut XferSave::new(&mut resaved, 1),
                    &mut lookup,
                )
                .unwrap();
            assert_eq!(
                saved,
                resaved.into_inner(),
                "exact AI/FSM continuation retains movement and unresolved waypoint before supplied resolution"
            );
            restored_moving_exact = true;
        }
    }
    assert!(
        restored_moving_exact,
        "exact save/load interrupts real movement"
    );
    if as_team {
        assert!(!game.groups[0].read().unwrap().speed_dirty_for_test());
        assert!(
            peer.groups[0].read().unwrap().speed_dirty_for_test(),
            "same-ID peer group cache remains untouched"
        );
    }
    assert!(
        completed,
        "CPP exact route must complete the authored chain and report its entry waypoint"
    );
    assert!(game.source.read().unwrap().get_position().x >= 235.0);
    assert_eq!(
        ai_snapshot(&peer, "OwnedExactWaypointPeerLoco"),
        peer_before
    );
}

#[test]
fn installed_immune_flight_preserves_receiving_owner_and_saved_future() {
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut game = new_game("OwnedFlight");
    let mut peer = new_game("OwnedFlightPeer");
    let owner = Arc::clone(&game.source);
    let receiver = Arc::clone(&peer.source);
    for (session, object) in [(&mut game, &owner), (&mut peer, &receiver)] {
        session.operate(0, |world| {
            world
                .set_object_position(
                    &mut object.write().unwrap(),
                    &Coord3D::new(30.0, 30.0, 20.0),
                )
                .unwrap()
        });
        let physics = object.read().unwrap().get_physics().unwrap();
        let mut state = physics.access().unwrap();
        state.set_velocity(&Coord3D::new(1.0, 0.0, -1.0));
        drop(state);
        physics
            .set_immune_freefall_for_test(&object.read().unwrap())
            .unwrap();
    }
    let peer_before = receiver.read().unwrap().get_transform_matrix();
    physics_phase(&mut game, 1);
    let at_save = owner.read().unwrap().get_transform_matrix();
    assert!(at_save.w_axis.z < 20.0);
    assert_eq!(receiver.read().unwrap().get_transform_matrix(), peer_before);
    let source_module = owner
        .read()
        .unwrap()
        .module_by_name(&"PhysicsBehavior".into())
        .unwrap();
    let receiving_module = receiver
        .read()
        .unwrap()
        .module_by_name(&"PhysicsBehavior".into())
        .unwrap();
    let mut saved = Cursor::new(Vec::new());
    source_module.with_module(|module| module.xfer(&mut XferSave::new(&mut saved, 1)).unwrap());
    let saved = saved.into_inner();
    receiving_module.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    });
    peer.operate(1, |world| {
        world
            .set_object_position(&mut receiver.write().unwrap(), &at_save.w_axis.truncate())
            .unwrap()
    });
    for frame in 2..=40 {
        physics_phase(&mut game, frame);
        physics_phase(&mut peer, frame);
        assert_eq!(
            owner.read().unwrap().get_transform_matrix(),
            receiver.read().unwrap().get_transform_matrix()
        );
    }
    assert_eq!(owner.read().unwrap().get_position().z, 0.0);
    assert_eq!(
        owner.read().unwrap().get_health(),
        receiver.read().unwrap().get_health()
    );
}
