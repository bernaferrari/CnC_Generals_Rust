//! Synchronous scripted delete, kill, damage, and containment effects.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::{ScriptActionResult, ScriptError};

#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleAction {
    NamedDelete { unit: String },
    NamedKill { unit: String },
    NamedDamage { unit: String, amount: i32 },
    TeamDelete { team: String, ignore_dead: bool },
    TeamKill { team: String },
    TeamDamage { team: String, amount: f32 },
    DestroyAllContained { unit: String },
}

impl LifecycleAction {
    pub fn decode(action: &ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let parameter = |slot| {
            action
                .get_parameter(slot)
                .ok_or_else(|| ScriptError::missing_parameter(slot))
        };
        let name = || parameter(0).map(|value| value.get_string().to_owned());
        let kind = action.get_action_type();
        let effect = match kind {
            NamedDelete => Self::NamedDelete { unit: name()? },
            NamedKill => Self::NamedKill { unit: name()? },
            NamedDamage => Self::NamedDamage {
                unit: name()?,
                amount: parameter(1)?.get_int(),
            },
            TeamDelete | TeamDeleteLiving => Self::TeamDelete {
                team: name()?,
                ignore_dead: kind == TeamDeleteLiving,
            },
            TeamKill => Self::TeamKill { team: name()? },
            DamageMembersOfTeam => Self::TeamDamage {
                team: name()?,
                amount: parameter(1)?.get_real(),
            },
            UnitDestroyAllContained => Self::DestroyAllContained { unit: name()? },
            _ => return Ok(None),
        };
        Ok(Some(effect))
    }

    pub fn apply_to<Interpreter: ?Sized>(
        &self,
        world: &mut dyn ScriptExecutionDriver<Interpreter>,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        Some(
            world
                .object_lifecycle(
                    self,
                    interpreter.script_object_id(),
                    interpreter.calling_team(),
                    interpreter.condition_team(),
                )?
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string())),
        )
    }
}
