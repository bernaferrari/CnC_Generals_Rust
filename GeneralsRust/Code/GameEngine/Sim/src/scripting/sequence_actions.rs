//! Authored sequence admission and removal, from ScriptActions.cpp:4657–4788.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{Script, ScriptAction, ScriptActionType};
use super::sequential_scripts::SequentialScript;
use super::world::ScriptOwnerQuery;
use super::{ScriptActionResult, ScriptError};

enum SequenceActor<'a> {
    Unit(&'a str),
    Team(&'a str),
}

enum SequenceOperation<'a> {
    Start { script: &'a str, loops: i32 },
    Stop,
}

pub struct SequenceAction<'a> {
    actor: SequenceActor<'a>,
    operation: SequenceOperation<'a>,
}

impl<'a> SequenceAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let parameter = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let kind = action.get_action_type();
        let actor = match kind {
            UnitExecuteSequentialScript
            | UnitExecuteSequentialScriptLooping
            | UnitStopSequentialScript => SequenceActor::Unit(parameter(0)?.get_string()),
            TeamExecuteSequentialScript
            | TeamExecuteSequentialScriptLooping
            | TeamStopSequentialScript => SequenceActor::Team(parameter(0)?.get_string()),
            _ => return Ok(None),
        };
        let operation = match kind {
            UnitStopSequentialScript | TeamStopSequentialScript => SequenceOperation::Stop,
            _ => {
                let script = parameter(1)?.get_string();
                let loops = if matches!(
                    kind,
                    UnitExecuteSequentialScriptLooping | TeamExecuteSequentialScriptLooping
                ) {
                    parameter(2)?.get_int().checked_sub(1).ok_or_else(|| {
                        ScriptError::InvalidParameterType("Sequence loop count underflow".into())
                    })?
                } else {
                    0
                };
                SequenceOperation::Start { script, loops }
            }
        };
        Ok(Some(Self { actor, operation }))
    }

    fn program(&self, interpreter: &ScriptInterpreter) -> Option<(Script, i32)> {
        let SequenceOperation::Start { script, loops } = self.operation else {
            return None;
        };
        interpreter
            .sides()
            .find_script(script, false, Script::clone)
            .map(|script| (script, loops))
    }

    pub fn apply_to(
        &self,
        world: &mut dyn ScriptExecutionDriver,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let starting = matches!(self.operation, SequenceOperation::Start { .. });
        let mut sequence = SequentialScript::new();
        let program = match self.actor {
            SequenceActor::Unit(name) => {
                let object = match world.named_object_id(name, interpreter.script_object_id()) {
                    ScriptOwnerQuery::Present(id) => id,
                    ScriptOwnerQuery::Missing => return Some(Ok(ScriptActionResult::Success)),
                    ScriptOwnerQuery::Unavailable => return None,
                };
                if !starting {
                    interpreter.sequential().remove_object(object);
                    return Some(Ok(ScriptActionResult::Success));
                }
                sequence.object_id = object;
                self.program(interpreter)
            }
            SequenceActor::Team(name) => {
                // Resolve the program before the receiving owner may idle its group.
                let program = self.program(interpreter);
                if starting && program.is_none() {
                    return Some(Ok(ScriptActionResult::Success));
                }
                match world.team_for_sequence(
                    name,
                    interpreter.calling_team(),
                    interpreter.condition_team(),
                    starting,
                )? {
                    Ok(Some(id)) if starting => sequence.team_to_exec_on = Some(id),
                    Ok(Some(id)) => {
                        interpreter.remove_team_sequences(id);
                        return Some(Ok(ScriptActionResult::Success));
                    }
                    Ok(None) => return Some(Ok(ScriptActionResult::Success)),
                    Err(error) => {
                        return Some(Err(ScriptError::ExecutionFailed(error.to_string())));
                    }
                }
                program
            }
        };
        if let Some((script, loops)) = program {
            sequence.script_to_execute_sequentially = Some(Box::new(script));
            sequence.times_to_loop = loops;
            interpreter.sequential().append(sequence);
        }
        Some(Ok(ScriptActionResult::Success))
    }
}
