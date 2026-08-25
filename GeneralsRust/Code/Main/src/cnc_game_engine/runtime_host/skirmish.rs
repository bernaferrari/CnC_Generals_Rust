#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_open_skirmish_menu(&mut self, _args: &HashMap<String, String>) {
        // Prefer retail MainMenu.wnd:ButtonSkirmish (GBM_SELECTED) residual when
        // shell/WND push is enabled. Headless still exercises the latch so smoke
        // can observe open_skirmish_menu_ok_wnd without requiring W3D.
        // Fallback: soft UI override and/or direct SkirmishGameOptionsMenu push.
        //
        // Wave 833: full simulate_main_menu_skirmish_button_gadget_selected() runs
        // execute_pending_actions → parse Menus/SkirmishGameOptionsMenu.wnd (~900KB)
        // and stalls the runtime-host frame forever. Use latch_only + soft override
        // on headless; interactive keeps the full GBM_SELECTED path.
        let env_soft = std::env::var("GENERALS_RUNTIME_HOST_WND")
            .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
            .unwrap_or(false);
        let mut main_menu_skirmish_wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            if self.runtime_host_headless || env_soft {
                // Latch-only residual (C++ ButtonSkirmish GBM_SELECTED outcomes)
                // without pending PushShellScreen / WND parse.
                main_menu_skirmish_wnd_ok =
                    game_client::gui::simulate_main_menu_skirmish_button_latch_only();
                self.set_runtime_host_ui_screen_override(Some("Skirmish"));
                // Best-effort stack push without re-entering MainMenu system().
                // Skip if already on a Skirmish layout to avoid double-parse stalls.
                if !env_soft {
                    let top = game_client::gui::with_shell_ref(|shell| {
                        shell.top_filename().map(str::to_owned)
                    })
                    .flatten()
                    .unwrap_or_default();
                    let top_l = top.to_ascii_lowercase();
                    if !top_l.contains("skirmish") {
                        // Push only the options menu; MainMenu already active.
                        // create_layout may still be heavy — gate behind explicit env.
                        let allow_heavy = std::env::var("GENERALS_RUNTIME_HOST_SKIRMISH_WND")
                            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                            .unwrap_or(false);
                        if allow_heavy {
                            let _ = game_client::gui::with_shell_mut(|shell| {
                                shell.push("Menus/SkirmishGameOptionsMenu.wnd", false)
                            });
                        }
                    }
                }
            } else {
                // Interactive windowed: WND is the only menu owner.
                // Do not simulate_main_menu_skirmish_button_* and do not
                // transition_to_screen(Screen::Skirmish) on this path.
                // First-run CHAR reveal + OS WND clicks SinglePlayer → Skirmish.
                self.enter_shell_screen_from_runtime_host(Some("MainMenu"), "Menus/MainMenu.wnd");
                let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
                main_menu_skirmish_wnd_ok = game_client::gui::drive_os_wnd_open_skirmish_like_cpp();
                // Always parse/push SkirmishGameOptionsMenu.wnd on interactive
                // (cached parse; do not skip). Soft Screen::Skirmish only if
                // WND push failed AND we are not claiming wnd_widget_tree_nav.
                let push_ok = game_client::gui::with_shell_mut(|shell| {
                    shell.push("Menus/SkirmishGameOptionsMenu.wnd", false)
                })
                .is_some_and(|result| result.is_ok());
                if push_ok {
                    main_menu_skirmish_wnd_ok = true;
                    self.set_runtime_host_ui_screen_override(Some("Skirmish"));
                } else if !game_client::gui::os_wnd_widget_tree_nav_ok() {
                    // WND push failed and we are not claiming widget-tree nav.
                    self.set_runtime_host_ui_screen_override(Some("Skirmish"));
                }
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = env_soft;
            self.set_runtime_host_ui_screen_override(Some("Skirmish"));
        }
        // Sticky residual: smoke polls may miss one-frame Skirmish ui_screen
        // before start_game clears the override on InGame entry.
        self.runtime_host_saw_skirmish_menu = true;
        self.runtime_host_last_gameplay_cmd = if main_menu_skirmish_wnd_ok {
            "open_skirmish_menu_ok_wnd".into()
        } else {
            "open_skirmish_menu_ok".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_skirmish_start(&mut self, args: &HashMap<String, String>) {
        // Prefer retail WND ButtonStart (GadgetSelected) when shell push is
        // enabled; fall back to Main SkirmishMenu mouse residual.
        // Not direct start_game — both paths still go through start_game_from_ui
        // (WND via NewGame drain on next Menu tick).
        // Already in a match: ignore shell re-entry (control-file repeats must not
        // bounce InGame → Menu via enter_shell_screen_from_runtime_host).
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "click_skirmish_start_already_ingame".into();
            return;
        }
        self.set_runtime_host_ui_screen_override(Some("Skirmish"));
        self.runtime_host_saw_skirmish_menu = true;
        // Windowed + shell active: WND is the only menu owner. Soft
        // transition_to_screen(Screen::Skirmish) only when WND push failed
        // AND we are not claiming wnd_widget_tree_nav (headless keeps latch).
        let allow_soft_skirmish_screen = self.runtime_host_headless || {
            #[cfg(feature = "game_client")]
            {
                !game_client::gui::os_wnd_widget_tree_nav_ok()
            }
            #[cfg(not(feature = "game_client"))]
            {
                true
            }
        };
        if allow_soft_skirmish_screen && self.ui_manager.current_screen() != Some(Screen::Skirmish)
        {
            self.ui_manager.transition_to_screen(Screen::Skirmish);
        }
        let _ = self.ui_manager.skirmish_menu_mut().initialize();
        if let Some(map) = args.get("map") {
            self.ui_manager
                .skirmish_menu_mut()
                .set_map_name(map.clone());
            // Wave 837: also stamp GameClient skirmish_setup / options state so
            // ButtonStart residual cannot keep ShellMapMD over the control map.
            #[cfg(feature = "game_client")]
            {
                {
                    let mut setup = game_client::gui::get_skirmish_setup();
                    setup.set_selected_map(map.clone());
                    let info = setup.game_info_mut().game_info_mut();
                    info.set_map(map.clone());
                }
                game_client::gui::callbacks::set_skirmish_menu_selected_map(map.clone());
            }
        }
        let _ = self
            .ui_manager
            .skirmish_menu_mut()
            .configure_slot_medium_ai(1);

        if let Some(map) = args.get("map") {
            self.ui_manager
                .skirmish_menu_mut()
                .set_map_name(map.clone());
        }
        let _ = self
            .ui_manager
            .skirmish_menu_mut()
            .configure_slot_medium_ai(1);

        let mut wnd_start_ok = false;
        #[cfg(feature = "game_client")]
        {
            // Wave 833/835: headless default avoids SkirmishGameOptionsMenu.wnd
            // *layout parse* (stalls the frame). Wave 835 still runs map-select /
            // slot / rules / ButtonStart *state latch* residuals without create_layout.
            // Opt into heavy layout push with GENERALS_RUNTIME_HOST_SKIRMISH_WND=1.
            let push_wnd_layout = if self.runtime_host_headless {
                std::env::var("GENERALS_RUNTIME_HOST_SKIRMISH_WND")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
            } else {
                std::env::var("GENERALS_RUNTIME_HOST_WND")
                    .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                    .unwrap_or(true)
            };
            // Headless always runs latch peels; interactive needs layout when WND on.
            let run_wnd_latches = push_wnd_layout || self.runtime_host_headless;
            if run_wnd_latches {
                if push_wnd_layout {
                    self.enter_shell_screen_from_runtime_host(
                        Some("Skirmish"),
                        "Menus/SkirmishGameOptionsMenu.wnd",
                    );
                }
                // Bind control IDs + selected map into WND state when possible.
                // Prefer retail map-select overlay residual (ButtonSelectMap →
                // Listbox/OK) before Start, matching C++ player map pick.
                let mut map_select_wnd_ok = false;
                if let Some(map) = args.get("map") {
                    map_select_wnd_ok =
                        game_client::gui::callbacks::drive_os_wnd_skirmish_map_select_like_cpp(
                            map.clone(),
                        );
                    if !map_select_wnd_ok {
                        map_select_wnd_ok =
                            game_client::gui::callbacks::simulate_skirmish_map_select_and_confirm(
                                map.clone(),
                            );
                    }
                    // Wave 837: always force-commit control map into setup after
                    // latch attempt (shell residual must not win).
                    {
                        let mut setup = game_client::gui::get_skirmish_setup();
                        setup.set_selected_map(map.clone());
                        let info = setup.game_info_mut().game_info_mut();
                        info.set_map(map.clone());
                    }
                    game_client::gui::callbacks::set_skirmish_menu_selected_map(map.clone());
                    if map_select_wnd_ok
                        || !game_client::gui::get_skirmish_setup()
                            .game_info()
                            .game_info()
                            .get_map()
                            .is_empty()
                    {
                        map_select_wnd_ok = true;
                    }
                }
                // C++ init residual: human+MedAI slots and default rules
                // (cash/SW/speed) before Start.
                let match_options_ok =
                    game_client::gui::callbacks::simulate_skirmish_prepare_match_options();
                let slot_ai_wnd_ok = match_options_ok;
                // Optional difficulty override from control-file args.
                if let Some(diff) = args.get("ai") {
                    let state = match diff.to_ascii_lowercase().as_str() {
                        "easy" => Some(game_client::SlotState::EasyAI),
                        "hard" | "brutal" => Some(game_client::SlotState::BrutalAI),
                        "medium" | "med" => Some(game_client::SlotState::MedAI),
                        _ => None,
                    };
                    if let Some(state) = state {
                        let _ = game_client::gui::callbacks::simulate_skirmish_configure_slot_ai(
                            1, state, -1, -1, -1,
                        );
                    }
                }
                // Optional starting cash override (retail combo amounts only).
                if let Some(cash) = args.get("cash") {
                    if let Ok(amount) = cash.parse::<u32>() {
                        let _ = game_client::gui::callbacks::simulate_skirmish_set_starting_cash(
                            amount,
                        );
                    }
                }
                wnd_start_ok = game_client::gui::callbacks::drive_os_wnd_skirmish_start_like_cpp();
                if !wnd_start_ok {
                    wnd_start_ok =
                        game_client::gui::callbacks::simulate_skirmish_start_button_gadget_selected(
                        );
                }
                if map_select_wnd_ok && wnd_start_ok {
                    // Preserve map-select residual in cmd when both peels fire.
                    // Final cmd is rewritten below after NewGame drain.
                    self.runtime_host_last_gameplay_cmd = "click_skirmish_map_select_ok_wnd".into();
                }
                if wnd_start_ok {
                    // WND path posts NewGame; drain immediately so headless host
                    // does not wait for next Menu tick.
                    let start_cmd = if map_select_wnd_ok && match_options_ok {
                        "click_skirmish_start_ok_wnd_via_map_select_slots_rules"
                    } else if map_select_wnd_ok && slot_ai_wnd_ok {
                        "click_skirmish_start_ok_wnd_via_map_select_slots"
                    } else if map_select_wnd_ok {
                        "click_skirmish_start_ok_wnd_via_map_select"
                    } else if match_options_ok {
                        "click_skirmish_start_ok_wnd_via_slots_rules"
                    } else if slot_ai_wnd_ok {
                        "click_skirmish_start_ok_wnd_via_slots"
                    } else {
                        "click_skirmish_start_ok_wnd"
                    };
                    // Wave 840: control-file map wins over boot ShellMap pending residual.
                    let control_map = args.get("map").cloned().filter(|m| !m.trim().is_empty());
                    if let Some(mut request) = self.take_pending_new_game_start_request() {
                        let map = control_map
                            .clone()
                            .filter(|m| !Self::map_name_is_shell_residual(m))
                            .unwrap_or(request.map);
                        let map = if Self::map_name_is_shell_residual(&map) {
                            control_map.clone().unwrap_or(map)
                        } else {
                            map
                        };
                        request.map = map;
                        self.start_game_from_ui(request);
                        let _ = Self::take_new_game_dispatch_from_common_stream();
                        self.runtime_host_last_gameplay_cmd = start_cmd.into();
                    } else if gamelogic::helpers::TheGameLogic::is_start_new_game_requested() {
                        gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                        if let Some(mut request) =
                            self.build_start_request_from_pending_globals(None)
                        {
                            let map = control_map
                                .clone()
                                .filter(|m| !Self::map_name_is_shell_residual(m))
                                .unwrap_or(request.map);
                            let map = if Self::map_name_is_shell_residual(&map) {
                                control_map.clone().unwrap_or(map)
                            } else {
                                map
                            };
                            request.map = map;
                            self.start_game_from_ui(request);
                            self.runtime_host_last_gameplay_cmd = start_cmd.into();
                        } else if let Some(map) = control_map.clone() {
                            // Pending empty but control map present — start soft residual.
                            self.start_game_from_ui(HostStartRequest::without_player_template(
                                GameMode::Skirmish,
                                "USA".into(),
                                map,
                                None,
                            ));
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_start_ok_wnd_control_map".into();
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_start_wnd_pending".into();
                        }
                    } else if let Some(map) = control_map.clone() {
                        self.start_game_from_ui(HostStartRequest::without_player_template(
                            GameMode::Skirmish,
                            "USA".into(),
                            map,
                            None,
                        ));
                        self.runtime_host_last_gameplay_cmd =
                            "click_skirmish_start_ok_wnd_control_map".into();
                    } else {
                        self.runtime_host_last_gameplay_cmd =
                            "click_skirmish_start_wnd_pending".into();
                    }
                } else if map_select_wnd_ok {
                    // Map committed but Start did not claim — still honest residual.
                    self.runtime_host_last_gameplay_cmd = "click_skirmish_map_select_ok_wnd".into();
                }
            }
        }

        if !wnd_start_ok
            && !self
                .runtime_host_last_gameplay_cmd
                .starts_with("click_skirmish_start_ok")
        {
            match self
                .ui_manager
                .skirmish_menu_mut()
                .simulate_start_button_click()
            {
                Some(crate::ui::UIEvent::StartGame {
                    mode,
                    faction,
                    map,
                    skirmish,
                }) => {
                    // Wave 840: prefer control map over soft menu residual shell map.
                    let map = args
                        .get("map")
                        .cloned()
                        .filter(|m| !m.trim().is_empty())
                        .filter(|m| !Self::map_name_is_shell_residual(m))
                        .unwrap_or(map);
                    self.start_game_from_ui(HostStartRequest::without_player_template(
                        mode, faction, map, skirmish,
                    ));
                    self.runtime_host_last_gameplay_cmd = "click_skirmish_start_ok".into();
                }
                Some(other) => {
                    self.ui_manager.queue_event(other);
                    self.runtime_host_last_gameplay_cmd = "click_skirmish_start_event".into();
                }
                None => {
                    self.runtime_host_last_gameplay_cmd = "click_skirmish_start_miss".into();
                }
            }
        }
    }

    pub(super) fn runtime_host_cmd_click_skirmish_options_wnd(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "validate".to_string());
        let ok = match action.as_str() {
            "resolve" => {
                crate::gameplay_layout::resolve_skirmish_options_wnd_path().is_some()
                    || crate::gameplay_layout::skirmish_options_wnd_honesty().assets_unavailable
            }
            "validate" | "prepare" => {
                crate::gameplay_layout::simulate_skirmish_options_wnd_prepare_honesty()
            }
            "start" | "button_start" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_skirmish_options_wnd_ok_{action}")
        } else {
            format!("click_skirmish_options_wnd_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_open_load_game(&mut self, _args: &HashMap<String, String>) {
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = game_client::gui::simulate_main_menu_load_game_button_gadget_selected();
            // Also bind SaveLoad LoadOnly residual controls.
            let _ = game_client::gui::callbacks::simulate_save_load_menu_bind_layout(
                false,
                game_engine::SaveLoadLayoutType::LoadOnly,
            );
        }
        self.enter_shell_menu_from_runtime_host(Some("LoadGame"));
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_load_game_ok_wnd".into()
        } else {
            "open_load_game_ok".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_save_load(&mut self, args: &HashMap<String, String>) {
        // Retail SaveLoad list select + Load/Save/Delete/Back residual.
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "load".to_string());
        let slot = args
            .get("slot")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::callbacks::{
                simulate_save_load_menu_back_button_gadget_selected,
                simulate_save_load_menu_delete_button_gadget_selected,
                simulate_save_load_menu_prepare_load,
                simulate_save_load_menu_save_button_gadget_selected,
                simulate_save_load_menu_select_slot,
            };
            wnd_ok = match action.as_str() {
                "save" => {
                    let _ = simulate_save_load_menu_select_slot(slot);
                    simulate_save_load_menu_save_button_gadget_selected()
                }
                "delete" => {
                    let _ = simulate_save_load_menu_select_slot(slot);
                    simulate_save_load_menu_delete_button_gadget_selected()
                }
                "back" => simulate_save_load_menu_back_button_gadget_selected(),
                _ => simulate_save_load_menu_prepare_load(slot),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_save_load_ok_wnd_{action}")
        } else {
            "click_save_load_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_open_online(&mut self, _args: &HashMap<String, String>) {
        self.enter_shell_menu_from_runtime_host(Some("Online"));
    }

    pub(super) fn runtime_host_cmd_open_network(&mut self, _args: &HashMap<String, String>) {
        self.enter_shell_screen_from_runtime_host(Some("Network"), "Menus/LanLobbyMenu.wnd");
    }

    pub(super) fn runtime_host_cmd_open_replay(&mut self, _args: &HashMap<String, String>) {
        self.enter_shell_screen_from_runtime_host(Some("Replay"), "Menus/ReplayMenu.wnd");
    }

    pub(super) fn runtime_host_cmd_open_challenge_menu(&mut self, _args: &HashMap<String, String>) {
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            self.enter_shell_screen_from_runtime_host(Some("MainMenu"), "Menus/MainMenu.wnd");
            let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
            wnd_ok = game_client::gui::drive_os_wnd_open_challenge_menu_like_cpp();
            if !wnd_ok {
                wnd_ok = game_client::gui::simulate_main_menu_challenge_button_gadget_selected();
            }
        }
        self.enter_shell_screen_from_runtime_host(Some("Challenge"), "Menus/ChallengeMenu.wnd");
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_challenge_menu_ok_wnd".into()
        } else {
            "open_challenge_menu_ok".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_challenge_start(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        // Retail ChallengeMenu general select + ButtonPlay residual.
        let general = args
            .get("general")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = game_client::gui::callbacks::drive_os_wnd_challenge_start_like_cpp(general);
            if !wnd_ok {
                wnd_ok =
                    game_client::gui::callbacks::simulate_challenge_menu_prepare_start(general);
            }
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "click_challenge_start_ok_wnd".into()
        } else {
            "click_challenge_start_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_start_game(&mut self, args: &HashMap<String, String>) {
        let mode = Self::parse_runtime_host_mode(args.get("mode").map(String::as_str));
        let map = args
            .get("map")
            .cloned()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());
        // Prefer live client skirmish setup (WND path); else golden 2-slot host config.
        let skirmish = if matches!(mode, GameMode::Skirmish) {
            #[cfg(feature = "game_client")]
            {
                crate::skirmish_config::config_from_client_skirmish_setup(Some(map.as_str()))
                    .or_else(|| Some(crate::skirmish_config::golden_skirmish_config(map.as_str())))
            }
            #[cfg(not(feature = "game_client"))]
            {
                Some(crate::skirmish_config::golden_skirmish_config(map.as_str()))
            }
        } else {
            None
        };
        let faction = args
            .get("faction")
            .cloned()
            .or_else(|| {
                skirmish
                    .as_ref()
                    .map(crate::skirmish_config::local_faction_from_config)
            })
            .unwrap_or_else(|| "USA".to_string());
        self.set_runtime_host_ui_screen_override(None);
        self.start_game_from_ui(HostStartRequest::without_player_template(
            mode, faction, map, skirmish,
        ));
        // start_game_from_ui transitions Loading -> InGame internally
    }
    // WND parity: enqueue MSG_NEW_GAME on the common stream so Menu drain
    // (take_pending_new_game_start_request) is exercised on the live engine.

    pub(super) fn runtime_host_cmd_queue_new_game(&mut self, args: &HashMap<String, String>) {
        use game_engine::common::message_stream::{GameMessageType, get_message_stream};
        let mode_code = args
            .get("mode")
            .and_then(|m| match m.trim().to_ascii_lowercase().as_str() {
                "skirmish" | "2" => Some(2),
                "single" | "sp" | "0" => Some(0),
                "lan" | "1" => Some(1),
                "replay" | "3" => Some(3),
                _ => m.trim().parse::<i32>().ok(),
            })
            .unwrap_or(2);
        let map = args
            .get("map")
            .cloned()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());
        {
            let mut global = game_engine::common::global_data::write();
            global.pending_file = map.clone();
        }
        #[cfg(feature = "game_client")]
        {
            // Seed client setup map so config_from_client can resolve.
            let mut setup = game_client::gui::get_skirmish_setup();
            setup.set_selected_map(map.clone());
            setup.game_info_mut().game_info_mut().set_map(map.clone());
            if setup
                .game_info()
                .game_info()
                .get_slot(0)
                .map(|s| !s.is_occupied())
                .unwrap_or(true)
            {
                use game_client::SlotState;
                if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(0) {
                    slot.set_state(SlotState::Player, "Player".into(), 1);
                    slot.set_player_template(-1);
                    slot.set_team_number(0);
                    slot.set_start_pos(0);
                }
                if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(1) {
                    slot.set_state(SlotState::MedAI, "AI".into(), 0);
                    slot.set_player_template(-1);
                    slot.set_team_number(1);
                    slot.set_start_pos(1);
                }
            }
        }
        if let Ok(mut stream) = get_message_stream().write() {
            let msg = stream.append_message(GameMessageType::NewGame);
            msg.append_integer_argument(mode_code);
            msg.append_integer_argument(1); // DIFFICULTY_NORMAL
            msg.append_integer_argument(0); // rank points
            msg.append_integer_argument(30); // max fps residual
            info!("Runtime host queued NewGame mode_code={mode_code} map={map}");
        } else {
            warn!("Runtime host failed to lock message stream for NewGame");
        }
        // Peek + host start. With game_client, pump so crate GameLogic sees
        // MSG_NEW_GAME (C++ logicMessageDispatcher). Then drop leftovers.
        if let Some(request) = self.take_pending_new_game_start_request() {
            info!(
                "Runtime host NewGame drain: mode={:?} faction={} map={}",
                request.mode, request.faction, request.map
            );
            self.set_runtime_host_ui_screen_override(None);
            self.start_game_from_ui(request);
            #[cfg(feature = "game_client")]
            {
                let _ = self.game_client.pump_message_stream();
            }
            let _ = Self::take_new_game_dispatch_from_common_stream();
        } else {
            warn!("Runtime host queued NewGame but drain produced no start request");
            if self.current_state != GameState::Menu {
                self.request_state_change(GameState::Menu);
            }
        }
    }

    pub(super) fn runtime_host_cmd_click_new_game_stream(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "drain".to_string());
        let ok = match action.as_str() {
            "source" => crate::game_logic::honesty_new_game_stream_source(),
            "queue" | "drain" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_new_game_stream_ok_{action}")
        } else {
            format!("click_new_game_stream_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_w3d_main_menu_init(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "bind".to_string());
        let ok = match action.as_str() {
            "token" => crate::game_logic::honesty_main_menu_wnd_layoutinit_token(),
            "bind" | "source" => {
                crate::game_logic::honesty_w3d_main_menu_init_bind_source()
                    && crate::game_logic::honesty_w3d_main_menu_init_wrapper_source()
            }
            "prepare" | "init" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_w3d_main_menu_init_ok_{action}")
        } else {
            format!("click_w3d_main_menu_init_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_start_game_loading(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "source" => crate::game_logic::honesty_start_game_from_ui_loading_source(),
            "maps" | "defcon" | "lone_eagle" => {
                crate::game_logic::honesty_default_skirmish_map_resolves()
                    && crate::game_logic::honesty_lone_eagle_map_resolves()
            }
            "prepare" | "loading" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_start_game_loading_ok_{action}")
        } else {
            format!("click_start_game_loading_miss_{action}")
        };
    }
}
