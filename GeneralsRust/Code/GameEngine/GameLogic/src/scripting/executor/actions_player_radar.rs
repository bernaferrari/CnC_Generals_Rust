//! Player economy, radar, and debug actions from ScriptActions.cpp.

use super::*;
use crate::scripting::engine::ScriptExecutionDriver;

impl ScriptActionDispatcher<'_> {
    // ============================================================================
    // PLAYER ACTIONS
    // C++ Reference: ScriptActions.cpp line (set money)
    // ============================================================================

    /// C++ Reference: ScriptActions::doSetMoney() line (in header)
    pub(crate) fn do_set_money(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let money_amount = self.get_int_param(action, 1)?;
        let request = crate::scripting::HostScriptMoneyRequest::Set {
            player: player_name.clone(),
            amount: money_amount,
        };
        let current_player = self
            .context
            .with_engine_ref(|engine| engine.get_current_player_name())
            .flatten();
        if let Some(result) = driver.money(&request, current_player.as_deref()) {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }

        let player_name = self.resolve_player_name_token(&player_name);
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
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doGiveMoney() line (in header)
    pub(crate) fn do_give_money(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<crate::scripting::engine::ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let money_amount = self.get_int_param(action, 1)?;
        let request = crate::scripting::HostScriptMoneyRequest::Give {
            player: player_name.clone(),
            amount: money_amount,
        };
        let current_player = self
            .context
            .with_engine_ref(|engine| engine.get_current_player_name())
            .flatten();
        if let Some(result) = driver.money(&request, current_player.as_deref()) {
            result.map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
            return Ok(ScriptActionResult::Success);
        }

        let player_name = self.resolve_player_name_token(&player_name);
        log::info!("Giving player '{}' {} money", player_name, money_amount);

        // Get player by name and add money (can be negative)
        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    let facts =
                        crate::helpers::capture_player_audio_locality(&player_guard, &list_guard);
                    player_guard
                        .get_money_mut_with_locality(facts)
                        .add_money(money_amount);
                    log::info!("Player '{}' received {} money", player_name, money_amount);
                }
            } else {
                log::warn!("Player '{}' not found for give money", player_name);
            }
        }
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
    // RADAR ACTIONS
    // ============================================================================

    pub(crate) fn do_radar_disable(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Disabling radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.hide(true);
        }

        if let Some(handler) = self.current_script_action_handler() {
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

        if let Some(handler) = self.current_script_action_handler() {
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
        let terrain_owner_handle = get_terrain_logic();
        let target = terrain_owner_handle.read().ok().and_then(|terrain| {
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

        let __player_list_arc = player_list();
        let Ok(players) = __player_list_arc.read() else {
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
        let terrain_owner_handle = get_terrain_logic();
        let target = terrain_owner_handle.read().ok().and_then(|terrain| {
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

        let __player_list_arc = player_list();
        let Ok(players) = __player_list_arc.read() else {
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
    // SCRIPT CONTROL ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_call_subroutine(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let subroutine_name = self.get_string_param(action, 0)?;
        log::debug!("Calling subroutine '{}'", subroutine_name);

        // CRITICAL: never hold get_script_engine().write() across execute_subroutine_by_name.
        // Nested CALL_SUBROUTINE / set_flag / set_timer re-enter the same std RwLock and
        // deadlocked campaign maps (MD_USA01 SUB-Generate Random Number).
        let found = match self.context.with_engine_mut(|engine| {
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
