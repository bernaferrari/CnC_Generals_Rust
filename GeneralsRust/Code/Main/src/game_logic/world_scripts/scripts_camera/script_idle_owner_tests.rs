//! CPP ScriptActions4519,4744 and7209: Idle alone and lookup-before-Idle.
use super::named_command_test_support::{execute, world};
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

fn attacking(owner: &mut GameLogic, unit: ObjectId, target: ObjectId) {
    let (object, attack_events) = owner.host_object_and_attack_events_mut(unit).unwrap();
    object.set_target(Some(target), attack_events);
    object.set_ai_state(AIState::Attacking);
    object.requested_destination = Some(Vec3::new(90.0, 0.0, 90.0));
}
fn roster(owner: &GameLogic, unit: ObjectId) -> u32 {
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team("OwnedIdleTeam".into(), "NamedOwner".into(), false, None)
        .unwrap();
    let team = factory.create_team("OwnedIdleTeam").unwrap();
    let mut team = team.write().unwrap();
    team.add_member(unit.0);
    team.get_id()
}
fn assert_idle(owner: &GameLogic, unit: ObjectId) {
    let object = owner.host_object(unit).unwrap();
    assert_eq!(object.ai_state, AIState::Idle);
    assert_eq!(object.target, None);
    assert_eq!(object.requested_destination, None);
    assert_eq!(object.formation_id, 19);
    assert_eq!(
        object.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_PANIC")
    );
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
}
fn run(engine: &ScriptEngine, owner: &mut GameLogic, action: &ScriptAction) {
    let frame = owner.frame;
    engine.friend_execute_action_with_driver(
        action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(frame),
        &mut HostScriptExecutionDriver::with_interpreter(owner, engine),
    );
}

#[test]
fn named_stop_preserves_group_and_locomotor_in_the_driving_owner() {
    let (mut owner, unit, target) = world();
    let (mut peer, peer_unit, peer_target) = world();
    assert_eq!(unit, peer_unit);
    attacking(&mut owner, unit, target);
    attacking(&mut peer, peer_unit, peer_target);
    assert_eq!(
        execute(
            &mut owner,
            &[(ScriptActionType::NamedStop, "MissingUnit", "")]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().target, Some(target));
    assert_eq!(
        execute(
            &mut owner,
            &[(ScriptActionType::NamedStop, "NamedUnit", "")]
        ),
        1
    );
    assert_idle(&owner, unit);
    assert_eq!(
        peer.host_object(peer_unit).unwrap().target,
        Some(peer_target)
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().ai_state,
        AIState::Attacking
    );
}

#[test]
fn timed_guard_uses_idle_without_guard_or_peer_mutation() {
    let (mut owner, unit, target) = world();
    let (mut peer, peer_unit, peer_target) = world();
    roster(&owner, unit);
    roster(&peer, peer_unit);
    attacking(&mut peer, peer_unit, peer_target);
    let engine = ScriptEngine::new().unwrap();
    for kind in [
        ScriptActionType::TeamGuardForFramecount,
        ScriptActionType::TeamIdleForFramecount,
    ] {
        attacking(&mut owner, unit, target);
        let mut action = ScriptAction::new(kind);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "OwnedIdleTeam".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, 7))
            .unwrap();
        run(&engine, &mut owner, &action);
        assert_idle(&owner, unit);
        assert_eq!(owner.host_object(unit).unwrap().guard_position, None);
        assert_eq!(
            peer.host_object(peer_unit).unwrap().target,
            Some(peer_target)
        );
    }
}

#[test]
fn missing_sequential_script_preserves_live_ai_then_valid_script_idles_before_append() {
    let (mut owner, unit, target) = world();
    let (mut peer, peer_unit, peer_target) = world();
    let team = roster(&owner, unit);
    let peer_team = roster(&peer, peer_unit);
    assert_eq!(team, peer_team);
    attacking(&mut owner, unit, target);
    attacking(&mut peer, peer_unit, peer_target);
    let engine = ScriptEngine::new().unwrap();
    let mut action = ScriptAction::new(ScriptActionType::TeamExecuteSequentialScript);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "OwnedIdleTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            "OwnedSequence".into(),
        ))
        .unwrap();
    run(&engine, &mut owner, &action);
    assert_eq!(owner.host_object(unit).unwrap().target, Some(target));
    assert_eq!(
        owner.host_object(unit).unwrap().ai_state,
        AIState::Attacking
    );
    assert!(!engine.has_active_sequential_script_for_team(team));
    let mut script = Script::new();
    script.script_name = "OwnedSequence".into();
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    run(&engine, &mut owner, &action);
    assert_idle(&owner, unit);
    assert!(engine.has_active_sequential_script_for_team(team));
    assert_eq!(
        peer.host_object(peer_unit).unwrap().target,
        Some(peer_target)
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().ai_state,
        AIState::Attacking
    );
}
