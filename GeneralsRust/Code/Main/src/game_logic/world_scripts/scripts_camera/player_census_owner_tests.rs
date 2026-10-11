//! Real ScriptEngine conditions read this world's current player and object census.
use super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};

fn credits(name: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::PlayerHasCredits);
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 500))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 0))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, name.into()))
        .unwrap();
    condition
}
fn side(kind: ConditionType, name: &str) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, name.into()))
        .unwrap();
    condition
}
#[test]
fn owned_player_census_conditions_ignore_same_id_peer_and_observe_live_mutation() {
    let (mut owner, unit, _) = world();
    let (mut peer, foreign, _) = world();
    assert_eq!(unit, foreign);
    owner.players.get_mut(&1).unwrap().resources.supplies = 2500;
    owner.players.get_mut(&1).unwrap().power_produced = 10;
    owner.players.get_mut(&1).unwrap().power_consumed = 20;
    peer.players.get_mut(&1).unwrap().resources.supplies = 0;
    peer.players.get_mut(&1).unwrap().power_produced = 30;
    peer.players.get_mut(&1).unwrap().power_consumed = 10;
    assert!(owner.owned_script_condition_for_test(credits("NamedOwner")));
    assert!(!peer.owned_script_condition_for_test(credits("NamedOwner")));
    assert!(!owner.owned_script_condition_for_test(credits("MissingOwner")));
    assert!(
        !owner.owned_script_condition_for_test(side(ConditionType::PlayerHasPower, "NamedOwner"))
    );
    assert!(
        peer.owned_script_condition_for_test(side(ConditionType::PlayerHasPower, "NamedOwner"))
    );
    let mut has_unit = side(ConditionType::PlayerHasObjectComparison, "NamedOwner");
    has_unit
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 4))
        .unwrap();
    has_unit
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    has_unit
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "OwnedNamedInfantry".into(),
        ))
        .unwrap();
    assert!(owner.owned_script_condition_for_test(has_unit));
    owner.players.get_mut(&1).unwrap().resources.supplies = 0;
    assert!(!owner.owned_script_condition_for_test(credits("NamedOwner")));
    assert_eq!(peer.players.get(&1).unwrap().effective_supplies(), 0);
}

#[test]
fn built_by_player_requires_owned_literal_definition_before_type_list_expansion() {
    use gamelogic::scripting::core::{OrCondition, Script};
    use gamelogic::scripting::engine::ScriptEngine;
    let (mut owner, unit, _) = world();
    let (mut peer, _, _) = world();
    owner.host_object_mut(unit).unwrap().status.effectively_dead = true;
    let engine = ScriptEngine::new().unwrap();
    let mut list = gamelogic::object::object_types::ObjectTypes::new();
    list.add_object_type(gamelogic::common::AsciiString::from("OwnedNamedInfantry"));
    engine.set_object_types("OwnerInfantryList".into(), list);
    owner.install_owned_script_engine(engine);
    for (name, expected) in [("OwnerInfantryList", false), ("OwnedNamedInfantry", true)] {
        let mut condition = Condition::new(ConditionType::BuiltByPlayer);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                name.into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "NamedOwner".into(),
            ))
            .unwrap();
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(condition)));
        let mut script = Script::new();
        script.condition = Some(Box::new(branch));
        assert_eq!(
            owner
                .with_owned_script_engine(|engine, world| world.evaluate_owned_script_conditions(
                    engine,
                    &mut script,
                    None
                ))
                .unwrap(),
            expected
        );
    }
    // The same spelling resolves a literal template in this second owner,
    // while its independent list/census is absent in the first owner.
    let mut template = ThingTemplate::new("OwnerInfantryList");
    template.add_kind_of(KindOf::Infantry);
    peer.templates.insert(template.name.clone(), template);
    peer.create_object_for_player("OwnerInfantryList", 1, Vec3::ZERO)
        .unwrap();
    let mut condition = Condition::new(ConditionType::BuiltByPlayer);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "OwnerInfantryList".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "NamedOwner".into(),
        ))
        .unwrap();
    assert!(peer.owned_script_condition_for_test(condition));
    assert!(!owner.templates.contains_key("OwnerInfantryList"));
}
