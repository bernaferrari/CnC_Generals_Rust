//! Authored waypoint, facing, and skirmish approach orders.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::world::{ScriptNamedCommand, ScriptNamedRuntimeRequest};
use super::{ScriptActionResult, ScriptError};

pub enum TeamFaceTarget<'a> {
    Object(&'a str),
    Waypoint(&'a str),
}

pub enum NavigationAction<'a> {
    NamedWaypoint {
        unit: &'a str,
        waypoint: &'a str,
        follow: Option<bool>,
    },
    TeamWaypoint(&'a str, &'a str),
    TeamPath {
        team: &'a str,
        path: &'a str,
        as_team: bool,
        exact: bool,
    },
    NamedFaceObject(&'a str, &'a str),
    NamedFaceWaypoint(&'a str, &'a str),
    TeamFace(&'a str, TeamFaceTarget<'a>),
    Approach {
        team: &'a str,
        path: &'a str,
        as_team: bool,
        follow: bool,
    },
}

impl<'a> NavigationAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let string = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
                .map(|parameter| parameter.get_string())
        };
        let kind = action.get_action_type();
        let request = match kind {
            MoveNamedUnitTo | NamedFollowWaypoints | NamedFollowWaypointsExact => {
                Self::NamedWaypoint {
                    unit: string(0)?,
                    waypoint: string(1)?,
                    follow: (kind != MoveNamedUnitTo).then_some(kind == NamedFollowWaypointsExact),
                }
            }
            MoveTeamTo => Self::TeamWaypoint(string(0)?, string(1)?),
            TeamFollowWaypoints | TeamFollowWaypointsExact => Self::TeamPath {
                team: string(0)?,
                path: string(1)?,
                as_team: action
                    .get_parameter(2)
                    .is_none_or(|parameter| parameter.get_int() != 0),
                exact: kind == TeamFollowWaypointsExact,
            },
            NamedFaceNamed => Self::NamedFaceObject(string(0)?, string(1)?),
            NamedFaceWaypoint => Self::NamedFaceWaypoint(string(0)?, string(1)?),
            TeamFaceNamed => Self::TeamFace(string(0)?, TeamFaceTarget::Object(string(1)?)),
            TeamFaceWaypoint => Self::TeamFace(string(0)?, TeamFaceTarget::Waypoint(string(1)?)),
            SkirmishFollowApproachPath | SkirmishMoveToApproachPath => {
                let team = string(0)?;
                let path = string(1)?;
                let follow = kind == SkirmishFollowApproachPath;
                let as_team = follow
                    && action
                        .get_parameter(2)
                        .ok_or_else(|| ScriptError::missing_parameter(2))?
                        .get_int()
                        != 0;
                Self::Approach {
                    team,
                    path,
                    as_team,
                    follow,
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(request))
    }

    pub fn apply_to<Interpreter: ?Sized>(
        self,
        world: &mut dyn ScriptExecutionDriver<Interpreter>,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let object = interpreter.script_object_id();
        let calling = interpreter.calling_team();
        let condition = interpreter.condition_team();
        let result = match self {
            Self::NamedWaypoint {
                unit,
                waypoint,
                follow,
            } => world.named_waypoint(unit, waypoint, follow, object),
            Self::TeamWaypoint(team, waypoint) => {
                world.team_move_to_waypoint(team, waypoint, calling, condition)
            }
            Self::TeamPath {
                team,
                path,
                as_team,
                exact,
            } => world.team_waypoint_path(team, path, as_team, exact, calling, condition),
            Self::NamedFaceObject(unit, target) => {
                world.named_command(ScriptNamedCommand::FaceObject { unit, target }, object)
            }
            Self::NamedFaceWaypoint(unit, waypoint) => world.named_runtime(
                ScriptNamedRuntimeRequest::FaceWaypoint { unit, waypoint },
                object,
            ),
            Self::TeamFace(team, target) => {
                world.team_face(team, target, object, calling, condition)
            }
            Self::Approach {
                team,
                path,
                as_team,
                follow,
            } => {
                let result = world.skirmish_approach_path(
                    team,
                    path,
                    as_team,
                    follow,
                    interpreter.current_player().as_deref(),
                    calling,
                    condition,
                )?;
                // Approach orders report receiving errors and let the script continue.
                if let Err(error) = result {
                    log::warn!("Owned approach path failed: {error}");
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
