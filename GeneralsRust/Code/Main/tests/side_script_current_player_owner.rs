use gamelogic::scripting::core::{
    Condition, ConditionType, LOCAL_PLAYER, OrCondition, Parameter, ParameterType, Script,
    ScriptAction, ScriptActionType, ScriptList, THIS_PLAYER,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};

fn script() -> ScriptEngine {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut action = ScriptAction::new(ScriptActionType::PlayerSetMoney);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 900))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "SideCurrentPlayerOwner".into();
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    script.is_one_shot = true;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn world(script_name: &str, cash: u32) -> GameLogic {
    let mut world = GameLogic::new();
    let mut player =
        Player::new(1, Team::USA, "SameDisplayName", true).with_script_identity(script_name);
    player.resources.supplies = cash;
    world.add_player(player);
    world
}

fn ai_player_action_script() -> ScriptEngine {
    fn action(kind: ScriptActionType, parameter_type: ParameterType, value: &str) -> ScriptAction {
        let mut action = ScriptAction::new(kind);
        action
            .add_parameter(Parameter::with_string(parameter_type, value.into()))
            .unwrap();
        action
    }
    let mut select = action(
        ScriptActionType::PlayerSelectSkillset,
        ParameterType::Side,
        LOCAL_PLAYER,
    );
    select
        .add_parameter(Parameter::with_int(ParameterType::Int, 4))
        .unwrap();
    let mut delay = action(
        ScriptActionType::SetBaseConstructionSpeed,
        ParameterType::Side,
        LOCAL_PLAYER,
    );
    delay
        .add_parameter(Parameter::with_int(ParameterType::Int, 21))
        .unwrap();
    let mut upgrade = action(
        ScriptActionType::AiPlayerBuildUpgrade,
        ParameterType::Side,
        LOCAL_PLAYER,
    );
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_AmericaRangerCaptureBuilding".into(),
        ))
        .unwrap();
    delay.next_action = Some(Box::new(upgrade));
    select.next_action = Some(Box::new(delay));

    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "LocalAiPlayerTokenOwner".into();
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(select));
    script.is_one_shot = true;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    // Side slot 2 maps to the receiving Player whose numeric ID is also 2.
    engine
        .set_script_list_for_player(2, Some(Box::new(list)))
        .unwrap();
    engine
}

fn missing_current_player_script() -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::PlayerSelectSkillset);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 4))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "MissingCurrentPlayerDoesNotSelectLocal".into();
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    script.is_one_shot = true;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    // Slot 1 has no receiving Player. The world still has a different local
    // AI Player, which must not be substituted for THIS_PLAYER.
    engine
        .set_script_list_for_player(1, Some(Box::new(list)))
        .unwrap();
    engine
}

fn ai_world(local_id: u32) -> GameLogic {
    let mut world = GameLogic::new();
    for (id, team, name) in [
        (1, Team::USA, "WorldOnePlayer"),
        (2, Team::USA, "WorldTwoPlayer"),
    ] {
        let mut player = Player::new(id, team, name, false).with_script_identity(name);
        player.resources.supplies = 10_000;
        player.set_can_build_units(true);
        world.add_player(player);
        world.add_ai_opponent(id, team, generals_main::ai::AIDifficulty::Medium);
    }
    assert!(world.select_local_player(Some(local_id)));
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks.add_kind_of(KindOf::Structure);
    world.templates.insert(barracks.name.clone(), barracks);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry);
    world.templates.insert(ranger.name.clone(), ranger);
    for id in [1, 2] {
        world
            .create_object_for_player("AmericaBarracks", id, glam::Vec3::ZERO)
            .unwrap();
    }
    world
}

#[test]
fn side_script_current_player_is_resolved_from_each_receiving_world() {
    let mut alpha = world("AlphaSidePlayer", 100);
    let mut beta = world("BetaSidePlayer", 700);
    assert_eq!(
        alpha.get_player(1).unwrap().id,
        beta.get_player(1).unwrap().id
    );

    // The production ScriptEngine side walk resolves THIS_PLAYER through each
    // receiving Main driver; both engines execute side 0 with colliding IDs.
    alpha
        .update_local_script_engine_with_owner(&script())
        .unwrap();
    beta.update_local_script_engine_with_owner(&script())
        .unwrap();

    assert_eq!(alpha.get_player(1).unwrap().effective_supplies(), 900);
    assert_eq!(beta.get_player(1).unwrap().effective_supplies(), 900);
}

#[test]
fn ai_player_tokens_resolve_locally_when_local_differs_from_current_and_ids_collide() {
    let mut first = ai_world(1);
    let mut peer = ai_world(2);
    assert_eq!(
        first.get_player(1).unwrap().id,
        peer.get_player(1).unwrap().id
    );
    assert_eq!(
        first.get_player(2).unwrap().id,
        peer.get_player(2).unwrap().id
    );

    first
        .update_local_script_engine_with_owner(&ai_player_action_script())
        .unwrap();
    peer.update_local_script_engine_with_owner(&ai_player_action_script())
        .unwrap();

    let ai = |world: &GameLogic, id| {
        world
            .capture_ai_player_queue_persist()
            .into_iter()
            .find(|row| row.player_id == id)
            .unwrap()
    };
    let first_local = ai(&first, 1);
    let first_current = ai(&first, 2);
    let peer_local = ai(&peer, 2);
    let peer_other = ai(&peer, 1);
    assert_eq!(first_local.skillset_selector, 3);
    assert_eq!(first_local.team_seconds, 21.0);
    assert_eq!(first_current.skillset_selector, -1);
    assert_eq!(peer_local.skillset_selector, 3);
    assert_eq!(peer_local.team_seconds, 21.0);
    assert_eq!(peer_other.skillset_selector, -1);
    assert!(
        first
            .get_player(1)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
    assert!(
        !first
            .get_player(2)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
    assert!(
        peer.get_player(2)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
    assert!(
        !peer
            .get_player(1)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
}

#[test]
fn missing_receiving_current_player_does_not_fall_back_to_local_ai() {
    let mut world = GameLogic::new();
    let player = Player::new(1, Team::USA, "OnlyPlayer", false).with_script_identity("OnlyPlayer");
    world.add_player(player);
    world.add_ai_opponent(1, Team::USA, generals_main::ai::AIDifficulty::Medium);
    assert!(world.select_local_player(Some(1)));
    world
        .update_local_script_engine_with_owner(&missing_current_player_script())
        .unwrap();

    let local = world
        .capture_ai_player_queue_persist()
        .into_iter()
        .find(|row| row.player_id == 1)
        .unwrap();
    assert_eq!(local.skillset_selector, -1);
}
