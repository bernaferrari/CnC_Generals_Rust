//! CPP ScriptEngine5933/6963/7051/7779/7807/8127: instance IDs,
//! scoped immediate callbacks, and retained continuation identity.
use super::*;
use crate::scripting::executor::ScriptContext;

fn guard() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    action
}

#[derive(Default)]
struct Owner {
    seen: Vec<(Option<TeamID>, Option<TeamID>)>,
    panic: bool,
}
impl ScriptExecutionDriver for Owner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn team_instances(&self, name: &str) -> ScriptOwnerQuery<Vec<TeamID>> {
        if name == "Copies" {
            ScriptOwnerQuery::Present(vec![90, 10])
        } else {
            ScriptOwnerQuery::Missing
        }
    }
    fn team_current_player(&self, id: TeamID) -> ScriptOwnerQuery<Option<String>> {
        ScriptOwnerQuery::Present(Some(format!("owner-{id}")))
    }
    fn team_guard(
        &mut self,
        _: &str,
        calling: Option<TeamID>,
        condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        self.seen.push((calling, condition));
        assert!(!self.panic, "controlled callback unwind");
        Some(Ok(()))
    }
}

#[test]
fn callback_scopes_exact_identity_and_controller_without_a_registry() {
    let mut engine = ScriptEngine::new().unwrap();
    let saved = engine.set_external_eval_context(Some("outer-player".into()), Some(10));
    let mut owner = Owner::default();
    engine.friend_execute_action_with_driver(
        &guard(),
        Some(90),
        ScriptContext::at_frame(1),
        &mut owner,
    );
    assert_eq!(owner.seen, vec![(Some(90), Some(10))]);
    assert_eq!(engine.get_calling_team_id(), None);
    assert_eq!(engine.get_condition_team_id(), Some(10));
    assert_eq!(
        engine.get_current_player_name().as_deref(),
        Some("outer-player")
    );
    engine.restore_external_eval_context(saved);
}

#[test]
fn callback_unwind_restores_outer_context() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(Some("outer-player".into()), Some(10));
    let mut owner = Owner {
        panic: true,
        ..Default::default()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.friend_execute_action_with_driver(
            &guard(),
            Some(90),
            ScriptContext::at_frame(1),
            &mut owner,
        );
    }));
    assert!(result.is_err());
    assert_eq!(engine.get_calling_team_id(), None);
    assert_eq!(engine.get_condition_team_id(), Some(10));
    assert_eq!(
        engine.get_current_player_name().as_deref(),
        Some("outer-player")
    );
}

fn sequence(team: TeamID, name: &str) -> SequentialScript {
    let mut sequence = SequentialScript::new();
    sequence.team_to_exec_on = Some(team);
    let mut script = Script::new();
    script.script_name = name.into();
    sequence.script_to_execute_sequentially = Some(Box::new(script));
    sequence
}

#[test]
fn snapshots_retain_ids_without_name_lookup_or_an_installed_world() {
    let engine = ScriptEngine::new().unwrap();
    engine.append_sequential_script(sequence(90, "Older"));
    engine.append_sequential_script(sequence(10, "Head"));
    let snapshot = engine.snapshot_sequential_scripts();
    assert_eq!(
        snapshot.iter().map(|s| s.team_id).collect::<Vec<_>>(),
        vec![90, 10]
    );
    let other = ScriptEngine::new().unwrap();
    other.restore_sequential_scripts(&snapshot);
    assert_eq!(other.snapshot_sequential_scripts(), snapshot);
}

#[test]
fn same_prototype_sequences_are_grouped_by_instance_and_destroy_only_exact_id() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.append_sequential_script(sequence(90, "first"));
    engine.append_sequential_script(sequence(10, "sibling"));
    engine.append_sequential_script(sequence(90, "continuation"));
    assert_eq!(engine.sequential_script_count(), 2);
    engine.set_external_eval_context(None, Some(10));
    engine.notify_of_team_destruction(90);
    assert!(!engine.has_active_sequential_script_for_team(90));
    assert!(engine.has_active_sequential_script_for_team(10));
    assert_eq!(engine.get_condition_team_id(), Some(10));
    assert_eq!(
        engine.snapshot_sequential_scripts()[0].script_name,
        "sibling"
    );
    engine.remove_all_sequential_scripts_for_team(10);
    assert_eq!(engine.get_condition_team_id(), None);
    assert_eq!(engine.sequential_script_count(), 0);
}

#[test]
fn sequential_timer_changes_only_the_selected_instance() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.append_sequential_script(sequence(90, "first"));
    engine.append_sequential_script(sequence(10, "sibling"));
    engine.set_sequential_timer_for_team(90, 13);
    let snapshot = engine.snapshot_sequential_scripts();
    assert_eq!(
        (snapshot[0].frames_to_wait, snapshot[1].frames_to_wait),
        (13, -1)
    );
}

#[test]
fn condition_loop_uses_owned_visit_order_even_after_oneshot_success() {
    let mut engine = ScriptEngine::new().unwrap();
    let mut script = Script::new();
    script.script_name = "each-copy".into();
    script.condition_team_name = "Copies".into();
    script.is_active = true;
    script.is_one_shot = true;
    let condition = Condition::new(ConditionType::ConditionTrue);
    let mut or = OrCondition::new();
    or.set_first_and_condition(Some(Box::new(condition)));
    script.condition = Some(Box::new(or));
    script.action = Some(Box::new(guard()));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine.set_external_eval_context(None, Some(777));
    let mut owner = Owner::default();
    engine
        .update_with_driver(ScriptContext::at_frame(1), &mut owner)
        .unwrap();
    assert_eq!(owner.seen, vec![(None, Some(90)), (None, Some(10))]);
    assert_eq!(engine.get_condition_team_id(), Some(777));
}

#[test]
fn nested_callback_restores_outer_identity_and_controller_before_continuing() {
    struct Nested<'a> {
        engine: &'a ScriptEngine,
        seen: bool,
    }
    impl ScriptExecutionDriver for Nested<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_current_player(&self, id: TeamID) -> ScriptOwnerQuery<Option<String>> {
            ScriptOwnerQuery::Present(Some(format!("owner-{id}")))
        }
        fn team_guard(
            &mut self,
            _: &str,
            calling: Option<TeamID>,
            condition: Option<TeamID>,
        ) -> Option<GameLogicResult<()>> {
            assert_eq!((calling, condition), (Some(90), Some(777)));
            assert_eq!(
                self.engine.get_current_player_name().as_deref(),
                Some("owner-90")
            );
            let mut nested = Owner::default();
            self.engine.friend_execute_action_with_driver(
                &guard(),
                Some(10),
                ScriptContext::at_frame(2),
                &mut nested,
            );
            assert_eq!(nested.seen, vec![(Some(10), Some(777))]);
            assert_eq!(self.engine.get_calling_team_id(), Some(90));
            assert_eq!(
                self.engine.get_current_player_name().as_deref(),
                Some("owner-90")
            );
            self.seen = true;
            Some(Ok(()))
        }
    }
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(Some("outer".into()), Some(777));
    let mut owner = Nested {
        engine: &engine,
        seen: false,
    };
    engine.friend_execute_action_with_driver(
        &guard(),
        Some(90),
        ScriptContext::at_frame(1),
        &mut owner,
    );
    assert!(owner.seen);
    assert_eq!(engine.get_calling_team_id(), None);
    assert_eq!(engine.get_condition_team_id(), Some(777));
    assert_eq!(engine.get_current_player_name().as_deref(), Some("outer"));
}
