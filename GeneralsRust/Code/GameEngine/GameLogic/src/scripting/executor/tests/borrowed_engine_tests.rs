//! ScriptActions.cpp default attack priority mutation through actual ScriptEvaluator.
use super::*;
use crate::scripting::engine::{ScriptEngineHandle, get_script_engine};
use crate::scripting::evaluator::ScriptEvaluator;

struct GlobalRestore(Option<ScriptEngine>);
impl GlobalRestore {
    fn install(engine: Option<ScriptEngine>) -> Self {
        Self(std::mem::replace(
            &mut *get_script_engine().write().unwrap(),
            engine,
        ))
    }
}
impl Drop for GlobalRestore {
    fn drop(&mut self) {
        *get_script_engine().write().unwrap() = self.0.take();
    }
}

fn default_action(name: &str, value: i32) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::SetDefaultAttackPriority);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::AttackPrioritySet,
            name.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, value))
        .unwrap();
    action
}
fn own_engine(value: i32) -> ScriptEngineHandle {
    let engine = ScriptEngine::new().unwrap();
    engine.set_priority_default("Shared", value);
    ScriptEngineHandle::from_engine(engine)
}
fn priority(handle: &ScriptEngineHandle) -> i32 {
    handle
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .get_attack_info("Shared")
        .unwrap()
        .default_priority
}

#[test]
fn injected_evaluator_priority_action_mutates_its_engine() {
    let _serial = crate::test_sync::lock();
    let ambient = ScriptEngine::new().unwrap();
    ambient.set_priority_default("Shared", 19);
    let _restore = GlobalRestore::install(Some(ambient));
    let own = own_engine(4);
    ScriptEvaluator::new(own.clone())
        .execute_action(&default_action("Shared", 13))
        .unwrap();
    assert_eq!(
        priority(&own),
        13,
        "action must mutate the evaluator's injected engine"
    );
    assert_eq!(
        priority(&get_script_engine()),
        19,
        "ambient engine must keep its row"
    );
}

#[test]
fn injected_evaluator_with_no_global_engine_keeps_live_mutations() {
    let _serial = crate::test_sync::lock();
    let _restore = GlobalRestore::install(None);
    let own = own_engine(4);
    let evaluator = ScriptEvaluator::new(own.clone());
    evaluator
        .execute_action(&default_action("Shared", 8))
        .unwrap();
    evaluator
        .execute_action(&default_action("Shared", 12))
        .unwrap();
    assert_eq!(priority(&own), 12);
    assert!(get_script_engine().read().unwrap().is_none());
}

#[test]
fn two_injected_evaluators_keep_same_named_rows_independent() {
    let _serial = crate::test_sync::lock();
    let _restore = GlobalRestore::install(Some(ScriptEngine::new().unwrap()));
    let a = own_engine(4);
    let b = own_engine(19);
    let eval_a = ScriptEvaluator::new(a.clone());
    let eval_b = ScriptEvaluator::new(b.clone());
    eval_a.execute_action(&default_action("Shared", 8)).unwrap();
    eval_b
        .execute_action(&default_action("Shared", 23))
        .unwrap();
    eval_a
        .execute_action(&default_action("Shared", 12))
        .unwrap();
    assert_eq!(priority(&a), 12);
    assert_eq!(priority(&b), 23);
    assert_eq!(priority(&get_script_engine()), 1);
}

#[test]
fn injected_evaluator_preserves_default_and_empty_named_row_identity() {
    let _serial = crate::test_sync::lock();
    let _restore = GlobalRestore::install(Some(ScriptEngine::new().unwrap()));
    let own = own_engine(4);
    ScriptEvaluator::new(own.clone())
        .execute_action(&default_action("", 77))
        .unwrap();
    let guard = own.read().unwrap();
    let engine = guard.as_ref().unwrap();
    assert_eq!(engine.get_attack_info("").unwrap().default_priority, 77);
    assert_eq!(
        engine.get_attack_info("Missing").unwrap().default_priority,
        1
    );
    assert_eq!(
        engine
            .snapshot_xfer_tail()
            .attack_priorities
            .iter()
            .map(|row| (row.0.as_str(), row.1))
            .collect::<Vec<_>>(),
        vec![("", 1), ("Shared", 4), ("", 77)]
    );
}
