#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_toggle_control_bar(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_control_bar(&mut self, args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_toggle_in_game_chat(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_in_game_chat(&mut self, args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_idle_worker(&mut self, args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_open_generals_exp(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_generals_exp(&mut self, args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_open_popup_communicator(&mut self, _args: &HashMap<String, String>)  {
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

    pub(super) fn runtime_host_cmd_click_popup_communicator(&mut self, args: &HashMap<String, String>)  {
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

}
