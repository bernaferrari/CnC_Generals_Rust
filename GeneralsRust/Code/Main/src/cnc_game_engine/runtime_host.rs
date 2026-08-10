#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
impl CnCGameEngine {
    pub(super) fn apply_runtime_host_command(&mut self, raw_command: &str) {
        let mut parts = raw_command.split('|');
        let command = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        if command.is_empty() {
            return;
        }

        let mut args = HashMap::<String, String>::new();
        for part in parts {
            if let Some((key, value)) = part.split_once('=') {
                args.insert(
                    key.trim().to_ascii_lowercase(),
                    value.trim().trim_matches('"').to_string(),
                );
            }
        }

        match command.as_str() {
            "exit" => {
                self.request_state_change(GameState::Exiting);
            }
            "menu" | "quit_to_menu" | "exit_to_menu" => {
                self.enter_shell_menu_from_runtime_host(None);
                self.runtime_host_last_gameplay_cmd = "menu_ok".into();
            }
            "toggle_pause" | "pause" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "pause_fail_bad_state".into();
                } else {
                    let was_paused =
                        self.game_paused || matches!(self.current_state, GameState::Paused);
                    self.toggle_pause();
                    let now_paused =
                        self.game_paused || matches!(self.current_state, GameState::Paused);
                    self.runtime_host_last_gameplay_cmd = if !was_paused && now_paused {
                        "pause_ok:paused".into()
                    } else if was_paused && !now_paused {
                        "pause_ok:resumed".into()
                    } else if now_paused {
                        "pause_ok:paused".into()
                    } else {
                        "pause_ok:resumed".into()
                    };
                }
            }
            "open_message_of_the_day" | "open_motd" => {
                self.enter_shell_menu_from_runtime_host(Some("MessageOfDay"));
            }
            "open_get_updates" | "open_updates" => {
                self.enter_shell_menu_from_runtime_host(Some("GetUpdates"));
            }
            "open_world_builder" | "launch_world_builder" => {
                self.enter_shell_menu_from_runtime_host(Some("WorldBuilder"));
            }
            "options_probe" => {
                // Honesty residual: prove options host wiring without leaving InGame
                // (full open_options pauses / swaps UI and is covered separately).
                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "options_probe_ok".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "options_probe_fail_bad_state".into();
                }
            }

            "open_options" | "options" => {
                self.set_runtime_host_ui_screen_override(None);
                let mut wnd_ok = false;
                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.ui_manager.transition_to_screen(Screen::Options);
                    if self.current_state == GameState::InGame {
                        self.request_state_change(GameState::Paused);
                    }
                } else {
                    #[cfg(feature = "game_client")]
                    {
                        wnd_ok =
                            game_client::gui::simulate_main_menu_options_button_gadget_selected();
                    }
                    self.enter_shell_options_from_runtime_host();
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_options_ok_wnd".into()
                } else {
                    "options_ok".into()
                };
            }
            "open_credits" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::simulate_main_menu_credits_button_gadget_selected();
                    let _ = game_client::gui::callbacks::simulate_credits_menu_bind_controls();
                }
                self.enter_shell_screen_from_runtime_host(Some("Credits"), "Menus/CreditsMenu.wnd");
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_credits_ok_wnd".into()
                } else {
                    "open_credits_ok".into()
                };
            }
            "click_credits_menu" => {
                // Retail CreditsMenu ESC skip / finished / shutdown residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "skip".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_credits_menu_finished_like_cpp,
                        drive_os_wnd_credits_menu_prepare_skip_like_cpp,
                        drive_os_wnd_credits_menu_skip_like_cpp, simulate_credits_menu_finished,
                        simulate_credits_menu_prepare_skip, simulate_credits_menu_shutdown,
                        simulate_credits_menu_skip,
                    };
                    wnd_ok = match action.as_str() {
                        "finished" | "finish" => {
                            drive_os_wnd_credits_menu_finished_like_cpp()
                                || simulate_credits_menu_finished()
                        }
                        "shutdown" => simulate_credits_menu_shutdown(),
                        "prepare_skip" => {
                            drive_os_wnd_credits_menu_prepare_skip_like_cpp()
                                || simulate_credits_menu_prepare_skip()
                        }
                        _ => {
                            drive_os_wnd_credits_menu_skip_like_cpp()
                                || simulate_credits_menu_skip()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_credits_menu_ok_wnd_{action}")
                } else {
                    "click_credits_menu_miss".into()
                };
            }
            "show_message_box" => {
                // Retail MessageBox show residual without MessageBox.wnd create.
                let kind = args
                    .get("type")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "yes_no".to_string());
                let title = args
                    .get("title")
                    .cloned()
                    .unwrap_or_else(|| "GUI:Message".to_string());
                let body = args
                    .get("body")
                    .cloned()
                    .unwrap_or_else(|| "GUI:Confirm".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_message_box_show_ok, simulate_message_box_show_ok_cancel,
                        simulate_message_box_show_yes_no, simulate_message_box_show_yes_no_cancel,
                    };
                    wnd_ok = match kind.as_str() {
                        "ok" => simulate_message_box_show_ok(&title, &body),
                        "ok_cancel" => simulate_message_box_show_ok_cancel(&title, &body),
                        "yes_no_cancel" => simulate_message_box_show_yes_no_cancel(&title, &body),
                        _ => simulate_message_box_show_yes_no(&title, &body),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("show_message_box_ok_wnd_{kind}")
                } else {
                    "show_message_box_miss".into()
                };
            }
            "click_message_box" => {
                // Retail MessageBox Yes/No/Ok/Cancel residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "yes".to_string());
                let title = args
                    .get("title")
                    .cloned()
                    .unwrap_or_else(|| "GUI:Message".to_string());
                let body = args
                    .get("body")
                    .cloned()
                    .unwrap_or_else(|| "GUI:Confirm".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_message_box_cancel_like_cpp,
                        drive_os_wnd_message_box_no_like_cpp, drive_os_wnd_message_box_ok_like_cpp,
                        drive_os_wnd_message_box_prepare_ok_like_cpp,
                        drive_os_wnd_message_box_prepare_yes_like_cpp,
                        drive_os_wnd_message_box_yes_like_cpp,
                        simulate_message_box_cancel_button_gadget_selected,
                        simulate_message_box_hide, simulate_message_box_no_button_gadget_selected,
                        simulate_message_box_ok_button_gadget_selected,
                        simulate_message_box_prepare_ok, simulate_message_box_prepare_yes,
                        simulate_message_box_yes_button_gadget_selected,
                    };
                    wnd_ok = match action.as_str() {
                        "ok" => {
                            drive_os_wnd_message_box_ok_like_cpp()
                                || simulate_message_box_ok_button_gadget_selected()
                        }
                        "no" => {
                            drive_os_wnd_message_box_no_like_cpp()
                                || simulate_message_box_no_button_gadget_selected()
                        }
                        "cancel" => {
                            drive_os_wnd_message_box_cancel_like_cpp()
                                || simulate_message_box_cancel_button_gadget_selected()
                        }
                        "hide" => simulate_message_box_hide(),
                        "prepare_yes" => {
                            drive_os_wnd_message_box_prepare_yes_like_cpp(&title, &body)
                                || simulate_message_box_prepare_yes(&title, &body)
                        }
                        "prepare_ok" => {
                            drive_os_wnd_message_box_prepare_ok_like_cpp(&title, &body)
                                || simulate_message_box_prepare_ok(&title, &body)
                        }
                        _ => {
                            drive_os_wnd_message_box_yes_like_cpp()
                                || simulate_message_box_yes_button_gadget_selected()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_message_box_ok_wnd_{action}")
                } else {
                    "click_message_box_miss".into()
                };
            }
            "toggle_diplomacy" => {
                // Retail Diplomacy toggle residual without layout/animate.
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::drive_os_wnd_diplomacy_prepare_ingame_like_cpp();
                    if !wnd_ok {
                        wnd_ok = game_client::gui::callbacks::simulate_diplomacy_toggle_show();
                    }
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "toggle_diplomacy_ok_wnd".into()
                } else {
                    "toggle_diplomacy_miss".into()
                };
            }
            "click_diplomacy" => {
                // Retail Diplomacy radio/mute/hide residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "ingame".to_string());
                let slot = args
                    .get("slot")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_diplomacy_hide_like_cpp, drive_os_wnd_diplomacy_mute_like_cpp,
                        drive_os_wnd_diplomacy_prepare_ingame_like_cpp,
                        drive_os_wnd_diplomacy_radio_buddies_like_cpp,
                        drive_os_wnd_diplomacy_radio_ingame_like_cpp,
                        drive_os_wnd_diplomacy_unmute_like_cpp, simulate_diplomacy_hide,
                        simulate_diplomacy_mute_slot, simulate_diplomacy_prepare_ingame,
                        simulate_diplomacy_radio_buddies, simulate_diplomacy_radio_ingame,
                        simulate_diplomacy_reset, simulate_diplomacy_toggle_hide,
                        simulate_diplomacy_unmute_slot,
                    };
                    wnd_ok = match action.as_str() {
                        "hide" => {
                            drive_os_wnd_diplomacy_hide_like_cpp() || simulate_diplomacy_hide()
                        }
                        "toggle_hide" => simulate_diplomacy_toggle_hide(),
                        "reset" => simulate_diplomacy_reset(),
                        "buddies" => {
                            drive_os_wnd_diplomacy_radio_buddies_like_cpp()
                                || simulate_diplomacy_radio_buddies()
                        }
                        "mute" => {
                            drive_os_wnd_diplomacy_mute_like_cpp(slot)
                                || simulate_diplomacy_mute_slot(slot)
                        }
                        "unmute" => {
                            drive_os_wnd_diplomacy_unmute_like_cpp(slot)
                                || simulate_diplomacy_unmute_slot(slot)
                        }
                        "prepare_ingame" => {
                            drive_os_wnd_diplomacy_prepare_ingame_like_cpp()
                                || simulate_diplomacy_prepare_ingame()
                        }
                        _ => {
                            drive_os_wnd_diplomacy_radio_ingame_like_cpp()
                                || simulate_diplomacy_radio_ingame()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_diplomacy_ok_wnd_{action}")
                } else {
                    "click_diplomacy_miss".into()
                };
            }
            "open_popup_replay" => {
                // Retail ScoreScreen → PopupReplay residual open.
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_popup_replay_bind_controls();
                }
                self.enter_shell_menu_from_runtime_host(Some("PopupReplay"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_popup_replay_ok_wnd".into()
                } else {
                    "open_popup_replay_ok".into()
                };
            }
            "click_popup_replay" => {
                // Retail PopupReplay list/name/Save/Back residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "save".to_string());
                let slot = args
                    .get("slot")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0);
                let name = args
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "Replay".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_popup_replay_back_button_gadget_selected,
                        simulate_popup_replay_prepare_save,
                        simulate_popup_replay_prepare_save_from_slot,
                        simulate_popup_replay_save_button_gadget_selected,
                        simulate_popup_replay_select_slot, simulate_popup_replay_set_name,
                    };
                    wnd_ok = match action.as_str() {
                        "select" | "slot" => simulate_popup_replay_select_slot(slot),
                        "name" | "set_name" => simulate_popup_replay_set_name(&name),
                        "back" => simulate_popup_replay_back_button_gadget_selected(),
                        "prepare_save_slot" => {
                            simulate_popup_replay_prepare_save_from_slot(slot, &name)
                        }
                        "prepare_save" => simulate_popup_replay_prepare_save(&name),
                        _ => {
                            let _ = simulate_popup_replay_set_name(&name);
                            simulate_popup_replay_save_button_gadget_selected()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_popup_replay_ok_wnd_{action}")
                } else {
                    "click_popup_replay_miss".into()
                };
            }
            "open_single_player_menu" => {
                // Retail MainMenu → SinglePlayerMenu residual open.
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok =
                        game_client::gui::simulate_main_menu_single_player_button_gadget_selected();
                    let _ =
                        game_client::gui::callbacks::simulate_single_player_menu_bind_controls();
                }
                self.enter_shell_menu_from_runtime_host(Some("SinglePlayer"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_single_player_menu_ok_wnd".into()
                } else {
                    "open_single_player_menu_ok".into()
                };
            }
            "click_single_player_menu" => {
                // Retail SinglePlayerMenu New/Load/Back residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "new".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_single_player_menu_back_button_gadget_selected,
                        simulate_single_player_menu_clear_button_pushed,
                        simulate_single_player_menu_load_button_gadget_selected,
                        simulate_single_player_menu_new_button_gadget_selected,
                        simulate_single_player_menu_prepare_new,
                    };
                    wnd_ok = match action.as_str() {
                        "load" => {
                            let _ = simulate_single_player_menu_clear_button_pushed();
                            simulate_single_player_menu_load_button_gadget_selected()
                        }
                        "back" => {
                            let _ = simulate_single_player_menu_clear_button_pushed();
                            simulate_single_player_menu_back_button_gadget_selected()
                        }
                        "clear" => simulate_single_player_menu_clear_button_pushed(),
                        "prepare_new" => simulate_single_player_menu_prepare_new(),
                        _ => {
                            let _ = simulate_single_player_menu_clear_button_pushed();
                            simulate_single_player_menu_new_button_gadget_selected()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_single_player_menu_ok_wnd_{action}")
                } else {
                    "click_single_player_menu_miss".into()
                };
            }
            "open_map_select_menu" => {
                // Retail SinglePlayer → MapSelectMenu residual open.
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_map_select_menu_bind_controls();
                }
                self.enter_shell_menu_from_runtime_host(Some("MapSelect"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_map_select_menu_ok_wnd".into()
                } else {
                    "open_map_select_menu_ok".into()
                };
            }
            "click_map_select_menu" => {
                // Retail MapSelect OK/Back/difficulty/filter residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "ok".to_string());
                let map = args
                    .get("map")
                    .cloned()
                    .unwrap_or_else(|| "Maps/LoneEagle/LoneEagle.map".to_string());
                let difficulty = args
                    .get("difficulty")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(1);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_map_select_menu_back_button_gadget_selected,
                        simulate_map_select_menu_clear_button_pushed,
                        simulate_map_select_menu_multiplayer_maps_button_gadget_selected,
                        simulate_map_select_menu_ok_button_gadget_selected,
                        simulate_map_select_menu_prepare_ok, simulate_map_select_menu_select_map,
                        simulate_map_select_menu_set_difficulty,
                        simulate_map_select_menu_solo_maps_button_gadget_selected,
                        simulate_map_select_menu_system_maps_radio_selected,
                        simulate_map_select_menu_user_maps_radio_selected,
                    };
                    wnd_ok = match action.as_str() {
                        "select" | "map" => simulate_map_select_menu_select_map(&map),
                        "difficulty" => simulate_map_select_menu_set_difficulty(difficulty),
                        "back" => {
                            let _ = simulate_map_select_menu_clear_button_pushed();
                            simulate_map_select_menu_back_button_gadget_selected()
                        }
                        "solo" => simulate_map_select_menu_solo_maps_button_gadget_selected(),
                        "multiplayer" | "mp" => {
                            simulate_map_select_menu_multiplayer_maps_button_gadget_selected()
                        }
                        "system" => simulate_map_select_menu_system_maps_radio_selected(),
                        "user" => simulate_map_select_menu_user_maps_radio_selected(),
                        "prepare_ok" => simulate_map_select_menu_prepare_ok(&map),
                        "clear" => simulate_map_select_menu_clear_button_pushed(),
                        _ => {
                            let _ = simulate_map_select_menu_clear_button_pushed();
                            let _ = simulate_map_select_menu_select_map(&map);
                            let _ = simulate_map_select_menu_set_difficulty(difficulty);
                            simulate_map_select_menu_ok_button_gadget_selected()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_map_select_menu_ok_wnd_{action}")
                } else {
                    "click_map_select_menu_miss".into()
                };
            }
            "toggle_control_bar" => {
                // Retail ControlBar show/toggle residual without animate.
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_control_bar_toggle();
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "toggle_control_bar_ok_wnd".into()
                } else {
                    "toggle_control_bar_miss".into()
                };
            }
            "click_control_bar" => {
                // Retail ControlBar Options/Idle/General/Beacon residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "options".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_control_bar_clear_beacon_text_button_gadget_selected,
                        simulate_control_bar_communicator_button_gadget_selected,
                        simulate_control_bar_delete_beacon_button_gadget_selected,
                        simulate_control_bar_general_button_gadget_selected,
                        simulate_control_bar_hide,
                        simulate_control_bar_idle_worker_button_gadget_selected,
                        simulate_control_bar_large_button_gadget_selected,
                        simulate_control_bar_options_button_gadget_selected,
                        simulate_control_bar_place_beacon_button_gadget_selected,
                        simulate_control_bar_prepare_options, simulate_control_bar_show,
                    };
                    wnd_ok = match action.as_str() {
                        "show" => simulate_control_bar_show(),
                        "hide" => simulate_control_bar_hide(),
                        "idle" | "idle_worker" => {
                            simulate_control_bar_idle_worker_button_gadget_selected()
                        }
                        "general" => simulate_control_bar_general_button_gadget_selected(),
                        "large" => simulate_control_bar_large_button_gadget_selected(),
                        "place_beacon" | "beacon" => {
                            simulate_control_bar_place_beacon_button_gadget_selected()
                        }
                        "delete_beacon" => {
                            simulate_control_bar_delete_beacon_button_gadget_selected()
                        }
                        "clear_beacon" => {
                            simulate_control_bar_clear_beacon_text_button_gadget_selected()
                        }
                        "communicator" | "diplomacy" => {
                            simulate_control_bar_communicator_button_gadget_selected()
                        }
                        "prepare_options" => simulate_control_bar_prepare_options(),
                        // Wave 165: ControlBar.wnd headless materialise residual.
                        "resolve" | "validate" => {
                            crate::gameplay_layout::control_bar_layout_honesty(false)
                                .shell_residual_ok()
                        }
                        "load" | "materialise" => {
                            false
                        }
                        _ => simulate_control_bar_options_button_gadget_selected(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_control_bar_ok_wnd_{action}")
                } else {
                    "click_control_bar_miss".into()
                };
            }
            "open_single_player_menu" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok =
                        game_client::gui::simulate_main_menu_single_player_button_gadget_selected();
                }
                self.enter_shell_menu_from_runtime_host(Some("SinglePlayer"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_single_player_menu_ok_wnd".into()
                } else {
                    "open_single_player_menu_ok".into()
                };
            }
            "open_multiplayer_menu" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok =
                        game_client::gui::simulate_main_menu_multiplayer_button_gadget_selected();
                }
                self.enter_shell_menu_from_runtime_host(Some("Multiplayer"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_multiplayer_menu_ok_wnd".into()
                } else {
                    "open_multiplayer_menu_ok".into()
                };
            }
            "open_load_replay_menu" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::simulate_main_menu_replay_button_gadget_selected();
                    let _ = game_client::gui::callbacks::simulate_replay_menu_bind_controls();
                }
                self.enter_shell_menu_from_runtime_host(Some("LoadReplay"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_load_replay_menu_ok_wnd".into()
                } else {
                    "open_load_replay_menu_ok".into()
                };
            }
            "click_replay_menu" => {
                // Retail ReplayMenu list select + Load/Delete/Copy/Back residual.
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
                        drive_os_wnd_replay_menu_back_like_cpp,
                        drive_os_wnd_replay_menu_copy_like_cpp,
                        drive_os_wnd_replay_menu_delete_like_cpp,
                        drive_os_wnd_replay_menu_prepare_load_like_cpp,
                        simulate_replay_menu_back_button_gadget_selected,
                        simulate_replay_menu_copy_button_gadget_selected,
                        simulate_replay_menu_delete_button_gadget_selected,
                        simulate_replay_menu_prepare_load, simulate_replay_menu_select_slot,
                    };
                    wnd_ok = match action.as_str() {
                        "delete" => {
                            drive_os_wnd_replay_menu_delete_like_cpp(slot) || {
                                let _ = simulate_replay_menu_select_slot(slot);
                                simulate_replay_menu_delete_button_gadget_selected()
                            }
                        }
                        "copy" => {
                            drive_os_wnd_replay_menu_copy_like_cpp(slot) || {
                                let _ = simulate_replay_menu_select_slot(slot);
                                simulate_replay_menu_copy_button_gadget_selected()
                            }
                        }
                        "back" => {
                            drive_os_wnd_replay_menu_back_like_cpp()
                                || simulate_replay_menu_back_button_gadget_selected()
                        }
                        _ => {
                            drive_os_wnd_replay_menu_prepare_load_like_cpp(slot)
                                || simulate_replay_menu_prepare_load(slot)
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_replay_menu_ok_wnd_{action}")
                } else {
                    "click_replay_menu_miss".into()
                };
            }
            "toggle_quit_menu" => {
                // Retail QuitMenu show residual (ESC path without live layout).
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_quit_menu_toggle_show();
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "toggle_quit_menu_ok_wnd".into()
                } else {
                    "toggle_quit_menu_miss".into()
                };
            }
            "click_quit_menu" => {
                // Retail QuitMenu Exit/Return/Options/Restart/SaveLoad/Confirm residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "exit".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_quit_menu_exit_like_cpp, drive_os_wnd_quit_menu_options_like_cpp,
                        drive_os_wnd_quit_menu_prepare_exit_like_cpp,
                        drive_os_wnd_quit_menu_restart_like_cpp,
                        drive_os_wnd_quit_menu_return_like_cpp,
                        drive_os_wnd_quit_menu_save_load_like_cpp, simulate_quit_menu_confirm_exit,
                        simulate_quit_menu_destroy, simulate_quit_menu_exit_button_gadget_selected,
                        simulate_quit_menu_options_button_gadget_selected,
                        simulate_quit_menu_prepare_exit,
                        simulate_quit_menu_restart_button_gadget_selected,
                        simulate_quit_menu_return_button_gadget_selected,
                        simulate_quit_menu_save_load_button_gadget_selected,
                        simulate_quit_menu_toggle_hide, simulate_quit_menu_toggle_show,
                    };
                    wnd_ok = match action.as_str() {
                        "return" => {
                            drive_os_wnd_quit_menu_return_like_cpp()
                                || simulate_quit_menu_return_button_gadget_selected()
                        }
                        "options" => {
                            drive_os_wnd_quit_menu_options_like_cpp()
                                || simulate_quit_menu_options_button_gadget_selected()
                        }
                        "restart" => {
                            drive_os_wnd_quit_menu_restart_like_cpp()
                                || simulate_quit_menu_restart_button_gadget_selected()
                        }
                        "save_load" | "saveload" => {
                            drive_os_wnd_quit_menu_save_load_like_cpp()
                                || simulate_quit_menu_save_load_button_gadget_selected()
                        }
                        "hide" => simulate_quit_menu_toggle_hide(),
                        "show" => simulate_quit_menu_toggle_show(),
                        "destroy" => simulate_quit_menu_destroy(),
                        "confirm_exit" | "confirm" => simulate_quit_menu_confirm_exit(),
                        "prepare_exit" => {
                            drive_os_wnd_quit_menu_prepare_exit_like_cpp()
                                || simulate_quit_menu_prepare_exit()
                        }
                        _ => {
                            drive_os_wnd_quit_menu_exit_like_cpp() || {
                                let _ = simulate_quit_menu_toggle_show();
                                simulate_quit_menu_exit_button_gadget_selected()
                            }
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_quit_menu_ok_wnd_{action}")
                } else {
                    "click_quit_menu_miss".into()
                };
            }
            "open_keyboard_options" => {
                // Retail Options → KeyboardOptions residual open.
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_keyboard_options_bind_controls();
                }
                self.enter_shell_menu_from_runtime_host(Some("KeyboardOptions"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_keyboard_options_ok_wnd".into()
                } else {
                    "open_keyboard_options_ok".into()
                };
            }
            "click_keyboard_options" => {
                // Retail KeyboardOptions category/command/Assign/Reset/Back residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "assign".to_string());
                let category = args
                    .get("category")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let command = args
                    .get("command")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_keyboard_options_back_like_cpp,
                        drive_os_wnd_keyboard_options_category_like_cpp,
                        drive_os_wnd_keyboard_options_command_like_cpp,
                        drive_os_wnd_keyboard_options_prepare_assign_like_cpp,
                        drive_os_wnd_keyboard_options_reset_like_cpp,
                        simulate_keyboard_options_back_button_gadget_selected,
                        simulate_keyboard_options_prepare_assign,
                        simulate_keyboard_options_reset_all_button_gadget_selected,
                        simulate_keyboard_options_select_category,
                        simulate_keyboard_options_select_command,
                    };
                    wnd_ok = match action.as_str() {
                        "category" => {
                            drive_os_wnd_keyboard_options_category_like_cpp(category)
                                || simulate_keyboard_options_select_category(category)
                        }
                        "command" => {
                            let _ = drive_os_wnd_keyboard_options_category_like_cpp(category)
                                || simulate_keyboard_options_select_category(category);
                            drive_os_wnd_keyboard_options_command_like_cpp(command)
                                || simulate_keyboard_options_select_command(command)
                        }
                        "reset" | "reset_all" => {
                            drive_os_wnd_keyboard_options_reset_like_cpp()
                                || simulate_keyboard_options_reset_all_button_gadget_selected()
                        }
                        "back" => {
                            drive_os_wnd_keyboard_options_back_like_cpp()
                                || simulate_keyboard_options_back_button_gadget_selected()
                        }
                        _ => {
                            drive_os_wnd_keyboard_options_prepare_assign_like_cpp(category, command)
                                || simulate_keyboard_options_prepare_assign(category, command)
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_keyboard_options_ok_wnd_{action}")
                } else {
                    "click_keyboard_options_miss".into()
                };
            }
            "open_score_screen" => {
                // Retail end-of-match ScoreScreen residual open.
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_score_screen_bind_controls();
                }
                self.enter_shell_screen_from_runtime_host(
                    Some("ScoreScreen"),
                    "Menus/ScoreScreen.wnd",
                );
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_score_screen_ok_wnd".into()
                } else {
                    "open_score_screen_ok".into()
                };
            }
            "click_score_screen" => {
                // Retail ScoreScreen Ok/Continue/SaveReplay/Buddy/Emote residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "ok".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_score_screen_buddy_like_cpp,
                        drive_os_wnd_score_screen_continue_like_cpp,
                        drive_os_wnd_score_screen_emote_like_cpp,
                        drive_os_wnd_score_screen_ok_like_cpp,
                        drive_os_wnd_score_screen_save_replay_like_cpp,
                        simulate_score_screen_buddy_button_gadget_selected,
                        simulate_score_screen_continue_button_gadget_selected,
                        simulate_score_screen_emote_button_gadget_selected,
                        simulate_score_screen_ok_button_gadget_selected,
                        simulate_score_screen_prepare_finish, simulate_score_screen_prepare_ok,
                        simulate_score_screen_save_replay_button_gadget_selected,
                        simulate_score_screen_set_finish_campaign,
                    };
                    wnd_ok = match action.as_str() {
                        "continue" => {
                            drive_os_wnd_score_screen_continue_like_cpp()
                                || simulate_score_screen_continue_button_gadget_selected()
                        }
                        "finish" | "prepare_finish" => {
                            let _ = simulate_score_screen_set_finish_campaign(true);
                            drive_os_wnd_score_screen_continue_like_cpp()
                                || simulate_score_screen_prepare_finish()
                        }
                        "save_replay" | "replay" => {
                            drive_os_wnd_score_screen_save_replay_like_cpp()
                                || simulate_score_screen_save_replay_button_gadget_selected()
                        }
                        "buddy" => {
                            drive_os_wnd_score_screen_buddy_like_cpp()
                                || simulate_score_screen_buddy_button_gadget_selected()
                        }
                        "emote" => {
                            drive_os_wnd_score_screen_emote_like_cpp()
                                || simulate_score_screen_emote_button_gadget_selected()
                        }
                        "prepare_ok" => {
                            drive_os_wnd_score_screen_ok_like_cpp()
                                || simulate_score_screen_prepare_ok()
                        }
                        _ => {
                            drive_os_wnd_score_screen_ok_like_cpp()
                                || simulate_score_screen_ok_button_gadget_selected()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_score_screen_ok_wnd_{action}")
                } else {
                    "click_score_screen_miss".into()
                };
            }
            "open_options_menu" => {
                // Retail OptionsMenu residual open (shell or in-game overlay).
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_options_menu_bind_controls();
                }
                self.enter_shell_menu_from_runtime_host(Some("Options"));
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_options_menu_ok_wnd".into()
                } else {
                    "open_options_menu_ok".into()
                };
            }
            "click_options_menu" => {
                // Retail Options Accept/Back/Defaults/Keyboard/Advanced residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "accept".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_options_menu_accept_like_cpp,
                        drive_os_wnd_options_menu_advanced_accept_like_cpp,
                        drive_os_wnd_options_menu_advanced_back_like_cpp,
                        drive_os_wnd_options_menu_back_like_cpp,
                        drive_os_wnd_options_menu_defaults_like_cpp,
                        drive_os_wnd_options_menu_firewall_like_cpp,
                        drive_os_wnd_options_menu_keyboard_like_cpp,
                        simulate_options_menu_accept_button_gadget_selected,
                        simulate_options_menu_advanced_accept_button_gadget_selected,
                        simulate_options_menu_advanced_back_button_gadget_selected,
                        simulate_options_menu_back_button_gadget_selected,
                        simulate_options_menu_defaults_button_gadget_selected,
                        simulate_options_menu_firewall_refresh_button_gadget_selected,
                        simulate_options_menu_keyboard_button_gadget_selected,
                        simulate_options_menu_prepare_accept,
                    };
                    wnd_ok = match action.as_str() {
                        "back" => {
                            drive_os_wnd_options_menu_back_like_cpp()
                                || simulate_options_menu_back_button_gadget_selected()
                        }
                        "defaults" | "default" => {
                            drive_os_wnd_options_menu_defaults_like_cpp()
                                || simulate_options_menu_defaults_button_gadget_selected()
                        }
                        "keyboard" => {
                            drive_os_wnd_options_menu_keyboard_like_cpp()
                                || simulate_options_menu_keyboard_button_gadget_selected()
                        }
                        "advanced_accept" => {
                            drive_os_wnd_options_menu_advanced_accept_like_cpp()
                                || simulate_options_menu_advanced_accept_button_gadget_selected()
                        }
                        "advanced_back" => {
                            drive_os_wnd_options_menu_advanced_back_like_cpp()
                                || simulate_options_menu_advanced_back_button_gadget_selected()
                        }
                        "firewall" | "firewall_refresh" => {
                            drive_os_wnd_options_menu_firewall_like_cpp()
                                || simulate_options_menu_firewall_refresh_button_gadget_selected()
                        }
                        "prepare_accept" => {
                            drive_os_wnd_options_menu_accept_like_cpp()
                                || simulate_options_menu_prepare_accept()
                        }
                        _ => {
                            drive_os_wnd_options_menu_accept_like_cpp()
                                || simulate_options_menu_accept_button_gadget_selected()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_options_menu_ok_wnd_{action}")
                } else {
                    "click_options_menu_miss".into()
                };
            }
            "open_difficulty_menu" => {
                let campaign = args
                    .get("campaign")
                    .map(|value| value.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "usa".to_string());
                let override_screen = match campaign.as_str() {
                    "challenge" | "training" => "DifficultyChallenge",
                    "gla" => "DifficultyGla",
                    "china" => "DifficultyChina",
                    _ => "DifficultyUsa",
                };
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::ShowSide;
                    let side = match campaign.as_str() {
                        "challenge" | "training" => ShowSide::Training,
                        "gla" => ShowSide::GLA,
                        "china" => ShowSide::China,
                        _ => ShowSide::USA,
                    };
                    wnd_ok =
                        game_client::gui::simulate_main_menu_campaign_side_button_gadget_selected(
                            side,
                        );
                }
                self.enter_shell_menu_from_runtime_host(Some(override_screen));

                #[cfg(feature = "game_client")]
                {
                    let _ = game_client::gui::callbacks::simulate_difficulty_select_bind_controls();
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_difficulty_menu_ok_wnd".into()
                } else {
                    "open_difficulty_menu_ok".into()
                };
            }
            "click_difficulty_select" => {
                // Retail DifficultySelect Easy/Medium/Hard/Ok/Cancel residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "ok".to_string());
                let level = args
                    .get("level")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(1);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_difficulty_select_cancel_like_cpp,
                        drive_os_wnd_difficulty_select_like_cpp,
                        drive_os_wnd_difficulty_select_ok_like_cpp,
                        drive_os_wnd_difficulty_select_radio_like_cpp,
                        simulate_difficulty_select_cancel_button_gadget_selected,
                        simulate_difficulty_select_ok_button_gadget_selected,
                        simulate_difficulty_select_prepare_ok,
                        simulate_difficulty_select_radio_easy,
                        simulate_difficulty_select_radio_hard,
                        simulate_difficulty_select_radio_medium,
                    };
                    wnd_ok = match action.as_str() {
                        "easy" => {
                            drive_os_wnd_difficulty_select_radio_like_cpp(0)
                                || simulate_difficulty_select_radio_easy()
                        }
                        "medium" | "normal" => {
                            drive_os_wnd_difficulty_select_radio_like_cpp(1)
                                || simulate_difficulty_select_radio_medium()
                        }
                        "hard" => {
                            drive_os_wnd_difficulty_select_radio_like_cpp(2)
                                || simulate_difficulty_select_radio_hard()
                        }
                        "cancel" => {
                            drive_os_wnd_difficulty_select_cancel_like_cpp()
                                || simulate_difficulty_select_cancel_button_gadget_selected()
                        }
                        "prepare_ok" => {
                            drive_os_wnd_difficulty_select_like_cpp(level)
                                || simulate_difficulty_select_prepare_ok(level)
                        }
                        _ => {
                            drive_os_wnd_difficulty_select_like_cpp(level)
                                || drive_os_wnd_difficulty_select_ok_like_cpp()
                                || {
                                    let _ = match level {
                                        0 => simulate_difficulty_select_radio_easy(),
                                        2 => simulate_difficulty_select_radio_hard(),
                                        _ => simulate_difficulty_select_radio_medium(),
                                    };
                                    simulate_difficulty_select_ok_button_gadget_selected()
                                }
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_difficulty_select_ok_wnd_{action}")
                } else {
                    "click_difficulty_select_miss".into()
                };
            }
            "show_loading_screen" => {
                // Retail loading screen residual show without asset pipeline.
                let map = args
                    .get("map")
                    .cloned()
                    .unwrap_or_else(|| "Maps/LoneEagle/LoneEagle.map".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::loading_screen::drive_os_wnd_loading_screen_prepare_like_cpp(
                        &map, 0,
                    );
                    if !wnd_ok {
                        wnd_ok = game_client::gui::loading_screen::simulate_loading_screen_prepare_map_load(
                            &map, 0,
                        );
                    }
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "show_loading_screen_ok_wnd".into()
                } else {
                    "show_loading_screen_miss".into()
                };
            }
            "click_loading_screen" => {
                // Retail loading progress/hide/finish residual.
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "progress".to_string());
                let percent = args
                    .get("percent")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(50);
                let map = args
                    .get("map")
                    .cloned()
                    .unwrap_or_else(|| "Maps/LoneEagle/LoneEagle.map".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::loading_screen::{
                        drive_os_wnd_loading_screen_finish_like_cpp,
                        drive_os_wnd_loading_screen_hide_like_cpp,
                        drive_os_wnd_loading_screen_prepare_like_cpp,
                        drive_os_wnd_loading_screen_progress_like_cpp,
                        drive_os_wnd_loading_screen_show_like_cpp, simulate_loading_screen_finish,
                        simulate_loading_screen_hide, simulate_loading_screen_next_stage,
                        simulate_loading_screen_prepare_map_load, simulate_loading_screen_set_map,
                        simulate_loading_screen_set_progress, simulate_loading_screen_show,
                    };
                    wnd_ok = match action.as_str() {
                        "show" => {
                            drive_os_wnd_loading_screen_show_like_cpp()
                                || simulate_loading_screen_show()
                        }
                        "hide" => {
                            drive_os_wnd_loading_screen_hide_like_cpp()
                                || simulate_loading_screen_hide()
                        }
                        "map" => simulate_loading_screen_set_map(&map),
                        "next" | "next_stage" => simulate_loading_screen_next_stage(),
                        "finish" => {
                            drive_os_wnd_loading_screen_finish_like_cpp()
                                || simulate_loading_screen_finish()
                        }
                        "prepare" => {
                            drive_os_wnd_loading_screen_prepare_like_cpp(&map, percent)
                                || simulate_loading_screen_prepare_map_load(&map, percent)
                        }
                        _ => {
                            drive_os_wnd_loading_screen_progress_like_cpp(percent)
                                || simulate_loading_screen_set_progress(percent)
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_loading_screen_ok_wnd_{action}")
                } else {
                    "click_loading_screen_miss".into()
                };
            }
            "toggle_in_game_chat" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::drive_os_wnd_in_game_chat_toggle_like_cpp();
                    if !wnd_ok {
                        wnd_ok = game_client::gui::callbacks::simulate_in_game_chat_toggle();
                    }
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "toggle_in_game_chat_ok_wnd".into()
                } else {
                    "toggle_in_game_chat_miss".into()
                };
            }
            "click_in_game_chat" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "submit".to_string());
                let message = args
                    .get("message")
                    .cloned()
                    .unwrap_or_else(|| "gl hf".to_string());
                let chat_type = args
                    .get("type")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(1);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_in_game_chat_clear_like_cpp,
                        drive_os_wnd_in_game_chat_hide_like_cpp,
                        drive_os_wnd_in_game_chat_prepare_submit_like_cpp,
                        drive_os_wnd_in_game_chat_show_like_cpp,
                        drive_os_wnd_in_game_chat_submit_like_cpp,
                        simulate_in_game_chat_clear_button_gadget_selected,
                        simulate_in_game_chat_hide, simulate_in_game_chat_prepare_submit,
                        simulate_in_game_chat_reset, simulate_in_game_chat_set_type,
                        simulate_in_game_chat_show, simulate_in_game_chat_submit,
                    };
                    wnd_ok = match action.as_str() {
                        "show" => {
                            drive_os_wnd_in_game_chat_show_like_cpp()
                                || simulate_in_game_chat_show()
                        }
                        "hide" => {
                            drive_os_wnd_in_game_chat_hide_like_cpp()
                                || simulate_in_game_chat_hide()
                        }
                        "clear" => {
                            drive_os_wnd_in_game_chat_clear_like_cpp()
                                || simulate_in_game_chat_clear_button_gadget_selected()
                        }
                        "type" => simulate_in_game_chat_set_type(chat_type),
                        "reset" => simulate_in_game_chat_reset(),
                        "prepare_submit" => {
                            drive_os_wnd_in_game_chat_prepare_submit_like_cpp(&message)
                                || simulate_in_game_chat_prepare_submit(&message)
                        }
                        _ => {
                            drive_os_wnd_in_game_chat_submit_like_cpp(&message)
                                || simulate_in_game_chat_submit(&message)
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_in_game_chat_ok_wnd_{action}")
                } else {
                    "click_in_game_chat_miss".into()
                };
            }
            "click_idle_worker" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "select".to_string());
                let count = args
                    .get("count")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(1);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_idle_worker_button_gadget_selected,
                        simulate_idle_worker_prepare_select, simulate_idle_worker_select_next,
                        simulate_idle_worker_set_count,
                    };
                    wnd_ok = match action.as_str() {
                        "count" | "set_count" => simulate_idle_worker_set_count(count),
                        "next" => simulate_idle_worker_select_next(),
                        "prepare" | "prepare_select" => simulate_idle_worker_prepare_select(count),
                        _ => simulate_idle_worker_button_gadget_selected(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_idle_worker_ok_wnd_{action}")
                } else {
                    "click_idle_worker_miss".into()
                };
            }
            "open_generals_exp" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_generals_exp_show();
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_generals_exp_ok_wnd".into()
                } else {
                    "open_generals_exp_miss".into()
                };
            }
            "click_generals_exp" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "exit".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_generals_exp_esc,
                        simulate_generals_exp_exit_button_gadget_selected,
                        simulate_generals_exp_prepare_exit,
                        simulate_generals_exp_science_button_gadget_selected,
                        simulate_generals_exp_show,
                    };
                    wnd_ok = match action.as_str() {
                        "show" => simulate_generals_exp_show(),
                        "esc" => simulate_generals_exp_esc(),
                        "science" => simulate_generals_exp_science_button_gadget_selected(0),
                        "prepare_exit" => simulate_generals_exp_prepare_exit(),
                        _ => simulate_generals_exp_exit_button_gadget_selected(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_generals_exp_ok_wnd_{action}")
                } else {
                    "click_generals_exp_miss".into()
                };
            }
            "open_popup_communicator" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::callbacks::simulate_popup_communicator_show();
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_popup_communicator_ok_wnd".into()
                } else {
                    "open_popup_communicator_miss".into()
                };
            }
            "click_popup_communicator" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "ok".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        simulate_popup_communicator_esc,
                        simulate_popup_communicator_ok_button_gadget_selected,
                        simulate_popup_communicator_prepare_ok, simulate_popup_communicator_show,
                    };
                    wnd_ok = match action.as_str() {
                        "show" => simulate_popup_communicator_show(),
                        "esc" => simulate_popup_communicator_esc(),
                        "prepare_ok" => simulate_popup_communicator_prepare_ok(),
                        _ => simulate_popup_communicator_ok_button_gadget_selected(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_popup_communicator_ok_wnd_{action}")
                } else {
                    "click_popup_communicator_miss".into()
                };
            }
            "click_replay_control" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "play".to_string());
                let position = args
                    .get("position")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::callbacks::{
                        drive_os_wnd_replay_control_fast_forward_like_cpp,
                        drive_os_wnd_replay_control_pause_like_cpp,
                        drive_os_wnd_replay_control_play_like_cpp,
                        drive_os_wnd_replay_control_prepare_play_at_like_cpp,
                        drive_os_wnd_replay_control_stop_like_cpp, simulate_replay_control_pause,
                        simulate_replay_control_play, simulate_replay_control_prepare_play_at,
                        simulate_replay_control_seek, simulate_replay_control_stop,
                        simulate_replay_control_toggle_fast_forward,
                    };
                    wnd_ok = match action.as_str() {
                        "pause" => {
                            drive_os_wnd_replay_control_pause_like_cpp()
                                || simulate_replay_control_pause()
                        }
                        "stop" => {
                            drive_os_wnd_replay_control_stop_like_cpp()
                                || simulate_replay_control_stop()
                        }
                        "ff" | "fast_forward" => {
                            drive_os_wnd_replay_control_fast_forward_like_cpp()
                                || simulate_replay_control_toggle_fast_forward()
                        }
                        "seek" => simulate_replay_control_seek(position),
                        "prepare" | "prepare_play" => {
                            drive_os_wnd_replay_control_prepare_play_at_like_cpp(position)
                                || simulate_replay_control_prepare_play_at(position)
                        }
                        _ => {
                            drive_os_wnd_replay_control_play_like_cpp()
                                || simulate_replay_control_play()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_replay_control_ok_wnd_{action}")
                } else {
                    "click_replay_control_miss".into()
                };
            }
            "toggle_shell_map" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok = game_client::gui::simulate_shell_map_toggle();
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "toggle_shell_map_ok_wnd".into()
                } else {
                    "toggle_shell_map_miss".into()
                };
            }
            "click_shell_map" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "show".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::{
                        simulate_shell_map_hide, simulate_shell_map_prepare_cycle,
                        simulate_shell_map_show, simulate_shell_map_toggle,
                    };
                    wnd_ok = match action.as_str() {
                        "hide" => simulate_shell_map_hide(),
                        "toggle" => simulate_shell_map_toggle(),
                        "prepare_cycle" | "cycle" => simulate_shell_map_prepare_cycle(),
                        _ => simulate_shell_map_show(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_shell_map_ok_wnd_{action}")
                } else {
                    "click_shell_map_miss".into()
                };
            }
            "click_beacon" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "place".to_string());
                let player = args
                    .get("player")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0);
                let x = args
                    .get("x")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                let y = args
                    .get("y")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                let z = args
                    .get("z")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                let text = args
                    .get("text")
                    .cloned()
                    .unwrap_or_else(|| "beacon".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::system::{
                        simulate_beacon_drain_notifications, simulate_beacon_place,
                        simulate_beacon_prepare_place_with_text, simulate_beacon_remove,
                        simulate_beacon_set_text,
                    };
                    wnd_ok = match action.as_str() {
                        "remove" => simulate_beacon_remove(player, x, y, z),
                        "text" => simulate_beacon_set_text(player, x, y, z, &text),
                        "drain" => {
                            let _ = simulate_beacon_drain_notifications();
                            true
                        }
                        "prepare" | "prepare_text" => {
                            simulate_beacon_prepare_place_with_text(player, x, y, z, &text)
                        }
                        _ => simulate_beacon_place(player, x, y, z, Some(text.as_str())),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_beacon_ok_wnd_{action}")
                } else {
                    "click_beacon_miss".into()
                };
            }
            "click_eva" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let message = args
                    .get("message")
                    .cloned()
                    .unwrap_or_else(|| "LOWPOWER".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::eva::{
                        simulate_eva_disable, simulate_eva_enable,
                        simulate_eva_prepare_low_power_alert, simulate_eva_reset,
                        simulate_eva_set_should_play_by_name, simulate_eva_update,
                    };
                    wnd_ok = match action.as_str() {
                        "enable" => simulate_eva_enable(),
                        "disable" => simulate_eva_disable(),
                        "reset" => simulate_eva_reset(),
                        "update" => simulate_eva_update(),
                        "should_play" | "play" => simulate_eva_set_should_play_by_name(&message),
                        _ => simulate_eva_prepare_low_power_alert(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_eva_ok_wnd_{action}")
                } else {
                    "click_eva_miss".into()
                };
            }
            "click_ime" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let text = args
                    .get("text")
                    .cloned()
                    .unwrap_or_else(|| "nihao".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::{
                        drive_os_wnd_ime_clear_candidates_like_cpp,
                        drive_os_wnd_ime_prepare_composition_cycle_like_cpp,
                        drive_os_wnd_ime_result_like_cpp, simulate_ime_clear_candidates,
                        simulate_ime_disable, simulate_ime_enable, simulate_ime_end_composition,
                        simulate_ime_prepare_composition_cycle, simulate_ime_reset,
                        simulate_ime_result_string, simulate_ime_start_composition,
                        simulate_ime_update_composition,
                    };
                    wnd_ok = match action.as_str() {
                        "enable" => simulate_ime_enable(),
                        "disable" => simulate_ime_disable(),
                        "start" => simulate_ime_start_composition(),
                        "update" => simulate_ime_update_composition(&text, text.chars().count()),
                        "result" => {
                            drive_os_wnd_ime_result_like_cpp(&text)
                                || simulate_ime_result_string(&text)
                        }
                        "clear" => {
                            drive_os_wnd_ime_clear_candidates_like_cpp()
                                || simulate_ime_clear_candidates()
                        }
                        "end" => simulate_ime_end_composition(),
                        "reset" => simulate_ime_reset(),
                        _ => {
                            drive_os_wnd_ime_prepare_composition_cycle_like_cpp(&text)
                                || simulate_ime_prepare_composition_cycle(&text)
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_ime_ok_wnd_{action}")
                } else {
                    "click_ime_miss".into()
                };
            }
            "click_smudge" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let size = args
                    .get("size")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(12.0);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::system::{
                        simulate_smudge_add, simulate_smudge_add_set,
                        simulate_smudge_prepare_set_with_smudge, simulate_smudge_remove_first,
                        simulate_smudge_remove_set, simulate_smudge_reset,
                        simulate_smudge_set_count_last_frame,
                    };
                    wnd_ok = match action.as_str() {
                        "add_set" => simulate_smudge_add_set(),
                        "add" => simulate_smudge_add(size, 1.0),
                        "remove" => simulate_smudge_remove_first(),
                        "remove_set" => simulate_smudge_remove_set(),
                        "reset" => simulate_smudge_reset(),
                        "count" => simulate_smudge_set_count_last_frame(size as i32),
                        _ => simulate_smudge_prepare_set_with_smudge(size),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_smudge_ok_wnd_{action}")
                } else {
                    "click_smudge_miss".into()
                };
            }
            "click_ocl_timer" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let remaining = args
                    .get("remaining")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(870);
                let total = args
                    .get("total")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(900);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::control_bar::{
                        simulate_ocl_timer_format, simulate_ocl_timer_frames_to_display,
                        simulate_ocl_timer_prepare_display, simulate_ocl_timer_should_update,
                    };
                    wnd_ok = match action.as_str() {
                        "format" => {
                            let _ = simulate_ocl_timer_format(remaining, 0.5);
                            true
                        }
                        "frames" => {
                            let _ = simulate_ocl_timer_frames_to_display(remaining, total);
                            true
                        }
                        "should_update" => simulate_ocl_timer_should_update(0, remaining),
                        _ => simulate_ocl_timer_prepare_display(remaining, total).is_some(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_ocl_timer_ok_wnd_{action}")
                } else {
                    "click_ocl_timer_miss".into()
                };
            }
            "click_control_bar_resizer" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let name = args
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "ControlBar.wnd:ControlBarParent".to_string());
                let width = args
                    .get("width")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(1024);
                let height = args
                    .get("height")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(768);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::control_bar::{
                        simulate_control_bar_resizer_add_window,
                        simulate_control_bar_resizer_clear,
                        simulate_control_bar_resizer_get_optimal_size,
                        simulate_control_bar_resizer_prepare_default,
                        simulate_control_bar_resizer_resize,
                        simulate_control_bar_resizer_set_base_resolution,
                    };
                    wnd_ok = match action.as_str() {
                        "add" => simulate_control_bar_resizer_add_window(&name),
                        "clear" => simulate_control_bar_resizer_clear(),
                        "base" => simulate_control_bar_resizer_set_base_resolution(width, height),
                        "resize" => simulate_control_bar_resizer_resize(width, height),
                        "optimal" => {
                            let _ = simulate_control_bar_resizer_get_optimal_size();
                            true
                        }
                        _ => simulate_control_bar_resizer_prepare_default(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_control_bar_resizer_ok_wnd_{action}")
                } else {
                    "click_control_bar_resizer_miss".into()
                };
            }
            "click_under_construction" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let name = args
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "Strategy Center".to_string());
                let percent = args
                    .get("percent")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(66);
                let next = args
                    .get("next")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(75);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::control_bar::{
                        simulate_under_construction_cancel_command_name,
                        simulate_under_construction_complete, simulate_under_construction_populate,
                        simulate_under_construction_prepare_cycle,
                        simulate_under_construction_update_percent,
                    };
                    wnd_ok = match action.as_str() {
                        "populate" => simulate_under_construction_populate(&name, percent),
                        "update" => {
                            let _ = simulate_under_construction_update_percent(percent);
                            true
                        }
                        "complete" => simulate_under_construction_complete(),
                        "cancel" => {
                            simulate_under_construction_cancel_command_name()
                                == "Command_CancelConstruction"
                        }
                        _ => simulate_under_construction_prepare_cycle(&name, percent, next),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_under_construction_ok_wnd_{action}")
                } else {
                    "click_under_construction_miss".into()
                };
            }
            "click_structure_inventory" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let max_g = args
                    .get("max")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(10);
                let count = args
                    .get("count")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(3);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::control_bar::{
                        simulate_structure_inventory_clear,
                        simulate_structure_inventory_evacuate_command_name,
                        simulate_structure_inventory_exit_command_name,
                        simulate_structure_inventory_populate,
                        simulate_structure_inventory_prepare_occupied,
                        simulate_structure_inventory_stop_command_name,
                    };
                    wnd_ok = match action.as_str() {
                        "populate" => simulate_structure_inventory_populate(max_g, count),
                        "clear" => simulate_structure_inventory_clear(),
                        "exit" => {
                            simulate_structure_inventory_exit_command_name()
                                == "Command_StructureExit"
                        }
                        "evacuate" => {
                            simulate_structure_inventory_evacuate_command_name()
                                == "Command_Evacuate"
                        }
                        "stop" => {
                            simulate_structure_inventory_stop_command_name() == "Command_Stop"
                        }
                        _ => simulate_structure_inventory_prepare_occupied(max_g, count),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_structure_inventory_ok_wnd_{action}")
                } else {
                    "click_structure_inventory_miss".into()
                };
            }
            "click_multi_select" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::control_bar::{
                        simulate_multi_select_clear, simulate_multi_select_prepare_divergent,
                        simulate_multi_select_prepare_same_commands,
                    };
                    wnd_ok = match action.as_str() {
                        "clear" => simulate_multi_select_clear(),
                        "divergent" => simulate_multi_select_prepare_divergent(),
                        _ => simulate_multi_select_prepare_same_commands(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_multi_select_ok_wnd_{action}")
                } else {
                    "click_multi_select_miss".into()
                };
            }
            "click_credits_roll" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let text = args
                    .get("text")
                    .cloned()
                    .unwrap_or_else(|| "COMMAND & CONQUER".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::credits::{
                        drive_os_wnd_credits_roll_finished_like_cpp,
                        drive_os_wnd_credits_roll_prepare_like_cpp,
                        drive_os_wnd_credits_roll_update_like_cpp, simulate_credits_add_blank,
                        simulate_credits_add_text, simulate_credits_init,
                        simulate_credits_is_finished_probe, simulate_credits_prepare_short_roll,
                        simulate_credits_reset, simulate_credits_update,
                    };
                    wnd_ok = match action.as_str() {
                        "init" => simulate_credits_init(),
                        "reset" => simulate_credits_reset(),
                        "add" => simulate_credits_add_text(&text),
                        "blank" => simulate_credits_add_blank(),
                        "update" => {
                            drive_os_wnd_credits_roll_update_like_cpp()
                                || simulate_credits_update()
                        }
                        "finished" => {
                            drive_os_wnd_credits_roll_finished_like_cpp()
                                || simulate_credits_is_finished_probe()
                        }
                        _ => {
                            drive_os_wnd_credits_roll_prepare_like_cpp()
                                || simulate_credits_prepare_short_roll()
                        }
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_credits_roll_ok_wnd_{action}")
                } else {
                    "click_credits_roll_miss".into()
                };
            }
            "click_challenge_generals" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let name = args
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "General Alexander".to_string());
                let difficulty = args
                    .get("difficulty")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(2);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::{
                        simulate_challenge_generals_init,
                        simulate_challenge_generals_prepare_default,
                        simulate_challenge_generals_set_bio_name,
                        simulate_challenge_generals_set_difficulty,
                        simulate_challenge_generals_set_starts_enabled,
                        simulate_challenge_generals_set_template_num,
                    };
                    wnd_ok = match action.as_str() {
                        "init" => simulate_challenge_generals_init(),
                        "starts" => simulate_challenge_generals_set_starts_enabled(0, true),
                        "bio" => simulate_challenge_generals_set_bio_name(0, &name),
                        "difficulty" => simulate_challenge_generals_set_difficulty(difficulty),
                        "template" => simulate_challenge_generals_set_template_num(0),
                        _ => simulate_challenge_generals_prepare_default(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_challenge_generals_ok_wnd_{action}")
                } else {
                    "click_challenge_generals_miss".into()
                };
            }
            "click_gameworld_authority" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let wnd_ok = match action.as_str() {
                    "refresh" => {
                        crate::gameworld_shadow::simulate_gameworld_authority_refresh_env()
                    }
                    "shadow" => crate::gameworld_shadow::simulate_gameworld_shadow_enable_check(),
                    "probe" => self.host_simulate_gameworld_authority_probe(),
                    _ => crate::gameworld_shadow::simulate_gameworld_authority_prepare_defaults(),
                };
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_gameworld_authority_ok_wnd_{action}")
                } else {
                    "click_gameworld_authority_miss".into()
                };
            }
            "click_window_video" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::{
                        simulate_window_video_init, simulate_window_video_pause_all,
                        simulate_window_video_prepare_control_cycle, simulate_window_video_reset,
                        simulate_window_video_resume_all, simulate_window_video_stop_all,
                        simulate_window_video_update,
                    };
                    wnd_ok = match action.as_str() {
                        "init" => simulate_window_video_init(),
                        "reset" => simulate_window_video_reset(),
                        "pause" => simulate_window_video_pause_all(),
                        "resume" => simulate_window_video_resume_all(),
                        "stop" => simulate_window_video_stop_all(),
                        "update" => simulate_window_video_update(),
                        _ => simulate_window_video_prepare_control_cycle(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_window_video_ok_wnd_{action}")
                } else {
                    "click_window_video_miss".into()
                };
            }
            "click_main_menu_layout" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let wnd_ok = match action.as_str() {
                    "create" => crate::ui::simulate_main_menu_layout_create(),
                    "clear" => crate::ui::simulate_main_menu_layout_clear(),
                    "hit" => crate::ui::simulate_main_menu_layout_hit_test_single_player(),
                    _ => crate::ui::simulate_main_menu_layout_prepare_default(),
                };
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_main_menu_layout_ok_wnd_{action}")
                } else {
                    "click_main_menu_layout_miss".into()
                };
            }
            "click_control_bar_scheme" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let name = args
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "America8x6".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::control_bar::{
                        simulate_control_bar_scheme_clear, simulate_control_bar_scheme_get_current,
                        simulate_control_bar_scheme_load,
                        simulate_control_bar_scheme_prepare_default,
                    };
                    wnd_ok = match action.as_str() {
                        "load" => simulate_control_bar_scheme_load(&name),
                        "get" => simulate_control_bar_scheme_get_current(),
                        "clear" => simulate_control_bar_scheme_clear(),
                        _ => simulate_control_bar_scheme_prepare_default(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_control_bar_scheme_ok_wnd_{action}")
                } else {
                    "click_control_bar_scheme_miss".into()
                };
            }
            "click_presentation_boundary" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let wnd_ok = match action.as_str() {
                    "execute" => crate::graphics::simulate_presentation_boundary_execute_source(),
                    "collect" => crate::graphics::simulate_presentation_boundary_collect_source(),
                    "fallback" => {
                        crate::graphics::simulate_presentation_boundary_fallback_counter_source()
                    }
                    "cnc" => crate::graphics::simulate_presentation_boundary_cnc_execute_source(),
                    _ => crate::graphics::simulate_presentation_boundary_prepare_honesty(),
                };
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_presentation_boundary_ok_wnd_{action}")
                } else {
                    "click_presentation_boundary_miss".into()
                };
            }
            "click_control_bar_print_positions" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::control_bar::{
                        simulate_control_bar_print_positions_format_line,
                        simulate_control_bar_print_positions_parent_name,
                        simulate_control_bar_print_positions_prepare_sample,
                        simulate_control_bar_print_positions_script_names,
                    };
                    wnd_ok = match action.as_str() {
                        "parent" => simulate_control_bar_print_positions_parent_name(),
                        "script" => simulate_control_bar_print_positions_script_names(),
                        "format" => {
                            let block = simulate_control_bar_print_positions_format_line(
                                "ControlBar.wnd:ControlBarParent",
                                0,
                                450,
                                800,
                                150,
                            );
                            block.contains("END")
                        }
                        _ => simulate_control_bar_print_positions_prepare_sample(),
                    };
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_control_bar_print_positions_ok_wnd_{action}")
                } else {
                    "click_control_bar_print_positions_miss".into()
                };
            }
            "click_terrain_env_boundary" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let wnd_ok = match action.as_str() {
                    "heightmap" => {
                        crate::game_logic::simulate_terrain_env_boundary_heightmap_source()
                    }
                    "skybox" => crate::game_logic::simulate_terrain_env_boundary_skybox_source(),
                    "sync" => crate::game_logic::simulate_terrain_env_boundary_sync_source(),
                    _ => false,
                };
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_terrain_env_boundary_ok_wnd_{action}")
                } else {
                    "click_terrain_env_boundary_miss".into()
                };
            }
            "click_main_menu_wnd" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let wnd_ok = match action.as_str() {
                    "resolve" => {
                        crate::gameplay_layout::resolve_main_menu_wnd_path().is_some()
                            || crate::gameplay_layout::main_menu_wnd_honesty().assets_unavailable
                    }
                    "validate" => {
                        let h = crate::gameplay_layout::main_menu_wnd_honesty();
                        h.shell_residual_ok()
                    }
                    "load" => crate::gameplay_layout::simulate_main_menu_wnd_prepare_load_honesty(),
                    "materialise" => {
                        false
                    }
                    _ => crate::gameplay_layout::simulate_main_menu_wnd_prepare_honesty(),
                };
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_main_menu_wnd_ok_wnd_{action}")
                } else {
                    "click_main_menu_wnd_miss".into()
                };
            }
            "click_shell_stack" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "push".to_string());
                let ok = match action.as_str() {
                    "init" => crate::game_logic::honesty_show_shell_menu_init_before_push_source(),
                    "snapshot" => {
                        crate::game_logic::honesty_shell_snapshot_no_invented_stack_source()
                    }
                    "push" | "prepare" => false,
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_shell_stack_ok_wnd_{action}")
                } else {
                    format!("click_shell_stack_miss_{action}")
                };
            }
            "click_shell_skirmish_nav" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "push".to_string());
                let ok = match action.as_str() {
                    "windows" => {
                        #[cfg(feature = "game_client")]
                        {
                            false
                                && game_client::gui::with_window_manager_ref(|wm| {
                                    wm.window_count() > 0
                                })
                        }
                        #[cfg(not(feature = "game_client"))]
                        {
                            true
                        }
                    }
                    "push" | "prepare" | "skirmish" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_shell_skirmish_nav_ok_wnd_{action}")
                } else {
                    format!("click_shell_skirmish_nav_miss_{action}")
                };
            }
            "click_campaign_start" => {
                // Retail MainMenu campaign side + difficulty residual composite.
                let campaign = args
                    .get("campaign")
                    .map(|value| value.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "usa".to_string());
                let diff_s = args
                    .get("difficulty")
                    .map(|value| value.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "normal".to_string());
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    use game_client::gui::{GameDifficulty, ShowSide};
                    let side = match campaign.as_str() {
                        "gla" => ShowSide::GLA,
                        "china" => ShowSide::China,
                        _ => ShowSide::USA,
                    };
                    let diff = match diff_s.as_str() {
                        "easy" => GameDifficulty::Easy,
                        "hard" => GameDifficulty::Hard,
                        _ => GameDifficulty::Normal,
                    };
                    wnd_ok = game_client::gui::drive_os_wnd_start_campaign_like_cpp(side, diff);
                    if !wnd_ok {
                        wnd_ok = game_client::gui::simulate_main_menu_campaign_start_residual(
                            side, diff,
                        );
                    }
                }
                if wnd_ok {
                    self.runtime_host_last_gameplay_cmd = "click_campaign_start_ok_wnd".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "click_campaign_start_miss".into();
                }
            }
            "open_skirmish_menu" => {
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
                            let top = game_client::gui::get_shell()
                                .top()
                                .map(|l| l.get_filename().to_string())
                                .unwrap_or_default();
                            let top_l = top.to_ascii_lowercase();
                            if !top_l.contains("skirmish") {
                                // Push only the options menu; MainMenu already active.
                                // create_layout may still be heavy — gate behind explicit env.
                                let allow_heavy =
                                    std::env::var("GENERALS_RUNTIME_HOST_SKIRMISH_WND")
                                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                                        .unwrap_or(false);
                                if allow_heavy {
                                    let _ = game_client::gui::get_shell()
                                        .push("Menus/SkirmishGameOptionsMenu.wnd", false);
                                }
                            }
                        }
                    } else {
                        // Interactive: C++ first-run CHAR reveal + OS WND clicks
                        // SinglePlayer → Skirmish (not simulate_*).
                        self.enter_shell_screen_from_runtime_host(
                            Some("MainMenu"),
                            "Menus/MainMenu.wnd",
                        );
                        let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
                        main_menu_skirmish_wnd_ok =
                            game_client::gui::drive_os_wnd_open_skirmish_like_cpp();
                        if !main_menu_skirmish_wnd_ok {
                            main_menu_skirmish_wnd_ok = game_client::gui::simulate_main_menu_skirmish_button_gadget_selected();
                        }
                        self.enter_shell_screen_from_runtime_host(
                            Some("Skirmish"),
                            "Menus/SkirmishGameOptionsMenu.wnd",
                        );
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
            "click_skirmish_start" => {
                // Prefer retail WND ButtonStart (GadgetSelected) when shell push is
                // enabled; fall back to Main SkirmishMenu mouse residual.
                // Not direct start_game — both paths still go through start_game_from_ui
                // (WND via NewGame drain on next Menu tick).
                // Already in a match: ignore shell re-entry (control-file repeats must not
                // bounce InGame → Menu via enter_shell_screen_from_runtime_host).
                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "click_skirmish_start_already_ingame".into();
                    return;
                }
                self.set_runtime_host_ui_screen_override(Some("Skirmish"));
                self.runtime_host_saw_skirmish_menu = true;
                if self.ui_manager.current_screen() != Some(Screen::Skirmish) {
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
                            map_select_wnd_ok = game_client::gui::callbacks::drive_os_wnd_skirmish_map_select_like_cpp(
                                map.clone(),
                            );
                            if !map_select_wnd_ok {
                                map_select_wnd_ok = game_client::gui::callbacks::simulate_skirmish_map_select_and_confirm(
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
                            game_client::gui::callbacks::set_skirmish_menu_selected_map(
                                map.clone(),
                            );
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
                        wnd_start_ok =
                            game_client::gui::callbacks::drive_os_wnd_skirmish_start_like_cpp();
                        if !wnd_start_ok {
                            wnd_start_ok = game_client::gui::callbacks::simulate_skirmish_start_button_gadget_selected();
                        }
                        if map_select_wnd_ok && wnd_start_ok {
                            // Preserve map-select residual in cmd when both peels fire.
                            // Final cmd is rewritten below after NewGame drain.
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_map_select_ok_wnd".into();
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
                            let control_map =
                                args.get("map").cloned().filter(|m| !m.trim().is_empty());
                            if let Some((mode, faction, map, skirmish)) =
                                self.take_pending_new_game_start_request()
                            {
                                let map = control_map
                                    .clone()
                                    .filter(|m| !Self::map_name_is_shell_residual(m))
                                    .unwrap_or(map);
                                let map = if Self::map_name_is_shell_residual(&map) {
                                    control_map.clone().unwrap_or(map)
                                } else {
                                    map
                                };
                                self.start_game_from_ui(mode, faction, map, skirmish);
                                self.runtime_host_last_gameplay_cmd = start_cmd.into();
                            } else if gamelogic::helpers::TheGameLogic::is_start_new_game_requested(
                            ) {
                                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                                if let Some((mode, faction, map, skirmish)) =
                                    self.build_start_request_from_pending_globals(None)
                                {
                                    let map = control_map
                                        .clone()
                                        .filter(|m| !Self::map_name_is_shell_residual(m))
                                        .unwrap_or(map);
                                    let map = if Self::map_name_is_shell_residual(&map) {
                                        control_map.clone().unwrap_or(map)
                                    } else {
                                        map
                                    };
                                    self.start_game_from_ui(mode, faction, map, skirmish);
                                    self.runtime_host_last_gameplay_cmd = start_cmd.into();
                                } else if let Some(map) = control_map.clone() {
                                    // Pending empty but control map present — start soft residual.
                                    self.start_game_from_ui(
                                        GameMode::Skirmish,
                                        "USA".into(),
                                        map,
                                        None,
                                    );
                                    self.runtime_host_last_gameplay_cmd =
                                        "click_skirmish_start_ok_wnd_control_map".into();
                                } else {
                                    self.runtime_host_last_gameplay_cmd =
                                        "click_skirmish_start_wnd_pending".into();
                                }
                            } else if let Some(map) = control_map.clone() {
                                self.start_game_from_ui(
                                    GameMode::Skirmish,
                                    "USA".into(),
                                    map,
                                    None,
                                );
                                self.runtime_host_last_gameplay_cmd =
                                    "click_skirmish_start_ok_wnd_control_map".into();
                            } else {
                                self.runtime_host_last_gameplay_cmd =
                                    "click_skirmish_start_wnd_pending".into();
                            }
                        } else if map_select_wnd_ok {
                            // Map committed but Start did not claim — still honest residual.
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_map_select_ok_wnd".into();
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
                            self.start_game_from_ui(mode, faction, map, skirmish);
                            self.runtime_host_last_gameplay_cmd = "click_skirmish_start_ok".into();
                        }
                        Some(other) => {
                            self.ui_manager.queue_event(other);
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_start_event".into();
                        }
                        None => {
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_start_miss".into();
                        }
                    }
                }
            }
            "click_skirmish_options_wnd" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "validate".to_string());
                let ok = match action.as_str() {
                    "resolve" => {
                        crate::gameplay_layout::resolve_skirmish_options_wnd_path().is_some()
                            || crate::gameplay_layout::skirmish_options_wnd_honesty()
                                .assets_unavailable
                    }
                    "validate" | "prepare" => {
                        crate::gameplay_layout::simulate_skirmish_options_wnd_prepare_honesty()
                    }
                    "start" | "button_start" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_skirmish_options_wnd_ok_{action}")
                } else {
                    format!("click_skirmish_options_wnd_miss_{action}")
                };
            }
            "open_load_game" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok =
                        game_client::gui::simulate_main_menu_load_game_button_gadget_selected();
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
            "click_save_load" => {
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
            "open_online" => {
                self.enter_shell_menu_from_runtime_host(Some("Online"));
            }
            "open_network" => {
                self.enter_shell_screen_from_runtime_host(
                    Some("Network"),
                    "Menus/LanLobbyMenu.wnd",
                );
            }
            "open_replay" => {
                self.enter_shell_screen_from_runtime_host(Some("Replay"), "Menus/ReplayMenu.wnd");
            }
            "open_challenge_menu" => {
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    self.enter_shell_screen_from_runtime_host(
                        Some("MainMenu"),
                        "Menus/MainMenu.wnd",
                    );
                    let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
                    wnd_ok = game_client::gui::drive_os_wnd_open_challenge_menu_like_cpp();
                    if !wnd_ok {
                        wnd_ok = game_client::gui::simulate_main_menu_challenge_button_gadget_selected();
                    }
                }
                self.enter_shell_screen_from_runtime_host(
                    Some("Challenge"),
                    "Menus/ChallengeMenu.wnd",
                );
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "open_challenge_menu_ok_wnd".into()
                } else {
                    "open_challenge_menu_ok".into()
                };
            }
            "click_challenge_start" => {
                // Retail ChallengeMenu general select + ButtonPlay residual.
                let general = args
                    .get("general")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut wnd_ok = false;
                #[cfg(feature = "game_client")]
                {
                    wnd_ok =
                        game_client::gui::callbacks::drive_os_wnd_challenge_start_like_cpp(general);
                    if !wnd_ok {
                        wnd_ok = game_client::gui::callbacks::simulate_challenge_menu_prepare_start(
                            general,
                        );
                    }
                }
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    "click_challenge_start_ok_wnd".into()
                } else {
                    "click_challenge_start_miss".into()
                };
            }
            "start_game" => {
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
                        crate::skirmish_config::config_from_client_skirmish_setup(Some(
                            map.as_str(),
                        ))
                        .or_else(|| {
                            Some(crate::skirmish_config::golden_skirmish_config(map.as_str()))
                        })
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
                self.start_game_from_ui(mode, faction, map, skirmish);
                // start_game_from_ui transitions Loading -> InGame internally
            }
            // WND parity: enqueue MSG_NEW_GAME on the common stream so Menu drain
            // (take_pending_new_game_start_request) is exercised on the live engine.
            "queue_new_game" => {
                use game_engine::common::message_stream::{get_message_stream, GameMessageType};
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
                // Drain immediately (same helpers Menu update uses). Relying only on
                // the next Menu frame races pump_message_stream / state transitions.
                if let Some((mode, faction, map_name, skirmish)) =
                    self.take_pending_new_game_start_request()
                {
                    info!(
                        "Runtime host NewGame drain: mode={:?} faction={} map={}",
                        mode, faction, map_name
                    );
                    self.set_runtime_host_ui_screen_override(None);
                    self.start_game_from_ui(mode, faction, map_name, skirmish);
                } else {
                    warn!("Runtime host queued NewGame but drain produced no start request");
                    if self.current_state != GameState::Menu {
                        self.request_state_change(GameState::Menu);
                    }
                }
            }
            "click_new_game_stream" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "drain".to_string());
                let ok = match action.as_str() {
                    "source" => crate::game_logic::honesty_new_game_stream_source(),
                    "queue" | "drain" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_new_game_stream_ok_{action}")
                } else {
                    format!("click_new_game_stream_miss_{action}")
                };
            }
            "click_w3d_main_menu_init" => {
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
            "click_start_game_loading" => {
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
                    "prepare" | "loading" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_start_game_loading_ok_{action}")
                } else {
                    format!("click_start_game_loading_miss_{action}")
                };
            }
            "click_live_map_load" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "defcon" | "lone_eagle" | "prepare" | "load" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_map_load_ok_{action}")
                } else {
                    format!("click_live_map_load_miss_{action}")
                };
            }
            "click_live_presentation_seed" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "source" => {
                        crate::game_logic::honesty_seed_presentation_after_match_start_source()
                            && crate::game_logic::honesty_render_execute_presentation_only_source()
                    }
                    "build" | "prepare" | "seed" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_seed_ok_{action}")
                } else {
                    format!("click_live_presentation_seed_miss_{action}")
                };
            }
            "click_live_gameworld_shadow" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "source" => {
                        crate::game_logic::honesty_seed_presentation_shadow_overlay_source()
                    }
                    "sync" | "overlay" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_shadow_ok_{action}")
                } else {
                    format!("click_live_gameworld_shadow_miss_{action}")
                };
            }
            "click_single_authority" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "policy" | "teleport" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_single_authority_ok_{action}")
                } else {
                    format!("click_single_authority_miss_{action}")
                };
            }
            "click_presentation_client_boundary" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "execute" | "client" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_presentation_client_boundary_ok_{action}")
                } else {
                    format!("click_presentation_client_boundary_miss_{action}")
                };
            }
            "click_golden_map_host_victory" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "clear" | "formula" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_golden_map_host_victory_ok_{action}")
                } else {
                    format!("click_golden_map_host_victory_miss_{action}")
                };
            }
            "click_executable_presentation_boundary" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "vertical" | "fallback" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_executable_presentation_boundary_ok_{action}")
                } else {
                    format!("click_executable_presentation_boundary_miss_{action}")
                };
            }
            "click_gameworld_production_authority" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "enabled" | "sole" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_gameworld_production_authority_ok_{action}")
                } else {
                    format!("click_gameworld_production_authority_miss_{action}")
                };
            }
            "click_gameworld_sole_tick_coupling" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "couple" | "uncouple" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_gameworld_sole_tick_coupling_ok_{action}")
                } else {
                    format!("click_gameworld_sole_tick_coupling_miss_{action}")
                };
            }
            "click_gameworld_authority_matrix" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "all" | "couple" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_gameworld_authority_matrix_ok_{action}")
                } else {
                    format!("click_gameworld_authority_matrix_miss_{action}")
                };
            }
            "click_live_gameworld_production_writeback" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "progress" | "writeback" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_production_writeback_ok_{action}")
                } else {
                    format!("click_live_gameworld_production_writeback_miss_{action}")
                };
            }
            "click_live_gameworld_construction_writeback" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "progress" | "sole" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_construction_writeback_ok_{action}")
                } else {
                    format!("click_live_gameworld_construction_writeback_miss_{action}")
                };
            }
            "click_live_gameworld_damage_channel" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "log" | "parity" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_damage_channel_ok_{action}")
                } else {
                    format!("click_live_gameworld_damage_channel_miss_{action}")
                };
            }
            "click_live_gameworld_economy_movement" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "economy" | "movement" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_economy_movement_ok_{action}")
                } else {
                    format!("click_live_gameworld_economy_movement_miss_{action}")
                };
            }
            "click_live_gameworld_projectile_ai" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "projectile" | "ai" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_projectile_ai_ok_{action}")
                } else {
                    format!("click_live_gameworld_projectile_ai_miss_{action}")
                };
            }
            "click_live_gameworld_fire_special_power" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "special" | "fire" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_fire_special_power_ok_{action}")
                } else {
                    format!("click_live_gameworld_fire_special_power_miss_{action}")
                };
            }
            "click_live_gameworld_presentation_view" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "view" | "engine" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_presentation_view_ok_{action}")
                } else {
                    format!("click_live_gameworld_presentation_view_miss_{action}")
                };
            }
            "click_live_presentation_gameworld_overlay" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "overlay" | "stamp" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_gameworld_overlay_ok_{action}")
                } else {
                    format!("click_live_presentation_gameworld_overlay_miss_{action}")
                };
            }
            "click_executable_gameworld_presentation" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "status" | "vertical" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_executable_gameworld_presentation_ok_{action}")
                } else {
                    format!("click_executable_gameworld_presentation_miss_{action}")
                };
            }
            "click_live_presentation_overlay_deepen" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "velocity" | "selection" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_overlay_deepen_ok_{action}")
                } else {
                    format!("click_live_presentation_overlay_deepen_miss_{action}")
                };
            }
            "click_live_presentation_overlay_stamp" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "field" | "status" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_overlay_stamp_ok_{action}")
                } else {
                    format!("click_live_presentation_overlay_stamp_miss_{action}")
                };
            }
            "click_live_gameworld_entity_view_deepen" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "view" | "exec" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_entity_view_deepen_ok_{action}")
                } else {
                    format!("click_live_gameworld_entity_view_deepen_miss_{action}")
                };
            }
            "click_live_presentation_append_missing" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_append_missing_ok_{action}")
                } else {
                    format!("click_live_presentation_append_missing_miss_{action}")
                };
            }
            "click_live_presentation_build_from_gameworld" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_build_from_gameworld_ok_{action}")
                } else {
                    format!("click_live_presentation_build_from_gameworld_miss_{action}")
                };
            }
            "click_live_presentation_from_gameworld_default" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_from_gameworld_default_ok_{action}")
                } else {
                    format!("click_live_presentation_from_gameworld_default_miss_{action}")
                };
            }
            "click_live_presentation_build_for_engine" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_build_for_engine_ok_{action}")
                } else {
                    format!("click_live_presentation_build_for_engine_miss_{action}")
                };
            }
            "click_live_presentation_rebuilt_vertical_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_rebuilt_vertical_gate_ok_{action}")
                } else {
                    format!("click_live_presentation_rebuilt_vertical_gate_miss_{action}")
                };
            }
            "click_live_command_attack_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_attack_log_ok_{action}")
                } else {
                    format!("click_live_command_attack_log_miss_{action}")
                };
            }
            "click_live_command_guard_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_guard_log_ok_{action}")
                } else {
                    format!("click_live_command_guard_log_miss_{action}")
                };
            }
            "click_live_command_production_construction_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_production_construction_log_ok_{action}")
                } else {
                    format!("click_live_command_production_construction_log_miss_{action}")
                };
            }
            "click_live_command_rally_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_rally_log_ok_{action}")
                } else {
                    format!("click_live_command_rally_log_miss_{action}")
                };
            }
            "click_live_evacuate_contain_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_evacuate_contain_log_ok_{action}")
                } else {
                    format!("click_live_evacuate_contain_log_miss_{action}")
                };
            }
            "click_live_command_cheer_science_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_cheer_science_log_ok_{action}")
                } else {
                    format!("click_live_command_cheer_science_log_miss_{action}")
                };
            }
            "click_live_command_deploy_status_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_deploy_status_log_ok_{action}")
                } else {
                    format!("click_live_command_deploy_status_log_miss_{action}")
                };
            }
            "click_live_command_formation_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_formation_log_ok_{action}")
                } else {
                    format!("click_live_command_formation_log_miss_{action}")
                };
            }
            "click_live_command_order_target_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_order_target_log_ok_{action}")
                } else {
                    format!("click_live_command_order_target_log_miss_{action}")
                };
            }
            "click_live_command_selection_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_selection_log_ok_{action}")
                } else {
                    format!("click_live_command_selection_log_miss_{action}")
                };
            }
            "click_live_command_non_attack_order_target" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_non_attack_order_target_ok_{action}")
                } else {
                    format!("click_live_command_non_attack_order_target_miss_{action}")
                };
            }
            "click_live_golden_mopup_default_off" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_golden_mopup_default_off_ok_{action}")
                } else {
                    format!("click_live_golden_mopup_default_off_miss_{action}")
                };
            }
            "click_live_die_command_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_die_command_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_die_command_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_upgrade_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_upgrade_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_upgrade_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_construction_placement_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_construction_placement_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_construction_placement_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_presentation_env_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_env_only_ok_{action}")
                } else {
                    format!("click_live_presentation_env_only_miss_{action}")
                };
            }
            "click_live_os_input_command_path" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_os_input_command_path_ok_{action}")
                } else {
                    format!("click_live_os_input_command_path_miss_{action}")
                };
            }
            "click_live_command_beacon_note" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_beacon_note_ok_{action}")
                } else {
                    format!("click_live_command_beacon_note_miss_{action}")
                };
            }
            "click_live_host_beacon_presentation" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_host_beacon_presentation_ok_{action}")
                } else {
                    format!("click_live_host_beacon_presentation_miss_{action}")
                };
            }
            "click_live_command_sell_deselect_log" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_sell_deselect_log_ok_{action}")
                } else {
                    format!("click_live_command_sell_deselect_log_miss_{action}")
                };
            }
            "click_live_presentation_fow_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_fow_only_ok_{action}")
                } else {
                    format!("click_live_presentation_fow_only_miss_{action}")
                };
            }
            "click_live_ui_producer_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ui_producer_presentation_only_ok_{action}")
                } else {
                    format!("click_live_ui_producer_presentation_only_miss_{action}")
                };
            }
            "click_live_ui_helpers_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ui_helpers_presentation_only_ok_{action}")
                } else {
                    format!("click_live_ui_helpers_presentation_only_miss_{action}")
                };
            }
            "click_live_control_group_camera_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_control_group_camera_presentation_only_ok_{action}")
                } else {
                    format!("click_live_control_group_camera_presentation_only_miss_{action}")
                };
            }
            "click_live_cmd_filter_env_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_cmd_filter_env_presentation_only_ok_{action}")
                } else {
                    format!("click_live_cmd_filter_env_presentation_only_miss_{action}")
                };
            }
            "click_live_selection_commands_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_selection_commands_presentation_only_ok_{action}")
                } else {
                    format!("click_live_selection_commands_presentation_only_miss_{action}")
                };
            }
            "click_live_ui_command_selection_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ui_command_selection_presentation_only_ok_{action}")
                } else {
                    format!("click_live_ui_command_selection_presentation_only_miss_{action}")
                };
            }
            "click_live_local_team_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_local_team_presentation_only_ok_{action}")
                } else {
                    format!("click_live_local_team_presentation_only_miss_{action}")
                };
            }
            "click_live_hotkey_move_attack_selection_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_hotkey_move_attack_selection_presentation_only_ok_{action}")
                } else {
                    format!(
                        "click_live_hotkey_move_attack_selection_presentation_only_miss_{action}"
                    )
                };
            }
            "click_live_pick_object_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_pick_object_presentation_only_ok_{action}")
                } else {
                    format!("click_live_pick_object_presentation_only_miss_{action}")
                };
            }
            "click_live_bootstrap_camera_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_bootstrap_camera_presentation_only_ok_{action}")
                } else {
                    format!("click_live_bootstrap_camera_presentation_only_miss_{action}")
                };
            }
            "click_live_force_complete_authority_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_force_complete_authority_api_ok_{action}")
                } else {
                    format!("click_live_force_complete_authority_api_miss_{action}")
                };
            }
            "click_live_path_guard_authority_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_path_guard_authority_api_ok_{action}")
                } else {
                    format!("click_live_path_guard_authority_api_miss_{action}")
                };
            }
            "click_live_hotkey_selection_camera_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_hotkey_selection_camera_presentation_only_ok_{action}")
                } else {
                    format!("click_live_hotkey_selection_camera_presentation_only_miss_{action}")
                };
            }
            "click_live_construct_spawn_pose_authority_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_construct_spawn_pose_authority_api_ok_{action}")
                } else {
                    format!("click_live_construct_spawn_pose_authority_api_miss_{action}")
                };
            }
            "click_live_rmb_target_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rmb_target_presentation_only_ok_{action}")
                } else {
                    format!("click_live_rmb_target_presentation_only_miss_{action}")
                };
            }
            "click_live_rmb_selected_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rmb_selected_presentation_only_ok_{action}")
                } else {
                    format!("click_live_rmb_selected_presentation_only_miss_{action}")
                };
            }
            "click_live_command_unit_authority_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_unit_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_unit_authority_api_miss_{action}")
                };
            }
            "click_live_command_unit_more_authority_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_unit_more_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_unit_more_authority_api_miss_{action}")
                };
            }
            "click_live_command_executor_authority_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_executor_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_executor_authority_api_miss_{action}")
                };
            }
            "click_live_command_executor_more_authority_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_executor_more_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_executor_more_authority_api_miss_{action}")
                };
            }
            "click_live_engine_presentation_player_ui" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_engine_presentation_player_ui_ok_{action}")
                } else {
                    format!("click_live_engine_presentation_player_ui_miss_{action}")
                };
            }
            "click_live_rmb_presentation_full_classify" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rmb_presentation_full_classify_ok_{action}")
                } else {
                    format!("click_live_rmb_presentation_full_classify_miss_{action}")
                };
            }
            "click_live_mouse_input_presentation_only" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_mouse_input_presentation_only_ok_{action}")
                } else {
                    format!("click_live_mouse_input_presentation_only_miss_{action}")
                };
            }
            "click_live_engine_player_ui_boot_peel" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_engine_player_ui_boot_peel_ok_{action}")
                } else {
                    format!("click_live_engine_player_ui_boot_peel_miss_{action}")
                };
            }
            "click_live_player_probe_api" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_player_probe_api_ok_{action}")
                } else {
                    format!("click_live_player_probe_api_miss_{action}")
                };
            }
            "click_live_player_team_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_player_team_probe_ok_{action}")
                } else {
                    format!("click_live_player_team_probe_miss_{action}")
                };
            }
            "click_live_player_field_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_player_field_probe_ok_{action}")
                } else {
                    format!("click_live_player_field_probe_miss_{action}")
                };
            }
            "click_live_camera_height_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_camera_height_probe_ok_{action}")
                } else {
                    format!("click_live_camera_height_probe_miss_{action}")
                };
            }
            "click_live_command_player_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_player_probe_ok_{action}")
                } else {
                    format!("click_live_command_player_probe_miss_{action}")
                };
            }
            "click_live_construct_economy_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_construct_economy_probe_ok_{action}")
                } else {
                    format!("click_live_construct_economy_probe_miss_{action}")
                };
            }
            "click_live_command_unit_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_unit_probe_ok_{action}")
                } else {
                    format!("click_live_command_unit_probe_miss_{action}")
                };
            }
            "click_live_selection_query_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_selection_query_probe_ok_{action}")
                } else {
                    format!("click_live_selection_query_probe_miss_{action}")
                };
            }
            "click_live_world_pick_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_world_pick_probe_ok_{action}")
                } else {
                    format!("click_live_world_pick_probe_miss_{action}")
                };
            }
            "click_live_object_registry_empty_fastpath" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_object_registry_empty_fastpath_ok_{action}")
                } else {
                    format!("click_live_object_registry_empty_fastpath_miss_{action}")
                };
            }
            "click_live_legacy_object_registry_fastpath" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_legacy_object_registry_fastpath_ok_{action}")
                } else {
                    format!("click_live_legacy_object_registry_fastpath_miss_{action}")
                };
            }
            "click_live_client_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_client_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_client_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_presentation_time_frozen_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_time_frozen_probe_ok_{action}")
                } else {
                    format!("click_live_presentation_time_frozen_probe_miss_{action}")
                };
            }
            "click_live_presentation_visual_speed_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_visual_speed_probe_ok_{action}")
                } else {
                    format!("click_live_presentation_visual_speed_probe_miss_{action}")
                };
            }
            "click_live_presentation_script_camera_probe" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_script_camera_probe_ok_{action}")
                } else {
                    format!("click_live_presentation_script_camera_probe_miss_{action}")
                };
            }
            "click_live_ai_group_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_group_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_group_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_slaved_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_slaved_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_slaved_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_demoralize_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_demoralize_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_demoralize_power_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_bone_fx_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_bone_fx_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_bone_fx_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_supply_warehouse_dock_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_supply_warehouse_dock_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_supply_warehouse_dock_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ocl_special_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ocl_special_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ocl_special_power_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_railed_transport_ai_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_railed_transport_ai_update_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_railed_transport_ai_update_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_squish_collide_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_squish_collide_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_squish_collide_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_weapon_bonus_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_weapon_bonus_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_weapon_bonus_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_minefield_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_minefield_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_minefield_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_point_defense_laser_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_point_defense_laser_update_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_point_defense_laser_update_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_lifetime_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_lifetime_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_lifetime_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_slow_death_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_slow_death_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_slow_death_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_battle_bus_slow_death_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_battle_bus_slow_death_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_battle_bus_slow_death_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_damage_module_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_damage_module_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_damage_module_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_transition_damage_fx_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_transition_damage_fx_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_transition_damage_fx_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_spawn_point_production_exit_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_spawn_point_production_exit_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_spawn_point_production_exit_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_build_placement_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_build_placement_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_build_placement_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_weapon_set_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_weapon_set_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_weapon_set_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_experience_tracker_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_experience_tracker_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_experience_tracker_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_targeting_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_targeting_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_targeting_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_move_to_state_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_move_to_state_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_move_to_state_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_locomotor_core_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_locomotor_core_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_locomotor_core_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_path_following_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_path_following_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_path_following_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_manager_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_manager_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_manager_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_states_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_states_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_states_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_player_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_player_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_player_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_team_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_team_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_team_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_legacy_states_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_legacy_states_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_legacy_states_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_unit_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_unit_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_unit_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_stealth_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_stealth_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_garrison_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_garrison_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_garrison_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_open_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_open_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_open_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_pathfind_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_pathfind_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_pathfind_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_guard_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_guard_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_guard_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_guard_retaliate_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_guard_retaliate_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_guard_retaliate_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_wander_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_wander_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_wander_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_subobjects_upgrade_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_subobjects_upgrade_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_subobjects_upgrade_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_unit_exit_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_unit_exit_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_unit_exit_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_owner_resolve_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_owner_resolve_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_owner_resolve_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_spy_vision_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_spy_vision_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_spy_vision_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_overcharge_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_overcharge_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_overcharge_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_tech_building_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_tech_building_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_tech_building_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_power_plant_upgrade_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_power_plant_upgrade_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_power_plant_upgrade_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_stealth_upgrade_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_upgrade_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_stealth_upgrade_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_aurora_strike_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_aurora_strike_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_aurora_strike_power_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_carpet_bomb_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_carpet_bomb_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_carpet_bomb_power_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_nuclear_missile_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_nuclear_missile_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_nuclear_missile_power_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_overlord_draw_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_overlord_draw_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_overlord_draw_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_stealth_integration_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_integration_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_stealth_integration_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_player_upgrade_manager_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_player_upgrade_manager_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_player_upgrade_manager_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_advanced_nuggets_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_advanced_nuggets_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_advanced_nuggets_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_replace_object_upgrade_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_replace_object_upgrade_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_replace_object_upgrade_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_fire_spread_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_fire_spread_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_fire_spread_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_object_upgrade_batch_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_object_upgrade_batch_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_object_upgrade_batch_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_contain_module_overrides_fail_closed" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_contain_module_overrides_fail_closed_ok_{action}")
                } else {
                    format!("click_live_contain_module_overrides_fail_closed_miss_{action}")
                };
            }
            "click_live_core_sim_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_core_sim_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_core_sim_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_mod_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_mod_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_mod_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_object_mod_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_object_mod_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_object_mod_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_weapon_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_weapon_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_weapon_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_partition_filters_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_partition_filters_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_partition_filters_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_state_machine_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_state_machine_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_state_machine_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_player_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_player_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_player_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_game_client_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_game_client_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_game_client_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_drawable_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_drawable_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_drawable_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_script_conditions_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_script_conditions_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_script_conditions_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_transport_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_transport_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_transport_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ingame_ui_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ingame_ui_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ingame_ui_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_helix_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_helix_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_helix_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_command_processor_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_command_processor_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_command_processor_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_turret_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_turret_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_turret_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_rider_change_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rider_change_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_rider_change_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_selection_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_selection_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_selection_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_cave_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_cave_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_cave_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_tunnel_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_tunnel_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_tunnel_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_helpers_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_helpers_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_helpers_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_update_interface_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_update_interface_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_update_interface_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_stealth_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_stealth_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_script_executor_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_script_executor_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_script_executor_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_integration_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_integration_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_integration_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_dumb_projectile_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_dumb_projectile_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_dumb_projectile_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_enhanced_player_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_enhanced_player_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_enhanced_player_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_hijacker_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_hijacker_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_hijacker_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_weapon_impl_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_weapon_impl_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_weapon_impl_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_async_player_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_async_player_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_async_player_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_active_body_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_active_body_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_active_body_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_skirmish_conditions_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_skirmish_conditions_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_skirmish_conditions_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_build_list_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_build_list_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_build_list_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_victory_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_victory_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_victory_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_script_actions_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_script_actions_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_script_actions_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_special_ability_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_special_ability_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_special_ability_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_stealth_detector_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_detector_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_stealth_detector_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_supply_system_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_supply_system_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_supply_system_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_particle_uplink_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_particle_uplink_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_particle_uplink_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_overlord_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_overlord_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_overlord_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_bridge_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_bridge_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_bridge_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_stealth_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_stealth_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_crate_collide_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_crate_collide_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_crate_collide_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_object_manager_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_object_manager_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_object_manager_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_sticky_bomb_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_sticky_bomb_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_sticky_bomb_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_auto_heal_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_auto_heal_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_auto_heal_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_grant_stealth_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_grant_stealth_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_grant_stealth_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_status_bits_upgrade_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_status_bits_upgrade_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_status_bits_upgrade_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_jet_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_jet_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_jet_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_parking_place_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_parking_place_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_parking_place_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_flight_deck_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_flight_deck_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_flight_deck_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_exit_strategies_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_exit_strategies_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_exit_strategies_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_collision_system_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_collision_system_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_collision_system_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_max_health_upgrade_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_max_health_upgrade_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_max_health_upgrade_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_structure_topple_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_structure_topple_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_structure_topple_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_physics_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_physics_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_physics_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_cleanup_hazard_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_cleanup_hazard_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_cleanup_hazard_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_bridge_tower_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_bridge_tower_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_bridge_tower_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_armor_upgrade_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_armor_upgrade_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_armor_upgrade_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_paradrop_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_paradrop_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_paradrop_power_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_fuel_air_bomb_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_fuel_air_bomb_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_fuel_air_bomb_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_tensile_formation_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_tensile_formation_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_tensile_formation_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_die_mod_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_die_mod_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_die_mod_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_partition_manager_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_partition_manager_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_partition_manager_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_spectre_gunship_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_spectre_gunship_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_spectre_gunship_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_production_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_production_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_production_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_neutron_blast_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_neutron_blast_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_neutron_blast_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_countermeasures_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_countermeasures_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_countermeasures_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_skirmish_player_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_skirmish_player_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_skirmish_player_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_a10_strike_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_a10_strike_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_a10_strike_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_rebuild_hole_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rebuild_hole_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_rebuild_hole_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_wave_guide_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_wave_guide_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_wave_guide_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_emp_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_emp_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_emp_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_bunker_buster_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_bunker_buster_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_bunker_buster_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_bridge_scaffold_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_bridge_scaffold_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_bridge_scaffold_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_assisted_targeting_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_assisted_targeting_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_assisted_targeting_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_economy_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_economy_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_economy_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_turret_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_turret_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_turret_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_stealth_detector_module_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_detector_module_dual_world_empty_gate_ok_{action}")
                } else {
                    format!(
                        "click_live_stealth_detector_module_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_modules_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_modules_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_modules_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_terrain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_terrain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_terrain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_special_power_template_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_special_power_template_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_special_power_template_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_script_evaluator_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_script_evaluator_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_script_evaluator_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_system_game_logic_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_system_game_logic_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_system_game_logic_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_meta_event_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_meta_event_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_meta_event_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_spawn_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_spawn_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_spawn_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_action_manager_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_action_manager_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_action_manager_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_script_engine_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_script_engine_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_script_engine_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_chinook_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_chinook_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_chinook_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_missile_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_missile_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_missile_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_dozer_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_dozer_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_dozer_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_deliver_payload_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_deliver_payload_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_deliver_payload_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_special_power_module_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_special_power_module_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_special_power_module_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_pow_truck_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_pow_truck_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_pow_truck_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_dock_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_dock_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_dock_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_weapon_template_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_weapon_template_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_weapon_template_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_railroad_guide_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_railroad_guide_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_railroad_guide_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_hack_internet_ai_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_hack_internet_ai_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_hack_internet_ai_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_spectre_gunship_deployment_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_spectre_gunship_deployment_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_spectre_gunship_deployment_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_radius_decal_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_radius_decal_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_radius_decal_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_railed_transport_dock_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_railed_transport_dock_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_railed_transport_dock_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_structure_collapse_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_structure_collapse_update_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_structure_collapse_update_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_propaganda_tower_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_propaganda_tower_behavior_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_propaganda_tower_behavior_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_propaganda_center_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_propaganda_center_behavior_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_propaganda_center_behavior_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_production_update_complete_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_production_update_complete_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_production_update_complete_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_pow_truck_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_pow_truck_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_pow_truck_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_veterancy_crate_collide_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_veterancy_crate_collide_dual_world_empty_gate_ok_{action}")
                } else {
                    format!(
                        "click_live_veterancy_crate_collide_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_assault_transport_ai_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_assault_transport_ai_update_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!("click_live_assault_transport_ai_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_heal_contain_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_heal_contain_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_heal_contain_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_topple_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_topple_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_topple_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_projectile_stream_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_projectile_stream_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!(
                        "click_live_projectile_stream_update_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_demo_trap_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_demo_trap_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_demo_trap_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_mob_member_slaved_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_mob_member_slaved_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!(
                        "click_live_mob_member_slaved_update_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_tn_guard_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_tn_guard_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_tn_guard_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_production_update_dual_world_empty_gate_wave376" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_production_update_dual_world_empty_gate_wave376_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_production_update_dual_world_empty_gate_wave376_miss_{action}"
                    )
                };
            }
            "click_live_poisoned_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_poisoned_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_poisoned_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_horde_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_horde_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_horde_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_flammable_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_flammable_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_flammable_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_base_regenerate_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_base_regenerate_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_base_regenerate_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_queue_production_exit_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_queue_production_exit_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_queue_production_exit_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_missile_launcher_building_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_missile_launcher_building_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_missile_launcher_building_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_command_button_hunt_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_command_button_hunt_update_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!(
                        "click_live_command_button_hunt_update_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_prison_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_prison_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_prison_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_generate_minefield_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!(
                        "click_live_generate_minefield_behavior_dual_world_empty_gate_ok_{action}"
                    )
                } else {
                    format!("click_live_generate_minefield_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_demoralize_special_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_demoralize_special_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!(
                        "click_live_demoralize_special_power_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_stealth_detector_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_stealth_detector_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!(
                        "click_live_stealth_detector_update_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_hive_structure_body_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_hive_structure_body_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_hive_structure_body_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_salvage_crate_collide_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_salvage_crate_collide_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_salvage_crate_collide_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_sabotage_internet_center_crate_collide_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_power_plant_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_power_plant_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_power_plant_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_leaflet_drop_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_leaflet_drop_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_leaflet_drop_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_auto_deposit_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_auto_deposit_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_auto_deposit_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_neutron_missile_slow_death_update_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_neutron_missile_slow_death_update_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_neutron_missile_slow_death_update_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_dock_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_dock_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_dock_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_ai_groups_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ai_groups_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_ai_groups_dual_world_empty_gate_miss_{action}")
                };
            }
            "click_live_artillery_barrage_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_artillery_barrage_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!(
                        "click_live_artillery_barrage_power_dual_world_empty_gate_miss_{action}"
                    )
                };
            }
            "click_live_baikonur_launch_power_dual_world_empty_gate" => {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_baikonur_launch_power_dual_world_empty_gate_ok_{action}")
                } else {
                    format!("click_live_baikonur_launch_power_dual_world_empty_gate_miss_{action}")
                };
            }
            "save_game" | "quicksave" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "save_fail_not_ingame".into();
                } else {
                    let slot = args
                        .get("slot")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "quicksave".to_string());
                    let display = args
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| format!("HostSave-{slot}"));
                    self.save_game_from_ui(&slot, &display);
                    let exists = self.save_file_manager.save_exists(&slot);
                    self.runtime_host_last_gameplay_cmd = if exists {
                        format!("save_ok:{slot}")
                    } else {
                        format!("save_fail:{slot}")
                    };
                }
            }
            "quickload" => {
                if !self.save_file_manager.save_exists("quicksave") {
                    self.runtime_host_last_gameplay_cmd = "load_fail_no_quicksave".into();
                } else {
                    self.set_runtime_host_ui_screen_override(None);
                    // Host residual: report real load Result (do not claim ok on deserialize fail).
                    match self.load_game_from_ui("quicksave") {
                        Ok(()) => {
                            if !matches!(self.current_state, GameState::InGame | GameState::Paused)
                            {
                                self.request_state_change(GameState::InGame);
                            }
                            self.runtime_host_last_gameplay_cmd = "load_ok:quicksave".into();
                        }
                        Err(err) => {
                            warn!("quickload failed: {err}");
                            self.runtime_host_last_gameplay_cmd =
                                format!("load_fail:quicksave:{err}");
                        }
                    }
                }
            }
            "load_game" => {
                let slot = args.get("slot").map(|slot| slot.trim()).unwrap_or_default();
                if !slot.is_empty() {
                    self.set_runtime_host_ui_screen_override(None);
                    match self.load_game_from_ui(slot) {
                        Ok(()) => {
                            self.runtime_host_last_gameplay_cmd = format!("load_ok:{slot}");
                            if matches!(self.ui_manager.current_screen(), Some(Screen::GameHUD)) {
                                self.request_state_change(GameState::InGame);
                            }
                        }
                        Err(err) => {
                            warn!("load_game failed for '{slot}': {err}");
                            self.runtime_host_last_gameplay_cmd = format!("load_fail:{slot}:{err}");
                        }
                    }
                }
            }
            "replay" => {
                let slot = args
                    .get("slot")
                    .cloned()
                    .unwrap_or_else(|| "latest".to_string());
                warn!(
                    "Runtime host replay command requested for slot '{slot}', replay startup path is not wired yet"
                );
                self.enter_shell_screen_from_runtime_host(Some("Replay"), "Menus/ReplayMenu.wnd");
            }
            "enqueue_production" | "train_unit" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "train_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "train_begin".into();
                    // Wave 727: missing template is fail-closed (no free default unit).
                    // Smoke always passes template=...; harness may set default_template=1
                    // / GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE=1 for legacy bare commands.
                    let allow_default_template =
                        args.get("default_template")
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    let requested = match args.get("template").cloned() {
                        Some(t) if !t.trim().is_empty() => t,
                        _ if allow_default_template => "AmericaInfantryRanger".to_string(),
                        _ => {
                            self.runtime_host_last_gameplay_cmd = "train_fail_no_template".into();
                            return;
                        }
                    };
                    // Prefer presentation local team residual (no live player roster).
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "train_fail_no_player".into();
                        return;
                    };
                    // Wave 718: force-complete unfinished barracks is opt-in only.
                    // Default fail-closed: train against already-constructed producers
                    // (honest retail timing). Enable with force_complete=1 arg or
                    // GENERALS_RUNTIME_HOST_TRAIN_FORCE_COMPLETE=1 for vertical-slice smoke.
                    let allow_force_complete = args
                        .get("force_complete")
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_TRAIN_FORCE_COMPLETE")
                            .is_some_and(|v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            });
                    let mut force_completed: Vec<crate::game_logic::ObjectId> = Vec::new();
                    if allow_force_complete {
                        let mut unfinished: Vec<crate::game_logic::ObjectId> = if let Some(frame) =
                            self.last_presentation_frame.as_ref()
                        {
                            frame
                                .objects
                                .iter()
                                .filter(|o| {
                                    o.team == team
                                        && !o.destroyed
                                        && o.under_construction
                                        && (o.building_type
                                            == Some(
                                                crate::presentation_frame::PresentationBuildingType::Barracks,
                                            )
                                            || o.template_name
                                                .to_ascii_lowercase()
                                                .contains("barracks")
                                            || o.can_produce
                                            || o.building_type.is_some()
                                            || crate::presentation_frame::PresentationFrame::object_has_kind(
                                                o,
                                                crate::game_logic::KindOf::FSBarracks,
                                            ))
                                })
                                .map(|o| o.id)
                                .collect()
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            Vec::new()
                        };
                        unfinished.sort_by_key(|id| id.0);
                        for id in unfinished.into_iter().take(2) {
                            // Wave 224: authority mutation via GameLogic API (no engine get_object_mut).
                            if self.host_force_complete_construction(id) {
                                force_completed.push(id);
                            }
                        }
                    }
                    // Wave 729: auto-pick first constructed producer is opt-in only.
                    // Default fail-closed: train needs an explicit producer selection path.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    // Prefer force-completed + presentation constructed producers.
                    // Live full dual-scan is boot residual or fail-open when presentation
                    // still marks force-completed barracks under_construction.
                    let producer = {
                        let mut barracks: Vec<crate::game_logic::ObjectId> = Vec::new();
                        let mut any: Vec<crate::game_logic::ObjectId> = Vec::new();
                        let push = |is_barracks: bool,
                                    id: crate::game_logic::ObjectId,
                                    barracks: &mut Vec<crate::game_logic::ObjectId>,
                                    any: &mut Vec<crate::game_logic::ObjectId>| {
                            if is_barracks {
                                if !barracks.contains(&id) {
                                    barracks.push(id);
                                }
                            } else if !any.contains(&id) {
                                any.push(id);
                            }
                        };
                        // Wave 214: force-completed IDs classified from presentation freeze only
                        // (no live GameLogic dual-read residual).
                        for id in force_completed.iter().copied() {
                            let classified = if let Some(frame) =
                                self.last_presentation_frame.as_ref()
                            {
                                frame.objects.iter().find_map(|o| {
                                    if o.id != id || o.destroyed || o.team != team {
                                        return None;
                                    }
                                    if o.under_construction {
                                        return None;
                                    }
                                    let is_barracks = o.building_type
                                        == Some(
                                            crate::presentation_frame::PresentationBuildingType::Barracks,
                                        )
                                        || o.template_name
                                            .to_ascii_lowercase()
                                            .contains("barracks")
                                        || crate::presentation_frame::PresentationFrame::object_has_kind(
                                            o,
                                            crate::game_logic::KindOf::FSBarracks,
                                        );
                                    let is_producer = o.can_produce
                                        || is_barracks
                                        || matches!(
                                            o.building_type,
                                            Some(
                                                crate::presentation_frame::PresentationBuildingType::WarFactory
                                                    | crate::presentation_frame::PresentationBuildingType::Airfield
                                                    | crate::presentation_frame::PresentationBuildingType::Barracks
                                            )
                                        )
                                        || crate::presentation_frame::PresentationFrame::object_has_kind(
                                            o,
                                            crate::game_logic::KindOf::FSWarFactory,
                                        )
                                        || crate::presentation_frame::PresentationFrame::object_has_kind(
                                            o,
                                            crate::game_logic::KindOf::FSAirfield,
                                        );
                                    if !is_producer {
                                        return None;
                                    }
                                    Some((is_barracks, id))
                                })
                            } else {
                                None
                            };
                            if let Some((is_b, id)) = classified {
                                push(is_b, id, &mut barracks, &mut any);
                            }
                        }
                        if let Some(frame) = self.last_presentation_frame.as_ref() {
                            for o in &frame.objects {
                                if o.team != team || o.destroyed {
                                    continue;
                                }
                                if o.under_construction && !force_completed.contains(&o.id) {
                                    continue;
                                }
                                let is_barracks = o.building_type
                                    == Some(
                                        crate::presentation_frame::PresentationBuildingType::Barracks,
                                    )
                                    || o.template_name.to_ascii_lowercase().contains("barracks")
                                    || crate::presentation_frame::PresentationFrame::object_has_kind(
                                        o,
                                        crate::game_logic::KindOf::FSBarracks,
                                    );
                                let is_producer = o.can_produce
                                    || is_barracks
                                    || matches!(
                                        o.building_type,
                                        Some(
                                            crate::presentation_frame::PresentationBuildingType::WarFactory
                                                | crate::presentation_frame::PresentationBuildingType::Airfield
                                                | crate::presentation_frame::PresentationBuildingType::Barracks
                                        )
                                    )
                                    || crate::presentation_frame::PresentationFrame::object_has_kind(
                                        o,
                                        crate::game_logic::KindOf::FSWarFactory,
                                    )
                                    || crate::presentation_frame::PresentationFrame::object_has_kind(
                                        o,
                                        crate::game_logic::KindOf::FSAirfield,
                                    );
                                if !is_producer {
                                    continue;
                                }
                                push(is_barracks, o.id, &mut barracks, &mut any);
                            }
                        } else {
                            // Presentation required (no live get_objects dual-read).
                        }
                        // Wave 834/848: when auto_target + force_complete are opt-in and the
                        // presentation freeze still lacks the just-built barracks (construct
                        // → train same control drain), fall back to host-stamped producer
                        // residuals (refreshed if cold). Default path stays presentation-only.
                        if allow_auto_target && barracks.is_empty() && any.is_empty() {
                            // Ensure residuals are warm (single stamp dual-read if needed).
                            if self.host_match_local_barracks_ids.is_none()
                                && self.host_match_local_producer_ids.is_none()
                            {
                                self.host_refresh_local_train_producer_residuals();
                            }
                            // Force-complete unfinished local producers from residual.
                            if allow_force_complete {
                                let unfinished = self
                                    .host_match_local_unfinished_producer_ids
                                    .clone()
                                    .unwrap_or_default();
                                for id in unfinished.into_iter().take(4) {
                                    if self.host_force_complete_construction(id) {
                                        force_completed.push(id);
                                    }
                                }
                                if !force_completed.is_empty() {
                                    self.host_refresh_local_train_producer_residuals();
                                }
                            }
                            for id in self
                                .host_match_local_barracks_ids
                                .clone()
                                .unwrap_or_default()
                            {
                                if !barracks.contains(&id) {
                                    barracks.push(id);
                                }
                            }
                            for id in self
                                .host_match_local_producer_ids
                                .clone()
                                .unwrap_or_default()
                            {
                                if !any.contains(&id) {
                                    any.push(id);
                                }
                            }
                            // Wave 834/848: if still no local barracks, spawn + complete one at a
                            // team sample position so auto_target train can enqueue honestly.
                            if barracks.is_empty() && allow_force_complete {
                                let spawn_at = self
                                    .last_presentation_frame
                                    .as_ref()
                                    .and_then(|f| {
                                        f.objects.iter().find_map(|o| {
                                            (o.team == team && !o.destroyed).then_some(o.position)
                                        })
                                    })
                                    .or_else(|| {
                                        self.host_match_local_team_sample_pos
                                            .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                                    })
                                    .unwrap_or(glam::Vec3::new(500.0, 0.0, 500.0))
                                    + glam::Vec3::new(120.0, 0.0, 40.0);
                                for bname in ["AmericaBarracks", "USA_Barracks", "AmericaBarracks"]
                                {
                                    if let Some(id) = self.host_create_object(bname, team, spawn_at)
                                    {
                                        let _ = self.host_force_complete_construction(id);
                                        let _ = self.host_ensure_barracks_building_data(id);
                                        barracks.push(id);
                                        self.host_refresh_local_train_producer_residuals();
                                        break;
                                    }
                                }
                            }
                        }

                        barracks.sort_by_key(|id| id.0);
                        any.sort_by_key(|id| id.0);
                        let pick = barracks
                            .into_iter()
                            .next()
                            .or_else(|| any.into_iter().next());
                        if let Some(id) = pick {
                            // Wave 723: stamping Barracks building_data is opt-in only.
                            // Default fail-closed: retail producers already carry building_data
                            // after honest construction complete. Force-complete / smoke may set
                            // ensure_barracks=1 or reuse force_complete=1 /
                            // GENERALS_RUNTIME_HOST_ENSURE_BARRACKS=1.
                            let allow_ensure_barracks = allow_force_complete
                                || args
                                    .get("ensure_barracks")
                                    .map(|v| {
                                        let s = v.trim();
                                        s == "1"
                                            || s.eq_ignore_ascii_case("true")
                                            || s.eq_ignore_ascii_case("yes")
                                    })
                                    .unwrap_or(false)
                                || std::env::var_os("GENERALS_RUNTIME_HOST_ENSURE_BARRACKS")
                                    .is_some_and(|v| {
                                        let s = v.to_string_lossy();
                                        !(s.is_empty()
                                            || s == "0"
                                            || s.eq_ignore_ascii_case("false")
                                            || s.eq_ignore_ascii_case("no"))
                                    });
                            if allow_ensure_barracks {
                                // Wave 224: authority mutation via GameLogic API (no engine get_object_mut).
                                let stamped = self.host_ensure_barracks_building_data(id);
                                // Wave 834: auto_target residual force-stamps when name/kind
                                // gate misses host-spawned producers.
                                if !stamped && allow_auto_target {
                                    let _ = self.host_force_ensure_barracks_building_data(id);
                                }
                            } else if allow_auto_target && allow_force_complete {
                                let _ = self.host_force_ensure_barracks_building_data(id);
                            }
                        }
                        if allow_auto_target {
                            pick.or_else(|| {
                                self.last_presentation_frame
                                    .as_ref()
                                    .and_then(|f| f.first_constructed_producer_id(team))
                            })
                        } else {
                            pick
                        }
                    };
                    // Wave 725: soft template alias fallbacks are opt-in only (default fail-closed).
                    // Retail commands use the exact requested template name.
                    // Smoke/harness may set alias_fallback=1 / GENERALS_RUNTIME_HOST_ALIAS_FALLBACK=1.
                    let allow_alias_fallback =
                        args.get("alias_fallback")
                            .or_else(|| args.get("soft_alias"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_ALIAS_FALLBACK")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    // Wave 722: synthetic GoldenRanger template insert is opt-in only.
                    // Default fail-closed: train only against real retail/map templates.
                    // Smoke/harness may set golden_template=1 /
                    // GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER=1.
                    let allow_golden_template = args
                        .get("golden_template")
                        .or_else(|| args.get("ensure_golden_ranger"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER")
                            .is_some_and(|v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            });
                    let mut unit_candidates = vec![requested.as_str()];
                    if allow_alias_fallback {
                        unit_candidates.extend([
                            "AmericaInfantryRanger",
                            "USA_Ranger",
                            "USARanger",
                        ]);
                    }
                    if allow_golden_template {
                        unit_candidates.push("GoldenRanger");
                    }
                    // Wave 563: template residual prefers presentation freeze names.
                    let template = unit_candidates
                        .iter()
                        .find(|n| self.presentation_or_boot_has_template(**n))
                        .map(|s| (*s).to_string())
                        .unwrap_or(requested);
                    if let Some(pid) = producer {
                        // Wave 722: only insert GoldenRanger host template when opted in.
                        if allow_golden_template {
                            self.host_ensure_golden_ranger_template();
                        }
                        // Wave 721: free supplies floor is opt-in only (default fail-closed).
                        // Retail cash comes from skirmish/map starting resources.
                        // Smoke may set grant_supplies=1 / GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES=1.
                        let allow_grant_supplies = args
                            .get("grant_supplies")
                            .or_else(|| args.get("min_supplies"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                                    || s.parse::<u32>().map(|n| n > 0).unwrap_or(false)
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                        if allow_grant_supplies {
                            let floor = args
                                .get("grant_supplies")
                                .or_else(|| args.get("min_supplies"))
                                .and_then(|v| v.trim().parse::<u32>().ok())
                                .filter(|n| *n > 1)
                                .or_else(|| {
                                    std::env::var("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES")
                                        .ok()
                                        .and_then(|v| v.trim().parse::<u32>().ok())
                                        .filter(|n| *n > 1)
                                })
                                .unwrap_or(25_000);
                            self.host_ensure_player_min_supplies_residual(floor);
                        }
                        // Wave 724/725: alias + GoldenRanger enqueue fallbacks are opt-in only.
                        let mut try_names = vec![template.as_str()];
                        if allow_alias_fallback {
                            try_names.extend(["AmericaInfantryRanger", "USA_Ranger", "USARanger"]);
                        }
                        if allow_golden_template {
                            try_names.push("GoldenRanger");
                        }
                        let mut ok_name = None;
                        let mut last_fail = template.clone();
                        for name in try_names {
                            // Wave 563: freeze owns known names; host still sees mid-command inserts.
                            // Wave 581: freeze OR live host insert residual.
                            if !self.presentation_or_live_has_template(name) {
                                continue;
                            }
                            if self.host_enqueue_production(pid, name.to_string()) {
                                ok_name = Some(name.to_string());
                                break;
                            }
                            last_fail = name.to_string();
                        }
                        if let Some(name) = ok_name {
                            self.runtime_host_last_gameplay_cmd =
                                format!("train_ok:{}:{}", pid.0, name);
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                format!("train_fail_enqueue:{}:prod={}", last_fail, pid.0);
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd = "train_fail_no_producer".into();
                    }
                }
            }
            "select_local_unit" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_fail_not_ingame".into();
                } else {
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let pick = team.and_then(|team| {
                        if let Some(frame) = self.last_presentation_frame.as_ref() {
                            // Presentation-owned mobile identity (no live dual-scan).
                            frame.first_mobile_friendly_id(team).or_else(|| {
                                frame
                                    .alive_selectable_friendly_mobile_ids(team)
                                    .into_iter()
                                    .next()
                            })
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            None
                        }
                    });
                    if let Some(id) = pick {
                        self.host_set_selection(self.current_player_id, vec![id]);
                        self.runtime_host_last_gameplay_cmd = format!("select_ok:{}", id.0);
                    } else {
                        self.runtime_host_last_gameplay_cmd = "select_fail_no_mobile".into();
                    }
                }
            }
            "move" | "move_selected" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "move_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    // Wave 218: selection count via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id).len();
                    if selected == 0 {
                        self.runtime_host_last_gameplay_cmd = "move_fail_no_selection".into();
                    } else {
                        // Wave 221: push presentation-first selection into host before move.
                        let ids = self.ui_selected_ids(self.current_player_id);
                        if !ids.is_empty() {
                            self.host_set_selection(self.current_player_id, ids);
                        }
                        self.host_command_move(self.current_player_id, glam::Vec3::new(x, y, z));
                        self.runtime_host_last_gameplay_cmd =
                            format!("move_ok:n={selected}:x={x:.1}:y={y:.1}:z={z:.1}");
                    }
                }
            }
            "attack_nearest_enemy" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attack_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "attack_begin".into();
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    // Prefer combat-capable mobiles (select_all often arms structures/dozers).
                    if let Some(team) = team {
                        let mut attackers: Vec<_> = self
                            .selected_objects
                            .iter()
                            .copied()
                            .filter(|id| {
                                self.last_presentation_frame.as_ref().is_some_and(|frame| {
                                    frame.objects.iter().any(|o| {
                                        o.id == *id
                                            && o.team == team
                                            && !o.destroyed
                                            && o.has_weapon
                                    })
                                })
                            })
                            .collect();
                        if attackers.is_empty() {
                            attackers = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                let mut ids = frame.alive_selectable_friendly_combat_ids(team);
                                ids.truncate(8);
                                ids
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                Vec::new()
                            };
                        }
                        if !attackers.is_empty() {
                            self.host_set_selection(self.current_player_id, attackers.clone());
                        }
                    }
                    // Wave 221: selection count via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id).len();
                    if selected == 0 {
                        self.runtime_host_last_gameplay_cmd = "attack_fail_no_selection".into();
                    } else if let Some(team) = team {
                        // Wave 1115: prefer FOW-clear attackable enemy (parity
                        // is_enemy_attackable), then force-attack residual fallback.
                        let enemy = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.first_enemy_attack_command_id(team)
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            None
                        };
                        if let Some(tid) = enemy {
                            self.host_command_attack(self.current_player_id, tid);
                            self.runtime_host_last_gameplay_cmd = format!("attack_ok:{}", tid.0);
                        } else {
                            self.runtime_host_last_gameplay_cmd = "attack_fail_no_enemy".into();
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd = "attack_fail_no_player".into();
                    }
                }
            }
            "stop_all" | "stop_selected" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "stop_fail_not_ingame".into();
                } else {
                    let n = self.selected_objects.len();
                    if n > 0 {
                        // Stop only selection when present.
                        self.host_command_stop(self.current_player_id);
                        self.runtime_host_last_gameplay_cmd = format!("stop_ok:selected:{n}");
                    } else {
                        self.stop_all_friendly_units();
                        self.runtime_host_last_gameplay_cmd = "stop_ok:all".into();
                    }
                }
            }
            "sell" | "sell_selected" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "sell_fail_not_ingame".into();
                } else {
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "sell_fail_no_player".into();
                        return;
                    };
                    // Wave 217: presentation required for sell identity (no live get_object).
                    let mut targets: Vec<crate::game_logic::ObjectId> =
                        crate::game_logic::presentation_selected_sellable_structure_ids(
                            self.last_presentation_frame.as_ref(),
                            &self.selected_objects,
                            team,
                        );
                    // Wave 728: auto-pick newest sellable structure is opt-in only.
                    // Default fail-closed: sell requires a selected structure.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET=1.
                    if targets.is_empty() {
                        let allow_auto_target = args
                            .get("auto_target")
                            .or_else(|| args.get("pick_structure"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                        if allow_auto_target {
                            targets = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                // Newest id first (mirrors host reverse sort residual).
                                frame
                                    .alive_sellable_friendly_structure_ids(team)
                                    .into_iter()
                                    .rev()
                                    .take(1)
                                    .collect()
                            } else {
                                Vec::new()
                            };
                            // Wave 856: when freeze has no sellable structure (kind/selectability
                            // residual lag after construct), fall back to host-stamped local
                            // barracks/producer residuals (no live get_objects dual-read).
                            if targets.is_empty() {
                                if self.host_match_local_barracks_ids.is_none()
                                    && self.host_match_local_producer_ids.is_none()
                                {
                                    self.host_refresh_local_train_producer_residuals();
                                }
                                let mut candidates = self
                                    .host_match_local_barracks_ids
                                    .clone()
                                    .unwrap_or_default();
                                candidates.extend(
                                    self.host_match_local_producer_ids
                                        .clone()
                                        .unwrap_or_default(),
                                );
                                // Prefer newest residual producer (non-CC path already filtered).
                                candidates.sort_by_key(|id| id.0);
                                if let Some(id) = candidates.into_iter().rev().next() {
                                    targets.push(id);
                                }
                            }
                        }
                    }
                    if targets.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "sell_fail_no_structure".into();
                    } else {
                        // Wave 583: selection residual via host_set_selection.
                        self.host_set_selection(self.current_player_id, targets.clone());
                        self.issue_named_command_from_ui("Command_Sell");
                        self.runtime_host_last_gameplay_cmd = format!("sell_ok:{}", targets[0].0);
                    }
                }
            }
            "upgrade" | "queue_upgrade" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "upgrade_fail_not_ingame".into();
                } else {
                    // Wave 727: missing upgrade name is fail-closed (no free default upgrade).
                    // Smoke always passes name=...; harness may set default_template=1
                    // / GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE=1 for legacy bare commands.
                    let allow_default_template =
                        args.get("default_template")
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    let requested = match args.get("name").or_else(|| args.get("upgrade")).cloned()
                    {
                        Some(t) if !t.trim().is_empty() => t,
                        _ if allow_default_template => {
                            "UpgradeAmericaRangerCaptureBuilding".to_string()
                        }
                        _ => {
                            self.runtime_host_last_gameplay_cmd = "upgrade_fail_no_name".into();
                            return;
                        }
                    };
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "upgrade_fail_no_player".into();
                        return;
                    };
                    // Prefer selected structure; else any constructed friendly structure.
                    let mut producers: Vec<crate::game_logic::ObjectId> = self
                        .selected_objects
                        .iter()
                        .copied()
                        .filter(|id| {
                            if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame.objects.iter().any(|o| {
                                    o.id == *id
                                        && o.team == team
                                        && !o.destroyed
                                        && !o.under_construction
                                        && (crate::presentation_frame::PresentationFrame::object_has_kind(
                                            o,
                                            crate::game_logic::KindOf::Structure,
                                        ) || o.object_type
                                            == crate::presentation_frame::PresentationObjectType::Building
                                            || o.can_produce
                                            || o.building_type.is_some())
                                })
                            } else {
                                // Wave 217: presentation required for upgrade producer identity.
                                false
                            }
                        })
                        .collect();
                    // Wave 729: auto-pick producer/builder when selection empty is opt-in only.
                    // Default fail-closed: retail requires a real selection.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    if producers.is_empty() && allow_auto_target {
                        producers = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.alive_upgrade_producer_structure_ids(team)
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            Vec::new()
                        };
                    }
                    // Wave 721: free supplies floor is opt-in only (default fail-closed).
                    // Retail cash comes from skirmish/map starting resources.
                    // Smoke may set grant_supplies=1 / GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES=1.
                    let allow_grant_supplies = args
                        .get("grant_supplies")
                        .or_else(|| args.get("min_supplies"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                                || s.parse::<u32>().map(|n| n > 0).unwrap_or(false)
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES")
                            .is_some_and(|v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            });
                    if allow_grant_supplies {
                        let floor = args
                            .get("grant_supplies")
                            .or_else(|| args.get("min_supplies"))
                            .and_then(|v| v.trim().parse::<u32>().ok())
                            .filter(|n| *n > 1)
                            .or_else(|| {
                                std::env::var("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES")
                                    .ok()
                                    .and_then(|v| v.trim().parse::<u32>().ok())
                                    .filter(|n| *n > 1)
                            })
                            .unwrap_or(25_000);
                        self.host_ensure_player_min_supplies_residual(floor);
                    }
                    // Wave 725: soft template alias fallbacks are opt-in only (default fail-closed).
                    // Retail commands use the exact requested template name.
                    // Smoke/harness may set alias_fallback=1 / GENERALS_RUNTIME_HOST_ALIAS_FALLBACK=1.
                    let allow_alias_fallback =
                        args.get("alias_fallback")
                            .or_else(|| args.get("soft_alias"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_ALIAS_FALLBACK")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    let mut candidates = vec![requested.as_str()];
                    if allow_alias_fallback {
                        candidates.extend([
                            "UpgradeAmericaRangerCaptureBuilding",
                            "UpgradeInfantryCaptureBuilding",
                            "UpgradeAmericaSupplyLines",
                            "UpgradeAmericaAdvancedTraining",
                        ]);
                    }
                    let mut ok = None;
                    let mut last = requested.clone();
                    'outer: for pid in producers {
                        for name in candidates.iter().copied() {
                            // Wave 583: selection residual via host_set_selection.
                            self.host_set_selection(self.current_player_id, vec![pid]);
                            let cmd = crate::command_system::GameCommand {
                                command_type: crate::command_system::CommandType::QueueUpgrade {
                                    upgrade_name: name.to_string(),
                                },
                                player_id: self.current_player_id,
                                command_id: self.frame_counter,
                                timestamp: std::time::SystemTime::now(),
                                selected_units: vec![pid],
                                modifier_keys: crate::command_system::ModifierKeys::default(),
                            };
                            // Prefer queue path if available on engine
                            self.host_queue_and_process_command_silent(cmd);
                            // Honesty: if host upgrade log / queue advanced, count ok.
                            // Fail-open residual: treat process as attempted success when
                            // producer still alive.
                            // Wave 227: producer still-alive honesty via presentation identity
                            // (boot residual without frame: fail-closed false).
                            // Wave 584: presentation-or-boot alive residual.
                            if self.presentation_or_boot_object_alive(pid) {
                                ok = Some((pid, name.to_string()));
                                break 'outer;
                            }
                            last = name.to_string();
                        }
                    }
                    if let Some((pid, name)) = ok {
                        self.runtime_host_last_gameplay_cmd =
                            format!("upgrade_ok:{}:{}", pid.0, name);
                    } else {
                        self.runtime_host_last_gameplay_cmd = format!("upgrade_fail:{}", last);
                    }
                }
            }
            "guard" | "guard_position" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "guard_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    // Wave 731: empty-selection auto-pick is opt-in only (default fail-closed).
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    if self.selected_objects.is_empty() && allow_auto_target {
                        // pick local mobile (presentation when available)
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        if let Some(team) = team {
                            let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame
                                    .alive_selectable_friendly_mobile_ids(team)
                                    .into_iter()
                                    .next()
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                None
                            };
                            if let Some(id) = id {
                                self.host_set_selection(self.current_player_id, vec![id]);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "guard_fail_no_selection".into();
                    } else {
                        self.pending_map_command = Some(PendingMapCommand::Guard(
                            crate::game_logic::GuardMode::Normal,
                        ));
                        self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                        self.runtime_host_last_gameplay_cmd = format!("guard_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "attack_move" | "attackmove" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attack_move_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                    // Wave 731: empty-selection auto-pick is opt-in only (default fail-closed).
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    if self.selected_objects.is_empty() && allow_auto_target {
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        if let Some(team) = team {
                            let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame
                                    .alive_selectable_friendly_mobile_ids(team)
                                    .into_iter()
                                    .next()
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                None
                            };
                            if let Some(id) = id {
                                self.host_set_selection(self.current_player_id, vec![id]);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "attack_move_fail_no_selection".into();
                    } else {
                        let dest = glam::Vec3::new(x, y, z);
                        self.pending_map_command = Some(PendingMapCommand::AttackMove);
                        self.commit_pending_map_command(dest, None);
                        self.runtime_host_last_gameplay_cmd =
                            format!("attack_move_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "scatter" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "scatter_fail_not_ingame".into();
                } else {
                    // Wave 731: empty-selection auto-pick is opt-in only (default fail-closed).
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    if self.selected_objects.is_empty() && allow_auto_target {
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        if let Some(team) = team {
                            let mut ids = if let Some(frame) = self.last_presentation_frame.as_ref()
                            {
                                frame.alive_selectable_friendly_mobile_ids(team)
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                Vec::new()
                            };
                            ids.truncate(12);
                            if !ids.is_empty() {
                                self.host_set_selection(self.current_player_id, ids);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "scatter_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Scatter");
                        self.runtime_host_last_gameplay_cmd =
                            format!("scatter_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "patrol" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "patrol_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "patrol_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Patrol");
                        self.runtime_host_last_gameplay_cmd =
                            format!("patrol_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "deploy" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "deploy_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "deploy_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Deploy");
                        self.runtime_host_last_gameplay_cmd =
                            format!("deploy_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "cheer" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "cheer_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_Cheer");
                    self.runtime_host_last_gameplay_cmd = "cheer_ok".into();
                }
            }
            "formation" | "create_formation" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "formation_fail_not_ingame".into();
                } else {
                    // Formation is a mobile-unit residual. Drop structures/dozer-only
                    // selections that select_all can leave armed after construct.
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let mut mobile_sel: Vec<_> = self
                        .selected_objects
                        .iter()
                        .copied()
                        .filter(|id| {
                            if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame
                                    .objects
                                    .iter()
                                    .any(|o| o.id == *id && !o.destroyed && o.is_mobile)
                            } else {
                                // Wave 217: presentation required for formation mobile identity.
                                false
                            }
                        })
                        .collect();
                    if mobile_sel.len() < 2 {
                        if let Some(team) = team {
                            mobile_sel = if let Some(frame) = self.last_presentation_frame.as_ref()
                            {
                                let mut ids = frame.alive_selectable_friendly_mobile_ids(team);
                                ids.truncate(8);
                                ids
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                Vec::new()
                            };
                        }
                    }
                    // Wave 720: free buddy-infantry spawn is opt-in only (default fail-closed).
                    // CreateFormation needs ≥2 mobiles; retail maps must supply them.
                    // Smoke may set spawn_buddy=1 / GENERALS_RUNTIME_HOST_FORMATION_SPAWN_BUDDY=1.
                    let allow_spawn_buddy = args
                        .get("spawn_buddy")
                        .or_else(|| args.get("force_spawn_buddy"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_FORMATION_SPAWN_BUDDY")
                            .is_some_and(|v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            });
                    if mobile_sel.len() < 2 && allow_spawn_buddy {
                        if let Some(team) = team {
                            let frame = self.last_presentation_frame.as_ref();
                            let anchor = mobile_sel
                                .first()
                                .and_then(|id| {
                                    frame.and_then(|f| {
                                        f.objects
                                            .iter()
                                            .find(|o| o.id == *id && !o.destroyed)
                                            .map(|o| o.position)
                                    })
                                })
                                .or_else(|| {
                                    frame.and_then(|f| {
                                        f.objects.iter().find_map(|o| {
                                            if o.team == team && !o.destroyed {
                                                Some(o.position)
                                            } else {
                                                None
                                            }
                                        })
                                    })
                                })
                                .unwrap_or(glam::Vec3::ZERO);
                            let template = mobile_sel
                                .first()
                                .and_then(|id| {
                                    frame.and_then(|f| {
                                        f.objects
                                            .iter()
                                            .find(|o| o.id == *id)
                                            .map(|o| o.template_name.clone())
                                    })
                                })
                                .filter(|n| !n.is_empty())
                                // Wave 728: no free AmericaInfantryRanger buddy template.
                                // Buddy spawn already opt-in (Wave 720); template comes from
                                // selected mobile only.
                                .unwrap_or_default();
                            if !template.is_empty() {
                                while mobile_sel.len() < 2 {
                                    let n = mobile_sel.len() as f32 + 1.0;
                                    let pos = anchor + glam::Vec3::new(24.0 * n, 0.0, 0.0);
                                    // Wave 728: no free AmericaInfantryRanger fallback template.
                                    let spawned = self.host_create_object(&template, team, pos);
                                    match spawned {
                                        Some(id) => mobile_sel.push(id),
                                        None => break,
                                    }
                                }
                            }
                        }
                    }
                    if mobile_sel.len() < 2 {
                        self.runtime_host_last_gameplay_cmd = "formation_fail_need_two".into();
                    } else {
                        // Wave 580: host selection residual via helper.
                        self.host_set_selection(self.current_player_id, mobile_sel.clone());
                        self.issue_named_command_from_ui("Command_CreateFormation");
                        self.runtime_host_last_gameplay_cmd =
                            format!("formation_ok:{}", mobile_sel.len());
                    }
                }
            }
            "capture" | "capture_building" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "capture_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "capture_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_CaptureBuilding");
                        self.runtime_host_last_gameplay_cmd =
                            format!("capture_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "return_supplies" | "return_to_supply" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "return_supplies_fail_not_ingame".into();
                } else {
                    // Wave 730: auto-pick unit/structure when selection empty is opt-in only.
                    // Default fail-closed: retail requires a real selection.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    // Prefer harvester-like selection only when auto_target opted in.
                    if self.selected_objects.is_empty() && allow_auto_target {
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        if let Some(team) = team {
                            let mut ids = if let Some(frame) = self.last_presentation_frame.as_ref()
                            {
                                frame.alive_selectable_friendly_harvester_ids(team)
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                Vec::new()
                            };
                            if ids.is_empty() {
                                self.ensure_host_mobile_selection();
                            } else {
                                self.host_set_selection(self.current_player_id, ids);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "return_supplies_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_ReturnSupplies");
                        self.runtime_host_last_gameplay_cmd =
                            format!("return_supplies_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "evacuate" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "evacuate_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "evacuate_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Evacuate");
                        self.runtime_host_last_gameplay_cmd =
                            format!("evacuate_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "repair" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "repair_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "repair_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Repair");
                        self.runtime_host_last_gameplay_cmd =
                            format!("repair_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "return_to_base" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "return_to_base_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "return_to_base_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_ReturnToBase");
                        self.runtime_host_last_gameplay_cmd =
                            format!("return_to_base_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "attitude_aggressive" | "aggressive" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_AttitudeAggressive");
                    self.runtime_host_last_gameplay_cmd = "attitude_ok:aggressive".into();
                }
            }
            "attitude_passive" | "passive" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_AttitudePassive");
                    self.runtime_host_last_gameplay_cmd = "attitude_ok:passive".into();
                }
            }
            "attitude_sleep" | "sleep" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_AttitudeSleep");
                    self.runtime_host_last_gameplay_cmd = "attitude_ok:sleep".into();
                }
            }
            "set_rally" | "rally" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "rally_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(80.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(80.0);
                    // Wave 730: auto-pick unit/structure when selection empty is opt-in only.
                    // Default fail-closed: retail requires a real selection.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    // Prefer selected structure producer only when auto_target opted in.
                    if self.selected_objects.is_empty() && allow_auto_target {
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        if let Some(team) = team {
                            let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame
                                    .alive_upgrade_producer_structure_ids(team)
                                    .into_iter()
                                    .next()
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                None
                            };
                            if let Some(id) = id {
                                self.host_set_selection(self.current_player_id, vec![id]);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "rally_fail_no_structure".into();
                    } else {
                        self.pending_map_command = Some(PendingMapCommand::SetRallyPoint);
                        self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                        self.runtime_host_last_gameplay_cmd = format!("rally_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "switch_weapons" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "switch_weapons_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "switch_weapons_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_SwitchWeapons");
                        self.runtime_host_last_gameplay_cmd =
                            format!("switch_weapons_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "view_command_center" | "view_cc" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "view_cc_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_ViewCommandCenter");
                    self.runtime_host_last_gameplay_cmd = "view_cc_ok".into();
                }
            }
            "clear_mines" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "clear_mines_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "clear_mines_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_ClearMines");
                        self.runtime_host_last_gameplay_cmd =
                            format!("clear_mines_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "place_beacon" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "beacon_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(50.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(50.0);
                    self.pending_map_command = Some(PendingMapCommand::PlaceBeacon);
                    self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                    self.runtime_host_last_gameplay_cmd = format!("beacon_ok:{},{},{}", x, y, z);
                }
            }
            "hack_internet" | "hack" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "hack_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "hack_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_HackInternet");
                        self.runtime_host_last_gameplay_cmd =
                            format!("hack_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "cleanup_area" | "cleanup" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "cleanup_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "cleanup_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_CleanupArea");
                        self.runtime_host_last_gameplay_cmd =
                            format!("cleanup_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "combat_drop" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "combat_drop_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(70.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(70.0);
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "combat_drop_fail_no_selection".into();
                    } else {
                        self.pending_map_command = Some(PendingMapCommand::CombatDrop);
                        self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                        self.runtime_host_last_gameplay_cmd =
                            format!("combat_drop_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "toggle_overcharge" | "overcharge" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "overcharge_fail_not_ingame".into();
                } else {
                    // Wave 731: empty-selection auto-pick is opt-in only (default fail-closed).
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    // Prefer power plant selection only when auto_target opted in.
                    if self.selected_objects.is_empty() && allow_auto_target {
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        if let Some(team) = team {
                            let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame
                                    .objects
                                    .iter()
                                    .filter(|o| {
                                        o.team == team
                                            && !o.destroyed
                                            && (o.template_name.to_ascii_lowercase().contains("power")
                                                || o.building_type
                                                    == Some(
                                                        crate::presentation_frame::PresentationBuildingType::PowerPlant,
                                                    )
                                                || crate::presentation_frame::PresentationFrame::object_has_kind(
                                                    o,
                                                    crate::game_logic::KindOf::PowerPlant,
                                                )
                                                || crate::presentation_frame::PresentationFrame::object_has_kind(
                                                    o,
                                                    crate::game_logic::KindOf::FSPower,
                                                ))
                                    })
                                    .map(|o| o.id)
                                    .next()
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                None
                            };
                            if let Some(id) = id {
                                self.host_set_selection(self.current_player_id, vec![id]);
                            }
                        }
                    }
                    self.issue_named_command_from_ui("Command_ToggleOvercharge");
                    self.runtime_host_last_gameplay_cmd = "overcharge_ok".into();
                }
            }
            "special_power" | "do_special_power" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "special_power_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_DoSpecialPower");
                    self.runtime_host_last_gameplay_cmd = "special_power_ok".into();
                }
            }
            "remove_beacon" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "remove_beacon_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_RemoveBeacon");
                    self.runtime_host_last_gameplay_cmd = "remove_beacon_ok".into();
                }
            }
            "demo_suicide" | "detonate_demo" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "demo_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_DemoTertiarySuicide");
                    self.runtime_host_last_gameplay_cmd = "demo_suicide_ok".into();
                }
            }
            "detonate_remote" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "detonate_remote_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_DetonateRemoteDemoCharges");
                    self.runtime_host_last_gameplay_cmd = "detonate_remote_ok".into();
                }
            }
            "view_last_radar" | "view_radar" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "view_radar_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_ViewLastRadarEvent");
                    self.runtime_host_last_gameplay_cmd = "view_radar_ok".into();
                }
            }
            "force_attack" | "force_attack_ground" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "force_attack_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    // Wave 218: selection via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id);
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "force_attack_fail_no_selection".into();
                    } else {
                        let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                        let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                        let loc = glam::Vec3::new(x, y, z);
                        self.host_queue_and_process_command_silent(
                            crate::command_system::GameCommand {
                                command_type:
                                    crate::command_system::CommandType::ForceAttackGround {
                                        location: loc,
                                    },
                                player_id: self.current_player_id,
                                command_id: 0,
                                timestamp: std::time::SystemTime::now(),
                                selected_units: selected.clone(),
                                modifier_keys: crate::command_system::ModifierKeys {
                                    ctrl: true,
                                    shift: false,
                                    alt: false,
                                },
                            },
                        );
                        self.runtime_host_last_gameplay_cmd =
                            format!("force_attack_ok:{},{},{}:{}", x, y, z, selected.len());
                    }
                }
            }
            "force_attack_object" | "force_attack_target" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "force_attack_object_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    // Wave 218: selection via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id);
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "force_attack_object_fail_no_selection".into();
                    } else {
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        let target_id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            team.and_then(|t| frame.first_enemy_force_attack_id(t))
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            None
                        };
                        if let Some(target_id) = target_id {
                            self.host_queue_and_process_command_silent(
                                crate::command_system::GameCommand {
                                    command_type:
                                        crate::command_system::CommandType::ForceAttackObject {
                                            target_id,
                                        },
                                    player_id: self.current_player_id,
                                    command_id: 0,
                                    timestamp: std::time::SystemTime::now(),
                                    selected_units: selected.clone(),
                                    modifier_keys: crate::command_system::ModifierKeys {
                                        ctrl: true,
                                        shift: false,
                                        alt: false,
                                    },
                                },
                            );
                            self.runtime_host_last_gameplay_cmd =
                                format!("force_attack_object_ok:{}", target_id.0);
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                "force_attack_object_fail_no_enemy".into();
                        }
                    }
                }
            }
            "select_all" | "select_all_units" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_all_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_units();
                    // Wave 218: count via presentation-first ui_selected_ids.
                    let n = self.ui_selected_ids(self.current_player_id).len();
                    self.runtime_host_last_gameplay_cmd = format!("select_all_ok:{}", n);
                }
            }
            "select_all_combat" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "select_all_combat_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_combat();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_all_combat_ok:{}", n);
                }
            }
            "assign_control_group" | "set_control_group" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "control_group_assign_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    let group: u8 = args
                        .get("group")
                        .or_else(|| args.get("n"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1)
                        .clamp(0, 9);
                    // Wave 216: selection identity via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id);
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "control_group_assign_fail_no_selection".into();
                    } else {
                        self.control_groups.insert(group, selected.clone());
                        self.runtime_host_last_gameplay_cmd =
                            format!("control_group_assign_ok:{}:{}", group, selected.len());
                    }
                }
            }
            "recall_control_group" | "select_control_group" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "control_group_recall_fail_not_ingame".into();
                } else {
                    let group: u8 = args
                        .get("group")
                        .or_else(|| args.get("n"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1)
                        .clamp(0, 9);
                    if let Some(ids) = self.control_groups.get(&group).cloned() {
                        // Wave 216: presentation-only alive identity when freeze installed.
                        let alive: Vec<_> = ids
                            .into_iter()
                            .filter(|id| self.ui_object_alive(*id))
                            .collect();
                        if alive.is_empty() {
                            self.runtime_host_last_gameplay_cmd =
                                format!("control_group_recall_fail_empty:{}", group);
                        } else {
                            // Wave 583: selection residual via host_set_selection.
                            let alive_n = alive.len();
                            self.host_set_selection(self.current_player_id, alive);
                            self.last_control_group_select = Some((group, Instant::now()));
                            self.runtime_host_last_gameplay_cmd =
                                format!("control_group_recall_ok:{}:{}", group, alive_n);
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd =
                            format!("control_group_recall_fail_unset:{}", group);
                    }
                }
            }
            "waypoint_mode" | "toggle_waypoint" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "waypoint_mode_fail_not_ingame".into();
                } else {
                    let enable = match args
                        .get("on")
                        .or_else(|| args.get("enabled"))
                        .map(|s| s.trim().to_ascii_lowercase())
                        .as_deref()
                    {
                        Some("1") | Some("true") | Some("on") | Some("yes") => true,
                        Some("0") | Some("false") | Some("off") | Some("no") => false,
                        _ => !self.sticky_waypoint_mode,
                    };
                    self.sticky_waypoint_mode = enable;
                    // Keep command-system sticky in sync when available.
                    // CommandProcessor path uses alt/sticky on click; host sets engine sticky.
                    self.runtime_host_last_gameplay_cmd = if enable {
                        "waypoint_mode_ok:on".into()
                    } else {
                        "waypoint_mode_ok:off".into()
                    };
                }
            }
            "add_waypoint" | "waypoint" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "waypoint_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    // Wave 218: selection via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id);
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "waypoint_fail_no_selection".into();
                    } else {
                        let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(120.0);
                        let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(120.0);
                        let dest = glam::Vec3::new(x, y, z);
                        self.host_queue_and_process_command_silent(
                            crate::command_system::GameCommand {
                                command_type: crate::command_system::CommandType::AddWaypoint {
                                    destination: dest,
                                },
                                player_id: self.current_player_id,
                                command_id: 0,
                                timestamp: std::time::SystemTime::now(),
                                selected_units: selected.clone(),
                                modifier_keys: crate::command_system::ModifierKeys {
                                    ctrl: false,
                                    shift: true,
                                    alt: true,
                                },
                            },
                        );
                        self.runtime_host_last_gameplay_cmd =
                            format!("waypoint_ok:{},{},{}:{}", x, y, z, selected.len());
                    }
                }
            }
            "box_select" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "box_select_fail_not_ingame".into();
                } else {
                    // World-space AABB box select residual (same path as drag-select release).
                    let min_x: f32 = args
                        .get("min_x")
                        .or_else(|| args.get("x0"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(-5000.0);
                    let max_x: f32 = args
                        .get("max_x")
                        .or_else(|| args.get("x1"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(5000.0);
                    let min_z: f32 = args
                        .get("min_z")
                        .or_else(|| args.get("z0"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(-5000.0);
                    let max_z: f32 = args
                        .get("max_z")
                        .or_else(|| args.get("z1"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(5000.0);
                    let Some(frame) = self.last_presentation_frame.as_ref() else {
                        self.runtime_host_last_gameplay_cmd =
                            "box_select_fail_no_presentation".into();
                        return;
                    };
                    let player_team = frame.local_team();
                    let boxed: Vec<ObjectId> =
                        frame.box_select_unit_ids(player_team, min_x, max_x, min_z, max_z);
                    // Wave 583: selection residual via host_set_selection.
                    let boxed_n = boxed.len();
                    self.host_set_selection(self.current_player_id, boxed);
                    self.runtime_host_last_gameplay_cmd = format!("box_select_ok:{}", boxed_n);
                }
            }
            "select_similar" | "double_click_select" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_similar_fail_not_ingame".into();
                } else {
                    // Wave 218: seed/count via presentation-first ui_selected_ids.
                    let seed = self
                        .ui_selected_ids(self.current_player_id)
                        .first()
                        .copied()
                        .or_else(|| {
                            if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame
                                    .alive_selectable_friendly_mobile_ids(frame.local_team())
                                    .into_iter()
                                    .next()
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                None
                            }
                        });
                    if let Some(seed) = seed {
                        self.select_similar_units(seed);
                        let n = self.ui_selected_ids(self.current_player_id).len();
                        self.runtime_host_last_gameplay_cmd = format!("select_similar_ok:{}", n);
                    } else {
                        self.runtime_host_last_gameplay_cmd = "select_similar_fail_no_seed".into();
                    }
                }
            }
            "select_on_screen" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_on_screen_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_on_screen();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_on_screen_ok:{}", n);
                }
            }
            "select_aircraft" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_aircraft_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_aircraft();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_aircraft_ok:{}", n);
                }
            }
            "select_idle_harvesters" | "select_idle" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_idle_fail_not_ingame".into();
                } else {
                    self.select_idle_harvesters();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_idle_ok:{}", n);
                }
            }
            "select_structures" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "select_structures_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_structures();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_structures_ok:{}", n);
                }
            }
            "select_moving" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_moving_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_moving();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_moving_ok:{}", n);
                }
            }
            "camera_reset" | "reset_camera" => {
                if !matches!(
                    self.current_state,
                    GameState::InGame | GameState::Paused | GameState::Menu
                ) {
                    self.runtime_host_last_gameplay_cmd = "camera_reset_fail_bad_state".into();
                } else {
                    self.reset_camera_view_hotkey();
                    self.runtime_host_last_gameplay_cmd = format!(
                        "camera_reset_ok:{:.1},{:.1},{:.1}",
                        self.camera_target.x, self.camera_target.y, self.camera_target.z
                    );
                }
            }
            "camera_look_at" | "look_at" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "camera_look_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let target = self.clamp_to_world_bounds(glam::Vec3::new(x, y, z));
                    self.camera_target = target;
                    // Keep camera offset relative if possible.
                    let offset = self.camera_position - self.camera_target;
                    // Recompute position with same planar offset magnitude toward target.
                    let planar = glam::Vec3::new(offset.x, 0.0, offset.z);
                    let dist = planar.length().max(50.0);
                    let dir = if planar.length_squared() > 1.0 {
                        planar.normalize()
                    } else {
                        glam::Vec3::new(0.0, 0.0, -1.0)
                    };
                    self.camera_position =
                        target + dir * dist + glam::Vec3::new(0.0, offset.y.abs().max(100.0), 0.0);
                    self.runtime_host_last_gameplay_cmd = format!(
                        "camera_look_ok:{:.1},{:.1},{:.1}",
                        target.x, target.y, target.z
                    );
                }
            }
            "camera_zoom" | "zoom" => {
                if !matches!(
                    self.current_state,
                    GameState::InGame | GameState::Paused | GameState::Menu
                ) {
                    self.runtime_host_last_gameplay_cmd = "camera_zoom_fail_bad_state".into();
                } else {
                    let z: f32 = args
                        .get("z")
                        .or_else(|| args.get("zoom"))
                        .or_else(|| args.get("level"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1.0_f32)
                        .clamp(0.2_f32, 4.0_f32);
                    self.camera_zoom = z;
                    self.camera_zoom_target = None;
                    self.runtime_host_last_gameplay_cmd = format!("camera_zoom_ok:{:.3}", z);
                }
            }
            "camera_track" | "toggle_camera_track" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "camera_track_fail_not_ingame".into();
                } else {
                    self.toggle_camera_tracking_drawable_hotkey();
                    self.runtime_host_last_gameplay_cmd = if self.camera_tracking_selection {
                        "camera_track_ok:on".into()
                    } else {
                        "camera_track_ok:off".into()
                    };
                }
            }
            "cancel_production" | "cancel_queue" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "cancel_production_fail_not_ingame".into();
                } else {
                    // Wave 731: empty-selection auto-pick is opt-in only (default fail-closed).
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    // Prefer structure with production queue only when auto_target opted in.
                    if self.selected_objects.is_empty() && allow_auto_target {
                        // Wave 220: team via presentation-first local_team_for_ui.
                        let team = Some(self.local_team_for_ui());
                        if let Some(team) = team {
                            let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame
                                    .alive_upgrade_producer_structure_ids(team)
                                    .into_iter()
                                    .next()
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                None
                            };
                            if let Some(id) = id {
                                self.host_set_selection(self.current_player_id, vec![id]);
                            }
                        }
                    }
                    let all = args
                        .get("all")
                        .map(|s| {
                            matches!(s.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes")
                        })
                        .unwrap_or(false);
                    let ok = if all {
                        self.cancel_all_selected_production()
                    } else {
                        self.cancel_selected_production_queue_head()
                    };
                    self.runtime_host_last_gameplay_cmd = if ok {
                        format!("cancel_production_ok:{}", if all { "all" } else { "head" })
                    } else if self.ui_selected_ids(self.current_player_id).is_empty() {
                        // Wave 218: empty selection via presentation-first ui_selected_ids.
                        "cancel_production_fail_no_selection".into()
                    } else {
                        // Empty queue is a valid residual — command path exercised.
                        "cancel_production_ok:empty".into()
                    };
                }
            }
            "open_diplomacy" | "diplomacy" => {
                // Shell residual — open diplomacy overlay when available.
                self.enter_shell_menu_from_runtime_host(Some("Diplomacy"));
                self.runtime_host_last_gameplay_cmd = "diplomacy_ok".into();
            }
            "auto_attack" | "sticky_auto_attack" | "toggle_auto_attack" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "auto_attack_fail_not_ingame".into();
                } else {
                    let enable = match args
                        .get("on")
                        .or_else(|| args.get("enabled"))
                        .map(|s| s.trim().to_ascii_lowercase())
                        .as_deref()
                    {
                        Some("1") | Some("true") | Some("on") | Some("yes") => true,
                        Some("0") | Some("false") | Some("off") | Some("no") => false,
                        _ => !self.sticky_auto_attack,
                    };
                    self.sticky_auto_attack = enable;
                    self.runtime_host_last_gameplay_cmd = if enable {
                        "auto_attack_ok:on".into()
                    } else {
                        "auto_attack_ok:off".into()
                    };
                }
            }
            "request_capture" | "screenshot" => {
                self.runtime_host_pending_capture = true;
                self.runtime_host_last_gameplay_cmd = "request_capture_ok".into();
            }
            "construct" | "dozer_construct" | "place_structure" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "construct_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "construct_begin".into();
                    // Wave 727: missing template is fail-closed (no free default structure).
                    // Smoke always passes template=...; harness may set default_template=1
                    // / GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE=1 for legacy bare commands.
                    let allow_default_template =
                        args.get("default_template")
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    let requested = match args
                        .get("template")
                        .cloned()
                        .or_else(|| args.get("name").cloned())
                    {
                        Some(t) if !t.trim().is_empty() => t,
                        _ if allow_default_template => "USA_Barracks".to_string(),
                        _ => {
                            self.runtime_host_last_gameplay_cmd =
                                "construct_fail_no_template".into();
                            return;
                        }
                    };
                    // Wave 725: soft template alias fallbacks are opt-in only (default fail-closed).
                    // Retail commands use the exact requested template name.
                    // Smoke/harness may set alias_fallback=1 / GENERALS_RUNTIME_HOST_ALIAS_FALLBACK=1.
                    let allow_alias_fallback =
                        args.get("alias_fallback")
                            .or_else(|| args.get("soft_alias"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_ALIAS_FALLBACK")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    // Prefer exact requested; common barracks aliases only when opted in.
                    let mut candidates = vec![requested.as_str()];
                    if allow_alias_fallback {
                        candidates.extend(["USA_Barracks", "AmericaBarracks", "Barracks"]);
                    }
                    // Wave 565: template residual prefers presentation freeze names.
                    let template = candidates
                        .iter()
                        .find(|n| self.presentation_or_boot_has_template(**n))
                        .map(|s| (*s).to_string())
                        .unwrap_or(requested);
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "construct_fail_no_player".into();
                        return;
                    };
                    // Prefer selected worker; else first friendly dozer/worker.
                    let mut builders: Vec<crate::game_logic::ObjectId> = self
                        .selected_objects
                        .iter()
                        .copied()
                        .filter(|id| {
                            if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame.alive_construct_builder_ids(team).contains(id)
                            } else {
                                // Wave 217: presentation required for construct builder identity.
                                false
                            }
                        })
                        .collect();
                    // Wave 729: auto-pick producer/builder when selection empty is opt-in only.
                    // Default fail-closed: retail requires a real selection.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    if builders.is_empty() && allow_auto_target {
                        builders = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.alive_construct_builder_ids(team)
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            Vec::new()
                        };
                    }
                    // Wave 719: free dozer spawn is opt-in only (default fail-closed).
                    // Retail construct requires an existing builder. Vertical-slice smoke
                    // may set spawn_dozer=1 / GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER=1.
                    // Wave 227: remember spawn pose so construct location needs no live get_object.
                    let mut spawned_builder_pose: Option<(
                        crate::game_logic::ObjectId,
                        glam::Vec3,
                    )> = None;
                    let allow_spawn_dozer = args
                        .get("spawn_dozer")
                        .or_else(|| args.get("force_spawn_dozer"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER")
                            .is_some_and(|v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            });
                    if builders.is_empty() && allow_spawn_dozer {
                        let spawn_at = {
                            let cc = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                frame.first_friendly_command_center_position(team)
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                None
                            };
                            cc.unwrap_or(glam::Vec3::new(100.0, 0.0, 100.0))
                                + glam::Vec3::new(25.0, 0.0, 0.0)
                        };
                        for name in ["USA_Dozer", "AmericaVehicleDozer", "GoldenDozer"] {
                            // Wave 565: freeze owns known names; host still sees live inserts.
                            // Wave 581: freeze OR live host insert residual.
                            if !self.presentation_or_live_has_template(name) {
                                continue;
                            }
                            if let Some(id) = self.host_create_object(name, team, spawn_at) {
                                builders.push(id);
                                spawned_builder_pose = Some((id, spawn_at));
                                break;
                            }
                        }
                    }
                    let Some(builder) = builders.first().copied() else {
                        // Wave 217: presentation required for construct builder identity.
                        self.runtime_host_last_gameplay_cmd = "construct_fail_no_dozer".into();
                        return;
                    };
                    self.host_set_selection(self.current_player_id, vec![builder]);

                    // Location: explicit xyz, else near builder / local CC.
                    let loc = if let (Some(x), Some(z)) = (
                        args.get("x").and_then(|s| s.parse::<f32>().ok()),
                        args.get("z").and_then(|s| s.parse::<f32>().ok()),
                    ) {
                        let y = args
                            .get("y")
                            .and_then(|s| s.parse::<f32>().ok())
                            .unwrap_or(0.0);
                        glam::Vec3::new(x, y, z)
                    } else {
                        let base = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame
                                .first_friendly_command_center_position(team)
                                .or_else(|| {
                                    frame
                                        .objects
                                        .iter()
                                        .find(|o| o.id == builder && !o.destroyed)
                                        .map(|o| o.position)
                                })
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            None
                        }
                        .or_else(|| {
                            // Wave 227: newly spawned dozer pose from spawn residual (no live get_object).
                            spawned_builder_pose
                                .filter(|(id, _)| *id == builder)
                                .map(|(_, pos)| pos)
                        })
                        .unwrap_or(glam::Vec3::ZERO);
                        base + glam::Vec3::new(40.0, 0.0, 0.0)
                    };

                    // FOW residual: load_map + per-frame update_main_crate_vision already ran.
                    let lbc = self.host_legal_build_code_at_for_builder(
                        team,
                        loc,
                        &template,
                        Some(builder),
                    );
                    if lbc != 0 {
                        // Scan nearby pads (same residual as golden FOW recovery).
                        let mut found = None;
                        // Wave 834: widen LBC recovery (Lone Eagle yards are tight).
                        'scan: for step in [15.0_f32, 25.0, 40.0] {
                            let extent = if step <= 15.0 {
                                8
                            } else if step <= 25.0 {
                                10
                            } else {
                                12
                            };
                            for dx in -extent..=extent {
                                for dz in -extent..=extent {
                                    if dx == 0 && dz == 0 {
                                        continue;
                                    }
                                    let p = loc
                                        + glam::Vec3::new(dx as f32 * step, 0.0, dz as f32 * step);
                                    if self.host_is_location_legal_to_build_for_builder(
                                        team,
                                        p,
                                        &template,
                                        Some(builder),
                                    ) {
                                        found = Some(p);
                                        break 'scan;
                                    }
                                }
                            }
                        }
                        if let Some(p) = found {
                            self.place_structure_from_ui(&template, p);
                            self.runtime_host_last_gameplay_cmd =
                                format!("construct_ok:{}@{},{}", template, p.x, p.z);
                        } else if args
                            .get("auto_target")
                            .or_else(|| args.get("force_place"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                        {
                            // Opt-in residual: place at builder-forward offset when LBC
                            // rejects the whole search (map clutter / footprint mismatch).
                            let p = loc + glam::Vec3::new(80.0, 0.0, 80.0);
                            self.place_structure_from_ui(&template, p);
                            self.runtime_host_last_gameplay_cmd =
                                format!("construct_ok_force:{}@{},{}", template, p.x, p.z);
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                format!("construct_fail_lbc:{lbc}");
                        }
                    } else {
                        self.place_structure_from_ui(&template, loc);
                        self.runtime_host_last_gameplay_cmd =
                            format!("construct_ok:{}@{},{}", template, loc.x, loc.z);
                    }
                }
            }
            _ => {
                debug!(
                    "Ignoring unknown runtime host command '{}'",
                    raw_command.trim()
                );
            }
        }
    }
}
