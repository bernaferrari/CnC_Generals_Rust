//! Authored object creation and team prototype actions.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::world::{ScriptAiPlayerRequest, ScriptObjectCreation};
use super::{ScriptActionResult, ScriptError};

pub enum CreationAction<'a> {
    Object {
        name: Option<&'a str>,
        request: ScriptObjectCreation<'a>,
    },
    AtWaypoint {
        name: Option<&'a str>,
        template: &'a str,
        team: &'a str,
        waypoint: &'a str,
    },
    Reinforcements {
        prototype: &'a str,
        waypoint: &'a str,
    },
    TeamProduction {
        prototype: &'a str,
        recruit_radius: Option<f32>,
    },
}

impl<'a> CreationAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let parameter = |slot| {
            action
                .get_parameter(slot)
                .ok_or_else(|| ScriptError::missing_parameter(slot))
        };
        let kind = action.get_action_type();
        let creation = match kind {
            CreateObject | UnitSpawnNamedLocationOrientation => {
                let name = (kind == UnitSpawnNamedLocationOrientation)
                    .then(|| parameter(0).map(|parameter| parameter.get_string()))
                    .transpose()?;
                let offset = usize::from(name.is_some());
                let template = parameter(offset)?.get_string();
                let team = parameter(offset + 1)?.get_string();
                let position = parameter(offset + 2)?.get_coord();
                let angle = parameter(offset + 3)?.get_real();
                Self::Object {
                    name,
                    request: ScriptObjectCreation {
                        template,
                        team,
                        x: position.x,
                        y: position.y,
                        z: position.z,
                        angle,
                    },
                }
            }
            CreateNamedOnTeamAtWaypoint | CreateUnnamedOnTeamAtWaypoint => {
                let name = (kind == CreateNamedOnTeamAtWaypoint)
                    .then(|| parameter(0).map(|parameter| parameter.get_string()))
                    .transpose()?;
                let offset = usize::from(name.is_some());
                Self::AtWaypoint {
                    name,
                    template: parameter(offset)?.get_string(),
                    team: parameter(offset + 1)?.get_string(),
                    waypoint: parameter(offset + 2)?.get_string(),
                }
            }
            CreateReinforcementTeam => Self::Reinforcements {
                prototype: parameter(0)?.get_string(),
                waypoint: parameter(1)?.get_string(),
            },
            BuildTeam => Self::TeamProduction {
                prototype: parameter(0)?.get_string(),
                recruit_radius: None,
            },
            RecruitTeam => Self::TeamProduction {
                prototype: parameter(0)?.get_string(),
                recruit_radius: Some(parameter(1)?.get_real()),
            },
            _ => return Ok(None),
        };
        Ok(Some(creation))
    }

    pub fn apply_to<Interpreter: ?Sized>(
        self,
        world: &mut dyn ScriptExecutionDriver<Interpreter>,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let calling = interpreter.calling_team();
        let condition = interpreter.condition_team();
        let result = match self {
            Self::Object {
                name: Some(name),
                request,
            } => world.create_named_object(name, request, calling, condition),
            Self::Object {
                name: None,
                request,
            } => world.create_object(request, calling, condition),
            Self::AtWaypoint {
                name,
                template,
                team,
                waypoint,
            } => world.create_named_object_at_waypoint(
                name, template, team, waypoint, calling, condition,
            ),
            // ScriptActions.cpp:480 looks up the authored TeamPrototype directly.
            Self::Reinforcements {
                prototype,
                waypoint,
            } => world.create_reinforcements(prototype, waypoint),
            // ScriptActions.cpp:2284–2309 uses the literal prototype name.
            Self::TeamProduction {
                prototype,
                recruit_radius,
            } => {
                let request = match recruit_radius {
                    Some(radius) => ScriptAiPlayerRequest::RecruitTeam {
                        team: prototype,
                        radius,
                    },
                    None => ScriptAiPlayerRequest::BuildTeam { team: prototype },
                };
                if let Err(error) = world.ai_player(request)? {
                    log::warn!("Scripted team production for '{prototype}' failed: {error}");
                }
                return Some(Ok(ScriptActionResult::Success));
            }
        }?;
        Some(
            result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string())),
        )
    }
}
