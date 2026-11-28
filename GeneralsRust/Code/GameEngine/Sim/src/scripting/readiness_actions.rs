//! Authored hunt, guard, stop and idle orders.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::world::{ScriptNamedCommand, ScriptOwnerQuery};
use super::{ScriptActionResult, ScriptError};

pub enum ReadinessAction<'a> {
    Named(ScriptNamedCommand<'a>),
    TimedNamed(ScriptNamedCommand<'a>, i32),
    TeamHunt(&'a str),
    TeamHuntWithButton(&'a str, &'a str),
    TeamGuard(&'a str),
    TeamGuardPosition(&'a str, &'a str),
    TeamGuardObject(&'a str, &'a str),
    TeamGuardArea(&'a str, &'a str),
    TeamGuardSupplyCenter(&'a str, i32),
    TeamGuardTunnel(&'a str),
    TeamStop(&'a str, bool),
    TimedTeamIdle(&'a str, i32),
    PlayerUnitsIdle(bool),
}

impl<'a> ReadinessAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let parameter = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let name = || parameter(0).map(|parameter| parameter.get_string());
        let target = || parameter(1).map(|parameter| parameter.get_string());
        let count = || parameter(1).map(|parameter| parameter.get_int());
        let request = match action.get_action_type() {
            NamedHunt => Self::Named(ScriptNamedCommand::Hunt { unit: name()? }),
            NamedGuard => Self::Named(ScriptNamedCommand::Guard { unit: name()? }),
            NamedStop => Self::Named(ScriptNamedCommand::Stop { unit: name()? }),
            UnitGuardForFramecount => {
                Self::TimedNamed(ScriptNamedCommand::Guard { unit: name()? }, count()?)
            }
            UnitIdleForFramecount => {
                Self::TimedNamed(ScriptNamedCommand::Stop { unit: name()? }, count()?)
            }
            TeamHunt => Self::TeamHunt(name()?),
            TeamHuntWithCommandButton => Self::TeamHuntWithButton(name()?, target()?),
            TeamGuard => Self::TeamGuard(name()?),
            TeamGuardPosition => Self::TeamGuardPosition(name()?, target()?),
            TeamGuardObject => Self::TeamGuardObject(name()?, target()?),
            TeamGuardArea => Self::TeamGuardArea(name()?, target()?),
            TeamGuardSupplyCenter => Self::TeamGuardSupplyCenter(name()?, count()?),
            TeamGuardInTunnelNetwork => Self::TeamGuardTunnel(name()?),
            TeamStop => Self::TeamStop(name()?, false),
            TeamStopAndDisband => Self::TeamStop(name()?, true),
            // Both opcodes call doTeamIdleForFramecount in ScriptActions.cpp.
            TeamIdleForFramecount | TeamGuardForFramecount => {
                Self::TimedTeamIdle(name()?, count()?)
            }
            // The C++ dispatcher always supplies the empty player name.
            IdleAllUnits => Self::PlayerUnitsIdle(true),
            ResumeSupplyTrucking => Self::PlayerUnitsIdle(false),
            _ => return Ok(None),
        };
        Ok(Some(request))
    }

    pub fn apply_to(
        self,
        world: &mut dyn ScriptExecutionDriver,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let object = interpreter.script_object_id();
        let calling = interpreter.calling_team();
        let condition = interpreter.condition_team();
        let execution_error =
            |error: crate::GameLogicError| ScriptError::ExecutionFailed(error.to_string());
        let wait = |admitted, frames| {
            if admitted && frames > 0 {
                ScriptActionResult::Pending(frames as f32)
            } else {
                ScriptActionResult::Success
            }
        };
        let result = match self {
            Self::Named(command) => world.named_command(command, object),
            Self::TimedNamed(command, frames) => {
                return match world.named_frame_command(command, object) {
                    ScriptOwnerQuery::Present(()) => Some(Ok(wait(true, frames))),
                    ScriptOwnerQuery::Missing => Some(Ok(ScriptActionResult::Success)),
                    ScriptOwnerQuery::Unavailable => None,
                };
            }
            Self::TeamHunt(team) => world.team_hunt(team, calling, condition),
            Self::TeamHuntWithButton(team, button) => {
                world.team_hunt_button(team, button, calling, condition)
            }
            Self::TeamGuard(team) => world.team_guard(team, calling, condition),
            Self::TeamGuardPosition(team, waypoint) => {
                world.team_guard_position(team, waypoint, calling, condition)
            }
            Self::TeamGuardObject(team, target) => {
                world.team_guard_object(team, target, object, calling, condition)
            }
            Self::TeamGuardArea(team, area) => world.team_guard_area(
                team,
                area,
                interpreter.current_player().as_deref(),
                calling,
                condition,
            ),
            Self::TeamGuardSupplyCenter(team, supplies) => {
                world.team_guard_supply_center(team, supplies, calling, condition)
            }
            Self::TeamGuardTunnel(team) => world.team_guard_tunnel(team, calling, condition),
            Self::TeamStop(team, disband) => world.team_stop(team, calling, condition, disband),
            Self::TimedTeamIdle(team, frames) => {
                let result = world.team_idle(team, calling, condition)?;
                return Some(
                    result
                        .map(|admitted| wait(admitted, frames))
                        .map_err(execution_error),
                );
            }
            Self::PlayerUnitsIdle(idle) => world.player_units_idle(idle),
        }?;
        Some(
            result
                .map(|()| ScriptActionResult::Success)
                .map_err(execution_error),
        )
    }
}
