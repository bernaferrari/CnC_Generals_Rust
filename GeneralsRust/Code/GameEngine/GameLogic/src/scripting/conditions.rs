//! Script Conditions System
//!
//! This module provides condition evaluation for script triggers and decision making.

use super::{ScriptContext, ScriptValue};
use crate::common::{Coord3D, KindOf, LOGICFRAMES_PER_SECOND};
use crate::object_manager::get_object_manager;
use crate::player::{player_list, Player, PlayerType};
use crate::scripting::engine::{get_event_manager, get_script_engine};
use crate::scripting::events::{EventFilter, GameEventType};
use crate::team::get_team_factory;
use crate::upgrade::center::get_upgrade_center;
use crate::{GameLogicError, GameLogicResult};

use async_trait::async_trait;
use game_engine::common::rts::{get_science_store, SCIENCE_INVALID};
use std::collections::HashMap;

fn normalize_event_name(name: &str) -> String {
    name.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase()
}

fn event_type_from_name(name: &str) -> GameEventType {
    let normalized = normalize_event_name(name);
    match normalized.as_str() {
        "unitcreated" | "unit_created" => GameEventType::UnitCreated,
        "unitdestroyed" | "unit_destroyed" => GameEventType::UnitDestroyed,
        "unitdamaged" | "unit_damaged" => GameEventType::UnitDamaged,
        "unitmoved" | "unit_moved" => GameEventType::UnitMoved,
        "unitattacked" | "unit_attacked" => GameEventType::UnitAttacked,
        "weaponfired" | "weapon_fired" => GameEventType::WeaponFired,
        "combats_started" | "combatstarted" | "combat_started" => GameEventType::CombatStarted,
        "combatended" | "combat_ended" => GameEventType::CombatEnded,
        "playerdefeated" | "player_defeated" => GameEventType::PlayerDefeated,
        "playervictorious" | "player_victorious" => GameEventType::PlayerVictorious,
        "timerexpired" | "timer_expired" => GameEventType::TimerExpired,
        _ => GameEventType::Custom(name.to_string()),
    }
}

fn compare_i64(actual: i64, comparison: &str, expected: i64) -> GameLogicResult<bool> {
    Ok(match comparison {
        "greater" => actual > expected,
        "less" => actual < expected,
        "equal" => actual == expected,
        "greater_equal" => actual >= expected,
        "less_equal" => actual <= expected,
        _ => {
            return Err(GameLogicError::Configuration(format!(
                "Invalid comparison operator: {}",
                comparison
            )))
        }
    })
}

fn compare_f64(actual: f64, comparison: &str, expected: f64) -> GameLogicResult<bool> {
    Ok(match comparison {
        "greater" => actual > expected,
        "less" => actual < expected,
        "equal" => (actual - expected).abs() < 0.01,
        "greater_equal" => actual >= expected,
        "less_equal" => actual <= expected,
        _ => {
            return Err(GameLogicError::Configuration(format!(
                "Invalid comparison operator: {}",
                comparison
            )))
        }
    })
}

/// Script condition trait
#[async_trait]
pub trait ScriptCondition: Send + Sync {
    /// Evaluate the condition
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool>;

    /// Get condition name
    fn name(&self) -> &str;

    /// Get condition description
    fn description(&self) -> &str;

    /// Get required parameters
    fn required_parameters(&self) -> Vec<String>;

    /// Get optional parameters
    fn optional_parameters(&self) -> Vec<String>;
}

/// Condition registry
pub struct ConditionRegistry {
    conditions: HashMap<String, Box<dyn ScriptCondition>>,
}

impl ConditionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            conditions: HashMap::new(),
        };

        // Register built-in conditions
        registry.register_builtin_conditions();
        registry
    }

    /// Register built-in conditions
    fn register_builtin_conditions(&mut self) {
        // Player and team conditions
        self.register_condition(Box::new(PlayerAliveCondition));
        self.register_condition(Box::new(PlayerDefeatedCondition));
        self.register_condition(Box::new(PlayerHasResourceCondition));
        self.register_condition(Box::new(PlayerHasUnitsCondition));
        self.register_condition(Box::new(PlayerHasBuildingsCondition));
        self.register_condition(Box::new(PlayersAlliedCondition));

        // Unit and object conditions
        self.register_condition(Box::new(ObjectExistsCondition));
        self.register_condition(Box::new(ObjectHealthCondition));
        self.register_condition(Box::new(ObjectInAreaCondition));
        self.register_condition(Box::new(ObjectNearObjectCondition));
        self.register_condition(Box::new(ObjectOwnedByPlayerCondition));

        // Map and area conditions
        self.register_condition(Box::new(AreaClearCondition));
        self.register_condition(Box::new(AreaControlledByPlayerCondition));
        self.register_condition(Box::new(UnitsInAreaCondition));

        // Time and event conditions
        self.register_condition(Box::new(GameTimeCondition));
        self.register_condition(Box::new(TimerCondition));
        self.register_condition(Box::new(EventOccurredCondition));

        // Combat and military conditions
        self.register_condition(Box::new(UnitsDestroyedCondition));
        self.register_condition(Box::new(CombatOccurredCondition));
        self.register_condition(Box::new(PlayerCasualtiesCondition));

        // Technology and upgrade conditions
        self.register_condition(Box::new(PlayerHasTechnologyCondition));
        self.register_condition(Box::new(PlayerHasUpgradeCondition));
        self.register_condition(Box::new(SpecialPowerAvailableCondition));

        // Logical conditions
        self.register_condition(Box::new(AndCondition));
        self.register_condition(Box::new(OrCondition));
        self.register_condition(Box::new(NotCondition));

        // Variable conditions
        self.register_condition(Box::new(VariableEqualsCondition));
        self.register_condition(Box::new(VariableGreaterThanCondition));
        self.register_condition(Box::new(VariableLessThanCondition));

        // 20 Core Conditions - Priority 1 Implementation
        self.register_condition(Box::new(CounterComparisonCondition));
        self.register_condition(Box::new(FlagComparisonCondition));
        self.register_condition(Box::new(PositionInAreaCondition));
        self.register_condition(Box::new(ResourcesExceedCondition));
        self.register_condition(Box::new(StructureBuiltCondition));
        self.register_condition(Box::new(UnitTypeCountExceedsCondition));
        self.register_condition(Box::new(TeamAllUnitsDestroyedCondition));
        self.register_condition(Box::new(PlayerWonCondition));
        self.register_condition(Box::new(NoEnemyUnitsInAreaCondition));
        self.register_condition(Box::new(AlliesWithTeamCondition));
        self.register_condition(Box::new(BuildingDamagedCondition));
        self.register_condition(Box::new(UnitNearPositionCondition));
        self.register_condition(Box::new(UnitsInFormationCondition));
        self.register_condition(Box::new(ResearchCompleteCondition));
        self.register_condition(Box::new(SpecialPowerReadyCondition));
        self.register_condition(Box::new(AnyUnitInAreaCondition));
    }

    /// Register a condition
    pub fn register_condition(&mut self, condition: Box<dyn ScriptCondition>) {
        self.conditions
            .insert(condition.name().to_string(), condition);
    }

    /// Get condition by name
    pub fn get_condition(&self, name: &str) -> Option<&dyn ScriptCondition> {
        self.conditions
            .get(name)
            .map(|condition| condition.as_ref())
    }

    /// List all available conditions
    pub fn list_conditions(&self) -> Vec<String> {
        self.conditions.keys().cloned().collect()
    }
}

// Built-in condition implementations

/// Player alive condition
struct PlayerAliveCondition;

#[async_trait]
impl ScriptCondition for PlayerAliveCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;

        log::debug!("Checking if player {} is alive", player);

        // Check actual player state using player_list
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    // Player is alive if not defeated
                    return Ok(!player_guard.is_defeated());
                }
            }
        }

        // If player not found, consider them not alive
        Ok(false)
    }

    fn name(&self) -> &str {
        "player_alive"
    }

    fn description(&self) -> &str {
        "Checks if a player is still alive in the game"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player defeated condition
struct PlayerDefeatedCondition;

#[async_trait]
impl ScriptCondition for PlayerDefeatedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;

        log::debug!("Checking if player {} is defeated", player);

        // Check actual player defeated state
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    return Ok(player_guard.is_defeated());
                }
            }
        }

        // If player not found, consider them defeated
        Ok(true)
    }

    fn name(&self) -> &str {
        "player_defeated"
    }

    fn description(&self) -> &str {
        "Checks if a player has been defeated"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has resource condition
struct PlayerHasResourceCondition;

#[async_trait]
impl ScriptCondition for PlayerHasResourceCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let resource_type = super::actions::get_string_param(parameters, "resource_type")?;
        let amount = super::actions::get_int_param(parameters, "amount")?;

        log::debug!(
            "Checking if player {} has {} {}",
            player,
            amount,
            resource_type
        );

        // Check actual player resources (currently only supports money)
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    // For now, only handle "money" resource type
                    if resource_type.eq_ignore_ascii_case("money") {
                        let player_money = player_guard.get_money().get_money() as i64;
                        return Ok(player_money >= amount);
                    }
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "player_has_resource"
    }

    fn description(&self) -> &str {
        "Checks if a player has a certain amount of resources"
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

/// Player has units condition
struct PlayerHasUnitsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasUnitsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let unit_type = super::actions::get_string_param(parameters, "unit_type")?;
        let count = super::actions::get_int_param(parameters, "count")?;

        log::debug!(
            "Checking if player {} has {} units of type '{}'",
            player,
            count,
            unit_type
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };

        let owned = manager.get_objects_owned_by_player(player_id);
        let mut matches = 0i64;
        for object_id in owned {
            let Some(obj_arc) = manager.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let Ok(base_guard) = obj_guard.base.read() else {
                continue;
            };
            if base_guard.is_destroyed() {
                continue;
            }

            let Some(template) = obj_guard.template.as_ref() else {
                continue;
            };

            if template.is_kind_of(KindOf::Structure) || template.is_kind_of(KindOf::Building) {
                continue;
            }

            if template
                .get_name()
                .as_str()
                .eq_ignore_ascii_case(unit_type.as_str())
            {
                matches += 1;
                if matches >= count {
                    return Ok(true);
                }
            }
        }

        Ok(matches >= count)
    }

    fn name(&self) -> &str {
        "player_has_units"
    }

    fn description(&self) -> &str {
        "Checks if a player has a certain number of specific units"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has buildings condition
struct PlayerHasBuildingsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasBuildingsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let building_type = super::actions::get_string_param(parameters, "building_type")?;
        let count = super::actions::get_int_param(parameters, "count")?;

        log::debug!(
            "Checking if player {} has {} buildings of type '{}'",
            player,
            count,
            building_type
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };

        let owned = manager.get_objects_owned_by_player(player_id);
        let mut matches = 0i64;
        for object_id in owned {
            let Some(obj_arc) = manager.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let Ok(base_guard) = obj_guard.base.read() else {
                continue;
            };
            if base_guard.is_destroyed() {
                continue;
            }

            let Some(template) = obj_guard.template.as_ref() else {
                continue;
            };

            let is_building =
                template.is_kind_of(KindOf::Structure) || template.is_kind_of(KindOf::Building);
            if !is_building {
                continue;
            }

            if template
                .get_name()
                .as_str()
                .eq_ignore_ascii_case(building_type.as_str())
            {
                matches += 1;
                if matches >= count {
                    return Ok(true);
                }
            }
        }

        Ok(matches >= count)
    }

    fn name(&self) -> &str {
        "player_has_buildings"
    }

    fn description(&self) -> &str {
        "Checks if a player has a certain number of specific buildings"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "building_type".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Players allied condition
struct PlayersAlliedCondition;

#[async_trait]
impl ScriptCondition for PlayersAlliedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player1 = super::actions::get_int_param(parameters, "player1")?;
        let player2 = super::actions::get_int_param(parameters, "player2")?;

        log::debug!("Checking if players {} and {} are allied", player1, player2);
        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(p1) = list.get_player(player1 as i32) else {
            return Ok(false);
        };
        let Some(p2) = list.get_player(player2 as i32) else {
            return Ok(false);
        };
        let (Ok(p1_guard), Ok(p2_guard)) = (p1.read(), p2.read()) else {
            return Ok(false);
        };

        let rel = p1_guard.get_relationship(&p2_guard);
        Ok(matches!(
            rel,
            crate::common::Relationship::Ally
                | crate::common::Relationship::Allies
                | crate::common::Relationship::Friend
        ))
    }

    fn name(&self) -> &str {
        "players_allied"
    }

    fn description(&self) -> &str {
        "Checks if two players are allied"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player1".to_string(), "player2".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Object exists condition
struct ObjectExistsCondition;

#[async_trait]
impl ScriptCondition for ObjectExistsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = super::actions::get_int_param(parameters, "object_id")?;

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
struct ObjectHealthCondition;

#[async_trait]
impl ScriptCondition for ObjectHealthCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = super::actions::get_int_param(parameters, "object_id")?;
        let comparison = super::actions::get_string_param(parameters, "comparison")?; // "greater", "less", "equal"
        let value = super::actions::get_float_param(parameters, "value")?;

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
struct ObjectInAreaCondition;

#[async_trait]
impl ScriptCondition for ObjectInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = super::actions::get_int_param(parameters, "object_id")?;
        let x = super::actions::get_float_param(parameters, "x")?;
        let y = super::actions::get_float_param(parameters, "y")?;
        let radius = super::actions::get_float_param(parameters, "radius")?;

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
        let Ok(base_guard) = obj_guard.base.read() else {
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
struct ObjectNearObjectCondition;

#[async_trait]
impl ScriptCondition for ObjectNearObjectCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object1_id = super::actions::get_int_param(parameters, "object1_id")?;
        let object2_id = super::actions::get_int_param(parameters, "object2_id")?;
        let distance = super::actions::get_float_param(parameters, "distance")?;

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
        let (Ok(base1), Ok(base2)) = (obj1.base.read(), obj2.base.read()) else {
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
struct ObjectOwnedByPlayerCondition;

#[async_trait]
impl ScriptCondition for ObjectOwnedByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = super::actions::get_int_param(parameters, "object_id")?;
        let player = super::actions::get_int_param(parameters, "player")?;

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

/// Area clear condition
struct AreaClearCondition;

#[async_trait]
impl ScriptCondition for AreaClearCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = super::actions::get_float_param(parameters, "x")?;
        let y = super::actions::get_float_param(parameters, "y")?;
        let radius = super::actions::get_float_param(parameters, "radius")?;
        let exclude_player = super::actions::get_int_param_optional(parameters, "exclude_player");

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
            let Ok(base_guard) = obj_guard.base.read() else {
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
struct AreaControlledByPlayerCondition;

#[async_trait]
impl ScriptCondition for AreaControlledByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let x = super::actions::get_float_param(parameters, "x")?;
        let y = super::actions::get_float_param(parameters, "y")?;
        let radius = super::actions::get_float_param(parameters, "radius")?;

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
            let Ok(base_guard) = obj_guard.base.read() else {
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
                crate::common::Relationship::Enemy | crate::common::Relationship::Neutral => {
                    return Ok(false)
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
struct UnitsInAreaCondition;

#[async_trait]
impl ScriptCondition for UnitsInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = super::actions::get_float_param(parameters, "x")?;
        let y = super::actions::get_float_param(parameters, "y")?;
        let radius = super::actions::get_float_param(parameters, "radius")?;
        let comparison = super::actions::get_string_param(parameters, "comparison")?;
        let count = super::actions::get_int_param(parameters, "count")?;
        let _player = super::actions::get_int_param_optional(parameters, "player");
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
            let Ok(base_guard) = obj_guard.base.read() else {
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

/// Game time condition
struct GameTimeCondition;

#[async_trait]
impl ScriptCondition for GameTimeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let comparison = super::actions::get_string_param(parameters, "comparison")?;
        let time = super::actions::get_float_param(parameters, "time")?;

        let game_time = context.game_time.as_secs_f64();

        let result = match comparison.as_str() {
            "greater" => game_time > time,
            "less" => game_time < time,
            "equal" => (game_time - time).abs() < 1.0, // 1 second tolerance
            "greater_equal" => game_time >= time,
            "less_equal" => game_time <= time,
            _ => {
                return Err(GameLogicError::Configuration(format!(
                    "Invalid comparison operator: {}",
                    comparison
                )))
            }
        };

        Ok(result)
    }

    fn name(&self) -> &str {
        "game_time"
    }

    fn description(&self) -> &str {
        "Checks the current game time"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["comparison".to_string(), "time".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Timer condition
struct TimerCondition;

#[async_trait]
impl ScriptCondition for TimerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let timer_name = super::actions::get_string_param(parameters, "timer_name")?;
        let comparison = super::actions::get_string_param(parameters, "comparison")?;
        let time = super::actions::get_float_param(parameters, "time")?;

        log::debug!("Checking timer '{}' {} {}", timer_name, comparison, time);

        let timer_frames_left = get_script_engine()
            .read()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .and_then(|engine| engine.get_counter(&timer_name).map(|c| c.value))
            })
            .unwrap_or(0);
        let timer_value = timer_frames_left.max(0) as f64 / LOGICFRAMES_PER_SECOND as f64;

        compare_f64(timer_value, comparison.as_str(), time)
    }

    fn name(&self) -> &str {
        "timer"
    }

    fn description(&self) -> &str {
        "Checks a named timer value"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "timer_name".to_string(),
            "comparison".to_string(),
            "time".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Event occurred condition
struct EventOccurredCondition;

#[async_trait]
impl ScriptCondition for EventOccurredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let event_name = super::actions::get_string_param(parameters, "event_name")?;

        log::debug!("Checking if event '{}' occurred", event_name);

        let event_type = event_type_from_name(event_name.as_str());
        let filter = EventFilter {
            event_types: vec![event_type],
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: super::ScriptPriority::Low,
        };

        let event_manager = get_event_manager();
        Ok(!event_manager.query_history(&filter, 1).await?.is_empty())
    }

    fn name(&self) -> &str {
        "event_occurred"
    }

    fn description(&self) -> &str {
        "Checks if a specific event has occurred"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["event_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Units destroyed condition
struct UnitsDestroyedCondition;

#[async_trait]
impl ScriptCondition for UnitsDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let _player = super::actions::get_int_param_optional(parameters, "player");
        let _unit_type = parameters.get("unit_type");
        let comparison = super::actions::get_string_param(parameters, "comparison")?;
        let count = super::actions::get_int_param(parameters, "count")?;

        log::debug!("Checking destroyed units");

        let filter = EventFilter {
            event_types: vec![GameEventType::UnitDestroyed],
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: super::ScriptPriority::Low,
        };

        let history = get_event_manager().query_history(&filter, 10_000).await?;
        let mut destroyed_count = 0i64;
        for event in history {
            if let Some(player) = _player {
                let owner = event.parameters.get("owner_player").and_then(|v| match v {
                    ScriptValue::Int(i) => Some(*i),
                    ScriptValue::Float(f) => Some(*f as i64),
                    _ => None,
                });
                if owner != Some(player) {
                    continue;
                }
            }

            if let Some(ScriptValue::String(unit_type)) = _unit_type {
                let template = event.parameters.get("template_name").and_then(|v| match v {
                    ScriptValue::String(s) => Some(s),
                    _ => None,
                });
                if template
                    .map(|t| !t.eq_ignore_ascii_case(unit_type))
                    .unwrap_or(true)
                {
                    continue;
                }
            }

            destroyed_count += 1;
        }

        compare_i64(destroyed_count, comparison.as_str(), count)
    }

    fn name(&self) -> &str {
        "units_destroyed"
    }

    fn description(&self) -> &str {
        "Checks the number of units destroyed"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["comparison".to_string(), "count".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "unit_type".to_string()]
    }
}

/// Combat occurred condition
struct CombatOccurredCondition;

#[async_trait]
impl ScriptCondition for CombatOccurredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = super::actions::get_float_param_optional(parameters, "x");
        let y = super::actions::get_float_param_optional(parameters, "y");
        let radius = super::actions::get_float_param_optional(parameters, "radius");
        let time_window =
            super::actions::get_float_param_optional(parameters, "time_window").unwrap_or(60.0);

        log::debug!(
            "Checking if combat occurred in the last {} seconds",
            time_window
        );

        let filter = EventFilter {
            event_types: vec![
                GameEventType::CombatStarted,
                GameEventType::CombatEnded,
                GameEventType::WeaponFired,
                GameEventType::DamageDealt,
                GameEventType::UnitAttacked,
                GameEventType::UnitDamaged,
                GameEventType::UnitKilled,
            ],
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: super::ScriptPriority::Low,
        };

        let history = get_event_manager().query_history(&filter, 256).await?;
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs_f64(time_window);
        for event in history {
            if event.timestamp < cutoff {
                break;
            }

            if let (Some(x), Some(y), Some(radius)) = (x, y, radius) {
                let Some(ScriptValue::Coord3D([ex, ey, _])) = event.parameters.get("position")
                else {
                    continue;
                };
                let dx = *ex as f64 - x;
                let dy = *ey as f64 - y;
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
            }

            return Ok(true);
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "combat_occurred"
    }

    fn description(&self) -> &str {
        "Checks if combat has occurred recently"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
            "time_window".to_string(),
        ]
    }
}

/// Player casualties condition
struct PlayerCasualtiesCondition;

#[async_trait]
impl ScriptCondition for PlayerCasualtiesCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let comparison = super::actions::get_string_param(parameters, "comparison")?;
        let count = super::actions::get_int_param(parameters, "count")?;

        log::debug!("Checking casualties for player {}", player);

        let filter = EventFilter {
            event_types: vec![GameEventType::UnitDestroyed],
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: super::ScriptPriority::Low,
        };

        let history = get_event_manager().query_history(&filter, 10_000).await?;
        let mut casualties = 0i64;
        for event in history {
            let owner = event.parameters.get("owner_player").and_then(|v| match v {
                ScriptValue::Int(i) => Some(*i),
                ScriptValue::Float(f) => Some(*f as i64),
                _ => None,
            });
            if owner == Some(player) {
                casualties += 1;
            }
        }

        compare_i64(casualties, comparison.as_str(), count)
    }

    fn name(&self) -> &str {
        "player_casualties"
    }

    fn description(&self) -> &str {
        "Checks a player's casualty count"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "comparison".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has technology condition
struct PlayerHasTechnologyCondition;

#[async_trait]
impl ScriptCondition for PlayerHasTechnologyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let technology = super::actions::get_string_param(parameters, "technology")?;

        log::debug!(
            "Checking if player {} has technology '{}'",
            player,
            technology
        );
        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(player as i32) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let Some(store) = get_science_store() else {
            return Ok(false);
        };
        let science = store.get_science_from_internal_name(technology.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }
        Ok(player_guard.has_science(science))
    }

    fn name(&self) -> &str {
        "player_has_technology"
    }

    fn description(&self) -> &str {
        "Checks if a player has researched a technology"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "technology".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has upgrade condition
struct PlayerHasUpgradeCondition;

#[async_trait]
impl ScriptCondition for PlayerHasUpgradeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let upgrade = super::actions::get_string_param(parameters, "upgrade")?;

        log::debug!("Checking if player {} has upgrade '{}'", player, upgrade);
        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(player as i32) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let upgrade_center = get_upgrade_center();
        let Ok(center) = upgrade_center.read() else {
            return Ok(false);
        };
        let Some(template) = center.find_upgrade(upgrade.as_str()) else {
            return Ok(false);
        };
        Ok(player_guard.has_upgrade_complete(&template))
    }

    fn name(&self) -> &str {
        "player_has_upgrade"
    }

    fn description(&self) -> &str {
        "Checks if a player has an upgrade"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "upgrade".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Special power available condition
struct SpecialPowerAvailableCondition;

#[async_trait]
impl ScriptCondition for SpecialPowerAvailableCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let power_name = super::actions::get_string_param(parameters, "power_name")?;

        log::debug!(
            "Checking if special power '{}' is available for player {}",
            power_name,
            player
        );

        let player_id: crate::common::ObjectID = if player >= 0 {
            player as crate::common::ObjectID
        } else {
            return Ok(false);
        };

        let Some(registry_lock) = crate::special_power_module::get_power_registry() else {
            return Ok(false);
        };
        let Ok(registry) = registry_lock.read() else {
            return Ok(false);
        };

        let power_name_lower = power_name.to_ascii_lowercase();
        for power in registry.get_all_powers() {
            let Ok(power) = power.lock() else {
                continue;
            };
            if power.get_data().name.to_string().to_ascii_lowercase() != power_name_lower {
                continue;
            }
            if power.get_data().check_prerequisites(player_id) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "special_power_available"
    }

    fn description(&self) -> &str {
        "Checks if a special power is available for use"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// Logical conditions

/// AND condition
struct AndCondition;

#[async_trait]
impl ScriptCondition for AndCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        log::debug!("Evaluating AND condition");
        let Some(ScriptValue::Array(conditions)) = parameters.get("conditions") else {
            return Err(GameLogicError::Configuration(
                "AND condition requires 'conditions' array".to_string(),
            ));
        };

        let registry = ConditionRegistry::new();
        for condition in conditions {
            let (name, params) = parse_nested_condition(condition)?;
            let Some(handler) = registry.get_condition(&name) else {
                return Err(GameLogicError::Configuration(format!(
                    "Unknown condition in AND: {}",
                    name
                )));
            };
            if !handler.evaluate(&params, context).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        "and"
    }

    fn description(&self) -> &str {
        "Logical AND of multiple conditions"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["conditions".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// OR condition
struct OrCondition;

#[async_trait]
impl ScriptCondition for OrCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        log::debug!("Evaluating OR condition");
        let Some(ScriptValue::Array(conditions)) = parameters.get("conditions") else {
            return Err(GameLogicError::Configuration(
                "OR condition requires 'conditions' array".to_string(),
            ));
        };

        let registry = ConditionRegistry::new();
        for condition in conditions {
            let (name, params) = parse_nested_condition(condition)?;
            let Some(handler) = registry.get_condition(&name) else {
                return Err(GameLogicError::Configuration(format!(
                    "Unknown condition in OR: {}",
                    name
                )));
            };
            if handler.evaluate(&params, context).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "or"
    }

    fn description(&self) -> &str {
        "Logical OR of multiple conditions"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["conditions".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// NOT condition
struct NotCondition;

#[async_trait]
impl ScriptCondition for NotCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        log::debug!("Evaluating NOT condition");
        let Some(condition) = parameters.get("condition") else {
            return Err(GameLogicError::Configuration(
                "NOT condition requires 'condition' object".to_string(),
            ));
        };

        let (name, params) = parse_nested_condition(condition)?;
        let registry = ConditionRegistry::new();
        let Some(handler) = registry.get_condition(&name) else {
            return Err(GameLogicError::Configuration(format!(
                "Unknown condition in NOT: {}",
                name
            )));
        };
        Ok(!handler.evaluate(&params, context).await?)
    }

    fn name(&self) -> &str {
        "not"
    }

    fn description(&self) -> &str {
        "Logical NOT of a condition"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["condition".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// Variable conditions

/// Variable equals condition
struct VariableEqualsCondition;

#[async_trait]
impl ScriptCondition for VariableEqualsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let variable_name = super::actions::get_string_param(parameters, "variable_name")?;
        let expected_value = parameters.get("value").ok_or_else(|| {
            GameLogicError::Configuration("Required parameter 'value' not found".to_string())
        })?;

        // Check in context variables first
        if let Some(actual_value) = context.variables.get(&variable_name) {
            Ok(actual_value == expected_value)
        } else {
            // Variable not found
            Ok(false)
        }
    }

    fn name(&self) -> &str {
        "variable_equals"
    }

    fn description(&self) -> &str {
        "Checks if a variable equals a value"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["variable_name".to_string(), "value".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

fn parse_nested_condition(
    value: &ScriptValue,
) -> GameLogicResult<(String, HashMap<String, ScriptValue>)> {
    match value {
        ScriptValue::Object(map) => {
            let name_value = map
                .get("name")
                .or_else(|| map.get("condition"))
                .or_else(|| map.get("type"))
                .ok_or_else(|| {
                    GameLogicError::Configuration(
                        "Nested condition object missing 'name'".to_string(),
                    )
                })?;
            let ScriptValue::String(name) = name_value else {
                return Err(GameLogicError::Configuration(
                    "Nested condition 'name' must be a string".to_string(),
                ));
            };

            let params = match map.get("parameters") {
                Some(ScriptValue::Object(params)) => params.clone(),
                Some(_) => {
                    return Err(GameLogicError::Configuration(
                        "Nested condition 'parameters' must be an object".to_string(),
                    ))
                }
                None => HashMap::new(),
            };

            Ok((name.clone(), params))
        }
        _ => Err(GameLogicError::Configuration(
            "Nested condition must be an object".to_string(),
        )),
    }
}

/// Variable greater than condition
struct VariableGreaterThanCondition;

#[async_trait]
impl ScriptCondition for VariableGreaterThanCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let variable_name = super::actions::get_string_param(parameters, "variable_name")?;
        let threshold = super::actions::get_float_param(parameters, "value")?;

        if let Some(variable_value) = context.variables.get(&variable_name) {
            match variable_value {
                ScriptValue::Int(i) => Ok((*i as f64) > threshold),
                ScriptValue::Float(f) => Ok(*f > threshold),
                _ => Err(GameLogicError::Configuration(format!(
                    "Variable '{}' is not numeric",
                    variable_name
                ))),
            }
        } else {
            Ok(false)
        }
    }

    fn name(&self) -> &str {
        "variable_greater_than"
    }

    fn description(&self) -> &str {
        "Checks if a variable is greater than a value"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["variable_name".to_string(), "value".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Variable less than condition
struct VariableLessThanCondition;

#[async_trait]
impl ScriptCondition for VariableLessThanCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let variable_name = super::actions::get_string_param(parameters, "variable_name")?;
        let threshold = super::actions::get_float_param(parameters, "value")?;

        if let Some(variable_value) = context.variables.get(&variable_name) {
            match variable_value {
                ScriptValue::Int(i) => Ok((*i as f64) < threshold),
                ScriptValue::Float(f) => Ok(*f < threshold),
                _ => Err(GameLogicError::Configuration(format!(
                    "Variable '{}' is not numeric",
                    variable_name
                ))),
            }
        } else {
            Ok(false)
        }
    }

    fn name(&self) -> &str {
        "variable_less_than"
    }

    fn description(&self) -> &str {
        "Checks if a variable is less than a value"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["variable_name".to_string(), "value".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

// ============================================================================
// 20 CORE SCRIPT CONDITIONS - Priority 1 Implementation
// Based on C++ ScriptConditions from GENERALSMD_SCRIPTING_SYSTEM_GUIDE.md
// ============================================================================

/// Counter Comparison Condition - Matches C++ ConditionType::COUNTER
struct CounterComparisonCondition;

#[async_trait]
impl ScriptCondition for CounterComparisonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let counter_name = super::actions::get_string_param(parameters, "counter_name")?;
        let comparison = super::actions::get_string_param(parameters, "comparison")?;
        let value = super::actions::get_int_param(parameters, "value")?;

        log::debug!(
            "Checking counter '{}' {} {}",
            counter_name,
            comparison,
            value
        );

        let counter_value = crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| {
                engine
                    .as_ref()?
                    .get_counter(&counter_name)
                    .map(|counter| counter.value as i64)
            })
            .or_else(|| {
                _context.variables.get(&counter_name).and_then(|v| match v {
                    ScriptValue::Int(i) => Some(*i),
                    _ => None,
                })
            })
            .unwrap_or(0i64);

        let result = match comparison.as_str() {
            "less" => counter_value < value,
            "less_equal" => counter_value <= value,
            "equal" => counter_value == value,
            "greater_equal" => counter_value >= value,
            "greater" => counter_value > value,
            "not_equal" => counter_value != value,
            _ => {
                return Err(GameLogicError::Configuration(format!(
                    "Invalid comparison operator: {}",
                    comparison
                )))
            }
        };

        Ok(result)
    }

    fn name(&self) -> &str {
        "counter_comparison"
    }

    fn description(&self) -> &str {
        "Compares a counter value against a threshold"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "counter_name".to_string(),
            "comparison".to_string(),
            "value".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Flag Comparison Condition - Matches C++ ConditionType::FLAG
struct FlagComparisonCondition;

#[async_trait]
impl ScriptCondition for FlagComparisonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let flag_name = super::actions::get_string_param(parameters, "flag_name")?;
        let expected_value = parameters
            .get("value")
            .map(|v| match v {
                ScriptValue::Bool(b) => *b,
                ScriptValue::Int(i) => *i != 0,
                _ => false,
            })
            .unwrap_or(true);

        log::debug!("Checking flag '{}' == {}", flag_name, expected_value);

        // Get flag value from ScriptEngine flag system
        // Matches C++ Script::Get_Flag()->Get_Value()
        // For now, check context variables which are populated by the engine
        let flag_value = _context
            .variables
            .get(&flag_name)
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                ScriptValue::Int(i) => Some(*i != 0),
                _ => None,
            })
            .unwrap_or(false);

        Ok(flag_value == expected_value)
    }

    fn name(&self) -> &str {
        "flag_comparison"
    }

    fn description(&self) -> &str {
        "Checks if a flag equals a boolean value"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["flag_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["value".to_string()]
    }
}

/// Position In Area Condition - Checks if position/unit is inside area
struct PositionInAreaCondition;

#[async_trait]
impl ScriptCondition for PositionInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = super::actions::get_int_param_optional(parameters, "object_id");
        let pos_x = super::actions::get_float_param_optional(parameters, "x");
        let pos_y = super::actions::get_float_param_optional(parameters, "y");
        let area_x = super::actions::get_float_param(parameters, "area_x")?;
        let area_y = super::actions::get_float_param(parameters, "area_y")?;
        let area_radius = super::actions::get_float_param(parameters, "area_radius")?;

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

/// Resources Exceed Condition - Player has more resources than threshold
struct ResourcesExceedCondition;

#[async_trait]
impl ScriptCondition for ResourcesExceedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let amount = super::actions::get_int_param(parameters, "amount")?;

        log::debug!(
            "Checking if player {} has more than {} resources",
            player,
            amount
        );

        // Get actual player resources (money)
        // In C++: pPlayer->Get_Money() > amount
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    let player_money = player_guard.get_money().get_money() as i64;
                    return Ok(player_money > amount);
                }
            }
        }

        // If player not found, return false
        Ok(false)
    }

    fn name(&self) -> &str {
        "resources_exceed"
    }

    fn description(&self) -> &str {
        "Checks if player resources exceed a threshold"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Structure Built Condition - Player has built specific building
struct StructureBuiltCondition;

#[async_trait]
impl ScriptCondition for StructureBuiltCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let building_type = super::actions::get_string_param(parameters, "building_type")?;

        log::debug!(
            "Checking if player {} has built building '{}'",
            player,
            building_type
        );

        // Query player's built structures
        // In C++: Check if pPlayer has building of type in built list
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let owned_objects = manager.get_objects_owned_by_player(player as u32);
            for obj_id in owned_objects {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    if let Ok(obj) = obj_arc.read() {
                        if let Ok(base) = obj.base.read() {
                            // Check if it's a structure and matches the building type
                            if let Some(template) = &obj.template {
                                if template.get_name().eq_ignore_ascii_case(&building_type) {
                                    use crate::common::KindOf;
                                    if base.is_kind_of(KindOf::Structure) {
                                        return Ok(true);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "structure_built"
    }

    fn description(&self) -> &str {
        "Checks if player has built a specific structure"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "building_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Unit Type Count Exceeds - Player has N or more units of type
struct UnitTypeCountExceedsCondition;

#[async_trait]
impl ScriptCondition for UnitTypeCountExceedsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let unit_type = super::actions::get_string_param(parameters, "unit_type")?;
        let count = super::actions::get_int_param(parameters, "count")?;

        log::debug!(
            "Checking if player {} has more than {} units of type '{}'",
            player,
            count,
            unit_type
        );

        // Count player's units of this type
        // In C++: Count objects owned by player with matching template
        let obj_manager = get_object_manager();
        let mut actual_count = 0i64;

        if let Ok(manager) = obj_manager.read() {
            let owned_objects = manager.get_objects_owned_by_player(player as u32);
            for obj_id in owned_objects {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    if let Ok(obj) = obj_arc.read() {
                        if let Some(template) = &obj.template {
                            if template.get_name().eq_ignore_ascii_case(&unit_type) {
                                actual_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(actual_count > count)
    }

    fn name(&self) -> &str {
        "unit_type_count_exceeds"
    }

    fn description(&self) -> &str {
        "Checks if player has more than N units of a specific type"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Team All Units Destroyed - All units in team are dead
struct TeamAllUnitsDestroyedCondition;

#[async_trait]
impl ScriptCondition for TeamAllUnitsDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = super::actions::get_string_param(parameters, "team_name")?;

        log::debug!("Checking if team '{}' is fully destroyed", team_name);

        // Query team status from TeamFactory
        // In C++: Check if all team members are dead/gone
        let team_factory = get_team_factory();
        if let Ok(mut factory) = team_factory.lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Team is destroyed if it has no members or all members are dead
                    if team.get_member_count() == 0 {
                        return Ok(true);
                    }

                    // Check if all members are destroyed
                    let members = team.get_members().to_vec();
                    drop(team); // Drop team guard before getting obj_manager

                    if let Ok(manager) = get_object_manager().read() {
                        for &member_id in &members {
                            if let Some(obj_arc) = manager.get_object(member_id) {
                                if let Ok(obj) = obj_arc.read() {
                                    if !obj.is_destroyed() {
                                        return Ok(false); // At least one member alive
                                    }
                                }
                            }
                        }
                        return Ok(true); // All members destroyed
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_all_units_destroyed"
    }

    fn description(&self) -> &str {
        "Checks if all units in a team have been destroyed"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player Won - Matches C++ victory condition check
struct PlayerWonCondition;

#[async_trait]
impl ScriptCondition for PlayerWonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;

        log::debug!("Checking if player {} has won", player);

        // C++ parity behavior for mission/skirmish checks:
        // a player is considered "won" if they are still active and all other
        // non-observer, non-neutral players are defeated.
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    if player_guard.is_defeated()
                        || player_guard.is_player_observer()
                        || player_guard.get_player_type() == PlayerType::Neutral
                    {
                        return Ok(false);
                    }

                    let this_player_index = player_guard.get_player_index();
                    drop(player_guard);

                    for other_arc in list.iter() {
                        let Ok(other_guard) = other_arc.read() else {
                            continue;
                        };
                        if other_guard.get_player_index() == this_player_index {
                            continue;
                        }
                        if other_guard.is_player_observer()
                            || other_guard.get_player_type() == PlayerType::Neutral
                        {
                            continue;
                        }
                        if !other_guard.is_defeated() {
                            return Ok(false);
                        }
                    }

                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "player_won"
    }

    fn description(&self) -> &str {
        "Checks if player has achieved victory"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// No Enemy Units In Area - Area is clear of enemy forces
struct NoEnemyUnitsInAreaCondition;

#[async_trait]
impl ScriptCondition for NoEnemyUnitsInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let x = super::actions::get_float_param(parameters, "x")?;
        let y = super::actions::get_float_param(parameters, "y")?;
        let radius = super::actions::get_float_param(parameters, "radius")?;

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

/// Allies With Team - Players are allied
struct AlliesWithTeamCondition;

#[async_trait]
impl ScriptCondition for AlliesWithTeamCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player1 = super::actions::get_int_param(parameters, "player1")?;
        let player2 = super::actions::get_int_param(parameters, "player2")?;

        log::debug!("Checking if player {} and {} are allies", player1, player2);

        // Check player relationship
        // In C++: pPlayer1->Get_Relationship(pPlayer2) == ALLIES
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(p1_arc) = list.get_player(player1 as i32) {
                if let Ok(p1) = p1_arc.read() {
                    if let Some(p2_arc) = list.get_player(player2 as i32) {
                        if let Ok(p2) = p2_arc.read() {
                            return Ok(p1.is_allied_with_player(&p2));
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "allies_with_team"
    }

    fn description(&self) -> &str {
        "Checks if two players/teams are allied"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player1".to_string(), "player2".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Building Damaged - Structure health below threshold
struct BuildingDamagedCondition;

#[async_trait]
impl ScriptCondition for BuildingDamagedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = super::actions::get_int_param(parameters, "object_id")?;
        let health_percent = super::actions::get_float_param(parameters, "health_percent")?;

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
struct UnitNearPositionCondition;

#[async_trait]
impl ScriptCondition for UnitNearPositionCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let object_id = super::actions::get_int_param(parameters, "object_id")?;
        let x = super::actions::get_float_param(parameters, "x")?;
        let y = super::actions::get_float_param(parameters, "y")?;
        let distance = super::actions::get_float_param(parameters, "distance")?;

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

/// Units In Formation - Team members are grouped
struct UnitsInFormationCondition;

#[async_trait]
impl ScriptCondition for UnitsInFormationCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = super::actions::get_string_param(parameters, "team_name")?;
        let max_distance =
            super::actions::get_float_param_optional(parameters, "max_distance").unwrap_or(50.0);

        log::debug!(
            "Checking if team '{}' is in formation (max distance: {})",
            team_name,
            max_distance
        );

        // Check team member positions, verify they're close together
        // In C++: Calculate spread of team positions
        let team_factory = get_team_factory();
        if let Ok(mut factory) = team_factory.lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(false);
                    }

                    // Calculate center of mass
                    let member_ids = members.to_vec();
                    drop(team); // Drop team guard before getting obj_manager

                    if let Ok(manager) = get_object_manager().read() {
                        let mut positions = Vec::new();
                        for &member_id in &member_ids {
                            if let Some(obj_arc) = manager.get_object(member_id) {
                                if let Ok(obj) = obj_arc.read() {
                                    let pos = obj.get_position();
                                    positions.push((pos.x, pos.y));
                                }
                            }
                        }

                        if positions.is_empty() {
                            return Ok(false);
                        }

                        // Calculate center
                        let center_x =
                            positions.iter().map(|(x, _)| x).sum::<f32>() / positions.len() as f32;
                        let center_y =
                            positions.iter().map(|(_, y)| y).sum::<f32>() / positions.len() as f32;

                        // Check if all units are within max_distance of center
                        for (px, py) in positions {
                            let dist = ((px - center_x).powi(2) + (py - center_y).powi(2)).sqrt();
                            if dist > max_distance as f32 {
                                return Ok(false);
                            }
                        }
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "units_in_formation"
    }

    fn description(&self) -> &str {
        "Checks if team units are grouped together in formation"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["max_distance".to_string()]
    }
}

/// Research Complete - Player has completed science/upgrade
struct ResearchCompleteCondition;

#[async_trait]
impl ScriptCondition for ResearchCompleteCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let science_name = super::actions::get_string_param(parameters, "science_name")?;

        log::debug!(
            "Checking if player {} has completed research '{}'",
            player,
            science_name
        );

        // Check player science/upgrade state
        // Matches C++ Player::Has_Science(science_name)
        use game_engine::common::rts::ScienceType;

        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(_player_guard) = player_arc.read() {
                    // Science system integration: Check player's completed sciences
                    // The science name needs to be mapped to ScienceType enum
                    // This requires a string->enum mapping table in the science system
                    // For now, we cannot directly check without the mapping
                    // Full implementation needs: ScienceType::from_name(science_name)
                    return Ok(false);
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "research_complete"
    }

    fn description(&self) -> &str {
        "Checks if player has completed a research/science"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Special Power Ready - Player can use special power
struct SpecialPowerReadyCondition;

#[async_trait]
impl ScriptCondition for SpecialPowerReadyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = super::actions::get_int_param(parameters, "player")?;
        let power_name = super::actions::get_string_param(parameters, "power_name")?;

        log::debug!(
            "Checking if special power '{}' is ready for player {}",
            power_name,
            player
        );

        let player_id: crate::common::ObjectID = if player >= 0 {
            player as crate::common::ObjectID
        } else {
            return Ok(false);
        };

        let Some(registry_lock) = crate::special_power_module::get_power_registry() else {
            return Ok(false);
        };
        let Ok(registry) = registry_lock.read() else {
            return Ok(false);
        };

        let power_name_lower = power_name.to_ascii_lowercase();
        for power in registry.get_all_powers() {
            let Ok(power) = power.lock() else {
                continue;
            };
            if power.get_data().name.to_string().to_ascii_lowercase() != power_name_lower {
                continue;
            }

            if !power.get_data().check_prerequisites(player_id) {
                continue;
            }

            if power.is_ready() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "special_power_ready"
    }

    fn description(&self) -> &str {
        "Checks if special power is available/ready for use"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Any Unit In Area - At least one unit in area
struct AnyUnitInAreaCondition;

#[async_trait]
impl ScriptCondition for AnyUnitInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = super::actions::get_float_param(parameters, "x")?;
        let y = super::actions::get_float_param(parameters, "y")?;
        let radius = super::actions::get_float_param(parameters, "radius")?;
        let _player = super::actions::get_int_param_optional(parameters, "player");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    #[tokio::test]
    async fn test_condition_registry() {
        let registry = ConditionRegistry::new();

        let conditions = registry.list_conditions();
        assert!(conditions.contains(&"player_alive".to_string()));
        assert!(conditions.contains(&"object_exists".to_string()));
        assert!(conditions.contains(&"game_time".to_string()));
    }

    #[tokio::test]
    async fn test_game_time_condition() {
        let condition = GameTimeCondition;
        let mut params = HashMap::new();
        params.insert(
            "comparison".to_string(),
            ScriptValue::String("greater".to_string()),
        );
        params.insert("time".to_string(), ScriptValue::Float(30.0));

        let context = ScriptContext {
            game_time: Duration::from_secs(60),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = condition.evaluate(&params, &context).await.unwrap();
        assert!(result); // 60 > 30
    }

    #[tokio::test]
    async fn test_variable_equals_condition() {
        let condition = VariableEqualsCondition;
        let mut params = HashMap::new();
        params.insert(
            "variable_name".to_string(),
            ScriptValue::String("test_var".to_string()),
        );
        params.insert("value".to_string(), ScriptValue::Int(42));

        let mut variables = HashMap::new();
        variables.insert("test_var".to_string(), ScriptValue::Int(42));

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables,
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = condition.evaluate(&params, &context).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_object_health_condition() {
        use std::sync::{Arc, RwLock};

        use crate::common::Coord3D;
        use crate::common::DefaultThingTemplate;
        use crate::object_manager::{get_object_manager, GameObjectInstance, ObjectCreationFlags};

        if let Ok(mut manager) = get_object_manager().write() {
            manager.reset();

            let template = Arc::new(DefaultThingTemplate::new("TestObject".to_string()));
            let instance = Arc::new(RwLock::new(
                GameObjectInstance::new(123, Some(template), None, ObjectCreationFlags::new())
                    .expect("failed to create object instance"),
            ));
            manager
                .register_object_instance(instance, Coord3D::new(0.0, 0.0, 0.0))
                .unwrap();
        }

        let condition = ObjectHealthCondition;
        let mut params = HashMap::new();
        params.insert("object_id".to_string(), ScriptValue::Int(123));
        params.insert(
            "comparison".to_string(),
            ScriptValue::String("greater".to_string()),
        );
        params.insert("value".to_string(), ScriptValue::Float(50.0));

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let result = condition.evaluate(&params, &context).await.unwrap();
        assert!(result);

        if let Ok(mut manager) = get_object_manager().write() {
            manager.reset();
        }
    }
}
