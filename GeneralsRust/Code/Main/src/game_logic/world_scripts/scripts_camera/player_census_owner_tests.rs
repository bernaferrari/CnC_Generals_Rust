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
    assert_eq!(census["NamedOwner"].energy_production, 0);
    assert!(census["NamedOwner"].power_sabotaged);
    assert_eq!(owner.players[&1].power_produced, 20);
    assert!(
        !owner.owned_script_condition_for_test(side(ConditionType::PlayerHasPower, "NamedOwner"))
    );
    owner.frame = 50;
    let census = owner.owned_host_player_census();
    assert_eq!(census["NamedOwner"].energy_production, 20);
    assert!(!census["NamedOwner"].power_sabotaged);
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
        owner.add_player(
            Player::new(1, Team::USA, "Energy owner display", true)
                .with_script_identity("EnergyOwner"),
        );
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

#[test]
fn same_display_labels_keep_player_census_and_script_names_distinct() {
    let mut world = GameLogic::new();
    let mut first =
        Player::new(1, Team::USA, "Shared Display", true).with_script_identity("AuthoredFirst");
    first.resources.supplies = 0;
    let mut second =
        Player::new(2, Team::USA, "Shared Display", false).with_script_identity("AuthoredSecond");
    first.power_produced = 10;
    second.resources.supplies = 1_000;
    second.power_produced = 10;
    world.add_player(first);
    world.add_player(second);

    // These are actual ScriptEngine conditions over the owned host census.
    assert!(!world.owned_script_condition_for_test(credits("AuthoredFirst")));
    assert!(world.owned_script_condition_for_test(credits("AuthoredSecond")));
    assert!(
        !world
            .owned_script_condition_for_test(
                side(ConditionType::PlayerHasPower, "Shared Display",)
            )
    );

    let census = world.owned_host_player_census();
    assert_eq!(census["AuthoredFirst"].money, 0);
    assert_eq!(census["AuthoredSecond"].money, 1_000);
    assert!(!census.contains_key("Shared Display"));
    assert!(!census.contains_key("USA"));
    assert!(!census.contains_key("America"));
    assert!(!census.contains_key("PlyrAmerica"));
}

#[test]
fn crossed_display_labels_cannot_redirect_script_condition_or_action_owner() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
    use gamelogic::scripting::engine::ScriptEngine;

    let mut world = GameLogic::new();
    let mut first = Player::new(1, Team::USA, "Initial first display", true)
        .with_script_identity("AuthoredFirst");
    first.resources.supplies = 0;
    let mut second = Player::new(2, Team::China, "Initial second display", false)
        .with_script_identity("AuthoredSecond");
    second.resources.supplies = 1_000;
    world.add_player(first);
    world.add_player(second);

    // Match the real census iteration order, then create crossed labels. The
    // former display-key insertion deterministically overwrote the earlier
    // canonical census row, and the former action resolver matched its display
    // alias before reaching the later canonical owner.
    let mut ids = world.players.values().map(|player| player.id);
    let early = ids.next().unwrap();
    let late = ids.next().unwrap();
    let early_name = world.players[&early].map_side.map_player_name.clone();
    let late_name = world.players[&late].map_side.map_player_name.clone();
    world.players.get_mut(&early).unwrap().name = late_name.clone();
    world.players.get_mut(&late).unwrap().name = early_name.clone();
    world.players.get_mut(&early).unwrap().resources.supplies = 0;
    world.players.get_mut(&late).unwrap().resources.supplies = 1_000;

    assert!(!world.owned_script_condition_for_test(credits(&early_name)));
    assert!(world.owned_script_condition_for_test(credits(&late_name)));

    for id in [early, late] {
        let team = world.players[&id].team;
        world
            .ai_manager
            .add_ai_player(id, team, crate::ai::AIDifficulty::Medium);
    }
    world
        .ai_manager
        .ai_players
        .get_mut(&early)
        .unwrap()
        .select_skillset(1);
    world
        .ai_manager
        .ai_players
        .get_mut(&late)
        .unwrap()
        .select_skillset(0);
    let early_before = world.ai_manager.ai_players[&early].selected_skillset();
    let late_before = world.ai_manager.ai_players[&late].selected_skillset();
    let mut action = ScriptAction::new(ScriptActionType::PlayerSelectSkillset);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            late_name.clone(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 4))
        .unwrap();
    let engine = ScriptEngine::new().unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        None,
        gamelogic::scripting::executor::ScriptContext::new(),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(
        world.ai_manager.ai_players[&early].selected_skillset(),
        early_before
    );
    assert_eq!(world.ai_manager.ai_players[&late].selected_skillset(), 3);
    assert_ne!(
        world.ai_manager.ai_players[&late].selected_skillset(),
        late_before
    );

    let mut wrong_case = ScriptAction::new(ScriptActionType::PlayerSelectSkillset);
    wrong_case
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            late_name.to_ascii_lowercase(),
        ))
        .unwrap();
    wrong_case
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &wrong_case,
        None,
        gamelogic::scripting::executor::ScriptContext::new(),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(
        world.ai_manager.ai_players[&early].selected_skillset(),
        early_before
    );
    assert_eq!(world.ai_manager.ai_players[&late].selected_skillset(), 3);

    let before = [
        world.ai_manager.ai_players[&early].selected_skillset(),
        world.ai_manager.ai_players[&late].selected_skillset(),
    ];
    let mut faction_alias = ScriptAction::new(ScriptActionType::PlayerSelectSkillset);
    faction_alias
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    faction_alias
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &faction_alias,
        None,
        gamelogic::scripting::executor::ScriptContext::new(),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(
        [
            world.ai_manager.ai_players[&early].selected_skillset(),
            world.ai_manager.ai_players[&late].selected_skillset(),
        ],
        before
    );
}

#[test]
fn canonical_player_census_identity_survives_encoded_world_snapshot_restore() {
    use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};

    let mut source = GameLogic::new();
    let mut first =
        Player::new(1, Team::USA, "Shared Display", true).with_script_identity("SavedSideA");
    first.resources.supplies = 0;
    let mut second =
        Player::new(2, Team::China, "Shared Display", false).with_script_identity("SavedSideB");
    second.resources.supplies = 1_000;
    source.add_player(first);
    source.add_player(second);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
    let mut receiver = GameLogic::new();
    receiver.add_player(
        Player::new(1, Team::USA, "Receiver label A", true).with_script_identity("SavedSideA"),
    );
    receiver.add_player(
        Player::new(2, Team::China, "Receiver label B", false).with_script_identity("SavedSideB"),
    );
    builder
        .restore_from_snapshot(&decoded, &mut receiver)
        .unwrap();

    assert!(!receiver.owned_script_condition_for_test(credits("SavedSideA")));
    assert!(receiver.owned_script_condition_for_test(credits("SavedSideB")));
}
