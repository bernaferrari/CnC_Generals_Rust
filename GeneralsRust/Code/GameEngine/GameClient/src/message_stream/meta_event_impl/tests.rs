// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_stream::player_state::get_local_player_id;
    use game_engine::common::ini::TimeOfDay as GlobalTimeOfDay;
    use gamelogic::player::Player;
    use gamelogic::system::game_logic::{get_game_logic, GAME_LAN, GAME_NONE, GAME_SINGLE_PLAYER};
    use std::sync::{Arc, RwLock};
    use std::sync::{Mutex, OnceLock};

    fn test_state_lock() -> &'static Mutex<()> {
        static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_STATE_LOCK.get_or_init(|| Mutex::new(()))
    }
    use std::fs;

    fn repo_root() -> PathBuf {
        let mut dir = std::env::current_dir().expect("current_dir");
        loop {
            if dir.join("GeneralsMD").is_dir() && dir.join("windows_game").is_dir() {
                return dir;
            }
            if !dir.pop() {
                panic!("failed to locate repository root");
            }
        }
    }

    fn active_command_map_names() -> Vec<String> {
        let root = repo_root();
        let paths = [
            root.join("windows_game/extracted_big_files_v2/INI/CommandMap.ini"),
            root.join("windows_game/extracted_big_files_v2/INI/CommandMapDebug.ini"),
            root.join("windows_game/extracted_big_files_v2/INI/CommandMapDemo.ini"),
            root.join("windows_game/extracted_big_files_v2/EnglishZH/Data/English/CommandMap.ini"),
            root.join(
                "windows_game/extracted_big_files_v2/W3DEnglishZH/Data/English/CommandMap.ini",
            ),
        ];

        let mut names = Vec::new();
        for path in paths {
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            for line in contents.lines() {
                let line = line.trim_start();
                if line.starts_with(';') {
                    continue;
                }
                let Some(rest) = line.strip_prefix("CommandMap ") else {
                    continue;
                };
                let Some(name) = rest.split_whitespace().find(|token| *token != "=") else {
                    continue;
                };
                names.push(name.to_string());
            }
        }
        names
    }

    fn alias_record(name: &str) -> MetaMapRec {
        MetaMapRec {
            name: name.to_string(),
            meta: None,
            key: 0,
            transition: Transition::Down,
            mod_state: 0,
            usable_in: COMMANDUSABLE_NONE,
            category: String::new(),
            description: String::new(),
            display_name: String::new(),
        }
    }

    #[test]
    fn test_fast_forward_replay_meta_record_is_kept() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let mut record = alias_record("TOGGLE_FAST_FORWARD_REPLAY");
        record.meta = Some(GameMessageType::MetaToggleFastForwardReplay);
        assert_eq!(
            dispatch_map_entry(&record),
            Some(GameMessageDisposition::KeepMessage)
        );
    }

    #[test]
    fn test_command_map_display_name_is_translated() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        game_engine::common::language::Language::register_localized_string(
            "GUI:Wave5MetaDisplay",
            "Wave5 Localized Command",
        );
        assert_eq!(
            translate_command_map_label("GUI:Wave5MetaDisplay"),
            "Wave5 Localized Command"
        );
        assert_eq!(
            translate_command_map_label("GUI:Wave5MetaMissing"),
            "GUI:Wave5MetaMissing"
        );
    }

    #[test]
    fn test_keyboard_options_remap_reaches_lookup() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        {
            let mut map = get_meta_map().write().unwrap_or_else(|e| e.into_inner());
            map.add_record(MetaMapRec {
                name: "SELECT_ALL".to_string(),
                meta: Some(GameMessageType::MetaSelectAll),
                key: 0x51,
                transition: Transition::Down,
                mod_state: 0,
                usable_in: COMMANDUSABLE_GAME,
                category: "SELECTION".to_string(),
                description: "GUI:SelectAllDesc".to_string(),
                display_name: "GUI:SelectAll".to_string(),
            });
        }
        assert_eq!(
            lookup_command_map_name(0x51, 0).as_deref(),
            Some("SELECT_ALL")
        );
        assert!(command_map_binds("SELECT_ALL"));
        assert!(update_command_map_entry("SELECTION", "GUI:SelectAll", 0x4B, 0));
        assert_eq!(
            lookup_command_map_name(0x4B, 0).as_deref(),
            Some("SELECT_ALL")
        );
        assert_eq!(lookup_command_map_name(0x51, 0), None);
        reset_command_map_entries();
    }

    #[test]
    fn test_apply_toggle_lower_details_is_live_callable() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        // Without GlobalData the live helper fail-closes; with it, it toggles.
        let _ = apply_toggle_lower_details();
    }

    #[test]
    fn test_lookup_meta_message_type_uses_cpp_attack_move_spelling() {
        assert_eq!(lookup_meta_message_type("TOGGLE_ATTACKMOVE"), None);
        assert_eq!(lookup_meta_message_type("TOGGLE_ATTACK_MOVE"), None);
        assert!(!is_supported_command_map_name("TOGGLE_ATTACKMOVE"));
        assert!(is_supported_command_map_name("PLACE_BEACON"));
        assert!(is_supported_command_map_name("DELETE_BEACON"));
        assert!(is_supported_command_map_name("TOGGLE_LOWER_DETAILS"));
        assert!(is_supported_command_map_name("DEMO_TOGGLE_SOUND"));
        assert!(is_supported_command_map_name("CHEAT_ADD_CASH"));
        assert!(is_supported_command_map_name("DEBUG_OBJECT_ID_PERFORMANCE"));
        assert!(is_supported_command_map_name("HELP"));
        assert!(!is_supported_command_map_name("DEMO_NOT_A_REAL_COMMAND"));
        assert!(!is_supported_command_map_name("CHEAT_NOT_A_REAL_COMMAND"));
        assert!(!is_supported_command_map_name("DEBUG_NOT_A_REAL_COMMAND"));
        assert!(!is_supported_command_map_name("UNKNOWN_WIDGET"));
    }

    #[test]
    fn test_lookup_key_code_covers_cpp_keypad_entries() {
        assert_eq!(lookup_key_code("KEY_KP0"), Some(0x60));
        assert_eq!(lookup_key_code("KEY_KP9"), Some(0x69));
        assert_eq!(lookup_key_code("KEY_KPDEL"), Some(0x6E));
        assert_eq!(lookup_key_code("KEY_KPSTAR"), Some(0x6A));
        assert_eq!(lookup_key_code("KEY_KPMINUS"), Some(0x6D));
        assert_eq!(lookup_key_code("KEY_KPPLUS"), Some(0x6B));
        assert_eq!(lookup_key_code("KEY_KPSLASH"), Some(0x6F));
        assert_eq!(lookup_key_code("KEY_KPENTER"), Some(0x0D));
        assert_eq!(lookup_key_code("KEY_NONE"), Some(0));
    }

    #[test]
    fn test_discovered_command_map_names_are_either_mapped_or_intentionally_unresolved() {
        let names = active_command_map_names();
        assert!(!names.is_empty());

        for name in names {
            assert!(
                is_supported_command_map_name(&name),
                "unhandled CommandMap entry: {name}"
            );
        }
    }

    #[test]
    fn test_alias_command_map_entries_use_runtime_dispatch_paths() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        assert_eq!(
            dispatch_map_entry(&alias_record("PLACE_BEACON")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DELETE_BEACON")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("TOGGLE_LOWER_DETAILS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_unimplemented_cpp_command_entries_are_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        assert!(!is_unimplemented_cpp_command_name("DEMO_CYCLE_EXTENT_TYPE"));
    }

    #[test]
    fn test_dispatch_handled_cpp_command_entries_are_supported_and_not_unimplemented() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        for alias in [
            "CHEAT_ADD_CASH",
            "CHEAT_DESHROUD",
            "CHEAT_RUNSCRIPT3",
            "CHEAT_TOGGLE_HAND_OF_GOD_MODE",
            "DEBUG_DUMP_ALL_PLAYER_OBJECTS",
            "DEBUG_DUMP_PLAYER_OBJECTS",
            "DEBUG_DRAWABLE_ID_PERFORMANCE",
            "DEBUG_OBJECT_ID_PERFORMANCE",
            "DEBUG_SLEEPY_UPDATE_PERFORMANCE",
            "DEMO_CYCLE_EXTENT_TYPE",
            "DEMO_BEGIN_ADJUST_FOV",
            "DEMO_BEGIN_ADJUST_PITCH",
            "DEMO_CYCLE_LOD_LEVEL",
            "DEMO_DEBUG_SELECTION",
            "DEMO_DECR_ANIM_SKATE_SPEED",
            "DEMO_DECR_EXTENT_HEIGHT",
            "DEMO_DECR_EXTENT_HEIGHT_LARGE",
            "DEMO_DECR_EXTENT_MAJOR",
            "DEMO_DECR_EXTENT_MAJOR_LARGE",
            "DEMO_DECR_EXTENT_MINOR",
            "DEMO_DECR_EXTENT_MINOR_LARGE",
            "DEMO_DESHROUD",
            "DEMO_DUMP_ASSETS",
            "DEMO_END_ADJUST_FOV",
            "DEMO_END_ADJUST_PITCH",
            "DEMO_INCR_EXTENT_HEIGHT",
            "DEMO_INCR_EXTENT_HEIGHT_LARGE",
            "DEMO_INCR_EXTENT_MAJOR",
            "DEMO_INCR_EXTENT_MAJOR_LARGE",
            "DEMO_INCR_EXTENT_MINOR",
            "DEMO_INCR_EXTENT_MINOR_LARGE",
            "DEMO_INCR_ANIM_SKATE_SPEED",
            "DEMO_KILL_ALL_ENEMIES",
            "DEMO_LOCK_CAMERA_TO_PLANES",
            "DEMO_LOD_DECREASE",
            "DEMO_LOD_INCREASE",
            "DEMO_MUSIC_NEXT_TRACK",
            "DEMO_PLAY_CAMEO_MOVIE",
            "DEMO_PLAY_OBJECTIVE_MOVIE2",
            "DEMO_TEST_SURRENDER",
            "DEMO_TOGGLE_AUDIODEBUG",
            "DEMO_TOGGLE_AVI",
            "DEMO_TOGGLE_BW_VIEW",
            "DEMO_TOGGLE_DEBUG_STATS",
            "DEMO_TOGGLE_GREEN_VIEW",
            "DEMO_TOGGLE_HAND_OF_GOD_MODE",
            "DEMO_TOGGLE_HURT_ME_MODE",
            "DEMO_TOGGLE_LETTERBOX",
            "DEMO_TOGGLE_MOTION_BLUR_ZOOM",
            "DEMO_TOGGLE_NETWORK",
            "DEMO_TOGGLE_PARTICLEDEBUG",
            "DEMO_TOGGLE_RED_VIEW",
            "DEMO_ENSHROUD",
            "DEMO_VTUNE_OFF",
            "DEMO_VTUNE_ON",
            "HELP",
            "DEMO_WIN",
        ] {
            assert!(is_dispatch_handled_cpp_command_name(alias));
            assert!(!is_unimplemented_cpp_command_name(alias));
            assert!(is_supported_command_map_name(alias));
        }
    }

    #[test]
    fn test_demo_adjust_aliases_toggle_camera_adjust_state() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        reset_demo_camera_adjust_state_for_tests();
        apply_demo_camera_adjust_from_mouse_position(&ICoord2D::new(50, 60));

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_BEGIN_ADJUST_FOV")),
            None
        );
        let state = demo_camera_adjust_state_for_tests();
        assert!(!state.is_pitching);
        assert!(state.is_changing_fov);
        assert_eq!(state.anchor, ICoord2D::new(50, 60));

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_BEGIN_ADJUST_PITCH")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        let state = demo_camera_adjust_state_for_tests();
        assert!(state.is_pitching);
        assert!(state.is_changing_fov);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_END_ADJUST_FOV")),
            None
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_END_ADJUST_PITCH")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        let state = demo_camera_adjust_state_for_tests();
        assert!(!state.is_pitching);
        assert!(!state.is_changing_fov);
    }

    #[test]
    fn test_raw_mouse_position_applies_demo_pitch_and_fov_adjustments() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        reset_demo_camera_adjust_state_for_tests();
        let mut translator = MetaEventTranslator::default();
        with_tactical_view(|view| {
            view.set_pitch(0.0);
            view.set_field_of_view(1.0);
        });

        let _ = translator.translate_game_message(&GameMessage::new(
            GameMessageType::RawMousePosition(ICoord2D::new(100, 100)),
        ));
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_BEGIN_ADJUST_PITCH")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_BEGIN_ADJUST_FOV")),
            None
        );

        let _ = translator.translate_game_message(&GameMessage::new(
            GameMessageType::RawMousePosition(ICoord2D::new(100, 110)),
        ));
        let (pitch_after, fov_after) = crate::display::view::with_tactical_view_ref(|view| {
            (view.pitch(), view.field_of_view())
        });
        assert!((pitch_after - 0.1).abs() < 0.001);
        assert!((fov_after - 1.1).abs() < 0.001);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_END_ADJUST_PITCH")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_END_ADJUST_FOV")),
            None
        );

        let _ = translator.translate_game_message(&GameMessage::new(
            GameMessageType::RawMousePosition(ICoord2D::new(100, 140)),
        ));
        let (pitch_final, fov_final) = crate::display::view::with_tactical_view_ref(|view| {
            (view.pitch(), view.field_of_view())
        });
        assert!((pitch_final - pitch_after).abs() < 0.001);
        assert!((fov_final - fov_after).abs() < 0.001);
    }

    #[test]
    fn test_extent_adjust_aliases_are_consumed_and_adjust_geometry_values() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        for alias in [
            "DEMO_CYCLE_EXTENT_TYPE",
            "DEMO_INCR_EXTENT_MAJOR",
            "DEMO_DECR_EXTENT_MAJOR",
            "DEMO_INCR_EXTENT_MAJOR_LARGE",
            "DEMO_DECR_EXTENT_MAJOR_LARGE",
            "DEMO_INCR_EXTENT_MINOR",
            "DEMO_DECR_EXTENT_MINOR",
            "DEMO_INCR_EXTENT_MINOR_LARGE",
            "DEMO_DECR_EXTENT_MINOR_LARGE",
            "DEMO_INCR_EXTENT_HEIGHT",
            "DEMO_DECR_EXTENT_HEIGHT",
            "DEMO_INCR_EXTENT_HEIGHT_LARGE",
            "DEMO_DECR_EXTENT_HEIGHT_LARGE",
        ] {
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                Some(GameMessageDisposition::DestroyMessage),
                "alias {alias} should be consumed"
            );
        }

        let mut major = GeometryInfo::default();
        major.set_geometry_type(game_engine::system::geometry::GeometryType::Box);
        major.bounds.min.x = -5.0;
        major.bounds.max.x = 5.0;
        major.bounds.min.y = -3.0;
        major.bounds.max.y = 3.0;
        major.bounds.min.z = 0.0;
        major.bounds.max.z = 4.0;
        apply_extent_adjust(
            &mut major,
            parse_extent_adjust_alias("DEMO_CYCLE_EXTENT_TYPE").expect("extent alias"),
        );
        assert_eq!(
            major.get_geometry_type(),
            game_engine::system::geometry::GeometryType::Sphere
        );
        apply_extent_adjust(
            &mut major,
            parse_extent_adjust_alias("DEMO_INCR_EXTENT_MAJOR_LARGE").expect("extent alias"),
        );
        assert!((major.get_major_radius() - 15.0).abs() < 0.001);

        let mut minor = GeometryInfo::default();
        minor.bounds.min.x = -5.0;
        minor.bounds.max.x = 5.0;
        minor.bounds.min.y = -3.0;
        minor.bounds.max.y = 3.0;
        apply_extent_adjust(
            &mut minor,
            parse_extent_adjust_alias("DEMO_DECR_EXTENT_MINOR").expect("extent alias"),
        );
        assert!((minor.get_minor_radius() - 2.0).abs() < 0.001);

        let mut height = GeometryInfo::default();
        height.bounds.min.z = 0.0;
        height.bounds.max.z = 4.0;
        apply_extent_adjust(
            &mut height,
            parse_extent_adjust_alias("DEMO_DECR_EXTENT_HEIGHT_LARGE").expect("extent alias"),
        );
        assert!((height.get_max_height_above_position() + 6.0).abs() < 0.001);
    }

    #[test]
    fn test_demo_toggle_no_draw_sets_cpp_equivalent_runtime_value() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        global_data.write().no_draw = 0;

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_NO_DRAW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(global_data.read().no_draw, u32::MAX);
    }

    #[test]
    fn test_demo_lod_aliases_adjust_texture_reduction_factor_with_cpp_clamp() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        global_data.write().texture_reduction_factor = 0;

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_LOD_DECREASE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(global_data.read().texture_reduction_factor, 0);

        for _ in 0..6 {
            assert_eq!(
                dispatch_map_entry(&alias_record("DEMO_LOD_INCREASE")),
                Some(GameMessageDisposition::DestroyMessage)
            );
        }
        assert_eq!(global_data.read().texture_reduction_factor, 4);

        for _ in 0..6 {
            assert_eq!(
                dispatch_map_entry(&alias_record("DEMO_LOD_DECREASE")),
                Some(GameMessageDisposition::DestroyMessage)
            );
        }
        assert_eq!(global_data.read().texture_reduction_factor, 0);
    }

    #[test]
    fn test_deshroud_aliases_follow_cpp_keep_message_semantics() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(dispatch_map_entry(&alias_record("CHEAT_DESHROUD")), None);
        assert_eq!(dispatch_map_entry(&alias_record("DEMO_DESHROUD")), None);
        assert_eq!(dispatch_map_entry(&alias_record("DEMO_ENSHROUD")), None);
    }

    #[test]
    fn test_help_alias_is_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            dispatch_map_entry(&alias_record("HELP")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_demo_vtune_aliases_toggle_compat_state() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        set_vtune_enabled(false);
        assert!(!is_vtune_enabled_for_tests());

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_VTUNE_ON")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(is_vtune_enabled_for_tests());

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_VTUNE_OFF")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(!is_vtune_enabled_for_tests());
    }

    #[test]
    fn test_demo_skate_speed_aliases_keep_message_and_adjust_value() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        set_skate_distance_override_for_tests(0.0);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_INCR_ANIM_SKATE_SPEED")),
            None
        );
        assert!((adjust_skate_distance_override(0.0) - 0.25).abs() < f32::EPSILON);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_DECR_ANIM_SKATE_SPEED")),
            None
        );
        assert!((adjust_skate_distance_override(0.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_selection_debug_toggle_aliases_flip_compat_state() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        set_bool_state_for_tests(hand_of_god_mode_state(), false);
        set_bool_state_for_tests(hurt_me_mode_state(), false);
        set_bool_state_for_tests(debug_selection_mode_state(), false);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_HAND_OF_GOD_MODE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(bool_state_for_tests(hand_of_god_mode_state()));

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_TOGGLE_HAND_OF_GOD_MODE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(!bool_state_for_tests(hand_of_god_mode_state()));

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_HURT_ME_MODE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(bool_state_for_tests(hurt_me_mode_state()));

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_DEBUG_SELECTION")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(bool_state_for_tests(debug_selection_mode_state()));
    }

    #[test]
    fn test_demo_dump_assets_alias_is_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        let output_path = PathBuf::from("UsedMapAssets.txt");
        let _ = fs::remove_file(&output_path);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_DUMP_ASSETS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(output_path.exists());
        let _ = fs::remove_file(&output_path);
    }

    #[test]
    fn test_demo_toggle_aliases_apply_cpp_global_data_side_effects() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        {
            let mut global = global_data.write();
            global.use_shadow_volumes = true;
            global.use_shadow_decals = true;
            global.fog_of_war_on = true;
            global.make_track_marks = true;
            global.use_water_plane = true;
            global.disable_render = false;
            global.debug_supply_center_placement = false;
            global.debug_camera = false;
            global.debug_visibility = false;
            global.debug_projectile_path = false;
            global.debug_threat_map = false;
            global.debug_cash_value_map = true;
            global.debug_show_graphical_framerate = false;
            global.show_collision_extents = false;
            global.show_audio_locations = false;
            global.show_object_health = false;
            global.show_metrics = false;
            global.special_power_uses_delay = true;
            global.feather_water = 0;
            global.debug_ai.value = 0;
        }
        TheGameLogic::set_show_behind_building_markers(false);

        let aliases = [
            "DEMO_TOGGLE_SHADOW_VOLUMES",
            "DEMO_TOGGLE_FOGOFWAR",
            "DEMO_TOGGLE_TRACKMARKS",
            "DEMO_TOGGLE_WATERPLANE",
            "DEMO_TOGGLE_RENDER",
            "DEMO_TOGGLE_BEHIND_BUILDINGS",
            "DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT",
            "DEMO_TOGGLE_CAMERA_DEBUG",
            "DEMO_TOGGLE_VISIONDEBUG",
            "DEMO_TOGGLE_PROJECTILEDEBUG",
            "DEMO_TOGGLE_THREATDEBUG",
            "DEMO_TOGGLE_GRAPHICALFRAMERATEBAR",
            "DEMO_SHOW_EXTENTS",
            "DEMO_SHOW_AUDIO_LOCATIONS",
            "DEMO_SHOW_HEALTH",
            "DEMO_TOGGLE_METRICS",
            "DEMO_TOGGLE_SPECIAL_POWER_DELAYS",
            "DEMO_TOGGLE_FEATHER_WATER",
            "DEMO_TOGGLE_CASHMAPDEBUG",
            "DEMO_TOGGLE_AI_DEBUG",
            "CHEAT_SHOW_HEALTH",
            "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS",
        ];
        for alias in aliases {
            let expected = match alias {
                "DEMO_TOGGLE_RENDER"
                | "DEMO_SHOW_EXTENTS"
                | "DEMO_SHOW_AUDIO_LOCATIONS"
                | "DEMO_SHOW_HEALTH"
                | "DEMO_TOGGLE_METRICS"
                | "CHEAT_SHOW_HEALTH" => None,
                _ => Some(GameMessageDisposition::DestroyMessage),
            };
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                expected,
                "alias {alias} should be consumed"
            );
        }

        let global = global_data.read();
        assert!(!global.use_shadow_volumes);
        assert!(!global.use_shadow_decals);
        assert!(!global.fog_of_war_on);
        assert!(!global.make_track_marks);
        assert!(!global.use_water_plane);
        assert!(global.disable_render);
        assert!(global.debug_supply_center_placement);
        assert!(global.debug_camera);
        assert!(global.debug_visibility);
        assert!(global.debug_projectile_path);
        assert!(!global.debug_threat_map);
        assert!(global.debug_cash_value_map);
        assert!(global.debug_show_graphical_framerate);
        assert!(global.show_collision_extents);
        assert!(global.show_audio_locations);
        assert!(!global.show_object_health);
        assert!(global.show_metrics);
        assert!(global.special_power_uses_delay);
        assert_eq!(global.feather_water, 5);
        assert_eq!(global.debug_ai.value, 1);
        assert!(TheGameLogic::get_show_behind_building_markers());
    }

    #[test]
    fn test_demo_cash_and_science_point_aliases_apply_local_player_effects() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut local_guard = local_player.write().unwrap_or_else(|e| e.into_inner());
            local_guard.get_money_mut().set_money(0);
            let spp = local_guard.get_science_purchase_points();
            if spp != 0 {
                local_guard.add_science_purchase_points(-spp);
            }
        }

        {
            let mut list = ThePlayerList().write().unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }

        assert_eq!(dispatch_map_entry(&alias_record("DEMO_ADDCASH")), None);
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_SCIENCEPURCHASEPOINTS")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        {
            let local_guard = local_player.read().unwrap_or_else(|e| e.into_inner());
            assert_eq!(local_guard.get_money().get_money(), 10_000);
            assert_eq!(local_guard.get_science_purchase_points(), 1);
        }

        assert_eq!(dispatch_map_entry(&alias_record("CHEAT_ADD_CASH")), None);
        assert_eq!(
            local_player
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get_money()
                .get_money(),
            20_000
        );

        ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn test_demo_build_mode_aliases_toggle_local_player_debug_flags() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut list = ThePlayerList().write().unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_INSTANT_BUILD")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_FREE_BUILD")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_REMOVE_PREREQ")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        {
            let local_guard = local_player.read().unwrap_or_else(|e| e.into_inner());
            assert!(local_guard.builds_instantly());
            assert!(local_guard.builds_for_free());
            assert!(local_guard.ignores_prereqs());
        }

        ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn test_demo_rank_level_aliases_adjust_local_player_rank() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut local_guard = local_player.write().unwrap_or_else(|e| e.into_inner());
            let _ = local_guard.set_rank_level(1);
        }

        {
            let mut list = ThePlayerList().write().unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_RANKLEVEL")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_RANKLEVEL")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TAKE_RANKLEVEL")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        {
            let local_guard = local_player.read().unwrap_or_else(|e| e.into_inner());
            assert_eq!(local_guard.get_rank_level(), 2);
        }

        ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn test_message_text_aliases_toggle_ingame_ui_message_state() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        if !TheInGameUI::is_messages_on() {
            TheInGameUI::toggle_messages();
        }
        assert!(TheInGameUI::is_messages_on());

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MESSAGE_TEXT")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(!TheInGameUI::is_messages_on());

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_TOGGLE_MESSAGE_TEXT")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(TheInGameUI::is_messages_on());
    }

    #[test]
    fn test_demo_zoom_lock_alias_toggles_view_zoom_limit() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        with_tactical_view(|view| view.set_zoom_limited(false));
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_ZOOM_LOCK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(crate::display::view::with_tactical_view_ref(
            |view| view.is_zoom_limited()
        ));

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_ZOOM_LOCK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(!crate::display::view::with_tactical_view_ref(
            |view| view.is_zoom_limited()
        ));
    }

    #[test]
    fn test_demo_objective_movie_aliases_update_index_when_in_game() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_SINGLE_PLAYER);
        }
        if let Ok(mut index) = get_objective_movie_index().write() {
            *index = 1;
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_PLAY_OBJECTIVE_MOVIE4")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            *get_objective_movie_index()
                .read()
                .unwrap_or_else(|e| e.into_inner()),
            4
        );

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_NEXT_OBJECTIVE_MOVIE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            *get_objective_movie_index()
                .read()
                .unwrap_or_else(|e| e.into_inner()),
            5
        );

        if let Ok(mut index) = get_objective_movie_index().write() {
            *index = 6;
        }
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_NEXT_OBJECTIVE_MOVIE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            *get_objective_movie_index()
                .read()
                .unwrap_or_else(|e| e.into_inner()),
            1
        );

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
    }

    #[test]
    fn test_demo_military_subtitles_and_time_of_day_aliases_are_wired() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();

        {
            let mut global = global_data.write();
            global.time_of_day = GlobalTimeOfDay::Afternoon;
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MILITARY_SUBTITLES")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TIME_OF_DAY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(global_data.read().time_of_day, GlobalTimeOfDay::Evening);
    }

    #[test]
    fn test_demo_play_cameo_movie_alias_is_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_SINGLE_PLAYER);
        }
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_PLAY_CAMEO_MOVIE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
    }

    #[test]
    fn test_switch_team_aliases_cycle_or_swap_local_player_index() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let player_usa = Arc::new(RwLock::new(Player::new(0)));
        let player_china = Arc::new(RwLock::new(Player::new(1)));
        let player_neutral = Arc::new(RwLock::new(Player::new(2)));
        {
            let mut usa = player_usa.write().unwrap_or_else(|e| e.into_inner());
            usa.set_side("America");
            usa.set_player_type(PlayerType::Human, false);
        }
        {
            let mut china = player_china.write().unwrap_or_else(|e| e.into_inner());
            china.set_side("China");
            china.set_player_type(PlayerType::Human, false);
        }
        {
            let mut neutral = player_neutral.write().unwrap_or_else(|e| e.into_inner());
            neutral.set_side("Neutral");
            neutral.set_player_type(PlayerType::Neutral, false);
        }

        {
            let mut list = ThePlayerList().write().unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(Arc::clone(&player_usa));
            list.add_player(Arc::clone(&player_china));
            list.add_player(Arc::clone(&player_neutral));
            list.set_local_player_index(0);
        }
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_SINGLE_PLAYER);
        }
        set_local_player_id(0);

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_SWITCH_TEAMS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            ThePlayerList()
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get_local_player_index(),
            1
        );
        assert_eq!(get_local_player_id(), 1);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_SWITCH_TEAMS_CHINA_USA")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            ThePlayerList()
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get_local_player_index(),
            0
        );
        assert_eq!(get_local_player_id(), 0);

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
        ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn test_cheat_switch_teams_keeps_message_in_multiplayer() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let player_usa = Arc::new(RwLock::new(Player::new(0)));
        let player_china = Arc::new(RwLock::new(Player::new(1)));
        {
            let mut usa = player_usa.write().unwrap_or_else(|e| e.into_inner());
            usa.set_side("America");
            usa.set_player_type(PlayerType::Human, false);
        }
        {
            let mut china = player_china.write().unwrap_or_else(|e| e.into_inner());
            china.set_side("China");
            china.set_player_type(PlayerType::Human, false);
        }

        {
            let mut list = ThePlayerList().write().unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(Arc::clone(&player_usa));
            list.add_player(Arc::clone(&player_china));
            list.set_local_player_index(0);
        }
        set_local_player_id(0);
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_LAN);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_SWITCH_TEAMS")),
            None
        );
        assert_eq!(
            ThePlayerList()
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get_local_player_index(),
            0
        );
        assert_eq!(get_local_player_id(), 0);

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
        ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn test_multiplayer_gated_cheat_aliases_keep_message() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut list = ThePlayerList().write().unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }
        set_local_player_id(0);
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_LAN);
        }

        let aliases = [
            "CHEAT_ADD_CASH",
            "CHEAT_GIVE_ALL_SCIENCES",
            "CHEAT_GIVE_SCIENCEPURCHASEPOINTS",
            "CHEAT_INSTANT_BUILD",
            "CHEAT_KILL_SELECTION",
            "CHEAT_RUNSCRIPT3",
            "CHEAT_SHOW_HEALTH",
            "CHEAT_TOGGLE_MESSAGE_TEXT",
            "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS",
        ];
        for alias in aliases {
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                None,
                "{alias} should keep message in multiplayer"
            );
        }

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
        ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn test_demo_toggle_sound_and_music_aliases_update_audio_flags() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        {
            let mut audio = manager.lock().unwrap_or_else(|e| e.into_inner());
            audio.set_on(true, AudioAffect::All);
            audio.set_on(true, AudioAffect::Music);
            audio.add_track_name("meta_test_track_1".to_string());
            audio.add_track_name("meta_test_track_2".to_string());
            audio.set_music_track_name("meta_test_track_1".to_string());
        }
        {
            let script_engine = get_script_engine();
            if let Ok(mut guard) = script_engine.write() {
                *guard = None;
            };
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_SOUND")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().unwrap_or_else(|e| e.into_inner());
            assert!(!audio.is_on(AudioAffect::Sound));
            assert!(!audio.is_on(AudioAffect::Music));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_SOUND")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().unwrap_or_else(|e| e.into_inner());
            assert!(audio.is_on(AudioAffect::Sound));
            assert!(audio.is_on(AudioAffect::Music));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MUSIC")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().unwrap_or_else(|e| e.into_inner());
            assert!(!audio.is_on(AudioAffect::Music));
            assert!(audio.is_on(AudioAffect::Sound));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MUSIC")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().unwrap_or_else(|e| e.into_inner());
            assert!(audio.is_on(AudioAffect::Music));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_MUSIC_NEXT_TRACK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(audio.get_music_track_name(), "meta_test_track_2");
        }
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_MUSIC_PREV_TRACK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(audio.get_music_track_name(), "meta_test_track_1");
        }
    }

    #[test]
    fn test_demo_debug_display_and_movie_capture_aliases_are_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        for alias in [
            "DEMO_TOGGLE_DEBUG_STATS",
            "DEMO_TOGGLE_PARTICLEDEBUG",
            "DEMO_TOGGLE_AUDIODEBUG",
            "DEMO_TOGGLE_AVI",
        ] {
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                Some(GameMessageDisposition::DestroyMessage),
                "alias {alias} should be consumed"
            );
        }
    }

    #[test]
    fn test_demo_view_filter_aliases_toggle_expected_filter_modes() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        with_tactical_view(|view| {
            view.set_view_filter_mode(FilterMode::Null);
            view.set_view_filter(FilterType::Null);
            view.set_fade_parameters(0, -1);
            view.set_camera_lock(None);
            view.set_position(&crate::display::view::Point3::new(0.0, 0.0, 0.0));
        });
        if let Ok(mut saturate) = get_motion_blur_zoom_saturate_state().write() {
            *saturate = false;
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_RED_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::BlackAndWhite);
            assert_eq!(view.get_view_filter_mode(), FilterMode::BWRedAndWhite);
        });
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_RED_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::Null);
            assert_eq!(view.get_view_filter_mode(), FilterMode::Null);
        });

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_GREEN_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::BlackAndWhite);
            assert_eq!(view.get_view_filter_mode(), FilterMode::BWGreenAndWhite);
        });

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MOTION_BLUR_ZOOM")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::MotionBlur);
            assert_eq!(view.get_view_filter_mode(), FilterMode::MBInAndOutAlpha);
        });
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MOTION_BLUR_ZOOM")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::Null);
            assert_eq!(view.get_view_filter_mode(), FilterMode::Null);
        });
    }

    #[test]
    fn test_demo_toggle_bw_view_alias_cycles_cpp_compat_state() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        reset_bw_view_state_for_tests();
        with_tactical_view(|view| {
            view.set_view_filter_mode(FilterMode::Null);
            view.set_view_filter(FilterType::Null);
            view.set_fade_parameters(0, -1);
        });

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_BW_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(bw_view_mode_for_tests(), 1);
        let (wireframe_active, wireframe_pending) = bw_view_wireframe_for_tests();
        assert!(!wireframe_active);
        assert!(wireframe_pending);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_BW_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(bw_view_mode_for_tests(), 2);
        let (_, wireframe_pending) = bw_view_wireframe_for_tests();
        assert!(!wireframe_pending);
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::Crossfade);
            assert_eq!(view.get_view_filter_mode(), FilterMode::CrossfadeFbMask);
        });

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_BW_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(bw_view_mode_for_tests(), 0);
    }

    #[test]
    fn test_demo_toggle_network_alias_is_consumed_and_toggles_compat_state() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_NETWORK")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        #[cfg(not(feature = "network"))]
        {
            if let Some(network) = game_network::get_network() {
                let current = network.is_network_on();
                assert_eq!(
                    dispatch_map_entry(&alias_record("DEMO_TOGGLE_NETWORK")),
                    Some(GameMessageDisposition::DestroyMessage)
                );
                assert_eq!(network.is_network_on(), !current);
            }
        }
    }

    #[test]
    fn test_demo_cycle_lod_level_alias_matches_cpp_decrement_wrap_order() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        set_cycle_lod_level_state_for_tests(DynamicGameLODLevel::VeryHigh);

        for expected in ["High", "Medium", "Low", "VeryHigh"] {
            assert_eq!(
                dispatch_map_entry(&alias_record("DEMO_CYCLE_LOD_LEVEL")),
                Some(GameMessageDisposition::DestroyMessage)
            );
            assert_eq!(game_engine::common::game_lod::get_dynamic_lod(), expected);
        }
    }

    #[test]
    fn test_debug_dump_player_object_aliases_are_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_DUMP_PLAYER_OBJECTS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_DUMP_ALL_PLAYER_OBJECTS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_debug_sleepy_update_performance_alias_keeps_message() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_SLEEPY_UPDATE_PERFORMANCE")),
            None
        );
    }

    #[test]
    fn test_debug_id_performance_aliases_keep_message() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_OBJECT_ID_PERFORMANCE")),
            None
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_DRAWABLE_ID_PERFORMANCE")),
            None
        );
    }

    #[test]
    fn test_demo_perform_statistical_dump_sets_dump_flag() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        global_data.write().dump_performance_statistics = false;

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_PERFORM_STATISTICAL_DUMP")),
            None
        );
        assert!(global_data.read().dump_performance_statistics);
    }

    #[test]
    fn test_demo_win_alias_sets_local_victory_state() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut guard = local_player.write().unwrap_or_else(|e| e.into_inner());
            guard.set_defeated(true);
        }
        {
            let mut list = ThePlayerList().write().unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }
        TheVictoryConditions::set_local_allied_victory(false);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_WIN")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(TheVictoryConditions::is_local_allied_victory());
        assert!(!local_player
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_defeated());

        ThePlayerList()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn test_runscript_alias_parsing_accepts_cpp_ranges() {
        assert_eq!(parse_runscript_alias("CHEAT_RUNSCRIPT1"), Some((true, 1)));
        assert_eq!(parse_runscript_alias("CHEAT_RUNSCRIPT9"), Some((true, 9)));
        assert_eq!(parse_runscript_alias("DEMO_RUNSCRIPT2"), Some((false, 2)));
        assert_eq!(parse_runscript_alias("DEMO_RUNSCRIPT9"), Some((false, 9)));
        assert_eq!(parse_runscript_alias("CHEAT_RUNSCRIPT0"), None);
        assert_eq!(parse_runscript_alias("DEMO_RUNSCRIPT10"), None);
    }

    #[test]
    fn test_demo_battle_cry_alias_is_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_BATTLE_CRY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_kill_selection_and_runscript_aliases_are_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        if let Ok(mut manager) = get_selection_manager().write() {
            manager.initialize_player(0);
            if let Some(selection) = manager.get_player_selection(0) {
                selection.clear_selection();
            }
        }
        set_local_player_id(0);

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_KILL_SELECTION")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_KILL_SELECTION")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_KILL_ALL_ENEMIES")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_RUNSCRIPT3")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_RUNSCRIPT7")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_VETERANCY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TAKE_VETERANCY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_demo_lock_camera_to_selection_alias_clears_lock_when_no_selection() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());

        if let Ok(mut manager) = get_selection_manager().write() {
            manager.initialize_player(0);
            if let Some(selection) = manager.get_player_selection(0) {
                selection.clear_selection();
            }
        }
        set_local_player_id(0);

        with_tactical_view(|view| view.set_camera_lock(Some(42)));
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_LOCK_CAMERA_TO_SELECTION")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            crate::display::view::with_tactical_view_ref(|view| view.camera_lock_id()),
            None
        );
    }

    #[test]
    fn test_demo_lock_camera_to_planes_alias_is_consumed() {
        let _guard = test_state_lock().lock().unwrap_or_else(|e| e.into_inner());
        set_last_plane_lock_object_id_for_tests(None);
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_LOCK_CAMERA_TO_PLANES")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }
}
