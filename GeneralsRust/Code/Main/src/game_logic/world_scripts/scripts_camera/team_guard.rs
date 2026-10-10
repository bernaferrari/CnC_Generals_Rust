//! CPP ScriptActions1882: a team's AI members guard their own current positions.
use super::*;

impl GameLogic {
    pub(super) fn apply_owned_team_guard(
        &mut self,
        raw_team: &str,
        calling_team: Option<&str>,
        condition_team: Option<&str>,
    ) -> gamelogic::GameLogicResult<()> {
        let name = if raw_team == gamelogic::scripting::core::THIS_TEAM {
            let Some(name) = calling_team.or(condition_team) else {
                return Ok(());
            };
            name
        } else {
            raw_team
        };
        let contextual = calling_team == Some(name) || condition_team == Some(name);
        // This is the driving world's existing roster, never the published
        // factory. Copy IDs and release its narrow compatibility locks before
        // any AI callback. No census/faction fallback or auto-created team.
        let members = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .script_team_member_ids(name, contextual)?;
        let Some(members) = members else {
            return Ok(());
        };
        for raw_id in members {
            let id = ObjectId(raw_id);
            let Some(position) = self.host_object(id).map(|unit| unit.get_position()) else {
                continue;
            };
            self.apply_owned_script_guard_position(id, position);
        }
        Ok(())
    }
}

#[cfg(test)]
impl GameLogic {
    /// Exercise the real borrowed script walk without publishing a request.
    pub(crate) fn execute_team_guard_script_for_test(&mut self, team: &str) {
        use gamelogic::scripting::core::{
            Parameter, ParameterType, ScriptAction, ScriptActionType,
        };
        let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
        action
            .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
            .unwrap();
        let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
        engine.friend_execute_action_with_driver(
            &action,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(self.frame),
            &mut super::script_execution_driver::HostScriptExecutionDriver::new(self),
        );
    }
}
