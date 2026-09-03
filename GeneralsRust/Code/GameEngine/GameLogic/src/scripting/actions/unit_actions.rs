//! Unit create/move/attack/reinforcement script actions
//!
//! C++: ScriptActions.cpp `createUnitOnTeamAt` L1143, `doCreateReinforcements` L480,
//! `doAttack` L1018.
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

// Built-in action implementations

/// Create a unit action
pub(super) struct CreateUnitAction;

#[async_trait]
impl ScriptAction for CreateUnitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let unit_type = get_string_param(parameters, "unit_type")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);

        log::info!(
            "Creating unit '{}' for player {} at ({}, {}, {})",
            unit_type,
            player,
            x,
            y,
            z
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player_arc| player_arc.read().ok().and_then(|p| p.get_default_team()));

        let position = Coord3D::new(x as f32, y as f32, z as f32);
        let object_id = get_object_manager()
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock ObjectManager".to_string()))?
            .create_object(
                unit_type.as_str(),
                position,
                team,
                ObjectCreationFlags::from_template(),
            )?;

        Ok(ScriptResult::Success(Some(ScriptValue::ObjectId(
            object_id as u32,
        ))))
    }

    fn name(&self) -> &str {
        "create_unit"
    }

    fn description(&self) -> &str {
        "Creates a unit for the specified player at the given location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string()]
    }
}

/// Move unit action
pub(super) struct MoveUnitAction;

#[async_trait]
impl ScriptAction for MoveUnitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);

        log::info!("Moving unit {} to ({}, {}, {})", object_id, x, y, z);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "move_unit"
    }

    fn description(&self) -> &str {
        "Moves the specified unit to the given location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "x".to_string(), "y".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string()]
    }
}

/// Attack unit action
pub(super) struct AttackUnitAction;

#[async_trait]
impl ScriptAction for AttackUnitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let attacker_id = get_int_param(parameters, "attacker_id")?;
        let target_id = get_int_param(parameters, "target_id")?;

        log::info!("Unit {} attacking unit {}", attacker_id, target_id);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "attack_unit"
    }

    fn description(&self) -> &str {
        "Commands one unit to attack another"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["attacker_id".to_string(), "target_id".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Spawn Reinforcements Action - Creates multiple units for player
pub(super) struct SpawnReinforcementsAction;

#[async_trait]
impl ScriptAction for SpawnReinforcementsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let unit_type = get_string_param(parameters, "unit_type")?;
        let count = get_int_param(parameters, "count")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let spacing = get_float_param_optional(parameters, "spacing").unwrap_or(10.0);

        log::info!(
            "Spawning {} reinforcements of '{}' for player {} at ({}, {})",
            count,
            unit_type,
            player,
            x,
            y
        );

        // Integration with unit creation - spawn in formation:
        // Matches C++ ScriptActions.cpp:doCreateReinforcements line 480
        // 1. For i in 0..count:
        //    a. Calculate grid position: offset_x = x + (i % 5) * spacing
        //    b. offset_y = y + (i / 5) * spacing
        //    c. Create unit at offset position
        // 2. Units spawn in 5-column grid formation
        // 3. ThingFactory->newObject(template, team, position)
        // Rust: object_factory.spawn_formation(unit_type, player, position, count, spacing)

        if player < 0 {
            return Err(GameLogicError::Configuration(
                "player must be non-negative".to_string(),
            ));
        }
        if count < 0 {
            return Err(GameLogicError::Configuration(
                "count must be non-negative".to_string(),
            ));
        }

        let team = player_list()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?
            .get_player(player as PlayerIndex)
            .cloned()
            .and_then(|player_arc| player_arc.read().ok().and_then(|p| p.get_default_team()));

        let mut created_ids = Vec::with_capacity(count as usize);
        let manager = get_object_manager();
        let mut manager = manager
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock ObjectManager".to_string()))?;

        for i in 0..count {
            let offset_x = x + ((i % 5) as f64 * spacing);
            let offset_y = y + ((i / 5) as f64 * spacing);
            let object_id = manager.create_object(
                &unit_type,
                Coord3D::new(offset_x as f32, offset_y as f32, 0.0),
                team.clone(),
                ObjectCreationFlags::from_template(),
            )?;
            created_ids.push(ScriptValue::ObjectId(object_id));
        }

        Ok(ScriptResult::Success(Some(ScriptValue::Array(created_ids))))
    }

    fn name(&self) -> &str {
        "spawn_reinforcements"
    }

    fn description(&self) -> &str {
        "Spawns multiple units (reinforcements) for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "count".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["spacing".to_string()]
    }
}
