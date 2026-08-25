//! Logical, variable, counter, and flag script conditions.

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

// Logical conditions

/// AND condition
pub(super) struct AndCondition;

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
pub(super) struct OrCondition;

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
pub(super) struct NotCondition;

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
pub(super) struct VariableEqualsCondition;

#[async_trait]
impl ScriptCondition for VariableEqualsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let variable_name =
            crate::scripting::actions::get_string_param(parameters, "variable_name")?;
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

/// Variable greater than condition
pub(super) struct VariableGreaterThanCondition;

#[async_trait]
impl ScriptCondition for VariableGreaterThanCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let variable_name =
            crate::scripting::actions::get_string_param(parameters, "variable_name")?;
        let threshold = crate::scripting::actions::get_float_param(parameters, "value")?;

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
pub(super) struct VariableLessThanCondition;

#[async_trait]
impl ScriptCondition for VariableLessThanCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let variable_name =
            crate::scripting::actions::get_string_param(parameters, "variable_name")?;
        let threshold = crate::scripting::actions::get_float_param(parameters, "value")?;

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
pub(super) struct CounterComparisonCondition;

#[async_trait]
impl ScriptCondition for CounterComparisonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let counter_name = crate::scripting::actions::get_string_param(parameters, "counter_name")?;
        let comparison = crate::scripting::actions::get_string_param(parameters, "comparison")?;
        let value = crate::scripting::actions::get_int_param(parameters, "value")?;

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
                )));
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
pub(super) struct FlagComparisonCondition;

#[async_trait]
impl ScriptCondition for FlagComparisonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let flag_name = crate::scripting::actions::get_string_param(parameters, "flag_name")?;
        let expected_value = parameters
            .get("value")
            .map(|v| match v {
                ScriptValue::Bool(b) => *b,
                ScriptValue::Int(i) => *i != 0,
                _ => false,
            })
            .unwrap_or(true);

        log::debug!("Checking flag '{}' == {}", flag_name, expected_value);

        let (flag_value, ui_pulse) = get_script_engine()
            .read()
            .ok()
            .and_then(|engine_guard| {
                engine_guard.as_ref().map(|engine| {
                    let flag_value = engine
                        .get_flag(&flag_name)
                        .map(|flag| flag.value)
                        .unwrap_or(false);
                    (flag_value, engine.has_ui_interaction(&flag_name))
                })
            })
            .unwrap_or((false, false));

        Ok(flag_value == expected_value || ui_pulse)
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

//-------------------------------------------------------------------------------------------------
// CONDITION_FALSE / CONDITION_TRUE - C++ always returns false/true
//-------------------------------------------------------------------------------------------------
pub(super) struct ConditionFalseCondition;

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

pub(super) struct ConditionTrueCondition;

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

pub(super) fn register_logic_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(AndCondition));
    registry.register_condition(Box::new(OrCondition));
    registry.register_condition(Box::new(NotCondition));
    registry.register_condition(Box::new(VariableEqualsCondition));
    registry.register_condition(Box::new(VariableGreaterThanCondition));
    registry.register_condition(Box::new(VariableLessThanCondition));
    registry.register_condition(Box::new(CounterComparisonCondition));
    registry.register_condition(Box::new(FlagComparisonCondition));
    registry.register_condition(Box::new(ConditionFalseCondition));
    registry.register_condition(Box::new(ConditionTrueCondition));
}
