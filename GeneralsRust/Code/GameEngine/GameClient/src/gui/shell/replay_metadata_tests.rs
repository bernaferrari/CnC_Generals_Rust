use super::*;

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_recorded_file_listing_does_not_publish_over_peer_local_slot_cache() {
    const NAME: &str = "actual_recorded_file_listing_does_not_publish_over_peer_local_slot_cache";
    const MARKER: &str = "GENERALS_REPLAY_METADATA_CACHE_CHILD";
    if std::env::var(MARKER).as_deref() != Ok(NAME) {
        let module = module_path!().split_once("::").unwrap().1;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("{module}::{NAME}"),
                "--test-threads=1",
                "--nocapture",
            ])
            .env(MARKER, NAME)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use game_engine::common::recorder::{Recorder, RecorderOwnerInputs};
    use game_network::game_info::{GameInfo, GameSlot as NetworkSlot, SlotState as NetworkState};
    let dir = tempfile::tempdir().unwrap();
    // This child owns the metadata cache's legacy filesystem ingress too.
    // Do not create/update the user's real Maps or MapCache.ini during testing.
    let user_data = dir.path().join("UserData");
    std::fs::create_dir_all(&user_data).unwrap();
    game_engine::common::global_data::write()
        .set_user_data_dir(user_data.to_string_lossy().into_owned());
    let policy = RecorderOwnerInputs {
        map: "Maps/Listed".into(),
        seed: 59,
        replay_directory: dir.path().join("Replays"),
        exe_crc: 2,
        ini_crc: 3,
        save_stats: false,
        stats_directory: dir.path().join("Stats"),
    };
    let mut writer = Recorder::new_owned(policy.clone());
    writer.start_recording(0, 1, 0, 30).unwrap();
    writer.stop_recording();
    let mut peer = GameInfo::new();
    peer.enter_game();
    let mut human = NetworkSlot::new();
    human.set_state(NetworkState::Player, "Peer".into(), 0x01020304);
    human.set_team_number(4);
    peer.set_slot(0, human);
    let mut allied_ai = NetworkSlot::new();
    allied_ai.set_state(NetworkState::MedAI, String::new(), 0);
    allied_ai.set_team_number(4);
    peer.set_slot(1, allied_ai);
    peer.set_local_ip(0x01020304);
    assert_eq!(peer.get_local_slot_num(), 0);
    assert!(
        !peer.is_skirmish(),
        "allied AI is not a hostile skirmish opponent"
    );
    let version = get_version();
    let mut menu = ReplayMenu::new_owned(ReplayMenuInputs {
        replay_directory: policy.replay_directory,
        replay_extension: ".rep".into(),
        last_replay_filename: "00000000".into(),
        current_version: version.get_unicode_version(),
        current_version_number: version.get_version_number(),
        current_exe_crc: 2,
        current_ini_crc: 3,
        metadata: fixture_replay_metadata_policy(1, 1),
    });
    menu.populate_replay_listbox();
    assert_eq!(
        menu.get_replay_list().len(),
        1,
        "actual GENREP file must be listed"
    );
    assert_eq!(peer.get_local_slot_num(), 0);
    assert!(
        !peer.is_skirmish(),
        "metadata listing must not replace the live peer's local-slot view"
    );
}

#[test]
fn plain_metadata_projection_retains_map_and_all_menu_slot_fields() {
    let options = "US=1;M=00Maps/Listed;MC=A;MS=12;SD=7;C=100;SR=0;SC=10000;O=N;S=HOwner,1020304,8086,TT,3,4,5,3,2:CE,1,2,3,0:CM,2,3,4,1:CH,3,4,5,2:O:X:X:X:;";
    let mut info = ReplayGameInfo::new();
    let policy = fixture_replay_metadata_policy(4, 5);
    assert!(parse_ascii_string_to_game_info_with_policy(
        &mut info, options, &policy
    ));
    assert_eq!(info.get_map(), "Maps\\Listed\\Listed.map");
    let slot = info.get_slot(0).unwrap();
    assert_eq!(slot.state, SlotState::Player);
    assert_eq!(
        (
            slot.name.as_str(),
            slot.color,
            slot.player_template,
            slot.start_pos,
            slot.team_number,
            slot.ip,
            slot.port
        ),
        ("Owner", 3, 4, 5, 3, 0x01020304, 8086)
    );
    assert_eq!(info.get_slot(1).unwrap().state, SlotState::EasyAI);
    assert_eq!(info.get_slot(2).unwrap().state, SlotState::MedAI);
    assert_eq!(info.get_slot(3).unwrap().state, SlotState::BrutalAI);
    assert_eq!(info.get_slot(4).unwrap().state, SlotState::Open);
    assert_eq!(info.get_slot(5).unwrap().state, SlotState::Closed);
    assert_eq!(info.get_slot(1).unwrap().get_name(), "FixtureEasy");
    assert_eq!(info.get_slot(4).unwrap().get_name(), "FixtureOpen");
    assert!(!parse_ascii_string_to_game_info_with_policy(
        &mut ReplayGameInfo::new(),
        "SD=7;S=X:X:X:X:X:X:X:X:",
        &policy
    ));
    assert!(!parse_ascii_string_to_game_info_with_policy(
        &mut ReplayGameInfo::new(),
        &format!("{options}UNKNOWN=1;"),
        &policy
    ));
}

#[test]
fn malformed_or_out_of_range_slot_fields_leave_menu_target_unchanged() {
    let valid = "US=1;M=00Maps/Listed;MC=A;MS=12;SD=7;C=100;SR=0;SC=10000;O=N;S=HOwner,1020304,8086,TT,3,4,5,3,2:X:X:X:X:X:X:X:;";
    let policy = fixture_replay_metadata_policy(4, 5);
    for (from, to) in [
        (",8086,", ",65536,"),
        (",8086,", ",bad,"),
        (",8086,", ",-1,"),
        (",8086,", ",4294967296,"),
        (",1020304,", ",ZZZ,"),
        (",TT,", ",TTT,"),
        (",3,4,5,3,2:", ",4,4,5,3,2:"),
        (",3,4,5,3,2:", ",3,5,5,3,2:"),
        (",3,4,5,3,2:", ",3,4,8,3,2:"),
        (",3,4,5,3,2:", ",3,4,5,4,2:"),
        (",3,4,5,3,2:", ",3,4,5,3,129:"),
        (",3,4,5,3,2:", ",bad,4,5,3,2:"),
    ] {
        let mut target = ReplayGameInfo::new();
        target.set_map("KeepMap".into());
        let mut slot = GameSlot::new();
        slot.set_name("KeepOwner".into());
        target.set_slot(0, slot);
        assert!(
            !parse_ascii_string_to_game_info_with_policy(
                &mut target,
                &valid.replace(from, to),
                &policy
            ),
            "must reject {to}"
        );
        assert_eq!(target.get_map(), "KeepMap");
        assert_eq!(target.get_slot(0).unwrap().get_name(), "KeepOwner");
    }
}

#[test]
fn cpp_loaded_counts_nat_range_map_separators_and_required_keys() {
    let options = "US=1;M=00Maps/Mixed\\Leaf;MC=A;MS=12;SD=7;C=100;SR=0;SC=10000;O=N;S=HOwner,1020304,8086,TT,39,49,5,3,128:X:X:X:X:X:X:X:;";
    let mut policy = fixture_replay_metadata_policy(40, 50);
    policy.slot_labels[1] = "LocalizedClosed".into();
    let mut target = ReplayGameInfo::new();
    assert!(parse_ascii_string_to_game_info_with_policy(
        &mut target,
        options,
        &policy
    ));
    assert_eq!(target.get_map(), "Maps\\Mixed\\Leaf\\Leaf.map");
    assert_eq!(target.get_slot(1).unwrap().get_name(), "LocalizedClosed");
    for nat in [0, 3, 127, 128] {
        assert!(parse_ascii_string_to_game_info_with_policy(
            &mut target,
            &options.replace(",128:", &format!(",{nat}:")),
            &policy
        ));
    }
    for bad in [
        options.replace(",128:", ",129:"),
        options.replace("C=100;", ""),
        format!("{options}BROKEN;"),
        format!("{options}=value;"),
        options.replace("M=00Maps", "M=你Maps"),
    ] {
        let before = target.get_map().to_string();
        assert!(!parse_ascii_string_to_game_info_with_policy(
            &mut target,
            &bad,
            &policy
        ));
        assert_eq!(target.get_map(), before);
    }
    let mut too_few = policy.clone();
    too_few.limits.color_count = 39;
    assert!(!parse_ascii_string_to_game_info_with_policy(
        &mut target,
        options,
        &too_few
    ));
    too_few = policy.clone();
    too_few.limits.player_template_count = 49;
    assert!(!parse_ascii_string_to_game_info_with_policy(
        &mut target,
        options,
        &too_few
    ));
}
