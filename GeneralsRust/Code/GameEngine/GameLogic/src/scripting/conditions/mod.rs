//! Script Conditions System
//!
//! This module provides condition evaluation for script triggers and decision making.

pub mod skirmish_conditions;

pub use super::{ScriptContext, ScriptValue};
use crate::common::{Coord3D, KindOf, Relationship, LOGICFRAMES_PER_SECOND};
use crate::helpers::{TheGameLogic, ThePartitionManager, TheVictoryConditions};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::{player_list, Player, PlayerType};
use crate::scripting::engine::{
    get_area_tracker, get_event_manager, get_named_object_tracker, get_script_engine,
};
use crate::scripting::events::{EventFilter, GameEventType};
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::upgrade::center::get_upgrade_center;
use crate::{GameLogicError, GameLogicResult};

use async_trait::async_trait;
use game_engine::common::rts::{get_science_store, SCIENCE_INVALID};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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

/// Helper: get string parameter from condition parameters
pub(crate) fn get_str_param(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> GameLogicResult<String> {
    match parameters.get(key) {
        Some(ScriptValue::String(s)) => Ok(s.clone()),
        Some(v) => Err(GameLogicError::Configuration(format!(
            "Expected string for '{}', got {:?}",
            key, v
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Missing parameter '{}'",
            key
        ))),
    }
}

/// Helper: get optional string parameter
fn get_str_param_optional(parameters: &HashMap<String, ScriptValue>, key: &str) -> Option<String> {
    match parameters.get(key) {
        Some(ScriptValue::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Helper: get player arc from parameter value
pub(crate) fn get_player_arc(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> GameLogicResult<Option<Arc<RwLock<Player>>>> {
    let val = parameters
        .get(key)
        .ok_or_else(|| GameLogicError::Configuration(format!("Missing parameter '{}'", key)))?;
    match val {
        ScriptValue::PlayerId(id) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            Ok(guard.get_player(*id as i32).cloned())
        }
        ScriptValue::String(name) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            for i in 0..guard.get_player_count() {
                if let Some(player_arc) = guard.get_player(i as i32) {
                    if let Ok(player) = player_arc.read() {
                        if player.get_general_name() == name.as_str() {
                            return Ok(Some(player_arc.clone()));
                        }
                    }
                }
            }
            Ok(None)
        }
        ScriptValue::Int(id) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            Ok(guard.get_player(*id as i32).cloned())
        }
        _ => Err(GameLogicError::Configuration(format!(
            "Expected player id/name for '{}', got {:?}",
            key, val
        ))),
    }
}

/// Helper: look up a named object from the script engine's named object tracker.
/// Returns the ObjectID if found.
pub(crate) fn lookup_named_object_id(name: &str) -> GameLogicResult<Option<u32>> {
    let tracker = get_named_object_tracker();
    tracker.get_object_id(name)
}

/// Helper: perform C++-style comparison (less_than, less_equal, equal, etc.)
pub(crate) fn perform_comparison(actual: i64, comparison: &str, expected: i64) -> bool {
    match comparison.to_lowercase().as_str() {
        "less_than" | "<" => actual < expected,
        "less_equal" | "<=" => actual <= expected,
        "equal" | "==" | "=" => actual == expected,
        "greater_equal" | ">=" => actual >= expected,
        "greater" | ">" => actual > expected,
        "not_equal" | "!=" => actual != expected,
        _ => false,
    }
}

fn with_script_engine_mut<R>(
    f: impl FnOnce(&mut crate::scripting::engine::ScriptEngine) -> R,
) -> Option<R> {
    let engine = get_script_engine();
    let mut engine_guard = engine.write().ok()?;
    engine_guard.as_mut().map(f)
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

        // Register skirmish AI conditions
        skirmish_conditions::register_skirmish_conditions(&mut registry);

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

        // C++ parity conditions - ported from ScriptConditions.cpp
        // Named object conditions
        self.register_condition(Box::new(NamedUnitExistsCondition));
        self.register_condition(Box::new(NamedUnitDestroyedCondition));
        self.register_condition(Box::new(NamedUnitDyingCondition));
        self.register_condition(Box::new(NamedUnitTotallyDeadCondition));
        self.register_condition(Box::new(NamedOwnedByPlayerCondition));
        self.register_condition(Box::new(NamedInsideAreaCondition));
        self.register_condition(Box::new(NamedOutsideAreaCondition));
        self.register_condition(Box::new(NamedDiscoveredCondition));
        self.register_condition(Box::new(NamedBuildingIsEmptyCondition));
        self.register_condition(Box::new(NamedHasFreeContainerSlotsCondition));
        self.register_condition(Box::new(NamedCreatedCondition));
        self.register_condition(Box::new(NamedSelectedCondition));
        self.register_condition(Box::new(NamedReachedWaypointsEndCondition));

        // Player conditions
        self.register_condition(Box::new(PlayerAllDestroyedCondition));
        self.register_condition(Box::new(PlayerHasCreditsCondition));
        self.register_condition(Box::new(PlayerHasPowerCondition));
        self.register_condition(Box::new(PlayerHasNoPowerCondition));

        // Team conditions
        self.register_condition(Box::new(TeamDestroyedCondition));
        self.register_condition(Box::new(TeamHasUnitsCondition));
        self.register_condition(Box::new(TeamStateIsCondition));
        self.register_condition(Box::new(TeamStateIsNotCondition));
        self.register_condition(Box::new(TeamOwnedByPlayerCondition));
        self.register_condition(Box::new(TeamDiscoveredCondition));
        self.register_condition(Box::new(TeamCreatedCondition));

        // Bridge conditions
        self.register_condition(Box::new(BridgeRepairedCondition));
        self.register_condition(Box::new(BridgeBrokenCondition));

        // Area/team location conditions
        self.register_condition(Box::new(TeamInsideAreaPartiallyCondition));
        self.register_condition(Box::new(TeamInsideAreaEntirelyCondition));
        self.register_condition(Box::new(TeamOutsideAreaEntirelyCondition));

        // Building entry conditions
        self.register_condition(Box::new(BuildingEnteredByPlayerCondition));

        // Object status conditions
        self.register_condition(Box::new(UnitHasObjectStatusCondition));
        self.register_condition(Box::new(TeamAllHasObjectStatusCondition));
        self.register_condition(Box::new(TeamSomeHasObjectStatusCondition));

        // Built-by conditions
        self.register_condition(Box::new(BuiltByPlayerCondition));

        // Area entry/exit conditions (C++ NAMED_ENTERED/EXITED_AREA, TEAM_ENTERED/EXITED_AREA)
        self.register_condition(Box::new(NamedEnteredAreaCondition));
        self.register_condition(Box::new(NamedExitedAreaCondition));
        self.register_condition(Box::new(TeamEnteredAreaEntirelyCondition));
        self.register_condition(Box::new(TeamEnteredAreaPartiallyCondition));
        self.register_condition(Box::new(TeamExitedAreaEntirelyCondition));
        self.register_condition(Box::new(TeamExitedAreaPartiallyCondition));

        // Special power conditions
        self.register_condition(Box::new(PlayerTriggeredSpecialPowerCondition));
        self.register_condition(Box::new(PlayerTriggeredSpecialPowerFromNamedCondition));
        self.register_condition(Box::new(PlayerMidwaySpecialPowerCondition));
        self.register_condition(Box::new(PlayerMidwaySpecialPowerFromNamedCondition));
        self.register_condition(Box::new(PlayerCompletedSpecialPowerCondition));
        self.register_condition(Box::new(PlayerCompletedSpecialPowerFromNamedCondition));
        self.register_condition(Box::new(PlayerBuiltUpgradeCondition));
        self.register_condition(Box::new(PlayerBuiltUpgradeFromNamedCondition));

        // Science/upgrade conditions
        self.register_condition(Box::new(PlayerAcquiredScienceCondition));
        self.register_condition(Box::new(PlayerCanPurchaseScienceCondition));
        self.register_condition(Box::new(PlayerHasSciencePurchasePointsCondition));

        // Power comparison conditions
        self.register_condition(Box::new(PlayerPowerComparePercentCondition));
        self.register_condition(Box::new(PlayerExcessPowerCompareValueCondition));

        // Multiplayer conditions
        self.register_condition(Box::new(MultiplayerAlliedVictoryCondition));
        self.register_condition(Box::new(MultiplayerAlliedDefeatCondition));
        self.register_condition(Box::new(MultiplayerPlayerDefeatCondition));

        // Audio/video conditions
        self.register_condition(Box::new(VideoCompletedCondition));
        self.register_condition(Box::new(SpeechCompletedCondition));
        self.register_condition(Box::new(AudioCompletedCondition));
        self.register_condition(Box::new(MusicTrackCompletedCondition));

        // Other conditions
        self.register_condition(Box::new(CameraMovementFinishedCondition));
        self.register_condition(Box::new(MissionAttemptsCondition));
        self.register_condition(Box::new(UnitEmptiedCondition));
        self.register_condition(Box::new(PlayerLostObjectTypeCondition));

        // C++ parity conditions ported from ScriptConditions.cpp
        self.register_condition(Box::new(ConditionFalseCondition));
        self.register_condition(Box::new(ConditionTrueCondition));
        self.register_condition(Box::new(TimerExpiredCondition));
        self.register_condition(Box::new(EnemySightedCondition));
        self.register_condition(Box::new(UnitHealthCondition));
        self.register_condition(Box::new(PlayerHasObjectComparisonCondition));
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

        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    if super::actions::is_money_resource(&resource_type) {
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
            let __base_arc = obj_guard.base();
            let Ok(base_guard) = __base_arc.read() else {
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
            let __base_arc = obj_guard.base();
            let Ok(base_guard) = __base_arc.read() else {
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
        Ok(matches!(rel, crate::common::Relationship::Allies))
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

        let flag_value = get_script_engine()
            .read()
            .ok()
            .and_then(|engine_guard| {
                engine_guard
                    .as_ref()
                    .and_then(|engine| engine.get_flag(&flag_name).map(|flag| flag.value))
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
                    let (template_name, base_arc) = match obj_arc.read() {
                        Ok(obj) => (
                            obj.template.as_ref().map(|t| t.get_name().to_string()),
                            Some(obj.base()),
                        ),
                        Err(_) => (None, None),
                    };
                    if let (Some(template_name), Some(base_arc)) = (template_name, base_arc) {
                        if let Ok(base) = base_arc.read() {
                            // Check if it's a structure and matches the building type
                            if template_name.eq_ignore_ascii_case(&building_type) {
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
        let science = store.get_science_from_internal_name(science_name.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        Ok(player_guard.has_science(science))
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

//=================================================================================================
// C++ Parity Script Conditions
// Ported from GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptConditions.cpp
//=================================================================================================

//-------------------------------------------------------------------------------------------------
// PLAYER_ALL_DESTROYED - evaluateAllDestroyed
// Returns true if player has no objects (everything destroyed).
//-------------------------------------------------------------------------------------------------
struct PlayerAllDestroyedCondition;

#[async_trait]
impl ScriptCondition for PlayerAllDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(true), // Non-existent player is all destroyed
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        Ok(!player.has_any_objects())
    }

    fn name(&self) -> &str {
        "player_all_destroyed"
    }
    fn description(&self) -> &str {
        "Checks if a player has no objects remaining (C++ PLAYER_ALL_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_CREDITS - evaluatePlayerHasCredits
// Compares player's money against a threshold using a comparison operator.
//-------------------------------------------------------------------------------------------------
struct PlayerHasCreditsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasCreditsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let credits = super::actions::get_int_param(parameters, "credits")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let money = player.get_money().count_money() as i64;
        Ok(perform_comparison(credits, &comparison, money))
    }

    fn name(&self) -> &str {
        "player_has_credits"
    }
    fn description(&self) -> &str {
        "Checks if player has credits matching comparison (C++ PLAYER_HAS_CREDITS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "credits".to_string(),
            "comparison".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_POWER - evaluatePlayerHasPower
// Returns true if player has sufficient power.
//-------------------------------------------------------------------------------------------------
struct PlayerHasPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerHasPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        Ok(player.get_energy().has_sufficient_power())
    }

    fn name(&self) -> &str {
        "player_has_power"
    }
    fn description(&self) -> &str {
        "Checks if player has sufficient power (C++ PLAYER_HAS_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_NO_POWER - !evaluatePlayerHasPower
//-------------------------------------------------------------------------------------------------
struct PlayerHasNoPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerHasNoPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        PlayerHasPowerCondition
            .evaluate(parameters, context)
            .await
            .map(|b| !b)
    }

    fn name(&self) -> &str {
        "player_has_no_power"
    }
    fn description(&self) -> &str {
        "Checks if player does NOT have sufficient power (C++ PLAYER_HAS_NO_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_NOT_DESTROYED - evaluateNamedUnitExists
// Returns true if named unit exists and is not effectively dead.
//-------------------------------------------------------------------------------------------------
struct NamedUnitExistsCondition;

#[async_trait]
impl ScriptCondition for NamedUnitExistsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| !obj.is_effectively_dead())
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_unit_exists"
    }
    fn description(&self) -> &str {
        "Checks if named unit exists and is not dead (C++ NAMED_NOT_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_DESTROYED - evaluateNamedUnitDestroyed
// Returns true if named unit is effectively dead, or existed previously but no longer exists.
//-------------------------------------------------------------------------------------------------
struct NamedUnitDestroyedCondition;

#[async_trait]
impl ScriptCondition for NamedUnitDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => {
                // Object not in tracker — check if it previously existed
                let tracker = get_named_object_tracker();
                return tracker.did_object_exist(&unit_name);
            }
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| obj.is_effectively_dead())
            .unwrap_or(true)) // Was in tracker but gone from registry = destroyed
    }

    fn name(&self) -> &str {
        "named_unit_destroyed"
    }
    fn description(&self) -> &str {
        "Checks if named unit is destroyed (C++ NAMED_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_DYING - evaluateNamedUnitDying
// Returns true if named unit exists and is effectively dead (dying but not yet fully removed).
//-------------------------------------------------------------------------------------------------
struct NamedUnitDyingCondition;

#[async_trait]
impl ScriptCondition for NamedUnitDyingCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false), // Already totally dead, not just dying
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| obj.is_effectively_dead())
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_unit_dying"
    }
    fn description(&self) -> &str {
        "Checks if named unit is dying but not yet fully removed (C++ NAMED_DYING)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_TOTALLY_DEAD - evaluateNamedUnitTotallyDead
// Returns true if named unit previously existed but no longer exists in the object registry.
//-------------------------------------------------------------------------------------------------
struct NamedUnitTotallyDeadCondition;

#[async_trait]
impl ScriptCondition for NamedUnitTotallyDeadCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => {
                // Not in tracker — check history
                let tracker = get_named_object_tracker();
                return tracker.did_object_exist(&unit_name);
            }
        };
        // If still in tracker AND in registry, not totally dead
        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_some() {
            Ok(false)
        } else {
            let tracker = get_named_object_tracker();
            tracker.did_object_exist(&unit_name)
        }
    }

    fn name(&self) -> &str {
        "named_unit_totally_dead"
    }
    fn description(&self) -> &str {
        "Checks if named unit has been fully removed from the game (C++ NAMED_TOTALLY_DEAD)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_OWNED_BY_PLAYER - evaluateNamedOwnedByPlayer
//-------------------------------------------------------------------------------------------------
struct NamedOwnedByPlayerCondition;

#[async_trait]
impl ScriptCondition for NamedOwnedByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_id = player.get_id() as u32;
        drop(player);

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                obj.get_controlling_player_id() == Some(player_id)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_owned_by_player"
    }
    fn description(&self) -> &str {
        "Checks if named unit is owned by a specific player (C++ NAMED_OWNED_BY_PLAYER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_INSIDE_AREA - evaluateNamedInsideArea
//-------------------------------------------------------------------------------------------------
struct NamedInsideAreaCondition;

#[async_trait]
impl ScriptCondition for NamedInsideAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let area_name = get_str_param(parameters, "area_name")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_none() {
            return Ok(false);
        }

        // Check if object is in the area's tracked objects
        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        Ok(objects_in_area.contains(&object_id))
    }

    fn name(&self) -> &str {
        "named_inside_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit is inside a trigger area (C++ NAMED_INSIDE_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_OUTSIDE_AREA - evaluateNamedOutsideArea
//-------------------------------------------------------------------------------------------------
struct NamedOutsideAreaCondition;

#[async_trait]
impl ScriptCondition for NamedOutsideAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        NamedInsideAreaCondition
            .evaluate(parameters, context)
            .await
            .map(|b| !b)
    }

    fn name(&self) -> &str {
        "named_outside_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit is outside a trigger area (C++ NAMED_OUTSIDE_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_DISCOVERED - evaluateNamedDiscovered
//-------------------------------------------------------------------------------------------------
struct NamedDiscoveredCondition;

#[async_trait]
impl ScriptCondition for NamedDiscoveredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_index = player.get_id() as u32;
        drop(player);

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                // Held/disabled objects are not visible
                if obj.is_disabled_by_type(crate::common::DisabledType::Held) {
                    return false;
                }
                obj.is_visible_to_player(player_index)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_discovered"
    }
    fn description(&self) -> &str {
        "Checks if named unit has been discovered by a player (C++ NAMED_DISCOVERED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_BUILDING_IS_EMPTY - evaluateIsBuildingEmpty
//-------------------------------------------------------------------------------------------------
struct NamedBuildingIsEmptyCondition;

#[async_trait]
impl ScriptCondition for NamedBuildingIsEmptyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let building_name = get_str_param(parameters, "building_name")?;

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
                    .map(|contain_guard| contain_guard.get_contain_count() == 0)
                    .unwrap_or(false)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_building_is_empty"
    }
    fn description(&self) -> &str {
        "Checks if named building has no units inside (C++ NAMED_BUILDING_IS_EMPTY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["building_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_HAS_FREE_CONTAINER_SLOTS - evaluateNamedHasFreeContainerSlots
//-------------------------------------------------------------------------------------------------
struct NamedHasFreeContainerSlotsCondition;

#[async_trait]
impl ScriptCondition for NamedHasFreeContainerSlotsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
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
                        let max = contain_guard.get_contain_max() as u32;
                        let cur = contain_guard.get_contain_count();
                        cur < max
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_has_free_container_slots"
    }
    fn description(&self) -> &str {
        "Checks if named unit has free container/garrison slots (C++ NAMED_HAS_FREE_CONTAINER_SLOTS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_CREATED - evaluateNamedCreated
//-------------------------------------------------------------------------------------------------
struct NamedCreatedCondition;

#[async_trait]
impl ScriptCondition for NamedCreatedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        match lookup_named_object_id(&unit_name)? {
            Some(id) => {
                // Also verify the object actually exists in the registry
                Ok(OBJECT_REGISTRY.with_object(id, |_| ()).is_some())
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "named_created"
    }
    fn description(&self) -> &str {
        "Checks if named unit has been created (C++ NAMED_CREATED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_SELECTED - evaluateNamedSelected
//-------------------------------------------------------------------------------------------------
struct NamedSelectedCondition;

#[async_trait]
impl ScriptCondition for NamedSelectedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };

        // Check if object is in the current selection via selection manager
        let sel_mgr = crate::commands::selection::get_selection_manager();
        let mgr = sel_mgr.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read selection manager: {}", e))
        })?;
        Ok(mgr.is_object_selected_by_any_player(object_id))
    }

    fn name(&self) -> &str {
        "named_selected"
    }
    fn description(&self) -> &str {
        "Checks if named unit is currently selected (C++ NAMED_SELECTED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_REACHED_WAYPOINTS_END - evaluateNamedReachedWaypointsEnd
//-------------------------------------------------------------------------------------------------
struct NamedReachedWaypointsEndCondition;

#[async_trait]
impl ScriptCondition for NamedReachedWaypointsEndCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let _waypoint_path = get_str_param(parameters, "waypoint_path")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                let Some(ai) = obj.get_ai_update_interface() else {
                    return false;
                };
                ai.try_lock()
                    .ok()
                    .map(|ai_guard| ai_guard.is_idle())
                    .unwrap_or(false)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_reached_waypoints_end"
    }
    fn description(&self) -> &str {
        "Checks if named unit has reached the end of its waypoint path (C++ NAMED_REACHED_WAYPOINTS_END)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "waypoint_path".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_DESTROYED - evaluateIsDestroyed
//-------------------------------------------------------------------------------------------------
struct TeamDestroyedCondition;

#[async_trait]
impl ScriptCondition for TeamDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(!team.has_any_objects())
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_destroyed"
    }
    fn description(&self) -> &str {
        "Checks if a team has been destroyed (no objects remaining) (C++ TEAM_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_HAS_UNITS - evaluateHasUnits
//-------------------------------------------------------------------------------------------------
struct TeamHasUnitsCondition;

#[async_trait]
impl ScriptCondition for TeamHasUnitsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let factory = get_team_factory();
        let guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        for team_arc in guard.find_team_instances(&team_name) {
            let team = team_arc
                .read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;
            if team.has_any_units() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_has_units"
    }
    fn description(&self) -> &str {
        "Checks if a team has any living units (C++ TEAM_HAS_UNITS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_STATE_IS - evaluateTeamStateIs
//-------------------------------------------------------------------------------------------------
struct TeamStateIsCondition;

#[async_trait]
impl ScriptCondition for TeamStateIsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let state_name = get_str_param(parameters, "state")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(team.get_state().str() == state_name)
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_state_is"
    }
    fn description(&self) -> &str {
        "Checks if team's state matches a specific state (C++ TEAM_STATE_IS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "state".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_STATE_IS_NOT - evaluateTeamStateIsNot
//-------------------------------------------------------------------------------------------------
struct TeamStateIsNotCondition;

#[async_trait]
impl ScriptCondition for TeamStateIsNotCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        TeamStateIsCondition
            .evaluate(parameters, context)
            .await
            .map(|b| !b)
    }

    fn name(&self) -> &str {
        "team_state_is_not"
    }
    fn description(&self) -> &str {
        "Checks if team's state does NOT match a specific state (C++ TEAM_STATE_IS_NOT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "state".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_OWNED_BY_PLAYER - evaluateTeamOwnedByPlayer
//-------------------------------------------------------------------------------------------------
struct TeamOwnedByPlayerCondition;

#[async_trait]
impl ScriptCondition for TeamOwnedByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_id = player.get_id() as u32;
        drop(player);

        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(team.get_controlling_player_id() == Some(player_id))
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_owned_by_player"
    }
    fn description(&self) -> &str {
        "Checks if a team is owned by a specific player (C++ TEAM_OWNED_BY_PLAYER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_DISCOVERED - evaluateTeamDiscovered
// Returns true if any member of the team is visible to the specified player.
//-------------------------------------------------------------------------------------------------
struct TeamDiscoveredCondition;

#[async_trait]
impl ScriptCondition for TeamDiscoveredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_index = player.get_id() as u32;
        drop(player);

        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        for &member_id in team.get_members() {
            let visible = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    !obj.is_disabled_by_type(crate::common::DisabledType::Held)
                        && obj.is_visible_to_player(player_index)
                })
                .unwrap_or(false);
            if visible {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_discovered"
    }
    fn description(&self) -> &str {
        "Checks if any team member is visible to a player (C++ TEAM_DISCOVERED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_CREATED - evaluateTeamCreated
//-------------------------------------------------------------------------------------------------
struct TeamCreatedCondition;

#[async_trait]
impl ScriptCondition for TeamCreatedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(team.is_created())
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_created"
    }
    fn description(&self) -> &str {
        "Checks if a team has been created (C++ TEAM_CREATED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// BRIDGE_REPAIRED - evaluateBridgeRepaired
//-------------------------------------------------------------------------------------------------
struct BridgeRepairedCondition;

#[async_trait]
impl ScriptCondition for BridgeRepairedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let bridge_name = get_str_param(parameters, "bridge_name")?;
        let object_id = match lookup_named_object_id(&bridge_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        let terrain = get_terrain_logic()
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read terrain: {}", e)))?;
        Ok(terrain.is_bridge_repaired(object_id))
    }

    fn name(&self) -> &str {
        "bridge_repaired"
    }
    fn description(&self) -> &str {
        "Checks if a named bridge has been repaired (C++ BRIDGE_REPAIRED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["bridge_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// BRIDGE_BROKEN - evaluateBridgeBroken
//-------------------------------------------------------------------------------------------------
struct BridgeBrokenCondition;

#[async_trait]
impl ScriptCondition for BridgeBrokenCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let bridge_name = get_str_param(parameters, "bridge_name")?;
        let object_id = match lookup_named_object_id(&bridge_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        let terrain = get_terrain_logic()
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read terrain: {}", e)))?;
        Ok(terrain.is_bridge_broken(object_id))
    }

    fn name(&self) -> &str {
        "bridge_broken"
    }
    fn description(&self) -> &str {
        "Checks if a named bridge is broken (C++ BRIDGE_BROKEN)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["bridge_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_INSIDE_AREA_PARTIALLY - evaluateTeamInsideAreaPartially
//-------------------------------------------------------------------------------------------------
struct TeamInsideAreaPartiallyCondition;

#[async_trait]
impl ScriptCondition for TeamInsideAreaPartiallyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let members = team.get_members();
        let mut inside_count = 0u32;

        for &member_id in members {
            let area_tracker = get_area_tracker();
            let objects_in_area = area_tracker
                .get_objects_in_area(&area_name)
                .unwrap_or_default();
            if objects_in_area.contains(&member_id) {
                inside_count += 1;
            }
        }

        Ok(inside_count > 0)
    }

    fn name(&self) -> &str {
        "team_inside_area_partially"
    }
    fn description(&self) -> &str {
        "Checks if any team member is inside an area (C++ TEAM_INSIDE_AREA_PARTIALLY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_INSIDE_AREA_ENTIRELY - evaluateTeamInsideAreaEntirely
//-------------------------------------------------------------------------------------------------
struct TeamInsideAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamInsideAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let members = team.get_members();
        if members.is_empty() {
            return Ok(false);
        }

        let area_tracker = get_area_tracker();
        for &member_id in members {
            let objects_in_area = area_tracker
                .get_objects_in_area(&area_name)
                .unwrap_or_default();
            if !objects_in_area.contains(&member_id) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        "team_inside_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members are inside an area (C++ TEAM_INSIDE_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_OUTSIDE_AREA_ENTIRELY - evaluateTeamOutsideAreaEntirely
//-------------------------------------------------------------------------------------------------
struct TeamOutsideAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamOutsideAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Not entirely inside AND not partially inside = entirely outside
        let entirely_inside = TeamInsideAreaEntirelyCondition
            .evaluate(parameters, context)
            .await?;
        let partially_inside = TeamInsideAreaPartiallyCondition
            .evaluate(parameters, context)
            .await?;
        Ok(!entirely_inside && !partially_inside)
    }

    fn name(&self) -> &str {
        "team_outside_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members are outside an area (C++ TEAM_OUTSIDE_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// BUILDING_ENTERED_BY_PLAYER - evaluateBuildingEntered
//-------------------------------------------------------------------------------------------------
struct BuildingEnteredByPlayerCondition;

#[async_trait]
impl ScriptCondition for BuildingEnteredByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
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

//-------------------------------------------------------------------------------------------------
// UNIT_HAS_OBJECT_STATUS - evaluateUnitHasObjectStatus
//-------------------------------------------------------------------------------------------------
struct UnitHasObjectStatusCondition;

#[async_trait]
impl ScriptCondition for UnitHasObjectStatusCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let status_str = get_str_param(parameters, "status")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        let status_mask = parse_object_status_mask(&status_str);
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
// TEAM_ALL_HAS_OBJECT_STATUS - evaluateTeamHasObjectStatus(entireTeam=true)
//-------------------------------------------------------------------------------------------------
struct TeamAllHasObjectStatusCondition;

#[async_trait]
impl ScriptCondition for TeamAllHasObjectStatusCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let status_str = get_str_param(parameters, "status")?;
        let status_mask = parse_object_status_mask(&status_str);

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        for &member_id in team.get_members() {
            let ok = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    obj.get_status_bits().intersects(status_mask)
                })
                .unwrap_or(false);
            if !ok {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        "team_all_has_object_status"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members have a specific status (C++ TEAM_ALL_HAS_OBJECT_STATUS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "status".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_SOME_HAS_OBJECT_STATUS - evaluateTeamHasObjectStatus(entireTeam=false)
//-------------------------------------------------------------------------------------------------
struct TeamSomeHasObjectStatusCondition;

#[async_trait]
impl ScriptCondition for TeamSomeHasObjectStatusCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let status_str = get_str_param(parameters, "status")?;
        let status_mask = parse_object_status_mask(&status_str);

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        for &member_id in team.get_members() {
            match OBJECT_REGISTRY.with_object(member_id, |obj| {
                obj.get_status_bits().intersects(status_mask)
            }) {
                Some(true) => return Ok(true),
                Some(false) => {}
                None => return Ok(false),
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_some_has_object_status"
    }
    fn description(&self) -> &str {
        "Checks if ANY team member has a specific status (C++ TEAM_SOME_HAS_OBJECT_STATUS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "status".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// BUILT_BY_PLAYER - evaluateBuiltByPlayer
//-------------------------------------------------------------------------------------------------
struct BuiltByPlayerCondition;

#[async_trait]
impl ScriptCondition for BuiltByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_id = player.get_id() as u32;
        drop(player);

        let object_type = get_str_param(parameters, "object_type")?;

        // Search all objects for matching type owned by player
        // Host path: empty dual-world registry → no object residual.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(false);
        }
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    if obj.is_effectively_dead() {
                        continue;
                    }
                    if let Some(owner_id) = obj.get_controlling_player_id() {
                        if owner_id == player_id {
                            let template_name = obj.get_template_name();
                            if template_name == object_type {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "built_by_player"
    }
    fn description(&self) -> &str {
        "Checks if player has built an object of a specific type (C++ BUILT_BY_PLAYER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["object_type".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// C++ Parity Conditions - Session 8 additions
// Area entry/exit, special power, science, power, multiplayer, audio/video, misc
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
// NAMED_ENTERED_AREA - evaluateNamedEnteredArea
// Returns true if named unit has entered a trigger area.
//-------------------------------------------------------------------------------------------------
struct NamedEnteredAreaCondition;

#[async_trait]
impl ScriptCondition for NamedEnteredAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let area_name = get_str_param(parameters, "area_name")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        let Some(is_dead) = OBJECT_REGISTRY.with_object(object_id, |obj| obj.is_effectively_dead())
        else {
            return Ok(false);
        };
        if is_dead {
            return Ok(false);
        }

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        Ok(objects_in_area.contains(&object_id))
    }

    fn name(&self) -> &str {
        "named_entered_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit entered a trigger area (C++ NAMED_ENTERED_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_EXITED_AREA - evaluateNamedExitedArea
// Returns true if named unit has exited a trigger area (was inside, now outside).
//-------------------------------------------------------------------------------------------------
struct NamedExitedAreaCondition;

#[async_trait]
impl ScriptCondition for NamedExitedAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let area_name = get_str_param(parameters, "area_name")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        let Some(is_dead) = OBJECT_REGISTRY.with_object(object_id, |obj| obj.is_effectively_dead())
        else {
            return Ok(false);
        };
        if is_dead {
            return Ok(false);
        }

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        // "Exited" means not currently in the area
        Ok(!objects_in_area.contains(&object_id))
    }

    fn name(&self) -> &str {
        "named_exited_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit exited a trigger area (C++ NAMED_EXITED_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_ENTERED_AREA_ENTIRELY - evaluateTeamEnteredAreaEntirely
//-------------------------------------------------------------------------------------------------
struct TeamEnteredAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamEnteredAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let members = team.get_members();
        if members.is_empty() {
            return Ok(false);
        }

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        for &member_id in members {
            if !objects_in_area.contains(&member_id) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        "team_entered_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members entered an area (C++ TEAM_ENTERED_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_ENTERED_AREA_PARTIALLY - evaluateTeamEnteredAreaPartially
//-------------------------------------------------------------------------------------------------
struct TeamEnteredAreaPartiallyCondition;

#[async_trait]
impl ScriptCondition for TeamEnteredAreaPartiallyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ))
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        for &member_id in team.get_members() {
            if objects_in_area.contains(&member_id) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_entered_area_partially"
    }
    fn description(&self) -> &str {
        "Checks if any team member entered an area (C++ TEAM_ENTERED_AREA_PARTIALLY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_EXITED_AREA_ENTIRELY - evaluateTeamExitedAreaEntirely
//-------------------------------------------------------------------------------------------------
struct TeamExitedAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamExitedAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // All members outside = NOT (some inside OR all inside)
        let partially = TeamEnteredAreaPartiallyCondition
            .evaluate(parameters, _context)
            .await?;
        let entirely = TeamEnteredAreaEntirelyCondition
            .evaluate(parameters, _context)
            .await?;
        Ok(!partially && !entirely)
    }

    fn name(&self) -> &str {
        "team_exited_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members exited an area (C++ TEAM_EXITED_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_EXITED_AREA_PARTIALLY - evaluateTeamExitedAreaPartially
//-------------------------------------------------------------------------------------------------
struct TeamExitedAreaPartiallyCondition;

#[async_trait]
impl ScriptCondition for TeamExitedAreaPartiallyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Some members outside = NOT all inside
        let entirely = TeamEnteredAreaEntirelyCondition
            .evaluate(parameters, _context)
            .await?;
        Ok(!entirely)
    }

    fn name(&self) -> &str {
        "team_exited_area_partially"
    }
    fn description(&self) -> &str {
        "Checks if some team members exited an area (C++ TEAM_EXITED_AREA_PARTIALLY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_TRIGGERED_SPECIAL_POWER
//-------------------------------------------------------------------------------------------------
struct PlayerTriggeredSpecialPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerTriggeredSpecialPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_triggered(
                player_index,
                &power_name,
                true,
                crate::common::INVALID_ID,
            )
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_triggered_special_power"
    }
    fn description(&self) -> &str {
        "Checks if player triggered a special power (C++ PLAYER_TRIGGERED_SPECIAL_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED
//-------------------------------------------------------------------------------------------------
struct PlayerTriggeredSpecialPowerFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerTriggeredSpecialPowerFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_triggered(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_triggered_special_power_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player triggered a special power from a named unit (C++ PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "power_name".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_MIDWAY_SPECIAL_POWER
//-------------------------------------------------------------------------------------------------
struct PlayerMidwaySpecialPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerMidwaySpecialPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_midway(
                player_index,
                &power_name,
                true,
                crate::common::INVALID_ID,
            )
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_midway_special_power"
    }
    fn description(&self) -> &str {
        "Checks if player's special power is midway (C++ PLAYER_MIDWAY_SPECIAL_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED
//-------------------------------------------------------------------------------------------------
struct PlayerMidwaySpecialPowerFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerMidwaySpecialPowerFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_midway(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_midway_special_power_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player is midway through a special power from a named unit (C++ PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "power_name".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_COMPLETED_SPECIAL_POWER
//-------------------------------------------------------------------------------------------------
struct PlayerCompletedSpecialPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerCompletedSpecialPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_complete(
                player_index,
                &power_name,
                true,
                crate::common::INVALID_ID,
            )
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_completed_special_power"
    }
    fn description(&self) -> &str {
        "Checks if player completed a special power (C++ PLAYER_COMPLETED_SPECIAL_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED
//-------------------------------------------------------------------------------------------------
struct PlayerCompletedSpecialPowerFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerCompletedSpecialPowerFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_complete(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_completed_special_power_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player completed a special power from a named unit (C++ PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "power_name".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_BUILT_UPGRADE
//-------------------------------------------------------------------------------------------------
struct PlayerBuiltUpgradeCondition;

#[async_trait]
impl ScriptCondition for PlayerBuiltUpgradeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let upgrade_name = get_str_param(parameters, "upgrade")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_index = player.get_player_index() as usize;
        let completed_mask = player.get_completed_upgrade_mask();
        let upgrade_mask = crate::upgrade::upgrade_mask_for_name(&upgrade_name);
        let mask_bits = crate::common::UpgradeMaskType::from_bits_retain(upgrade_mask.to_bits());
        let has_upgrade = completed_mask.intersects(mask_bits);
        drop(player);

        let engine_hit = with_script_engine_mut(|engine| {
            engine.is_upgrade_complete(player_index, &upgrade_name, true, crate::common::INVALID_ID)
        })
        .unwrap_or(false);

        Ok(engine_hit || has_upgrade)
    }

    fn name(&self) -> &str {
        "player_built_upgrade"
    }
    fn description(&self) -> &str {
        "Checks if player built an upgrade (C++ PLAYER_BUILT_UPGRADE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "upgrade".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_BUILT_UPGRADE_FROM_NAMED
//-------------------------------------------------------------------------------------------------
struct PlayerBuiltUpgradeFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerBuiltUpgradeFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let upgrade_name = get_str_param(parameters, "upgrade")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_index = player.get_player_index() as usize;
        let completed_mask = player.get_completed_upgrade_mask();
        let upgrade_mask = crate::upgrade::upgrade_mask_for_name(&upgrade_name);
        let mask_bits = crate::common::UpgradeMaskType::from_bits_retain(upgrade_mask.to_bits());
        let has_upgrade = completed_mask.intersects(mask_bits);
        drop(player);

        let engine_hit = with_script_engine_mut(|engine| {
            engine.is_upgrade_complete(player_index, &upgrade_name, true, source_id)
        })
        .unwrap_or(false);

        Ok(engine_hit || has_upgrade)
    }

    fn name(&self) -> &str {
        "player_built_upgrade_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player built an upgrade from a named unit (C++ PLAYER_BUILT_UPGRADE_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "upgrade".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_ACQUIRED_SCIENCE
//-------------------------------------------------------------------------------------------------
struct PlayerAcquiredScienceCondition;

#[async_trait]
impl ScriptCondition for PlayerAcquiredScienceCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let science_name = get_str_param(parameters, "science")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let Some(store) = get_science_store() else {
            return Ok(false);
        };
        let science = store.get_science_from_internal_name(science_name.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        let player_index = player.get_player_index() as usize;
        drop(player);

        Ok(
            with_script_engine_mut(|engine| {
                engine.is_science_acquired(player_index, science, true)
            })
            .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "player_acquired_science"
    }
    fn description(&self) -> &str {
        "Checks if player has acquired a science (C++ PLAYER_ACQUIRED_SCIENCE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_CAN_PURCHASE_SCIENCE
//-------------------------------------------------------------------------------------------------
struct PlayerCanPurchaseScienceCondition;

#[async_trait]
impl ScriptCondition for PlayerCanPurchaseScienceCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let science_name = get_str_param(parameters, "science")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let Some(store) = get_science_store() else {
            return Ok(false);
        };
        let science = store.get_science_from_internal_name(science_name.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        Ok(player.is_capable_of_purchasing_science(science))
    }

    fn name(&self) -> &str {
        "player_can_purchase_science"
    }
    fn description(&self) -> &str {
        "Checks if player can purchase a science (C++ PLAYER_CAN_PURCHASE_SCIENCE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_SCIENCEPURCHASEPOINTS
//-------------------------------------------------------------------------------------------------
struct PlayerHasSciencePurchasePointsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasSciencePurchasePointsCondition {
    async fn evaluate(
        self: &Self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let points = super::actions::get_int_param(parameters, "points")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        Ok((player.get_science_purchase_points() as i64) >= points)
    }

    fn name(&self) -> &str {
        "player_has_science_purchase_points"
    }
    fn description(&self) -> &str {
        "Checks if player has enough science purchase points (C++ PLAYER_HAS_SCIENCEPURCHASEPOINTS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "points".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_POWER_COMPARE_PERCENT - evaluatePlayerHasComparisonPercentPower
//-------------------------------------------------------------------------------------------------
struct PlayerPowerComparePercentCondition;

#[async_trait]
impl ScriptCondition for PlayerPowerComparePercentCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let percent = super::actions::get_int_param(parameters, "percent")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let ratio = player.get_energy().supply_ratio();
        Ok(perform_comparison(
            (ratio * 100.0) as i64,
            &comparison,
            percent as i64,
        ))
    }

    fn name(&self) -> &str {
        "player_power_compare_percent"
    }
    fn description(&self) -> &str {
        "Compares player power supply ratio (C++ PLAYER_POWER_COMPARE_PERCENT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "percent".to_string(),
            "comparison".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_EXCESS_POWER_COMPARE_VALUE - evaluatePlayerHasComparisonValueExcessPower
//-------------------------------------------------------------------------------------------------
struct PlayerExcessPowerCompareValueCondition;

#[async_trait]
impl ScriptCondition for PlayerExcessPowerCompareValueCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let kwh = super::actions::get_int_param(parameters, "kwh")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let energy = player.get_energy();
        let actual_kwh = energy.production() - energy.consumption();
        Ok(perform_comparison(actual_kwh as i64, &comparison, kwh))
    }

    fn name(&self) -> &str {
        "player_excess_power_compare_value"
    }
    fn description(&self) -> &str {
        "Compares player excess power in KWH (C++ PLAYER_EXCESS_POWER_COMPARE_VALUE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "kwh".to_string(),
            "comparison".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MULTIPLAYER_ALLIED_VICTORY
//-------------------------------------------------------------------------------------------------
struct MultiplayerAlliedVictoryCondition;

#[async_trait]
impl ScriptCondition for MultiplayerAlliedVictoryCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        Ok(TheVictoryConditions::is_local_allied_victory())
    }

    fn name(&self) -> &str {
        "multiplayer_allied_victory"
    }
    fn description(&self) -> &str {
        "Checks if allies have won (C++ MULTIPLAYER_ALLIED_VICTORY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MULTIPLAYER_ALLIED_DEFEAT
//-------------------------------------------------------------------------------------------------
struct MultiplayerAlliedDefeatCondition;

#[async_trait]
impl ScriptCondition for MultiplayerAlliedDefeatCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let Ok(players) = player_list().read() else {
            return Ok(false);
        };
        let Some(local_player_arc) = players.get_local_player().cloned() else {
            return Ok(false);
        };
        let Ok(local_player) = local_player_arc.read() else {
            return Ok(false);
        };
        let local_index = local_player.get_player_index();
        let mut allied_count = 0usize;

        for player_arc in players.iter() {
            let Ok(player) = player_arc.read() else {
                continue;
            };
            if player.get_player_type() == PlayerType::Neutral || player.is_player_observer() {
                continue;
            }

            if player.get_player_index() == local_index {
                allied_count += 1;
                if !player.is_defeated() {
                    return Ok(false);
                }
                continue;
            }

            if local_player.is_allied_with_player(&player) {
                allied_count += 1;
                if !player.is_defeated() {
                    return Ok(false);
                }
            }
        }

        Ok(allied_count > 0)
    }

    fn name(&self) -> &str {
        "multiplayer_allied_defeat"
    }
    fn description(&self) -> &str {
        "Checks if allies have lost (C++ MULTIPLAYER_ALLIED_DEFEAT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MULTIPLAYER_PLAYER_DEFEAT
//-------------------------------------------------------------------------------------------------
struct MultiplayerPlayerDefeatCondition;

#[async_trait]
impl ScriptCondition for MultiplayerPlayerDefeatCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let Ok(players) = player_list().read() else {
            return Ok(false);
        };
        let Some(local_player_arc) = players.get_local_player().cloned() else {
            return Ok(false);
        };
        let Ok(local_player) = local_player_arc.read() else {
            return Ok(false);
        };
        Ok(local_player.is_defeated() || local_player.is_player_dead())
    }

    fn name(&self) -> &str {
        "multiplayer_player_defeat"
    }
    fn description(&self) -> &str {
        "Checks if local player is defeated (C++ MULTIPLAYER_PLAYER_DEFEAT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// HAS_FINISHED_VIDEO
//-------------------------------------------------------------------------------------------------
struct VideoCompletedCondition;

#[async_trait]
impl ScriptCondition for VideoCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let video_name = get_str_param(parameters, "video")?;
        Ok(
            with_script_engine_mut(|engine| engine.is_video_complete(&video_name, true))
                .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "video_completed"
    }
    fn description(&self) -> &str {
        "Checks if video has finished playing (C++ HAS_FINISHED_VIDEO)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["video".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// HAS_FINISHED_SPEECH
//-------------------------------------------------------------------------------------------------
struct SpeechCompletedCondition;

#[async_trait]
impl ScriptCondition for SpeechCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let speech_name = get_str_param(parameters, "speech")?;
        Ok(
            with_script_engine_mut(|engine| engine.is_speech_complete(&speech_name, true))
                .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "speech_completed"
    }
    fn description(&self) -> &str {
        "Checks if speech has finished (C++ HAS_FINISHED_SPEECH)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["speech".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// HAS_FINISHED_AUDIO
//-------------------------------------------------------------------------------------------------
struct AudioCompletedCondition;

#[async_trait]
impl ScriptCondition for AudioCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let audio_name = get_str_param(parameters, "audio")?;
        Ok(
            with_script_engine_mut(|engine| engine.is_audio_complete(&audio_name, true))
                .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "audio_completed"
    }
    fn description(&self) -> &str {
        "Checks if audio has finished (C++ HAS_FINISHED_AUDIO)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["audio".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MUSIC_TRACK_HAS_COMPLETED
//-------------------------------------------------------------------------------------------------
struct MusicTrackCompletedCondition;

#[async_trait]
impl ScriptCondition for MusicTrackCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let track = get_str_param(parameters, "track")?;
        let param = parameters
            .get("param")
            .and_then(|value| match value {
                ScriptValue::Int(value) => Some(*value),
                _ => None,
            })
            .unwrap_or(0);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(handler) = engine.action_handler() {
                    return Ok(handler.has_music_track_completed(&track, param as i32));
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "music_track_completed"
    }
    fn description(&self) -> &str {
        "Checks if music track has completed (C++ MUSIC_TRACK_HAS_COMPLETED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["track".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// CAMERA_MOVEMENT_FINISHED
//-------------------------------------------------------------------------------------------------
struct CameraMovementFinishedCondition;

#[async_trait]
impl ScriptCondition for CameraMovementFinishedCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(handler) = engine.action_handler() {
                    return Ok(handler.is_camera_movement_finished());
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "camera_movement_finished"
    }
    fn description(&self) -> &str {
        "Checks if camera movement finished (C++ CAMERA_MOVEMENT_FINISHED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MISSION_ATTEMPTS - Matches C++ ScriptConditions::evaluateMissionAttempts (line 1208)
// C++ returns false unconditionally; the player lookup is commented out.
//-------------------------------------------------------------------------------------------------
struct MissionAttemptsCondition;

#[async_trait]
impl ScriptCondition for MissionAttemptsCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        Ok(false)
    }

    fn name(&self) -> &str {
        "mission_attempts"
    }
    fn description(&self) -> &str {
        "Checks mission attempts (C++ MISSION_ATTEMPTS — always false)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "comparison".to_string(),
            "attempts".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// UNIT_EMPTIED - evaluateUnitHasEmptied
// Returns true if transport was emptied between last frame and this frame.
//-------------------------------------------------------------------------------------------------
struct UnitEmptiedCondition;

struct TransportStatus {
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
// PLAYER_LOST_OBJECT_TYPE - evaluatePlayerLostObjectType
//-------------------------------------------------------------------------------------------------
struct PlayerLostObjectTypeCondition;

#[async_trait]
impl ScriptCondition for PlayerLostObjectTypeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = get_player_arc(parameters, "player")?;
        let player = match player_arc {
            Some(p) => p,
            None => return Ok(false),
        };
        let player_index = {
            let p_guard = player
                .read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
            p_guard.get_player_index()
        };

        let object_type = get_str_param(parameters, "object_type")?;

        let current_count = get_script_engine()
            .read()
            .ok()
            .and_then(|engine| {
                engine
                    .as_ref()
                    .map(|engine| engine.get_object_count(player_index, &object_type))
            })
            .unwrap_or(0);

        let object_manager = get_object_manager();
        let sum_of_objs = object_manager
            .read()
            .ok()
            .map(|manager| {
                manager
                    .all_object_ids()
                    .into_iter()
                    .filter(|object_id| {
                        let Some(obj_arc) = manager.get_object(*object_id) else {
                            return false;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            return false;
                        };
                        if obj_guard.is_destroyed() {
                            return false;
                        }
                        let owner = {
                            let player = obj_guard.get_controlling_player();
                            player
                                .and_then(|p| p.read().ok().map(|g| g.get_player_index()))
                                .unwrap_or(-1)
                        };
                        if owner != player_index {
                            return false;
                        }
                        obj_guard
                            .template
                            .as_ref()
                            .map(|template| template.get_name() == object_type.as_str())
                            .unwrap_or(false)
                    })
                    .count() as i32
            })
            .unwrap_or(0);

        if sum_of_objs != current_count {
            if let Ok(mut engine_guard) = get_script_engine().write() {
                if let Some(ref mut engine) = *engine_guard {
                    engine.set_object_count(player_index, &object_type, sum_of_objs);
                }
            }
        }

        Ok(sum_of_objs < current_count)
    }

    fn name(&self) -> &str {
        "player_lost_object_type"
    }
    fn description(&self) -> &str {
        "Checks if player lost an object type (C++ PLAYER_LOST_OBJECT_TYPE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "object_type".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// Helper: parse object status mask from string name
//-------------------------------------------------------------------------------------------------
fn parse_object_status_mask(status_str: &str) -> crate::common::ObjectStatusMaskType {
    use crate::common::ObjectStatusMaskType as OSM;
    match status_str.to_lowercase().as_str() {
        "destroyed" => OSM::DESTROYED,
        "can_attack" => OSM::CAN_ATTACK,
        "under_construction" => OSM::UNDER_CONSTRUCTION,
        "unselectable" => OSM::UNSELECTABLE,
        "no_collisions" => OSM::NO_COLLISIONS,
        "no_attack" => OSM::NO_ATTACK,
        "airborne_target" => OSM::AIRBORNE_TARGET,
        "parachuting" => OSM::PARACHUTING,
        "hijacked" => OSM::HIJACKED,
        "aflame" => OSM::AFLAME,
        "burned" => OSM::BURNED,
        "stealthed" | "cloaked" => OSM::STEALTHED,
        "detected" => OSM::DETECTED,
        "can_stealth" => OSM::CAN_STEALTH,
        "sold" => OSM::SOLD,
        "undergoing_repair" => OSM::UNDERGOING_REPAIR,
        "reconstructing" => OSM::RECONSTRUCTING,
        "masked" => OSM::MASKED,
        "is_attacking" => OSM::IS_ATTACKING,
        "is_using_ability" => OSM::IS_USING_ABILITY,
        "is_aiming_weapon" => OSM::IS_AIMING_WEAPON,
        "no_attack_from_ai" => OSM::NO_ATTACK_FROM_AI,
        "ignoring_stealth" => OSM::IGNORING_STEALTH,
        "is_car_bomb" => OSM::IS_CAR_BOMB,
        "is_firing_weapon" => OSM::IS_FIRING_WEAPON,
        "braking" => OSM::BRAKING,
        "wet" => OSM::WET,
        "repulsor" => OSM::REPULSOR,
        "rider1" => OSM::RIDER1,
        "rider2" => OSM::RIDER2,
        "rider3" => OSM::RIDER3,
        "rider4" => OSM::RIDER4,
        "rider5" => OSM::RIDER5,
        "rider6" => OSM::RIDER6,
        "rider7" => OSM::RIDER7,
        "rider8" => OSM::RIDER8,
        _ => {
            log::warn!("Unknown object status: {}", status_str);
            OSM::NONE
        }
    }
}

//-------------------------------------------------------------------------------------------------
// CONDITION_FALSE / CONDITION_TRUE - C++ always returns false/true
//-------------------------------------------------------------------------------------------------
struct ConditionFalseCondition;
#[async_trait]
impl ScriptCondition for ConditionFalseCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        Ok(false)
    }
    fn name(&self) -> &str {
        "condition_false"
    }
    fn description(&self) -> &str {
        "Always evaluates to false (C++ CONDITION_FALSE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

struct ConditionTrueCondition;
#[async_trait]
impl ScriptCondition for ConditionTrueCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        Ok(true)
    }
    fn name(&self) -> &str {
        "condition_true"
    }
    fn description(&self) -> &str {
        "Always evaluates to true (C++ CONDITION_TRUE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TIMER_EXPIRED - C++ ScriptEngine::evaluateTimer
// Checks if a countdown timer counter has expired (value < 1).
//-------------------------------------------------------------------------------------------------
struct TimerExpiredCondition;
#[async_trait]
impl ScriptCondition for TimerExpiredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let timer_name = get_str_param(parameters, "timer_name")
            .or_else(|_| get_str_param(parameters, "timer"))?;

        let expired = get_script_engine()
            .read()
            .ok()
            .and_then(|guard| {
                guard.as_ref().and_then(|engine| {
                    engine.get_counter(&timer_name).map(|c| {
                        // C++: timers decrement down to -1; expired when value < 1
                        c.is_countdown_timer && c.value < 1
                    })
                })
            })
            .unwrap_or(false);

        Ok(expired)
    }

    fn name(&self) -> &str {
        "timer_expired"
    }
    fn description(&self) -> &str {
        "If a named timer has expired (C++ TIMER_EXPIRED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["timer_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec!["timer".to_string()]
    }
}

//-------------------------------------------------------------------------------------------------
// UNIT_HEALTH - C++ ScriptConditions::evaluateUnitHealth
// Gets named object, reads body module health/initial health, computes percentage,
// compares against threshold using the given comparison operator.
//-------------------------------------------------------------------------------------------------
struct UnitHealthCondition;
#[async_trait]
impl ScriptCondition for UnitHealthCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
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
                ))
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
struct EnemySightedCondition;
#[async_trait]
impl ScriptCondition for EnemySightedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
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

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_OBJECT_COMPARISON - C++ ScriptConditions::evaluatePlayerUnitCondition
// Counts objects the player owns matching the given thing template,
// then compares the count against the threshold using the given operator.
//-------------------------------------------------------------------------------------------------
struct PlayerHasObjectComparisonCondition;
#[async_trait]
impl ScriptCondition for PlayerHasObjectComparisonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = get_player_arc(parameters, "player")?;
        let player = match player_arc {
            Some(p) => p,
            None => return Ok(false),
        };

        let comparison = get_str_param(parameters, "comparison")?;
        let count_threshold = match parameters.get("count").or(parameters.get("threshold")) {
            Some(ScriptValue::Int(v)) => *v,
            Some(ScriptValue::Float(v)) => *v as i64,
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'count' parameter".to_string(),
                ))
            }
        };

        // Get the object type name to match
        let object_type_name = get_str_param(parameters, "object_type")
            .or_else(|_| get_str_param(parameters, "unit_type"))?;

        // Iterate player's owned objects and count those matching the template name.
        // C++ uses countObjectsByThingTemplate which matches by template pointer;
        // we match by template name string which is equivalent for single-template queries.
        let player_guard = player
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let mut count: i64 = 0;
        let all_objects = player_guard.get_all_objects();
        for obj_id in all_objects {
            let matches = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    obj_guard.is_alive() && obj_guard.get_template_name() == object_type_name
                })
                .unwrap_or(false);
            if matches {
                count += 1;
            }
        }

        Ok(perform_comparison(count, &comparison, count_threshold))
    }

    fn name(&self) -> &str {
        "player_has_object_comparison"
    }
    fn description(&self) -> &str {
        "Player has N objects of type, compared against threshold (C++ PLAYER_HAS_OBJECT_COMPARISON)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "comparison".to_string(),
            "count".to_string(),
            "object_type".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec!["unit_type".to_string(), "threshold".to_string()]
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
            let instance =
                GameObjectInstance::new(123, Some(template), None, ObjectCreationFlags::new())
                    .expect("failed to create object instance");
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

    #[tokio::test]
    async fn bridge_conditions_use_terrain_bridge_damage_state() {
        use crate::common::{AsciiString, BodyDamageType};
        use crate::terrain::{get_terrain_logic, BridgeInfo};

        let bridge_name = "RegistryBridgeDamageState";
        let bridge_id = 0x00B1_D6E0;
        get_named_object_tracker()
            .register_named_object(bridge_name.to_string(), bridge_id)
            .expect("register bridge name");

        {
            let mut terrain = get_terrain_logic().write().expect("terrain write lock");
            terrain.reset();
            let mut info = BridgeInfo::new();
            info.bridge_object_id = bridge_id;
            info.cur_damage_state = BodyDamageType::Rubble;
            info.damage_state_changed = true;
            terrain.add_bridge_to_logic(info, AsciiString::from("TestBridgeTemplate"));
        }

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
        let mut params = HashMap::new();
        params.insert(
            "bridge_name".to_string(),
            ScriptValue::String(bridge_name.to_string()),
        );

        assert!(BridgeBrokenCondition
            .evaluate(&params, &context)
            .await
            .expect("broken condition"));
        assert!(!BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .expect("repaired condition"));

        {
            let mut terrain = get_terrain_logic().write().expect("terrain write lock");
            terrain.reset();
            let mut info = BridgeInfo::new();
            info.bridge_object_id = bridge_id;
            info.cur_damage_state = BodyDamageType::Damaged;
            info.damage_state_changed = true;
            terrain.add_bridge_to_logic(info, AsciiString::from("TestBridgeTemplate"));
        }

        assert!(!BridgeBrokenCondition
            .evaluate(&params, &context)
            .await
            .expect("broken condition after repair"));
        assert!(BridgeRepairedCondition
            .evaluate(&params, &context)
            .await
            .expect("repaired condition after repair"));

        get_terrain_logic()
            .write()
            .expect("terrain write lock")
            .reset();
    }

    #[tokio::test]
    async fn research_complete_checks_player_science_store() {
        use game_engine::common::rts::science::{
            get_science_store_mut, init_science_store, ScienceInfo,
        };

        init_science_store();
        let science_name = "SCIENCE_RegistryResearchComplete";
        let science = {
            let mut store = get_science_store_mut().expect("science store");
            store.add_science(ScienceInfo::new(SCIENCE_INVALID, science_name));
            store.get_science_from_internal_name(science_name)
        };
        assert_ne!(science, SCIENCE_INVALID);

        player_list().write().unwrap().clear();
        let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
        player.write().unwrap().add_science(science);
        player_list().write().unwrap().add_player(player);

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
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "science_name".to_string(),
            ScriptValue::String(science_name.to_string()),
        );

        assert!(ResearchCompleteCondition
            .evaluate(&params, &context)
            .await
            .expect("research complete condition"));
    }

    #[tokio::test]
    async fn flag_comparison_reads_script_engine_flags() {
        crate::scripting::engine::initialize_script_engine().expect("script engine");

        let flag_name = "registry_flag_comparison_reads_script_engine_flags";
        {
            let engine = get_script_engine();
            let mut engine_guard = engine.write().expect("script engine write lock");
            engine_guard
                .as_mut()
                .expect("script engine initialized")
                .set_flag(flag_name, true)
                .expect("set flag");
        }

        let mut variables = HashMap::new();
        variables.insert(flag_name.to_string(), ScriptValue::Bool(false));
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
        let mut params = HashMap::new();
        params.insert(
            "flag_name".to_string(),
            ScriptValue::String(flag_name.to_string()),
        );
        params.insert("value".to_string(), ScriptValue::Bool(true));

        assert!(FlagComparisonCondition
            .evaluate(&params, &context)
            .await
            .expect("flag comparison condition"));
    }

    #[tokio::test]
    async fn player_has_resource_uses_money_aliases_like_resource_actions() {
        player_list().write().unwrap().clear();
        let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
        player.write().unwrap().get_money_mut().set_money(800);
        player_list().write().unwrap().add_player(player);

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
        let mut params = HashMap::new();
        params.insert("player".to_string(), ScriptValue::Int(0));
        params.insert(
            "resource_type".to_string(),
            ScriptValue::String("supplies".to_string()),
        );
        params.insert("amount".to_string(), ScriptValue::Int(800));

        assert!(PlayerHasResourceCondition
            .evaluate(&params, &context)
            .await
            .expect("supplies resource condition"));

        params.insert(
            "resource_type".to_string(),
            ScriptValue::String("oil".to_string()),
        );
        assert!(!PlayerHasResourceCondition
            .evaluate(&params, &context)
            .await
            .expect("unknown resource condition"));
    }
}
