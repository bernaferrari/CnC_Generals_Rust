//! UI countdown, time, weather, and command-bar actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;
use crate::scripting::engine::{
    ScriptExecutionDriver, ScriptWaterRequest,
};

impl ScriptActionDispatcher<'_> {
    // ============================================================================
    // ADDITIONAL DISPLAY/UI ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_cameo_flash(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let cameo_name = self.get_string_param(action, 0)?;
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!("Flashing cameo '{}' for {}s", cameo_name, time_in_seconds);

        let frames = LOGICFRAMES_PER_SECOND as i32 * time_in_seconds;
        let drawable_frames_per_flash = (LOGICFRAMES_PER_SECOND as i32 / 2).max(1);
        let mut count = frames / drawable_frames_per_flash;
        // C++: ensure the cameo ends in its original visual state.
        if (count % 2) == 1 {
            count += 1;
        }

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.cameo_flash(&cameo_name, count) {
                log::warn!("Script action handler cameo_flash failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_display_countdown_timer(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let timer_text = self.get_string_param(action, 1)?;
        log::debug!(
            "Displaying countdown timer '{}' text '{}'",
            timer_name,
            timer_text
        );

        if let Some(result) = driver.world(crate::scripting::engine::ScriptWorldRequest::Timer {
            name: &timer_name,
            text: &timer_text,
            countdown: true,
        }) {
            if let Err(error) = result {
                log::warn!("Script world effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.add_named_timer(&timer_name, &timer_text, true) {
                log::warn!("Script action handler add_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_hide_countdown_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding countdown timer '{}'", timer_name);

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.remove_named_timer(&timer_name) {
                log::warn!("Script action handler remove_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_enable_countdown_timer_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling countdown timer display");

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.show_named_timer_display(true) {
                log::warn!(
                    "Script action handler show_named_timer_display(true) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_countdown_timer_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling countdown timer display");

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.show_named_timer_display(false) {
                log::warn!(
                    "Script action handler show_named_timer_display(false) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_display_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        let counter_text = self.get_string_param(action, 1)?;
        log::debug!(
            "Displaying counter '{}' text '{}'",
            counter_name,
            counter_text
        );

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.add_named_timer(&counter_name, &counter_text, false) {
                log::warn!("Script action handler add_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_hide_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding counter '{}'", counter_name);

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.remove_named_timer(&counter_name) {
                log::warn!("Script action handler remove_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_special_power_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling special power display");

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.set_superweapon_display_enabled_by_script(false) {
                log::warn!(
                    "Script action handler set_superweapon_display_enabled_by_script(false) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_enable_special_power_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling special power display");

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.set_superweapon_display_enabled_by_script(true) {
                log::warn!(
                    "Script action handler set_superweapon_display_enabled_by_script(true) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_ingame_popup_message(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        let x_percent = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(50);
        let y_percent = action.get_parameter(2).map(|p| p.get_int()).unwrap_or(50);
        let width = action.get_parameter(3).map(|p| p.get_int()).unwrap_or(400);
        let pause = action
            .get_parameter(4)
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        log::info!(
            "In-game popup: '{}' at ({}, {}) width {} pause {}",
            message,
            x_percent,
            y_percent,
            width,
            pause
        );

        if let Some(result) =
            driver.popup_message(&message, x_percent, y_percent, width, pause, false)
        {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }
        // C++: TheInGameUI->popupMessage(message, x, y, width, pause, FALSE)
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) =
                handler.popup_message(&message, x_percent, y_percent, width, pause, false)
            {
                return Err(ScriptError::ExecutionFailed(err.to_string()));
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_object_force_select(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let center_in_view = action
            .get_parameter(2)
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        let audio_to_play = action
            .get_parameter(3)
            .map(|p| p.get_string().to_string())
            .unwrap_or_default();

        log::debug!(
            "Force selecting object type '{}' on team '{}' (center_in_view: {}, audio: '{}')",
            object_type,
            team_name,
            center_in_view,
            audio_to_play
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_force_select(super::HostScriptForceSelectRequest {
                team: team_name,
                object_type,
                center_in_view,
                audio: audio_to_play,
            });
            return Ok(ScriptActionResult::Success);
        }

        let published_team_factory = get_team_factory();
        let team_arc = published_team_factory
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name));
        let Some(team_arc) = team_arc else {
            return Ok(ScriptActionResult::Success);
        };

        let member_ids = if let Ok(team_guard) = team_arc.read() {
            team_guard.get_members().to_vec()
        } else {
            Vec::new()
        };

        let mut best_guess: Option<ObjectID> = None;
        for member_id in member_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_template_name() != object_type {
                continue;
            }
            if obj_guard.get_drawable().is_none() {
                continue;
            }
            if best_guess.is_none() || member_id < best_guess.unwrap_or(member_id) {
                best_guess = Some(member_id);
            }
        }

        let Some(selected_id) = best_guess else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(selected_obj) = TheGameLogic::find_object_by_id(selected_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let __player_list_arc = player_list();
            let local_player_mask = __player_list_arc.read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_mask()))
            .unwrap_or(crate::common::PLAYERMASK_ALL);

        let mut selected_pos = Coord3D::ZERO;
        if let Ok(selected_guard) = selected_obj.read() {
            selected_pos = *selected_guard.get_position();
            let _ = TheGameLogic::select_object(&*selected_guard, true, local_player_mask, true);
        }

        if !audio_to_play.is_empty() {
            let mut audio_event = crate::common::audio::AudioEventRts::new(audio_to_play.as_str());
            if let Some(local_player) = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
            {
                if let Ok(local_guard) = local_player.read() {
                    audio_event.set_player_index(local_guard.get_player_index() as u32);
                }
            }
            if let Some(audio) = self.the_audio() {
                let _ = audio.add_audio_event(&audio_event);
            }
        }

        if center_in_view {
            if let Some(handler) = self.current_script_action_handler() {
                let _ = handler.move_camera_to(
                    selected_pos.x,
                    selected_pos.y,
                    selected_pos.z,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // TIME CONTROL ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_freeze_time(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Freezing time");
        let _ = self
            .context
            .with_engine_mut(|script_engine| script_engine.do_freeze_time());
        if let Some(result) =
            driver.world(crate::scripting::engine::ScriptWorldRequest::Freeze(true))
        {
            if let Err(error) = result {
                log::warn!("Script world effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.freeze_time() {
                log::warn!("Script action handler freeze_time failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unfreeze_time(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Unfreezing time");
        let _ = self
            .context
            .with_engine_mut(|script_engine| script_engine.do_unfreeze_time());
        if let Some(result) =
            driver.world(crate::scripting::engine::ScriptWorldRequest::Freeze(false))
        {
            if let Err(error) = result {
                log::warn!("Script world effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.unfreeze_time() {
                log::warn!("Script action handler unfreeze_time failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_visual_speed_multiplier(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let multiplier = self.get_int_param(action, 0)?;
        log::debug!("Setting visual speed multiplier to {}", multiplier);
        if let Some(result) = driver.world(
            crate::scripting::engine::ScriptWorldRequest::VisualSpeed(multiplier),
        ) {
            if let Err(error) = result {
                log::warn!("Script world effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.set_visual_speed_multiplier(multiplier) {
                log::warn!(
                    "Script action handler set_visual_speed_multiplier failed: {}",
                    err
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_fps_limit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let fps = self.get_int_param(action, 0)?;
        log::debug!("Setting FPS limit to {}", fps);
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.set_fps_limit(fps) {
                log::warn!("Script action handler set_fps_limit failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ENVIRONMENT/WORLD ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_water_change_height(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let water_name = self.get_string_param(action, 0)?;
        let height = self.get_real_param(action, 1)?;
        log::debug!("Changing water '{}' height to {}", water_name, height);

        if let Some(result) = driver.water(ScriptWaterRequest::SetHeight {
            name: &water_name,
            height,
        }) {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }
        let water_name_ascii = AsciiString::from(water_name.as_str());
        if let Ok(mut terrain) = get_terrain_logic().write() {
            if terrain
                .get_water_handle_by_name(&water_name_ascii)
                .is_some()
            {
                terrain.set_water_height(&water_name_ascii, height, 999_999.9, true);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_water_change_height_over_time(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let water_name = self.get_string_param(action, 0)?;
        let height = self.get_real_param(action, 1)?;
        let time = self.get_real_param(action, 2)?;
        let damage = self.get_real_param(action, 3)?;
        log::debug!(
            "Changing water '{}' height to {} over {} seconds (damage {})",
            water_name,
            height,
            time,
            damage
        );

        if let Some(result) = driver.water(ScriptWaterRequest::OverTime {
            name: &water_name,
            height,
            seconds: time,
            damage,
        }) {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }
        let water_name_ascii = AsciiString::from(water_name.as_str());
        if let Ok(mut terrain) = get_terrain_logic().write() {
            terrain.change_water_height_over_time(&water_name_ascii, height, time, damage);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_cave_index(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let cave_name = self.get_string_param(action, 0)?;
        let cave_index = self.get_int_param(action, 1)?;
        log::debug!("Setting cave '{}' index to {}", cave_name, cave_index);

        let (names, objects, caves) = self.context.borrowed_engine().cave_action_owners();
        if let Ok(Some(object_id)) = names.get_object_id(&cave_name) {
            if let Some(object) = objects.get_object(object_id) {
                if let Ok(object) = object.read() {
                    if let Some(contain) = object.get_contain() {
                        if let Ok(mut contain) = contain.lock() {
                            if let Ok(mut caves) = caves.lock() {
                                if let Err(error) = contain.try_to_set_cave_index(cave_index, &mut caves) {
                                    log::warn!("Cave '{}' index switch failed: {}", cave_name, error);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_show_weather(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let show_weather = self.get_bool_param_optional(action, 0).unwrap_or(true);
        log::debug!("Setting weather visibility to {}", show_weather);

        if let Some(result) = driver.world(crate::scripting::engine::ScriptWorldRequest::Weather(
            show_weather,
        )) {
            if let Err(error) = result {
                log::warn!("Script world effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.set_weather_visible(show_weather) {
                log::warn!("Script action handler set_weather_visible failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_infantry_lighting_override(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let setting = self.get_real_param(action, 0)?;
        if setting != -1.0 && setting <= 0.0 {
            log::warn!(
                "Invalid infantry lighting override {}; expected -1.0 or > 0.0",
                setting
            );
        }
        if let Ok(mut gd) = global_data::write_safe() {
            gd.script_override_infantry_light_scale = setting;
        }
        log::debug!("Setting infantry lighting override to {}", setting);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_reset_infantry_lighting_override(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        if let Ok(mut gd) = global_data::write_safe() {
            gd.script_override_infantry_light_scale = -1.0;
        }
        log::debug!("Resetting infantry lighting override to -1.0");
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // CONSTRUCTION/TECHTREE ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_set_base_construction_speed(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let Some(player_name) =
            self.resolve_player_name_token_with_driver(&self.get_string_param(action, 0)?, driver)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let delay_seconds = self.get_int_param(action, 1)?;
        // CPP updateBaseConstructionSpeed calls Player::setTeamDelaySeconds;
        // the original signed seconds are carried unchanged to the owner.
        if let Some(result) = driver.ai_player(
            crate::scripting::engine::ScriptAiPlayerRequest::SetTeamDelay {
                player: &player_name,
                seconds: delay_seconds,
            },
        ) {
            if let Err(error) = result {
                log::warn!("Owned scripted AI player effect failed: {error}");
            }
            return Ok(ScriptActionResult::Success);
        }
        log::debug!(
            "Setting base construction speed for '{}' to {} seconds",
            player_name,
            delay_seconds
        );

        let __player_list_arc = player_list();
        if let Ok(list_guard) = __player_list_arc.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_team_delay_seconds(delay_seconds);
                };
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_techtree_modify_buildability_object(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_type = self.get_string_param(action, 0)?;
        let buildable_status = self.get_int_param(action, 1)?;
        if let Some(result) = driver.set_buildable_status_override(&object_type, buildable_status) {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }
        // Live leftover ThingFactory is empty. Always write the override by
        // the script object-type name so host can_make_unit can read it.
        TheGameLogic::set_buildable_status_override(&object_type, buildable_status);
        if let Some(template) = TheObjectFactory::find_template(&object_type) {
            TheGameLogic::set_buildable_status_override(
                template.get_name().as_str(),
                buildable_status,
            );
        }
        crate::scripting::executor::request_host_buildable_status_override(
            &object_type,
            buildable_status,
        );
        log::debug!(
            "Modifying buildability for '{}' to status {}",
            object_type,
            buildable_status
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_warehouse_set_value(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let warehouse_name = self.get_string_param(action, 0)?;
        let value = self.get_int_param(action, 1)?;
        log::debug!("Setting warehouse '{}' value to {}", warehouse_name, value);

        // Live host: leftover OBJECT_REGISTRY is empty. Always push the
        // C++ setCashValue through MissionScriptActionHandler.
        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.set_warehouse_value(&warehouse_name, value) {
                log::warn!("Script action handler set_warehouse_value failed: {}", err);
            }
        }

        let tracker = get_named_object_tracker();
        let Some(warehouse_id) = tracker.get_object_id(&warehouse_name).ok().flatten() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(warehouse_arc) = TheGameLogic::find_object_by_id(warehouse_id) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(warehouse_guard) = warehouse_arc.read() {
            let Some(module) = warehouse_guard.find_update_module("SupplyWarehouseDockUpdate")
            else {
                return Ok(ScriptActionResult::Success);
            };

            module.with_module(|module| {
                if let Some(warehouse) = module.get_supply_warehouse_dock_interface() {
                    warehouse.set_cash_value(value);
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_command_bar_remove_button_object_type(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let button_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;

        if let Some(result) = driver.command_bar_override(&button_name, &object_type, None) {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }
        let Some(template) = TheObjectFactory::find_template(&object_type) else {
            return Ok(ScriptActionResult::Success);
        };
        let command_set_name = template.get_command_set_string().as_str().to_string();

        let slot = self.owning_control_bar_bridge().and_then(|control_bar| {
            control_bar
                .find_command_set_by_name(command_set_name.as_str())
                .and_then(|set| {
                    set.buttons.iter().position(|button| {
                        button
                            .as_ref()
                            .map(|b| b.name.eq_ignore_ascii_case(&button_name))
                            .unwrap_or(false)
                    })
                })
        });

        if let Some(slot) = slot {
            let _ = self.set_owning_command_set_slot_override(command_set_name.as_str(), slot, None);
        }

        log::debug!(
            "Removing command bar button '{}' for '{}'",
            button_name,
            object_type
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_command_bar_add_button_object_type_slot(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let button_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let slot_num = self.get_int_param(action, 2)?;

        if let Some(result) =
            driver.command_bar_override(&button_name, &object_type, Some(slot_num))
        {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }
        let Some(template) = TheObjectFactory::find_template(&object_type) else {
            return Ok(ScriptActionResult::Success);
        };
        let command_set_name = template.get_command_set_string().as_str().to_string();

        let slot = slot_num - 1;
        if !(0..crate::command_button::MAX_COMMANDS_PER_SET as i32).contains(&slot) {
            return Ok(ScriptActionResult::Success);
        }

        let _ = self.set_owning_command_set_slot_override(
            command_set_name.as_str(),
            slot as usize,
            Some(button_name.as_str()),
        );

        log::debug!(
            "Adding command bar button '{}' for '{}' at slot {}",
            button_name,
            object_type,
            slot_num
        );
        Ok(ScriptActionResult::Success)
    }
}
