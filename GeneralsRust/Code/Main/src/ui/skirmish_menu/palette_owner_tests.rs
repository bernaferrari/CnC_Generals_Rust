use super::*;
use crate::skirmish_config::apply_skirmish_config;
use game_engine::common::recorder::{Recorder, RecorderOwnerInputs};
#[test]
fn actual_fallback_start_keeps_duplicate_rgb_indices_through_admission_save_and_rep_header() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_fallback_start_keeps_duplicate_rgb_indices_through_admission_save_and_rep_header",
        || {
            game_engine::common::ini::ensure_player_templates_loaded();
            let definitions =
                (*game_engine::common::rts::player_template::get_player_template_store()).clone();
            let first = game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|s| {
                s.get_num_colors()
            });
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source(
                "MultiplayerColor OwnedDuplicateOne
 RGBColor = R:19 G:29 B:39
 RGBNightColor = R:3 G:5 B:7
End
MultiplayerColor OwnedDuplicateTwo
 RGBColor = R:19 G:29 B:39
 RGBNightColor = R:11 G:13 B:17
End
",
                |ini| ini.parse_current_file(),
            )
            .unwrap();
            let settings = std::sync::Arc::new(
                game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(Clone::clone),
            );
            let mut menu = SkirmishMenu::new();
            menu.initialize().unwrap();
            assert!(menu.configure_slot_medium_ai(1));
            menu.slots[0].faction = Faction::USA;
            menu.slots[1].faction = Faction::GLA;
            menu.slots[0].start_position = 0;
            menu.slots[1].start_position = 1;
            menu.set_map_name("Lone Eagle");
            menu.set_random_seed(713);
            // Actual fallback slot color region is wired to the receiving palette selector.
            for (slot, selected) in [(0, first + 1), (1, first)] {
                let (left, top, width, height) = menu.get_slot_area(slot);
                let x = left + (width * 7 / 8) as i32;
                let y = top + (height / 2) as i32;
                Interactive::handle_mouse_move(&mut menu, x, y);
                let mut remaining = menu.palette.len() + 1;
                while menu.slots[slot].color.authored_index() != Some(selected) {
                    assert!(remaining > 0);
                    remaining -= 1;
                    menu.handle_mouse_click(x, y, MouseButton::Left);
                }
            }
            assert_eq!(menu.slots[0].color.to_rgb(), menu.slots[1].color.to_rgb());
            let UIEvent::StartGame {
                skirmish: Some(config),
                ..
            } = menu.simulate_start_button_click().unwrap()
            else {
                panic!("actual Start did not produce owned admission")
            };
            let (source, local) = config.source_game_info.as_ref().unwrap();
            assert!(source.in_game && source.in_progress);
            assert_eq!(*local, 0);
            assert_eq!(source.slots[0].color, first + 1);
            assert_eq!(source.slots[1].color, first);
            let mut world = crate::game_logic::GameLogic::new();
            crate::skirmish_config::lobby_definition_tests::admit_slot_world(&mut world, &definitions);
            world.multiplayer_definitions = Some(settings.clone());
            apply_skirmish_config(&mut world, &config).unwrap();
            assert_eq!(
                world.get_player(0).unwrap().color_night_rgb,
                (11, 13, 17),
                "equal day RGB must retain selected original night color"
            );
            let admission = world.initial_skirmish_admission.as_ref().unwrap();
            assert_eq!(admission.source.as_ref().unwrap().slots[0].color, first + 1);
            let snapshot = crate::save_load::SnapshotBuilder::new()
                .create_world_snapshot(&world)
                .unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = crate::save_load::decode_bincode_world_snapshot(&encoded).unwrap();
            let saved =
                crate::save_load::snapshot::initial_skirmish::read_for_restore(&decoded, None)
                    .unwrap()
                    .unwrap();
            assert_eq!(saved.source.as_ref().unwrap().slots[0].color, first + 1);
            assert_eq!(saved.source.as_ref().unwrap().slots[1].color, first);
            let mut restored = crate::game_logic::GameLogic::new();
            crate::skirmish_config::lobby_definition_tests::admit_slot_world(&mut restored, &definitions);
            restored.multiplayer_definitions = Some(settings.clone());
            saved.admit(&mut restored).unwrap();
            assert_eq!(
                restored.get_player(0).unwrap().color_night_rgb,
                (11, 13, 17),
                "save admission uses retained index, not equal day RGB"
            );
            let (recorded, local) =
                crate::skirmish_config::replay_admission::recording_info(&saved).unwrap();
            assert_eq!(recorded.slots[0].color, first + 1);
            assert_eq!(recorded.slots[1].color, first);
            let dir = tempfile::tempdir().unwrap();
            let inputs = RecorderOwnerInputs {
                map: config.map.clone(),
                seed: 713,
                replay_directory: dir.path().join("Replays"),
                exe_crc: 0x1234,
                ini_crc: 0x5678,
                save_stats: false,
                stats_directory: dir.path().join("Stats"),
            };
            let mut recorder = Recorder::new_owned(inputs.clone());
            recorder.set_recording_lobby(recorded, local);
            recorder.set_recording_seed(713);
            recorder.start_recording(1, 2, 0, 30).unwrap();
            assert!(recorder.is_recording());
            // current_replay_filename names playback, not the recording file.
            let filename = format!(
                "{}{}",
                recorder.last_replay_filename(),
                recorder.replay_extension()
            );
            recorder.stop_recording();
            let replay_path = inputs.replay_directory.join(&filename);
            assert!(
                std::fs::read(&replay_path).unwrap().starts_with(b"GENREP"),
                "actual owned recording must exist before header admission"
            );
            let mut reader = Recorder::new_owned(inputs);
            assert!(reader.playback_file_owned(filename, false).unwrap());
            assert_eq!(
                reader.get_game_info().slots[0].color,
                first + 1,
                "rep header must not infer first equal RGB index"
            );
            assert_eq!(reader.get_game_info().slots[1].color, first);
            let replay_config = crate::skirmish_config::replay_admission::config_from_recorded_info(
                reader.get_game_info(), Some(&definitions), Some(&settings),
            ).unwrap().unwrap();
            assert!(replay_config.source_game_info.is_none());
            let mut missing_settings = crate::game_logic::GameLogic::new();
            crate::skirmish_config::lobby_definition_tests::admit_slot_world(&mut missing_settings, &definitions);
            assert!(crate::skirmish_config::apply_skirmish_config_for_mode(
                &mut missing_settings, &replay_config, crate::game_logic::GameMode::Replay,
            ).is_err());
            let mut replay_world = crate::game_logic::GameLogic::new();
            crate::skirmish_config::lobby_definition_tests::admit_slot_world(&mut replay_world, &definitions);
            replay_world.multiplayer_definitions = Some(settings.clone());
            crate::skirmish_config::apply_skirmish_config_for_mode(
                &mut replay_world, &replay_config, crate::game_logic::GameMode::Replay,
            ).unwrap();
            assert_eq!(replay_world.get_player(0).unwrap().color_rgb, (19, 29, 39));
            assert_eq!(replay_world.get_player(0).unwrap().color_night_rgb, (11, 13, 17));
            assert_eq!(replay_world.get_player(1).unwrap().color_night_rgb, (3, 5, 7));
            let replay_saved = replay_world.initial_skirmish_admission.as_ref().unwrap();
            assert!(replay_saved.source.is_none());
            assert_eq!(replay_saved.config.recorded_palette.as_ref().unwrap()[0], first + 1);
            let mut another = crate::game_logic::GameLogic::new();
            crate::skirmish_config::lobby_definition_tests::admit_slot_world(&mut another, &definitions);
            another.multiplayer_definitions = Some(settings);
            replay_saved.admit(&mut another).unwrap();
            assert_eq!(another.get_player(0).unwrap().color_night_rgb, (11, 13, 17));
            assert_eq!(another.initial_skirmish_admission.as_ref().unwrap().config.recorded_palette.as_ref().unwrap()[0], first + 1);
            let mut invalid = reader.get_game_info().clone();
            invalid.slots[0].color = -2;
            assert!(crate::skirmish_config::replay_admission::config_from_recorded_info(
                &invalid, Some(&definitions), another.multiplayer_definitions.as_deref(),
            ).is_err());
            invalid.slots[0].color = another.multiplayer_definitions.as_ref().unwrap().get_num_colors();
            assert!(crate::skirmish_config::replay_admission::config_from_recorded_info(
                &invalid, Some(&definitions), another.multiplayer_definitions.as_deref(),
            ).is_err());
            invalid.slots[0].color = -1;
            let random = crate::skirmish_config::replay_admission::config_from_recorded_info(
                &invalid, Some(&definitions), another.multiplayer_definitions.as_deref(),
            ).unwrap().unwrap();
            let mut random_world = crate::game_logic::GameLogic::new();
            crate::skirmish_config::lobby_definition_tests::admit_slot_world(&mut random_world, &definitions);
            random_world.multiplayer_definitions = another.multiplayer_definitions.clone();
            crate::skirmish_config::apply_skirmish_config_for_mode(
                &mut random_world, &random, crate::game_logic::GameMode::Replay,
            ).unwrap();
            assert!(random_world.initial_skirmish_admission.as_ref().unwrap().config.recorded_palette.unwrap()[0] >= 0);
        },
    );
}

#[test]
fn default_random_and_duplicate_indices_resolve_once_on_receiving_admission() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "default_random_and_duplicate_indices_resolve_once_on_receiving_admission",
        || {
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source("MultiplayerColor RandomOwnerRed\n RGBColor = R:101 G:2 B:3\nEnd\nMultiplayerColor RandomOwnerBlue\n RGBColor = R:4 G:5 B:106\nEnd\n",|ini|ini.parse_current_file()).unwrap();
            game_engine::common::ini::ensure_player_templates_loaded();
            let definitions =
                (*game_engine::common::rts::player_template::get_player_template_store()).clone();
            let settings = std::sync::Arc::new(
                game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(Clone::clone),
            );
            // Each receiving game owns its startup slot catalog and the authored
            // Multiplayer.ini palette before admission (GameEngine.cpp init).
            let receiving_world = || {
                let mut world = crate::game_logic::GameLogic::new();
                crate::skirmish_config::lobby_definition_tests::admit_slot_world(
                    &mut world,
                    &definitions,
                );
                world.multiplayer_definitions = Some(settings.clone());
                world
            };
            let mut menu = SkirmishMenu::new();
            menu.initialize().unwrap();
            assert!(menu.configure_slot_medium_ai(1));
            menu.slots[0].faction = Faction::USA;
            menu.slots[1].faction = Faction::GLA;
            menu.slots[0].start_position = 0;
            menu.slots[1].start_position = 1;
            menu.set_map_name("Lone Eagle");
            menu.set_random_seed(901);
            assert_eq!(menu.slots[0].color.authored_index(), Some(-1));
            assert_eq!(menu.slots[1].color.authored_index(), Some(-1));
            let UIEvent::StartGame {
                skirmish: Some(config),
                ..
            } = menu.simulate_start_button_click().unwrap()
            else {
                panic!("Start")
            };
            let mut a = receiving_world();
            apply_skirmish_config(&mut a, &config).unwrap();
            let accepted = a.initial_skirmish_admission.as_ref().unwrap().clone();
            let source = accepted.source.as_ref().unwrap();
            assert!(source.slots[0].color >= 0);
            assert_ne!(source.slots[0].color, source.slots[1].color);
            assert_eq!(source.slots[0].orig_color, -1);
            assert_eq!(source.slots[1].orig_color, -1);
            let mut b = receiving_world();
            let mut other = config.clone();
            other.random_seed = 333;
            // This is another complete input owner: both its native and
            // original GameInfo seed identify that same distinct setup.
            other.source_game_info.as_mut().unwrap().0.seed = 333;
            assert_eq!(config.random_seed, 901);
            assert_eq!(config.source_game_info.as_ref().unwrap().0.seed, 901);
            apply_skirmish_config(&mut b, &other).unwrap();
            b.reset();
            drop(b);
            assert_eq!(
                a.initial_skirmish_admission.as_deref(),
                Some(accepted.as_ref())
            );
            let mut restored = receiving_world();
            accepted.admit(&mut restored).unwrap();
            assert_eq!(
                restored.initial_skirmish_admission.as_deref(),
                Some(accepted.as_ref()),
                "restore must not reroll current/original selection"
            );
            menu.cycle_slot_player_type(1);
            menu.cycle_slot_player_type(1);
            assert_eq!(menu.slots[1].player_type, PlayerType::BrutalAI);
            assert!(menu.select_slot_color(0, 0));
            assert!(menu.select_slot_color(1, 0));
            let UIEvent::StartGame {
                skirmish: Some(duplicate),
                ..
            } = menu.simulate_start_button_click().unwrap()
            else {
                panic!("Start")
            };
            apply_skirmish_config(&mut a, &duplicate).unwrap();
            let source = a
                .initial_skirmish_admission
                .as_ref()
                .unwrap()
                .source
                .as_ref()
                .unwrap();
            assert_eq!(source.slots[0].color, 0);
            assert_ne!(source.slots[1].color, 0);
            assert_eq!(source.slots[0].orig_color, 0);
            assert_eq!(source.slots[1].orig_color, 0);
            let (recorded, _) = crate::skirmish_config::replay_admission::recording_info(
                a.initial_skirmish_admission.as_ref().unwrap(),
            )
            .unwrap();
            assert_eq!(
                recorded.slots[1].ai_difficulty, 3,
                "native Brutal remains original hard replay slot"
            );
        },
    );
}
