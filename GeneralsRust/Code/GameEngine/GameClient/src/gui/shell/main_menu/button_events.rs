use super::*;

impl MainMenu {
    /// Handle button selected
    /// Port of GBM_SELECTED handler - C++ lines 1279-1677
    pub(super) fn handle_button_selected(
        state: &mut MainMenuState,
        control_id: u32,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) -> MainMenuResult<()> {
        if state.button_pushed {
            return Ok(());
        }

        // Don't allow mouse click slop during transitions - matches C++ lines 1304-1310
        if control_id != state.window_ids.button_easy_id
            && control_id != state.window_ids.button_medium_id
            && control_id != state.window_ids.button_hard_id
            && Self::transitions_finished(effects)
        {
            state.launch_challenge_menu = false;
        }

        // Handle each button type - matches C++ lines 1313-1673
        if control_id == state.window_ids.button_single_player_id {
            // Single Player button - C++ lines 1313-1324
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Single;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Single,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuDefaultMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse("MainMenuDefaultMenuBack", effects, selected.as_deref_mut());
            Self::transition_set_group(
                "MainMenuSinglePlayerMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Single Player button selected");
        } else if control_id == state.window_ids.button_single_back_id {
            // Single Player Back button - C++ lines 1326-1335
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.button_pushed = false;
            state.drop_down = DropdownType::Main;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Main,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuSinglePlayerMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuSinglePlayerMenuBack",
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDefaultMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            state.dont_allow_transitions = true;
            log::info!("Single Player Back button selected");
        } else if control_id == state.window_ids.button_multi_back_id {
            // Multiplayer Back button - C++ lines 1336-1346
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Main;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Main,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuMultiPlayerMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuMultiPlayerMenuReverse",
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDefaultMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Multiplayer Back button selected");
        } else if control_id == state.window_ids.button_load_replay_back_id {
            // Load Replay Back button - C++ lines 1347-1357
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Main;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Main,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuLoadReplayMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuLoadReplayMenuBack",
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDefaultMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Load Replay Back button selected");
        } else if control_id == state.window_ids.button_credits_id {
            // Credits button - C++ lines 1359-1368
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::Main;
            Self::queue_action(state, PendingMainMenuAction::PushCreditsThenRevealMain);
            log::info!("Credits button selected");
        } else if control_id == state.window_ids.button_multi_player_id {
            // Multiplayer button - C++ lines 1369-1380
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Multiplayer;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Multiplayer,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuDefaultMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse("MainMenuDefaultMenuBack", effects, selected.as_deref_mut());
            Self::transition_set_group(
                "MainMenuMultiPlayerMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Multiplayer button selected");
        } else if control_id == state.window_ids.button_load_replay_id {
            // Load Replay button - C++ lines 1381-1392
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::LoadReplay;
            Self::set_dropdown_hidden(
                state,
                DropdownType::LoadReplay,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuDefaultMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse("MainMenuDefaultMenuBack", effects, selected.as_deref_mut());
            Self::transition_set_group(
                "MainMenuLoadReplayMenu",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Load Replay button selected");
        } else if control_id == state.window_ids.button_load_id {
            // Load Game button - C++ lines 1393-1409
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::LoadReplay;
            Self::set_dropdown_hidden(
                state,
                DropdownType::LoadReplay,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuLoadReplayMenuBackTransition",
                effects,
                selected.as_deref_mut(),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/SaveLoad.wnd"),
            );
            log::info!("Load Game button selected");
        } else if control_id == state.window_ids.button_replay_id {
            // Replay button - C++ lines 1410-1419
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::LoadReplay;
            Self::set_dropdown_hidden(
                state,
                DropdownType::LoadReplay,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuLoadReplayMenuBackTransition",
                effects,
                selected.as_deref_mut(),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/ReplayMenu.wnd"),
            );
            log::info!("Replay button selected");
        } else if control_id == state.window_ids.skirmish_id {
            // Skirmish button - C++ lines 1420-1443
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.button_pushed = true;
            state.campaign_selected = true;
            state.drop_down = DropdownType::Single;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Single,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuFactionSkirmish",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuSinglePlayerMenuBackSkirmish",
                effects,
                selected.as_deref_mut(),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/SkirmishGameOptionsMenu.wnd"),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::SignalUiInteract(
                    THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED as usize],
                ),
            );
            log::info!("Skirmish button selected");
        } else if control_id == state.window_ids.online_id {
            // Online button - C++ lines 1444-1457
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::Multiplayer;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Multiplayer,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuMultiPlayerMenuTransitionToNext",
                effects,
                selected.as_deref_mut(),
            );
            Self::queue_action(state, PendingMainMenuAction::StartPatchCheck);
            state.drop_down = DropdownType::None;
            log::info!("Online button selected");
        } else if control_id == state.window_ids.network_id {
            // Network button - C++ lines 1458-1469
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::Multiplayer;
            Self::set_dropdown_hidden(
                state,
                DropdownType::Multiplayer,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuMultiPlayerMenuTransitionToNext",
                effects,
                selected.as_deref_mut(),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/LanLobbyMenu.wnd"),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::SignalUiInteract(
                    THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED as usize],
                ),
            );
            log::info!("Network button selected");
        } else if control_id == state.window_ids.options_id {
            // Options button - C++ lines 1470-1484
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            Self::queue_action(
                state,
                PendingMainMenuAction::SignalUiInteract(
                    THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED as usize],
                ),
            );
            Self::queue_action(state, PendingMainMenuAction::ShowOptionsLayout);
            log::info!("Options button selected");
        } else if control_id == state.window_ids.world_builder_id {
            // World Builder button - C++ lines 1485-1497
            Self::queue_action(state, PendingMainMenuAction::LaunchWorldBuilder);
            log::info!("World Builder button selected");
        } else if control_id == state.window_ids.get_update_id {
            // Get Update button - C++ lines 1498-1501
            Self::queue_action(state, PendingMainMenuAction::StartDownloadingPatches);
            log::info!("Get Update button selected");
        } else if control_id == state.window_ids.exit_id {
            // Exit button - C++ lines 1502-1521
            Self::request_quit(state);
            log::info!("Exit button selected");
        } else if control_id == state.window_ids.button_challenge_id {
            // Challenge button - C++ lines 1522-1538
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }

            state.drop_down = DropdownType::Difficulty;
            Self::transition_set_group(
                "MainMenuFactionTraining",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::hide_window_by_name(
                state,
                "MainMenu.wnd:WinFactionTraining",
                true,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuSinglePlayerMenuBackTraining",
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDifficultyMenuTraining",
                false,
                effects,
                selected.as_deref_mut(),
            );
            state.campaign_selected = true;
            state.show_logo = false;
            state.show_side = ShowSide::Training;
            state.launch_challenge_menu = true;
            log::info!("Challenge button selected");
        } else if control_id == state.window_ids.button_usa_id {
            // USA Campaign button - C++ lines 1560-1587
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Difficulty;
            if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                manager.set_campaign("USA");
            } else {
                return Ok(());
            }
            Self::transition_set_group(
                "MainMenuFactionUS",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove("MainMenuFactionUS", true, effects, selected.as_deref_mut());
            Self::hide_window_by_name(
                state,
                "MainMenu.wnd:WinFactionUS",
                true,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuSinglePlayerMenuBackUS",
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDifficultyMenuUS",
                false,
                effects,
                selected.as_deref_mut(),
            );
            state.campaign_selected = true;
            state.logo_is_shown = false;
            state.show_logo = false;
            state.show_side = ShowSide::USA;
            log::info!("USA Campaign button selected");
        } else if control_id == state.window_ids.button_gla_id {
            // GLA Campaign button - C++ lines 1588-1615
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Difficulty;
            if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                manager.set_campaign("GLA");
            } else {
                return Ok(());
            }
            Self::transition_set_group(
                "MainMenuFactionGLA",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove("MainMenuFactionGLA", true, effects, selected.as_deref_mut());
            Self::hide_window_by_name(
                state,
                "MainMenu.wnd:WinFactionGLA",
                true,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuSinglePlayerMenuBackGLA",
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDifficultyMenuGLA",
                false,
                effects,
                selected.as_deref_mut(),
            );
            state.campaign_selected = true;
            state.logo_is_shown = false;
            state.show_logo = false;
            state.show_side = ShowSide::GLA;
            log::info!("GLA Campaign button selected");
        } else if control_id == state.window_ids.button_china_id {
            // China Campaign button - C++ lines 1616-1643
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Difficulty;
            if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                manager.set_campaign("China");
            } else {
                return Ok(());
            }
            Self::transition_set_group(
                "MainMenuFactionChina",
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_remove(
                "MainMenuFactionChina",
                true,
                effects,
                selected.as_deref_mut(),
            );
            Self::hide_window_by_name(
                state,
                "MainMenu.wnd:WinFactionChina",
                true,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_reverse(
                "MainMenuSinglePlayerMenuBackChina",
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDifficultyMenuChina",
                false,
                effects,
                selected.as_deref_mut(),
            );
            state.campaign_selected = true;
            state.logo_is_shown = false;
            state.show_logo = false;
            state.show_side = ShowSide::China;
            log::info!("China Campaign button selected");
        } else if control_id == state.window_ids.button_easy_id {
            // Easy difficulty - C++ lines 1644-1650
            if state.dont_allow_transitions {
                return Ok(());
            }
            Self::check_cd_before_campaign(
                state,
                GameDifficulty::Easy,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Easy difficulty selected");
        } else if control_id == state.window_ids.button_medium_id {
            // Medium difficulty - C++ lines 1651-1657
            if state.dont_allow_transitions {
                return Ok(());
            }
            Self::check_cd_before_campaign(
                state,
                GameDifficulty::Normal,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Medium difficulty selected");
        } else if control_id == state.window_ids.button_hard_id {
            // Hard difficulty - C++ lines 1658-1664
            if state.dont_allow_transitions {
                return Ok(());
            }
            Self::check_cd_before_campaign(
                state,
                GameDifficulty::Hard,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Hard difficulty selected");
        } else if control_id == state.window_ids.button_diff_back_id {
            // Difficulty Back button - C++ lines 1665-1673
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Single;
            if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                manager.set_campaign("");
            } else {
                return Ok(());
            }
            Self::diff_reverse_side(state, effects, selected.as_deref_mut());
            state.campaign_selected = false;
            log::info!("Difficulty Back button selected");
        }

        Ok(())
    }
}
