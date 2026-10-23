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

#[test]
fn owned_census_reads_effective_energy_without_destroying_sabotaged_owner_counter() {
    let mut owner = GameLogic::new();
    owner.add_player(
        Player::new(1, Team::USA, "NamedOwner", true).with_script_identity("NamedOwner"),
    );
    let mut peer = GameLogic::new();
    peer.add_player(
        Player::new(1, Team::USA, "NamedOwner", true).with_script_identity("NamedOwner"),
    );
    owner.frame = 37;
    let player = owner.players.get_mut(&1).unwrap();
    player.power_produced = 20;
    player.power_consumed = 0;
    player.power_sabotaged_till_frame = 50;
    let raw_peer = peer.players[&1].power_produced;
    let census = owner.owned_host_player_census();
    assert_eq!(census["namedowner"].energy_production, 0);
    assert!(census["namedowner"].power_sabotaged);
    assert_eq!(owner.players[&1].power_produced, 20);
    assert!(
        !owner.owned_script_condition_for_test(side(ConditionType::PlayerHasPower, "NamedOwner"))
    );
    owner.frame = 50;
    let census = owner.owned_host_player_census();
    assert_eq!(census["namedowner"].energy_production, 20);
    assert!(!census["namedowner"].power_sabotaged);
    assert!(
        owner.owned_script_condition_for_test(side(ConditionType::PlayerHasPower, "NamedOwner"))
    );
    assert_eq!(peer.players[&1].power_produced, raw_peer);
}

#[test]
fn authored_sabotage_preserves_raw_owner_and_recovers_observers_at_deadline() {
    let make = |production: i32| {
        let mut owner = GameLogic::new();
        owner.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "sabotage-owner.ini".into(),
                format!("Object SabotageProducer\nKindOf = VEHICLE\nEnergyProduction = {production}\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject SabotageConsumer\nKindOf = VEHICLE\nEnergyProduction = -10\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"),
            )]).unwrap(),
        ).unwrap();
        owner.add_player(Player::new(1, Team::USA, "EnergyOwner", true));
        let producer = owner
            .create_object_for_player("SabotageProducer", 1, glam::Vec3::ZERO)
            .unwrap();
        let consumer = owner
            .create_object_for_player("SabotageConsumer", 1, glam::Vec3::ONE)
            .unwrap();
        owner.frame = 37;
        owner.update_player_resources(0.0);
        (owner, producer, consumer)
    };
    let (mut owner, producer, consumer) = make(20);
    let (peer, peer_producer, peer_consumer) = make(35);
    assert_eq!((producer, consumer), (peer_producer, peer_consumer));
    assert_eq!(
        (
            owner.players[&1].power_produced,
            owner.players[&1].power_consumed
        ),
        (20, 10)
    );
    owner
        .players
        .get_mut(&1)
        .unwrap()
        .power_sabotaged_till_frame = 50;
    owner.update_player_resources(0.0);
    assert_eq!(
        (
            owner.players[&1].power_produced,
            owner.players[&1].power_consumed
        ),
        (20, 10),
        "sabotage must mask observed power without erasing canonical Energy production"
    );
    assert_eq!(
        (
            owner.player_economy(1).unwrap().1,
            owner.player_economy(1).unwrap().2
        ),
        (-10, 0)
    );
    owner.frame = 50;
    // No economy update may be required for a getter to observe deadline equality.
    assert_eq!(
        (
            owner.player_economy(1).unwrap().1,
            owner.player_economy(1).unwrap().2
        ),
        (10, 20)
    );
    assert_eq!(
        (
            peer.players[&1].power_produced,
            peer.players[&1].power_consumed
        ),
        (35, 10)
    );
    assert_eq!(peer.player_economy(1).unwrap().1, 25);
    assert_eq!(peer.players[&1].power_sabotaged_till_frame, 0);
}
