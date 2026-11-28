//! Bind the simulation's Team phases to the legacy team data.

use super::*;

impl Team {
    pub fn needs_enemy_sighting_observation(&self, step: &TeamStateStep) -> bool {
        self.script_state.needs_enemy_sighting_observation(step)
    }

    /// Step the original update and return an owned name for synchronous dispatch
    /// after the world driver releases its team and factory guards.
    pub fn advance_owned_state(
        &mut self,
        step: &mut TeamStateStep,
        observation: TeamStateObservation,
    ) -> Option<String> {
        let hooks = TeamScriptHooks {
            on_create: self.script_on_create.as_str(),
            on_enemy_sighted: self.script_on_enemy_sighted.as_str(),
            on_all_clear: self.script_on_all_clear.as_str(),
            on_destroyed: self.script_on_destroyed.as_str(),
            on_idle: self.script_on_idle.as_str(),
        };
        self.script_state
            .advance(step, observation, hooks)
            .map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_team() -> Team {
        let mut team = Team::new("OwnedPhaseTeam".into(), 9771);
        team.set_active();
        team
    }

    #[test]
    fn owned_enemy_observation_requires_exact_phase_and_receiving_flag() {
        let mut team = active_team();
        for enabled in [false, true] {
            team.script_state.check_enemy_sighted = enabled;
            for step in [
                TeamStateStep::Start,
                TeamStateStep::AfterCreate,
                TeamStateStep::Sighted,
                TeamStateStep::Destroyed,
                TeamStateStep::AfterDestroyed,
                TeamStateStep::Idle,
                TeamStateStep::AfterIdle(false),
                TeamStateStep::AfterIdle(true),
                TeamStateStep::Done,
            ] {
                assert_eq!(
                    team.needs_enemy_sighting_observation(&step),
                    enabled && step == TeamStateStep::Sighted
                );
            }
        }
        let peer = active_team();
        assert!(!peer.needs_enemy_sighting_observation(&TeamStateStep::Sighted));
        assert!(team.needs_enemy_sighting_observation(&TeamStateStep::Sighted));
    }

    #[test]
    fn owned_state_observes_roster_after_create_and_defers_one_shot_writes() {
        let mut team = active_team();
        team.script_on_create = "Create".into();
        team.script_on_destroyed = "Destroyed".into();
        team.script_state.destroyed_threshold_ratio = 0.3;
        let mut step = TeamStateStep::default();
        assert_eq!(
            team.advance_owned_state(&mut step, TeamStateObservation::default()),
            Some("Create".into())
        );
        assert!(!team.is_created());
        assert_eq!(step, TeamStateStep::AfterCreate);
        // An immediate OnCreate callback admits two actual members. A frozen
        // pre-callback census would incorrectly arm a zero threshold.
        team.add_member(10);
        team.add_member(11);
        team.advance_owned_state(
            &mut step,
            TeamStateObservation {
                member_count: 2,
                ..Default::default()
            },
        );
        assert_eq!(team.script_state.cur_units, 2);
        assert_eq!(
            team.script_state.destroy_threshold, 1,
            "CPP casts the entire float subtraction"
        );
        team.advance_owned_state(&mut step, TeamStateObservation::default());
        assert_eq!(step, TeamStateStep::Destroyed);
        let effect = team.advance_owned_state(
            &mut step,
            TeamStateObservation {
                live_count: 1,
                ..Default::default()
            },
        );
        assert_eq!(effect, Some("Destroyed".into()));
        assert_eq!(
            team.script_state.destroy_threshold, 1,
            "callback still observes the old threshold"
        );
        team.script_state.destroy_threshold = 900; // Callback write is followed by the original one-shot write.
        team.advance_owned_state(&mut step, TeamStateObservation::default());
        assert_eq!(team.script_state.destroy_threshold, -1);
        assert_eq!(step, TeamStateStep::Idle);
    }

    #[test]
    fn owned_idle_latch_commits_original_observation_after_callback() {
        let mut team = active_team();
        team.script_on_idle = "Idle".into();
        let observation = TeamStateObservation {
            all_ai_idle: true,
            any_live_ai: true,
            ..Default::default()
        };
        let mut step = TeamStateStep::Idle;
        team.script_state.was_idle = false;
        assert!(team.advance_owned_state(&mut step, observation).is_none());
        assert!(team.script_state.was_idle);
        step = TeamStateStep::Idle;
        assert_eq!(
            team.advance_owned_state(&mut step, observation),
            Some("Idle".into())
        );
        assert_eq!(step, TeamStateStep::AfterIdle(true));
        team.script_state.was_idle = false;
        team.advance_owned_state(&mut step, TeamStateObservation::default());
        assert!(team.script_state.was_idle);
        assert_eq!(step, TeamStateStep::Done);
    }

    #[test]
    fn owned_state_inactive_and_same_id_peer_remain_independent() {
        let mut first = active_team();
        let mut peer = Team::new("OwnedPhaseTeam".into(), 9771);
        first.script_on_create = "FirstCreate".into();
        peer.script_on_create = "PeerCreate".into();
        peer.script_state.entered_or_exited = true;
        let mut first_step = TeamStateStep::Start;
        let mut peer_step = TeamStateStep::Start;
        assert_eq!(
            first.advance_owned_state(&mut first_step, Default::default()),
            Some("FirstCreate".into())
        );
        assert!(
            peer.advance_owned_state(&mut peer_step, Default::default())
                .is_none()
        );
        assert_eq!(peer_step, TeamStateStep::Done);
        assert!(!peer.did_enter_or_exit());
        assert!(
            !peer.is_created(),
            "inactive construction must retain the original uncreated default"
        );
        peer.set_active();
        peer_step = TeamStateStep::Start;
        assert_eq!(
            peer.advance_owned_state(&mut peer_step, Default::default()),
            Some("PeerCreate".into())
        );
    }

    #[test]
    fn standalone_destruction_threshold_uses_the_original_float_subtraction() {
        use crate::scripting::HostScriptQuery;
        use crate::system::engine_stores::WorldServices;

        let world = Arc::new(WorldServices::new_for_world());
        let mut team = active_team();
        team.script_on_destroyed = "Destroyed".into();
        team.script_state.destroyed_threshold_ratio = 0.3;
        for member in 1..=4 {
            team.add_member(member);
        }
        team.update_state(&HostScriptQuery::of(&world));
        assert_eq!(team.script_state.cur_units, 4);
        assert_eq!(team.script_state.destroy_threshold, 2);
    }

    #[test]
    fn queued_team_update_uses_the_receiving_interpreter_during_a_foreign_scope() {
        use crate::GameLogicResult;
        use crate::scripting::HostScriptQuery;
        use crate::scripting::core::{
            Condition, ConditionType, OrCondition, Parameter, ParameterType, ScriptAction,
            ScriptActionType, ScriptList,
        };
        use crate::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
        use crate::scripting::executor::ScriptContext;
        use crate::system::engine_stores::{WorldServices, with_world_services};

        struct Driver;
        impl ScriptExecutionDriver for Driver {
            fn after_action(&mut self) -> GameLogicResult<()> {
                Ok(())
            }
        }

        fn interpreter(world: &Arc<WorldServices>, flag: &str) -> ScriptEngine {
            let mut condition = OrCondition::new();
            condition.first_and = Some(Box::new(Condition::new(ConditionType::ConditionTrue)));
            let mut action = ScriptAction::new(ScriptActionType::SetFlag);
            action
                .add_parameter(Parameter::with_string(ParameterType::Flag, flag.into()))
                .unwrap();
            action
                .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
                .unwrap();
            let mut script = Script::new();
            script.script_name = "TeamCreated".into();
            script.is_subroutine = true;
            script.condition = Some(Box::new(condition));
            script.action = Some(Box::new(action));
            let mut list = ScriptList::new();
            list.append_script(Box::new(script));
            let engine = ScriptEngine::new_with_services(world).unwrap();
            engine
                .set_script_list_for_player(0, Some(Box::new(list)))
                .unwrap();
            engine
        }

        let owner = Arc::new(WorldServices::new_for_world());
        let peer = Arc::new(WorldServices::new_for_world());
        let receiving = interpreter(&owner, "Receiving");
        *peer.scripts().engine().write().unwrap() = Some(interpreter(&peer, "Foreign"));
        let mut team = active_team();
        team.script_on_create = "TeamCreated".into();
        with_world_services(&peer, || {
            team.update_state(&HostScriptQuery::of(&owner));
            receiving
                .update_with_driver(ScriptContext::at_frame(42), &mut Driver)
                .unwrap();
        });
        assert!(
            receiving
                .get_flag("Receiving")
                .is_some_and(|flag| flag.value)
        );
        let peer_engine = peer.scripts().engine().read().unwrap();
        assert!(peer_engine.as_ref().unwrap().get_flag("Foreign").is_none());
    }
}
