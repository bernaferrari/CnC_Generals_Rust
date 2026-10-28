//! The capture-nearest action must command the receiving world's roster in
//! the same action phase; no later TLS/team-tick drain is involved.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::ScriptEngine;

fn install_roster(world: &mut GameLogic, name: &str, member: ObjectId) -> u32 {
    let team = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(name.into(), "NamedOwner".into(), false, None, Some(1))
            .unwrap();
        factory.create_team(name).unwrap()
    };
    world.host_object_mut(member).unwrap().team_instance_name = name.into();
    team.write().unwrap().add_member(member.0);
    team.read().unwrap().get_id()
}

fn capture_nearest(world: &mut GameLogic, condition_team: u32) {
    let mut action = ScriptAction::new(ScriptActionType::TeamCaptureNearestUnownedFactionUnit);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            gamelogic::scripting::core::THIS_TEAM.into(),
        ))
        .unwrap();
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(None, Some(condition_team));
    engine.friend_execute_action_with_driver(
        &action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
        &mut HostScriptExecutionDriver::new(world),
    );
}

#[test]
fn capture_nearest_is_synchronous_and_world_owned_with_same_ids() {
    let (mut first, first_unit, first_candidate) = world();
    let (mut second, second_unit, second_candidate) = world();
    assert_eq!(first_unit, second_unit);
    assert_eq!(first_candidate, second_candidate);
    let first_far = first
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(250.0, 0.0, 250.0))
        .unwrap();
    let second_far = second
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(250.0, 0.0, 250.0))
        .unwrap();
    assert_eq!(first_far, second_far);
    let first_off_map = first
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(-40.0, 0.0, 50.0))
        .unwrap();
    let second_off_map = second
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(-40.0, 0.0, 50.0))
        .unwrap();
    assert_eq!(first_off_map, second_off_map);

    for (world, candidate) in [
        (&mut first, first_candidate),
        (&mut second, second_candidate),
    ] {
        let object = world.host_object_mut(candidate).unwrap();
        object.team = Team::Neutral;
        object.owner_player_id = None;
        object.status.disabled_unmanned = true;
    }
    for (world, candidate) in [(&mut first, first_far), (&mut second, second_far)] {
        let object = world.host_object_mut(candidate).unwrap();
        object.team = Team::Neutral;
        object.owner_player_id = None;
        object.status.disabled_unmanned = true;
    }
    for (world, candidate) in [(&mut first, first_off_map), (&mut second, second_off_map)] {
        let object = world.host_object_mut(candidate).unwrap();
        object.team = Team::Neutral;
        object.owner_player_id = None;
        object.status.disabled_unmanned = true;
    }
    let first_team = install_roster(&mut first, "CaptureRoster", first_unit);
    let second_team = install_roster(&mut second, "CaptureRoster", second_unit);
    assert_eq!(first_team, second_team);

    capture_nearest(&mut first, first_team);

    assert_eq!(
        first.host_object(first_unit).unwrap().target,
        Some(first_candidate),
        "the driver must synchronously groupEnter the nearest eligible target"
    );
    assert_eq!(
        first.host_object(first_unit).unwrap().ai_state(),
        crate::game_logic::AIState::Entering
    );
    assert_eq!(
        second.host_object(second_unit).unwrap().target,
        None,
        "a same-ID peer in another world must not receive the action"
    );
    for id in [second_candidate, second_far, second_off_map] {
        second.host_object_mut(id).unwrap().status.disabled_unmanned = false;
    }
    capture_nearest(&mut second, second_team);
    assert_eq!(
        second.host_object(second_unit).unwrap().target,
        None,
        "no eligible unmanned unit leaves the receiving roster unchanged"
    );
}
