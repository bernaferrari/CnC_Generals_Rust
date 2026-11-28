//! ScriptActions.cpp:1082–1087,2284–2310 through real borrowed host admission.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use generals_sim::scripting::program::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::ScriptEngine;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Owned builder display", false)
        .with_script_identity("OwnedBuilder");
    player.resources.supplies = 10_000;
    player.set_can_build_units(true);
    world.add_player(player);
    world.add_ai_opponent(1, Team::USA, crate::ai::AIDifficulty::Medium);
    // Exercise the authored admission, not the constructor's default layout pad.
    world
        .ai_manager
        .ai_players
        .get_mut(&1)
        .unwrap()
        .building_queue
        .clear();
    world.get_player_mut(1).unwrap().set_can_build_units(true);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.set_cost(225, 0);
    world.templates.insert(ranger.name.clone(), ranger);
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks.set_cost(500, 0).add_kind_of(KindOf::Structure);
    world.templates.insert(barracks.name.clone(), barracks);
    world
        .create_object_for_player("AmericaBarracks", 1, Vec3::ZERO)
        .unwrap();
    let unit = world
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    world.host_object_mut(unit).unwrap().is_recruitable = true;
    let mut pile = ThingTemplate::new("SupplyWarehouse");
    pile.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Harvestable)
        .add_kind_of(KindOf::Resource)
        .add_kind_of(KindOf::SupplySource);
    pile.dock_kind = DockKind::SupplyWarehouse;
    world.templates.insert(pile.name.clone(), pile);
    let mut center = ThingTemplate::new("AmericaSupplyCenter");
    center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter);
    world.templates.insert(center.name.clone(), center);
    let warehouse = world
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(200.0, 0.0, 0.0))
        .unwrap();
    world
        .host_object_mut(warehouse)
        .unwrap()
        .set_warehouse_boxes(50);
    {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "OwnedSquad".into(),
                "OwnedBuilder".into(),
                false,
                None,
                Some(1),
            )
            .unwrap();
        let mut prototype = (*factory.find_team_prototype("OwnedSquad").unwrap()).clone();
        prototype.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 1,
                max_units: 1,
                unit_thing_name: "AmericaInfantryRanger".into(),
            },
        );
        prototype.set_production_priority(100);
        factory.replace_team_prototype(prototype);
        let admitted = factory.find_team_prototype("OwnedSquad").unwrap();
        assert_eq!(admitted.units_info().len(), 1);
        assert_eq!(
            admitted.units_info()[0].unit_thing_name,
            "AmericaInfantryRanger"
        );
        assert_eq!(admitted.units_info()[0].min_units, 1);
        assert_eq!(admitted.units_info()[0].max_units, 1);
        assert_eq!(admitted.get_owner_name().as_str(), "OwnedBuilder");
        assert_eq!(admitted.get_production_priority(), 100);
    }
    (world, unit, warehouse)
}

fn action(kind: ScriptActionType, token_type: ParameterType, token: &str) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(token_type, token.into()))
        .unwrap();
    action
}

fn run(engine: &ScriptEngine, world: &mut GameLogic, action: &ScriptAction) {
    let frame = world.frame;
    engine.friend_execute_action_with_driver(
        action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(frame),
        &mut HostScriptExecutionDriver::new(world),
    );
}

#[test]
fn authored_build_recruit_and_supply_actions_mutate_only_the_borrowed_world() {
    let (mut owner, unit, warehouse) = world();
    let (peer, peer_unit, peer_warehouse) = world();
    assert_eq!(unit, peer_unit);
    assert_eq!(warehouse, peer_warehouse);
    let peer_team_before = peer.host_object(peer_unit).unwrap().team_instance_id;
    let peer_before =
        serde_json::to_vec(&peer.ai_manager.ai_players[&1].capture_queue_persist()).unwrap();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    run(
        &engine,
        &mut owner,
        &action(
            ScriptActionType::BuildTeam,
            ParameterType::Team,
            "OwnedSquad",
        ),
    );
    let queued = owner.ai_manager.ai_players[&1]
        .team_queue
        .front()
        .expect("actual AI build queue");
    assert_eq!(queued.name, "OwnedSquad");
    assert!(queued.priority_build);
    assert_eq!(queued.work_orders[0].num_required, 1);
    let mut recruit = action(
        ScriptActionType::RecruitTeam,
        ParameterType::Team,
        "OwnedSquad",
    );
    recruit
        .add_parameter(Parameter::with_real(ParameterType::Real, 250.0))
        .unwrap();
    run(&engine, &mut owner, &recruit);
    let ready = owner.ai_manager.ai_players[&1]
        .team_ready_queue
        .front()
        .expect("actual recruited team");
    // CPP AIPlayer.cpp2593-2601 prepends the ready team with NULL workOrders.
    // Its canonical roster, not a duplicate observed list, owns membership.
    assert!(ready.work_orders.is_empty());
    assert_eq!(ready.frame_started, owner.frame);
    let recruited_id = ready.team_id.expect("exact recruited instance");
    assert_eq!(
        owner.host_object(unit).unwrap().team_instance_id,
        Some(recruited_id)
    );
    assert_eq!(
        owner.host_object(unit).unwrap().team_instance_name,
        "OwnedSquad"
    );
    assert_eq!(owner.host_object(unit).unwrap().owner_player_id, Some(1));
    let recruited = owner
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(recruited_id)
        .unwrap();
    assert_eq!(recruited.read().unwrap().get_members(), &[unit.0]);
    assert_eq!(
        peer.host_object(peer_unit).unwrap().team_instance_id,
        peer_team_before
    );
    let mut supply = action(
        ScriptActionType::AiPlayerBuildSupplyCenter,
        ParameterType::Side,
        "OwnedBuilder",
    );
    supply
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaSupplyCenter".into(),
        ))
        .unwrap();
    supply
        .add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();
    run(&engine, &mut owner, &supply);
    let pad = owner.ai_manager.ai_players[&1]
        .building_queue
        .iter()
        .find(|pad| pad.template_name == "AmericaSupplyCenter")
        .expect("actual supply construction pad");
    assert!(pad.is_priority);
    assert!((pad.position - Vec3::new(200.0, 0.0, 0.0)).length() < 80.0);
    assert_eq!(
        serde_json::to_vec(&peer.ai_manager.ai_players[&1].capture_queue_persist()).unwrap(),
        peer_before
    );
    assert!(
        peer.host_object(peer_unit)
            .unwrap()
            .requested_destination
            .is_none()
    );
    let before = owner.ai_manager.ai_players[&1].team_queue.len();
    run(
        &engine,
        &mut owner,
        &action(
            ScriptActionType::BuildTeam,
            ParameterType::Team,
            "MissingPrototype",
        ),
    );
    assert_eq!(owner.ai_manager.ai_players[&1].team_queue.len(), before);
}

#[test]
fn supply_script_acquires_enemy_at_deadline_in_only_the_receiving_world() {
    fn prepared_world() -> GameLogic {
        let (mut world, _, _) = world();
        let own = world.get_player_mut(1).unwrap();
        own.map_side.is_skirmish_ai = true;
        own.alliance_team = 1;
        let mut enemy = Player::new(2, Team::China, "Enemy", false);
        enemy.alliance_team = 2;
        world.add_player(enemy);
        let mut structure = ThingTemplate::new("EnemyStructure");
        structure.add_kind_of(KindOf::Structure);
        world.templates.insert(structure.name.clone(), structure);
        world
            .create_object_for_player("EnemyStructure", 2, Vec3::new(200.0, 0.0, 0.0))
            .unwrap();
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.base_center = Vec3::ZERO;
        ai.enemy_player_id = None;
        ai.frame_to_check_enemy = 150;
        world.frame = 149;
        world
    }

    let mut first = prepared_world();
    let mut second = prepared_world();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut supply = action(
        ScriptActionType::AiPlayerBuildSupplyCenter,
        ParameterType::Side,
        "OwnedBuilder",
    );
    supply
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaSupplyCenter".into(),
        ))
        .unwrap();
    supply
        .add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();

    // Before the deadline no enemy has been acquired, so the warehouse can
    // be selected. The other same-ID world's controller is untouched.
    run(&engine, &mut first, &supply);
    assert_eq!(first.ai_manager.ai_players[&1].enemy_player_id, None);
    assert_eq!(first.ai_manager.ai_players[&1].frame_to_check_enemy, 150);
    assert!(first.ai_manager.ai_players[&1]
        .building_queue
        .iter()
        .any(|pad| pad.template_name == "AmericaSupplyCenter"));
    assert!(second.ai_manager.ai_players[&1].building_queue.is_empty());
    assert_eq!(second.ai_manager.ai_players[&1].frame_to_check_enemy, 150);
    first.ai_manager.ai_players.get_mut(&1).unwrap().building_queue.clear();

    // At the boundary the same real action acquires the enemy before supply
    // selection; its structure at the warehouse makes that warehouse unsafe.
    first.frame = 150;
    run(&engine, &mut first, &supply);
    assert_eq!(first.ai_manager.ai_players[&1].enemy_player_id, Some(2));
    assert_eq!(first.ai_manager.ai_players[&1].frame_to_check_enemy, 300);
    assert!(first.ai_manager.ai_players[&1].building_queue.is_empty());
    assert_eq!(second.ai_manager.ai_players[&1].enemy_player_id, None);
    assert_eq!(second.ai_manager.ai_players[&1].frame_to_check_enemy, 150);

    second.frame = 150;
    run(&engine, &mut second, &supply);
    assert_eq!(second.ai_manager.ai_players[&1].enemy_player_id, Some(2));
    assert_eq!(second.ai_manager.ai_players[&1].frame_to_check_enemy, 300);
    assert!(second.ai_manager.ai_players[&1].building_queue.is_empty());
}

#[test]
fn ai_match_phase_queues_research_only_after_the_authored_action() {
    use crate::game_logic::host_upgrade_rules::register_test_upgrade;
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

    let (mut owner, _, _) = world();
    let producer = owner
        .create_object_for_player("AmericaSupplyCenter", 1, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let cost = 321;
    register_test_upgrade(&mut owner, UPGRADE_AMERICA_SUPPLY_LINES, "PLAYER", cost, 7);
    owner.get_player_mut(1).unwrap().science_purchase_points = 0;
    let money_before = owner.get_player(1).unwrap().effective_supplies();

    owner.frame = 2;
    crate::ai::AIManager::update_owned_with_effects(&mut owner, 2.0 / 30.0, &mut Default::default())
        .unwrap();
    assert_eq!(
        owner.get_player(1).unwrap().effective_supplies(),
        money_before
    );
    assert!(owner.host_upgrades().entries_snapshot().is_empty());
    assert!(
        owner
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );

    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut upgrade = action(
        ScriptActionType::AiPlayerBuildUpgrade,
        ParameterType::Side,
        "OwnedBuilder",
    );
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            UPGRADE_AMERICA_SUPPLY_LINES.into(),
        ))
        .unwrap();
    run(&engine, &mut owner, &upgrade);
    let player = owner.get_player(1).unwrap();
    assert_eq!(player.effective_supplies(), money_before - cost);
    assert!(player.has_queued_upgrade(UPGRADE_AMERICA_SUPPLY_LINES));
    let queue = &owner
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue;
    assert_eq!(queue.len(), 1);
    assert!(queue[0].is_upgrade());
    assert_eq!(queue[0].template_name, UPGRADE_AMERICA_SUPPLY_LINES);
}

#[test]
fn ordinary_script_supply_uses_warehouse_and_script_enemy_context_for_priority_rows() {
    use crate::ai::{AIDifficulty, OrdinaryAiTiming};
    use crate::fixture_content::{FixtureObject, admit_fixture_content_with_actions};
    use game_engine::common::game_common::Relationship;

    let mut world = GameLogic::new();
    world.override_world_size(1000.0, 1000.0);
    admit_fixture_content_with_actions(
        &mut world,
        &[
            FixtureObject::new("CampaignSupplyWarehouse")
                .kind_of("STRUCTURE IMMOBILE SUPPLY_SOURCE")
                .field("Geometry = CYLINDER")
                .field("GeometryMajorRadius = 10")
                .module(
                    "Behavior = SupplyWarehouseDockUpdate Dock
 StartingBoxes = 4
 End",
                ),
            FixtureObject::new("CampaignSupplyFake").kind_of("STRUCTURE IMMOBILE SUPPLY_SOURCE"),
            FixtureObject::new("CampaignSupplyCenter")
                .kind_of("STRUCTURE IMMOBILE CASH_GENERATOR")
                .field("Geometry = BOX")
                .field("GeometryMajorRadius = 4")
                .field("GeometryMinorRadius = 3")
                .field("PlacementViewAngle = 30"),
            FixtureObject::new("CampaignSupplyGuard")
                .kind_of("STRUCTURE IMMOBILE")
                .field("Geometry = BOX")
                .field("GeometryMajorRadius = 4")
                .field("GeometryMinorRadius = 3")
                .field("PlacementViewAngle = 60"),
            FixtureObject::new("CampaignSupplyCover")
                .kind_of("STRUCTURE IMMOBILE")
                .field("Geometry = BOX")
                .field("GeometryMajorRadius = 220")
                .field("GeometryMinorRadius = 220"),
        ],
        &[],
        "",
    );
    for (id, faction, name, human) in [
        (1, Team::USA, "SupplyBuilder", false),
        (2, Team::USA, "SameFactionEnemy", false),
        (3, Team::China, "SupplyCaller", false),
        (4, Team::GLA, "FirstHuman", true),
        (5, Team::China, "EnemyCacheDecoy", true),
    ] {
        world.add_player(Player::new(id, faction, name, human).with_script_identity(name));
    }
    world
        .get_player_mut(1)
        .unwrap()
        .set_map_relationship(2, Relationship::Enemies);
    world
        .get_player_mut(1)
        .unwrap()
        .set_map_relationship(3, Relationship::Allies);
    world.get_player_mut(1).unwrap().can_build_base = false;
    world
        .ai_manager
        .add_ai_player(1, Team::USA, AIDifficulty::Medium);
    let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
    ai.building_queue.clear();
    ai.base_center = Vec3::ZERO;
    ai.enemy_player_id = Some(5);
    ai.ordinary_timing = Some(OrdinaryAiTiming {
        ready_to_build_team: false,
        ready_to_build_structure: false,
        team_timer: 2,
        structure_timer: 2,
        build_delay: 11,
        team_delay: 0,
        team_seconds: 10,
        frame_last_building_built: 0,
        bridge_timer: 0,
    });
    let fake = world
        .create_object_for_player("CampaignSupplyFake", 3, Vec3::new(50.0, 0.0, 30.0))
        .unwrap();
    world.host_object_mut(fake).unwrap().set_stored_supplies(900);
    world
        .create_object_for_player("CampaignSupplyWarehouse", 2, Vec3::new(70.0, 0.0, 0.0))
        .unwrap();
    let earlier = world
        .create_object_for_player("CampaignSupplyWarehouse", 3, Vec3::new(200.0, 0.0, 80.0))
        .unwrap();
    let warehouse_position = Vec3::new(200.0, 0.0, -80.0);
    let warehouse = world
        .create_object_for_player("CampaignSupplyWarehouse", 3, warehouse_position)
        .unwrap();
    assert_eq!(
        world
            .host_object(warehouse)
            .unwrap().warehouse_boxes(),
        4
    );
    for x in [-310.0, -280.0, -110.0] {
        world
            .create_object_for_player("CampaignSupplyGuard", 4, Vec3::new(x, 0.0, -80.0))
            .unwrap();
    }
    world
        .create_object_for_player("CampaignSupplyGuard", 5, Vec3::new(420.0, 0.0, -80.0))
        .unwrap();

    let engine = ScriptEngine::new_in_published_world().unwrap();
    let caller = world
        .get_player(3)
        .unwrap()
        .default_team_id
        .expect("owned caller Team");
    let supply_action = |building: &str, cash| {
        let mut order = action(
            ScriptActionType::AiPlayerBuildSupplyCenter,
            ParameterType::Side,
            "SupplyBuilder",
        );
        order
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                building.into(),
            ))
            .unwrap();
        order
            .add_parameter(Parameter::with_int(ParameterType::Int, cash))
            .unwrap();
        order
    };

    // 550 halves to 275, admitting the two authored 300-cash allied piles.
    // Their tie keeps the first world-record member, the newer Object.
    run(
        &engine,
        &mut world,
        &supply_action("CampaignSupplyCenter", 550),
    );
    let ai = &world.ai_manager.ai_players[&1];
    assert_eq!(ai.capture_queue_persist().current_warehouse_id, Some(warehouse.0));
    let [site] = ai.building_queue.as_slice() else {
        panic!("expected a single supply site");
    };
    assert!(site.is_priority);
    assert_eq!(site.max_rebuilds, 1);
    assert_eq!(site.position.y, 0.0);
    let expected = warehouse_position - warehouse_position.normalize() * 30.0;
    assert!(site.position.distance(expected) < 0.001);
    let angle = site.ordinary.as_deref().unwrap().angle;
    assert!((angle - std::f32::consts::FRAC_PI_6).abs() < 0.00001);

    world
        .host_object_mut(warehouse)
        .unwrap()
        .set_warehouse_boxes(0);
    world
        .host_object_mut(earlier)
        .unwrap()
        .set_warehouse_boxes(9);
    let guard = supply_action("CampaignSupplyGuard", 550);
    let context = gamelogic::scripting::executor::ScriptContext::at_frame(world.frame);
    engine.friend_execute_action_with_driver(
        &guard,
        Some(caller),
        context,
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    let ai = &world.ai_manager.ai_players[&1];
    assert_eq!(ai.capture_queue_persist().current_warehouse_id, Some(warehouse.0));
    let [guard_site, supply_site] = ai.building_queue.as_slice() else {
        panic!("the defensive row should precede the supply row");
    };
    assert_eq!(guard_site.template_name, "CampaignSupplyGuard");
    assert_eq!(supply_site.template_name, "CampaignSupplyCenter");
    // Pure west through the real script context's first human. The 10-unit
    // offset overlaps the warehouse, so the ring walk accepts its first corner.
    assert_eq!(guard_site.position, Vec3::new(180.0, 0.0, -90.0));
    assert!(guard_site.is_priority);
    assert_eq!(guard_site.max_rebuilds, 1);
    let angle = guard_site.ordinary.as_deref().unwrap().angle;
    assert!((angle - std::f32::consts::FRAC_PI_3).abs() < 0.00001);

    world
        .create_object_for_player("CampaignSupplyCover", 3, Vec3::new(190.0, 0.0, -80.0))
        .unwrap();
    let context = gamelogic::scripting::executor::ScriptContext::at_frame(world.frame);
    engine.friend_execute_action_with_driver(
        &guard,
        Some(caller),
        context,
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    let ai = &world.ai_manager.ai_players[&1];
    assert_eq!(ai.building_queue.len(), 3);
    assert_eq!(ai.building_queue[0].position, Vec3::new(190.0, 0.0, -80.0));
    assert_eq!(ai.ordinary_timing.unwrap().build_delay, 11);
}

#[test]
fn ordinary_script_nearest_team_queues_one_typed_priority_pad_only_after_relocation() {
    use super::condition_cursor_control_tests::admit;
    use crate::ai::{AIBuildingInfo, AIDifficulty, OrdinaryAiTiming};
    use crate::fixture_content::{FixtureObject, admit_fixture_content};

    let mut world = GameLogic::new();
    admit_fixture_content(
        &mut world,
        &[
            FixtureObject::new("NearestTeamMember").kind_of("VEHICLE SELECTABLE"),
            FixtureObject::new("NearestTeamBlocker")
                .kind_of("STRUCTURE IMMOBILE")
                .field("Geometry = BOX")
                .field("GeometryMajorRadius = 1")
                .field("GeometryMinorRadius = 1"),
            FixtureObject::new("NearestTeamPad")
                .kind_of("STRUCTURE IMMOBILE")
                .field("Geometry = BOX")
                .field("GeometryMajorRadius = 4")
                .field("GeometryMinorRadius = 3")
                .field("PlacementViewAngle = 45"),
        ],
        &[],
    );
    let mut player = Player::new(1, Team::USA, "Ordinary builder", false)
        .with_script_identity("OrdinaryBuilder");
    player.can_build_base = false;
    world.add_player(player);
    world
        .ai_manager
        .add_ai_player(1, Team::USA, AIDifficulty::Medium);
    let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
    ai.building_queue = vec![AIBuildingInfo::new(
        "NearestTeamBlocker".into(),
        Vec3::new(-100.0, 0.0, -100.0),
        0,
    )];
    ai.ordinary_timing = Some(OrdinaryAiTiming {
        ready_to_build_team: false,
        ready_to_build_structure: false,
        team_timer: 2,
        structure_timer: 2,
        build_delay: 0,
        team_delay: 0,
        team_seconds: 10,
        frame_last_building_built: 0,
        bridge_timer: 0,
    });

    let earlier = world
        .create_object_for_player("NearestTeamMember", 1, Vec3::new(70.0, 0.0, 130.0))
        .unwrap();
    let position = Vec3::new(150.0, 0.0, 130.0);
    let head = world
        .create_object_for_player("NearestTeamMember", 1, position)
        .unwrap();
    admit(&world, "NearestCampaignTeam", 77, None);
    world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(77)
        .unwrap()
        .write()
        .unwrap()
        .restore_owned_members(&[head.0, earlier.0]);
    assert_eq!(
        world.resolve_owned_script_team("NearestCampaignTeam", None, None).unwrap(),
        Some((77, vec![head.0, earlier.0]))
    );

    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut nearest = action(
        ScriptActionType::AiPlayerBuildTypeNearestTeam,
        ParameterType::Side,
        "OrdinaryBuilder",
    );
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "NearestTeamPad".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "NearestCampaignTeam".into(),
        ))
        .unwrap();

    run(&engine, &mut world, &nearest);
    assert_eq!(world.ai_manager.ai_players[&1].building_queue.len(), 1);

    world
        .create_object_for_player("NearestTeamBlocker", 1, position)
        .unwrap();
    run(&engine, &mut world, &nearest);

    let ai = &world.ai_manager.ai_players[&1];
    let [pad, previous] = ai.building_queue.as_slice() else {
        panic!("a relocated priority row should precede the old row");
    };
    assert_eq!(previous.template_name, "NearestTeamBlocker");
    assert_eq!(pad.template_name, "NearestTeamPad");
    assert_eq!(pad.position, Vec3::new(140.0, 0.0, 120.0));
    assert!(pad.is_priority && pad.automatic_build);
    assert_eq!(
        (pad.rebuild_count, pad.max_rebuilds, pad.object_id),
        (0, 1, None)
    );
    let ordinary = pad
        .ordinary
        .as_deref()
        .expect("receiving ordinary BuildList fields");
    assert!((ordinary.angle - std::f32::consts::FRAC_PI_4).abs() < 0.00001);
    assert_eq!(ordinary.object_timestamp, 0);
    assert!(!ordinary.under_construction && !ordinary.initially_built);
    assert_eq!(ai.ordinary_timing.unwrap().build_delay, 0);
}

#[test]
fn upgrade_and_nearest_team_actions_apply_immediately_to_actual_owner() {
    use super::condition_cursor_control_tests::admit;
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    admit(&owner, "NearestOwnedTeam", 77, Some(unit));
    admit(&peer, "NearestOwnedTeam", 77, Some(peer_unit));
    let before = peer.ai_manager.ai_players[&1].building_queue.len();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut nearest = action(
        ScriptActionType::AiPlayerBuildTypeNearestTeam,
        ParameterType::Side,
        "OwnedBuilder",
    );
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaPatriotBattery".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "NearestOwnedTeam".into(),
        ))
        .unwrap();
    run(&engine, &mut owner, &nearest);
    let pad = owner.ai_manager.ai_players[&1]
        .building_queue
        .last()
        .unwrap();
    assert_eq!(pad.template_name, "AmericaPatriotBattery");
    assert_eq!(
        pad.position,
        owner.host_object(unit).unwrap().get_position()
    );
    assert_eq!(peer.ai_manager.ai_players[&1].building_queue.len(), before);
    let mut upgrade = action(
        ScriptActionType::AiPlayerBuildUpgrade,
        ParameterType::Side,
        "OwnedBuilder",
    );
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_AmericaRangerCaptureBuilding".into(),
        ))
        .unwrap();
    run(&engine, &mut owner, &upgrade);
    assert!(
        owner
            .get_player(1)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
    assert!(
        !peer
            .get_player(1)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
    assert_eq!(owner.ai_manager.ai_players[&1].activity_count, 1);
    assert_eq!(owner.ai_manager.ai_players.len(), 1);
    let money = owner.get_player(1).unwrap().resources.supplies;
    run(&engine, &mut owner, &upgrade);
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, money); // already queued admission rejected
}

#[test]
fn approach_move_uses_owned_team_and_owned_enemy_numbered_waypoint() {
    use super::condition_cursor_control_tests::admit;
    let (mut owner, unit, _) = super::named_command_test_support::world();
    let (mut peer, peer_unit, _) = super::named_command_test_support::world();
    assert_eq!(unit, peer_unit);
    for (logic, x) in [(&mut owner, 200.0), (&mut peer, 800.0)] {
        admit(logic, "ActualApproach", 77, Some(unit));
        logic.get_player_mut(1).unwrap().is_human = false;
        let enemy = logic.get_player_mut(2).unwrap();
        enemy.is_human = true;
        enemy.start_position = 1;
        let mut map = game_engine::map_data::MapData::new();
        map.width = 100;
        map.height = 100;
        map.heightmap = vec![0; 10000];
        map.waypoints
            .push(game_engine::map_data::MapWaypoint {
                id: 71,
                name: "OwnedApproachPoint".into(),
                location: glam::Vec3::new(x, 60.0, 0.0),
                path_label1: "ActualPath2".into(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            });
        logic
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
    }
    let mut action = action(
        ScriptActionType::SkirmishMoveToApproachPath,
        ParameterType::Team,
        "ActualApproach",
    );
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "ActualPath".into(),
        ))
        .unwrap();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    run(&engine, &mut owner, &action);
    assert_eq!(owner.ai_group_members_of(unit), Some(vec![unit]));
    assert_eq!(owner.host_object(unit).unwrap().requested_destination, None);
    let context = gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame);
    engine
        .friend_execute_action_with_driver_checked(
            &action,
            Some(77),
            context,
            &mut HostScriptExecutionDriver::new(&mut owner),
        )
        .unwrap();
    assert_eq!(
        owner.host_object(unit).unwrap().requested_destination,
        Some(Vec3::new(200.0, 0.0, 60.0))
    );
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );
}

#[test]
fn front_defense_script_admits_actual_layout_row_in_supplied_ai() {
    let (mut owner, _, _) = world();
    let (peer, _, _) = world();
    let mut defense = ThingTemplate::new("OwnedDefense");
    defense
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0);
    owner.templates.insert(defense.name.clone(), defense);
    owner.world_min = Vec3::splat(-1000.0);
    owner.world_max = Vec3::splat(1000.0);
    owner.ai_manager.ai_players.get_mut(&1).unwrap().base_center = Vec3::ZERO;
    let before = owner.ai_manager.ai_players[&1].building_queue.len();
    let peer_before = peer.ai_manager.ai_players[&1].building_queue.len();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    super::condition_cursor_control_tests::admit(&owner, "DefenseCaller", 77, None);
    engine.friend_execute_action_with_driver(
        &action(
            ScriptActionType::SkirmishBuildStructureFront,
            ParameterType::ObjectType,
            "OwnedDefense",
        ),
        Some(77),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut owner),
    );
    assert_eq!(
        owner.ai_manager.ai_players[&1].building_queue.len(),
        before + 1
    );
    assert_eq!(
        owner.ai_manager.ai_players[&1]
            .building_queue
            .last()
            .unwrap()
            .template_name,
        "OwnedDefense"
    );
    assert_eq!(
        peer.ai_manager.ai_players[&1].building_queue.len(),
        peer_before
    );
}

#[test]
fn panic_action_uses_actual_team_member_and_own_closest_waypoint() {
    use super::condition_cursor_control_tests::admit;
    let (mut owner, unit, foreign) = super::named_command_test_support::world();
    let (mut peer, peer_unit, _) = super::named_command_test_support::world();
    for logic in [&mut owner, &mut peer] {
        admit(logic, "OwnedPanic", 88, Some(unit));
        let mut map = game_engine::map_data::MapData::new();
        map.width = 100;
        map.height = 100;
        map.heightmap = vec![0; 10000];
        for (id, x) in [(81, 60.0), (82, 500.0)] {
            map.waypoints
                .push(game_engine::map_data::MapWaypoint {
                    id,
                    name: format!("Panic{id}"),
                    location: glam::Vec3::new(x, 50.0, 0.0),
                    path_label1: "ActualPanicPath".into(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                });
        }
        logic
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
    }
    let mut panic = action(
        ScriptActionType::TeamPanic,
        ParameterType::Team,
        "OwnedPanic",
    );
    panic
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "MissingPath".into(),
        ))
        .unwrap();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    run(&engine, &mut owner, &panic);
    assert!(owner.host_object(unit).unwrap().panic_runtime.is_none());
    panic.parameters[1].as_mut().unwrap().string_value = "ActualPanicPath".into();
    run(&engine, &mut owner, &panic);
    assert_eq!(
        owner
            .host_object(unit)
            .unwrap()
            .panic_runtime
            .as_ref()
            .unwrap()
            .current_waypoint_id,
        81
    );
    assert_eq!(owner.host_object(unit).unwrap().ai_state(), AIState::Panic);
    assert!(owner.host_object(foreign).unwrap().panic_runtime.is_none());
    assert!(peer.host_object(peer_unit).unwrap().panic_runtime.is_none());
}

#[test]
fn partial_button_filters_actual_valid_members_before_percentage_without_readiness_gate() {
    use crate::command_system::SpecialPowerType;
    use game_engine::common::ini::ini_command_button::CommandButton;
    let (mut owner, first, invalid) = super::named_command_test_support::world();
    let (peer, _, _) = super::named_command_test_support::world();
    let second = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(80.0, 0.0, 50.0))
        .unwrap();
    for id in [first, second] {
        owner
            .host_object_mut(id)
            .unwrap()
            .template_mut()
            .special_power_modules
            .push(SpecialPowerModuleMetadata {
                source_index: 0,
                module_tag: None,
                module_kind: SpecialPowerModuleKind::SpecialAbility,
                special_power_template: "OwnerCia".into(),
                special_power_template_id: 99,
                command_power: Some(SpecialPowerType::CiaIntelligence),
                reload_time_frames: 1800,
                required_science: None,
                public_timer: false,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
                cash_bounty: None,
            });
        owner
            .host_object_mut(id)
            .unwrap()
            .set_special_power_ready_seconds(&SpecialPowerType::CiaIntelligence, 999.0);
    }
    let mut button = CommandButton::new("OwnerCiaButton".into());
    button.special_power_template = Some("OwnerCia".into());
    let roster = [invalid.0, first.0, second.0];
    assert_eq!(
        owner.owned_partial_button_members(&roster, &button, 50.0),
        vec![first]
    );
    assert_eq!(
        owner.owned_partial_button_members(&roster, &button, 100.0),
        vec![first, second]
    );
    assert!(
        peer.owned_partial_button_members(&roster, &button, 100.0)
            .is_empty()
    );
    assert!(
        owner
            .owned_partial_button_members(
                &roster,
                &CommandButton::new("Command_Stop".into()),
                100.0
            )
            .is_empty()
    );
    button.options_bits =
        gamelogic::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_POS
            .bits();
    assert!(
        owner
            .owned_partial_button_members(&roster, &button, 100.0)
            .is_empty()
    );
}

#[test]
fn partial_authored_button_executes_actual_power_for_first_valid_owned_team_member() {
    use super::condition_cursor_control_tests::admit;
    use crate::command_system::SpecialPowerType;
    use game_engine::common::ini::ini_command_button::CommandButton;
    let (mut owner, first, _) = super::named_command_test_support::world();
    let (peer, peer_first, _) = super::named_command_test_support::world();
    assert_eq!(first, peer_first);
    let second = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(80.0, 0.0, 50.0))
        .unwrap();
    let invalid = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(90.0, 0.0, 50.0))
        .unwrap();
    for id in [first, second] {
        owner
            .host_object_mut(id)
            .unwrap()
            .template_mut()
            .special_power_modules
            .push(SpecialPowerModuleMetadata {
                source_index: 0,
                module_tag: None,
                module_kind: SpecialPowerModuleKind::SpyVisionSpecialPower,
                special_power_template: "OwnerCia".into(),
                special_power_template_id: 99,
                command_power: Some(SpecialPowerType::CiaIntelligence),
                reload_time_frames: 1800,
                required_science: None,
                public_timer: false,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
                cash_bounty: None,
            });
        owner
            .host_object_mut(id)
            .unwrap()
            .set_special_power_ready_seconds(&SpecialPowerType::CiaIntelligence, 0.0);
    }
    admit(&owner, "ActualPartial", 91, Some(first));
    owner
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(91)
        .unwrap()
        .write()
        .unwrap()
        .restore_owned_members(&[invalid.0, first.0, second.0]);
    let mut button = CommandButton::new("ArbitraryOwnedCiaAbility".into());
    button.command = "SPECIAL_POWER".into();
    button.special_power_template = Some("OwnerCia".into());
    owner
        .apply_owned_team_partial_button_definition(
            generals_sim::scripting::program::THIS_TEAM,
            &button,
            50.0,
            Some(91),
            None,
        )
        .unwrap();
    assert_eq!(owner.cia_intelligence.activations(), 1);
    assert_eq!(
        owner.cia_intelligence.active_scans()[0].caster_id,
        Some(first)
    );
    assert_eq!(peer.cia_intelligence.activations(), 0);
    assert!(owner.host_object(first).unwrap().special_power_countdown_seconds(&SpecialPowerType::CiaIntelligence) > 0.0);
    assert_eq!(owner.host_object(second).unwrap().special_power_countdown_seconds(&SpecialPowerType::CiaIntelligence), 0.0);
}
