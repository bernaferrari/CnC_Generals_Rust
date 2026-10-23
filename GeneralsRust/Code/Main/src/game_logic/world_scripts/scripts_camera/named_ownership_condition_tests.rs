//! CPP ScriptConditions1217: controlling-player identity survives dead bodies.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, Parameter, ParameterType, THIS_OBJECT, THIS_PLAYER,
};
use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
fn owned(unit: &str, player: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::NamedOwnedByPlayer);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, unit.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    condition
}
#[test]
fn named_ownership_uses_canonical_player_identity_without_alive_gate_or_foreign_world() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    peer.host_object_mut(peer_unit).unwrap().owner_player_id = Some(2);
    assert!(owner.owned_script_condition_for_test(owned("NamedUnit", "NamedOwner")));
    assert!(!peer.owned_script_condition_for_test(owned("NamedUnit", "NamedOwner")));
    // Zero health and destruction state do not erase the controlling Player
    // pointer while the canonical named object still exists.
    let object = owner.host_object_mut(unit).unwrap();
    object.health.current = 0.0;
    object.status.effectively_dead = true;
    object.status.destroyed = true;
    assert!(owner.owned_script_condition_for_test(owned("NamedUnit", "NamedOwner")));
    assert!(!owner.owned_script_condition_for_test(owned("AbsentUnit", "NamedOwner")));
    assert!(!owner.owned_script_condition_for_test(owned("NamedUnit", "AbsentPlayer")));
    // Authored map names identify players; matching display/faction labels do
    // not fabricate identity or transfer ownership to another real player.
    owner.players.get_mut(&1).unwrap().map_side.map_player_name = "AuthoredOwner".into();
    owner.players.get_mut(&2).unwrap().name = "NamedOwner".into();
    assert!(owner.owned_script_condition_for_test(owned("NamedUnit", "AuthoredOwner")));
    assert!(!owner.owned_script_condition_for_test(owned("NamedUnit", "NamedOwner")));
    let driver = HostScriptExecutionDriver::new(&mut owner);
    assert_eq!(
        driver.named_owned_by_player(
            THIS_OBJECT,
            THIS_PLAYER,
            Some("AuthoredOwner"),
            Some(unit.0)
        ),
        ScriptOwnerQuery::Present(true)
    );
    assert_eq!(
        driver.named_owned_by_player(THIS_OBJECT, THIS_PLAYER, Some("NamedVictim"), Some(unit.0)),
        ScriptOwnerQuery::Present(false)
    );
    assert_eq!(
        driver.named_owned_by_player(THIS_OBJECT, THIS_PLAYER, Some("NamedOwner"), Some(unit.0)),
        ScriptOwnerQuery::Missing,
        "display-name collision must not fabricate a canonical current player"
    );
    owner.host_object_mut(unit).unwrap().owner_player_id = None;
    assert!(!owner.owned_script_condition_for_test(owned("NamedUnit", "AuthoredOwner")));
}
