//! Shared by ordinary conditions and sequential waits (ScriptConditions.cpp:2269).

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{Condition, ConditionType, Parameter, ScriptAction, ScriptActionType};
use super::program_execution::{ScriptConditionQueries, ScriptProgramDriver};
use super::world::ScriptOwnerQuery;
use super::{ScriptActionResult, ScriptError};
use crate::{GameLogicError, GameLogicResult};

impl ScriptInterpreter {
    pub(super) fn try_evaluate_command_button_condition(
        &self,
        condition: &Condition,
        queries: &mut impl ScriptConditionQueries,
    ) -> Result<Option<bool>, ScriptError> {
        let all = match condition.get_condition_type() {
            ConditionType::SkirmishCommandButtonReadyAll => true,
            ConditionType::SkirmishCommandButtonReadyPartial => false,
            _ => return Ok(None),
        };
        self.command_button_ready(
            all,
            |i| condition.get_parameter(i),
            queries.receiving_world(),
        )
    }

    pub(super) fn try_execute_command_button_wait(
        &self,
        action: &ScriptAction,
        driver: &mut impl ScriptProgramDriver,
    ) -> GameLogicResult<Option<ScriptActionResult>> {
        let all = match action.action_type {
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll => true,
            ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial => false,
            _ => return Ok(None),
        };
        let mut queries = driver.condition_queries();
        let ready = self
            .command_button_ready(all, |i| action.get_parameter(i), queries.receiving_world())
            .map_err(|error| {
                GameLogicError::Configuration(format!("Script action error: {error}"))
            })?;
        Ok(ready.map(|ready| {
            if ready {
                ScriptActionResult::Success
            } else {
                ScriptActionResult::Pending(1.0)
            }
        }))
    }

    fn command_button_ready<'a, T: ?Sized>(
        &self,
        all: bool,
        parameter: impl Fn(usize) -> Option<&'a Parameter>,
        driver: &dyn ScriptExecutionDriver<T>,
    ) -> Result<Option<bool>, ScriptError> {
        // The SIDE parameter is unused. An admitted team is required before looking up the button.
        let team_name = parameter(1).ok_or_else(|| ScriptError::missing_parameter(1))?;
        let team = match driver
            .condition_team_roster(
                team_name.get_string(),
                self.calling_team(),
                self.condition_team(),
            )
            .map_err(|error| ScriptError::EvaluationFailed(error.to_string()))?
        {
            ScriptOwnerQuery::Present(team) => team,
            ScriptOwnerQuery::Missing => return Ok(Some(false)),
            ScriptOwnerQuery::Unavailable => return Ok(None),
        };
        let button_name = parameter(2).ok_or_else(|| ScriptError::missing_parameter(2))?;
        let button = match driver.script_command_button(button_name.get_string()) {
            ScriptOwnerQuery::Present(button) => button,
            ScriptOwnerQuery::Missing => return Ok(Some(false)),
            ScriptOwnerQuery::Unavailable => return Ok(None),
        };

        for member in team.members {
            let power_ready = match &button.special_power {
                Some(power) => match driver.script_command_button_power(member, power) {
                    Some(ready) => ready,
                    None => continue,
                },
                None if button.upgrade.is_some() => false,
                None => break,
            };
            let ready = power_ready
                || button
                    .upgrade
                    .as_deref()
                    .is_some_and(|upgrade| driver.script_command_button_upgrade(member, upgrade));
            if ready != all {
                return Ok(Some(ready));
            }
        }
        Ok(Some(all))
    }
}
