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
