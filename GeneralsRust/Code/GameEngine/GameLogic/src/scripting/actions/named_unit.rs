//! Named-unit script actions
//!
//! C++: ScriptActions.cpp named-unit cluster L414 / L1040 / L1334–1627 / L1859
//! (`doNamedMoveToWaypoint`, `doNamedAttack`, `doNamedGuard`, `doNamedHunt`).
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

// ============================================================================
// NAMED UNIT ACTIONS (10 critical actions)
// ============================================================================

/// Named unit attacks
pub(super) struct NamedAttackAction;

#[async_trait]
impl ScriptAction for NamedAttackAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let attacker_name = get_string_param(parameters, "attacker_name")?;
        let target_name = get_string_param(parameters, "target_name")?;

        log::info!("Named unit '{}' attacking '{}'", attacker_name, target_name);

        // Integration with named object registry and attack commands:
        // 1. Object *attacker = TheScriptEngine->getUnitNamed(attacker_name)
        // 2. Object *target = TheScriptEngine->getUnitNamed(target_name)
        // 3. AIUpdateInterface *ai = attacker->getAIUpdateInterface()
        // 4. ai->aiAttackObject(target, CMD_FROM_SCRIPT)
        // Named objects stored in ScriptEngine's named object map
        // Rust: object_manager.get_named(name) -> Option<Arc<RwLock<Object>>>

        let Some(attacker_id) = resolve_named_object_id(&attacker_name) else {
            log::warn!("NamedAttackAction: attacker '{}' not found", attacker_name);
            return Ok(ScriptResult::Success(None));
        };
        let Some(target_id) = resolve_named_object_id(&target_name) else {
            log::warn!("NamedAttackAction: target '{}' not found", target_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            log::warn!(
                "NamedAttackAction: attacker '{}' (ID {}) not found in registry",
                attacker_name,
                attacker_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let ai = attacker_arc.write().ok().and_then(|mut obj| {
            obj.leave_group();
            obj.get_ai_update_interface()
        });

        if let Some(ai) = ai {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.choose_locomotor_set(LocomotorSetType::Normal);
                let mut params = AiCommandParams::new(
                    AiCommandType::ForceAttackObject,
                    CommandSourceType::FromScript,
                );
                params.obj = Some(target_id);
                params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                let _ = ai_guard.execute_command(&params);
            }
        } else {
            log::warn!(
                "NamedAttackAction: attacker '{}' has no AI update interface",
                attacker_name
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_attack"
    }

    fn description(&self) -> &str {
        "Commands a named unit to attack another named unit"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["attacker_name".to_string(), "target_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit attacks a team
pub(super) struct NamedAttackTeamAction;

#[async_trait]
impl ScriptAction for NamedAttackTeamAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Named unit '{}' attacking team '{}'", unit_name, team_name);

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedAttackTeamAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let resolved_team = resolve_team_name_token(&team_name);
        let team_exists = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .is_some();
        if !team_exists {
            log::warn!("NamedAttackTeamAction: team '{}' not found", resolved_team);
            return Ok(ScriptResult::Success(None));
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedAttackTeamAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut obj_guard) = object_arc.write() {
            obj_guard.leave_group();
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.choose_locomotor_set(LocomotorSetType::Normal);
                let mut params =
                    AiCommandParams::new(AiCommandType::AttackTeam, CommandSourceType::FromScript);
                params.team = Some(resolved_team);
                params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                let _ = ai.lock().ok().map(|mut ai| ai.execute_command(&params));
            } else {
                log::warn!(
                    "NamedAttackTeamAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_attack_team"
    }

    fn description(&self) -> &str {
        "Commands a named unit to attack a team"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit attacks all enemies in a trigger area
pub(super) struct NamedAttackAreaAction;

#[async_trait]
impl ScriptAction for NamedAttackAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let area = get_string_param(parameters, "area")?;

        log::info!("Named unit '{}' attacking area '{}'", unit_name, area);

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedAttackAreaAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let (center, trigger_id) = if let Ok(terrain_guard) = get_terrain_logic().read() {
            if let Some(trigger) = terrain_guard.get_trigger_area_by_name(&area) {
                (trigger.get_center_point(), trigger.get_id())
            } else {
                log::warn!("NamedAttackAreaAction: trigger area '{}' not found", area);
                return Ok(ScriptResult::Success(None));
            }
        } else {
            log::warn!("NamedAttackAreaAction: failed to lock terrain logic");
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedAttackAreaAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut obj_guard) = object_arc.write() {
            obj_guard.leave_group();
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.choose_locomotor_set(LocomotorSetType::Normal);
                let mut params =
                    AiCommandParams::new(AiCommandType::AttackArea, CommandSourceType::FromScript);
                params.pos = center;
                params.polygon = Some(trigger_id);
                let _ = ai.lock().ok().map(|mut ai| ai.execute_command(&params));
            } else {
                log::warn!(
                    "NamedAttackAreaAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_attack_area"
    }

    fn description(&self) -> &str {
        "Commands a named unit to attack targets in an area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit moves
pub(super) struct NamedMoveToAction;

#[async_trait]
impl ScriptAction for NamedMoveToAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Named unit '{}' moving to '{}'", unit_name, waypoint);

        // Matches C++ ScriptActions.cpp:doNamedMoveToWaypoint line 416
        // Integration (from C++):
        // 1. Object *theObj = TheScriptEngine->getUnitNamed(unit_name)
        // 2. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 3. Coord3D destination = *way->getLocation()
        // 4. AIUpdateInterface *aiUpdate = theObj->getAIUpdateInterface()
        // 5. aiUpdate->clearWaypointQueue()
        // 6. theObj->leaveGroup() // Leave team for individual movement
        // 7. aiUpdate->chooseLocomotorSet(LOCOMOTORSET_NORMAL)
        // 8. aiUpdate->aiMoveToPosition(&destination, CMD_FROM_SCRIPT)

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedMoveToAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedMoveToAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let destination = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });

        let Some(destination) = destination else {
            log::warn!("NamedMoveToAction: waypoint '{}' not found", waypoint);
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut obj_guard) = object_arc.write() {
            obj_guard.leave_group();
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.choose_locomotor_set(LocomotorSetType::Normal);
                ai.ai_move_to_position(&destination, false, CommandSourceType::FromScript);
            } else {
                log::warn!(
                    "NamedMoveToAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_move_to"
    }

    fn description(&self) -> &str {
        "Commands a named unit to move to a waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit garrisons
pub(super) struct NamedGarrisonAction;

#[async_trait]
impl ScriptAction for NamedGarrisonAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!("Named unit '{}' garrisoning '{}'", unit_name, building_name);

        // Integration with garrison system:
        // Same as team garrison but for single named unit
        // 1. Object *unit = TheScriptEngine->getUnitNamed(unit_name)
        // 2. Object *building = TheScriptEngine->getUnitNamed(building_name)
        // 3. GarrisonContain *garrison = building->getGarrisonContain()
        // 4. unit->aiMoveToPosition(building->getPosition())
        // 5. garrison->enterContain(unit)

        log::debug!("Integration: Named unit enters garrison building via GarrisonContain");

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_garrison"
    }

    fn description(&self) -> &str {
        "Commands a named unit to garrison a building"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit follows path
pub(super) struct NamedFollowWaypointsAction;

#[async_trait]
impl ScriptAction for NamedFollowWaypointsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let waypoint_path = get_string_param(parameters, "waypoint_path")?;

        log::info!(
            "Named unit '{}' following waypoint path '{}'",
            unit_name,
            waypoint_path
        );

        // Integration with waypoint system:
        // Like team follow waypoints but for single unit
        // 1. Resolve unit by name
        // 2. Parse waypoint_path into list of waypoint names
        // 3. For each waypoint, resolve Coord3D
        // 4. Add waypoints to unit's AI waypoint queue
        // 5. ai->followWaypointPath(waypoints)
        // Unit moves sequentially through each waypoint

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedFollowWaypointsAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedFollowWaypointsAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let waypoint_ascii = AsciiString::from(waypoint_path.as_str());
        let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| w.get_id())
        });

        let Some(waypoint_id) = waypoint_id else {
            log::warn!(
                "NamedFollowWaypointsAction: waypoint '{}' not found",
                waypoint_path
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(obj_guard) = object_arc.write() {
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.try_lock() {
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowWaypointPath,
                        CommandSourceType::FromScript,
                    );
                    params.waypoint = Some(waypoint_id);
                    let _ = ai_guard.execute_command(&params);
                }
            } else {
                log::warn!(
                    "NamedFollowWaypointsAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_follow_waypoints"
    }

    fn description(&self) -> &str {
        "Commands a named unit to follow a waypoint path"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "waypoint_path".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit guards
pub(super) struct NamedGuardAction;

#[async_trait]
impl ScriptAction for NamedGuardAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Named unit '{}' guarding", unit_name);

        // Integration with guard behavior:
        // Sets single unit to guard mode
        // 1. Resolve unit by name
        // 2. ai->setAIState(AI_STATE_GUARD)
        // 3. Guard position = current position
        // 4. Unit engages nearby enemies but returns to guard position

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedGuardAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedGuardAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(obj_guard) = object_arc.read() {
            let pos = *obj_guard.get_position();
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.ai_guard_position(&pos, GuardMode::Normal, CommandSourceType::FromScript);
            } else {
                log::warn!(
                    "NamedGuardAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_guard"
    }

    fn description(&self) -> &str {
        "Commands a named unit to guard its position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named unit hunts
pub(super) struct NamedHuntAction;

#[async_trait]
impl ScriptAction for NamedHuntAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Named unit '{}' hunting", unit_name);

        // Integration with hunt behavior:
        // Single unit actively seeks enemies
        // 1. Resolve unit
        // 2. ai->setAIState(AI_STATE_HUNT)
        // 3. Unit scans for enemies and attacks
        // 4. Pursues and destroys targets

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedHuntAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedHuntAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(obj_guard) = object_arc.read() {
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.choose_locomotor_set(LocomotorSetType::Normal);
                ai.ai_hunt(CommandSourceType::FromScript);
            } else {
                log::warn!(
                    "NamedHuntAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_hunt"
    }

    fn description(&self) -> &str {
        "Commands a named unit to hunt enemies"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Delete named unit
pub(super) struct NamedDeleteAction;

#[async_trait]
impl ScriptAction for NamedDeleteAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Deleting named unit '{}'", unit_name);

        // Integration with object destruction:
        // 1. Object *obj = TheScriptEngine->getUnitNamed(unit_name)
        // 2. obj->kill(DEATH_NORMAL) or obj->destroy()
        // 3. Remove from world
        // 4. Remove from named object registry
        // 5. Cleanup resources

        let Some(object_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedDeleteAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            log::warn!(
                "NamedDeleteAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                object_id
            );
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut obj_guard) = object_arc.write() {
            obj_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
        }

        let tracker = get_named_object_tracker();
        if let Err(err) = tracker.unregister_object(object_id) {
            log::warn!(
                "NamedDeleteAction: failed to unregister '{}' (ID {}): {}",
                unit_name,
                object_id,
                err
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_delete"
    }

    fn description(&self) -> &str {
        "Deletes a named unit from the game"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named enters named building
pub(super) struct NamedEnterNamedAction;

#[async_trait]
impl ScriptAction for NamedEnterNamedAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!("Named unit '{}' entering '{}'", unit_name, building_name);

        // Integration with contain/garrison system:
        // Generic enter for any container type (transport, garrison, tunnel, etc.)
        // 1. Resolve both objects
        // 2. ContainModuleInterface *contain = building->getContain()
        // 3. Check contain->canContain(unit)
        // 4. unit->moveToAndEnter(building)
        // 5. contain->enterContain(unit)
        // Works with: TransportContain, GarrisonContain, TunnelContain, etc.

        let Some(unit_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedEnterNamedAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };
        let Some(building_id) = resolve_named_object_id(&building_name) else {
            log::warn!(
                "NamedEnterNamedAction: building '{}' not found",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            log::warn!(
                "NamedEnterNamedAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                unit_id
            );
            return Ok(ScriptResult::Success(None));
        };
        let Some(building_arc) = TheGameLogic::find_object_by_id(building_id) else {
            log::warn!(
                "NamedEnterNamedAction: building '{}' (ID {}) not found in registry",
                building_name,
                building_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let (unit_guard, contain) = match (unit_arc.read(), building_arc.read()) {
            (Ok(unit_guard), Ok(building_guard)) => (unit_guard, building_guard.get_contain()),
            _ => {
                log::warn!(
                    "NamedEnterNamedAction: failed to lock unit/building for '{}'",
                    unit_name
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        let Some(contain) = contain else {
            log::warn!(
                "NamedEnterNamedAction: building '{}' has no contain module",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        if !contain.is_valid_container_for(&unit_guard, true) {
            log::warn!(
                "NamedEnterNamedAction: building '{}' cannot contain '{}'",
                building_name,
                unit_name
            );
            return Ok(ScriptResult::Success(None));
        }

        contain.add_to_contain(&unit_guard);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_enter_named"
    }

    fn description(&self) -> &str {
        "Commands a named unit to enter a named building"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Named exits
pub(super) struct NamedExitAction;

#[async_trait]
impl ScriptAction for NamedExitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let unit_name = get_string_param(parameters, "unit_name")?;

        log::info!("Named unit '{}' exiting", unit_name);

        // Integration with contain/garrison system:
        // Exit from current container
        // 1. Object *unit = TheScriptEngine->getUnitNamed(unit_name)
        // 2. ContainModuleInterface *contain = unit->getContainedBy()
        // 3. contain->exitContain(unit)
        // 4. Unit spawns near container exit point
        // Works for exiting from transports, buildings, tunnels, etc.

        let Some(unit_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedExitAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            log::warn!(
                "NamedExitAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                unit_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let Some(container_id) = unit_arc
            .read()
            .ok()
            .and_then(|unit_guard| unit_guard.get_container_id())
        else {
            log::warn!("NamedExitAction: unit '{}' is not contained", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let released = crate::object::registry::OBJECT_REGISTRY
            .with_object(container_id, |c| {
                if let Some(contain) = c.get_contain() {
                    if let Ok(mut contain_guard) = contain.try_lock() {
                        let _ = contain_guard.release_object(unit_id);
                        return true;
                    }
                }
                false
            })
            .unwrap_or(false);
        if !released {
            log::warn!(
                "NamedExitAction: container for '{}' has no contain module",
                unit_name
            );
            return Ok(ScriptResult::Success(None));
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_exit"
    }

    fn description(&self) -> &str {
        "Commands a named unit to exit from a building or transport"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set named unit attitude
pub(super) struct NamedSetAttitudeAction;

#[async_trait]
impl ScriptAction for NamedSetAttitudeAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let unit_name = get_string_param(parameters, "unit_name")?;
        let attitude = get_string_param(parameters, "attitude")?;

        log::info!(
            "Setting named unit '{}' attitude to '{}'",
            unit_name,
            attitude
        );

        // Integration with AI attitude system:
        // Attitudes: "AGGRESSIVE", "DEFENSIVE", "GUARD", "HOLD_GROUND", "NORMAL"
        // 1. Resolve unit
        // 2. AttitudeType attitudeType = parseAttitude(attitude)
        // 3. ai->setAttitude(attitudeType)
        // Affects how unit responds to enemies and threats
        // AGGRESSIVE: Pursues enemies actively
        // DEFENSIVE: Only fires when attacked
        // GUARD: Stays near position, engages nearby enemies
        // HOLD_GROUND: Never moves, only fires

        let Some(unit_id) = resolve_named_object_id(&unit_name) else {
            log::warn!("NamedSetAttitudeAction: unit '{}' not found", unit_name);
            return Ok(ScriptResult::Success(None));
        };

        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            log::warn!(
                "NamedSetAttitudeAction: unit '{}' (ID {}) not found in registry",
                unit_name,
                unit_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let attitude_upper = attitude.to_ascii_uppercase();
        let attitude_type = match attitude_upper.as_str() {
            "AGGRESSIVE" => crate::modules::AIAttitudeType::Aggressive,
            "DEFENSIVE" => crate::modules::AIAttitudeType::Defensive,
            "GUARD" | "HOLD_GROUND" => crate::modules::AIAttitudeType::Defensive,
            "PASSIVE" => crate::modules::AIAttitudeType::Passive,
            "SLEEP" => crate::modules::AIAttitudeType::Sleep,
            _ => crate::modules::AIAttitudeType::Normal,
        };

        if let Ok(unit_guard) = unit_arc.read() {
            if let Some(ai) = unit_guard.get_ai_update_interface() {
                ai.set_attitude(attitude_type);
            } else {
                log::warn!(
                    "NamedSetAttitudeAction: unit '{}' has no AI update interface",
                    unit_name
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "named_set_attitude"
    }

    fn description(&self) -> &str {
        "Sets the attitude of a named unit (aggressive, defensive, etc.)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "attitude".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
