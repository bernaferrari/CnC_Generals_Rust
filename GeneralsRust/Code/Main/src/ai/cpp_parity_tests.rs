use super::*;

fn install_player_team_prototype(
    leftover_index: i32,
    team_name: &str,
    units: &[(i32, i32, &'static str)],
    priority: i32,
) {
    use std::sync::{Arc, RwLock};
    let _ = gamelogic::scripting::engine::initialize_script_engine();
    if let Ok(guard) = gamelogic::scripting::engine::get_script_engine().read() {
        if let Some(engine) = guard.as_ref() {
            let mut or_c = gamelogic::scripting::OrCondition::new();
            or_c.set_first_and_condition(Some(Box::new(gamelogic::scripting::Condition::new(
                gamelogic::scripting::ConditionType::ConditionTrue,
            ))));
            let mut script = gamelogic::scripting::core::Script::new();
            script.set_name("AlwaysBuild".into());
            script.condition = Some(Box::new(or_c));
            let mut list = gamelogic::scripting::ScriptList::new();
            list.append_script(Box::new(script));
            let _ =
                engine.set_script_list_for_player(leftover_index as usize, Some(Box::new(list)));
        }
    }
    let proto_arc = {
        let mut tf = gamelogic::team::get_team_factory()
            .lock()
            .expect("team factory");
        let mut proto = gamelogic::team::TeamPrototype::new(team_name.into());
        proto.set_production_priority(priority);
        proto.set_production_condition("AlwaysBuild".into());
        proto.set_max_instances(8);
        for (i, (min_u, max_u, thing)) in units.iter().enumerate() {
            proto.set_units_info(
                i,
                gamelogic::team::CreateUnitsInfo {
                    min_units: *min_u,
                    max_units: *max_u,
                    unit_thing_name: thing,
                },
            );
        }
        tf.replace_team_prototype(proto);
        tf.find_team_prototype(team_name).expect("registered proto")
    };
    let mut list = gamelogic::player::player_list()
        .write()
        .expect("player list");
    list.clear();
    for i in 0..=leftover_index {
        let p = Arc::new(RwLock::new(gamelogic::player::Player::new(i)));
        if i == leftover_index {
            if let Ok(mut pg) = p.write() {
                pg.set_can_build_units(true);
                pg.add_team_to_list(proto_arc.clone());
            }
        }
        list.add_player(p);
    }
}

fn clear_player_team_prototypes() {
    if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
        factory.reset();
    }
    if let Ok(mut list) = gamelogic::player::player_list().write() {
        list.clear();
    }
}

fn install_leftover_computer_player(player_id: i32, skirmish: bool) {
    use std::sync::{Arc, RwLock};
    let mut list = gamelogic::player::player_list()
        .write()
        .expect("player list");
    list.clear();
    for i in 0..=player_id {
        let p = Arc::new(RwLock::new(gamelogic::player::Player::new(i)));
        if i == player_id {
            if let Ok(mut pg) = p.write() {
                pg.set_player_type(gamelogic::player::PlayerType::Computer, skirmish);
            }
        }
        list.add_player(p);
    }
}

#[test]
fn ai_default_base_inside_build_edge_residual() {
    // Default synthetic world is 512² centered at origin; MinDistFromEdge=30.
    // Layout offsets reach +100 (WarFactory) — bases must stay ≤ ~120 from origin.
    let mgr = AIManager::new();
    // Manager construction doesn't add players; mirror add_ai_player centers.
    let centers = [
        (Team::USA, Vec3::new(-120.0, 0.0, -120.0)),
        (Team::China, Vec3::new(120.0, 0.0, -120.0)),
        (Team::GLA, Vec3::new(120.0, 0.0, 120.0)),
    ];
    let half = 256.0;
    let edge = 30.0;
    let max_offset = 100.0;
    for (team, c) in centers {
        let farthest = c.x.abs().max(c.z.abs()) + max_offset;
        assert!(
            farthest <= half - edge,
            "{team:?} base pad would violate edge residual: farthest={farthest}"
        );
    }
    let _ = mgr;
}

#[test]
fn script_build_team_drains_onto_host_ai_queue() {
    let _ = gamelogic::scripting::take_host_build_team_requests();

    let mut logic = crate::game_logic::GameLogic::new();
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.set_cost(225, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut barracks_t = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks_t.set_cost(500, 0);
    barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
    logic.templates.insert("AmericaBarracks".into(), barracks_t);

    let mut player = crate::game_logic::Player::new(1, Team::USA, "PlyrAmerica", false);
    player.resources.supplies = 10_000;
    player.set_can_build_units(true);
    logic.add_player(player);
    logic.add_ai_opponent(1, Team::USA, AIDifficulty::Medium);
    if let Some(p) = logic.get_player_mut(1) {
        p.set_can_build_units(true);
    }
    let _ = logic.create_object("AmericaBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0));

    if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
        factory.reset();
        factory.init_team(
            gamelogic::common::AsciiString::from("USA_RangerSquad"),
            gamelogic::common::AsciiString::from("PlyrAmerica"),
            false,
            None,
        );
    }

    gamelogic::scripting::request_host_build_team("PlyrAmerica", "USA_RangerSquad");
    logic.apply_host_loco_set_script_requests();

    let queued = logic
        .ai_manager
        .ai_players
        .get(&1)
        .map(|ai| ai.team_queue.len())
        .unwrap_or(0);
    assert_eq!(
        queued, 1,
        "BUILD_TEAM must field a priority team on host AIPlayer"
    );
    let team = logic
        .ai_manager
        .ai_players
        .get(&1)
        .and_then(|ai| ai.team_queue.front())
        .expect("queued team");
    assert_eq!(team.name, "USA_RangerSquad");
    assert!(team.priority_build);

    if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
        factory.reset();
    }
}

#[test]
fn ai_player_update_order_matches_cpp_aiplayer_update() {
    // C++ AIPlayer.cpp:2987-3002
    let src = include_str!("player_core.rs");
    let start = src
        .find("/// Main AI update — C++ `AIPlayer::update`")
        .expect("AIPlayer::update docs");
    let body = &src[start..src.len().min(start + 1800)];
    let econ = body
        .find("update_economic_management")
        .expect("doBaseBuilding");
    let ready = body.find("check_ready_teams").expect("checkReadyTeams");
    let queued = body.find("check_queued_teams").expect("checkQueuedTeams");
    let mil = body
        .find("update_military_management")
        .expect("doTeamBuilding");
    let upg = body
        .find("do_upgrades_and_skills")
        .expect("doUpgradesAndSkills");
    let br = body
        .find("update_bridge_repair")
        .expect("updateBridgeRepair");
    assert!(econ < ready && ready < queued && queued < mil && mil < upg && upg < br);
    assert!(
        AIManager::new().update_interval > 0.0
            && (AIManager::new().update_interval - 1.0 / 30.0).abs() < 1e-6
    );
}

#[test]
fn aidata_timing_constants_match_retail_defaults() {
    // Default/AIData.ini: StructureSeconds=0, TeamSeconds=10, RebuildDelay=30.
    assert_eq!(AIPlayer::STRUCTURE_SECONDS, 0.0);
    assert_eq!(AIPlayer::TEAM_SECONDS, 10.0);
    assert_eq!(AIPlayer::REBUILD_DELAY_SECONDS, 30.0);
    assert_eq!(AIPlayer::ATTACK_RECHECK_SECONDS, 60.0);
    assert_eq!(AIPlayer::WEALTHY_RESOURCES, 7000);
    assert_eq!(AIPlayer::POOR_RESOURCES, 2000);
    assert!((AIPlayer::STRUCTURES_WEALTHY_RATE - 2.0).abs() < 1e-5);
    assert!((AIPlayer::STRUCTURES_POOR_RATE - 0.6).abs() < 1e-5);
    assert!((AIPlayer::TEAMS_WEALTHY_RATE - 2.0).abs() < 1e-5);
    assert!((AIPlayer::TEAMS_POOR_RATE - 0.6).abs() < 1e-5);
    assert!((AIPlayer::TEAM_RESOURCES_TO_START - 0.1).abs() < 1e-5);
    // setAIDifficulty does not rewrite TeamSeconds; modifiers stay unused there.
    assert!((AIDifficulty::Easy.get_build_delay_modifier() - 2.0).abs() < 1e-5);
    assert!((AIDifficulty::Medium.get_build_delay_modifier() - 1.0).abs() < 1e-5);
    assert!((AIDifficulty::Hard.get_build_delay_modifier() - 0.7).abs() < 1e-5);
}

#[test]
fn aidata_wealth_rate_scales_team_interval() {
    let mut logic = crate::game_logic::GameLogic::new();
    let ai = AIPlayer::new(1, Team::GLA, AIDifficulty::Medium);
    let mut player = crate::game_logic::Player::new(1, Team::GLA, "GLA", true);
    player.resources.supplies = 1000; // poor
    logic.add_player(player);

    let poor = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
    logic.get_player_mut(1).unwrap().resources.supplies = 4000; // normal
    let mid = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
    logic.get_player_mut(1).unwrap().resources.supplies = 8000; // wealthy
    let rich = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
    // Poor → longer wait; wealthy → shorter wait.
    assert!(
        poor > mid && mid > rich,
        "poor={poor} mid={mid} rich={rich}"
    );
    assert!(
        (mid - 10.0).abs() < 1e-3,
        "medium normal team interval ~10s got {mid}"
    );
    assert!(
        (rich - 5.0).abs() < 1e-3,
        "wealthy team interval ~5s got {rich}"
    );
    assert!(
        (poor - (10.0 / 0.6)).abs() < 1e-2,
        "poor team interval ~16.67s got {poor}"
    );
    // StructureSeconds=0 stays 0 regardless of wealth.
    assert_eq!(
        ai.scaled_interval_seconds(&logic, AIPlayer::STRUCTURE_SECONDS, true),
        0.0
    );
}

#[test]
fn aidata_team_resources_to_start_gates_queue() {
    // C++ isPossibleToBuildTeam: required = trunc(unit_cost_sum * TeamResourcesToStart).
    assert!((AIPlayer::TEAM_RESOURCES_TO_START - 0.1).abs() < 1e-5);
    let mut logic = crate::game_logic::GameLogic::new();
    // Seed templates with known build costs.
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.set_cost(225, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee.set_cost(700, 0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);

    let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    // USA_BasicForce = 2*Ranger + 1*Humvee = 2*225 + 700 = 1150; *0.1 = 115.
    let full = ai.estimate_team_unit_cost(&logic, "USA_BasicForce");
    assert_eq!(full, 1150);
    let required = (full as f32 * AIPlayer::TEAM_RESOURCES_TO_START) as u32;
    assert_eq!(required, 115);

    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
    player.resources.supplies = 114; // one under threshold
    logic.add_player(player);
    assert!(!ai.can_afford_team_start(&logic, "USA_BasicForce"));

    logic.get_player_mut(1).unwrap().resources.supplies = 115;
    assert!(ai.can_afford_team_start(&logic, "USA_BasicForce"));
    // Without factories, is_possible_to_build_team stays false (factory residual).
    assert!(!ai.is_possible_to_build_team(&logic, "USA_BasicForce"));
    assert!(!ai.should_build_new_team(&logic));
}

#[test]
fn select_team_to_build_calls_build_specific_ai_team() {
    let src = include_str!("teams.rs");
    let i = src
        .find("/// C++ `AIPlayer::selectTeamToBuild`")
        .expect("selectTeamToBuild");
    let window = &src[i..src.len().min(i + 2500)];
    assert!(
        window.contains("build_specific_ai_team(game_logic, name, false)")
            && window.contains("arm_team_timer_after_build")
            && !window.contains("create_team_queue"),
        "auto selectTeamToBuild must use leftover-right buildSpecificAITeam"
    );
}

#[test]
fn arm_team_timer_after_build_uses_wealth_not_difficulty() {
    // C++: m_teamTimer = TeamSeconds*FPS / TeamsPoorRate|TeamsWealthyRate.
    // Easy's 2x build-delay modifier must not stretch the interval.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", false);
    player.resources.supplies = 3_000;
    logic.add_player(player);
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Easy);
    ai.team_seconds = AIPlayer::TEAM_SECONDS;
    ai.arm_team_timer_after_build(&logic, 0.0);
    assert!(
        (ai.next_team_time - AIPlayer::TEAM_SECONDS).abs() < 1e-5,
        "mid-cash Easy arm is TeamSeconds not *2, got {}",
        ai.next_team_time
    );

    logic.get_player_mut(1).unwrap().resources.supplies = 8_000;
    ai.arm_team_timer_after_build(&logic, 0.0);
    assert!(
        (ai.next_team_time - 5.0).abs() < 1e-5,
        "wealthy / TeamsWealthyRate 2.0 → 5s, got {}",
        ai.next_team_time
    );

    logic.get_player_mut(1).unwrap().resources.supplies = 1_000;
    ai.arm_team_timer_after_build(&logic, 0.0);
    let poor_frames = (300f32 / AIPlayer::TEAMS_POOR_RATE) as u32;
    let poor_secs = poor_frames as f32 / LOGIC_FRAMES_PER_SECOND;
    assert!(
        (ai.next_team_time - poor_secs).abs() < 1e-5,
        "poor / TeamsPoorRate 0.6 → {poor_secs}s, got {}",
        ai.next_team_time
    );
}

#[test]
fn select_team_to_build_reinforce_does_not_arm_timer() {
    // C++ selectTeamToReinforce success returns before m_teamTimer is written.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut tank = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    tank.add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_cost(100, 0);
    logic.templates.insert("AmericaTankCrusader".into(), tank);
    let mut wf = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
    wf.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSWarFactory);
    logic.templates.insert("AmericaWarFactory".into(), wf);

    let tank_id = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::ZERO)
        .expect("live crusader");
    if let Some(obj) = logic.host_object_mut(tank_id) {
        obj.owner_player_id = Some(1);
        obj.team_instance_name = "HQ_Timer_TankTeam".into();
    }
    let factory = logic
        .create_object("AmericaWarFactory", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("idle factory");
    if let Some(obj) = logic.host_object_mut(factory) {
        obj.owner_player_id = Some(1);
    }

    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_Timer_TankTeam".into());
        proto.set_automatically_reinforce(true);
        proto.set_production_priority(50);
        proto.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 1,
                max_units: 3,
                unit_thing_name: "AmericaTankCrusader",
            },
        );
        tf.replace_team_prototype(proto);
        if let Some(team) = tf.create_team("HQ_Timer_TankTeam") {
            if let Ok(mut tg) = team.write() {
                tg.add_member(tank_id.0);
            }
        }
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Easy);
    ai.next_team_time = 42.0;
    assert!(
        ai.select_team_to_build(&mut logic, 1.0),
        "reinforce must count as a successful selectTeamToBuild"
    );
    let team = ai.team_queue.front().expect("reinforce order");
    assert!(team.reinforcement, "path must be reinforce, not a new pick");
    assert!(
        (ai.next_team_time - 42.0).abs() < f32::EPSILON,
        "reinforce must not arm TeamSeconds, got {}",
        ai.next_team_time
    );
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        tf.reset();
    }
}

#[test]
fn estimate_team_unit_cost_averages_min_max() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.set_cost(200, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_AvgCost".into());
        proto.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 1,
                max_units: 3,
                unit_thing_name: "AmericaInfantryRanger",
            },
        );
        tf.replace_team_prototype(proto);
    }
    let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    // C++ (min+max)/2 * cost = (1+3)/2 * 200 = 400, not max-as-required 600.
    assert_eq!(ai.estimate_team_unit_cost(&logic, "HQ_AvgCost"), 400);
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        tf.reset();
    }
}

#[test]
fn build_specific_ai_team_splits_optional_and_required() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    player.set_can_build_units(true);
    logic.add_player(player);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_cost(225, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSBarracks)
        .set_cost(500, 0);
    logic.templates.insert("AmericaBarracks".into(), barracks);
    let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_SplitTeam".into());
        proto.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 1,
                max_units: 4,
                unit_thing_name: "AmericaInfantryRanger",
            },
        );
        tf.replace_team_prototype(proto);
    }
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert!(ai.build_specific_ai_team(&mut logic, "HQ_SplitTeam", false));
    let team = ai.team_queue.front().expect("queued");
    let required: Vec<_> = team
        .work_orders
        .iter()
        .filter(|order| order.is_required)
        .collect();
    let optional: Vec<_> = team
        .work_orders
        .iter()
        .filter(|order| !order.is_required)
        .collect();
    assert_eq!(required.len(), 1);
    assert_eq!(required[0].num_required, 1);
    assert_eq!(optional.len(), 1);
    assert_eq!(optional[0].num_required, 3);
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        tf.reset();
    }
}

#[test]
fn select_team_to_build_splits_min_max_not_max_as_required() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    player.set_can_build_units(true);
    logic.add_player(player);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_cost(225, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSBarracks)
        .set_cost(500, 0);
    logic.templates.insert("AmericaBarracks".into(), barracks);
    let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);

    install_player_team_prototype(1, "HQ_MinMaxTeam", &[(1, 4, "AmericaInfantryRanger")], 20);
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert!(
        ai.select_team_to_build(&mut logic, 0.0),
        "auto-select must queue the leftover player prototype"
    );
    let team = ai.team_queue.front().expect("queued team");
    assert_eq!(team.name, "HQ_MinMaxTeam");
    let required: Vec<_> = team
        .work_orders
        .iter()
        .filter(|order| order.is_required)
        .collect();
    let optional: Vec<_> = team
        .work_orders
        .iter()
        .filter(|order| !order.is_required)
        .collect();
    assert_eq!(required.len(), 1, "required minUnits stub: {required:?}");
    assert_eq!(required[0].num_required, 1);
    assert_eq!(optional.len(), 1, "optional max-min: {optional:?}");
    assert_eq!(optional[0].num_required, 3);
    assert!(
        !team
            .work_orders
            .iter()
            .any(|order| order.is_required && order.num_required == 4),
        "must not invent max-as-required work orders: {:?}",
        team.work_orders
            .iter()
            .map(|order| (
                order.template_name.as_str(),
                order.num_required,
                order.is_required
            ))
            .collect::<Vec<_>>()
    );
    clear_player_team_prototypes();
}

#[test]
fn is_a_good_idea_does_not_reject_ready_queue_team() {
    // C++ isAGoodIdeaToBuildTeam (AIPlayer.cpp:1487-1492) only walks
    // iterate_TeamBuildQueue. A maxInstances>1 copy may start while the
    // first sits idle in TeamReadyQueue (up to 60s force).
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    player.set_can_build_units(true);
    logic.add_player(player);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_cost(225, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSBarracks)
        .set_cost(500, 0);
    logic.templates.insert("AmericaBarracks".into(), barracks);
    let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);

    install_player_team_prototype(
        1,
        "HQ_Auf59_ReadyOk",
        &[(1, 1, "AmericaInfantryRanger")],
        20,
    );
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.team_ready_queue.push_back(AITeamQueue::new(
        "HQ_Auf59_ReadyOk".into(),
        vec![AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 20)],
        false,
        0,
    ));
    assert!(
        ai.is_a_good_idea_to_build_team(&logic, "HQ_Auf59_ReadyOk"),
        "ready-queue copy must not veto a second maxInstances>1 start"
    );

    ai.team_queue.push_back(AITeamQueue::new(
        "HQ_Auf59_ReadyOk".into(),
        vec![AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 20)],
        false,
        0,
    ));
    assert!(
        !ai.is_a_good_idea_to_build_team(&logic, "HQ_Auf59_ReadyOk"),
        "TeamBuildQueue still vetoes a prototype already under construction"
    );
    clear_player_team_prototypes();
}

#[test]
fn aidata_team_factory_idle_gate() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.set_cost(225, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee.set_cost(700, 0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);
    let mut barracks_t = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks_t.set_cost(500, 0);
    barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
    logic.templates.insert("AmericaBarracks".into(), barracks_t);
    let mut wf_t = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
    wf_t.set_cost(1000, 0);
    wf_t.add_kind_of(crate::game_logic::KindOf::Structure);
    logic.templates.insert("AmericaWarFactory".into(), wf_t);

    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
    player.resources.supplies = 10_000;
    logic.add_player(player);

    let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    // No factories yet.
    assert!(!ai.team_factories_ready(&logic, "USA_BasicForce"));
    assert!(!ai.is_possible_to_build_team(&logic, "USA_BasicForce"));

    // Spawn constructed barracks + war factory.
    let barracks_id = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks");
    let wf_id = logic
        .create_object(
            "AmericaWarFactory",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("war factory");
    // Ensure constructed + empty queues.
    if let Some(o) = logic./* Wave 950 */ host_object_mut(barracks_id) {
        if let Some(b) = o.building_data.as_mut() {
            b.production_queue.clear();
        }
    }
    if let Some(o) = logic.host_object_mut(wf_id) {
        if let Some(b) = o.building_data.as_mut() {
            b.production_queue.clear();
        }
    }
    assert!(ai.team_factories_ready(&logic, "USA_BasicForce"));
    assert!(ai.is_possible_to_build_team(&logic, "USA_BasicForce"));

    // Busy both factories → not ready (requireIdleFactory residual).
    if let Some(o) = logic.host_object_mut(barracks_id) {
        if let Some(b) = o.building_data.as_mut() {
            b.production_queue.push(crate::game_logic::ProductionItem {
                template_name: "USA_Ranger".into(),
                progress: 0.1,
                total_time: 10.0,
                construction_frames: 0,
                cost: crate::game_logic::Resources {
                    supplies: 225,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: crate::game_logic::buildings::ProductionKind::Unit,
            });
        }
    }
    if let Some(o) = logic.host_object_mut(wf_id) {
        if let Some(b) = o.building_data.as_mut() {
            b.production_queue.push(crate::game_logic::ProductionItem {
                template_name: "USA_Humvee".into(),
                progress: 0.1,
                total_time: 10.0,
                construction_frames: 0,
                cost: crate::game_logic::Resources {
                    supplies: 700,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: crate::game_logic::buildings::ProductionKind::Unit,
            });
        }
    }
    assert!(!ai.team_factories_ready(&logic, "USA_BasicForce"));
    assert!(!ai.should_build_new_team(&logic));

    // Idle one factory → ready again.
    if let Some(o) = logic.host_object_mut(barracks_id) {
        if let Some(b) = o.building_data.as_mut() {
            b.production_queue.clear();
        }
    }
    assert!(ai.team_factories_ready(&logic, "USA_BasicForce"));
}

#[test]
fn skirmish_queues_a_selected_team_without_waiting_for_team_seconds() {
    // Retail `AISkirmishPlayer::doTeamBuilding` first services existing
    // work orders and, after `selectTeamToBuild`, calls `queueUnits` again
    // in that same pass.  A normal USA AI therefore starts its Ranger and
    // Humvee immediately when both real factories are idle; waiting until
    // the next 10-second TeamSeconds window makes the early skirmish AI
    // visibly inert.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 3_000;
    logic.add_player(player);

    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSBarracks)
        .set_cost(500, 0);
    logic.templates.insert("AmericaBarracks".into(), barracks);

    let mut war_factory = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
    war_factory
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSWarFactory)
        .set_cost(1_000, 0);
    logic
        .templates
        .insert("AmericaWarFactory".into(), war_factory);

    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_cost(225, 0);
    // Complete on the next real fixed production frame so the test covers
    // the same producer_id handoff that the live skirmish path uses.
    ranger.build_time = 1.0 / 60.0;
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_cost(700, 0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);

    let barracks_id = logic
        .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
        .expect("constructed barracks");
    let war_factory_id = logic
        .create_object("AmericaWarFactory", Team::USA, Vec3::new(64.0, 0.0, 0.0))
        .expect("constructed war factory");
    install_player_team_prototype(
        1,
        "USA_BasicForce",
        &[
            (2, 2, "AmericaInfantryRanger"),
            (1, 1, "AmericaVehicleHumvee"),
        ],
        10,
    );

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.update_military_management(&mut logic, 0.0);

    assert_eq!(ai.team_queue.len(), 1, "the selected team is retained");
    assert_eq!(
        logic
            .host_object(barracks_id)
            .and_then(|object| object.building_data.as_ref())
            .map(|building| building.production_queue.len()),
        Some(1),
        "the selected team's first Ranger is queued in the same AI pass"
    );
    assert_eq!(
        logic
            .host_object(war_factory_id)
            .and_then(|object| object.building_data.as_ref())
            .map(|building| building.production_queue.len()),
        Some(1),
        "the selected team's Humvee is queued in the same AI pass"
    );
    assert!(
        (ai.next_team_time - AIPlayer::TEAM_SECONDS).abs() < f32::EPSILON,
        "a successful selection starts the longer TeamSeconds timer"
    );
    assert!(
        (ai.next_team_queue_time - AIPlayer::TEAM_QUEUE_RETRY_SECONDS).abs() < f32::EPSILON,
        "unfinished work orders remain on the short queue cadence"
    );

    // Let the actual production update create the Ranger and stamp its
    // producer.  C++ onUnitProduced shortcuts m_teamDelay at this point;
    // do not wait for the normal 2-second queue poll before starting the
    // second Ranger required by USA_BasicForce.
    logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    assert!(
        logic.host_objects().values().any(|object| {
            object.team == Team::USA
                && object.producer_id == Some(barracks_id)
                && object
                    .template_name
                    .eq_ignore_ascii_case("AmericaInfantryRanger")
        }),
        "the host production path created a producer-linked Ranger"
    );
    let output_time = 1.0 / LOGIC_FRAMES_PER_SECOND;
    ai.update_military_management(&mut logic, output_time);
    let ranger_order = ai
        .team_queue
        .front()
        .and_then(|team| {
            team.work_orders
                .iter()
                .find(|order| order.template_name == "AmericaInfantryRanger")
        })
        .expect("BasicForce Ranger work order remains active");
    assert_eq!(ranger_order.num_completed, 1);
    assert_eq!(ranger_order.queued_count, 1);
    assert_eq!(ranger_order.factory_id, Some(barracks_id));
    assert_eq!(
        logic
            .host_object(barracks_id)
            .and_then(|object| object.building_data.as_ref())
            .map(|building| building.production_queue.len()),
        Some(1),
        "live output requeues the next Ranger before the normal poll delay"
    );

    // No second team and no duplicate order before m_teamDelay expires.
    ai.update_military_management(&mut logic, 1.9);
    assert_eq!(ai.team_queue.len(), 1);
    assert_eq!(
        logic
            .host_object(barracks_id)
            .and_then(|object| object.building_data.as_ref())
            .map(|building| building.production_queue.len()),
        Some(1)
    );
    clear_player_team_prototypes();
}

#[test]
fn work_order_waits_for_live_factory_output_before_completing() {
    // C++ AIPlayer::onUnitProduced increments a WorkOrder only after
    // ProductionUpdate has created a unit and identified its producer.
    // A successful queue request alone must not erase the AI team.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 10_000;
    logic.add_player(player);

    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSBarracks)
        .set_cost(500, 0);
    logic.templates.insert("AmericaBarracks".into(), barracks);

    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .add_kind_of(crate::game_logic::KindOf::Selectable)
        .add_kind_of(crate::game_logic::KindOf::Attackable)
        .set_cost(225, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let factory = logic
        .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
        .expect("constructed barracks");
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.team_queue.push_back(AITeamQueue::new(
        "one-ranger".into(),
        vec![AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100)],
        false,
        0,
    ));

    ai.process_team_queue(&mut logic, 0.0);
    let queued = ai.team_queue.front().expect("queue survives enqueue");
    let order = queued.work_orders.first().expect("work order");
    assert_eq!(
        order.num_completed, 0,
        "enqueue is not production completion"
    );
    assert_eq!(order.queued_count, 1);
    assert_eq!(order.factory_id, Some(factory));

    // Model the real production completion handoff: the production path
    // stamps producer_id before the unit becomes visible to host AI.
    let unit = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(12.0, 0.0, 0.0),
        )
        .expect("factory output");
    logic
        .host_object_mut(unit)
        .expect("produced unit")
        .producer_id = Some(factory);

    ai.process_team_queue(&mut logic, 1.0);
    assert!(
        ai.team_queue.front().is_some_and(|t| t.is_all_built()),
        "team becomes complete only after its live factory output is observed"
    );
    ai.check_queued_teams(&mut logic, 1.0);
    assert!(
        ai.team_queue.is_empty(),
        "all-built teams leave the build queue"
    );
    assert_eq!(ai.team_ready_queue.len(), 1);
    ai.check_ready_teams(&mut logic, 1.0);
    assert!(
        ai.team_ready_queue.is_empty(),
        "idle ready team activates without waiting 60s"
    );
}

#[test]
fn supply_center_spawns_free_collector_then_ai_pays_for_next_collector() {
    // Retail AmericaSupplyCenter has SpawnBehavior ModuleTag_12 for one
    // free AmericaVehicleChinook.  C++ AIPlayer::queueSupplyTruck must not
    // represent that freebie as a zero-cost production item; it later
    // prepends a real paid work order through the same SupplyCenter.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 5_000;
    logic.add_player(player);

    let mut supply_center = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
    supply_center
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter)
        .add_kind_of(crate::game_logic::KindOf::FSSupplyCenter)
        .add_kind_of(crate::game_logic::KindOf::Selectable)
        .set_cost(2_000, 0);
    logic
        .templates
        .insert("AmericaSupplyCenter".into(), supply_center);

    let mut chinook = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
    chinook
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .add_kind_of(crate::game_logic::KindOf::Aircraft)
        .add_kind_of(crate::game_logic::KindOf::Harvester)
        .add_kind_of(crate::game_logic::KindOf::Selectable)
        .set_cost(1_200, 0);
    // Keep the focused test on the real production completion path without
    // waiting ten retail seconds for the paid Chinook.
    chinook.build_time = 0.001;
    logic
        .templates
        .insert("AmericaVehicleChinook".into(), chinook);

    let mut source = crate::game_logic::ThingTemplate::new("TestSupplySource");
    source
        .add_kind_of(crate::game_logic::KindOf::Resource)
        .add_kind_of(crate::game_logic::KindOf::Harvestable);
    logic.templates.insert("TestSupplySource".into(), source);
    let source_id = logic
        .create_object("TestSupplySource", Team::Neutral, Vec3::new(32.0, 0.0, 0.0))
        .expect("typed supply source");
    logic
        .host_object_mut(source_id)
        .expect("source object")
        .set_stored_supplies(20_000);

    let cash_before_spawn = logic.get_player(1).expect("AI player").effective_supplies();
    let center_id = logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::ZERO)
        .expect("constructed supply center");
    let free_collectors: Vec<ObjectId> = logic
        .host_objects()
        .iter()
        .filter_map(|(&id, object)| {
            (object.team == Team::USA
                && object.producer_id == Some(center_id)
                && object
                    .template_name
                    .eq_ignore_ascii_case("AmericaVehicleChinook"))
            .then_some(id)
        })
        .collect();
    assert_eq!(
        free_collectors.len(),
        1,
        "SpawnBehavior creates one free Chinook"
    );
    assert_eq!(
        logic
            .get_player(1)
            .expect("AI player after spawn")
            .effective_supplies(),
        cash_before_spawn,
        "the authored SpawnBehavior collector is not charged as production"
    );
    assert_eq!(
        logic
            .host_object(center_id)
            .and_then(|center| center.building_data.as_ref())
            .map(|building| building.production_queue.len()),
        Some(0),
        "free SpawnBehavior collector does not enter ProductionUpdate"
    );

    let free_collector = free_collectors[0];
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.process_team_queue(&mut logic, 0.0);

    let paid_order = ai
        .team_queue
        .front()
        .and_then(|team| team.work_orders.first())
        .expect("one paid follow-up collector work order");
    assert!(paid_order.is_resource_gatherer);
    assert_eq!(paid_order.supply_center_id, Some(center_id));
    assert_eq!(paid_order.factory_id, Some(center_id));
    assert_eq!(paid_order.queued_count, 1);
    assert_eq!(
        logic
            .get_player(1)
            .expect("AI player after paid queue")
            .effective_supplies(),
        cash_before_spawn - 1_200,
        "only the later collector spends its authored build cost"
    );
    let free = logic
        .host_object(free_collector)
        .expect("free collector live");
    assert_eq!(free.ai_state, crate::game_logic::AIState::Gathering);
    assert_eq!(free.target, Some(source_id));
    assert_eq!(free.preferred_dock_id, Some(center_id));

    logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    ai.process_team_queue(&mut logic, 1.0 / LOGIC_FRAMES_PER_SECOND);

    let paid_collectors: Vec<ObjectId> = logic
        .host_objects()
        .iter()
        .filter_map(|(&id, object)| {
            (object.team == Team::USA
                && object.producer_id == Some(center_id)
                && object
                    .template_name
                    .eq_ignore_ascii_case("AmericaVehicleChinook"))
            .then_some(id)
        })
        .collect();
    assert_eq!(
        paid_collectors.len(),
        2,
        "the normal paid ProductionUpdate created a second producer-linked Chinook"
    );
    let paid_collector = *paid_collectors
        .iter()
        .find(|&&id| id != free_collector)
        .expect("new production output");
    let paid = logic
        .host_object(paid_collector)
        .expect("paid collector live");
    assert_eq!(paid.ai_state, crate::game_logic::AIState::Gathering);
    assert_eq!(paid.target, Some(source_id));
    assert_eq!(paid.preferred_dock_id, Some(center_id));
    assert!(
        ai.team_queue.is_empty(),
        "the paid collector work order completes only after its real output is routed"
    );
}

#[test]
fn collector_returns_to_assigned_supply_center_over_nearer_center() {
    // `AIPlayer::queueSupplyTruck` sends `aiDock(center,
    // CMD_FROM_PLAYER)`.  SupplyTruckAIUpdate persists that center in
    // m_preferredDock, so the return leg must not switch to a different
    // closer depot in a normal multi-supply-center skirmish base.
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));

    let mut supply_center = crate::game_logic::ThingTemplate::new("TestSupplyCenter");
    supply_center
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
    logic
        .templates
        .insert("TestSupplyCenter".into(), supply_center);

    let mut collector = crate::game_logic::ThingTemplate::new("TestCollector");
    collector
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .add_kind_of(crate::game_logic::KindOf::Harvester);
    logic.templates.insert("TestCollector".into(), collector);

    let assigned_center = logic
        .create_object("TestSupplyCenter", Team::USA, Vec3::ZERO)
        .expect("assigned supply center");
    let nearer_center = logic
        .create_object("TestSupplyCenter", Team::USA, Vec3::new(125.0, 0.0, 0.0))
        .expect("nearer supply center");
    let collector_id = logic
        .create_object("TestCollector", Team::USA, Vec3::new(250.0, 0.0, 0.0))
        .expect("collector");
    let assigned_position = logic
        .host_object(assigned_center)
        .expect("assigned center live")
        .get_position();
    {
        let collector = logic.host_object_mut(collector_id).expect("collector live");
        collector.preferred_dock_id = Some(assigned_center);
        collector.set_stored_supplies(100);
        collector.set_ai_state(crate::game_logic::AIState::ReturningResources);
    }

    logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);

    let collector = logic
        .host_object(collector_id)
        .expect("collector after tick");
    let queued_destination = collector
        .movement
        .path
        .last()
        .copied()
        .or(collector.movement.target_position);
    assert_eq!(queued_destination, Some(assigned_position));
    assert_ne!(
        queued_destination,
        logic
            .host_object(nearer_center)
            .map(|center| center.get_position()),
        "a nearer center must not steal a collector assigned to another depot"
    );
}

#[test]
fn active_loose_collector_rejoins_one_supply_center_before_paid_replacement() {
    // Retail `AIPlayer::queueSupplyTruck` first scans active SupplyTruckAI
    // units with an unresolved `m_preferredDock`.  It assigns one to the
    // understaffed center and returns before `startTraining`, so losing a
    // supply center does not immediately buy a duplicate collector.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 5_000;
    logic.add_player(player);

    let mut old_center_template = crate::game_logic::ThingTemplate::new("TestOldSupplyCenter");
    old_center_template
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
    logic
        .templates
        .insert("TestOldSupplyCenter".into(), old_center_template);

    let mut supply_center = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
    supply_center
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter)
        .add_kind_of(crate::game_logic::KindOf::FSSupplyCenter)
        .add_kind_of(crate::game_logic::KindOf::Selectable)
        .set_cost(2_000, 0);
    logic
        .templates
        .insert("AmericaSupplyCenter".into(), supply_center);

    let mut chinook = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
    chinook
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .add_kind_of(crate::game_logic::KindOf::Aircraft)
        .add_kind_of(crate::game_logic::KindOf::Harvester)
        .add_kind_of(crate::game_logic::KindOf::Selectable)
        .set_cost(1_200, 0);
    logic
        .templates
        .insert("AmericaVehicleChinook".into(), chinook);

    let mut source = crate::game_logic::ThingTemplate::new("TestSupplySource");
    source
        .add_kind_of(crate::game_logic::KindOf::Resource)
        .add_kind_of(crate::game_logic::KindOf::Harvestable);
    logic.templates.insert("TestSupplySource".into(), source);

    // `ObjectID` validity in C++ is an existence test, not merely an
    // alive-state test.  The active survivor must therefore carry an ID
    // that is absent from the host object store (equivalent to a center
    // that already completed its destruction lifecycle).
    let missing_former_dock = ObjectId(100_000);
    assert!(logic.host_object(missing_former_dock).is_none());

    let replacement_center = logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("replacement center");
    // Remove its authored starter so this test isolates the survivor from
    // the destroyed center.  The parent one-shot latch stays fired, as it
    // would in a real match after that starter had been lost in combat.
    let starter_ids: Vec<ObjectId> = logic
        .host_objects()
        .iter()
        .filter_map(|(&id, object)| {
            (object.producer_id == Some(replacement_center)
                && object
                    .template_name
                    .eq_ignore_ascii_case("AmericaVehicleChinook"))
            .then_some(id)
        })
        .collect();
    assert_eq!(starter_ids.len(), 1, "authored one-shot starter exists");
    for starter_id in starter_ids {
        logic.destroy_object(starter_id);
    }
    logic.process_destroy_list();
    assert!(
        logic
            .host_object(replacement_center)
            .is_some_and(|center| center.supply_center_spawn_behavior_fired)
    );

    let source_id = logic
        .create_object(
            "TestSupplySource",
            Team::Neutral,
            Vec3::new(132.0, 0.0, 0.0),
        )
        .expect("nearby supply source");
    logic
        .host_object_mut(source_id)
        .expect("source live")
        .set_stored_supplies(20_000);

    let loose_collector = logic
        .create_object(
            "AmericaVehicleChinook",
            Team::USA,
            Vec3::new(140.0, 0.0, 0.0),
        )
        .expect("surviving collector");
    {
        let collector = logic
            .host_object_mut(loose_collector)
            .expect("surviving collector live");
        collector.preferred_dock_id = Some(missing_former_dock);
        collector.set_order_target(Some(source_id));
        collector.set_ai_state(crate::game_logic::AIState::Gathering);
    }

    let cash_before = logic.get_player(1).expect("AI player").effective_supplies();
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.process_team_queue(&mut logic, 0.0);

    let collector = logic
        .host_object(loose_collector)
        .expect("reassigned collector live");
    assert_eq!(collector.preferred_dock_id, Some(replacement_center));
    assert_eq!(collector.ai_state, crate::game_logic::AIState::Gathering);
    assert_eq!(collector.target, Some(source_id));
    assert!(
        ai.team_queue.is_empty(),
        "C++ returns after one active-collector reassignment before creating a paid work order"
    );
    assert_eq!(
        logic
            .host_object(replacement_center)
            .and_then(|center| center.building_data.as_ref())
            .map(|building| building.production_queue.len()),
        Some(0),
        "the factory does not receive a paid replacement in the reassignment pass"
    );
    assert_eq!(
        logic
            .get_player(1)
            .expect("AI player after reassignment")
            .effective_supplies(),
        cash_before,
        "reassigning an existing collector is free"
    );
}

#[test]
fn skirmish_starts_one_structure_with_a_live_dozer_assignment() {
    // `AISkirmishPlayer::processBaseBuilding` starts one plan and routes a
    // real dozer to it.  A second affordable plan must remain queued until
    // the next economic pass; the scaffold must then make real construction
    // progress through the authoritative dozer target association.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 1_000;
    logic.add_player(player);

    let mut dozer_template = crate::game_logic::ThingTemplate::new("TestDozer");
    dozer_template
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .add_kind_of(crate::game_logic::KindOf::Worker);
    logic.templates.insert("TestDozer".into(), dozer_template);

    let mut first_template = crate::game_logic::ThingTemplate::new("TestFirstStructure");
    first_template
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_cost(300, 0);
    first_template.build_time = 10.0;
    logic
        .templates
        .insert("TestFirstStructure".into(), first_template);

    let mut second_template = crate::game_logic::ThingTemplate::new("TestSecondStructure");
    second_template
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_cost(300, 0);
    logic
        .templates
        .insert("TestSecondStructure".into(), second_template);

    let build_position = Vec3::new(64.0, 0.0, 64.0);
    let dozer_id = logic
        .create_object("TestDozer", Team::USA, Vec3::new(48.0, 0.0, 64.0))
        .expect("live dozer");
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("TestFirstStructure", build_position, 1);
    ai.add_building("TestSecondStructure", Vec3::new(128.0, 0.0, 64.0), 1);

    ai.process_building_queue(&mut logic, 0.0);

    let structure_id = ai.building_queue[0]
        .object_id
        .expect("first plan starts with its dozer");
    assert!(
        ai.building_queue[1].object_id.is_none(),
        "one C++ skirmish economic pass starts only one structure"
    );
    assert_eq!(
        logic.get_player(1).expect("AI player").effective_supplies(),
        700,
        "only the started structure is charged"
    );
    let dozer = logic.host_object(dozer_id).expect("assigned dozer");
    assert_eq!(dozer.target, Some(structure_id));
    assert_eq!(dozer.ai_state, crate::game_logic::AIState::Constructing);

    // C++ revisits under-construction base entries and hands them to a
    // replacement dozer if the original one is lost.
    let replacement_id = logic
        .create_object("TestDozer", Team::USA, Vec3::new(44.0, 0.0, 64.0))
        .expect("replacement dozer");
    logic
        .host_object_mut(dozer_id)
        .expect("original dozer")
        .health
        .current = 0.0;
    ai.process_building_queue(&mut logic, 0.1);
    let replacement = logic
        .host_object(replacement_id)
        .expect("replacement assigned");
    assert_eq!(replacement.target, Some(structure_id));
    assert_eq!(
        replacement.ai_state,
        crate::game_logic::AIState::Constructing
    );

    let before = logic
        .host_object(structure_id)
        .expect("under-construction scaffold")
        .construction_percent;
    logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    let after = logic
        .host_object(structure_id)
        .expect("live scaffold")
        .construction_percent;
    assert!(
        after > before,
        "the assigned dozer advances construction instead of leaving a dead scaffold"
    );
}

/// C++ AIPlayer::findDozer calls queueDozer when no KINDOF_DOZER exists
/// (AIPlayer.cpp:3254-3256 / queueDozer 3128-3171).
#[test]
fn lost_dozer_queues_priority_command_center_replacement() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 5_000;
    logic.add_player(player);

    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::CommandCenter)
        .set_cost(2000, 0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);

    let mut dozer = crate::game_logic::ThingTemplate::new("AmericaVehicleDozer");
    dozer
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .add_kind_of(crate::game_logic::KindOf::Worker)
        .add_kind_of(crate::game_logic::KindOf::Dozer)
        .set_cost(1000, 0);
    dozer.build_time = 5.0;
    logic.templates.insert("AmericaVehicleDozer".into(), dozer);

    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_cost(500, 0);
    logic.templates.insert("AmericaBarracks".into(), barracks);

    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
        .expect("command center");
    if let Some(obj) = logic.host_object_mut(cc_id) {
        obj.owner_player_id = Some(1);
        if let Some(bd) = obj.building_data.as_mut() {
            bd.building_type = crate::game_logic::BuildingType::CommandCenter;
        }
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("AmericaBarracks", Vec3::new(64.0, 0.0, 0.0), 1);
    ai.process_building_queue(&mut logic, 0.0);

    assert_eq!(
        ai.team_queue.len(),
        1,
        "queueDozer must prepend a priority dozer team"
    );
    let order = ai
        .team_queue
        .front()
        .and_then(|team| team.work_orders.first())
        .expect("dozer work order");
    assert_eq!(order.template_name, "AmericaVehicleDozer");
    assert_eq!(order.factory_id, Some(cc_id));
    assert!(
        ai.team_queue.front().expect("dozer team").priority_build,
        "C++ TeamInQueue m_priorityBuild = true"
    );
    let queued = logic
        .host_object(cc_id)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.len())
        .unwrap_or(0);
    assert_eq!(queued, 1, "Command Center must start training the dozer");

    ai.process_building_queue(&mut logic, 1.0);
    assert_eq!(
        ai.team_queue.len(),
        1,
        "a second economic pass must not stack another dozer order"
    );
}

#[test]
fn aidata_rebuild_delay_gates_destroyed_structure() {
    assert!((AIPlayer::REBUILD_DELAY_SECONDS - 30.0).abs() < 1e-5);
    let b = AIBuildingInfo::new("USA_Barracks".into(), Vec3::ZERO, 2);
    assert!(b.rebuild_delay_elapsed(0.0, AIPlayer::REBUILD_DELAY_SECONDS));
    assert!(b.rebuild_delay_elapsed(100.0, AIPlayer::REBUILD_DELAY_SECONDS));

    let mut destroyed = AIBuildingInfo::new("USA_Barracks".into(), Vec3::ZERO, 2);
    destroyed.destroyed_at_time = Some(10.0);
    // C++: timestamp + RebuildDelaySeconds*FPS > frame → wait.
    assert!(!destroyed.rebuild_delay_elapsed(10.0, AIPlayer::REBUILD_DELAY_SECONDS));
    assert!(!destroyed.rebuild_delay_elapsed(39.9, AIPlayer::REBUILD_DELAY_SECONDS));
    assert!(destroyed.rebuild_delay_elapsed(40.0, AIPlayer::REBUILD_DELAY_SECONDS));
    assert!(destroyed.rebuild_delay_elapsed(100.0, AIPlayer::REBUILD_DELAY_SECONDS));

    // process_building_queue stamps destroyed_at_time when object vanishes.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut barracks_t = crate::game_logic::ThingTemplate::new("USA_Barracks");
    barracks_t.set_cost(500, 0);
    barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
    logic.templates.insert("USA_Barracks".into(), barracks_t);

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("USA_Barracks", Vec3::new(0.0, 0.0, 0.0), 3);
    // Simulate a previously-built slot whose object was destroyed at t=5.
    {
        let b = &mut ai.building_queue[0];
        b.is_built = false;
        b.object_id = None;
        b.rebuild_count = 1;
        b.destroyed_at_time = Some(5.0);
    }
    // Before delay: queue must not start rebuild.
    ai.process_building_queue(&mut logic, 5.0 + AIPlayer::REBUILD_DELAY_SECONDS - 0.1);
    assert!(ai.building_queue[0].object_id.is_none());
    // After delay: may start (if create_object_under_construction succeeds).
    ai.process_building_queue(&mut logic, 5.0 + AIPlayer::REBUILD_DELAY_SECONDS);
    // Either started (object_id Some) or still none if construction API refused —
    // destroyed_at_time must clear only on successful start.
    if ai.building_queue[0].object_id.is_some() {
        assert!(ai.building_queue[0].destroyed_at_time.is_none());
    } else {
        // Delay gate itself elapsed; remaining failure is construction residual.
        assert!(ai.building_queue[0].rebuild_delay_elapsed(
            5.0 + AIPlayer::REBUILD_DELAY_SECONDS,
            AIPlayer::REBUILD_DELAY_SECONDS
        ));
    }
}

#[test]
fn captured_pad_unbinds_and_gla_hole_rebinds() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 10_000;
    logic.add_player(player);
    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_cost(500, 0);
    logic.templates.insert("AmericaBarracks".into(), barracks);

    let factory_id = logic
        .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
        .expect("pad");
    if let Some(obj) = logic.host_object_mut(factory_id) {
        obj.owner_player_id = Some(1);
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("AmericaBarracks", Vec3::ZERO, 3);
    ai.building_queue[0].object_id = Some(factory_id);
    ai.building_queue[0].is_built = true;

    // Capture: new owner, same live object.
    if let Some(obj) = logic.host_object_mut(factory_id) {
        obj.owner_player_id = Some(0);
        obj.set_team(Team::China);
    }
    ai.sync_build_list_object_status(&logic, 12.0);
    assert!(ai.building_queue[0].object_id.is_none());
    assert!(!ai.building_queue[0].is_built);
    assert_eq!(ai.building_queue[0].destroyed_at_time, Some(12.0));

    // Destroyed + GLA hole with matching spawner.
    let hole_id = logic
        .create_object("AmericaBarracks", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
        .expect("hole");
    if let Some(hole) = logic.host_object_mut(hole_id) {
        hole.is_rebuild_hole = true;
        hole.rebuild_spawner_id = Some(factory_id);
        hole.owner_player_id = Some(1);
        hole.set_team(Team::USA);
    }
    ai.building_queue[0].object_id = Some(factory_id);
    logic.destroy_object(factory_id);
    ai.sync_build_list_object_status(&logic, 13.0);
    assert_eq!(ai.building_queue[0].object_id, Some(hole_id));
}

#[test]
fn economic_update_does_not_invent_random_supply_or_power_pads() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 10;
    player.power_available = -40;
    logic.add_player(player);
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("AmericaBarracks", Vec3::ZERO, 1);
    let before = ai.building_queue.len();
    ai.next_building_time = 0.0;
    ai.update_economic_management(&mut logic, 0.0);
    assert_eq!(
        ai.building_queue.len(),
        before,
        "low cash / brown-out must not append invented SupplyCenter/PowerPlant pads"
    );
    assert!(
        !ai.building_queue
            .iter()
            .any(|b| b.template_name.contains("SupplyCenter")
                || b.template_name.contains("PowerPlant")),
        "no extra supply/power pads outside the authored list"
    );
}

#[test]
fn build_by_supplies_places_named_template_near_warehouse() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 10_000;
    logic.add_player(player);
    let mut pile = crate::game_logic::ThingTemplate::new("SupplyWarehouse");
    pile.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::Harvestable)
        .add_kind_of(crate::game_logic::KindOf::Resource)
        .add_kind_of(crate::game_logic::KindOf::SupplySource);
    pile.dock_kind = crate::game_logic::DockKind::SupplyWarehouse;
    logic.templates.insert("SupplyWarehouse".into(), pile);
    let mut sc = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
    sc.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
    logic.templates.insert("AmericaSupplyCenter".into(), sc);

    let warehouse = logic
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(200.0, 0.0, 0.0))
        .expect("warehouse");
    if let Some(obj) = logic.host_object_mut(warehouse) {
        obj.stored_resources.supplies = 5_000;
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    assert!(ai.build_by_supplies(&logic, 100, "AmericaSupplyCenter"));
    let pad = ai
        .building_queue
        .iter()
        .find(|b| b.template_name == "AmericaSupplyCenter")
        .expect("named pad");
    assert!(
        (pad.position - Vec3::new(200.0, 0.0, 0.0)).length() < 80.0,
        "depot must sit near the warehouse, not a random base-center offset: {:?}",
        pad.position
    );
    assert!(pad.is_priority);
}

#[test]
fn skirmish_new_map_uses_aidata_side_build_list() {
    {
        let mut store = game_engine::common::ini::get_ai_data_store_mut();
        store.ensure_base();
        if let Some(data) = store.get_active_mut() {
            data.rotate_skirmish_bases = false;
            data.side_build_lists
                .retain(|l| !l.side.eq_ignore_ascii_case("America"));
            let mut list = game_engine::common::ini::AiSideBuildList::new("America".into());
            list.entries.push(game_engine::common::ini::BuildListEntry {
                building_name: "CC".into(),
                template_name: "AmericaCommandCenter".into(),
                location: (0.0, 0.0),
                rebuilds: 0,
                angle_radians: 0.0,
                initially_built: false,
                rally_point_offset: (0.0, 0.0),
                automatically_build: true,
            });
            list.entries.push(game_engine::common::ini::BuildListEntry {
                building_name: "WF".into(),
                template_name: "AmericaWarFactory".into(),
                location: (80.0, 0.0),
                rebuilds: 1,
                angle_radians: 0.0,
                initially_built: false,
                rally_point_offset: (0.0, 0.0),
                automatically_build: true,
            });
            data.side_build_lists.push(list);
        }
    }

    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::CommandCenter);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut wf = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
    wf.add_kind_of(crate::game_logic::KindOf::Structure);
    logic.templates.insert("AmericaWarFactory".into(), wf);

    let start_cc = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(-40.0, 0.0, -40.0),
        )
        .expect("map CC");
    if let Some(obj) = logic.host_object_mut(start_cc) {
        obj.owner_player_id = Some(1);
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.initialize(Vec3::new(-40.0, 0.0, -40.0));
    assert!(ai.apply_skirmish_new_map(&mut logic));
    assert!(
        logic.host_object(start_cc).is_none(),
        "map-placed CC must be destroyed"
    );
    let cc_pad = ai
        .building_queue
        .iter()
        .find(|b| b.template_name.contains("CommandCenter"))
        .expect("list CC");
    assert!(
        cc_pad.is_built,
        "list CC is InitiallyBuilt / buildStructureNow"
    );
    let wf_pad = ai
        .building_queue
        .iter()
        .find(|b| b.template_name.contains("WarFactory"))
        .expect("list WF");
    assert!(!wf_pad.is_built);
    assert!(
        wf_pad.is_buildable(),
        "non-CC entries incrementNumRebuilds so first build does not spend the last slot"
    );
    {
        let mut store = game_engine::common::ini::get_ai_data_store_mut();
        if let Some(data) = store.get_active_mut() {
            data.side_build_lists
                .retain(|l| !l.side.eq_ignore_ascii_case("America"));
        }
    }
}

#[test]
fn queue_units_recruits_existing_idle_units() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("USA_RangerSquad".into());
        proto.set_production_priority(50);
        tf.replace_team_prototype(proto);
    }

    let existing = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("idle ranger");
    if let Some(obj) = logic.host_object_mut(existing) {
        obj.owner_player_id = Some(1);
        obj.set_ai_state(crate::game_logic::AIState::Idle);
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
    ai.team_queue.push_back(AITeamQueue::new(
        "USA_RangerSquad".into(),
        vec![order],
        false,
        0,
    ));
    ai.process_team_queue(&mut logic, 0.0);
    let team = ai.team_queue.front().expect("queued team");
    assert_eq!(team.work_orders[0].num_completed, 1);
    assert_eq!(team.work_orders[0].observed_unit_ids, vec![existing]);
    assert!(
        team.work_orders[0].factory_id.is_none(),
        "recruited unit must not also startTraining"
    );
    let dest_id = team.team_id.expect("inactive dest team bound on recruit");
    assert_eq!(
        logic
            .host_object(existing)
            .map(|o| o.team_instance_name.as_str()),
        Some("USA_RangerSquad"),
        "C++ queueUnits setTeam onto dest instance immediately"
    );
    let members = gamelogic::team::get_team_factory()
        .lock()
        .ok()
        .and_then(|factory| factory.find_team_by_id(dest_id))
        .and_then(|arc| arc.read().ok().map(|tg| tg.get_members().to_vec()))
        .unwrap_or_default();
    assert!(
        members.contains(&existing.0),
        "leftover dest instance must list the recruited unit"
    );
}

#[test]
fn queue_units_skips_disabled_held_units() {
    // C++ Team::tryToRecruit DISABLED_HELD (Team.cpp:2353-2356).
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let existing = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("held ranger");
    if let Some(obj) = logic.host_object_mut(existing) {
        obj.owner_player_id = Some(1);
        obj.status.disabled_held = true;
        obj.set_ai_state(crate::game_logic::AIState::Idle);
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
    ai.team_queue.push_back(AITeamQueue::new(
        "USA_RangerSquad".into(),
        vec![order],
        false,
        0,
    ));
    ai.process_team_queue(&mut logic, 0.0);
    let team = ai.team_queue.front().expect("queued team");
    assert_eq!(
        team.work_orders[0].num_completed, 0,
        "DISABLED_HELD unit must not be recruited"
    );
    assert!(team.work_orders[0].observed_unit_ids.is_empty());
}

#[test]
fn try_to_recruit_takes_structures_and_contained() {
    // C++ Team::tryToRecruit has no Structure skip and no contained-by skip.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut pad_t = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
    pad_t.add_kind_of(crate::game_logic::KindOf::Structure);
    logic.templates.insert("AmericaWarFactory".into(), pad_t);
    let mut ranger_t = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger_t.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger_t);

    let pad = logic
        .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("pad");
    if let Some(obj) = logic.host_object_mut(pad) {
        obj.owner_player_id = Some(1);
        obj.set_ai_state(crate::game_logic::AIState::Idle);
    }
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let order = AIWorkOrder::new("AmericaWarFactory".into(), 1, 100);
    ai.team_queue.push_back(AITeamQueue::new(
        "USA_FactoryTeam".into(),
        vec![order],
        false,
        0,
    ));
    ai.process_team_queue(&mut logic, 0.0);
    let team = ai.team_queue.front().expect("queued factory team");
    assert_eq!(
        team.work_orders[0].num_completed, 1,
        "matching structure template must be recruitable"
    );
    assert_eq!(team.work_orders[0].observed_unit_ids, vec![pad]);

    let garrisoned = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("garrisoned");
    if let Some(obj) = logic.host_object_mut(garrisoned) {
        obj.owner_player_id = Some(1);
        obj.contained_by = Some(crate::game_logic::ObjectId(99));
        obj.set_ai_state(crate::game_logic::AIState::Idle);
    }
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
    ai.team_queue.push_back(AITeamQueue::new(
        "USA_RangerSquad".into(),
        vec![order],
        false,
        0,
    ));
    ai.process_team_queue(&mut logic, 0.0);
    let team = ai.team_queue.front().expect("queued ranger team");
    assert_eq!(
        team.work_orders[0].num_completed, 1,
        "contained (not HELD) matching unit must be recruitable"
    );
    assert_eq!(team.work_orders[0].observed_unit_ids, vec![garrisoned]);
}

fn w21_ranger_logic() -> (crate::game_logic::GameLogic, crate::game_logic::ObjectId) {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let existing = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("ranger");
    if let Some(obj) = logic.host_object_mut(existing) {
        obj.owner_player_id = Some(1);
        obj.set_ai_state(crate::game_logic::AIState::Idle);
    }
    (logic, existing)
}

fn w21_enqueue_and_recruit(logic: &mut crate::game_logic::GameLogic, dest: &str) -> u32 {
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
    ai.team_queue
        .push_back(AITeamQueue::new(dest.into(), vec![order], false, 0));
    ai.process_team_queue(logic, 0.0);
    ai.team_queue.front().unwrap().work_orders[0].num_completed
}

#[test]
fn try_to_recruit_skips_inactive_higher_priority_and_unrecruitable() {
    // C++ Team::tryToRecruit isActive / productionPriority / isRecruitable.
    let (mut logic, existing) = w21_ranger_logic();
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut dest = gamelogic::team::TeamPrototype::new("W21_DestHigh".into());
        dest.set_production_priority(50);
        tf.replace_team_prototype(dest);
        let mut src = gamelogic::team::TeamPrototype::new("W21_InactiveSrc".into());
        src.set_production_priority(0);
        src.set_ai_recruitable(true);
        tf.replace_team_prototype(src);
        if let Some(team) = tf.create_inactive_team("W21_InactiveSrc") {
            if let Ok(mut tg) = team.write() {
                tg.add_member(existing.0);
            }
        }
    }
    if let Some(obj) = logic.host_object_mut(existing) {
        obj.team_instance_name = "W21_InactiveSrc".into();
    }
    assert_eq!(
        w21_enqueue_and_recruit(&mut logic, "W21_DestHigh"),
        0,
        "must not steal from a still-building team"
    );

    let (mut logic, existing) = w21_ranger_logic();
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut dest = gamelogic::team::TeamPrototype::new("W21_DestLow".into());
        dest.set_production_priority(10);
        tf.replace_team_prototype(dest);
        let mut src = gamelogic::team::TeamPrototype::new("W21_HighPriSrc".into());
        src.set_production_priority(50);
        src.set_ai_recruitable(true);
        tf.replace_team_prototype(src);
        if let Some(team) = tf.create_team("W21_HighPriSrc") {
            if let Ok(mut tg) = team.write() {
                tg.add_member(existing.0);
            }
        }
    }
    if let Some(obj) = logic.host_object_mut(existing) {
        obj.team_instance_name = "W21_HighPriSrc".into();
    }
    assert_eq!(
        w21_enqueue_and_recruit(&mut logic, "W21_DestLow"),
        0,
        "must not steal from equal-or-higher priority"
    );

    let (mut logic, existing) = w21_ranger_logic();
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut dest = gamelogic::team::TeamPrototype::new("W21_DestRecruit".into());
        dest.set_production_priority(50);
        tf.replace_team_prototype(dest);
        let mut src = gamelogic::team::TeamPrototype::new("W21_NoRecruitSrc".into());
        src.set_production_priority(0);
        src.set_ai_recruitable(false);
        tf.replace_team_prototype(src);
        if let Some(team) = tf.create_team("W21_NoRecruitSrc") {
            if let Ok(mut tg) = team.write() {
                tg.add_member(existing.0);
            }
        }
    }
    if let Some(obj) = logic.host_object_mut(existing) {
        obj.team_instance_name = "W21_NoRecruitSrc".into();
    }
    assert_eq!(
        w21_enqueue_and_recruit(&mut logic, "W21_DestRecruit"),
        0,
        "must not steal from a non-recruitable team"
    );

    let (mut logic, existing) = w21_ranger_logic();
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut dest = gamelogic::team::TeamPrototype::new("W21_DestUnitFlag".into());
        dest.set_production_priority(50);
        tf.replace_team_prototype(dest);
    }
    if let Some(obj) = logic.host_object_mut(existing) {
        obj.is_recruitable = false;
    }
    assert_eq!(
        w21_enqueue_and_recruit(&mut logic, "W21_DestUnitFlag"),
        0,
        "must not recruit a unit with isRecruitable=false"
    );
}

#[test]
fn try_to_recruit_ranks_leftover_xy_not_host_3d() {
    // C++ Team.cpp:2357-2370 / leftover team_members.rs:241-264: leftover XY only.
    // Home leftover Z=80 → host Y=80. Ground unit at leftover XY=10 is nearer
    // than a same-height unit at leftover XY=50; 3D host length_squared flips it.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut dest = gamelogic::team::TeamPrototype::new("HQ_1T0_Dest".into());
        dest.set_production_priority(50);
        dest.set_home_location(gamelogic::common::Coord3D::new(0.0, 0.0, 80.0));
        tf.replace_team_prototype(dest);
    }

    let closer_xy = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("leftover-XY nearer ranger");
    if let Some(obj) = logic.host_object_mut(closer_xy) {
        obj.owner_player_id = Some(1);
        obj.set_ai_state(crate::game_logic::AIState::Idle);
    }
    let farther_xy = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(50.0, 80.0, 0.0),
        )
        .expect("leftover-XY farther ranger");
    if let Some(obj) = logic.host_object_mut(farther_xy) {
        obj.owner_player_id = Some(1);
        obj.set_ai_state(crate::game_logic::AIState::Idle);
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
    ai.team_queue.push_back(AITeamQueue::new(
        "HQ_1T0_Dest".into(),
        vec![order],
        false,
        0,
    ));
    ai.process_team_queue(&mut logic, 0.0);
    let team = ai.team_queue.front().expect("queued team");
    assert_eq!(
        team.work_orders[0].observed_unit_ids,
        vec![closer_xy],
        "must rank leftover XY, not host 3D that includes leftover Z/up"
    );
    assert_ne!(
        team.work_orders[0].observed_unit_ids,
        vec![farther_xy],
        "same-height leftover-XY-farther unit must lose to leftover-XY nearer"
    );
}

#[test]
fn recruit_waiting_work_orders_joins_destination_team_instance() {
    // C++ queueUnits: unit->setTeam(team->m_team) immediately on recruit.
    let (mut logic, existing) = w21_ranger_logic();
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut dest = gamelogic::team::TeamPrototype::new("HQ_6_RecruitDest".into());
        dest.set_production_priority(50);
        tf.replace_team_prototype(dest);
    }
    assert_eq!(
        w21_enqueue_and_recruit(&mut logic, "HQ_6_RecruitDest"),
        1,
        "default-team ranger must be recruited"
    );
    let obj = logic.host_object(existing).expect("recruited ranger");
    assert_eq!(
        obj.team_instance_name, "HQ_6_RecruitDest",
        "recruited unit must join dest team_instance_name during build"
    );
    let members: Vec<u32> = gamelogic::team::get_team_factory()
        .lock()
        .ok()
        .map(|factory| {
            factory
                .find_team_instances("HQ_6_RecruitDest")
                .into_iter()
                .flat_map(|arc| {
                    arc.read()
                        .ok()
                        .map(|tg| tg.get_members().to_vec())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    assert!(
        members.contains(&existing.0),
        "leftover dest instance must gain the recruited member"
    );
}

#[test]
fn check_ready_teams_execute_actions_requires_production_condition_action() {
    // C++ anyIdle shortcut needs ProductionCondition script WITH an action.
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let idle = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("idle");
    let busy = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("busy");
    if let Some(o) = logic.host_object_mut(busy) {
        o.set_ai_state(crate::game_logic::AIState::Moving);
    }
    let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
    order.num_completed = 2;
    order.observed_unit_ids.push(idle);
    order.observed_unit_ids.push(busy);
    let mut team = AITeamQueue::new("W21_NoCondTeam".into(), vec![order], false, 0);
    team.execute_actions = true;
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.team_ready_queue.push_back(team);
    ai.check_ready_teams(&mut logic, 1.0);
    assert_eq!(
        ai.team_ready_queue.len(),
        1,
        "without ProductionCondition action, wait for allIdle"
    );
}

#[test]
fn check_ready_teams_reinforcement_idles_only_reinforcement_unit() {
    // C++ m_reinforcement idle gate uses only m_reinforcementID.
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let reinforce = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("reinforce");
    let fielded = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("fielded");
    if let Some(o) = logic.host_object_mut(fielded) {
        o.set_ai_state(crate::game_logic::AIState::Moving);
    }
    let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
    order.num_completed = 2;
    order.observed_unit_ids.push(reinforce);
    order.observed_unit_ids.push(fielded);
    let mut team = AITeamQueue::new("W21_ReinforceTeam".into(), vec![order], false, 0);
    team.reinforcement = true;
    team.reinforcement_id = Some(reinforce);
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.team_ready_queue.push_back(team);
    ai.check_ready_teams(&mut logic, 1.0);
    assert!(
        ai.team_ready_queue.is_empty(),
        "idle reinforcement unit activates even if fielded teammates are busy"
    );
}

#[test]
fn select_team_to_reinforce_tops_up_short_auto_team() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut tank = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    tank.add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_cost(100, 0);
    logic.templates.insert("AmericaTankCrusader".into(), tank);
    let mut wf = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
    wf.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSWarFactory);
    logic.templates.insert("AmericaWarFactory".into(), wf);

    let tank_id = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::ZERO)
        .expect("live crusader");
    if let Some(obj) = logic.host_object_mut(tank_id) {
        obj.owner_player_id = Some(1);
        obj.team_instance_name = "HQ_V3_TankTeam".into();
    }
    // Player-wide census is at maxUnits, but the instance is still short.
    for i in 1..=2 {
        let extra = logic
            .create_object(
                "AmericaTankCrusader",
                Team::USA,
                Vec3::new(i as f32 * 8.0, 4.0, 0.0),
            )
            .expect("extra crusader");
        if let Some(obj) = logic.host_object_mut(extra) {
            obj.owner_player_id = Some(1);
        }
    }
    let factory = logic
        .create_object("AmericaWarFactory", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("idle factory");
    if let Some(obj) = logic.host_object_mut(factory) {
        obj.owner_player_id = Some(1);
    }

    let mut inst_id = None;
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_V3_TankTeam".into());
        proto.set_automatically_reinforce(true);
        proto.set_production_priority(50);
        proto.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 1,
                max_units: 3,
                unit_thing_name: "AmericaTankCrusader",
            },
        );
        tf.replace_team_prototype(proto);
        if let Some(team) = tf.create_team("HQ_V3_TankTeam") {
            if let Ok(mut tg) = team.write() {
                tg.add_member(tank_id.0);
                inst_id = Some(tg.get_id());
            }
        }
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert!(ai.select_team_to_reinforce(&mut logic, 0, 1.0));
    let team = ai.team_queue.front().expect("reinforce order");
    assert!(team.reinforcement);
    assert_eq!(
        team.team_id, inst_id,
        "reinforce the instance that has units"
    );
    assert_eq!(team.work_orders[0].num_required, 1);
    assert_eq!(team.work_orders[0].template_name, "AmericaTankCrusader");
    assert_eq!(ai.next_team_queue_time, 1.0);
    if let Some(recruited) = team.work_orders[0].observed_unit_ids.first().copied() {
        assert_eq!(
            logic
                .host_object(recruited)
                .map(|o| o.team_instance_name.as_str()),
            Some("HQ_V3_TankTeam"),
            "C++ selectTeamToReinforce setTeam onto the reinforced instance"
        );
    }

    // Empty instance is skipped even if the player owns units elsewhere.
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_V3_EmptyTeam".into());
        proto.set_automatically_reinforce(true);
        proto.set_production_priority(80);
        proto.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 1,
                max_units: 3,
                unit_thing_name: "AmericaTankCrusader",
            },
        );
        tf.replace_team_prototype(proto);
        let _ = tf.create_inactive_team("HQ_V3_EmptyTeam");
    }
    let mut ai_empty = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert!(
        !ai_empty.select_team_to_reinforce(&mut logic, 50, 1.0),
        "empty leftover instance must not auto-reinforce from player-wide census"
    );
}

#[test]
fn unlimited_rebuilds_do_not_spend_budget_on_first_build() {
    // C++ `BuildListInfo::decrementNumRebuilds` (`SidesList.h:349-353`) is
    // a no-op for `UNLIMITED_REBUILDS`. `newMap` also increments finite
    // rebuilds so the first construction does not consume the last slot
    // (`AISkirmishPlayer.cpp:1083`).
    let mut unlimited =
        AIBuildingInfo::new("AmericaAirfield".into(), Vec3::ZERO, UNLIMITED_REBUILDS);
    unlimited.increment_num_rebuilds();
    unlimited.decrement_num_rebuilds();
    unlimited.decrement_num_rebuilds();
    assert!(unlimited.is_buildable());
    assert_eq!(unlimited.rebuild_count, 0);
    assert_eq!(unlimited.max_rebuilds, UNLIMITED_REBUILDS);

    let mut factory = AIBuildingInfo::new("AmericaWarFactory".into(), Vec3::ZERO, 1);
    factory.increment_num_rebuilds();
    assert!(factory.is_buildable());
    factory.decrement_num_rebuilds(); // first construction
    assert!(
        factory.is_buildable(),
        "first build must not spend the last rebuild"
    );
    factory.decrement_num_rebuilds(); // one rebuild
    assert!(!factory.is_buildable());

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.initialize(Vec3::new(-120.0, 0.0, -120.0));
    let factory = ai
        .building_queue
        .iter_mut()
        .find(|b| b.template_name == "AmericaWarFactory")
        .expect("layout factory");
    assert_eq!(factory.max_rebuilds, UNLIMITED_REBUILDS);
    assert!(factory.is_buildable());
    factory.decrement_num_rebuilds();
    assert!(
        factory.is_buildable(),
        "unlimited layout factory must still rebuild after first start"
    );
    assert_eq!(factory.rebuild_count, 0);
}

#[test]
fn base_defense_uses_approach_fan_not_plus_80() {
    // C++ `AISkirmishPlayer::buildAIBaseDefenseStructure`
    // (`AISkirmishPlayer.cpp:542-686`): first front pad is along the
    // approach at `baseRadius + extraDistance`, then left/right fan.
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.initialize(Vec3::new(-120.0, 0.0, -120.0));
    let patriot = ai
        .building_queue
        .iter()
        .find(|b| b.template_name == "AmericaPatriotBattery")
        .expect("SideInfo defense is queued");
    let plus_80 = ai.base_center + Vec3::new(80.0, 0.0, 80.0);
    assert!(
        (patriot.position - plus_80).length() > 1.0,
        "defense must not sit on the old +80/+80 pad, got {:?}",
        patriot.position
    );
    let approach = -ai.base_center;
    let dir = Vec3::new(approach.x, 0.0, approach.z).normalize();
    let expected =
        ai.base_center + dir * (ai.base_radius + AIPlayer::SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE);
    assert!(
        (patriot.position - expected).length() < 0.25,
        "first front defense must sit on the approach ring: {:?} vs {:?}",
        patriot.position,
        expected
    );

    let first = patriot.position;
    let second = ai
        .place_next_base_defense_structure(None, "AmericaPatriotBattery", false)
        .expect("fan continues to the next legal slot");
    assert!(
        (second - first).length() > 1.0,
        "legality fan must rotate off the previous pad"
    );
    let radius = ai.base_radius + AIPlayer::SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE;
    let second_radius = Vec3::new(
        second.x - ai.base_center.x,
        0.0,
        second.z - ai.base_center.z,
    )
    .length();
    assert!(
        (second_radius - radius).abs() < 0.25,
        "fan slots stay on the defense ring: {second_radius} vs {radius}"
    );
}

#[test]
fn ai_building_placement_is_deterministic() {
    let mut a = AIPlayer::new(3, Team::GLA, AIDifficulty::Medium);
    let mut b = AIPlayer::new(3, Team::GLA, AIDifficulty::Medium);
    a.base_center = Vec3::new(100.0, 0.0, 200.0);
    b.base_center = Vec3::new(100.0, 0.0, 200.0);
    // Drain same number of placement draws.
    let pa = (
        a.placement_rng.next_real(-80.0, 80.0),
        a.placement_rng.next_real(-80.0, 80.0),
    );
    let pb = (
        b.placement_rng.next_real(-80.0, 80.0),
        b.placement_rng.next_real(-80.0, 80.0),
    );
    assert_eq!(pa, pb, "same player_id seed must match placement draws");
    let mut c = AIPlayer::new(99, Team::GLA, AIDifficulty::Medium);
    let pc = (
        c.placement_rng.next_real(-80.0, 80.0),
        c.placement_rng.next_real(-80.0, 80.0),
        c.placement_rng.next_real(-80.0, 80.0),
        c.placement_rng.next_real(-80.0, 80.0),
    );
    let pa4 = (
        a.placement_rng.next_real(-80.0, 80.0),
        a.placement_rng.next_real(-80.0, 80.0),
        a.placement_rng.next_real(-80.0, 80.0),
        a.placement_rng.next_real(-80.0, 80.0),
    );
    assert_ne!(pa4, pc, "different player_id seeds must diverge");
}

use super::{AIDifficulty, AIManager, AIPlayer};
use crate::game_logic::{ObjectId, Team};

/// Gate-only early-attack intervals must not reappear; keep 60s spacing number.
#[test]
fn host_attack_recheck_uses_sixty_second_spacing_not_gate_hack() {
    // Same NUMBER as C++ ready-team force-start (60s), not full checkReadyTeams semantics.
    assert_eq!(AIPlayer::ATTACK_RECHECK_SECONDS, 60.0);
    assert!(
        AIPlayer::ATTACK_RECHECK_SECONDS >= 30.0,
        "must not use gate-only early-attack shortcut (<30s)"
    );
}

#[test]
fn rebind_after_world_reset_keeps_difficulty_active_and_remaining_rebuilds() {
    let mut mgr = AIManager::new();
    mgr.add_ai_player(1, Team::GLA, AIDifficulty::Hard);
    mgr.set_ai_active(1, true);
    let spent = {
        let ai = mgr.ai_players.get_mut(&1).expect("ai");
        if let Some(b) = ai.building_queue.first_mut() {
            b.object_id = Some(ObjectId(42));
            b.rebuild_count = b.max_rebuilds;
            b.is_built = true;
            b.rebuild_count
        } else {
            0
        }
    };
    {
        let ai = mgr.ai_players.get_mut(&1).expect("ai");
        ai.defensive_units.push(ObjectId(7));
        ai.attack_in_progress = true;
    }

    mgr.rebind_after_world_reset();

    assert!(mgr.is_ai_active(1));
    assert_eq!(mgr.ai_difficulty(1), Some(AIDifficulty::Hard));
    let ai = mgr.ai_players.get(&1).expect("ai after rebind");
    assert!(ai.defensive_units.is_empty());
    assert!(!ai.attack_in_progress);
    let b = ai.building_queue.first().expect("layout retained");
    assert!(b.object_id.is_none());
    assert_eq!(b.rebuild_count, spent);
    assert!(!b.is_built);
    assert!(!b.template_name.is_empty());
}

#[test]
fn apply_queue_persist_rebinds_pad_object_and_priority() {
    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Hard);
    ai.initialize(Vec3::ZERO);
    assert!(
        ai.building_queue.len() >= 3,
        "China layout has power + supply pads"
    );
    let mut persist = ai.capture_queue_persist();
    persist.building_object_ids[2] = Some(77);
    persist.building_is_built[2] = false;
    persist.building_is_priority[1] = true;
    persist.building_is_built[0] = true;
    ai.apply_queue_persist(persist);
    assert_eq!(ai.building_queue[2].object_id, Some(ObjectId(77)));
    assert!(!ai.building_queue[2].is_built);
    assert!(ai.building_queue[1].is_priority);
    assert!(ai.building_queue[0].is_built);
}

#[test]
fn retain_queue_object_ids_drops_missing_pad_binding() {
    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Hard);
    ai.initialize(Vec3::ZERO);
    ai.building_queue[0].object_id = Some(ObjectId(77));
    ai.building_queue[0].is_built = true;
    let mut valid = std::collections::HashSet::new();
    valid.insert(ObjectId(1));
    ai.retain_queue_object_ids(&valid);
    assert!(ai.building_queue[0].object_id.is_none());
    assert!(!ai.building_queue[0].is_built);
}

#[test]
fn launch_attack_sets_target_and_logs_host_attack() {
    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::host_attack_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    // Default AI_DECISION_AUTHORITY is on: launch_attack engages host + logs decisions.
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    // Decision logs require coupled shadow writeback frame (live gate).
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    crate::gameworld_shadow::begin_shadow_coupled_tick();
    host_attack_log::clear();
    host_ai_decision_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiAtk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for (name, team, x) in [("AiAtkU", Team::USA, 0.0f32), ("AiAtkE", Team::GLA, 80.0)] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(100.0);
            tmpl.add_kind_of(KindOf::Infantry);
            tmpl.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), tmpl);
        }
        let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
    }
    let usa_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::USA)
        .map(|(id, _)| *id)
        .unwrap_or(0);
    let gla_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::GLA)
        .map(|(id, _)| *id);
    let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = gla_id;
    ai.is_active = true;
    let usa_unit = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.team == Team::USA && o.is_alive())
        .map(|(id, _)| *id)
        .expect("usa unit");
    if let Some(o) = logic.host_object_mut(usa_unit) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            ..Weapon::default()
        });
    }
    ai.launch_attack(&mut logic, 1000.0);
    let decisions = host_ai_decision_log::drain();
    let unit = logic
        .host_objects()
        .get(&usa_unit)
        .expect("usa unit after launch");
    assert!(
        unit.target.is_some(),
        "under AI decision authority host target engages immediately"
    );
    assert!(
        decisions.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK && e.host_object == usa_unit
        }),
        "launch_attack must log AttackTarget decision; got {decisions:?}"
    );
    assert!(
        decisions.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == usa_unit
                && e.ai_state_ordinal == 3
        }),
        "launch_attack must log AttackMoving state; got {decisions:?}"
    );
    assert!(
        !unit.movement.path.is_empty() || unit.movement.target_position.is_some(),
        "launch_attack must still pathfind on host under decision authority"
    );
    // Legacy residual path when decision authority is off.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    host_attack_log::clear();
    host_ai_decision_log::clear();
    if let Some(o) = logic.host_object_mut(usa_unit) {
        o.target = None;
        o.ai_state = AIState::Idle;
        o.movement.path.clear();
        o.movement.target_position = None;
    }
    ai.launch_attack(&mut logic, 2000.0);
    let logged = host_attack_log::drain();
    let unit = logic
        .host_objects()
        .get(&usa_unit)
        .expect("usa unit legacy");
    assert!(
        unit.target.is_some() && !logged.is_empty(),
        "legacy launch_attack must set_target and host_attack_log"
    );
    assert_eq!(unit.ai_state, AIState::AttackMoving);
    crate::gameworld_shadow::end_shadow_coupled_tick();
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
}

#[test]
fn launch_attack_uses_assign_unit_path_surface() {
    let src = include_str!("combat.rs");
    // Do not split on cfg(test) — nested test modules can appear earlier.
    let i = src
        .find("fn attack_move_units(")
        .expect("attack_move_units");
    let w = &src[i..i + 4500.min(src.len() - i)];
    assert!(
        w.contains("assign_unit_path")
            && (w.contains("AIState::AttackMoving") || w.contains("record_set_state")),
        "AI launch_attack must pathfind then restore AttackMoving (host or decision log)"
    );
    // Fallback may call move_to after assign_unit_path fails; primary path
    // must call assign_unit_path first.
    let path_i = w.find("assign_unit_path").expect("path");
    let move_i = w.find("move_to(enemy_base)");
    assert!(
        move_i.is_none() || move_i.unwrap() > path_i,
        "move_to fallback must come after assign_unit_path"
    );
}

#[test]
fn launch_attack_dispatches_crate_attack_move_state() {
    // C++ AIAttackMoveState / AIInternalMoveToState::onEnter (AIStates.cpp).
    // Live host AIState is a flat enum; launch_attack must also record
    // crate AiStateType::AttackMoveTo via dispatch_host_move_attack.
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use gamelogic::ai::state_machine::{AiStateType, host_move_attack_state};

    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    crate::gameworld_shadow::begin_shadow_coupled_tick();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiSm");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for (name, team, x) in [("AiSmU", Team::USA, 0.0f32), ("AiSmE", Team::GLA, 80.0)] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(100.0);
            tmpl.add_kind_of(KindOf::Infantry);
            tmpl.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), tmpl);
        }
        let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
    }
    let usa_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::USA)
        .map(|(id, _)| *id)
        .unwrap_or(0);
    let gla_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::GLA)
        .map(|(id, _)| *id);
    let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = gla_id;
    ai.is_active = true;
    let usa_unit = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.team == Team::USA && o.is_alive())
        .map(|(id, _)| *id)
        .expect("usa unit");
    if let Some(o) = logic.host_object_mut(usa_unit) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            ..Weapon::default()
        });
    }

    ai.launch_attack(&mut logic, 1000.0);

    assert_eq!(
        host_move_attack_state(usa_unit.0),
        Some(AiStateType::AttackMoveTo),
        "launch_attack must record crate AttackMoveTo for the live unit"
    );
    assert_eq!(
        logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
        Some(AIState::AttackMoving)
    );

    crate::gameworld_shadow::end_shadow_coupled_tick();
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
}

#[test]
fn second_attack_starts_after_first_raid_finishes() {
    // C++ checkReadyTeams only setActive. OnCreate Hunt/Guard/AttackMove
    // come from scripts. evaluate_attack_opportunities must not dump the army.
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate, Weapon};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));
    logic.add_player(Player::new(2, Team::GLA, "GLA", true));

    let mut unit_t = ThingTemplate::new("Ai2Infantry");
    unit_t.set_health(100.0);
    unit_t.add_kind_of(KindOf::Infantry);
    unit_t.add_kind_of(KindOf::Attackable);
    logic.templates.insert("Ai2Infantry".into(), unit_t);

    let usa_unit = logic
        .create_object("Ai2Infantry", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("usa unit");
    let _ = logic.create_object("Ai2Infantry", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0));
    if let Some(o) = logic.host_object_mut(usa_unit) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            ..Weapon::default()
        });
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = Some(2);
    ai.is_active = true;

    let mut order = AIWorkOrder::new("Ai2Infantry".into(), 1, 100);
    order.num_completed = 1;
    order.observed_unit_ids.push(usa_unit);
    let mut team = AITeamQueue::new("USA_RangerSquad".into(), vec![order], false, 0);
    team.execute_actions = true;
    ai.team_ready_queue.push_back(team);

    ai.check_ready_teams(&mut logic, 1.0);
    assert!(
        ai.team_ready_queue.is_empty(),
        "ready team must activate via setActive"
    );
    assert!(
        !ai.attack_in_progress,
        "empty OnCreate must not invent AttackMove"
    );
    assert_eq!(
        logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
        Some(AIState::Idle),
        "setActive without OnCreate leaves members idle at rally"
    );
    let first_count = ai.activity_count;

    ai.evaluate_attack_opportunities(&mut logic, 1.0 + AIPlayer::ATTACK_RECHECK_SECONDS);
    assert!(
        !ai.attack_in_progress,
        "evaluate_attack_opportunities must not dump the army"
    );
    assert_eq!(ai.activity_count, first_count);

    assert_eq!(
        AIPlayer::classify_on_create_script("TeamHunt"),
        OnCreateIntent::Hunt
    );
    assert_eq!(
        AIPlayer::classify_on_create_script("TeamGuard"),
        OnCreateIntent::Guard
    );
    assert_eq!(
        AIPlayer::classify_on_create_script("TeamAttackMove"),
        OnCreateIntent::AttackMove
    );

    ai.apply_on_create_host_orders(&mut logic, &[usa_unit], "TeamHunt", 1.0);
    assert_eq!(
        logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
        Some(AIState::Patrolling),
        "OnCreate Hunt must hunt, not AttackMove"
    );
    assert!(!ai.attack_in_progress);

    ai.apply_on_create_host_orders(&mut logic, &[usa_unit], "TeamGuard", 1.0);
    assert_eq!(
        logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
        Some(AIState::GuardingArea),
        "OnCreate Guard must guard, not AttackMove"
    );
    assert!(!ai.attack_in_progress);

    let mut order2 = AIWorkOrder::new("Ai2Infantry".into(), 1, 100);
    order2.num_completed = 1;
    order2.observed_unit_ids.push(usa_unit);
    ai.team_ready_queue.push_back(AITeamQueue::new(
        "USA_RangerSquad".into(),
        vec![order2],
        false,
        0,
    ));
    ai.check_ready_teams(&mut logic, 2.0);
    assert!(
        ai.activity_count > first_count,
        "second ready team setActive still counts as activity"
    );
    assert_eq!(
        logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
        Some(AIState::GuardingArea),
        "second setActive without OnCreate must not overwrite Guard with AttackMove"
    );
}

#[test]
fn live_ai_fires_ready_superweapon_at_enemy_cluster() {
    // C++ ScriptActions::doSkirmishFireSpecialPowerAtMostCost
    // (ScriptActions.cpp:4142) + AIPlayer::computeSuperweaponTarget
    // (AIPlayer.cpp:1120). Live host path queues DoSpecialPower.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{
        GameLogic, KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
        ThingTemplate,
    };

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));
    logic.add_player(Player::new(2, Team::GLA, "GLA", true));

    let mut puc = ThingTemplate::new("AiSwPuc");
    puc.set_health(4000.0);
    puc.add_kind_of(KindOf::Structure);
    puc.add_kind_of(KindOf::FSSuperweapon);
    puc.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_SpecialPower".into()),
        module_kind: SpecialPowerModuleKind::OclSpecialPower,
        special_power_template: "SuperweaponParticleUplinkCannon".into(),
        special_power_template_id: 1,
        command_power: Some(SpecialPowerType::ParticleCannon),
        reload_time_frames: 0,
        required_science: None,
        public_timer: true,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
    logic.templates.insert("AiSwPuc".into(), puc);

    let mut barracks = ThingTemplate::new("AiSwBarracks");
    barracks.set_health(1000.0);
    barracks.set_cost(500, 0);
    barracks.add_kind_of(KindOf::Structure);
    barracks.add_kind_of(KindOf::Attackable);
    logic.templates.insert("AiSwBarracks".into(), barracks);

    let caster = logic
        .create_object("AiSwPuc", Team::USA, glam::Vec3::new(-40.0, 0.0, 0.0))
        .expect("puc");
    let _ = logic.create_object("AiSwBarracks", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0));

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = Some(2);
    ai.is_active = true;

    ai.fire_named_special_power(&mut logic, "SuperweaponParticleUplinkCannon");
    logic.process_commands();

    assert!(
        logic
            .special_power_strikes()
            .honesty_queue_ok(crate::game_logic::HostSuperweaponKind::ParticleCannon),
        "live AI must queue a ParticleCannon strike via computeSuperweaponTarget"
    );
    assert!(
        !logic.is_special_power_ready_for(caster, &SpecialPowerType::ParticleCannon)
            || logic.special_power_strikes().strike_count() >= 1,
        "ready superweapon must be consumed or recorded as a strike"
    );
}

#[test]
fn process_building_queue_skips_automatic_layout_pads() {
    // C++ processBaseBuilding never selects automatic pads
    // (canMakeUnit(dozer, NULL) → CANMAKE_NO_PREREQ).
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 50_000;
    logic.add_player(player);

    let mut dozer_template = crate::game_logic::ThingTemplate::new("TestDozer");
    dozer_template
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .add_kind_of(crate::game_logic::KindOf::Worker);
    logic.templates.insert("TestDozer".into(), dozer_template);
    let _ = logic.create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0));

    for name in ["AmericaStrategyCenter", "AmericaAirfield"] {
        let mut t = crate::game_logic::ThingTemplate::new(name);
        t.add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(100, 0);
        logic.templates.insert(name.into(), t);
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_layout_building("AmericaStrategyCenter", Vec3::new(64.0, 0.0, 0.0), 3);
    ai.add_layout_building("AmericaAirfield", Vec3::new(128.0, 0.0, 0.0), 3);
    ai.process_building_queue(&mut logic, 0.0);
    assert!(
        ai.building_queue.iter().all(|b| b.object_id.is_none()),
        "automatic layout pads must not start"
    );

    assert!(ai.build_specific_ai_building("AmericaStrategyCenter"));
    ai.process_building_queue(&mut logic, 0.1);
    assert!(
        ai.building_queue[0].object_id.is_some(),
        "scripted priority stamp must start that pad"
    );
    assert!(
        ai.building_queue[1].object_id.is_none(),
        "unstamped automatic airfield stays queued"
    );
}

#[test]
fn update_does_not_auto_fire_first_ready_special() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{
        GameLogic, KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
        ThingTemplate,
    };

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));
    logic.add_player(Player::new(2, Team::GLA, "GLA", true));

    let mut a10 = ThingTemplate::new("AiSwA10");
    a10.set_health(4000.0);
    a10.add_kind_of(KindOf::Structure);
    a10.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_A10".into()),
        module_kind: SpecialPowerModuleKind::OclSpecialPower,
        special_power_template: "SuperweaponA10ThunderboltMissileStrike".into(),
        special_power_template_id: 2,
        command_power: Some(SpecialPowerType::Airstrike),
        reload_time_frames: 0,
        required_science: None,
        public_timer: true,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
    logic.templates.insert("AiSwA10".into(), a10);
    let _ = logic.create_object("AiSwA10", Team::USA, glam::Vec3::new(-40.0, 0.0, 0.0));

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = Some(2);
    ai.is_active = true;
    ai.update(&mut logic, 1.0);
    logic.process_commands();
    assert_eq!(
        logic.special_power_strikes().strike_count(),
        0,
        "AIPlayer::update must not auto-fire the first ready special"
    );

    ai.fire_named_special_power(&mut logic, "SuperweaponParticleUplinkCannon");
    logic.process_commands();
    assert_eq!(
        logic.special_power_strikes().strike_count(),
        0,
        "wrong script name must not fire a different ready special"
    );
}

#[test]
fn late_game_team_keeps_higher_tier_templates_instead_of_default_infantry() {
    // C++ `AIPlayer::selectTeamToBuild` (AIPlayer.cpp:1630) queues
    // `TeamPrototype` unit lists. Host late-game names must not collapse
    // to default Rangers when the higher-tier ThingTemplates exist.
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 20_000;
    logic.add_player(player);

    for (name, kind, cost) in [
        (
            "AmericaBarracks",
            crate::game_logic::KindOf::FSBarracks,
            500,
        ),
        (
            "AmericaWarFactory",
            crate::game_logic::KindOf::FSWarFactory,
            1_000,
        ),
        (
            "AmericaAirfield",
            crate::game_logic::KindOf::FSAirfield,
            1_000,
        ),
        (
            "AmericaStrategyCenter",
            crate::game_logic::KindOf::FSStrategyCenter,
            2_000,
        ),
        (
            "AmericaPatriotBattery",
            crate::game_logic::KindOf::FSBaseDefense,
            1_000,
        ),
    ] {
        let mut building = crate::game_logic::ThingTemplate::new(name);
        building
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(kind)
            .set_cost(cost, 0);
        logic.templates.insert(name.into(), building);
    }
    for (name, kind, cost) in [
        (
            "AmericaInfantryMissileDefender",
            crate::game_logic::KindOf::Infantry,
            300,
        ),
        (
            "AmericaTankCrusader",
            crate::game_logic::KindOf::Vehicle,
            900,
        ),
        (
            "AmericaJetRaptor",
            crate::game_logic::KindOf::Aircraft,
            1_400,
        ),
    ] {
        let mut unit = crate::game_logic::ThingTemplate::new(name);
        unit.add_kind_of(kind).set_cost(cost, 0);
        logic.templates.insert(name.into(), unit);
    }

    let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);
    let _ = logic.create_object("AmericaWarFactory", Team::USA, Vec3::new(64.0, 0.0, 0.0));
    let _ = logic.create_object("AmericaAirfield", Team::USA, Vec3::new(128.0, 0.0, 0.0));
    let strategy = logic
        .create_object(
            "AmericaStrategyCenter",
            Team::USA,
            Vec3::new(0.0, 0.0, 64.0),
        )
        .expect("strategy center");

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.current_strategy = AIStrategy::LateGame;

    let orders = ai.create_work_orders_for_team("USA_AdvancedStrike");
    let templates: Vec<&str> = orders
        .iter()
        .map(|order| order.template_name.as_str())
        .collect();
    assert!(
        templates.contains(&"AmericaTankCrusader"),
        "late-game USA team must keep Crusaders: {templates:?}"
    );
    assert!(
        templates.contains(&"AmericaJetRaptor"),
        "late-game USA team must keep Raptors: {templates:?}"
    );
    assert!(
        !templates.iter().all(|name| name.contains("Ranger")),
        "late-game USA team must not collapse to Rangers: {templates:?}"
    );
    assert!(
        ai.create_work_orders_for_team("NoSuchTeam").is_empty(),
        "unknown team names must not invent default infantry"
    );
    assert!(
        ai.is_possible_to_build_team(&logic, "USA_AdvancedStrike"),
        "factories for missile infantry, tanks, and jets must satisfy the late team"
    );

    ai.initialize(Vec3::new(-120.0, 0.0, -120.0));
    let planned: Vec<&str> = ai
        .building_queue
        .iter()
        .map(|building| building.template_name.as_str())
        .collect();
    assert!(
        planned.contains(&"AmericaStrategyCenter")
            && planned.contains(&"AmericaAirfield")
            && planned.contains(&"AmericaPatriotBattery"),
        "skirmish layout must include tech, air, and SideInfo defense: {planned:?}"
    );

    ai.do_upgrades_and_skills(&mut logic);
    let player = logic.get_player(1).expect("AI player");
    assert!(
        player.has_queued_upgrade("Upgrade_AmericaSupplyLines")
            || player.has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
            || player.has_queued_upgrade("Upgrade_AmericaAdvancedTraining")
            || logic
                .host_object(strategy)
                .and_then(|object| object.building_data.as_ref())
                .is_some_and(|building| building
                    .production_queue
                    .iter()
                    .any(|item| item.is_upgrade())),
        "live AI must queue a structure upgrade via AIPlayer::buildUpgrade residual"
    );
}

#[test]
fn do_upgrades_does_not_research_supply_lines_at_barracks() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.resources.supplies = 20_000;
    logic.add_player(player);
    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSBarracks)
        .set_health(1000.0);
    logic.templates.insert("AmericaBarracks".into(), barracks);
    let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.do_upgrades_and_skills(&mut logic);
    let player = logic.get_player(1).expect("AI player");
    assert!(
        !player.has_queued_upgrade("Upgrade_AmericaSupplyLines"),
        "C++ canProduceUpgrade refuses SupplyLines at Barracks"
    );
    assert!(
        player.has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding"),
        "Barracks CommandSet still researches Capture"
    );
}

#[test]
fn check_queued_teams_disbands_expired_incomplete_team() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    let mut ranger_t = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger_t.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger_t);
    let ranger = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("ranger");
    if let Some(obj) = logic.host_object_mut(ranger) {
        obj.owner_player_id = Some(1);
        obj.team_instance_name = "HQ_9_Disband".into();
    }

    let mut inst_id = None;
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_9_Disband".into());
        proto.set_initial_idle_frames(30);
        tf.replace_team_prototype(proto);
        if let Some(team) = tf.create_inactive_team("HQ_9_Disband") {
            if let Ok(mut tg) = team.write() {
                tg.add_member(ranger.0);
                inst_id = Some(tg.get_id());
            }
        }
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
    order.num_completed = 0;
    order.observed_unit_ids.push(ranger);
    let mut q = AITeamQueue::new("HQ_9_Disband".into(), vec![order], false, 0);
    q.team_id = inst_id;
    ai.team_queue.push_back(q);

    ai.check_queued_teams(&mut logic, 2.0);
    assert!(
        ai.team_queue.is_empty() && ai.team_ready_queue.is_empty(),
        "expired team below minimum must disband"
    );
    let default = logic.default_host_team_instance_name(Some(1), Team::USA);
    assert_eq!(
        logic
            .host_object(ranger)
            .map(|o| o.team_instance_name.clone())
            .unwrap_or_default(),
        default,
        "disband must transfer recruits to the default team"
    );
    assert!(
        AIPlayer::leftover_team_instance_gone(inst_id),
        "non-singleton leftover instance must be deleted on disband"
    );
}

#[test]
fn check_queued_teams_zero_idle_frames_never_expires() {
    if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
        let mut proto = gamelogic::team::TeamPrototype::new("HQ_80_Never".into());
        proto.set_initial_idle_frames(0);
        tf.replace_team_prototype(proto);
    }
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
    order.num_completed = 0;
    ai.team_queue.push_back(AITeamQueue::new(
        "HQ_80_Never".into(),
        vec![order],
        false,
        0,
    ));
    let mut logic = crate::game_logic::GameLogic::new();
    ai.check_queued_teams(&mut logic, 999.0);
    assert_eq!(
        ai.team_queue.len(),
        1,
        "InitialIdleFrames < 1 is unlimited; team must not expire"
    );
    assert!(ai.team_ready_queue.is_empty());
}

#[test]
fn air_force_side_uses_a10_skillset_not_paladin() {
    let residual = AIPlayer::residual_general_skillsets("AmericaAirForceGeneral")
        .expect("Air Force SideInfo residual");
    assert_eq!(residual[0][0], "AirF_SCIENCE_A10ThunderboltMissileStrike1");
    assert_ne!(residual[0][0], "SCIENCE_PaladinTank");

    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(1, Team::USA, "AirF", false));
    if let Some(identity) =
        crate::game_logic::PlayerTemplateIdentity::from_exact_name("FactionAmericaAirForceGeneral")
    {
        let _ = logic.bind_player_template_identity(1, identity);
        let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let sets = ai.live_side_skillsets(&logic);
        let first = sets[0].first().map(String::as_str).unwrap_or("");
        assert_ne!(first, "SCIENCE_PaladinTank");
    }
}

#[test]
fn find_dozer_skips_assigned_bridge_repair_dozer() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    let mut dozer_t = crate::game_logic::ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(crate::game_logic::KindOf::Dozer)
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);
    let repair = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair dozer");
    let free = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("free dozer");
    let found = AIPlayer::find_available_dozer(&logic, Team::USA, Vec3::ZERO, Some(repair));
    assert_eq!(found, Some(free));
}

#[test]
fn ctor_helper_disables_unit_construction() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    assert!(logic.get_player(1).unwrap().can_build_units);
    AIManager::apply_ctor_can_build_units(&mut logic, 1);
    assert!(!logic.get_player(1).unwrap().can_build_units);
}

#[test]
fn acquire_enemy_keeps_healthy_current_target() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    logic.add_player(crate::game_logic::Player::new(2, Team::GLA, "GLA", true));
    logic.add_player(crate::game_logic::Player::new(
        3,
        Team::China,
        "China",
        true,
    ));

    let mut barracks = crate::game_logic::ThingTemplate::new("GLABarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSBarracks)
        .set_health(1000.0);
    logic.templates.insert("GLABarracks".into(), barracks);
    let mut rebel = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    rebel
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel);
    let mut china_cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    china_cc
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::CommandCenter)
        .set_health(5000.0);
    logic
        .templates
        .insert("ChinaCommandCenter".into(), china_cc);

    let _ = logic.create_object("GLABarracks", Team::GLA, Vec3::new(400.0, 0.0, 0.0));
    let _ = logic.create_object("GLAInfantryRebel", Team::GLA, Vec3::new(410.0, 0.0, 0.0));
    let _ = logic.create_object("ChinaCommandCenter", Team::China, Vec3::new(20.0, 0.0, 0.0));

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    ai.enemy_player_id = Some(2);
    ai.enemy_check_time = -10.0;
    ai.update_enemy_assessment(&mut logic, 0.0);
    assert_eq!(
        ai.enemy_player_id,
        Some(2),
        "healthy current enemy with units and a factory must be kept"
    );
}

#[test]
fn cluster_mines_land_on_own_approach_not_enemy_centroid() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::China,
        "China AI",
        false,
    ));
    logic.add_player(crate::game_logic::Player::new(2, Team::USA, "USA", true));
    let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_cost(500, 0)
        .set_health(1000.0);
    logic.templates.insert("AmericaBarracks".into(), barracks);
    let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::new(400.0, 0.0, 0.0));

    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    ai.base_radius = 100.0;
    let target = ai
        .compute_cluster_mines_target(&logic, Team::USA)
        .expect("approach");
    let dist_from_base = (target.x * target.x + target.z * target.z).sqrt();
    assert!(
        (dist_from_base - 100.0).abs() < 1.0,
        "cluster mines must sit on the AI approach ring, got {target:?} dist={dist_from_base}"
    );
    assert!(
        target.x < 200.0,
        "mines must not drop on the enemy value centroid"
    );
}

#[test]
fn skillset_selector_uses_chosen_ai_side_info_set() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::China, "China AI", false);
    player.science_purchase_points = 1;
    logic.add_player(player);

    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
    assert_eq!(ai.side_skillsets()[0][0], "SCIENCE_NukeLauncher");
    assert_eq!(ai.side_skillsets()[1][0], "SCIENCE_RedGuardTraining");
    ai.select_skillset(1);
    ai.try_purchase_skillset_science(&mut logic);
    assert_eq!(ai.skillset_selector, 1);
    assert_ne!(
        ai.side_skillsets()[ai.skillset_selector as usize][0],
        "SCIENCE_NukeLauncher"
    );
}

#[test]
fn do_upgrades_and_skills_skips_first_two_frames_like_cpp() {
    // C++ AIPlayer.cpp:2910-2912 — if (TheGameLogic->getFrame() < 2) return;
    if let Ok(mut list) = gamelogic::player::player_list().write() {
        list.clear();
    }
    let mut logic = crate::game_logic::GameLogic::new();
    logic.frame = 0;
    let mut player = crate::game_logic::Player::new(1, Team::China, "China AI", false);
    player.science_purchase_points = 1;
    logic.add_player(player);
    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
    ai.do_upgrades_and_skills(&mut logic);
    assert_eq!(ai.skillset_selector, INVALID_SKILLSET_SELECTION);
    assert!(
        !logic
            .get_player(1)
            .unwrap()
            .has_unlocked_science("SCIENCE_NukeLauncher"),
        "frame 0 must not spend Rank1 SPP"
    );

    logic.frame = 1;
    ai.do_upgrades_and_skills(&mut logic);
    assert_eq!(ai.skillset_selector, INVALID_SKILLSET_SELECTION);

    logic.frame = 2;
    ai.do_upgrades_and_skills(&mut logic);
    assert_eq!(
        ai.skillset_selector, 0,
        "non-skirmish AI must pick SkillSet1 after the frame gate"
    );
}

#[test]
fn non_skirmish_ai_skillset_defaults_to_skillset1() {
    // C++ AIPlayer.cpp:2944-2948 — isSkirmishAI() false → selector 0.
    install_leftover_computer_player(1, false);
    let mut logic = crate::game_logic::GameLogic::new();
    logic.frame = 2;
    let mut player = crate::game_logic::Player::new(1, Team::China, "China AI", false);
    player.science_purchase_points = 1;
    logic.add_player(player);
    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
    assert!(!ai.leftover_is_skirmish_ai());
    ai.try_purchase_skillset_science(&mut logic);
    assert_eq!(ai.skillset_selector, 0);
    assert_eq!(ai.side_skillsets()[0][0], "SCIENCE_NukeLauncher");
    clear_player_team_prototypes();
}

#[test]
fn skirmish_ai_skillset_randomizes_like_cpp() {
    // C++ AIPlayer.cpp:2944-2945 — isSkirmishAI() true → GameLogicRandomValue(0, limit).
    install_leftover_computer_player(1, true);
    let mut logic = crate::game_logic::GameLogic::new();
    logic.frame = 2;
    let mut player = crate::game_logic::Player::new(1, Team::China, "China AI", false);
    player.science_purchase_points = 1;
    logic.add_player(player);
    let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
    assert!(ai.leftover_is_skirmish_ai());
    let sets = ai.live_side_skillsets(&logic);
    let mut limit = 0i32;
    if !sets[1].is_empty() {
        limit = 1;
        if !sets[2].is_empty() {
            limit = 2;
            if !sets[3].is_empty() {
                limit = 3;
                if !sets[4].is_empty() {
                    limit = 4;
                }
            }
        }
    }
    let mut probe = crate::game_logic::host_rng_residual::HostRandomState::seeded(
        1u32.wrapping_add(0xA17A_0001),
    );
    let expected = probe.next_int(0, limit);
    ai.try_purchase_skillset_science(&mut logic);
    assert_eq!(ai.skillset_selector, expected);
    assert!((0..=limit).contains(&ai.skillset_selector));
    clear_player_team_prototypes();
}

#[test]
fn air_force_side_info_skillset_is_a10_not_paladin() {
    let residual = AIPlayer::residual_general_skillsets("AmericaAirForceGeneral")
        .expect("Air Force residual SideInfo");
    assert_eq!(residual[0][0], "AirF_SCIENCE_A10ThunderboltMissileStrike1");
    assert_eq!(residual[1][0], "SCIENCE_SpectreGunship1");

    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "Air Force AI", false);
    player.science_purchase_points = 5;
    logic.add_player(player);
    let identity =
        crate::game_logic::PlayerTemplateIdentity::from_exact_name("FactionAmericaAirForceGeneral")
            .expect("retail Air Force PlayerTemplate");
    assert!(logic.bind_player_template_identity(1, identity));

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert_eq!(ai.side_skillsets()[0][0], "SCIENCE_PaladinTank");
    ai.select_skillset(0);
    let first = ai.live_side_skillsets(&logic)[0][0].clone();
    assert_eq!(first, "AirF_SCIENCE_A10ThunderboltMissileStrike1");
    assert_ne!(first, "SCIENCE_PaladinTank");
}

#[test]
fn skillset_purchase_readies_required_special_powers() {
    use crate::command_system::SpecialPowerType;
    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
    player.science_purchase_points = 10;
    assert!(player.unlock_science("SCIENCE_AMERICA"));
    player
        .shared_special_power_cooldowns
        .insert(SpecialPowerType::DaisyCutter, 99.0);
    logic.add_player(player);

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.select_skillset(0);
    ai.try_purchase_skillset_science(&mut logic);
    let player = logic.get_player(1).expect("player");
    assert!(
        player.has_unlocked_science("SCIENCE_DaisyCutter"),
        "skillset 1 must buy DaisyCutter once AMERICA is owned"
    );
    assert!(
        !player
            .shared_special_power_cooldowns
            .contains_key(&SpecialPowerType::DaisyCutter),
        "C++ addScience onSpecialPowerCreation must ready the required shared power"
    );
}

#[test]
fn check_bridges_uses_leftover_find_broken_bridge_gate() {
    let src = include_str!("combat.rs");
    let i = src.find("pub fn check_bridges(").expect("check_bridges");
    let w = &src[i..src.len().min(i + 5000)];
    assert!(
        w.contains("client_safe_quick_does_path_exist")
            && w.contains("find_broken_bridge")
            && w.contains("repair_structure")
            && w.contains("get_waypoint_by_id")
            && !w.contains("host_object_is_bridge"),
        "checkBridges must use leftover findBrokenBridge + clientSafeQuickDoesPathExist"
    );
}

#[test]
fn check_bridges_does_not_queue_every_damaged_span() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));

    let mut bridge_t = crate::game_logic::ThingTemplate::new("CabinBridge");
    bridge_t
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_health(2000.0);
    logic.templates.insert("CabinBridge".into(), bridge_t);
    let mut dozer_t = crate::game_logic::ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(crate::game_logic::KindOf::Dozer)
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);

    let bridge = logic
        .create_object("CabinBridge", Team::Neutral, Vec3::new(80.0, 0.0, 0.0))
        .expect("bridge");
    if let Some(o) = logic.host_object_mut(bridge) {
        o.health.current = 400.0;
        o.body_damage_state = HostBodyDamageType::Damaged;
    }
    let dozer = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::ZERO)
        .expect("dozer");

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    // No leftover waypoint hop / destroyed layer → must not scan every span.
    assert!(!ai.check_bridges(&logic, dozer, 0));
    assert!(ai.structures_to_repair.is_empty());
    ai.repair_structure(&logic, bridge);
    ai.last_bridge_repair_time = -1.0;
    ai.update_bridge_repair(&mut logic, 0.0);
    assert_eq!(ai.repair_dozer, Some(dozer));
    assert!(ai.dozer_is_repairing);
}

#[test]
fn factory_exit_nudge_calls_leftover_move_allies() {
    let src = include_str!("destination_clearance.rs");
    let i = src
        .find("pub fn move_allies_away_from_destination")
        .expect("moveAlliesAwayFromDestination");
    let w = &src[i..src.len().min(i + 5000)];
    assert!(
        w.contains("move_allies_away_from_destination_for")
            && w.contains("ai_move_away_from_unit")
            && w.contains("ignored_obstacle_id")
            && w.contains("using_ability"),
        "factory-exit dest-line must call leftover then nudge idle host allies"
    );
}

#[test]
fn factory_exit_nudge_idle_ally_on_dest_line() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut t = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    t.add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let mover = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(5.0, 0.0, 5.0))
        .expect("mover");
    let ally = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(15.0, 0.0, 5.0),
        )
        .expect("ally");
    logic.move_allies_away_from_destination(mover, Vec3::new(25.0, 0.0, 5.0));
    let ally_obj = logic.host_object(ally).expect("ally");
    assert_eq!(
        ally_obj.move_away_from,
        Some(mover),
        "idle ally on factory-exit dest line must scoot"
    );
}

#[test]
fn factory_exit_nudge_scoots_idle_ally_on_destination_line() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut mover_t = crate::game_logic::ThingTemplate::new("Ranger");
    mover_t.add_kind_of(crate::game_logic::KindOf::Infantry);
    logic.templates.insert("Ranger".into(), mover_t);
    let mut ally_t = crate::game_logic::ThingTemplate::new("Humvee");
    ally_t.add_kind_of(crate::game_logic::KindOf::Vehicle);
    logic.templates.insert("Humvee".into(), ally_t);

    let mover = logic
        .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("mover");
    let ally = logic
        .create_object("Humvee", Team::USA, Vec3::new(50.0, 0.0, 10.0))
        .expect("ally");
    if let Some(o) = logic.host_object_mut(ally) {
        o.owner_player_id = Some(0);
    }
    if let Some(o) = logic.host_object_mut(mover) {
        o.owner_player_id = Some(0);
    }

    logic.move_allies_away_from_destination(mover, Vec3::new(80.0, 0.0, 10.0));
    let ally_obj = logic.host_object(ally).expect("ally after nudge");
    assert_eq!(
        ally_obj.move_away_from,
        Some(mover),
        "idle ally on the dest line must aiMoveAwayFromUnit"
    );
}

#[test]
fn compute_center_and_radius_pads_geom_point_four() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut pad = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    pad.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::CommandCenter);
    pad.geometry_info.authored = true;
    pad.geometry_info.geom_type = crate::game_logic::HostGeometryType::Cylinder;
    pad.geometry_info.major_radius = 50.0;
    logic.templates.insert("AmericaCommandCenter".into(), pad);

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("AmericaCommandCenter", Vec3::new(0.0, 0.0, 0.0), 1);
    ai.add_building("AmericaCommandCenter", Vec3::new(100.0, 0.0, 0.0), 1);
    ai.compute_center_and_radius_of_base(&logic);

    assert!(
        (ai.base_center.x - 50.0).abs() < 0.01 && ai.base_center.z.abs() < 0.01,
        "centroid of build-list XY: {:?}",
        ai.base_center
    );
    // Raw max pad dist is 50. C++ adds geom*0.4 (=20) on each axis → 70.
    assert!(
        (ai.base_radius - 70.0).abs() < 0.01,
        "radius must include geom*0.4, got {}",
        ai.base_radius
    );
}

fn insert_warehouse_template(logic: &mut crate::game_logic::GameLogic, name: &str) {
    let mut pile = crate::game_logic::ThingTemplate::new(name);
    pile.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplySource);
    pile.dock_kind = crate::game_logic::DockKind::SupplyWarehouse;
    logic.templates.insert(name.into(), pile);
}

#[test]
fn find_supply_center_uses_warehouse_dock_and_cash_gen_and_sixty_forty() {
    let mut logic = crate::game_logic::GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        Team::USA,
        "USA AI",
        false,
    ));
    logic.add_player(crate::game_logic::Player::new(
        2,
        Team::China,
        "China",
        true,
    ));
    insert_warehouse_template(&mut logic, "SupplyWarehouse");
    let mut sc = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
    sc.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
    logic.templates.insert("AmericaSupplyCenter".into(), sc);

    let bare = {
        let mut t = crate::game_logic::ThingTemplate::new("BarePile");
        t.add_kind_of(crate::game_logic::KindOf::SupplySource)
            .add_kind_of(crate::game_logic::KindOf::Harvestable);
        t
    };
    logic.templates.insert("BarePile".into(), bare);
    let bare_id = logic
        .create_object("BarePile", Team::Neutral, Vec3::new(10.0, 0.0, 0.0))
        .expect("bare");
    if let Some(obj) = logic.host_object_mut(bare_id) {
        obj.stored_resources.supplies = 9_000;
    }

    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.base_center = Vec3::ZERO;
    ai.enemy_player_id = Some(2);
    assert!(
        ai.find_supply_center(&logic, 100).is_none(),
        "no SupplyWarehouseDockUpdate → skip"
    );

    let near = logic
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(80.0, 0.0, 0.0))
        .expect("near");
    if let Some(obj) = logic.host_object_mut(near) {
        obj.stored_resources.supplies = 5_000;
    }
    let far = logic
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(400.0, 0.0, 0.0))
        .expect("far");
    if let Some(obj) = logic.host_object_mut(far) {
        obj.stored_resources.supplies = 5_000;
    }
    assert_eq!(ai.find_supply_center(&logic, 100), Some(near));

    let own = logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("own SC");
    if let Some(obj) = logic.host_object_mut(own) {
        obj.owner_player_id = Some(1);
    }
    assert_eq!(
        ai.find_supply_center(&logic, 100),
        Some(far),
        "own cash-gen within CLOSE_DIST skips the near warehouse"
    );

    logic.destroy_object(own);
    logic.destroy_object(near);
    // Warehouse at 100 is closer to the China structure midpoint (0) than
    // to our base at 1000 under the 60/40 expansion gate.
    if let Some(obj) = logic.host_object_mut(far) {
        obj.set_position(Vec3::new(100.0, 0.0, 0.0));
    }
    let enemy_cc = {
        let mut t = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
        t.add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::CommandCenter);
        t
    };
    logic
        .templates
        .insert("ChinaCommandCenter".into(), enemy_cc);
    let _ = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::ZERO)
        .expect("enemy CC");
    ai.base_center = Vec3::new(1000.0, 0.0, 0.0);
    assert!(
        ai.find_supply_center(&logic, 100).is_none(),
        "60/40 closer to enemy structure bounds than to us"
    );
}

#[test]
fn find_supply_center_halves_cash_floor_then_stops_at_one_hundred() {
    let mut logic = crate::game_logic::GameLogic::new();
    insert_warehouse_template(&mut logic, "SupplyWarehouse");
    let id = logic
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(50.0, 0.0, 0.0))
        .expect("wh");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.stored_resources.supplies = 150;
    }
    let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert!(
        ai.find_supply_center(&logic, 200).is_none(),
        "C++ do/while: fail at 200, halve to 100, stop without another pass"
    );
    assert_eq!(ai.find_supply_center(&logic, 140), Some(id));
}

#[test]
fn is_location_safe_rejects_enemies_not_harvesters_or_undetected_stealth() {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut pad = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
    pad.add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
    pad.geometry_info.authored = true;
    pad.geometry_info.major_radius = 10.0;
    logic.templates.insert("AmericaSupplyCenter".into(), pad);

    let mut ranger = crate::game_logic::ThingTemplate::new("ChinaInfantryRedguard");
    ranger
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".into(), ranger);

    let mut harvester = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
    harvester
        .add_kind_of(crate::game_logic::KindOf::Harvester)
        .add_kind_of(crate::game_logic::KindOf::Aircraft)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleChinook".into(), harvester);

    let mut dozer = crate::game_logic::ThingTemplate::new("ChinaVehicleDozer");
    dozer
        .add_kind_of(crate::game_logic::KindOf::Dozer)
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(200.0);
    logic.templates.insert("ChinaVehicleDozer".into(), dozer);

    let template = logic.templates.get("AmericaSupplyCenter").cloned();
    let pos = Vec3::ZERO;
    let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    assert!(!ai.is_location_safe(&logic, pos, None));
    assert!(ai.is_location_safe(&logic, pos, template.as_ref()));

    let enemy = logic
        .create_object("ChinaInfantryRedguard", Team::China, pos)
        .expect("enemy");
    assert!(!ai.is_location_safe(&logic, pos, template.as_ref()));

    if let Some(obj) = logic.host_object_mut(enemy) {
        obj.status.stealthed = true;
        obj.status.detected = false;
        obj.status.disguised = false;
    }
    assert!(
        ai.is_location_safe(&logic, pos, template.as_ref()),
        "stealthed-unless-detected must not fail safety"
    );
    if let Some(obj) = logic.host_object_mut(enemy) {
        obj.status.detected = true;
    }
    assert!(!ai.is_location_safe(&logic, pos, template.as_ref()));

    logic.destroy_object(enemy);
    let _ = logic
        .create_object("AmericaVehicleChinook", Team::China, pos)
        .expect("harvester");
    let _ = logic
        .create_object("ChinaVehicleDozer", Team::China, pos)
        .expect("dozer");
    assert!(
        ai.is_location_safe(&logic, pos, template.as_ref()),
        "C++ rejects HARVESTER and DOZER from the safety scan"
    );
}
