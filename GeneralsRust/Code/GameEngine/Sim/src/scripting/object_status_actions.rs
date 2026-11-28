//! Synchronous mutations of object status through the receiving simulation.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::{ScriptActionResult, ScriptError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectStatusTarget {
    Named(String),
    Team(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectStatusAction {
    SetUnmanned(ObjectStatusTarget),
    DeleteAllUnmanned,
    SetStealth {
        target: ObjectStatusTarget,
        enabled: bool,
    },
    PlantBoobytrap {
        target: ObjectStatusTarget,
        template: String,
    },
    SetHeld {
        unit: String,
        held: bool,
    },
    SetRepulsor {
        target: ObjectStatusTarget,
        enabled: bool,
    },
    SetStoppingDistance {
        target: ObjectStatusTarget,
        distance: f32,
    },
}

impl ObjectStatusAction {
    pub fn decode(action: &ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let parameter = |slot| {
            action
                .get_parameter(slot)
                .ok_or_else(|| ScriptError::missing_parameter(slot))
        };
        let name = |slot| parameter(slot).map(|value| value.get_string().to_owned());
        let kind = action.get_action_type();
        let effect = match kind {
            NamedSetUnmannedStatus => Self::SetUnmanned(ObjectStatusTarget::Named(name(0)?)),
            TeamSetUnmannedStatus => Self::SetUnmanned(ObjectStatusTarget::Team(name(0)?)),
            DeleteAllUnmanned => Self::DeleteAllUnmanned,
            NamedSetHeld => Self::SetHeld {
                unit: name(0)?,
                held: parameter(1)?.get_int() != 0,
            },
            NamedSetRepulsor | TeamSetRepulsor => Self::SetRepulsor {
                target: if kind == NamedSetRepulsor {
                    ObjectStatusTarget::Named(name(0)?)
                } else {
                    ObjectStatusTarget::Team(name(0)?)
                },
                enabled: parameter(1)?.get_int() != 0,
            },
            NamedSetStoppingDistance | SetStoppingDistance => Self::SetStoppingDistance {
                target: if kind == NamedSetStoppingDistance {
                    ObjectStatusTarget::Named(name(0)?)
                } else {
                    ObjectStatusTarget::Team(name(0)?)
                },
                distance: parameter(1)?.get_real(),
            },
            NamedSetStealthEnabled | TeamSetStealthEnabled => Self::SetStealth {
                target: if kind == NamedSetStealthEnabled {
                    ObjectStatusTarget::Named(name(0)?)
                } else {
                    ObjectStatusTarget::Team(name(0)?)
                },
                enabled: parameter(1)?.get_int() != 0,
            },
            NamedSetBoobytrapped | TeamSetBoobytrapped => {
                let template = name(0)?;
                let target = if kind == NamedSetBoobytrapped {
                    ObjectStatusTarget::Named(name(1)?)
                } else {
                    ObjectStatusTarget::Team(name(1)?)
                };
                Self::PlantBoobytrap { target, template }
            }
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
                .object_status_action(
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
