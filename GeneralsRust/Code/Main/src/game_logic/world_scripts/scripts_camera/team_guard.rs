//! CPP ScriptActions1882: a team's AI members guard their own current positions.
use super::*;

impl GameLogic {
    fn resolve_owned_script_team(
        &self,
        raw_team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> gamelogic::GameLogicResult<Option<(u32, Vec<u32>)>> {
        let factory = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        let contextual = if raw_team == gamelogic::scripting::core::THIS_TEAM {
            calling_team.or(condition_team)
        } else {
            let mut selected = None;
            for id in [calling_team, condition_team].into_iter().flatten() {
                let Some(team) = factory.find_team_by_id(id) else {
                    continue;
                };
                let team = team
                    .read()
                    .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
                if team.get_name().as_str() == raw_team {
                    selected = Some(id);
                    break;
                }
            }
            selected
        };
        let team = if let Some(id) = contextual {
            factory.find_team_by_id(id)
        } else if raw_team == gamelogic::scripting::core::THIS_TEAM {
            None
        } else {
            let Some(prototype) = factory.find_team_prototype(raw_team) else {
                return Ok(None);
            };
            let Some(team) = factory.find_team_instances(raw_team).into_iter().next() else {
                return Ok(None);
            };
            if prototype.is_singleton()
                && !team
                    .read()
                    .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
                    .is_active()
            {
                return Ok(None);
            }
            Some(team)
        };
        let Some(team) = team else { return Ok(None) };
        let team = team
            .read()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        Ok(Some((team.get_id(), team.get_members().to_vec())))
    }

    pub(super) fn apply_owned_team_guard(
        &mut self,
        raw_team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        // CPP5933 contextual pointers precede prototype head selection.
        // Copy the exact roster and release its guards before member effects.
        let Some((_, members)) =
            self.resolve_owned_script_team(raw_team, calling_team, condition_team)?
        else {
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

    pub(super) fn apply_owned_team_attitude(
        &mut self,
        raw_team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
        mood: i32,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((_, members)) =
            self.resolve_owned_script_team(raw_team, calling_team, condition_team)?
        else {
            return Ok(());
        };
        let attitude = i8::try_from(mood).map_err(|_| {
            gamelogic::GameLogicError::Configuration(format!(
                "Script attitude {mood} exceeds the host attitude representation"
            ))
        })?;
        // CPP ScriptActions1280 -> AIGroup2968 only tests AI presence.
        // Dead/Sleep/immobile AI still receive the modifier, with no command
        // admission or player/faction census. AI_INVALID(3) remains representable.
        for id in members {
            let Some(unit) = self.host_object_mut(ObjectId(id)) else {
                continue;
            };
            if unit.has_ai_update_interface() {
                unit.ai_attitude = attitude;
                unit.record_host_ai_attitude();
            }
        }
        Ok(())
    }

    pub(super) fn apply_owned_team_sequential(
        &mut self,
        raw_team: &str,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
        script: Option<(gamelogic::scripting::core::Script, i32)>,
        engine: &gamelogic::scripting::engine::ScriptEngine,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((id, members)) =
            self.resolve_owned_script_team(raw_team, calling_team, condition_team)?
        else {
            return Ok(());
        };
        if let Some((script, loops)) = script {
            // CPP ScriptActions4746: resolve script, idle group, then append.
            for member in members {
                self.apply_owned_script_idle(ObjectId(member));
            }
            let mut sequence = gamelogic::scripting::engine::SequentialScript::new();
            sequence.team_to_exec_on = Some(id);
            sequence.script_to_execute_sequentially = Some(Box::new(script));
            sequence.times_to_loop = loops;
            engine.append_sequential_script(sequence);
        } else {
            engine.remove_all_sequential_scripts_for_team(id);
        }
        Ok(())
    }

    pub(crate) fn execute_script_team_actions(
        &mut self,
        engine: &mut gamelogic::scripting::engine::ScriptEngine,
        action: &gamelogic::scripting::core::ScriptAction,
        team_id: Option<u32>,
    ) {
        engine.friend_execute_action_with_driver(
            action,
            team_id,
            gamelogic::scripting::executor::ScriptContext::at_frame(self.frame),
            &mut super::script_execution_driver::HostScriptExecutionDriver::new(self),
        );
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
