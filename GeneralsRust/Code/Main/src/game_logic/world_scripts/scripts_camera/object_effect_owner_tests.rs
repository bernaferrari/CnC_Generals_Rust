//! Actual script dispatch mutates only its supplied Main object owner.
use super::super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn execute(world: &mut GameLogic, kind: ScriptActionType, name: &str, value: Option<i32>) {
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    let mut action = ScriptAction::new(kind);
    if kind != ScriptActionType::DeleteAllUnmanned {
        action
            .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
            .unwrap();
    }
    if let Some(value) = value {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
    }
    let context = gamelogic::scripting::executor::ScriptContext::at_frame(17);
    let mut driver = HostScriptExecutionDriver::with_interpreter(world, &engine);
    engine.friend_execute_action_with_driver(&action, None, context, &mut driver);
}

#[test]
fn supplied_script_object_effects_are_synchronous_and_same_id_isolated() {
    let (mut owner, id, _) = world();
    let (peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    let name = owner.host_object(id).unwrap().name.clone();
    execute(
        &mut owner,
        ScriptActionType::DisableObjectSound,
        &name,
        None,
    );
    assert!(
        !owner
            .host_object(id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    assert!(
        peer.host_object(id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    execute(&mut owner, ScriptActionType::EnableObjectSound, &name, None);
    assert!(
        owner
            .host_object(id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    execute(
        &mut owner,
        ScriptActionType::NamedSetStealthEnabled,
        &name,
        Some(0),
    );
    assert!(owner.host_object(id).unwrap().script_unstealthed);
    assert!(!peer.host_object(id).unwrap().script_unstealthed);
    execute(
        &mut owner,
        ScriptActionType::ObjectCreateRadarEvent,
        &name,
        Some(4),
    );
    assert!(owner.radar().get_last_event_loc().is_some());
    assert!(peer.radar().get_last_event_loc().is_none());
    execute(
        &mut owner,
        ScriptActionType::NamedSetUnmannedStatus,
        &name,
        None,
    );
    assert!(owner.host_object(id).unwrap().status.disabled_unmanned);
    assert!(!peer.host_object(id).unwrap().status.disabled_unmanned);
    assert_eq!(owner.host_object(id).unwrap().team, Team::Neutral);
    execute(&mut owner, ScriptActionType::DeleteAllUnmanned, "", None);
    assert!(
        owner.host_object(id).unwrap().status.destroyed
            || !owner.host_object(id).unwrap().is_alive()
    );
    assert!(peer.host_object(id).unwrap().is_alive());
}
