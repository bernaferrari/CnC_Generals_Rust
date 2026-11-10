//! Behavior suite extracted from `production_and_mobs`: terrain-cell selection,
//! construction/general start, addon + residual unit bodies, deploy-style units,
//! jet rearm, and command-button hunt.
use super::*;

#[test]
fn plant_approach_ignores_the_structure() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("PlanterApproach");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("PlanterApproach".into(), inf_t);
    let mut bld_t = ThingTemplate::new("PlantApproachBuilding");
    bld_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic
        .templates
        .insert("PlantApproachBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("PlantApproachBuilding", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("building");
    let planter = logic
        .create_object_for_player("PlanterApproach", 0, Vec3::ZERO)
        .expect("planter");
    logic.pending_special_abilities.insert(
        planter,
        PendingSpecialAbility::PlantTimedDemoCharge {
            target_id: building,
        },
    );
    {
        let ai_entry_frame = logic.frame;
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(planter)
            .expect("planter");
        unit.set_ai_state(
            AIState::SpecialAbility,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(building),
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = None;
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(120.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            building.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update_support_states_for_test(&[planter], 1.0 / 30.0);
    logic.process_pathfind_queue();
    let unit = logic.host_object(planter).expect("planter");
    assert_eq!(unit.ai_state(), AIState::SpecialAbility);
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "plant approach must path through the structure, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn successful_disguise_drops_the_vehicle_ignore() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("DisguiseOkInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("DisguiseOkInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("DisguiseOkTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("DisguiseOkTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("DisguiseOkTruck", 1, Vec3::ZERO)
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("DisguiseOkInf", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::DisguiseAsVehicle { target_id: vehicle },
    );
    {
        let ai_entry_frame = logic.frame;
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(infantry)
            .expect("infantry");
        unit.set_ai_state(
            AIState::SpecialAbility,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(vehicle),
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(vehicle);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(
        unit.disguise_pending_template.as_deref(),
        Some("DisguiseOkTruck")
    );
    assert!(!unit.status.disguised);
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.movement.path.is_empty());
}

#[test]
fn flee_after_plant_ignores_the_structure() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("Planter");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Planter".into(), inf_t);
    let mut bld_t = ThingTemplate::new("TrappedBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("TrappedBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("TrappedBuilding", 0, Vec3::ZERO)
        .expect("building");
    let planter = logic
        .create_object_for_player("Planter", 0, Vec3::ZERO)
        .expect("planter");
    {
        let ai_entry_frame = logic.frame;
        let (unit, fixture_partition_unit) = logic
            .host_object_and_partition_mut(planter)
            .expect("planter");
        unit.ignored_obstacle_id = None;
        unit.set_ai_state(
            AIState::SpecialAbility,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.set_orientation_with_partition_owner(0.0, fixture_partition_unit);
    }
    use crate::game_logic::pathfinding::GridPos;
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(-40.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            building.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.leftover_flee_after_plant(planter, building, Team::USA, 40.0, false);
    logic.process_pathfind_queue();
    let unit = logic.host_object(planter).expect("planter");
    assert_eq!(unit.ai_state(), AIState::Moving);
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "flee after plant must path through the structure, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn recrew_enter_ignores_the_empty_vehicle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("RecrewInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("RecrewInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("EmptyHusk");
    veh_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
    logic.templates.insert("EmptyHusk".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("EmptyHusk", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("RecrewInf", 0, Vec3::ZERO)
        .expect("infantry");
    logic
        .host_object_mut(vehicle)
        .expect("vehicle")
        .set_status_disabled_unmanned(true);
    {
        let ai_entry_frame = logic.frame;
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(infantry)
            .expect("infantry");
        unit.set_order_target(
            Some(vehicle),
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::Entering,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    use crate::game_logic::pathfinding::GridPos;
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(200.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            vehicle.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(vehicle));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "recrew enter must path through the vehicle column, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn water_cell_set_change_falls_back_to_ground_not_the_previous_member() {
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    use crate::game_logic::object::LOCO_SURFACE_CLIFF;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("WaterBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeCliffLocomotor".into(),
            "CombatBikeGroundLocomotor".into(),
        ],
    }]);
    logic.templates.insert("WaterBike".into(), template);
    let id = logic
        .create_object_for_player("WaterBike", 0, Vec3::ZERO)
        .expect("bike");
    {
        let bike = logic.host_object_mut(id).expect("bike");
        bike.locomotor_surfaces = LOCO_SURFACE_CLIFF;
        bike.precise_z_pos = true;
    }
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Water);
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    {
        let bike = logic.host_object(id).expect("bike");
        assert_eq!(
            bike.cur_locomotor_name.as_deref(),
            Some("CombatBikeGroundLocomotor"),
            "a cell with no matching member must bind GROUND, not the previous cliff locomotor"
        );
        assert!(!bike.precise_z_pos, "changing the member clears precise-z");
    }
    {
        let bike = logic.host_object_mut(id).expect("bike");
        bike.precise_z_pos = true;
    }
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    let bike = logic.host_object(id).expect("bike");
    assert!(bike.precise_z_pos, "the same set must not clear precise-z");
    assert_eq!(
        bike.cur_locomotor_name.as_deref(),
        Some("CombatBikeGroundLocomotor")
    );
}

#[test]
fn missing_bridge_layer_uses_the_ground_cell_not_clear() {
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("BridgeBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeGroundLocomotor".into(),
            "RaptorJetLocomotor".into(),
        ],
    }]);
    logic.templates.insert("BridgeBike".into(), template);
    let id = logic
        .create_object_for_player("BridgeBike", 0, Vec3::ZERO)
        .expect("bike");
    {
        let bike = logic.host_object_mut(id).expect("bike");
        // Above LAYER_GROUND, with no deck cell. getCell falls through to the map.
        bike.pathfind_layer = 2;
    }
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Water);
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    let bike = logic.host_object(id).expect("bike");
    assert_eq!(
        bike.cur_locomotor_name.as_deref(),
        Some("RaptorJetLocomotor"),
        "a missing bridge cell must use the water underneath, which only the air member fits"
    );
}

#[test]
fn bridge_deck_locomotor_ignores_the_water_underneath() {
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("DeckBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeGroundLocomotor".into(),
            "RaptorJetLocomotor".into(),
        ],
    }]);
    logic.templates.insert("DeckBike".into(), template);
    let id = logic
        .create_object_for_player("DeckBike", 0, Vec3::ZERO)
        .expect("bike");
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Water);
    ({
        let ground_owner = logic.world_services.terrain();
        let ground_loan = ground_owner
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &ground_loan,
            logic.map_loaded,
            logic.terrain.as_ref(),
            logic.pathfinding_height_samples.as_ref(),
            &logic.pathfinding_system.grid,
        );
        logic.pathfinding_system.grid.stamp_bridge_deck(
            Vec3::new(-30.0, 20.0, -15.0),
            Vec3::new(-30.0, 20.0, 15.0),
            Vec3::new(30.0, 20.0, -15.0),
            Vec3::new(30.0, 20.0, 15.0),
            false,
            &ground_source,
        )
    });
    let layer = logic
        .pathfinding_system
        .grid
        .first_bridge_layer_id()
        .expect("deck layer");
    assert_eq!(
        logic.pathfinding_system.grid.layer_cell_type(layer, cell),
        Some(PathfindCellType::Clear),
        "the deck over the bike must be Clear"
    );
    {
        let bike = logic.host_object_mut(id).expect("bike");
        bike.pathfind_layer = layer;
    }
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    assert_eq!(
        logic.host_object(id).unwrap().cur_locomotor_name.as_deref(),
        Some("CombatBikeGroundLocomotor"),
        "the deck is Clear, so the ground member wins over the air member"
    );
    logic.update();
    assert_eq!(
        logic.host_object(id).unwrap().cur_locomotor_name.as_deref(),
        Some("CombatBikeGroundLocomotor"),
        "the movement tick must keep the deck cell, not the water underneath"
    );
}

#[test]
fn queued_infantry_spawns_during_simulation() {
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(player) = logic.get_player_mut(0) {
        player.resources.supplies = 50_000;
        player.power_available = 100;
    }
    // Energy.cpp:51-65 and ThingTemplate.cpp:1541-1554 use produced/consumed
    // power, not the legacy power_available scalar. Keep this producer powered
    // through the real object-owned energy fold on every simulation frame.
    let mut power = ThingTemplate::new("InfantryQueueTestPower");
    power.add_kind_of(KindOf::Structure).set_health(1000.0);
    power.energy_production = Some(100);
    logic
        .templates
        .insert("InfantryQueueTestPower".into(), power);
    let power_id = logic
        .create_object_for_player("InfantryQueueTestPower", 0, Vec3::new(-200.0, 0.0, -200.0))
        .expect("completed owner-local power source");
    assert!(logic.host_object(power_id).unwrap().is_constructed());
    logic.restore_player_power();
    assert_eq!(logic.get_player(0).unwrap().power_produced, 100);

    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    if let Some(infantry) = logic.templates.get_mut("TestInfantry") {
        infantry.build_time = 0.05;
    }
    let barracks = logic
        .create_object_for_player("TestBarracks", 0, Vec3::ZERO)
        .expect("barracks");
    if let Some(building) = logic.host_object_mut(barracks) {
        building.construction_percent = 1.0;
        building.set_status_under_construction(false);
    }
    assert!(logic.enqueue_production(barracks, "TestInfantry".to_string()));
    for _ in 0..60 {
        logic.update_with_dt_budget(LOGIC_FRAME_TIMESTEP, 1);
    }
    let unit = logic
        .objects
        .registered_values()
        .find(|object| object.template_name == "TestInfantry")
        .expect("spawned infantry");
    assert!(
        unit.get_position().distance(Vec3::ZERO) > 5.0,
        "spawned infantry must walk clear of the barracks, pos={:?} ai={:?} vel={:?} target={:?} path={:?} waiting={}",
        unit.get_position(),
        unit.ai_state(),
        unit.movement.velocity,
        unit.movement.target_position,
        unit.movement.path,
        unit.waiting_for_path
    );
    assert_eq!(
        unit.owner_player_id,
        Some(0),
        "spawned infantry must belong to the factory owner"
    );
}

#[test]
fn reissued_build_stays_constructing() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("ReissuePad");
    pad.add_kind_of(KindOf::Structure).set_health(1_000.0);
    logic.templates.insert("ReissuePad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("ReissueDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("ReissueDozer".into(), dozer_tpl);
    let from = Vec3::new(10.0, 0.0, 10.0);
    let to = Vec3::new(160.0, 0.0, 10.0);
    let pad_id = logic
        .create_object_for_player("ReissuePad", 0, to)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("ReissueDozer", 0, from)
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start_cell = grid.world_to_grid(from);
        let goal_cell = grid.world_to_grid(to);
        (start_cell.x + goal_cell.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            pad_id.0,
            None,
            None,
        );
    }
    {
        let ai_entry_frame = logic.frame;
        let (obj, attack_events) = logic
            .host_object_and_attack_events_mut(dozer)
            .expect("dozer");
        obj.set_order_target(
            Some(pad_id),
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.set_ai_state(
            AIState::Constructing,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.pending_move = Some(to);
        obj.shock_stun_frames = 0;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let (state, path) = {
        let obj = logic.host_object(dozer).expect("installed");
        (obj.ai_state().clone(), obj.movement.path.clone())
    };
    assert_eq!(state, AIState::Constructing);
    let xs: Vec<i32> = path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crossed = xs.windows(2).any(|pair| {
        let lo = pair[0].min(pair[1]);
        let hi = pair[0].max(pair[1]);
        (lo..=hi).contains(&wall_x)
    });
    assert!(
        crossed,
        "path must cross the owned obstacle column {wall_x}, cells={xs:?}"
    );
}

#[test]
fn ignored_structure_keeps_its_stored_position() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("StampPad");
    pad.add_kind_of(KindOf::Structure).set_health(1_000.0);
    logic.templates.insert("StampPad".into(), pad);
    let at = Vec3::new(80.0, 0.0, 40.0);
    let pad_id = logic
        .create_object_for_player("StampPad", 0, at)
        .expect("pad");
    let cell = logic.pathfinding_system.grid.world_to_grid(at);
    ({
        let ground_owner = logic.world_services.terrain();
        let ground_loan = ground_owner
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &ground_loan,
            logic.map_loaded,
            logic.terrain.as_ref(),
            logic.pathfinding_height_samples.as_ref(),
            &logic.pathfinding_system.grid,
        );
        logic
            .pathfinding_system
            .grid
            .update_dynamic_obstacles(logic.objects.read(), &ground_source)
    });
    assert_eq!(
        logic.pathfinding_system.grid.ground_position_unit(cell),
        pad_id.0,
        "a live structure stamps its cell"
    );
    ({
        let ground_owner = logic.world_services.terrain();
        let ground_loan = ground_owner
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &ground_loan,
            logic.map_loaded,
            logic.terrain.as_ref(),
            logic.pathfinding_height_samples.as_ref(),
            &logic.pathfinding_system.grid,
        );
        logic
            .pathfinding_system
            .grid
            .update_dynamic_obstacles_ignoring(logic.objects.read(), Some(pad_id), &ground_source)
    });
    assert_eq!(
        logic.pathfinding_system.grid.ground_position_unit(cell),
        pad_id.0,
        "an ignore request must not erase the stored occupant"
    );
}
#[test]
fn construction_complete_end_dock_uses_stored_action() {
    // hq-pogoh: complete END is ACTION + 5 cells, not the dozer's current pose.
    use crate::game_logic::host_repair::dozer_complete_end_dock;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));

    let mut pad = ThingTemplate::new("EndDockPad");
    pad.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    pad.build_time = 10.0;
    logic.templates.insert("EndDockPad".into(), pad);

    let mut dozer_tpl = ThingTemplate::new("EndDockDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("EndDockDozer".into(), dozer_tpl);

    let pad_id = logic
        .create_object_for_player("EndDockPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("EndDockDozer", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
    let action = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action())
        .expect("ACTION");
    let off_line = Vec3::new(0.0, 0.0, 80.0);
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.construction_percent = 1.0;
        obj.builder_id = Some(dozer);
    }
    {
        let ai_entry_frame = logic.frame;
        let (obj, attack_events, _fixture_audio_obj, fixture_partition_obj) = logic
            .host_object_attack_audio_partition_mut(dozer)
            .expect("dozer");
        obj.set_position_with_partition_owner(
            off_line,
            0,
            &mut Default::default(),
            fixture_partition_obj,
        );
        obj.set_target(
            Some(pad_id),
            ai_entry_frame,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.set_ai_state(
            AIState::Constructing,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.status.moving = false;
    }

    logic.update_construction(&[pad_id], 1.0);
    let dz = logic.host_object(dozer).expect("dz");
    let dest = dz
        .movement
        .path
        .last()
        .copied()
        .or(dz.requested_destination)
        .expect("END destination");
    let expected = dozer_complete_end_dock(Some(action), off_line, Vec3::ZERO);
    assert!(
        (dest - expected).length() < 2.0,
        "hq-pogoh: END must come from stored ACTION, dest={dest:?} expected={expected:?}"
    );
}

#[test]
fn selected_general_start_binds_exact_template_and_rejects_late_invalid_identity() {
    let selected = PlayerTemplateIdentity::from_exact_name("FactionAmericaAirForceGeneral")
        .expect("retail Air Force General PlayerTemplate");
    game_engine::common::ini::ensure_player_templates_loaded();
    let (air_force_index, tank_index) = {
        let store = game_engine::common::rts::player_template::get_player_template_store();
        (
            store
                .find_template_index("FactionAmericaAirForceGeneral")
                .expect("retail Air Force General template") as i32,
            store
                .find_template_index("FactionChinaTankGeneral")
                .expect("retail Tank General template") as i32,
        )
    };
    assert!(
        PlayerTemplateIdentity::from_exact_indexed_name(
            "FactionAmericaAirForceGeneral",
            air_force_index,
        )
        .is_some()
    );
    assert!(
        PlayerTemplateIdentity::from_exact_indexed_name(
            "FactionAmericaAirForceGeneral",
            tank_index,
        )
        .is_none(),
        "a Challenge index is part of the selected General identity"
    );
    let mut logic = GameLogic::new();
    logic.admit_object_ini_sources(std::iter::empty()).unwrap();

    assert!(
        logic.start_new_game_with_player_template(GameMode::SinglePlayer, 0, selected.clone(),).unwrap()
    );
    let player = logic.get_player(0).expect("bound local player");
    assert_eq!(player.team, Team::USA);
    assert!(player.has_unlocked_science("SCIENCE_AMERICA"));
    assert_eq!(player.color_rgb, (0, 0, 255));
    assert_eq!(
        logic
            .player_template_identity(0)
            .map(|identity| identity.template_name.as_str()),
        Some(selected.template_name.as_str())
    );

    let invalid = PlayerTemplateIdentity {
        template_name: "MissingExactPlayerTemplate".to_string(),
        template_index: None,
    };
    assert!(
        !logic.start_new_game_with_player_template(GameMode::SinglePlayer, 0, invalid).unwrap(),
        "a late missing identity must not fall back to a USA/China/GLA team"
    );
    assert!(logic.get_player(0).is_none());
    assert!(logic.player_template_identity(0).is_none());

    let stale_index_pair = PlayerTemplateIdentity {
        template_name: "FactionChinaTankGeneral".to_string(),
        template_index: Some(air_force_index),
    };
    assert!(
        !logic.start_new_game_with_player_template(GameMode::SinglePlayer, 0, stale_index_pair).unwrap(),
        "GameLogic must independently reject a stale index/name pair before map load"
    );
    assert!(logic.get_player(0).is_none());
    assert!(logic.player_template_identity(0).is_none());
}

#[test]
fn overlord_propaganda_addon_residual_heals_allies() {

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    game_logic.admit_object_ini_sources([(
        "TestPortablePropaganda.ini".into(),
        "Object TestPortablePropaganda\nKindOf = PORTABLE_STRUCTURE SCORE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = PropagandaTowerBehavior Tower\nRadius = 150\nDelayBetweenUpdates = 2000\nHealPercentEachSecond = 1%\nEnd\nEnd\n".into(),
    )]).unwrap();
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::ZERO, None);
    let portable = game_logic
        .create_object("TestPortablePropaganda", Team::China, Vec3::ZERO)
        .expect("portable tower");
    game_logic.attach_overlord_portable_occupant(overlord_id, portable);
    assert!(game_logic.host_object(portable).unwrap()
        .authors_behavior_module("PropagandaTowerBehavior"));
    assert_eq!(game_logic.host_object(overlord_id).unwrap().overlord_portable_occupant, Some(portable));

    let ally_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .expect("ally");
    {
        let a = game_logic.host_object_mut(ally_id).unwrap();
        a.health.current = a.health.maximum * 0.5;
    }
    let hp_before = game_logic.host_object(ally_id).unwrap().health.current;
    // Pulse residual for 1 second (30 frames @ 1/30).
    game_logic.frame = 1;
    game_logic.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let hp_after = game_logic.host_object(ally_id).unwrap().health.current;
    assert!(
        hp_after > hp_before + 0.01,
        "propaganda addon must heal ally (before={hp_before} after={hp_after})"
    );
    assert!(
        game_logic.honesty_propaganda_heal_ok(),
        "propaganda residual honesty"
    );
}

#[test]
fn overlord_portable_addon_mirrors_host_body_damage() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut game_logic = GameLogic::new();
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::ZERO, None);
    let mut turret = ThingTemplate::new("TestPortableGattling");
    turret.add_kind_of(KindOf::PortableStructure).set_health(1100.0);
    game_logic.templates.insert(turret.name.clone(), turret);
    let occupant = game_logic.create_object("TestPortableGattling", Team::China, Vec3::ZERO)
        .expect("portable gun");
    game_logic.attach_overlord_portable_occupant(overlord_id, occupant);

    let occupant_id = {
        let o = game_logic.host_object(overlord_id).unwrap();
        o.overlord_portable_occupant
            .expect("portable occupant spawned")
    };
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert!(addon.is_kind_of(KindOf::PortableStructure));
        assert_eq!(addon.body_damage_state, HostBodyDamageType::Pristine);
        assert_eq!(addon.contained_by, Some(overlord_id));
    }

    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.internal_change_health(o.health.maximum * 0.5 - o.health.current);
        assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
    }
    game_logic.mirror_overlord_addon_damage_to_occupant(overlord_id);
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert_eq!(
            addon.body_damage_state,
            HostBodyDamageType::Damaged,
            "gattling addon must go yellow with the hull"
        );
    }

    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.internal_change_health(o.health.maximum * 0.2 - o.health.current);
        assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
    }
    game_logic.mirror_overlord_addon_damage_to_occupant(overlord_id);
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert_eq!(
            addon.body_damage_state,
            HostBodyDamageType::ReallyDamaged,
            "gattling addon must go red with the hull"
        );
    }

    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.internal_change_health(-o.health.current);
        o.status.destroyed = true;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Rubble);
    }
    game_logic.mirror_overlord_addon_damage_to_occupant(overlord_id);
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert_eq!(
            addon.body_damage_state,
            HostBodyDamageType::ReallyDamaged,
            "portable addon must not be set to rubble by the host state change"
        );
    }
}

#[test]
fn emperor_innate_propaganda_and_helix_transport_residual() {
    use crate::game_logic::host_overlord_addons::HELIX_TRANSPORT_SLOTS;

    // hq-2qedt.256: Emperor innate tower is the authored PropagandaTowerBehavior
    // on an OverlordContain host (TankGeneral.ini), not a name peel.
    let mut game_logic = GameLogic::new();
    game_logic
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "emperor.ini".into(),
                "Object Tank_ChinaTankEmperor\nKindOf = VEHICLE SELECTABLE SCORE\nBody = ActiveBody Body\nMaxHealth = 1100\nInitialHealth = 1100\nEnd\nBehavior = OverlordContain ModuleTag_06\nSlots = 1\nEnd\nBehavior = PropagandaTowerBehavior ModulePropaganda_04\nRadius = 150.0\nDelayBetweenUpdates = 2000\nHealPercentEachSecond = 1%\nEnd\nEnd\nObject TestAllyTank\nKindOf = VEHICLE SELECTABLE SCORE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
                    .to_string(),
            )])
            .unwrap(),
        )
        .unwrap();
    game_logic.add_player(Player::new(1, Team::China, "owner", true));
    let spawn = |game_logic: &mut GameLogic, name: &str, pos: Vec3| {
        game_logic
            .create_object_for_player_with_effects_checked(
                name,
                1,
                pos,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap()
    };

    let emp_id = spawn(&mut game_logic, "Tank_ChinaTankEmperor", Vec3::new(0.0, 0.0, 0.0));
    {
        let e = game_logic.host_object(emp_id).unwrap();
        assert!(e.authors_innate_propaganda_tower());
    }

    let ally_id = spawn(&mut game_logic, "TestAllyTank", Vec3::new(10.0, 0.0, 0.0));
    {
        let a = game_logic.host_object_mut(ally_id).unwrap();
        a.health.current = a.health.maximum * 0.5;
    }
    let hp_before = game_logic.host_object(ally_id).unwrap().health.current;
    // C++ attemptHealingFromSoleBenefactor needs now > expiration (0) to claim.
    game_logic.frame = 1;
    game_logic.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let hp_after = game_logic.host_object(ally_id).unwrap().health.current;
    assert!(
        hp_after > hp_before + 0.01,
        "emperor innate propaganda must heal (before={hp_before} after={hp_after})"
    );

    let mut game_logic = GameLogic::new();
    // Helix transport residual.
    let mut helix_tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleHelix");
    helix_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    helix_tpl.contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
    helix_tpl.contain_module.is_helix_contain = true;
    helix_tpl.contain_module.slots = Some(HELIX_TRANSPORT_SLOTS);
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), helix_tpl);
    let helix_id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(100.0, 0.0, 0.0))
        .expect("helix");
    {
        let h = game_logic.host_object(helix_id).unwrap();
        assert!(h.is_helix_transport);
        assert_eq!(h.transport_capacity(), HELIX_TRANSPORT_SLOTS);
        assert!(
            !h.passengers_allowed_to_fire,
            "stock Helix fire is gated on Battle Bunker"
        );
    }
}

#[test]
fn battle_bus_residual_capacity_and_flags_installed() {
    let mut game_logic = GameLogic::new();
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let bus = game_logic.host_object(bus_id).expect("bus");
    assert!(bus.is_battle_bus_style_container());
    assert!(bus.can_contain());
    assert_eq!(
        bus.transport_capacity(),
        crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS
    );
    assert!(bus.passengers_allowed_to_fire);
    assert!(bus.armed_riders_upgrade_weapon_set);
    assert!(!bus.test_weapon_condition(3));
}

#[test]
fn battle_bus_residual_enter_sets_docked_and_upgrades_weapon_set() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    // C++ objects always have a controlling player; register GLA so
    // create_object stamps owner and Enter resolves Allies.  Author the
    // TestInfantry metadata (KindOf + TransportSlotCount=1): C++
    // Object::getTransportSlotCount (Object.cpp:700-717) is the raw INI value
    // and a zero-slot source can never board a capacity-checked transport.
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    // Armed rider residual (rifle) so ArmedRidersUpgradeMyWeaponSet applies.
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        crate::fixture_content::arm_with_stats(
            unit,
            crate::game_logic::Weapon {
                damage: 25.0,
                range: 100.0,
                reload_time: 0.5,
                ..Default::default()
            },
        );
        unit.weapon.as_mut().unwrap().last_fire_time = -10.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: bus_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic
        .update_ai(&[infantry_id, bus_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");

    let bus = game_logic.host_object(bus_id).expect("bus after");
    assert!(bus.contained_units().contains(&infantry_id));
    assert_eq!(bus.transport_count(), 1);
    assert!(
        bus.test_weapon_condition(3),
        "armed riders must upgrade weapon set"
    );
    assert!(
        bus.weapon.is_some(),
        "PLAYER_UPGRADE residual binds passenger dummy weapon"
    );

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(infantry.ai_state(), AIState::Docked);
    assert_eq!(infantry.contained_by, Some(bus_id));
    assert_eq!(game_logic.battle_bus_residual_loads(), 1);
    assert_eq!(
        game_logic.transport_residual_loads(),
        0,
        "Battle Bus load must not count as generic transport load"
    );
    assert!(
        game_logic.honesty_battle_bus_weapon_set_upgrade_ok(),
        "weapon-set upgrade residual honesty"
    );
}

#[test]
fn battle_bus_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let unit_a = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    for unit_id in [unit_a, unit_b] {
        {
            let ai_entry_frame = game_logic.frame;
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            crate::fixture_content::arm_with_stats(
                unit,
                crate::game_logic::Weapon {
                    damage: 20.0,
                    range: 80.0,
                    reload_time: 0.5,
                    ..Default::default()
                },
            );
            unit.weapon.as_mut().unwrap().last_fire_time = -10.0;
            unit.target = Some(bus_id);
            unit.set_ai_state(
                AIState::Entering,
                ai_entry_frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        game_logic
            .update_ai(&[unit_id, bus_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }

    let bus = game_logic.host_object(bus_id).expect("bus loaded");
    assert!(
        bus.contained_units().contains(&unit_a) && bus.contained_units().contains(&unit_b),
        "both infantry must be loaded into Battle Bus residual"
    );
    assert_eq!(bus.transport_count(), 2);
    assert_eq!(game_logic.battle_bus_residual_loads(), 2);
    assert!(bus.test_weapon_condition(3));

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state(), AIState::Docked);
        assert_eq!(unit.contained_by, Some(bus_id));
        assert!(!unit.can_move());
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 2,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // C++ TransportContain does not dump riders synchronously: Evacuate calls
    // orderAllPassengersToExit → aiExit per rider (OpenContain.cpp:1353-1371)
    // and each exit is paced by the transport exit door (TransportContain
    // ExitDelay). Advance frames so the exit door cycles both riders out and
    // their exit walks settle, matching the C++ stream and the Combat Chinook
    // residual twin below.
    for _ in 0..60 {
        game_logic.frame += 1;
        game_logic.update_movement_for_test(&[bus_id, unit_a, unit_b], 1.0 / 30.0);
        game_logic
            .update_ai(&[bus_id, unit_a, unit_b], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }
    let bus = game_logic.host_object(bus_id).expect("bus empty");
    assert!(
        bus.contained_units().is_empty(),
        "evacuate must clear all Battle Bus residual occupants"
    );
    assert_eq!(bus.transport_count(), 0);
    assert!(
        !bus.test_weapon_condition(3),
        "weapon set upgrade must clear when empty"
    );

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        // C++ OpenContain::exitObjectViaDoor places the rider at ExitStart and
        // issues aiFollowPath to ExitEnd (OpenContain.cpp:915-1020); the freed
        // rider walks its exit path instead of Idling in place.
        assert_eq!(
            unit.ai_state(),
            AIState::Moving,
            "unloaded unit must walk its exit path (C++ aiFollowPath)"
        );
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.battle_bus_residual_unloads(), 2);
    assert!(
        game_logic.honesty_battle_bus_load_unload_ok(),
        "load+unload residual honesty"
    );
    assert_eq!(
        game_logic.transport_residual_unloads(),
        0,
        "Battle Bus unload must not count as generic transport unload"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "Battle Bus unload must not count as garrison exit"
    );
}

#[test]
fn battle_bus_residual_passenger_fire_damages_nearby_enemy() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("infantry");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");

    {
        let ai_entry_frame = game_logic.frame;
        let (unit, fixture_partition_unit) = game_logic
            .host_object_and_partition_mut(infantry_id)
            .unwrap();
        crate::fixture_content::arm_with_stats(
            unit,
            crate::game_logic::Weapon {
                damage: 40.0,
                range: 100.0,
                reload_time: 0.1,
                ..Default::default()
            },
        );
        unit.weapon.as_mut().unwrap().last_fire_time = -10.0;
        unit.target = Some(bus_id);
        unit.set_contained_by_with_partition_owner(Some(bus_id), true, fixture_partition_unit);
        unit.set_ai_state(
            AIState::Docked,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.set_position_with_partition_owner(
            Vec3::new(0.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_unit,
        );
    }
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        assert!(bus.add_occupant(infantry_id));
    }
    game_logic.refresh_battle_bus_armed_riders_weapon_set(bus_id);

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.update_combat(&[infantry_id, bus_id, enemy_id], 1.0 / 30.0);

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "Battle Bus passenger residual fire must damage nearby enemy (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert!(
        game_logic.honesty_battle_bus_passenger_fire_ok(),
        "passenger fire residual honesty"
    );
    let rider = game_logic.host_object(infantry_id).unwrap();
    assert_eq!(
        rider.contained_by,
        Some(bus_id),
        "firing must not eject Battle Bus passenger"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().contained_by,
        Some(bus_id)
    );
}

#[test]
fn battle_bus_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    // Fill all 8 residual slots.
    let mut loaded = Vec::new();
    for i in 0..crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS {
        let id = game_logic
            .create_object(
                "TestInfantry",
                Team::GLA,
                Vec3::new(1.0 + i as f32 * 0.1, 0.0, 0.0),
            )
            .expect("infantry");
        {
            let ai_entry_frame = game_logic.frame;
            let unit = game_logic.host_object_mut(id).unwrap();
            unit.target = Some(bus_id);
            unit.set_ai_state(
                AIState::Entering,
                ai_entry_frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        game_logic
            .update_ai(&[id, bus_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        loaded.push(id);
    }
    assert_eq!(
        game_logic.host_object(bus_id).unwrap().transport_count(),
        crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS
    );

    let extra_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
        .expect("extra");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: bus_id },
        player_id: 2,
        command_id: 9,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![extra_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let extra = game_logic.host_object(extra_id).expect("extra after");
    assert_ne!(
        extra.ai_state(),
        AIState::Entering,
        "full Battle Bus residual must reject Enter"
    );
    assert_eq!(
        game_logic.host_object(bus_id).unwrap().transport_count(),
        crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS
    );
    let _ = loaded;
}

#[test]
fn battle_bus_residual_rejects_vehicle_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: bus_id },
        player_id: 2,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(
        tank.ai_state(),
        AIState::Entering,
        "vehicles must not enter Battle Bus residual"
    );
    assert_eq!(game_logic.battle_bus_residual_loads(), 0);
    assert!(
        game_logic
            .host_object(bus_id)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}

#[test]
fn battle_bus_undead_body_first_life_converts_to_second_life() {
    let mut game_logic = GameLogic::new();
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.health.maximum = 400.0;
        bus.health.current = 50.0;
        bus.template_mut().armor = 0.0;
    }
    // Lethal explosion should intercept → second life 650 HP full.
    // BattleBusSlowDeathBehavior.cpp:119 arms the driving match frame + 10.
    let damage_type = crate::game_logic::combat::DamageType::Explosive;
    let killed = game_logic
        .apply_owned_damage(
            bus_id,
            500.0,
            None,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
            None,
            &crate::game_logic::object::DamageHitContext::new(None, None, damage_type),
        )
        .expect("receiving BattleBus body")
        .destroyed();
    assert!(!killed, "UndeadBody must intercept first lethal hit");
    let bus = game_logic.host_object(bus_id).unwrap();
    assert!(bus.is_alive());
    assert!(
        (bus.health.maximum - 650.0).abs() < 0.1,
        "second life max {}",
        bus.health.maximum
    );
    assert!((bus.health.current - 650.0).abs() < 0.1);
    assert!(bus.armor_set_second_life);
    let body = bus.battle_bus_body.as_ref().expect("body");
    assert!(body.is_second_life);
    // Tick drains pending passenger damage + progresses air time / land.
    for f in 1..25 {
        game_logic.frame = f;
        game_logic.tick_battle_bus_slow_deaths();
    }
    assert!(game_logic.battle_bus.honesty_undeath_detonate_ok());
    let bus = game_logic.host_object(bus_id).unwrap();
    let body = bus.battle_bus_body.as_ref().unwrap();
    assert!(
        body.landed_hulk,
        "first death should land after ground check"
    );
    assert!(
        bus.model_condition_bits
            & (1u128 << crate::game_logic::host_battle_bus::BATTLE_BUS_MC_BIT_SECOND_LIFE)
            != 0
    );
}

#[test]
fn battle_bus_undead_damages_passengers_and_empty_hulk_destroys() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(10.0, 10.0, 0.0));
    let rider_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(10.0, 12.0, 0.0))
        .expect("rider");
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.health.maximum = 400.0;
        bus.health.current = 40.0;
        bus.template_mut().armor = 0.0;
    }
    {
        let r = game_logic.host_object_mut(rider_id).unwrap();
        r.health.maximum = 100.0;
        r.health.current = 100.0;
    }
    // Force dock residual.
    if let Some(bus) = game_logic.host_object_mut(bus_id) {
        if !bus.occupants.contains(&rider_id) {
            bus.occupants.push(rider_id);
        }
    }
    if let Some(r) = game_logic.host_object_mut(rider_id) {
        r.contained_by = Some(bus_id);
    }
    // BattleBusSlowDeathBehavior.cpp:119 arms the driving match frame + 10.
    let damage_type = crate::game_logic::combat::DamageType::Explosive;
    let _ = game_logic
        .apply_owned_damage(
            bus_id,
            999.0,
            None,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
            None,
            &crate::game_logic::object::DamageHitContext::new(None, None, damage_type),
        )
        .expect("receiving BattleBus body")
        .destroyed();
    // First tick applies 50% passenger damage.
    game_logic.frame = 1;
    game_logic.tick_battle_bus_slow_deaths();
    let rider_hp = game_logic
        .host_object(rider_id)
        .map(|r| r.health.current)
        .unwrap_or(0.0);
    assert!(
        (rider_hp - 50.0).abs() < 0.5,
        "PercentDamageToPassengers 50% residual, got {rider_hp}"
    );
    // Unload rider so empty hulk can fire.
    if let Some(bus) = game_logic.host_object_mut(bus_id) {
        bus.occupants.clear();
    }
    if let Some((r, fixture_partition_r)) = game_logic.host_object_and_partition_mut(rider_id) {
        r.set_contained_by_with_partition_owner(None, true, fixture_partition_r);
    }
    // Advance past ground check + land + empty delay.
    for f in 2..90 {
        game_logic.frame = f;
        game_logic.tick_battle_bus_slow_deaths();
    }
    let bus = game_logic
        .host_object(bus_id)
        .expect("body death precedes physical removal");
    assert_eq!(bus.health.current, 0.0);
    assert!(
        bus.status.on_die_started,
        "empty-hulk penalty must enter onDie"
    );
    assert_eq!(
        bus.status.death_type.ordinal(),
        15,
        "CPP DEATH_EXTRA_4 selects the real death module"
    );
    assert!(bus.battle_bus_body.as_ref().unwrap().is_real_death);
}

#[test]
fn battle_bus_unresistable_bypasses_undead_body() {
    let mut game_logic = GameLogic::new();
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::ZERO);
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.health.maximum = 400.0;
        bus.health.current = 50.0;
        bus.template_mut().armor = 0.0;
    }
    let killed = {
        let (bus, health_events) = game_logic
            .host_object_and_health_events_mut(bus_id)
            .unwrap();
        bus.take_damage_from_typed(
            500.0,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            0,
            health_events,
        )
    };
    assert!(killed, "UNRESISTABLE must bypass UndeadBody");
}

/// C++ selects HighlanderBody only from the authored Body module; a NatureProp
/// style template name without that Body must not install the clamp.
#[test]
fn unauthored_tree_named_template_does_not_install_highlander_body() {
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("Tree01UnauthoredBody");
    tpl.add_kind_of(KindOf::Structure).set_health(50.0);
    game_logic.templates.insert("Tree01UnauthoredBody".into(), tpl);
    let id = game_logic
        .create_object("Tree01UnauthoredBody", Team::Neutral, Vec3::ZERO)
        .expect("tree");
    assert!(
        !game_logic.host_object(id).unwrap().highlander_body,
        "template-name peel must not stand in for authored Body = HighlanderBody"
    );
}

#[test]
fn authored_highlander_body_installs_without_template_name_peel() {
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("PlainAuthoredRock");
    tpl.add_kind_of(KindOf::Structure).set_health(50.0);
    tpl.highlander_body = true;
    game_logic.templates.insert("PlainAuthoredRock".into(), tpl);
    let id = game_logic
        .create_object("PlainAuthoredRock", Team::Neutral, Vec3::ZERO)
        .expect("rock");
    assert!(game_logic.host_object(id).unwrap().highlander_body);
}

#[test]
fn highlander_body_clamps_normal_and_penalty_damage_unresistable_kills() {
    let mut game_logic = GameLogic::new();
    // Ensure template + highlander install.
    let mut tpl = ThingTemplate::new("TreeHighlanderTest");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Immobile)
        .set_health(50.0);
    // Fixture stands in for authored `Body = HighlanderBody`.
    tpl.highlander_body = true;
    game_logic
        .templates
        .insert("TreeHighlanderTest".into(), tpl);
    let id = game_logic
        .create_object("TreeHighlanderTest", Team::Neutral, Vec3::ZERO)
        .expect("tree");
    {
        let o = game_logic.host_object_mut(id).unwrap();
        assert!(
            o.highlander_body,
            "create_object must install HighlanderBody"
        );
        o.template_mut().armor = 0.0;
        o.health.maximum = 50.0;
        o.health.current = 50.0;
    }
    let killed = {
        let (o, health_events) = game_logic.host_object_and_health_events_mut(id).unwrap();
        o.take_damage_from_typed(
            999.0,
            None,
            crate::game_logic::combat::DamageType::Explosive,
            0,
            health_events,
        )
    };
    assert!(!killed);
    let o = game_logic.host_object(id).unwrap();
    assert!(o.is_alive());
    assert!(
        (o.health.current - 1.0).abs() < 0.01,
        "highlander must leave 1 HP, got {}",
        o.health.current
    );

    // C++ HighlanderBody::attemptDamage clamps every lethal type except the
    // literal DAMAGE_UNRESISTABLE comparison.  OverchargeBehavior uses
    // DAMAGE_PENALTY, so it belongs on the clamped side rather than sharing
    // the unresistable bypass.
    let penalty_id = game_logic
        .create_object("TreeHighlanderTest", Team::Neutral, Vec3::X * 10.0)
        .expect("penalty highlander");
    {
        let penalty = game_logic.host_object_mut(penalty_id).unwrap();
        assert!(penalty.highlander_body);
        penalty.template_mut().armor = 0.0;
        penalty.health.maximum = 50.0;
        penalty.health.current = 50.0;
    }
    let penalty_killed = {
        let (penalty, health_events) = game_logic
            .host_object_and_health_events_mut(penalty_id)
            .unwrap();
        penalty.take_damage_from_typed(
            999.0,
            None,
            crate::game_logic::combat::DamageType::Penalty,
            0,
            health_events,
        )
    };
    assert!(
        !penalty_killed,
        "DAMAGE_PENALTY must not bypass HighlanderBody's one-HP floor"
    );
    let penalty = game_logic.host_object(penalty_id).unwrap();
    assert!(penalty.is_alive());
    assert!(
        (penalty.health.current - 1.0).abs() < 0.01,
        "DAMAGE_PENALTY must leave one HP, got {}",
        penalty.health.current
    );

    // UNRESISTABLE kills.
    let killed2 = {
        let (o, health_events) = game_logic.host_object_and_health_events_mut(id).unwrap();
        o.take_damage_from_typed(
            10.0,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            0,
            health_events,
        )
    };
    assert!(killed2);
}

#[test]
fn deploy_style_sentry_must_unpack_before_fire_and_pack_before_move() {
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaVehicleSentryDrone");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    // The source behavior, rather than the retail template identity, grants
    // DeployStyle authority.  Keep this fixture deliberately name-agnostic.
    tpl.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        ..Default::default()
    });
    game_logic
        .templates
        .insert("AmericaVehicleSentryDrone".into(), tpl);
    let id = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    {
        let o = game_logic.host_object_mut(id).unwrap();
        assert!(o.deploy_style.is_some(), "install DeployStyle residual");
        assert!(o.deploy_style_allows_move());
        assert!(!o.deploy_style_allows_fire());
    }
    // Fire blocked until unpack completes.
    assert!(!game_logic.ensure_deploy_style_ready_to_fire(id));
    assert!(game_logic.deploy_style_reg.deploys > 0);
    // Advance unpack 30 frames.
    for f in 1..=30 {
        game_logic.frame = f;
        game_logic.tick_deploy_style_updates();
    }
    assert!(
        game_logic.ensure_deploy_style_ready_to_fire(id),
        "ready to attack after unpack"
    );
    assert!(game_logic.host_object(id).unwrap().is_deployed());

    // Move while deployed starts pack and blocks path.
    assert!(!game_logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
    assert!(game_logic.deploy_style_reg.undeploys > 0);
    // Pack completes → can path.
    let start = game_logic.frame;
    for f in 1..=30 {
        game_logic.frame = start + f;
        game_logic.tick_deploy_style_updates();
    }
    assert!(
        game_logic
            .host_object(id)
            .unwrap()
            .deploy_style_allows_move()
    );
    assert!(game_logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
}

#[test]
fn deploy_style_plays_authored_deploy_and_undeploy_sounds() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_deploy_style::{
        DEPLOY_STYLE_DEPLOY_AUDIO, DEPLOY_STYLE_UNDEPLOY_AUDIO,
    };

    clear_test_template_voices();
    set_test_per_unit_sound(
        "AmericaVehicleSentryDrone",
        DEPLOY_STYLE_DEPLOY_AUDIO,
        "SentryDroneDeploy",
    );
    set_test_per_unit_sound(
        "AmericaVehicleSentryDrone",
        DEPLOY_STYLE_UNDEPLOY_AUDIO,
        "SentryDroneUndeploy",
    );
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaVehicleSentryDrone");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    tpl.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        ..Default::default()
    });
    game_logic
        .templates
        .insert("AmericaVehicleSentryDrone".into(), tpl);
    let id = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");

    assert!(!game_logic.ensure_deploy_style_ready_to_fire(id));
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "SentryDroneDeploy" && e.object_id == Some(id) }),
        "Deploy must play the authored per-unit event: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != DEPLOY_STYLE_DEPLOY_AUDIO),
        "must not queue the Deploy slot token: {:?}",
        game_logic.queued_audio_events
    );

    for f in 1..=30 {
        game_logic.frame = f;
        game_logic.tick_deploy_style_updates();
    }
    assert!(game_logic.ensure_deploy_style_ready_to_fire(id));
    game_logic.queued_audio_events.clear();
    assert!(!game_logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "SentryDroneUndeploy" && e.object_id == Some(id) }),
        "Undeploy must play the authored per-unit event: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != DEPLOY_STYLE_UNDEPLOY_AUDIO),
        "must not queue the Undeploy slot token: {:?}",
        game_logic.queued_audio_events
    );
    clear_test_template_voices();
}

#[test]
fn deploy_style_missing_unit_sound_stays_silent() {
    use crate::game_logic::audio_dispatch_impl::clear_test_template_voices;
    use crate::game_logic::host_deploy_style::{
        DEPLOY_STYLE_DEPLOY_AUDIO, DEPLOY_STYLE_UNDEPLOY_AUDIO,
    };

    clear_test_template_voices();
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SilentDeployer");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    tpl.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 10,
        unpack_time_frames: 10,
        ..Default::default()
    });
    game_logic.templates.insert("SilentDeployer".into(), tpl);
    let id = game_logic
        .create_object("SilentDeployer", Team::USA, Vec3::ZERO)
        .expect("deployer");
    assert!(!game_logic.ensure_deploy_style_ready_to_fire(id));
    assert!(
        game_logic.queued_audio_events.iter().all(|e| {
            e.event_type != DEPLOY_STYLE_DEPLOY_AUDIO && e.event_type != "SilentDeployerDeploy"
        }),
        "missing UnitSpecificSounds.Deploy must stay silent: {:?}",
        game_logic.queued_audio_events
    );
    let _ = DEPLOY_STYLE_UNDEPLOY_AUDIO;
}

#[test]
fn deploy_style_nuke_launcher_normal_attack_waits_for_range_and_unpack() {
    use crate::game_logic::host_deploy_style::HostDeployStyleState;

    let mut logic = GameLogic::new();
    let mut launcher = ThingTemplate::new("ChinaVehicleNukeLauncher");
    launcher
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon(Weapon {
            damage: 30.0,
            range: 100.0,
            min_range: 0.0,
            // Start reloading: C++ DeployStyleAIUpdate checks current-weapon
            // range, not weapon readiness, before it begins unpacking.
            reload_time: 100.0,
            last_fire_time: 0.0,
            last_fire_frame: 0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            // A finite speed above the 50-unit in-range shot distance gives
            // C++'s projectileless path a sub-frame delay, so it damages on
            // the firing frame without relying on the undefined 0/0 case.
            projectile_speed: 200.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
            next_fire_frame: 0,
            reload_started_frame: 0,
            frame_clock_active: false,
            frame_clock_status: Default::default(),
        });
    // Retail ChinaVehicleNukeLauncher has 3333ms Pack/Unpack, parsed with
    // C++ duration rounding into 100 logic frames. TurretsMustCenterBeforePacking
    // is live: READY_TO_ATTACK + move waits ALIGNING_TURRETS until natural.
    launcher.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 100,
        unpack_time_frames: 100,
        turrets_function_only_when_deployed: true,
        turrets_must_center_before_packing: true,
        manual_deploy_animations: true,
        ..Default::default()
    });
    logic
        .templates
        .insert("ChinaVehicleNukeLauncher".to_string(), launcher);

    let mut target_template = ThingTemplate::new("DeployStyleTarget");
    target_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic
        .templates
        .insert("DeployStyleTarget".to_string(), target_template);

    let launcher_id = logic
        .create_object(
            "ChinaVehicleNukeLauncher",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("nuke launcher");
    let target_id = logic
        .create_object("DeployStyleTarget", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("out-of-range target");
    let hp_before = logic.host_object(target_id).unwrap().health.current;
    let object_ids = [launcher_id, target_id];
    let mut last_ticked_frame = 0;
    let tick_through_frame = |logic: &mut GameLogic, last_frame: &mut u32, target_frame: u32| {
        // Preserve every logic-frame update like C++ does. Skipping frames
        // also skips AIM/FIRE transitions and turret alignment, so only
        // advancing the DeployStyle timer would not be a valid comparison.
        for frame in (*last_frame + 1)..=target_frame {
            logic.set_current_frame(frame.into());
            logic.tick_deploy_style_updates();
            // Mirror Phase 7: legacy combat first, then the nested machine
            // that owns a normal AttackObject command.
            logic.update_combat(&object_ids, LOGIC_FRAME_TIMESTEP);
            logic.tick_nested_attack_machines(
                &object_ids,
                frame as f32 * LOGIC_FRAME_TIMESTEP,
                frame,
            );
        }
        *last_frame = target_frame;
    };

    // A normal player AttackObject remains accepted and approaches; it must
    // not begin the DeployStyle timer merely because the target is distant.
    assert!(
        logic
            .unit_command_attack(launcher_id, target_id)
            .expect("valid attack command fixture")
    );
    tick_through_frame(&mut logic, &mut last_ticked_frame, 1);
    let launcher_after_oor = logic.host_object(launcher_id).unwrap();
    assert_eq!(launcher_after_oor.target, Some(target_id));
    assert!(matches!(
        launcher_after_oor
            .deploy_style
            .as_ref()
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::ReadyToMove)
    ));
    assert_eq!(
        logic.deploy_style_reg.deploys, 0,
        "an out-of-range attack must preserve the approach instead of unpacking"
    );

    // Once the same pending attack is actually in range, C++ DeployStyle
    // begins its authored timer and blocks damage through the final frame.
    logic.set_fixture_object_position(target_id, Vec3::new(50.0, 0.0, 0.0));
    tick_through_frame(&mut logic, &mut last_ticked_frame, 2);
    assert!(matches!(
        logic
            .host_object(launcher_id)
            .and_then(|object| object.deploy_style.as_ref())
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::Deploying)
    ));
    assert_eq!(
        logic.host_object(target_id).unwrap().health.current,
        hp_before,
        "the normal attack cannot fire while the launcher is unpacking"
    );

    // The deploy timer was allowed to begin while the weapon was reloading;
    // make the shot ready now so the remainder isolates the exact timer edge.
    if let Some(weapon) = logic
        .host_object_mut(launcher_id)
        .and_then(|launcher| launcher.weapon.as_mut())
    {
        weapon.reload_time = 0.0;
        weapon.last_fire_time = -100.0;
    }

    tick_through_frame(&mut logic, &mut last_ticked_frame, 101);
    assert!(
        !logic.attack_can_fire_at(launcher_id, target_id, 101.0 * LOGIC_FRAME_TIMESTEP, false,),
        "every fire authority must reject a packed DeployStyle weapon"
    );
    assert_eq!(
        logic.host_object(target_id).unwrap().health.current,
        hp_before,
        "retail 100-frame unpack still blocks one frame before completion"
    );

    tick_through_frame(&mut logic, &mut last_ticked_frame, 102);
    assert!(
        logic.host_object(launcher_id).unwrap().is_deployed(),
        "the exact timer boundary enters ReadyToAttack"
    );
    assert_eq!(
        logic
            .host_object(launcher_id)
            .and_then(|launcher| launcher.weapon.as_ref())
            .map(|weapon| weapon.last_fire_time),
        Some(102.0 * LOGIC_FRAME_TIMESTEP),
        "the retained attack discharges on the first frame ReadyToAttack"
    );
    // The complete GameLogic update resolves accepted shots after the nested
    // attack machine in its projectile phase. Reproduce those exact Phase 9
    // calls rather than expecting projectile damage during weapon acceptance.
    // This synthetic weapon has no ProjectileObject and its finite-speed shot
    // crosses the remaining range in less than one logic frame, so it follows
    // the projectileless delayed-damage path rather than becoming a
    // CombatSystem projectile. Production applies ready damage in this phase
    // after draining the fire queue.

    crate::game_logic::combat::drain_pending_projectiles(
        &mut logic.combat_system,
        logic.objects.read(),
        102,
        &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
    );
    crate::game_logic::combat::apply_ready_projectileless_delayed_damage(
        &mut logic.combat_system,
        &mut logic.objects,
        102,
        Some(&logic.players),
        &mut logic.health_events,
        &mut logic.ownership_events,
    );
    logic.combat_system.update_projectiles_with_relationships(
        LOGIC_FRAME_TIMESTEP,
        &mut logic.objects,
        Some(&mut logic.countermeasures),
        102,
        Some(&logic.players),
        Some(&logic.team_factory),
        &mut logic.health_events,
        &mut logic.ownership_events,
    );
    assert!(
        logic.host_object(target_id).unwrap().health.current < hp_before,
        "the accepted shot damages its target during the production projectile phase"
    );
}

#[test]
fn deploy_style_sentry_auto_target_loss_clears_pending_attack() {
    use crate::game_logic::host_deploy_style::HostDeployStyleState;

    let mut logic = GameLogic::new();
    let mut sentry = ThingTemplate::new("AmericaVehicleSentryDrone");
    sentry
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    sentry.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        turrets_function_only_when_deployed: true,
        turrets_must_center_before_packing: true,
        ..Default::default()
    });
    logic
        .templates
        .insert("AmericaVehicleSentryDrone".to_string(), sentry);
    let mut target_template = ThingTemplate::new("DeployStyleAutoTarget");
    target_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("DeployStyleAutoTarget".to_string(), target_template);

    let sentry_id = logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    let target_id = logic
        .create_object(
            "DeployStyleAutoTarget",
            Team::GLA,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("auto target");
    {
        let sentry = logic.host_object_mut(sentry_id).unwrap();
        crate::fixture_content::arm_with_stats(
            sentry,
            crate::game_logic::Weapon {
                damage: 10.0,
                range: 100.0,
                min_range: 0.0,
                reload_time: 0.0,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: false,
                can_target_ground: true,
                projectile_speed: 0.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
                ..Default::default()
            },
        );
        sentry.weapon.as_mut().unwrap().last_fire_time = -100.0;
    }

    logic.set_current_frame(1);
    logic.update_combat(&[sentry_id, target_id], LOGIC_FRAME_TIMESTEP);
    let sentry_after_acquire = logic.host_object(sentry_id).unwrap();
    assert_eq!(sentry_after_acquire.target, Some(target_id));
    assert!(matches!(
        sentry_after_acquire
            .deploy_style
            .as_ref()
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::Deploying)
    ));

    // The acquired enemy can disappear while the unit is still packing. The
    // next normal combat pass owns invalid-target cleanup; it must not leave a
    // stale target that fires when ReadyToAttack is eventually reached.
    assert!(logic.objects.remove(&target_id).is_some());
    logic.set_current_frame(2);
    logic.tick_deploy_style_updates();
    let sentry_after_loss = logic.host_object(sentry_id).unwrap();
    assert!(sentry_after_loss.target.is_none());
    assert!(!sentry_after_loss.status.attacking);
    assert_eq!(sentry_after_loss.ai_state(), AIState::Idle);
    assert!(matches!(
        sentry_after_loss
            .deploy_style
            .as_ref()
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::Deploying)
    ));
}

#[test]
fn deploy_style_must_center_turret_before_pack() {
    use crate::game_logic::host_deploy_style::HostDeployStyleState;

    let mut logic = GameLogic::new();
    let mut sentry = ThingTemplate::new("AmericaVehicleSentryDrone");
    sentry
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0);
    sentry.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        turrets_must_center_before_packing: true,
        ..Default::default()
    });
    logic
        .templates
        .insert("AmericaVehicleSentryDrone".to_string(), sentry);

    let id = logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    {
        let obj = logic.host_object_mut(id).unwrap();
        obj.turret_ai.turret_enabled = true;
        obj.turret_ai.turret_turn_rate_rad = 0.1;
        obj.turret_ai.turret_angle_deg = 45.0;
        obj.turret_ai.turret_natural_angle_deg = 0.0;
        obj.turret_ai.turret_pitch_deg = 0.0;
        obj.turret_ai.turret_natural_pitch_deg = 0.0;
    }

    let ai_entry_frame = logic.frame;
    logic.host_object_mut(id).unwrap().set_ai_state(
        AIState::Moving,
        ai_entry_frame,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    logic.set_current_frame(0);
    assert!(
        logic
            .unit_command_toggle_deploy_style(id)
            .expect("checked deploy-style transition")
    );
    {
        let entered = logic.host_object(id).unwrap();
        assert_eq!(entered.ai_state(), AIState::Idle);
        assert!(!entered.idle_mood_reset_pending);
    }
    logic.set_current_frame(30);
    logic.tick_deploy_style_updates();
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::ReadyToAttack)
        ),
        "unpack must finish before the pack-align path"
    );
    assert!(logic.host_object(id).unwrap().is_deployed());

    {
        let obj = logic.host_object_mut(id).unwrap();
        obj.turret_ai.turret_angle_deg = 45.0;
    }
    logic.set_current_frame(31);
    assert!(
        logic
            .unit_command_toggle_deploy_style(id)
            .expect("checked deploy-style transition")
    );
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::AligningTurrets)
        ),
        "TurretsMustCenterBeforePacking must enter ALIGNING_TURRETS"
    );
    assert!(
        logic.host_object(id).unwrap().is_deployed(),
        "ALIGNING stays DEPLOYED until UNDEPLOY"
    );

    logic.set_current_frame(32);
    logic.tick_deploy_style_updates();
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::AligningTurrets)
        ),
        "off-natural turret must not pack"
    );

    {
        let obj = logic.host_object_mut(id).unwrap();
        obj.turret_ai.turret_angle_deg = 0.0;
    }
    logic.set_current_frame(33);
    logic.tick_deploy_style_updates();
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::Undeploying)
        ),
        "isTurretInNaturalPosition must start UNDEPLOY"
    );
    assert!(
        !logic.host_object(id).unwrap().is_deployed(),
        "C++ setMyState(UNDEPLOY) clears OBJECT_STATUS_DEPLOYED immediately"
    );
}

#[test]
fn jet_out_of_ammo_paths_to_distant_airfield_then_rearms() {
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);

    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(2000.0, 0.0, 40.0))
        .expect("jet");

    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        crate::fixture_content::arm_with_stats(
            jet,
            crate::game_logic::Weapon {
                damage: 50.0,
                range: 200.0,
                reload_time: 0.0,
                clip_size: 4,
                can_target_air: true,
                can_target_ground: true,
                ..Default::default()
            },
        );
        jet.weapon.as_mut().unwrap().last_fire_time = -100.0;
        jet.weapon.as_mut().unwrap().ammo = Some(0);
        jet.status.airborne_target = true;
    }
    assert!(
        logic
            .objects
            .get(&jet_id)
            .unwrap()
            .needs_return_to_base_rearm()
    );

    // Distant: path toward airfield (not docked yet).
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_ne!(jet.contained_by, Some(af_id), "still en route");
        assert!(
            jet.movement.target_position.is_some()
                || !jet.movement.path.is_empty()
                || matches!(jet.ai_state(), AIState::Moving),
            "should path toward airfield"
        );
        // Ammo still empty until dock.
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
    }

    // C++ lands on the runway then taxis; dock only once grounded at the pad.
    {
        let (jet, fixture_partition_jet) = logic.host_object_and_partition_mut(jet_id).unwrap();
        jet.set_position_with_partition_owner(
            Vec3::new(0.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_jet,
        );
        jet.status.airborne_target = false;
        jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
        if let Some(w) = jet.weapon.as_mut() {
            w.ammo = Some(0);
        }
    }
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.contained_by, Some(af_id));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
        assert!(jet.needs_return_to_base_rearm());
    }
    logic.frame = logic.frame.saturating_add(1);
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(4));
        assert!(!jet.needs_return_to_base_rearm());
    }
}

#[test]
fn jet_airfield_rearm_waits_clip_reload_frames() {
    use crate::game_logic::host_raptor::{RAPTOR_CLIP_RELOAD_FRAMES, RAPTOR_CLIP_SIZE};
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(40.0, 40.0, 0.0))
        .unwrap();
    {
        let (jet, fixture_partition_jet) = logic.host_object_and_partition_mut(jet_id).unwrap();
        crate::fixture_content::arm_with_stats(
            jet,
            crate::game_logic::Weapon {
                damage: 50.0,
                range: 200.0,
                reload_time: 0.0,
                clip_size: RAPTOR_CLIP_SIZE,
                // Retail ClipReload 8000ms → 240 frames @ 30 FPS.
            clip_reload_time: (RAPTOR_CLIP_RELOAD_FRAMES as f32) / 30.0,
                can_target_air: true,
                can_target_ground: true,
                ..Default::default()
            },
        );
        jet.weapon.as_mut().unwrap().last_fire_time = -100.0;
        jet.weapon.as_mut().unwrap().ammo = Some(0);
        jet.status.airborne_target = false;
        jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
        jet.set_position_with_partition_owner(
            Vec3::ZERO,
            0,
            &mut Default::default(),
            fixture_partition_jet,
        );
    }

    logic.frame = 10;
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.contained_by, Some(af_id), "must dock immediately");
        assert_eq!(
            jet.weapon.as_ref().unwrap().ammo,
            Some(0),
            "ammo still empty during ClipReload"
        );
        assert_eq!(
            jet.airfield_rearm_ready_frame,
            Some(10 + RAPTOR_CLIP_RELOAD_FRAMES)
        );
        assert!(jet.needs_return_to_base_rearm());
    }

    // Mid-reload: C++ setClipPercentFull((reloadTime-(done-now))/reloadTime).
    logic.frame = 10 + RAPTOR_CLIP_RELOAD_FRAMES - 1;
    assert!(logic.try_return_to_base_rearm(jet_id));
    assert_eq!(
        logic
            .objects
            .get(&jet_id)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(3)
    );

    // ClipReload elapsed → full rearm.
    logic.frame = 10 + RAPTOR_CLIP_RELOAD_FRAMES;
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(RAPTOR_CLIP_SIZE));
        assert!(jet.airfield_rearm_ready_frame.is_none());
        assert!(!jet.needs_return_to_base_rearm());
    }
}

#[test]
fn empty_jet_circles_last_airfield_instead_of_bleeding_in_place() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    clear_test_template_voices();
    // C++ JetOrHeliCirclingDeadAirfieldState::onEnter plays
    // getPerUnitSound("VoiceLowFuel") — retail RaptorUnit authors
    // VoiceLowFuel = RaptorVoiceLowFuel; the resolver is override-or-factory.
    set_test_per_unit_sound("AmericaJetRaptor", "VoiceLowFuel", "RaptorVoiceLowFuel");
    let mut logic = GameLogic::new();
    // C++ airfields always author ParkingPlaceBehavior; JetAI RTB reservation
    // additionally needs the exact-controller owner pair (airfield.rs:1262).
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .expect("af");
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(2000.0, 50.0, 0.0))
        .expect("jet");
    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.producer_id = Some(af_id);
        jet.status.airborne_target = true;
        crate::fixture_content::arm_with_stats(
            jet,
            crate::game_logic::Weapon {
                damage: 50.0,
                range: 200.0,
                reload_time: 0.0,
                clip_size: 4,
                can_target_air: true,
                can_target_ground: true,
                ..Default::default()
            },
        );
        jet.weapon.as_mut().unwrap().last_fire_time = -100.0;
        jet.weapon.as_mut().unwrap().ammo = Some(0);
    }

    logic
        .tick_out_of_ammo_jet_damage()
        .expect("receiving AI operation succeeds");
    let hp_after_first = logic.objects.get(&jet_id).unwrap().health.current;
    assert!(
        (hp_after_first - 100.0).abs() < 1e-3,
        "must not bleed while a live airfield exists or while flying home"
    );

    logic.destroy_object(af_id).unwrap();
    if let Some(af) = logic.objects.get_mut(&af_id) {
        af.status.destroyed = true;
        af.health.current = 0.0;
    }
    let hp_before = logic.objects.get(&jet_id).unwrap().health.current;
    logic
        .tick_out_of_ammo_jet_damage()
        .expect("receiving AI operation succeeds");
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert!(
            (jet.health.current - hp_before).abs() < 1e-3,
            "must not bleed in place after airfield dies"
        );
        assert!(
            !jet.jet_circling_dead_airfield(),
            "still returning to last airfield"
        );
        let goal = jet.jet_producer_location_vec().expect("remembered wreck");
        assert!(goal.x.abs() < 1.0 && goal.z.abs() < 1.0);
    }

    {
        let (jet, fixture_partition_jet) = logic.host_object_and_partition_mut(jet_id).unwrap();
        jet.set_position_with_partition_owner(
            Vec3::new(5.0, 50.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_jet,
        );
    }
    logic.queued_audio_events.clear();
    logic
        .tick_out_of_ammo_jet_damage()
        .expect("receiving AI operation succeeds");
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert!(jet.jet_circling_dead_airfield());
        assert!(
            jet.health.current < hp_before - 1e-4,
            "OutOfAmmoDamage only after circling the wreck"
        );
    }
    let events: Vec<&str> = logic
        .queued_audio_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();
    assert_eq!(
        events,
        vec!["RaptorVoiceLowFuel"],
        "circling enter plays authored PerUnitSound VoiceLowFuel at the jet"
    );
    assert!(
        !events
            .iter()
            .any(|e| *e == "VoiceLowFuel" || *e == "AmericaJetRaptorVoiceLowFuel"),
        "must not queue slot token or invented concat"
    );
    clear_test_template_voices();
}

#[test]
fn command_button_hunt_hijack_issues_nearest_enemy_vehicle() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let hijacker = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hijacker");
    {
        let ai_entry_frame = logic.frame;
        let h = logic.host_object_mut(hijacker).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let near = logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("near");
    let far = logic
        .create_object("TestTank", Team::USA, Vec3::new(400.0, 0.0, 0.0))
        .expect("far");
    let _ = far;

    assert!(
        logic
            .start_command_button_hunt(hijacker, HostCommandButtonHuntMode::HijackVehicle)
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();

    assert!(logic.honesty_command_button_hunt_ok());
    assert!(
        logic.pending_special_abilities.get(&hijacker).is_some(),
        "should queue Hijack on nearest tank"
    );
    match logic.pending_special_abilities.get(&hijacker) {
        Some(PendingSpecialAbility::Hijack { target_id }) => {
            assert_eq!(*target_id, near);
        }
        other => panic!("expected Hijack, got {other:?}"),
    }
}

#[test]
fn command_button_hunt_named_arms_capture_and_flashbang() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let ranger = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("ranger");
    {
        let ai_entry_frame = logic.frame;
        let r = logic.host_object_mut(ranger).unwrap();
        r.template_name = "AmericaInfantryRanger".into();
        r.secondary_weapon = Some(Weapon {
            range: 100.0,
            damage: 5.0,
            ..Weapon::default()
        });
        r.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    assert!(
        logic
            .start_command_button_hunt_named(ranger, Some("Command_AmericaRangerFlashBangGrenade"))
            .unwrap()
    );
    {
        let r = logic.host_object(ranger).unwrap();
        assert_eq!(
            r.command_button_hunt.as_ref().map(|h| h.mode),
            Some(HostCommandButtonHuntMode::FireWeapon)
        );
        assert_eq!(
            r.command_button_hunt.as_ref().map(|h| h.weapon_slot),
            Some(1)
        );
    }
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    let r = logic.host_object(ranger).unwrap();
    assert_eq!(r.ai_state(), AIState::Patrolling);
    assert_eq!(r.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(r.weapon_lock_slot, 1);

    let lotus = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("lotus");
    {
        let ai_entry_frame = logic.frame;
        let l = logic.host_object_mut(lotus).unwrap();
        l.template_name = "AmericaInfantryColonelBurton".into();
        l.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    assert!(
        logic
            .start_command_button_hunt_named(lotus, Some("Command_CaptureBuilding"))
            .unwrap(),
        "capture hunt must arm"
    );
}

#[test]
fn fire_weapon_command_button_hunt_rearms_lock_every_frame() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{Team, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let ranger = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("ranger");
    {
        let ai_entry_frame = logic.frame;
        let r = logic.host_object_mut(ranger).unwrap();
        r.template_name = "AmericaInfantryRanger".into();
        r.secondary_weapon = Some(Weapon {
            range: 100.0,
            damage: 5.0,
            ..Weapon::default()
        });
        r.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    assert!(
        logic
            .start_command_button_hunt_named(ranger, Some("Command_AmericaRangerFlashBangGrenade"))
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    {
        let r = logic.host_object(ranger).unwrap();
        assert_eq!(r.ai_state(), AIState::Patrolling);
        assert_eq!(r.weapon_lock_type, WeaponLockType::LockedTemporarily);
        assert_eq!(r.weapon_lock_slot, 1);
    }
    // C++ Object::fireCurrentWeapon releases temp lock when the clip auto-reloads.
    logic
        .host_object_mut(ranger)
        .unwrap()
        .release_weapon_lock(WeaponLockType::LockedTemporarily);
    assert_eq!(
        logic.host_object(ranger).unwrap().weapon_lock_type,
        WeaponLockType::NotLocked
    );
    // Old live scheduled +30 for FireWeapon, so frame 1 would not re-arm.
    logic.frame = 1;
    logic.tick_command_button_hunt_updates();
    let r = logic.host_object(ranger).unwrap();
    assert_eq!(
        r.command_button_hunt.as_ref().map(|h| h.mode),
        Some(HostCommandButtonHuntMode::FireWeapon)
    );
    assert_eq!(
        r.weapon_lock_type,
        WeaponLockType::LockedTemporarily,
        "FireWeapon hunt must re-arm LOCKED_TEMPORARILY the next frame"
    );
    assert_eq!(r.weapon_lock_slot, 1);
}

#[test]
fn command_button_hunt_quits_after_player_move() {
    use crate::game_logic::Team;
    use crate::game_logic::host_command_button_hunt::{
        HUNT_CMD_FROM_PLAYER, HostCommandButtonHuntMode,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);
    let hijacker = logic
        .create_object("TestInfantry", Team::GLA, Vec3::ZERO)
        .expect("hijacker");
    {
        let ai_entry_frame = logic.frame;
        let h = logic.host_object_mut(hijacker).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let _tank = logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("tank");
    assert!(
        logic
            .start_command_button_hunt(hijacker, HostCommandButtonHuntMode::HijackVehicle)
            .unwrap()
    );
    assert!(logic.unit_command_move_to(hijacker, Vec3::new(10.0, 0.0, 0.0)));
    assert_eq!(
        logic.host_object(hijacker).unwrap().last_command_source,
        HUNT_CMD_FROM_PLAYER
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    let h = logic.host_object(hijacker).unwrap();
    assert!(
        h.command_button_hunt.is_none(),
        "player move must permanently end CommandButtonHunt"
    );
    assert!(logic.pending_special_abilities.get(&hijacker).is_none());
}

#[test]
fn command_button_hunt_enter_skips_stealth_ally_and_drone() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    let mut china = Player::new(1, Team::China, "China", false);
    let mut gla = Player::new(2, Team::GLA, "GLA", false);
    usa.alliance_team = 7;
    china.alliance_team = 7;
    gla.alliance_team = 9;
    logic.add_player(usa);
    logic.add_player(china);
    logic.add_player(gla);
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);
    let mut drone = ThingTemplate::new("TestDrone");
    drone
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Drone)
        .set_health(50.0);
    logic.templates.insert("TestDrone".into(), drone);

    let hijacker = logic
        .create_object("TestInfantry", Team::GLA, Vec3::ZERO)
        .expect("hijacker");
    {
        let ai_entry_frame = logic.frame;
        let h = logic.host_object_mut(hijacker).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.owner_player_id = Some(2);
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let stealth = logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("stealth");
    {
        let t = logic.host_object_mut(stealth).unwrap();
        t.owner_player_id = Some(0);
        t.status.stealthed = true;
        t.status.detected = false;
    }
    let ally = logic
        .create_object("TestTank", Team::China, Vec3::new(25.0, 0.0, 0.0))
        .expect("ally");
    logic.host_object_mut(ally).unwrap().owner_player_id = Some(1);
    // China is allied with USA, not GLA — this is a GLA hijacker vs China tank
    // so China is an enemy. Use a same-alliance tank by making the hunter USA.
    let usa_hunter = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("usa hunter");
    {
        let ai_entry_frame = logic.frame;
        let h = logic.host_object_mut(usa_hunter).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.owner_player_id = Some(0);
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let drone_id = logic
        .create_object("TestDrone", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("drone");
    logic.host_object_mut(drone_id).unwrap().owner_player_id = Some(2);
    let legal = logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("legal");
    logic.host_object_mut(legal).unwrap().owner_player_id = Some(2);

    assert!(
        logic
            .start_command_button_hunt(usa_hunter, HostCommandButtonHuntMode::HijackVehicle)
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&usa_hunter) {
        Some(PendingSpecialAbility::Hijack { target_id }) => {
            assert_eq!(*target_id, legal, "must skip stealth, 2v2 ally, and drone");
        }
        other => panic!("expected Hijack of legal enemy, got {other:?}"),
    }
}

#[test]
fn command_button_hunt_special_skips_ally_mine_and_uses_priority() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{AttackPriorityInfo, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    let mut china = Player::new(1, Team::China, "China", false);
    let mut gla = Player::new(2, Team::GLA, "GLA", false);
    usa.alliance_team = 7;
    china.alliance_team = 7;
    gla.alliance_team = 9;
    logic.add_player(usa);
    logic.add_player(china);
    logic.add_player(gla);
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);
    let mut bldg = ThingTemplate::new("TestBuilding");
    bldg.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("TestBuilding".into(), bldg);
    let mut mine_t = ThingTemplate::new("TestMine");
    mine_t.add_kind_of(KindOf::Mine).set_health(10.0);
    logic.templates.insert("TestMine".into(), mine_t);

    // Capture: ally building closer than enemy — must pick enemy.
    let lotus = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("lotus");
    {
        let ai_entry_frame = logic.frame;
        let l = logic.host_object_mut(lotus).unwrap();
        l.template_name = "AmericaInfantryColonelBurton".into();
        l.owner_player_id = Some(0);
        l.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let ally_b = logic
        .create_object("TestBuilding", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("ally bldg");
    logic.host_object_mut(ally_b).unwrap().owner_player_id = Some(1);
    let enemy_b = logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(90.0, 0.0, 0.0))
        .expect("enemy bldg");
    logic.host_object_mut(enemy_b).unwrap().owner_player_id = Some(2);
    assert!(
        logic
            .start_command_button_hunt_named(lotus, Some("Command_CaptureBuilding"))
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    let lotus_obj = logic.host_object(lotus).unwrap();
    assert_eq!(
        lotus_obj.target,
        Some(enemy_b),
        "capture must skip 2v2 ally"
    );

    // TNT: owned mine next to nearer tank → skip it, pick farther unmined tank.
    // Stay inside default GameLogic extent (-256..256) so same-map passes.
    let hunter = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(-180.0, 0.0, 0.0))
        .expect("th");
    {
        let ai_entry_frame = logic.frame;
        let h = logic.host_object_mut(hunter).unwrap();
        h.template_name = "ChinaInfantryTankHunter".into();
        h.owner_player_id = Some(0);
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let mined = logic
        .create_object("TestTank", Team::GLA, Vec3::new(-150.0, 0.0, 0.0))
        .expect("mined");
    logic.host_object_mut(mined).unwrap().owner_player_id = Some(2);
    let mine = logic
        .create_object("TestMine", Team::USA, Vec3::new(-150.0, 0.0, 0.0))
        .expect("mine");
    logic.host_object_mut(mine).unwrap().owner_player_id = Some(0);
    let clean = logic
        .create_object("TestTank", Team::GLA, Vec3::new(-80.0, 0.0, 0.0))
        .expect("clean");
    logic.host_object_mut(clean).unwrap().owner_player_id = Some(2);
    assert!(
        logic
            .start_command_button_hunt_named(hunter, Some("Command_ChinaTankHunterTNT"))
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&hunter) {
        Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }) => {
            assert_eq!(*target_id, clean, "TNT hunt must skip already-mined target");
        }
        other => panic!("expected TNT on clean tank, got {other:?}"),
    }

    // Priority: farther Dozer outranks nearer Tank (on-map).
    let mut info = AttackPriorityInfo::new("HuntPrio");
    info.default_priority = 1;
    info.set_priority_template("TestTank", 5);
    info.set_priority_template("TestDozer", 80);
    logic.register_attack_priority_set(info);
    ensure_test_dozer_template(&mut logic);
    let prio_hunter = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(120.0, 0.0, 0.0))
        .expect("prio");
    {
        let ai_entry_frame = logic.frame;
        let h = logic.host_object_mut(prio_hunter).unwrap();
        h.template_name = "ChinaInfantryTankHunter".into();
        h.owner_player_id = Some(0);
        h.attack_priority_set = Some("HuntPrio".into());
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let near_tank = logic
        .create_object("TestTank", Team::GLA, Vec3::new(140.0, 0.0, 0.0))
        .expect("near tank");
    logic.host_object_mut(near_tank).unwrap().owner_player_id = Some(2);
    let far_dozer = logic
        .create_object("TestDozer", Team::GLA, Vec3::new(220.0, 0.0, 0.0))
        .expect("far dozer");
    logic.host_object_mut(far_dozer).unwrap().owner_player_id = Some(2);
    assert!(
        logic
            .start_command_button_hunt_named(prio_hunter, Some("Command_ChinaTankHunterTNT"))
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&prio_hunter) {
        Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }) => {
            assert_eq!(*target_id, far_dozer, "priority must beat raw nearest");
        }
        other => panic!("expected TNT on high-priority dozer, got {other:?}"),
    }
    let _ = HostCommandButtonHuntMode::SpecialPower;
}

#[test]
fn command_button_hunt_script_drain_requires_module_not_mobile() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let hijacker = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("hijacker");
    {
        let u = logic.host_object_mut(hijacker).unwrap();
        u.template_name = "GLAInfantryHijacker".into();
        u.status.disabled_held = true;
        assert!(!u.can_move(), "HELD hijacker cannot walk");
    }
    assert!(
        logic.unit_can_team_hunt_with_command_button(hijacker, Some("Command_HijackVehicle")),
        "C++ doTeamHuntWithCommandButton has no is_mobile gate"
    );

    let crusader = logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("crusader");
    {
        let u = logic.host_object_mut(crusader).unwrap();
        u.template_name = "AmericaTankCrusader".into();
    }
    assert!(
        !logic.unit_can_team_hunt_with_command_button(crusader, Some("Command_ChinaTankHunterTNT")),
        "units without CommandButtonHuntUpdate must not arm"
    );

    let mut mine_t = ThingTemplate::new("TestHuntMine");
    mine_t.add_kind_of(KindOf::Mine).set_health(10.0);
    logic.templates.insert("TestHuntMine".into(), mine_t);
    let mine = logic
        .create_object("TestHuntMine", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("mine");
    assert!(
        !logic.unit_can_team_hunt_with_command_button(mine, Some("Command_HijackVehicle")),
        "no-AI members must skip"
    );
}

#[test]
fn command_button_hunt_tnt_and_booby_reject_neutral() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    // C++ players are always live controlling players at scan time; the host
    // ownership resolution filters dead players out (get_player().filter
    // is_alive), which would turn every relationship Neutral and reject both
    // scan candidates.
    // Distinct alliance teams model skirmish diplomacy: no shared
    // alliance_team between live controlling players resolves Enemies
    // (player_relationship_from_map, object_queries.rs:445-451).
    let mut china = Player::new(0, Team::China, "China", true);
    china.alliance_team = 0;
    let mut gla = Player::new(1, Team::GLA, "GLA", true);
    gla.alliance_team = 1;
    logic.add_player(china);
    logic.add_player(gla);
    ensure_test_infantry_template(&mut logic);
    let mut bldg = ThingTemplate::new("TestBuilding");
    bldg.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("TestBuilding".into(), bldg);

    let hunter = logic
        .create_object("TestInfantry", Team::China, Vec3::ZERO)
        .expect("th");
    {
        let ai_entry_frame = logic.frame;
        let h = logic.host_object_mut(hunter).unwrap();
        h.template_name = "ChinaInfantryTankHunter".into();
        h.owner_player_id = Some(0);
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let civilian = logic
        .create_object("TestBuilding", Team::Neutral, Vec3::new(30.0, 0.0, 0.0))
        .expect("civ");
    logic.host_object_mut(civilian).unwrap().owner_player_id = None;
    let enemy = logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(90.0, 0.0, 0.0))
        .expect("enemy");
    logic.host_object_mut(enemy).unwrap().owner_player_id = Some(1);

    assert!(
        logic
            .start_command_button_hunt_named(hunter, Some("Command_ChinaTankHunterTNT"))
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&hunter) {
        Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }) => {
            assert_eq!(
                *target_id, enemy,
                "TNT hunt must ignore Neutral civilian buildings"
            );
        }
        other => panic!("expected TNT on enemy, got {other:?}"),
    }

    logic.pending_special_abilities.remove(&hunter);
    let ai_entry_frame = logic.frame;
    if let Some(h) = logic.host_object_mut(hunter) {
        h.clear_command_button_hunt();
        h.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        h.target = None;
    }
    assert!(
        logic
            .start_command_button_hunt_named(hunter, Some("Command_BoobyTrapBuilding"))
            .unwrap()
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&hunter) {
        Some(PendingSpecialAbility::PlantBoobyTrap { target_id }) => {
            assert_eq!(*target_id, enemy, "booby hunt must ignore Neutral");
        }
        other => panic!("expected booby on enemy, got {other:?}"),
    }
}
