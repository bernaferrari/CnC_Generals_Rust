//! Ordinary script execution and the action dispatch shared by both interpreters.
//! C++ ScriptEngine.cpp:6829–6859, 6926–7022, 7558–7650.

use super::ScriptActionResult;
use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::media_conditions::ScriptMediaQueries;
use super::player_area_supply_conditions::ScriptAreaSupplyQueries;
use super::player_event_conditions::ScriptPlayerEventQueries;
use super::program::{Condition, OrCondition, Script, ScriptAction, ScriptActionType};
use super::sequential_scripts::is_sequential_only_action;
use crate::time::LOGIC_HZ;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::system::kind_of::KindOfMask;

#[derive(Clone, Copy)]
pub enum ScriptDifficulty {
    Easy,
    Normal,
    Hard,
}

impl From<generals_game_domain::AIDifficulty> for ScriptDifficulty {
    fn from(difficulty: generals_game_domain::AIDifficulty) -> Self {
        use generals_game_domain::AIDifficulty;

        match difficulty {
            AIDifficulty::Easy => Self::Easy,
            AIDifficulty::Medium => Self::Normal,
            AIDifficulty::Hard | AIDifficulty::Brutal => Self::Hard,
        }
    }
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
    fn condition_queries(&mut self) -> impl ScriptConditionQueries + '_;
    fn evaluate_world_condition(&mut self, condition: &mut Condition) -> GameLogicResult<bool>;
    fn execute_world_action(
        &mut self,
        action: &ScriptAction,
    ) -> GameLogicResult<ScriptActionResult>;
    fn timer_random_int(&self, low: i32, high: i32) -> i32;
    fn timer_random_real(&self, low: f32, high: f32) -> f32;
    fn script_activation_changed(&self, _name: &str, _active: bool) {}
    fn priority_template_name(&self, _name: &str) -> Option<String> {
        None
    }
    fn priority_templates_with_kind(&self, _kind: KindOfMask) -> Option<Vec<String>> {
        None
    }
    fn call_subroutine(&mut self, name: &str) -> GameLogicResult<()>;
    fn after_action(&mut self) -> GameLogicResult<()>;
}

pub trait ScriptConditionQueries:
    ScriptMediaQueries + ScriptPlayerEventQueries + ScriptAreaSupplyQueries
{
    type HostInterpreter: ?Sized;

    fn receiving_world(&mut self) -> &mut dyn ScriptExecutionDriver<Self::HostInterpreter>;

    fn can_observe_transport(&self) -> bool {
        true
    }
}

impl ScriptInterpreter {
    pub fn try_evaluate_receiving_condition(
        &self,
        condition: &mut Condition,
        frame: u32,
        queries: &mut impl ScriptConditionQueries,
    ) -> Result<Option<bool>, super::ScriptError> {
        if let Some(value) = self.try_evaluate_player_event(condition, queries)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_evaluate_player_census(condition, queries)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_evaluate_area_supply_condition(condition, frame, queries)? {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_evaluate_command_button_condition(condition, queries)? {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.try_evaluate_skirmish_condition(condition, frame, queries.receiving_world())?
        {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.try_evaluate_map_condition(condition, queries.receiving_world())?
        {
            return Ok(Some(value));
        }
        if let Some(value) = self.try_evaluate_named_condition(condition, queries)? {
            return Ok(Some(value));
        }
        if let Some(value) =
            self.try_evaluate_team_combat_condition(condition, queries.receiving_world())?
        {
            return Ok(Some(value));
        }
        self.try_evaluate_team_condition(condition, queries)
    }
}

/// Run one installed script under the interpreter's condition-team scope.
pub(super) fn execute_script(
    interpreter: &ScriptInterpreter,
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
            interpreter.set_condition_team(Some(team));
            evaluate_script(interpreter, script, frame, driver, false)?;
            // Like DLINK_ITERATOR::advance, read the next team after the
            // condition and action callbacks have had a chance to mutate it.
            let Some(next) = driver.condition_team_after(&team_name, Some(team)) else {
                return Ok(());
            };
            team = next;
        }
    }

    interpreter.set_condition_team(None);
    evaluate_script(interpreter, script, frame, driver, true)
}

fn evaluate_script(
    interpreter: &ScriptInterpreter,
    script: &mut Script,
    frame: u32,
    driver: &mut impl ScriptProgramDriver,
    deactivate_on_false_action: bool,
) -> GameLogicResult<()> {
    let condition_true = evaluate_conditions(script.condition.as_deref_mut(), |condition| {
        let local = interpreter
            .try_evaluate_local_condition(condition)
            .and_then(|value| {
                if value.is_some() {
                    return Ok(value);
                }
                let mut queries = driver.condition_queries();
                if let Some(value) =
                    interpreter.try_evaluate_media_condition(condition, frame, &mut queries)?
                {
                    return Ok(Some(value));
                }
                interpreter.try_evaluate_receiving_condition(condition, frame, &mut queries)
            })
            .map_err(|error| GameLogicError::Configuration(format!("Script condition error: {error}")))?;
        match local {
            Some(value) => Ok(value),
            None => driver.evaluate_world_condition(condition),
        }
    })?;
    let actions = if condition_true {
        script.action.as_deref()
    } else if let Some(action) = script.action_false.as_deref() {
        Some(action)
    } else {
        return Ok(());
    };
    match execute_action_chain(interpreter, actions, driver)? {
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
    interpreter: &ScriptInterpreter,
    mut actions: Option<&ScriptAction>,
    driver: &mut impl ScriptProgramDriver,
) -> GameLogicResult<Option<f32>> {
    while let Some(action) = actions {
        match dispatch_action(interpreter, action, driver)? {
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
    interpreter: &ScriptInterpreter,
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
    } else if interpreter
        .try_execute_variable_action(
            action,
            |low, high| driver.timer_random_int(low, high),
            |low, high| driver.timer_random_real(low, high),
        )
        .map_err(|error| GameLogicError::Configuration(format!("Script action error: {error}")))?
        || interpreter
            .try_execute_control_action(
                action,
                |name, active| driver.script_activation_changed(name, active),
                |name| driver.priority_template_name(name),
                |kind| driver.priority_templates_with_kind(kind),
            )
            .map_err(|error| {
                GameLogicError::Configuration(format!("Script action error: {error}"))
            })?
    {
        ScriptActionResult::Success
    } else {
        let wait = interpreter.try_execute_command_button_wait(action, driver)?;
        match wait {
            Some(result) => result,
            None => driver.execute_world_action(action)?,
        }
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

    #[test]
    fn variable_opcodes_stay_inside_the_interpreter_and_notify_after_each_action() {
        struct UnusedQueries;

        impl ScriptMediaQueries for UnusedQueries {
            fn audio_duration_ms(&self, _name: &str) -> f32 {
                panic!("variable condition requested an audio duration")
            }

            fn music_track_completed(
                &mut self,
                _name: &str,
                _times: i32,
            ) -> Result<bool, super::super::ScriptError> {
                panic!("variable condition queried music playback")
            }
        }

        impl ScriptPlayerEventQueries for UnusedQueries {}
        impl super::super::player_census_conditions::ScriptCensusQueries for UnusedQueries {}
        impl ScriptAreaSupplyQueries for UnusedQueries {}

        impl ScriptConditionQueries for UnusedQueries {
            type HostInterpreter = ();

            fn receiving_world(&mut self) -> &mut dyn ScriptExecutionDriver<()> {
                panic!("variable condition queried the receiving world")
            }
        }

        struct World<'a> {
            interpreter: &'a ScriptInterpreter,
            observed: Vec<i32>,
        }

        impl ScriptProgramDriver for World<'_> {
            fn difficulty(&self) -> ScriptDifficulty {
                ScriptDifficulty::Normal
            }

            fn condition_team_after(&self, name: &str, after: Option<u32>) -> Option<u32> {
                (name == "Patrol" && after.is_none()).then_some(23)
            }

            fn condition_queries(&mut self) -> impl ScriptConditionQueries + '_ {
                UnusedQueries
            }

            fn evaluate_world_condition(
                &mut self,
                condition: &mut Condition,
            ) -> GameLogicResult<bool> {
                panic!(
                    "variable condition reached the world: {:?}",
                    condition.condition_type
                )
            }

            fn execute_world_action(
                &mut self,
                action: &ScriptAction,
            ) -> GameLogicResult<ScriptActionResult> {
                assert_eq!(action.action_type, ScriptActionType::DebugMessageBox);
                assert_eq!(self.interpreter.condition_team(), Some(23));
                assert_eq!(
                    self.interpreter
                        .frame()
                        .variables
                        .counter("Score")
                        .unwrap()
                        .value,
                    9
                );
                Ok(ScriptActionResult::Success)
            }

            fn timer_random_int(&self, low: i32, high: i32) -> i32 {
                assert_eq!((low, high), (3, 15));
                9
            }

            fn timer_random_real(&self, _low: f32, _high: f32) -> f32 {
                panic!("integer timer requested a real draw")
            }

            fn call_subroutine(&mut self, name: &str) -> GameLogicResult<()> {
                assert_eq!(name, "Then");
                self.interpreter
                    .frame_mut()
                    .variables
                    .set_counter("Score", 11)
            }

            fn after_action(&mut self) -> GameLogicResult<()> {
                self.observed.push(
                    self.interpreter
                        .frame()
                        .variables
                        .counter("Score")
                        .unwrap()
                        .value,
                );
                Ok(())
            }
        }

        let score_name = || Parameter::with_string(ParameterType::Counter, "Score".into());
        let number = |value| Parameter::with_int(ParameterType::Int, value);
        let mut counter = Condition::new(ConditionType::Counter);
        for param in [
            score_name(),
            Parameter::with_int(ParameterType::Comparison, 2),
            number(0),
        ] {
            counter.add_parameter(param).unwrap();
        }
        let mut condition = OrCondition::new();
        condition.set_first_and_condition(Some(Box::new(counter)));
        let mut timer = ScriptAction::new(ScriptActionType::SetRandomTimer);
        for param in [score_name(), number(3), number(15)] {
            timer.add_parameter(param).unwrap();
        }
        let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
        call.add_parameter(Parameter::with_string(
            ParameterType::ScriptSubroutine,
            "Then".into(),
        ))
        .unwrap();

        let mut script = Script::new();
        script.condition_team_name = "Patrol".into();
        script.condition = Some(Box::new(condition));
        for action in [
            timer,
            ScriptAction::new(ScriptActionType::DebugMessageBox),
            call,
        ] {
            script.append_action(Box::new(action));
        }

        let interpreter = ScriptInterpreter::new(1);
        interpreter.set_condition_team(Some(12));
        let mut world = World {
            interpreter: &interpreter,
            observed: Vec::new(),
        };
        interpreter
            .execute_script(&mut script, 0, &mut world)
            .unwrap();
        assert_eq!(world.observed, [9, 9, 11]);
        assert_eq!(interpreter.condition_team(), Some(12));
    }
}
