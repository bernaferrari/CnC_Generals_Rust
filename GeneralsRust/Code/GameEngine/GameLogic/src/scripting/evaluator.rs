//! Script Evaluation System
//!
//! This module provides script evaluation logic that matches the C++ ScriptEngine evaluation,
//! including condition evaluation, action execution, and script state management.

use super::core::*;
use super::engine::{get_area_tracker, get_named_object_tracker, ScriptActionHandler, *};
use super::executor::{
    ScriptActionDispatcher, ScriptActionResult, ScriptConditionEvaluator, ScriptConditionResult,
    ScriptContext,
};
use crate::commands::get_selection_manager;
use crate::common::{
    AsciiString, DisabledType, KindOf, ObjectID, ObjectShroudStatus, PlayerMaskType, UnsignedInt,
    LOGICFRAMES_PER_SECOND,
};
use crate::helpers::TheGameLogic;
use crate::modules::ContainModuleInterface;
use crate::object::object_types::ObjectTypes;
use crate::player::player_list;
use crate::polygon_trigger::PolygonTrigger;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{get_science_store, SCIENCE_INVALID};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Script Evaluator matching C++ ScriptEngine evaluation logic
pub struct ScriptEvaluator {
    engine: Arc<RwLock<Option<ScriptEngine>>>,
}

static TRANSPORT_STATUSES: Lazy<RwLock<HashMap<ObjectID, (UnsignedInt, usize)>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

impl ScriptEvaluator {
    pub fn new(engine: Arc<RwLock<Option<ScriptEngine>>>) -> Self {
        Self { engine }
    }

    /// Evaluate a complete script matching C++ EvaluateScripts
    pub fn evaluate_script(&self, script: &mut Script) -> GameLogicResult<bool> {
        log::debug!("Evaluating script: {}", script.get_name());

        // Check if script is active
        if !script.is_active() {
            return Ok(false);
        }

        // Evaluate conditions
        let condition_result = if let Some(or_condition) = script.condition.as_deref_mut() {
            self.evaluate_or_condition(or_condition)?
        } else {
            true // No conditions means always true
        };

        log::debug!(
            "Script '{}' condition result: {}",
            script.get_name(),
            condition_result
        );

        // Execute actions based on condition result
        if condition_result {
            if let Some(action) = script.get_action() {
                self.execute_action_sequence(action)?;
            }
        } else {
            if let Some(false_action) = script.get_false_action() {
                self.execute_action_sequence(false_action)?;
            }
        }

        Ok(condition_result)
    }

    /// Evaluate OR condition matching C++ EvaluateConditions
    pub fn evaluate_or_condition(&self, or_condition: &mut OrCondition) -> GameLogicResult<bool> {
        let mut current_or = Some(or_condition);

        while let Some(or_cond) = current_or {
            // Evaluate all AND conditions in this OR clause
            if let Some(and_condition) = or_cond.first_and.as_deref_mut() {
                if self.evaluate_and_condition(and_condition)? {
                    return Ok(true); // Short-circuit on first true OR
                }
            }

            current_or = or_cond.next_or.as_deref_mut();
        }

        Ok(false) // All OR conditions were false
    }

    /// Evaluate AND condition chain
    pub fn evaluate_and_condition(&self, and_condition: &mut Condition) -> GameLogicResult<bool> {
        let mut current_and = Some(and_condition);

        while let Some(and_cond) = current_and {
            if !self.evaluate_condition(and_cond)? {
                return Ok(false); // Short-circuit on first false AND
            }

            current_and = and_cond.next_and_condition.as_deref_mut();
        }

        Ok(true) // All AND conditions were true
    }

    /// Evaluate a single condition matching C++ EvaluateCondition
    pub fn evaluate_condition(&self, condition: &mut Condition) -> GameLogicResult<bool> {
        const SLOW_SCRIPT_CONDITION_WARN_MS: u64 = 40;
        let condition_type = condition.get_condition_type();
        let eval_started = Instant::now();
        let result = match condition_type {
            ConditionType::ConditionFalse => Ok(false),
            ConditionType::ConditionTrue => Ok(true),
            ConditionType::Counter => self.evaluate_counter_condition(condition),
            ConditionType::Flag => self.evaluate_flag_condition(condition),
            ConditionType::TimerExpired => self.evaluate_timer_expired_condition(condition),
            ConditionType::PlayerAllDestroyed => {
                self.evaluate_player_all_destroyed_condition(condition)
            }
            ConditionType::PlayerAllBuildfacilitiesDestroyed => {
                self.evaluate_player_all_buildfacilities_destroyed_condition(condition)
            }
            ConditionType::TeamInsideAreaPartially => {
                self.evaluate_team_inside_area_partially_condition(condition)
            }
            ConditionType::TeamDestroyed => self.evaluate_team_destroyed_condition(condition),
            ConditionType::TeamHasUnits => self.evaluate_team_has_units_condition(condition),
            ConditionType::TeamStateIs => self.evaluate_team_state_is_condition(condition),
            ConditionType::TeamStateIsNot => self.evaluate_team_state_is_not_condition(condition),
            ConditionType::NamedInsideArea => self.evaluate_named_inside_area_condition(condition),
            ConditionType::NamedOutsideArea => {
                self.evaluate_named_outside_area_condition(condition)
            }
            ConditionType::NamedDestroyed => self.evaluate_named_destroyed_condition(condition),
            ConditionType::NamedNotDestroyed => {
                self.evaluate_named_not_destroyed_condition(condition)
            }
            ConditionType::NamedAttackedByObjecttype => {
                self.evaluate_named_attacked_by_object_type_condition(condition)
            }
            ConditionType::TeamAttackedByObjecttype => {
                self.evaluate_team_attacked_by_object_type_condition(condition)
            }
            ConditionType::NamedAttackedByPlayer => {
                self.evaluate_named_attacked_by_player_condition(condition)
            }
            ConditionType::TeamAttackedByPlayer => {
                self.evaluate_team_attacked_by_player_condition(condition)
            }
            ConditionType::NamedCreated => self.evaluate_named_created_condition(condition),
            ConditionType::TeamCreated => self.evaluate_team_created_condition(condition),
            ConditionType::NamedDiscovered => self.evaluate_named_discovered_condition(condition),
            ConditionType::TeamDiscovered => self.evaluate_team_discovered_condition(condition),
            ConditionType::TeamInsideAreaEntirely => {
                self.evaluate_team_inside_area_entirely_condition(condition)
            }
            ConditionType::TeamOutsideAreaEntirely => {
                self.evaluate_team_outside_area_entirely_condition(condition)
            }
            ConditionType::PlayerHasCredits => {
                self.evaluate_player_has_credits_condition(condition)
            }
            ConditionType::PlayerHasPower => self.evaluate_player_has_power_condition(condition),
            ConditionType::PlayerHasNoPower => {
                self.evaluate_player_has_no_power_condition(condition)
            }
            ConditionType::NamedOwnedByPlayer => {
                self.evaluate_named_owned_by_player_condition(condition)
            }
            ConditionType::TeamOwnedByPlayer => {
                self.evaluate_team_owned_by_player_condition(condition)
            }
            ConditionType::PlayerHasNOrFewerBuildings => {
                self.evaluate_player_has_n_or_fewer_buildings_condition(condition)
            }
            ConditionType::BuildingEnteredByPlayer => {
                self.evaluate_building_entered_by_player_condition(condition)
            }
            ConditionType::HasFinishedVideo => {
                self.evaluate_has_finished_video_condition(condition)
            }
            ConditionType::HasFinishedSpeech => {
                self.evaluate_has_finished_speech_condition(condition)
            }
            ConditionType::HasFinishedAudio => {
                self.evaluate_has_finished_audio_condition(condition)
            }
            ConditionType::UnitHealth => self.evaluate_unit_health_condition(condition),
            ConditionType::NamedEnteredArea => {
                self.evaluate_named_entered_area_condition(condition)
            }
            ConditionType::NamedExitedArea => self.evaluate_named_exited_area_condition(condition),
            ConditionType::NamedDying => self.evaluate_named_dying_condition(condition),
            ConditionType::NamedTotallyDead => {
                self.evaluate_named_totally_dead_condition(condition)
            }
            ConditionType::NamedSelected => self.evaluate_named_selected_condition(condition),
            ConditionType::TeamEnteredAreaEntirely => {
                self.evaluate_team_entered_area_entirely_condition(condition)
            }
            ConditionType::TeamEnteredAreaPartially => {
                self.evaluate_team_entered_area_partially_condition(condition)
            }
            ConditionType::TeamExitedAreaEntirely => {
                self.evaluate_team_exited_area_entirely_condition(condition)
            }
            ConditionType::TeamExitedAreaPartially => {
                self.evaluate_team_exited_area_partially_condition(condition)
            }
            ConditionType::PlayerHasNOrFewerFactionBuildings => {
                self.evaluate_player_has_n_or_fewer_faction_buildings_condition(condition)
            }
            ConditionType::BuiltByPlayer => self.evaluate_built_by_player_condition(condition),
            ConditionType::NamedBuildingIsEmpty => {
                self.evaluate_named_building_is_empty_condition(condition)
            }
            ConditionType::PlayerPowerComparePercent => {
                self.evaluate_player_power_compare_percent_condition(condition)
            }
            ConditionType::PlayerExcessPowerCompareValue => {
                self.evaluate_player_excess_power_compare_value_condition(condition)
            }
            ConditionType::UnitHasObjectStatus => {
                self.evaluate_unit_has_object_status_condition(condition)
            }
            ConditionType::TeamAllHasObjectStatus => {
                self.evaluate_team_has_object_status_condition(condition, true)
            }
            ConditionType::TeamSomeHaveObjectStatus => {
                self.evaluate_team_has_object_status_condition(condition, false)
            }
            ConditionType::PlayerAcquiredScience => {
                self.evaluate_player_acquired_science_condition(condition)
            }
            ConditionType::PlayerHasSciencepurchasepoints => {
                self.evaluate_player_has_science_purchase_points_condition(condition)
            }
            ConditionType::PlayerCanPurchaseScience => {
                self.evaluate_player_can_purchase_science_condition(condition)
            }
            ConditionType::NamedHasFreeContainerSlots => {
                self.evaluate_named_has_free_container_slots_condition(condition)
            }
            ConditionType::UnitEmptied => self.evaluate_unit_emptied_condition(condition),

            // Camera movement finished (C++: TheTacticalView->isCameraMovementFinished())
            ConditionType::CameraMovementFinished => {
                // C++ checks TheTacticalView->isCameraMovementFinished()
                // Query the action handler for camera state; default true (no camera = no movement = finished)
                let engine = self.engine.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
                })?;
                Ok(engine
                    .as_ref()
                    .and_then(|e| e.action_handler())
                    .map(|h| h.is_camera_movement_finished())
                    .unwrap_or(true))
            }

            // Mission attempts comparison (C++: evaluateMissionAttempts - always returns false)
            ConditionType::MissionAttempts => {
                // C++ evaluateMissionAttempts is a stub that always returns false
                Ok(false)
            }

            // Named unit reached end of waypoint path
            ConditionType::NamedReachedWaypointsEnd => {
                let unit_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "NamedReachedWaypointsEnd condition missing unit parameter".to_string(),
                    )
                })?;
                let _waypoint_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "NamedReachedWaypointsEnd condition missing waypoint parameter".to_string(),
                    )
                })?;

                let unit_name = unit_param.get_string();

                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
                    return Ok(false);
                };
                let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                    return Ok(false);
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    return Ok(false);
                };
                let Some(ai) = obj_guard.get_ai_update_interface() else {
                    return Ok(false);
                };
                let Ok(ai_guard) = ai.lock() else {
                    return Ok(false);
                };
                let Some(completed_id) = ai_guard.get_completed_waypoint_id() else {
                    return Ok(false);
                };

                // C++ checks waypoint pathLabel1/2/3 against the waypoint path name.
                // Query the terrain Waypoint (which carries path labels) by ID.
                let waypoint_path_name = _waypoint_param.get_string();
                let Ok(terrain) = get_terrain_logic().read() else {
                    return Ok(false);
                };
                let matches = terrain
                    .get_waypoint_by_id(completed_id)
                    .is_some_and(|wp| wp.matches_path_label(&waypoint_path_name));
                Ok(matches)
            }

            // Team reached end of waypoint path (any member)
            ConditionType::TeamReachedWaypointsEnd => {
                let team_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TeamReachedWaypointsEnd condition missing team parameter".to_string(),
                    )
                })?;
                let _waypoint_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TeamReachedWaypointsEnd condition missing waypoint parameter".to_string(),
                    )
                })?;

                let team_name = self.resolve_team_name_token(team_param.get_string());

                for team_arc in self.resolve_team_instances(&team_name) {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    for &member_id in team_guard.get_members() {
                        let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            continue;
                        };
                        let Ok(member_guard) = member_arc.read() else {
                            continue;
                        };
                        let Some(ai) = member_guard.get_ai_update_interface() else {
                            continue;
                        };
                        let Ok(ai_guard) = ai.lock() else {
                            continue;
                        };
                        if ai_guard.get_completed_waypoint_id().is_some() {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }

            // Multiplayer: local player's alliance achieved victory
            ConditionType::MultiplayerAlliedVictory => {
                Ok(crate::helpers::TheVictoryConditions::is_local_allied_victory())
            }

            // Multiplayer: local player's alliance was defeated
            // C++: TheVictoryConditions->isLocalAlliedDefeat()
            ConditionType::MultiplayerAlliedDefeat => {
                let Ok(list) = player_list().read() else {
                    return Ok(false);
                };
                let Some(local_player_arc) = list.get_local_player() else {
                    return Ok(false);
                };
                let Ok(local_player) = local_player_arc.read() else {
                    return Ok(false);
                };

                let mut allied_count = 0usize;
                for player_arc in list.iter() {
                    if Arc::ptr_eq(player_arc, &local_player_arc) {
                        allied_count += 1;
                        if !local_player.is_defeated() {
                            return Ok(false);
                        }
                        continue;
                    }
                    let Ok(player) = player_arc.read() else {
                        continue;
                    };
                    if local_player.is_allied_with_player(&player) {
                        allied_count += 1;
                        if !player.is_defeated() {
                            return Ok(false);
                        }
                    }
                }
                Ok(allied_count > 0)
            }

            // Multiplayer: local player individually defeated (not whole alliance)
            // C++: TheVictoryConditions->isLocalDefeat() && !TheVictoryConditions->isLocalAlliedDefeat()
            ConditionType::MultiplayerPlayerDefeat => {
                let Ok(list) = player_list().read() else {
                    return Ok(false);
                };
                let Some(local_player_arc) = list.get_local_player() else {
                    return Ok(false);
                };
                let Ok(local_player) = local_player_arc.read() else {
                    return Ok(false);
                };

                if !local_player.is_player_dead() {
                    return Ok(false);
                }

                let mut has_alive_ally = false;
                for player_arc in list.iter() {
                    if Arc::ptr_eq(player_arc, &local_player_arc) {
                        continue;
                    }
                    let Ok(player) = player_arc.read() else {
                        continue;
                    };
                    if local_player.is_allied_with_player(&player) && !player.is_defeated() {
                        has_alive_ally = true;
                        break;
                    }
                }
                Ok(has_alive_ally)
            }

            // Named unit has sighted an enemy/friendly/neutral unit belonging to a side
            ConditionType::EnemySighted => {
                let unit_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "EnemySighted condition missing unit parameter".to_string(),
                    )
                })?;
                let _alliance_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "EnemySighted condition missing alliance parameter".to_string(),
                    )
                })?;
                let player_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "EnemySighted condition missing player parameter".to_string(),
                    )
                })?;

                let unit_name = unit_param.get_string();
                let target_player = self.resolve_player_from_param(player_param);
                if target_player.is_none() {
                    return Ok(false);
                }

                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
                    return Ok(false);
                };
                let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                    return Ok(false);
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    return Ok(false);
                };

                let obj_pos = *obj_guard.get_position();
                let vision = obj_guard.get_vision_range();

                let partition = crate::helpers::ThePartitionManager::get()
                    .map(|pm| pm.get_objects_in_range(&obj_pos, vision))
                    .unwrap_or_default();

                let target_player_id = target_player
                    .as_ref()
                    .and_then(|p| p.read().ok())
                    .map(|p| p.get_player_index());

                for nearby_id in partition {
                    if nearby_id == object_id {
                        continue;
                    }
                    let Some(nearby_arc) = TheGameLogic::find_object_by_id(nearby_id) else {
                        continue;
                    };
                    let Ok(nearby_guard) = nearby_arc.read() else {
                        continue;
                    };
                    if nearby_guard.is_effectively_dead() {
                        continue;
                    }
                    let Some(nearby_player_id) = nearby_guard.get_controlling_player_id() else {
                        continue;
                    };
                    if Some(nearby_player_id as i32) == target_player_id {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            // Named bridge has been repaired (damage state changed to intact)
            ConditionType::BridgeRepaired => {
                let bridge_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "BridgeRepaired condition missing bridge parameter".to_string(),
                    )
                })?;

                let bridge_name = bridge_param.get_string();
                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(bridge_name).ok().flatten() else {
                    return Ok(false);
                };

                let Ok(terrain) = get_terrain_logic().read() else {
                    return Ok(false);
                };
                Ok(terrain.is_bridge_repaired(object_id))
            }

            // Named bridge has been broken (damage state changed to broken)
            ConditionType::BridgeBroken => {
                let bridge_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "BridgeBroken condition missing bridge parameter".to_string(),
                    )
                })?;

                let bridge_name = bridge_param.get_string();
                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(bridge_name).ok().flatten() else {
                    return Ok(false);
                };

                let Ok(terrain) = get_terrain_logic().read() else {
                    return Ok(false);
                };
                Ok(terrain.is_bridge_broken(object_id))
            }

            // Player has comparison count of a specific object type
            ConditionType::PlayerHasObjectComparison => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing comparison parameter"
                            .to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing count parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing type parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                let types = self.resolve_object_types(type_param);

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut count = 0;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                        continue;
                    }
                    if types.contains_template(Some(obj_guard.get_template())) {
                        count += 1;
                    }
                }

                match comparison {
                    0 => Ok(count < target_count),  // LessThan
                    1 => Ok(count <= target_count), // LessEqual
                    2 => Ok(count == target_count), // Equal
                    3 => Ok(count >= target_count), // GreaterEqual
                    4 => Ok(count > target_count),  // Greater
                    5 => Ok(count != target_count), // NotEqual
                    _ => Ok(false),
                }
            }

            // Obsolete script conditions (no longer used in C++)
            ConditionType::ObsoleteScript1 | ConditionType::ObsoleteScript2 => {
                // C++ has no handler for these; they fall through to DEBUG_CRASH
                Ok(false)
            }

            // Player triggered a special power (any source unit)
            ConditionType::PlayerTriggeredSpecialPower => {
                self.evaluate_special_power_condition(condition, false, false)
            }

            // Player completed a special power (any source unit)
            ConditionType::PlayerCompletedSpecialPower => {
                self.evaluate_special_power_condition(condition, false, true)
            }

            // Player midway through special power (any source unit)
            ConditionType::PlayerMidwaySpecialPower => {
                self.evaluate_special_power_condition(condition, true, false)
            }

            // Player triggered special power from a specific named unit
            ConditionType::PlayerTriggeredSpecialPowerFromNamed => {
                self.evaluate_special_power_condition(condition, false, false)
            }

            // Player completed special power from a specific named unit
            ConditionType::PlayerCompletedSpecialPowerFromNamed => {
                self.evaluate_special_power_condition(condition, false, true)
            }

            // Player midway through special power from a specific named unit
            ConditionType::PlayerMidwaySpecialPowerFromNamed => {
                self.evaluate_special_power_condition(condition, true, false)
            }

            // Defunct: player selected general (removed in C++)
            ConditionType::DefunctPlayerSelectedGeneral
            | ConditionType::DefunctPlayerSelectedGeneralFromNamed => {
                // C++ DEBUG_CRASH: "PLAYER_SELECTED_GENERAL script conditions are no longer in use"
                Ok(false)
            }

            // Player built an upgrade (any source unit)
            ConditionType::PlayerBuiltUpgrade => self.evaluate_upgrade_condition(condition, false),

            // Player built an upgrade from a specific named unit
            ConditionType::PlayerBuiltUpgradeFromNamed => {
                self.evaluate_upgrade_condition(condition, true)
            }

            // Player destroyed N or more of opponent's buildings
            ConditionType::PlayerDestroyedNBuildingsPlayer => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerDestroyedNBuildingsPlayer condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let opponent_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerDestroyedNBuildingsPlayer condition missing opponent parameter"
                            .to_string(),
                    )
                })?;

                // C++ evaluatePlayerDestroyedNOrMoreBuildings resolves both players, ignores N,
                // then returns FALSE because the condition body is still a TODO.
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(_player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let Some(opponent_arc) = self.resolve_player_from_param(opponent_param) else {
                    return Ok(false);
                };
                let Ok(_opponent_guard) = opponent_arc.read() else {
                    return Ok(false);
                };
                Ok(false)
            }

            // Unit completed sequential script execution
            // C++: NO case in switch — falls through to DEBUG_CRASH returning false.
            // ScriptEngine::hasUnitCompletedSequentialScript() always returns FALSE.
            ConditionType::UnitCompletedSequentialExecution => Ok(false),

            // Team completed sequential script execution
            // C++: NO case in switch — falls through to DEBUG_CRASH returning false.
            // ScriptEngine::hasTeamCompletedSequentialScript() always returns FALSE.
            ConditionType::TeamCompletedSequentialExecution => Ok(false),

            // Player has comparison count of unit type within a trigger area
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing count parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing type parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(4).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing trigger parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                let types = self.resolve_object_types(type_param);
                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut count = 0;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_effectively_dead() {
                        continue;
                    }
                    if types.contains_template(Some(obj_guard.get_template())) {
                        if obj_guard.is_inside_trigger(&trigger) {
                            // C++ allows crates even though they are "dead"
                            if !obj_guard.is_kind_of(KindOf::Inert)
                                || obj_guard.is_kind_of(KindOf::Crate)
                            {
                                count += 1;
                            }
                        }
                    }
                }

                match comparison {
                    0 => Ok(count < target_count),
                    1 => Ok(count <= target_count),
                    2 => Ok(count == target_count),
                    3 => Ok(count >= target_count),
                    4 => Ok(count > target_count),
                    5 => Ok(count != target_count),
                    _ => Ok(false),
                }
            }

            // Player has comparison count of unit kind within a trigger area
            // C++: evaluatePlayerHasUnitKindInArea filters by pObj->isKindOf((KindOfType)kindParam)
            ConditionType::PlayerHasComparisonUnitKindInTriggerArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing count parameter".to_string(),
                    )
                })?;
                let kind_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing kind parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(4).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing trigger parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                let kind_of_type_int = kind_param.get_int();
                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let kind_of_filter = Self::kind_of_type_to_mask(kind_of_type_int);
                let mut count = 0;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_effectively_dead() || obj_guard.is_kind_of(KindOf::Inert) {
                        continue;
                    }
                    if obj_guard.is_inside_trigger(&trigger) {
                        if let Some(kind) = kind_of_filter {
                            if !obj_guard.is_kind_of(kind) {
                                continue;
                            }
                        }
                        count += 1;
                    }
                }

                match comparison {
                    0 => Ok(count < target_count),
                    1 => Ok(count <= target_count),
                    2 => Ok(count == target_count),
                    3 => Ok(count >= target_count),
                    4 => Ok(count > target_count),
                    5 => Ok(count != target_count),
                    _ => Ok(false),
                }
            }

            // Named unit has sighted a specific object type
            ConditionType::TypeSighted => {
                let unit_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TypeSighted condition missing unit parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TypeSighted condition missing type parameter".to_string(),
                    )
                })?;
                let player_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TypeSighted condition missing player parameter".to_string(),
                    )
                })?;

                let unit_name = unit_param.get_string();
                let types = self.resolve_object_types(type_param);
                let target_player = self.resolve_player_from_param(player_param);
                if target_player.is_none() {
                    return Ok(false);
                }

                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
                    return Ok(false);
                };
                let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                    return Ok(false);
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    return Ok(false);
                };

                let obj_pos = *obj_guard.get_position();
                let vision = obj_guard.get_vision_range();
                let partition = crate::helpers::ThePartitionManager::get()
                    .map(|pm| pm.get_objects_in_range(&obj_pos, vision))
                    .unwrap_or_default();

                let target_player_id = target_player
                    .as_ref()
                    .and_then(|p| p.read().ok())
                    .map(|p| p.get_player_index());

                for nearby_id in partition {
                    if nearby_id == object_id {
                        continue;
                    }
                    let Some(nearby_arc) = TheGameLogic::find_object_by_id(nearby_id) else {
                        continue;
                    };
                    let Ok(nearby_guard) = nearby_arc.read() else {
                        continue;
                    };
                    if nearby_guard.is_effectively_dead() {
                        continue;
                    }
                    if Some(nearby_guard.get_controlling_player_id().unwrap_or(0) as i32)
                        == target_player_id
                    {
                        if types.contains_template(Some(nearby_guard.get_template())) {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }

            // --- Skirmish AI conditions ---

            // Skirmish: a specific special power is ready to use
            ConditionType::SkirmishSpecialPowerReady => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSpecialPowerReady condition missing player parameter".to_string(),
                    )
                })?;
                let power_name_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSpecialPowerReady condition missing power name parameter"
                            .to_string(),
                    )
                })?;

                let power_name = power_name_param.get_string();
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_destroyed() {
                        continue;
                    }
                    if obj_guard
                        .with_special_power_module_interface_by_name(&power_name, |module| {
                            module.get_percent_ready() >= 1.0
                        })
                        .unwrap_or(false)
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            // Skirmish: total value of player's units inside an area meets comparison
            ConditionType::SkirmishValueInArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing comparison parameter".to_string(),
                    )
                })?;
                let value_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing value parameter".to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing trigger parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_value = value_param.get_int();
                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut total_cost = 0i32;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_kind_of(KindOf::Inert) {
                        continue;
                    }
                    if !obj_guard.is_effectively_dead() && obj_guard.is_inside_trigger(&trigger) {
                        total_cost += obj_guard.get_template().get_build_cost();
                    }
                }

                match comparison {
                    0 => Ok(total_cost < target_value),
                    1 => Ok(total_cost <= target_value),
                    2 => Ok(total_cost == target_value),
                    3 => Ok(total_cost >= target_value),
                    4 => Ok(total_cost > target_value),
                    5 => Ok(total_cost != target_value),
                    _ => Ok(false),
                }
            }

            // Skirmish: player is a specific faction
            ConditionType::SkirmishPlayerFaction => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerFaction condition missing player parameter".to_string(),
                    )
                })?;
                let faction_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerFaction condition missing faction parameter".to_string(),
                    )
                })?;

                let faction_name = faction_param.get_string();
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                Ok(player_guard.get_side() == faction_name)
            }

            // Skirmish: supplies value within distance of a location meets threshold
            ConditionType::SkirmishSuppliesValueWithinDistance => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let distance_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing distance parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing trigger parameter"
                            .to_string(),
                    )
                })?;
                let threshold_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing threshold parameter"
                            .to_string(),
                    )
                })?;

                let distance = distance_param.get_real();
                let area_name = trigger_param.get_string();
                let threshold = threshold_param.get_real();

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let Some(trigger) = self.get_trigger_area(area_name) else {
                    return Ok(false);
                };
                let Some(partition) = crate::helpers::ThePartitionManager::get() else {
                    return Ok(false);
                };

                let center = trigger.get_center_point();
                let radius = trigger.get_radius() + distance;
                let supply_box_value = player_guard.get_supply_box_value() as f32;
                let mut max_value = 0.0f32;

                for obj_id in partition.get_objects_in_range(&center, radius) {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_destroyed() || obj_guard.is_off_map() {
                        continue;
                    }
                    if !obj_guard.is_kind_of(KindOf::Structure) {
                        continue;
                    }

                    let allow_affiliation =
                        if let Some(owner_id) = obj_guard.get_controlling_player_id() {
                            if owner_id == player_guard.get_player_index() as u32 {
                                true
                            } else if let Some(owner_arc) = player_list()
                                .read()
                                .ok()
                                .and_then(|list| list.get_player(owner_id as i32).cloned())
                            {
                                if let Ok(owner_guard) = owner_arc.read() {
                                    player_guard.get_relationship(&owner_guard)
                                        == crate::common::Relationship::Neutral
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                    if !allow_affiliation {
                        continue;
                    }

                    let Some(module) = obj_guard.find_update_module("SupplyWarehouseDockUpdate")
                    else {
                        continue;
                    };
                    let mut boxes = None;
                    module.with_module_downcast::<crate::object::production::SupplyWarehouseDockUpdateModule, _, _>(|m| {
                        boxes = Some(m.behavior().get_boxes_stored());
                    });
                    let Some(boxes) = boxes else {
                        continue;
                    };

                    max_value = max_value.max(supply_box_value * boxes as f32);
                }

                Ok(max_value > threshold)
            }

            // Skirmish: tech building within distance of a location
            // C++: ThePartitionManager->getClosestObject with KindOf::TECH_BUILDING + player filters
            ConditionType::SkirmishTechBuildingWithinDistance => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishTechBuildingWithinDistance condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let distance_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishTechBuildingWithinDistance condition missing distance parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishTechBuildingWithinDistance condition missing trigger parameter"
                            .to_string(),
                    )
                })?;

                let distance = distance_param.get_real();
                let area_name = trigger_param.get_string();

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let player_index = player_guard.get_player_index();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let center = trigger.get_center_point();
                let radius = trigger.get_radius() + distance;

                let Some(partition) = crate::helpers::ThePartitionManager::get() else {
                    return Ok(false);
                };

                for obj_id in partition.get_objects_in_range(&center, radius) {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_destroyed() || obj_guard.is_off_map() {
                        continue;
                    }
                    if !obj_guard.is_kind_of(KindOf::TechBuilding) {
                        continue;
                    }

                    let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                        continue;
                    };
                    if owner_id == player_index as u32 {
                        continue;
                    }
                    if let Some(owner_arc) = player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(owner_id as i32).cloned())
                    {
                        if let Ok(owner_guard) = owner_arc.read() {
                            if !player_guard.is_allied_with_player(&owner_guard) {
                                continue;
                            }
                        }
                    }

                    return Ok(true);
                }
                Ok(false)
            }

            // Skirmish: all team members have command button ready
            ConditionType::SkirmishCommandButtonReadyAll => {
                let team_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyAll condition missing team parameter"
                            .to_string(),
                    )
                })?;
                let button_name_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyAll condition missing button name parameter"
                            .to_string(),
                    )
                })?;

                let button_name = button_name_param.get_string();
                let Some(bridge) = crate::control_bar::get_control_bar_bridge() else {
                    return Ok(false);
                };
                let Some(_button) = bridge.find_command_button_by_name(&button_name) else {
                    return Ok(false);
                };

                let team_name = self.resolve_team_name_token(team_param.get_string());
                let team_instances = self.resolve_team_instances(&team_name);
                if team_instances.is_empty() {
                    return Ok(false);
                }

                let mut all_ready = true;
                'outer: for team_arc in &team_instances {
                    let Ok(team_guard) = team_arc.read() else {
                        all_ready = false;
                        break;
                    };
                    for &member_id in team_guard.get_members() {
                        let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            all_ready = false;
                            break 'outer;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            all_ready = false;
                            break 'outer;
                        };
                        if !obj_guard.is_destroyed() && _button.is_ready(&obj_guard) {
                            continue;
                        }
                        all_ready = false;
                        break 'outer;
                    }
                }
                Ok(all_ready)
            }

            // Skirmish: any team member has command button ready
            ConditionType::SkirmishCommandButtonReadyPartial => {
                let team_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyPartial condition missing team parameter"
                            .to_string(),
                    )
                })?;
                let button_name_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyPartial condition missing button name parameter"
                            .to_string(),
                    )
                })?;

                let button_name = button_name_param.get_string();
                let Some(bridge) = crate::control_bar::get_control_bar_bridge() else {
                    return Ok(false);
                };
                let Some(_button) = bridge.find_command_button_by_name(&button_name) else {
                    return Ok(false);
                };

                let team_name = self.resolve_team_name_token(team_param.get_string());
                for team_arc in self.resolve_team_instances(&team_name) {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    for &member_id in team_guard.get_members() {
                        let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            continue;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            continue;
                        };
                        if !obj_guard.is_destroyed() && _button.is_ready(&obj_guard) {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }

            // Skirmish: unowned (neutral) faction unit count meets comparison
            ConditionType::SkirmishUnownedFactionUnitExists => {
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishUnownedFactionUnitExists condition missing comparison parameter"
                            .to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishUnownedFactionUnitExists condition missing count parameter"
                            .to_string(),
                    )
                })?;

                // C++ counts neutral player objects with DISABLED_UNMANNED
                let Ok(list) = player_list().read() else {
                    return Ok(false);
                };
                let neutral_player = list.get_neutral_player();
                let Some(neutral_arc) = neutral_player else {
                    return Ok(false);
                };
                let Ok(neutral_guard) = neutral_arc.read() else {
                    return Ok(false);
                };

                let mut num_faction_units = 0i32;
                for obj_arc in neutral_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_disabled_by_type(DisabledType::Unmanned) {
                        num_faction_units += 1;
                    }
                }

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                match comparison {
                    0 => Ok(num_faction_units < target_count),
                    1 => Ok(num_faction_units <= target_count),
                    2 => Ok(num_faction_units == target_count),
                    3 => Ok(num_faction_units >= target_count),
                    4 => Ok(num_faction_units > target_count),
                    5 => Ok(num_faction_units != target_count),
                    _ => Ok(false),
                }
            }

            // Skirmish: player has prerequisites to build a specific object type
            // C++: types.m_types->canBuildAny(player)
            ConditionType::SkirmishPlayerHasPrerequisiteToBuild => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasPrerequisiteToBuild condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasPrerequisiteToBuild condition missing type parameter"
                            .to_string(),
                    )
                })?;

                let types = self.resolve_object_types(type_param);

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                Ok(types.can_build_any(&player_guard))
            }

            // Skirmish: player's garrisoned building count meets comparison
            ConditionType::SkirmishPlayerHasComparisonGarrisoned => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonGarrisoned condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonGarrisoned condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonGarrisoned condition missing count parameter"
                            .to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                // C++ counts buildings with ContainModuleInterface::isGarrisonable() && getContainCount() > 0
                let mut num_garrisoned = 0i32;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let Some(contain) = obj_guard.get_contain() else {
                        continue;
                    };
                    let Ok(contain_guard) = contain.lock() else {
                        continue;
                    };
                    if contain_guard.is_garrisonable() && contain_guard.get_contained_count() > 0 {
                        num_garrisoned += 1;
                    }
                }

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                match comparison {
                    0 => Ok(num_garrisoned < target_count),
                    1 => Ok(num_garrisoned <= target_count),
                    2 => Ok(num_garrisoned == target_count),
                    3 => Ok(num_garrisoned >= target_count),
                    4 => Ok(num_garrisoned > target_count),
                    5 => Ok(num_garrisoned != target_count),
                    _ => Ok(false),
                }
            }

            // Skirmish: player's captured unit count meets comparison
            ConditionType::SkirmishPlayerHasComparisonCapturedUnits => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonCapturedUnits condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonCapturedUnits condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonCapturedUnits condition missing count parameter".to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut num_captured = 0i32;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_captured() {
                        num_captured += 1;
                    }
                }

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                match comparison {
                    0 => Ok(num_captured < target_count),
                    1 => Ok(num_captured <= target_count),
                    2 => Ok(num_captured == target_count),
                    3 => Ok(num_captured >= target_count),
                    4 => Ok(num_captured > target_count),
                    5 => Ok(num_captured != target_count),
                    _ => Ok(false),
                }
            }

            // Skirmish: named trigger area exists on the map
            ConditionType::SkirmishNamedAreaExist => {
                let trigger_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishNamedAreaExist condition missing trigger parameter".to_string(),
                    )
                })?;

                let area_name = trigger_param.get_string();
                Ok(self.get_trigger_area(area_name).is_some())
            }

            // Skirmish: player has units inside a trigger area
            ConditionType::SkirmishPlayerHasUnitsInArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasUnitsInArea condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasUnitsInArea condition missing trigger parameter"
                            .to_string(),
                    )
                })?;

                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut count = 0;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_inside_trigger(&trigger) {
                        if !obj_guard.is_effectively_dead()
                            && !obj_guard.is_kind_of(KindOf::Inert)
                            && !obj_guard.is_kind_of(KindOf::Projectile)
                        {
                            count += 1;
                        }
                    }
                }

                Ok(count > 0)
            }

            // Skirmish: player has been attacked by another player
            ConditionType::SkirmishPlayerHasBeenAttackedByPlayer => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasBeenAttackedByPlayer condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let attacker_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasBeenAttackedByPlayer condition missing attacker parameter".to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let Some(attacker_arc) = self.resolve_player_from_param(attacker_param) else {
                    return Ok(false);
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    return Ok(false);
                };

                Ok(player_guard.get_attacked_by(attacker_guard.get_player_index()))
            }

            // Skirmish: player has no units inside a trigger area
            ConditionType::SkirmishPlayerIsOutsideArea => {
                // C++: !evaluateSkirmishPlayerHasUnitsInArea

                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerIsOutsideArea condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerIsOutsideArea condition missing trigger parameter"
                            .to_string(),
                    )
                })?;

                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(true), // No trigger = no units inside = outside
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(true);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(true);
                };

                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_inside_trigger(&trigger) {
                        if !obj_guard.is_effectively_dead()
                            && !obj_guard.is_kind_of(KindOf::Inert)
                            && !obj_guard.is_kind_of(KindOf::Projectile)
                        {
                            return Ok(false); // Found a unit inside = not outside
                        }
                    }
                }

                Ok(true)
            }

            // Skirmish: player has discovered another player's units
            ConditionType::SkirmishPlayerHasDiscoveredPlayer => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasDiscoveredPlayer condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let discovered_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasDiscoveredPlayer condition missing discovered-by parameter".to_string(),
                    )
                })?;

                let Some(discovered_by_arc) = self.resolve_player_from_param(discovered_param)
                else {
                    return Ok(false);
                };
                let Ok(discovered_by_guard) = discovered_by_arc.read() else {
                    return Ok(false);
                };
                let discovered_by_index = discovered_by_guard.get_player_index() as i32;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                // C++: iterates player objects checking shroud status against discoveredByIndex
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let shroud = obj_guard.get_shrouded_status(discovered_by_index);
                    if matches!(
                        shroud,
                        ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
                    ) {
                        return Ok(true);
                    }
                }

                Ok(false)
            }

            // Music track has completed playback
            // C++: TheAudio->hasMusicTrackCompleted(str, param)
            ConditionType::MusicTrackHasCompleted => {
                let music_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "MusicTrackHasCompleted condition missing music parameter".to_string(),
                    )
                })?;
                let int_param = condition.get_parameter(1);

                let track_name = music_param.get_string();
                let param = int_param.map(|p| p.get_int()).unwrap_or(0);

                let engine = self.engine.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
                })?;
                Ok(engine
                    .as_ref()
                    .and_then(|e| e.action_handler())
                    .map(|h| h.has_music_track_completed(&track_name, param))
                    .unwrap_or(false))
            }

            // Player lost all objects of a specific type (had them before, now fewer)
            ConditionType::PlayerLostObjectType => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerLostObjectType condition missing player parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerLostObjectType condition missing type parameter".to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let player_index = player_arc.read().ok().map(|p| p.get_player_index() as i32);
                let Some(player_index) = player_index else {
                    return Ok(false);
                };

                let type_name = type_param.get_string();
                let types = self.resolve_object_types(type_param);

                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut current_count = 0i32;
                for obj_arc in player_guard.get_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if !obj_guard.is_destroyed()
                        && types.contains_template(Some(obj_guard.get_template()))
                    {
                        current_count += 1;
                    }
                }

                // C++ compares current count to previously stored count via ScriptEngine
                let stored_count = if let Ok(engine_guard) = get_script_engine().read() {
                    engine_guard
                        .as_ref()
                        .map(|e| e.get_object_count(player_index, type_name))
                        .unwrap_or(current_count)
                } else {
                    current_count
                };

                if let Ok(mut engine_guard) = get_script_engine().write() {
                    if let Some(engine) = engine_guard.as_mut() {
                        engine.set_object_count(player_index, type_name, current_count);
                    }
                }

                Ok(current_count < stored_count)
            }

            // Skirmish: player's supply source is safe (above minimum amount)
            ConditionType::SupplySourceSafe => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SupplySourceSafe condition missing player parameter".to_string(),
                    )
                })?;
                let min_param = condition.get_parameter(1);

                let player_arc = self.resolve_player_from_param(player_param);
                let Some(player_arc) = player_arc else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let player_id = player_guard.get_player_index() as u32;
                let min_supplies = min_param.as_ref().map(|p| p.get_int()).unwrap_or(0) as i32;

                let safe = crate::ai::integration::with_ai_integration(|manager| {
                    manager.with_ai_player(player_id, |ai| ai.is_supply_source_safe(min_supplies))
                })
                .flatten()
                .unwrap_or(false);
                Ok(safe)
            }

            // Skirmish: player's supply source is under attack
            ConditionType::SupplySourceAttacked => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SupplySourceAttacked condition missing player parameter".to_string(),
                    )
                })?;

                let player_arc = self.resolve_player_from_param(player_param);
                let Some(player_arc) = player_arc else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let player_id = player_guard.get_player_index() as u32;

                let attacked = crate::ai::integration::with_ai_integration(|manager| {
                    manager.with_ai_player(player_id, |ai| ai.is_supply_source_attacked())
                })
                .flatten()
                .unwrap_or(false);
                Ok(attacked)
            }

            // Skirmish: player's start position matches a specific index
            ConditionType::StartPositionIs => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "StartPositionIs condition missing player parameter".to_string(),
                    )
                })?;
                let start_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "StartPositionIs condition missing start index parameter".to_string(),
                    )
                })?;

                // C++: ndx = pStartNdx->getInt()-1 (externally 1-based, internally 0-based)
                let ndx = start_param.get_int() - 1;
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                Ok(player_guard.get_mp_start_index() == ndx)
            }

            _ => {
                let ctx = self.make_script_context();
                let mut evaluator = ScriptConditionEvaluator::new(ctx);
                match evaluator.evaluate_condition(condition) {
                    Ok(ScriptConditionResult::True) => Ok(true),
                    Ok(ScriptConditionResult::False) => Ok(false),
                    Ok(ScriptConditionResult::Error(msg)) => Err(GameLogicError::Configuration(
                        format!("Script condition evaluation error: {}", msg),
                    )),
                    Err(err) => Err(GameLogicError::Configuration(format!(
                        "Script condition evaluation failed: {}",
                        err
                    ))),
                }
            }
        };
        let elapsed = eval_started.elapsed();
        if elapsed >= std::time::Duration::from_millis(SLOW_SCRIPT_CONDITION_WARN_MS) {
            log::warn!(
                "Slow script condition evaluate: {:?} took {:?}",
                condition_type,
                elapsed
            );
        }
        result
    }

    fn resolve_team_name_token(&self, raw: &str) -> String {
        match raw {
            THIS_TEAM => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref().and_then(|e| {
                        e.get_condition_team_name()
                            .or_else(|| e.get_calling_team_name())
                            .map(|s| s.to_string())
                    })
                })
                .unwrap_or_else(|| raw.to_string()),
            TEAM_THE_PLAYER => {
                let current_player = get_script_engine().read().ok().and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                });
                let Some(player_name) = current_player else {
                    return raw.to_string();
                };

                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(&player_name))
                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                    .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                    .unwrap_or_else(|| raw.to_string())
            }
            _ => raw.to_string(),
        }
    }

    fn resolve_team_instances(&self, team_name: &str) -> Vec<Arc<RwLock<crate::team::Team>>> {
        let Ok(mut factory) = get_team_factory().lock() else {
            return Vec::new();
        };

        if let Some(team_arc) = factory.find_team(team_name) {
            return vec![team_arc];
        }

        factory.find_team_instances(team_name)
    }

    fn resolve_object_types(&self, param: &Parameter) -> ObjectTypes {
        let type_name = param.get_string();
        let mut types = ObjectTypes::new();
        if type_name.is_empty() {
            return types;
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(found) = engine.get_object_types(type_name) {
                    return found;
                }
            }
        }

        types.add_object_type(AsciiString::from(type_name));
        types
    }

    fn resolve_player_from_param(
        &self,
        param: &Parameter,
    ) -> Option<Arc<RwLock<crate::player::Player>>> {
        let mask_bits = param.get_int() as u32;
        if param.get_parameter_type() == ParameterType::Side && mask_bits != 0 {
            let mask = PlayerMaskType::from_bits_truncate(mask_bits);
            if let Ok(list) = player_list().read() {
                for player_arc in list.iter() {
                    let Ok(player_guard) = player_arc.read() else {
                        continue;
                    };
                    if mask.intersects(player_guard.get_player_mask()) {
                        return Some(Arc::clone(player_arc));
                    }
                }
            }
        }

        let raw = param.get_string();
        let resolved = match raw {
            THE_PLAYER | THIS_PLAYER => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                })
                .unwrap_or_else(|| raw.to_string()),
            LOCAL_PLAYER => player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| {
                    p.read()
                        .ok()
                        .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                })
                .unwrap_or_else(|| raw.to_string()),
            _ => raw.to_string(),
        };

        if resolved.is_empty() {
            return None;
        }
        player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&resolved))
    }

    fn get_trigger_area(&self, area_name: &str) -> Option<PolygonTrigger> {
        let Ok(terrain) = get_terrain_logic().read() else {
            return None;
        };
        terrain.get_trigger_area_by_name(area_name).cloned()
    }

    fn is_object_considerable(obj: &crate::object::Object) -> bool {
        if obj.is_effectively_dead() {
            return false;
        }
        !obj.is_kind_of(KindOf::Inert)
    }

    fn is_object_inside_trigger(obj: &crate::object::Object, trigger: &PolygonTrigger) -> bool {
        let pos = obj.get_position();
        let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
        trigger.point_in_trigger_int(&point)
    }

    fn kind_of_type_to_mask(kind_of_type_int: i32) -> Option<KindOf> {
        match kind_of_type_int {
            0 => Some(KindOf::Obstacle),
            1 => Some(KindOf::Selectable),
            2 => Some(KindOf::Immobile),
            3 => Some(KindOf::CanAttack),
            4 => Some(KindOf::StickToTerrainSlope),
            5 => Some(KindOf::CanCastReflections),
            6 => Some(KindOf::Shrubbery),
            7 => Some(KindOf::Structure),
            8 => Some(KindOf::Infantry),
            9 => Some(KindOf::Vehicle),
            10 => Some(KindOf::Aircraft),
            11 => Some(KindOf::HugeVehicle),
            12 => Some(KindOf::Dozer),
            13 => Some(KindOf::Harvester),
            14 => Some(KindOf::CommandCenter),
            15 => Some(KindOf::Prison),
            16 => Some(KindOf::CollectsPrisonBounty),
            17 => Some(KindOf::PowTruck),
            18 => Some(KindOf::LineBuild),
            19 => Some(KindOf::Salvager),
            20 => Some(KindOf::WeaponSalvager),
            21 => Some(KindOf::Transport),
            22 => Some(KindOf::Bridge),
            23 => Some(KindOf::LandmarkBridge),
            24 => Some(KindOf::BridgeTower),
            25 => Some(KindOf::Projectile),
            26 => Some(KindOf::Preload),
            27 => Some(KindOf::NoGarrison),
            28 => Some(KindOf::WaveGuide),
            29 => Some(KindOf::WaveEffect),
            30 => Some(KindOf::NoCollide),
            31 => Some(KindOf::RepairPad),
            32 => Some(KindOf::HealPad),
            33 => Some(KindOf::StealthGarrison),
            34 => Some(KindOf::CashGenerator),
            35 => Some(KindOf::DrawableOnly),
            36 => Some(KindOf::CountsForVictory),
            37 => Some(KindOf::RebuildHole),
            38 => Some(KindOf::Score),
            39 => Some(KindOf::ScoreCreate),
            40 => Some(KindOf::ScoreDestroy),
            41 => Some(KindOf::NoHealIcon),
            42 => Some(KindOf::CanRappel),
            43 => Some(KindOf::Parachutable),
            44 => Some(KindOf::CanSurrender),
            45 => Some(KindOf::CanBeRepulsed),
            46 => Some(KindOf::MobNexus),
            47 => Some(KindOf::IgnoredInGui),
            48 => Some(KindOf::Crate),
            49 => Some(KindOf::Capturable),
            50 => Some(KindOf::ClearedByBuild),
            51 => Some(KindOf::SmallMissile),
            52 => Some(KindOf::AlwaysVisible),
            53 => Some(KindOf::Unattackable),
            54 => Some(KindOf::Mine),
            55 => Some(KindOf::CleanupHazard),
            56 => Some(KindOf::PortableStructure),
            57 => Some(KindOf::AlwaysSelectable),
            58 => Some(KindOf::AttackNeedsLineOfSight),
            59 => Some(KindOf::WalkOnTopOfWall),
            60 => Some(KindOf::DefensiveWall),
            61 => Some(KindOf::FSPower),
            64 => Some(KindOf::FSTechnology),
            65 => Some(KindOf::AircraftPathAround),
            66 => Some(KindOf::LowOverlappable),
            67 => Some(KindOf::ForceAttackable),
            68 => Some(KindOf::AutoRallypoint),
            69 => Some(KindOf::TechBuilding),
            70 => Some(KindOf::Powered),
            71 => Some(KindOf::ProducedAtHelipad),
            72 => Some(KindOf::Drone),
            74 => Some(KindOf::BallisticMissile),
            75 => Some(KindOf::ClickThrough),
            76 => Some(KindOf::SupplySourceOnPreview),
            78 => Some(KindOf::GarrisonableUntilDestroyed),
            79 => Some(KindOf::Boat),
            80 => Some(KindOf::ImmuneToCapture),
            81 => Some(KindOf::Hulk),
            82 => Some(KindOf::ShowPortraitWhenControlled),
            83 => Some(KindOf::SpawnsAreTheWeapons),
            84 => Some(KindOf::CannotBuildNearSupplies),
            85 => Some(KindOf::SupplySource),
            86 => Some(KindOf::RevealToAll),
            87 => Some(KindOf::Disguiser),
            88 => Some(KindOf::Inert),
            89 => Some(KindOf::Hero),
            90 => Some(KindOf::IgnoresSelectAll),
            91 => Some(KindOf::DontAutoCrushInfantry),
            92 => Some(KindOf::CliffJumper),
            93 => Some(KindOf::FSSupplyDropzone),
            94 => Some(KindOf::FSSuperweapon),
            95 => Some(KindOf::FsBlackMarket),
            96 => Some(KindOf::FSSupplyCenter),
            97 => Some(KindOf::FSStrategyCenter),
            98 => Some(KindOf::MoneyHacker),
            99 => Some(KindOf::ArmorSalvager),
            100 => Some(KindOf::RevealsEnemyPaths),
            101 => Some(KindOf::BoobyTrap),
            102 => Some(KindOf::FSFake),
            103 => Some(KindOf::FSInternetCenter),
            104 => Some(KindOf::BlastCrater),
            _ => None,
        }
    }

    fn evaluate_named_inside_area_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedInsideArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedInsideArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        if !Self::is_object_considerable(&obj_guard) {
            return Ok(false);
        }

        Ok(Self::is_object_inside_trigger(&obj_guard, &trigger))
    }

    fn evaluate_named_outside_area_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_named_inside_area_condition(condition)?)
    }

    fn evaluate_team_inside_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.some_inside_some_outside(&trigger, which_to_consider)
                || team_guard.all_inside(&trigger, which_to_consider)
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_inside_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.all_inside(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_outside_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !(team_guard.all_inside(&trigger, which_to_consider)
                || team_guard.some_inside_some_outside(&trigger, which_to_consider))
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_named_entered_area_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedEnteredArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedEnteredArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let current_frame = TheGameLogic::get_frame() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }

        Ok(area_tracker
            .get_last_enter_frame(area_name, object_id)
            .map(|frame| frame == current_frame)
            .unwrap_or(false))
    }

    fn evaluate_named_exited_area_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedExitedArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedExitedArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let current_frame = TheGameLogic::get_frame() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }

        Ok(area_tracker
            .get_last_exit_frame(area_name, object_id)
            .map(|frame| frame == current_frame)
            .unwrap_or(false))
    }

    fn evaluate_team_entered_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !team_guard.did_enter_or_exit() {
                continue;
            }

            if team_guard.did_all_enter(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_entered_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.did_partial_enter(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_exited_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !team_guard.did_enter_or_exit() {
                continue;
            }

            if team_guard.did_all_exit(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_exited_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.did_partial_exit(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Evaluate counter condition
    fn evaluate_counter_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let counter_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing counter parameter".to_string())
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "Counter condition missing comparison parameter".to_string(),
            )
        })?;
        let value_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing value parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let target_value = value_param.get_int();

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(counter) = engine.get_counter(counter_name) {
            let current_value = counter.value;
            match comparison {
                0 => Ok(current_value < target_value),  // LessThan
                1 => Ok(current_value <= target_value), // LessEqual
                2 => Ok(current_value == target_value), // Equal
                3 => Ok(current_value >= target_value), // GreaterEqual
                4 => Ok(current_value > target_value),  // Greater
                5 => Ok(current_value != target_value), // NotEqual
                _ => Err(GameLogicError::Configuration(format!(
                    "Invalid comparison type: {}",
                    comparison
                ))),
            }
        } else {
            Ok(false) // Counter doesn't exist
        }
    }

    /// Evaluate flag condition
    fn evaluate_flag_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let flag_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing flag parameter".to_string())
        })?;
        let value_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing value parameter".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let target_value = value_param.get_int() != 0;

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(flag) = engine.get_flag(flag_name) {
            Ok(flag.value == target_value)
        } else {
            Ok(false) // Flag doesn't exist
        }
    }

    /// Evaluate timer expired condition
    fn evaluate_timer_expired_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let counter_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Timer condition missing counter parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(counter) = engine.get_counter(counter_name) {
            if !counter.is_countdown_timer {
                return Ok(false);
            }
            Ok(counter.value < 1)
        } else {
            Ok(false) // Timer doesn't exist
        }
    }

    /// Evaluate player all destroyed condition
    fn evaluate_player_all_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAllDestroyed condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!("Evaluating PlayerAllDestroyed for player: {}", player_name);

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let alive = if let Ok(obj_guard) = obj_arc.read() {
                !obj_guard.is_destroyed()
            } else {
                false
            };
            if alive {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Evaluate player all build facilities destroyed condition
    fn evaluate_player_all_buildfacilities_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAllBuildfacilitiesDestroyed condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerAllBuildfacilitiesDestroyed for player: {}",
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !obj_guard.is_destroyed() && is_build_facility(&obj_guard) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Evaluate team destroyed condition
    fn evaluate_team_destroyed_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDestroyed condition missing team parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        log::debug!("Evaluating TeamDestroyed for team: {}", team_name);

        let teams = self.resolve_team_instances(&team_name);
        if teams.is_empty() {
            return Ok(true);
        }

        for team_arc in teams {
            if let Ok(team) = team_arc.read() {
                if team.has_any_objects() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Evaluate team has units condition
    fn evaluate_team_has_units_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasUnits condition missing team parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        log::debug!("Evaluating TeamHasUnits for team: {}", team_name);

        for team_arc in self.resolve_team_instances(&team_name) {
            if let Ok(team) = team_arc.read() {
                if team.has_any_units() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Evaluate named destroyed condition
    fn evaluate_named_destroyed_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDestroyed condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        log::debug!("Evaluating NamedDestroyed for unit: {}", unit_name);

        // Look up the named object using the tracker
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(unit_name) {
            // Check if the object exists and is destroyed
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    return Ok(obj.is_destroyed());
                }
            }
            // Object ID exists but object not found - considered destroyed
            return Ok(true);
        }
        // Object not in tracker - considered destroyed
        Ok(true)
    }

    fn evaluate_named_created_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedCreated condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        Ok(TheGameLogic::find_object_by_id(object_id).is_some())
    }

    fn evaluate_team_created_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamCreated condition missing team parameter".to_string(),
            )
        })?;
        let team_name = self.resolve_team_name_token(team_param.get_string());

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.is_created())
    }

    fn evaluate_team_state_is_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIs condition missing team parameter".to_string(),
            )
        })?;
        let state_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIs condition missing state parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let expected_state = state_param.get_string();

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.get_state().as_str() == expected_state)
    }

    fn evaluate_team_state_is_not_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIsNot condition missing team parameter".to_string(),
            )
        })?;
        let state_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIsNot condition missing state parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let expected_state = state_param.get_string();

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.get_state().as_str() != expected_state)
    }

    fn evaluate_named_attacked_by_object_type_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByObjecttype condition missing unit parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByObjecttype condition missing type parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(body) = obj_guard.get_body_module() else {
            return Ok(false);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(false);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(false);
        };

        let types = self.resolve_object_types(type_param);
        if let Some(template) = last.input.source_template.as_deref() {
            return Ok(types.contains_template(Some(template)));
        }

        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(false);
        };
        let Ok(attacker_guard) = attacker_arc.read() else {
            return Ok(false);
        };
        Ok(types.contains_template(Some(attacker_guard.get_template())))
    }

    fn evaluate_team_attacked_by_object_type_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByObjecttype condition missing team parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByObjecttype condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let types = self.resolve_object_types(type_param);

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            for member_id in team_guard.get_members() {
                let Some(member_arc) = TheGameLogic::find_object_by_id(*member_id) else {
                    continue;
                };
                let Ok(member_guard) = member_arc.read() else {
                    continue;
                };
                let Some(body) = member_guard.get_body_module() else {
                    continue;
                };
                let Ok(body_guard) = body.lock() else {
                    continue;
                };
                let Some(last) = body_guard.get_last_damage_info() else {
                    continue;
                };

                if let Some(template) = last.input.source_template.as_deref() {
                    if types.contains_template(Some(template)) {
                        return Ok(true);
                    }
                    continue;
                }

                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    continue;
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    continue;
                };
                if types.contains_template(Some(attacker_guard.get_template())) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_named_attacked_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByPlayer condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(body) = obj_guard.get_body_module() else {
            return Ok(false);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(false);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(false);
        };

        let target_player = self.resolve_player_from_param(player_param);
        if target_player.is_none() {
            return Ok(false);
        }

        if last.input.source_player_mask != PlayerMaskType::none() {
            if let Some(target_player) = target_player.as_ref() {
                if let Ok(target_guard) = target_player.read() {
                    if last
                        .input
                        .source_player_mask
                        .intersects(target_guard.get_player_mask())
                    {
                        return Ok(true);
                    }
                }
            }
        }

        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(false);
        };
        let Ok(attacker_guard) = attacker_arc.read() else {
            return Ok(false);
        };
        let Some(attacker_player) = attacker_guard.get_controlling_player() else {
            return Ok(false);
        };
        let Some(target_player) = target_player else {
            return Ok(false);
        };
        Ok(Arc::ptr_eq(&attacker_player, &target_player))
    }

    fn evaluate_team_attacked_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByPlayer condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let target_player = self.resolve_player_from_param(player_param);
        if target_player.is_none() {
            return Ok(false);
        }

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            for member_id in team_guard.get_members() {
                let Some(member_arc) = TheGameLogic::find_object_by_id(*member_id) else {
                    continue;
                };
                let Ok(member_guard) = member_arc.read() else {
                    continue;
                };
                let Some(body) = member_guard.get_body_module() else {
                    continue;
                };
                let Ok(body_guard) = body.lock() else {
                    continue;
                };
                let Some(last) = body_guard.get_last_damage_info() else {
                    continue;
                };
                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    continue;
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    continue;
                };
                let Some(attacker_player) = attacker_guard.get_controlling_player() else {
                    continue;
                };
                if Arc::ptr_eq(
                    &attacker_player,
                    target_player.as_ref().expect("checked above"),
                ) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_named_dying_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("NamedDying condition missing unit parameter".to_string())
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        if let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                return Ok(false);
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return Ok(false);
            };
            return Ok(obj_guard.is_effectively_dead());
        }

        Ok(false)
    }

    fn evaluate_named_totally_dead_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedTotallyDead condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        if tracker.get_object_id(unit_name).ok().flatten().is_some() {
            return Ok(false);
        }
        Ok(tracker.did_object_exist(unit_name).unwrap_or(false))
    }

    fn evaluate_named_selected_condition(
        &self,
        condition: &mut Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedSelected condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let selection_manager = get_selection_manager();
        let Ok(manager_guard) = selection_manager.read() else {
            return Ok(false);
        };

        let frame_changed = manager_guard.get_frame_selection_changed();
        if condition.custom_data != 0 && condition.custom_frame == frame_changed {
            return Ok(condition.custom_data == 1);
        }

        let mut is_selected = false;
        if let Ok(list) = player_list().read() {
            let local_index = list.get_local_player_index();
            if local_index >= 0 {
                if let Some(selection) = manager_guard.get_player_selection_ref(local_index) {
                    is_selected = selection.is_object_selected(object_id);
                }
            }
        }

        if !is_selected {
            is_selected = manager_guard.is_object_selected_by_any_player(object_id);
        }

        condition.custom_data = if is_selected { 1 } else { -1 };
        condition.custom_frame = frame_changed;
        Ok(is_selected)
    }

    fn evaluate_built_by_player_condition(
        &self,
        condition: &mut Condition,
    ) -> GameLogicResult<bool> {
        let type_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuiltByPlayer condition missing type parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuiltByPlayer condition missing player parameter".to_string(),
            )
        })?;

        if condition.custom_data != 0 {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    if engine.get_frame_object_count_changed() == condition.custom_frame {
                        return Ok(condition.custom_data == 1);
                    }
                }
            }
        }

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let types = self.resolve_object_types(type_param);

        let mut count = 0;
        for obj_arc in player_guard.get_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if types.contains_template(Some(obj_guard.get_template())) {
                count += 1;
            }
        }

        let result = count != 0;
        condition.custom_data = if result { 1 } else { -1 };
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                condition.custom_frame = engine.get_frame_object_count_changed();
            }
        }
        Ok(result)
    }

    fn evaluate_named_building_is_empty_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedBuildingIsEmpty condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };
        Ok(contain_guard.get_contained_count() == 0)
    }

    fn evaluate_building_entered_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuildingEnteredByPlayer condition missing player parameter".to_string(),
            )
        })?;
        let building_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuildingEnteredByPlayer condition missing building parameter".to_string(),
            )
        })?;

        let building_name = building_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(building_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };

        let player_mask = contain_guard.get_player_who_entered();
        if player_mask == PlayerMaskType::none() {
            return Ok(false);
        }

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        Ok(player_mask.intersects(player_guard.get_player_mask()))
    }

    /// Evaluate named not destroyed condition
    fn evaluate_named_not_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_named_destroyed_condition(condition)?)
    }

    fn evaluate_named_discovered_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDiscovered condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDiscovered condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let player_name = player_param.get_string();

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_index = player_arc.read().ok().map(|p| p.get_player_index());
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        if obj_guard.is_disabled_by_type(DisabledType::Held) {
            return Ok(false);
        }

        if obj_guard.test_status(crate::common::ObjectStatusTypes::Stealthed)
            && !obj_guard.test_status(crate::common::ObjectStatusTypes::Detected)
            && !obj_guard.test_status(crate::common::ObjectStatusTypes::Disguised)
        {
            return Ok(false);
        }

        let shroud = obj_guard.get_shrouded_status(player_index as i32);
        Ok(matches!(
            shroud,
            ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
        ))
    }

    fn evaluate_team_discovered_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDiscovered condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDiscovered condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let player_name = player_param.get_string();

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_index = player_arc.read().ok().map(|p| p.get_player_index());
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            for &member_id in team_guard.get_members() {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                if obj_guard.is_disabled_by_type(DisabledType::Held) {
                    continue;
                }
                if obj_guard.test_status(crate::common::ObjectStatusTypes::Stealthed)
                    && !obj_guard.test_status(crate::common::ObjectStatusTypes::Detected)
                    && !obj_guard.test_status(crate::common::ObjectStatusTypes::Disguised)
                {
                    continue;
                }

                let shroud = obj_guard.get_shrouded_status(player_index as i32);
                if matches!(
                    shroud,
                    ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
                ) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Evaluate player has credits condition
    fn evaluate_player_has_credits_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let credits_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing credits parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing comparison parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing player parameter".to_string(),
            )
        })?;

        let target_credits = credits_param.get_int();
        let comparison = comparison_param.get_int() as u32;
        let player_name = player_param.get_string();

        log::debug!(
            "Evaluating PlayerHasCredits for player: {} target: {} comparison: {}",
            player_name,
            target_credits,
            comparison
        );

        // Look up the player and get their credits
        let current_credits = if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(player_name) {
                if let Ok(player) = player_arc.read() {
                    player.get_money().get_money()
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        match comparison {
            0 => Ok(current_credits < target_credits),  // LessThan
            1 => Ok(current_credits <= target_credits), // LessEqual
            2 => Ok(current_credits == target_credits), // Equal
            3 => Ok(current_credits >= target_credits), // GreaterEqual
            4 => Ok(current_credits > target_credits),  // Greater
            5 => Ok(current_credits != target_credits), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    /// Evaluate player has power condition
    fn evaluate_player_has_power_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasPower condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!("Evaluating PlayerHasPower for player: {}", player_name);

        // Look up the player and check their power status
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(player_name) {
                if let Ok(player) = player_arc.read() {
                    // Player has power if production >= consumption (not low power)
                    return Ok(!player.get_energy().is_low_power());
                }
            }
        }
        // If player doesn't exist, default to no power
        Ok(false)
    }

    /// Evaluate player has no power condition
    fn evaluate_player_has_no_power_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_player_has_power_condition(condition)?)
    }

    fn evaluate_named_owned_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedOwnedByPlayer condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedOwnedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating NamedOwnedByPlayer for unit: {} player: {}",
            unit_name,
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_id = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as UnsignedInt);

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        Ok(player_id == obj_guard.get_controlling_player_id())
    }

    fn evaluate_team_owned_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOwnedByPlayer condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOwnedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating TeamOwnedByPlayer for team: {} player: {}",
            team_name,
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_id = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as UnsignedInt);

        for team_arc in self.resolve_team_instances(&team_name) {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.get_controlling_player_id() == player_id {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_player_has_n_or_fewer_buildings_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let building_count_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerBuildings condition missing building count parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerBuildings condition missing player parameter".to_string(),
            )
        })?;

        let max_buildings = building_count_param.get_int();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerHasNOrFewerBuildings for player: {}",
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let mut count = 0;
        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                continue;
            }
            if obj_guard.is_kind_of(KindOf::Structure) {
                count += 1;
            }
        }

        Ok(max_buildings >= count)
    }

    fn evaluate_player_has_n_or_fewer_faction_buildings_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let building_count_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerFactionBuildings condition missing building count parameter"
                    .to_string(),
            )
        })?;
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerFactionBuildings condition missing player parameter".to_string(),
            )
        })?;

        let max_buildings = building_count_param.get_int();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerHasNOrFewerFactionBuildings for player: {}",
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let mut count = 0;
        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                continue;
            }
            if obj_guard.is_kind_of(KindOf::Structure)
                && obj_guard.is_kind_of(KindOf::CountsForVictory)
            {
                count += 1;
            }
        }

        Ok(max_buildings >= count)
    }

    fn evaluate_player_power_compare_percent_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing player parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing comparison parameter".to_string(),
            )
        })?;
        let percent_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing percent parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let percent = percent_param.get_int() as f64 / 100.0;

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let ratio = player_guard.get_energy().supply_ratio() as f64;
        match comparison {
            0 => Ok(ratio < percent),  // LessThan
            1 => Ok(ratio <= percent), // LessEqual
            2 => Ok(ratio == percent), // Equal
            3 => Ok(ratio >= percent), // GreaterEqual
            4 => Ok(ratio > percent),  // Greater
            5 => Ok(ratio != percent), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    fn evaluate_player_excess_power_compare_value_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing player parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing comparison parameter".to_string(),
            )
        })?;
        let value_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing value parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let desired_excess = value_param.get_int() as i64;

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let energy = player_guard.get_energy();
        let actual_excess = (energy.production() - energy.consumption()) as i64;

        match comparison {
            0 => Ok(actual_excess < desired_excess),  // LessThan
            1 => Ok(actual_excess <= desired_excess), // LessEqual
            2 => Ok(actual_excess == desired_excess), // Equal
            3 => Ok(actual_excess >= desired_excess), // GreaterEqual
            4 => Ok(actual_excess > desired_excess),  // Greater
            5 => Ok(actual_excess != desired_excess), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    /// Evaluate has finished video condition
    fn evaluate_has_finished_video_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let video_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedVideo condition missing video parameter".to_string(),
            )
        })?;

        let video_name = video_param.get_string();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        Ok(engine.is_video_complete(video_name, true))
    }

    /// Evaluate has finished speech condition
    fn evaluate_has_finished_speech_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let speech_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedSpeech condition missing speech parameter".to_string(),
            )
        })?;

        let speech_name = speech_param.get_string();
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;
        Ok(engine.is_speech_complete(speech_name, true))
    }

    /// Evaluate has finished audio condition
    fn evaluate_has_finished_audio_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let audio_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedAudio condition missing audio parameter".to_string(),
            )
        })?;

        let audio_name = audio_param.get_string();
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;
        Ok(engine.is_audio_complete(audio_name, true))
    }

    fn evaluate_unit_health_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("UnitHealth condition missing unit parameter".to_string())
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHealth condition missing comparison parameter".to_string(),
            )
        })?;
        let health_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHealth condition missing health parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let target_percent = health_param.get_int() as i64;

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        let max_health = obj_guard.get_max_health();
        if max_health <= f32::EPSILON {
            return Ok(false);
        }
        let cur_health = obj_guard.get_health();
        let cur_percent = ((cur_health * 100.0) + (max_health / 2.0)) / max_health;
        let cur_percent = cur_percent.round() as i64;

        match comparison {
            0 => Ok(cur_percent < target_percent),  // LessThan
            1 => Ok(cur_percent <= target_percent), // LessEqual
            2 => Ok(cur_percent == target_percent), // Equal
            3 => Ok(cur_percent >= target_percent), // GreaterEqual
            4 => Ok(cur_percent > target_percent),  // Greater
            5 => Ok(cur_percent != target_percent), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    fn evaluate_unit_has_object_status_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHasObjectStatus condition missing unit parameter".to_string(),
            )
        })?;
        let status_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitHasObjectStatus condition missing status parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let status_mask = status_param.get_object_status();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        Ok(obj_guard.get_status_bits().intersects(status_mask))
    }

    fn evaluate_team_has_object_status_condition(
        &self,
        condition: &Condition,
        entire_team: bool,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasObjectStatus condition missing team parameter".to_string(),
            )
        })?;
        let status_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasObjectStatus condition missing status parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let status_mask = status_param.get_object_status();

        let teams = self.resolve_team_instances(&team_name);
        if teams.is_empty() {
            return Ok(false);
        }

        for team_arc in teams {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            for &member_id in team_guard.get_members() {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                let has_status = obj_guard.get_status_bits().intersects(status_mask);
                if entire_team && !has_status {
                    return Ok(false);
                } else if !entire_team && has_status {
                    return Ok(true);
                }
            }
        }

        Ok(entire_team)
    }

    fn evaluate_player_acquired_science_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAcquiredScience condition missing player parameter".to_string(),
            )
        })?;
        let science_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAcquiredScience condition missing science parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let science_name = science_param.get_string();

        let science = if let Some(science_store) = get_science_store() {
            science_store.get_science_from_internal_name(science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as usize);
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        Ok(engine.is_science_acquired(player_index, science, true))
    }

    fn evaluate_player_has_science_purchase_points_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasSciencepurchasepoints condition missing player parameter".to_string(),
            )
        })?;
        let points_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasSciencepurchasepoints condition missing points parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let points_needed = points_param.get_int();

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        Ok(player_guard.get_science_purchase_points() >= points_needed)
    }

    fn evaluate_player_can_purchase_science_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerCanPurchaseScience condition missing player parameter".to_string(),
            )
        })?;
        let science_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerCanPurchaseScience condition missing science parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let science_name = science_param.get_string();

        let science = if let Some(science_store) = get_science_store() {
            science_store.get_science_from_internal_name(science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        Ok(player_guard.is_capable_of_purchasing_science(science))
    }

    fn evaluate_named_has_free_container_slots_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedHasFreeContainerSlots condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };

        Ok(contain_guard.get_contained_count() < contain_guard.get_max_capacity())
    }

    fn evaluate_unit_emptied_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "UnitEmptied condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };

        let num_peeps = obj_guard
            .get_contain()
            .and_then(|contain| contain.lock().ok().map(|c| c.get_contained_count()))
            .unwrap_or(0);

        let frame = TheGameLogic::get_frame();
        let mut statuses = TRANSPORT_STATUSES.write().map_err(|e| {
            GameLogicError::Threading(format!("Transport status lock error: {}", e))
        })?;

        let entry = statuses.entry(object_id).or_insert((frame, num_peeps));
        if entry.0 == frame.saturating_sub(1) && entry.1 > 0 && num_peeps == 0 {
            return Ok(true);
        }

        *entry = (frame, num_peeps);
        Ok(false)
    }

    /// Execute a sequence of actions
    pub fn execute_action_sequence(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let mut current_action = Some(action);

        while let Some(act) = current_action {
            self.execute_action(act)?;
            current_action = act.get_next();
        }

        Ok(())
    }

    /// Execute a single action matching C++ DoAction
    pub fn execute_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        log::debug!("Executing action: {:?}", action.get_action_type());

        match action.get_action_type() {
            ScriptActionType::NoOp => Ok(()), // Do nothing
            ScriptActionType::Victory => self.execute_victory_action(action),
            ScriptActionType::Defeat => self.execute_defeat_action(action),
            ScriptActionType::SetFlag => self.execute_set_flag_action(action),
            ScriptActionType::SetCounter => self.execute_set_counter_action(action),
            ScriptActionType::IncrementCounter => self.execute_increment_counter_action(action),
            ScriptActionType::DecrementCounter => self.execute_decrement_counter_action(action),
            ScriptActionType::SetTimer => self.execute_set_timer_action(action),
            ScriptActionType::SetMillisecondTimer => {
                self.execute_set_millisecond_timer_action(action)
            }
            ScriptActionType::DisplayText => self.execute_display_text_action(action),
            ScriptActionType::PlaySoundEffect => self.execute_play_sound_effect_action(action),
            ScriptActionType::EnableScript => self.execute_enable_script_action(action),
            ScriptActionType::DisableScript => self.execute_disable_script_action(action),
            ScriptActionType::FreezeTime => self.execute_freeze_time_action(action),
            ScriptActionType::UnfreezeTime => self.execute_unfreeze_time_action(action),
            ScriptActionType::PlayerSetMoney => self.execute_player_set_money_action(action),
            ScriptActionType::PlayerGiveMoney => self.execute_player_give_money_action(action),
            ScriptActionType::Quickvictory => self.execute_quick_victory_action(action),
            _ => {
                let ctx = self.make_script_context();
                let mut dispatcher = ScriptActionDispatcher::new(ctx);
                match dispatcher.execute_action(action) {
                    Ok(ScriptActionResult::Success) => Ok(()),
                    Ok(ScriptActionResult::Pending(_frames)) => Ok(()),
                    Ok(ScriptActionResult::Failed(msg)) => Err(GameLogicError::Configuration(
                        format!("Script action failed: {}", msg),
                    )),
                    Err(err) => Err(GameLogicError::Configuration(format!(
                        "Script action dispatch failed: {}",
                        err
                    ))),
                }
            }
        }
    }

    /// Helper for special power conditions (triggered, midway, complete).
    /// C++ evaluatePlayerSpecialPowerFromUnitTriggered/Midway/Complete with optional named source.
    fn evaluate_special_power_condition(
        &self,
        condition: &Condition,
        midway: bool,
        complete: bool,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "SpecialPower condition missing player parameter".to_string(),
            )
        })?;
        let power_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "SpecialPower condition missing power parameter".to_string(),
            )
        })?;

        let power_name = power_param.get_string();
        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as usize);
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let has_named = condition.get_parameter(2).is_some();
        let mut source_id = crate::common::INVALID_ID;
        if has_named {
            let named_param = condition.get_parameter(2).unwrap();
            let named_name = named_param.get_string();
            let tracker = get_named_object_tracker();
            if let Some(object_id) = tracker.get_object_id(named_name).ok().flatten() {
                if TheGameLogic::find_object_by_id(object_id).is_none() {
                    return Ok(false);
                }
                source_id = object_id;
            } else {
                return Ok(false);
            }
        }

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if midway {
            Ok(engine.is_special_power_midway(player_index, power_name, true, source_id))
        } else if complete {
            Ok(engine.is_special_power_complete(player_index, power_name, true, source_id))
        } else {
            Ok(engine.is_special_power_triggered(player_index, power_name, true, source_id))
        }
    }

    /// Helper for upgrade conditions (built upgrade, built upgrade from named).
    /// C++ evaluateUpgradeFromUnitComplete with optional named source.
    fn evaluate_upgrade_condition(
        &self,
        condition: &Condition,
        from_named: bool,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerBuiltUpgrade condition missing player parameter".to_string(),
            )
        })?;
        let upgrade_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerBuiltUpgrade condition missing upgrade parameter".to_string(),
            )
        })?;

        let upgrade_name = upgrade_param.get_string();
        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as usize);
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let mut source_id = crate::common::INVALID_ID;
        if from_named {
            let named_param = condition.get_parameter(2).ok_or_else(|| {
                GameLogicError::Configuration(
                    "PlayerBuiltUpgradeFromNamed condition missing unit parameter".to_string(),
                )
            })?;
            let named_name = named_param.get_string();
            let tracker = get_named_object_tracker();
            if let Some(object_id) = tracker.get_object_id(named_name).ok().flatten() {
                if TheGameLogic::find_object_by_id(object_id).is_none() {
                    return Ok(false);
                }
                source_id = object_id;
            } else {
                return Ok(false);
            }
        }

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        Ok(engine.is_upgrade_complete(player_index, upgrade_name, true, source_id))
    }

    fn make_script_context(&self) -> Arc<RwLock<ScriptContext>> {
        let mut context = ScriptContext::new();
        context.current_frame = TheGameLogic::get_frame();
        Arc::new(RwLock::new(context))
    }

    fn with_action_handler<F>(&self, f: F) -> GameLogicResult<()>
    where
        F: FnOnce(&dyn ScriptActionHandler) -> GameLogicResult<()>,
    {
        if let Some(handler) = self.get_action_handler()? {
            f(handler.as_ref())
        } else {
            Ok(())
        }
    }

    fn get_action_handler(&self) -> GameLogicResult<Option<Arc<dyn ScriptActionHandler>>> {
        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        Ok(engine.as_ref().and_then(|engine| engine.action_handler()))
    }

    /// Execute victory action
    fn execute_victory_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Victory action executed");

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.start_end_game_timer();
        Ok(())
    }

    /// Execute defeat action
    fn execute_defeat_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Defeat action executed");

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.start_end_game_timer();
        Ok(())
    }

    /// Execute quick victory action
    fn execute_quick_victory_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Quick victory action executed");

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.start_quick_end_game_timer();
        Ok(())
    }

    /// Execute set flag action
    fn execute_set_flag_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let flag_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing flag parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing value parameter".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let flag_value = value_param.get_int() != 0;

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_flag(flag_name, flag_value)?;
        log::debug!("Set flag '{}' to {}", flag_name, flag_value);
        Ok(())
    }

    /// Execute set counter action
    fn execute_set_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing counter parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing value parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let counter_value = value_param.get_int();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_counter(counter_name, counter_value)?;
        log::debug!("Set counter '{}' to {}", counter_name, counter_value);
        Ok(())
    }

    /// Execute increment counter action
    fn execute_increment_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "IncrementCounter action missing counter parameter".to_string(),
            )
        })?;
        let value_param = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(1);

        let counter_name = counter_param.get_string();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        // Get current value and increment
        let current_value = engine
            .get_counter(counter_name)
            .map(|c| c.value)
            .unwrap_or(0);
        engine.set_counter(counter_name, current_value + value_param)?;
        log::debug!(
            "Incremented counter '{}' by {} to {}",
            counter_name,
            value_param,
            current_value + value_param
        );
        Ok(())
    }

    /// Execute decrement counter action
    fn execute_decrement_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DecrementCounter action missing counter parameter".to_string(),
            )
        })?;
        let value_param = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(1);

        let counter_name = counter_param.get_string();

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        // Get current value and decrement
        let current_value = engine
            .get_counter(counter_name)
            .map(|c| c.value)
            .unwrap_or(0);
        engine.set_counter(counter_name, current_value - value_param)?;
        log::debug!(
            "Decremented counter '{}' by {} to {}",
            counter_name,
            value_param,
            current_value - value_param
        );
        Ok(())
    }

    /// Execute set timer action
    fn execute_set_timer_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing counter parameter".to_string())
        })?;
        let seconds_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing seconds parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let seconds = seconds_param.get_int();
        let frames = seconds * 30; // 30 fps typical game framerate

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_counter(counter_name, frames)?;

        // Mark as countdown timer
        let index = engine.allocate_counter(counter_name)?;
        if let Some(counter) = &mut engine.counters[index] {
            counter.is_countdown_timer = true;
        }

        log::debug!(
            "Set timer '{}' to {} seconds ({} frames)",
            counter_name,
            seconds,
            frames
        );
        Ok(())
    }

    /// Execute set millisecond timer action
    fn execute_set_millisecond_timer_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "SetMillisecondTimer action missing counter parameter".to_string(),
            )
        })?;
        let milliseconds_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "SetMillisecondTimer action missing milliseconds parameter".to_string(),
            )
        })?;

        let counter_name = counter_param.get_string();
        let seconds = milliseconds_param.get_real();
        let frames = (seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32).ceil() as i32;

        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.set_counter(counter_name, frames)?;

        // Mark as countdown timer
        let index = engine.allocate_counter(counter_name)?;
        if let Some(counter) = &mut engine.counters[index] {
            counter.is_countdown_timer = true;
        }

        log::debug!(
            "Set millisecond timer '{}' to {} script-seconds ({} frames)",
            counter_name,
            seconds,
            frames
        );
        Ok(())
    }

    /// Execute display text action
    fn execute_display_text_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let text_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("DisplayText action missing text parameter".to_string())
        })?;

        let text = text_param.get_string().to_string();
        self.with_action_handler(|handler| handler.display_text(&text))
    }

    /// Execute play sound effect action
    fn execute_play_sound_effect_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let sound_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlaySoundEffect action missing sound parameter".to_string(),
            )
        })?;

        let sound_name = sound_param.get_string().to_string();
        self.with_action_handler(|handler| handler.play_sound_effect(&sound_name))
    }

    /// Execute enable script action
    fn execute_enable_script_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "EnableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string().to_string();
        self.with_action_handler(|handler| handler.enable_script(&script_name, true))
    }

    /// Execute disable script action
    fn execute_disable_script_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DisableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string().to_string();
        self.with_action_handler(|handler| handler.enable_script(&script_name, false))
    }

    /// Execute freeze time action
    fn execute_freeze_time_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.do_freeze_time();
        Ok(())
    }

    /// Execute unfreeze time action
    fn execute_unfreeze_time_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        let mut engine = self.engine.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_mut().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        engine.do_unfreeze_time();
        Ok(())
    }

    /// Execute player set money action
    fn execute_player_set_money_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let player_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerSetMoney action missing player parameter".to_string(),
            )
        })?;
        let amount_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerSetMoney action missing amount parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let amount = amount_param.get_int();

        log::info!("Set player '{}' money to {}", player_name, amount);

        // In a real implementation, this would set the player's money
        Ok(())
    }

    /// Execute player give money action
    fn execute_player_give_money_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let player_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerGiveMoney action missing player parameter".to_string(),
            )
        })?;
        let amount_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerGiveMoney action missing amount parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let amount = amount_param.get_int();

        log::info!("Give player '{}' {} money", player_name, amount);

        // In a real implementation, this would add money to the player
        Ok(())
    }
}

fn is_build_facility(obj: &crate::object::Object) -> bool {
    obj.is_kind_of(KindOf::Factory)
        || obj.is_kind_of(KindOf::CommandCenter)
        || obj.is_kind_of(KindOf::FSBarracks)
        || obj.is_kind_of(KindOf::FSWarfactory)
        || obj.is_kind_of(KindOf::FSAirfield)
        || obj.is_kind_of(KindOf::FSInternetCenter)
        || obj.is_kind_of(KindOf::FSPower)
        || obj.is_kind_of(KindOf::FSSupplyDropzone)
        || obj.is_kind_of(KindOf::FSSupplyCenter)
        || obj.is_kind_of(KindOf::FSSuperweapon)
        || obj.is_kind_of(KindOf::FSStrategyCenter)
}

#[cfg(test)]
mod tests {
    use super::super::engine::initialize_script_engine;
    use super::*;

    #[tokio::test]
    async fn test_script_evaluator_creation() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine);

        // Create a simple script to test
        let mut script = Script::new();
        script.set_name("test_script".to_string());

        // Should evaluate to true with no conditions
        let result = evaluator.evaluate_script(&mut script).unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_counter_condition() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());

        // Set up a counter
        {
            let mut engine_guard = engine.write().unwrap();
            let engine = engine_guard.as_mut().unwrap();
            engine.set_counter("test_counter", 50).unwrap();
        }

        // Create counter condition: counter >= 40
        let mut condition = Condition::new(ConditionType::Counter);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "test_counter".to_string(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
            .unwrap(); // GreaterEqual
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 40))
            .unwrap();

        let result = evaluator.evaluate_condition(&mut condition).unwrap();
        assert!(result); // 50 >= 40 should be true
    }

    #[tokio::test]
    async fn test_flag_condition() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());

        // Set up a flag
        {
            let mut engine_guard = engine.write().unwrap();
            let engine = engine_guard.as_mut().unwrap();
            engine.set_flag("test_flag", true).unwrap();
        }

        // Create flag condition: flag == true
        let mut condition = Condition::new(ConditionType::Flag);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                "test_flag".to_string(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap(); // true

        let result = evaluator.evaluate_condition(&mut condition).unwrap();
        assert!(result);
    }

    #[test]
    fn test_victory_action() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());

        let action = ScriptAction::new(ScriptActionType::Victory);
        evaluator.execute_action(&action).unwrap();

        // Check that end game timer was started
        let engine_guard = engine.read().unwrap();
        let engine = engine_guard.as_ref().unwrap();
        assert!(engine.is_game_ending());
    }
}
