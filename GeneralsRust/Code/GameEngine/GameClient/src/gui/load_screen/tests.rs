// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_network::{GameInfo, GameSlot, SlotState};
    use crate::gui::gadgets::progressbar::ProgressBar;
    use crate::gui::game_window::WindowWidget;
    use game_engine::common::ini::ini_map_cache::{Coord3D, Region3D};
    use game_engine::common::language::Language;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    };

    static TEST_LANGUAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static TEST_LOAD_SCREEN_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static TEST_MOUSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_test_language() -> std::sync::MutexGuard<'static, ()> {
        TEST_LANGUAGE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_test_load_screen_state() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOAD_SCREEN_STATE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_test_mouse() -> std::sync::MutexGuard<'static, ()> {
        TEST_MOUSE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn selection_matches_cpp_game_logic_modes() {
        let base = LoadScreenRequest {
            mode: LoadScreenGameMode::None,
            loading_save_game: false,
            has_current_campaign: false,
            current_campaign_is_challenge: false,
        };

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Shell,
                ..base
            }),
            Some(LoadScreenKind::ShellGame)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Replay,
                ..base
            }),
            Some(LoadScreenKind::ShellGame)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Skirmish,
                ..base
            }),
            Some(LoadScreenKind::Multiplayer)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Lan,
                ..base
            }),
            Some(LoadScreenKind::Multiplayer)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Internet,
                ..base
            }),
            Some(LoadScreenKind::GameSpy)
        );
        assert_eq!(select_load_screen(base), None);
    }

    #[test]
    fn single_player_selection_matches_campaign_and_save_rules() {
        let normal_campaign = LoadScreenRequest {
            mode: LoadScreenGameMode::SinglePlayer,
            loading_save_game: false,
            has_current_campaign: true,
            current_campaign_is_challenge: false,
        };
        assert_eq!(
            select_load_screen(normal_campaign),
            Some(LoadScreenKind::SinglePlayer)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                current_campaign_is_challenge: true,
                ..normal_campaign
            }),
            Some(LoadScreenKind::Challenge)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                loading_save_game: true,
                ..normal_campaign
            }),
            Some(LoadScreenKind::ShellGame)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                has_current_campaign: false,
                ..normal_campaign
            }),
            Some(LoadScreenKind::ShellGame)
        );
    }

    #[test]
    fn descriptors_match_cpp_layout_names() {
        let single = descriptor_for_kind(LoadScreenKind::SinglePlayer);
        assert_eq!(single.layout, "Menus/SinglePlayerLoadScreen.wnd");
        assert_eq!(
            single.primary_progress,
            "SinglePlayerLoadScreen.wnd:ProgressLoad"
        );
        assert!(single.uses_progress_fudge);

        let multiplayer = descriptor_for_kind(LoadScreenKind::Multiplayer);
        assert_eq!(multiplayer.layout, "Menus/MultiplayerLoadScreen.wnd");
        assert_eq!(
            multiplayer.primary_progress,
            "MultiplayerLoadScreen.wnd:ProgressLoad0"
        );
        assert_eq!(multiplayer.slot_count, MAX_LOAD_SCREEN_SLOTS);

        let map_transfer = descriptor_for_kind(LoadScreenKind::MapTransfer);
        assert_eq!(map_transfer.layout, "Menus/MapTransferScreen.wnd");
        assert_eq!(
            map_transfer.primary_progress,
            "MapTransferScreen.wnd:ProgressLoad0"
        );
        assert_eq!(map_transfer.slot_count, MAX_LOAD_SCREEN_SLOTS);
        assert!(!map_transfer.uses_progress_fudge);
    }

    #[test]
    fn multiplayer_init_compacts_visible_context_slots_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 3);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_status(WindowStatus::IMAGE);

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot_with_text_color(
                    "Alice",
                    "USA",
                    0,
                    Some(2),
                    Some(0xFF11_2233),
                    false,
                    true,
                ),
                load_screen_slot("Empty", "GLA", 1, false, false),
                load_screen_slot_with_color("Bob", "China", 2, Some(4), false, true),
            ],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);

        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer0"),
            "Alice"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer1"),
            "Bob"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam1"),
            "Team:3"
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer0"),
            0xFF11_2233
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextSide0"),
            0xFF11_2233
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam0"),
            0xFF11_2233
        );
        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAFactionLogoLg_US".to_string())
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures"),
            "USA"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralName"),
            "USA"
        );
        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:ProgressLoad1", 6),
            Some("LoadingBar_ProgressCenter4".to_string())
        );
        assert!(
            !window_status(&wm, "MultiplayerLoadScreen.wnd:WinMapPreview")
                .contains(WindowStatus::IMAGE)
        );
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ProgressLoad1"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ProgressLoad2"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:StaticTextPlayer2"
        ));
    }

    #[test]
    fn multiplayer_row_team_text_is_localized_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("Team:2", "Team Two");

        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 1);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Local", "USA", 1, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);

        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam0"),
            "Team Two"
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_progress_bar_image_falls_back_when_color_image_missing_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let collection = get_mapped_image_collection();
        {
            let mut collection = collection.write();
            collection.clear();
            collection.add_image(crate::display::image::Image::with_name(
                "LoadingBar_Progress",
            ));
        }

        let colored = load_screen_slot_with_color("Player", "USA", 3, Some(6), false, true);
        assert_eq!(
            multiplayer_progress_bar_image(&colored),
            Some("LoadingBar_Progress".to_string())
        );

        collection.write().clear();
    }

    #[test]
    fn multiplayer_init_uses_context_general_portrait_features_and_name_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 1);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmericaSuperWeaponGeneral".to_string(),
            local_general_name: "General Alexander".to_string(),
            local_general_features: "Super Weapon General".to_string(),
            local_general_portrait: Some("SAGeneralPortrait".to_string()),
            local_load_screen_music: "Load_USA".to_string(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Local", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);

        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAGeneralPortrait".to_string())
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures"),
            "Super Weapon General"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralName"),
            "General Alexander"
        );
    }

    #[test]
    fn gamespy_init_keeps_player_row_for_ai_but_hides_ai_stats() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "GameSpyLoadScreen.wnd", 3);
        create_gamespy_slot_windows(&mut wm, 3);
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot("Human", "USA", 0, false, true),
                load_screen_slot("AI", "GLA", -1, true, true),
            ],
        };

        initialize_gamespy_windows(&mut wm, &context);

        assert!(!window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer0"));
        assert!(!window_hidden(
            &wm,
            "GameSpyLoadScreen.wnd:StaticTextWinLoss0"
        ));
        assert!(!window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer1"));
        assert!(window_hidden(
            &wm,
            "GameSpyLoadScreen.wnd:StaticTextWinLoss1"
        ));
        assert!(wm
            .find_window_by_name("GameSpyLoadScreen.wnd:StaticTextTeam1")
            .is_none());
        assert_eq!(
            window_image_name(&wm, "GameSpyLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAFactionLogo144_US".to_string())
        );
        assert!(window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer2"));
    }

    #[test]
    fn map_transfer_init_compacts_human_slots_and_hides_done_progress_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_map_transfer_load_screen_state();
        let mut wm = WindowManager::new();
        create_map_transfer_slot_windows(&mut wm, 4);

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot_with_map(0, "Host", Some(0xFF11_2233), true, false, true),
                load_screen_slot_with_map(1, "AI", Some(0xFF44_5566), true, true, true),
                load_screen_slot_with_map(2, "NeedsMap", Some(0xFF77_8899), false, false, true),
                load_screen_slot_with_map(3, "HasMap", Some(0xFFAA_BBCC), true, false, true),
            ],
        };

        initialize_map_transfer_windows(&mut wm, &context);

        assert_eq!(
            window_text(&wm, "MapTransferScreen.wnd:StaticTextPlayer0"),
            "Host"
        );
        assert_eq!(
            window_text(&wm, "MapTransferScreen.wnd:StaticTextPlayer1"),
            "NeedsMap"
        );
        assert_eq!(
            window_text(&wm, "MapTransferScreen.wnd:StaticTextPlayer2"),
            "HasMap"
        );
        assert!(window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad0"));
        assert!(!window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad1"));
        assert!(window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad2"));
        assert!(window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad3"));
        assert_eq!(
            window_text_color(&wm, "MapTransferScreen.wnd:StaticTextProgress1"),
            0xFF77_8899
        );
        assert_eq!(
            progress_fill_color(&wm, "MapTransferScreen.wnd:ProgressLoad1"),
            crate::gui::gadgets::Color::rgba(0x77, 0x88, 0x99, 0xFF)
        );

        let state = with_map_transfer_load_screen_state(|state| state.clone());
        assert_eq!(state.player_lookup[0], 0);
        assert_eq!(state.player_lookup[1], -1);
        assert_eq!(state.player_lookup[2], 1);
        assert_eq!(state.player_lookup[3], 2);
    }

    #[test]
    fn map_transfer_progress_timeout_and_filename_cache_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("MapTransfer:Unpacking", "Unpacking map");
        Language::register_localized_string("MapTransfer:Timeout", "%d:%d remaining");
        Language::register_localized_string("MapTransfer:CurrentFile", "Current: %s");
        reset_map_transfer_load_screen_state();

        let mut wm = WindowManager::new();
        create_map_transfer_slot_windows(&mut wm, 2);
        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot_with_map(
                2, "NeedsMap", None, false, false, true,
            )],
        };
        initialize_map_transfer_windows(&mut wm, &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        assert!(!process_load_screen_progress(
            LoadScreenKind::MapTransfer,
            2,
            47.0
        ));
        assert!(process_map_transfer_progress(
            2,
            47,
            "MapTransfer:Unpacking"
        ));
        assert_eq!(
            progress_value("MapTransferScreen.wnd:ProgressLoad0"),
            Some(0.47)
        );
        with_window_manager(|wm| {
            assert_eq!(
                window_text(wm, "MapTransferScreen.wnd:StaticTextProgress0"),
                "Unpacking map"
            );
        });
        assert!(!process_map_transfer_progress(
            2,
            47,
            "MapTransfer:Unpacking"
        ));
        assert!(!process_map_transfer_progress(
            7,
            30,
            "MapTransfer:Unpacking"
        ));

        assert!(process_map_transfer_timeout(125));
        with_window_manager(|wm| {
            assert_eq!(
                window_text(wm, "MapTransferScreen.wnd:StaticTextTimeout"),
                "2:5 remaining"
            );
        });
        assert!(!process_map_transfer_timeout(125));

        set_map_transfer_current_filename("Maps\\Official\\Tournament.map");
        with_window_manager(|wm| {
            assert_eq!(
                window_text(wm, "MapTransferScreen.wnd:StaticTextCurrentFile"),
                "Current: Tournament.map"
            );
        });

        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_process_progress_uses_cpp_player_lookup_mapping() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 3);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Alice".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot("Alice", "USA", 0, false, true),
                load_screen_slot("Empty", "GLA", 1, false, false),
                load_screen_slot("Bob", "China", 2, false, true),
            ],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        assert!(process_load_screen_progress(
            LoadScreenKind::Multiplayer,
            2,
            62.0
        ));
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1"),
            Some(0.62)
        );
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0"),
            Some(0.0)
        );
        assert!(!process_load_screen_progress(
            LoadScreenKind::Multiplayer,
            1,
            30.0
        ));
    }

    #[test]
    fn multiplayer_update_without_lookup_does_not_fallback_to_slot_zero() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 2);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Missing".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 7,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        update_load_screen(LoadScreenKind::Multiplayer, 41.0);

        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0"),
            Some(0.0)
        );
    }

    #[test]
    fn multiplayer_update_reports_progress_before_local_ui_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        clear_multiplayer_load_progress_hook();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let hook_calls = Arc::clone(&calls);
        register_multiplayer_load_progress_hook(move |player_id, percent| {
            hook_calls.lock().unwrap().push((player_id, percent));
        });

        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 2);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Bob".to_string(),
            local_side_name: "China".to_string(),
            local_template_name: "FactionChina".to_string(),
            local_general_name: "China".to_string(),
            local_general_features: "China".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 1,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![
                load_screen_slot("Alice", "USA", 0, false, true),
                load_screen_slot("Bob", "China", 1, false, true),
            ],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        update_load_screen(LoadScreenKind::Multiplayer, 41.0);

        assert_eq!(*calls.lock().unwrap(), vec![(1, 41)]);
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1"),
            Some(0.41)
        );
        clear_multiplayer_load_progress_hook();
    }

    #[test]
    fn multiplayer_update_above_complete_only_pumps_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        clear_multiplayer_load_progress_hook();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let hook_calls = Arc::clone(&calls);
        register_multiplayer_load_progress_hook(move |player_id, percent| {
            hook_calls.lock().unwrap().push((player_id, percent));
        });

        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 1);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        let context = LoadScreenInitContext {
            local_player_name: "Alice".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        update_load_screen(LoadScreenKind::Multiplayer, 101.0);

        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0"),
            Some(0.0)
        );
        clear_multiplayer_load_progress_hook();
    }

    #[test]
    fn gamespy_update_reports_progress_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        clear_multiplayer_load_progress_hook();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let hook_calls = Arc::clone(&calls);
        register_multiplayer_load_progress_hook(move |player_id, percent| {
            hook_calls.lock().unwrap().push((player_id, percent));
        });

        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "GameSpyLoadScreen.wnd", 1);
        create_gamespy_slot_windows(&mut wm, 1);

        let context = LoadScreenInitContext {
            local_player_name: "Alice".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
        };

        initialize_gamespy_windows(&mut wm, &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        update_load_screen(LoadScreenKind::GameSpy, 77.0);

        assert_eq!(*calls.lock().unwrap(), vec![(0, 77)]);
        assert_eq!(
            progress_value("GameSpyLoadScreen.wnd:ProgressLoad0"),
            Some(0.77)
        );
        clear_multiplayer_load_progress_hook();
    }

    #[test]
    fn multiplayer_init_resets_all_progress_bars_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_multiplayer_load_screen_state();
        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "MultiplayerLoadScreen.wnd", 3);
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures");
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:LocalGeneralName");

        for slot in 0..3 {
            let name = format!("MultiplayerLoadScreen.wnd:ProgressLoad{slot}");
            wm.find_window_by_name(&name)
                .expect("progress")
                .borrow_mut()
                .progress_bar_mut()
                .expect("progress widget")
                .set_value(0.87);
        }

        let context = LoadScreenInitContext {
            local_player_name: "Alice".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmerica".to_string(),
            local_general_name: "USA".to_string(),
            local_general_features: "USA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context);
        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0"),
            Some(0.0)
        );
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1"),
            Some(0.0)
        );
        assert_eq!(
            progress_value("MultiplayerLoadScreen.wnd:ProgressLoad2"),
            Some(0.0)
        );
    }

    #[test]
    fn load_screen_init_context_default_preserves_single_local_slot() {
        let context = LoadScreenInitContext {
            local_player_name: "Fallback".to_string(),
            local_side_name: "GLA".to_string(),
            local_template_name: "FactionGLA".to_string(),
            local_general_name: "GLA".to_string(),
            local_general_features: "GLA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 4,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: Vec::new(),
        };

        let slots = multiplayer_slot_contexts(&context);

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].player_name, "Fallback");
        assert_eq!(slots[0].side_name, "GLA");
        assert_eq!(multiplayer_team_text(&slots[0]), "Team:5");
    }

    #[test]
    fn multiplayer_team_text_matches_cpp_team_plus_one() {
        let ai_slot = load_screen_slot("AI", "GLA", -1, true, true);

        assert_eq!(multiplayer_team_text(&ai_slot), "Team:0");
        assert!(!load_screen_has_team_windows("GameSpyLoadScreen.wnd"));
        assert!(load_screen_has_team_windows("MultiplayerLoadScreen.wnd"));
    }

    #[test]
    fn multiplayer_start_position_buttons_match_map_waypoints_and_apparent_slots() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        let mut wm = WindowManager::new();
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_size(100, 100)
            .expect("preview size");
        create_multiplayer_start_position_windows(&mut wm, "MultiplayerLoadScreen.wnd");

        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = true;
        metadata.num_players = 3;
        metadata.extent =
            Region3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(100.0, 100.0, 0.0));
        metadata.set_waypoint("Player_1_Start".to_string(), Coord3D::new(25.0, 75.0, 0.0));
        metadata.set_waypoint("Player_2_Start".to_string(), Coord3D::new(75.0, 25.0, 0.0));
        metadata.set_waypoint("Player_3_Start".to_string(), Coord3D::new(25.0, 75.0, 0.0));

        update_multiplayer_start_position_buttons(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            Some(&metadata),
            &[Some(1), None, Some(0)],
        );

        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition3"
        ));
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            (20, 20)
        );
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            (70, 70)
        );
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"),
            (20, 31)
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            GameText::fetch("NUMBER:3")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            GameText::fetch("NUMBER:1")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"),
            ""
        );
        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_map_preview_keeps_start_positions_when_preview_image_missing_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        game_engine::common::ini::ini_map_cache::init_global_map_cache();

        let map_name = "Maps/TestNoPreview/TestNoPreview.map";
        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = true;
        metadata.num_players = 2;
        metadata.extent =
            Region3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(100.0, 100.0, 0.0));
        metadata.set_waypoint("Player_1_Start".to_string(), Coord3D::new(20.0, 80.0, 0.0));
        metadata.set_waypoint("Player_2_Start".to_string(), Coord3D::new(80.0, 20.0, 0.0));
        {
            let mut cache =
                game_engine::common::ini::ini_map_cache::get_map_cache_mut().expect("map cache");
            cache.insert(map_name.to_string(), metadata);
        }

        let mut wm = WindowManager::new();
        named_test_window(&mut wm, "MultiplayerLoadScreen.wnd:WinMapPreview");
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_size(100, 100)
            .expect("preview size");
        create_multiplayer_start_position_windows(&mut wm, "MultiplayerLoadScreen.wnd");

        initialize_multiplayer_map_preview(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            Some(map_name),
            &[Some(1), Some(0)],
        );

        assert!(
            !window_status(&wm, "MultiplayerLoadScreen.wnd:WinMapPreview")
                .contains(WindowStatus::IMAGE)
        );
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"
        ));
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            GameText::fetch("NUMBER:2")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            GameText::fetch("NUMBER:1")
        );

        game_engine::common::ini::ini_map_cache::get_map_cache_mut()
            .expect("map cache")
            .remove(map_name);
        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_start_position_buttons_hide_without_multiplayer_metadata() {
        let _state_guard = lock_test_load_screen_state();
        let mut wm = WindowManager::new();
        named_test_window(&mut wm, "GameSpyLoadScreen.wnd:WinMapPreview");
        create_multiplayer_start_position_windows(&mut wm, "GameSpyLoadScreen.wnd");

        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = false;
        metadata.num_players = 2;

        update_multiplayer_start_position_buttons(
            &mut wm,
            "GameSpyLoadScreen.wnd",
            Some(&metadata),
            &[Some(0)],
        );

        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            assert!(window_hidden(
                &wm,
                &format!("GameSpyLoadScreen.wnd:ButtonMapStartPosition{slot}")
            ));
        }
    }

    #[test]
    fn game_info_context_preserves_original_slot_ids_and_apparent_colors() {
        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);
        game_info.set_map("Maps/Test/Test.map".to_string());

        let mut alice = GameSlot::new();
        alice.set_state(SlotState::Player, "Alice".to_string(), 0x7F00_0001);
        alice.set_player_template(-1);
        alice.set_team_number(0);
        alice.set_color(2);
        alice.set_start_pos(1);

        let mut empty = GameSlot::new();
        empty.set_state(SlotState::Open, String::new(), 0);

        let mut bob = GameSlot::new();
        bob.set_state(SlotState::BrutalAI, String::new(), 0);
        bob.set_player_template(-1);
        bob.set_team_number(-1);
        bob.set_color(5);
        bob.set_start_pos(0);

        game_info.set_slot(0, alice);
        game_info.set_slot(1, empty);
        game_info.set_slot(2, bob);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(context.map_name.as_deref(), Some("Maps/Test/Test.map"));
        assert_eq!(context.start_positions[0], Some(1));
        assert_eq!(context.start_positions[1], None);
        assert_eq!(context.start_positions[2], Some(0));
        assert_eq!(context.local_player_name, "Alice");
        assert_eq!(context.local_team_number, 0);
        assert_eq!(context.slots.len(), 2);
        assert_eq!(context.slots[0].player_id, 0);
        assert_eq!(context.slots[0].apparent_color, Some(2));
        assert_eq!(context.slots[1].player_id, 2);
        assert_eq!(context.slots[1].team_number, -1);
        assert_eq!(context.slots[1].apparent_color, Some(5));
        assert!(context.slots[1].is_ai);
    }

    #[test]
    fn game_info_context_hides_map_when_local_pregame_slot_lacks_map_like_cpp() {
        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0002);
        game_info.set_map("Maps/MissingLocal/MissingLocal.map".to_string());

        let mut host = GameSlot::new();
        host.set_state(SlotState::Player, "Host".to_string(), 0x7F00_0001);
        host.set_player_template(-1);
        host.set_start_pos(0);
        game_info.set_slot(0, host);

        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0002);
        local.set_player_template(-1);
        local.set_start_pos(1);
        local.set_map_availability(false);
        game_info.set_slot(1, local);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(game_info.get_local_slot_num(), 1);
        assert_eq!(context.local_player_name, "Local");
        assert_eq!(context.map_name, None);
        assert_eq!(context.start_positions[0], Some(0));
        assert_eq!(context.start_positions[1], Some(1));

        game_info.set_game_in_progress(true);
        let context = load_screen_init_context_from_game_info(&game_info);
        assert_eq!(
            context.map_name.as_deref(),
            Some("Maps/MissingLocal/MissingLocal.map")
        );
    }

    #[test]
    fn game_info_context_hides_map_when_local_slot_is_missing_like_cpp() {
        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0009);
        game_info.set_map("Maps/NoLocal/NoLocal.map".to_string());

        let mut host = GameSlot::new();
        host.set_state(SlotState::Player, "Host".to_string(), 0x7F00_0001);
        host.set_player_template(-1);
        host.set_start_pos(0);
        game_info.set_slot(0, host);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(game_info.get_local_slot_num(), -1);
        assert_eq!(context.local_player_name, "Host");
        assert_eq!(context.map_name, None);
    }

    #[test]
    fn game_info_context_uses_game_lod_mem_pass_for_shell_intro_gate() {
        game_engine::common::game_lod::set_mem_passed_override_for_tests(Some(false));

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);

        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0001);
        local.set_player_template(-1);
        game_info.set_slot(0, local);

        let context = load_screen_init_context_from_game_info(&game_info);
        assert!(!context.shell_game_did_mem_pass);

        game_engine::common::game_lod::set_mem_passed_override_for_tests(Some(true));
        let context = load_screen_init_context_from_game_info(&game_info);
        assert!(context.shell_game_did_mem_pass);

        game_engine::common::game_lod::set_mem_passed_override_for_tests(None);
    }

    #[test]
    fn game_info_context_uses_local_template_for_general_presentation_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("GUI:AirFeatures", "Air Force General");
        Language::register_localized_string("CHALLENGE:AirName", "General Granger");

        {
            let mut store =
                game_engine::common::rts::player_template::get_player_template_store_mut();
            store.clear();
            let mut template = PlayerTemplate::new("FactionAirForceGeneral".to_string());
            template.display_name = "Air Force".to_string();
            template.features = "GUI:AirFeatures".to_string();
            template.load_screen_music = "Load_USA".to_string();
            store.add_template(template);
        }

        init_challenge_generals();
        {
            let mut generals = get_challenge_generals_mut().expect("challenge generals");
            let positions = generals.challenge_generals_mut();
            positions[0] = GeneralPersona::new();
            positions[0].set_player_template_name("FactionAirForceGeneral".to_string());
            positions[0].set_bio_name("CHALLENGE:AirName".to_string());
            positions[0].set_bio_portrait_large(Some("AirGeneralPortrait".to_string()));
        }

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);
        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0001);
        local.set_player_template(0);
        local.set_team_number(0);
        local.set_color(2);
        local.set_start_pos(0);
        game_info.set_slot(0, local);

        let context = load_screen_init_context_from_game_info(&game_info);

        assert_eq!(context.local_player_name, "Local");
        assert_eq!(context.local_general_name, "General Granger");
        assert_eq!(context.local_general_features, "Air Force General");
        assert_eq!(
            context.local_general_portrait.as_deref(),
            Some("AirGeneralPortrait")
        );
        assert_eq!(context.local_load_screen_music, "Load_USA");

        game_engine::common::rts::player_template::get_player_template_store_mut().clear();
        Language::clear_localized_strings();
    }

    #[test]
    fn multiplayer_progress_bar_images_match_cpp_apparent_color_names() {
        let _state_guard = lock_test_load_screen_state();
        get_mapped_image_collection().write().clear();

        let colored = load_screen_slot_with_color("Player", "USA", 3, Some(6), false, true);
        assert_eq!(
            multiplayer_progress_bar_image(&colored),
            Some("LoadingBar_ProgressCenter6".to_string())
        );

        let fallback = load_screen_slot("Player", "USA", 3, false, true);
        assert_eq!(multiplayer_progress_bar_image(&fallback), None);

        let invalid = load_screen_slot_with_color("Player", "USA", 3, Some(-1), false, true);
        assert_eq!(multiplayer_progress_bar_image(&invalid), None);

        get_mapped_image_collection().write().clear();
    }

    #[test]
    fn progress_fudge_matches_single_player_cpp_formula() {
        let single = descriptor_for_kind(LoadScreenKind::SinglePlayer);
        assert!((transformed_progress_percent(single, 0.0) - (30.0 / 1.3)).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, 100.0) - 100.0).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, 150.0) - (180.0 / 1.3)).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, -50.0) - (-20.0 / 1.3)).abs() < f32::EPSILON);

        let shell = descriptor_for_kind(LoadScreenKind::ShellGame);
        assert!((transformed_progress_percent(shell, 42.0) - 42.0).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(shell, 150.0) - 150.0).abs() < f32::EPSILON);

        let map_transfer = descriptor_for_kind(LoadScreenKind::MapTransfer);
        assert!((transformed_progress_percent(map_transfer, 42.0) - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_progress_window_uses_cpp_progress_message_range_rules() {
        let mut wm = WindowManager::new();
        named_progress_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad");

        set_progress_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 37.9);
        let progress = wm
            .find_window_by_name("SinglePlayerLoadScreen.wnd:ProgressLoad")
            .expect("progress");
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );

        set_progress_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 138.4);
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );

        set_progress_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", -4.2);
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );
    }

    #[test]
    fn update_load_screen_clears_cursor_tooltip_like_cpp() {
        let _mouse_guard = lock_test_mouse();
        with_mouse(|mouse| {
            mouse.set_cursor_tooltip("Stale tooltip".to_string(), Some(0), None, None);
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "Stale tooltip");
            assert!(!mouse.cursor_tooltip_state().is_tooltip_empty);
        });

        update_load_screen(LoadScreenKind::SinglePlayer, 50.0);

        with_mouse(|mouse| {
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "");
            assert!(mouse.cursor_tooltip_state().is_tooltip_empty);
        });
    }

    #[test]
    fn map_transfer_update_pumps_liteupdate_before_clearing_tooltip_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _mouse_guard = lock_test_mouse();
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        clear_map_transfer_liteupdate_hook();
        register_map_transfer_liteupdate_hook(move || {
            hook_calls.fetch_add(1, Ordering::SeqCst);
            with_mouse(|mouse| {
                mouse.set_cursor_tooltip(
                    "Network pump touched tooltip".to_string(),
                    None,
                    None,
                    None,
                )
            });
        });

        update_load_screen(LoadScreenKind::MapTransfer, 0.0);

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        with_mouse(|mouse| {
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "");
            assert!(mouse.cursor_tooltip_state().is_tooltip_empty);
        });
        clear_map_transfer_liteupdate_hook();
    }

    #[test]
    fn non_map_transfer_updates_do_not_pump_liteupdate() {
        let _state_guard = lock_test_load_screen_state();
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        clear_map_transfer_liteupdate_hook();
        register_map_transfer_liteupdate_hook(move || {
            hook_calls.fetch_add(1, Ordering::SeqCst);
        });

        update_load_screen(LoadScreenKind::SinglePlayer, 0.0);

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        clear_map_transfer_liteupdate_hook();
    }

    #[test]
    fn non_multiplayer_updates_do_not_report_load_progress() {
        let _state_guard = lock_test_load_screen_state();
        clear_multiplayer_load_progress_hook();
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        register_multiplayer_load_progress_hook(move |_, _| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
        });

        update_load_screen(LoadScreenKind::SinglePlayer, 50.0);
        update_load_screen(LoadScreenKind::Challenge, 50.0);
        update_load_screen(LoadScreenKind::MapTransfer, 50.0);

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        clear_multiplayer_load_progress_hook();
    }

    #[test]
    fn update_load_screen_finishes_every_branch_after_local_work_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _mouse_guard = lock_test_mouse();
        clear_map_transfer_liteupdate_hook();
        clear_multiplayer_load_progress_hook();
        clear_load_screen_finish_update_hook();
        clear_load_screen_presentation_pump();

        let finish_calls = Arc::new(AtomicUsize::new(0));
        let hook_finish_calls = Arc::clone(&finish_calls);
        register_load_screen_finish_update_hook(move || {
            hook_finish_calls.fetch_add(1, Ordering::SeqCst);
            with_mouse(|mouse| {
                assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "");
                assert!(mouse.cursor_tooltip_state().is_tooltip_empty);
            });
        });

        let presentation_calls = Arc::new(AtomicUsize::new(0));
        let hook_presentation_calls = Arc::clone(&presentation_calls);
        register_load_screen_presentation_pump(move || {
            hook_presentation_calls.fetch_add(1, Ordering::SeqCst);
        });

        register_map_transfer_liteupdate_hook(|| {
            with_mouse(|mouse| {
                mouse.set_cursor_tooltip("Liteupdate touched tooltip".to_string(), None, None, None)
            });
        });

        update_load_screen(LoadScreenKind::MapTransfer, 0.0);
        update_load_screen(LoadScreenKind::SinglePlayer, 25.0);
        update_load_screen(LoadScreenKind::Challenge, 50.0);
        update_load_screen(LoadScreenKind::Multiplayer, 75.0);

        assert_eq!(finish_calls.load(Ordering::SeqCst), 4);
        assert_eq!(presentation_calls.load(Ordering::SeqCst), 4);
        clear_map_transfer_liteupdate_hook();
        clear_load_screen_finish_update_hook();
        clear_load_screen_presentation_pump();
    }

    #[test]
    fn single_player_audio_prelude_plays_only_ambient_like_zh_md() {
        // C++ LoadScreen.cpp:532-533 PULLED FROM THE MISSION DISK — BriefingVoice
        // is never force-played after the prelude. Only LoadScreenAmbient (:590).
        let _state_guard = lock_test_load_screen_state();
        reset_single_player_load_screen_audio_state();
        with_single_player_load_screen_state(|state| {
            *state = SinglePlayerLoadScreenState::default()
        });
        {
            let mut manager = get_campaign_manager();
            let campaign = manager.new_campaign("AudioPrelude".to_string());
            let mission = campaign.new_mission("Mission1".to_string());
            mission.briefing_voice =
                game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_sound_file(
                    "BriefingVoiceEvent".to_string(),
                );
            manager.set_campaign_and_mission("AudioPrelude", "Mission1");
        }

        finish_single_player_load_screen_audio_prelude();

        let (briefing_played, briefing_handle, ambient_handle) =
            with_single_player_load_screen_state(|state| {
                (
                    state.briefing_voice_played,
                    state.briefing_voice_handle,
                    state.ambient_loop_handle,
                )
            });
        assert!(!briefing_played);
        assert_eq!(briefing_handle, 0);
        assert_eq!(ambient_handle, add_audio_event("LoadScreenAmbient"));

        reset_single_player_load_screen_audio_state();
        let (briefing_played, briefing_handle, ambient_handle) =
            with_single_player_load_screen_state(|state| {
                (
                    state.briefing_voice_played,
                    state.briefing_voice_handle,
                    state.ambient_loop_handle,
                )
            });
        assert!(!briefing_played);
        assert_eq!(briefing_handle, 0);
        assert_eq!(ambient_handle, 0);
    }

    #[test]
    fn single_player_movie_open_failure_returns_before_audio_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_single_player_load_screen_audio_state();
        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
        let movie_requests = Arc::new(Mutex::new(Vec::new()));
        let hook_requests = Arc::clone(&movie_requests);
        register_single_player_movie_play_hook(move |movie_name| {
            hook_requests.lock().unwrap().push(movie_name.to_string());
            false
        });

        {
            let mut manager = get_campaign_manager();
            let campaign = manager.new_campaign("USA".to_string());
            let mission = campaign.new_mission("Mission1".to_string());
            mission.movie_label = "MissingMovie.bik".to_string();
            mission.briefing_voice =
                game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_sound_file(
                    "BriefingVoiceEvent".to_string(),
                );
            manager.set_campaign_and_mission("USA", "Mission1");
        }

        let mut wm = WindowManager::new();
        named_test_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
        );
        named_progress_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad");
        named_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:Percent");

        initialize_single_player_windows(&mut wm, true);

        assert_eq!(
            *movie_requests.lock().unwrap(),
            vec!["MissingMovie.bik".to_string()]
        );
        let (prelude_active, briefing_played, briefing_handle, ambient_handle) =
            with_single_player_load_screen_state(|state| {
                (
                    state.movie_prelude_active,
                    state.briefing_voice_played,
                    state.briefing_voice_handle,
                    state.ambient_loop_handle,
                )
            });
        assert!(!prelude_active);
        assert!(!briefing_played);
        assert_eq!(briefing_handle, 0);
        assert_eq!(ambient_handle, 0);
        assert!(wm
            .find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
            .expect("percent")
            .borrow()
            .is_hidden());
        assert_eq!(
            window_enabled_image_name(
                &wm,
                "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                0
            ),
            None
        );
        assert_eq!(
            window_enabled_image_name(&wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 6),
            None
        );

        with_window_manager(|global_wm| *global_wm = wm);
        assert_eq!(
            run_load_screen_prelude_with_limit(LoadScreenKind::SinglePlayer, 1),
            LoadScreenPreludeOutcome::Failed,
            "a failed movie open must not turn into a synthetic prelude or audio path"
        );

        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
        reset_single_player_load_screen_audio_state();
    }

    #[test]
    fn single_player_movie_prelude_defers_ambient_until_movie_finishes_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        reset_single_player_load_screen_audio_state();
        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
        let movie_requests = Arc::new(Mutex::new(Vec::new()));
        let hook_requests = Arc::clone(&movie_requests);
        register_single_player_movie_play_hook(move |movie_name| {
            hook_requests.lock().unwrap().push(movie_name.to_string());
            true
        });
        let movie_playing = Arc::new(AtomicBool::new(true));
        let hook_movie_playing = Arc::clone(&movie_playing);
        register_single_player_movie_playing_hook(move |movie_name| {
            assert_eq!(movie_name, "EA_LOGO.BIK");
            hook_movie_playing.load(Ordering::SeqCst)
        });

        {
            let mut manager = get_campaign_manager();
            let campaign = manager.new_campaign("China".to_string());
            let mission = campaign.new_mission("Mission1".to_string());
            mission.movie_label = "EA_LOGO.BIK".to_string();
            manager.set_campaign_and_mission("China", "Mission1");
        }

        let mut wm = WindowManager::new();
        named_test_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
        );
        named_progress_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad");
        named_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:Percent");

        initialize_single_player_windows(&mut wm, true);

        assert_eq!(
            *movie_requests.lock().unwrap(),
            vec!["EA_LOGO.BIK".to_string()]
        );
        let (prelude_active, ambient_handle) = with_single_player_load_screen_state(|state| {
            (state.movie_prelude_active, state.ambient_loop_handle)
        });
        assert!(prelude_active);
        assert_eq!(ambient_handle, 0);
        assert!(wm
            .find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
            .expect("percent")
            .borrow()
            .is_hidden());
        assert_eq!(
            window_enabled_image_name(
                &wm,
                "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                0
            ),
            Some("MissionLoad_China".to_string())
        );
        assert_eq!(
            window_enabled_image_name(&wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 6),
            Some("LoadingBar_ProgressCenter1".to_string())
        );

        with_window_manager(|global_wm| {
            *global_wm = wm;
        });

        update_load_screen(LoadScreenKind::SinglePlayer, 25.0);
        let (prelude_active, ambient_handle) = with_single_player_load_screen_state(|state| {
            (state.movie_prelude_active, state.ambient_loop_handle)
        });
        assert!(prelude_active);
        assert_eq!(ambient_handle, 0);
        assert_eq!(
            progress_value("SinglePlayerLoadScreen.wnd:ProgressLoad"),
            Some(0.42)
        );
        with_window_manager(|wm| {
            assert_eq!(window_text(wm, "SinglePlayerLoadScreen.wnd:Percent"), "42%");
            assert!(wm
                .find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
                .expect("percent")
                .borrow()
                .is_hidden());
        });

        movie_playing.store(false, Ordering::SeqCst);
        update_load_screen(LoadScreenKind::SinglePlayer, 50.0);
        let (prelude_active, ambient_handle) = with_single_player_load_screen_state(|state| {
            (state.movie_prelude_active, state.ambient_loop_handle)
        });
        assert!(!prelude_active);
        assert_eq!(ambient_handle, add_audio_event("LoadScreenAmbient"));
        assert_eq!(
            progress_value("SinglePlayerLoadScreen.wnd:ProgressLoad"),
            Some(0.61)
        );
        with_window_manager(|wm| {
            assert_eq!(window_text(wm, "SinglePlayerLoadScreen.wnd:Percent"), "61%");
            assert!(wm
                .find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
                .expect("percent")
                .borrow()
                .is_hidden());
        });

        reset_single_player_load_screen_audio_state();
        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
    }

    #[test]
    fn single_player_prelude_driver_pumps_until_movie_completion_before_map_work() {
        let _state_guard = lock_test_load_screen_state();
        reset_single_player_load_screen_audio_state();
        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
        clear_load_screen_presentation_pump();

        register_single_player_movie_play_hook(|movie_name| movie_name == "EA_LOGO.BIK");
        let checks = Arc::new(AtomicUsize::new(0));
        let hook_checks = Arc::clone(&checks);
        register_single_player_movie_playing_hook(move |movie_name| {
            assert_eq!(movie_name, "EA_LOGO.BIK");
            hook_checks.fetch_add(1, Ordering::SeqCst) < 2
        });

        {
            let mut manager = get_campaign_manager();
            let campaign = manager.new_campaign("USA".to_string());
            let mission = campaign.new_mission("MissionPrelude".to_string());
            mission.movie_label = "EA_LOGO.BIK".to_string();
            manager.set_campaign_and_mission("USA", "MissionPrelude");
        }

        let mut wm = WindowManager::new();
        named_test_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
        );
        named_progress_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad");
        named_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:Percent");
        initialize_single_player_windows(&mut wm, true);
        with_window_manager(|global_wm| *global_wm = wm);

        let presentation_pumps = Rc::new(Cell::new(0));
        let hook_pumps = Rc::clone(&presentation_pumps);
        register_load_screen_presentation_pump(move || {
            hook_pumps.set(hook_pumps.get() + 1);
        });

        assert_eq!(
            run_load_screen_prelude_with_limit(LoadScreenKind::SinglePlayer, 8),
            LoadScreenPreludeOutcome::Complete
        );
        assert_eq!(checks.load(Ordering::SeqCst), 3);
        assert_eq!(presentation_pumps.get(), 3);
        let (prelude_state, briefing_played, ambient_handle) =
            with_single_player_load_screen_state(|state| {
                (
                    state.prelude_state,
                    state.briefing_voice_played,
                    state.ambient_loop_handle,
                )
            });
        assert_eq!(prelude_state, LoadScreenPreludeState::Complete);
        assert!(
            !briefing_played,
            "ZH MD LoadScreen.cpp:532-533 never plays BriefingVoice after prelude"
        );
        assert_eq!(ambient_handle, add_audio_event("LoadScreenAmbient"));

        clear_load_screen_presentation_pump();
        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
        reset_single_player_load_screen_audio_state();
    }

    #[test]
    fn single_player_prelude_driver_forces_terminal_skip_when_movie_never_finishes() {
        let _state_guard = lock_test_load_screen_state();
        reset_single_player_load_screen_audio_state();
        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
        clear_load_screen_presentation_pump();

        register_single_player_movie_play_hook(|movie_name| movie_name == "StuckMovie.bik");
        register_single_player_movie_playing_hook(|movie_name| movie_name == "StuckMovie.bik");
        {
            let mut manager = get_campaign_manager();
            let campaign = manager.new_campaign("GLA".to_string());
            let mission = campaign.new_mission("MissionStuck".to_string());
            mission.movie_label = "StuckMovie.bik".to_string();
            manager.set_campaign_and_mission("GLA", "MissionStuck");
        }

        let mut wm = WindowManager::new();
        named_test_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
        );
        named_progress_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:ProgressLoad");
        named_test_window(&mut wm, "SinglePlayerLoadScreen.wnd:Percent");
        initialize_single_player_windows(&mut wm, true);
        with_window_manager(|global_wm| *global_wm = wm);

        let presentation_pumps = Rc::new(Cell::new(0));
        let hook_pumps = Rc::clone(&presentation_pumps);
        register_load_screen_presentation_pump(move || {
            hook_pumps.set(hook_pumps.get() + 1);
        });

        assert_eq!(
            run_load_screen_prelude_with_limit(LoadScreenKind::SinglePlayer, 2),
            LoadScreenPreludeOutcome::Skipped
        );
        assert_eq!(presentation_pumps.get(), 2);
        let (prelude_state, ambient_handle) = with_single_player_load_screen_state(|state| {
            (state.prelude_state, state.ambient_loop_handle)
        });
        assert_eq!(prelude_state, LoadScreenPreludeState::Skipped);
        assert_eq!(ambient_handle, add_audio_event("LoadScreenAmbient"));

        clear_load_screen_presentation_pump();
        clear_single_player_movie_play_hook();
        clear_single_player_movie_playing_hook();
        reset_single_player_load_screen_audio_state();
    }

    #[test]
    fn single_player_campaign_images_match_cpp_side_mapping() {
        assert_eq!(
            single_player_campaign_images("USA"),
            Some(("MissionLoad_USA", "LoadingBar_ProgressCenter2"))
        );
        assert_eq!(
            single_player_campaign_images("gla"),
            Some(("MissionLoad_GLA", "LoadingBar_ProgressCenter3"))
        );
        assert_eq!(
            single_player_campaign_images("China"),
            Some(("MissionLoad_China", "LoadingBar_ProgressCenter1"))
        );
        assert_eq!(single_player_campaign_images("Challenge"), None);
    }

    #[test]
    fn multiplayer_local_general_faction_logos_match_cpp_fallbacks() {
        assert_eq!(
            multiplayer_local_general_faction_logo("USA", "MultiplayerLoadScreen.wnd"),
            Some("SAFactionLogoLg_US")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("FactionGLA", "MultiplayerLoadScreen.wnd"),
            Some("SUFactionLogoLg_GLA")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("China", "GameSpyLoadScreen.wnd"),
            Some("SNFactionLogo144_China")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("Random", "GameSpyLoadScreen.wnd"),
            None
        );
    }

    #[test]
    fn single_player_mission_text_fetches_cpp_labels() {
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("MISSION:Objective0", "Capture the base");
        Language::register_localized_string("MISSION:Objective2", "Hold position");
        Language::register_localized_string("UNIT:Ranger", "Ranger");
        Language::register_localized_string("UNIT:Humvee", "Humvee");
        Language::register_localized_string("MISSION:Location", "Northern sector");

        let mut mission = Mission::new();
        mission.mission_objectives_label[0] = "MISSION:Objective0".to_string();
        mission.mission_objectives_label[2] = "MISSION:Objective2".to_string();
        mission.unit_names[0] = "UNIT:Ranger".to_string();
        mission.unit_names[1] = "UNIT:Humvee".to_string();
        mission.location_name_label = "MISSION:Location".to_string();

        let text = single_player_mission_text(&mission);

        assert_eq!(text.objective_lines[0], "Capture the base");
        assert_eq!(text.objective_lines[1], "");
        assert_eq!(text.objective_lines[2], "Hold position");
        assert_eq!(text.unit_descriptions[0], "Ranger");
        assert_eq!(text.unit_descriptions[1], "Humvee");
        assert_eq!(text.unit_descriptions[2], "");
        assert_eq!(text.location, "Northern sector");

        with_single_player_load_screen_state(|state| {
            state.mission_text = text.clone();
            state.current_objective_line = 0;
            state.current_objective_width_offset = 0;
            state.current_objective_line_character = 0;
            state.finished_objective_text = false;
        });
        let cached = with_single_player_load_screen_state(|state| state.clone());
        assert_eq!(cached.mission_text.objective_lines[0], "Capture the base");
        assert_eq!(cached.current_objective_line, 0);
        assert_eq!(cached.current_objective_width_offset, 0);
        assert_eq!(cached.current_objective_line_character, 0);
        assert!(!cached.finished_objective_text);

        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_persona_text_matches_cpp_load_screen_fields() {
        let _language_guard = lock_test_language();
        Language::clear_localized_strings();
        Language::register_localized_string("CHALLENGE:PlayerName", "General Player");
        Language::register_localized_string("CHALLENGE:PlayerRank", "General");
        Language::register_localized_string("CHALLENGE:PlayerStrategy", "Air superiority");
        Language::register_localized_string("CHALLENGE:OpponentName", "General Opponent");
        Language::register_localized_string("CHALLENGE:OpponentRank", "Prince");
        Language::register_localized_string("CHALLENGE:OpponentStrategy", "Ambush");

        let mut generals = ChallengeGenerals::new();
        {
            let positions = generals.challenge_generals_mut();
            positions[0].set_campaign("ChallengeCampaign".to_string());
            positions[0].set_bio_name("CHALLENGE:PlayerName".to_string());
            positions[0].set_bio_rank("CHALLENGE:PlayerRank".to_string());
            positions[0].set_bio_strategy("CHALLENGE:PlayerStrategy".to_string());
            positions[0].set_bio_portrait_large(Some("PlayerPortrait".to_string()));
            positions[0].set_portrait_movie_left_name("PlayerMovieLeft".to_string());
            positions[0].set_portrait_movie_right_name("PlayerMovieRight".to_string());
            positions[0].set_name_sound("PlayerNameSound".to_string());
            positions[0].set_taunt_sound_1("PlayerTaunt1".to_string());
            positions[0].set_taunt_sound_2("PlayerTaunt2".to_string());
            positions[0].set_taunt_sound_3("PlayerTaunt3".to_string());

            positions[1].set_bio_name("CHALLENGE:OpponentName".to_string());
            positions[1].set_bio_rank("CHALLENGE:OpponentRank".to_string());
            positions[1].set_bio_strategy("CHALLENGE:OpponentStrategy".to_string());
            positions[1].set_bio_portrait_large(Some("OpponentPortrait".to_string()));
            positions[1].set_portrait_movie_left_name("OpponentMovieLeft".to_string());
            positions[1].set_portrait_movie_right_name("OpponentMovieRight".to_string());
            positions[1].set_name_sound("OpponentNameSound".to_string());
            positions[1].set_taunt_sound_1("OpponentTaunt1".to_string());
            positions[1].set_taunt_sound_2("OpponentTaunt2".to_string());
            positions[1].set_taunt_sound_3("OpponentTaunt3".to_string());
        }

        let (player, opponent) = challenge_persona_text_for_current_mission(
            "ChallengeCampaign",
            "CHALLENGE:OpponentName",
            &generals,
        )
        .expect("challenge personas");

        assert_eq!(player.big_name, "General Player");
        assert_eq!(player.name, "General Player");
        assert_eq!(player.rank, "General");
        assert_eq!(player.strategy, "Air superiority");
        assert_eq!(player.portrait_large.as_deref(), Some("PlayerPortrait"));
        assert_eq!(player.portrait_movie_left, "PlayerMovieLeft");
        assert_eq!(player.portrait_movie_right, "PlayerMovieRight");
        assert_eq!(player.name_sound, "PlayerNameSound");
        assert_eq!(
            player.taunt_sounds,
            ["PlayerTaunt1", "PlayerTaunt2", "PlayerTaunt3"]
        );

        assert_eq!(opponent.big_name, "General Opponent");
        assert_eq!(opponent.name, "General Opponent");
        assert_eq!(opponent.rank, "Prince");
        assert_eq!(opponent.strategy, "Ambush");
        assert_eq!(opponent.portrait_large.as_deref(), Some("OpponentPortrait"));
        assert_eq!(opponent.portrait_movie_left, "OpponentMovieLeft");
        assert_eq!(opponent.portrait_movie_right, "OpponentMovieRight");
        assert_eq!(opponent.name_sound, "OpponentNameSound");
        assert_eq!(
            opponent.taunt_sounds,
            ["OpponentTaunt1", "OpponentTaunt2", "OpponentTaunt3"]
        );

        Language::clear_localized_strings();
    }

    fn named_test_window(wm: &mut WindowManager, name: &str) {
        let window = wm.create_window(None, 0, 0, 100, 20).expect("window");
        let mut window = window.borrow_mut();
        window.set_name(name);
        let _ = window.hide(true);
    }

    fn named_progress_test_window(wm: &mut WindowManager, name: &str) {
        let window = wm.create_window(None, 0, 0, 100, 20).expect("window");
        let mut window = window.borrow_mut();
        window.set_name(name);
        window.set_widget(WindowWidget::ProgressBar(ProgressBar::new(
            0, 0, 0, 100, 20,
        )));
        let _ = window.hide(true);
    }

    fn create_multiplayer_slot_windows(wm: &mut WindowManager, prefix: &str, count: usize) {
        for slot in 0..count {
            named_progress_test_window(wm, &format!("{prefix}:ProgressLoad{slot}"));
            let suffixes = if load_screen_has_team_windows(prefix) {
                &["StaticTextPlayer", "StaticTextSide", "StaticTextTeam"][..]
            } else {
                &["StaticTextPlayer", "StaticTextSide"][..]
            };
            for suffix in suffixes {
                named_test_window(wm, &format!("{prefix}:{suffix}{slot}"));
            }
        }
    }

    fn create_multiplayer_start_position_windows(wm: &mut WindowManager, prefix: &str) {
        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            let name = format!("{prefix}:ButtonMapStartPosition{slot}");
            named_test_window(wm, &name);
            wm.find_window_by_name(&name)
                .expect("start position button")
                .borrow_mut()
                .set_size(10, 10)
                .expect("button size");
        }
    }

    fn create_gamespy_slot_windows(wm: &mut WindowManager, count: usize) {
        for slot in 0..count {
            named_test_window(wm, &format!("GameSpyLoadScreen.wnd:WinPlayer{slot}"));
            for suffix in gamespy_stats_suffixes() {
                named_test_window(wm, &format!("GameSpyLoadScreen.wnd:{suffix}{slot}"));
            }
        }
    }

    fn create_map_transfer_slot_windows(wm: &mut WindowManager, count: usize) {
        named_test_window(wm, "MapTransferScreen.wnd:StaticTextCurrentFile");
        named_test_window(wm, "MapTransferScreen.wnd:StaticTextTimeout");
        for slot in 0..count {
            named_progress_test_window(wm, &format!("MapTransferScreen.wnd:ProgressLoad{slot}"));
            named_test_window(wm, &format!("MapTransferScreen.wnd:StaticTextPlayer{slot}"));
            named_test_window(
                wm,
                &format!("MapTransferScreen.wnd:StaticTextProgress{slot}"),
            );
        }
    }

    fn load_screen_slot(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        load_screen_slot_with_color(player_name, side_name, team_number, None, is_ai, visible)
    }

    fn load_screen_slot_with_color(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        apparent_color: Option<i32>,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        load_screen_slot_with_text_color(
            player_name,
            side_name,
            team_number,
            apparent_color,
            None,
            is_ai,
            visible,
        )
    }

    fn load_screen_slot_with_text_color(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        apparent_color: Option<i32>,
        apparent_text_color: Option<u32>,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        LoadScreenSlotInitContext {
            player_id: team_number,
            player_name: player_name.to_string(),
            side_name: side_name.to_string(),
            team_number,
            apparent_color,
            apparent_text_color,
            is_ai,
            has_map: true,
            visible,
        }
    }

    fn load_screen_slot_with_map(
        player_id: i32,
        player_name: &str,
        apparent_text_color: Option<u32>,
        has_map: bool,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        let mut slot = load_screen_slot_with_text_color(
            player_name,
            "USA",
            player_id,
            None,
            apparent_text_color,
            is_ai,
            visible,
        );
        slot.player_id = player_id;
        slot.has_map = has_map;
        slot
    }

    fn window_text(wm: &WindowManager, name: &str) -> String {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_text()
            .to_string()
    }

    fn window_text_color(wm: &WindowManager, name: &str) -> u32 {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_text_color()
    }

    fn window_enabled_image_name(wm: &WindowManager, name: &str, index: usize) -> Option<String> {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_draw_data(index)
            .and_then(|draw| draw.image)
            .map(|image| image.name)
    }

    fn window_hidden(wm: &WindowManager, name: &str) -> bool {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .is_hidden()
    }

    fn window_position(wm: &WindowManager, name: &str) -> (i32, i32) {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_position()
    }

    fn window_image_name(wm: &WindowManager, name: &str, index: usize) -> Option<String> {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_draw_data(index)?
            .image
            .map(|image| image.name)
    }

    fn window_status(wm: &WindowManager, name: &str) -> WindowStatus {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_status()
    }

    fn progress_value(name: &str) -> Option<f32> {
        with_window_manager(|wm| {
            let window = wm.find_window_by_name(name)?;
            let mut window = window.borrow_mut();
            Some(window.progress_bar_mut()?.value())
        })
    }

    fn progress_fill_color(wm: &WindowManager, name: &str) -> crate::gui::gadgets::Color {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow_mut()
            .progress_bar_mut()
            .expect("progress widget")
            .fill_color()
    }

    fn reset_shell_game_first_load_for_tests(value: bool) {
        with_shell_game_first_load(|first_load| *first_load = value);
    }

    #[test]
    fn shell_game_first_load_matches_cpp_title_and_legal_state() {
        reset_shell_game_first_load_for_tests(true);
        clear_load_screen_presentation_pump();
        let presentation_calls = Arc::new(AtomicUsize::new(0));
        let hook_presentation_calls = Arc::clone(&presentation_calls);
        register_load_screen_presentation_pump(move || {
            hook_presentation_calls.fetch_add(1, Ordering::SeqCst);
        });

        let mut wm = WindowManager::new();
        let root = wm.create_window(None, 0, 0, 800, 600).expect("root");
        root.borrow_mut()
            .set_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen");
        named_test_window(&mut wm, "ShellGameLoadScreen.wnd:StaticTextLegal");
        named_test_window(&mut wm, "ShellGameLoadScreen.wnd:ProgressLoad");

        initialize_shell_game_windows(&mut wm, true);

        let root = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
            .expect("root");
        assert_eq!(
            root.borrow()
                .get_enabled_draw_data(0)
                .and_then(|draw| draw.image)
                .map(|image| image.name),
            Some("TitleScreen".to_string())
        );
        let legal = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
            .expect("legal");
        assert!(!legal.borrow().is_hidden());
        let progress = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:ProgressLoad")
            .expect("progress");
        assert!(!progress.borrow().is_hidden());
        assert_eq!(presentation_calls.load(Ordering::SeqCst), 1);

        let mut second_wm = WindowManager::new();
        let second_root = second_wm.create_window(None, 0, 0, 800, 600).expect("root");
        second_root
            .borrow_mut()
            .set_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen");
        named_test_window(&mut second_wm, "ShellGameLoadScreen.wnd:StaticTextLegal");

        initialize_shell_game_windows(&mut second_wm, true);

        let second_legal = second_wm
            .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
            .expect("legal");
        assert!(second_legal.borrow().is_hidden());
        reset_shell_game_first_load_for_tests(true);
        clear_load_screen_presentation_pump();
    }

    #[test]
    fn shell_game_first_load_skips_legal_intro_when_mem_check_fails_like_cpp() {
        reset_shell_game_first_load_for_tests(true);
        let mut wm = WindowManager::new();
        let root = wm.create_window(None, 0, 0, 800, 600).expect("root");
        root.borrow_mut()
            .set_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen");
        named_test_window(&mut wm, "ShellGameLoadScreen.wnd:StaticTextLegal");
        named_test_window(&mut wm, "ShellGameLoadScreen.wnd:ProgressLoad");

        initialize_shell_game_windows(&mut wm, false);

        let root = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
            .expect("root");
        assert!(root
            .borrow()
            .get_enabled_draw_data(0)
            .and_then(|draw| draw.image)
            .is_none());
        let legal = wm
            .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
            .expect("legal");
        assert!(legal.borrow().is_hidden());

        let mut later_wm = WindowManager::new();
        let later_root = later_wm.create_window(None, 0, 0, 800, 600).expect("root");
        later_root
            .borrow_mut()
            .set_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen");
        named_test_window(&mut later_wm, "ShellGameLoadScreen.wnd:StaticTextLegal");
        named_test_window(&mut later_wm, "ShellGameLoadScreen.wnd:ProgressLoad");

        initialize_shell_game_windows(&mut later_wm, true);

        assert_eq!(
            later_wm
                .find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
                .expect("root")
                .borrow()
                .get_enabled_draw_data(0)
                .and_then(|draw| draw.image)
                .map(|image| image.name),
            Some("TitleScreen".to_string())
        );
        assert!(!later_wm
            .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
            .expect("legal")
            .borrow()
            .is_hidden());
        reset_shell_game_first_load_for_tests(true);
    }

    fn challenge_test_windows(wm: &mut WindowManager) {
        named_test_window(wm, "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen");
        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
            .chain(
                [
                    "ChallengeLoadScreen.wnd:PortraitLeft",
                    "ChallengeLoadScreen.wnd:PortraitRight",
                    "ChallengeLoadScreen.wnd:PortraitMovieLeft",
                    "ChallengeLoadScreen.wnd:PortraitMovieRight",
                    "ChallengeLoadScreen.wnd:CircleAlphaOuter",
                    "ChallengeLoadScreen.wnd:CircleAlphaInner",
                    "ChallengeLoadScreen.wnd:VersusBackdrop",
                    "ChallengeLoadScreen.wnd:OverlayVs",
                ]
                .into_iter(),
            )
        {
            named_test_window(wm, name);
        }
    }

    fn setup_current_challenge_for_tests(movie_label: &str) {
        Language::clear_localized_strings();
        Language::register_localized_string("CHALLENGE:PlayerName", "General Player");
        Language::register_localized_string("CHALLENGE:PlayerRank", "General");
        Language::register_localized_string("CHALLENGE:PlayerStrategy", "Air superiority");
        Language::register_localized_string("CHALLENGE:OpponentName", "General Opponent");
        Language::register_localized_string("CHALLENGE:OpponentRank", "Prince");
        Language::register_localized_string("CHALLENGE:OpponentStrategy", "Ambush");

        init_challenge_generals();
        let mut generals = get_challenge_generals_mut().expect("challenge generals");
        let positions = generals.challenge_generals_mut();
        positions[0] = GeneralPersona::new();
        positions[0].set_campaign("challengecampaign".to_string());
        positions[0].set_bio_name("CHALLENGE:PlayerName".to_string());
        positions[0].set_bio_rank("CHALLENGE:PlayerRank".to_string());
        positions[0].set_bio_strategy("CHALLENGE:PlayerStrategy".to_string());
        positions[0].set_bio_portrait_large(Some("PlayerPortrait".to_string()));
        positions[0].set_portrait_movie_left_name("PlayerMovieLeft".to_string());
        positions[0].set_portrait_movie_right_name("PlayerMovieRight".to_string());
        positions[0].set_name_sound("PlayerNameSound".to_string());

        positions[1] = GeneralPersona::new();
        positions[1].set_bio_name("CHALLENGE:OpponentName".to_string());
        positions[1].set_bio_rank("CHALLENGE:OpponentRank".to_string());
        positions[1].set_bio_strategy("CHALLENGE:OpponentStrategy".to_string());
        positions[1].set_bio_portrait_large(Some("OpponentPortrait".to_string()));
        positions[1].set_portrait_movie_left_name("OpponentMovieLeft".to_string());
        positions[1].set_portrait_movie_right_name("OpponentMovieRight".to_string());
        positions[1].set_name_sound("OpponentNameSound".to_string());
        drop(generals);

        let mut manager = get_campaign_manager();
        {
            let campaign = manager.new_campaign("challengecampaign".to_string());
            campaign.first_mission = "mission1".to_string();
            campaign.is_challenge_campaign = true;
            let mission = campaign.new_mission("mission1".to_string());
            mission.general_name = "CHALLENGE:OpponentName".to_string();
            mission.movie_label = movie_label.to_string();
        }
        manager.set_campaign_and_mission("challengecampaign", "mission1");
    }

    fn cache_challenge_test_personas() {
        with_challenge_load_screen_state(|state| {
            *state = ChallengeLoadScreenState {
                player: Some(ChallengePersonaText {
                    big_name: "General Player".to_string(),
                    name: "General Player".to_string(),
                    rank: "General".to_string(),
                    strategy: "Air superiority".to_string(),
                    portrait_large: Some("PlayerPortrait".to_string()),
                    portrait_movie_left: "PlayerMovieLeft".to_string(),
                    portrait_movie_right: "PlayerMovieRight".to_string(),
                    name_sound: "PlayerNameSound".to_string(),
                    taunt_sounds: [
                        "PlayerTaunt1".to_string(),
                        "PlayerTaunt2".to_string(),
                        "PlayerTaunt3".to_string(),
                    ],
                }),
                opponent: Some(ChallengePersonaText {
                    big_name: "General Opponent".to_string(),
                    name: "General Opponent".to_string(),
                    rank: "Prince".to_string(),
                    strategy: "Ambush".to_string(),
                    portrait_large: Some("OpponentPortrait".to_string()),
                    portrait_movie_left: "OpponentMovieLeft".to_string(),
                    portrait_movie_right: "OpponentMovieRight".to_string(),
                    name_sound: "OpponentNameSound".to_string(),
                    taunt_sounds: [
                        "OpponentTaunt1".to_string(),
                        "OpponentTaunt2".to_string(),
                        "OpponentTaunt3".to_string(),
                    ],
                }),
                ..ChallengeLoadScreenState::default()
            };
        });
    }

    #[test]
    fn challenge_init_with_movie_waits_for_frame_activation_like_cpp_high_spec() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        register_challenge_movie_play_hook(|movie_name| movie_name == "ChallengeIntro");
        let frame = Arc::new(AtomicUsize::new(0));
        let hook_frame = Arc::clone(&frame);
        register_challenge_movie_advance_hook(move |movie_name| {
            assert_eq!(movie_name, "ChallengeIntro");
            Some(LoadScreenMovieAdvance {
                frame_index: hook_frame.fetch_add(1, Ordering::SeqCst) as i32 + 1,
                frame_count: 200,
                completed: false,
            })
        });
        setup_current_challenge_for_tests("ChallengeIntro");
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        initialize_challenge_windows(&mut wm, true);

        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
        {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }

        for _ in 0..FRAME_TITLES_START {
            let _ = advance_challenge_load_screen_prelude(&mut wm);
        }

        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }

        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_init_resets_window_video_manager_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        register_challenge_movie_play_hook(|_| false);
        setup_current_challenge_for_tests("ChallengeIntro");
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);
        with_window_video_manager(|manager| manager.set_global_flags_for_tests(true, true));

        initialize_challenge_windows(&mut wm, true);

        let flags = with_window_video_manager(|manager| manager.global_flags_for_tests());
        assert_eq!(flags, (false, false));
        assert_eq!(
            with_challenge_load_screen_state(|state| state.prelude_state),
            LoadScreenPreludeState::Failed
        );
        with_window_manager(|global_wm| *global_wm = wm);
        assert_eq!(
            run_load_screen_prelude_with_limit(LoadScreenKind::Challenge, 1),
            LoadScreenPreludeOutcome::Failed
        );
        update_load_screen(LoadScreenKind::Challenge, 100.0);
        let (postlude_audio_played, ambient_handle) = with_challenge_load_screen_state(|state| {
            (state.postlude_audio_played, state.ambient_loop_handle)
        });
        assert!(!postlude_audio_played);
        assert_eq!(ambient_handle, 0);
        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        reset_challenge_load_screen_audio_state();
        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_init_without_movie_returns_without_synthetic_reveal_like_cpp() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        setup_current_challenge_for_tests("");
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        initialize_challenge_windows(&mut wm, true);

        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }
        assert!(wm
            .find_window_by_name("ChallengeLoadScreen.wnd:BioStrategyEntryRight")
            .expect("right strategy")
            .borrow()
            .get_text()
            .is_empty());
        let (prelude_state, postlude_played, ambient_handle) =
            with_challenge_load_screen_state(|state| {
                (
                    state.prelude_state,
                    state.postlude_audio_played,
                    state.ambient_loop_handle,
                )
            });
        assert_eq!(prelude_state, LoadScreenPreludeState::Failed);
        assert!(!postlude_played);
        assert_eq!(ambient_handle, 0);

        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_prelude_driver_pumps_authored_frames_before_completion() {
        let _state_guard = lock_test_load_screen_state();
        let _language_guard = lock_test_language();
        reset_challenge_load_screen_audio_state();
        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        clear_load_screen_presentation_pump();
        Language::clear_localized_strings();

        register_challenge_movie_play_hook(|movie_name| movie_name == "ChallengeIntro");
        let advances = Arc::new(AtomicUsize::new(0));
        let hook_advances = Arc::clone(&advances);
        register_challenge_movie_advance_hook(move |movie_name| {
            assert_eq!(movie_name, "ChallengeIntro");
            let advance = hook_advances.fetch_add(1, Ordering::SeqCst);
            Some(if advance == 0 {
                LoadScreenMovieAdvance {
                    frame_index: FRAME_TITLES_START,
                    frame_count: 200,
                    completed: false,
                }
            } else {
                LoadScreenMovieAdvance {
                    frame_index: FRAME_RIGHT_VOICE,
                    frame_count: 200,
                    completed: true,
                }
            })
        });
        setup_current_challenge_for_tests("ChallengeIntro");

        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);
        initialize_challenge_windows(&mut wm, true);
        with_window_manager(|global_wm| *global_wm = wm);

        let presentation_pumps = Rc::new(Cell::new(0));
        let hook_pumps = Rc::clone(&presentation_pumps);
        register_load_screen_presentation_pump(move || {
            hook_pumps.set(hook_pumps.get() + 1);
        });

        assert_eq!(
            run_load_screen_prelude_with_limit(LoadScreenKind::Challenge, 8),
            LoadScreenPreludeOutcome::Complete
        );
        assert_eq!(advances.load(Ordering::SeqCst), 2);
        assert_eq!(presentation_pumps.get(), 2);
        with_window_manager(|wm| {
            for name in CHALLENGE_BIO_LABEL_WINDOWS {
                assert!(!window_hidden(wm, name), "{name}");
            }
        });
        let (prelude_state, current_frame, high_spec_active, postlude_audio_played, ambient_handle) =
            with_challenge_load_screen_state(|state| {
                (
                    state.prelude_state,
                    state.current_frame,
                    state.high_spec_prelude_active,
                    state.postlude_audio_played,
                    state.ambient_loop_handle,
                )
            });
        assert_eq!(prelude_state, LoadScreenPreludeState::Complete);
        assert_eq!(current_frame, FRAME_RIGHT_VOICE);
        assert!(!high_spec_active);
        assert!(postlude_audio_played);
        assert_eq!(ambient_handle, add_audio_event("LoadScreenAmbient"));

        clear_load_screen_presentation_pump();
        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        reset_challenge_load_screen_audio_state();
        Language::clear_localized_strings();
    }

    #[test]
    fn challenge_postlude_audio_fires_once_and_selects_opponent_taunt() {
        cache_challenge_test_personas();

        assert_eq!(
            challenge_taunt_sound(
                &with_challenge_load_screen_state(|state| state.opponent.clone().unwrap()),
                0
            ),
            Some("OpponentTaunt1")
        );
        assert_eq!(
            challenge_taunt_sound(
                &with_challenge_load_screen_state(|state| state.opponent.clone().unwrap()),
                4
            ),
            Some("OpponentTaunt2")
        );
        let sparse_taunts = ChallengePersonaText {
            taunt_sounds: [
                String::new(),
                "SparseOpponentTaunt2".to_string(),
                String::new(),
            ],
            ..ChallengePersonaText::default()
        };
        assert_eq!(challenge_taunt_sound(&sparse_taunts, 0), Some(""));
        assert_eq!(
            challenge_taunt_sound(&sparse_taunts, 1),
            Some("SparseOpponentTaunt2")
        );
        assert_eq!(challenge_taunt_sound(&sparse_taunts, 2), Some(""));

        finish_challenge_load_screen_audio_postlude();
        let first = with_challenge_load_screen_state(|state| {
            (
                state.postlude_audio_played,
                state.high_spec_prelude_active,
                state.ambient_loop_handle,
            )
        });
        assert!(first.0);
        assert!(!first.1);

        finish_challenge_load_screen_audio_postlude();
        let second = with_challenge_load_screen_state(|state| state.ambient_loop_handle);
        assert_eq!(second, first.2);
    }

    #[test]
    fn challenge_frame_activation_matches_cpp_teletype_gates() {
        cache_challenge_test_personas();
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TITLES_START);
        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }
        for name in CHALLENGE_BIO_ENTRY_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START);
        for name in CHALLENGE_BIO_ENTRY_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            let window = window.borrow();
            assert!(!window.is_hidden(), "{name}");
            assert_eq!(window.get_text(), "");
        }

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START + 1);
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioNameEntryLeft")
                .expect("left name")
                .borrow()
                .get_text(),
            ""
        );

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START + 2);
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioNameEntryLeft")
                .expect("left name")
                .borrow()
                .get_text(),
            "G"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioBirthplaceEntryRight")
                .expect("right rank")
                .borrow()
                .get_text(),
            "P"
        );
    }

    #[test]
    fn challenge_min_spec_activation_matches_cpp_final_reveal() {
        cache_challenge_test_personas();
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm);

        activate_challenge_pieces_min_spec_windows(&mut wm);

        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
            .chain(
                [
                    "ChallengeLoadScreen.wnd:PortraitLeft",
                    "ChallengeLoadScreen.wnd:PortraitRight",
                    "ChallengeLoadScreen.wnd:CircleAlphaOuter",
                    "ChallengeLoadScreen.wnd:CircleAlphaInner",
                    "ChallengeLoadScreen.wnd:VersusBackdrop",
                    "ChallengeLoadScreen.wnd:OverlayVs",
                ]
                .into_iter(),
            )
        {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }

        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BigNameEntryLeft")
                .expect("left big name")
                .borrow()
                .get_text(),
            "General Player"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioBirthplaceEntryRight")
                .expect("right rank")
                .borrow()
                .get_text(),
            "Prince"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioStrategyEntryRight")
                .expect("right strategy")
                .borrow()
                .get_text(),
            "Ambush"
        );

        let left_portrait = wm
            .find_window_by_name("ChallengeLoadScreen.wnd:PortraitLeft")
            .expect("left portrait");
        let left_portrait = left_portrait.borrow();
        assert_eq!(
            left_portrait
                .get_enabled_draw_data(0)
                .and_then(|draw| draw.image)
                .map(|image| image.name),
            Some("PlayerPortrait".to_string())
        );
    }
}
