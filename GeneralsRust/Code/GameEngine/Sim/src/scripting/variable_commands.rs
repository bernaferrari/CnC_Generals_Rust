//! Authored scalar commands from C++ ScriptEngine.cpp:6315–6478, 6694–6799.

use super::ScriptError;
use super::interpreter::ScriptInterpreter;
use super::program::{ComparisonType, Condition, ConditionType, ScriptAction, ScriptActionType};

impl ScriptInterpreter {
    pub fn try_execute_variable_action(
        &self,
        action: &ScriptAction,
        random_int: impl FnOnce(i32, i32) -> i32,
        random_real: impl FnOnce(f32, f32) -> f32,
    ) -> Result<bool, ScriptError> {
        use ScriptActionType::*;

        let parameter = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let name = |index| parameter(index).map(|value| value.get_string());
        let int = |index| parameter(index).map(|value| value.get_int());
        let real = |index| parameter(index).map(|value| value.get_real());

        // The existing action contract treats a full variable table as a no-op.
        let _ = match action.get_action_type() {
            SetFlag => {
                let (name, value) = (name(0)?, int(1)? != 0);
                self.frame_mut().variables.set_flag(name, value)
            }
            SetCounter => {
                let (name, value) = (name(0)?, int(1)?);
                self.frame_mut().variables.set_counter(name, value)
            }
            IncrementCounter => {
                let amount = int(0)?;
                let name = name(1)?;
                self.frame_mut().variables.increment_counter(name, amount)
            }
            DecrementCounter => {
                let amount = int(0)?;
                let name = name(1)?;
                self.frame_mut().variables.decrement_counter(name, amount)
            }
            SetTimer => {
                let (name, frames) = (name(0)?, int(1)?);
                self.frame_mut().variables.set_timer(name, frames)
            }
            SetMillisecondTimer => {
                let (name, seconds) = (name(0)?, real(1)?);
                self.frame_mut()
                    .variables
                    .set_timer_millisecond_script_seconds(name, seconds)
            }
            SetRandomTimer => {
                let (name, low, high) = (name(0)?, int(1)?, int(2)?);
                // C++ RandomValue.cpp uses a wrapping unsigned span. The stream
                // handles reversed bounds and selects when a draw is required.
                let frames = random_int(low, high);
                self.frame_mut().variables.set_timer(name, frames)
            }
            SetRandomMsecTimer => {
                let (name, low, high) = (name(0)?, real(1)?, real(2)?);
                let seconds = if high <= low {
                    high
                } else {
                    random_real(low, high)
                };
                self.frame_mut()
                    .variables
                    .set_timer_millisecond_script_seconds(name, seconds)
            }
            StopTimer => {
                let name = name(0)?;
                self.frame_mut().variables.stop_timer(name)
            }
            RestartTimer => {
                let name = name(0)?;
                self.frame_mut().variables.restart_timer(name)
            }
            AddToMsecTimer => {
                let seconds = real(0)?;
                let name = name(1)?;
                self.frame_mut()
                    .variables
                    .add_to_timer_millisecond_script_seconds(name, seconds)
            }
            SubFromMsecTimer => {
                let seconds = real(0)?;
                let name = name(1)?;
                self.frame_mut()
                    .variables
                    .subtract_from_timer_millisecond_script_seconds(name, seconds)
            }
            _ => return Ok(false),
        };
        Ok(true)
    }

    pub fn try_evaluate_local_condition(
        &self,
        condition: &Condition,
    ) -> Result<Option<bool>, ScriptError> {
        let parameter = |index| {
            condition
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let frame = || {
            self.try_frame_mut().map_err(|_| {
                ScriptError::EvaluationFailed("script variables are already borrowed".into())
            })
        };
        let result = match condition.get_condition_type() {
            ConditionType::ConditionTrue => true,
            ConditionType::ConditionFalse => false,
            ConditionType::Counter => {
                let name = parameter(0)?.get_string();
                let comparison = ComparisonType::from_i32(parameter(1)?.get_int());
                let target = parameter(2)?.get_int();
                let value = frame()?
                    .variables
                    .counter_mut(name)
                    .map_or(0, |counter| counter.value);
                comparison.is_some_and(|comparison| comparison.compare_i32(value, target))
            }
            ConditionType::Flag => {
                let name = parameter(0)?.get_string();
                let expected = parameter(1)?.get_int() != 0;
                let mut frame = frame()?;
                let value = frame.variables.flag_mut(name).is_ok_and(|flag| flag.value);
                value == expected
                    || frame
                        .ui_interactions
                        .iter()
                        .any(|interaction| interaction == name)
            }
            ConditionType::TimerExpired => {
                let name = parameter(0)?.get_string();
                frame()?
                    .variables
                    .counter_mut(name)
                    .is_ok_and(|counter| counter.is_countdown_timer && counter.value <= 0)
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }
}
