//! Combat and casualty script conditions.

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

/// Units destroyed condition
pub(super) struct UnitsDestroyedCondition;

#[async_trait]
impl ScriptCondition for UnitsDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let _player = crate::scripting::actions::get_int_param_optional(parameters, "player");
        let _unit_type = parameters.get("unit_type");
        let comparison = crate::scripting::actions::get_string_param(parameters, "comparison")?;
        let count = crate::scripting::actions::get_int_param(parameters, "count")?;

        log::debug!("Checking destroyed units");

        let filter = EventFilter {
            event_types: vec![GameEventType::UnitDestroyed],
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: crate::scripting::ScriptPriority::Low,
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
pub(super) struct CombatOccurredCondition;

#[async_trait]
impl ScriptCondition for CombatOccurredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let x = crate::scripting::actions::get_float_param_optional(parameters, "x");
        let y = crate::scripting::actions::get_float_param_optional(parameters, "y");
        let radius = crate::scripting::actions::get_float_param_optional(parameters, "radius");
        let time_window =
            crate::scripting::actions::get_float_param_optional(parameters, "time_window")
                .unwrap_or(60.0);

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
            min_priority: crate::scripting::ScriptPriority::Low,
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
pub(super) struct PlayerCasualtiesCondition;

#[async_trait]
impl ScriptCondition for PlayerCasualtiesCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let comparison = crate::scripting::actions::get_string_param(parameters, "comparison")?;
        let count = crate::scripting::actions::get_int_param(parameters, "count")?;

        log::debug!("Checking casualties for player {}", player);

        let filter = EventFilter {
            event_types: vec![GameEventType::UnitDestroyed],
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: crate::scripting::ScriptPriority::Low,
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

pub(super) fn register_combat_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(UnitsDestroyedCondition));
    registry.register_condition(Box::new(CombatOccurredCondition));
    registry.register_condition(Box::new(PlayerCasualtiesCondition));
}
