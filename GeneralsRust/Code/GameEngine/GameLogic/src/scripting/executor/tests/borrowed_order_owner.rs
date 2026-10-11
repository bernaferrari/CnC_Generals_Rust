//! Typed dispatch fixtures. Actual Main command effects have separate owner tests.
use super::*;
use crate::scripting::engine::{
    ScriptNamedCommand, ScriptNearestObjectRequest, ScriptNearestRecipient, ScriptOwnerQuery,
};
#[derive(Default)]
pub(super) struct OrderOwner {
    pub named: Vec<(String, String)>,
    pub idle: Vec<String>,
    pub attitudes: Vec<(String, i32)>,
    pub nearest: Vec<HostScriptMoveAttackRequest>,
    pub missing: bool,
}
impl ScriptExecutionDriver for OrderOwner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn named_frame_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        _: Option<u32>,
    ) -> ScriptOwnerQuery<()> {
        if self.missing {
            return ScriptOwnerQuery::Missing;
        }
        let row = match request {
            ScriptNamedCommand::Stop { unit } => ("Stop", unit),
            ScriptNamedCommand::Guard { unit } => ("Guard", unit),
            _ => panic!("unexpected frame command"),
        };
        self.named.push((row.0.into(), row.1.into()));
        ScriptOwnerQuery::Present(())
    }
    fn team_idle(
        &mut self,
        team: &str,
        _: Option<u32>,
        _: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        self.idle.push(team.into());
        Some(Ok(()))
    }
    fn team_attitude(
        &mut self,
        team: &str,
        _: Option<u32>,
        _: Option<u32>,
        mood: i32,
    ) -> Option<GameLogicResult<()>> {
        self.attitudes.push((team.into(), mood));
        Some(Ok(()))
    }
    fn nearest_object_move(
        &mut self,
        request: ScriptNearestObjectRequest<'_>,
    ) -> Option<GameLogicResult<()>> {
        self.nearest.push(match request.recipient {
            ScriptNearestRecipient::Named(unit) => {
                HostScriptMoveAttackRequest::NamedMoveTowardsNearest {
                    unit: unit.into(),
                    object_type: request.object_type.into(),
                    trigger: request.trigger.into(),
                }
            }
            ScriptNearestRecipient::Team(team) => {
                HostScriptMoveAttackRequest::TeamMoveTowardsNearest {
                    team: team.into(),
                    object_type: request.object_type.into(),
                    trigger: request.trigger.into(),
                }
            }
        });
        Some(Ok(()))
    }
}
