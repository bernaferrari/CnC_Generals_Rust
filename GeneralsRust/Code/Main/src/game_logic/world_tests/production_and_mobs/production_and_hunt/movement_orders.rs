//! Movement order transitions and obstacle ownership fixtures.
use super::*;

#[test]
fn repair_action_after_filling_the_vehicle_leaves_for_the_rally() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("FillPad");
    pad_t.repair_dock_frames_for_full_heal = Some(1.0);
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("FillPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("FillTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("FillTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("FillPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("FillTank", 0, Vec3::ZERO)
        .expect("tank");
    let rally = Vec3::new(90.0, 0.0, 0.0);
    {
        let building = logic.host_object_mut(pad).expect("pad");
        let mut data = BuildingData::new(BuildingType::RepairPad);
        data.rally_point = Some(rally);
        building.building_data = Some(data);
        building.dock_active_docker = Some(tank);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let (unit, attack_events) = logic.host_object_and_attack_events_mut(tank).expect("tank");
        unit.set_order_target(
            Some(pad),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::SeekingRepair,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.health.current = unit.health.maximum - 1.0;
    }
    // An authored one-frame RepairDock heals once regardless of display dt.
    // CPP action returns TRUE on this pulse; its next action observes full HP.
    logic.update_support_states_for_test(&[tank], 30.0);
    {
        let unit = logic.host_object(tank).expect("tank");
        assert_eq!(unit.health.current, unit.health.maximum);
        assert_eq!(unit.ai_state(), AIState::SeekingRepair);
        assert_eq!(unit.target, Some(pad));
        assert_eq!(
            logic.host_object(pad).unwrap().dock_active_docker,
            Some(tank)
        );
    }
    logic.update_support_states_for_test(&[tank], 0.0);
    let unit = logic.host_object(tank).expect("tank");
    assert!(
        unit.health.current >= unit.health.maximum - 0.01,
        "the tick must finish the heal, hp={}",
        unit.health.current
    );
    assert!(
        matches!(unit.ai_state, AIState::Moving | AIState::AttackMoving),
        "filled vehicle left in {:?}, path={:?}",
        unit.ai_state(),
        unit.movement.path
    );
    let end = unit.movement.path.last().copied().expect("rally path");
    let rally_cell = logic.pathfinding_system.grid.world_to_grid(rally);
    let cpp_adjusted_rally = ({
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
            .adjust_coord_to_cell(rally_cell, true, &ground_source)
    });
    assert!(
        logic.pathfinding_system.grid.world_to_grid(end) == rally_cell
            && end.distance(cpp_adjusted_rally) < 0.1,
        "ordinary ground path ends at the CPP-adjusted rally cell, end={end:?} expected={cpp_adjusted_rally:?}"
    );
    assert_eq!(unit.ignored_obstacle_id, Some(pad));
}

#[test]
fn repair_action_after_filling_without_a_rally_goes_idle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("NoRallyPad");
    pad_t.repair_dock_frames_for_full_heal = Some(1.0);
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("NoRallyPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("NoRallyTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("NoRallyTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("NoRallyPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("NoRallyTank", 0, Vec3::ZERO)
        .expect("tank");
    {
        let building = logic.host_object_mut(pad).expect("pad");
        building.dock_active_docker = Some(tank);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let (unit, attack_events) = logic.host_object_and_attack_events_mut(tank).expect("tank");
        unit.set_order_target(
            Some(pad),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::SeekingRepair,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.health.current = unit.health.maximum - 1.0;
        unit.ignored_obstacle_id = Some(pad);
        unit.movement.path = vec![Vec3::new(10.0, 0.0, 0.0)];
    }
    // An authored one-frame RepairDock heals once regardless of display dt.
    // CPP action returns TRUE on this pulse; its next action observes full HP.
    logic.update_support_states_for_test(&[tank], 30.0);
    {
        let unit = logic.host_object(tank).expect("tank");
        assert_eq!(unit.health.current, unit.health.maximum);
        assert_eq!(unit.ai_state(), AIState::SeekingRepair);
        assert_eq!(unit.target, Some(pad));
        assert_eq!(
            logic.host_object(pad).unwrap().dock_active_docker,
            Some(tank)
        );
    }
    logic.update_support_states_for_test(&[tank], 0.0);
    let unit = logic.host_object(tank).expect("tank");
    assert!(unit.health.current >= unit.health.maximum - 0.01);
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn aborted_capture_clears_the_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("Capturer");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Capturer".into(), inf_t);
    let mut bld_t = ThingTemplate::new("CaptureMe");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("CaptureMe".into(), bld_t);
    let building = logic
        .create_object_for_player("CaptureMe", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let infantry = logic
        .create_object_for_player("Capturer", 0, Vec3::ZERO)
        .expect("infantry");
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_ai_state(
            AIState::Capturing,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(20.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.target.is_none());
}

#[test]
fn dropped_special_ability_clears_the_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("Hacker");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Hacker".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackMe");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackMe".into(), bld_t);
    let building = logic
        .create_object_for_player("HackMe", 0, Vec3::new(30.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("Hacker", 0, Vec3::ZERO)
        .expect("hacker");
    {
        let unit = logic.host_object_mut(hacker).expect("hacker");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(15.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[hacker], 1.0 / 30.0);
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn finished_hacker_channel_clears_the_building_ignore() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HackerFinish");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HackerFinish".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackFinish");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackFinish".into(), bld_t);
    let building = logic
        .create_object_for_player("HackFinish", 0, Vec3::new(30.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("HackerFinish", 0, Vec3::ZERO)
        .expect("hacker");
    logic.pending_special_abilities.insert(
        hacker,
        PendingSpecialAbility::HackerDisableBuilding {
            target_id: building,
        },
    );
    {
        let unit = logic.host_object_mut(hacker).expect("hacker");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(12.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[hacker], 1.0 / 30.0);
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&hacker));
}

#[test]
fn queued_hacker_path_stays_special_ability() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HackerQueue");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HackerQueue".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackQueue");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackQueue".into(), bld_t);
    let building = logic
        .create_object_for_player("HackQueue", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("HackerQueue", 0, Vec3::ZERO)
        .expect("hacker");
    {
        let unit = logic.host_object_mut(hacker).expect("hacker");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_path_ignoring(
        hacker,
        Vec3::new(80.0, 0.0, 0.0),
        &[],
        Some(building),
    ));
    {
        let unit = logic.host_object(hacker).expect("hacker");
        assert_eq!(unit.ai_state(), AIState::SpecialAbility);
        assert!(unit.waiting_for_path);
    }
    logic.process_pathfind_queue();
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(
        unit.ai_state(),
        AIState::SpecialAbility,
        "installing the hacker path must not look like a new move order"
    );
    assert!(!unit.movement.path.is_empty() || unit.waiting_for_path);
    assert_eq!(unit.ignored_obstacle_id, Some(building));
}

#[test]
fn reissued_hacker_move_keeps_special_ability_and_the_building() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HackerReplay");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HackerReplay".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackReplay");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackReplay".into(), bld_t);
    let building = logic
        .create_object_for_player("HackReplay", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("HackerReplay", 0, Vec3::ZERO)
        .expect("hacker");
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(hacker)
            .expect("hacker");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(building),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.pending_move = Some(Vec3::new(90.0, 0.0, 0.0));
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
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
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(unit.ai_state(), AIState::SpecialAbility);
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    assert!(unit.pending_move.is_none());
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
        "replayed hacker path must ignore the building, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn reissued_attack_crosses_the_target_building() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("AttackReplay");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackReplay".into(), inf_t);
    let mut bld_t = ThingTemplate::new("AttackBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("AttackBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("AttackBuilding", 1, Vec3::new(90.0, 0.0, 0.0))
        .expect("building");
    let attacker = logic
        .create_object_for_player("AttackReplay", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(attacker)
            .expect("attacker");
        unit.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(building),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.pending_move = Some(Vec3::new(90.0, 0.0, 0.0));
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
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
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(unit.ai_state(), AIState::Attacking);
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
        "replayed attack must ignore the target building, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn reissued_gather_paths_to_the_approach() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("GatherReplay");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("GatherReplay".into(), truck_t);
    let mut wh_t = ThingTemplate::new("GatherWarehouse");
    wh_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("GatherWarehouse".into(), wh_t);
    let warehouse = logic
        .create_object_for_player("GatherWarehouse", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("warehouse");
    let truck = logic
        .create_object_for_player("GatherReplay", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(truck)
        .expect("truck")
        .cur_locomotor_name = Some("SupplyTruckLocomotor".into());
    logic
        .host_object_mut(truck)
        .expect("truck")
        .locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    let approach = Vec3::new(70.0, 0.0, 0.0);
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_ai_state(
            AIState::Gathering,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.preferred_dock_id = Some(warehouse);
        unit.pending_move = Some(approach);
        // Xfer restores this request kind alongside the queued destination.
        unit.requested_destination = Some(approach);
        unit.is_final_goal = true;
        unit.is_approach_path = true;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state(), AIState::Gathering);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(
        unit.is_approach_path,
        "reissued dock request keeps its saved mode"
    );
    assert_eq!(unit.requested_destination, Some(approach));
    let end = unit.movement.path.last().copied().expect("approach path");
    assert!(
        logic.pathfinding_system.grid.valid_movement_position(
            logic.pathfinding_system.grid.world_to_grid(end),
            gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground,
            unit.locomotor_surfaces,
            unit.crusher_level > 0,
            0,
        ) && end.distance(approach) < Vec3::ZERO.distance(approach),
        "replay must reach a legal path point closer to the saved dock approach, end={end:?}"
    );
}

#[test]
fn gather_click_paths_to_the_warehouse_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("GatherClick");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("GatherClick".into(), truck_t);
    let mut wh_t = ThingTemplate::new("ClickWarehouse");
    wh_t.add_kind_of(KindOf::Structure).set_health(500.0);
    wh_t.dock_kind = DockKind::SupplyWarehouse;
    logic.templates.insert("ClickWarehouse".into(), wh_t);
    let warehouse_pos = Vec3::new(90.0, 0.0, 0.0);
    let warehouse = logic
        .create_object_for_player("ClickWarehouse", 0, warehouse_pos)
        .expect("warehouse");
    let truck = logic
        .create_object_for_player("GatherClick", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(truck)
        .expect("truck")
        .cur_locomotor_name = Some("SupplyTruckLocomotor".into());
    logic
        .host_object_mut(truck)
        .expect("truck")
        .locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    logic
        .host_object_mut(warehouse)
        .expect("warehouse")
        .selection_radius = 40.0;
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.unit_command_dock_at_supply_warehouse(truck, warehouse));
    logic.process_pathfind_queue();
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state(), AIState::Gathering);
    assert_reserved_dock_approach_path(&logic, warehouse, truck, Vec3::ZERO, warehouse_pos, 40.0);
}

#[test]
fn stunned_gather_click_holds_the_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("StunnedGather");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("StunnedGather".into(), truck_t);
    let mut wh_t = ThingTemplate::new("StunnedWarehouse");
    wh_t.add_kind_of(KindOf::Structure).set_health(500.0);
    wh_t.dock_kind = DockKind::SupplyWarehouse;
    logic.templates.insert("StunnedWarehouse".into(), wh_t);
    let warehouse = logic
        .create_object_for_player("StunnedWarehouse", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("warehouse");
    let truck = logic
        .create_object_for_player("StunnedGather", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(warehouse)
        .expect("warehouse")
        .selection_radius = 40.0;
    logic
        .host_object_mut(truck)
        .expect("truck")
        .shock_stun_frames = 100;
    assert!(logic.unit_command_dock_at_supply_warehouse(truck, warehouse));
    let unit = logic.host_object(truck).expect("truck");
    let held = unit.pending_move.expect("held approach");
    assert!(
        held.distance(Vec3::new(70.0, 0.0, 0.0)) < 1.0,
        "a stunned gather must hold the approach, not the center, held={held:?}"
    );
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.ai_state(), AIState::Gathering);
}

#[test]
fn return_click_paths_to_the_supply_center_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("ReturnClick");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("ReturnClick".into(), truck_t);
    let mut center_t = ThingTemplate::new("ReturnCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic.templates.insert("ReturnCenter".into(), center_t);
    let center = logic
        .create_object_for_player("ReturnCenter", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("center");
    let truck = logic
        .create_object_for_player("ReturnClick", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(truck)
        .expect("truck")
        .cur_locomotor_name = Some("SupplyTruckLocomotor".into());
    logic
        .host_object_mut(truck)
        .expect("truck")
        .locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.selection_radius = 40.0;
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.unit_command_return_supplies(truck, center));
    logic.process_pathfind_queue();
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state(), AIState::ReturningResources);
    assert_reserved_dock_approach_path(
        &logic,
        center,
        truck,
        Vec3::ZERO,
        Vec3::new(90.0, 0.0, 0.0),
        40.0,
    );
}

#[test]
fn stunned_return_click_holds_the_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("StunnedReturn");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("StunnedReturn".into(), truck_t);
    let mut center_t = ThingTemplate::new("StunnedReturnCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic
        .templates
        .insert("StunnedReturnCenter".into(), center_t);
    let center = logic
        .create_object_for_player("StunnedReturnCenter", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("center");
    let truck = logic
        .create_object_for_player("StunnedReturn", 0, Vec3::ZERO)
        .expect("truck");
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.selection_radius = 40.0;
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
    }
    logic
        .host_object_mut(truck)
        .expect("truck")
        .shock_stun_frames = 100;
    assert!(logic.unit_command_return_supplies(truck, center));
    let unit = logic.host_object(truck).expect("truck");
    let held = unit.pending_move.expect("held approach");
    assert!(
        held.distance(Vec3::new(70.0, 0.0, 0.0)) < 1.0,
        "a stunned return must hold the approach, not the center, held={held:?}"
    );
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.ai_state(), AIState::ReturningResources);
}

#[test]
fn guard_object_adjusts_off_an_impassable_post() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{GuardMode, KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("GuardInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GuardInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("GuardPost");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("GuardPost".into(), bld_t);
    let post = Vec3::new(90.0, 0.0, 0.0);
    let building = logic
        .create_object_for_player("GuardPost", 0, post)
        .expect("post");
    let guard = logic
        .create_object_for_player("GuardInf", 0, Vec3::ZERO)
        .expect("guard");
    let blocked = logic.pathfinding_system.grid.world_to_grid(post);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(blocked, PathfindCellType::Obstacle);
    let expected = ({
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
        logic.pathfinding_system.grid.adjust_destination(
            blocked,
            crate::game_logic::object::LOCO_SURFACE_GROUND,
            false,
            400,
            &ground_source,
        )
    })
    .expect("adjustDestination cell");
    assert_ne!(expected, blocked);
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.unit_command_guard_object(guard, building));
    logic.process_pathfind_queue();
    let unit = logic.host_object(guard).expect("guard");
    assert_eq!(unit.ai_state(), AIState::GuardingObject);
    let end_cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(unit.movement.path.last().copied().expect("guard path"));
    assert_eq!(
        end_cell, expected,
        "guard_object must use adjustDestination"
    );
    let guard2 = logic
        .create_object_for_player("GuardInf", 0, Vec3::new(0.0, 0.0, 20.0))
        .expect("guard2");
    let expected2 = ({
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
        logic.pathfinding_system.grid.adjust_destination(
            blocked,
            crate::game_logic::object::LOCO_SURFACE_GROUND,
            false,
            400,
            &ground_source,
        )
    })
    .expect("second adjustDestination cell");
    assert!(logic.unit_command_guard_full(guard2, None, Some(building), 100.0, GuardMode::Normal,));
    logic.process_pathfind_queue();
    let unit2 = logic.host_object(guard2).expect("guard2");
    assert_eq!(unit2.ai_state(), AIState::GuardingObject);
    let end2 = logic
        .pathfinding_system
        .grid
        .world_to_grid(unit2.movement.path.last().copied().expect("full path"));
    assert_eq!(end2, expected2, "guard_full must use adjustDestination");
    assert_ne!(end2, blocked);
}

#[test]
fn attack_exit_clears_the_victim_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("AttackExit");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackExit".into(), inf_t);
    let mut bld_t = ThingTemplate::new("AttackExitBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("AttackExitBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("AttackExitBuilding", 1, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let attacker = logic
        .create_object_for_player("AttackExit", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(attacker)
            .expect("attacker");
        unit.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(building),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
    }
    logic
        .host_object_mut(building)
        .expect("building")
        .status
        .destroyed = true;
    logic.attack_state_exit(attacker);
    let unit = logic.host_object(attacker).expect("attacker");
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.target, Some(building));
}

#[test]
fn live_attack_move_keeps_the_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("AttackMoveKeep");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackMoveKeep".into(), inf_t);
    let mut bld_t = ThingTemplate::new("LiveAttackBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("LiveAttackBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("LiveAttackBuilding", 1, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let attacker = logic
        .create_object_for_player("AttackMoveKeep", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(attacker)
            .expect("attacker");
        unit.set_ai_state(
            AIState::AttackMoving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(building),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
        unit.requested_destination = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.movement.path = vec![Vec3::new(80.0, 0.0, 0.0)];
    }
    logic.attack_state_exit(attacker);
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    assert_eq!(unit.movement.path, vec![Vec3::new(80.0, 0.0, 0.0)]);
}

#[test]
fn player_stop_clears_a_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("StopInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("StopInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("StopBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("StopBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("StopBuilding", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let unit_id = logic
        .create_object_for_player("StopInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(40.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    assert!(logic.unit_command_stop(unit_id));
    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn aircraft_stop_clears_a_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("StopJet");
    jet_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0);
    logic.templates.insert("StopJet".into(), jet_t);
    let mut bld_t = ThingTemplate::new("StopJetBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("StopJetBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("StopJetBuilding", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let jet = logic
        .create_object_for_player("StopJet", 0, Vec3::new(0.0, 30.0, 0.0))
        .expect("jet");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.status.airborne_target = true;
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(40.0, 30.0, 0.0)];
        unit.set_status_moving(true);
    }
    assert!(logic.unit_command_stop(jet));
    let unit = logic.host_object(jet).expect("jet");
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn player_move_clears_a_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("MoveInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("MoveInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("MoveBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("MoveBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("MoveBuilding", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let unit_id = logic
        .create_object_for_player("MoveInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
    }
    assert!(logic.unit_command_move_to(unit_id, Vec3::new(30.0, 0.0, 10.0)));
    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.ai_state(), AIState::Moving);
    assert!(unit.ignored_obstacle_id.is_none());
}

#[test]
fn carrier_idle_replaces_the_jet_building_ignore_with_the_carrier() {
    use crate::game_logic::host_flight_deck::{
        HostFlightDeckCommand, HostFlightDeckSpace, HostFlightDeckState,
    };
    use crate::game_logic::{FlightDeckMetadata, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut carrier_t = ThingTemplate::new("StopCarrier");
    carrier_t.add_kind_of(KindOf::Structure).set_health(2000.0);
    carrier_t.flight_deck = Some(FlightDeckMetadata {
        payload_template: "StopJet".into(),
        num_rows: 1,
        num_cols: 1,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        heal_amount_per_second: 0.0,
        cleanup_frames: 1,
        human_follow_frames: 1,
        replacement_frames: 1,
        dock_animation_frames: 1,
        launch_wave_frames: 1,
        launch_ramp_frames: 1,
        lower_ramp_frames: 1,
        catapult_fire_frames: 1,
        catapult_system: [None, None],
    });
    logic.templates.insert("StopCarrier".into(), carrier_t);
    let mut jet_t = ThingTemplate::new("DeckJet");
    jet_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0);
    logic.templates.insert("DeckJet".into(), jet_t);
    let mut bld_t = ThingTemplate::new("JetAttackBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("JetAttackBuilding".into(), bld_t);
    let carrier = logic
        .create_object_for_player("StopCarrier", 0, Vec3::ZERO)
        .expect("carrier");
    let jet = logic
        .create_object_for_player("DeckJet", 0, Vec3::new(20.0, 30.0, 0.0))
        .expect("jet");
    let building = logic
        .create_object_for_player("JetAttackBuilding", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("building");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.status.airborne_target = true;
        unit.ignored_obstacle_id = Some(building);
        unit.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    let mut state = HostFlightDeckState::default();
    state.got_info = true;
    state.spaces.push(HostFlightDeckSpace {
        object_id: Some(jet),
        position: Vec3::new(5.0, 0.0, 0.0),
        orientation: 0.0,
        runway: 0,
    });
    logic.flight_decks.insert(carrier, state);
    assert!(logic.flight_deck_ai_do_command(carrier, HostFlightDeckCommand::Idle, None, None));
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state(), AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(carrier));
}

#[test]
fn leaving_enter_clears_the_carrier_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("LeaveEnterJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("LeaveEnterJet".into(), jet_t);
    let mut deck_t = ThingTemplate::new("LeaveEnterDeck");
    deck_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("LeaveEnterDeck".into(), deck_t);
    let deck = logic
        .create_object_for_player("LeaveEnterDeck", 0, Vec3::ZERO)
        .expect("deck");
    let jet = logic
        .create_object_for_player("LeaveEnterJet", 0, Vec3::new(10.0, 20.0, 0.0))
        .expect("jet");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.set_ai_state(
            AIState::Entering,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(deck);
        unit.set_ai_state(
            AIState::Docked,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state(), AIState::Docked);
    assert!(unit.ignored_obstacle_id.is_none());
}

#[test]
fn queued_enter_path_keeps_the_carrier_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("EnterQueueJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("EnterQueueJet".into(), jet_t);
    let mut deck_t = ThingTemplate::new("EnterQueueDeck");
    deck_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("EnterQueueDeck".into(), deck_t);
    let deck = logic
        .create_object_for_player("EnterQueueDeck", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("deck");
    let jet = logic
        .create_object_for_player("EnterQueueJet", 0, Vec3::ZERO)
        .expect("jet");
    {
        let (unit, attack_events) = logic.host_object_and_attack_events_mut(jet).expect("jet");
        unit.set_ai_state(
            AIState::Entering,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(deck);
        unit.set_order_target(
            Some(deck),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_path_ignoring(jet, Vec3::new(80.0, 0.0, 0.0), &[], Some(deck)));
    logic.process_pathfind_queue();
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state(), AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(deck));
    assert!(!unit.movement.path.is_empty() || unit.waiting_for_path);
}

#[test]
fn reissued_enter_crosses_the_carrier() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("EnterReplayJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("EnterReplayJet".into(), jet_t);
    let mut deck_t = ThingTemplate::new("EnterReplayDeck");
    deck_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("EnterReplayDeck".into(), deck_t);
    let deck = logic
        .create_object_for_player("EnterReplayDeck", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("deck");
    let jet = logic
        .create_object_for_player("EnterReplayJet", 0, Vec3::ZERO)
        .expect("jet");
    {
        let (unit, attack_events) = logic.host_object_and_attack_events_mut(jet).expect("jet");
        unit.set_ai_state(
            AIState::Entering,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(deck);
        unit.set_order_target(
            Some(deck),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.pending_move = Some(Vec3::new(90.0, 0.0, 0.0));
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            deck.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state(), AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(deck));
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
        "replayed enter must ignore the carrier, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn cliff_cell_picks_the_cliff_member_when_the_set_changes() {
    use crate::game_logic::host_upgrade_module_residuals::{AuthoredLocomotorSet, HostLocomotorSetKind};
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("CliffBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeGroundLocomotor".into(),
            "CombatBikeCliffLocomotor".into(),
        ],
    }]);
    logic.templates.insert("CliffBike".into(), template);
    let id = logic
        .create_object_for_player("CliffBike", 0, Vec3::ZERO)
        .expect("bike");
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Cliff);
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    let bike = logic.host_object(id).expect("bike");
    assert_eq!(
        bike.cur_locomotor_name.as_deref(),
        Some("CombatBikeCliffLocomotor"),
        "a cliff cell must bind the cliff member before the next movement frame"
    );
}
#[test]
fn garrison_enter_ignores_the_bunker() {
    use crate::game_logic::{
        BuildingData, BuildingType, ContainAdmission, ContainModuleKind, KindOf, Player,
        ThingTemplate,
    };
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("GarrisonInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    inf_t.transport_slot_count = Some(1);
    logic.templates.insert("GarrisonInf".into(), inf_t);
    let mut bunker_t = ThingTemplate::new("GarrisonBunker");
    bunker_t.add_kind_of(KindOf::Structure).set_health(800.0);
    bunker_t.contain_module.kind = ContainModuleKind::Garrison;
    bunker_t.contain_module.admission = ContainAdmission::InfantryOnly;
    logic.templates.insert("GarrisonBunker".into(), bunker_t);
    let bunker = logic
        .create_object_for_player("GarrisonBunker", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("bunker");
    let infantry = logic
        .create_object_for_player("GarrisonInf", 0, Vec3::ZERO)
        .expect("infantry");
    {
        let building = logic.host_object_mut(bunker).expect("bunker");
        let mut data = BuildingData::new(BuildingType::PowerPlant);
        data.max_garrison = 4;
        building.building_data = Some(data);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    use crate::game_logic::pathfinding::GridPos;
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(40.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            bunker.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(
        logic.try_guard_enter_or_hijack(infantry, bunker, false, Team::USA),
        "infantry should be allowed to enter the bunker"
    );
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(bunker));
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
        "garrison enter must path through the bunker column, xs={xs:?} wall={wall_x}"
    );
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Entering);
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
        "the next enter tick must still path through the bunker, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn hijack_paths_through_the_vehicle() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("HijackInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HijackInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("HijackTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("HijackTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("HijackTruck", 1, Vec3::new(90.0, 0.0, 0.0))
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("HijackInf", 0, Vec3::ZERO)
        .expect("infantry");
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
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
    assert!(logic.try_guard_enter_or_hijack(infantry, vehicle, true, Team::USA));
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::SpecialAbility);
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
        "hijack must path through the vehicle column, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn non_hijacker_at_the_vehicle_is_released() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("PlainInfantry");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("PlainInfantry".into(), inf_t);
    let mut veh_t = ThingTemplate::new("StealMe");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("StealMe".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("StealMe", 1, Vec3::ZERO)
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("PlainInfantry", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::Hijack { target_id: vehicle },
    );
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(infantry)
            .expect("infantry");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(vehicle),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(vehicle);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
    assert!(!logic.host_object(vehicle).expect("vehicle").status.hijacked);
}

#[test]
fn rejected_car_bomb_releases_the_infantry() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("BombInfantry");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("BombInfantry".into(), inf_t);
    let mut boat_t = ThingTemplate::new("BombBoat");
    boat_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Boat)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("BombBoat".into(), boat_t);
    let boat = logic
        .create_object_for_player("BombBoat", 1, Vec3::ZERO)
        .expect("boat");
    let infantry = logic
        .create_object_for_player("BombInfantry", 0, Vec3::ZERO)
        .expect("infantry");
    logic
        .pending_special_abilities
        .insert(infantry, PendingSpecialAbility::CarBomb { target_id: boat });
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(infantry)
            .expect("infantry");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(boat),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(boat);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
}

#[test]
fn rejected_disguise_releases_the_infantry() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("DisguiseInfantry");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("DisguiseInfantry".into(), inf_t);
    let mut jet_t = ThingTemplate::new("DisguiseJet");
    jet_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("DisguiseJet".into(), jet_t);
    let jet = logic
        .create_object_for_player("DisguiseJet", 1, Vec3::ZERO)
        .expect("jet");
    let infantry = logic
        .create_object_for_player("DisguiseInfantry", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::DisguiseAsVehicle { target_id: jet },
    );
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(infantry)
            .expect("infantry");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(jet),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(jet);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
}

#[test]
fn sabotage_of_a_vehicle_releases_the_infantry() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("SaboteurInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("SaboteurInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("SabotageTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("SabotageTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("SabotageTruck", 1, Vec3::ZERO)
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("SaboteurInf", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::Sabotage { target_id: vehicle },
    );
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(infantry)
            .expect("infantry");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(vehicle),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(vehicle);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
}

#[test]
fn hijack_followup_tick_still_crosses_the_vehicle() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("HijackFollow");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HijackFollow".into(), inf_t);
    let mut veh_t = ThingTemplate::new("HijackFollowTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("HijackFollowTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("HijackFollowTruck", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("HijackFollow", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::Hijack { target_id: vehicle },
    );
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(infantry)
            .expect("infantry");
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_order_target(
            Some(vehicle),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
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
            vehicle.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state(), AIState::SpecialAbility);
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
        "hijack follow-up must path through the vehicle, xs={xs:?} wall={wall_x}"
    );
}
