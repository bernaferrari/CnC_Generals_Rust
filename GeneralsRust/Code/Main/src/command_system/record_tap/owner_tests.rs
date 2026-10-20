use super::*;

fn inputs(path: &std::path::Path, map: &str, seed: u32) -> RecorderOwnerInputs {
    RecorderOwnerInputs {
        map: map.into(),
        seed,
        replay_directory: path.join("Replays"),
        exe_crc: 0,
        ini_crc: 0,
        save_stats: false,
        stats_directory: path.join("Stats"),
    }
}
fn pose(player: i32, x: f32) -> ReplayCameraPose {
    ReplayCameraPose {
        pos: Vec3::new(x, 2.0, 3.0),
        yaw: 0.25,
        pitch: 0.75,
        zoom: 1.5,
        cursor: 0,
        pixel: (0, 0),
        player_index: player,
    }
}
fn write_file(state: &mut ReplayPendingState, player: i32, x: f32) -> String {
    state.record_new_game_owned(GameMode::Skirmish, 37, 1, 0, 30);
    state.flush_owned(0, &mut VecDeque::new(), 0, player, None);
    assert!(state.recorder_owned().is_recording());
    state.record_camera_owned(pose(player, x));
    state.flush_owned(2, &mut VecDeque::new(), 0, player, None);
    state.record_camera_owned(pose(player, x + 10.0));
    state.flush_owned(4, &mut VecDeque::new(), 0, player, None);
    let filename = format!(
        "{}{}",
        state.recorder_owned().last_replay_filename(),
        state.recorder_owned().replay_extension()
    );
    state.recorder.stop_recording();
    filename
}

#[test]
fn real_replay_reader_moves_with_world_and_transient_clear_preserves_future_camera() {
    let directory = tempfile::tempdir().unwrap();
    let mut writer = ReplayPendingState::default();
    let owner = inputs(&directory.path().join("A"), "Maps/A.map", 37);
    writer.reset_recorder_owned(owner.clone());
    let filename = write_file(&mut writer, 7, 100.0);
    let mut world = GameLogic::new();
    world.replay_pending.reset_recorder_owned(owner);
    assert!(
        world
            .replay_pending
            .playback_file_owned(filename, false)
            .unwrap()
    );
    assert!(
        matches!(world.replay_pending.take_lifecycle_messages_owned().as_slice(), [message] if matches!(message.get_type(), GameMessageType::NewGame))
    );
    world
        .replay_pending
        .flush_owned(2, &mut VecDeque::new(), 0, 7, None);
    assert_eq!(world.replay_pending.take_camera(), Some(pose(7, 100.0)));
    // Moving the actual GameLogic moves the open reader, with no frame callback rebind.
    let mut installed_world = world;
    installed_world.replay_pending.clear();
    installed_world
        .replay_pending
        .flush_owned(4, &mut VecDeque::new(), 0, 7, None);
    assert_eq!(
        installed_world.replay_pending.take_camera(),
        Some(pose(7, 110.0))
    );
    assert_eq!(
        installed_world.replay_pending.recorder_owned().get_mode(),
        game_engine::common::recorder::RecorderMode::Playback
    );
    assert!(
        matches!(installed_world.replay_pending.take_lifecycle_messages_owned().as_slice(), [message] if matches!(message.get_type(), GameMessageType::ClearGameData))
    );
}

#[test]
fn two_world_recorders_do_not_rebind_each_other_on_reset_or_failed_header() {
    let directory = tempfile::tempdir().unwrap();
    let a_inputs = inputs(&directory.path().join("A"), "Maps/A.map", 37);
    let b_inputs = inputs(&directory.path().join("B"), "Maps/B.map", 91);
    let mut a = ReplayPendingState::default();
    let mut b = ReplayPendingState::default();
    a.reset_recorder_owned(a_inputs.clone());
    b.reset_recorder_owned(b_inputs.clone());
    let a_file = write_file(&mut a, 7, 100.0);
    let b_file = write_file(&mut b, 7, 900.0);
    a.reset_recorder_owned(a_inputs);
    b.reset_recorder_owned(b_inputs.clone());
    assert!(a.playback_file_owned(a_file, false).unwrap());
    assert!(b.playback_file_owned(b_file, false).unwrap());
    a.take_lifecycle_messages_owned();
    b.take_lifecycle_messages_owned();
    a.flush_owned(2, &mut VecDeque::new(), 0, 7, None);
    b.flush_owned(2, &mut VecDeque::new(), 0, 7, None);
    assert_eq!(a.take_camera(), Some(pose(7, 100.0)));
    assert_eq!(b.take_camera(), Some(pose(7, 900.0)));
    b.reset_recorder_owned(b_inputs.clone());
    std::fs::write(b_inputs.replay_directory.join("invalid.rep"), b"BADREP").unwrap();
    assert!(!b.playback_file_owned("invalid.rep".into(), false).unwrap());
    assert!(b.take_lifecycle_messages_owned().is_empty());
    b.stop_playback_owned();
    b.reset_recorder_owned(b_inputs);
    drop(b);
    a.flush_owned(4, &mut VecDeque::new(), 0, 7, None);
    assert_eq!(a.take_camera(), Some(pose(7, 110.0)));
    assert_eq!(a.recorder_owned().get_game_info().map, "Maps/A.map");
}

#[test]
fn receiving_playback_continuation_clears_actual_world_before_mode_or_header_admission() {
    let directory = tempfile::tempdir().unwrap();
    let owner = inputs(directory.path(), "Maps/BeforeClear.map", 37);
    std::fs::create_dir_all(&owner.replay_directory).unwrap();
    let mut world = GameLogic::new();
    world.replay_pending.reset_recorder_owned(owner.clone());
    world.start_new_game_for_restore(GameMode::Skirmish);
    world.force_map_loaded_for_path_test(true);
    let unit = world
        .create_object_for_player("USA_Ranger", 0, Vec3::ZERO)
        .expect("actual receiving factory must create old-world object");
    assert!(world.get_object(unit).is_some());
    assert!(world.isInGame());
    // The file does not exist yet: preparation cannot perform header I/O.
    let admission = world
        .replay_pending
        .prepare_playback_owned("after-clear.rep".into(), true);
    assert!(admission.requires_clear());
    assert_eq!(
        world.replay_pending.recorder_owned().get_mode(),
        game_engine::common::recorder::RecorderMode::None,
        "Playback mode/header admission must not precede source clearGameData"
    );
    assert!(
        world.isInGame(),
        "preparation must release a continuation before clearing its receiver"
    );
    world.clearGameData();
    world
        .replay_pending
        .reset_recorder_for_match_owned(String::new(), world.logic_base_seed);
    assert!(!world.isInGame());
    assert!(world.get_current_map_name().is_empty());
    assert!(
        world.get_object(unit).is_none(),
        "actual clear must remove the old-world object before header admission"
    );
    // This is the real failed-header path after the actual world was cleared.
    std::fs::write(owner.replay_directory.join("after-clear.rep"), b"BADREP").unwrap();
    assert!(
        !world
            .replay_pending
            .admit_prepared_playback_owned(admission)
            .unwrap()
    );
    assert!(
        !world.isInGame(),
        "failed admission cannot restore the old running world"
    );
    assert!(world.get_current_map_name().is_empty());
    assert!(
        world.get_object(unit).is_none(),
        "actual clear must remove the old-world object before header admission"
    );
    assert_eq!(
        world.replay_pending.recorder_owned().get_mode(),
        game_engine::common::recorder::RecorderMode::Playback
    );
    assert!(
        world
            .replay_pending
            .take_lifecycle_messages_owned()
            .is_empty(),
        "clear already executed synchronously, never enqueue another late reset"
    );
}

#[test]
fn recorded_missing_map_cannot_admit_a_different_match() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "recorded_missing_map_cannot_admit_a_different_match",
        || {
            let directory = tempfile::tempdir().unwrap();
            let missing = directory
                .path()
                .join("MissingRecordedMission.map")
                .to_string_lossy()
                .into_owned();
            let policy = inputs(directory.path(), &missing, 37);
            let mut writer = ReplayPendingState::default();
            writer.reset_recorder_owned(policy.clone());
            let filename = write_file(&mut writer, 0, 100.0);
            let mut receiver = GameLogic::new();
            receiver.replay_pending.reset_recorder_owned(policy);
            assert!(
                receiver
                    .replay_pending
                    .playback_file_owned(filename, false)
                    .unwrap()
            );
            let requested = receiver
                .replay_pending
                .recorder_owned()
                .get_game_info()
                .map
                .clone();
            assert_eq!(requested, missing);
            receiver.start_new_game_with_recorder_payload(GameMode::Replay, 1, 0, 30);
            assert_eq!(
                receiver.load_recorded_map_with_progress(&requested, |_, _| {}),
                None
            );
            assert!(
                !receiver.isInGame(),
                "missing recorded map cannot become a playable default match"
            );
            assert_eq!(
                receiver.replay_pending.recorder_owned().get_game_info().map,
                missing
            );
            assert!(receiver.replay_pending.recorder_owned().is_playback());
        },
    );
}

#[test]
fn actual_shell_and_match_clear_before_valid_or_failed_replay_header() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_shell_and_match_clear_before_valid_or_failed_replay_header",
        || {
            let directory = tempfile::tempdir().unwrap();
            let policy = inputs(directory.path(), "Maps/ReceivingReplay.map", 37);
            let mut writer = ReplayPendingState::default();
            writer.reset_recorder_owned(policy.clone());
            let valid = write_file(&mut writer, 0, 100.0);
            let invalid = policy.replay_directory.join("invalid.rep");
            std::fs::write(&invalid, b"BADREP").unwrap();
            for mode in [GameMode::Shell, GameMode::Skirmish] {
                for (filename, expected) in [
                    (valid.clone(), true),
                    (invalid.to_string_lossy().into_owned(), false),
                ] {
                    let mut world = GameLogic::new();
                    world.replay_pending.reset_recorder_owned(policy.clone());
                    world.start_new_game_for_restore(mode);
                    // In particular Shell is active for source clear although no map is loaded.
                    assert_ne!(world.game_mode(), GameMode::None);
                    let active = world.game_mode() != GameMode::None;
                    let token = world
                        .replay_pending
                        .prepare_playback_owned(filename, active);
                    assert!(token.requires_clear());
                    assert!(!world.replay_pending.recorder_owned().is_playback());
                    world.clearGameData();
                    world
                        .replay_pending
                        .reset_recorder_for_match_owned(String::new(), world.logic_base_seed);
                    assert_eq!(world.game_mode(), GameMode::None);
                    let admitted = world
                        .replay_pending
                        .admit_prepared_playback_owned(token)
                        .unwrap();
                    assert_eq!(admitted, expected);
                    assert_eq!(
                        world.game_mode(),
                        GameMode::None,
                        "header admission cannot start or resurrect a world"
                    );
                    assert!(!world.isInGame());
                    let lifecycle = world.replay_pending.take_lifecycle_messages_owned();
                    assert_eq!(lifecycle.len(), usize::from(expected));
                    if expected {
                        assert!(matches!(lifecycle[0].get_type(), GameMessageType::NewGame));
                    }
                    assert!(world.replay_pending.recorder_owned().is_playback());
                }
            }
        },
    );
}

#[test]
fn receiving_freeze_peek_preserves_batch_order_once_and_reset_disconnects_old_loan() {
    let mut state = ReplayPendingState::default();
    let stale_sender = state.sender.clone();
    state
        .receive_client_messages(vec![
            GameMessage::new(GameMessageType::ClearGameData),
            GameMessage::new(GameMessageType::NewGame),
        ])
        .unwrap();
    assert!(state.receiving_clear_command_pending());
    assert!(state.receiving_clear_command_pending());
    let messages = state.take_incoming();
    assert_eq!(messages.len(), 2);
    assert!(matches!(
        messages[0].get_type(),
        GameMessageType::ClearGameData
    ));
    assert!(matches!(messages[1].get_type(), GameMessageType::NewGame));
    assert!(state.take_incoming().is_empty());
    state
        .receive_client_messages(vec![GameMessage::new(GameMessageType::ClearGameData)])
        .unwrap();
    assert!(state.receiving_clear_command_pending());
    state.clear();
    assert!(!state.receiving_clear_command_pending());
    assert!(
        stale_sender
            .send(vec![GameMessage::new(GameMessageType::ClearGameData)])
            .is_err()
    );
    assert!(state.take_incoming().is_empty());
}
