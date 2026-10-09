//! Same-faction skirmish players stay distinct: script player tokens,
//! script AI requests and enemy acquisition resolve the exact player
//! (C++ ScriptEngine / AISkirmishPlayer act on Player instances, not factions).
use super::*;

fn same_faction_ai_match() -> crate::game_logic::GameLogic {
    let mut logic = crate::game_logic::GameLogic::new();
    for (id, name) in [(1, "PlyrChinaA"), (2, "PlyrChinaB")] {
        logic.add_player(crate::game_logic::Player::new(id, Team::China, name, false));
        logic.add_ai_opponent(id, Team::China, AIDifficulty::Medium);
        let ai = logic.ai_manager.ai_players.get_mut(&id).expect("AI player");
        ai.building_queue.clear();
        ai.building_queue.push(crate::ai::AIBuildingInfo::new(
            "ChinaBarracks".to_string(),
            glam::Vec3::ZERO,
            crate::ai::UNLIMITED_REBUILDS,
        ));
    }
    logic
}

#[test]
fn script_player_tokens_resolve_the_exact_same_faction_player() {
    let logic = same_faction_ai_match();
    for _ in 0..8 {
        assert_eq!(AIManager::resolve_player_id(&logic, "PlyrChinaA"), Some(1));
        assert_eq!(AIManager::resolve_player_id(&logic, "plyrchinab"), Some(2));
    }
    // A bare faction token is ambiguous between two China players.
    assert_eq!(AIManager::resolve_player_id(&logic, "China"), None);
}

#[test]
fn skirmish_build_building_marks_only_the_executing_players_pad() {
    // C++ ScriptActions::doBuildBuilding acts on TheScriptEngine->getCurrentPlayer().
    let _ = gamelogic::scripting::take_host_skirmish_build_requests();
    let mut logic = same_faction_ai_match();
    gamelogic::scripting::request_host_skirmish_build_building("PlyrChinaB", "ChinaBarracks");
    logic.apply_host_skirmish_script_requests();
    let priority = |logic: &crate::game_logic::GameLogic, id: u32| {
        logic.ai_manager.ai_players[&id].building_queue[0].is_priority
    };
    assert!(
        !priority(&logic, 1),
        "the other same-faction AI must not be marked"
    );
    assert!(priority(&logic, 2));
}

#[test]
fn supply_source_queries_for_a_player_without_ai_match_cpp_defaults() {
    // C++ Player::isSupplySourceSafe returns TRUE and isSupplySourceAttacked
    // FALSE when the player has no AI (Player.cpp:1576-1593).
    gamelogic::scripting::clear_host_script_query_snapshot();
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "PlyrHuman",
        true,
    ));
    logic.inject_host_supply_source_queries();
    assert_eq!(
        gamelogic::scripting::host_query_supply_source_safe("PlyrHuman", 1_000),
        Some(true)
    );
    assert_eq!(
        gamelogic::scripting::host_query_supply_source_attacked("PlyrHuman"),
        Some(false)
    );
    assert_eq!(
        gamelogic::scripting::host_query_supply_source_safe("NoSuchPlayer", 1_000),
        None
    );
    gamelogic::scripting::clear_host_script_query_snapshot();
}

#[test]
fn acquire_enemy_scores_same_faction_players_by_their_own_objects() {
    // C++ AISkirmishPlayer::acquireEnemy queries each candidate Player's own
    // teams (Player::hasAnyObjects/hasAnyUnits/hasAnyBuildFacility and
    // getPlayerStructureBounds), never every object of the same faction.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::CommandCenter)
        .set_health(1000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut plant = crate::game_logic::ThingTemplate::new("AmericaPowerPlant");
    plant
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_health(1000.0);
    logic.templates.insert("AmericaPowerPlant".into(), plant);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "PlyrUSA_A",
        true,
    ));
    logic.add_player(crate::game_logic::Player::new(
        2,
        Team::USA,
        "PlyrUSA_B",
        false,
    ));
    logic.add_player(crate::game_logic::Player::new(
        3,
        Team::China,
        "PlyrChina",
        false,
    ));
    // Player 1: a real base far from the AI.
    logic
        .create_object_for_player("AmericaCommandCenter", 1, Vec3::new(1000.0, 0.0, 1000.0))
        .expect("cc");
    logic
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(1010.0, 0.0, 1000.0))
        .expect("ranger");
    // Player 2: only a stray power plant close to the AI (no units, no facility).
    logic
        .create_object_for_player("AmericaPowerPlant", 2, Vec3::new(100.0, 0.0, 100.0))
        .expect("plant");

    let mut ai = AIPlayer::new(3, Team::China, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    assert_eq!(
        ai.player_structure_bounds(&logic, 1),
        (1000.0, 1000.0, 1000.0, 1000.0),
        "player 1's bounds must not include player 2's structure"
    );
    let facilities = AiWorldView::new(&logic).build_facility_template_names();
    let player_two = logic.get_player(2).expect("player 2").clone();
    assert!(ai.player_in_bad_shape(&logic, &player_two, &facilities));
    let player_one = logic.get_player(1).expect("player 1").clone();
    assert!(!ai.player_in_bad_shape(&logic, &player_one, &facilities));

    ai.update_enemy_assessment(&mut logic, 10.0);
    assert_eq!(
        ai.enemy_player_id,
        Some(1),
        "the crippled near player is deprioritized; the healthy far one is chosen"
    );
}
