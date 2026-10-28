//! CPP duplicate admission, not observation Clone or per-evaluation reset.
use super::*;
use crate::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, ScriptAction,
    ScriptActionType, ScriptList,
};
use crate::scripting::engine::{ScriptEngine, ScriptEngineHandle};

fn parameter(kind: ParameterType) -> Parameter {
    let mut parameter = Parameter::with_string(kind, "authored".into());
    parameter.initialized = true;
    parameter.int_value = 73;
    parameter.real_value = 2.5;
    parameter.coord_value.x = 4.0;
    parameter.coord_value.y = 5.0;
    parameter.coord_value.z = 6.0;
    parameter.object_status = crate::common::ObjectStatusMaskType::STEALTHED;
    parameter
}
fn condition() -> Box<Condition> {
    let mut condition = Condition::new(ConditionType::NamedSelected);
    condition
        .add_parameter(parameter(ParameterType::Unit))
        .unwrap();
    condition.has_warnings = true;
    condition.custom_data = -1;
    condition.custom_frame = 91;
    Box::new(condition)
}
fn action() -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::DisplayText);
    action
        .add_parameter(parameter(ParameterType::TextString))
        .unwrap();
    action.has_warnings = true;
    Box::new(action)
}
fn declaration() -> Script {
    let mut script = Script::new();
    script.script_name = "Cached".into();
    script.comment = "comment".into();
    script.condition_comment = "condition".into();
    script.action_comment = "action".into();
    script.delay_evaluation_seconds = 17;
    script.is_active = false;
    script.is_one_shot = false;
    script.is_subroutine = true;
    script.easy = false;
    script.normal = false;
    script.hard = true;
    script.frame_to_evaluate_at = 19;
    script.has_warnings = true;
    script.condition_team_name = "OldTeam".into();
    script.condition_time = 3.0;
    script.cur_time = 4.0;
    script.condition_executed_count = 5;
    let mut first_and = condition();
    first_and.next_and_condition = Some(condition());
    script.condition = Some(Box::new(OrCondition {
        first_and: Some(first_and),
        next_or: Some(Box::new(OrCondition {
            first_and: Some(condition()),
            next_or: None,
        })),
    }));
    let mut first_action = action();
    first_action.next_action = Some(action());
    script.action = Some(first_action);
    let mut false_action = action();
    false_action.next_action = Some(action());
    script.action_false = Some(false_action);
    script
}
fn admitted(script: Script) -> ScriptEngineHandle {
    let mut suffix = Script::new();
    suffix.script_name = "Suffix".into();
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    list.append_script(Box::new(suffix));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    ScriptEngineHandle::from_engine(engine)
}
fn prototype() -> TeamPrototype {
    let mut prototype = TeamPrototype::new("Receiving".into());
    prototype.set_generic_script(0, "Cached".into());
    prototype
}
fn assert_parameter(parameter: &Parameter, kind: ParameterType) {
    assert!(parameter.initialized);
    assert_eq!(parameter.param_type, kind);
    assert_eq!(parameter.int_value, 73);
    assert_eq!(parameter.real_value, 2.5);
    assert_eq!(parameter.string_value, "authored");
    assert_eq!(parameter.coord_value.x, 4.0);
    assert_eq!(parameter.coord_value.y, 5.0);
    assert_eq!(parameter.coord_value.z, 6.0);
    assert_eq!(
        parameter.object_status,
        crate::common::ObjectStatusMaskType::STEALTHED
    );
}
#[test]
fn generic_cache_admission_duplicates_one_definition_and_resets_only_cpp_transients() {
    let engine = admitted(declaration());
    // List admission initializes deadlines. Compare against that actual admitted
    // root rather than assuming the declaration's pre-admission frame survives.
    let admitted_root = engine
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .find_script_clone_by_name("Cached")
        .unwrap();
    let prototype = prototype();
    let cached = prototype
        .take_or_load_generic_script_runtime(0, &engine)
        .unwrap();
    assert_eq!(cached.script_name, "Cached");
    assert_eq!(cached.comment, "comment");
    assert_eq!(cached.condition_comment, "condition");
    assert_eq!(cached.action_comment, "action");
    assert_eq!(cached.delay_evaluation_seconds, 17);
    assert!(!cached.is_active && !cached.is_one_shot && cached.is_subroutine);
    assert!(!cached.easy && !cached.normal && cached.hard);
    assert!(
        cached.next_script.is_none(),
        "CPP duplicate excludes the source list suffix"
    );
    assert_eq!(cached.frame_to_evaluate_at, 0);
    assert!(!cached.has_warnings);
    assert!(cached.condition_team_name.is_empty());
    assert_eq!(cached.condition_time, 0.0);
    assert_eq!(cached.cur_time, 0.0);
    assert_eq!(cached.condition_executed_count, 0);
    let mut or_node = cached.condition.as_deref();
    let mut or_count = 0;
    let mut and_count = 0;
    while let Some(node) = or_node {
        or_count += 1;
        let mut and_node = node.first_and.as_deref();
        while let Some(node) = and_node {
            and_count += 1;
            assert_eq!(node.condition_type, ConditionType::NamedSelected);
            assert_eq!(node.num_parms, 1);
            assert!(!node.has_warnings);
            assert_eq!(node.custom_data, 0);
            assert_eq!(node.custom_frame, 0);
            assert_parameter(node.get_parameter(0).unwrap(), ParameterType::Unit);
            and_node = node.next_and_condition.as_deref();
        }
        or_node = node.next_or.as_deref();
    }
    assert_eq!((or_count, and_count), (2, 3));
    for head in [&cached.action, &cached.action_false] {
        let mut node = head.as_deref();
        let mut count = 0;
        while let Some(action) = node {
            count += 1;
            assert_eq!(action.action_type, ScriptActionType::DisplayText);
            assert_eq!(action.num_parms, 1);
            assert!(!action.has_warnings);
            assert_parameter(action.get_parameter(0).unwrap(), ParameterType::TextString);
            node = action.next_action.as_deref();
        }
        assert_eq!(count, 2);
    }
    // Ordinary root observation clones retain their runtime/list, and admission
    // must not mutate the actual root definition while making the duplicate.
    let root = engine
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .find_script_clone_by_name("Cached")
        .unwrap();
    assert_eq!(root.next_script.as_deref().unwrap().script_name, "Suffix");
    assert!(root.has_warnings);
    assert_eq!(
        root.frame_to_evaluate_at,
        admitted_root.frame_to_evaluate_at
    );
    assert_eq!(
        root.condition
            .as_deref()
            .unwrap()
            .first_and
            .as_deref()
            .unwrap()
            .custom_data,
        -1
    );
    assert!(root.action.as_deref().unwrap().has_warnings);
}
#[test]
fn generic_cached_runtime_mutations_survive_retrieval_and_do_not_cross_receiving_engines() {
    let own = admitted(declaration());
    let mut peer_root = declaration();
    peer_root.comment = "peer".into();
    let peer = admitted(peer_root);
    let own_prototype = prototype();
    let peer_prototype = prototype();
    let mut runtime = own_prototype
        .take_or_load_generic_script_runtime(0, &own)
        .unwrap();
    runtime.condition_executed_count = 12;
    let condition = runtime
        .condition
        .as_deref_mut()
        .unwrap()
        .first_and
        .as_deref_mut()
        .unwrap();
    condition.custom_data = 1;
    condition.custom_frame = 42;
    condition.parameters[0].as_mut().unwrap().int_value = 101;
    drop(runtime);
    let returned = own_prototype
        .take_or_load_generic_script_runtime(0, &own)
        .unwrap();
    assert_eq!(returned.condition_executed_count, 12);
    let condition = returned
        .condition
        .as_deref()
        .unwrap()
        .first_and
        .as_deref()
        .unwrap();
    assert_eq!((condition.custom_data, condition.custom_frame), (1, 42));
    assert_eq!(condition.get_parameter(0).unwrap().int_value, 101);
    drop(returned);
    let peer_runtime = peer_prototype
        .take_or_load_generic_script_runtime(0, &peer)
        .unwrap();
    assert_eq!(peer_runtime.comment, "peer");
    assert_eq!(peer_runtime.condition_executed_count, 0);
    let condition = peer_runtime
        .condition
        .as_deref()
        .unwrap()
        .first_and
        .as_deref()
        .unwrap();
    assert_eq!((condition.custom_data, condition.custom_frame), (0, 0));
    assert_eq!(condition.get_parameter(0).unwrap().int_value, 73);
    drop(peer_runtime);
}

#[test]
fn taking_a_generic_runtime_marks_it_in_flight_for_reentrant_evaluation() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let mut prototype = TeamPrototype::new("InFlightGeneric".into());
    prototype.set_generic_script(0, "InFlightScript".into());
    let mut script = Script::new();
    script.set_name("InFlightScript".into());
    script.comment = "original-runtime".into();
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    let engine = ScriptEngineHandle::from_engine(engine);

    let mut outer = prototype
        .take_or_load_generic_script_runtime_checked(0, &engine)
        .unwrap()
        .unwrap();
    outer.comment = "mutated-runtime".into();
    assert!(matches!(
        prototype.take_or_load_generic_script_runtime_checked(0, &engine),
        Err(GenericScriptRuntimeBusy::InFlight)
    ));
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let mut outer = outer;
        outer.comment = "panic-restored-runtime".into();
        panic!("exercise generic runtime lease unwind restoration");
    }))
    .is_err());

    let returned = prototype
        .take_or_load_generic_script_runtime_checked(0, &engine)
        .unwrap()
        .unwrap();
    assert_eq!(
        returned.comment, "panic-restored-runtime",
        "nested evaluation cannot duplicate runtime state, and unwind restores it"
    );
    drop(returned);
}
