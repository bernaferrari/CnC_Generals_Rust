//! CPP ScriptConditions1614–1631: real driving-object entry edge/window.
use super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
fn condition(name: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::NamedEnteredArea);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "OwnedEntryTriangle".into(),
        ))
        .unwrap();
    condition
}
#[test]
fn named_entered_condition_observes_owned_geometry_edge_window_and_same_id_peer() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    let trigger = gamelogic::polygon_trigger::PolygonTrigger::new(
        1814,
        "OwnedEntryTriangle".into(),
        vec![
            gamelogic::common::ICoord3D::new(0, 0, 0),
            gamelogic::common::ICoord3D::new(20, 0, 0),
            gamelogic::common::ICoord3D::new(0, 20, 0),
        ],
    );
    for world in [&mut owner, &mut peer] {
        world.frame = 20;
        let mut flags = world.host_trigger_world.lock().unwrap();
        flags.set_trigger_areas(&[trigger.clone()]);
        flags.set_current_frame(20);
        flags.update_object_flags(unit.0, 18.0, 18.0, 19, false, None);
    }
    owner.set_fixture_object_position(unit, Vec3::new(2.0, 0.0, 2.0));
    owner
        .host_trigger_world
        .lock()
        .unwrap()
        .update_object_flags(unit.0, 2.0, 2.0, 20, false, None);
    assert!(owner.owned_script_condition_for_test(condition("NamedUnit")));
    assert!(!peer.owned_script_condition_for_test(condition("NamedUnit")));
    assert!(!owner.owned_script_condition_for_test(condition("AbsentUnit")));
    owner.frame = 21;
    assert!(owner.owned_script_condition_for_test(condition("NamedUnit")));
    owner.frame = 22;
    assert!(!owner.owned_script_condition_for_test(condition("NamedUnit")));
    // Inert objects never produce a gameplay entry condition even if geometry
    // flags exist for the same identity.
    owner.frame = 20;
    owner
        .host_object_mut(unit)
        .unwrap()
        .template_mut()
        .add_kind_of(KindOf::Inert);
    assert!(!owner.owned_script_condition_for_test(condition("NamedUnit")));
}
