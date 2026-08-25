//! Player economy/science, display text, camera move, audio, radar, flags/timers, and debug actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptActionDispatcher {
    // ============================================================================
    // PLAYER ACTIONS
    // C++ Reference: ScriptActions.cpp line (set money)
    // ============================================================================

    /// C++ Reference: ScriptActions::doSetMoney() line (in header)
    pub(crate) fn do_set_money(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let money_amount = self.get_int_param(action, 1)?;

        log::info!("Setting player '{}' money to {}", player_name, money_amount);

        // Get player by name and set money
        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.get_money_mut().set_money(money_amount);
                    log::info!("Player '{}' money set to {}", player_name, money_amount);
                }
            } else {
                log::warn!("Player '{}' not found for set money", player_name);
            }
        }
        crate::scripting::executor::request_host_money(
            crate::scripting::executor::HostScriptMoneyRequest::Set {
                player: player_name,
                amount: money_amount,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doGiveMoney() line (in header)
    pub(crate) fn do_give_money(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let money_amount = self.get_int_param(action, 1)?;

        log::info!("Giving player '{}' {} money", player_name, money_amount);

        // Get player by name and add money (can be negative)
        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.get_money_mut().add_money(money_amount);
                    log::info!("Player '{}' received {} money", player_name, money_amount);
                }
            } else {
                log::warn!("Player '{}' not found for give money", player_name);
            }
        }
        crate::scripting::executor::request_host_money(
            crate::scripting::executor::HostScriptMoneyRequest::Give {
                player: player_name,
                amount: money_amount,
            },
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_grant_science(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        use game_engine::common::rts::science::{SCIENCE_INVALID, get_science_store};

        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let science_name = self.get_string_param(action, 1)?;

        log::info!(
            "Granting player '{}' science '{}'",
            player_name,
            science_name
        );

        // Look up the science type by name
        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.grant_science(science_type);
                    log::info!(
                        "Player '{}' granted science '{}'",
                        player_name,
                        science_name
                    );
                }
            } else {
                log::warn!("Player '{}' not found for grant science", player_name);
            }
        }
        crate::scripting::executor::request_host_science_action(&player_name, &science_name, true);

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doPlayerKill() -> Player::killPlayer()
    pub(crate) fn do_player_kill(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);

        log::info!("Killing all units for player '{}' (scripted)", player_name);

        // Live host objects are not in leftover OBJECT_REGISTRY. Always queue.
        super::request_host_script_player_misc(super::HostScriptPlayerMiscRequest::Kill {
            player: player_name.clone(),
        });

        let player_arc = {
            let list = player_list();
            let Ok(list_guard) = list.read() else {
                return Ok(ScriptActionResult::Success);
            };
            list_guard.find_player_by_name(&player_name).clone()
        };

        // Drop PlayerList before kill_player: Team::kill_team re-enters the list.
        if let Some(player_arc) = player_arc {
            if let Ok(mut player) = player_arc.write() {
                player.kill_player();
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_player_hunt(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);

        log::info!("Player '{}' units hunting", player_name);
        if dual_world_registry_unavailable() {
            super::request_host_script_hunt_guard(super::HostScriptHuntGuardRequest::PlayerHunt {
                player: player_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_units_should_hunt(true, CommandSourceType::FromScript);
                    log::info!("Player '{}' units now hunting", player_name);
                }
            } else {
                log::warn!("Player '{}' not found for hunt", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // DISPLAY/UI ACTIONS
    // C++ Reference: ScriptActions.cpp line (display text)
    // ============================================================================

    pub(crate) fn do_display_text(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let text = self.get_string_param(action, 0)?;

        log::info!("Displaying text: {}", text);
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.display_text(&text) {
                log::warn!("Script action handler display_text failed: {}", err);
            }
            return Ok(ScriptActionResult::Success);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_display_cinematic_text(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let text = self.get_string_param(action, 0)?;
        let font_type = action
            .get_parameter(1)
            .map(|p| p.get_string().to_string())
            .unwrap_or_else(|| "Default".to_string());
        let duration_seconds = action.get_parameter(2).map(|p| p.get_int()).unwrap_or(0);

        log::info!(
            "Displaying cinematic text: {} (font: {}, duration: {}s)",
            text,
            font_type,
            duration_seconds
        );
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.display_cinematic_text(&text, &font_type, duration_seconds) {
                log::warn!(
                    "Script action handler display_cinematic_text failed: {}",
                    err
                );
            }
            return Ok(ScriptActionResult::Success);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_military_caption(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let briefing_text = self.get_string_param(action, 0)?;
        let mut duration_ms = self.get_int_param(action, 1)?;

        if let Ok(global) = global_data::read_safe() {
            if global.writable.disable_military_caption {
                duration_ms = 1;
            }
        }

        log::info!(
            "Showing military caption: {} (duration: {} ms)",
            briefing_text,
            duration_ms
        );

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.military_caption(&briefing_text, duration_ms) {
                log::warn!("Script action handler military_caption failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // CAMERA ACTIONS
    // C++ Reference: ScriptActions.cpp line (move camera)
    // ============================================================================

    pub(crate) fn do_move_camera_to(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // Ini scripts can call this with either:
        // - `MOVE_CAMERA_TO X:.. Y:.. Z:..` (coordinate)
        // - `MOVE_CAMERA_TO WaypointName <duration>` (waypoint + optional duration)
        let Some(param0) = action.get_parameter(0) else {
            return Err(ScriptError::ParameterNotFound(
                "Parameter 0 not found".to_string(),
            ));
        };

        let duration_seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let camera_stutter_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(4).map(|p| p.get_real()).unwrap_or(0.0);

        let target = if param0.get_parameter_type() == ParameterType::Coord3D {
            let pos = param0.get_coord();
            Some(crate::common::Coord3D::new(pos.x, pos.y, pos.z))
        } else {
            let waypoint_name = param0.get_string();
            let waypoint_ascii = AsciiString::from(waypoint_name);
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(target) = target else {
            log::warn!(
                "MOVE_CAMERA_TO: waypoint '{}' not found; action ignored",
                param0.get_string()
            );
            return Ok(ScriptActionResult::Success);
        };

        log::info!(
            "Moving camera to ({}, {}, {}) (sec: {}, stutter: {}, ease_in: {}, ease_out: {})",
            target.x,
            target.y,
            target.z,
            duration_seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.move_camera_to(
                target.x,
                target.y,
                target.z,
                duration_seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            ) {
                log::warn!("Script action handler move_camera_to failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doCameraFollowNamed() line 468
    pub(crate) fn do_camera_follow_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let snap_to_unit = self.get_bool_param_optional(action, 1).unwrap_or(false);

        log::info!(
            "Camera following named unit '{}' (snap: {})",
            unit_name,
            snap_to_unit
        );

        // C++ `TheScriptEngine->getUnitNamed` — host injects name→id into the
        // crate tracker. Empty OBJECT_REGISTRY must not skip named lookup.
        let tracker = get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&unit_name).ok().flatten();

        // Wave 284: empty dual-world → skip crate Object walk only.
        if object_id.is_none() && !dual_world_registry_unavailable() {
            let lower = unit_name.to_ascii_lowercase();
            object_id = OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find_map(|obj_ref| {
                    obj_ref.read().ok().and_then(|obj| {
                        if obj.get_name().to_ascii_lowercase() == lower {
                            Some(obj.get_id())
                        } else {
                            None
                        }
                    })
                });
        }

        let Some(object_id) = object_id else {
            log::warn!("Camera follow failed: unit '{}' not found", unit_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.camera_follow_object(object_id, snap_to_unit) {
                log::warn!("Script action handler camera_follow_object failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doStopCameraFollowUnit() line 484
    pub(crate) fn do_stop_camera_follow(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Stopping camera follow");

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.stop_camera_follow() {
                log::warn!("Script action handler stop_camera_follow failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_reset_camera(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let duration_seconds = self.get_real_param(action, 1)?;
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);

        log::info!(
            "Resetting camera to waypoint '{}' over {} seconds (ease_in: {}, ease_out: {})",
            waypoint_name,
            duration_seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!("RESET_CAMERA: waypoint '{}' not found", waypoint_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.reset_camera_to(
                target.x,
                target.y,
                target.z,
                duration_seconds,
                ease_in_seconds,
                ease_out_seconds,
            ) {
                log::warn!("Script action handler reset_camera_to failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // AUDIO ACTIONS
    // C++ Reference: ScriptActions.cpp line 353 (play sound)
    // ============================================================================

    /// C++ Reference: ScriptActions::doPlaySoundEffect() line 353
    pub(crate) fn do_play_sound_effect(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_name = self.get_string_param(action, 0)?;

        log::info!("Playing sound effect: {}", sound_name);
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.play_sound_effect(&sound_name) {
                log::warn!("Script action handler play_sound_effect failed: {}", err);
            }
            return Ok(ScriptActionResult::Success);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doPlaySoundEffectAt() line 365
    pub(crate) fn do_play_sound_effect_at(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;

        log::info!(
            "Playing sound effect '{}' at waypoint '{}'",
            sound_name,
            waypoint_name
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "PLAY_SOUND_EFFECT_AT: waypoint '{}' not found; action ignored",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) =
                handler.play_sound_effect_at(&sound_name, target.x, target.y, target.z)
            {
                log::warn!("Script action handler play_sound_effect_at failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_speech_play(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let speech_name = self.get_string_param(action, 0)?;
        let allow_overlap = self.get_bool_param_optional(action, 1).unwrap_or(false);

        log::info!(
            "Playing speech: {} (overlap: {})",
            speech_name,
            allow_overlap
        );

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.speech_play(&speech_name, allow_overlap) {
                log::warn!("Script action handler speech_play failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_music_track_change(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let track_name = self.get_string_param(action, 0)?;
        let fade_out = self.get_bool_param_optional(action, 1).unwrap_or(true);
        let fade_in = self.get_bool_param_optional(action, 2).unwrap_or(true);

        log::debug!(
            "Changing music to '{}' (fade out: {}, fade in: {})",
            track_name,
            fade_out,
            fade_in
        );

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.music_set_track(&track_name, fade_out, fade_in) {
                log::warn!("Script action handler music_set_track failed: {}", err);
            }
        }

        let _ = with_script_engine_mut(|engine| engine.set_current_track_name(track_name.clone()));

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // RADAR ACTIONS
    // ============================================================================

    pub(crate) fn do_radar_disable(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Disabling radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.hide(true);
        }

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_radar_enabled(false) {
                log::warn!(
                    "Script action handler set_radar_enabled(false) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_radar_enable(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Enabling radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.hide(false);
        }

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_radar_enabled(true) {
                log::warn!(
                    "Script action handler set_radar_enabled(true) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_reveal_map_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let radius = self.get_real_param(action, 1)?;
        let player_name = self.get_string_param(action, 2)?;

        log::info!(
            "Revealing map at waypoint '{}' with radius {} for player '{}'",
            waypoint_name,
            radius,
            player_name
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "REVEAL_MAP_AT_WAYPOINT: waypoint '{}' not found; action ignored",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let player_mask = if let Some(player_arc) = players.find_player_by_name(&player_name) {
            let Ok(player) = player_arc.read() else {
                return Ok(ScriptActionResult::Success);
            };
            player.get_player_mask().bits()
        } else {
            players
                .iter()
                .filter_map(|player_arc| player_arc.read().ok())
                .filter(|player| player.get_player_type() == PlayerType::Human)
                .fold(0u32, |mask, player| mask | player.get_player_mask().bits())
        };

        if player_mask != 0 {
            let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
            if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
                shroud_mgr.do_shroud_reveal(&target, radius, player_mask);
                shroud_mgr.undo_shroud_reveal(&target, radius, player_mask);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_shroud_map_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let radius = self.get_real_param(action, 1)?;
        let player_name = self.get_string_param(action, 2)?;

        log::info!(
            "Shrouding map at waypoint '{}' with radius {} for player '{}'",
            waypoint_name,
            radius,
            player_name
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "SHROUD_MAP_AT_WAYPOINT: waypoint '{}' not found; action ignored",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let player_mask = if let Some(player_arc) = players.find_player_by_name(&player_name) {
            let Ok(player) = player_arc.read() else {
                return Ok(ScriptActionResult::Success);
            };
            player.get_player_mask().bits()
        } else {
            players
                .iter()
                .filter_map(|player_arc| player_arc.read().ok())
                .filter(|player| player.get_player_type() == PlayerType::Human)
                .fold(0u32, |mask, player| mask | player.get_player_mask().bits())
        };

        if player_mask != 0 {
            let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
            if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
                shroud_mgr.do_shroud_cover(&target, radius, player_mask);
                shroud_mgr.undo_shroud_cover(&target, radius, player_mask);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // DISABLE_INPUT / ENABLE_INPUT live in `actions_input_ui.rs`.

    // ============================================================================
    // COUNTER/FLAG/TIMER ACTION IMPLEMENTATIONS
    // C++ Reference: ScriptActions.cpp
    // ============================================================================

    /// C++ Reference: ScriptActions::doSetFlag()
    pub(crate) fn do_set_flag(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let flag_name = self.get_string_param(action, 0)?;
        let value = self.get_int_param(action, 1)? != 0;
        log::debug!("Setting flag '{}' to {}", flag_name, value);

        // Re-entrant: may run nested under CALL_SUBROUTINE.
        let _ = with_script_engine_mut(|engine| engine.set_flag(&flag_name, value));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetCounter()
    pub(crate) fn do_set_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        let value = self.get_int_param(action, 1)?;
        log::debug!("Setting counter '{}' to {}", counter_name, value);

        let _ = with_script_engine_mut(|engine| engine.set_counter(&counter_name, value));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptEngine::addCounter() — param0=amount, param1=counter name.
    pub(crate) fn do_increment_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let amount = self.get_int_param(action, 0)?;
        let counter_name = self.get_string_param(action, 1)?;
        log::debug!("Incrementing counter '{}' by {}", counter_name, amount);

        let _ = with_script_engine_mut(|engine| engine.increment_counter(&counter_name, amount));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptEngine::subCounter() — param0=amount, param1=counter name.
    pub(crate) fn do_decrement_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let amount = self.get_int_param(action, 0)?;
        let counter_name = self.get_string_param(action, 1)?;
        log::debug!("Decrementing counter '{}' by {}", counter_name, amount);

        let _ = with_script_engine_mut(|engine| engine.decrement_counter(&counter_name, amount));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptEngine::setTimer(non-msec) stores INT frames verbatim.
    pub(crate) fn do_set_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Setting timer '{}' to {} frames", timer_name, frames);

        let _ = with_script_engine_mut(|engine| engine.set_timer(&timer_name, frames));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetMillisecondTimer()
    pub(crate) fn do_set_millisecond_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let seconds = self.get_real_param(action, 1)?;
        log::debug!(
            "Setting legacy millisecond timer '{}' to {} script-seconds",
            timer_name,
            seconds
        );

        let _ = with_script_engine_mut(|engine| {
            engine.set_timer_millisecond_script_seconds(&timer_name, seconds)
        });
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetRandomTimer()
    pub(crate) fn do_set_random_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let min_seconds = self.get_int_param(action, 1)?;
        let max_seconds = self.get_int_param(action, 2)?;
        log::debug!(
            "Setting random timer '{}' between {}-{} frames",
            timer_name,
            min_seconds,
            max_seconds
        );

        let random_frames = get_game_logic_random_value(min_seconds, max_seconds);

        let _ = with_script_engine_mut(|engine| engine.set_timer(&timer_name, random_frames));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetRandomMsecTimer()
    pub(crate) fn do_set_random_msec_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let min_seconds = self.get_real_param(action, 1)?;
        let max_seconds = self.get_real_param(action, 2)?;
        log::debug!(
            "Setting legacy random millisecond timer '{}' between {}-{} script-seconds",
            timer_name,
            min_seconds,
            max_seconds
        );

        let random_seconds = get_game_logic_random_value_real(min_seconds, max_seconds);

        let _ = with_script_engine_mut(|engine| {
            engine.set_timer_millisecond_script_seconds(&timer_name, random_seconds)
        });
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doStopTimer()
    pub(crate) fn do_stop_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        log::debug!("Stopping timer '{}'", timer_name);

        let _ = with_script_engine_mut(|engine| engine.stop_timer(&timer_name));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doRestartTimer()
    pub(crate) fn do_restart_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        log::debug!("Restarting timer '{}'", timer_name);

        let _ = with_script_engine_mut(|engine| engine.restart_timer(&timer_name));
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doAddToMsecTimer()
    pub(crate) fn do_add_to_msec_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let seconds = self.get_real_param(action, 0)?;
        let timer_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Adding {} script-seconds to legacy millisecond timer '{}'",
            seconds,
            timer_name
        );

        let _ = with_script_engine_mut(|engine| {
            engine.add_to_timer_millisecond_script_seconds(&timer_name, seconds)
        });
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSubFromMsecTimer()
    pub(crate) fn do_sub_from_msec_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let seconds = self.get_real_param(action, 0)?;
        let timer_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Subtracting {} script-seconds from legacy millisecond timer '{}'",
            seconds,
            timer_name
        );

        let _ = with_script_engine_mut(|engine| {
            engine.subtract_from_timer_millisecond_script_seconds(&timer_name, seconds)
        });
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // SCRIPT CONTROL ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_enable_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let script_name = self.get_string_param(action, 0)?;
        log::debug!("Enabling script '{}'", script_name);
        let found =
            with_script_engine_mut(|engine| engine.set_script_active_by_name(&script_name, true))
                .unwrap_or(false);
        if !found {
            log::warn!("ENABLE_SCRIPT: script '{}' not found", script_name);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let script_name = self.get_string_param(action, 0)?;
        log::debug!("Disabling script '{}'", script_name);
        let found =
            with_script_engine_mut(|engine| engine.set_script_active_by_name(&script_name, false))
                .unwrap_or(false);
        if !found {
            log::warn!("DISABLE_SCRIPT: script '{}' not found", script_name);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_call_subroutine(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let subroutine_name = self.get_string_param(action, 0)?;
        log::debug!("Calling subroutine '{}'", subroutine_name);

        // CRITICAL: never hold get_script_engine().write() across execute_subroutine_by_name.
        // Nested CALL_SUBROUTINE / set_flag / set_timer re-enter the same std RwLock and
        // deadlocked campaign maps (MD_USA01 SUB-Generate Random Number).
        let found = match with_script_engine_mut(|engine| {
            engine
                .execute_subroutine_by_name(&subroutine_name)
                .map_err(|e| ScriptError::ExecutionFailed(e.to_string()))
        }) {
            Some(result) => result?,
            None => false,
        };

        if !found {
            log::warn!(
                "CALL_SUBROUTINE: subroutine '{}' not found",
                subroutine_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_debug_message_box(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        log::info!("[DEBUG MESSAGE BOX] {}", message);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_debug_string(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        log::debug!("[DEBUG STRING] {}", message);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_debug_crash_box(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        log::error!("[DEBUG CRASH] {}", message);
        Ok(ScriptActionResult::Success)
    }
}
