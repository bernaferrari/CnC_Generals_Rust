//! ScriptActions.cpp default attack priority mutation through the installed script interpreter.
use super::*;
use crate::scripting::engine::{CanonicalScriptExecutionDriver, ScriptEngineHandle, get_script_engine};

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
    let engine = ScriptEngine::new_in_published_world().unwrap();
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

fn apply_priority(handle: &ScriptEngineHandle, name: &str, value: i32) {
    handle.read().unwrap().as_ref().unwrap().friend_execute_action_with_driver_checked(
        &default_action(name, value),
        None,
        ScriptContext::at_frame(0),
        &mut CanonicalScriptExecutionDriver,
    ).unwrap();
}

#[test]
fn injected_engine_priority_action_mutates_its_row() {
    let _serial = crate::test_sync::lock();
    let ambient = ScriptEngine::new_in_published_world().unwrap();
    ambient.set_priority_default("Shared", 19);
    let _restore = GlobalRestore::install(Some(ambient));
    let own = own_engine(4);
    apply_priority(&own, "Shared", 13);
    assert_eq!(
        priority(&own),
        13,
        "action must mutate the receiving interpreter"
    );
    assert_eq!(
        priority(&get_script_engine()),
        19,
        "ambient engine must keep its row"
    );
}

#[test]
fn injected_engine_with_no_global_engine_keeps_live_mutations() {
    let _serial = crate::test_sync::lock();
    let _restore = GlobalRestore::install(None);
    let own = own_engine(4);
    apply_priority(&own, "Shared", 8);
    apply_priority(&own, "Shared", 12);
    assert_eq!(priority(&own), 12);
    assert!(get_script_engine().read().unwrap().is_none());
}

#[test]
fn two_injected_engines_keep_same_named_rows_independent() {
    let _serial = crate::test_sync::lock();
    let _restore = GlobalRestore::install(Some(ScriptEngine::new_in_published_world().unwrap()));
    let a = own_engine(4);
    let b = own_engine(19);
    apply_priority(&a, "Shared", 8);
    apply_priority(&b, "Shared", 23);
    apply_priority(&a, "Shared", 12);
    assert_eq!(priority(&a), 12);
    assert_eq!(priority(&b), 23);
    assert_eq!(priority(&get_script_engine()), 1);
}

#[test]
fn injected_engine_preserves_default_and_empty_named_row_identity() {
    let _serial = crate::test_sync::lock();
    let _restore = GlobalRestore::install(Some(ScriptEngine::new_in_published_world().unwrap()));
    let own = own_engine(4);
    apply_priority(&own, "", 77);
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

/// C++ ScriptConditions skirmish command-button-ready reads the evaluating
/// engine's TheControlBar. Two worlds take turns on ONE thread: each evaluator
/// resolves buttons only from its own world, whichever world is published.
#[test]
fn evaluator_command_buttons_come_from_the_evaluating_engines_world() {
    let _serial = crate::test_sync::lock();
    use crate::system::engine_stores::{new_world_services, with_world_services};
    const NAME: &str = "Command_EvaluatorOwnedButton";
    let world_a = new_world_services();
    let world_b = new_world_services();
    world_a.common().ini.control_bar.write().new_command_button(NAME.into()).command = "STOP".into();
    for world in [&world_a, &world_b] {
        crate::control_bar::refresh_control_bar_bridge_for_world(world, None).unwrap();
    }
    let a = ScriptEngine::new_with_services(&world_a).unwrap();
    let b = ScriptEngine::new_with_services(&world_b).unwrap();
    let state = RefCell::new(ScriptContext::at_frame(0));
    let eval_a = ScriptConditionEvaluator::new(&a, &state);
    let eval_b = ScriptConditionEvaluator::new(&b, &state);
    let sees = |eval: &ScriptConditionEvaluator<'_>| eval.owning_control_bar_bridge()
        .is_some_and(|bar| bar.find_command_button_by_name(NAME).is_some());
    for published in [&world_b, &world_a, &world_b] {
        with_world_services(published, || {
            assert!(sees(&eval_a), "A finds its own button");
            assert!(!sees(&eval_b), "B never sees A's catalog");
        });
    }
}
