//! Script control, variable, wait, and timer actions
//!
//! C++: ScriptActions.cpp timers L4018–4060. Enable/disable/execute script
//! are ScriptEngine-side, not ScriptActions `do*`.
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

/// Enable script action
pub(super) struct EnableScriptAction;

#[async_trait]
impl ScriptAction for EnableScriptAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let script_name = get_string_param(parameters, "script_name")?;
        log::info!("Enabling script '{}'", script_name);
        // C++ ScriptEngine::enableScript ScriptEngine.cpp:6797-6823.
        let found = crate::scripting::engine::with_script_engine_mut(|engine| {
            engine.set_script_active_by_name(&script_name, true)
        })
        .unwrap_or(false);
        if !found {
            log::warn!("ENABLE_SCRIPT: script '{}' not found", script_name);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "enable_script"
    }

    fn description(&self) -> &str {
        "Enables another script"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["script_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable script action
pub(super) struct DisableScriptAction;

#[async_trait]
impl ScriptAction for DisableScriptAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let script_name = get_string_param(parameters, "script_name")?;
        log::info!("Disabling script '{}'", script_name);
        // C++ ScriptEngine::disableScript ScriptEngine.cpp:6797-6823.
        let found = crate::scripting::engine::with_script_engine_mut(|engine| {
            engine.set_script_active_by_name(&script_name, false)
        })
        .unwrap_or(false);
        if !found {
            log::warn!("DISABLE_SCRIPT: script '{}' not found", script_name);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "disable_script"
    }

    fn description(&self) -> &str {
        "Disables another script"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["script_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Execute script action
pub(super) struct ExecuteScriptAction;

#[async_trait]
impl ScriptAction for ExecuteScriptAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let script_name = get_string_param(parameters, "script_name")?;
        log::info!("Executing script '{}'", script_name);
        // C++ CALL_SUBROUTINE / executeScript via ScriptEngine::execute_subroutine_by_name.
        let found = crate::scripting::engine::with_script_engine_mut(|engine| {
            engine.execute_subroutine_by_name(&script_name)
        })
        .transpose()?
        .unwrap_or(false);
        if !found {
            log::warn!("EXECUTE_SCRIPT: script '{}' not found", script_name);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "execute_script"
    }

    fn description(&self) -> &str {
        "Executes another script"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["script_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set variable action
pub(super) struct SetVariableAction;

#[async_trait]
impl ScriptAction for SetVariableAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let variable_name = get_string_param(parameters, "variable_name")?;
        let value = parameters
            .get("value")
            .cloned()
            .unwrap_or(ScriptValue::Null);

        log::info!("Setting variable '{}' to '{}'", variable_name, value);
        // C++ SET_FLAG / SET_COUNTER live on ScriptEngine, not a no-op log.
        let _ = crate::scripting::engine::with_script_engine_mut(|engine| match &value {
            ScriptValue::Bool(flag) => engine.set_flag(&variable_name, *flag),
            ScriptValue::Int(int) => engine.set_counter(&variable_name, clamp_script_money(*int)),
            ScriptValue::Float(real) => {
                engine.set_counter(&variable_name, clamp_script_money(*real as i64))
            }
            ScriptValue::String(text) => {
                if let Ok(int) = text.parse::<i64>() {
                    engine.set_counter(&variable_name, clamp_script_money(int))
                } else if text.eq_ignore_ascii_case("true") {
                    engine.set_flag(&variable_name, true)
                } else if text.eq_ignore_ascii_case("false") {
                    engine.set_flag(&variable_name, false)
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        });

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_variable"
    }

    fn description(&self) -> &str {
        "Sets a script variable"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["variable_name".to_string(), "value".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Invented leftover wait. C++ has no generic `wait` ScriptAction.
/// Sequential waits live in `scripting/executor` (`TEAM_WAIT_*`,
/// `SKIRMISH_WAIT_*`, `TEAM_SPIN_FOR_FRAMECOUNT`, `frames_to_wait`).
/// Leftover-only: not registered on ActionRegistry (hq-8ta4n).
#[allow(dead_code)]
pub(super) struct WaitAction;
#[async_trait]
#[allow(dead_code)]
impl ScriptAction for WaitAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let duration = get_float_param(parameters, "duration")?;
        log::debug!("Leftover wait ({duration}s) skipped; C++ sequential waits are executor-only");
        Ok(ScriptResult::Skipped)
    }

    fn name(&self) -> &str {
        "wait"
    }

    fn description(&self) -> &str {
        "Waits for a specified duration"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Start Timer Action - Matches C++ ScriptActionType::SET_MILLISECOND_TIMER
pub(super) struct StartTimerAction;

#[async_trait]
impl ScriptAction for StartTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let counter_name = get_string_param(parameters, "counter_name")?;
        let milliseconds = get_int_param(parameters, "milliseconds")?;

        log::info!("Starting timer '{}' for {} ms", counter_name, milliseconds);

        with_script_engine_mut(|engine| {
            engine.set_timer_millisecond_script_seconds(&counter_name, milliseconds as f32 / 1000.0)
        })?;

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "start_timer"
    }

    fn description(&self) -> &str {
        "Starts a countdown timer with specified milliseconds"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["counter_name".to_string(), "milliseconds".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Stop Timer Action - Matches C++ ScriptActionType::STOP_TIMER
pub(super) struct StopTimerAction;

#[async_trait]
impl ScriptAction for StopTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let counter_name = get_string_param(parameters, "counter_name")?;

        log::info!("Stopping timer '{}'", counter_name);

        with_script_engine_mut(|engine| engine.stop_timer(&counter_name))?;

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "stop_timer"
    }

    fn description(&self) -> &str {
        "Stops/pauses a countdown timer"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["counter_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set Timer Action - Matches C++ ScriptActions::doDisplayCounter (line 4020)
pub(super) struct SetTimerAction;

#[async_trait]
impl ScriptAction for SetTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let timer_name = get_string_param(parameters, "timer")?;
        let value = get_int_param(parameters, "value")?;

        log::info!("Setting timer '{}' to {}", timer_name, value);

        with_script_engine_mut(|engine| {
            engine.set_counter(&timer_name, clamp_script_money(value))
        })?;
        dispatch_named_timer(&timer_name, &timer_name, false);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_timer"
    }

    fn description(&self) -> &str {
        "Creates or sets a named timer/counter on HUD"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["timer".to_string(), "value".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Countdown Timer Action - Matches C++ ScriptActions::doDisplayCountdownTimer (line 4036)
pub(super) struct CountdownTimerAction;

#[async_trait]
impl ScriptAction for CountdownTimerAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let timer_name = get_string_param(parameters, "timer")?;
        let seconds = get_int_param(parameters, "seconds")?;

        log::info!(
            "Starting countdown timer '{}' for {} seconds",
            timer_name,
            seconds
        );

        with_script_engine_mut(|engine| {
            engine.set_timer_seconds(&timer_name, seconds.max(0) as f32)
        })?;
        dispatch_named_timer(&timer_name, &timer_name, true);

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "countdown_timer"
    }

    fn description(&self) -> &str {
        "Starts a countdown timer on HUD"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["timer".to_string(), "seconds".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
