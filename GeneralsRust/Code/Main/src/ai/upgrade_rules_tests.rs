//! Production-boundary checks for C++ AIPlayer::buildUpgrade and authored rules.
use super::*;
use crate::game_logic::host_upgrade_rules::register_test_upgrade;
use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;
use gamelogic::system::engine_stores::with_world_services;

fn upgrade_world(kind: &str, cost: u32, seconds: u32) -> (GameLogic, AIPlayer, ObjectId) {
    let mut world = GameLogic::new();
    register_test_upgrade(&world, UPGRADE_AMERICA_SUPPLY_LINES, kind, cost, seconds);
    world.add_player(Player::new(1, Team::USA, "Owned upgrade AI", false));
    // This is the actual retail producer identity and the ordinary authored
    // CommandSet/residual checks remain enabled. Do not bypass canProduceUpgrade.
    let mut template = ThingTemplate::new("AmericaSupplyCenter");
    template.add_kind_of(KindOf::Structure);
    world.templates.insert(template.name.clone(), template);
    let producer = world
        .create_object_for_player("AmericaSupplyCenter", 1, Vec3::ZERO)
        .expect("ready supply center");
    assert!(world.host_object(producer).unwrap().is_constructed());
    assert!(
        world
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    (
        world,
        AIPlayer::new(1, Team::USA, AIDifficulty::Medium),
        producer,
    )
}

fn assert_queued(world: &GameLogic, producer: ObjectId, cost: u32, seconds: u32) {
    let queue = &world
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue;
    assert_eq!(queue.len(), 1);
    assert!(queue[0].is_upgrade());
    assert_eq!(queue[0].template_name, UPGRADE_AMERICA_SUPPLY_LINES);
    assert_eq!(queue[0].cost.supplies, cost);
    assert_eq!(queue[0].total_time, seconds as f32);
    assert_eq!(queue[0].construction_frames, 0);
    let player = world.get_player(1).unwrap();
    assert_eq!(
        player.effective_supplies(),
        Player::DEFAULT_STARTING_MONEY - cost
    );
    assert!(player.has_queued_upgrade(UPGRADE_AMERICA_SUPPLY_LINES));
    let records = world.host_upgrades().entries_snapshot();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].source_object, Some(producer));
    assert_eq!(records[0].build_cost_paid, cost);
    assert_eq!(records[0].retail_research_frames, seconds * 30);
    assert_eq!(records[0].residual_research_frames, seconds * 30);
}

#[test]
fn named_ai_upgrade_queues_from_driving_rules_when_active_world_has_object_type() {
    let (mut player_world, mut player_ai, player_producer) = upgrade_world("PLAYER", 321, 7);
    let (mut object_world, mut object_ai, object_producer) = upgrade_world("OBJECT", 654, 19);
    assert_eq!(player_producer, object_producer);

    let other_stores = std::sync::Arc::clone(&object_world.world_services);
    with_world_services(&other_stores, || {
        assert!(player_ai.build_upgrade(&mut player_world, UPGRADE_AMERICA_SUPPLY_LINES));
    });
    assert_queued(&player_world, player_producer, 321, 7);

    let other_stores = std::sync::Arc::clone(&player_world.world_services);
    with_world_services(&other_stores, || {
        assert!(!object_ai.build_upgrade(&mut object_world, UPGRADE_AMERICA_SUPPLY_LINES));
    });
    assert_eq!(
        object_world.get_player(1).unwrap().effective_supplies(),
        Player::DEFAULT_STARTING_MONEY
    );
    assert!(
        !object_world
            .get_player(1)
            .unwrap()
            .has_queued_upgrade(UPGRADE_AMERICA_SUPPLY_LINES)
    );
    assert!(object_world.host_upgrades().entries_snapshot().is_empty());
    assert!(
        object_world
            .host_object(object_producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
}

#[test]
fn automatic_ai_upgrade_queues_authored_cost_and_time_in_each_driving_world() {
    let (mut first, mut first_ai, first_producer) = upgrade_world("PLAYER", 321, 7);
    let (mut second, mut second_ai, second_producer) = upgrade_world("PLAYER", 654, 19);
    assert_eq!(first_producer, second_producer);
    let other_stores = std::sync::Arc::clone(&second.world_services);
    with_world_services(&other_stores, || {
        first_ai.try_queue_structure_upgrade(&mut first)
    });
    assert_queued(&first, first_producer, 321, 7);
    let other_stores = std::sync::Arc::clone(&first.world_services);
    with_world_services(&other_stores, || {
        second_ai.try_queue_structure_upgrade(&mut second)
    });
    assert_queued(&second, second_producer, 654, 19);
    assert_eq!(first_ai.activity_count, 1);
    assert_eq!(second_ai.activity_count, 1);
}

#[test]
fn automatic_ai_research_accepts_authored_zero_cost_without_charging_player() {
    // AIPlayer.cpp:1728-1803 and ProductionUpdate.cpp:250-284 do not
    // reject an authored zero-cost upgrade.
    let (mut world, mut ai, producer) = upgrade_world("PLAYER", 0, 7);
    ai.try_queue_structure_upgrade(&mut world);
    assert_queued(&world, producer, 0, 7);
    assert_eq!(ai.activity_count, 1);
}
