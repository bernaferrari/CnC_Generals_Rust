//! Team command and movement script actions
//!
//! C++: ScriptActions.cpp team cluster L389–2161 / L2341–2495
//! (`doTeamFollowWaypoints`, `doTeamGuard*`, `doTeamHunt`, `doTeamAttackArea`).
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

// ============================================================================
// TEAM ACTIONS (15 critical actions)
// ============================================================================

/// Team attacks another team
pub(super) struct TeamAttackTeamAction;

#[async_trait]
impl ScriptAction for TeamAttackTeamAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let attacker_team = get_string_param(parameters, "attacker_team")?;
        let target_team = get_string_param(parameters, "target_team")?;

        log::info!("Team '{}' attacking team '{}'", attacker_team, target_team);

        // Matches C++ ScriptActions.cpp - team attack command
        // Get both teams from the team system
        // For each member of attacker team, issue attack command targeting target team
        // In C++: theTeam->attackTeam(targetTeam)
        // Integration: Requires team system to resolve team names and issue AI group attack commands

        let resolved_attacker = resolve_team_name_token(&attacker_team);
        let resolved_target = resolve_team_name_token(&target_team);

        let group_arc = match create_ai_group_from_team(&resolved_attacker) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamAttackTeamAction: failed to create AI group for team '{}': {}",
                    resolved_attacker,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackTeam, CommandSourceType::FromScript);
            params.team = Some(resolved_target);
            params.int_value = -1; // NO_MAX_SHOTS_LIMIT
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_attack_team"
    }

    fn description(&self) -> &str {
        "Commands one team to attack another team"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["attacker_team".to_string(), "target_team".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team follows waypoint path
pub(super) struct TeamFollowWaypointsAction;

#[async_trait]
impl ScriptAction for TeamFollowWaypointsAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let waypoint_path = get_string_param(parameters, "waypoint_path")?;
        let as_team = parameters
            .get("as_team")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        log::info!(
            "Team '{}' following waypoint path '{}' (as_team: {})",
            team_name,
            waypoint_path,
            as_team
        );

        // Matches C++ ScriptActions.cpp:doTeamFollowWaypoints
        // Integration steps:
        // 1. Resolve team by name from team system
        // 2. Parse waypoint_path (comma-separated waypoint names or waypoint list name)
        // 3. For each waypoint, resolve Coord3D position from waypoint system
        // 4. Create AI group from team members
        // 5. Issue sequential movement commands through waypoint queue
        // 6. If as_team=true, maintain formation; if false, units move independently
        // In C++: theTeam->getTeamAsAIGroup() then aiGroup->groupFollowWaypoints(waypoints, asTeam)

        let waypoint_ascii = AsciiString::from(waypoint_path.as_str());
        let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| w.get_id())
        });

        let Some(waypoint_id) = waypoint_id else {
            log::warn!(
                "TeamFollowWaypointsAction: waypoint '{}' not found",
                waypoint_path
            );
            return Ok(ScriptResult::Success(None));
        };

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamFollowWaypointsAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let cmd = if as_team {
                AiCommandType::FollowWaypointPathAsTeam
            } else {
                AiCommandType::FollowWaypointPath
            };
            let mut params = AiCommandParams::new(cmd, CommandSourceType::FromScript);
            params.waypoint = Some(waypoint_id);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_follow_waypoints"
    }

    fn description(&self) -> &str {
        "Commands a team to follow a waypoint path"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "waypoint_path".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["as_team".to_string()]
    }
}

/// Team guards position
pub(super) struct TeamGuardAction;

#[async_trait]
impl ScriptAction for TeamGuardAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let x = get_float_param_optional(parameters, "x");
        let y = get_float_param_optional(parameters, "y");

        if let (Some(x_pos), Some(y_pos)) = (x, y) {
            log::info!(
                "Team '{}' guarding position ({}, {})",
                team_name,
                x_pos,
                y_pos
            );
        } else {
            log::info!("Team '{}' guarding current position", team_name);
        }

        let resolved_team = resolve_team_name_token(&team_name);
        let guard_pos = if let (Some(x_pos), Some(y_pos)) = (x, y) {
            let z = get_terrain_logic()
                .read()
                .ok()
                .map(|terrain| terrain.get_ground_height(x_pos as f32, y_pos as f32, None))
                .unwrap_or(0.0);
            Coord3D::new(x_pos as f32, y_pos as f32, z)
        } else {
            let Some(team_arc) = get_team_factory()
                .lock()
                .ok()
                .and_then(|mut guard| guard.find_team(&resolved_team))
            else {
                log::warn!("TeamGuardAction: team '{}' not found", resolved_team);
                return Ok(ScriptResult::Success(None));
            };

            let members = team_arc
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to read Team".to_string()))?
                .get_members()
                .to_vec();
            if members.is_empty() {
                log::warn!("TeamGuardAction: team '{}' has no members", resolved_team);
                return Ok(ScriptResult::Success(None));
            }

            let mut guarded_count = 0usize;
            for member_id in members {
                if let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) {
                    if let Ok(obj_guard) = obj_arc.read() {
                        let pos = *obj_guard.get_position();
                        if let Some(ai) = obj_guard.get_ai_update_interface() {
                            ai.ai_guard_position(
                                &pos,
                                GuardMode::Normal,
                                CommandSourceType::FromScript,
                            );
                            guarded_count += 1;
                        }
                    }
                }
            }

            if guarded_count == 0 {
                log::warn!(
                    "TeamGuardAction: team '{}' has no valid AI members",
                    resolved_team
                );
            }

            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamGuardAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardPosition, CommandSourceType::FromScript);
            params.pos = guard_pos;
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_guard"
    }

    fn description(&self) -> &str {
        "Commands a team to guard a position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string()]
    }
}

/// Team hunts enemies
pub(super) struct TeamHuntAction;

#[async_trait]
impl ScriptAction for TeamHuntAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' hunting enemies", team_name);

        // Matches C++ ScriptActions.cpp - hunt AI behavior
        // Integration: Set AI state to HUNT mode
        // In C++: theTeam->setHunt() - actively seeks and destroys enemy units
        // Hunt behavior: scan for enemies, prioritize targets, pursue and destroy
        // Different from guard - doesn't return to position, keeps hunting
        // Uses CommandButtonHuntUpdate module behavior

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamHuntAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Hunt, CommandSourceType::FromScript);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_hunt"
    }

    fn description(&self) -> &str {
        "Commands a team to hunt for enemies"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team moves to waypoint
pub(super) struct TeamMoveToWaypointAction;

#[async_trait]
impl ScriptAction for TeamMoveToWaypointAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Team '{}' moving to waypoint '{}'", team_name, waypoint);

        // Matches C++ ScriptActions.cpp:doMoveToWaypoint line 391
        // Integration steps (from C++):
        // 1. Team *theTeam = TheScriptEngine->getTeamNamed(team)
        // 2. AIGroup* theGroup = TheAI->createGroup()
        // 3. theTeam->getTeamAsAIGroup(theGroup)
        // 4. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 5. Coord3D destination = *way->getLocation()
        // 6. theGroup->groupMoveToPosition(&destination, false, CMD_FROM_SCRIPT)
        // Rust: Resolve team -> get waypoint coordinates -> issue group movement command

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let position = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });

        let Some(position) = position else {
            log::warn!(
                "TeamMoveToWaypointAction: waypoint '{}' not found",
                waypoint
            );
            return Ok(ScriptResult::Success(None));
        };

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamMoveToWaypointAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromScript);
            params.pos = position;
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_move_to_waypoint"
    }

    fn description(&self) -> &str {
        "Commands a team to move to a specific waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team enters building
pub(super) struct TeamGarrisonBuildingAction;

#[async_trait]
impl ScriptAction for TeamGarrisonBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!(
            "Team '{}' garrisoning building '{}'",
            team_name,
            building_name
        );

        // Matches C++ ScriptActions.cpp:doTeamGarrisonSpecificBuilding line 3291
        // Integration with garrison/contain system:
        // 1. Resolve team by name
        // 2. Find building object by name
        // 3. Get GarrisonContain module from building
        // 4. For each team member:
        //    a. Check if unit can garrison (has KINDOF_CAN_ATTACK_GARRISONED flag)
        //    b. Move unit to building position
        //    c. Call contain->enterContain(unit)
        //    d. Unit becomes contained, gains garrison bonuses
        // 5. Building shows garrison indicators, units can fire from inside
        // Uses: GarrisonContain from object/contain/garrison_contain.rs

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(building_id) = resolve_named_object_id(&building_name) else {
            log::warn!(
                "TeamGarrisonBuildingAction: building '{}' not found",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        let Some(building_arc) = TheGameLogic::find_object_by_id(building_id) else {
            log::warn!(
                "TeamGarrisonBuildingAction: building '{}' (ID {}) not found in registry",
                building_name,
                building_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamGarrisonBuildingAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let contain = building_arc.read().ok().and_then(|b| b.get_contain());

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(unit_guard) = unit_arc.read() else {
                continue;
            };
            if let Some(ai) = unit_guard.get_ai_update_interface() {
                ai.ai_enter(building_id, CommandSourceType::FromScript);
                continue;
            }

            if let Some(contain) = &contain {
                if contain.is_valid_container_for(&unit_guard, true) {
                    contain.add_to_contain(&unit_guard);
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_garrison_building"
    }

    fn description(&self) -> &str {
        "Commands a team to garrison a specific building"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team exits garrison
pub(super) struct TeamExitBuildingAction;

#[async_trait]
impl ScriptAction for TeamExitBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' exiting all buildings", team_name);

        // Matches C++ ScriptActions.cpp:doTeamEvacuateBuilding
        // Integration with garrison system:
        // 1. Resolve team by name
        // 2. For each team member currently contained:
        //    a. Get containing building's GarrisonContain module
        //    b. Call contain->exitContain(unit) or contain->evacuate()
        //    c. Unit spawns near building exit points
        //    d. Unit returns to team control
        // 3. Team members reform after exiting
        // Uses: GarrisonContain::evacuate() from object/contain/garrison_contain.rs

        let resolved_team = resolve_team_name_token(&team_name);
        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamExitBuildingAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Some(container_id) = unit_arc
                .read()
                .ok()
                .and_then(|unit_guard| unit_guard.get_container_id())
            else {
                continue;
            };
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(container_id, |c| {
                if let Some(contain) = c.get_contain() {
                    if let Ok(mut contain_guard) = contain.try_lock() {
                        let _ = contain_guard.release_object(member_id);
                    }
                }
            });
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_exit_building"
    }

    fn description(&self) -> &str {
        "Commands a team to exit from garrisoned buildings"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team captures structure
pub(super) struct TeamCaptureBuildingAction;

#[async_trait]
impl ScriptAction for TeamCaptureBuildingAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let building_name = get_string_param(parameters, "building_name")?;

        log::info!(
            "Team '{}' capturing building '{}'",
            team_name,
            building_name
        );

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(building_id) = resolve_named_object_id(&building_name) else {
            log::warn!(
                "TeamCaptureBuildingAction: building '{}' not found",
                building_name
            );
            return Ok(ScriptResult::Success(None));
        };

        let Some(building_arc) = TheGameLogic::find_object_by_id(building_id) else {
            log::warn!(
                "TeamCaptureBuildingAction: building '{}' (ID {}) not found in registry",
                building_name,
                building_id
            );
            return Ok(ScriptResult::Success(None));
        };

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamCaptureBuildingAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let mut issued = 0;
        if let Ok(mut factory) = get_object_factory().write() {
            for member_id in members {
                let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(unit_guard) = unit_arc.read() else {
                    continue;
                };
                let Ok(building_guard) = building_arc.read() else {
                    continue;
                };
                if !TheActionManager::can_capture_building(
                    &unit_guard,
                    &building_guard,
                    CommandSourceType::FromScript,
                ) {
                    continue;
                }

                let Some(GameObjectInstance::Unit(unit)) = factory.get_object_mut(member_id) else {
                    continue;
                };

                let _ = unit.give_capture_order(building_id, false);
                issued += 1;
            }
        }

        if issued == 0 {
            log::warn!(
                "TeamCaptureBuildingAction: team '{}' has no capture-capable units for '{}'",
                resolved_team,
                building_name
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_capture_building"
    }

    fn description(&self) -> &str {
        "Commands a team to capture a neutral or enemy structure"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "building_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team repairs target
pub(super) struct TeamRepairAction;

#[async_trait]
impl ScriptAction for TeamRepairAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let target_name = get_string_param(parameters, "target_name")?;

        log::info!("Team '{}' repairing target '{}'", team_name, target_name);

        // Integration with repair mechanics:
        // 1. Resolve team (must contain repair-capable units like Dozers, Workers)
        // 2. Resolve target object by name
        // 3. Check team has repair-capable units (KINDOF_CAN_REPAIR)
        // 4. Move repair units to target
        // 5. Initiate repair: gradually restore target health over time
        // 6. Repair cost: consumes player resources proportional to damage
        // In C++: Uses SpecialAbilityUpdate::Repair or RepairModule

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(target_id) = resolve_named_object_id(&target_name) else {
            log::warn!("TeamRepairAction: target '{}' not found", target_name);
            return Ok(ScriptResult::Success(None));
        };

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!("TeamRepairAction: team '{}' has no members", resolved_team);
            return Ok(ScriptResult::Success(None));
        }

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(unit_guard) = unit_arc.read() else {
                continue;
            };
            if !unit_guard.is_kind_of(crate::common::KindOf::CanRepair) {
                continue;
            }
            let ai = unit_guard.get_ai_update_interface();
            drop(unit_guard);
            let Some(ai) = ai else {
                continue;
            };
            let Ok(mut ai_guard) = ai.lock() else {
                continue;
            };
            let mut params =
                AiCommandParams::new(AiCommandType::Repair, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            let _ = ai_guard.execute_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_repair"
    }

    fn description(&self) -> &str {
        "Commands a team to repair a target structure or vehicle"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "target_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team wanders area
pub(super) struct TeamWanderAction;

#[async_trait]
impl ScriptAction for TeamWanderAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' wandering", team_name);

        // Integration with AI wander behavior:
        // Wander makes units move semi-randomly within an area
        // 1. Resolve team
        // 2. Set AI state to WANDER for all members
        // 3. AI picks random nearby positions and moves there
        // 4. Units engage enemies encountered but don't pursue far
        // 5. After reaching position, pick new random destination
        // Wander radius typically 100-200 game units from current position
        // In C++: AI state WANDER with periodic random destination selection

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamWanderAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Wander, CommandSourceType::FromScript);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_wander"
    }

    fn description(&self) -> &str {
        "Commands a team to wander around their current area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team stops
pub(super) struct TeamIdleAction;

#[async_trait]
impl ScriptAction for TeamIdleAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' going idle", team_name);

        // Integration with AI system - stop all actions:
        // 1. Resolve team
        // 2. For each team member:
        //    a. Clear AI command queue
        //    b. Cancel current action
        //    c. Set state to IDLE
        // 3. Units stop moving, stop attacking, enter standby
        // 4. Will still defend if attacked (return fire)
        // In C++: theTeam->stopAllActions() clears all AI update queues

        let resolved_team = resolve_team_name_token(&team_name);
        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamIdleAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_idle"
    }

    fn description(&self) -> &str {
        "Commands a team to stop all actions and go idle"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set team state
pub(super) struct TeamSetStateAction;

#[async_trait]
impl ScriptAction for TeamSetStateAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;
        let state = get_string_param(parameters, "state")?;

        log::info!("Setting team '{}' state to '{}'", team_name, state);

        // Matches C++ ScriptActions.cpp:doSetTeamState line 468
        // Integration with team state management:
        // In C++: theTeam->setState(state)
        // Team state is a string that scripts can use to track team status
        // Common states: "idle", "attacking", "defending", "retreating", etc.
        // Scripts check these states in conditions to trigger appropriate actions
        // State stored in Team::m_state (AsciiString)
        // Rust: team.set_state(state.clone())

        let resolved_team = resolve_team_name_token(&team_name);
        let factory = get_team_factory();
        let Some(team_arc) = factory
            .lock()
            .ok()
            .and_then(|mut guard| guard.find_team(&resolved_team))
        else {
            log::warn!("TeamSetStateAction: team '{}' not found", resolved_team);
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(mut team_guard) = team_arc.write() {
            team_guard.set_state(AsciiString::from(state.as_str()));
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_set_state"
    }

    fn description(&self) -> &str {
        "Sets the state of a team"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "state".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Delete team
pub(super) struct TeamDeleteAction;

#[async_trait]
impl ScriptAction for TeamDeleteAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Deleting team '{}'", team_name);

        // Matches C++ ScriptActions.cpp:doKillTeam line 2504
        // Integration with team management:
        // In C++: theTeam->killTeam()
        // 1. Resolve team by name
        // 2. For each member in team:
        //    a. Call object->kill(DEATH_NORMAL) or object->destroy()
        //    b. Remove from world
        //    c. Cleanup object resources
        // 3. Remove team from team registry
        // 4. Free team memory
        // WARNING: Destructive operation, cannot be undone

        let resolved_team = resolve_team_name_token(&team_name);
        let factory = get_team_factory();
        let team_arc = {
            let mut guard = factory
                .lock()
                .map_err(|_| GameLogicError::Threading("Failed to lock TeamFactory".to_string()))?;
            guard.find_team(&resolved_team)
        };

        let Some(team_arc) = team_arc else {
            log::warn!("TeamDeleteAction: team '{}' not found", resolved_team);
            return Ok(ScriptResult::Success(None));
        };

        let (team_id, members) = if let Ok(team_guard) = team_arc.read() {
            (team_guard.get_id(), team_guard.get_members().to_vec())
        } else {
            log::warn!("TeamDeleteAction: failed to read team '{}'", resolved_team);
            return Ok(ScriptResult::Success(None));
        };

        for member_id in members {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    obj_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                }
            }
        }

        if let Ok(mut factory_guard) = factory.lock() {
            factory_guard.team_about_to_be_deleted(team_id);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_delete"
    }

    fn description(&self) -> &str {
        "Deletes a team and all its members"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team follows another team
pub(super) struct TeamFollowTeamAction;

#[async_trait]
impl ScriptAction for TeamFollowTeamAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let follower_team = get_string_param(parameters, "follower_team")?;
        let target_team = get_string_param(parameters, "target_team")?;

        log::info!("Team '{}' following team '{}'", follower_team, target_team);

        // Integration with team AI follow behavior:
        // 1. Resolve both teams
        // 2. Set follower team's AI state to FOLLOW
        // 3. Set target team as follow target
        // 4. Follower team continuously updates position to stay near target
        // 5. Follow distance typically 20-50 game units behind
        // 6. If target moves, follower adjusts position
        // 7. If target engages enemies, follower assists
        // In C++: theTeam->followTeam(targetTeam)

        let resolved_follower = resolve_team_name_token(&follower_team);
        let resolved_target = resolve_team_name_token(&target_team);

        let target_id = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_target))
            .and_then(|team_arc| {
                team_arc
                    .read()
                    .ok()
                    .and_then(|team| team.get_members().first().copied())
            });

        let Some(target_id) = target_id else {
            log::warn!(
                "TeamFollowTeamAction: target team '{}' has no members",
                resolved_target
            );
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_follower) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamFollowTeamAction: failed to create AI group for team '{}': {}",
                    resolved_follower,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };

        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_follow_team"
    }

    fn description(&self) -> &str {
        "Commands one team to follow another team"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["follower_team".to_string(), "target_team".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team guards tunnel
pub(super) struct TeamGuardInTunnelAction;

#[async_trait]
impl ScriptAction for TeamGuardInTunnelAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team_name")?;

        log::info!("Team '{}' guarding in tunnel network", team_name);

        let resolved_team = resolve_team_name_token(&team_name);
        let team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&resolved_team));

        let Some(team_arc) = team_arc else {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' not found",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        };

        let (members, controlling_player_id) = team_arc
            .read()
            .ok()
            .map(|team| {
                (
                    team.get_members().to_vec(),
                    team.get_controlling_player_id(),
                )
            })
            .unwrap_or_default();

        if members.is_empty() {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' has no members",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let player_arc = if let Some(player_id) = controlling_player_id {
            player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(player_id as PlayerIndex).cloned())
        } else {
            None
        };

        let player_arc = player_arc.or_else(|| {
            members.iter().find_map(|member_id| {
                TheGameLogic::find_object_by_id(*member_id).and_then(|obj| {
                    obj.read()
                        .ok()
                        .and_then(|guard| guard.get_controlling_player())
                })
            })
        });

        let Some(player_arc) = player_arc else {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' has no controlling player",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        };

        let tunnel_ids = player_arc
            .read()
            .ok()
            .and_then(|player| player.get_tunnel_system().cloned())
            .and_then(|tracker| tracker.get_container_list().ok())
            .unwrap_or_default();

        if tunnel_ids.is_empty() {
            log::warn!(
                "TeamGuardInTunnelAction: team '{}' has no tunnel network",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        let mut tunnel_entries = Vec::new();
        for tunnel_id in tunnel_ids {
            let Some(tunnel_arc) = TheGameLogic::find_object_by_id(tunnel_id) else {
                continue;
            };
            let Ok(tunnel_guard) = tunnel_arc.read() else {
                continue;
            };
            tunnel_entries.push((tunnel_id, *tunnel_guard.get_position()));
        }

        if tunnel_entries.is_empty() {
            log::warn!(
                "TeamGuardInTunnelAction: tunnel entries unavailable for team '{}'",
                resolved_team
            );
            return Ok(ScriptResult::Success(None));
        }

        for member_id in members {
            let Some(unit_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(unit_guard) = unit_arc.read() else {
                continue;
            };
            let unit_pos = *unit_guard.get_position();
            let mut best_tunnel = tunnel_entries[0].0;
            let mut best_dist_sq = Real::MAX;
            for (tunnel_id, tunnel_pos) in &tunnel_entries {
                let dx = unit_pos.x - tunnel_pos.x;
                let dy = unit_pos.y - tunnel_pos.y;
                let dz = unit_pos.z - tunnel_pos.z;
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_tunnel = *tunnel_id;
                }
            }

            if let Some(ai) = unit_guard.get_ai_update_interface() {
                ai.ai_enter(best_tunnel, CommandSourceType::FromScript);
                continue;
            }

            if let Some(tunnel_arc) = TheGameLogic::find_object_by_id(best_tunnel) {
                if let Ok(tunnel_guard) = tunnel_arc.read() {
                    if let Some(contain) = tunnel_guard.get_contain() {
                        if contain.is_valid_container_for(&unit_guard, true) {
                            contain.add_to_contain(&unit_guard);
                        }
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_guard_in_tunnel"
    }

    fn description(&self) -> &str {
        "Commands a team to guard inside a tunnel network"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team Attack Action - Commands team to attack target
pub(super) struct TeamAttackAction;

#[async_trait]
impl ScriptAction for TeamAttackAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team")?;
        let target = get_string_param(parameters, "target")?;

        log::info!("Team '{}' attacking target '{}'", team_name, target);

        // Matches C++ ScriptActions.cpp:doTeamAttackNamed line 1414
        // Implementation:
        // 1. Team *team = TheScriptEngine->getTeamNamed(teamName)
        // 2. Object *target = TheScriptEngine->getUnitNamed(targetName)
        // 3. for each unit in team:
        //    a. AIUpdateInterface *ai = unit->getAIUpdateInterface()
        //    b. ai->aiAttackObject(target, CMD_FROM_SCRIPT)
        // All team members attack same target
        // Rust: team_manager.get_team(team_name).attack_target(target)

        let resolved_team = resolve_team_name_token(&team_name);
        let Some(target_id) = resolve_named_object_id(&target) else {
            log::warn!("TeamAttackAction: target '{}' not found", target);
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamAttackAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };
        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            params.int_value = -1; // NO_MAX_SHOTS_LIMIT
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_attack"
    }

    fn description(&self) -> &str {
        "Commands a team to attack a target"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "target".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team Attack Area Action - Matches C++ ScriptActions::doTeamAttackArea (line 1387)
pub(super) struct TeamAttackAreaAction;

#[async_trait]
impl ScriptAction for TeamAttackAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team")?;
        let area = get_string_param(parameters, "area")?;

        log::info!("Team '{}' attacking area '{}'", team_name, area);

        let resolved_team = resolve_team_name_token(&team_name);
        let (center, trigger_id) = if let Ok(terrain_guard) = get_terrain_logic().read() {
            if let Some(trigger) = terrain_guard.get_trigger_area_by_name(&area) {
                (trigger.get_center_point(), trigger.get_id())
            } else {
                log::warn!("TeamAttackAreaAction: trigger area '{}' not found", area);
                return Ok(ScriptResult::Success(None));
            }
        } else {
            log::warn!("TeamAttackAreaAction: failed to lock terrain logic");
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamAttackAreaAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };
        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackArea, CommandSourceType::FromScript);
            params.pos = center;
            params.polygon = Some(trigger_id);
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_attack_area"
    }

    fn description(&self) -> &str {
        "Commands a team to attack targets in an area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team Guard Area Action - Matches C++ ScriptActions::doTeamGuardArea (line 1946)
pub(super) struct TeamGuardAreaAction;

#[async_trait]
impl ScriptAction for TeamGuardAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team")?;
        let area = get_string_param(parameters, "area")?;

        log::info!("Team '{}' guarding area '{}'", team_name, area);

        // Matches C++ ScriptActions.cpp:doTeamGuardArea line 1946
        // Implementation:
        // 1. Team *team = TheScriptEngine->getTeamNamed(teamName)
        // 2. PolygonTrigger *area = TheScriptEngine->getAreaByName(areaName)
        // 3. for each unit in team:
        //    a. AIUpdateInterface *ai = unit->getAIUpdateInterface()
        //    b. ai->aiGuardArea(area, CMD_FROM_SCRIPT)
        // Units patrol and defend area perimeter
        // Engages enemies that enter area
        // Rust: team_manager.get_team(team_name).guard_area(area)

        let resolved_team = resolve_team_name_token(&team_name);
        let (center, trigger_id) = if let Ok(terrain_guard) = get_terrain_logic().read() {
            if let Some(trigger) = terrain_guard.get_trigger_area_by_name(&area) {
                (trigger.get_center_point(), trigger.get_id())
            } else {
                log::warn!("TeamGuardAreaAction: trigger area '{}' not found", area);
                return Ok(ScriptResult::Success(None));
            }
        } else {
            log::warn!("TeamGuardAreaAction: failed to lock terrain logic");
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamGuardAreaAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };
        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardArea, CommandSourceType::FromScript);
            params.pos = center;
            params.polygon = Some(trigger_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_guard_area"
    }

    fn description(&self) -> &str {
        "Commands a team to guard an area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team Follow Action - Commands team to follow another team or unit
pub(super) struct TeamFollowAction;

#[async_trait]
impl ScriptAction for TeamFollowAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = get_string_param(parameters, "team")?;
        let target = get_string_param(parameters, "target")?;

        log::info!("Team '{}' following target '{}'", team_name, target);

        // C++ Implementation (TeamFollow behavior):
        // 1. Team *team = TheScriptEngine->getTeamNamed(teamName)
        // 2. Object *target = TheScriptEngine->getUnitNamed(targetName)
        //    Or: Team *targetTeam = TheScriptEngine->getTeamNamed(targetName)
        // 3. for each unit in team:
        //    a. AIUpdateInterface *ai = unit->getAIUpdateInterface()
        //    b. ai->aiFollowObject(target, CMD_FROM_SCRIPT)
        // Units maintain formation following target
        // Updates position as target moves
        // Rust: team_manager.get_team(team_name).follow_target(target)

        let resolved_team = resolve_team_name_token(&team_name);
        let target_id = resolve_named_object_id(&target).or_else(|| {
            let factory = get_team_factory();
            factory
                .lock()
                .ok()
                .and_then(|mut factory_guard| {
                    factory_guard.find_team(&resolve_team_name_token(&target))
                })
                .and_then(|team_arc| {
                    team_arc
                        .read()
                        .ok()
                        .and_then(|team| team.get_members().first().copied())
                })
        });

        let Some(target_id) = target_id else {
            log::warn!("TeamFollowAction: target '{}' not found", target);
            return Ok(ScriptResult::Success(None));
        };

        let group_arc = match create_ai_group_from_team(&resolved_team) {
            Ok(group) => group,
            Err(err) => {
                log::warn!(
                    "TeamFollowAction: failed to create AI group for team '{}': {}",
                    resolved_team,
                    err
                );
                return Ok(ScriptResult::Success(None));
            }
        };
        if let Ok(mut group_guard) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group_guard.ai_do_command(&params);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_follow"
    }

    fn description(&self) -> &str {
        "Commands a team to follow a target"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "target".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
