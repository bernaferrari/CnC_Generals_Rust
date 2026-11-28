//! Authored object panel and railroad controls from ScriptActions.cpp.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::{ScriptActionResult, ScriptError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectPanelFlag {
    Enabled,
    Powered,
    Indestructible,
    Unsellable,
    Selectable,
    AiRecruitable,
    PlayerTargetable,
}

impl ObjectPanelFlag {
    pub fn parse(flag: &str) -> Option<Self> {
        let normalized = flag
            .chars()
            .filter(|c| !c.is_ascii_whitespace() && *c != '_')
            .collect::<String>()
            .to_ascii_lowercase();
        match normalized.as_str() {
            "enabled" => Some(Self::Enabled),
            "powered" => Some(Self::Powered),
            "indestructible" => Some(Self::Indestructible),
            "unsellable" => Some(Self::Unsellable),
            "selectable" => Some(Self::Selectable),
            "airecruitable" => Some(Self::AiRecruitable),
            "playertargetable" => Some(Self::PlayerTargetable),
            _ => None,
        }
    }
}

pub enum PanelTarget<'a> {
    Named(&'a str),
    Team(&'a str),
}

pub enum ObjectAction<'a> {
    PanelFlag {
        target: PanelTarget<'a>,
        flag: ObjectPanelFlag,
        enabled: bool,
    },
    TrainHeld {
        name: &'a str,
        held: bool,
    },
    IgnoreUnknownPanelFlag,
}

impl<'a> ObjectAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let parameter = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let request = match action.get_action_type() {
            UnitAffectObjectPanelFlags | TeamAffectObjectPanelFlags => {
                let name = parameter(0)?.get_string();
                let flag = parameter(1)?.get_string();
                let enabled = parameter(2)?.get_int() != 0;
                let Some(flag) = ObjectPanelFlag::parse(flag) else {
                    return Ok(Some(Self::IgnoreUnknownPanelFlag));
                };
                let target = match action.get_action_type() {
                    UnitAffectObjectPanelFlags => PanelTarget::Named(name),
                    _ => PanelTarget::Team(name),
                };
                Self::PanelFlag {
                    target,
                    flag,
                    enabled,
                }
            }
            SetTrainHeld => Self::TrainHeld {
                name: parameter(0)?.get_string(),
                held: parameter(1)?.get_int() != 0,
            },
            _ => return Ok(None),
        };
        Ok(Some(request))
    }

    /// Deliver the effect with the current invocation's object or team identity.
    /// `None` requests the standalone world adapter.
    pub fn apply_to<Interpreter: ?Sized>(
        &self,
        world: &mut dyn ScriptExecutionDriver<Interpreter>,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let result = match self {
            Self::PanelFlag {
                target,
                flag,
                enabled,
            } => match target {
                PanelTarget::Named(name) => {
                    world.named_panel_flag(name, *flag, *enabled, interpreter.script_object_id())
                }
                PanelTarget::Team(name) => world.team_panel_flag(
                    name,
                    *flag,
                    *enabled,
                    interpreter.calling_team(),
                    interpreter.condition_team(),
                ),
            },
            Self::TrainHeld { name, held } => world
                .set_named_train_held(name, *held, interpreter.script_object_id())
                .map(Ok),
            Self::IgnoreUnknownPanelFlag => Some(Ok(())),
        }?;
        Some(
            result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string())),
        )
    }
}
