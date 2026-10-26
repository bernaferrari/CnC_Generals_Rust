#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_open_single_player_menu(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        // Retail MainMenu → SinglePlayerMenu residual open.
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::simulate_main_menu_single_player_button_gadget_selected(effects)
            })?;
            let _ = self.with_receiving_menu_effects(|effects| {
                game_client::gui::callbacks::simulate_single_player_menu_bind_controls(effects)
            })?;
        }
        self.enter_shell_menu_from_runtime_host(Some("SinglePlayer"))?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_single_player_menu_ok_wnd".into()
        } else {
            "open_single_player_menu_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_single_player_menu(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_single_player_menu_clear_button_pushed(effects)
                    })?;
                    self.with_receiving_menu_effects(|effects| {
                        simulate_single_player_menu_load_button_gadget_selected(effects)
                    })?
                }
                "back" => {
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_single_player_menu_clear_button_pushed(effects)
                    })?;
                    self.with_receiving_menu_effects(|effects| {
                        simulate_single_player_menu_back_button_gadget_selected(effects)
                    })?
                }
                "clear" => self.with_receiving_menu_effects(|effects| {
                    simulate_single_player_menu_clear_button_pushed(effects)
                })?,
                "prepare_new" => self.with_receiving_menu_effects(|effects| {
                    simulate_single_player_menu_prepare_new(effects)
                })?,
                _ => {
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_single_player_menu_clear_button_pushed(effects)
                    })?;
                    self.with_receiving_menu_effects(|effects| {
                        simulate_single_player_menu_new_button_gadget_selected(effects)
                    })?
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_single_player_menu_ok_wnd_{action}")
        } else {
            "click_single_player_menu_miss".into()
        };
        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_map_select_menu(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        // Retail SinglePlayer → MapSelectMenu residual open.
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::callbacks::simulate_map_select_menu_bind_controls(effects)
            })?;
        }
        self.enter_shell_menu_from_runtime_host(Some("MapSelect"))?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_map_select_menu_ok_wnd".into()
        } else {
            "open_map_select_menu_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_map_select_menu(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                "select" | "map" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_select_map(&map, effects)
                })?,
                "difficulty" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_set_difficulty(difficulty, effects)
                })?,
                "back" => {
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_map_select_menu_clear_button_pushed(effects)
                    })?;
                    self.with_receiving_menu_effects(|effects| {
                        simulate_map_select_menu_back_button_gadget_selected(effects)
                    })?
                }
                "solo" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_solo_maps_button_gadget_selected(effects)
                })?,
                "multiplayer" | "mp" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_multiplayer_maps_button_gadget_selected(effects)
                })?,
                "system" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_system_maps_radio_selected(effects)
                })?,
                "user" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_user_maps_radio_selected(effects)
                })?,
                "prepare_ok" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_prepare_ok(&map, effects)
                })?,
                "clear" => self.with_receiving_menu_effects(|effects| {
                    simulate_map_select_menu_clear_button_pushed(effects)
                })?,
                _ => {
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_map_select_menu_clear_button_pushed(effects)
                    })?;
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_map_select_menu_select_map(&map, effects)
                    })?;
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_map_select_menu_set_difficulty(difficulty, effects)
                    })?;
                    self.with_receiving_menu_effects(|effects| {
                        simulate_map_select_menu_ok_button_gadget_selected(effects)
                    })?
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_map_select_menu_ok_wnd_{action}")
        } else {
            "click_map_select_menu_miss".into()
        };
        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_single_player_menu_2(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::simulate_main_menu_single_player_button_gadget_selected(effects)
            })?;
        }
        self.enter_shell_menu_from_runtime_host(Some("SinglePlayer"))?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_single_player_menu_ok_wnd".into()
        } else {
            "open_single_player_menu_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_multiplayer_menu(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::simulate_main_menu_multiplayer_button_gadget_selected(effects)
            })?;
        }
        self.enter_shell_menu_from_runtime_host(Some("Multiplayer"))?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_multiplayer_menu_ok_wnd".into()
        } else {
            "open_multiplayer_menu_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_load_replay_menu(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::simulate_main_menu_replay_button_gadget_selected(effects)
            })?;
            let _ = self
                .game_client
                .window_manager_owner()
                .with_manager(|manager| {
                    game_client::gui::callbacks::simulate_replay_menu_bind_controls(manager)
                });
        }
        self.enter_shell_menu_from_runtime_host(Some("LoadReplay"))?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_load_replay_menu_ok_wnd".into()
        } else {
            "open_load_replay_menu_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_replay_menu(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                drive_os_wnd_replay_menu_back_like_cpp, drive_os_wnd_replay_menu_copy_like_cpp,
                drive_os_wnd_replay_menu_delete_like_cpp,
                drive_os_wnd_replay_menu_prepare_load_like_cpp,
                simulate_replay_menu_back_button_gadget_selected,
                simulate_replay_menu_copy_button_gadget_selected,
                simulate_replay_menu_delete_button_gadget_selected,
                simulate_replay_menu_prepare_load, simulate_replay_menu_select_slot,
            };
            wnd_ok = match action.as_str() {
                "delete" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_replay_menu_delete_like_cpp(slot, effects)
                    })? || {
                        let _ = self
                            .game_client
                            .window_manager_owner()
                            .with_manager(|manager| {
                                simulate_replay_menu_select_slot(slot, manager)
                            });
                        self.game_client
                            .window_manager_owner()
                            .with_manager(|manager| {
                                simulate_replay_menu_delete_button_gadget_selected(manager)
                            })
                    }
                }
                "copy" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_replay_menu_copy_like_cpp(slot, effects)
                    })? || {
                        let _ = self
                            .game_client
                            .window_manager_owner()
                            .with_manager(|manager| {
                                simulate_replay_menu_select_slot(slot, manager)
                            });
                        self.game_client
                            .window_manager_owner()
                            .with_manager(|manager| {
                                simulate_replay_menu_copy_button_gadget_selected(manager)
                            })
                    }
                }
                "back" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_replay_menu_back_like_cpp(effects)
                    })? || self
                        .game_client
                        .window_manager_owner()
                        .with_manager(|manager| {
                            simulate_replay_menu_back_button_gadget_selected(manager)
                        })
                }
                _ => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_replay_menu_prepare_load_like_cpp(slot, effects)
                    })? || self
                        .game_client
                        .window_manager_owner()
                        .with_manager(|manager| simulate_replay_menu_prepare_load(slot, manager))
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_replay_menu_ok_wnd_{action}")
        } else {
            "click_replay_menu_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_toggle_quit_menu(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        // C++ ESC: MSG_META_OPTIONS -> ToggleQuitMenu() — the live QuitMenu.wnd
        // callback (creates the layout, owns pause/resume via Main's bridge).
        // The residual latch below is only the fallback for hosts without the
        // offline WND bridge (headless / non-offline states).
        let mut wnd_ok = self.host_toggle_retail_quit_menu()?;
        if !wnd_ok {
            #[cfg(feature = "game_client")]
            {
                wnd_ok = self.with_receiving_menu_effects(|effects| {
                    game_client::gui::callbacks::simulate_quit_menu_toggle_show(effects)
                })?;
            }
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "toggle_quit_menu_ok_wnd".into()
        } else {
            "toggle_quit_menu_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_quit_menu(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                drive_os_wnd_quit_menu_restart_like_cpp, drive_os_wnd_quit_menu_return_like_cpp,
                drive_os_wnd_quit_menu_save_load_like_cpp, simulate_quit_menu_confirm_exit,
                simulate_quit_menu_destroy, simulate_quit_menu_exit_button_gadget_selected,
                simulate_quit_menu_options_button_gadget_selected, simulate_quit_menu_prepare_exit,
                simulate_quit_menu_restart_button_gadget_selected,
                simulate_quit_menu_return_button_gadget_selected,
                simulate_quit_menu_save_load_button_gadget_selected,
                simulate_quit_menu_toggle_hide, simulate_quit_menu_toggle_show,
            };
            wnd_ok = match action.as_str() {
                "return" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_quit_menu_return_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_quit_menu_return_button_gadget_selected(effects)
                    })?
                }
                "options" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_quit_menu_options_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_quit_menu_options_button_gadget_selected(effects)
                    })?
                }
                "restart" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_quit_menu_restart_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_quit_menu_restart_button_gadget_selected(effects)
                    })?
                }
                "save_load" | "saveload" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_quit_menu_save_load_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_quit_menu_save_load_button_gadget_selected(effects)
                    })?
                }
                "hide" => self.with_receiving_menu_effects(|effects| {
                    simulate_quit_menu_toggle_hide(effects)
                })?,
                "show" => self.with_receiving_menu_effects(|effects| {
                    simulate_quit_menu_toggle_show(effects)
                })?,
                "destroy" => {
                    self.with_receiving_menu_effects(|effects| simulate_quit_menu_destroy(effects))?
                }
                "confirm_exit" | "confirm" => self.with_receiving_menu_effects(|effects| {
                    simulate_quit_menu_confirm_exit(effects)
                })?,
                "prepare_exit" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_quit_menu_prepare_exit_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_quit_menu_prepare_exit(effects)
                    })?
                }
                _ => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_quit_menu_exit_like_cpp(effects)
                    })? || {
                        let _ = self.with_receiving_menu_effects(|effects| {
                            simulate_quit_menu_toggle_show(effects)
                        })?;
                        self.with_receiving_menu_effects(|effects| {
                            simulate_quit_menu_exit_button_gadget_selected(effects)
                        })?
                    }
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_quit_menu_ok_wnd_{action}")
        } else {
            "click_quit_menu_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_keyboard_options(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        // Retail Options → KeyboardOptions residual open.
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = game_client::gui::callbacks::simulate_keyboard_options_bind_controls();
        }
        self.enter_shell_menu_from_runtime_host(Some("KeyboardOptions"))?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_keyboard_options_ok_wnd".into()
        } else {
            "open_keyboard_options_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_keyboard_options(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_keyboard_options_category_like_cpp(category, effects)
                    })? || simulate_keyboard_options_select_category(category)
                }
                "command" => {
                    let _ = self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_keyboard_options_category_like_cpp(category, effects)
                    })? || simulate_keyboard_options_select_category(category);
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_keyboard_options_command_like_cpp(command, effects)
                    })? || simulate_keyboard_options_select_command(command)
                }
                "reset" | "reset_all" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_keyboard_options_reset_like_cpp(effects)
                    })? || simulate_keyboard_options_reset_all_button_gadget_selected()
                }
                "back" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_keyboard_options_back_like_cpp(effects)
                    })? || simulate_keyboard_options_back_button_gadget_selected()
                }
                _ => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_keyboard_options_prepare_assign_like_cpp(
                            category, command, effects,
                        )
                    })? || simulate_keyboard_options_prepare_assign(category, command)
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_keyboard_options_ok_wnd_{action}")
        } else {
            "click_keyboard_options_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_score_screen(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        // Retail end-of-match ScoreScreen residual open.
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::callbacks::simulate_score_screen_bind_controls(effects)
            })?;
        }
        self.enter_shell_screen_from_runtime_host(Some("ScoreScreen"), "Menus/ScoreScreen.wnd")?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_score_screen_ok_wnd".into()
        } else {
            "open_score_screen_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_score_screen(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                drive_os_wnd_score_screen_emote_like_cpp, drive_os_wnd_score_screen_ok_like_cpp,
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
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_score_screen_continue_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_continue_button_gadget_selected(effects)
                    })?
                }
                "finish" | "prepare_finish" => {
                    let _ = self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_set_finish_campaign(true, effects)
                    })?;
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_score_screen_continue_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_prepare_finish(effects)
                    })?
                }
                "save_replay" | "replay" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_score_screen_save_replay_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_save_replay_button_gadget_selected(effects)
                    })?
                }
                "buddy" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_score_screen_buddy_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_buddy_button_gadget_selected(effects)
                    })?
                }
                "emote" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_score_screen_emote_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_emote_button_gadget_selected(effects)
                    })?
                }
                "prepare_ok" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_score_screen_ok_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_prepare_ok(effects)
                    })?
                }
                _ => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_score_screen_ok_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_score_screen_ok_button_gadget_selected(effects)
                    })?
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_score_screen_ok_wnd_{action}")
        } else {
            "click_score_screen_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_options_menu(
        &mut self,
        _args: &HashMap<String, String>,
    ) -> Result<(), String> {
        // Retail OptionsMenu residual open (shell or in-game overlay).
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::callbacks::simulate_options_menu_bind_controls(effects)
            })?;
        }
        self.enter_shell_menu_from_runtime_host(Some("Options"))?;
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_options_menu_ok_wnd".into()
        } else {
            "open_options_menu_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_options_menu(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_back_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_back_button_gadget_selected(effects)
                    })?
                }
                "defaults" | "default" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_defaults_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_defaults_button_gadget_selected(effects)
                    })?
                }
                "keyboard" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_keyboard_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_keyboard_button_gadget_selected(effects)
                    })?
                }
                "advanced_accept" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_advanced_accept_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_advanced_accept_button_gadget_selected(effects)
                    })?
                }
                "advanced_back" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_advanced_back_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_advanced_back_button_gadget_selected(effects)
                    })?
                }
                "firewall" | "firewall_refresh" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_firewall_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_firewall_refresh_button_gadget_selected(effects)
                    })?
                }
                "prepare_accept" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_accept_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_prepare_accept(effects)
                    })?
                }
                _ => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_options_menu_accept_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_options_menu_accept_button_gadget_selected(effects)
                    })?
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_options_menu_ok_wnd_{action}")
        } else {
            "click_options_menu_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_open_difficulty_menu(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::simulate_main_menu_campaign_side_button_gadget_selected(
                    side, effects,
                )
            })?;
        }
        self.enter_shell_menu_from_runtime_host(Some(override_screen))?;

        #[cfg(feature = "game_client")]
        {
            let _ = self.with_receiving_menu_effects(|effects| {
                game_client::gui::callbacks::simulate_difficulty_select_bind_controls(effects)
            })?;
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_difficulty_menu_ok_wnd".into()
        } else {
            "open_difficulty_menu_ok".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_difficulty_select(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                simulate_difficulty_select_prepare_ok, simulate_difficulty_select_radio_easy,
                simulate_difficulty_select_radio_hard, simulate_difficulty_select_radio_medium,
            };
            wnd_ok = match action.as_str() {
                "easy" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_difficulty_select_radio_like_cpp(0, effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_difficulty_select_radio_easy(effects)
                    })?
                }
                "medium" | "normal" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_difficulty_select_radio_like_cpp(1, effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_difficulty_select_radio_medium(effects)
                    })?
                }
                "hard" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_difficulty_select_radio_like_cpp(2, effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_difficulty_select_radio_hard(effects)
                    })?
                }
                "cancel" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_difficulty_select_cancel_like_cpp(effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_difficulty_select_cancel_button_gadget_selected(effects)
                    })?
                }
                "prepare_ok" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_difficulty_select_like_cpp(level, effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        simulate_difficulty_select_prepare_ok(level, effects)
                    })?
                }
                _ => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_difficulty_select_like_cpp(level, effects)
                    })? || self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_difficulty_select_ok_like_cpp(effects)
                    })? || {
                        let _ = match level {
                            0 => self.with_receiving_menu_effects(|effects| {
                                simulate_difficulty_select_radio_easy(effects)
                            })?,
                            2 => self.with_receiving_menu_effects(|effects| {
                                simulate_difficulty_select_radio_hard(effects)
                            })?,
                            _ => self.with_receiving_menu_effects(|effects| {
                                simulate_difficulty_select_radio_medium(effects)
                            })?,
                        };
                        self.with_receiving_menu_effects(|effects| {
                            simulate_difficulty_select_ok_button_gadget_selected(effects)
                        })?
                    }
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_difficulty_select_ok_wnd_{action}")
        } else {
            "click_difficulty_select_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_show_loading_screen(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
        // Retail loading screen residual show without asset pipeline.
        let map = args
            .get("map")
            .cloned()
            .unwrap_or_else(|| "Maps/LoneEagle/LoneEagle.map".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::loading_screen::drive_os_wnd_loading_screen_prepare_like_cpp(
                    &map, 0, effects,
                )
            })?;
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

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_loading_screen(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_loading_screen_show_like_cpp(effects)
                    })? || simulate_loading_screen_show()
                }
                "hide" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_loading_screen_hide_like_cpp(effects)
                    })? || simulate_loading_screen_hide()
                }
                "map" => simulate_loading_screen_set_map(&map),
                "next" | "next_stage" => simulate_loading_screen_next_stage(),
                "finish" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_loading_screen_finish_like_cpp(effects)
                    })? || simulate_loading_screen_finish()
                }
                "prepare" => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_loading_screen_prepare_like_cpp(&map, percent, effects)
                    })? || simulate_loading_screen_prepare_map_load(&map, percent)
                }
                _ => {
                    self.with_receiving_menu_effects(|effects| {
                        drive_os_wnd_loading_screen_progress_like_cpp(percent, effects)
                    })? || simulate_loading_screen_set_progress(percent)
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_loading_screen_ok_wnd_{action}")
        } else {
            "click_loading_screen_miss".into()
        };

        Ok(())
    }

    pub(super) fn runtime_host_cmd_click_campaign_start(
        &mut self,
        args: &HashMap<String, String>,
    ) -> Result<(), String> {
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
            wnd_ok = self.with_receiving_menu_effects(|effects| {
                game_client::gui::drive_os_wnd_start_campaign_like_cpp(side, diff, effects)
            })?;
            if !wnd_ok {
                wnd_ok = self.with_receiving_menu_effects(|effects| {
                    game_client::gui::simulate_main_menu_campaign_start_residual(
                        side, diff, effects,
                    )
                })?;
            }
        }
        if wnd_ok {
            // The WND campaign click just queued C++ `MSG_NEW_GAME` alongside
            // the GameClient-owned CampaignLaunch descriptor.  Drain that
            // exact payload before falling back to the runtime-host shortcut:
            // constructing a generic `(USA/China/GLA, map)` request here used
            // to discard the selected PlayerTemplate before GameLogic saw it.
            //
            // Split dispatch extraction from request construction so a queued
            // but invalid Challenge/Campaign descriptor fails closed rather
            // than looking indistinguishable from "nothing was queued" and
            // silently launching a base-faction game.
            let requested_map = args
                .get("map")
                .map(|v| v.trim().to_string())
                .filter(|s| !s.is_empty());
            if let Some(dispatch) = Self::take_new_game_dispatch_from_common_stream(
                self.game_client.message_stream_mut(),
            ) {
                let Some(mut request) =
                    self.build_start_request_from_pending_globals(Some(dispatch))
                else {
                    self.runtime_host_last_gameplay_cmd = "click_campaign_start_rejected".into();
                    return Ok(());
                };
                // The runtime-host command's explicit map is an intentional
                // test/control override.  It changes only the map, never the
                // validated PlayerTemplate identity carried by `request`.
                if let Some(map) = requested_map.clone() {
                    request.map = map;
                }
                self.start_game_from_ui(request)?;
                self.runtime_host_last_gameplay_cmd = "click_campaign_start_ok_wnd".into();
                return Ok(());
            }

            // Development/headless fallback when the shell click itself did
            // not enqueue `MSG_NEW_GAME`.  Deliberately construct a request
            // with no PlayerTemplate so stale GameClient selection state can
            // never leak into this non-C++ fallback path.
            let map = requested_map
                .unwrap_or_else(|| crate::golden_campaign::OFFLINE_USA_CAMPAIGN_MAP.to_string());
            let faction = match campaign.as_str() {
                "gla" => "GLA",
                "china" => "China",
                _ => "USA",
            };
            self.start_game_from_ui(HostStartRequest::without_player_template(
                GameMode::SinglePlayer,
                faction.into(),
                map,
                None,
            ))?;
            self.runtime_host_last_gameplay_cmd = "click_campaign_start_ok_wnd".into();
        } else {
            self.runtime_host_last_gameplay_cmd = "click_campaign_start_miss".into();
        }

        Ok(())
    }
}
