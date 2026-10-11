//! CPP ScriptConditions1008/1589: module presence is independent of admission.
use super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
fn condition(kind: ConditionType, name: &str) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    condition
}
fn container(owner: &mut GameLogic, name: &str, kind: ContainModuleKind, slots: usize) -> ObjectId {
    let mut template = ThingTemplate::new(name);
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    template.contain_module = ContainModuleMetadata {
        kind,
        slots: Some(slots),
        ..Default::default()
    };
    owner.templates.insert(name.into(), template);
    let id = owner.create_object_for_player(name, 1, Vec3::ZERO).unwrap();
    owner.host_object_mut(id).unwrap().name = name.into();
    id
}
#[test]
fn containment_conditions_use_actual_zero_dead_and_owned_local_rosters() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    let zero = container(&mut owner, "ObserveZero", ContainModuleKind::Garrison, 0);
    let peer_zero = container(&mut peer, "ObserveZero", ContainModuleKind::Garrison, 0);
    assert_eq!((unit, zero), (peer_unit, peer_zero));
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveZero"
    )));
    assert!(!owner.owned_script_condition_for_test(condition(
        ConditionType::NamedHasFreeContainerSlots,
        "ObserveZero"
    )));
    owner.host_object_mut(zero).unwrap().health.current = 0.0;
    owner.host_object_mut(zero).unwrap().status.destroyed = true;
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveZero"
    )));
    let filled = container(&mut owner, "ObserveTwo", ContainModuleKind::Garrison, 2);
    let peer_filled = container(&mut peer, "ObserveTwo", ContainModuleKind::Garrison, 2);
    assert_eq!(filled, peer_filled);
    assert!(owner.host_object_mut(filled).unwrap().add_occupant(unit));
    assert!(!owner.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveTwo"
    )));
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::NamedHasFreeContainerSlots,
        "ObserveTwo"
    )));
    assert!(peer.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveTwo"
    )));
    let target = owner
        .host_objects()
        .values()
        .find(|object| object.name == "NamedTarget")
        .unwrap()
        .id;
    assert!(owner.host_object_mut(filled).unwrap().add_occupant(target));
    assert!(!owner.owned_script_condition_for_test(condition(
        ConditionType::NamedHasFreeContainerSlots,
        "ObserveTwo"
    )));
    for name in ["NamedUnit", "AbsentContain"] {
        assert!(
            !owner.owned_script_condition_for_test(condition(
                ConditionType::NamedBuildingIsEmpty,
                name
            ))
        );
        assert!(!owner.owned_script_condition_for_test(condition(
            ConditionType::NamedHasFreeContainerSlots,
            name
        )));
    }
}
#[test]
fn containment_conditions_read_canonical_shared_tunnel_and_cave_pools() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    let tunnel = container(&mut owner, "ObserveTunnel", ContainModuleKind::Tunnel, 0);
    let peer_tunnel = container(&mut peer, "ObserveTunnel", ContainModuleKind::Tunnel, 0);
    assert_eq!((unit, tunnel), (peer_unit, peer_tunnel));
    assert!(owner.tunnel_network.record_enter(1, unit, tunnel));
    assert!(!owner.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveTunnel"
    )));
    assert!(peer.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveTunnel"
    )));
    assert!(owner.owned_script_condition_for_test(condition(
        ConditionType::NamedHasFreeContainerSlots,
        "ObserveTunnel"
    )));
    let cave = container(&mut owner, "ObserveCave", ContainModuleKind::Cave, 0);
    let peer_cave = container(&mut peer, "ObserveCave", ContainModuleKind::Cave, 0);
    assert_eq!(cave, peer_cave);
    let index = owner.cave_system.index_of(cave).unwrap();
    assert!(
        owner
            .cave_system
            .record_enter(index, unit, cave, Team::USA)
            .0
    );
    assert!(!owner.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveCave"
    )));
    assert!(peer.owned_script_condition_for_test(condition(
        ConditionType::NamedBuildingIsEmpty,
        "ObserveCave"
    )));
}
