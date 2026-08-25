//! Object and unit script conditions.

use super::helpers::{
    compare_f64, compare_i64, dual_world_registry_unavailable, event_type_from_name,
    get_player_arc, get_str_param, get_str_param_optional, host_eval_unit_has_object_status,
    lookup_named_object_id, parse_nested_condition, parse_object_status_mask, perform_comparison,
    with_script_engine_mut,
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

/// Object exists condition
pub(super) struct ObjectExistsCondition;

#[async_trait]
impl ScriptCondition for ObjectExistsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = crate::scripting::actions::get_int_param(parameters, "object_id")?;

        log::debug!("Checking if object {} exists", object_id);

        // Check if object exists in ObjectManager
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            if let Some(obj_arc) = manager.get_object(object_id as u32) {
                if let Ok(obj) = obj_arc.read() {
                    // Object exists and is not destroyed
                    return Ok(!obj.is_destroyed());
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "object_exists"
    }

    fn description(&self) -> &str {
        "Checks if an object exists"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Object health condition
pub(super) struct ObjectHealthCondition;

#[async_trait]
impl ScriptCondition for ObjectHealthCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = crate::scripting::actions::get_int_param(parameters, "object_id")?;
        let comparison = crate::scripting::actions::get_string_param(parameters, "comparison")?; // "greater", "less", "equal"
        let value = crate::scripting::actions::get_float_param(parameters, "value")?;

        log::debug!(
            "Checking if object {} health is {} {}",
            object_id,
            comparison,
            value
        );

        // Get actual object health from ObjectManager
        let obj_manager = get_object_manager();
        let object_health = if let Ok(manager) = obj_manager.read() {
            if let Some(obj_arc) = manager.get_object(object_id as u32) {
                if let Ok(obj) = obj_arc.read() {
                    (obj.get_health_percentage() * 100.0) as f64
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        match comparison.as_str() {
            "greater" => Ok(object_health > value),
            "less" => Ok(object_health < value),
            "equal" => Ok((object_health - value).abs() < 0.01),
            "greater_equal" => Ok(object_health >= value),
            "less_equal" => Ok(object_health <= value),
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison operator: {}",
                comparison
            ))),
        }
    }

    fn name(&self) -> &str {
        "object_health"
    }

    fn description(&self) -> &str {
        "Checks an object's health against a value"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "object_id".to_string(),
            "comparison".to_string(),
            "value".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Object in area condition
pub(super) struct ObjectInAreaCondition;

#[async_trait]
impl ScriptCondition for ObjectInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = crate::scripting::actions::get_int_param(parameters, "object_id")?;
        let x = crate::scripting::actions::get_float_param(parameters, "x")?;
        let y = crate::scripting::actions::get_float_param(parameters, "y")?;
        let radius = crate::scripting::actions::get_float_param(parameters, "radius")?;

        log::debug!(
            "Checking if object {} is in area ({}, {}) with radius {}",
            object_id,
            x,
            y,
            radius
        );

        if object_id < 0 {
            return Ok(false);
        }

        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };
        let Some(obj_arc) = manager.get_object(object_id as u32) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let __base_arc = obj_guard.base();
        let Ok(base_guard) = __base_arc.read() else {
            return Ok(false);
        };
        if base_guard.is_destroyed() {
            return Ok(false);
        }

        let pos = *base_guard.get_position();
        let dx = pos.x as f64 - x;
        let dy = pos.y as f64 - y;
        Ok(dx * dx + dy * dy <= radius * radius)
    }

    fn name(&self) -> &str {
        "object_in_area"
    }

    fn description(&self) -> &str {
        "Checks if an object is within a circular area"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "object_id".to_string(),
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Object near object condition
pub(super) struct ObjectNearObjectCondition;

#[async_trait]
impl ScriptCondition for ObjectNearObjectCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object1_id = crate::scripting::actions::get_int_param(parameters, "object1_id")?;
        let object2_id = crate::scripting::actions::get_int_param(parameters, "object2_id")?;
        let distance = crate::scripting::actions::get_float_param(parameters, "distance")?;

        log::debug!(
            "Checking if object {} is within {} units of object {}",
            object1_id,
            distance,
            object2_id
        );

        if object1_id < 0 || object2_id < 0 {
            return Ok(false);
        }

        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };

        let Some(obj1_arc) = manager.get_object(object1_id as u32) else {
            return Ok(false);
        };
        let Some(obj2_arc) = manager.get_object(object2_id as u32) else {
            return Ok(false);
        };
        let (Ok(obj1), Ok(obj2)) = (obj1_arc.read(), obj2_arc.read()) else {
            return Ok(false);
        };
        let __b1 = obj1.base();
        let __b2 = obj2.base();
        let (Ok(base1), Ok(base2)) = (__b1.read(), __b2.read()) else {
            return Ok(false);
        };
        if base1.is_destroyed() || base2.is_destroyed() {
            return Ok(false);
        }

        let p1 = *base1.get_position();
        let p2 = *base2.get_position();
        let dx = p1.x as f64 - p2.x as f64;
        let dy = p1.y as f64 - p2.y as f64;
        Ok(dx * dx + dy * dy <= distance * distance)
    }

    fn name(&self) -> &str {
        "object_near_object"
    }

    fn description(&self) -> &str {
        "Checks if one object is near another object"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "object1_id".to_string(),
            "object2_id".to_string(),
            "distance".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Object owned by player condition
pub(super) struct ObjectOwnedByPlayerCondition;

#[async_trait]
impl ScriptCondition for ObjectOwnedByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = crate::scripting::actions::get_int_param(parameters, "object_id")?;
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;

        log::debug!(
            "Checking if object {} is owned by player {}",
            object_id,
            player
        );

        // Check object ownership
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            if let Some(obj_arc) = manager.get_object(object_id as u32) {
                if let Ok(obj) = obj_arc.read() {
                    if let Some(owner_id) = obj.get_controlling_player_id() {
                        return Ok(owner_id == player as u32);
                    }
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "object_owned_by_player"
    }

    fn description(&self) -> &str {
        "Checks if an object is owned by a specific player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Building Damaged - Structure health below threshold
pub(super) struct BuildingDamagedCondition;

#[async_trait]
impl ScriptCondition for BuildingDamagedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = crate::scripting::actions::get_int_param(parameters, "object_id")?;
        let health_percent =
            crate::scripting::actions::get_float_param(parameters, "health_percent")?;

        log::debug!(
            "Checking if building {} health is below {}%",
            object_id,
            health_percent
        );

        // Get actual building health
        // In C++: pObject->Get_Health() / pObject->Get_Max_Health() * 100
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            if let Some(obj_arc) = manager.get_object(object_id as u32) {
                if let Ok(obj) = obj_arc.read() {
                    let current_health_percent = (obj.get_health_percentage() * 100.0) as f64;
                    return Ok(current_health_percent < health_percent);
                }
            }
        }

        // Object not found, assume not damaged
        Ok(false)
    }

    fn name(&self) -> &str {
        "building_damaged"
    }

    fn description(&self) -> &str {
        "Checks if building health is below percentage threshold"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_id".to_string(), "health_percent".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Unit Near Position - Unit within distance of point
pub(super) struct UnitNearPositionCondition;

#[async_trait]
impl ScriptCondition for UnitNearPositionCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = crate::scripting::actions::get_int_param(parameters, "object_id")?;
        let x = crate::scripting::actions::get_float_param(parameters, "x")?;
        let y = crate::scripting::actions::get_float_param(parameters, "y")?;
        let distance = crate::scripting::actions::get_float_param(parameters, "distance")?;

        log::debug!(
            "Checking if object {} is within {} of ({}, {})",
            object_id,
            distance,
            x,
            y
        );

        // Get object position and calculate distance
        // In C++: Calculate distance between object pos and target pos
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            if let Some(obj_arc) = manager.get_object(object_id as u32) {
                if let Ok(obj) = obj_arc.read() {
                    let pos = obj.get_position();
                    let object_x = pos.x as f64;
                    let object_y = pos.y as f64;
                    let actual_distance = ((object_x - x).powi(2) + (object_y - y).powi(2)).sqrt();
                    return Ok(actual_distance <= distance);
                }
            }
        }

        // Object not found
        Ok(false)
    }

    fn name(&self) -> &str {
        "unit_near_position"
    }

    fn description(&self) -> &str {
        "Checks if unit is within distance of a position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "object_id".to_string(),
            "x".to_string(),
            "y".to_string(),
            "distance".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// UNIT_HAS_OBJECT_STATUS - evaluateUnitHasObjectStatus
//-------------------------------------------------------------------------------------------------
pub(super) struct UnitHasObjectStatusCondition;

#[async_trait]
impl ScriptCondition for UnitHasObjectStatusCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let status_str = get_str_param(parameters, "status")?;
        let status_mask = parse_object_status_mask(&status_str);
        if dual_world_registry_unavailable() {
            return Ok(
                host_eval_unit_has_object_status(&unit_name, status_mask.bits()).unwrap_or(false),
            );
        }

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                obj.get_status_bits().intersects(status_mask)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "unit_has_object_status"
    }
    fn description(&self) -> &str {
        "Checks if named unit has a specific object status (C++ UNIT_HAS_OBJECT_STATUS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "status".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// UNIT_EMPTIED - evaluateUnitHasEmptied
// Returns true if transport was emptied between last frame and this frame.
//-------------------------------------------------------------------------------------------------
pub(super) struct UnitEmptiedCondition;

pub(super) struct TransportStatus {
    obj_id: u32,
    frame_number: u32,
    unit_count: i32,
}

static TRANSPORT_STATUSES: std::sync::Mutex<Vec<TransportStatus>> =
    std::sync::Mutex::new(Vec::new());

#[async_trait]
impl ScriptCondition for UnitEmptiedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };

        let Some((obj_id, num_peeps)) = OBJECT_REGISTRY.with_object(object_id, |obj| {
            let obj_id = obj.get_id();
            let num_peeps = if let Some(contain_arc) = obj.get_contain() {
                if let Ok(contain_guard) = contain_arc.lock() {
                    contain_guard.get_contained_count() as i32
                } else {
                    0
                }
            } else {
                0
            };
            (obj_id, num_peeps)
        }) else {
            return Ok(false);
        };

        let frame_num = TheGameLogic::get_frame();

        let mut statuses = TRANSPORT_STATUSES.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to lock transport statuses: {}", e))
        })?;

        let existing_idx = statuses.iter().position(|s| s.obj_id == obj_id);

        match existing_idx {
            None => {
                statuses.push(TransportStatus {
                    obj_id,
                    frame_number: frame_num,
                    unit_count: num_peeps,
                });
                Ok(false)
            }
            Some(idx) => {
                let stats = &statuses[idx];
                if stats.frame_number == frame_num.saturating_sub(1)
                    && stats.unit_count > 0
                    && num_peeps == 0
                {
                    Ok(true)
                } else {
                    statuses[idx].frame_number = frame_num;
                    statuses[idx].unit_count = num_peeps;
                    Ok(false)
                }
            }
        }
    }

    fn name(&self) -> &str {
        "unit_emptied"
    }
    fn description(&self) -> &str {
        "Checks if transport was just emptied (C++ UNIT_EMPTIED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// UNIT_HEALTH - C++ ScriptConditions::evaluateUnitHealth
// Gets named object, reads body module health/initial health, computes percentage,
// compares against threshold using the given comparison operator.
//-------------------------------------------------------------------------------------------------
pub(super) struct UnitHealthCondition;

#[async_trait]
impl ScriptCondition for UnitHealthCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")
            .or_else(|_| get_str_param(parameters, "unit"))?;
        let comparison = get_str_param(parameters, "comparison")?;
        let health_percent = match parameters
            .get("health_percent")
            .or(parameters.get("percent"))
        {
            Some(ScriptValue::Int(v)) => *v as i64,
            Some(ScriptValue::Float(v)) => *v as i64,
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'health_percent' parameter".to_string(),
                ));
            }
        };

        // Look up the named object
        let tracker = get_named_object_tracker();
        let object_id = tracker.get_object_id(&unit_name)?.ok_or_else(|| {
            GameLogicError::Configuration(format!("Unit '{}' not found", unit_name))
        })?;

        let body = OBJECT_REGISTRY
            .with_object(object_id, |obj| obj.get_body_module())
            .flatten()
            .ok_or_else(|| {
                GameLogicError::Configuration(format!(
                    "Unit '{}' (id={}) missing or has no body module",
                    unit_name, object_id
                ))
            })?;

        let body_guard = body
            .lock()
            .map_err(|e| GameLogicError::Threading(format!("Failed to lock body module: {}", e)))?;

        let cur_health = body_guard.get_health();
        let initial_health = body_guard.get_initial_health();

        if initial_health <= 0.0 {
            return Ok(false);
        }

        // C++: Int curPercent = (curHealth*100 + initialHealth/2)/initialHealth;
        let cur_percent = ((cur_health * 100.0 + initial_health / 2.0) / initial_health) as i64;

        Ok(perform_comparison(cur_percent, &comparison, health_percent))
    }

    fn name(&self) -> &str {
        "unit_health"
    }
    fn description(&self) -> &str {
        "Compare unit health percentage against threshold (C++ UNIT_HEALTH)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "unit_name".to_string(),
            "comparison".to_string(),
            "health_percent".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec!["unit".to_string(), "percent".to_string()]
    }
}

//-------------------------------------------------------------------------------------------------
// ENEMY_SIGHTED - C++ ScriptConditions::evaluateEnemySighted
// Gets the named unit, looks up the target player, iterates objects within the unit's
// vision range, filters by relationship (enemy/neutral/ally), returns true if any
// living object belongs to the target player.
//-------------------------------------------------------------------------------------------------
pub(super) struct EnemySightedCondition;

#[async_trait]
impl ScriptCondition for EnemySightedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")
            .or_else(|_| get_str_param(parameters, "unit"))?;

        // Alliance parameter: "enemy", "neutral", "friend" (default: "enemy")
        let alliance =
            get_str_param_optional(parameters, "alliance").unwrap_or_else(|| "enemy".to_string());

        // Target player
        let player_arc = get_player_arc(parameters, "player")?;
        let player = match player_arc {
            Some(p) => p,
            None => return Ok(false),
        };
        let player_id = {
            let p_guard = player
                .read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
            p_guard.get_player_index()
        };

        // Look up the named unit
        let tracker = get_named_object_tracker();
        let object_id = match tracker.get_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };

        let Some((unit_pos, vision_range, source_player_arc)) =
            OBJECT_REGISTRY.with_object(object_id, |obj| {
                // Get the unit's position and vision range
                (
                    *obj.get_position(),
                    obj.get_vision_range(),
                    obj.get_controlling_player(),
                )
            })
        else {
            return Ok(false);
        };

        // Get objects in range via partition manager
        let objects_in_range = match ThePartitionManager::get() {
            Some(pm) => pm.get_objects_in_range(&unit_pos, vision_range),
            None => Vec::new(),
        };

        for candidate_id in objects_in_range {
            if candidate_id == object_id {
                continue; // Skip self
            }

            let Some(candidate_player_id) = OBJECT_REGISTRY
                .with_object(candidate_id, |candidate| {
                    // Must be alive
                    if !candidate.is_alive() {
                        return None;
                    }
                    // Check if candidate belongs to the target player
                    candidate.get_controlling_player_id().map(|id| id as i32)
                })
                .flatten()
            else {
                continue;
            };

            if candidate_player_id != player_id {
                continue;
            }

            // Filter by alliance relationship
            let passes_alliance = match alliance.as_str() {
                "neutral" => true,
                "friend" | "ally" => {
                    if let Some(ref src_arc) = source_player_arc {
                        if let Ok(src_player) = src_arc.read() {
                            if let Ok(tgt_player) = player.read() {
                                let rel = src_player.get_relationship(&tgt_player);
                                matches!(rel, Relationship::Allies)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => {
                    // "enemy" (default)
                    if let Some(ref src_arc) = source_player_arc {
                        if let Ok(src_player) = src_arc.read() {
                            if let Ok(tgt_player) = player.read() {
                                src_player.is_enemy_with_player(&tgt_player)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            };

            if passes_alliance {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "enemy_sighted"
    }
    fn description(&self) -> &str {
        "Unit sees a unit belonging to a player (C++ ENEMY_SIGHTED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec!["unit".to_string(), "alliance".to_string()]
    }
}

pub(super) fn register_object_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(ObjectExistsCondition));
    registry.register_condition(Box::new(ObjectHealthCondition));
    registry.register_condition(Box::new(ObjectInAreaCondition));
    registry.register_condition(Box::new(ObjectNearObjectCondition));
    registry.register_condition(Box::new(ObjectOwnedByPlayerCondition));
    registry.register_condition(Box::new(BuildingDamagedCondition));
    registry.register_condition(Box::new(UnitNearPositionCondition));
    registry.register_condition(Box::new(UnitHasObjectStatusCondition));
    registry.register_condition(Box::new(UnitEmptiedCondition));
    registry.register_condition(Box::new(UnitHealthCondition));
    registry.register_condition(Box::new(EnemySightedCondition));
}
