//! Player money, resource, and handicap script actions
//!
//! C++: ScriptActions.cpp `doSetMoney` L3980, `doGiveMoney` L3999.
//!
//! Split from `scripting/actions.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::ScriptAction;
use super::helpers::*;
use crate::action_manager::TheActionManager;
use crate::ai::integration::with_ai_integration_mut;
use crate::ai::{AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, GuardMode, the_ai};
use crate::commands::command::CommandType;
use crate::commands::{Command, CommandPriority, QueuedCommand, get_command_queue_manager};
use crate::common::PlayerIndex;
use crate::common::{
    AsciiString, CommandSourceType, Coord3D, INVALID_OBJECT_ID, LocomotorSetType, Real,
    Relationship,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::effects::FXList;
use crate::helpers::{TheGameLogic, TheVictoryConditions};
use crate::modules::{AIUpdateInterfaceExt};
use crate::object::object_factory::{GameObjectInstance, get_object_factory};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object_manager::{ObjectCreationFlags, get_object_manager};
use crate::player::{PlayerType, player_list};
use crate::scripting::core::{LOCAL_PLAYER, TEAM_THE_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_TEAM};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::{ScriptContext, ScriptResult, ScriptValue};
use crate::system::shroud_manager::get_shroud_manager;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{RadarEventType, get_radar_system};

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Set player resource action
pub(super) struct SetPlayerResourceAction;

#[async_trait]
impl ScriptAction for SetPlayerResourceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let resource_type = get_string_param(parameters, "resource_type")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Setting player {} {} to {}", player, resource_type, amount);

        if is_money_resource(&resource_type) {
            let new_amount = clamp_script_money(amount);
            let _ = crate::player::with_player_mut(player as i32, |player_guard| {
                set_script_player_money(player_guard, new_amount);
            });
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_player_resource"
    }

    fn description(&self) -> &str {
        "Sets a player's resource amount"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "resource_type".to_string(),
            "amount".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Add player resource action
pub(super) struct AddPlayerResourceAction;

#[async_trait]
impl ScriptAction for AddPlayerResourceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let resource_type = get_string_param(parameters, "resource_type")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Adding {} {} to player {}", amount, resource_type, player);

        if is_money_resource(&resource_type) {
            let _ = crate::player::with_player_mut(player as i32, |player_guard| {
                if amount < 0 {
                    let requested = clamp_script_money(amount.saturating_neg());
                    spend_script_player_money(player_guard, requested);
                } else {
                    let deposit_amount = clamp_script_money(amount);
                    grant_script_player_money(player_guard, deposit_amount);
                }
            });
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "add_player_resource"
    }

    fn description(&self) -> &str {
        "Adds resources to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "resource_type".to_string(),
            "amount".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Add player money
pub(super) struct PlayerAddMoneyAction;

#[async_trait]
impl ScriptAction for PlayerAddMoneyAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Adding ${} to player {}", amount, player);

        // Integration with player resource system:
        // Adds money to player's resource pool
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. pPlayer->addMoney(amount) or pPlayer->setMoney(current + amount)
        // 3. Updates UI display
        // 4. Amount can be negative to subtract money
        // Rust: player.add_money(amount)

        let player_idx = player.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let _ = crate::player::with_player_mut(player_idx, |player_guard| {
            if amount >= 0 {
                grant_script_player_money(player_guard, clamp_script_money(amount));
            } else {
                spend_script_player_money(
                    player_guard,
                    clamp_script_money(amount.saturating_neg()),
                );
            }
        });

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_add_money"
    }

    fn description(&self) -> &str {
        "Adds money to a player's resources"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// ============================================================================
// HIGH-PRIORITY MISSING ACTIONS - PORTED FROM C++
// ============================================================================

/// Give Money Action - Matches C++ ScriptActions::doGiveMoney (line 3999)
pub(super) struct GiveMoneyAction;

#[async_trait]
impl ScriptAction for GiveMoneyAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Giving ${} to player '{}'", amount, player_name);

        // Matches C++ ScriptActions.cpp:doGiveMoney line 3999
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. Money *m = player->getMoney()
        // 3. if (money < 0) m->withdraw(-money) else m->deposit(money)
        // Supports negative amounts for withdrawing money
        // Rust: player_list.get_player(player_name).add_money(amount)

        let resolved_name = resolve_player_name_token(&player_name);
        let host_amount = amount.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        crate::scripting::executor::request_host_money(
            crate::scripting::executor::HostScriptMoneyRequest::Give {
                player: resolved_name.clone(),
                amount: host_amount,
            },
        );
        if crate::player::with_player_named_mut(&resolved_name, |player_guard| {
            if amount < 0 {
                spend_script_player_money(
                    player_guard,
                    clamp_script_money(amount.saturating_neg()),
                );
            } else {
                grant_script_player_money(player_guard, clamp_script_money(amount));
            }
        })
        .is_none()
        {
            log::warn!("GiveMoneyAction: player '{}' not found", resolved_name);
            return Ok(ScriptResult::Success(None));
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "give_money"
    }

    fn description(&self) -> &str {
        "Gives money to a player (positive) or takes money away (negative)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Money Action - Matches C++ ScriptActions::doSetMoney (line 3980)
pub(super) struct SetMoneyAction;

#[async_trait]
impl ScriptAction for SetMoneyAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Setting player '{}' money to ${}", player_name, amount);

        // Matches C++ ScriptActions.cpp:doSetMoney line 3980
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. Money *m = player->getMoney()
        // 3. m->withdraw(m->countMoney()) // Withdraw all current money
        // 4. m->deposit(money) // Deposit new amount
        // Sets absolute money value (not additive)
        // Rust: player_list.get_player(player_name).set_money(amount)

        let resolved_name = resolve_player_name_token(&player_name);
        let host_amount = amount.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        crate::scripting::executor::request_host_money(
            crate::scripting::executor::HostScriptMoneyRequest::Set {
                player: resolved_name.clone(),
                amount: host_amount,
            },
        );
        if crate::player::with_player_named_mut(&resolved_name, |player_guard| {
            set_script_player_money(player_guard, clamp_script_money(amount));
        })
        .is_none()
        {
            log::warn!("SetMoneyAction: player '{}' not found", resolved_name);
            return Ok(ScriptResult::Success(None));
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_money"
    }

    fn description(&self) -> &str {
        "Sets a player's money to an exact amount"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Handicap Action - Sets player difficulty/handicap modifier
pub(super) struct SetHandicapAction;

#[async_trait]
impl ScriptAction for SetHandicapAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;
        let handicap = get_float_param(parameters, "handicap")?;

        log::info!("Setting player '{}' handicap to {}", player_name, handicap);

        // C++ Implementation (from Player.h/cpp):
        // Handicap affects:
        // - Resource collection rate (multiplier)
        // - Build speed (multiplier)
        // - Unit damage output (multiplier)
        // - Unit health (multiplier)
        // Typical values: 0.5 (easy), 1.0 (normal), 1.5 (hard)
        // player->setHandicap(handicap)
        // Rust: player_list.get_player(player_name).set_handicap(handicap)

        let resolved_name = resolve_player_name_token(&player_name);
        if crate::player::with_player_named_mut(&resolved_name, |player_guard| {
            player_guard.set_handicap(handicap as f32);
        })
        .is_none()
        {
            log::warn!("SetHandicapAction: player '{}' not found", resolved_name);
            return Ok(ScriptResult::Success(None));
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_handicap"
    }

    fn description(&self) -> &str {
        "Sets a player's handicap/difficulty multiplier"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "handicap".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
