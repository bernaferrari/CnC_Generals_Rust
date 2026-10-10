//! Host recorder and replay regressions through live command boundaries.

use super::*;

fn move_command(destination: Vec3, player_id: u32) -> GameCommand {
    GameCommand {
        command_type: CommandType::MoveTo {
            destination,
            waypoints: Vec::new(),
        },
        player_id,
        command_id: 7,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(11)],
        modifier_keys: ModifierKeys::default(),
    }
}

#[test]
fn host_move_round_trips_to_do_move_to() {
    // C++ Recorder.cpp:455-492 writes network messages from TheCommandList.
    let command = move_command(Vec3::new(12.0, 0.0, -4.0), 3);
    let message = game_command_to_message(&command).expect("move must be a network message");
    assert!(matches!(
        message.get_type(),
        GameMessageType::DoMoveTo(coord) if (coord.x - 12.0).abs() < f32::EPSILON
    ));
    assert_eq!(message.get_player_index(), 3);

    let restored = game_message_to_host_command(&message).expect("playback must restore MoveTo");
    match restored.command_type {
        CommandType::MoveTo { destination, .. } => {
            assert!((destination.x - 12.0).abs() < f32::EPSILON);
            assert!((destination.z + 4.0).abs() < f32::EPSILON);
        }
        other => panic!("expected MoveTo, got {other:?}"),
    }
    assert_eq!(restored.player_id, 3);
}

#[test]
fn unknown_host_command_is_fail_closed() {
    let command = GameCommand {
        command_type: CommandType::Invalid,
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: Vec::new(),
        modifier_keys: ModifierKeys::default(),
    };
    assert!(game_command_to_message(&command).is_none());
}

#[test]
fn new_game_skirmish_code_matches_cpp() {
    assert_eq!(game_mode_to_new_game_code(GameMode::Skirmish), 2);
    assert_eq!(game_mode_to_new_game_code(GameMode::SinglePlayer), 0);
    assert_eq!(game_mode_to_new_game_code(GameMode::Shell), 4);
}

#[test]
fn set_replay_camera_message_uses_cpp_argument_layout() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "set_replay_camera_message_uses_cpp_argument_layout",
        || {
            tap_replay_camera_for_recorder(ReplayCameraPose {
                pos: Vec3::new(8.0, 1.0, 3.0),
                yaw: 0.25,
                pitch: 0.5,
                zoom: 1.5,
                cursor: 4,
                pixel: (12, 34),
                player_index: 2,
            });
            let snap = snapshot_command_list();
            let camera = snap
                .iter()
                .rev()
                .find(|msg| matches!(msg.get_type(), GameMessageType::SetReplayCamera(..)))
                .expect("tap must append MSG_SET_REPLAY_CAMERA");
            match camera.get_argument(0) {
                Some(GameMessageArgumentType::Location(coord)) => {
                    assert!((coord.x - 8.0).abs() < f32::EPSILON);
                }
                other => panic!("expected location, got {other:?}"),
            }
            match camera.get_argument(1) {
                Some(GameMessageArgumentType::Real(yaw)) => {
                    assert!((yaw - 0.25).abs() < f32::EPSILON);
                }
                other => panic!("expected yaw, got {other:?}"),
            }
            match camera.get_argument(2) {
                Some(GameMessageArgumentType::Real(pitch)) => {
                    assert!((pitch - 0.5).abs() < f32::EPSILON);
                }
                other => panic!("expected pitch, got {other:?}"),
            }
            match camera.get_argument(3) {
                Some(GameMessageArgumentType::Real(zoom)) => {
                    assert!((zoom - 1.5).abs() < f32::EPSILON);
                }
                other => panic!("expected zoom, got {other:?}"),
            }
            match camera.get_argument(4) {
                Some(GameMessageArgumentType::Integer(cursor)) => assert_eq!(*cursor, 4),
                other => panic!("expected cursor int, got {other:?}"),
            }
            match camera.get_argument(5) {
                Some(GameMessageArgumentType::Pixel(pixel)) => {
                    assert_eq!(pixel.x, 12);
                    assert_eq!(pixel.y, 34);
                }
                other => panic!("expected pixel, got {other:?}"),
            }
            assert_eq!(camera.get_player_index(), 2);
        },
    );
}

fn host_command(command_type: CommandType) -> GameCommand {
    GameCommand {
        command_type,
        player_id: 2,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(9)],
        modifier_keys: ModifierKeys::default(),
    }
}

#[test]
fn dozer_construct_round_trips_through_replay_tap() {
    // C++ Recorder.cpp:488-492 writes MSG_DOZER_CONSTRUCT from TheCommandList.
    let command = host_command(CommandType::DozerConstruct {
        template_name: "AmericaBarracks".to_string(),
        location: Vec3::new(40.0, 0.0, 8.0),
        orientation: 1.25,
    });
    let message = game_command_to_message(&command).expect("dozer construct must record");
    assert!(matches!(
        message.get_type(),
        GameMessageType::DozerConstruct(_, coord, angle)
            if (coord.x - 40.0).abs() < f32::EPSILON && (*angle - 1.25).abs() < f32::EPSILON
    ));
    match game_message_to_host_command(&message)
        .expect("playback must restore DozerConstruct")
        .command_type
    {
        CommandType::DozerConstruct {
            template_name,
            location,
            orientation,
        } => {
            assert_eq!(template_name, "AmericaBarracks");
            assert!((location.z - 8.0).abs() < f32::EPSILON);
            assert!((orientation - 1.25).abs() < f32::EPSILON);
        }
        other => panic!("expected DozerConstruct, got {other:?}"),
    }
}

#[test]
fn queue_unit_and_special_power_round_trip() {
    // C++ MessageStream.h:462-584 includes queue unit + special power network IDs.
    let queue = host_command(CommandType::QueueUnitCreate {
        template_name: "AmericaInfantryRanger".to_string(),
        quantity: 3,
    });
    let queue_msg = game_command_to_message(&queue).expect("queue unit must record");
    match game_message_to_host_command(&queue_msg)
        .expect("playback must restore QueueUnitCreate")
        .command_type
    {
        CommandType::QueueUnitCreate {
            template_name,
            quantity,
        } => {
            assert_eq!(template_name, "AmericaInfantryRanger");
            assert_eq!(quantity, 3);
        }
        other => panic!("expected QueueUnitCreate, got {other:?}"),
    }

    let power = host_command(CommandType::DoSpecialPower {
        power_type: SpecialPowerType::ParticleCannon,
        target: PowerTarget::Location(Vec3::new(15.0, 0.0, 4.0)),
    });
    let power_msg = game_command_to_message(&power).expect("special power must record");
    match game_message_to_host_command(&power_msg)
        .expect("playback must restore DoSpecialPower")
        .command_type
    {
        CommandType::DoSpecialPower { power_type, target } => {
            assert_eq!(power_type, SpecialPowerType::ParticleCannon);
            match target {
                PowerTarget::Location(pos) => {
                    assert!((pos.x - 15.0).abs() < f32::EPSILON);
                }
                other => panic!("expected location target, got {other:?}"),
            }
        }
        other => panic!("expected DoSpecialPower, got {other:?}"),
    }
}

#[test]
fn special_power_location_facing_round_trip() {
    let power = host_command(CommandType::DoSpecialPower {
        power_type: SpecialPowerType::SneakAttack,
        target: PowerTarget::LocationFacing {
            pos: Vec3::new(15.0, 0.0, 4.0),
            angle: 1.25,
        },
    });
    let power_msg = game_command_to_message(&power).expect("facing special must record");
    match game_message_to_host_command(&power_msg)
        .expect("playback must restore facing")
        .command_type
    {
        CommandType::DoSpecialPower { power_type, target } => {
            assert_eq!(power_type, SpecialPowerType::SneakAttack);
            assert!((target.location_pos().unwrap().x - 15.0).abs() < f32::EPSILON);
            assert!((target.location_angle() - 1.25).abs() < 1.0e-5);
        }
        other => panic!("expected DoSpecialPower, got {other:?}"),
    }
}

#[test]
fn apply_replay_new_game_posts_to_the_message_stream() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "apply_replay_new_game_posts_to_the_message_stream",
        || {
            // C++ GameLogicDispatch.cpp:396-421 MSG_NEW_GAME starts the match.
            use game_engine::common::message_stream::get_message_stream;
            {
                let stream = get_message_stream();
                stream
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .clear_messages();
            }
            let mut message = GameMessage::new(GameMessageType::NewGame);
            message.append_integer_argument(3);
            message.append_integer_argument(2);
            message.append_integer_argument(10);
            message.append_integer_argument(45);
            apply_replay_messages_to_host(&mut ReplayPendingState::default(), &[message]);
            let stream = get_message_stream();
            let guard = stream.read().unwrap_or_else(|e| e.into_inner());
            let found = guard
                .get_messages()
                .iter()
                .any(|msg| matches!(msg.get_type(), GameMessageType::NewGame));
            assert!(found, "playback NewGame must land on TheMessageStream");
        },
    );
}

#[test]
fn tap_create_team_slot_records_object_ids() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "tap_create_team_slot_records_object_ids",
        || {
            // C++ SelectionXlat.cpp:1047 MSG_CREATE_TEAM0+group carries object IDs.
            tap_host_team_slot_for_recorder(3, 0, &[ObjectId(11), ObjectId(12)]);
            let snap = snapshot_command_list();
            let team = snap
                .iter()
                .rev()
                .find(|msg| matches!(msg.get_type(), GameMessageType::CreateTeamSlot(3)))
                .expect("create team must append MSG_CREATE_TEAM3");
            let ids = object_ids_from_message(team);
            assert_eq!(ids, vec![ObjectId(11), ObjectId(12)]);
        },
    );
}

#[test]
fn observer_mismatch_blocks_replay_camera_apply() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "observer_mismatch_blocks_replay_camera_apply",
        || {
            // C++ GameLogicDispatch.cpp:1803 requires getObserverLookAtPlayer()==thisPlayer.
            #[cfg(feature = "game_client")]
            {
                game_client::gui::control_bar::control_bar_observer::set_observer_look_at_player(
                    Some(1),
                );
                assert!(host_replay_observer_matches_player(1));
                assert!(!host_replay_observer_matches_player(0));
                game_client::gui::control_bar::control_bar_observer::set_observer_look_at_player(
                    None,
                );
                assert!(!host_replay_observer_matches_player(1));
            }
            #[cfg(not(feature = "game_client"))]
            {
                assert!(!host_replay_observer_matches_player(0));
            }
        },
    );
}

#[test]
fn playback_move_queues_observer_selection_remirror() {
    // C++ GameLogicDispatch.cpp:1970-1984 remirrors after every network command.
    let mut state = ReplayPendingState::default();
    let message =
        GameMessage::with_player(GameMessageType::DoMoveTo(Coord3D::new(4.0, 0.0, 1.0)), 4);
    apply_replay_messages_to_host(&mut state, &[message]);
    assert_eq!(state.take_selection_remirror(), vec![4]);
}

#[test]
fn replay_handoffs_are_isolated_between_game_instances() {
    let mut first = ReplayPendingState::default();
    let mut second = ReplayPendingState::default();
    let move_message =
        GameMessage::with_player(GameMessageType::DoMoveTo(Coord3D::new(4.0, 0.0, 1.0)), 4);
    apply_replay_messages_to_host(&mut first, &[move_message]);
    stamp_host_logic_frame(&mut first, 100);
    stamp_host_logic_frame(&mut second, 30);

    assert_eq!(first.commands.len(), 1);
    assert!(second.commands.is_empty());
    assert_eq!(first.take_selection_remirror(), vec![4]);
    assert!(second.take_selection_remirror().is_empty());
    assert_eq!(first.host_logic_frame.load(Ordering::Relaxed), 100);
    assert_eq!(second.host_logic_frame.load(Ordering::Relaxed), 30);

    let stale_sender = first.sender.clone();
    first.clear();
    assert!(first.commands.is_empty());
    assert_eq!(first.host_logic_frame.load(Ordering::Relaxed), 0);
    assert_eq!(second.host_logic_frame.load(Ordering::Relaxed), 30);
    assert!(stale_sender.send(Vec::new()).is_err());
}

#[test]
fn set_replay_camera_does_not_queue_selection_remirror() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "set_replay_camera_does_not_queue_selection_remirror",
        || {
            let mut state = ReplayPendingState::default();
            let pose = ReplayCameraPose {
                pos: Vec3::new(1.0, 2.0, 3.0),
                yaw: 0.0,
                pitch: 0.0,
                zoom: 1.0,
                cursor: 0,
                pixel: (0, 0),
                player_index: 3,
            };
            tap_replay_camera_for_recorder(pose);
            let snap = snapshot_command_list();
            let camera = snap
                .iter()
                .rev()
                .find(|msg| matches!(msg.get_type(), GameMessageType::SetReplayCamera(..)))
                .cloned()
                .expect("camera tap");
            apply_replay_messages_to_host(&mut state, &[camera]);
            assert!(state.take_selection_remirror().is_empty());
            let stored = state.take_camera().expect("pose stored");
            assert_eq!(stored.player_index, 3);
        },
    );
}

#[test]
fn live_host_posts_logic_crc_every_replay_interval() {
    // C++ GameLogic.cpp:3634 — m_frame > 0 && (m_frame % REPLAY_CRC_INTERVAL) == 0.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "live_host_posts_logic_crc_every_replay_interval",
        || {
            let mut state = ReplayPendingState::default();
            clear_command_list();
            stamp_host_logic_frame(&mut state, 100);
            let posted =
                post_host_logic_crc_if_due(&mut state, 100, 0xABCD_0001).expect("frame 100 is due");
            let snap = snapshot_command_list();
            let crc_msg = snap
                .iter()
                .rev()
                .find(|msg| matches!(msg.get_type(), GameMessageType::LogicCRC(_)))
                .expect("MSG_LOGIC_CRC must land on TheCommandList");
            match crc_msg.get_type() {
                GameMessageType::LogicCRC(value) => assert_eq!(*value, posted),
                other => panic!("expected LogicCRC, got {other:?}"),
            }
            assert!(matches!(
                crc_msg.get_argument(0),
                Some(GameMessageArgumentType::Boolean(_))
            ));

            assert!(
                post_host_logic_crc_if_due(&mut state, 101, 0xABCD_0002).is_none(),
                "off-interval frames must not emit LogicCRC"
            );
            assert!(
                post_host_logic_crc_if_due(&mut state, 0, 0xABCD_0003).is_none(),
                "frame 0 must not emit LogicCRC (C++ m_frame > 0 guard)"
            );
            clear_command_list();
        },
    );
}

#[test]
fn logic_crc_post_records_supplied_owner_state_unchanged() {
    // GameLogic.cpp:3636-3652 emits the checksum without hashing it again.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "logic_crc_post_records_supplied_owner_state_unchanged",
        || {
            let mut state = ReplayPendingState::default();
            clear_command_list();
            stamp_host_logic_frame(&mut state, 100);
            let plain = 0xABCD_0001;
            let posted =
                post_host_logic_crc_if_due(&mut state, 100, plain).expect("frame 100 is due");
            assert_eq!(
                posted, plain,
                "record the driving simulation's checksum unchanged"
            );
            assert_eq!(
                post_host_logic_crc_if_due(&mut state, 100, 0xBAD_0002),
                Some(plain),
                "a second command pass retains the original frame-phase observation"
            );
            assert_eq!(
                snapshot_command_list()
                    .iter()
                    .filter(|msg| matches!(msg.get_type(), GameMessageType::LogicCRC(_)))
                    .count(),
                1
            );
            clear_command_list();
        },
    );
}

#[test]
fn main_command_flush_crc_observes_driving_world_with_foreign_core_ai_held() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "main_command_flush_crc_observes_driving_world_with_foreign_core_ai_held",
        || {
            use crate::game_logic::{Player, ThingTemplate};
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let mut foreign_ai = foreign.ai().write().unwrap();
            let group = foreign_ai.create_group();
            let group_id = group.read().unwrap().get_id();
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            let mut ids = Vec::new();
            for world in [&mut first, &mut second] {
                world.set_logic_random_seed(0x5EED);
                world.frame = 100;
                world.add_player(Player::new(0, Team::USA, "Human", true));
                world
                    .templates
                    .insert("CrcUnit".into(), ThingTemplate::new("CrcUnit"));
                ids.push(
                    world
                        .create_object_for_player("CrcUnit", 0, Vec3::ZERO)
                        .unwrap(),
                );
            }
            assert_eq!(ids[0], ids[1], "same admitted identity in distinct owners");
            second.get_object_mut(ids[1]).unwrap().health.current = 37.0;
            second.get_player_mut(0).unwrap().resources.supplies = 27;
            let first_crc = first.logic_crc();
            let second_crc = second.logic_crc();
            assert_ne!(
                first_crc, second_crc,
                "CRC sees canonical owner state, not an empty Core registry"
            );
            let _ = with_recorder_mut(|recorder| recorder.reset());
            clear_command_list();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let first_services = first.world_services.clone();
                gamelogic::system::engine_stores::with_world_services(&first_services, || {
                    first.process_commands();
                });
                assert_eq!(first.replay_pending.last_logic_crc, first_crc);
                assert_eq!(first.replay_pending.last_logic_crc_frame, 100);
                let second_services = second.world_services.clone();
                gamelogic::system::engine_stores::with_world_services(&second_services, || {
                    second.process_commands();
                });
                assert_eq!(second.replay_pending.last_logic_crc, second_crc);
                assert_eq!(
                    first.logic_crc(),
                    first_crc,
                    "interleaved command flush leaves A intact"
                );
                first.get_object_mut(ids[0]).unwrap().health.current = 19.0;
                gamelogic::system::engine_stores::with_world_services(&first_services, || {
                    first.process_commands();
                });
                assert_eq!(
                    first.replay_pending.last_logic_crc, first_crc,
                    "second pass cannot replace the original pre-command checksum"
                );
            });
            assert!(foreign_ai.get_group_by_id(group_id).is_some());
            clear_command_list();
        },
    );
}

#[test]
fn logic_crc_message_carries_local_player_index() {
    // C++ GameLogicDispatch.cpp:1904-1946 threads the local player index.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "logic_crc_message_carries_local_player_index",
        || {
            let mut state = ReplayPendingState::default();
            clear_command_list();
            {
                let mut list = gamelogic::player::ThePlayerList()
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                list.clear();
                for index in 0..3 {
                    list.add_player(Arc::new(std::sync::RwLock::new(
                        gamelogic::player::Player::new(index),
                    )));
                }
                list.set_local_player_index(2);
            }
            stamp_host_logic_frame(&mut state, 300);
            assert!(
                post_host_logic_crc_if_due(&mut state, 300, 0).is_some(),
                "frame 300 is due"
            );
            let snap = snapshot_command_list();
            let crc_msg = snap
                .iter()
                .rev()
                .find(|msg| matches!(msg.get_type(), GameMessageType::LogicCRC(_)))
                .expect("MSG_LOGIC_CRC must land on TheCommandList");
            assert_eq!(
                crc_msg.get_player_index(),
                2,
                "MSG_LOGIC_CRC must be attributed to the local player slot"
            );
            {
                let mut list = gamelogic::player::ThePlayerList()
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                list.clear();
            }
            clear_command_list();
        },
    );
}

#[test]
fn flush_recorder_writes_then_consumes_logic_crc() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "flush_recorder_writes_then_consumes_logic_crc",
        || {
            let mut state = ReplayPendingState::default();
            bind_host_replay_authority(&state);
            let _ = with_recorder_mut(|recorder| recorder.reset());
            clear_command_list();
            stamp_host_logic_frame(&mut state, 200);
            // Premise: the stamp is the frame source (crate GameLogic
            // singleton untouched in this test binary).
            assert_eq!(host_logic_frame(&state), 200);
            let mut queue = VecDeque::new();
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            // The flush posts MSG_LOGIC_CRC for updateRecord before update()...
            assert_eq!(
                state.last_logic_crc_frame, 200,
                "flush must post MSG_LOGIC_CRC before updateRecord"
            );
            // ...then consumes it after the write: C++ processCommandList
            // (GameLogic.cpp:3669) drains TheCommandList, so a later pump
            // must not re-write the same CRC.
            let snap = snapshot_command_list();
            assert!(
                snap.iter()
                    .all(|msg| !matches!(msg.get_type(), GameMessageType::LogicCRC(_))),
                "written MSG_LOGIC_CRC entries must be consumed, not retained"
            );
        },
    );
}

#[test]
fn recorder_updates_once_even_when_commands_are_processed_twice_in_a_frame() {
    // C++ GameLogic::update calls Recorder::UPDATE once before command
    // dispatch. Rust's second post-AI command pass still executes orders,
    // but must not tick the recorder for the same logic frame again.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "recorder_updates_once_even_when_commands_are_processed_twice_in_a_frame",
        || {
            use std::sync::atomic::AtomicUsize;

            let mut state = ReplayPendingState::default();
            bind_host_replay_authority(&state);
            clear_command_list();
            let updates = Arc::new(AtomicUsize::new(0));
            let counted_updates = Arc::clone(&updates);
            with_recorder_mut(|recorder| {
                recorder.reset();
                recorder.set_command_source(Some(Arc::new(move || {
                    counted_updates.fetch_add(1, Ordering::SeqCst);
                    Vec::new()
                })));
            });

            let mut queue = VecDeque::new();
            stamp_host_logic_frame(&mut state, 7);
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            assert_eq!(updates.load(Ordering::SeqCst), 1);

            stamp_host_logic_frame(&mut state, 8);
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            assert_eq!(updates.load(Ordering::SeqCst), 2);

            with_recorder_mut(|recorder| {
                recorder.reset();
                recorder.set_command_source(Some(Arc::new(snapshot_command_list)));
            });
            clear_command_list();
        },
    );
}

#[test]
fn recorder_preserves_a_direct_order_arriving_after_the_first_flush() {
    // Main UI paths synchronously queue and process an order between logic
    // ticks. Once this frame has recorded, its CommandList tap belongs to
    // the next recorder snapshot; another process_commands call cannot
    // drop it merely because the logic frame has not advanced yet.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "recorder_preserves_a_direct_order_arriving_after_the_first_flush",
        || {
            let mut state = ReplayPendingState::default();
            bind_host_replay_authority(&state);
            clear_command_list();
            let (captured, snapshots) = std::sync::mpsc::channel::<Vec<GameMessage>>();
            with_recorder_mut(|recorder| {
                recorder.reset();
                recorder.set_command_source(Some(Arc::new(move || {
                    let snapshot = snapshot_command_list();
                    captured.send(snapshot.clone()).unwrap();
                    snapshot
                })));
            });

            let mut queue = VecDeque::new();
            stamp_host_logic_frame(&mut state, 7);
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            append_to_command_list(GameMessage::with_player(GameMessageType::DoStop, 1));
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            assert!(matches!(
                snapshot_command_list()[0].get_type(),
                GameMessageType::DoStop
            ));

            stamp_host_logic_frame(&mut state, 8);
            state
                .sender
                .send(vec![GameMessage::with_player(
                    GameMessageType::DoMoveTo(Coord3D::new(3.0, 0.0, 4.0)),
                    1,
                )])
                .unwrap();
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            let captured: Vec<_> = snapshots.try_iter().collect();
            assert_eq!(captured.len(), 2);
            assert!(captured[0].is_empty());
            assert_eq!(captured[1].len(), 2);
            assert!(matches!(captured[1][0].get_type(), GameMessageType::DoStop));
            assert!(matches!(
                captured[1][1].get_type(),
                GameMessageType::DoMoveTo(_)
            ));

            with_recorder_mut(|recorder| {
                recorder.reset();
                recorder.set_command_source(Some(Arc::new(snapshot_command_list)));
            });
            clear_command_list();
        },
    );
}

#[test]
fn recorder_sees_routed_stream_orders_before_same_frame_crc() {
    // C++ GameEngine::update propagates the MessageStream before the
    // GameLogic CRC/Recorder phase. The GameClient route callback transfers
    // the ordered batch, which the owner places on CommandList before the
    // recorder snapshots this logic frame.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "recorder_sees_routed_stream_orders_before_same_frame_crc",
        || {
            let mut state = ReplayPendingState::default();
            bind_host_replay_authority(&state);
            let temp = tempfile::tempdir().unwrap();
            let global = game_engine::common::ini::ini_game_data::ensure_global_data();
            {
                let mut data = global.write();
                data.set_path_user_data(temp.path().to_string_lossy().to_string());
                data.map_name = "Maps/RecorderOrder.map".to_string();
                data.pending_file.clear();
            }
            clear_command_list();
            let (captured, snapshots) = std::sync::mpsc::channel::<Vec<GameMessage>>();
            with_recorder_mut(|recorder| {
                recorder.reset();
                recorder.set_command_source(Some(Arc::new(move || {
                    let messages = snapshot_command_list();
                    captured.send(messages.clone()).unwrap();
                    messages
                })));
                recorder
                    .start_recording(1, 1, 0, 30)
                    .expect("start order recording");
            });

            state
                .sender
                .send(vec![
                    GameMessage::with_player(GameMessageType::DoStop, 1),
                    GameMessage::with_player(
                        GameMessageType::DoMoveTo(Coord3D::new(7.0, 0.0, 9.0)),
                        1,
                    ),
                ])
                .unwrap();
            let mut crc = GameMessage::new(GameMessageType::LogicCRC(0x1234));
            crc.append_boolean_argument(false);
            append_to_command_list(crc);

            stamp_host_logic_frame(&mut state, 7);
            let mut queue = VecDeque::new();
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);

            let captured: Vec<_> = snapshots.try_iter().collect();
            assert_eq!(captured.len(), 1, "one recorder snapshot per logic frame");
            assert!(matches!(captured[0][0].get_type(), GameMessageType::DoStop));
            assert!(matches!(
                captured[0][1].get_type(),
                GameMessageType::DoMoveTo(_)
            ));
            assert!(matches!(
                captured[0][2].get_type(),
                GameMessageType::LogicCRC(0x1234)
            ));
            assert_eq!(queue.len(), 2, "routed orders execute once");

            with_recorder_mut(|recorder| recorder.stop_recording());
            let (played_sink, played) = std::sync::mpsc::channel::<GameMessage>();
            let mut reader = game_engine::common::recorder::Recorder::new();
            reader.set_command_sink(Some(Arc::new(move |message| {
                played_sink.send(message).unwrap();
            })));
            assert!(reader.playback_file("00000000.rep".to_string()).unwrap());
            played.try_iter().for_each(drop); // playback_file emits its own MSG_NEW_GAME.
            for frame in 1..=7 {
                reader.set_current_frame(frame);
                reader.update();
            }
            let played: Vec<_> = played.try_iter().collect();
            assert!(matches!(played[0].get_type(), GameMessageType::DoStop));
            assert!(matches!(played[1].get_type(), GameMessageType::DoMoveTo(_)));
            assert!(matches!(
                played[2].get_type(),
                GameMessageType::LogicCRC(0x1234)
            ));
            assert_eq!(played.len(), 4, "duplicate flush must not write twice");
            assert!(matches!(
                played[3].get_type(),
                GameMessageType::ClearGameData
            )); // End-of-file playback lifecycle message.

            with_recorder_mut(|recorder| {
                recorder.reset();
                recorder.set_command_source(Some(Arc::new(snapshot_command_list)));
            });
            clear_command_list();
        },
    );
}

#[test]
fn router_batch_arrives_in_the_same_logic_frame() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "router_batch_arrives_in_the_same_logic_frame",
        || {
            let mut state = ReplayPendingState::default();
            bind_host_replay_authority(&state);
            let _ = with_recorder_mut(|recorder| recorder.reset());
            clear_command_list();
            let message =
                GameMessage::with_player(GameMessageType::DoMoveTo(Coord3D::new(7.0, 0.0, 9.0)), 2);
            state.sender.send(vec![message]).unwrap();
            stamp_host_logic_frame(&mut state, 1);
            let mut queue = VecDeque::new();
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            assert_eq!(queue.len(), 1);
            assert_eq!(queue.front().unwrap().player_id, 2);
            assert!(matches!(
                &queue.front().unwrap().command_type,
                CommandType::MoveTo { .. }
            ));
            clear_command_list();
        },
    );
}

#[test]
fn new_game_starts_one_recording_and_leaves_no_lifecycle_message() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "new_game_starts_one_recording_and_leaves_no_lifecycle_message",
        || {
            let mut state = ReplayPendingState::default();
            bind_host_replay_authority(&state);
            let temp = tempfile::tempdir().unwrap();
            let global = game_engine::common::ini::ini_game_data::ensure_global_data();
            {
                let mut data = global.write();
                data.set_path_user_data(temp.path().to_string_lossy().to_string());
                data.map_name = "Maps/RecorderLifecycle.map".to_string();
                data.pending_file.clear();
            }
            clear_command_list();
            install_host_replay_bridges();
            with_recorder_mut(|recorder| recorder.reset());
            tap_host_new_game_for_recorder(GameMode::Skirmish, 0x5EED_0021);

            let mut queue = VecDeque::new();
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            assert!(with_recorder(|recorder| recorder.is_recording()).unwrap_or(false));
            assert_eq!(
                with_recorder(|recorder| recorder.get_game_info().seed),
                Some(0x5EED_0021),
                "the actual Main recorder tap carries the selected owner seed"
            );
            assert!(
                snapshot_command_list()
                    .iter()
                    .all(|msg| !matches!(msg.get_type(), GameMessageType::NewGame)),
                "C++ resets TheCommandList after each logic frame, so MSG_NEW_GAME must not start a second recording"
            );

            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            assert!(with_recorder(|recorder| recorder.is_recording()).unwrap_or(false));
            reset_host_recorder_after_successful_load(&mut state);
            bind_host_replay_authority(&state);
            assert!(!with_recorder(|recorder| recorder.is_recording()).unwrap_or(true));
            flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            assert!(!with_recorder(|recorder| recorder.is_recording()).unwrap_or(true));
            clear_command_list();
        },
    );
}

#[test]
fn slow_client_pump_records_each_logic_crc_exactly_once() {
    // C++ processCommandList consumes TheCommandList every logic frame
    // (GameLogic.cpp:3669). With a slow client pump (3 fixed steps per
    // pump) each interval boundary's MSG_LOGIC_CRC must still reach the
    // .rep exactly once — retaining them would re-write old CRCs on
    // every pump and drift the playback CRC queue.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "slow_client_pump_records_each_logic_crc_exactly_once",
        || {
            let mut state = ReplayPendingState::default();
            bind_host_replay_authority(&state);
            let temp = tempfile::tempdir().unwrap();
            // Recorder reads the replay dir from ini_game_data's GlobalData.
            let global = game_engine::common::ini::ini_game_data::ensure_global_data();
            {
                let mut data = global.write();
                data.set_path_user_data(temp.path().to_string_lossy().to_string());
                data.map_name = "Maps/SlowPump.map".to_string();
                data.pending_file.clear();
            }
            // Local player in slot 2 so the recorded attribution is non-zero.
            {
                let mut list = gamelogic::player::ThePlayerList()
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                list.clear();
                for index in 0..3 {
                    list.add_player(Arc::new(std::sync::RwLock::new(
                        gamelogic::player::Player::new(index),
                    )));
                }
                list.set_local_player_index(2);
            }
            clear_command_list();
            // The global recorder is lazy: `with_recorder_mut` is a no-op
            // until the host replay bridges install the singleton, so
            // `start_recording` would silently never run and the playback
            // below would find no .rep (NotFound). Install the bridges first,
            // exactly like every live entry point does.
            install_host_replay_bridges();
            with_recorder_mut(|recorder| {
                recorder.reset();
                recorder
                    .start_recording(1, 2, 0, 30)
                    .expect("start recording");
            });

            let interval = game_engine::common::crc_debug::replay_crc_interval().max(1) as u32;
            const STEPS_PER_PUMP: u32 = 3;
            let pumps = (3 * interval) / STEPS_PER_PUMP + 2;
            let mut frame = 0u32;
            for _ in 0..pumps {
                for _ in 0..STEPS_PER_PUMP {
                    frame += 1;
                    stamp_host_logic_frame(&mut state, frame);
                    let _ = post_host_logic_crc_if_due(&mut state, frame, 0xFEED_F00D);
                }
                let mut queue = VecDeque::new();
                flush_recorder_and_replay_authority(&mut state, &mut queue, 0xCAFE_0001);
            }
            with_recorder_mut(|recorder| recorder.stop_recording());

            // Read the .rep back through a playback sink.
            let (sink_players, crc_players) = std::sync::mpsc::channel::<i32>();
            let mut reader = game_engine::common::recorder::Recorder::new();
            reader.set_command_sink(Some(Arc::new(move |msg: GameMessage| {
                if matches!(msg.get_type(), GameMessageType::LogicCRC(_)) {
                    sink_players.send(msg.get_player_index()).unwrap();
                }
            })));
            assert!(
                reader
                    .playback_file("00000000.rep".to_string())
                    .expect("open recorded .rep")
            );
            for f in 1..=frame {
                reader.set_current_frame(f);
                reader.update();
            }
            let crc_players: Vec<_> = crc_players.try_iter().collect();
            assert_eq!(
                crc_players.as_slice(),
                &[2, 2, 2],
                "each recorded MSG_LOGIC_CRC carries the local player slot"
            );
            gamelogic::player::ThePlayerList()
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
        },
    );
}
