//! Scripted object controls and template overrides.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::{ScriptActionResult, ScriptError};

pub struct ForceSelection<'a> {
    pub team: &'a str,
    pub template: &'a str,
    pub center_in_view: bool,
    pub audio: &'a str,
}

pub enum NamedPowerTarget<'a> {
    Waypoint(&'a str),
    Named(&'a str),
}

pub struct NamedSpecialPower<'a> {
    pub unit: &'a str,
    pub power: &'a str,
    pub target: NamedPowerTarget<'a>,
}

pub enum ObjectControlAction<'a> {
    NamedAttitude {
        unit: &'a str,
        mood: i32,
    },
    TeamAttitude {
        team: &'a str,
        mood: i32,
    },
    ForceSelect(ForceSelection<'a>),
    SetBuildability {
        template: &'a str,
        status: i32,
    },
    GrantUpgrade {
        unit: &'a str,
        upgrade: &'a str,
    },
    SetToppleDirection {
        name: &'a str,
        direction: glam::Vec3,
    },
    FireSpecialPower(NamedSpecialPower<'a>),
}

impl<'a> ObjectControlAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        let parameter = |slot| {
            action
                .get_parameter(slot)
                .ok_or_else(|| ScriptError::missing_parameter(slot))
        };
        let kind = action.get_action_type();
        let effect = match kind {
            ScriptActionType::NamedSetAttitude => Self::NamedAttitude {
                unit: parameter(0)?.get_string(),
                mood: parameter(1)?.get_int(),
            },
            ScriptActionType::TeamSetAttitude => Self::TeamAttitude {
                team: parameter(0)?.get_string(),
                mood: parameter(1)?.get_int(),
            },
            ScriptActionType::ObjectForceSelect => Self::ForceSelect(ForceSelection {
                team: parameter(0)?.get_string(),
                template: parameter(1)?.get_string(),
                center_in_view: action.get_parameter(2).is_some_and(|p| p.get_int() != 0),
                audio: action
                    .get_parameter(3)
                    .map(|p| p.get_string())
                    .unwrap_or(""),
            }),
            ScriptActionType::TechtreeModifyBuildabilityObject => Self::SetBuildability {
                template: parameter(0)?.get_string(),
                status: parameter(1)?.get_int(),
            },
            ScriptActionType::NamedReceiveUpgrade => Self::GrantUpgrade {
                unit: parameter(0)?.get_string(),
                upgrade: parameter(1)?.get_string(),
            },
            ScriptActionType::NamedSetToppleDirection => Self::SetToppleDirection {
                name: parameter(0)?.get_string(),
                direction: parameter(1)?.get_coord(),
            },
            ScriptActionType::NamedFireSpecialPowerAtWaypoint
            | ScriptActionType::NamedFireSpecialPowerAtNamed => {
                Self::FireSpecialPower(NamedSpecialPower {
                    unit: parameter(0)?.get_string(),
                    power: parameter(1)?.get_string(),
                    target: if kind == ScriptActionType::NamedFireSpecialPowerAtWaypoint {
                        NamedPowerTarget::Waypoint(parameter(2)?.get_string())
                    } else {
                        NamedPowerTarget::Named(parameter(2)?.get_string())
                    },
                })
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
        let result = match self {
            Self::NamedAttitude { unit, mood } => {
                world.named_attitude(unit, interpreter.script_object_id(), *mood)
            }
            Self::TeamAttitude { team, mood } => world.team_attitude(
                team,
                interpreter.calling_team(),
                interpreter.condition_team(),
                *mood,
            ),
            Self::ForceSelect(request) => world.force_select_object(
                request,
                interpreter.calling_team(),
                interpreter.condition_team(),
            ),
            Self::SetBuildability { template, status } => {
                world.set_buildable_status_override(template, *status)
            }
            Self::GrantUpgrade { unit, upgrade } => {
                world.named_upgrade(unit, upgrade, interpreter.script_object_id())
            }
            Self::SetToppleDirection { name, direction } => {
                interpreter.set_topple_direction(name, Some(*direction));
                Some(Ok(()))
            }
            Self::FireSpecialPower(request) => {
                world.named_special_power(request, interpreter.script_object_id())
            }
        }?;
        Some(
            result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string())),
        )
    }
}
