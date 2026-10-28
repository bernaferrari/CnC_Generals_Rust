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

fn add_non_ai_roster_member(world: &mut GameLogic, team_id: u32) -> ObjectId {
    let mut template = ThingTemplate::new("CaptureNonAI");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("CaptureNonAI", 1, Vec3::new(300.0, 0.0, 300.0))
        .unwrap();
    world.host_object_mut(id).unwrap().team_instance_name = "CaptureRoster".into();
    let mut factory = world.team_factory.lock().unwrap();
    factory
        .find_team_by_id(team_id)
        .unwrap()
        .write()
        .unwrap()
        .add_member(id.0);
    id
}

fn capture_nearest(
    world: &mut GameLogic,
    condition_team: u32,
    observed_member: ObjectId,
) -> (i32, Vec<Option<ObjectId>>) {
    let mut action = ScriptAction::new(ScriptActionType::TeamCaptureNearestUnownedFactionUnit);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            gamelogic::scripting::core::THIS_TEAM.into(),
        ))
        .unwrap();
    let mut after_capture = ScriptAction::new(ScriptActionType::IncrementCounter);
    after_capture
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    after_capture
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "AfterCapture".into(),
        ))
        .unwrap();
    action.next_action = Some(Box::new(after_capture));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("AfterCapture", 0).unwrap();
    engine.set_external_eval_context(None, Some(condition_team));
    let mut observations = Vec::new();
    {
        let mut observer = |owner: &GameLogic| {
            observations.push(
                owner
                    .host_object(observed_member)
                    .and_then(|object| object.target),
            );
        };
        engine.friend_execute_action_with_driver(
            &action,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world).with_after_action_observer(&mut observer),
        );
    }
    (
        engine.get_counter("AfterCapture").unwrap().value,
        observations,
    )
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
    for world in [&mut first, &mut second] {
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
        let mut ally =
            Player::new(3, Team::USA, "CaptureAlly", false).with_script_identity("CaptureAlly");
        ally.set_map_relationship(1, gamelogic::common::Relationship::Allies);
        world.add_player(ally);
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_map_relationship(3, gamelogic::common::Relationship::Allies);
    }
    let first_ally = first
        .create_object_for_player("OwnedNamedInfantry", 3, Vec3::new(51.0, 0.0, 50.0))
        .unwrap();
    let second_ally = second
        .create_object_for_player("OwnedNamedInfantry", 3, Vec3::new(51.0, 0.0, 50.0))
        .unwrap();
    assert_eq!(first_ally, second_ally);

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
        object.status.disabled_unmanned = true;
    }
    for (world, candidate) in [(&mut first, first_ally), (&mut second, second_ally)] {
        world
            .host_object_mut(candidate)
            .unwrap()
            .status
            .disabled_unmanned = true;
    }
    let first_team = install_roster(&mut first, "CaptureRoster", first_unit);
    let second_team = install_roster(&mut second, "CaptureRoster", second_unit);
    assert_eq!(first_team, second_team);
    let first_non_ai = add_non_ai_roster_member(&mut first, first_team);
    let second_non_ai = add_non_ai_roster_member(&mut second, second_team);
    assert_eq!(first_non_ai, second_non_ai);

    let (counter, observations) = capture_nearest(&mut first, first_team, first_unit);
    assert_eq!(counter, 1);
    assert_eq!(
        observations.first(),
        Some(&Some(first_candidate)),
        "the action callback immediately after capture must observe groupEnter's target"
    );

    assert_eq!(
        first.host_object(first_unit).unwrap().target,
        Some(first_candidate),
        "an allied unmanned unit is excluded; the nearest enemy is entered"
    );
    assert_eq!(
        first.host_object(first_non_ai).unwrap().target,
        None,
        "AIGroup::groupEnter skips members without an AIUpdateInterface"
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
    first
        .host_object_mut(first_candidate)
        .unwrap()
        .status
        .disabled_unmanned = false;
    first
        .host_object_mut(first_far)
        .unwrap()
        .status
        .disabled_unmanned = false;
    first
        .host_object_mut(first_ally)
        .unwrap()
        .status
        .disabled_unmanned = true;
    let (counter, observations) = capture_nearest(&mut first, first_team, first_unit);
    assert_eq!(counter, 1);
    assert_eq!(
        observations.first(),
        Some(&Some(first_candidate)),
        "the following action callback observes that the allied target was rejected"
    );
    assert_eq!(
        first.host_object(first_unit).unwrap().target,
        Some(first_candidate),
        "an allied unmanned Object is not eligible after enemy candidates are removed"
    );
    for id in [second_candidate, second_far, second_off_map, second_ally] {
        second.host_object_mut(id).unwrap().status.disabled_unmanned = false;
    }
    let (counter, observations) = capture_nearest(&mut second, second_team, second_unit);
    assert_eq!(counter, 1);
    assert_eq!(observations.first(), Some(&None));
    assert_eq!(
        second.host_object(second_unit).unwrap().target,
        None,
        "no eligible unmanned unit leaves the receiving roster unchanged"
    );
}
