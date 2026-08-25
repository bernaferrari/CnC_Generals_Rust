use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn with_campaign_start_global_data_restored(f: impl FnOnce()) {
        runtime_global_data::with_global_data_restored(|| {
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            let ini_snapshot = ini_global.read().clone();
            let campaign_difficulty = get_campaign_manager().get_game_difficulty();
            let challenge_difficulty =
                get_challenge_generals_mut().map(|generals| generals.current_difficulty());

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

            *ini_global.write() = ini_snapshot;
            get_campaign_manager().set_game_difficulty(campaign_difficulty);
            if let (Some(difficulty), Some(mut generals)) =
                (challenge_difficulty, get_challenge_generals_mut())
            {
                generals.set_current_difficulty(difficulty);
            }

            if let Err(payload) = result {
                std::panic::resume_unwind(payload);
            }
        });
    }

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    fn reset_main_menu_for_os_wnd() {
        let menu = get_main_menu();
        let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
        state.not_shown = true;
        state.campaign_selected = false;
        state.start_game = false;
        state.button_pushed = false;
        state.dont_allow_transitions = false;
        state.launch_challenge_menu = false;
        state.show_side = ShowSide::None;
        state.drop_down = DropdownType::None;
    }

    #[test]
    fn normal_campaign_start_mirrors_selected_map_to_both_global_data_residences() {
        with_campaign_start_global_data_restored(|| {
            let selected_map = "Maps\\Campaign\\MD_USA01.map";
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            ini_global.write().pending_file = "Maps\\Legacy\\Ini.map".to_string();
            runtime_global_data::write().pending_file = "Maps\\Legacy\\Runtime.map".to_string();

            let menu = MainMenu::new();
            let mut state = MainMenuState::default();
            menu.setup_game_start(&mut state, selected_map, GameDifficulty::Normal);

            assert!(state.start_game);
            assert_eq!(ini_global.read().pending_file, selected_map);
            assert_eq!(runtime_global_data::read().pending_file, selected_map);
        });
    }

    #[test]
    fn challenge_campaign_start_defers_map_publication_and_game_start() {
        with_campaign_start_global_data_restored(|| {
            let selected_map = "Maps\\Campaign\\ChallengeExpected.map";
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            ini_global.write().pending_file = "Maps\\Legacy\\Ini.map".to_string();
            runtime_global_data::write().pending_file = "Maps\\Legacy\\Runtime.map".to_string();

            let menu = MainMenu::new();
            let mut state = MainMenuState::default();
            state.launch_challenge_menu = true;
            menu.setup_game_start(&mut state, selected_map, GameDifficulty::Hard);

            assert!(state.campaign_selected);
            assert!(
                !state.start_game,
                "Challenge opens its selector before it can publish a map or start a match"
            );
            assert_eq!(ini_global.read().pending_file, "Maps\\Legacy\\Ini.map");
            assert_eq!(
                runtime_global_data::read().pending_file,
                "Maps\\Legacy\\Runtime.map"
            );
        });
    }

    fn drive_os_wnd_campaign_side(side: ShowSide) {
        reset_main_menu_for_os_wnd();
        install_named_button("MainMenu.wnd:ButtonSinglePlayer", 10, 10);
        let faction = match side {
            ShowSide::USA => "MainMenu.wnd:ButtonUSA",
            ShowSide::GLA => "MainMenu.wnd:ButtonGLA",
            ShowSide::China => "MainMenu.wnd:ButtonChina",
            _ => "MainMenu.wnd:ButtonUSA",
        };
        install_named_button(faction, 10, 40);
        install_named_button("MainMenu.wnd:ButtonMedium", 10, 70);
        {
            let mut manager = get_campaign_manager();
            manager.init();
        }
        assert!(
            drive_os_wnd_start_campaign_like_cpp(side, GameDifficulty::Normal),
            "OS WND {side:?} Medium must latch campaign start"
        );
        {
            let menu = get_main_menu();
            let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
            assert!(state.start_game);
            assert_eq!(state.show_side, side);
        }
    }

    #[test]
    fn os_wnd_start_usa_gla_china_campaign_hits_single_player_faction_medium() {
        drive_os_wnd_campaign_side(ShowSide::USA);
        let usa = get_campaign_manager().get_current_map().unwrap_or_default();
        if !usa.is_empty() {
            assert!(
                usa.to_ascii_lowercase().contains("md_usa"),
                "USA first mission map, got {usa}"
            );
        }
        drive_os_wnd_campaign_side(ShowSide::GLA);
        let gla = get_campaign_manager().get_current_map().unwrap_or_default();
        if !gla.is_empty() {
            assert!(
                gla.to_ascii_lowercase().contains("md_gla"),
                "GLA first mission map, got {gla}"
            );
        }
        drive_os_wnd_campaign_side(ShowSide::China);
        let china = get_campaign_manager().get_current_map().unwrap_or_default();
        if !china.is_empty() {
            assert!(
                china.to_ascii_lowercase().contains("md_chi")
                    || china.to_ascii_lowercase().contains("md_china"),
                "China first mission map, got {china}"
            );
        }
        assert!(!drive_os_wnd_start_campaign_like_cpp(
            ShowSide::None,
            GameDifficulty::Normal
        ));
    }

    #[test]
    fn os_wnd_start_usa_campaign_hits_single_player_usa_medium() {
        reset_main_menu_for_os_wnd();
        install_named_button("MainMenu.wnd:ButtonSinglePlayer", 10, 10);
        install_named_button("MainMenu.wnd:ButtonUSA", 10, 40);
        install_named_button("MainMenu.wnd:ButtonMedium", 10, 70);
        {
            let mut manager = get_campaign_manager();
            manager.init();
        }
        assert!(
            drive_os_wnd_start_usa_campaign_like_cpp(),
            "OS WND USA Medium must latch campaign start"
        );
        {
            let menu = get_main_menu();
            let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
            assert!(state.start_game);
            assert_eq!(state.show_side, ShowSide::USA);
        }
        let campaign = get_campaign_manager()
            .get_current_campaign()
            .map(|c| c.name.clone())
            .unwrap_or_default();
        assert!(
            campaign.eq_ignore_ascii_case("usa") || campaign.is_empty(),
            "setCampaign(USA) when Campaign.ini loaded, got {campaign}"
        );
        let map = get_campaign_manager().get_current_map().unwrap_or_default();
        if !map.is_empty() {
            assert!(
                map.to_ascii_lowercase().contains("md_usa"),
                "USA first mission map, got {map}"
            );
        }
        assert!(!drive_os_wnd_start_campaign_like_cpp(
            ShowSide::None,
            GameDifficulty::Normal
        ));
    }

    #[test]
    fn dispatch_os_click_named_window_requires_hit_test_for_used() {
        reset_os_wnd_widget_tree_nav_for_tests();
        assert!(
            !dispatch_os_click_named_window("MainMenu.wnd:MissingHitTest"),
            "missing gadget must not count as a click"
        );
        assert!(
            !last_os_wnd_widget_tree_click_ok(),
            "Used must not latch without get_window_under_cursor hit"
        );
        assert!(
            !os_wnd_widget_tree_nav_ok(),
            "sticky nav must not latch on a miss"
        );
    }

    #[test]
    fn test_main_menu_creation() {
        let mut menu = MainMenu::new();
        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());

        assert!(state.raise_message_boxes);
        assert!(!state.campaign_selected);
        assert!(!state.button_pushed);
        assert!(!state.is_shutting_down);
        assert!(!state.start_game);
        assert_eq!(state.drop_down, DropdownType::None);
        assert_eq!(state.show_side, ShowSide::None);
        assert!(!state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 0);
        assert_eq!(state.time_through_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn shutdown_with_immediate_userdata_completes_without_animation_like_cpp() {
        let mut menu = MainMenu::new();
        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.is_shutting_down = true;
            state.start_game = false;
        }

        let pop_immediate = true;
        menu.shutdown(&(), Some(&pop_immediate)).unwrap();

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(!state.is_shutting_down);
    }

    #[test]
    fn test_dropdown_type_conversion() {
        assert_eq!(DropdownType::from_i32(0), Some(DropdownType::None));
        assert_eq!(DropdownType::from_i32(1), Some(DropdownType::Single));
        assert_eq!(DropdownType::from_i32(2), Some(DropdownType::Multiplayer));
        assert_eq!(DropdownType::from_i32(3), Some(DropdownType::Main));
        assert_eq!(DropdownType::from_i32(4), Some(DropdownType::LoadReplay));
        assert_eq!(DropdownType::from_i32(5), Some(DropdownType::Difficulty));
        assert_eq!(DropdownType::from_i32(99), None);
    }

    #[test]
    fn test_show_side_conversion() {
        assert_eq!(ShowSide::from_i32(0), Some(ShowSide::None));
        assert_eq!(ShowSide::from_i32(1), Some(ShowSide::Training));
        assert_eq!(ShowSide::from_i32(2), Some(ShowSide::USA));
        assert_eq!(ShowSide::from_i32(3), Some(ShowSide::GLA));
        assert_eq!(ShowSide::from_i32(4), Some(ShowSide::China));
        assert_eq!(ShowSide::from_i32(5), Some(ShowSide::Skirmish));
        assert_eq!(ShowSide::from_i32(99), None);
    }

    #[test]
    fn test_display_settings() {
        let settings = DisplaySettings::default();
        assert_eq!(settings.x_res, 1024);
        assert_eq!(settings.y_res, 768);
        assert_eq!(settings.bit_depth, 32);
        assert!(!settings.windowed);
    }

    #[test]
    fn test_resolution_accept() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.new_disp_settings.x_res = 1920;
            state.new_disp_settings.y_res = 1080;
            state.disp_changed = true;
        }

        menu.accept_resolution();

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert_eq!(state.old_disp_settings.x_res, 1920);
        assert_eq!(state.old_disp_settings.y_res, 1080);
        assert!(!state.disp_changed);
    }

    #[test]
    fn test_resolution_rollback_state_restores_old_settings() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.old_disp_settings = DisplaySettings {
                x_res: 1280,
                y_res: 720,
                bit_depth: 32,
                windowed: true,
            };
            state.new_disp_settings = DisplaySettings {
                x_res: 1920,
                y_res: 1080,
                bit_depth: 32,
                windowed: false,
            };
            state.disp_changed = true;
        }

        let reverted = menu.rollback_resolution_state();

        assert_eq!(reverted.x_res, 1280);
        assert_eq!(reverted.y_res, 720);

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert_eq!(state.new_disp_settings.x_res, 1280);
        assert_eq!(state.new_disp_settings.y_res, 720);
        assert!(!state.disp_changed);
    }

    #[test]
    fn test_handle_canceled_download() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.button_pushed = true;
            state.drop_down = DropdownType::Difficulty;
            state.checking_for_patch_before_gamespy = true;
            state.cant_connect_before_online = true;
            state.checks_left_before_online = 4;
            state.online_cancel_window_open = true;
        }

        menu.handle_canceled_download(true);

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(!state.button_pushed);
        assert_eq!(state.drop_down, DropdownType::Difficulty);
        assert!(state.checking_for_patch_before_gamespy);
        assert!(state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 4);
        assert!(state.online_cancel_window_open);
    }

    #[test]
    fn test_cancel_patch_check_callback_clears_patch_state_and_window() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.button_pushed = true;
            state.drop_down = DropdownType::Difficulty;
            state.checking_for_patch_before_gamespy = true;
            state.cant_connect_before_online = true;
            state.checks_left_before_online = 4;
            state.online_cancel_window_open = true;
        }

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            MainMenu::cancel_patch_check_callback_state(&mut state);
        }

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(!state.button_pushed);
        assert_eq!(state.drop_down, DropdownType::Difficulty);
        assert!(!state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn test_initial_state() {
        let mut menu = MainMenu::new();
        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());

        assert_eq!(state.initial_gadget_delay, INITIAL_GADGET_DELAY_DEFAULT);
        assert!(state.not_shown);
        assert!(state.first_time_running_the_game);
        assert!(!state.show_logo);
        assert!(!state.logo_is_shown);
        assert!(!state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 0);
        assert_eq!(state.time_through_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn test_start_patch_check_sets_patch_state_without_cancelling_menu() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.button_pushed = true;
            state.dont_allow_transitions = true;
        }

        menu.start_patch_check();

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(state.button_pushed);
        assert!(state.dont_allow_transitions);
        assert!(state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 4);
        assert_eq!(state.time_through_online, 1);
        assert!(state.online_cancel_window_open);
    }

    #[test]
    fn test_input_focus_writeback_sets_keyboard_focus() {
        use crate::gui::{
            WindowMsgPayload, pop_payload, push_payload, write_input_focus_response as safe_focus,
        };

        // Production path: set_focus pushes a Bool payload token, never a raw ptr.
        let token = push_payload(WindowMsgPayload::Bool(false));
        let _ = safe_focus(1, token, true);
        assert_eq!(pop_payload(token), Some(WindowMsgPayload::Bool(true)));

        // Losing focus (data1 == 0) must not clobber payload.
        let lose = push_payload(WindowMsgPayload::Bool(false));
        let _ = safe_focus(0, lose, true);
        assert_eq!(pop_payload(lose), Some(WindowMsgPayload::Bool(false)));

        // Garbage data2 must not SIGSEGV (fail closed).
        let _ = safe_focus(1, 0xDEAD_BEEF, true);
    }

    #[test]
    fn test_patch_check_http_think_completes_handoff_after_four_ticks() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.checking_for_patch_before_gamespy = true;
            state.checks_left_before_online = 4;
            state.online_cancel_window_open = true;
        }

        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            menu.http_think_wrapper(&mut state);
            menu.http_think_wrapper(&mut state);
            menu.http_think_wrapper(&mut state);
            menu.http_think_wrapper(&mut state);
        }

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(!state.checking_for_patch_before_gamespy);
        assert_eq!(state.checks_left_before_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn test_reveal_hidden_main_menu_sets_cpp_startup_state() {
        let mut menu = MainMenu::new();
        let mut state = MainMenuState::default();

        menu.reveal_hidden_main_menu(&mut state);

        assert_eq!(state.initial_gadget_delay, 1);
        assert_eq!(state.drop_down, DropdownType::Main);
        assert!(!state.not_shown);
    }

    #[test]
    fn test_main_menu_init_unhides_startup_controls_like_cpp_layout_hide() {
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.pending_file.clear();
            global.shell_map_name = "ShellMapMD".to_string();
            global.shell_map_on = true;
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(gamelogic::system::game_logic::GAME_NONE);
        }
        {
            let message_stream = get_message_stream();
            message_stream
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .clear_messages();
        }
        show_shell_map_if_available(false);
        assert!(!get_shell().is_shell_map_on());

        let previous_first_time =
            FIRST_TIME_RUNNING_GAME.swap(false, std::sync::atomic::Ordering::SeqCst);

        let mut menu = MainMenu::new();
        let (layout, _info) = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/MainMenu.wnd")
                .expect("expected MainMenu.wnd to load")
        });

        let ids = build_window_ids();
        let get_update_id = ids.get_update_id as i32;
        let motd_id = ids.motd_id as i32;
        let map_pack_id = NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGetMapPack") as i32;

        let capture_hidden = |id: i32| {
            with_window_manager(|manager| {
                manager
                    .get_window_by_id(id)
                    .map(|window| window.borrow().is_hidden())
            })
        };

        let before_get_update = capture_hidden(get_update_id);
        let before_motd = capture_hidden(motd_id);
        let before_map_pack = capture_hidden(map_pack_id);
        assert_eq!(before_get_update, Some(true));
        assert_eq!(before_map_pack, Some(true));

        menu.init(&*layout.borrow(), None).unwrap();

        let after_get_update = capture_hidden(get_update_id);
        let after_motd = capture_hidden(motd_id);
        let after_map_pack = capture_hidden(map_pack_id);

        // C++ WindowLayout::hide walks info.windows (top-level only).
        // GetUpdate / GetMapPack are authored HIDDEN children of MainMenuParent.
        assert_eq!(after_get_update, Some(true));
        assert_eq!(after_motd, before_motd);
        assert_eq!(after_map_pack, Some(true));
        assert!(get_shell().is_shell_map_on());
        assert_eq!(
            get_global_data()
                .expect("global data initialized")
                .read()
                .pending_file,
            "ShellMapMD"
        );

        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.pending_file.clear();
            global.shell_map_on = false;
        }
        show_shell_map_if_available(false);

        FIRST_TIME_RUNNING_GAME.store(previous_first_time, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn test_input_focus_does_not_reveal_hidden_menu() {
        let mut menu = MainMenu::new();
        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.window_ids = build_window_ids();
            state.not_shown = true;
        }

        let handled = menu.system(build_window_ids().main_menu_id, GWM_INPUT_FOCUS, 1, 0);
        assert!(handled);

        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(state.not_shown);
        assert_eq!(state.drop_down, DropdownType::None);
    }

    #[test]
    fn test_mouse_hover_queues_cpp_shell_hooks() {
        let menu = MainMenu::new();
        let mut state = MainMenuState::default();
        state.window_ids.online_id = 11;
        state.window_ids.network_id = 12;
        state.window_ids.options_id = 13;
        state.window_ids.exit_id = 14;

        menu.handle_mouse_entering(&mut state, 11);
        menu.handle_mouse_entering(&mut state, 12);
        menu.handle_mouse_leaving(&mut state, 13);
        menu.handle_mouse_leaving(&mut state, 14);

        let hooks = state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            hooks,
            vec![
                "ShellMainMenuOnlineHighlighted",
                "ShellMainMenuNetworkHighlighted",
                "ShellMainMenuOptionsUnhighlighted",
                "ShellMainMenuExitUnhighlighted",
            ]
        );
    }

    #[test]
    fn test_mouse_hover_transient_logo_state_matches_cpp() {
        let menu = MainMenu::new();
        let mut state = MainMenuState::default();
        state.window_ids = build_window_ids();
        state.dont_allow_transitions = true;
        state.campaign_selected = false;
        let usa_id = state.window_ids.button_usa_id;

        menu.handle_mouse_entering(&mut state, usa_id);
        assert!(state.show_logo);
        assert_eq!(state.show_side, ShowSide::USA);

        menu.handle_mouse_leaving(&mut state, usa_id);
        assert!(!state.show_logo);
        assert_eq!(state.show_side, ShowSide::None);
    }

    #[test]
    fn test_selected_actions_queue_cpp_selected_hooks() {
        let menu = MainMenu::new();
        let ids = build_window_ids();

        let mut skirmish_state = MainMenuState::default();
        skirmish_state.window_ids = ids.clone();
        menu.handle_button_selected(&mut skirmish_state, ids.skirmish_id)
            .unwrap();
        let skirmish_hooks = skirmish_state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(skirmish_hooks, vec!["ShellMainMenuSkirmishPushed"]);

        let mut network_state = MainMenuState::default();
        network_state.window_ids = ids.clone();
        menu.handle_button_selected(&mut network_state, ids.network_id)
            .unwrap();
        let network_hooks = network_state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(network_hooks, vec!["ShellMainMenuNetworkPushed"]);

        let mut options_state = MainMenuState::default();
        options_state.window_ids = ids;
        let options_id = options_state.window_ids.options_id;
        menu.handle_button_selected(&mut options_state, options_id)
            .unwrap();
        let options_hooks = options_state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(options_hooks, vec!["ShellMainMenuOptionsPushed"]);
    }

    #[test]
    fn test_exit_selection_defers_button_latch_until_quit_confirmed() {
        let menu = MainMenu::new();
        let ids = build_window_ids();
        let mut state = MainMenuState::default();
        state.window_ids = ids.clone();

        menu.handle_button_selected(&mut state, ids.exit_id)
            .unwrap();

        assert!(!state.button_pushed);
        assert!(matches!(
            state.pending_actions.as_slice(),
            [PendingMainMenuAction::QuitRequest]
        ));
    }

    #[test]
    fn test_no_cd_retry_keeps_prompt_open_without_starting_campaign() {
        let mut menu = MainMenu::new();

        let result =
            menu.retry_campaign_start_after_cd_check_with_status(GameDifficulty::Normal, false);

        assert_eq!(result, ExMessageBoxReturnType::KeepOpen);
        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(!state.start_game);
        assert!(!state.dont_allow_transitions);
    }

    #[test]
    fn test_selected_ignores_clicks_when_button_pushed() {
        let menu = MainMenu::new();
        let ids = build_window_ids();
        let mut state = MainMenuState::default();
        state.window_ids = ids.clone();
        state.button_pushed = true;
        state.dont_allow_transitions = false;
        state.launch_challenge_menu = true;
        state.drop_down = DropdownType::Difficulty;

        menu.handle_button_selected(&mut state, ids.button_credits_id)
            .unwrap();

        assert!(state.button_pushed);
        assert!(!state.dont_allow_transitions);
        assert!(state.launch_challenge_menu);
        assert_eq!(state.drop_down, DropdownType::Difficulty);
        assert!(state.pending_actions.is_empty());
    }

    #[test]
    fn test_button_pushed_guard_blocks_non_network_pushes() {
        let menu = MainMenu::new();
        let ids = build_window_ids();

        for control_id in [
            ids.button_credits_id,
            ids.button_load_id,
            ids.button_replay_id,
            ids.skirmish_id,
        ] {
            let mut state = MainMenuState::default();
            state.window_ids = ids.clone();
            state.button_pushed = true;
            state.drop_down = DropdownType::Difficulty;

            menu.handle_button_selected(&mut state, control_id).unwrap();

            assert!(state.button_pushed);
            assert!(!state.dont_allow_transitions);
            assert!(!state.campaign_selected);
            assert_eq!(state.drop_down, DropdownType::Difficulty);
            assert!(state.pending_actions.is_empty());
        }
    }

    #[test]
    fn decline_resolution_pushes_main_menu_wnd_like_cpp() {
        // C++ DeclineResolution (MainMenu.cpp:739-750) deletes/recreates
        // TheShell then TheShell->push("Menus/MainMenu.wnd"). Pre-fix Rust
        // called shell.reset()+show_shell(true); show_shell only pushes when
        // shell map is OFF, so GAME_SHELL menus were left empty (hq-90rl).
        let src = include_str!("main_menu.rs");
        let start = src
            .find("pub fn decline_resolution")
            .expect("decline_resolution");
        let body = src[start..]
            .split("fn rollback_resolution_state")
            .next()
            .expect("decline_resolution body");
        assert!(
            body.contains("shell.push(\"Menus/MainMenu.wnd\""),
            "DeclineResolution must push Menus/MainMenu.wnd like C++: {body}"
        );
        assert!(
            !body.contains("shell.show_shell("),
            "DeclineResolution must not rely on show_shell (no-op when shell map on): {body}"
        );
    }
}

#[cfg(test)]
mod main_menu_shell_borrow_residual_tests {
    use super::*;

    #[test]
    fn main_menu_shutdown_nested_shell_borrow_residual() {
        let src = include_str!("main_menu.rs");
        assert!(
            src.contains("queue_shell_reverse_animate_window();"),
            "MainMenuShutdown must queue reverse animation while Shell::push owns the borrow"
        );
    }

    #[test]
    fn first_run_char_reveals_dropdown_like_cpp_main_menu_input() {
        {
            let mut menu = get_main_menu();
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.not_shown = true;
            state.window_ids = build_window_ids();
        }
        assert!(
            reveal_main_menu_first_input_like_cpp(),
            "C++ MainMenuInput GWM_CHAR must unhide DROPDOWN_MAIN"
        );
        let menu = get_main_menu();
        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(!state.not_shown);
        assert_eq!(state.drop_down, DropdownType::Main);
    }

    #[test]
    fn first_update_hides_overlapping_borders_then_char_reveal_hits_skirmish() {
        use crate::gui::window_manager::with_window_manager;

        // C++ MainMenuInit 525-530 hides every dropdown including MapBorder2
        // (DROPDOWN_MAIN). First-run keeps it hidden until GWM_CHAR
        // (MainMenu.cpp:982-1006) winHide(FALSE) MapBorder2 only.
        // Construct already-hidden (Init hide pack owns live retry).
        with_window_manager(|manager| {
            manager.reset();
            let ids = build_window_ids();
            let parent = manager
                .create_window(None, 0, 0, 800, 600)
                .expect("MainMenuParent");
            parent.borrow_mut().set_name("MainMenu.wnd:MainMenuParent");
            parent.borrow_mut().set_id(ids.main_menu_id as i32);

            let mut border2 = None;
            for name in [
                "MainMenu.wnd:MapBorder",
                "MainMenu.wnd:MapBorder1",
                "MainMenu.wnd:MapBorder2",
                "MainMenu.wnd:MapBorder3",
                "MainMenu.wnd:MapBorder4",
            ] {
                let border = manager
                    .create_window(Some(&parent), 0, 0, 800, 600)
                    .expect(name);
                border.borrow_mut().set_name(name);
                border
                    .borrow_mut()
                    .set_id(NameKeyGenerator::name_to_key(name) as i32);
                let _ = border.borrow_mut().hide(true);
                if name.ends_with("MapBorder2") {
                    border2 = Some(border);
                }
            }
            let border2 = border2.expect("MapBorder2");
            let skirmish = manager
                .create_window(Some(&border2), 540, 276, 208, 36)
                .expect("ButtonSkirmish");
            skirmish
                .borrow_mut()
                .set_name("MainMenu.wnd:ButtonSkirmish");
            skirmish.borrow_mut().set_id(ids.skirmish_id as i32);
            let _ = skirmish.borrow_mut().hide(false);
            let _ = skirmish.borrow_mut().enable(true);
        });

        {
            let menu = get_main_menu();
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.window_ids = build_window_ids();
            state.not_shown = true;
            state.drop_down = DropdownType::None;
        }

        with_window_manager(|manager| {
            let hidden = |name: &str| {
                manager
                    .find_window_by_name(name)
                    .map(|w| w.borrow().is_hidden())
                    .unwrap_or(false)
            };
            assert!(
                hidden("MainMenu.wnd:MapBorder2"),
                "hide pack keeps MapBorder2 hidden"
            );
            assert!(
                manager.get_window_under_cursor(644, 294, false).is_none()
                    || manager
                        .get_window_under_cursor(644, 294, false)
                        .map(|w| w.borrow().get_name() != "MainMenu.wnd:ButtonSkirmish")
                        .unwrap_or(true),
                "hidden MapBorder2 must make ButtonSkirmish unhittable"
            );
        });

        // C++ CHAR: winHide(FALSE) DROPDOWN_MAIN only. Skip transition_set_group.
        assert!(
            apply_first_run_dropdown_reveal_visibility_for_tests(),
            "CHAR reveal must unhide MapBorder2 without transition_set_group"
        );

        with_window_manager(|manager| {
            let border2 = manager
                .find_window_by_name("MainMenu.wnd:MapBorder2")
                .expect("MapBorder2");
            assert!(
                !border2.borrow().is_hidden(),
                "CHAR reveal must winHide(FALSE) MapBorder2 only"
            );
            let hit = manager
                .get_window_under_cursor(644, 294, false)
                .expect("skirmish hit after MapBorder2 winHide(FALSE)");
            assert_eq!(hit.borrow().get_name(), "MainMenu.wnd:ButtonSkirmish");
        });
    }

    #[test]
    fn first_run_char_reveal_makes_named_gadgets_hittable_at_640x480() {
        use crate::gui::window_manager::with_window_manager;

        with_window_manager(|manager| {
            manager.reset();
            manager.set_screen_size(640, 480);

            let parent = manager
                .create_window(None, 0, 0, 640, 480)
                .expect("MainMenuParent");
            parent.borrow_mut().set_name("MainMenu.wnd:MainMenuParent");

            // Unbound dropdown_windows: CHAR must still winHide by WND name.
            let mut border2 = None;
            for name in [
                "MainMenu.wnd:MapBorder",
                "MainMenu.wnd:MapBorder1",
                "MainMenu.wnd:MapBorder2",
                "MainMenu.wnd:MapBorder3",
                "MainMenu.wnd:MapBorder4",
            ] {
                let border = manager
                    .create_window(Some(&parent), 0, 0, 640, 480)
                    .expect(name);
                border.borrow_mut().set_name(name);
                let _ = border.borrow_mut().hide(true);
                if name.ends_with("MapBorder2") {
                    border2 = Some(border);
                }
            }
            let border2 = border2.expect("MapBorder2");

            // Retail 800x600 SP/Skirmish rects scaled to 640x480 sit at
            // 432,93-598,122 and 432,221-598,250. Place them under MapBorder2
            // so a hidden parent would steal the hit before CHAR reveal.
            let sp = manager
                .create_window(Some(&border2), 432, 93, 166, 29)
                .expect("ButtonSinglePlayer");
            sp.borrow_mut().set_name("MainMenu.wnd:ButtonSinglePlayer");
            let _ = sp.borrow_mut().hide(false);
            let _ = sp.borrow_mut().enable(true);

            let skirmish = manager
                .create_window(Some(&border2), 432, 221, 166, 29)
                .expect("ButtonSkirmish");
            skirmish
                .borrow_mut()
                .set_name("MainMenu.wnd:ButtonSkirmish");
            let _ = skirmish.borrow_mut().hide(false);
            let _ = skirmish.borrow_mut().enable(true);
        });

        {
            let menu = get_main_menu();
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.not_shown = true;
            state.drop_down = DropdownType::None;
            state.dropdown_windows.insert(DropdownType::Single, None);
            state
                .dropdown_windows
                .insert(DropdownType::Multiplayer, None);
            state.dropdown_windows.insert(DropdownType::Main, None);
            state
                .dropdown_windows
                .insert(DropdownType::LoadReplay, None);
            state
                .dropdown_windows
                .insert(DropdownType::Difficulty, None);
        }

        with_window_manager(|manager| {
            let name_at = |x: i32, y: i32| {
                manager
                    .get_window_under_cursor(x, y, false)
                    .map(|w| w.borrow().get_name().to_string())
            };
            let sp = name_at(515, 107);
            assert_ne!(
                sp.as_deref(),
                Some("MainMenu.wnd:ButtonSinglePlayer"),
                "hidden MapBorder2 must hide ButtonSinglePlayer before CHAR, got {sp:?}"
            );
            let sk = name_at(515, 235);
            assert_ne!(
                sk.as_deref(),
                Some("MainMenu.wnd:ButtonSkirmish"),
                "hidden MapBorder2 must hide ButtonSkirmish before CHAR, got {sk:?}"
            );
        });

        assert!(
            apply_first_run_dropdown_reveal_visibility_for_tests(),
            "CHAR visibility must unhide DROPDOWN_MAIN without transition_set_group"
        );

        with_window_manager(|manager| {
            let border2 = manager
                .find_window_by_name("MainMenu.wnd:MapBorder2")
                .expect("MapBorder2");
            assert!(
                !border2.borrow().is_hidden(),
                "CHAR reveal name-fallback must winHide(FALSE) MapBorder2"
            );

            let hit_sp = manager
                .get_window_under_cursor(515, 107, false)
                .expect("ButtonSinglePlayer hittable after first-run reveal");
            assert_eq!(
                hit_sp.borrow().get_name(),
                "MainMenu.wnd:ButtonSinglePlayer"
            );

            let hit_sk = manager
                .get_window_under_cursor(515, 235, false)
                .expect("ButtonSkirmish hittable after first-run reveal");
            assert_eq!(hit_sk.borrow().get_name(), "MainMenu.wnd:ButtonSkirmish");

            for name in [
                "MainMenu.wnd:ButtonSinglePlayer",
                "MainMenu.wnd:ButtonSkirmish",
            ] {
                let window = manager.find_window_by_name(name).expect(name);
                let guard = window.borrow();
                assert!(!guard.is_hidden(), "{name} must be visible");
                assert!(guard.is_enabled(), "{name} must be enabled");
                let (w, h) = guard.get_size();
                assert!(w > 0 && h > 0, "{name} must have positive size");
                let (sx, sy) = guard.get_screen_position();
                let cx = sx + w / 2;
                let cy = sy + h / 2;
                assert!(
                    cx >= 0 && cy >= 0 && cx < 640 && cy < 480,
                    "{name} center ({cx},{cy}) must be on 640x480"
                );
            }
        });
    }

    #[test]
    fn is_first_cd_present_matches_cpp_not_copy_protection() {
        let production = include_str!("main_menu.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or("");
        assert!(
            production.contains("crate::cd_check::is_first_cd_present()"),
            "MainMenu must use CDCheck IsFirstCDPresent, not launcher copy-protection"
        );
        assert!(
            !production.contains("comprehensive_validation")
                && !production.contains("copy_protection"),
            "MainMenu CD check must not call ProtectionStatus"
        );
        assert!(crate::cd_check::is_first_cd_present());
    }

    #[test]
    fn dont_allow_transitions_clears_when_handler_finished_like_cpp() {
        crate::gui::window_manager::with_window_manager(|manager| {
            manager.reset();
            assert!(
                manager.transitions_finished(),
                "C++ TheTransitionHandler->isFinished() is TRUE with no current group"
            );
        });

        let mut menu = MainMenu::new();
        {
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.dont_allow_transitions = true;
            state.just_entered = false;
            state.start_game = false;
            state.is_shutting_down = false;
        }
        menu.update(&(), None).unwrap();
        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(
            !state.dont_allow_transitions,
            "MainMenuUpdate clears dontAllowTransitions when TransitionHandler is finished"
        );
    }

    #[test]
    fn os_mouse_click_button_skirmish_bubbles_gbm_selected_like_cpp() {
        use crate::gui::WindowMessage;
        use crate::gui::gadgets::PushButton;
        use crate::gui::game_window::{
            GWS_MOUSE_TRACK, GWS_PUSH_BUTTON, WindowInputReturnCode, WindowWidget,
        };
        use crate::gui::window_manager::{
            dispatch_os_mouse_to_window_manager, with_window_manager,
        };
        use crate::gui::window_script::WindowDefinition;

        let parent_id = NameKeyGenerator::name_to_key("MainMenu.wnd:MainMenuParent") as i32;
        let border_id = NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder") as i32;
        let skirmish_id = NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSkirmish") as i32;

        {
            let mut menu = get_main_menu();
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.button_pushed = false;
            state.campaign_selected = false;
            state.dont_allow_transitions = false;
            state.not_shown = false;
            state.pending_actions.clear();
            state.window_ids = build_window_ids();
        }

        crate::gui::shell::get_shell().set_shell_active(true);
        with_window_manager(|manager| {
            manager.reset();
            let parent = manager
                .create_window_with_id(None, 0, 0, 400, 400, parent_id)
                .unwrap();
            manager.bind_window_callbacks(
                &mut parent.borrow_mut(),
                &WindowDefinition {
                    system_callback: "MainMenuSystem".to_string(),
                    input_callback: "MainMenuInput".to_string(),
                    ..WindowDefinition::default()
                },
            );

            let border = manager
                .create_window_with_id(Some(&parent), 0, 0, 200, 200, border_id)
                .unwrap();
            manager.bind_window_callbacks(
                &mut border.borrow_mut(),
                &WindowDefinition {
                    system_callback: "PassSelectedButtonsToParentSystem".to_string(),
                    input_callback: "[None]".to_string(),
                    ..WindowDefinition::default()
                },
            );

            let button_win = manager
                .create_window_with_id(Some(&border), 10, 10, 80, 30, skirmish_id)
                .unwrap();
            {
                let mut button_mut = button_win.borrow_mut();
                button_mut.instance_data_mut().style |= GWS_PUSH_BUTTON | GWS_MOUSE_TRACK;
                button_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                    skirmish_id as u32,
                    0,
                    0,
                    80,
                    30,
                )));
                manager.bind_window_callbacks(
                    &mut button_mut,
                    &WindowDefinition {
                        system_callback: "[None]".to_string(),
                        input_callback: "[None]".to_string(),
                        tooltip_callback: "[None]".to_string(),
                        draw_callback: "[None]".to_string(),
                        ..WindowDefinition::default()
                    },
                );
            }

            assert!(
                button_win.borrow().point_in_window(20, 20),
                "click must land on ButtonSkirmish"
            );
            let down = manager.process_mouse_event(WindowMessage::LeftDown, 20, 20, 0);
            assert_eq!(down, WindowInputReturnCode::Used);
            let up = manager.process_mouse_event(WindowMessage::LeftUp, 20, 20, 0);
            assert_eq!(up, WindowInputReturnCode::Used);
        });

        let wrapped = dispatch_os_mouse_to_window_manager(WindowMessage::LeftDown, 20, 20);
        assert_eq!(wrapped, WindowInputReturnCode::Used);

        let menu = get_main_menu();
        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(
            state.button_pushed && state.campaign_selected,
            "OS click must bubble GBM_SELECTED through PassSelectedButtonsToParentSystem into MainMenuSystem ButtonSkirmish"
        );
    }

    #[test]
    fn os_mouse_click_button_single_player_gbm_selected_unhides_map_border() {
        // C++ MainMenu.cpp:1313-1324 ButtonSinglePlayer GBM_SELECTED:
        // dropDownWindows[DROPDOWN_SINGLE]->winHide(FALSE) (MapBorder)
        // so SKIRMISH (a MapBorder child) is hittable. Physical click must
        // bubble through PassSelectedButtonsToParentSystem into MainMenuSystem.
        use crate::gui::WindowMessage;
        use crate::gui::gadgets::PushButton;
        use crate::gui::game_window::{
            GWS_MOUSE_TRACK, GWS_PUSH_BUTTON, WindowInputReturnCode, WindowStatus, WindowWidget,
        };
        use crate::gui::window_manager::with_window_manager;
        use crate::gui::window_script::WindowDefinition;

        let ids = build_window_ids();
        let parent_id = ids.main_menu_id as i32;
        let border_id = NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder") as i32;
        let border2_id = NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder2") as i32;
        let solo_id = ids.button_single_player_id as i32;
        let skirmish_id = ids.skirmish_id as i32;

        {
            let mut menu = get_main_menu();
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.button_pushed = false;
            state.campaign_selected = false;
            state.dont_allow_transitions = false;
            state.not_shown = false;
            state.pending_actions.clear();
            state.window_ids = ids.clone();
            state.drop_down = DropdownType::Main;
        }

        crate::gui::shell::get_shell().set_shell_active(true);
        with_window_manager(|manager| {
            manager.reset();
            manager.set_screen_size(800, 600);

            let parent = manager
                .create_window_with_id(None, 0, 0, 800, 600, parent_id)
                .unwrap();
            {
                let mut parent_mut = parent.borrow_mut();
                parent_mut.set_name("MainMenu.wnd:MainMenuParent");
                parent_mut.set_status(WindowStatus::NO_INPUT);
                manager.bind_window_callbacks(
                    &mut parent_mut,
                    &WindowDefinition {
                        system_callback: "MainMenuSystem".to_string(),
                        input_callback: "MainMenuInput".to_string(),
                        ..WindowDefinition::default()
                    },
                );
            }

            let border2 = manager
                .create_window_with_id(Some(&parent), 0, 0, 800, 600, border2_id)
                .unwrap();
            {
                let mut border2_mut = border2.borrow_mut();
                border2_mut.set_name("MainMenu.wnd:MapBorder2");
                manager.bind_window_callbacks(
                    &mut border2_mut,
                    &WindowDefinition {
                        system_callback: "PassSelectedButtonsToParentSystem".to_string(),
                        input_callback: "[None]".to_string(),
                        ..WindowDefinition::default()
                    },
                );
            }
            let _ = border2.borrow_mut().hide(false);

            let solo = manager
                .create_window_with_id(Some(&border2), 540, 116, 208, 36, solo_id)
                .unwrap();
            {
                let mut solo_mut = solo.borrow_mut();
                solo_mut.set_name("MainMenu.wnd:ButtonSinglePlayer");
                solo_mut.instance_data_mut().style |= GWS_PUSH_BUTTON | GWS_MOUSE_TRACK;
                solo_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                    solo_id as u32,
                    0,
                    0,
                    208,
                    36,
                )));
                manager.bind_window_callbacks(
                    &mut solo_mut,
                    &WindowDefinition {
                        system_callback: "[None]".to_string(),
                        input_callback: "[None]".to_string(),
                        tooltip_callback: "[None]".to_string(),
                        draw_callback: "[None]".to_string(),
                        ..WindowDefinition::default()
                    },
                );
            }
            let _ = solo.borrow_mut().hide(false);
            let _ = solo.borrow_mut().enable(true);

            let border = manager
                .create_window_with_id(Some(&parent), 0, 0, 800, 600, border_id)
                .unwrap();
            {
                let mut border_mut = border.borrow_mut();
                border_mut.set_name("MainMenu.wnd:MapBorder");
                manager.bind_window_callbacks(
                    &mut border_mut,
                    &WindowDefinition {
                        system_callback: "PassSelectedButtonsToParentSystem".to_string(),
                        input_callback: "[None]".to_string(),
                        ..WindowDefinition::default()
                    },
                );
            }
            let _ = border.borrow_mut().hide(true);

            let skirmish = manager
                .create_window_with_id(Some(&border), 540, 276, 208, 36, skirmish_id)
                .unwrap();
            {
                let mut skirmish_mut = skirmish.borrow_mut();
                skirmish_mut.set_name("MainMenu.wnd:ButtonSkirmish");
                skirmish_mut.instance_data_mut().style |= GWS_PUSH_BUTTON | GWS_MOUSE_TRACK;
                skirmish_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                    skirmish_id as u32,
                    0,
                    0,
                    208,
                    36,
                )));
                manager.bind_window_callbacks(
                    &mut skirmish_mut,
                    &WindowDefinition {
                        system_callback: "[None]".to_string(),
                        input_callback: "[None]".to_string(),
                        tooltip_callback: "[None]".to_string(),
                        draw_callback: "[None]".to_string(),
                        ..WindowDefinition::default()
                    },
                );
            }
            let _ = skirmish.borrow_mut().hide(false);
            let _ = skirmish.borrow_mut().enable(true);

            assert!(
                solo.borrow().point_in_window(644, 134),
                "click must land on ButtonSinglePlayer"
            );
            let down = manager.process_mouse_event(WindowMessage::LeftDown, 644, 134, 0);
            assert_eq!(down, WindowInputReturnCode::Used);
            let up = manager.process_mouse_event(WindowMessage::LeftUp, 644, 134, 0);
            assert_eq!(up, WindowInputReturnCode::Used);
        });

        {
            let menu = get_main_menu();
            let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
            assert_eq!(
                state.drop_down,
                DropdownType::Single,
                "OS click must bubble GBM_SELECTED through PassSelectedButtonsToParentSystem into MainMenuSystem ButtonSinglePlayer"
            );
            assert!(
                !state.button_pushed,
                "C++ ButtonSinglePlayer sets buttonPushed = FALSE"
            );
        }

        with_window_manager(|manager| {
            let border = manager
                .find_window_by_name("MainMenu.wnd:MapBorder")
                .expect("MapBorder");
            assert!(
                !border.borrow().is_hidden(),
                "C++ 1320 dropDownWindows[DROPDOWN_SINGLE]->winHide(FALSE)"
            );
            let border2 = manager
                .find_window_by_name("MainMenu.wnd:MapBorder2")
                .expect("MapBorder2");
            assert!(
                border2.borrow().is_hidden(),
                "SOLO PLAY must hide MapBorder2 so SKIRMISH is hittable"
            );

            let hit = manager.get_window_under_cursor(644, 294, false);
            let hit_name = hit.as_ref().map(|w| w.borrow().get_name().to_string());
            assert!(
                hit_name.as_deref() == Some("MainMenu.wnd:ButtonSkirmish")
                    || hit_name.as_deref() == Some("MainMenu.wnd:MapBorder"),
                "after ButtonSinglePlayer GBM_SELECTED, SKIRMISH or MapBorder must be hittable, got {hit_name:?}"
            );
        });
    }

    #[test]
    fn simulate_main_menu_skirmish_button_gadget_selected_latches() {
        // Fresh residual call should fire ButtonSkirmish GBM_SELECTED latch.
        assert!(
            simulate_main_menu_skirmish_button_gadget_selected(),
            "ButtonSkirmish residual must latch button_pushed+campaign_selected"
        );
        // Second call resets latches internally and re-fires.
        assert!(simulate_main_menu_skirmish_button_gadget_selected());
        let menu = get_main_menu();
        let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
        assert!(state.button_pushed);
        assert!(state.campaign_selected);
    }

    #[test]
    fn cpp_init_hide_pack_hides_overlapping_dropdowns_ruler_and_recent_save() {
        // C++ MainMenuInit 525-530 winHide(TRUE) dropDownWindows[1..],
        // initialHide, showSelectiveButtons(SHOW_NONE), then 627-629
        // MainMenu.wnd:MainMenuRuler. Dummy WM lookups must not latch.
        use crate::gui::game_window::WindowStatus;
        use crate::gui::window_manager::with_window_manager;

        with_window_manager(|manager| {
            manager.reset();
            let ids = build_window_ids();
            let parent = manager
                .create_window(None, 0, 0, 800, 600)
                .expect("MainMenuParent");
            {
                let mut parent = parent.borrow_mut();
                parent.set_name("MainMenu.wnd:MainMenuParent");
                parent.set_id(ids.main_menu_id as i32);
                // Isolate child hide: parent must not steal hits after children hide.
                parent.set_status(WindowStatus::NO_INPUT);
            }

            for name in [
                "MainMenu.wnd:MapBorder",
                "MainMenu.wnd:MapBorder1",
                "MainMenu.wnd:MapBorder2",
                "MainMenu.wnd:MapBorder3",
                "MainMenu.wnd:MapBorder4",
            ] {
                let border = manager
                    .create_window(Some(&parent), 0, 0, 800, 600)
                    .expect(name);
                border.borrow_mut().set_name(name);
                border
                    .borrow_mut()
                    .set_id(NameKeyGenerator::name_to_key(name) as i32);
                let _ = border.borrow_mut().hide(false);
            }

            let border2 = manager
                .find_window_by_name("MainMenu.wnd:MapBorder2")
                .expect("MapBorder2");
            let solo = manager
                .create_window(Some(&border2), 540, 116, 208, 36)
                .expect("ButtonSinglePlayer");
            solo.borrow_mut()
                .set_name("MainMenu.wnd:ButtonSinglePlayer");
            let _ = solo.borrow_mut().hide(false);
            let _ = solo.borrow_mut().enable(true);

            let border = manager
                .find_window_by_name("MainMenu.wnd:MapBorder")
                .expect("MapBorder");
            let skirmish = manager
                .create_window(Some(&border), 540, 276, 208, 36)
                .expect("ButtonSkirmish");
            skirmish
                .borrow_mut()
                .set_name("MainMenu.wnd:ButtonSkirmish");
            let _ = skirmish.borrow_mut().hide(false);
            let _ = skirmish.borrow_mut().enable(true);

            let recent = manager
                .create_window(Some(&parent), 440, 104, 92, 24)
                .expect("ButtonUSARecentSave");
            recent
                .borrow_mut()
                .set_name("MainMenu.wnd:ButtonUSARecentSave");
            let _ = recent.borrow_mut().hide(false);
            let _ = recent.borrow_mut().enable(true);

            let ruler = manager
                .create_window(Some(&parent), 0, 0, 800, 600)
                .expect("MainMenuRuler");
            ruler.borrow_mut().set_name("MainMenu.wnd:MainMenuRuler");
            let _ = ruler.borrow_mut().hide(false);
        });

        {
            let menu = get_main_menu();
            let mut state = menu.state.write().unwrap_or_else(|e| e.into_inner());
            state.window_ids = build_window_ids();
            state.just_entered = true;
            state.init_dropdowns_hidden = false;
            state.not_shown = true;
            state.drop_down = DropdownType::None;
            state.initial_gadget_delay = 2;
        }

        let dummy = ();
        let _ = get_main_menu().update(&dummy, None);

        {
            let menu = get_main_menu();
            let state = menu.state.read().unwrap_or_else(|e| e.into_inner());
            assert!(
                state.init_dropdowns_hidden,
                "hide pack must latch only after live MapBorder2 was hidden"
            );
        }

        with_window_manager(|manager| {
            let border2 = manager
                .find_window_by_name("MainMenu.wnd:MapBorder2")
                .expect("MapBorder2");
            assert!(
                border2.borrow().is_hidden(),
                "C++ dropDownWindows[DROPDOWN_MAIN] winHide(TRUE)"
            );
            let ruler = manager
                .find_window_by_name("MainMenu.wnd:MainMenuRuler")
                .expect("MainMenuRuler");
            assert!(
                ruler.borrow().is_hidden(),
                "C++ MainMenuInit 627-629 hides MainMenuRuler"
            );
            let recent = manager
                .find_window_by_name("MainMenu.wnd:ButtonUSARecentSave")
                .expect("RecentSave");
            assert!(
                recent.borrow().is_hidden(),
                "C++ showSelectiveButtons(SHOW_NONE) hides RecentSave"
            );

            // Retail 800x600 centers: SOLO PLAY 644,134; Skirmish 644,294;
            // RecentSave 486,116.
            for (x, y, label) in [
                (644, 134, "SOLO PLAY"),
                (644, 294, "Skirmish"),
                (486, 116, "RecentSave"),
            ] {
                let hit = manager.get_window_under_cursor(x, y, false);
                assert!(
                    hit.is_none(),
                    "after C++ hide pack, {label} center ({x},{y}) must not hit {:?}",
                    hit.as_ref().map(|w| w.borrow().get_name().to_string())
                );
            }
        });
    }

    #[test]
    fn show_only_dropdown_single_unhides_map_border_hides_map_border2() {
        use crate::gui::window_manager::with_window_manager;

        with_window_manager(|manager| {
            manager.reset();
            let parent = manager
                .create_window(None, 0, 0, 800, 600)
                .expect("MainMenuParent");
            parent.borrow_mut().set_name("MainMenu.wnd:MainMenuParent");
            for name in ["MainMenu.wnd:MapBorder", "MainMenu.wnd:MapBorder2"] {
                let border = manager
                    .create_window(Some(&parent), 0, 0, 800, 600)
                    .expect(name);
                border.borrow_mut().set_name(name);
                // Start both visible so hide must run on the live manager.
                let _ = border.borrow_mut().hide(false);
            }
        });

        {
            let menu = MainMenu::new();
            let state = MainMenuState::default();
            menu.show_only_dropdown(&state, DropdownType::Single);
        }

        with_window_manager(|manager| {
            let hidden = |name: &str| {
                manager
                    .find_window_by_name(name)
                    .map(|w| w.borrow().is_hidden())
                    .expect(name)
            };
            assert!(
                !hidden("MainMenu.wnd:MapBorder"),
                "show_only_dropdown(Single) must winHide(FALSE) MapBorder"
            );
            assert!(
                hidden("MainMenu.wnd:MapBorder2"),
                "show_only_dropdown(Single) must winHide(TRUE) MapBorder2"
            );
        });
    }

    #[test]
    fn show_only_dropdown_multiplayer_unhides_map_border1_hides_map_border2() {
        // C++ MainMenu.cpp:1369-1376 ButtonMultiplayer:
        // dropDownWindows[DROPDOWN_MULTIPLAYER]->winHide(FALSE) (MapBorder1)
        // after MainMenuInit hid MapBorder2 (DROPDOWN_MAIN).
        use crate::gui::window_manager::with_window_manager;

        with_window_manager(|manager| {
            manager.reset();
            let parent = manager
                .create_window(None, 0, 0, 800, 600)
                .expect("MainMenuParent");
            parent.borrow_mut().set_name("MainMenu.wnd:MainMenuParent");
            for name in ["MainMenu.wnd:MapBorder1", "MainMenu.wnd:MapBorder2"] {
                let border = manager
                    .create_window(Some(&parent), 0, 0, 800, 600)
                    .expect(name);
                border.borrow_mut().set_name(name);
                // Start both visible so hide must run on the live manager.
                let _ = border.borrow_mut().hide(false);
            }
        });

        {
            let menu = MainMenu::new();
            let state = MainMenuState::default();
            menu.show_only_dropdown(&state, DropdownType::Multiplayer);
        }

        with_window_manager(|manager| {
            let hidden = |name: &str| {
                manager
                    .find_window_by_name(name)
                    .map(|w| w.borrow().is_hidden())
                    .expect(name)
            };
            assert!(
                !hidden("MainMenu.wnd:MapBorder1"),
                "show_only_dropdown(Multiplayer) must winHide(FALSE) MapBorder1"
            );
            assert!(
                hidden("MainMenu.wnd:MapBorder2"),
                "show_only_dropdown(Multiplayer) must winHide(TRUE) MapBorder2"
            );
        });
    }

    fn install_named_map_borders(names: &[&str], hidden: bool) {
        use crate::gui::window_manager::with_window_manager;
        with_window_manager(|manager| {
            manager.reset();
            let parent = manager
                .create_window(None, 0, 0, 800, 600)
                .expect("MainMenuParent");
            parent.borrow_mut().set_name("MainMenu.wnd:MainMenuParent");
            for name in names {
                let border = manager
                    .create_window(Some(&parent), 0, 0, 800, 600)
                    .expect(*name);
                border.borrow_mut().set_name(*name);
                let _ = border.borrow_mut().hide(hidden);
            }
        });
    }

    fn border_hidden(name: &str) -> bool {
        use crate::gui::window_manager::with_window_manager;
        with_window_manager(|manager| {
            manager
                .find_window_by_name(name)
                .map(|w| w.borrow().is_hidden())
                .expect(name)
        })
    }

    fn ready_menu_for_gbm() -> (MainMenu, MainMenuState) {
        let menu = MainMenu::new();
        let mut state = MainMenuState::default();
        state.window_ids = build_window_ids();
        state.button_pushed = false;
        state.dont_allow_transitions = false;
        state.campaign_selected = false;
        state.pending_actions.clear();
        (menu, state)
    }

    #[test]
    fn handle_button_selected_remaining_screens_winhide_or_push_like_cpp() {
        // C++ MainMenu.cpp GBM_SELECTED:
        // MULTIPLAYER 1369-1376 MapBorder1; LOAD 1381-1391 MapBorder3;
        // Back 1326-1356 MapBorder2; OPTIONS 1470-1483 OptionsMenu.wnd;
        // SKIRMISH 1420-1441 SkirmishGameOptionsMenu.wnd.
        const BORDERS: &[&str] = &[
            "MainMenu.wnd:MapBorder",
            "MainMenu.wnd:MapBorder1",
            "MainMenu.wnd:MapBorder2",
            "MainMenu.wnd:MapBorder3",
        ];

        install_named_map_borders(BORDERS, false);
        let (menu, mut state) = ready_menu_for_gbm();
        let multi_id = state.window_ids.button_multi_player_id;
        menu.handle_button_selected(&mut state, multi_id)
            .expect("ButtonMultiplayer");
        assert_eq!(state.drop_down, DropdownType::Multiplayer);
        assert!(
            !border_hidden("MainMenu.wnd:MapBorder1"),
            "MULTIPLAYER must winHide(FALSE) MapBorder1"
        );
        assert!(
            border_hidden("MainMenu.wnd:MapBorder2"),
            "MULTIPLAYER must winHide(TRUE) MapBorder2"
        );

        install_named_map_borders(BORDERS, false);
        state.dont_allow_transitions = false;
        state.button_pushed = false;
        let load_id = state.window_ids.button_load_replay_id;
        menu.handle_button_selected(&mut state, load_id)
            .expect("ButtonLoadReplay");
        assert_eq!(state.drop_down, DropdownType::LoadReplay);
        assert!(
            !border_hidden("MainMenu.wnd:MapBorder3"),
            "LOAD must winHide(FALSE) MapBorder3"
        );
        assert!(
            border_hidden("MainMenu.wnd:MapBorder2"),
            "LOAD must winHide(TRUE) MapBorder2"
        );

        install_named_map_borders(BORDERS, true);
        state.dont_allow_transitions = false;
        state.button_pushed = false;
        let back_id = state.window_ids.button_multi_back_id;
        menu.handle_button_selected(&mut state, back_id)
            .expect("ButtonMultiBack");
        assert_eq!(state.drop_down, DropdownType::Main);
        assert!(
            !border_hidden("MainMenu.wnd:MapBorder2"),
            "Back must winHide(FALSE) MapBorder2"
        );

        state.dont_allow_transitions = false;
        state.button_pushed = false;
        state.pending_actions.clear();
        let options_id = state.window_ids.options_id;
        menu.handle_button_selected(&mut state, options_id)
            .expect("ButtonOptions");
        assert!(
            state
                .pending_actions
                .iter()
                .any(|a| matches!(a, PendingMainMenuAction::ShowOptionsLayout)),
            "OPTIONS must queue getOptionsLayout(OptionsMenu.wnd)"
        );

        state.dont_allow_transitions = false;
        state.button_pushed = false;
        state.campaign_selected = false;
        state.pending_actions.clear();
        let skirmish_id = state.window_ids.skirmish_id;
        menu.handle_button_selected(&mut state, skirmish_id)
            .expect("ButtonSkirmish");
        assert!(
            state.pending_actions.iter().any(|a| matches!(
                a,
                PendingMainMenuAction::PushShellScreen("Menus/SkirmishGameOptionsMenu.wnd")
            )),
            "SKIRMISH must queue PushShellScreen(SkirmishGameOptionsMenu.wnd)"
        );
    }
}
