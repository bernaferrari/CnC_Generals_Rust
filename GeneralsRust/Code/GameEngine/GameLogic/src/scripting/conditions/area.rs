//! Area and location script conditions.

use super::helpers::{
    compare_f64, compare_i64, dual_world_registry_unavailable, event_type_from_name,
    get_player_arc, get_str_param, get_str_param_optional, lookup_named_object_id,
    parse_nested_condition, parse_object_status_mask, perform_comparison, with_script_engine_mut,
};
use super::{ConditionRegistry, ScriptCondition, ScriptContext, ScriptValue};
use crate::common::{Coord3D, KindOf, LOGICFRAMES_PER_SECOND, Relationship};
use crate::helpers::{TheGameLogic, ThePartitionManager, TheVictoryConditions};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::{Player, PlayerType, player_list};
use crate::scripting::engine::{
    get_area_tracker, get_event_manager, get_named_object_tracker, get_script_engine,
};
use crate::scripting::events::{EventFilter, GameEventType};
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::upgrade::center::get_upgrade_center;
use crate::{GameLogicError, GameLogicResult};
use async_trait::async_trait;
use game_engine::common::rts::{SCIENCE_INVALID, get_science_store};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Area clear condition
pub(super) struct AreaClearCondition;

#[async_trait]
impl ScriptCondition for AreaClearCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = crate::scripting::actions::get_float_param(parameters, "x")?;
        let y = crate::scripting::actions::get_float_param(parameters, "y")?;
        let radius = crate::scripting::actions::get_float_param(parameters, "radius")?;
        let exclude_player =
            crate::scripting::actions::get_int_param_optional(parameters, "exclude_player");

        log::debug!(
            "Checking if area ({}, {}) with radius {} is clear",
            x,
            y,
            radius
        );
        if let Some(player) = exclude_player {
            log::debug!("Excluding player {} units from check", player);
        }

        let center = Coord3D::new(x as f32, y as f32, 0.0);
        let radius = radius as f32;
        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(true);
        };

        for object_id in manager.find_objects_in_radius(center, radius) {
            let Some(obj_arc) = manager.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let __base_arc = obj_guard.base();
            let Ok(base_guard) = __base_arc.read() else {
                continue;
            };
            if base_guard.is_destroyed() {
                continue;
            }

            // Restrict to "units" (excluding buildings/structures) to match typical mission scripting usage.
            let Some(template) = obj_guard.template.as_ref() else {
                continue;
            };
            if template.is_kind_of(KindOf::Structure) || template.is_kind_of(KindOf::Building) {
                continue;
            }

            if let Some(player) = exclude_player {
                if base_guard
                    .get_controlling_player_id()
                    .map(|id| id as i64 == player)
                    .unwrap_or(false)
                {
                    continue;
                }
            }

            return Ok(false);
        }

        Ok(true)
    }

    fn name(&self) -> &str {
        "area_clear"
    }

    fn description(&self) -> &str {
        "Checks if an area is clear of units"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string(), "radius".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["exclude_player".to_string()]
    }
}

/// Area controlled by player condition
pub(super) struct AreaControlledByPlayerCondition;

#[async_trait]
impl ScriptCondition for AreaControlledByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let x = crate::scripting::actions::get_float_param(parameters, "x")?;
        let y = crate::scripting::actions::get_float_param(parameters, "y")?;
        let radius = crate::scripting::actions::get_float_param(parameters, "radius")?;

        log::debug!(
            "Checking if player {} controls area ({}, {}) with radius {}",
            player,
            x,
            y,
            radius
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(player_id as i32) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let center = Coord3D::new(x as f32, y as f32, 0.0);
        let radius = radius as f32;
        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };

        let mut saw_friendly = false;
        for object_id in manager.find_objects_in_radius(center, radius) {
            let Some(obj_arc) = manager.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let __base_arc = obj_guard.base();
            let Ok(base_guard) = __base_arc.read() else {
                continue;
            };
            if base_guard.is_destroyed() {
                continue;
            }

            let Some(owner_team) = base_guard.get_team() else {
                continue;
            };
            let Ok(owner_team_guard) = owner_team.read() else {
                continue;
            };
            let rel = player_guard.get_relationship_with_team(&owner_team_guard);
            match rel {
                crate::common::Relationship::Enemies | crate::common::Relationship::Neutral => {
                    return Ok(false);
                }
                _ => {}
            }

            if base_guard.get_controlling_player_id() == Some(player_id) {
                saw_friendly = true;
            }
        }

        Ok(saw_friendly)
    }

    fn name(&self) -> &str {
        "area_controlled_by_player"
    }

    fn description(&self) -> &str {
        "Checks if a player controls an area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Units in area condition
pub(super) struct UnitsInAreaCondition;

#[async_trait]
impl ScriptCondition for UnitsInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = crate::scripting::actions::get_float_param(parameters, "x")?;
        let y = crate::scripting::actions::get_float_param(parameters, "y")?;
        let radius = crate::scripting::actions::get_float_param(parameters, "radius")?;
        let comparison = crate::scripting::actions::get_string_param(parameters, "comparison")?;
        let count = crate::scripting::actions::get_int_param(parameters, "count")?;
        let _player = crate::scripting::actions::get_int_param_optional(parameters, "player");
        let _unit_type = parameters.get("unit_type");

        log::debug!(
            "Checking units in area ({}, {}) with radius {}",
            x,
            y,
            radius
        );

        let center = Coord3D::new(x as f32, y as f32, 0.0);
        let radius = radius as f32;
        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };

        let object_ids = manager.find_objects_in_radius(center, radius);
        let mut actual_count = 0i64;
        for object_id in object_ids {
            let Some(obj_arc) = manager.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let __base_arc = obj_guard.base();
            let Ok(base_guard) = __base_arc.read() else {
                continue;
            };
            if base_guard.is_destroyed() {
                continue;
            }

            if let Some(player) = _player {
                if let Some(owner) = base_guard.get_controlling_player_id() {
                    if owner as i64 != player {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if let Some(ScriptValue::String(unit_type)) = _unit_type {
                let Some(template) = obj_guard.template.as_ref() else {
                    continue;
                };
                if !template.get_name().as_str().eq_ignore_ascii_case(unit_type) {
                    continue;
                }
            }

            actual_count += 1;
        }

        compare_i64(actual_count, comparison.as_str(), count)
    }

    fn name(&self) -> &str {
        "units_in_area"
    }

    fn description(&self) -> &str {
        "Checks the number of units in an area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
            "comparison".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "unit_type".to_string()]
    }
}

/// Position In Area Condition - Checks if position/unit is inside area
pub(super) struct PositionInAreaCondition;

#[async_trait]
impl ScriptCondition for PositionInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = crate::scripting::actions::get_int_param_optional(parameters, "object_id");
        let pos_x = crate::scripting::actions::get_float_param_optional(parameters, "x");
        let pos_y = crate::scripting::actions::get_float_param_optional(parameters, "y");
        let area_x = crate::scripting::actions::get_float_param(parameters, "area_x")?;
        let area_y = crate::scripting::actions::get_float_param(parameters, "area_y")?;
        let area_radius = crate::scripting::actions::get_float_param(parameters, "area_radius")?;

        // Check position or object position
        let (check_x, check_y) = if let (Some(x), Some(y)) = (pos_x, pos_y) {
            (x, y)
        } else if let Some(obj_id) = object_id {
            log::debug!("Checking position of object {}", obj_id);
            // Get object position from ObjectManager
            if let Ok(manager) = get_object_manager().read() {
                if let Some(obj_arc) = manager.get_object(obj_id as u32) {
                    if let Ok(obj) = obj_arc.read() {
                        let pos = obj.get_position();
                        (pos.x as f64, pos.y as f64)
                    } else {
                        (0.0, 0.0)
                    }
                } else {
                    (0.0, 0.0)
                }
            } else {
                (0.0, 0.0)
            }
        } else {
            return Err(GameLogicError::Configuration(
                "Either (x, y) or object_id must be provided".to_string(),
            ));
        };

        let distance = ((check_x - area_x).powi(2) + (check_y - area_y).powi(2)).sqrt();
        let in_area = distance <= area_radius;

        log::debug!(
            "Position ({}, {}) is {} area at ({}, {}) radius {}",
            check_x,
            check_y,
            if in_area { "inside" } else { "outside" },
            area_x,
            area_y,
            area_radius
        );

        Ok(in_area)
    }

    fn name(&self) -> &str {
        "position_in_area"
    }

    fn description(&self) -> &str {
        "Checks if a position or object is within a circular area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "area_x".to_string(),
            "area_y".to_string(),
            "area_radius".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string(), "object_id".to_string()]
    }
}

/// No Enemy Units In Area - Area is clear of enemy forces
pub(super) struct NoEnemyUnitsInAreaCondition;

#[async_trait]
impl ScriptCondition for NoEnemyUnitsInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let x = crate::scripting::actions::get_float_param(parameters, "x")?;
        let y = crate::scripting::actions::get_float_param(parameters, "y")?;
        let radius = crate::scripting::actions::get_float_param(parameters, "radius")?;

        log::debug!(
            "Checking if area ({}, {}) radius {} is clear of enemies for player {}",
            x,
            y,
            radius,
            player
        );

        // Query units in area, filter for enemies
        // In C++: Check all objects in partition cell, filter by enemy relationship
        use crate::common::Coord3D;
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let center = Coord3D::new(x as f32, y as f32, 0.0);
            let objects_in_area = manager.find_objects_in_radius(center, radius as f32);

            // Check each object to see if it's an enemy
            for obj_id in objects_in_area {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    if let Ok(obj) = obj_arc.read() {
                        // Get object's controlling player
                        if let Some(obj_player_id) = obj.get_controlling_player_id() {
                            if obj_player_id != player as u32 {
                                // Check if this player is an enemy
                                let player_list_lock = player_list();
                                if let Ok(list) = player_list_lock.read() {
                                    if let Some(our_player_arc) = list.get_player(player as i32) {
                                        if let Ok(our_player) = our_player_arc.read() {
                                            if let Some(their_player_arc) =
                                                list.get_player(obj_player_id as i32)
                                            {
                                                if let Ok(their_player) = their_player_arc.read() {
                                                    if our_player
                                                        .is_enemy_with_player(&their_player)
                                                    {
                                                        return Ok(false); // Found an enemy
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(true) // No enemies found
    }

    fn name(&self) -> &str {
        "no_enemy_units_in_area"
    }

    fn description(&self) -> &str {
        "Checks if area is clear of enemy units for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Any Unit In Area - At least one unit in area
pub(super) struct AnyUnitInAreaCondition;

#[async_trait]
impl ScriptCondition for AnyUnitInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = crate::scripting::actions::get_float_param(parameters, "x")?;
        let y = crate::scripting::actions::get_float_param(parameters, "y")?;
        let radius = crate::scripting::actions::get_float_param(parameters, "radius")?;
        let _player = crate::scripting::actions::get_int_param_optional(parameters, "player");
        let _unit_type = parameters.get("unit_type");

        log::debug!(
            "Checking if any units are in area ({}, {}) radius {}",
            x,
            y,
            radius
        );

        // Query ObjectManager for units in area using spatial partitioning
        use crate::common::Coord3D;
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let center = Coord3D::new(x as f32, y as f32, 0.0);
            let objects_in_area = manager.find_objects_in_radius(center, radius as f32);

            // Filter by player and unit type if specified
            for obj_id in objects_in_area {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    if let Ok(obj) = obj_arc.read() {
                        if obj.is_destroyed() {
                            continue;
                        }

                        // Check player filter
                        if let Some(player_id) = _player {
                            if let Some(owner_id) = obj.get_controlling_player_id() {
                                if owner_id != player_id as u32 {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        }

                        // Check unit type filter
                        if let Some(unit_type_value) = _unit_type {
                            if let Some(template) = &obj.template {
                                if let ScriptValue::String(unit_type) = unit_type_value {
                                    if !template.get_name().eq_ignore_ascii_case(unit_type) {
                                        continue;
                                    }
                                }
                            } else {
                                continue;
                            }
                        }

                        // Found at least one matching unit
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "any_unit_in_area"
    }

    fn description(&self) -> &str {
        "Checks if any units are present in the specified area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string(), "radius".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "unit_type".to_string()]
    }
}

//-------------------------------------------------------------------------------------------------
// BUILDING_ENTERED_BY_PLAYER - evaluateBuildingEntered
//-------------------------------------------------------------------------------------------------
pub(super) struct BuildingEnteredByPlayerCondition;

#[async_trait]
impl ScriptCondition for BuildingEnteredByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → host snapshot (C++ getPlayerWhoEntered).
        if dual_world_registry_unavailable() {
            let building_name = get_str_param(parameters, "building_name")?;
            let player_name = get_str_param(parameters, "player")?;
            return Ok(super::helpers::host_building_entered_by_player(
                &building_name,
                &player_name,
            )
            .unwrap_or(false));
        }

        let building_name = get_str_param(parameters, "building_name")?;
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_mask = player.get_player_mask();
        drop(player);

        let object_id = match lookup_named_object_id(&building_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                let Some(contain) = obj.get_contain() else {
                    return false;
                };
                contain
                    .lock()
                    .ok()
                    .map(|contain_guard| {
                        let entered_mask = contain_guard.get_player_who_entered();
                        !entered_mask.is_empty() && entered_mask == player_mask
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "building_entered_by_player"
    }
    fn description(&self) -> &str {
        "Checks if a building was entered by a specific player (C++ BUILDING_ENTERED_BY_PLAYER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["building_name".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

pub(super) fn register_area_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(AreaClearCondition));
    registry.register_condition(Box::new(AreaControlledByPlayerCondition));
    registry.register_condition(Box::new(UnitsInAreaCondition));
    registry.register_condition(Box::new(PositionInAreaCondition));
    registry.register_condition(Box::new(NoEnemyUnitsInAreaCondition));
    registry.register_condition(Box::new(AnyUnitInAreaCondition));
    registry.register_condition(Box::new(BuildingEnteredByPlayerCondition));
}
