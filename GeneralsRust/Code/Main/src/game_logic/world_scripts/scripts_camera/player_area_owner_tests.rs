//! CPP420/513/2088: canonical players/objects with the owned polygon.
use super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
fn area(world: &mut GameLogic) {
    world
        .host_trigger_world
        .lock()
        .unwrap()
        .set_trigger_areas(&[gamelogic::polygon_trigger::PolygonTrigger::new(
            501,
            "OwnerTriangle".into(),
            vec![
                gamelogic::common::ICoord3D::new(0, 0, 0),
                gamelogic::common::ICoord3D::new(20, 0, 0),
                gamelogic::common::ICoord3D::new(0, 20, 0),
            ],
        )]);
}

fn run_team_set_state_action(world: &mut GameLogic, team: &str, state: &str) {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{
        OrCondition, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    use gamelogic::scripting::engine::ScriptEngine;

    let mut action = ScriptAction::new(ScriptActionType::TeamSetState);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TeamState,
            state.into(),
        ))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "SetOwnedTeamState".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
}

fn team_state_condition(kind: ConditionType, team: &str, state: &str) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TeamState,
            state.into(),
        ))
        .unwrap();
    condition
}

#[test]
fn team_inside_area_condition_reads_the_receiving_same_id_objects() {
    use super::condition_cursor_control_tests::admit;

    let (mut owner, id, _) = world();
    let (mut peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    area(&mut owner);
    area(&mut peer);
    admit(&owner, "AreaTeam", 81, Some(id));
    admit(&peer, "AreaTeam", 81, Some(peer_id));
    owner.set_fixture_object_position(id, Vec3::new(2.0, 0.0, 2.0));
    peer.set_fixture_object_position(peer_id, Vec3::new(18.0, 0.0, 18.0));
    // Production `evaluate_and_execute_scripts_with_effects` projects these
    // receiving object positions into each owner's trigger slots before the
    // early ScriptEngine condition walk.
    owner.inject_host_script_query_snapshot();
    peer.inject_host_script_query_snapshot();

    let condition = |kind, team: &str| {
        let mut condition = Condition::new(kind);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::TriggerArea,
                "OwnerTriangle".into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::SurfacesAllowed, 1))
            .unwrap();
        condition
    };

    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::TeamInsideAreaPartially,
        "AreaTeam"
    )));
    let member = owner.host_object(id).unwrap();
    assert!(member.has_ai_update_interface());
    let authored_surfaces = member.locomotor_surfaces;
    assert_ne!(authored_surfaces, 0);
    owner.host_object_mut(id).unwrap().locomotor_surfaces = 0;
    assert!(
        !owner.owned_script_condition_for_test(condition(
            ConditionType::TeamInsideAreaPartially,
            "AreaTeam"
        )),
        "an AI member's valid-surfaces mask of zero matches no requested surface"
    );
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::TeamOutsideAreaEntirely,
        "AreaTeam"
    )));
    owner.host_object_mut(id).unwrap().locomotor_surfaces = authored_surfaces;
    assert!(
        !peer.owned_script_condition_for_test(condition(
            ConditionType::TeamInsideAreaPartially,
            "AreaTeam"
        )),
        "same numeric object and team IDs in a peer cannot borrow the owner's position"
    );
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::TeamInsideAreaEntirely,
        "AreaTeam"
    )));
    assert!(!peer.owned_script_condition_for_test(condition(
        ConditionType::TeamInsideAreaEntirely,
        "AreaTeam"
    )));
    assert!(!owner.owned_script_condition_for_test(condition(
        ConditionType::TeamOutsideAreaEntirely,
        "AreaTeam"
    )));
    assert!(peer.owned_script_condition_for_test(condition(
        ConditionType::TeamOutsideAreaEntirely,
        "AreaTeam"
    )));
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::TeamOutsideAreaEntirely,
        "MissingTeam"
    )));
}

#[test]
fn team_set_state_action_and_conditions_share_the_receiving_team_owner() {
    use super::condition_cursor_control_tests::admit;

    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    admit(&owner, "ActionTeam", 82, Some(unit));
    admit(&peer, "ActionTeam", 82, Some(peer_unit));

    run_team_set_state_action(&mut owner, "ActionTeam", "OwnerState");
    assert!(owner.owned_script_condition_for_test(team_state_condition(
        ConditionType::TeamStateIs,
        "ActionTeam",
        "OwnerState"
    )));
    assert!(!peer.owned_script_condition_for_test(team_state_condition(
        ConditionType::TeamStateIs,
        "ActionTeam",
        "OwnerState"
    )));
    assert!(owner.owned_script_condition_for_test(team_state_condition(
        ConditionType::TeamStateIsNot,
        "ActionTeam",
        "PeerState"
    )));

    run_team_set_state_action(&mut peer, "ActionTeam", "PeerState");
    assert!(peer.owned_script_condition_for_test(team_state_condition(
        ConditionType::TeamStateIs,
        "ActionTeam",
        "PeerState"
    )));
    assert!(!owner.owned_script_condition_for_test(team_state_condition(
        ConditionType::TeamStateIs,
        "ActionTeam",
        "PeerState"
    )));
}

fn condition(
    kind: ConditionType,
    comparison: i32,
    count: i32,
    selected: &str,
    region: &str,
) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "NamedOwner".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, comparison))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, count))
        .unwrap();
    if kind == ConditionType::PlayerHasComparisonUnitKindInTriggerArea {
        let index = gamelogic::common::ALL_KIND_OF
            .iter()
            .position(|kind| *kind == gamelogic::common::KindOf::Infantry)
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(
                ParameterType::KindOfParam,
                index as i32,
            ))
            .unwrap();
    } else if kind != ConditionType::SkirmishValueInArea {
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                selected.into(),
            ))
            .unwrap();
    }
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            region.into(),
        ))
        .unwrap();
    condition
}
#[test]
fn actual_owned_area_counts_respect_polygon_owner_zero_and_same_id_peer() {
    let (mut owner, id, foreign) = world();
    let (mut peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    area(&mut owner);
    area(&mut peer);
    owner.set_fixture_object_position(id, Vec3::new(2.0, 0.0, 2.0));
    owner.set_fixture_object_position(foreign, Vec3::new(2.0, 0.0, 2.0));
    peer.set_fixture_object_position(peer_id, Vec3::new(18.0, 0.0, 18.0)); // AABB inside, polygon outside
    let kind = ConditionType::PlayerHasComparisonUnitTypeInTriggerArea;
    assert!(owner.owned_script_condition_for_test(condition(
        kind,
        2,
        1,
        "OwnedNamedInfantry",
        "OwnerTriangle"
    )));
    assert!(!owner.owned_script_condition_for_test(condition(
        kind,
        2,
        0,
        "OwnedNamedInfantry",
        "OwnerTriangle"
    )));
    assert!(peer.owned_script_condition_for_test(condition(
        kind,
        2,
        0,
        "OwnedNamedInfantry",
        "OwnerTriangle"
    )));
    assert!(!owner.owned_script_condition_for_test(condition(
        kind,
        2,
        0,
        "OwnedNamedInfantry",
        "MissingArea"
    )));
    assert!(owner.owned_script_condition_for_test(condition(
        kind,
        2,
        0,
        "OtherTemplate",
        "OwnerTriangle"
    )));
}
#[test]
fn actual_area_type_counts_dead_crates_but_kind_and_value_exclude_dead_inert() {
    let (mut owner, id, _) = world();
    area(&mut owner);
    let (object, fixture_partition_object) = owner.host_object_and_partition_mut(id).unwrap();
    object.set_position_with_partition_owner(
        Vec3::new(2.0, 0.0, 2.0),
        0,
        &mut Default::default(),
        fixture_partition_object,
    );
    object
        .template_mut()
        .add_kind_of(KindOf::Crate)
        .add_kind_of(KindOf::Inert);
    object.status.effectively_dead = true;
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::PlayerHasComparisonUnitTypeInTriggerArea,
        2,
        1,
        "OwnedNamedInfantry",
        "OwnerTriangle"
    )));
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::PlayerHasComparisonUnitKindInTriggerArea,
        2,
        0,
        "INFANTRY",
        "OwnerTriangle"
    )));
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::SkirmishValueInArea,
        2,
        0,
        "",
        "OwnerTriangle"
    )));
    owner.host_object_mut(id).unwrap().status.effectively_dead = false;
    owner
        .host_object_mut(id)
        .unwrap()
        .template_mut()
        .kind_of
        .remove(&KindOf::Inert);
    owner
        .host_object_mut(id)
        .unwrap()
        .template_mut()
        .build_cost
        .supplies = 731;
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::SkirmishValueInArea,
        2,
        731,
        "",
        "OwnerTriangle"
    )));
}

fn strings(kind: ConditionType, params: &[(ParameterType, &str)]) -> Condition {
    let mut condition = Condition::new(kind);
    for (kind, value) in params {
        condition
            .add_parameter(Parameter::with_string(*kind, (*value).into()))
            .unwrap();
    }
    condition
}

fn run_team_waypoint_trigger(world: &mut GameLogic, team: &str, path: &str) -> bool {
    run_condition_trigger(
        world,
        strings(
            ConditionType::TeamReachedWaypointsEnd,
            &[
                (ParameterType::Team, team),
                (ParameterType::WaypointPath, path),
            ],
        ),
    )
}

fn run_condition_trigger(world: &mut GameLogic, condition: Condition) -> bool {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{
        OrCondition, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    use gamelogic::scripting::engine::ScriptEngine;

    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut action = ScriptAction::new(ScriptActionType::SetCounter);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "WaypointObserved".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ObserveOwnedTeamWaypoint".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("WaypointObserved", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
    engine.get_counter("WaypointObserved").unwrap().value == 1
}

#[test]
fn discovery_and_waypoint_conditions_read_actual_owned_state_with_colliding_ids() {
    use super::condition_cursor_control_tests::admit;
    use gamelogic::common::ObjectShroudStatus;
    let (mut owner, unit, victim) = world();
    let (mut peer, peer_unit, peer_victim) = world();
    assert_eq!((unit, victim), (peer_unit, peer_victim));
    for logic in [&owner, &peer] {
        admit(logic, "FinishedTeam", 91, Some(unit));
    }
    owner
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(1, victim.0, ObjectShroudStatus::PartialClear);
    peer.world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(1, peer_victim.0, ObjectShroudStatus::Fogged);
    let discovered = || {
        strings(
            ConditionType::SkirmishPlayerHasDiscoveredPlayer,
            &[
                (ParameterType::Side, "NamedVictim"),
                (ParameterType::Side, "NamedOwner"),
            ],
        )
    };
    assert!(owner.owned_script_condition_for_test(discovered()));
    assert!(!peer.owned_script_condition_for_test(discovered()));
    owner
        .host_object_mut(unit)
        .unwrap()
        .pending_waypoint_labels
        .push("ActualPath".into());
    let finished = || {
        strings(
            ConditionType::TeamReachedWaypointsEnd,
            &[
                (ParameterType::Team, "FinishedTeam"),
                (ParameterType::WaypointPath, "ActualPath"),
            ],
        )
    };
    assert!(!owner.owned_script_condition_for_test(finished()));
    owner
        .host_object_mut(unit)
        .unwrap()
        .commit_completed_waypoint_labels();
    assert!(owner.owned_script_condition_for_test(finished()));
    assert!(!peer.owned_script_condition_for_test(finished()));
    owner
        .host_object_mut(unit)
        .unwrap()
        .template_mut()
        .set_authored_ai_update_interface(Some(false));
    assert!(!owner.owned_script_condition_for_test(finished()));
}

#[test]
fn authored_team_waypoint_trigger_uses_the_receiving_world_for_colliding_ids() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    for logic in [&owner, &peer] {
        super::condition_cursor_control_tests::admit(logic, "FinishedTeam", 91, Some(unit));
    }

    owner
        .host_object_mut(unit)
        .unwrap()
        .pending_waypoint_labels
        .push("OwnerOnlyPath".into());
    owner
        .host_object_mut(unit)
        .unwrap()
        .commit_completed_waypoint_labels();
    peer.host_object_mut(peer_unit)
        .unwrap()
        .pending_waypoint_labels
        .push("PeerOnlyPath".into());
    peer.host_object_mut(peer_unit)
        .unwrap()
        .commit_completed_waypoint_labels();

    assert!(run_team_waypoint_trigger(
        &mut owner,
        "FinishedTeam",
        "OwnerOnlyPath"
    ));
    assert!(!run_team_waypoint_trigger(
        &mut peer,
        "FinishedTeam",
        "OwnerOnlyPath"
    ));
    assert!(run_team_waypoint_trigger(
        &mut peer,
        "FinishedTeam",
        "PeerOnlyPath"
    ));
    assert!(!run_team_waypoint_trigger(
        &mut owner,
        "FinishedTeam",
        "PeerOnlyPath"
    ));
}

#[test]
fn authored_team_state_conditions_use_the_receiving_team_instance() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    for logic in [&owner, &peer] {
        super::condition_cursor_control_tests::admit(logic, "StatefulTeam", 92, Some(unit));
    }
    for (logic, state) in [(&owner, "OwnerState"), (&peer, "PeerState")] {
        let factory = logic.team_factory.lock().unwrap();
        factory
            .find_team_instances("StatefulTeam")
            .first()
            .unwrap()
            .write()
            .unwrap()
            .set_state(state.into());
    }

    let is_state = |state: &str| {
        strings(
            ConditionType::TeamStateIs,
            &[
                (ParameterType::Team, "StatefulTeam"),
                (ParameterType::TeamState, state),
            ],
        )
    };
    let is_not_state = |state: &str| {
        strings(
            ConditionType::TeamStateIsNot,
            &[
                (ParameterType::Team, "StatefulTeam"),
                (ParameterType::TeamState, state),
            ],
        )
    };
    assert!(run_condition_trigger(&mut owner, is_state("OwnerState")));
    assert!(!run_condition_trigger(&mut peer, is_state("OwnerState")));
    assert!(run_condition_trigger(&mut peer, is_state("PeerState")));
    assert!(!run_condition_trigger(&mut owner, is_state("PeerState")));
    assert!(run_condition_trigger(&mut owner, is_not_state("PeerState")));
    assert!(!run_condition_trigger(&mut peer, is_not_state("PeerState")));
    let missing_team = strings(
        ConditionType::TeamStateIsNot,
        &[
            (ParameterType::Team, "MissingTeam"),
            (ParameterType::TeamState, "OwnerState"),
        ],
    );
    assert!(!run_condition_trigger(&mut owner, missing_team));
}
#[test]
fn skirmish_counts_use_actual_owner_and_neutral_identity() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::engine::{
        ScriptExecutionDriver, ScriptOwnerQuery, ScriptSkirmishCensus,
    };
    let (mut owner, unit, victim) = world();
    let (mut peer, _, _) = world();
    owner.host_object_mut(unit).unwrap().status.private_captured = true;
    owner.host_object_mut(unit).unwrap().status.effectively_dead = true; // C++ has no dead filter.
    owner
        .host_object_mut(unit)
        .unwrap()
        .template_mut()
        .contain_module
        .kind = ContainModuleKind::Garrison;
    owner.host_object_mut(unit).unwrap().occupants.push(victim);
    owner.add_player(Player::new(3, Team::Neutral, "OwnedNeutral", false));
    peer.add_player(Player::new(3, Team::Neutral, "OwnedNeutral", false));
    let peer_neutral = peer
        .create_object("OwnedNamedInfantry", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let neutral = owner
        .create_object("OwnedNamedInfantry", Team::Neutral, Vec3::ZERO)
        .unwrap();
    // Neutral map objects are canonically admitted without an explicit player
    // ID; their Neutral team belongs to this session's neutral player.
    assert_eq!(owner.host_object(neutral).unwrap().owner_player_id, None);
    assert_eq!(neutral, peer_neutral);
    owner
        .host_object_mut(neutral)
        .unwrap()
        .status
        .disabled_unmanned = true;
    owner
        .host_object_mut(victim)
        .unwrap()
        .status
        .disabled_unmanned = true; // wrong owner is excluded.
    let driver = HostScriptExecutionDriver::new(&mut owner);
    assert_eq!(
        driver.skirmish_census("NamedOwner", None, ScriptSkirmishCensus::Captured),
        ScriptOwnerQuery::Present(1)
    );
    assert_eq!(
        driver.skirmish_census("NamedOwner", None, ScriptSkirmishCensus::Garrisoned),
        ScriptOwnerQuery::Present(1)
    );
    assert_eq!(
        driver.skirmish_census(
            "MissingScriptedSide",
            None,
            ScriptSkirmishCensus::NeutralUnmanned
        ),
        ScriptOwnerQuery::Present(1)
    );
    assert_eq!(
        HostScriptExecutionDriver::new(&mut peer).skirmish_census(
            "NamedOwner",
            None,
            ScriptSkirmishCensus::Captured
        ),
        ScriptOwnerQuery::Present(0)
    );
    assert_eq!(
        HostScriptExecutionDriver::new(&mut peer).skirmish_census(
            "MissingScriptedSide",
            None,
            ScriptSkirmishCensus::NeutralUnmanned
        ),
        ScriptOwnerQuery::Present(0)
    );
}
#[test]
fn special_power_readiness_uses_owned_module_and_timer() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use crate::command_system::SpecialPowerType;
    use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
    let (mut owner, unit, _) = world();
    let (mut peer, _, _) = world();
    let module = SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: None,
        module_kind: SpecialPowerModuleKind::OclSpecialPower,
        special_power_template: "OwnerSpy".into(),
        special_power_template_id: 901,
        command_power: Some(SpecialPowerType::SpySatellite),
        reload_time_frames: 1800,
        required_science: None,
        public_timer: true,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    };
    owner
        .templates
        .get_mut("OwnedNamedInfantry")
        .unwrap()
        .special_power_modules
        .push(module.clone());
    owner
        .host_object_mut(unit)
        .unwrap()
        .template_mut()
        .special_power_modules
        .push(module);
    owner
        .host_object_mut(unit)
        .unwrap()
        .set_special_power_ready_seconds(&SpecialPowerType::SpySatellite, 2.0);
    let ScriptOwnerQuery::Present(state) = HostScriptExecutionDriver::new(&mut owner)
        .skirmish_special_power_readiness("NamedOwner", "OwnerSpy", None)
    else {
        panic!("owned power")
    };
    assert!(state.template_exists);
    assert!(!state.ready);
    assert_eq!(state.next_frame, 60);
    owner
        .host_object_mut(unit)
        .unwrap()
        .set_special_power_ready_seconds(&SpecialPowerType::SpySatellite, 0.0);
    let power = || {
        strings(
            ConditionType::SkirmishSpecialPowerReady,
            &[
                (ParameterType::Side, "NamedOwner"),
                (ParameterType::SpecialPower, "OwnerSpy"),
            ],
        )
    };
    assert!(owner.owned_script_condition_for_test(power()));
    assert!(!peer.owned_script_condition_for_test(power()));
    owner
        .host_object_mut(unit)
        .unwrap()
        .status
        .under_construction = true;
    assert!(!owner.owned_script_condition_for_test(power()));
}
