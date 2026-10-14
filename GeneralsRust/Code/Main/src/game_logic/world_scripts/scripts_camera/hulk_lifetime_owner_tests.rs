//! Real ScriptActions admission changes only the explicitly driving session.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn apply(world: &mut GameLogic, engine: &gamelogic::scripting::engine::ScriptEngine, seconds: f32) {
    let mut action = ScriptAction::new(ScriptActionType::ScriptingOverrideHulkLifetime);
    action.add_parameter(Parameter::with_real(ParameterType::Real, seconds));
    let mut driver = HostScriptExecutionDriver::with_interpreter(world, engine);
    engine.friend_execute_action_with_driver(
        &action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(23),
        &mut driver,
    );
}

#[test]
fn hulk_override_script_targets_one_owner_and_truncates_cpp_seconds() {
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    let mut owner = GameLogic::new();
    let mut peer = GameLogic::new();
    assert_eq!(owner.native_hulk_max_lifetime_override, -1);
    assert_eq!(peer.native_hulk_max_lifetime_override, -1);
    apply(&mut owner, &engine, 1.019);
    assert_eq!(owner.native_hulk_max_lifetime_override, 30);
    assert_eq!(peer.native_hulk_max_lifetime_override, -1);
    apply(&mut peer, &engine, 2.0);
    assert_eq!(peer.native_hulk_max_lifetime_override, 60);
    assert_eq!(owner.native_hulk_max_lifetime_override, 30);
    apply(&mut owner, &engine, 0.0);
    assert_eq!(owner.native_hulk_max_lifetime_override, 0);
    apply(&mut owner, &engine, -0.001);
    assert_eq!(owner.native_hulk_max_lifetime_override, -1);
    assert_eq!(peer.native_hulk_max_lifetime_override, 60);
    drop(peer);
    assert_eq!(owner.native_hulk_max_lifetime_override, -1);
    let fresh = GameLogic::new();
    assert_eq!(fresh.native_hulk_max_lifetime_override, -1);
}

#[test]
fn invalid_hulk_script_values_do_not_mutate_receiving_override() {
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    let mut owner = GameLogic::new();
    apply(&mut owner, &engine, 2.0);
    for seconds in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        apply(&mut owner, &engine, seconds);
        assert_eq!(owner.native_hulk_max_lifetime_override, 60);
    }
}
