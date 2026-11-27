//! Ordinary script execution and the action dispatch shared by both interpreters.
//! C++ ScriptEngine.cpp:6829–6859, 6926–7022, 7558–7650.

use super::ScriptActionResult;
use super::program::{Condition, OrCondition, Script, ScriptAction, ScriptActionType};
use super::sequential_scripts::is_sequential_only_action;
use crate::time::LOGIC_HZ;
use crate::{GameLogicError, GameLogicResult};

#[derive(Clone, Copy)]
pub enum ScriptDifficulty {
    Easy,
    Normal,
    Hard,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
enum ScriptActionError {
    #[error("Script action error: Parameter not found: Parameter 0 not found")]
    MissingSubroutineName,
    #[error("Script action failed: {0}")]
    Failed(String),
}

impl From<ScriptActionError> for GameLogicError {
    fn from(error: ScriptActionError) -> Self {
        Self::Configuration(error.to_string())
    }
}

/// The receiving world's observations and effects during script execution.
pub trait ScriptProgramDriver {
    fn difficulty(&self) -> ScriptDifficulty;
    fn condition_team_after(&self, name: &str, after: Option<u32>) -> Option<u32>;
    fn set_condition_team(&mut self, team: Option<u32>);
    fn evaluate_condition(&mut self, condition: &mut Condition) -> GameLogicResult<bool>;
    fn execute_action(&mut self, action: &ScriptAction) -> GameLogicResult<ScriptActionResult>;
    fn call_subroutine(&mut self, name: &str) -> GameLogicResult<()>;
    fn after_action(&mut self) -> GameLogicResult<()>;
}

/// Run one installed script. The driver scopes the condition team to this call.
pub fn execute_script(
    script: &mut Script,
    frame: u32,
    driver: &mut impl ScriptProgramDriver,
) -> GameLogicResult<()> {
    if !script.is_active {
        return Ok(());
    }
    let matches_difficulty = match driver.difficulty() {
        ScriptDifficulty::Easy => script.easy,
        ScriptDifficulty::Normal => script.normal,
        ScriptDifficulty::Hard => script.hard,
    };
    if !matches_difficulty || frame < script.frame_to_evaluate_at {
        return Ok(());
    }
    if script.delay_evaluation_seconds > 0 {
        script.frame_to_evaluate_at = frame + script.delay_evaluation_seconds as u32 * LOGIC_HZ;
    }

    let team_name = script.condition_team_name.trim().to_owned();
    if !team_name.is_empty()
        && let Some(mut team) = driver.condition_team_after(&team_name, None)
    {
        loop {
            driver.set_condition_team(Some(team));
            evaluate_script(script, frame, driver, false)?;
            // Like DLINK_ITERATOR::advance, read the next team after the
            // condition and action callbacks have had a chance to mutate it.
            let Some(next) = driver.condition_team_after(&team_name, Some(team)) else {
                return Ok(());
            };
            team = next;
        }
    }

    driver.set_condition_team(None);
    evaluate_script(script, frame, driver, true)
}

fn evaluate_script(
    script: &mut Script,
    frame: u32,
    driver: &mut impl ScriptProgramDriver,
    deactivate_on_false_action: bool,
) -> GameLogicResult<()> {
    let condition_true = evaluate_conditions(script.condition.as_deref_mut(), |condition| {
        driver.evaluate_condition(condition)
    })?;
    let actions = if condition_true {
        script.action.as_deref()
    } else if let Some(action) = script.action_false.as_deref() {
        Some(action)
    } else {
        return Ok(());
    };
    match execute_action_chain(actions, driver)? {
        Some(frames) => {
            script.frame_to_evaluate_at = script
                .frame_to_evaluate_at
                .max(pending_resume_frame(frame, frames));
        }
        None if script.is_one_shot && (condition_true || deactivate_on_false_action) => {
            script.is_active = false;
        }
        None => {}
    }
    Ok(())
}

/// Evaluate OR-of-AND branches in the authored order. Empty branches are false.
pub fn evaluate_conditions<E>(
    mut branches: Option<&mut OrCondition>,
    mut evaluate: impl FnMut(&mut Condition) -> Result<bool, E>,
) -> Result<bool, E> {
    while let Some(branch) = branches {
        if let Some(mut condition) = branch.first_and.as_deref_mut() {
            while evaluate(condition)? {
                let Some(next) = condition.next_and_condition.as_deref_mut() else {
                    return Ok(true);
                };
                condition = next;
            }
        }
        branches = branch.next_or.as_deref_mut();
    }
    Ok(false)
}

/// Execute an ordinary action chain. Returns the delay in frames if it pauses.
pub fn execute_action_chain(
    mut actions: Option<&ScriptAction>,
    driver: &mut impl ScriptProgramDriver,
) -> GameLogicResult<Option<f32>> {
    while let Some(action) = actions {
        match dispatch_action(action, driver)? {
            ScriptActionResult::Pending(frames)
                if !is_sequential_only_action(action.action_type) =>
            {
                return Ok(Some(frames));
            }
            ScriptActionResult::Failed(message) => {
                return Err(ScriptActionError::Failed(message).into());
            }
            ScriptActionResult::Success | ScriptActionResult::Pending(_) => {}
        }
        actions = action.next_action.as_deref();
    }
    Ok(None)
}

/// Dispatch one action, completing immediate subroutines before the callback.
pub fn dispatch_action(
    action: &ScriptAction,
    driver: &mut impl ScriptProgramDriver,
) -> GameLogicResult<ScriptActionResult> {
    let result = if action.action_type == ScriptActionType::CallSubroutine {
        let name = action
            .get_parameter(0)
            .ok_or(ScriptActionError::MissingSubroutineName)?
            .get_string();
        driver.call_subroutine(name)?;
        ScriptActionResult::Success
    } else {
        driver.execute_action(action)?
    };
    driver.after_action()?;
    Ok(result)
}

fn pending_resume_frame(frame: u32, frames: f32) -> u32 {
    frame.saturating_add(frames.max(1.0).ceil() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::program::{ConditionType, Parameter, ParameterType};

    fn condition(id: i32, next: Option<Box<Condition>>) -> Option<Box<Condition>> {
        let mut condition = Condition::new(ConditionType::ConditionTrue);
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, id))
            .unwrap();
        condition.next_and_condition = next;
        Some(Box::new(condition))
    }

    #[test]
    fn condition_walk_skips_empty_branches_and_short_circuits_in_authored_order() {
        let mut branches = OrCondition::new();
        let mut third = OrCondition::new();
        third.first_and = condition(5, None);
        let mut second = OrCondition::new();
        second.first_and = condition(3, condition(4, None));
        second.next_or = Some(Box::new(third));
        let mut first = OrCondition::new();
        first.first_and = condition(1, condition(2, None));
        first.next_or = Some(Box::new(second));
        branches.next_or = Some(Box::new(first));

        let mut visited = Vec::new();
        let result = evaluate_conditions(Some(&mut branches), |condition| {
            let id = condition.get_parameter(0).unwrap().get_int();
            visited.push(id);
            Ok::<_, ()>(id != 1)
        });
        assert_eq!(result, Ok(true));
        assert_eq!(visited, [1, 3, 4]);
    }

    #[test]
    fn pending_resume_frame_is_next_frame_for_single_frame_wait() {
        assert_eq!(pending_resume_frame(100, 1.0), 101);
        assert_eq!(pending_resume_frame(100, 0.0), 101);
    }
}
