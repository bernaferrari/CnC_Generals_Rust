//! Actual factory delivery and synchronous AI production completion parity.

use super::*;

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
    // C++ Energy.cpp:61-62 — a player with no authored power plants has
    // production=0/consumption=0, which is ratio 1.0 (no low-power penalty).
    // The host recomputes power only in update_player_resources, after the
    // production tick, so the fixture seeds the same powered state directly.
    player.power_produced = 10;
    player.power_consumed = 0;
    // C++ arms m_teamTimer from live money (AIPlayer.cpp:1702-1706): the
    // wealthy/poor rate divisors fire on the post-queue balance.  Seed a
    // mid-range balance (queued units cost 925) so it lands between
    // ResourcesPoor (2000) and ResourcesWealthy (7000) and the unmodified
    // TeamSeconds interval is observable.
    player.resources.supplies = 4_000;
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
        &logic,
        1,
        "USA_BasicForce",
        &[
            (2, 2, "AmericaInfantryRanger"),
            (1, 1, "AmericaVehicleHumvee"),
        ],
        10,
    );

    let mut ai = ai_for_world(&logic, 1, Team::USA, AIDifficulty::Medium);
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
    // C++ arms m_teamTimer from live money (AIPlayer.cpp:1702-1706): the
    // wealthy/poor rate divisors fire on the post-queue balance.  Seed a
    // mid-range balance so the queued 925 cost lands between ResourcesPoor
    // (2000) and ResourcesWealthy (7000) and the unmodified TeamSeconds
    // interval is observable.
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
    // second Ranger required by USA_BasicForce.  Retail AmericaBarracks
    // authors a production door (NumDoorAnimations=1, ProductionUpdate.cpp:
    // 746-776): the finished head starts OPENING on its completing frame and
    // spawns only once that door reads WAITING_OPEN, so a couple of fixed
    // logic frames are required before the producer-linked output exists.
    // Production delivers synchronously to the AI owned by this match.
    // Suppress its ordinary update here so this fixture can explicitly
    // exercise the preexisting military scheduling assertions below.
    ai.is_active = false;
    logic.ai_manager.ai_players.insert(1, ai);
    for _ in 0..3 {
        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    }
    let mut ai = logic
        .ai_manager
        .ai_players
        .remove(&1)
        .expect("receiving AI");
    ai.is_active = true;
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
    // Finish training in one frame; the authored door still gates delivery.
    ranger.build_time = 0.001;
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let factory = logic
        .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
        .expect("constructed barracks");
    let mut ai = ai_for_world(&logic, 1, Team::USA, AIDifficulty::Medium);
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

    // Keep the actual receiving AI in the manager during the production
    // completion; callbacks synchronously update this owned entry.
    ai.is_active = false;
    logic.ai_manager.ai_players.insert(1, ai);
    // ProductionUpdate.cpp:746-776: a completed head opens the retail
    // barracks door first; it cannot deliver until WAITING_OPEN is observed.
    logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    let pending = &logic.ai_manager.ai_players[&1].team_queue[0].work_orders[0];
    assert_eq!(pending.num_completed, 0, "door opening is not delivery");
    assert_eq!(pending.queued_count, 1);
    assert!(
        !logic.host_objects().values().any(|unit| {
            unit.producer_id == Some(factory) && unit.template_name == "AmericaInfantryRanger"
        }),
        "the closed/opening door holds the completed production head"
    );
    for _ in 0..2 {
        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    }
    let delivered = &logic.ai_manager.ai_players[&1].team_queue[0].work_orders[0];
    assert_eq!(delivered.num_completed, 1);
    assert_eq!(delivered.queued_count, 0);
    assert!(
        logic.host_objects().values().any(|unit| {
            unit.producer_id == Some(factory) && unit.template_name == "AmericaInfantryRanger"
        }),
        "the real factory callback delivers after its door opens"
    );
    let mut ai = logic
        .ai_manager
        .ai_players
        .remove(&1)
        .expect("receiving AI");

    ai.process_team_queue(&mut logic, 1.0);
    assert!(
        ai.team_queue.front().is_some_and(|t| t.is_all_built()),
        "team completes at its real factory delivery callback"
    );
    ai.check_queued_teams(&mut logic, 1.0);
    assert!(
        ai.team_queue.is_empty(),
        "all-built teams leave the build queue"
    );
    assert_eq!(ai.team_ready_queue.len(), 1);
    // C++ checkReadyTeams (AIPlayer.cpp:2729-2761) observes the unit's
    // real idle state. Delivery finishes its work order before its factory
    // exit path finishes; the ready team must wait for that path, not 60s.
    let unit = ai.team_ready_queue[0].work_orders[0].observed_unit_ids[0];
    let now = logic.get_frame() as f32 / LOGIC_FRAMES_PER_SECOND;
    ai.check_ready_teams(&mut logic, now);
    assert_eq!(
        ai.team_ready_queue.len(),
        1,
        "the live factory exit is not yet idle"
    );
    for _ in 0..300 {
        if logic.host_object(unit).is_some_and(|unit| {
            matches!(
                unit.ai_state,
                crate::game_logic::AIState::Idle
                    | crate::game_logic::AIState::GuardingArea
                    | crate::game_logic::AIState::GuardingObject
            )
        }) {
            break;
        }
        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    }
    assert!(
        logic.host_object(unit).is_some_and(|unit| {
            matches!(
                unit.ai_state,
                crate::game_logic::AIState::Idle
                    | crate::game_logic::AIState::GuardingArea
                    | crate::game_logic::AIState::GuardingObject
            )
        }),
        "the actual factory exit must settle before the 60-second ready timeout"
    );
    let now = logic.get_frame() as f32 / LOGIC_FRAMES_PER_SECOND;
    assert!(
        now < 60.0,
        "this must exercise idle activation, not forced timeout"
    );
    ai.check_ready_teams(&mut logic, now);
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
    let mut ai = ai_for_world(&logic, 1, Team::USA, AIDifficulty::Medium);
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

    ai.is_active = false;
    logic.ai_manager.ai_players.insert(1, ai);
    logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
    let mut ai = logic
        .ai_manager
        .ai_players
        .remove(&1)
        .expect("receiving AI");
    ai.process_team_queue(&mut logic, 1.0 / LOGIC_FRAMES_PER_SECOND);
    // C++ queueUnits does not promote completed teams — checkQueuedTeams
    // (AIPlayer.cpp:2810-2870) runs beside it in doTeamBuilding and retires
    // the team once every work order has its real output.
    ai.check_queued_teams(&mut logic, 1.0 / LOGIC_FRAMES_PER_SECOND);

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
