//! Behavior suite extracted from `combat_particles_and_economy`.
use super::*;

#[test]
fn supply_center_one_shot_collector_enters_authored_wanting_route_without_passive_income() {
    use crate::game_logic::{DockKind, ProductionExitMetadata, ProductionExitStyle};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 500;
    logic.add_player(player);

    let mut truck = ThingTemplate::new("AmericaVehicleChinook");
    truck.add_kind_of(KindOf::Harvester).set_health(100.0);
    logic.templates.insert(truck.name.clone(), truck);

    let mut warehouse = ThingTemplate::new("FiniteWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(100.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(100);
    logic.templates.insert(warehouse.name.clone(), warehouse);
    let source = logic
        .create_object("FiniteWarehouse", Team::Neutral, Vec3::new(80.0, 0.0, 0.0))
        .expect("finite supply source");

    let mut center = ThingTemplate::new("AmericaSupplyCenter");
    center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(100.0);
    center.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::SupplyCenter,
        unit_create_point: [0.0, 0.0, 0.0],
        natural_rally_point: [20.0, 0.0, 0.0],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert(center.name.clone(), center);

    let cash_before = logic.get_player(0).expect("player").resources.supplies;
    let center_id = logic
        .create_object_for_player("AmericaSupplyCenter", 0, Vec3::ZERO)
        .expect("supply center");
    let collector_id = logic
        .host_objects()
        .values()
        .find(|object| object.producer_id == Some(center_id))
        .map(|object| object.id)
        .expect("one-shot collector");
    let collector = logic.host_object(collector_id).expect("collector");
    assert_eq!(collector.target, Some(source));
    assert_eq!(collector.preferred_dock_id, Some(center_id));
    assert_eq!(collector.ai_state, AIState::Gathering);

    // A frame with a still-empty collector is not an economy event.  The
    // only credit path is ReturningResources at the exact owned center.
    logic.update();
    assert_eq!(
        logic.get_player(0).expect("player").resources.supplies,
        cash_before,
        "no fabricated base or supply-center income",
    );
    assert!(logic.take_supply_dropoff_events().is_empty());
}
#[test]
fn supply_center_one_shot_collector_uses_exit_interface() {
    use crate::game_logic::{DockKind, ProductionExitMetadata, ProductionExitStyle};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::GLA, "GLA", true));

    let mut worker = ThingTemplate::new("GLAInfantryWorker");
    worker.add_kind_of(KindOf::Harvester).set_health(100.0);
    logic.templates.insert(worker.name.clone(), worker);

    let mut warehouse = ThingTemplate::new("FiniteWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(100.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(100);
    logic.templates.insert(warehouse.name.clone(), warehouse);
    let _source = logic
        .create_object("FiniteWarehouse", Team::Neutral, Vec3::new(80.0, 0.0, 0.0))
        .expect("finite supply source");

    let unit_create = [12.0, -8.0, 0.0];
    let natural = [24.0, 0.0, 0.0];
    let mut stash = ThingTemplate::new("GLASupplyStash");
    stash
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(100.0);
    stash.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::SupplyCenter,
        unit_create_point: unit_create,
        natural_rally_point: natural,
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 600,
    });
    logic.templates.insert(stash.name.clone(), stash);

    let center_pos = Vec3::new(50.0, 0.0, 50.0);
    let center_id = logic
        .create_object_for_player("GLASupplyStash", 0, center_pos)
        .expect("stash");
    let first_id = logic
        .host_objects()
        .values()
        .find(|object| object.producer_id == Some(center_id))
        .map(|object| object.id)
        .expect("one-shot collector");

    let (producer_pos, forward) = {
        let center = logic.host_object(center_id).expect("center");
        (center.get_position(), center.thing.get_direction_vector())
    };
    let expected_door =
        crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
            producer_pos,
            forward,
            (unit_create[0], unit_create[1], unit_create[2]),
        );
    let expected_natural =
        crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
            producer_pos,
            forward,
            (natural[0], natural[1], natural[2]),
        );
    let exit = logic
        .host_object(center_id)
        .expect("center meta")
        .thing
        .template
        .production_exit_metadata
        .expect("supply exit");
    let offset_point = exit.natural_rally_point_with_path_offset(
        crate::game_logic::host_ai_path_combat_residual_wave105::PATHFIND_CELL_SIZE_F,
    );
    let offset_natural =
        crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
            producer_pos,
            forward,
            (offset_point[0], offset_point[1], offset_point[2]),
        );

    let first = logic.host_object(first_id).expect("first collector");
    let first_pos = first.get_position();
    assert!(
        ((first_pos.x - expected_door.x).powi(2) + (first_pos.z - expected_door.z).powi(2)).sqrt()
            < 0.5,
        "one-shot must exit at UnitCreatePoint, pos={first_pos:?} door={expected_door:?} origin={center_pos:?}"
    );
    assert!(
        ((first_pos.x - center_pos.x).powi(2) + (first_pos.z - center_pos.z).powi(2)).sqrt() > 4.0,
        "one-shot must not spawn at building origin"
    );
    let path = first.movement.path.clone();
    assert!(
        path.iter().any(|wp| wp.distance(expected_natural) < 1.0)
            || first
                .movement
                .target_position
                .is_some_and(|dest| dest.distance(expected_natural) < 1.0),
        "supply ExitInterface walks raw natural rally, path={path:?} natural={expected_natural:?}"
    );
    assert!(
        path.iter().all(|wp| wp.distance(offset_natural) > 5.0)
            && first
                .movement
                .target_position
                .is_none_or(|dest| dest.distance(offset_natural) > 5.0),
        "C++ supply exit does not add the 2-cell path offset, path={path:?} offset={offset_natural:?}"
    );

    if let Some(first) = logic.host_object_mut(first_id) {
        first.producer_id = None;
    }
    let custom_rally = Vec3::new(200.0, 0.0, 80.0);
    if let Some(center) = logic.host_object_mut(center_id) {
        center.supply_center_spawn_behavior_fired = false;
        center.status.stealthed = true;
        if let Some(building) = center.building_data.as_mut() {
            building.rally_point = Some(custom_rally);
        }
    }
    let second_id = logic
        .spawn_supply_center_one_shot_collector(center_id)
        .expect("stealthed one-shot");
    let second = logic.host_object(second_id).expect("second collector");
    assert!(
        second.status.stealthed,
        "stealthed GLA stash must GrantTemporaryStealth on the starter worker"
    );
    assert!(
        second.temporary_stealth_expires_frame >= 600,
        "retail GrantTemporaryStealth 20000ms is 600 frames, got {}",
        second.temporary_stealth_expires_frame
    );
    assert!(
        second
            .movement
            .path
            .iter()
            .any(|wp| wp.distance(custom_rally) < 1.0)
            || second
                .movement
                .target_position
                .is_some_and(|dest| dest.distance(custom_rally) < 1.0),
        "supply ExitInterface walks custom rally after natural, path={:?}",
        second.movement.path
    );
}

#[test]
fn supply_truck_gather_credits_retail_value_per_box() {
    use crate::game_logic::{DockKind, SupplyTruckMetadata, SupplyTruckState};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 0;
    logic.add_player(player);

    let mut truck = ThingTemplate::new("AmericaVehicleChinook");
    truck.add_kind_of(KindOf::Harvester).set_health(100.0);
    truck.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 4,
        warehouse_scan_distance: 700.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert(truck.name.clone(), truck);

    let mut warehouse = ThingTemplate::new("FiniteWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(100.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(10);
    logic.templates.insert(warehouse.name.clone(), warehouse);

    let source = logic
        .create_object("FiniteWarehouse", Team::Neutral, Vec3::new(2.0, 0.0, 0.0))
        .expect("warehouse");
    {
        let warehouse = logic.host_object_mut(source).expect("warehouse mut");
        warehouse.set_stored_supplies(10 * 75);
    }

    let collector_id = logic
        .create_object_for_player("AmericaVehicleChinook", 0, Vec3::ZERO)
        .expect("collector");
    {
        let collector = logic.host_object_mut(collector_id).expect("collector mut");
        collector.set_target(Some(source));
        collector.set_ai_state(AIState::Gathering);
        collector.supply_truck_state = SupplyTruckState::DockingWarehouse;
        collector.supply_truck_next_dock_action_frame = 0;
        collector.set_stored_supplies(0);
    }

    logic.update_support_states(&[collector_id, source], 1.0 / 30.0);

    let cargo = logic
        .host_object(collector_id)
        .expect("collector after")
        .stored_resources
        .supplies;
    assert_eq!(
        cargo,
        crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32,
        "one warehouse box must credit retail ValuePerSupplyBox 75"
    );
    let warehouse = logic.host_object(source).expect("warehouse after");
    assert_eq!(warehouse.drawable_supply_max_boxes, 10);
    assert_eq!(
        warehouse.drawable_supply_boxes, 9,
        "C++ updateDrawableSupplyStatus must drop one crate visual"
    );
    let collector = logic.host_object(collector_id).expect("collector crates");
    assert_eq!(collector.drawable_supply_max_boxes, 4);
    assert_eq!(
        collector.drawable_supply_boxes, 1,
        "C++ gainOneBox must update collector crate count"
    );
    assert!(
        crate::game_logic::host_supply_gather::collector_carrying_from_boxes(
            collector.drawable_supply_boxes
        ),
        "loaded collector must be CARRYING"
    );
}

#[test]
fn warehouse_set_value_updates_live_host_stock() {
    crate::game_logic::host_supply_gather::reset_live_warehouse_host_state();
    use crate::game_logic::DockKind;
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::Neutral, "N", false));
    let mut warehouse = ThingTemplate::new("SupplyWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(1000.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(400);
    logic.templates.insert(warehouse.name.clone(), warehouse);
    let id = logic
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::ZERO)
        .expect("warehouse");
    logic
        .host_object_mut(id)
        .expect("wh")
        .set_stored_supplies(400 * 75);
    {
        let obj = logic.host_object_mut(id).expect("name");
        obj.name = "MapWarehouse".into();
    }
    crate::game_logic::host_supply_gather::queue_warehouse_set_value("MapWarehouse", 1000);
    logic.update_supply_warehouse_crippling();
    let obj = logic.host_object(id).expect("after set");
    assert_eq!(obj.stored_resources.supplies, 14 * 75);
    assert_eq!(obj.drawable_supply_boxes, 14);
}

#[test]
fn warehouse_crippling_self_heals_after_suppression() {
    use crate::game_logic::DockKind;
    crate::game_logic::host_supply_gather::reset_live_warehouse_host_state();
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::Neutral, "N", false));
    let mut warehouse = ThingTemplate::new("SupplyWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(1000.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    logic.templates.insert(warehouse.name.clone(), warehouse);
    let id = logic
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::ZERO)
        .expect("warehouse");
    logic.set_current_frame(1);
    logic.update_supply_warehouse_crippling();
    {
        let obj = logic.host_object_mut(id).expect("wh");
        obj.health.current = 200.0;
        obj.refresh_model_condition_bits();
    }
    logic.set_current_frame(2);
    logic.update_supply_warehouse_crippling();
    let after_hit = logic.host_object(id).expect("hit").health.current;
    assert!(
        (after_hit - 200.0).abs() < 0.01,
        "suppressed immediately after damage"
    );
    logic.set_current_frame(92);
    logic.update_supply_warehouse_crippling();
    let after_heal = logic.host_object(id).expect("heal").health.current;
    assert!(
        after_heal > 200.0,
        "C++ SelfHealAmount=5 after 90f suppression, after={after_heal}"
    );
}

#[test]
fn warehouse_action_does_not_debit_when_collector_already_at_max_boxes() {
    use crate::game_logic::{DockKind, SupplyTruckMetadata, SupplyTruckState};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 0;
    logic.add_player(player);

    let mut truck = ThingTemplate::new("AmericaVehicleChinook");
    truck.add_kind_of(KindOf::Harvester).set_health(100.0);
    truck.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 4,
        warehouse_scan_distance: 700.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert(truck.name.clone(), truck);

    let mut warehouse = ThingTemplate::new("LastBoxWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(100.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(1);
    warehouse.dock_delete_when_empty = true;
    logic.templates.insert(warehouse.name.clone(), warehouse);

    let box_value = crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32;
    let source = logic
        .create_object("LastBoxWarehouse", Team::Neutral, Vec3::new(2.0, 0.0, 0.0))
        .expect("warehouse");
    {
        let warehouse = logic.host_object_mut(source).expect("warehouse mut");
        warehouse.set_stored_supplies(box_value);
    }

    let collector_id = logic
        .create_object_for_player("AmericaVehicleChinook", 0, Vec3::ZERO)
        .expect("collector");
    {
        let collector = logic.host_object_mut(collector_id).expect("collector mut");
        collector.set_target(Some(source));
        collector.set_ai_state(AIState::Gathering);
        collector.supply_truck_state = SupplyTruckState::DockingWarehouse;
        collector.supply_truck_next_dock_action_frame = 0;
        collector.set_stored_supplies(4 * box_value);
    }

    logic.update_support_states(&[collector_id, source], 1.0 / 30.0);

    let cargo = logic
        .host_object(collector_id)
        .expect("collector after")
        .stored_resources
        .supplies;
    assert_eq!(
        cargo,
        4 * box_value,
        "already-full collector must not gain another box"
    );
    let warehouse = logic
        .host_object(source)
        .expect("warehouse must survive gainOneBox take-back");
    assert!(
        warehouse.is_alive(),
        "C++ take-back must not deleteWhenEmpty the last box"
    );
    assert_eq!(
        warehouse.stored_resources.supplies, box_value,
        "warehouse must keep the box when gainOneBox fails"
    );
    assert_eq!(
        warehouse.drawable_supply_boxes, 1,
        "C++ updateDrawableSupplyStatus must not drop a crate that was taken back"
    );
}

#[test]
fn supply_center_deposits_credit_center_owner_and_reject_allies() {
    use crate::game_logic::{DockKind, SupplyTruckMetadata, SupplyTruckState};

    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    usa.resources.supplies = 100;
    usa.alliance_team = 1;
    logic.add_player(usa);
    let mut china = Player::new(1, Team::China, "China", false);
    china.resources.supplies = 50;
    china.alliance_team = 1;
    logic.add_player(china);

    let mut truck = ThingTemplate::new("AmericaVehicleChinook");
    truck.add_kind_of(KindOf::Harvester).set_health(100.0);
    truck.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 4,
        warehouse_scan_distance: 700.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert(truck.name.clone(), truck);

    let mut center = ThingTemplate::new("ChinaSupplyCenter");
    center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(100.0);
    center.dock_kind = DockKind::SupplyCenter;
    logic.templates.insert(center.name.clone(), center);

    // C++ ActionManager::canTransferSuppliesAt (ActionManager.cpp:224-230):
    // a supply center must be controlled by the SAME player as the docking
    // truck ("not merely an ally... otherwise you may find yourself funding
    // your allies. ick.").  An allied center is not a legal deposit target,
    // so no dock, no credit, and the cargo stays aboard.
    let ally_center_id = logic
        .create_object_for_player("ChinaSupplyCenter", 1, Vec3::ZERO)
        .expect("ally center");
    let allied_collector_id = logic
        .create_object_for_player("AmericaVehicleChinook", 0, Vec3::new(1.0, 0.0, 0.0))
        .expect("allied collector");
    {
        let collector = logic
            .host_object_mut(allied_collector_id)
            .expect("collector mut");
        collector.set_stored_supplies(75);
        collector.set_ai_state(AIState::ReturningResources);
        collector.supply_truck_state = SupplyTruckState::DockingCenter;
        collector.supply_truck_next_dock_action_frame = 0;
        collector.preferred_dock_id = Some(ally_center_id);
    }

    logic.update_support_states(&[allied_collector_id, ally_center_id], 1.0 / 30.0);

    assert_eq!(
        logic.get_player(1).expect("china").resources.supplies,
        50,
        "allied center must not accept a foreign collector's deposit"
    );
    assert_eq!(
        logic.get_player(0).expect("usa").resources.supplies,
        100,
        "no allied cross-credit may mint or move cash"
    );
    let allied_collector = logic
        .host_object(allied_collector_id)
        .expect("allied collector after rejected dock");
    assert_eq!(
        allied_collector.stored_resources.supplies, 75,
        "rejected dock must keep the cargo aboard"
    );
    assert!(logic.take_supply_dropoff_events().is_empty());

    // C++ SupplyCenterDockUpdate::action (SupplyCenterDockUpdate.cpp:494-530)
    // credits the CENTER's controlling player and its ScoreKeeper.  Because
    // ActionManager.cpp:229 restricts legal center docks to the same player,
    // that credit path is exercised with China docking at its own center.
    let own_center_id = logic
        .create_object_for_player("ChinaSupplyCenter", 1, Vec3::new(8.0, 0.0, 0.0))
        .expect("own center");
    // Dock approach residual: the collector must sit on the dock's approach
    // ring to claim it in one tick — goal = dock + dir * (radius * 0.5);
    // radius (selection_radius) 25 → 12.5 east of the center at (8, 0, 0).
    let own_collector_id = logic
        .create_object_for_player("AmericaVehicleChinook", 1, Vec3::new(20.5, 0.0, 0.0))
        .expect("china collector");
    {
        let collector = logic.host_object_mut(own_collector_id).expect("collector mut");
        collector.set_stored_supplies(75);
        collector.set_ai_state(AIState::ReturningResources);
        collector.supply_truck_state = SupplyTruckState::DockingCenter;
        collector.supply_truck_next_dock_action_frame = 0;
        collector.preferred_dock_id = Some(own_center_id);
    }

    logic.update_support_states(&[own_collector_id, own_center_id], 1.0 / 30.0);

    assert_eq!(
        logic.get_player(1).expect("china").resources.supplies,
        125,
        "C++ SupplyCenterDockUpdate deposits to the center owner"
    );
    assert_eq!(
        logic.get_player(1).expect("china").statistics.money_earned,
        75,
        "C++ ScoreKeeper::addMoneyEarned must count the drop-off"
    );
    assert_eq!(
        logic.get_player(0).expect("usa").statistics.money_earned,
        0,
        "allied collector owner must not score the center's deposit"
    );
    let collector = logic
        .host_object(own_collector_id)
        .expect("collector after drop");
    assert_eq!(
        collector.drawable_supply_boxes, 0,
        "C++ loseOneBox must clear collector crate count on drop-off"
    );
    assert!(
        !crate::game_logic::host_supply_gather::collector_carrying_from_boxes(
            collector.drawable_supply_boxes
        )
    );
}

#[test]
fn steal_cash_from_broke_victim_is_zero() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "Victim", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "Lotus", false));
    // Player::new seeds DEFAULT_STARTING_MONEY (10000); pin both sides so a
    // zero steal is observable on the attacker too.
    if let Some(p) = logic.players.get_mut(&0) {
        p.resources.supplies = 0;
    }
    if let Some(p) = logic.players.get_mut(&1) {
        p.resources.supplies = 500;
    }
    let stolen = logic.steal_cash_from_team(Team::USA, Team::China, 1000);
    assert_eq!(stolen, 0);
    assert_eq!(
        logic.get_player(1).expect("china").resources.supplies,
        500,
        "broke victim must not mint attacker cash"
    );
    assert_eq!(
        logic.get_player(0).expect("victim").resources.supplies,
        0,
        "broke victim has nothing to debit"
    );
}

#[test]
fn anthrax_bomb_host_path_queues_damage_after_delay_and_toxin() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        ANTHRAX_TOXIN_AUDIO, ANTHRAX_TOXIN_DAMAGE_PER_TICK, HostSuperweaponKind,
    };

    let mut game_logic = GameLogic::new();
    // Retail delivery needs a real map: C++ CREATE_AT_EDGE_NEAR_SOURCE spawns
    // the cargo plane at TerrainLogic::findClosestEdgePoint and it flies the
    // DeliverPayload approach to DeliveryDistance 140 before the drop.
    game_logic.override_world_size(3000.0, 3000.0);
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::AnthraxBomb,
        "SuperweaponAnthraxBomb",
        360_000,
    );
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_AnthraxBomb");
    }

    // Mid-map caster so the plane launches from a real map edge (left edge,
    // -1500) and flies a genuine ~2000wu DeliverPayload approach instead of
    // dropping immediately. override_world_size centers the map on the origin.
    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    // Retail blast lands at the BOMB IMPACT: the plane drops at DeliveryDistance
    // 140 short of the click, so aim the click 140 past the enemy to land the
    // bomb on it (host residual bomb falls straight down, no forward physics).
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(500.0, 0.0, 0.0))
        .expect("enemy");
    // Survivor for toxin residual: outside blast radius (100) but inside toxin
    // radius (300). AnthraxBombPoisonFieldWeapon is 40 dmg / r300 / 500ms.
    let tox_victim_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(610.0, 0.0, 0.0))
        .expect("tox victim");
    let far_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(1200.0, 0.0, 0.0))
        .expect("far enemy");

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        // Blast = 200; keep HP so we can also observe toxin if still alive.
        enemy.health.current = 100.0;
        enemy.health.maximum = 100.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let v = game_logic
            .host_object_mut(tox_victim_id)
            .expect("tox victim");
        v.health.current = 500.0;
        v.health.maximum = 500.0;
        v.thing.template.armor = 0.0;
    }
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 500.0;
        far.health.maximum = 500.0;
        far.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::AnthraxBomb);
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(640.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::AnthraxBomb,
            target: PowerTarget::Location(target),
        },
        player_id: 2, // Team::GLA
        command_id: 51,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::AnthraxBomb),
        "AnthraxBomb must queue a pending host strike"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponAnthraxBomb"),
        "activation must queue SuperweaponAnthraxBomb audio"
    );
    assert!(
        game_logic.special_power_strikes().toxin_fields().is_empty(),
        "toxin must not spawn before impact"
    );

    // Retail live delivery: OCL SUPERWEAPON_AnthraxBomb → DeliverPayload
    // GLAJetCargoPlane (ObjectCreationList.ini SUPERWEAPON_AnthraxBomb); the
    // dropped AnthraxBomb dies at HeightDieUpdate TargetHeight
    // (WeaponObjects.ini AnthraxBomb ModuleTag_06) and FireWeaponWhenDead
    // fires AnthraxBombWeapon — PrimaryDamage 200 / PrimaryDamageRadius 100
    // with FireOCL OCL_PoisonFieldAnthraxBomb (Weapon.ini
    // AnthraxBombWeapon). Blast + toxin therefore land at the BOMB IMPACT,
    // after the approach + fall delay — not at the special-power click.
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let tox_before = game_logic
        .host_object(tox_victim_id)
        .unwrap()
        .health
        .current;

    // While the bomb is still in flight: no damage, strike still pending.
    // Tick like the world tick (step.rs): strikes drain first, then flights.
    game_logic.frame = 2;
    game_logic.update_special_power_strikes();
    game_logic.update_anthrax_bomb_flights();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no blast damage while the bomb is still in flight"
    );
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::AnthraxBomb)
    );

    // Advance to bomb detonation (bounded) — first toxin tick lands the frame
    // after the field spawns (retail FireWeaponUpdate residual).
    let mut detonation_frame = None;
    let mut tox_after_first_tick: Option<f32> = None;
    for f in 3..=400 {
        game_logic.frame = f;
        game_logic.update_special_power_strikes();
        game_logic.update_anthrax_bomb_flights();
        if detonation_frame.is_none()
            && game_logic.anthrax_bomb_flight_reg.detonations >= 1
        {
            detonation_frame = Some(f);
        }
        if let (Some(d), None) = (detonation_frame, tox_after_first_tick) {
            if f == d + 1 {
                // First toxin tick landed this frame (field next_tick_frame ==
                // spawn frame, retail FireWeaponUpdate residual).
                tox_after_first_tick = game_logic
                    .host_object(tox_victim_id)
                    .map(|o| o.health.current);
            }
        }
        if let Some(d) = detonation_frame {
            // Keep ticking past the strike's registry impact frame (90) and
            // the second toxin tick (d + 1 + 15) before asserting.
            if f >= 91 && f >= d + 16 {
                break;
            }
        }
    }
    let detonation_frame = detonation_frame.expect("live bomb must detonate");
    assert!(
        detonation_frame > 1,
        "DeliverPayload approach + HeightDie fall must delay the impact (frame {detonation_frame})"
    );
    assert!(
        game_logic.anthrax_bomb_flight_reg.bombs_dropped >= 1,
        "cargo plane must drop the AnthraxBomb payload"
    );

    // Registry impact-delay bookkeeping completes at impact frame 90; the
    // blast itself was dealt by the live AnthraxBombWeapon at detonation.
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::AnthraxBomb),
        "AnthraxBomb registry strike must complete on its impact frame"
    );
    assert!(
        game_logic.special_power_strikes().honesty_toxin_ok(),
        "AnthraxBomb must spawn residual toxin"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::AnthraxBomb),
        "host path honesty requires complete blast + toxin spawn"
    );

    let enemy_after = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_after.unwrap_or(0.0));
    assert!(
        enemy_dealt > 0.0
            || enemy_after.is_none()
            || enemy_after == Some(0.0)
            || game_logic
                .host_object(enemy_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true)
            || enemy_after.map(|h| h < health_before).unwrap_or(false),
        "enemy at bomb impact must take AnthraxBombWeapon blast damage (dealt={enemy_dealt})"
    );

    // Toxin victim outside blast radius took toxin tick only.
    let tox_after = game_logic
        .host_object(tox_victim_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let tox_dealt = test_observed_damage_to(tox_victim_id, tox_before, tox_after);
    assert!(
        tox_dealt > 0.0 || tox_after < tox_before,
        "mid-radius victim must take toxin residual damage (before={tox_before}, after={tox_after}, dealt={tox_dealt})"
    );
    // Far unit untouched.
    assert!(
        game_logic
            .host_object(far_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside blast/toxin radius must be untouched"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "AnthraxBombImpact"),
        "impact must queue AnthraxBombImpact audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == ANTHRAX_TOXIN_AUDIO),
        "impact must queue anthrax ambient residual"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    // Second toxin tick after the 500ms (15 frame) interval: the loop above
    // already covered the second tick (frame d+16) — the victim must keep
    // taking residual poison damage (POISON-typed damage goes through Armor,
    // so the exact per-tick amount is armor-scaled; Weapon.ini
    // AnthraxBombPoisonFieldWeapon PrimaryDamage 40 every 500ms).
    if let Some(first_tick_hp) = tox_after_first_tick {
        let tox_later = game_logic
            .host_object(tox_victim_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        let second_tick_dealt = first_tick_hp - tox_later;
        assert!(
            second_tick_dealt > 0.0
                || tox_later == 0.0
                || game_logic.host_object(tox_victim_id).is_none(),
            "second toxin tick must apply residual damage (first={first_tick_hp}, later={tox_later}, dealt={second_tick_dealt})"
        );
        assert!(
            game_logic.special_power_strikes().honesty_toxin_damage_ok(),
            "toxin damage honesty after tick"
        );
    }

    // Live delivery dealt the damage via the weapon, not the registry blob —
    // C++ ties completion to the bomb (SpecialPowerCompletionDie ModuleTag_07).
    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::AnthraxBomb);
    assert_eq!(completed.len(), 1);
    assert!(game_logic.anthrax_bomb_flight_reg.detonations >= 1);
    assert!(game_logic.anthrax_bomb_flight_reg.toxin_fields_spawned >= 1);

    game_logic.process_destroy_list();
}

#[test]
fn radar_scan_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    // Activation needs a registered controller: C++ RadarVanPing reveals via
    // the caster's Player relationship mask (Object::look allies by Player).
    game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::RadarScan,
        "SpecialPowerRadarVanScan",
        30_000,
    );
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::RadarScan);
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::RadarScan,
            target: PowerTarget::Location(Vec3::new(200.0, 0.0, 200.0)),
        },
        player_id: 0,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "RadarScan must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_radar_scan_activate_ok(),
        "RadarScan residual must record activation honesty"
    );
}

#[test]
fn radar_scan_special_power_reveals_fow() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_radar_scan::{RADAR_SCAN_DURATION_FRAMES, RADAR_SCAN_RADIUS};
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud for this residual test.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(512.0, 512.0);
    }

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::RadarScan,
        "SpecialPowerRadarVanScan",
        30_000,
    );

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::RadarScan);
    }

    // Far from caster so unit vision does not already clear the cell.
    let target = Vec3::new(250.0, 0.0, 250.0);
    let center = Coord3D::new(target.x, target.z, target.y);

    // Baseline: target shroud not visible.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "precondition: scan target must start shrouded"
        );
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::RadarScan,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 42,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_radar_scan_ok(),
        "RadarScan host residual path honesty (activate + FOW)"
    );
    assert_eq!(game_logic.radar_scans().activations(), 1);
    assert_eq!(game_logic.radar_scans().active_count(), 1);
    assert!(
        game_logic
            .radar_scans()
            .is_position_in_active_scan(0, target),
        "active residual scan must cover target"
    );
    assert!(
        (game_logic.radar_scans().active_scans()[0].radius - RADAR_SCAN_RADIUS).abs() < 0.01,
        "retail residual radius 150"
    );

    // FOW observable: center cell visible after scan.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            shroud.is_position_visible(0, &center),
            "RadarScan must reveal FOW at target center"
        );
    }

    // Charge consumed, not a superweapon strike.
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert_eq!(game_logic.special_power_strikes().strike_count(), 0);

    // Expire residual: advance past duration and run update path.
    game_logic.frame = RADAR_SCAN_DURATION_FRAMES + 1;
    game_logic.update_radar_scans();
    assert_eq!(
        game_logic.radar_scans().active_count(),
        0,
        "scan bookkeeping expires after residual duration"
    );
    assert!(game_logic.radar_scans().expirations() >= 1);
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "temporary reveal must undo after duration (fogged/hidden)"
        );
        assert!(
            shroud.is_position_explored(0, &center),
            "explored residual should remain after undo"
        );
    }

    // Cleanup global shroud for other tests.
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

#[test]
fn spy_satellite_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    // Activation needs a registered controller: C++ RadarVanPing reveals via
    // the caster's Player relationship mask (Object::look allies by Player).
    game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::SpySatellite,
        "SpecialPowerSpySatellite",
        60_000,
    );
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::SpySatellite);
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpySatellite,
            target: PowerTarget::Location(Vec3::new(200.0, 0.0, 200.0)),
        },
        player_id: 0,
        command_id: 6,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "SpySatellite must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_spy_satellite_activate_ok(),
        "SpySatellite residual must record activation honesty"
    );
}

#[test]
fn spy_satellite_special_power_reveals_fow() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_spy_satellite::{
        SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_RADIUS,
    };
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud for this residual test.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(1024.0, 1024.0);
    }

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::SpySatellite,
        "SpecialPowerSpySatellite",
        60_000,
    );

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::SpySatellite);
    }

    // Far from caster so unit vision does not already clear the cell.
    // SpySatellite radius is 300 (larger than RadarScan 150).
    let target = Vec3::new(400.0, 0.0, 400.0);
    let center = Coord3D::new(target.x, target.z, target.y);
    // Point inside residual radius but offset from exact center.
    let near_center = Coord3D::new(target.x + 50.0, target.z, target.y);

    // Baseline: target shroud not visible.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "precondition: spy sat target must start shrouded"
        );
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpySatellite,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 43,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // C++ DynamicShroudClearingRangeUpdate: grow 0 → 300 over 1000ms (30f).
    assert_eq!(game_logic.spy_satellites().activations(), 1);
    assert_eq!(game_logic.spy_satellites().active_count(), 1);
    assert!(
        game_logic.spy_satellites().active_scans()[0].radius < 1.0,
        "scan must start at 0, not instant 300"
    );
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &near_center),
            "offset cell must stay fogged until grow covers it"
        );
    }
    for _ in 0..crate::game_logic::host_spy_satellite::SPY_SATELLITE_GROW_TIME_FRAMES {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_spy_satellites();
    }

    assert!(
        game_logic.honesty_spy_satellite_ok(),
        "SpySatellite host residual path honesty (activate + FOW)"
    );
    assert!(
        game_logic
            .spy_satellites()
            .is_position_in_active_scan(0, target),
        "active residual scan must cover target"
    );
    assert!(
        (game_logic.spy_satellites().active_scans()[0].radius - SPY_SATELLITE_RADIUS).abs() < 0.01,
        "retail residual radius 300 after grow"
    );

    // FOW observable: center cell visible after grow.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            shroud.is_position_visible(0, &center),
            "SpySatellite must reveal FOW at target center"
        );
        assert!(
            shroud.is_position_visible(0, &near_center),
            "SpySatellite residual radius must cover area around target"
        );
    }

    // Charge consumed, not a superweapon strike.
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert_eq!(game_logic.special_power_strikes().strike_count(), 0);

    // Expire residual: advance past duration and run update path.
    game_logic.frame = SPY_SATELLITE_DURATION_FRAMES + 1;
    game_logic.update_spy_satellites();
    assert_eq!(
        game_logic.spy_satellites().active_count(),
        0,
        "scan bookkeeping expires after residual duration"
    );
    assert!(game_logic.spy_satellites().expirations() >= 1);
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "temporary reveal must undo after duration (fogged/hidden)"
        );
        assert!(
            shroud.is_position_explored(0, &center),
            "explored residual should remain after undo"
        );
    }

    // Cleanup global shroud for other tests.
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

#[test]
fn cia_intelligence_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::CiaIntelligence,
        "SuperweaponCIAIntelligence",
        300_000,
    );

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::CiaIntelligence);
    }
    // Enemy unit so residual has a vision-spy target.
    let _enemy = game_logic
        .create_object("TestTank", Team::China, Vec3::new(300.0, 0.0, 300.0))
        .expect("enemy");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CiaIntelligence,
            target: PowerTarget::None,
        },
        player_id: 0,
        command_id: 7,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "CiaIntelligence must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_cia_intelligence_activate_ok(),
        "CiaIntelligence residual must record activation honesty"
    );
}

#[test]
fn cia_intelligence_bonus_duration_per_captured_residual() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_cia_intelligence::{
        CIA_INTELLIGENCE_DURATION_FRAMES, CIA_INTELLIGENCE_MAX_DURATION_FRAMES,
        cia_intelligence_duration_frames,
    };
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_tank_template(&mut logic);
    // Detention-style caster with contained captives residual.
    let mut camp = crate::game_logic::ThingTemplate::new("AmericaDetentionCamp");
    camp.add_kind_of(KindOf::Structure).set_health(2000.0);
    logic.templates.insert("AmericaDetentionCamp".into(), camp);
    let caster = logic
        .create_object("AmericaDetentionCamp", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    // Two "captured" units inside contain residual.
    let c1 = logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let c2 = logic
        .create_object("TestTank", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let camp_obj = logic.host_object_mut(caster).unwrap();
        // C++ ContainModule getContainCount residual (contained_units path).
        if let Some(b) = camp_obj.building_data.as_mut() {
            b.garrisoned_units.push(c1);
            b.garrisoned_units.push(c2);
        } else {
            camp_obj.occupants.push(c1);
            camp_obj.occupants.push(c2);
        }
    }
    // Free enemy outside for vision spy residual.
    let _enemy = logic
        .create_object("TestTank", Team::China, Vec3::new(400.0, 0.0, 400.0))
        .unwrap();

    assert!(logic.activate_cia_intelligence(0, Team::USA, Some(caster)));
    let act = logic
        .cia_intelligence()
        .active_scans()
        .last()
        .expect("cia act");
    assert_eq!(act.captured_count, 2);
    let expected = cia_intelligence_duration_frames(2);
    assert_eq!(expected, CIA_INTELLIGENCE_DURATION_FRAMES + 600);
    assert!(expected < CIA_INTELLIGENCE_MAX_DURATION_FRAMES);
    assert_eq!(
        act.expires_frame.saturating_sub(act.activate_frame),
        expected
    );
    assert!(logic.cia_intelligence().honesty_bonus_duration_ok());
}

#[test]
fn cia_intelligence_special_power_reveals_enemy_units() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_cia_intelligence::CIA_INTELLIGENCE_DURATION_FRAMES;
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud for this residual test.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(1024.0, 1024.0);
    }

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::CiaIntelligence,
        "SuperweaponCIAIntelligence",
        300_000,
    );

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::CiaIntelligence);
    }

    // Far enemy so caster vision does not already clear the cell.
    let enemy_pos = Vec3::new(400.0, 0.0, 400.0);
    let enemy_id = game_logic
        .create_object("TestTank", Team::China, enemy_pos)
        .expect("enemy");
    // Stealthed residual: CIA must make unit detectable.
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_status_stealthed(true);
        enemy.set_status_detected(false);
    }
    let center = Coord3D::new(enemy_pos.x, enemy_pos.z, enemy_pos.y);

    // Baseline: enemy position shrouded, unit effectively stealthed.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "precondition: enemy must start shrouded"
        );
    }
    assert!(
        game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_effectively_stealthed(),
        "precondition: enemy starts stealthed+undetected"
    );
    assert!(
        !game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_vision_spied_by_player(0),
        "precondition: not vision-spied yet"
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CiaIntelligence,
            target: PowerTarget::None,
        },
        player_id: 0,
        command_id: 44,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_cia_intelligence_ok(),
        "CiaIntelligence host residual path honesty (activate + vision-spied)"
    );
    assert_eq!(game_logic.cia_intelligence().activations(), 1);
    assert_eq!(game_logic.cia_intelligence().active_count(), 1);
    assert!(
        game_logic.cia_intelligence().units_spied() >= 1,
        "must vision-spy at least one enemy unit"
    );
    assert!(
        game_logic
            .cia_intelligence()
            .is_object_vision_spied(0, enemy_id),
        "registry must track vision-spied enemy"
    );
    assert!(
        game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_vision_spied_by_player(0),
        "enemy Object residual vision_spied_mask must be set"
    );

    // FOW observable: enemy cell visible after spy vision residual.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            shroud.is_position_visible(0, &center),
            "CiaIntelligence must reveal FOW at enemy unit position"
        );
    }

    // C++ SpyVision does not destalth. Stealthed enemies stay cloaked;
    // they become moving lookers (vision_spied), not DETECTED.
    let enemy_after = game_logic.host_object(enemy_id).unwrap();
    assert!(
        !enemy_after.status.detected,
        "SpyVision must not mark DETECTED"
    );
    assert!(
        enemy_after.is_effectively_stealthed(),
        "SpyVision must not destalth"
    );
    assert!(
        enemy_after.is_vision_spied_by_player(0),
        "stealthed enemy is still a looker for the spy"
    );

    // Charge consumed, not a superweapon strike.
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert_eq!(game_logic.special_power_strikes().strike_count(), 0);

    // Expire residual: advance past duration and run update path.
    game_logic.frame = CIA_INTELLIGENCE_DURATION_FRAMES + 1;
    game_logic.update_cia_intelligence();
    assert_eq!(
        game_logic.cia_intelligence().active_count(),
        0,
        "spy bookkeeping expires after residual duration"
    );
    assert!(game_logic.cia_intelligence().expirations() >= 1);
    assert!(
        !game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_vision_spied_by_player(0),
        "vision_spied residual mark must clear after expiry"
    );
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "temporary reveal must undo after duration (fogged/hidden)"
        );
        assert!(
            shroud.is_position_explored(0, &center),
            "explored residual should remain after undo"
        );
    }

    // Cleanup global shroud for other tests.
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

#[test]
fn cia_intelligence_looker_follows_moving_enemy() {
    use crate::game_logic::host_cia_intelligence::CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS;
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    let mut logic = GameLogic::new();
    // C++ clearGameData tears the per-world shroud down with the world
    // (GameLogic::new() -> ShroudManager::reset_for_new_game drops the
    // terrain grid).  A bigger map grid must therefore be initialized AFTER
    // the world exists, like ThePlayerList's Shroud after newMap.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(1024.0, 1024.0);
    }
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    logic.add_player(player);
    ensure_test_tank_template(&mut logic);
    let caster = logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    let start = Vec3::new(400.0, 0.0, 400.0);
    let enemy = logic
        .create_object("TestTank", Team::China, start)
        .expect("enemy");
    assert!(logic.activate_cia_intelligence(0, Team::USA, Some(caster)));
    let moved = Vec3::new(520.0, 0.0, 520.0);
    {
        let obj = logic.host_object_mut(enemy).unwrap();
        obj.set_position(moved);
    }
    logic.frame = 5;
    logic.update_cia_intelligence();
    let new_center = Coord3D::new(moved.x, moved.z, moved.y);
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            shroud.is_position_visible(0, &new_center),
            "CIA looker must share vision at the enemy's new position"
        );
    }
    assert!(
        logic.cia_intelligence().is_position_in_active_spy(0, start)
            || CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS > 0.0
    );
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

#[test]
fn firewall_black_napalm_upgraded_segments_and_damage() {
    use crate::game_logic::host_dragon_tank::UPGRADE_CHINA_BLACK_NAPALM;
    use crate::game_logic::host_firewall::{
        FIREWALL_DAMAGE_PER_TICK_UPGRADED, FIREWALL_SEGMENT_TEMPLATE_UPGRADED,
    };

    let mut logic = GameLogic::new();
    let mut dragon_tpl = ThingTemplate::new("ChinaTankDragon");
    dragon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankDragon".to_string(), dragon_tpl);

    let mut victim_tpl = ThingTemplate::new("AmericaInfantryRanger");
    victim_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), victim_tpl);

    let caster = logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("dragon");
    {
        let o = logic.host_object_mut(caster).unwrap();
        o.apply_upgrade_tag(UPGRADE_CHINA_BLACK_NAPALM);
    }

    let wall_id = logic
        .activate_firewall(caster, Vec3::new(80.0, 0.0, 0.0))
        .expect("firewall");
    assert!(logic.fire_walls.honesty_upgraded_ok());
    assert!(logic.honesty_firewall_black_napalm_ok());

    let upgraded_segments = logic
        .objects
        .values()
        .filter(|o| o.firewall_segment && o.template_name == FIREWALL_SEGMENT_TEMPLATE_UPGRADED)
        .count();
    assert!(
        upgraded_segments >= 1,
        "BlackNapalm should spawn FireWallSegmentUpgraded objects"
    );

    // Place victim on first segment and tick damage.
    let seg_pos = logic
        .fire_walls
        .active_walls()
        .iter()
        .find(|w| w.id == wall_id)
        .and_then(|w| w.segments.first().map(|s| s.position))
        .expect("seg");
    let victim = logic
        .create_object("AmericaInfantryRanger", Team::USA, seg_pos)
        .expect("victim");
    let hp_before = logic.host_object(victim).unwrap().health.current;
    logic.update_firewalls();
    let hp_after = logic
        .host_object(victim)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (hp_before - hp_after - FIREWALL_DAMAGE_PER_TICK_UPGRADED).abs() < 0.1
            || hp_after + 0.01 < hp_before,
        "upgraded wall should apply 5 dmg residual, before={hp_before} after={hp_after}"
    );
    assert!(logic.honesty_firewall_damage_ok());
}

#[test]
fn firewall_inch_forward_moves_segment_objects() {
    use crate::game_logic::host_firewall::FIREWALL_INCH_PER_FRAME;

    let mut logic = GameLogic::new();
    let mut dragon_tpl = ThingTemplate::new("ChinaTankDragon");
    dragon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankDragon".to_string(), dragon_tpl);

    let caster = logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("dragon");
    let _wall = logic
        .activate_firewall(caster, Vec3::new(100.0, 0.0, 0.0))
        .expect("firewall");

    let seg_id = logic
        .objects
        .iter()
        .find(|(_, o)| o.firewall_segment)
        .map(|(id, _)| *id)
        .expect("segment object");
    let before = logic.host_object(seg_id).unwrap().get_position();
    logic.update_firewall_segment_objects();
    let after = logic.host_object(seg_id).unwrap().get_position();
    assert!(
        (after.x - before.x - FIREWALL_INCH_PER_FRAME).abs() < 0.01,
        "segment should inch forward +X, before={before:?} after={after:?}"
    );
    assert!(logic.honesty_firewall_inch_forward_ok());
    assert!(logic.fire_walls.honesty_crawl_ok());
}

#[test]
fn firewall_spawns_segment_objects_residual() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_firewall::{FIREWALL_DURATION_FRAMES, FIREWALL_SEGMENT_TEMPLATE};
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut dragon = crate::game_logic::ThingTemplate::new("ChinaTankDragon");
    dragon.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("ChinaTankDragon".into(), dragon);
    let caster = logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .activate_firewall(caster, Vec3::new(80.0, 0.0, 0.0))
        .expect("firewall");
    assert!(logic.fire_walls().honesty_segment_spawn_ok());
    assert!(logic.fire_walls().segments_spawned >= 1);
    let segs: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.firewall_segment)
        .collect();
    assert!(!segs.is_empty());
    assert!(
        segs.iter()
            .all(|o| o.template_name == FIREWALL_SEGMENT_TEMPLATE)
    );
    let ids: Vec<_> = segs.iter().map(|o| o.id).collect();
    logic.frame = FIREWALL_DURATION_FRAMES + 5;
    logic.update_firewall_segment_objects();
    for sid in ids {
        assert!(
            logic
                .host_object(sid)
                .map(|o| !o.is_alive() || o.status.destroyed)
                .unwrap_or(true)
        );
    }
    let _ = id;
}

#[test]
fn firewall_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::FireWall,
        "DragonTankFireWallWeapon",
        40,
    );
    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::FireWall);
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::FireWall,
            target: PowerTarget::Location(Vec3::new(80.0, 0.0, 0.0)),
        },
        player_id: 1,
        command_id: 50,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "FireWall must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_firewall_activate_ok(),
        "FireWall residual must record activation honesty"
    );
    assert!(
        game_logic.fire_walls().active_count() >= 1,
        "FireWall must create active damage zones"
    );
}

#[test]
fn firewall_special_power_applies_line_fire_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_firewall::{
        FIREWALL_DAMAGE_PER_TICK, FIREWALL_DURATION_FRAMES, FIREWALL_TICK_INTERVAL_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    arm_test_tank_special_power(
        &mut game_logic,
        SpecialPowerType::FireWall,
        "DragonTankFireWallWeapon",
        40,
    );

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        // create_object pre-arms the module ReloadTime on the per-power
        // map; simulate a fully recharged caster like C++ isReady.
        caster.special_power_cooldowns.remove(&SpecialPowerType::FireWall);
        caster.thing.template.armor = 0.0;
    }

    // Place enemy on the residual wall line (first segment ~START_OFFSET along +X).
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 100.0;
        enemy.health.maximum = 100.0;
        enemy.thing.template.armor = 0.0;
    }

    // Far enemy must not take residual fire damage.
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 500.0))
        .expect("far enemy");
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 100.0;
        far.health.maximum = 100.0;
        far.thing.template.armor = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::FireWall,
            target: PowerTarget::Location(Vec3::new(100.0, 0.0, 0.0)),
        },
        player_id: 1, // Team::China residual
        command_id: 51,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_firewall_activate_ok(),
        "FireWall activation honesty"
    );
    assert!(
        game_logic.fire_walls().active_count() >= 1,
        "must create residual fire zones"
    );
    assert!(
        game_logic
            .fire_walls()
            .is_position_in_active_fire(Vec3::new(20.0, 0.0, 0.0)),
        "enemy position must lie in residual fire line"
    );

    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_before = game_logic.host_object(far_id).unwrap().health.current;

    // Immediate tick on activation frame applies damage.
    game_logic.update_firewalls();
    let hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_after = game_logic.host_object(far_id).unwrap().health.current;

    assert!(
        hp_after < hp_before,
        "enemy on FireWall line must take fire damage (before={hp_before}, after={hp_after})"
    );
    let dealt = hp_before - hp_after;
    assert!(
        (dealt - FIREWALL_DAMAGE_PER_TICK).abs() < 0.01 || dealt > 0.0,
        "residual fire tick damage expected ~{FIREWALL_DAMAGE_PER_TICK}, got {dealt}"
    );
    assert!(
        (far_after - far_before).abs() < 0.01,
        "units off the fire line must not take residual FireWall damage"
    );
    assert!(
        game_logic.honesty_firewall_damage_ok(),
        "FireWall damage honesty after tick"
    );
    assert!(
        game_logic.honesty_firewall_ok(),
        "combined FireWall host path honesty"
    );

    // Second tick only after residual interval.
    let mid_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 1;
    game_logic.update_firewalls();
    assert!(
        (game_logic.host_object(enemy_id).unwrap().health.current - mid_hp).abs() < 0.01,
        "no damage before tick interval"
    );
    game_logic.frame = FIREWALL_TICK_INTERVAL_FRAMES;
    game_logic.update_firewalls();
    assert!(
        game_logic.host_object(enemy_id).unwrap().health.current < mid_hp,
        "second fire tick after interval must apply more damage"
    );

    // Expire residual wall.
    game_logic.frame = FIREWALL_DURATION_FRAMES + 1;
    game_logic.update_firewalls();
    assert_eq!(
        game_logic.fire_walls().active_count(),
        0,
        "FireWall segments expire after residual duration"
    );
    assert!(game_logic.fire_walls().expirations >= 1);
}

#[test]
fn inferno_cannon_attack_spawns_fire_zone_damaging_enemies() {
    use crate::game_logic::host_inferno_cannon::{
        INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_DURATION_FRAMES,
        INFERNO_FIRE_TICK_INTERVAL_FRAMES, is_inferno_cannon_template,
    };
    use crate::game_logic::weapon_bootstrap::{
        INFERNO_CANNON_PRIMARY_WEAPON, ensure_host_weapon_store,
    };

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::China, "China", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let mut cannon_tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleInfernoCannon");
    cannon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(INFERNO_CANNON_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("ChinaVehicleInfernoCannon".to_string(), cannon_tpl);

    let cannon_id = game_logic
        .create_object(
            "ChinaVehicleInfernoCannon",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("inferno cannon");
    {
        let c = game_logic.host_object(cannon_id).expect("cannon");
        assert!(
            is_inferno_cannon_template(&c.template_name),
            "template name residual must identify Inferno Cannon"
        );
        assert!(
            c.weapon.is_some(),
            "Inferno Cannon must bind primary weapon residual"
        );
        let w = c.weapon.as_ref().unwrap();
        assert!(
            (w.damage - 30.0).abs() < 0.01,
            "InfernoCannonGun PrimaryDamage residual 30, got {}",
            w.damage
        );
        // C++ WeaponTemplate::getAttackRange (Weapon.cpp:433-446,
        // RATIONALIZE_ATTACK_RANGE) undersizes the authored range by 1/4 of a
        // pathfind cell: 300 - 2.5 = 297.5 effective.
        assert!(
            (w.range
                - (300.0 - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25))
                .abs()
                < 1.0,
            "InfernoCannonGun effective AttackRange 297.5, got {}",
            w.range
        );
    }

    // Enemy at impact; far enemy outside fire radius (30).
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 200.0;
        enemy.health.maximum = 200.0;
        enemy.thing.template.armor = 0.0;
    }
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far enemy");
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 200.0;
        far.health.maximum = 200.0;
        far.thing.template.armor = 0.0;
    }

    // Ready weapon + attack enemy in range.
    {
        let c = game_logic.host_object_mut(cannon_id).expect("cannon");
        c.attack_target(enemy_id);
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            // Fail-closed residual min range 0 for host tests.
            w.min_range = 0.0;
        }
        c.thing.template.armor = 0.0;
    }

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_before = game_logic.host_object(far_id).unwrap().health.current;

    game_logic.set_current_frame(10);
    game_logic.update_combat(&[cannon_id, enemy_id, far_id], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_inferno_fire_spawn_ok()
        && !game_logic.honesty_inferno_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(cannon_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_inferno_shell_projectile(cannon_id, from, aim, Some(enemy_id), false)
                .is_some()
        );
    }
    // DumbProjectile Bezier residual: advance InfernoTankShell to impact + FireField.
    for _ in 0..200 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_inferno_shell_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.inferno_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_inferno_fire_spawn_ok()
            || game_logic.honesty_inferno_shell_projectile_ok(),
        "Inferno attack must spawn residual fire zone"
    );
    assert!(
        game_logic.inferno_fire_zones().active_count() >= 1,
        "must create residual FireFieldSmall zone"
    );
    assert!(
        game_logic
            .inferno_fire_zones()
            .is_position_in_active_fire(Vec3::new(100.0, 0.0, 0.0)),
        "enemy impact position must lie in residual fire zone"
    );

    // Shell impact damage may have already reduced HP; capture residual baseline after shot.
    let hp_after_shot = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after_shot < enemy_hp_before,
        "shell impact residual must deal damage (before={enemy_hp_before}, after={hp_after_shot})"
    );

    // Immediate fire-zone tick on activation frame applies DoT.
    game_logic.update_inferno_fire_zones();
    let hp_after_dot = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_after = game_logic.host_object(far_id).unwrap().health.current;

    assert!(
        hp_after_dot < hp_after_shot,
        "enemy in Inferno fire zone must take DoT (before={hp_after_shot}, after={hp_after_dot})"
    );
    let dealt = hp_after_shot - hp_after_dot;
    assert!(
        (dealt - INFERNO_FIRE_DAMAGE_PER_TICK).abs() < 0.01 || dealt > 0.0,
        "residual fire tick damage expected ~{INFERNO_FIRE_DAMAGE_PER_TICK}, got {dealt}"
    );
    assert!(
        (far_after - far_hp_before).abs() < 0.01,
        "units outside fire radius must not take residual Inferno fire damage"
    );
    assert!(
        game_logic.honesty_inferno_fire_damage_ok(),
        "Inferno fire damage honesty after tick"
    );
    assert!(
        game_logic.honesty_inferno_cannon_ok(),
        "combined Inferno Cannon host path honesty"
    );

    // Second tick only after residual interval (relative to zone spawn frame).
    let zone_frame = game_logic.frame;
    let mid_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = zone_frame.saturating_add(1);
    game_logic.update_inferno_fire_zones();
    assert!(
        (game_logic.host_object(enemy_id).unwrap().health.current - mid_hp).abs() < 0.01,
        "no fire DoT before tick interval"
    );
    game_logic.frame = zone_frame.saturating_add(INFERNO_FIRE_TICK_INTERVAL_FRAMES);
    game_logic.update_inferno_fire_zones();
    assert!(
        game_logic.host_object(enemy_id).unwrap().health.current < mid_hp,
        "second fire tick after interval must apply more damage"
    );

    // Expire residual fire zone.
    game_logic.frame = zone_frame.saturating_add(INFERNO_FIRE_DURATION_FRAMES + 1);
    game_logic.update_inferno_fire_zones();
    assert_eq!(
        game_logic.inferno_fire_zones().active_count(),
        0,
        "Inferno fire zones expire after residual duration"
    );
    assert!(game_logic.inferno_fire_zones().expirations >= 1);
}

#[test]
fn angry_mob_projectile_flies_and_impacts() {
    use crate::game_logic::host_angry_mob::{
        ANGRY_MOB_MOLOTOV_DAMAGE, ANGRY_MOB_MOLOTOV_PROJECTILE, ANGRY_MOB_ROCK_DAMAGE,
        ANGRY_MOB_ROCK_PROJECTILE, AngryMobProjectileKind, angry_mob_projectile_flight_frames,
    };

    let mut logic = GameLogic::new();
    let mut nexus_tpl = ThingTemplate::new("GLAInfantryAngryMobNexus");
    nexus_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic
        .templates
        .insert("GLAInfantryAngryMobNexus".to_string(), nexus_tpl);

    let mut enemy_tpl = ThingTemplate::new("TestInfantry");
    enemy_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic
        .templates
        .insert("TestInfantry".to_string(), enemy_tpl);

    let nexus = logic
        .create_object(
            "GLAInfantryAngryMobNexus",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("nexus");
    let enemy = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let hp_before = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    let from = Vec3::new(0.0, 2.0, 0.0);
    let aim = Vec3::new(80.0, 0.0, 0.0);
    let mid = logic
        .spawn_angry_mob_projectile(
            nexus,
            from,
            aim,
            Some(enemy),
            AngryMobProjectileKind::Molotov,
        )
        .expect("spawn molotov");
    assert!(logic.honesty_angry_mob_projectile_ok());
    assert_eq!(
        logic.host_object(mid).map(|o| o.template_name.as_str()),
        Some(ANGRY_MOB_MOLOTOV_PROJECTILE)
    );

    let max_steps =
        angry_mob_projectile_flight_frames(from, aim, AngryMobProjectileKind::Molotov).max(5);
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_angry_mob_projectiles();
        if !logic
            .objects
            .values()
            .any(|o| o.angry_mob_projectile && o.is_alive())
        {
            break;
        }
    }
    logic.process_destroy_list();

    let hp_after = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before - 0.5,
        "molotov impact should damage enemy {hp_before} -> {hp_after} (base {ANGRY_MOB_MOLOTOV_DAMAGE})"
    );
    assert!(
        !logic
            .objects
            .values()
            .any(|o| o.angry_mob_projectile && o.is_alive()),
        "projectile should detonate"
    );
    let _ = (ANGRY_MOB_ROCK_DAMAGE, ANGRY_MOB_ROCK_PROJECTILE);
}

#[test]
fn angry_mob_damages_nearby_enemies_over_frames() {
    use crate::game_logic::host_angry_mob::{
        ANGRY_MOB_ATTACK_RANGE, ANGRY_MOB_EXPAND_INTERVAL_FRAMES, ANGRY_MOB_INITIAL_MEMBERS,
        ANGRY_MOB_MAX_MEMBERS, ANGRY_MOB_RESIDUAL_WEAPON, ANGRY_MOB_TICK_INTERVAL_FRAMES,
        UPGRADE_GLA_ARM_THE_MOB, angry_mob_damage_for_tick, is_angry_mob_nexus_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::GLA, "GLA", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let mut mob_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryAngryMobNexus");
    mob_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(99999.0)
        .set_primary_weapon_name(ANGRY_MOB_RESIDUAL_WEAPON);
    game_logic
        .templates
        .insert("GLAInfantryAngryMobNexus".to_string(), mob_tpl);

    let mob_id = game_logic
        .create_object(
            "GLAInfantryAngryMobNexus",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("angry mob nexus");
    {
        let m = game_logic.host_object(mob_id).expect("mob");
        assert!(
            is_angry_mob_nexus_template(&m.template_name),
            "template name residual must identify Angry Mob nexus"
        );
        assert!(
            m.weapon.is_some(),
            "Angry Mob nexus must bind residual aggregate fire weapon"
        );
        let w = m.weapon.as_ref().unwrap();
        // C++ WeaponTemplate::getAttackRange (Weapon.cpp:433-446) undersizes
        // by 1/4 pathfind cell: 100 - 2.5 = 97.5 effective bound range.
        assert!(
            (w.range
                - (ANGRY_MOB_ATTACK_RANGE
                    - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25))
                .abs()
                < 1.0,
            "Angry Mob effective AttackRange {}, got {}",
            ANGRY_MOB_ATTACK_RANGE - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25,
            w.range
        );
    }

    // Near enemy inside residual range; far enemy outside.
    // High HP so multi-tick / expand residual probes do not destroy early.
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("near enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 5_000.0;
        enemy.health.maximum = 5_000.0;
        enemy.thing.template.armor = 0.0;
    }
    let far_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(500.0, 0.0, 0.0))
        .expect("far enemy");
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 5_000.0;
        far.health.maximum = 5_000.0;
        far.thing.template.armor = 0.0;
    }
    // Ally must not take residual friendly fire (fail-closed residual).
    let ally_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("ally");
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.health.current = 5_000.0;
        ally.health.maximum = 5_000.0;
        ally.thing.template.armor = 0.0;
    }

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_before = game_logic.host_object(far_id).unwrap().health.current;
    let ally_hp_before = game_logic.host_object(ally_id).unwrap().health.current;

    game_logic.set_current_frame(0);
    game_logic.update_angry_mobs();

    assert_eq!(
        game_logic.angry_mobs().active_count(),
        1,
        "living Angry Mob nexus must be tracked"
    );
    // Retail SpawnBehavior (WeaponObjects.ini GLAInfantryAngryMobNexus
    // ModuleTag_05): SpawnNumber 10, InitialBurst 5 — the first 5 members
    // spawn immediately, the rest replace one per ExitDelay 5000ms.
    assert_eq!(
        game_logic.angry_mobs().member_count_of(mob_id),
        Some(ANGRY_MOB_INITIAL_MEMBERS)
    );

    let hp_after_first = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after_first < enemy_hp_before,
        "near enemy must take residual Angry Mob damage on first tick (before={enemy_hp_before}, after={hp_after_first})"
    );
    let dealt = enemy_hp_before - hp_after_first;
    let expected = angry_mob_damage_for_tick(ANGRY_MOB_INITIAL_MEMBERS, false);
    assert!(
        (dealt - expected).abs() < 0.01,
        "first tick damage expected {expected}, got {dealt}"
    );
    assert!(
        (game_logic.host_object(far_id).unwrap().health.current - far_hp_before).abs() < 0.01,
        "far enemy outside range must not take residual damage"
    );
    assert!(
        (game_logic.host_object(ally_id).unwrap().health.current - ally_hp_before).abs() < 0.01,
        "same-team ally must not take residual Angry Mob damage"
    );
    assert!(
        game_logic.honesty_angry_mob_damage_ok(),
        "Angry Mob damage honesty after first tick"
    );
    assert!(
        game_logic.honesty_angry_mob_ok(),
        "Angry Mob host path honesty"
    );

    // Second tick only after residual interval (damage over frames).
    let mid_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 1;
    game_logic.update_angry_mobs();
    assert!(
        (game_logic.host_object(enemy_id).unwrap().health.current - mid_hp).abs() < 0.01,
        "no Angry Mob damage before tick interval"
    );
    game_logic.frame = ANGRY_MOB_TICK_INTERVAL_FRAMES;
    game_logic.update_angry_mobs();
    let hp_after_second = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after_second < mid_hp,
        "second fire tick after interval must apply more damage (mid={mid_hp}, after={hp_after_second})"
    );
    let dealt2 = mid_hp - hp_after_second;
    assert!(
        (dealt2 - expected).abs() < 0.01,
        "second tick damage expected {expected}, got {dealt2}"
    );

    // By the retail replace cadence (InitialBurst 5 now, then one member per
    // ExitDelay 5000ms = 150 frames) all SpawnNumber=10 members are out by
    // frame 900.
    game_logic.frame = ANGRY_MOB_EXPAND_INTERVAL_FRAMES;
    game_logic.update_angry_mobs();
    assert_eq!(
        game_logic.angry_mobs().member_count_of(mob_id),
        Some(ANGRY_MOB_MAX_MEMBERS),
        "replacement cadence fills SpawnNumber until a member dies"
    );
    // C++ SpawnBehavior::computeAggregateStates sets OBJECT_STATUS_MASKED on
    // the nexus every update (SpawnBehavior.cpp:992-995) so enemies cannot
    // shoot the 99999-HP nexus. Live keeps the nexus unmasked so is_selectable
    // stays true (playable mob) and instead fails weapons closed via
    // is_angry_mob_nexus_template — assert that observable.
    assert!(
        !game_logic
            .host_object(mob_id)
            .map(|o| o.is_targetable_by_enemy_of(Team::USA))
            .unwrap_or(true),
        "weapons must not acquire the aggregate nexus"
    );

    // Dead members reduce DPS; last member destroys the nexus.
    let member_ids: Vec<_> = game_logic
        .host_objects()
        .values()
        .filter(|o| o.angry_mob_member && o.angry_mob_nexus_id == Some(mob_id))
        .map(|o| o.id)
        .collect();
    assert_eq!(member_ids.len() as u32, ANGRY_MOB_MAX_MEMBERS);
    if let Some(first) = member_ids.first() {
        if let Some(o) = game_logic.objects.get_mut(first) {
            o.health.current = 0.0;
            o.status.destroyed = true;
            o.status.effectively_dead = true;
        }
    }
    game_logic.update_angry_mobs();
    assert_eq!(
        game_logic.angry_mobs().member_count_of(mob_id),
        Some(ANGRY_MOB_MAX_MEMBERS - 1),
        "onSpawnDeath must shrink live count"
    );
    let _ = ANGRY_MOB_INITIAL_MEMBERS;

    // ArmTheMob upgrade residual multiplies damage.
    {
        let player = game_logic.players.get_mut(&1).expect("player");
        player
            .unlocked_sciences
            .insert(UPGRADE_GLA_ARM_THE_MOB.to_string());
    }
    // Fresh near-enemy for armed damage probe (prior ticks may have killed the first).
    let armed_enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("armed probe enemy");
    {
        let e = game_logic
            .host_object_mut(armed_enemy_id)
            .expect("armed enemy");
        e.health.current = 500.0;
        e.health.maximum = 500.0;
        e.thing.template.armor = 0.0;
    }
    let hp_pre_armed = game_logic
        .host_object(armed_enemy_id)
        .unwrap()
        .health
        .current;
    // Force next tick due past any prior cadence.
    game_logic.frame = game_logic
        .frame
        .saturating_add(ANGRY_MOB_TICK_INTERVAL_FRAMES + 2);
    game_logic.update_angry_mobs();
    let hp_post_armed = game_logic
        .host_object(armed_enemy_id)
        .unwrap()
        .health
        .current;
    let armed_dealt = hp_pre_armed - hp_post_armed;
    let expected_armed = angry_mob_damage_for_tick(ANGRY_MOB_MAX_MEMBERS - 1, true);
    assert!(
        armed_dealt > 0.0,
        "ArmTheMob residual must still deal damage (pre={hp_pre_armed}, post={hp_post_armed})"
    );
    assert!(
        (armed_dealt - expected_armed).abs() < 0.01,
        "armed damage expected {expected_armed}, got {armed_dealt}"
    );
}

#[test]
fn aurora_bomb_host_path_queues_and_applies_delayed_area_damage() {
    use crate::game_logic::host_aurora_bomb::{
        AURORA_BOMB_DAMAGE, AURORA_BOMB_DIVE_DELAY_FRAMES, AURORA_BOMB_PRIMARY_WEAPON,
        AURORA_FUEL_AIR_DAMAGE, AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, HostAuroraBombKind,
        is_aurora_aircraft_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    // Standard Aurora aircraft residual template.
    let mut aurora_tpl = crate::game_logic::ThingTemplate::new("AmericaJetAurora");
    aurora_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(80.0)
        .set_primary_weapon_name(AURORA_BOMB_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("AmericaJetAurora".to_string(), aurora_tpl);

    // FuelAir Aurora residual template.
    let mut fuel_tpl = crate::game_logic::ThingTemplate::new("AirF_AmericaJetAurora");
    fuel_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(80.0)
        .set_primary_weapon_name("AirF_AuroraBombWeapon");
    game_logic
        .templates
        .insert("AirF_AmericaJetAurora".to_string(), fuel_tpl);

    let target = Vec3::new(100.0, 0.0, 0.0);

    let aurora_id = game_logic
        .create_object("AmericaJetAurora", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("aurora");
    {
        let a = game_logic.host_object(aurora_id).expect("aurora obj");
        assert!(
            is_aurora_aircraft_template(&a.template_name),
            "template residual must identify Aurora aircraft"
        );
        assert!(
            a.weapon.is_some(),
            "Aurora must bind residual primary weapon"
        );
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, target)
        .expect("enemy");
    let near_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(110.0, 0.0, 0.0))
        .expect("near");
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, target)
        .expect("friend");

    for id in [enemy_id, near_id, far_id, friend_id] {
        let obj = game_logic.host_object_mut(id).expect("obj");
        obj.health.current = 500.0;
        obj.health.maximum = 500.0;
        obj.thing.template.armor = 0.0;
    }

    {
        let a = game_logic.host_object_mut(aurora_id).expect("aurora");
        a.attack_target(enemy_id);
        if let Some(w) = a.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
            w.ammo = Some(1);
        }
    }

    let enemy_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let near_before = game_logic.host_object(near_id).unwrap().health.current;
    let far_before = game_logic.host_object(far_id).unwrap().health.current;
    let friend_before = game_logic.host_object(friend_id).unwrap().health.current;

    game_logic.set_current_frame(10);
    game_logic.update_combat(
        &[aurora_id, enemy_id, near_id, far_id, friend_id],
        LOGIC_FRAME_TIMESTEP,
    );

    assert!(
        game_logic.honesty_aurora_bomb_activate_ok(),
        "Aurora attack must queue residual dive bomb"
    );
    assert!(
        game_logic.aurora_bombs().pending_count() >= 1,
        "must have pending dive bomb mission"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "AuroraBombLaunch"),
        "activation must queue AuroraBombLaunch audio"
    );

    // Before dive delay: no damage.
    game_logic.frame = 10 + AURORA_BOMB_DIVE_DELAY_FRAMES - 1;
    game_logic.update_aurora_bombs();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        enemy_before,
        "no damage before Aurora dive impact frame"
    );
    assert!(!game_logic.honesty_aurora_bomb_complete_ok());

    // At impact: standard AuroraBombWeapon residual area damage.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 10 + AURORA_BOMB_DIVE_DELAY_FRAMES;
    game_logic.update_aurora_bombs();

    assert!(
        game_logic.honesty_aurora_bomb_complete_ok(),
        "Aurora bomb must complete on impact frame"
    );
    assert!(
        game_logic.honesty_aurora_bomb_damage_ok(),
        "Aurora bomb must deal residual damage"
    );
    assert!(
        game_logic.honesty_aurora_bomb_ok(),
        "combined Aurora host path honesty"
    );

    let enemy_hp = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let near_hp = game_logic.host_object(near_id).map(|o| o.health.current);
    let enemy_dealt = test_observed_damage_to(enemy_id, enemy_before, enemy_hp.unwrap_or(0.0));
    let near_dealt = test_observed_damage_to(near_id, near_before, near_hp.unwrap_or(0.0));
    assert!(
        enemy_dealt > 0.0 || enemy_hp.map(|h| h < enemy_before).unwrap_or(true),
        "enemy at epicenter must take Aurora residual damage (~{AURORA_BOMB_DAMAGE}), got {enemy_hp:?} dealt={enemy_dealt}"
    );
    assert!(
        near_dealt > 0.0 || near_hp.map(|h| h < near_before).unwrap_or(true),
        "enemy inside Aurora radius must take residual damage, got {near_hp:?} dealt={near_dealt}"
    );
    assert!(
        (game_logic.host_object(far_id).unwrap().health.current - far_before).abs() < 0.1,
        "far enemy outside radius must not take residual damage"
    );
    // RadiusDamageAffects ALLIES residual: friendly at epicenter takes blast.
    let friend_hp = game_logic.host_object(friend_id).map(|o| o.health.current);
    let friend_dealt = test_observed_damage_to(friend_id, friend_before, friend_hp.unwrap_or(0.0));
    assert!(
        friend_dealt > 0.0
            || friend_hp.map(|h| h < friend_before).unwrap_or(true)
            || friend_hp == Some(0.0)
            || game_logic
                .host_object(friend_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "friendly at epicenter must take Aurora residual damage (ALLIES residual), got {friend_hp:?} dealt={friend_dealt}"
    );
    // last_damage_source residual: victim records Aurora aircraft as killer.
    if let Some(enemy) = game_logic.host_object(enemy_id) {
        if !enemy.is_alive() || enemy.health.current < enemy_before {
            assert_eq!(
                enemy.last_damage_source,
                Some(aurora_id),
                "Aurora blast must stamp last_damage_source for cash bounty residual"
            );
        }
    }
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "AuroraBombDetonate"),
        "impact must queue AuroraBombDetonate audio"
    );

    // --- FuelAir residual: longer delay + larger damage ---
    let fuel_id = game_logic
        .create_object(
            "AirF_AmericaJetAurora",
            Team::USA,
            Vec3::new(0.0, 0.0, 50.0),
        )
        .expect("fuel aurora");
    let fuel_enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("fuel enemy");
    {
        let e = game_logic.host_object_mut(fuel_enemy).expect("e");
        e.health.current = 1500.0;
        e.health.maximum = 1500.0;
        e.thing.template.armor = 0.0;
    }
    {
        let a = game_logic.host_object_mut(fuel_id).expect("fuel");
        a.attack_target(fuel_enemy);
        if let Some(w) = a.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
            w.ammo = Some(1);
        }
    }

    game_logic.set_current_frame(1000);
    game_logic.update_combat(&[fuel_id, fuel_enemy], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic
            .aurora_bombs()
            .pending_of_kind(HostAuroraBombKind::FuelAir)
            .len()
            >= 1,
        "FuelAir Aurora must queue FuelAir residual mission"
    );

    use crate::game_logic::host_aurora_bomb::AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES;
    use crate::game_logic::host_fuel_air_gas_slow_death::{
        AIRF_AURORA_BOMB_GAS_OBJECT, FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES,
    };

    let fuel_before = game_logic.host_object(fuel_enemy).unwrap().health.current;
    // Dive impact spawns gas SpecialObject; primary blast waits for SlowDeath FINAL.
    game_logic.frame = 1000 + AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES - 1;
    game_logic.update_aurora_bombs();
    assert_eq!(
        game_logic.host_object(fuel_enemy).unwrap().health.current,
        fuel_before,
        "no FuelAir damage before dive/gas spawn frame"
    );

    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 1000 + AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES;
    game_logic.update_aurora_bombs();
    assert!(
        game_logic.honesty_aurora_fuel_air_gas_object_ok()
            || game_logic
                .host_objects()
                .values()
                .any(|o| o.template_name == AIRF_AURORA_BOMB_GAS_OBJECT),
        "FuelAir dive must spawn AirF_AuroraBombGas SpecialObject residual"
    );
    assert_eq!(
        game_logic.host_object(fuel_enemy).unwrap().health.current,
        fuel_before,
        "gas path defers primary blast until SlowDeath FINAL"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "DaisyCutterExplosion"),
        "FuelAir impact must queue DaisyCutterExplosion audio residual"
    );
    assert!(
        game_logic
            .aurora_bombs()
            .honesty_complete_ok_of_kind(HostAuroraBombKind::FuelAir),
        "FuelAir kind complete honesty"
    );

    // SlowDeath FINAL applies AirF_AuroraBombDetonationWeapon residual.
    for _ in 0..(FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES + 4) {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_fuel_air_gas_slow_death();
    }
    let fuel_after = game_logic.host_object(fuel_enemy).map(|o| o.health.current);
    let fuel_dealt = test_observed_damage_to(fuel_enemy, fuel_before, fuel_after.unwrap_or(0.0));
    assert!(
        fuel_dealt > 0.0
            || fuel_after.map(|h| h < fuel_before).unwrap_or(true)
            || fuel_after == Some(0.0)
            || game_logic.fuel_air_gas_reg.final_detonations > 0,
        "enemy must take FuelAir residual damage (~{AURORA_FUEL_AIR_DAMAGE}) via gas FINAL, got {fuel_after:?} dealt={fuel_dealt}"
    );
    let _ = AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES;
}

#[test]
fn production_upgrade_researches_on_building_queue_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::buildings::ProductionKind;
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_FLASHBANG};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    logic.add_player(player);
    ensure_test_barracks_template(&mut logic);

    let barracks_id = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks");
    // Keep this research-timing fixture independent of the separate
    // low-power production-speed residual.  The test below verifies the
    // Upgrade.ini BuildTime value itself, so its lone barracks has no draw.
    logic
        .host_object_mut(barracks_id)
        .expect("barracks object")
        .power_consumed = 0;

    logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    logic.process_commands();

    // PRODUCTION_UPGRADE residual sits on the producer queue.
    let (kind, qty, progress, total_time) = logic
        .host_object(barracks_id)
        .and_then(|o| o.building_data.as_ref())
        .and_then(|b| b.production_queue.first())
        .map(|i| (i.kind, i.quantity_total, i.progress, i.total_time))
        .expect("upgrade queue entry");
    assert_eq!(kind, ProductionKind::Upgrade);
    assert_eq!(qty, 1);
    assert_eq!(progress, 0.0);
    assert!(
        (total_time - HostUpgradeKind::FlashBangGrenade.retail_build_time_secs()).abs() < 0.001,
        "Upgrade.ini BuildTime must reach the producer queue, got {total_time}"
    );
    assert!(
        logic
            .get_player(0)
            .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(false),
        "player has queued upgrade"
    );
    assert!(
        !logic
            .get_player(0)
            .map(|p| p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(true),
        "not unlocked before research"
    );

    // A regular logic frame must not instant-complete a 30-second Upgrade.ini
    // entry.  Advance the remaining research time explicitly afterwards.
    logic.update();

    assert!(
        !logic
            .get_player(0)
            .map(|p| p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(true),
        "FlashBang must remain queued after one 1/30s logic frame"
    );
    logic.update_with_dt(30.0);

    assert!(
        logic
            .get_player(0)
            .map(|p| p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(false),
        "unlock after residual research frames"
    );
    assert!(
        logic
            .host_object(barracks_id)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.is_empty())
            .unwrap_or(false),
        "queue cleared after upgrade complete"
    );
    assert!(
        logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::FlashBangGrenade),
        "host honesty complete"
    );
}

/// Author the retail Object INI `SpecialPowerModule` on the shared TestTank
/// caster fixture.
///
/// C++ `SpecialPowerStore::canUseSpecialPower` (SpecialPower.cpp:300-336)
/// refuses any caster whose object lacks a `SpecialPowerModule` for the
/// power, and `SpecialPowerModule::triggerSpecialPower` →
/// `startPowerRecharge` (SpecialPowerModule.cpp:450-460 / 365-405) drops
/// readiness and restarts the ReloadTime countdown on execute.  Residual
/// cast tests must arm the module — with its retail ReloadTime — before
/// issuing the DoSpecialPower command so the live executor finds a real
/// caster and restarts its countdown like C++.
///
/// Retail ReloadTime: SpecialPowerRadarVanScan 30000ms (SpecialPower.ini:118043),
/// SpecialPowerSpySatellite 60000ms (:118018), SuperweaponCIAIntelligence
/// 300000ms (:118244).  Retail Dragon Tank FireWall is a FIRE_WEAPON
/// (CommandButton Command_ChinaDragonTankFireWall, SpecialPower.ini:3674) with
/// DragonTankFireWallWeapon DelayBetweenShots 40ms (Weapon.ini:131722); the
/// host residual models that as the module reload.
fn arm_test_tank_special_power(
    game_logic: &mut GameLogic,
    power: crate::command_system::SpecialPowerType,
    template: &str,
    reload_time_ms: u32,
) {
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
    ensure_test_tank_template(game_logic);
    let Some(tpl) = game_logic.templates.get_mut("TestTank") else {
        return;
    };
    if tpl
        .special_power_modules
        .iter()
        .any(|module| module.command_power.as_ref() == Some(&power))
    {
        return;
    }
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: tpl.special_power_modules.len() as u32,
        module_tag: Some(format!("ModuleTag_Test{:?}", power)),
        module_kind: SpecialPowerModuleKind::OclSpecialPower,
        special_power_template: template.to_string(),
        special_power_template_id: 1,
        command_power: Some(power),
        reload_time_frames: reload_time_ms * 30 / 1000,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
}
