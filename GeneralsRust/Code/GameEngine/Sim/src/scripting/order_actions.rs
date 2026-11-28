//! Authored attack orders and movement toward the nearest matching object.

use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::world::{ScriptNamedCommand, ScriptNearestObjectRequest, ScriptNearestRecipient};
use super::{ScriptActionResult, ScriptError};

pub enum OrderAction<'a> {
    NamedAttackNamed(&'a str, &'a str),
    NamedAttackArea(&'a str, &'a str),
    NamedAttackTeam(&'a str, &'a str),
    TeamAttackNamed(&'a str, &'a str),
    TeamAttackArea(&'a str, &'a str),
    TeamAttackTeam(&'a str, &'a str),
    MoveTowardsNearest {
        recipient: ScriptNearestRecipient<'a>,
        object_type: &'a str,
        trigger: &'a str,
    },
}

impl<'a> OrderAction<'a> {
    pub fn decode(action: &'a ScriptAction) -> Result<Option<Self>, ScriptError> {
        use ScriptActionType::*;
        let string = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
                .map(|parameter| parameter.get_string())
        };
        let order = match action.get_action_type() {
            NamedAttackNamed => Self::NamedAttackNamed(string(0)?, string(1)?),
            NamedAttackArea => Self::NamedAttackArea(string(0)?, string(1)?),
            NamedAttackTeam => Self::NamedAttackTeam(string(0)?, string(1)?),
            TeamAttackNamed => Self::TeamAttackNamed(string(0)?, string(1)?),
            TeamAttackArea => Self::TeamAttackArea(string(0)?, string(1)?),
            TeamAttackTeam => Self::TeamAttackTeam(string(0)?, string(1)?),
            UnitMoveTowardsNearestObjectType | TeamMoveTowardsNearestObjectType => {
                let name = string(0)?;
                let recipient = if action.get_action_type() == TeamMoveTowardsNearestObjectType {
                    ScriptNearestRecipient::Team(name)
                } else {
                    ScriptNearestRecipient::Named(name)
                };
                Self::MoveTowardsNearest {
                    recipient,
                    object_type: string(1)?,
                    trigger: string(2)?,
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(order))
    }

    /// Deliver the order with the active invocation's identities. A declined
    /// order is handled by the standalone adapter.
    pub fn apply_to(
        &self,
        world: &mut dyn ScriptExecutionDriver,
        interpreter: &ScriptInterpreter,
    ) -> Option<Result<ScriptActionResult, ScriptError>> {
        let object = interpreter.script_object_id();
        let calling = interpreter.calling_team();
        let condition = interpreter.condition_team();
        let result = match *self {
            Self::NamedAttackNamed(unit, target) => {
                world.named_command(ScriptNamedCommand::ForceAttack { unit, target }, object)
            }
            Self::NamedAttackArea(unit, area) => {
                world.named_attack_area(unit, area, interpreter.current_player().as_deref(), object)
            }
            Self::NamedAttackTeam(unit, team) => {
                world.named_attack_team(unit, team, object, calling, condition)
            }
            Self::TeamAttackNamed(team, target) => {
                world.team_attack_named(team, target, object, calling, condition)
            }
            Self::TeamAttackArea(team, area) => world.team_attack_area(
                team,
                area,
                interpreter.current_player().as_deref(),
                calling,
                condition,
            ),
            Self::TeamAttackTeam(source, target) => {
                world.team_attack_team(source, target, calling, condition)
            }
            Self::MoveTowardsNearest {
                recipient,
                object_type,
                trigger,
            } => {
                // Copy the ordered list before delivering the order, since a
                // receiving callback can re-enter the interpreter.
                let object_types: Vec<String> = interpreter
                    .frame()
                    .object_types
                    .get(object_type)
                    .map(|types| types.iter().map(|name| name.as_str().to_owned()).collect())
                    .unwrap_or_default();
                world.nearest_object_move(ScriptNearestObjectRequest {
                    recipient,
                    object_type,
                    object_types: &object_types,
                    trigger,
                    current_player: interpreter.current_player().as_deref(),
                    this_object: object,
                    calling_team: calling,
                    condition_team: condition,
                })
            }
        }?;
        Some(
            result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string())),
        )
    }
}
