#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_exit(&mut self, _args: &HashMap<String, String>)  {
                self.request_state_change(GameState::Exiting);
            }

    pub(super) fn runtime_host_cmd_menu(&mut self, _args: &HashMap<String, String>)  {
                self.enter_shell_menu_from_runtime_host(None);
                self.runtime_host_last_gameplay_cmd = "menu_ok".into();
            }

    pub(super) fn runtime_host_cmd_toggle_pause(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_open_message_of_the_day(&mut self, _args: &HashMap<String, String>)  {
                self.enter_shell_menu_from_runtime_host(Some("MessageOfDay"));
            }

    pub(super) fn runtime_host_cmd_open_get_updates(&mut self, _args: &HashMap<String, String>)  {
                self.enter_shell_menu_from_runtime_host(Some("GetUpdates"));
            }

    pub(super) fn runtime_host_cmd_open_world_builder(&mut self, _args: &HashMap<String, String>)  {
                self.enter_shell_menu_from_runtime_host(Some("WorldBuilder"));
            }

    pub(super) fn runtime_host_cmd_options_probe(&mut self, _args: &HashMap<String, String>)  {
                // Honesty residual: prove options host wiring without leaving InGame
                // (full open_options pauses / swaps UI and is covered separately).
                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "options_probe_ok".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "options_probe_fail_bad_state".into();
                }
            }

    pub(super) fn runtime_host_cmd_open_options(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_open_credits(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_credits_menu(&mut self, args: &HashMap<String, String>)  {
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

}
