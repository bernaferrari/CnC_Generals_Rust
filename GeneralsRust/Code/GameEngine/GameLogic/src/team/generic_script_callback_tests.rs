//! Team.cpp:2506-2532: eligibility belongs to the Team, not the shared Script.
use super::*;
use crate::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, ScriptAction,
    ScriptActionType, ScriptList,
};
use crate::scripting::engine::{ScriptActionHandler, ScriptEngine, ScriptEngineHandle};
use std::sync::atomic::AtomicUsize;

struct ObserveEligibility {
    pending: PendingTeamGenericScriptEval,
    one_shot: bool,
    calls: Arc<AtomicUsize>,
}
impl ScriptActionHandler for ObserveEligibility {
    fn display_text(&self, text: &str) -> crate::GameLogicResult<()> {
        assert_eq!(text, "generic callback");
        assert_eq!(
            self.pending
                .team
                .read()
                .unwrap()
                .should_attempt_generic_script(0),
            !self.one_shot,
            "C++ consumes one-shot eligibility before its action"
        );
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.one_shot {
            // Revisit the same actual Team through the production collector
            // during its action. No handler -> interpreter ownership cycle.
            let mut nested = factory_for(&self.pending);
            nested.update();
            assert!(nested.drain_pending_generic_script_evals().is_empty());
        }
        Ok(())
    }
}

fn factory_for(pending: &PendingTeamGenericScriptEval) -> TeamFactory {
    let mut factory = TeamFactory::new();
    factory
        .prototypes
        .insert(pending.team_name.clone(), pending.prototype.clone());
    let id = pending.team.read().unwrap().get_id();
    factory.admit_team_instance(id, pending.team.clone());
    factory
}

fn run_callback_case(one_shot: bool, condition_true: bool) {
    let mut prototype = TeamPrototype::new("GenericCallbackTeam".into());
    prototype.set_generic_script(0, "GenericCallbackScript".into());
    let prototype = Arc::new(prototype);
    let team = Arc::new(RwLock::new(Team::new("GenericCallbackTeam".into(), 41)));
    let peer = Arc::new(RwLock::new(Team::new("GenericCallbackTeam".into(), 41)));
    let pending = PendingTeamGenericScriptEval {
        team: team.clone(),
        prototype: prototype.clone(),
        team_name: "GenericCallbackTeam".into(),
        script_name: "GenericCallbackScript".into(),
        script_index: 0,
        current_player_name: None,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let mut script = Script::new();
    script.set_name("GenericCallbackScript".into());
    script.set_one_shot(one_shot);
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(if condition_true {
        ConditionType::ConditionTrue
    } else {
        ConditionType::ConditionFalse
    }))));
    script.condition = Some(Box::new(condition));
    let mut action = ScriptAction::new(ScriptActionType::DisplayText);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            "generic callback".into(),
        ))
        .unwrap();
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine.set_action_handler(Some(Arc::new(ObserveEligibility {
        pending,
        one_shot,
        calls: calls.clone(),
    })));
    let engine = ScriptEngineHandle::from_engine(engine);
    let mut factory = TeamFactory::new();
    factory
        .prototypes
        .insert("GenericCallbackTeam".into(), prototype);
    factory.admit_team_instance(41, team.clone());
    // Two real collection visits before unlock model records already collected
    // before a preceding callback consumes the eligibility latch.
    factory.update();
    factory.update();
    let collected = factory.drain_pending_generic_script_evals();
    assert_eq!(collected.len(), 2);
    execute_pending_team_generic_script_evals_with_engine(collected, &engine);
    let first_calls = if !condition_true {
        0
    } else if one_shot {
        1
    } else {
        2
    };
    assert_eq!(calls.load(Ordering::Relaxed), first_calls);
    assert_eq!(
        team.read().unwrap().should_attempt_generic_script(0),
        !(one_shot && condition_true)
    );
    assert!(
        peer.read().unwrap().should_attempt_generic_script(0),
        "same-ID peer is untouched"
    );
    factory.update();
    execute_pending_team_generic_script_evals_with_engine(
        factory.drain_pending_generic_script_evals(),
        &engine,
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        first_calls + usize::from(condition_true && !one_shot)
    );
}

#[test]
fn generic_one_shot_is_consumed_before_callback_and_duplicate_revisit() {
    run_callback_case(true, true);
}
#[test]
fn generic_repeatable_callback_retains_team_eligibility() {
    run_callback_case(false, true);
}
#[test]
fn generic_false_condition_preserves_one_shot_eligibility() {
    run_callback_case(true, false);
}

fn local_failure_case(
    condition_type: Option<ConditionType>,
) -> (PendingTeamGenericScriptEval, ScriptEngineHandle) {
    let mut prototype = TeamPrototype::new("GenericLocalFailureTeam".into());
    prototype.set_generic_script(0, "GenericLocalFailureScript".into());
    let mut list = ScriptList::new();
    if let Some(condition_type) = condition_type {
        let mut script = Script::new();
        script.set_name("GenericLocalFailureScript".into());
        let mut condition = OrCondition::new();
        condition.set_first_and_condition(Some(Box::new(Condition::new(condition_type))));
        script.condition = Some(Box::new(condition));
        list.append_script(Box::new(script));
    }
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    let pending = PendingTeamGenericScriptEval {
        team: Arc::new(RwLock::new(Team::new("GenericLocalFailureTeam".into(), 73))),
        prototype: Arc::new(prototype),
        team_name: "GenericLocalFailureTeam".into(),
        script_name: "GenericLocalFailureScript".into(),
        script_index: 0,
        current_player_name: None,
    };
    (pending, ScriptEngineHandle::from_engine(engine))
}

#[test]
fn generic_local_missing_definition_disables_only_attempted_slot() {
    let (pending, engine) = local_failure_case(None);
    let peer = Team::new("GenericLocalFailureTeam".into(), 73);
    execute_pending_team_generic_script_evals_with_engine(
        vec![PendingTeamGenericScriptEval {
            team: pending.team.clone(),
            prototype: pending.prototype.clone(),
            team_name: pending.team_name.clone(),
            script_name: pending.script_name.clone(),
            script_index: pending.script_index,
            current_player_name: None,
        }],
        &engine,
    );
    let team = pending.team.read().unwrap();
    assert!(!team.should_attempt_generic_script(0));
    assert!(team.should_attempt_generic_script(1));
    assert!(peer.should_attempt_generic_script(0));
}

#[test]
fn generic_local_absent_interpreter_returns_cached_runtime_without_consuming_latch() {
    let (pending, engine) = local_failure_case(Some(ConditionType::ConditionFalse));
    let mut cached = pending
        .prototype
        .take_or_load_generic_script_runtime(0, &engine)
        .unwrap();
    cached.comment = "runtime-only marker".into();
    pending
        .prototype
        .store_generic_script_runtime(0, Some(cached));
    let saved_engine = engine.write().unwrap().take().unwrap();
    let mut factory = factory_for(&pending);
    factory.update();
    execute_pending_team_generic_script_evals_with_engine(
        factory.drain_pending_generic_script_evals(),
        &engine,
    );
    assert!(
        pending
            .team
            .read()
            .unwrap()
            .should_attempt_generic_script(0)
    );
    // This lookup cannot reload a definition: the exact local handle is None.
    let returned = pending
        .prototype
        .take_or_load_generic_script_runtime(0, &engine)
        .unwrap();
    assert_eq!(returned.comment, "runtime-only marker");
    pending
        .prototype
        .store_generic_script_runtime(0, Some(returned));
    *engine.write().unwrap() = Some(saved_engine);
    factory.update();
    execute_pending_team_generic_script_evals_with_engine(
        factory.drain_pending_generic_script_evals(),
        &engine,
    );
    assert!(
        pending
            .team
            .read()
            .unwrap()
            .should_attempt_generic_script(0)
    );
    assert_eq!(
        pending
            .prototype
            .take_or_load_generic_script_runtime(0, &engine)
            .unwrap()
            .comment,
        "runtime-only marker"
    );
}

#[test]
fn generic_local_condition_error_restores_context_and_returns_runtime() {
    // Counter without its required parameters is an actual evaluator Err,
    // not a false condition or an injected replacement evaluator.
    let (pending, engine) = local_failure_case(Some(ConditionType::Counter));
    let mut cached = pending
        .prototype
        .take_or_load_generic_script_runtime(0, &engine)
        .unwrap();
    cached.comment = "error runtime marker".into();
    let mut error_probe = cached.clone();
    assert!(
        evaluate_generic_script_conditions(
            &mut error_probe,
            &ScriptEvaluator::new(engine.clone()),
        )
        .is_err(),
        "malformed Counter must execute the evaluator error branch"
    );
    pending
        .prototype
        .store_generic_script_runtime(0, Some(cached));
    engine
        .write()
        .unwrap()
        .as_mut()
        .unwrap()
        .set_external_eval_context(Some("outer-player".into()), Some(29));
    let mut factory = factory_for(&pending);
    factory.update();
    execute_pending_team_generic_script_evals_with_engine(
        factory.drain_pending_generic_script_evals(),
        &engine,
    );
    assert!(
        pending
            .team
            .read()
            .unwrap()
            .should_attempt_generic_script(0)
    );
    let guard = engine.read().unwrap();
    let actual = guard.as_ref().unwrap();
    assert_eq!(
        actual.get_current_player_name().as_deref(),
        Some("outer-player")
    );
    assert_eq!(actual.get_condition_team_id(), Some(29));
    assert_eq!(actual.get_calling_team_id(), None);
    drop(guard);
    assert_eq!(
        pending
            .prototype
            .take_or_load_generic_script_runtime(0, &engine)
            .unwrap()
            .comment,
        "error runtime marker"
    );
}
