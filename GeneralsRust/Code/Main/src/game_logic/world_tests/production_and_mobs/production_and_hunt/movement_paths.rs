//! Path geometry, dock approach and arrival fixtures.
use super::*;

#[test]
fn finished_move_clears_the_ignored_obstacle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut tpl = ThingTemplate::new("Mover");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("Mover".into(), tpl);
    let id = logic
        .create_object_for_player("Mover", 0, Vec3::ZERO)
        .expect("unit");
    let building = logic
        .create_object_for_player("Mover", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.ignored_obstacle_id = Some(building);
        unit.set_ai_state(
            AIState::Repairing,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.movement.path.push(Vec3::new(10.0, 0.0, 0.0));
        unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
        assert_eq!(
            unit.ignored_obstacle_id,
            Some(building),
            "a pack or repair stop must keep the building ignore"
        );
        assert_eq!(unit.ai_state(), AIState::Repairing);
    }
    {
        let (unit, fixture_partition_unit) = logic.host_object_and_partition_mut(id).expect("unit");
        unit.set_ai_state(
            AIState::Moving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_position_with_partition_owner(
            Vec3::ZERO,
            0,
            &mut Default::default(),
            fixture_partition_unit,
        );
        unit.movement.path = vec![Vec3::ZERO];
        unit.movement.current_path_index = 0;
        unit.movement.target_position = Some(Vec3::ZERO);
        unit.set_locomotor_goal_position_on_path();
        unit.ignored_obstacle_id = Some(building);
    }
    logic.update_movement(&[id], 1.0 / 30.0);
    {
        let unit = logic.host_object(id).expect("same admitted mover");
        assert_eq!(unit.ignored_obstacle_id, None);
        assert_eq!(unit.ai_state(), AIState::Idle);
        assert!(unit.movement.path.is_empty());
    }
    {
        let (unit, fixture_partition_unit) = logic.host_object_and_partition_mut(id).expect("unit");
        let near = Vec3::new(1.0, 0.0, 0.0);
        unit.set_ai_state(
            AIState::Moving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_position_with_partition_owner(
            Vec3::ZERO,
            0,
            &mut Default::default(),
            fixture_partition_unit,
        );
        unit.movement.velocity = Vec3::ZERO;
        unit.movement.path = vec![near];
        unit.movement.current_path_index = 0;
        unit.movement.target_position = Some(near);
        unit.close_enough_dist = Some(2.0);
        unit.set_locomotor_goal_position_on_path();
        unit.ignored_obstacle_id = Some(building);
    }
    logic.update_movement(&[id], 1.0 / 30.0);
    {
        let unit = logic.host_object(id).expect("same admitted mover");
        assert_eq!(
            unit.ignored_obstacle_id, None,
            "arriving inside close-enough distance must drop the ignore"
        );
    }
}

#[test]
fn repair_abort_clears_the_ignored_obstacle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut tpl = ThingTemplate::new("NotADozer");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("NotADozer".into(), tpl);
    let id = logic
        .create_object_for_player("NotADozer", 0, Vec3::ZERO)
        .expect("unit");
    let building = logic
        .create_object_for_player("NotADozer", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    {
        let (unit, attack_events) = logic.host_object_and_attack_events_mut(id).expect("unit");
        unit.set_order_target(
            Some(building),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::Repairing,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(building);
    }
    logic.update_support_states_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert_eq!(
        unit.ignored_obstacle_id, None,
        "a unit that cannot repair must drop the building ignore"
    );
    assert!(unit.target.is_none());
}

#[test]
fn full_truck_returns_to_the_supply_center_approach() {
    use crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
    use crate::game_logic::{
        DockKind, KindOf, Player, SupplyTruckMetadata, SupplyTruckState, ThingTemplate,
    };
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut warehouse_t = ThingTemplate::new("SupplyWarehouse");
    warehouse_t.add_kind_of(KindOf::Structure).set_health(500.0);
    warehouse_t.dock_kind = DockKind::SupplyWarehouse;
    logic
        .templates
        .insert("SupplyWarehouse".into(), warehouse_t);
    let mut center_t = ThingTemplate::new("AmericaSupplyCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic
        .templates
        .insert("AmericaSupplyCenter".into(), center_t);
    let mut truck_t = ThingTemplate::new("AmericaSupplyTruck");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
    truck_t.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 1,
        warehouse_scan_distance: 200.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert("AmericaSupplyTruck".into(), truck_t);
    let center_pos = Vec3::new(200.0, 0.0, 0.0);
    let warehouse = logic
        .create_object_for_player("SupplyWarehouse", 0, Vec3::ZERO)
        .expect("warehouse");
    let center = logic
        .create_object_for_player("AmericaSupplyCenter", 0, center_pos)
        .expect("center");
    let truck = logic
        .create_object_for_player("AmericaSupplyTruck", 0, Vec3::ZERO)
        .expect("truck");
    {
        let src = logic.host_object_mut(warehouse).expect("warehouse");
        src.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
        src.dock_active_docker = Some(truck);
        src.selection_radius = 40.0;
    }
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
        pad.selection_radius = 40.0;
    }
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(truck)
            .expect("truck");
        unit.set_order_target(
            Some(warehouse),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::Gathering,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
        unit.supply_truck_state = SupplyTruckState::DockingWarehouse;
        unit.supply_truck_next_dock_action_frame = 0;
    }
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state(), AIState::ReturningResources);
    let end = unit
        .movement
        .path
        .last()
        .copied()
        .expect("installed return path");
    let approach = Vec3::new(180.0, 0.0, 0.0);
    assert!(
        end.distance(approach) < 1.0,
        "path must end at the dock approach {approach:?}, end={end:?} path={:?}",
        unit.movement.path
    );
    assert!(unit.ignored_obstacle_id.is_none());
    logic
        .host_object_mut(warehouse)
        .expect("warehouse")
        .status
        .destroyed = true;
    let mut next_t = ThingTemplate::new("NextSupplyWarehouse");
    next_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplySource)
        .set_health(500.0);
    next_t.dock_kind = DockKind::SupplyWarehouse;
    logic.templates.insert("NextSupplyWarehouse".into(), next_t);
    let next = logic
        .create_object_for_player("NextSupplyWarehouse", 0, Vec3::new(300.0, 0.0, 0.0))
        .expect("next warehouse");
    {
        let src = logic.host_object_mut(next).expect("next");
        src.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
        src.selection_radius = 40.0;
        src.set_status_under_construction(false);
        src.construction_percent = 1.0;
    }
    {
        let (unit, fixture_partition_unit) =
            logic.host_object_and_partition_mut(truck).expect("truck");
        unit.set_position_with_partition_owner(
            end,
            0,
            &mut Default::default(),
            fixture_partition_unit,
        );
        unit.movement.path.clear();
        unit.movement.target_position = None;
        unit.supply_truck_state = SupplyTruckState::Wanting;
    }
    let start = logic.players.get(&0).expect("player").resources.supplies;
    for _ in 0..4 {
        logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    }
    let cash = logic.players.get(&0).expect("player").resources.supplies;
    assert_eq!(
        cash,
        start + VALUE_PER_SUPPLY_BOX as u32,
        "a truck on the dock approach must pay the box"
    );
    let back = logic.host_object(truck).expect("truck");
    assert_eq!(back.stored_resources.supplies, 0);
    assert_eq!(back.ai_state(), AIState::Gathering);
    assert_ne!(back.ai_state(), AIState::Attacking);
    let back_end = back
        .movement
        .path
        .last()
        .copied()
        .expect("path to the next warehouse");
    assert!(
        back_end.distance(Vec3::new(280.0, 0.0, 0.0)) < 1.0,
        "the next warehouse must be approached at its dock, end={back_end:?} path={:?}",
        back.movement.path
    );
}

#[test]
fn empty_truck_regroups_outside_the_supply_center() {
    use crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
    use crate::game_logic::{DockKind, KindOf, Player, SupplyTruckMetadata, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut center_t = ThingTemplate::new("RegroupCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic.templates.insert("RegroupCenter".into(), center_t);
    let mut truck_t = ThingTemplate::new("RegroupTruck");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
    truck_t.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 1,
        warehouse_scan_distance: 500.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert("RegroupTruck".into(), truck_t);
    let center_pos = Vec3::new(200.0, 0.0, 0.0);
    let center = logic
        .create_object_for_player("RegroupCenter", 0, center_pos)
        .expect("center");
    let truck = logic
        .create_object_for_player("RegroupTruck", 0, Vec3::ZERO)
        .expect("truck");
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.selection_radius = 40.0;
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.supply_truck_force_pending = true;
        unit.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
    }
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let loaded = logic.host_object(truck).expect("truck");
    assert_eq!(loaded.ai_state(), AIState::ReturningResources);
    let to_center = loaded.movement.path.last().copied().expect("center path");
    assert!(
        to_center.distance(Vec3::new(180.0, 0.0, 0.0)) < 1.0,
        "wanting with a box must dock the approach, end={to_center:?}"
    );
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_stored_supplies(0);
        unit.supply_truck_force_pending = true;
        unit.set_status_moving(false);
        unit.movement.path.clear();
        unit.movement.target_position = None;
        unit.ignored_obstacle_id = Some(center);
    }
    let mut terrain = crate::game_logic::terrain::TerrainData::flat(
        Vec3::new(-1000.0, 0.0, -1000.0),
        Vec3::new(1000.0, 0.0, 1000.0),
    );
    terrain.add_water_polygon(vec![(247, -3), (253, -3), (253, 3), (247, 3)], 10.0);
    logic.terrain = Some(terrain);
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let regrouped = logic.host_object(truck).expect("truck");
    assert_eq!(regrouped.ai_state(), AIState::Moving);
    assert_eq!(
        regrouped.supply_truck_state,
        crate::game_logic::SupplyTruckState::Regrouping
    );
    assert!(regrouped.ignored_obstacle_id.is_none());
    let regroup = regrouped
        .movement
        .path
        .last()
        .copied()
        .expect("regroup path");
    let from_center = regroup.distance(center_pos);
    assert!(
        from_center >= 45.0 - 0.1 && from_center <= 100.0,
        "regroup must clear the building by a 5-unit sphere and stay within 100, end={regroup:?}"
    );
    let in_water =
        regroup.x >= 247.0 && regroup.x <= 253.0 && regroup.z >= -3.0 && regroup.z <= 3.0;
    assert!(
        !in_water,
        "the first +X ring is underwater and must be skipped, end={regroup:?}"
    );
    {
        let (unit, fixture_partition_unit) =
            logic.host_object_and_partition_mut(truck).expect("truck");
        unit.set_position_with_partition_owner(
            Vec3::new(150.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_unit,
        );
        unit.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_status_moving(false);
        unit.set_stored_supplies(0);
        unit.supply_truck_force_pending = true;
        unit.movement.path.clear();
        unit.movement.target_position = None;
        unit.ignored_obstacle_id = Some(center);
    }
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let close = logic.host_object(truck).expect("truck");
    assert!(
        close.movement.path.is_empty(),
        "a truck already inside the bounding-sphere gap must not path, path={:?}",
        close.movement.path
    );
    assert!(close.ignored_obstacle_id.is_none());
    assert_eq!(
        close.supply_truck_state,
        crate::game_logic::SupplyTruckState::Regrouping
    );
}

#[test]
fn repair_approach_crosses_the_buildings_obstacle() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("RepairPad");
    pad.add_kind_of(KindOf::Structure).set_health(1_000.0);
    logic.templates.insert("RepairPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("RepairDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("RepairDozer".into(), dozer_tpl);
    let from = Vec3::new(10.0, 0.0, 10.0);
    let to = Vec3::new(160.0, 0.0, 10.0);
    let pad_id = logic
        .create_object_for_player("RepairPad", 0, to)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("RepairDozer", 0, from)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.health.current = 100.0;
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
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
        let (obj, attack_events) = logic
            .host_object_and_attack_events_mut(dozer)
            .expect("dozer");
        obj.set_target(
            Some(pad_id),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        obj.set_ai_state(
            AIState::Repairing,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        obj.movement.path.clear();
        obj.waiting_for_path = false;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update();
    logic.process_pathfind_queue();
    let (state, path) = {
        let obj = logic.host_object(dozer).expect("dozer");
        (obj.ai_state().clone(), obj.movement.path.clone())
    };
    assert_eq!(state, AIState::Repairing);
    let xs: Vec<i32> = path
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
        "repair approach must ignore the building and cross its obstacle, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn finished_repair_releases_the_dozer() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("HealPad");
    pad.add_kind_of(KindOf::Structure).set_health(200.0);
    logic.templates.insert("HealPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("HealDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("HealDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("HealPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("HealDozer", 0, Vec3::ZERO)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
        building.health.current = building.health.maximum - 1.0;
    }
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(dozer)
            .expect("dozer");
        unit.set_order_target(
            Some(pad_id),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::Repairing,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.dozer_task_repair_target = Some(pad_id);
        unit.ignored_obstacle_id = Some(pad_id);
        unit.movement.path = vec![Vec3::new(20.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.frame = 10;
    for _ in 0..8 {
        logic.update_support_states_for_test(&[dozer], 1.0);
        logic.frame += 1;
    }
    let building = logic.host_object(pad_id).expect("pad");
    assert!(
        building.health.current >= building.health.maximum - 0.01,
        "in-range dozer must finish the repair, hp={}",
        building.health.current
    );
    let unit = logic.host_object(dozer).expect("dozer");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.dozer_task_repair_target.is_none());
}

#[test]
fn dead_repair_target_releases_the_dozer() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("DeadPad");
    pad.add_kind_of(KindOf::Structure).set_health(200.0);
    logic.templates.insert("DeadPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("DeadDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("DeadDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("DeadPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("DeadDozer", 0, Vec3::ZERO)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.health.current = 0.0;
        building.status.destroyed = true;
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let (unit, attack_events) = logic
            .host_object_and_attack_events_mut(dozer)
            .expect("dozer");
        unit.set_order_target(
            Some(pad_id),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::Repairing,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.dozer_task_repair_target = Some(pad_id);
        unit.ignored_obstacle_id = Some(pad_id);
        unit.movement.path = vec![Vec3::new(30.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[dozer], 1.0 / 30.0);
    let unit = logic.host_object(dozer).expect("dozer");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.dozer_task_repair_target.is_none());
}

#[test]
fn repair_without_a_target_does_not_keep_the_task() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("OrphanPad");
    pad.add_kind_of(KindOf::Structure).set_health(200.0);
    logic.templates.insert("OrphanPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("OrphanDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("OrphanDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("OrphanPad", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("pad");
    let dozer = logic
        .create_object_for_player("OrphanDozer", 0, Vec3::ZERO)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.health.current = 50.0;
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(dozer).expect("dozer");
        unit.set_ai_state(
            AIState::Repairing,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.dozer_task_repair_target = Some(pad_id);
        unit.ignored_obstacle_id = Some(pad_id);
        unit.movement.path = vec![Vec3::new(40.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[dozer], 1.0 / 30.0);
    let unit = logic.host_object(dozer).expect("dozer");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.dozer_task_repair_target.is_none());
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.target.is_none());
}

#[test]
fn seeker_without_a_pad_goes_idle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut tpl = ThingTemplate::new("SeekTank");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("SeekTank".into(), tpl);
    let id = logic
        .create_object_for_player("SeekTank", 0, Vec3::ZERO)
        .expect("tank");
    {
        let unit = logic.host_object_mut(id).expect("tank");
        unit.set_ai_state(
            AIState::SeekingRepair,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.ignored_obstacle_id = Some(id);
        unit.movement.path = vec![Vec3::new(40.0, 0.0, 0.0)];
        unit.set_status_moving(true);
        unit.health.current = 50.0;
    }
    logic.update_support_states_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("tank");
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.target.is_none());
}

#[test]
fn full_vehicle_leaves_the_repair_pad_for_its_rally() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("RallyPad");
    pad_t.repair_dock_frames_for_full_heal = Some(150.0);
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("RallyPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("FixedTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("FixedTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("RallyPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("FixedTank", 0, Vec3::ZERO)
        .expect("tank");
    let rally = Vec3::new(80.0, 0.0, 0.0);
    {
        let building = logic.host_object_mut(pad).expect("pad");
        let mut data = BuildingData::new(BuildingType::PowerPlant);
        data.rally_point = Some(rally);
        building.building_data = Some(data);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let (unit, attack_events) = logic.host_object_and_attack_events_mut(tank).expect("tank");
        // C++ requestPath requires an active locomotor with nonzero valid
        // surfaces; synthetic vehicles default to an immobile fixture.
        unit.cur_locomotor_name = Some("GattlingTankLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.set_order_target(
            Some(pad),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::SeekingRepair,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.health.current = unit.health.maximum;
    }
    {
        let building = logic.host_object(pad).expect("pad");
        assert!(building.is_kind_of(KindOf::RepairPad), "pad kind missing");
        assert_eq!(
            building.building_data.as_ref().and_then(|b| b.rally_point),
            Some(rally)
        );
        let unit = logic.host_object(tank).expect("tank");
        assert_eq!(unit.target, Some(pad));
        assert!(unit.can_move(), "tank cannot move");
        assert!(
            unit.health.current >= unit.health.maximum - 0.01,
            "hp {} / {}",
            unit.health.current,
            unit.health.maximum
        );
    }
    logic.update_support_states_for_test(&[tank], 1.0 / 30.0);
    let unit = logic.host_object(tank).expect("tank");
    assert!(
        matches!(unit.ai_state, AIState::Moving | AIState::AttackMoving),
        "left for rally in {:?}, path={:?}",
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
fn passthrough_rally_snapshots_the_pad() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("SnapPad");
    pad_t.repair_dock_frames_for_full_heal = Some(150.0);
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("SnapPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("SnapTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("SnapTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("SnapPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("SnapTank", 0, Vec3::ZERO)
        .expect("tank");
    {
        let building = logic.host_object_mut(pad).expect("pad");
        let mut data = BuildingData::new(BuildingType::PowerPlant);
        data.rally_point = Some(Vec3::new(80.0, 0.0, 0.0));
        building.building_data = Some(data);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let (unit, attack_events) = logic.host_object_and_attack_events_mut(tank).expect("tank");
        // C++ requestPath requires an active locomotor with nonzero valid
        // surfaces; synthetic vehicles default to an immobile fixture.
        unit.cur_locomotor_name = Some("GattlingTankLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.set_order_target(
            Some(pad),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.set_ai_state(
            AIState::SeekingRepair,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        unit.health.current = unit.health.maximum;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update_support_states_for_test(&[tank], 1.0 / 30.0);
    let queued_ignore: Vec<_> = logic
        .pathfinding_system
        .pending_paths()
        .map(|p| p.ignore_obstacle)
        .collect();
    assert_eq!(queued_ignore, vec![Some(pad)]);
}

#[test]
fn contact_attack_ignores_the_victim() {
    use crate::game_logic::{KindOf, Player, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("ContactInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ContactInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("ContactBuilding");
    bld_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("ContactBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("ContactBuilding", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("building");
    logic
        .host_object_mut(building)
        .expect("building")
        .selection_radius = 25.0;
    let attacker = logic
        .create_object_for_player("ContactInf", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let unit = logic.host_object_mut(attacker).expect("attacker");
        // C++ attack-path requests use the attacker's active locomotor
        // surfaces; a newly-created test object otherwise has mask zero.
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 5.0,
            min_range: 0.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
    }
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
        logic.pathfinding_system.apply_structure_static_blocks(
            logic.objects.read(),
            0.0,
            &ground_source,
        )
    });
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
    let target_cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(Vec3::new(120.0, 0.0, 0.0));
    assert_eq!(
        logic.pathfinding_system.grid.cell_type(target_cell),
        gamelogic::ai::pathfind_astar::PathfindCellType::Obstacle,
        "fixture must exercise the victim's actual obstacle footprint"
    );
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_attack_path(attacker, Some(building), Vec3::new(120.0, 0.0, 0.0)));
    logic.process_pathfind_queue();
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    let enters_victim_footprint = unit.movement.path.iter().any(|wp| {
        let cell = logic.pathfinding_system.grid.world_to_grid(*wp);
        logic.pathfinding_system.grid.cell_type(cell)
            == gamelogic::ai::pathfind_astar::PathfindCellType::Obstacle
    });
    assert!(
        enters_victim_footprint,
        "contact attack must route into the victim's real obstacle footprint, path={:?}",
        unit.movement.path
    );
    assert!(
        unit.movement
            .path
            .last()
            .is_some_and(|last| last.distance(Vec3::new(120.0, 0.0, 0.0)) < 0.1),
        "contact attack should jam the final node to the victim position: {:?}",
        unit.movement.path
    );
    assert_eq!(unit.path_extra_distance, 100.0);
}

#[test]
fn ranged_attack_does_not_keep_a_stale_ignore() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("RangedInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("RangedInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("RangedBuilding");
    bld_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("RangedBuilding".into(), bld_t);
    let mut block_t = ThingTemplate::new("StaleBlocker");
    block_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("StaleBlocker".into(), block_t);
    let building = logic
        .create_object_for_player("RangedBuilding", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("building");
    let blocker = logic
        .create_object_for_player("StaleBlocker", 1, Vec3::new(0.0, 0.0, 80.0))
        .expect("blocker");
    let attacker = logic
        .create_object_for_player("RangedInf", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let unit = logic.host_object_mut(attacker).expect("attacker");
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            min_range: 0.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
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
            blocker.0,
            None,
            None,
        );
    }
    logic.pathfinding_system.set_ignore_obstacle(Some(blocker));
    assert!(logic.assign_unit_attack_path(attacker, Some(building), Vec3::new(120.0, 0.0, 0.0)));
    assert_eq!(logic.pathfinding_system.ignore_obstacle(), None);
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(
        unit.ignored_obstacle_id, None,
        "firing scan should succeed; the fail fallback is not this case"
    );
    assert_eq!(unit.path_extra_distance, 0.0);
    let end = unit.movement.path.last().copied().expect("firing path");
    let end_x = logic.pathfinding_system.grid.world_to_grid(end).x;
    assert!(
        end_x < wall_x,
        "ranged firing path must stop short of the wall, end_x={end_x} wall={wall_x} path={:?}",
        unit.movement.path
    );
}

#[test]
fn intermediate_path_keeps_the_next_segment_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("PathExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("PathExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("PathExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    let unit = logic.host_object_mut(id).expect("unit");
    unit.movement.path = vec![
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(10.0, 0.0, 30.0),
        Vec3::new(50.0, 0.0, 30.0),
    ];
    unit.movement.current_path_index = 1;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 70.0).abs() < 0.01);
    unit.path_extra_distance = 100.0;
    unit.is_attack_path = false;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 70.0).abs() < 0.01);
    unit.path_extra_distance = 100.0;
    unit.is_attack_path = true;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 100.0).abs() < 0.01);
}

#[test]
fn exact_waypoint_path_sums_five_links() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("WayExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("WayExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("WayExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    let points: Vec<Vec3> = (0..7)
        .map(|i| Vec3::new(i as f32 * 10.0, 0.0, 0.0))
        .collect();
    let unit = logic.host_object_mut(id).expect("unit");
    unit.movement.path = points;
    unit.movement.current_path_index = 0;
    unit.is_exact_path = true;
    unit.is_attack_path = false;
    unit.path_extra_distance = 100.0;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 51.0).abs() < 0.01);
}

#[test]
fn non_final_hop_keeps_the_obstacle_corner() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HopInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HopInf".into(), inf_t);
    let grid = &logic.pathfinding_system.grid;
    let start_cell = grid.world_to_grid(Vec3::ZERO);
    let corner_cell = grid.world_to_grid(Vec3::new(80.0, 0.0, 0.0));
    let corner = ({
        let ground_owner = logic.world_services.terrain();
        let ground_loan = ground_owner
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &ground_loan,
            logic.map_loaded,
            logic.terrain.as_ref(),
            logic.pathfinding_height_samples.as_ref(),
            grid,
        );
        grid.grid_to_world(corner_cell, &ground_source)
    });
    let blocked_cell = GridPos::new((start_cell.x + corner_cell.x) / 2, start_cell.y);
    logic.pathfinding_system.grid.set_cell_obstacle_owned(
        blocked_cell,
        false,
        false,
        9,
        None,
        None,
    );
    let unit = logic
        .create_object_for_player("HopInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit).expect("unit");
        // C++ requestPath asserts that a unit has at least one valid locomotor
        // surface before invoking Pathfinder::findPath.
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    }
    logic.force_map_loaded_for_path_test(true);
    let ok = logic.assign_unit_path(unit, Vec3::new(160.0, 0.0, 0.0), &[corner]);
    logic.process_pathfind_queue();
    let path = logic.host_object(unit).expect("unit").movement.path.clone();
    let keeps_corner = path
        .iter()
        .any(|wp| logic.pathfinding_system.grid.world_to_grid(*wp) == corner_cell);
    let crosses_blocked_cell = path.windows(2).any(|segment| {
        !logic.pathfinding_system.line_passable_for_surfaces(
            segment[0],
            segment[1],
            crate::game_logic::object::LOCO_SURFACE_GROUND,
        )
    });
    let blocked_segments: Vec<_> = path
        .windows(2)
        .filter(|segment| {
            !logic.pathfinding_system.line_passable_for_surfaces(
                segment[0],
                segment[1],
                crate::game_logic::object::LOCO_SURFACE_GROUND,
            )
        })
        .map(|segment| {
            (
                segment[0],
                segment[1],
                logic.pathfinding_system.grid.world_to_grid(segment[0]),
                logic.pathfinding_system.grid.world_to_grid(segment[1]),
            )
        })
        .collect();
    let detours_around_blocker = path
        .iter()
        .any(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).y != start_cell.y);
    assert!(
        ok && keeps_corner && detours_around_blocker && !crosses_blocked_cell,
        "the non-final hop must keep its reachable corner while routing around the blocker, \
         ok={ok} path={path:?} corner={corner:?} blocked={blocked_cell:?} \
         unpassable_segments={blocked_segments:?}"
    );
}

#[test]
fn live_corner_refreshes_the_extra_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("LiveExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("LiveExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("LiveExtraInf", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.movement.path = vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
            Vec3::new(90.0, 0.0, 0.0),
            Vec3::new(130.0, 0.0, 0.0),
        ];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(Vec3::new(10.0, 0.0, 0.0));
        unit.movement.max_speed = 0.0;
        unit.path_extra_distance = 0.0;
        unit.set_status_moving(true);
        unit.set_ai_state(
            crate::game_logic::AIState::Moving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert_eq!(unit.movement.current_path_index, 2);
    assert!(
        (unit.path_extra_distance - 80.0).abs() < 0.01,
        "live corner must keep the next segment plus the one after, extra={}",
        unit.path_extra_distance
    );
}

#[test]
fn click_move_sets_follow_path_extra_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("ClickExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ClickExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("ClickExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.path_extra_distance = 100.0;
        unit.is_attack_path = true;
        unit.is_exact_path = true;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.move_object_with_pathfinding_for_test(id, Vec3::new(200.0, 0.0, 0.0), None);
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.is_attack_path, "a click move is not a contact attack");
    assert!(
        !unit.is_exact_path,
        "a click move is not an exact waypoint path"
    );
    let path = &unit.movement.path;
    let index = unit.movement.current_path_index;
    let expected = if index + 1 < path.len() {
        let goal = path[index];
        let next = path[index + 1];
        let dx = next.x - goal.x;
        let dz = next.z - goal.z;
        let mut seg = (dx * dx + dz * dz).sqrt();
        if index + 2 < path.len() {
            seg += 40.0;
        }
        seg
    } else {
        0.0
    };
    assert!(
        (unit.path_extra_distance - expected).abs() < 0.01,
        "click move must replace a leftover contact 100, extra={} expected={expected} path={path:?}",
        unit.path_extra_distance
    );
}

#[test]
fn aircraft_quick_path_restores_a_false_adjust_flag() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("AdjustJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("AdjustJet".into(), jet_t);
    let id = logic
        .create_object_for_player("AdjustJet", 0, Vec3::new(0.0, 30.0, 0.0))
        .expect("jet");
    {
        let jet = logic.host_object_mut(id).expect("jet");
        jet.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        jet.loco_appearance = crate::game_logic::LocomotorAppearance::Wings;
    }
    logic.pathfinding_system.set_adjust_goal(false);
    assert!(logic.assign_unit_path_for_test(id, Vec3::new(80.0, 30.0, 0.0), &[]));
    assert!(
        !logic.pathfinding_system.adjusts_goal(),
        "aircraft quick path must restore the caller's false adjust flag"
    );
}

#[test]
fn queued_waypoint_drops_the_exact_path_formula() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("QueueExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("QueueExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("QueueExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.movement.path = vec![Vec3::ZERO, Vec3::new(40.0, 0.0, 0.0)];
        unit.movement.current_path_index = 1;
        unit.is_exact_path = true;
        unit.path_extra_distance = 100.0;
        unit.set_status_moving(true);
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.append_unit_waypoint_for_test(id, Vec3::new(120.0, 0.0, 0.0)));
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.is_exact_path);
    let path = &unit.movement.path;
    let index = unit.movement.current_path_index;
    let expected = if index + 1 < path.len() {
        let goal = path[index];
        let next = path[index + 1];
        let dx = next.x - goal.x;
        let dz = next.z - goal.z;
        let mut seg = (dx * dx + dz * dz).sqrt();
        if index + 2 < path.len() {
            seg += 40.0;
        }
        seg
    } else {
        0.0
    };
    assert!(
        (unit.path_extra_distance - expected).abs() < 0.01,
        "queued waypoint must use the follow-path distance, extra={} expected={expected} path={path:?}",
        unit.path_extra_distance
    );
}

#[test]
fn residual_path_installs_drop_the_exact_path_formula() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("ResidualExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ResidualExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("ResidualExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.is_exact_path = true;
        unit.path_extra_distance = 100.0;
        unit.request_path(
            Vec3::new(30.0, 0.0, 0.0),
            Some(vec![
                Vec3::ZERO,
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(30.0, 0.0, 0.0),
            ]),
        );
        assert!(!unit.is_exact_path);
        assert_eq!(unit.movement.current_path_index, 0);
        assert_eq!(
            unit.movement.target_position,
            Some(Vec3::new(10.0, 0.0, 0.0))
        );
        assert!((unit.path_extra_distance - 50.0).abs() < 0.01);
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    {
        let unit = logic.host_object(id).expect("unit");
        assert_eq!(unit.movement.current_path_index, 1);
        assert_eq!(
            unit.movement.target_position,
            Some(Vec3::new(10.0, 0.0, 0.0))
        );
        assert!(
            unit.get_position().x >= 0.0,
            "movement did not jump behind the path start"
        );
    }
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.is_exact_path = true;
        unit.path_extra_distance = 100.0;
        unit.apply_move_away_path(
            id,
            &[
                Vec3::ZERO,
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(50.0, 0.0, 0.0),
            ],
        );
        assert!(!unit.is_exact_path);
        assert!((unit.path_extra_distance - 30.0).abs() < 0.01);
    }
}

#[test]
fn projectile_precise_z_starts_on_the_last_segment() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut shell_t = ThingTemplate::new("Shell");
    shell_t.add_kind_of(KindOf::Projectile).set_health(1.0);
    logic.templates.insert("Shell".into(), shell_t);
    let id = logic
        .create_object_for_player("Shell", 0, Vec3::ZERO)
        .expect("shell");
    {
        let shell = logic.host_object_mut(id).expect("shell");
        shell.movement.path = vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(30.0, 0.0, 0.0),
        ];
        shell.movement.current_path_index = 1;
        shell.refresh_follow_path_extra_distance();
        assert!(!shell.precise_z_pos);
        shell.movement.current_path_index = 2;
        shell.refresh_follow_path_extra_distance();
        assert!(shell.precise_z_pos);
        shell.stop_moving(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
        assert!(!shell.precise_z_pos);
    }
}

#[test]
fn extra_distance_uses_the_easy_goal_metric() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("EasyDistInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("EasyDistInf".into(), inf_t);
    let id = logic
        .create_object_for_player("EasyDistInf", 0, Vec3::ZERO)
        .expect("unit");
    let here = Vec3::ZERO;
    let middle = Vec3::new(20.0, 0.0, 0.0);
    let last = Vec3::new(20.0, 0.0, 20.0);
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.movement.path = vec![here, middle, last];
        unit.movement.current_path_index = 1;
        unit.path_extra_distance = 0.0;
    }
    logic.register_ground_path_goal(id, last);
    let unit = logic.host_object_mut(id).expect("unit");
    let strict = unit.host_locomotor_distance_to_goal(here, last);
    unit.path_extra_distance = 80.0;
    let easy = unit.host_locomotor_distance_to_goal(here, last);
    assert!(
        (strict - 40.0).abs() < 0.01,
        "ground locomotion measures the routed path to the last node, got {strict}"
    );
    assert!(
        easy < strict,
        "extra distance must be the shorter flight to the last node, strict={strict} easy={easy}"
    );
}

#[test]
fn attack_path_request_preserves_the_exact_path_flag() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("AttackReqInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackReqInf".into(), inf_t);
    let id = logic
        .create_object_for_player("AttackReqInf", 0, Vec3::ZERO)
        .expect("unit");
    let unit = logic.host_object_mut(id).expect("unit");
    unit.is_exact_path = true;
    assert!(unit.begin_request_attack_path(None, Vec3::new(40.0, 0.0, 0.0), 10));
    assert!(unit.is_attack_path);
    assert!(
        unit.is_exact_path,
        "CPP requestAttackPath changes attack-path state but preserves the exact-path flag"
    );
}

#[test]
fn saved_object_keeps_path_extra_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("SaveExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("SaveExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("SaveExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    let envelope = {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.path_extra_distance = 80.0;
        unit.is_exact_path = true;
        unit.is_attack_path = false;
        unit.entity_lifecycle_envelope()
            .expect("valid lifecycle capture")
    };
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.path_extra_distance = 0.0;
        unit.is_exact_path = false;
        unit.is_attack_path = true;
        unit.entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        assert!((unit.path_extra_distance - 80.0).abs() < 0.01);
        assert!(unit.is_exact_path);
        assert!(!unit.is_attack_path);
    }
}

#[test]
fn live_arrival_clears_the_ignored_obstacle() {
    use crate::game_logic::{KindOf, ObjectId, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("ArriveInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ArriveInf".into(), inf_t);
    let goal = Vec3::new(10.0, 0.0, 0.0);
    let id = logic
        .create_object_for_player("ArriveInf", 0, goal)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.movement.path = vec![Vec3::ZERO, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        unit.movement.max_speed = 0.0;
        unit.ignored_obstacle_id = Some(ObjectId(9));
        unit.queue_for_path_frames = 60;
        unit.set_status_moving(true);
        unit.set_ai_state(
            crate::game_logic::AIState::Moving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert!(unit.movement.path.is_empty() || !unit.status.moving);
    assert_eq!(unit.ignored_obstacle_id, None);
    assert_eq!(unit.queue_for_path_frames, 0);
}

#[test]
fn live_air_hold_clears_the_ignore_and_the_queue() {
    use crate::game_logic::{KindOf, LocomotorAppearance, ObjectId, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("HoldJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("HoldJet".into(), jet_t);
    let goal = Vec3::new(10.0, 20.0, 0.0);
    let id = logic
        .create_object_for_player("HoldJet", 0, goal)
        .expect("jet");
    {
        let unit = logic.host_object_mut(id).expect("jet");
        unit.loco_appearance = LocomotorAppearance::Wings;
        unit.movement.path = vec![Vec3::ZERO, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        unit.movement.max_speed = 0.0;
        unit.ignored_obstacle_id = Some(ObjectId(9));
        unit.queue_for_path_frames = 60;
        unit.can_path_through_units = true;
        unit.set_precise_z_pos(true);
        unit.locomotor_goal_type = crate::game_logic::LocoGoalType::PositionOnPath;
        unit.set_status_moving(true);
        unit.set_ai_state(
            crate::game_logic::AIState::Moving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("jet");
    assert!(unit.movement.path.is_empty());
    assert_eq!(unit.ignored_obstacle_id, None);
    assert_eq!(unit.queue_for_path_frames, 0);
    assert!(!unit.can_path_through_units);
    assert!(!unit.precise_z_pos);
    assert!(!unit.status.moving);
    assert_eq!(
        unit.locomotor_goal_type,
        crate::game_logic::LocoGoalType::None
    );
}

#[test]
fn far_arrival_snaps_the_current_cell_not_the_goal() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("FarArrive");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("FarArrive".into(), inf_t);
    let here = Vec3::ZERO;
    let goal = Vec3::new(20.0, 0.0, 0.0);
    let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
        8.0,
        logic.pathfinding_system.grid.grid_size(),
    );
    let stand = logic
        .pathfinding_system
        .grid
        .cell_for_unit_position(here, center);
    logic.pathfinding_system.grid.set_cell_obstacle_owned(
        GridPos::new(stand.x, stand.y),
        false,
        false,
        9,
        None,
        None,
    );
    let id = logic
        .create_object_for_player("FarArrive", 0, here)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.selection_radius = 8.0;
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.movement.path = vec![here, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        unit.movement.max_speed = 0.0;
        unit.close_enough_dist = Some(30.0);
        unit.final_position = Vec3::new(999.0, 0.0, 999.0);
        unit.do_final_position = true;
        unit.set_status_moving(true);
        unit.set_ai_state(
            crate::game_logic::AIState::Moving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.register_ground_path_goal(id, goal);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.do_final_position);
    let planted = logic
        .pathfinding_system
        .grid
        .world_to_grid(unit.final_position);
    assert_eq!(
        planted, stand,
        "snapPosition stays on the blocked stand cell, planted={planted:?} stand={stand:?}"
    );
}

#[test]
fn close_arrival_plants_the_goal_cell_not_the_raw_point() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("CloseArrive");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("CloseArrive".into(), inf_t);
    let here = Vec3::ZERO;
    let raw = Vec3::new(2.0, 7.0, 1.0);
    let id = logic
        .create_object_for_player("CloseArrive", 0, here)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.selection_radius = 8.0;
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.movement.path = vec![here, raw];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(raw);
        unit.movement.max_speed = 0.0;
        unit.close_enough_dist = Some(30.0);
        unit.final_position = Vec3::new(999.0, 0.0, 999.0);
        unit.do_final_position = true;
        unit.set_status_moving(true);
        unit.set_ai_state(
            crate::game_logic::AIState::Moving,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.register_ground_path_goal(id, raw);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.do_final_position);
    let grid = &logic.pathfinding_system.grid;
    let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(8.0, grid.grid_size());
    let registered_goal = GridPos::new(unit.pathfind_goal_cell.0, unit.pathfind_goal_cell.1);
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
            grid,
        );
        grid.adjust_coord_to_ground_cell(registered_goal, center, &ground_source)
    });
    let planted = unit.final_position;
    assert!(
        (planted.x - expected.x).abs() < 0.01 && (planted.z - expected.z).abs() < 0.01,
        "close arrival must plant the goal cell, planted={planted:?} expected={expected:?} raw={raw:?}"
    );
    assert!(
        (planted.y - expected.y).abs() < 0.01,
        "keep the cell height, planted={planted:?} expected={expected:?}"
    );
    assert!(
        (planted.x - raw.x).abs() > 0.01 || (planted.z - raw.z).abs() > 0.01,
        "the raw waypoint is not the cell point"
    );
}
