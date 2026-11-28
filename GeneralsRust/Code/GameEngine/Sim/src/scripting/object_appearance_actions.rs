//! Synchronous scripted visual changes published by the receiving world.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::object_status_actions::ObjectStatusTarget;
use super::program::{ScriptAction, ScriptActionType};
use super::{ScriptActionResult, ScriptError};
use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectAppearanceAction {
    CustomColor {
        unit: String,
        color: u32,
    },
    Flash {
        target: ObjectStatusTarget,
        seconds: i32,
        white: bool,
    },
    Emoticon {
        target: ObjectStatusTarget,
        name: String,
        frames: i32,
    },
}

impl ObjectAppearanceAction {
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
            NamedCustomColor => Self::CustomColor {
                unit: name(0)?,
                color: parameter(1)?.get_int() as u32,
            },
            NamedFlash | NamedFlashWhite | TeamFlash | TeamFlashWhite => Self::Flash {
                target: if matches!(kind, TeamFlash | TeamFlashWhite) {
                    ObjectStatusTarget::Team(name(0)?)
                } else {
                    ObjectStatusTarget::Named(name(0)?)
                },
                seconds: parameter(1)?.get_int(),
                white: matches!(kind, NamedFlashWhite | TeamFlashWhite),
            },
            NamedSetEmoticon | TeamSetEmoticon => Self::Emoticon {
                target: if kind == NamedSetEmoticon {
                    ObjectStatusTarget::Named(name(0)?)
                } else {
                    ObjectStatusTarget::Team(name(0)?)
                },
                name: name(1)?,
                frames: (parameter(2)?.get_real() * LOGICFRAMES_PER_SECOND as f32) as i32,
            },
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
                .object_appearance(
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
