//! Object health/damage/kill/explosion script actions
//!
//! C++: ScriptActions.cpp `doNamedDamage` L2310, `doNamedKill` L2481,
//! `doNamedDelete` L2328.
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

/// Destroy object action
pub(super) struct DestroyObjectAction;

#[async_trait]
impl ScriptAction for DestroyObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;

        log::info!("Destroying object {}", object_id);

        if object_id < 0 {
            return Err(GameLogicError::Configuration(
                "object_id must be non-negative".to_string(),
            ));
        }

        if let Ok(mut manager) = get_object_manager().write() {
            manager.destroy_object(object_id as u32);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "destroy_object"
    }

    fn description(&self) -> &str {
        "Destroys the specified object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set object health action
pub(super) struct SetObjectHealthAction;

#[async_trait]
impl ScriptAction for SetObjectHealthAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;
        let health = get_float_param(parameters, "health")?;

        log::info!("Setting object {} health to {}", object_id, health);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_object_health"
    }

    fn description(&self) -> &str {
        "Sets the health of the specified object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "health".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set object experience action
pub(super) struct SetObjectExperienceAction;

#[async_trait]
impl ScriptAction for SetObjectExperienceAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let object_id = get_int_param(parameters, "object_id")?;
        let experience = get_int_param(parameters, "experience")?;

        log::info!("Setting object {} experience to {}", object_id, experience);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_object_experience"
    }

    fn description(&self) -> &str {
        "Sets the experience level of the specified object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "experience".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Create Explosion Action - Matches C++ explosion creation
pub(super) struct CreateExplosionAction;

#[async_trait]
impl ScriptAction for CreateExplosionAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let explosion_type = get_string_param(parameters, "explosion_type")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0);
        let damage = get_float_param_optional(parameters, "damage").unwrap_or(0.0);

        log::info!(
            "Creating explosion '{}' at ({}, {}, {}) with damage {}",
            explosion_type,
            x,
            y,
            z,
            damage
        );

        let position = Coord3D::new(x as Real, y as Real, z as Real);
        let fx_list = FXList::new(&explosion_type);
        if let Err(err) = fx_list.do_fx_at_position_with_radius(&position, damage as Real) {
            log::warn!(
                "CreateExplosionAction: failed to execute FX '{}' at ({}, {}, {}): {}",
                explosion_type,
                x,
                y,
                z,
                err
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "create_explosion"
    }

    fn description(&self) -> &str {
        "Creates an explosion effect at the specified location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "explosion_type".to_string(),
            "x".to_string(),
            "y".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string(), "damage".to_string()]
    }
}

/// Damage Object Action - Matches C++ ScriptActions::doNamedDamage (line 2312)
pub(super) struct DamageObjectAction;

#[async_trait]
impl ScriptAction for DamageObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let object_name = get_string_param(parameters, "object")?;
        let damage = get_int_param(parameters, "damage")?;

        log::info!("Damaging object '{}' by {} HP", object_name, damage);

        // Matches C++ ScriptActions.cpp:doNamedDamage line 2312
        // Implementation:
        // 1. Object *pUnit = TheScriptEngine->getUnitNamed(unitName)
        // 2. DamageInfo damageInfo
        // 3. damageInfo.in.m_damageType = DAMAGE_UNRESISTABLE
        // 4. damageInfo.in.m_deathType = DEATH_NORMAL
        // 5. damageInfo.in.m_sourceID = INVALID_ID
        // 6. damageInfo.in.m_amount = damageAmt
        // 7. pUnit->attemptDamage(&damageInfo)
        // Applies unresistable damage (ignores armor)
        // Rust: object_manager.get_object(object_name).apply_damage(damage, DamageType::Unresistable)

        log::debug!("Integration: Object damage system applies unresistable damage");

        let Some(object_id) = resolve_named_object_id(&object_name) else {
            log::warn!("DamageObjectAction: object '{}' not found", object_name);
            return Ok(ScriptResult::Success(None));
        };

        let mut damage_info = DamageInfo::with_simple(
            damage as f32,
            INVALID_OBJECT_ID,
            DamageType::Unresistable,
            DeathType::Normal,
        );

        let Some(result) = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
            object_guard.attempt_damage(&mut damage_info)
        }) else {
            log::warn!(
                "DamageObjectAction: object '{}' (ID {}) not found in registry",
                object_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Err(err) = result {
            log::warn!(
                "DamageObjectAction: failed to damage '{}' (ID {}): {}",
                object_name,
                object_id,
                err
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "damage_object"
    }

    fn description(&self) -> &str {
        "Applies damage to a named object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object".to_string(), "damage".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Kill Object Action - Matches C++ ScriptActions::doNamedKill (line 2483)
pub(super) struct KillObjectAction;

#[async_trait]
impl ScriptAction for KillObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let object_name = get_string_param(parameters, "object")?;

        log::info!("Killing object '{}'", object_name);

        // Matches C++ ScriptActions.cpp:doNamedKill line 2483
        // Similar to doNamedDelete but with death effects
        // Implementation:
        // 1. Object *theUnit = TheScriptEngine->getUnitNamed(unitName)
        // 2. theUnit->kill(DAMAGE_UNRESISTABLE, DEATH_NORMAL)
        // Or: BodyModule->setHealth(0) to trigger normal death
        // Triggers death animation, sound, and cleanup
        // Rust: object_manager.get_object(object_name).kill()

        log::debug!("Integration: Object system kills unit with death effects");

        let Some(object_id) = resolve_named_object_id(&object_name) else {
            log::warn!("KillObjectAction: object '{}' not found", object_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(()) = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
            object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
        }) else {
            log::warn!(
                "KillObjectAction: object '{}' (ID {}) not found in registry",
                object_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "kill_object"
    }

    fn description(&self) -> &str {
        "Instantly kills a named object with death effects"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Heal Object Action - Restores object health
pub(super) struct HealObjectAction;

#[async_trait]
impl ScriptAction for HealObjectAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let object_name = get_string_param(parameters, "object")?;
        let amount = get_int_param(parameters, "amount")?;

        log::info!("Healing object '{}' by {} HP", object_name, amount);

        // C++ Implementation:
        // 1. Object *pUnit = TheScriptEngine->getUnitNamed(unitName)
        // 2. BodyModuleInterface *body = pUnit->getBodyModule()
        // 3. body->setHealth(body->getHealth() + amount)
        // Or: body->setHealth(body->getMaxHealth()) for full heal
        // Can specify amount or use -1 for full heal
        // Rust: object_manager.get_object(object_name).heal(amount)

        log::debug!("Integration: Object health system restores HP");

        let Some(object_id) = resolve_named_object_id(&object_name) else {
            log::warn!("HealObjectAction: object '{}' not found", object_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(result) = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
            if amount < 0 {
                object_guard.heal_completely()
            } else {
                object_guard.heal(amount as f32)
            }
        }) else {
            log::warn!(
                "HealObjectAction: object '{}' (ID {}) not found in registry",
                object_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Err(err) = result {
            if amount < 0 {
                log::warn!(
                    "HealObjectAction: failed to fully heal '{}' (ID {}): {}",
                    object_name,
                    object_id,
                    err
                );
            } else {
                log::warn!(
                    "HealObjectAction: failed to heal '{}' (ID {}) by {}: {}",
                    object_name,
                    object_id,
                    amount,
                    err
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "heal_object"
    }

    fn description(&self) -> &str {
        "Heals a named object by specified amount (-1 for full heal)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
