//! CPP ScriptConditions1008/1589: module presence is independent of admission.
use super::*;
use generals_sim::scripting::program::{Condition, ConditionType, Parameter, ParameterType};
fn condition(kind: ConditionType, name: &str) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    condition
}
/// The named world plus this file's authored containers.
fn world() -> (GameLogic, ObjectId, ObjectId) {
    let container = |name: &str, module: &str, slots: usize| {
        crate::fixture_content::FixtureObject::new(name)
            .kind_of("STRUCTURE")
            .health(100.0)
            .module(format!("Behavior = {module} ModuleTag_Contain\n ContainMax = {slots}\nEnd"))
    };
    super::named_command_test_support::world_with(
        &[
            container("ObserveZero", "GarrisonContain", 0),
            container("ObserveTwo", "GarrisonContain", 2),
            container("ObserveTunnel", "TunnelContain", 0),
            container("ObserveCave", "CaveContain", 0),
        ],
        &[],
        "",
    )
}

fn container(owner: &mut GameLogic, name: &str) -> ObjectId {
    let id = owner.create_object_for_player(name, 1, Vec3::ZERO).unwrap();
    owner.host_object_mut(id).unwrap().name = name.into();
    id
}
#[test]
fn containment_conditions_use_actual_zero_dead_and_owned_local_rosters() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    let zero = container(&mut owner, "ObserveZero");
    let peer_zero = container(&mut peer, "ObserveZero");
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
    let filled = container(&mut owner, "ObserveTwo");
    let peer_filled = container(&mut peer, "ObserveTwo");
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
        .registered_values()
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
    let tunnel = container(&mut owner, "ObserveTunnel");
    let peer_tunnel = container(&mut peer, "ObserveTunnel");
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
    let cave = container(&mut owner, "ObserveCave");
    let peer_cave = container(&mut peer, "ObserveCave");
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

#[test]
fn destroy_all_contained_uses_the_shared_tunnel_roster() {
    destroy_through_other_entrance("ObserveTunnel");
}

#[test]
fn destroy_all_contained_uses_the_shared_cave_roster() {
    destroy_through_other_entrance("ObserveCave");
}

fn destroy_through_other_entrance(template: &str) {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::engine::ScriptEngine;
    use gamelogic::scripting::executor::ScriptContext;
    use generals_sim::scripting::program::{ScriptAction, ScriptActionType};

    let prepare = || {
        let (mut world, passenger, outsider) = world();
        let entrance = container(&mut world, template);
        let other = container(&mut world, template);
        world.host_object_mut(entrance).unwrap().name = "Entry".into();
        world.host_object_mut(other).unwrap().name = "OtherEntrance".into();
        assert!(world.add_receiving_named_script_object(other));
        if template == "ObserveTunnel" {
            assert!(world.tunnel_network.record_enter(1, passenger, entrance));
        } else {
            let index = world.cave_system.index_of(entrance).unwrap();
            assert_eq!(world.cave_system.index_of(other), Some(index));
            assert!(
                world
                    .cave_system
                    .record_enter(index, passenger, entrance, Team::USA)
                    .0
            );
        }
        world.set_fixture_object_contained_by(passenger, Some(entrance));
        (world, passenger, outsider, other)
    };
    let (mut owner, passenger, outsider, entrance) = prepare();
    let (peer, peer_passenger, _, peer_entrance) = prepare();
    assert_eq!((passenger, entrance), (peer_passenger, peer_entrance));
    assert!(
        owner
            .host_object(entrance)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(owner.owned_script_containment(entrance).unwrap().count, 1);

    let mut action = ScriptAction::new(ScriptActionType::UnitDestroyAllContained);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "OtherEntrance".into(),
        ))
        .unwrap();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    engine
        .friend_execute_action_with_driver_checked(
            &action,
            None,
            ScriptContext::at_frame(0),
            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
        )
        .unwrap();
    assert!(
        !owner.host_object(passenger).unwrap().is_alive(),
        "kill the receiving shared passenger through the other entrance"
    );
    assert!(owner.host_object(outsider).unwrap().is_alive());
    assert!(peer.host_object(peer_passenger).unwrap().is_alive());
    assert_eq!(
        peer.owned_script_containment(peer_entrance).unwrap().count,
        1
    );
}
