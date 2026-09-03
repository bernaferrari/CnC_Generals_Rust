//! Science, upgrade, and special-power script actions
//!
//! C++: ScriptActions.cpp special-power display/fire L3905–4215,
//! `doUnitReceiveUpgrade` L5313, science L5905.
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
use crate::modules::{AIUpdateInterfaceExt, ContainModuleInterfaceExt};
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

/// Trigger special power action
pub(super) struct TriggerSpecialPowerAction;

#[async_trait]
impl ScriptAction for TriggerSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;
        let x = get_float_param_optional(parameters, "x");
        let y = get_float_param_optional(parameters, "y");
        let target_id = get_int_param_optional(parameters, "target_id");

        log::info!(
            "Triggering special power '{}' for player {}",
            power_name,
            player
        );
        if let (Some(x_pos), Some(y_pos)) = (x, y) {
            log::info!("Target position: ({}, {})", x_pos, y_pos);
        }
        if let Some(target) = target_id {
            log::info!("Target object: {}", target);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "trigger_special_power"
    }

    fn description(&self) -> &str {
        "Triggers a special power for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string(), "target_id".to_string()]
    }
}

/// Enable special power action
pub(super) struct EnableSpecialPowerAction;

#[async_trait]
impl ScriptAction for EnableSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;

        log::info!(
            "Enabling special power '{}' for player {}",
            power_name,
            player
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "enable_special_power"
    }

    fn description(&self) -> &str {
        "Enables a special power for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable special power action
pub(super) struct DisableSpecialPowerAction;

#[async_trait]
impl ScriptAction for DisableSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;

        log::info!(
            "Disabling special power '{}' for player {}",
            power_name,
            player
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "disable_special_power"
    }

    fn description(&self) -> &str {
        "Disables a special power for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Grant upgrade action
pub(super) struct GrantUpgradeAction;

#[async_trait]
impl ScriptAction for GrantUpgradeAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let upgrade_name = get_string_param(parameters, "upgrade_name")?;

        log::info!("Granting upgrade '{}' to player {}", upgrade_name, player);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "grant_upgrade"
    }

    fn description(&self) -> &str {
        "Grants an upgrade to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "upgrade_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Enable science action
pub(super) struct EnableScienceAction;

#[async_trait]
impl ScriptAction for EnableScienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let science_name = get_string_param(parameters, "science_name")?;

        log::info!("Enabling science '{}' for player {}", science_name, player);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "enable_science"
    }

    fn description(&self) -> &str {
        "Enables a science/technology for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable science action
pub(super) struct DisableScienceAction;

#[async_trait]
impl ScriptAction for DisableScienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let science_name = get_string_param(parameters, "science_name")?;

        log::info!("Disabling science '{}' for player {}", science_name, player);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "disable_science"
    }

    fn description(&self) -> &str {
        "Disables a science/technology for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// ============================================================================
// PLAYER ACTIONS (10 critical actions)
// ============================================================================

/// Grant tech to player
pub(super) struct PlayerGrantScienceAction;

#[async_trait]
impl ScriptAction for PlayerGrantScienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let science = get_string_param(parameters, "science")?;

        log::info!("Granting science '{}' to player {}", science, player);

        // Integration with science/tech system:
        // Sciences unlock new units, abilities, upgrades
        // 1. Player *pPlayer = ThePlayerList->getPlayer(player)
        // 2. Science *scienceTemplate = TheScienceStore->findScience(science)
        // 3. pPlayer->grantScience(scienceTemplate)
        // 4. Triggers buildability updates, UI changes
        // 5. May enable special powers, units, or upgrades
        // Rust: player.grant_science(science_id)

        use game_engine::common::rts::science::{SCIENCE_INVALID, get_science_store};

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science)
        } else {
            log::warn!("PlayerGrantScienceAction: science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("PlayerGrantScienceAction: science '{}' not found", science);
            return Ok(ScriptResult::Success(None));
        }

        if let Ok(list) = player_list().read() {
            let index = player as i32;
            if let Some(player_arc) = list.get_player(index) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.grant_science(science_type);
                }
            } else {
                log::warn!("PlayerGrantScienceAction: player {} not found", player);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "player_grant_science"
    }

    fn description(&self) -> &str {
        "Grants a science/technology to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Give Special Power Action - Matches C++ special power grant logic
pub(super) struct GiveSpecialPowerAction;

#[async_trait]
impl ScriptAction for GiveSpecialPowerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let power_name = get_string_param(parameters, "power_name")?;

        if player < 0 {
            return Err(GameLogicError::Configuration(format!(
                "player must be non-negative, got {player}"
            )));
        }

        log::info!(
            "Granting special power '{}' to player {}",
            power_name,
            player
        );

        let power_template =
            find_or_create_special_power_template(&AsciiString::from(power_name.as_str()));
        let player_arc = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?
            .get_player(player as PlayerIndex)
            .cloned();

        let Some(player_arc) = player_arc else {
            log::warn!(
                "Cannot grant special power '{}' to missing player {}",
                power_name,
                player
            );
            return Ok(ScriptResult::Success(None));
        };

        let ready_frame = TheGameLogic::get_frame();
        player_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?
            .express_special_power_ready_frame(&power_template, ready_frame);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "give_special_power"
    }

    fn description(&self) -> &str {
        "Grants a special power/ability to a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
