//! CPP privateAttackTeam and AttackSquad retain the concrete team identity.
use super::named_command_test_support::{execute, world};
use super::*;
use gamelogic::scripting::core::ScriptActionType;

fn roster(owner: &GameLogic, name: &str, members: &[ObjectId]) -> u32 {
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team(name.into(), "".into(), false, None)
        .unwrap();
    let team = factory.create_team(name).unwrap();
    let mut team = team.write().unwrap();
    for member in members {
        team.add_member(member.0);
    }
    team.get_id()
}

#[test]
fn named_attack_team_retargets_only_the_resolved_owner_roster() {
    let (mut first, unit, target) = world();
    let (mut foreign, foreign_unit, foreign_target) = world();
    assert_eq!(unit, foreign_unit);
    let second = first
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(55.0, 0.0, 55.0))
        .unwrap();
    let team_id = roster(&first, "OwnedSquad", &[target]);
    roster(&foreign, "OwnedSquad", &[foreign_target]);
    assert_eq!(
        execute(
            &mut first,
            &[(ScriptActionType::NamedAttackTeam, "NamedUnit", "OwnedSquad")]
        ),
        1
    );
    assert_eq!(first.host_object(unit).unwrap().target, Some(target));
    assert_eq!(
        first
            .host_object(unit)
            .unwrap()
            .attack_priority_set
            .as_deref(),
        Some(format!("AIGroup.AttackTeam.owned:{team_id}").as_str())
    );
    // Even a nearer object of the same faction is never selected unless
    // the concrete team's current roster admits it.
    first.host_object_mut(target).unwrap().health.current = 0.0;
    first.tick_attack_team_persist(&[unit]);
    assert_ne!(first.host_object(unit).unwrap().target, Some(second));
    assert_eq!(first.host_object(unit).unwrap().attack_priority_set, None);
    assert_eq!(foreign.host_object(foreign_unit).unwrap().formation_id, 19);
}

#[test]
fn empty_resolved_team_clears_previous_attack_without_faction_fallback() {
    let (mut owner, unit, target) = world();
    roster(&owner, "EmptySquad", &[]);
    owner
        .host_object_mut(unit)
        .unwrap()
        .set_target(Some(target), &mut owner.attack_events);
    owner
        .host_object_mut(unit)
        .unwrap()
        .set_ai_state(AIState::Attacking);
    assert_eq!(
        execute(
            &mut owner,
            &[(ScriptActionType::NamedAttackTeam, "NamedUnit", "EmptySquad")]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().target, None);
    owner.tick_attack_team_persist(&[unit]);
    assert_eq!(owner.host_object(unit).unwrap().target, None);
    assert_eq!(owner.host_object(unit).unwrap().attack_priority_set, None);
}

#[test]
fn team_attack_named_runs_on_the_driving_roster_and_named_target() {
    let (mut owner, unit, target) = world();
    let (foreign, peer, _) = world();
    roster(&owner, "OwnedSource", &[unit]);
    assert_eq!(unit, peer);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamAttackNamed,
                "OwnedSource",
                "MissingVictim"
            )]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().target, None);
    let target_name = owner.host_object(target).unwrap().name.clone();
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamAttackNamed,
                "OwnedSource",
                target_name.as_str()
            )]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().target, Some(target));
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(foreign.host_object(peer).unwrap().target, None);
}

#[test]
fn named_attitude_changes_only_owned_ai_without_command_admission() {
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction};
    use gamelogic::scripting::engine::ScriptEngine;
    let (mut owner, unit, _) = world();
    let (foreign, peer, _) = world();
    let peer_mood = foreign.host_object(peer).unwrap().ai_attitude;
    let mut action = ScriptAction::new(ScriptActionType::NamedSetAttitude);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedUnit".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::AiMood, 2))
        .unwrap();
    let engine = ScriptEngine::new().unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut super::script_execution_driver::HostScriptExecutionDriver::new(&mut owner),
    );
    assert_eq!(owner.host_object(unit).unwrap().ai_attitude, 2);
    assert_eq!(foreign.host_object(peer).unwrap().ai_attitude, peer_mood);
}

#[test]
fn team_stop_disband_stops_then_recruits_and_merges_actual_owned_rosters() {
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction};
    use gamelogic::scripting::engine::ScriptEngine;
    let (mut owner, unit, target) = world();
    let (foreign, peer, _) = world();
    let source_id = roster(&owner, "StopDisbandSource", &[unit]);
    let default_name = owner.default_host_team_instance_name(Some(1), Team::USA);
    let default_id = roster(&owner, &default_name, &[]);
    {
        let factory = owner.team_factory.lock().unwrap();
        factory
            .find_team_by_id(source_id)
            .unwrap()
            .write()
            .unwrap()
            .set_controlling_player_id(Some(1));
        factory
            .find_team_by_id(default_id)
            .unwrap()
            .write()
            .unwrap()
            .set_controlling_player_id(Some(1));
    }
    let (object, attack_events) = owner.host_object_and_attack_events_mut(unit).unwrap();
    object.team_instance_name = "StopDisbandSource".into();
    object.is_recruitable = false;
    object.set_target(Some(target), attack_events);
    object.set_ai_state(AIState::Attacking);
    let peer_name = foreign
        .host_object(peer)
        .unwrap()
        .team_instance_name
        .clone();
    let mut action = ScriptAction::new(ScriptActionType::TeamStopAndDisband);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "StopDisbandSource".into(),
        ))
        .unwrap();
    ScriptEngine::new()
        .unwrap()
        .friend_execute_action_with_driver(
            &action,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(0),
            &mut super::script_execution_driver::HostScriptExecutionDriver::new(&mut owner),
        );
    let object = owner.host_object(unit).unwrap();
    assert_eq!(object.target, None);
    assert!(object.is_recruitable);
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(object.team_instance_name, default_name);
    assert_eq!(object.owner_player_id, Some(1));
    let factory = owner.team_factory.lock().unwrap();
    assert!(
        !factory
            .find_team_by_id(source_id)
            .unwrap()
            .read()
            .unwrap()
            .has_member(unit.0)
    );
    assert!(
        factory
            .find_team_by_id(default_id)
            .unwrap()
            .read()
            .unwrap()
            .has_member(unit.0)
    );
    assert_eq!(
        foreign.host_object(peer).unwrap().team_instance_name,
        peer_name
    );
}

#[test]
fn create_object_uses_exact_team_controller_before_factory_admission() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction};
    use gamelogic::scripting::engine::ScriptEngine;
    let (mut owner, _, _) = world();
    let (mut peer, _, _) = world();
    for world in [&mut owner, &mut peer] {
        world
            .team_factory
            .lock()
            .unwrap()
            .init_team_owned(
                "CreatedSquad".into(),
                "NamedVictim".into(),
                false,
                None,
                Some(2),
            )
            .unwrap();
    }
    let mut action = ScriptAction::new(ScriptActionType::CreateObject);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "OwnedNamedInfantry".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "CreatedSquad".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_coord(
            ParameterType::Coord3D,
            gamelogic::scripting::core::Coord3D::new(123.0, 234.0, 7.0),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_real(ParameterType::Angle, 1.5))
        .unwrap();
    let engine = ScriptEngine::new().unwrap();
    let peer_count = peer.objects.len();
    let before = owner.objects.len();
    let mut driver = HostScriptExecutionDriver::new(&mut owner);
    engine.friend_execute_action_with_driver(
        &action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut driver,
    );
    assert_eq!(owner.objects.len(), before + 1);
    let object = owner
        .objects
        .values()
        .find(|object| object.team_instance_name == "CreatedSquad")
        .unwrap();
    assert_eq!(object.owner_player_id, Some(2));
    assert_eq!(object.get_position(), Vec3::new(123.0, 7.0, 234.0));
    assert_eq!(object.get_orientation(), 1.5);
    let id = object.id;
    assert!(
        owner
            .team_factory
            .lock()
            .unwrap()
            .find_team_instances("CreatedSquad")[0]
            .read()
            .unwrap()
            .has_member(id.0)
    );
    assert_eq!(peer.objects.len(), peer_count);
    assert!(
        peer.team_factory
            .lock()
            .unwrap()
            .find_team_instances("CreatedSquad")
            .is_empty()
    );
}
