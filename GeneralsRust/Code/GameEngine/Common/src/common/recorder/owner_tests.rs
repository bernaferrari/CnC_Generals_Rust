use super::*;

fn inputs(directory: &std::path::Path, map: &str, seed: u32) -> RecorderOwnerInputs {
    RecorderOwnerInputs {
        map: map.into(),
        seed,
        replay_directory: directory.join("Replays"),
        exe_crc: 0x1234,
        ini_crc: 0x5678,
        save_stats: false,
        stats_directory: directory.join("Stats"),
    }
}

fn camera(player: i32, x: f32) -> GameMessage {
    let position = Coord3D::new(x, 2.0, 3.0);
    let mut message = GameMessage::with_player(
        GameMessageType::SetReplayCamera(position.clone(), 0.25, 1.5),
        player,
    );
    message.append_location_argument(position);
    message.append_real_argument(0.25);
    message.append_real_argument(0.75);
    message.append_real_argument(1.5);
    message.append_integer_argument(2);
    message.append_pixel_argument(ICoord2D { x: 30, y: 40 });
    message
}

fn write_replay(owner: &RecorderOwnerInputs, player: i32, x: f32) -> String {
    let mut writer = Recorder::new_owned(owner.clone());
    writer.start_recording(1, 2, 0, 30).unwrap();
    writer.update_owned(2, &[camera(player, x)]);
    writer.update_owned(4, &[camera(player, x + 10.0)]);
    let filename = writer.filename.clone();
    writer.stop_recording();
    filename
}

fn camera_values(messages: Vec<GameMessage>) -> Vec<(i32, f32)> {
    messages
        .into_iter()
        .filter_map(|message| match message.get_type() {
            GameMessageType::SetReplayCamera(position, _, _) => {
                Some((message.get_player_index(), position.x))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn two_owned_real_replay_readers_keep_independent_frames_headers_and_outputs() {
    let directory = tempfile::tempdir().unwrap();
    let a_inputs = inputs(&directory.path().join("A"), "Maps/A.map", 7);
    let b_inputs = inputs(&directory.path().join("B"), "Maps/B.map", 19);
    let a_file = write_replay(&a_inputs, 3, 100.0);
    let b_file = write_replay(&b_inputs, 5, 900.0);
    let mut a = Recorder::new_owned(a_inputs.clone());
    let mut b = Recorder::new_owned(b_inputs.clone());
    assert!(a.playback_file_owned(a_file, false).unwrap());
    assert!(b.playback_file_owned(b_file, false).unwrap());
    assert_eq!(a.get_game_info().map, "Maps/A.map");
    assert_eq!(b.get_game_info().map, "Maps/B.map");
    assert_eq!(a.take_owner_effects().admitted_seed, Some(7));
    assert_eq!(b.take_owner_effects().admitted_seed, Some(19));
    assert!(
        matches!(a.drain_pending_commands().as_slice(), [message] if matches!(message.get_type(), GameMessageType::NewGame))
    );
    assert!(
        matches!(b.drain_pending_commands().as_slice(), [message] if matches!(message.get_type(), GameMessageType::NewGame))
    );
    a.update_owned(2, &[]);
    assert_eq!(camera_values(a.drain_pending_commands()), [(3, 100.0)]);
    b.update_owned(2, &[]);
    assert_eq!(camera_values(b.drain_pending_commands()), [(5, 900.0)]);
    // B's reset/destroy cannot close or advance A's actual open replay file.
    b.reset_owned(b_inputs);
    drop(b);
    a.update_owned(4, &[]);
    let output = a.drain_pending_commands();
    assert_eq!(camera_values(output.clone()), [(3, 110.0)]);
    assert_eq!(
        output
            .iter()
            .filter(|m| matches!(m.get_type(), GameMessageType::ClearGameData))
            .count(),
        1
    );
    assert_eq!(
        a.get_mode(),
        RecorderMode::Playback,
        "C++ EOF stop retains mode"
    );
    assert_eq!(a.current_frame, 4);
}

#[test]
fn owned_failed_header_and_stop_keep_cpp_mode_and_exact_lifecycle_effects() {
    let directory = tempfile::tempdir().unwrap();
    let owner = inputs(directory.path(), "Maps/Owned.map", 23);
    std::fs::create_dir_all(&owner.replay_directory).unwrap();
    std::fs::write(owner.replay_directory.join("bad.rep"), b"BADREP").unwrap();
    let mut recorder = Recorder::new_owned(owner.clone());
    // Header I/O cannot run until the caller has cleared the receiving world.
    let admission = recorder.prepare_playback_owned("bad.rep".into(), true);
    assert!(admission.requires_clear());
    assert_eq!(recorder.get_mode(), RecorderMode::None);
    assert!(recorder.file.is_none());
    // Model this Common-level caller's actual reset phase; Engine integration
    // must execute real GameLogic clear, not merely append a late command.
    recorder.reset_owned(owner.clone());
    assert!(!recorder.admit_prepared_playback_owned(admission).unwrap());
    assert_eq!(recorder.get_mode(), RecorderMode::Playback);
    assert!(recorder.drain_pending_commands().is_empty());
    let effects = recorder.take_owner_effects();
    assert!(effects.pending_map.is_none());
    assert!(effects.admitted_seed.is_none());
    assert!(effects.crc_interval.is_none());
    assert!(effects.controls_hidden.is_none());
    recorder.stop_playback();
    assert_eq!(recorder.get_mode(), RecorderMode::Playback);
    assert!(
        matches!(recorder.drain_pending_commands().as_slice(), [message] if matches!(message.get_type(), GameMessageType::ClearGameData))
    );
    recorder.reset_owned(owner);
    assert_eq!(recorder.get_mode(), RecorderMode::None);
    assert!(recorder.drain_pending_commands().is_empty());
}

#[test]
fn explicit_owned_operations_never_consult_installed_legacy_callbacks() {
    let directory = tempfile::tempdir().unwrap();
    let owner = inputs(directory.path(), "Maps/Owned.map", 31);
    let filename = write_replay(&owner, 2, 50.0);
    let mut recorder = Recorder::new_owned(owner);
    recorder.set_frame_provider(Some(Arc::new(|| panic!("ambient frame"))));
    recorder.set_game_mode_provider(Some(Arc::new(|| panic!("ambient mode"))));
    recorder.set_command_source(Some(Arc::new(|| panic!("ambient source"))));
    recorder.set_command_sink(Some(Arc::new(|_| panic!("ambient sink"))));
    recorder.set_command_cull(Some(Arc::new(|| panic!("ambient cull"))));
    recorder.set_lobby_info_provider(Some(Arc::new(|| panic!("ambient lobby"))));
    recorder.set_replay_control_visibility_hook(Some(Arc::new(|_| panic!("ambient UI"))));
    recorder.set_player_name_key_provider(Some(Arc::new(|_| panic!("ambient player"))));
    assert!(recorder.playback_file_owned(filename, false).unwrap());
    recorder.drain_pending_commands();
    recorder.update_owned(2, &[]);
    assert_eq!(
        camera_values(recorder.drain_pending_commands()),
        [(2, 50.0)]
    );
    recorder.handle_crc_message_with_name_key(0, false, Some("player2"));
    recorder.stop_playback();
    let inputs = recorder.owner_inputs.clone().unwrap();
    recorder.reset_owned(inputs);
    let mut new_game = GameMessage::new(GameMessageType::NewGame);
    for argument in [2, 1, 0, 30] {
        new_game.append_integer_argument(argument);
    }
    // NewGame triggers the unchanged update_record -> start_recording path.
    recorder.update_owned(0, &[new_game]);
    assert!(recorder.is_recording());
    recorder.update_owned(2, &[camera(2, 70.0)]);
    recorder.stop_recording();
}

#[test]
fn owned_recording_preserves_explicit_no_local_slot_in_real_header() {
    let directory = tempfile::tempdir().unwrap();
    let policy = inputs(directory.path(), "Maps/Observer.map", 17);
    let mut writer = Recorder::new_owned(policy.clone());
    let mut lobby = ReplayGameInfo::new();
    lobby.map = "Maps/Observer.map".into();
    lobby.slots[3].is_occupied = true;
    lobby.slots[3].is_human = true;
    lobby.slots[3].name = "Remote player".into();
    lobby.slots[3].player_template = 0;
    writer.set_recording_lobby(lobby, -1);
    writer.start_recording(1, 2, 0, 30).unwrap();
    let filename = writer.filename.clone();
    writer.stop_recording();
    let mut reader = Recorder::new_owned(policy);
    let header = reader.load_replay_header(filename.clone()).unwrap();
    assert_eq!(header.local_player_index, -1);
    assert!(reader.playback_file_owned(filename, false).unwrap());
    assert_eq!(
        reader.crc_info.as_ref().unwrap().get_local_player() as i32,
        -1,
        "explicit no-local admission must not select occupied slot zero"
    );
    assert!(reader.get_game_info().slots[3].is_occupied);
    assert_eq!(reader.get_game_info().slots[3].name, "Remote player");
}

#[test]
fn owned_file_admission_defers_controls_until_explicit_game_start_phase() {
    let directory = tempfile::tempdir().unwrap();
    let policy = inputs(directory.path(), "Maps/Controls.map", 31);
    let mut writer = Recorder::new_owned(policy.clone());
    writer.start_recording(1, 2, 0, 30).unwrap();
    assert_eq!(writer.take_owner_effects().controls_hidden, None);
    writer.init_controls();
    assert_eq!(writer.take_owner_effects().controls_hidden, Some(true));
    let filename = writer.filename.clone();
    writer.stop_recording();
    let mut reader = Recorder::new_owned(policy);
    assert!(reader.playback_file_owned(filename, false).unwrap());
    assert_eq!(
        reader.take_owner_effects().controls_hidden,
        None,
        "header admission precedes original game start controls phase"
    );
    reader.init_controls();
    assert_eq!(reader.take_owner_effects().controls_hidden, Some(false));
}

#[test]
fn controls_visibility_admission_preserves_unrelated_header_effects() {
    let directory = tempfile::tempdir().unwrap();
    let policy = inputs(directory.path(), "Maps/ControlsOwner.map", 71);
    let filename = write_replay(&policy, 2, 50.0);
    let mut reader = Recorder::new_owned(policy.clone());
    assert!(reader.playback_file_owned(filename, false).unwrap());
    assert_eq!(reader.take_controls_visibility_owned(), None);
    reader.init_controls();
    assert_eq!(reader.take_controls_visibility_owned(), Some(false));
    let effects = reader.take_owner_effects();
    assert_eq!(effects.pending_map.as_deref(), Some(policy.map.as_str()));
    assert_eq!(effects.admitted_seed, Some(policy.seed));
    assert_eq!(effects.crc_interval, Some(100));
    assert_eq!(effects.controls_hidden, None);
}

#[test]
fn explicit_header_inspection_uses_full_path_without_mutating_live_reader() {
    let directory = tempfile::tempdir().unwrap();
    let a = inputs(&directory.path().join("A"), "Maps/Live.map", 7);
    let b = inputs(&directory.path().join("B"), "Maps/Listed.map", 19);
    let a_file = write_replay(&a, 3, 100.0);
    let b_file = write_replay(&b, 5, 900.0);
    let mut live = Recorder::new_owned(a);
    assert!(live.playback_file_owned(a_file, false).unwrap());
    live.take_owner_effects();
    live.drain_pending_commands();
    let listed = inspect_replay_header(&b.replay_directory.join(&b_file)).unwrap();
    assert!(!listed.for_playback);
    assert!(listed.game_options.contains("Maps/Listed.map"));
    assert_eq!(live.get_game_info().map, "Maps/Live.map");
    assert!(live.take_owner_effects().admitted_seed.is_none());
    assert!(live.drain_pending_commands().is_empty());
    live.update_owned(2, &[]);
    assert_eq!(camera_values(live.drain_pending_commands()), [(3, 100.0)]);
    let corrupt = directory.path().join("bad.rep");
    std::fs::write(&corrupt, b"BADREP").unwrap();
    assert_eq!(
        inspect_replay_header(&corrupt).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    live.update_owned(4, &[]);
    assert_eq!(camera_values(live.drain_pending_commands()), [(3, 110.0)]);
}

#[test]
fn strict_menu_policy_does_not_change_actual_reader_admission() {
    let directory = tempfile::tempdir().unwrap();
    let policy = inputs(directory.path(), "Maps/ReaderPolicy", 7);
    let mut writer = Recorder::new_owned(policy.clone());
    let mut info = ReplayGameInfo::new();
    info.map = policy.map.clone();
    info.slots[0].name = "Owner".into();
    info.slots[0].is_human = true;
    info.slots[0].is_occupied = true;
    info.slots[0].color = 39;
    info.slots[0].player_template = 49;
    info.slots[0].port = 70000;
    info.slots[0].nat = 128;
    writer.set_recording_lobby(info, 0);
    writer.start_recording(1, 1, 0, 30).unwrap();
    let file = policy.replay_directory.join(&writer.filename);
    let filename = writer.filename.clone();
    writer.stop_recording();
    let mut reader = Recorder::new_owned(policy);
    assert!(reader.playback_file_owned(filename, false).unwrap());
    assert_eq!(reader.get_game_info().slots[0].port, 70000);
    let metadata = inspect_replay_header(&file).unwrap();
    assert!(
        decode_replay_menu_metadata(
            &metadata.game_options,
            ReplayMenuMetadataLimits {
                color_count: 40,
                player_template_count: 50
            }
        )
        .is_none(),
        "metadata-only safe narrowing rejects overflow without changing Reader policy"
    );
    assert_eq!(reader.get_game_info().slots[0].port, 70000);
    let mut corrupt = std::fs::read(&file).unwrap();
    let mask = corrupt
        .windows(4)
        .position(|bytes| bytes == b"M=00")
        .unwrap();
    corrupt.splice(mask + 2..mask + 4, "你".as_bytes().iter().copied());
    let bad_file = directory.path().join("bad-mask.rep");
    std::fs::write(&bad_file, corrupt).unwrap();
    assert_eq!(
        inspect_replay_header(&bad_file).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[test]
fn recorded_template_one_is_a_faction_and_minus_two_is_observer() {
    let directory = tempfile::tempdir().unwrap();
    let policy = inputs(directory.path(), "Maps/ObserverIdentity", 7);
    let mut writer = Recorder::new_owned(policy.clone());
    let mut info = ReplayGameInfo::new();
    info.map = policy.map.clone();
    for (slot, template) in info.slots.iter_mut().zip([1, -2]) {
        slot.is_human = true;
        slot.is_occupied = true;
        slot.player_template = template;
        slot.name = format!("Template{template}");
    }
    writer.set_recording_lobby(info, 0);
    writer.start_recording(1, 1, 0, 30).unwrap();
    let filename = writer.filename.clone();
    writer.stop_recording();
    let mut reader = Recorder::new_owned(policy);
    assert!(reader.playback_file_owned(filename, false).unwrap());
    let slots = &reader.get_game_info().slots;
    assert_eq!(slots[0].player_template, 1);
    assert!(!slots[0].is_observer, "authored faction 1 is not Observer");
    assert_eq!(slots[1].player_template, -2);
    assert!(slots[1].is_observer, "C++ Observer sentinel is -2");
}
