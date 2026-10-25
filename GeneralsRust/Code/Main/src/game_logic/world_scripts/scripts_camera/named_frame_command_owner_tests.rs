//! CPP4793/4814: frame commands retain group, with distinct locomotor preparation.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::{
    ScriptEngine, ScriptExecutionDriver, ScriptNamedCommand, ScriptOwnerQuery,
};

fn execute(world: &mut GameLogic, kind: ScriptActionType, name: &str) {
    let engine = ScriptEngine::new().unwrap();
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 9))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
        &mut HostScriptExecutionDriver::with_interpreter(world, &engine),
    );
}

#[test]
fn timed_named_guard_and_idle_keep_group_and_mutate_only_the_driving_ai() {
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    execute(
        &mut owner,
        ScriptActionType::UnitIdleForFramecount,
        "NamedUnit",
    );
    let object = owner.host_object(unit).unwrap();
    assert_eq!(object.ai_state(), AIState::Idle);
    assert_eq!(object.formation_id, 19);
    assert_eq!(
        object.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_PANIC")
    );
    execute(
        &mut owner,
        ScriptActionType::UnitGuardForFramecount,
        "NamedUnit",
    );
    let object = owner.host_object(unit).unwrap();
    assert_eq!(
        object.formation_id, 19,
        "timed guard preserves the original group"
    );
    assert_eq!(
        object.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_NORMAL")
    );
    assert_eq!(
        peer.host_object(peer_unit)
            .unwrap()
            .jet_ai
            .cur_locomotor_set
            .as_deref(),
        Some("SET_PANIC")
    );
    assert_eq!(peer.host_object(peer_unit).unwrap().formation_id, 19);
    let mut driver = HostScriptExecutionDriver::new(&mut owner);
    assert!(matches!(
        driver.named_frame_command(ScriptNamedCommand::Guard { unit: "Absent" }, None),
        ScriptOwnerQuery::Missing
    ));
}
