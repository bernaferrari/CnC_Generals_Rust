//! Same-faction skirmish players stay distinct: script player tokens,
//! script AI requests and enemy acquisition resolve the exact player
//! (C++ ScriptEngine / AISkirmishPlayer act on Player instances, not factions).
use super::*;

/// Run `body` in an exact child process with the Common/Main/World service owners
/// these AI fixtures require (no process-wide service fallback exists).
fn with_owned_services(test: &str, body: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), test, || {
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, body);
    });
}

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
    with_owned_services(
        "skirmish_build_building_marks_only_the_executing_players_pad",
        || {
            // C++ ScriptActions::doBuildBuilding acts on TheScriptEngine->getCurrentPlayer().
            let mut logic = same_faction_ai_match();
            // The request queue is owned by the receiving world's services.
            logic.with_own_services(|| {
                let _ = gamelogic::scripting::take_host_skirmish_build_requests();
                gamelogic::scripting::request_host_skirmish_build_building(
                    "PlyrChinaB",
                    "ChinaBarracks",
                );
            });
            logic.apply_host_skirmish_script_requests();
            let priority = |logic: &crate::game_logic::GameLogic, id: u32| {
                logic.ai_manager.ai_players[&id].building_queue[0].is_priority
            };
            assert!(
                !priority(&logic, 1),
                "the other same-faction AI must not be marked"
            );
            assert!(priority(&logic, 2));
        },
    );
}

fn supply_condition(
    kind: gamelogic::scripting::core::ConditionType,
    player: &str,
) -> gamelogic::scripting::core::Script {
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
    };

    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    if kind == ConditionType::SupplySourceSafe {
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 1_000))
            .unwrap();
    }
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.condition = Some(Box::new(branch));
    script
}

fn evaluate_supply(
    world: &mut GameLogic,
    condition: &mut gamelogic::scripting::core::Script,
) -> bool {
    world
        .with_owned_script_engine(|engine, world| {
            world.evaluate_owned_script_conditions(engine, condition, None)
        })
        .expect("receiving ScriptEngine")
}

#[test]
fn supply_source_queries_for_a_player_without_ai_match_cpp_defaults() {
    use gamelogic::scripting::core::ConditionType;

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Display Label", true)
        .with_script_identity("PlyrHuman"));
    let mut safe = supply_condition(ConditionType::SupplySourceSafe, "PlyrHuman");
    let mut attacked = supply_condition(ConditionType::SupplySourceAttacked, "PlyrHuman");
    let mut unknown = supply_condition(ConditionType::SupplySourceSafe, "NoSuchPlayer");
    assert!(evaluate_supply(&mut world, &mut safe));
    assert!(!evaluate_supply(&mut world, &mut attacked));
    assert!(!evaluate_supply(&mut world, &mut unknown));
}

#[test]
fn supply_source_queries_use_canonical_map_player_identity() {
    use gamelogic::scripting::core::ConditionType;

    fn world(identity: &str) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Shared Display", false)
            .with_script_identity(identity));
        world.get_player_mut(0).unwrap().map_side.is_skirmish_ai = true;
        world.add_ai_opponent(0, Team::USA, AIDifficulty::Medium);
        let mut dozer = ThingTemplate::new("SupplyQueryDozer");
        dozer.add_kind_of(KindOf::Dozer).set_health(100.0);
        world.templates.insert(dozer.name.clone(), dozer);
        let id = world.create_object_for_player(
            "SupplyQueryDozer",
            0,
            Vec3::new(10.0, 0.0, 20.0),
        ).unwrap();
        (world, id)
    }

    let (mut first, dozer) = world("AuthoredSideA");
    let (mut peer, peer_dozer) = world("AuthoredSideB");
    assert_eq!(dozer, peer_dozer);
    first.frame = 50;
    peer.frame = 50;
    first.get_player_mut(0).unwrap().set_attacked_by_at(1, 50);
    first.host_object_mut(dozer).unwrap().last_damage_timestamp = Some(50);

    let mut attacked = supply_condition(ConditionType::SupplySourceAttacked, "AuthoredSideA");
    assert!(evaluate_supply(&mut first, &mut attacked));
    // The first attack scan consumed the ten-frame window. Only the safety
    // query below is allowed to trigger getAiEnemy for this actor.
    assert!(!evaluate_supply(&mut first, &mut attacked));
    assert_eq!(first.ai_manager.ai_players[&0].frame_to_check_enemy, 0);
    let mut peer_attack = supply_condition(ConditionType::SupplySourceAttacked, "AuthoredSideB");
    let mut missing = supply_condition(ConditionType::SupplySourceAttacked, "AuthoredSideA");
    assert!(!evaluate_supply(&mut peer, &mut peer_attack));
    assert!(!evaluate_supply(&mut peer, &mut missing));

    let mut display = supply_condition(ConditionType::SupplySourceSafe, "Shared Display");
    assert!(!evaluate_supply(&mut first, &mut display));
    assert_eq!(first.ai_manager.ai_players[&0].frame_to_check_enemy, 0);
    let mut safe = supply_condition(ConditionType::SupplySourceSafe, "AuthoredSideA");
    assert!(evaluate_supply(&mut first, &mut safe));
    assert_eq!(first.ai_manager.ai_players[&0].frame_to_check_enemy, 200);
    assert_eq!(peer.ai_manager.ai_players[&0].frame_to_check_enemy, 0);

    // Refresh the condition just before getAiEnemy's deadline. Its own
    // inclusive cache makes the next script call wait until frame 260.
    for frame in [110, 199, 200, 259] {
        first.frame = frame;
        assert!(evaluate_supply(&mut first, &mut safe));
        assert_eq!(first.ai_manager.ai_players[&0].frame_to_check_enemy, 200);
    }
    first.frame = 260;
    assert!(evaluate_supply(&mut first, &mut safe));
    assert_eq!(first.ai_manager.ai_players[&0].frame_to_check_enemy, 410);
    assert_eq!(peer.ai_manager.ai_players[&0].frame_to_check_enemy, 0);
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

    for (id, team, name) in [
        (1, Team::USA, "PlyrUSA_A"),
        (2, Team::USA, "PlyrUSA_B"),
        (3, Team::China, "PlyrChina"),
    ] {
        // Free-for-all lobby teams: everyone is an enemy.
        let mut player = crate::game_logic::Player::new(id, team, name, id == 1);
        player.alliance_team = id as i32;
        logic.add_player(player);
    }
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

fn relationship_match() -> crate::game_logic::GameLogic {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut cc = crate::game_logic::ThingTemplate::new("RelCommandCenter");
    cc.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::CommandCenter)
        .set_health(1000.0);
    logic.templates.insert("RelCommandCenter".into(), cc);
    let mut ranger = crate::game_logic::ThingTemplate::new("RelRanger");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    logic.templates.insert("RelRanger".into(), ranger);
    for (id, team, x) in [
        (1, Team::China, 0.0),
        (2, Team::China, 500.0),
        (3, Team::USA, 900.0),
    ] {
        let identity = format!("Rel{id}");
        logic.add_player(
            crate::game_logic::Player::new(id, team, &identity, false)
                .with_script_identity(&identity),
        );
        if id != 1 {
            logic
                .create_object_for_player("RelCommandCenter", id, Vec3::new(x, 0.0, 0.0))
                .expect("cc");
            logic
                .create_object_for_player("RelRanger", id, Vec3::new(x, 0.0, 10.0))
                .expect("ranger");
        }
    }
    logic
}

#[test]
fn acquire_enemy_follows_relationships_not_factions() {
    // C++ acquireEnemy: m_player->getRelationship(cur->getDefaultTeam()) == ENEMIES.
    use game_engine::common::game_common::Relationship;
    let mut logic = relationship_match();
    // Same faction, explicit ENEMIES; different faction, no relation (NEUTRAL).
    logic
        .get_player_mut(1)
        .unwrap()
        .set_map_relationship(2, Relationship::Enemies);
    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    ai.update_enemy_assessment(&mut logic, 10.0);
    assert_eq!(ai.enemy_player_id, Some(2));

    // A team override on the candidate's default team beats the player relation.
    let default_team = logic
        .host_default_team_id(2)
        .expect("receiving default Team");
    logic
        .get_player_mut(1)
        .unwrap()
        .set_team_relationship_override(default_team, Relationship::Allies);
    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    ai.update_enemy_assessment(&mut logic, 10.0);
    assert_eq!(
        ai.enemy_player_id, None,
        "allied default team and neutral USA"
    );
}
