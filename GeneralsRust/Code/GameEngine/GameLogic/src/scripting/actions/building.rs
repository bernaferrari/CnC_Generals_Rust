//! Building create/destroy script actions
//!
//! C++: ScriptActions.cpp `doCreateObject` L952, `doBuildBuilding` L1070.
//!
//! Split from `scripting/actions.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::ScriptAction;
use super::helpers::*;
use crate::action_manager::TheActionManager;
use crate::ai::integration::with_ai_integration_mut;
use crate::ai::{AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, GuardMode, THE_AI};
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

/// Create Building Action - Matches C++ ScriptActionType::CREATE_OBJECT (for structures)
pub(super) struct CreateBuildingAction;

#[async_trait]
impl ScriptAction for CreateBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let building_type = get_string_param(parameters, "building_type")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);
        let angle = get_float_param_optional(parameters, "angle").unwrap_or(0.0);

        log::info!(
            "Creating building '{}' for player {} at ({}, {}, {}) angle {}",
            building_type,
            player,
            x,
            y,
            z,
            angle
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
                building_type.as_str(),
                position,
                team,
                ObjectCreationFlags::from_template(),
            )?;

        if angle != 0.0 {
            if let Ok(manager) = get_object_manager().read() {
                if let Some(object) = manager.get_object(object_id) {
                    if let Ok(guard) = object.write() {
                        let _ = guard
                            .base()
                            .write()
                            .map(|mut base| base.set_orientation(angle as f32));
                    }
                }
            }
        }

        Ok(ScriptResult::Success(Some(ScriptValue::ObjectId(
            object_id as u32,
        ))))
    }

    fn name(&self) -> &str {
        "create_building"
    }

    fn description(&self) -> &str {
        "Creates a building/structure at the specified location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "building_type".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string(), "angle".to_string()]
    }
}

/// Destroy Building Action - Matches C++ destroy/kill object logic
pub(super) struct DestroyBuildingAction;

#[async_trait]
impl ScriptAction for DestroyBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;

        log::info!("Destroying building {}", object_id);

        // Integration with object system:
        // 1. Object *obj = TheObjectList->getObject(object_id)
        // 2. obj->kill(DEATH_NORMAL) // Normal death with effects
        // 3. Triggers death animations, explosions
        // 4. Removes from world
        // 5. Frees resources
        // Rust: object_manager.destroy_object(object_id, DeathType::Normal)

        if object_id < 0 {
            return Err(GameLogicError::Configuration(
                "object_id must be non-negative".to_string(),
            ));
        }

        get_object_manager()
            .write()
            .map_err(|_| GameLogicError::Threading("Failed to lock ObjectManager".to_string()))?
            .destroy_object(object_id as u32);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "destroy_building"
    }

    fn description(&self) -> &str {
        "Destroys/removes a building from the game"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
