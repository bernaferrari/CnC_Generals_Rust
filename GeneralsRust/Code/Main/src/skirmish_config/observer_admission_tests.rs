//! C++ GameLogic1300–1328 retains occupied observers before adding ReplayObserver.
use super::saved_admission::SavedSkirmishAdmission;
use super::*;
use game_engine::common::system::skirmish_game_info::SkirmishGameInfo;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}
fn observer_slot(index: usize) -> SkirmishSlotConfig {
    SkirmishSlotConfig {
        slot_index: index,
        is_human: true,
        is_active: true,
        player_template: SkirmishPlayerTemplateSelection::Observer,
        faction: "Observer".into(),
        color_rgb: (10, 20, 30),
        team: -1,
        start_position: -1,
        player_name: "Recorded spectator".into(),
        ai_difficulty: None,
    }
}
fn admit_observer_color() -> i32 {
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source("MultiplayerColor OccupiedObserverChoice\n RGBColor = R:10 G:20 B:30\n RGBNightColor = R:40 G:50 B:60\nEnd\n", |ini| ini.parse_current_file()).unwrap();
    game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
        settings
            .color_definitions
            .iter()
            .position(|definition| definition.name.as_str() == "OccupiedObserverChoice")
            .unwrap() as i32
    })
}
fn original_observer() -> SkirmishGameInfo {
    let mut source = SkirmishGameInfo::default();
    // C++ GameSlot::setState resets unused colors to Random. Collision checks
    // inspect every slot, including unoccupied ones.
    for slot in &mut source.slots {
        slot.color = -1;
        slot.orig_color = -1;
    }
    source.in_game = true;
    source.in_progress = true;
    source.map_name = "OwnedObserverAdmission.map".into();
    source.seed = 47;
    source.starting_cash = 19000;
    source.slots[3].state = 5;
    source.slots[3].name = "Recorded spectator".into();
    source.slots[3].player_template = -2;
    source.slots[3].orig_player_template = -2;
    source.slots[3].color = admit_observer_color();
    source.slots[3].orig_color = -1;
    source.slots[3].start_pos = -1;
    source.slots[3].orig_start_pos = -1;
    source.slots[3].team_number = -1;
    source
}
#[test]
fn appended_observer_variant_preserves_historical_selection_discriminants() {
    assert_eq!(
        bincode_legacy::serialize(&SkirmishPlayerTemplateSelection::Random).unwrap(),
        [1, 0, 0, 0]
    );
    assert_eq!(
        bincode_legacy::serialize(&SkirmishPlayerTemplateSelection::Observer).unwrap(),
        [2, 0, 0, 0]
    );
    let exact = SkirmishPlayerTemplateSelection::Exact {
        template_name: "FactionAmerica".into(),
        template_index: 2,
    };
    assert_eq!(
        &bincode_legacy::serialize(&exact).unwrap()[..4],
        &[0, 0, 0, 0]
    );
}
#[test]
fn occupied_observer_precedes_synthetic_replay_observer_and_never_takes_a_start() {
    isolated(
        "occupied_observer_precedes_synthetic_replay_observer_and_never_takes_a_start",
        || {
            lobby_definition_tests::admit_slot_definitions();
            let mut config = golden_skirmish_config("OwnedObserverAdmission.map");
            config.slots.truncate(1);
            config.slots.push(observer_slot(3));
            assert_eq!(
                skirmish_num_teams(&config.slots),
                1,
                "occupied observer is not a gameplay team"
            );
            let mut world = GameLogic::new();
            apply_skirmish_config_for_mode(&mut world, &config, GameMode::Replay).unwrap();
            let observer = world.get_player(3).expect("original occupied player3");
            assert_eq!(observer.map_side.map_player_name, "player3");
            assert_eq!(observer.name, "Recorded spectator");
            assert!(observer.is_observer);
            assert_eq!(observer.team, Team::Neutral);
            assert_eq!(observer.start_position, -1);
            assert!(!observer.is_alive);
            assert!(!observer.is_replay_observer());
            let identity = world.player_template_identity(3).unwrap();
            assert_eq!(identity.template_name, "FactionObserver");
            assert_eq!(
                identity.template_index, None,
                "raw -2 resolves by name, not an invented store index"
            );
            let synthetic = world.replay_observer_player_id().unwrap();
            assert_ne!(synthetic, 3);
            assert!(
                synthetic > 3,
                "synthetic observer is allocated after retained original slots"
            );
            assert!(world.get_player(synthetic).unwrap().is_local);
            assert!(!world.get_player(3).unwrap().is_local);
            assert_eq!(world.host_ai_player_count(), 0);
            assert!(
                !world.will_install_multiplayer_scripts(),
                "observer must not create a second team"
            );
            let admission = world.initial_skirmish_admission.as_ref().unwrap();
            assert_eq!(
                admission
                    .config
                    .slots
                    .iter()
                    .find(|s| s.slot_index == 3)
                    .unwrap()
                    .player_template,
                SkirmishPlayerTemplateSelection::Observer
            );
            let mut receiving = GameLogic::new();
            admission.admit(&mut receiving).unwrap();
            assert!(receiving.get_player(3).unwrap().is_observer);
            assert_eq!(receiving.get_player(3).unwrap().start_position, -1);
            assert_ne!(receiving.replay_observer_player_id(), Some(3));
        },
    );
}
#[test]
fn original_minus_two_observer_survives_native_initial_capsule_and_reconstruction() {
    isolated(
        "original_minus_two_observer_survives_native_initial_capsule_and_reconstruction",
        || {
            lobby_definition_tests::admit_slot_definitions();
            let original = original_observer();
            let bytes = original.encode().unwrap();
            let decoded = SkirmishGameInfo::decode(&bytes).unwrap();
            assert_eq!(decoded, original);
            let admission = SavedSkirmishAdmission::from_original(decoded).unwrap();
            admission.validate_definitions(None).unwrap();
            let slot = admission
                .config
                .slots
                .iter()
                .find(|s| s.slot_index == 3)
                .unwrap();
            assert!(slot.is_active && slot.is_human);
            assert_eq!(
                slot.player_template,
                SkirmishPlayerTemplateSelection::Observer
            );
            assert_eq!(slot.color_rgb, (10, 20, 30));
            assert_eq!(slot.start_position, -1);
            assert_eq!(
                admission.source.as_ref().unwrap().slots[3].player_template,
                -2
            );
            assert_eq!(admission.local_slot, Some(3));
            let mut snapshot = crate::save_load::WorldSnapshot::default();
            crate::save_load::snapshot::initial_skirmish::append(
                &mut snapshot.lifecycle_tail,
                &admission,
            )
            .unwrap();
            let receiving =
                crate::save_load::snapshot::initial_skirmish::read_for_restore(&snapshot, None)
                    .unwrap()
                    .unwrap();
            assert_eq!(*receiving, admission);
            let mut world = GameLogic::new();
            receiving.admit(&mut world).unwrap();
            let player = world.get_player(3).unwrap();
            assert!(player.is_observer);
            assert_eq!(player.team, Team::Neutral);
            assert_eq!(player.map_side.map_player_name, "player3");
            assert_eq!(
                world.player_template_identity(3).unwrap().template_name,
                "FactionObserver"
            );
            assert_ne!(world.replay_observer_player_id(), Some(3));
        },
    );
}
#[test]
fn observer_original_capture_retains_raw_sentinel_and_rejects_unknown_negative_template() {
    isolated(
        "observer_original_capture_retains_raw_sentinel_and_rejects_unknown_negative_template",
        || {
            lobby_definition_tests::admit_slot_definitions();
            let original = original_observer();
            let config = SkirmishMatchConfig {
                source_game_info: Some((original.clone(), 3)),
                map: original.map_name.clone(),
                random_seed: 47,
                old_factions_only: false,
                rules: GameRulesSnapshot {
                    starting_cash: 19000,
                    ..GameRulesSnapshot::default_rules()
                },
                slots: vec![observer_slot(3)],
            };
            let resolved = resolve_skirmish_slots(&config).unwrap();
            let captured =
                SavedSkirmishAdmission::capture(&config, &config.slots, &resolved).unwrap();
            assert_eq!(captured.source, Some(original.clone()));
            assert_eq!(
                captured.config.slots[0].player_template,
                SkirmishPlayerTemplateSelection::Observer
            );
            let mut invalid = original;
            invalid.slots[3].player_template = -3;
            assert!(
                SavedSkirmishAdmission::from_original(invalid).is_err(),
                "unknown negative template must not become Observer or Random"
            );
        },
    );
}

#[test]
fn occupied_observer_random_color_survives_save_and_actual_rep_header_admission() {
    isolated(
        "occupied_observer_random_color_survives_save_and_actual_rep_header_admission",
        || {
            use game_engine::common::recorder::{Recorder, RecorderOwnerInputs};
            lobby_definition_tests::admit_slot_definitions();
            let mut original = original_observer();
            original.slots[3].color = -1;
            let config = SkirmishMatchConfig {
                source_game_info: Some((original.clone(), 3)),
                map: original.map_name.clone(),
                random_seed: original.seed as u32,
                old_factions_only: false,
                rules: GameRulesSnapshot {
                    starting_cash: 19000,
                    ..GameRulesSnapshot::default_rules()
                },
                slots: vec![observer_slot(3)],
            };
            let mut invalid = config.clone();
            invalid.source_game_info.as_mut().unwrap().0.slots[3].color = -2;
            let mut rejected_world = GameLogic::new();
            assert!(
                apply_skirmish_config(&mut rejected_world, &invalid).is_err(),
                "new occupied observer input must reject the invented color -2 sentinel"
            );
            let mut world = GameLogic::new();
            apply_skirmish_config(&mut world, &config).unwrap();
            let admission = world.initial_skirmish_admission.as_ref().unwrap();
            let source = admission.source.as_ref().unwrap();
            let selected = source.slots[3].color;
            assert!(
                selected >= 0,
                "occupied observer Random must resolve to an authored index"
            );
            assert_eq!(source.slots[3].orig_color, -1);
            assert_eq!(source.slots[3].player_template, -2);
            assert_eq!(
                original.slots[3].color, -1,
                "receiving admission must not mutate caller facts"
            );
            let expected = (
                world.get_player(3).unwrap().color_rgb,
                world.get_player(3).unwrap().color_night_rgb,
            );
            let authored =
                game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
                    let color = &settings.color_definitions[selected as usize];
                    let rgb = |packed: u32| {
                        (
                            ((packed >> 16) & 255) as u8,
                            ((packed >> 8) & 255) as u8,
                            (packed & 255) as u8,
                        )
                    };
                    (rgb(color.get_color()), rgb(color.night_color))
                });
            assert_eq!(
                expected, authored,
                "observer day/night must use the selected authored definition"
            );
            let mut snapshot = crate::save_load::WorldSnapshot::default();
            crate::save_load::snapshot::initial_skirmish::append(
                &mut snapshot.lifecycle_tail,
                admission,
            )
            .unwrap();
            let saved =
                crate::save_load::snapshot::initial_skirmish::read_for_restore(&snapshot, None)
                    .unwrap()
                    .unwrap();
            let mut restored = GameLogic::new();
            saved.admit(&mut restored).unwrap();
            assert_eq!(
                (
                    restored.get_player(3).unwrap().color_rgb,
                    restored.get_player(3).unwrap().color_night_rgb
                ),
                expected
            );
            assert_eq!(saved.source.as_ref().unwrap().slots[3].color, selected);
            assert_eq!(saved.source.as_ref().unwrap().slots[3].orig_color, -1);
            let mut historical_white = (*saved).clone();
            historical_white.source.as_mut().unwrap().slots[3].color = -2;
            assert!(
                replay_admission::recording_info(&historical_white).is_err(),
                "new original replay headers must not publish historical white color -2"
            );
            let (recorded, local) = replay_admission::recording_info(&saved).unwrap();
            assert_eq!(recorded.slots[3].color, selected);
            assert_eq!(recorded.slots[3].player_template, -2);
            let dir = tempfile::tempdir().unwrap();
            let inputs = RecorderOwnerInputs {
                map: config.map.clone(),
                seed: 47,
                replay_directory: dir.path().join("Replays"),
                exe_crc: 0x1234,
                ini_crc: 0x5678,
                save_stats: false,
                stats_directory: dir.path().join("Stats"),
            };
            let mut recorder = Recorder::new_owned(inputs.clone());
            recorder.set_recording_lobby(recorded, local);
            recorder.set_recording_seed(47);
            recorder.start_recording(1, 2, 0, 30).unwrap();
            let filename = format!(
                "{}{}",
                recorder.last_replay_filename(),
                recorder.replay_extension()
            );
            recorder.stop_recording();
            assert!(
                std::fs::read(inputs.replay_directory.join(&filename))
                    .unwrap()
                    .starts_with(b"GENREP")
            );
            let mut reader = Recorder::new_owned(inputs);
            assert!(reader.playback_file_owned(filename, false).unwrap());
            let header = reader.get_game_info();
            assert_eq!(header.slots[3].color, selected);
            assert!(header.slots[3].is_observer);
            let replay_config = replay_admission::config_from_recorded_info(header)
                .unwrap()
                .unwrap();
            assert!(
                replay_config.source_game_info.is_none(),
                "a replay header is not complete original saved GameInfo"
            );
            let replay_slot = replay_config
                .slots
                .iter()
                .find(|slot| slot.slot_index == 3)
                .unwrap();
            assert_eq!(
                replay_slot.player_template,
                SkirmishPlayerTemplateSelection::Observer
            );
            assert_eq!(replay_slot.color_rgb, expected.0);
            // Header selected identity is verified above. The historical RGB-only
            // replay config cannot yet carry an authored night-color receipt;
            // that separate ingress migration must not manufacture saved GameInfo.
        },
    );
}

#[test]
fn source_less_recording_requires_unique_observed_palette_identity() {
    isolated(
        "source_less_recording_requires_unique_observed_palette_identity",
        || {
            lobby_definition_tests::admit_slot_definitions();
            let original = original_observer();
            let selected = original.slots[3].color;
            let mut admission = SavedSkirmishAdmission::from_original(original).unwrap();
            admission.source = None;
            let before = admission.clone();
            let (recorded, _) = replay_admission::recording_info(&admission).unwrap();
            assert_eq!(recorded.slots[3].color, selected);
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source(
                "MultiplayerColor SameDayDifferentObserverNight\n RGBColor = R:10 G:20 B:30\n RGBNightColor = R:70 G:80 B:90\nEnd\n",
                |ini| ini.parse_current_file(),
            )
            .unwrap();
            let error = replay_admission::recording_info(&admission).unwrap_err();
            assert!(error.contains("ambiguous palette identity"), "{error}");
            assert_eq!(
                admission, before,
                "metadata projection must not repair its owner"
            );
        },
    );
}
