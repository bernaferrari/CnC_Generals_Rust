#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_show_message_box(&mut self, args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_message_box(&mut self, args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_toggle_diplomacy(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_diplomacy(&mut self, args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_open_popup_replay(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_popup_replay(&mut self, args: &HashMap<String, String>)  {
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

}
