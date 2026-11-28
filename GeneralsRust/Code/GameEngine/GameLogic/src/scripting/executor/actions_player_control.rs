//! Player economy, subroutine, and debug actions from ScriptActions.cpp.

use super::*;
use crate::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
use generals_sim::scripting::player_actions::PlayerAction;
use generals_sim::scripting::world::HostScriptMoneyRequest;

impl ScriptActionDispatcher<'_> {
    pub(crate) fn execute_standalone_player_action(
        &mut self,
        action: PlayerAction<'_>,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        match action {
            PlayerAction::Money(HostScriptMoneyRequest::Set { player, amount }) => {
                self.do_set_money(&player, amount)
            }
            PlayerAction::Money(HostScriptMoneyRequest::Give { player, amount }) => {
                self.do_give_money(&player, amount)
            }
            PlayerAction::Science { player, science, grant } => {
                self.do_player_science_action(player, science, grant)
            }
            PlayerAction::ScienceAvailability { player, science, availability } => {
                self.do_player_science_availability(player, science, availability)
            }
            PlayerAction::Construction(request) => self.do_player_construction(request),
            PlayerAction::Rank(request) => self.do_player_rank(request, driver),
            PlayerAction::Kill(player) => self.do_player_kill(player),
            PlayerAction::Sell(player) => self.do_player_sell_everything(player),
            PlayerAction::Transfer { from, to } => self.do_player_transfer_ownership_player(from, to),
            PlayerAction::IgnoreInvalidScienceAvailability => Ok(ScriptActionResult::Success),
        }
    }

    // ============================================================================
    // PLAYER ACTIONS
    // C++ Reference: ScriptActions.cpp line (set money)
    // ============================================================================

    /// C++ Reference: ScriptActions::doSetMoney() line (in header)
    fn do_set_money(
        &mut self,
        player_token: &str,
        money_amount: i32,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(player_token);
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
    fn do_give_money(
        &mut self,
        player_token: &str,
        money_amount: i32,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(player_token);
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

    /// C++ Reference: ScriptActions::doPlayerKill() -> Player::killPlayer()
    fn do_player_kill(
        &mut self,
        player: &str,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(player);

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
