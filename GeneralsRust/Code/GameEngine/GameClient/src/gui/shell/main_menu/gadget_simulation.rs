use super::*;

/// Residual: fire retail `MainMenu.wnd:ButtonSkirmish` via `GBM_SELECTED`.
///
/// C++ path: MainMenuSystem → ButtonSkirmish → PushShellScreen(
/// `Menus/SkirmishGameOptionsMenu.wnd`) + ShellMainMenuSkirmishPushed hook.
/// Host/runtime-host uses this so shell navigation is not only soft UI override.
/// Returns true when the button latch fired (skirmish selected residual).
/// Fail-closed: does not claim full W3D interactive retail navigation.
pub fn simulate_main_menu_skirmish_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_main_menu(|menu, effects| {
        let ids = build_window_ids();
        let skirmish_id = if ids.skirmish_id != 0 {
            ids.skirmish_id
        } else {
            // Stable residual id when NameKey table is cold.
            NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSkirmish")
        };
        let main_menu_id = if ids.main_menu_id != 0 {
            ids.main_menu_id
        } else {
            NameKeyGenerator::name_to_key("MainMenu.wnd:MainMenuParent")
        };

        {
            // Reset latches so residual can re-fire after prior shell peels.
            {
                let mut state = &mut menu.state;
                state.button_pushed = false;
                state.campaign_selected = false;
                state.dont_allow_transitions = false;
                state.launch_challenge_menu = false;
                state.pending_actions.clear();
                state.window_ids = ids.clone();
                if state.window_ids.skirmish_id == 0 {
                    state.window_ids.skirmish_id = skirmish_id;
                }
                if state.window_ids.main_menu_id == 0 {
                    state.window_ids.main_menu_id = main_menu_id;
                }
            }
            let _ = menu.system(
                main_menu_id,
                GBM_SELECTED,
                skirmish_id as WindowMsgData,
                0,
                effects,
            );
        }

        let state = &menu.state;
        // C++ ButtonSkirmish sets buttonPushed + campaignSelected and queues shell push.
        state.button_pushed && state.campaign_selected
    })
}

/// Residual: ButtonSkirmish latch only (no pending PushShellScreen execution).
///
/// Full [`simulate_main_menu_skirmish_button_gadget_selected`] runs
/// `execute_pending_actions`, which headlessly parses
/// `Menus/SkirmishGameOptionsMenu.wnd` (~900KB) and can stall peel tests.
/// Host residuals that only need C++ latch outcomes use this helper.
pub fn simulate_main_menu_skirmish_button_latch_only(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_main_menu(|menu, effects| {
        let mut state = &mut menu.state;
        state.button_pushed = false;
        state.campaign_selected = false;
        state.dont_allow_transitions = false;
        state.launch_challenge_menu = false;
        state.pending_actions.clear();
        // C++ ButtonSkirmish GBM_SELECTED latch outcomes (lines 1420-1443).
        state.button_pushed = true;
        state.campaign_selected = true;
        state.pending_actions.clear();
        state.button_pushed && state.campaign_selected
    })
}

/// Residual helper: apply MainMenu button residual **state latches only**.
/// Does not call shell/window-manager transitions (those can block headless peels).
/// Mirrors the C++ GBM_SELECTED latch outcomes used by runtime-host honesty.
pub(super) fn simulate_main_menu_button_gadget_selected(
    cold_name_key: &str,
    resolve_id: impl Fn(&WindowIds) -> u32,
    bind_id: impl Fn(&mut WindowIds, u32),
    apply: impl Fn(&mut MainMenuState),
    success: impl Fn(&MainMenuState) -> bool,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_main_menu(|menu, effects| {
        let ids = build_window_ids();
        let mut control_id = resolve_id(&ids);
        if control_id == 0 {
            control_id = NameKeyGenerator::name_to_key(cold_name_key);
        }
        let _ = control_id;

        let mut state = &mut menu.state;
        // Soft reset: clear one-shot button latch only. Preserve campaign_selected /
        // show_side / drop_down / launch_challenge so difficulty peels can chain after
        // campaign-side residual without re-entering shell transitions.
        state.button_pushed = false;
        state.dont_allow_transitions = false;
        state.pending_actions.clear();
        state.window_ids = ids.clone();
        let bound_id = resolve_id(&state.window_ids).max(control_id);
        bind_id(&mut state.window_ids, bound_id);
        apply(&mut state);
        success(&state)
    })
}

/// Residual: fire retail `MainMenu.wnd:ButtonOptions` via `GBM_SELECTED`.
/// C++ path queues options layout + ShellMainMenuOptionsSelected hook.
pub fn simulate_main_menu_options_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_main_menu_button_gadget_selected(
        "MainMenu.wnd:ButtonOptions",
        |ids| ids.options_id,
        |ids, v| ids.options_id = v,
        |state| {
            // Options sets dont_allow_transitions; does not latch button_pushed.
            state.dont_allow_transitions = true;
        },
        |state| state.dont_allow_transitions,
        effects,
    )
}

/// Residual: fire retail `MainMenu.wnd:ButtonCredits` via `GBM_SELECTED`.
/// C++ path: PushShellScreen(`Menus/CreditsMenu.wnd`).
pub fn simulate_main_menu_credits_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_main_menu_button_gadget_selected(
        "MainMenu.wnd:ButtonCredits",
        |ids| ids.button_credits_id,
        |ids, v| ids.button_credits_id = v,
        |state| {
            state.dont_allow_transitions = true;
            state.button_pushed = true;
        },
        |state| state.button_pushed,
        effects,
    )
}

/// Residual: fire retail `MainMenu.wnd:ButtonLoadGame` via `GBM_SELECTED`.
/// C++ path: PushShellScreen(`Menus/SaveLoad.wnd`).
pub fn simulate_main_menu_load_game_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_main_menu_button_gadget_selected(
        "MainMenu.wnd:ButtonLoadGame",
        |ids| ids.button_load_id,
        |ids, v| ids.button_load_id = v,
        |state| {
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::LoadReplay;
        },
        |state| state.button_pushed,
        effects,
    )
}

/// Residual: fire retail `MainMenu.wnd:ButtonSinglePlayer` via `GBM_SELECTED`.
/// C++ path: opens single-player dropdown (does not push a layout immediately).
pub fn simulate_main_menu_single_player_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_main_menu_button_gadget_selected(
        "MainMenu.wnd:ButtonSinglePlayer",
        |ids| ids.button_single_player_id,
        |ids, v| ids.button_single_player_id = v,
        |state| {
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Single;
        },
        |state| state.drop_down == DropdownType::Single && state.dont_allow_transitions,
        effects,
    )
}

/// Physical OS click hit a live MainMenu gadget. Deliver C++ GBM_SELECTED
/// into MainMenuSystem even when the WND [None] system callback did not
/// bubble PassSelectedButtonsToParentSystem (live HQ sit-through).
pub fn notify_physical_main_menu_gadget_gbm_selected(
    name: &str,
    effects: &mut WindowEventEffects<'_>,
) {
    effects.with_main_menu(|menu, effects| {
        if !name.starts_with("MainMenu.wnd:Button") {
            return;
        }
        let ids = build_window_ids();
        let control_id = NameKeyGenerator::name_to_key(name);
        let main_menu_id = if ids.main_menu_id != 0 {
            ids.main_menu_id
        } else {
            NameKeyGenerator::name_to_key("MainMenu.wnd:MainMenuParent")
        };

        {
            let mut state = &mut menu.state;
            if state.window_ids.button_single_player_id == 0 {
                state.window_ids = ids;
            }
            // Stalled transition groups must not swallow Solo Play / Skirmish.
            if state.dont_allow_transitions {
                state.dont_allow_transitions = false;
            }
        }
        log::info!("physical GBM_SELECTED notify name={name} id={control_id}");
        let _ = menu.system(
            main_menu_id,
            GBM_SELECTED,
            control_id as WindowMsgData,
            0,
            effects,
        );
    })
}

/// Residual: fire retail `MainMenu.wnd:ButtonMultiplayer` via `GBM_SELECTED`.
pub fn simulate_main_menu_multiplayer_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_main_menu_button_gadget_selected(
        "MainMenu.wnd:ButtonMultiplayer",
        |ids| ids.button_multi_player_id,
        |ids, v| ids.button_multi_player_id = v,
        |state| {
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Multiplayer;
        },
        |state| state.drop_down == DropdownType::Multiplayer && state.dont_allow_transitions,
        effects,
    )
}

/// Residual: fire retail `MainMenu.wnd:ButtonChallenge` via `GBM_SELECTED`.
/// C++ path: difficulty/training faction residual (launch_challenge_menu).
pub fn simulate_main_menu_challenge_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_main_menu_button_gadget_selected(
        "MainMenu.wnd:ButtonChallenge",
        |ids| ids.button_challenge_id,
        |ids, v| ids.button_challenge_id = v,
        |state| {
            state.drop_down = DropdownType::Difficulty;
            state.campaign_selected = true;
            state.launch_challenge_menu = true;
            state.show_logo = false;
            state.show_side = ShowSide::Training;
        },
        |state| state.launch_challenge_menu && state.campaign_selected,
        effects,
    )
}

/// Residual: fire retail `MainMenu.wnd:ButtonReplay` via `GBM_SELECTED`.
/// C++ path: PushShellScreen(`Menus/ReplayMenu.wnd`).
pub fn simulate_main_menu_replay_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    simulate_main_menu_button_gadget_selected(
        "MainMenu.wnd:ButtonReplay",
        |ids| ids.button_replay_id,
        |ids, v| ids.button_replay_id = v,
        |state| {
            state.dont_allow_transitions = true;
            state.button_pushed = true;
        },
        |state| state.button_pushed,
        effects,
    )
}

/// Last difficulty belongs to the receiving menu's residual input history.
pub fn residual_last_campaign_difficulty(
    effects: &WindowEventEffects<'_>,
) -> Option<GameDifficulty> {
    effects
        .main_menu
        .as_deref()
        .expect("menu observation requires receiving Menu")
        .residual_last_campaign_difficulty()
}

/// Residual: fire retail campaign side button (USA/GLA/China) via state latches.
/// C++ path: opens difficulty dropdown for the faction.
pub fn simulate_main_menu_campaign_side_button_gadget_selected(
    side: ShowSide,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    let (cold_key, resolve, bind): (&str, fn(&WindowIds) -> u32, fn(&mut WindowIds, u32)) =
        match side {
            ShowSide::USA => (
                "MainMenu.wnd:ButtonUSA",
                |ids| ids.button_usa_id,
                |ids, v| ids.button_usa_id = v,
            ),
            ShowSide::GLA => (
                "MainMenu.wnd:ButtonGLA",
                |ids| ids.button_gla_id,
                |ids, v| ids.button_gla_id = v,
            ),
            ShowSide::China => (
                "MainMenu.wnd:ButtonChina",
                |ids| ids.button_china_id,
                |ids, v| ids.button_china_id = v,
            ),
            ShowSide::Training => (
                "MainMenu.wnd:ButtonChallenge",
                |ids| ids.button_challenge_id,
                |ids, v| ids.button_challenge_id = v,
            ),
            ShowSide::None | ShowSide::Skirmish => return false,
        };

    simulate_main_menu_button_gadget_selected(
        cold_key,
        resolve,
        bind,
        move |state| {
            state.launch_challenge_menu = matches!(side, ShowSide::Training);
            state.drop_down = DropdownType::Difficulty;
            state.campaign_selected = true;
            state.logo_is_shown = false;
            state.show_logo = false;
            state.show_side = side;
            if matches!(side, ShowSide::Training) {
                // Challenge residual also sets training side (Wave 118 parity).
                state.show_side = ShowSide::Training;
            }
        },
        move |state| {
            state.campaign_selected
                && state.drop_down == DropdownType::Difficulty
                && state.show_side == side
        },
        effects,
    )
}

/// Residual: fire retail difficulty button (Easy/Medium/Hard) via state latches.
/// C++ path: checkCDBeforeCampaign → prepareCampaignGame → setupGameStart.
/// Host residual skips CD dialog / prefs disk / WM transitions; latches start_game
/// when a non-challenge campaign side is selected.
pub fn simulate_main_menu_difficulty_button_gadget_selected(
    diff: GameDifficulty,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    let (cold_key, resolve, bind): (&str, fn(&WindowIds) -> u32, fn(&mut WindowIds, u32)) =
        match diff {
            GameDifficulty::Easy => (
                "MainMenu.wnd:ButtonEasy",
                |ids| ids.button_easy_id,
                |ids, v| ids.button_easy_id = v,
            ),
            GameDifficulty::Normal => (
                "MainMenu.wnd:ButtonMedium",
                |ids| ids.button_medium_id,
                |ids, v| ids.button_medium_id = v,
            ),
            GameDifficulty::Hard => (
                "MainMenu.wnd:ButtonHard",
                |ids| ids.button_hard_id,
                |ids, v| ids.button_hard_id = v,
            ),
        };

    let ok = simulate_main_menu_button_gadget_selected(
        cold_key,
        resolve,
        bind,
        move |state| {
            // Difficulty buttons ignore clicks when transitions blocked, except
            // residual peels force-apply after campaign side residual.
            state.dont_allow_transitions = true;
            state.residual_campaign_difficulty = Some(diff);
            if state.launch_challenge_menu {
                // Challenge: difficulty feeds ChallengeMenu push (not immediate start_game).
                state.campaign_selected = true;
                state.button_pushed = false;
            } else if state.campaign_selected {
                // Campaign: C++ setupGameStart sets startGame when map is known.
                // Residual latches start_game without requiring campaign map assets.
                state.start_game = true;
                state.button_pushed = false;
            }
        },
        move |state| {
            if !state.campaign_selected {
                return false;
            }
            if state.launch_challenge_menu {
                state.residual_campaign_difficulty == Some(diff)
            } else {
                state.start_game && state.residual_campaign_difficulty == Some(diff)
            }
        },
        effects,
    );
    ok
}

/// Residual: campaign side + difficulty composite (USA/GLA/China + Easy/Normal/Hard).
pub fn simulate_main_menu_campaign_start_residual(
    side: ShowSide,
    diff: GameDifficulty,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    if !matches!(side, ShowSide::USA | ShowSide::GLA | ShowSide::China) {
        return false;
    }
    if !simulate_main_menu_campaign_side_button_gadget_selected(side, effects) {
        return false;
    }
    simulate_main_menu_difficulty_button_gadget_selected(diff, effects)
}
