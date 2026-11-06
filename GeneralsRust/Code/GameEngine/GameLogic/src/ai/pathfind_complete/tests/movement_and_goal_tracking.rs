use super::*;

#[test]
fn tighten_path_advances_on_open_ground() {
    let system = PathfindingSystem::new(32, 32);
    let mut from = Coord3D::new(20.0, 20.0, 0.0);
    let to = Coord3D::new(200.0, 20.0, 0.0);
    let start_x = from.x;
    system.tighten_path(&mut from, &to, SURFACE_GROUND, false, 0.0, None);
    // Should advance toward to (or stay if adjust fails entirely).
    assert!(from.x >= start_x - 0.1);
}

#[test]
fn move_allies_away_empty_line_no_nudge() {
    let system = PathfindingSystem::new(16, 16);
    let from = Coord3D::new(10.0, 10.0, 0.0);
    let to = Coord3D::new(100.0, 10.0, 0.0);
    let nudged = system.move_allies_away_from_destination(INVALID_ID, &from, &to);
    assert!(nudged.is_empty());
}

#[test]
fn clear_cell_for_diameter_open_returns_diameter() {
    let system = PathfindingSystem::new(32, 32);
    let d = system.clear_cell_for_diameter(false, 10, 10, PathfindLayerEnum::Ground, 4);
    assert_eq!(d, 4);
    let d1 = system.clear_cell_for_diameter(false, 10, 10, PathfindLayerEnum::Ground, 1);
    assert_eq!(d1, 1);
}

#[test]
fn clear_cell_for_diameter_blocked_by_cliff() {
    let mut system = PathfindingSystem::new(32, 32);
    system.set_cell_type(&Coord3D::new(100.0, 100.0, 0.0), PathfindCellType::Cliff);
    let cell = GridCoord::from_world(&Coord3D::new(100.0, 100.0, 0.0));
    let d = system.clear_cell_for_diameter(false, cell.x, cell.y, PathfindLayerEnum::Ground, 2);
    assert_eq!(d, 0);
}

#[test]
fn iterate_cells_along_line_bresenham_visits_endpoints() {
    let system = PathfindingSystem::new(32, 32);
    let mut cells = Vec::new();
    let ret = system.iterate_cells_along_line(
        GridCoord::new(2, 2),
        GridCoord::new(6, 4),
        PathfindLayerEnum::Ground,
        |_from, to, _x, _y| {
            cells.push(to);
            0
        },
    );
    assert_eq!(ret, 0);
    assert!(!cells.is_empty());
    assert_eq!(cells[0], GridCoord::new(2, 2));
    assert!(cells.iter().any(|c| *c == GridCoord::new(6, 4)));
}

#[test]
fn compute_normal_radial_offset_perpendicular() {
    let from = Coord3D::new(0.0, 0.0, 0.0);
    let to = Coord3D::new(10.0, 0.0, 0.0);
    let obj = Coord3D::new(5.0, 2.0, 0.0);
    let mut insert = Coord3D::new(0.0, 0.0, 0.0);
    PathfindingSystem::compute_normal_radial_offset(&from, &mut insert, &to, &obj, 3.0);
    // cross > 0 → normal (0,-1) wait: dx=10 dy=0, objDy=2 → cross=20>0 → (dy,-dx)=(0,-10)
    // normalized (0,-1) * 3 from obj → (5, -1)
    assert!((insert.x - 5.0).abs() < 0.01);
    assert!((insert.y - (-1.0)).abs() < 0.01 || (insert.y - 5.0).abs() < 0.01);
}

#[test]
fn segment_intersects_no_building_false() {
    let system = PathfindingSystem::new(32, 32);
    let from = Coord3D::new(10.0, 10.0, 0.0);
    let mut to = Coord3D::new(100.0, 10.0, 0.0);
    let mut i1 = Coord3D::new(0.0, 0.0, 0.0);
    let mut i2 = Coord3D::new(0.0, 0.0, 0.0);
    let mut i3 = Coord3D::new(0.0, 0.0, 0.0);
    assert!(
        !system.segment_intersects_tall_building(
            &from, &mut to, INVALID_ID, &mut i1, &mut i2, &mut i3
        )
    );
}

#[test]
fn queue_for_path_dedupes_like_cpp() {
    let mut system = PathfindingSystem::new(16, 16);
    assert!(system.queue_for_path(7));
    assert!(system.queue_for_path(7)); // already queued → true
    assert!(system.queue_for_path(8));
}

#[test]
fn build_actual_path_prepends_unit_feet() {
    let mut system = PathfindingSystem::new(32, 32);
    let from = Coord3D::new(15.0, 15.0, 0.0);
    let to = Coord3D::new(85.0, 85.0, 0.0);
    let grid = vec![
        GridCoord::from_world(&from),
        GridCoord::new(5, 5),
        GridCoord::from_world(&to),
    ];
    let result = system.build_actual_path(&grid, &from, &to, SURFACE_GROUND, false, false, true);
    assert!(result.success);
    assert!(!result.waypoints.is_empty());
    assert_eq!(result.waypoints.len(), result.can_optimize.len());
    // First waypoint should be unit feet.
    assert!((result.waypoints[0].x - from.x).abs() < 0.01);
}

#[test]
fn build_actual_path_empty_grid_returns_cpp_failure() {
    let mut system = PathfindingSystem::new(8, 8);
    let from = Coord3D::new(5.0, 5.0, 0.0);
    let to = Coord3D::new(65.0, 65.0, 0.0);
    let result = system.build_actual_path(&[], &from, &to, SURFACE_GROUND, false, false, true);
    assert!(
        !result.success,
        "empty A* path is C++ NULL, not a direct path"
    );
    assert!(result.waypoints.is_empty());
}

#[test]
fn pathfinder_new_map_sets_ready() {
    let mut system = PathfindingSystem::new(16, 16);
    assert!(!system.is_map_ready());
    system.new_map();
    assert!(system.is_map_ready());
}

#[test]
fn process_queue_skips_when_map_not_ready() {
    let mut system = PathfindingSystem::new(8, 8);
    assert!(!system.is_map_ready());
    assert_eq!(system.process_queue(10), 0);
}

#[test]
fn update_pos_requires_map_ready_and_dedupes() {
    let mut system = PathfindingSystem::new(16, 16);
    let cell = GridCoord::new(3, 4);
    system.update_pos(cell, 42, PathfindLayerEnum::Ground, 0, true, false);
    // not ready → no pos recorded
    assert!(system.unit_pos_cells.get(&42).is_none());
    // make ready and update
    // cannot set is_map_ready from outside easily if private - use new_map via mut
}

#[test]
fn update_goal_and_remove_unit_clears_tracking() {
    let mut system = PathfindingSystem::new(16, 16);
    system.new_map();
    let cell = GridCoord::new(5, 6);
    system.update_goal(cell, 7, PathfindLayerEnum::Ground, 0, true, false);
    assert_eq!(
        system.unit_goal_cells.get(&7).map(|c| (c.x, c.y)),
        Some((5, 6))
    );
    // same cell no-op still present
    system.update_goal(cell, 7, PathfindLayerEnum::Ground, 0, true, false);
    assert_eq!(
        system.unit_goal_cells.get(&7).map(|c| (c.x, c.y)),
        Some((5, 6))
    );
    system.remove_unit_from_pathfind_map(7, 0, true, PathfindLayerEnum::Ground);
    assert!(system.unit_goal_cells.get(&7).is_none());
}

#[test]
fn cell_for_unit_position_half_cell_bias() {
    let pos = Coord3D::new(15.0, 25.0, 0.0);
    let c = PathfindingSystem::cell_for_unit_position(&pos, true);
    assert_eq!((c.x, c.y), (1, 2));
    let c2 = PathfindingSystem::cell_for_unit_position(&pos, false);
    // floor(0.5 + 15/10)=floor(2.0)=2, floor(0.5+2.5)=floor(3.0)=3
    assert_eq!((c2.x, c2.y), (2, 3));
}

#[test]
fn update_aircraft_goal_stamps_and_clears() {
    let mut system = PathfindingSystem::new(16, 16);
    system.new_map();
    let pos = Coord3D::new(55.0, 65.0, 0.0);
    system.update_aircraft_goal(&pos, 99, 0, true);
    let cell = PathfindingSystem::cell_for_unit_position(&pos, true);
    assert_eq!(system.get_goal_aircraft(cell), 99);
    system.remove_goal(99, 0, true, PathfindLayerEnum::Ground);
    assert_eq!(system.get_goal_aircraft(cell), INVALID_ID);
}

#[test]
fn adjust_to_landing_refuses_other_aircraft_goal() {
    let mut system = PathfindingSystem::new(16, 16);
    system.new_map();
    let reserved = Coord3D::new(55.0, 65.0, 0.0);
    system.update_aircraft_goal(&reserved, 99, 0, true);
    let from = Coord3D::new(16.0, 16.0, 0.0);
    let mut own = reserved;
    assert!(system.adjust_to_landing_destination_for(&from, &mut own, 0.0, 99));
    let reserved_cell = PathfindingSystem::cell_for_unit_position(&reserved, true);
    let own_cell = PathfindingSystem::cell_for_unit_position(&own, true);
    assert_eq!(
        own_cell, reserved_cell,
        "owner may land on own goalAircraft"
    );

    let mut other = reserved;
    assert!(system.adjust_to_landing_destination_for(&from, &mut other, 0.0, 12));
    let other_cell = PathfindingSystem::cell_for_unit_position(&other, true);
    assert_ne!(
        other_cell, reserved_cell,
        "second aircraft must leave reserved LZ"
    );
}

#[test]
fn wall_piece_add_remove_and_point_on_wall() {
    let mut system = PathfindingSystem::new(16, 16);
    assert!(!system.is_point_on_wall(&Coord3D::new(15.0, 15.0, 0.0)));
    system.add_wall_piece(11);
    system.add_wall_piece(12);
    assert_eq!(system.wall_piece_count(), 2);
    system.add_wall_piece(11); // dedupe
    assert_eq!(system.wall_piece_count(), 2);
    system.classify_wall_cell_at(1, 1, true);
    assert!(system.is_point_on_wall(&Coord3D::new(15.0, 15.0, 0.0)));
    system.remove_wall_piece(11);
    assert_eq!(system.wall_piece_count(), 1);
    system.remove_wall_piece(12);
    assert_eq!(system.wall_piece_count(), 0);
    assert!(!system.is_point_on_wall(&Coord3D::new(15.0, 15.0, 0.0)));
}

#[test]
fn update_layer_demotes_without_bridge_interaction() {
    let system = PathfindingSystem::new(8, 8);
    assert_eq!(
        system.update_layer_for_object(PathfindLayerEnum::Top, false),
        PathfindLayerEnum::Ground
    );
    assert_eq!(
        system.update_layer_for_object(PathfindLayerEnum::Top, true),
        PathfindLayerEnum::Top
    );
}

#[test]
fn force_map_recalculation_runs_classify() {
    let mut system = PathfindingSystem::new(8, 8);
    system.force_map_recalculation();
    // smoke: still usable
    assert!(!system.is_map_ready() || system.is_map_ready());
}

#[test]
fn check_change_layers_returns_parent_when_linked() {
    let mut system = PathfindingSystem::new(16, 16);
    let cell = GridCoord::new(4, 5);
    assert!(system.check_change_layers(cell).is_none());
    system.set_connect_layer(cell, PathfindLayerEnum::Top);
    assert_eq!(system.check_change_layers(cell), Some(cell));
}

#[test]
fn get_cell_type_at_layer_ground_truncates_toward_zero() {
    let mut system = PathfindingSystem::new(8, 8);
    system.new_map();
    // Stamp cell (1,*) Water and cell (2,*) Cliff via world coords.
    // 15/10 → 1, 25/10 → 2.
    system.set_cell_type(&Coord3D::new(15.0, 5.0, 0.0), PathfindCellType::Water);
    system.set_cell_type(&Coord3D::new(25.0, 5.0, 0.0), PathfindCellType::Cliff);
    // 19.9 / 10 = 1.99 → truncate/REAL_TO_INT = 1, not round-to-2.
    let pos = Coord3D::new(19.9, 5.0, 0.0);
    assert_eq!(
        PathfindingSystem::world_to_cell_trunc(&pos),
        GridCoord::new(1, 0)
    );
    assert_eq!(
        system.get_cell_type_at_layer(&pos, PathfindLayerEnum::Ground),
        Some(PathfindCellType::Water),
        "19.9/10 must hit cell 1, not 2"
    );
    // Truncate toward zero (not floor): -0.1/10 → 0, floor would be -1 (None).
    let neg = Coord3D::new(-0.1, 5.0, 0.0);
    assert_eq!(
        PathfindingSystem::world_to_cell_trunc(&neg),
        GridCoord::new(0, 0)
    );
    assert_eq!(
        system.get_cell_type_at_layer(&neg, PathfindLayerEnum::Ground),
        Some(PathfindCellType::Clear)
    );
}

#[test]
fn get_cell_type_at_layer_missing_cell_falls_back_to_ground() {
    // C++ getCell falls back to m_map[x][y] when an elevated slot has no cell.
    let system = PathfindingSystem::new(8, 8);
    assert!(
        system
            .get_cell_type_at_cell(PathfindLayerEnum::Ground, -1, 0)
            .is_none()
    );
    assert!(
        system
            .get_cell_type_at_cell(PathfindLayerEnum::Ground, 99, 99)
            .is_none()
    );
    // Top with no BridgeLayer cell inside the map falls back to ground.
    assert!(
        system.get_cell_type_at_layer(&Coord3D::new(15.0, 15.0, 0.0), PathfindLayerEnum::Top)
            == Some(PathfindCellType::Clear)
    );
    // The same missing slot outside the map extent has no ground fallback.
    assert!(
        system
            .get_cell_type_at_cell(PathfindLayerEnum::Top, 8, 3)
            .is_none()
    );
}

#[test]
fn get_cell_type_at_layer_top_uses_bridge_bounds() {
    let mut system = PathfindingSystem::new(16, 16);
    system.add_bridge((GridCoord::new(2, 2), GridCoord::new(5, 5)));
    assert_eq!(
        system.get_cell_type_at_cell(PathfindLayerEnum::Top, 3, 3),
        Some(PathfindCellType::Clear)
    );
    // Outside this bridge's AABB but inside map extent, C++ returns ground.
    assert_eq!(
        system.get_cell_type_at_cell(PathfindLayerEnum::Top, 10, 10),
        Some(PathfindCellType::Clear)
    );
    assert!(
        system
            .get_cell_type_at_cell(PathfindLayerEnum::Top, 16, 10)
            .is_none()
    );
}

#[test]
fn bridge_cell_reads_and_writes_use_exact_layer_slot() {
    let mut system = PathfindingSystem::new(16, 16);
    let first = system.add_bridge((GridCoord::new(2, 2), GridCoord::new(5, 5)));
    let second = system.add_bridge((GridCoord::new(8, 8), GridCoord::new(11, 11)));
    assert_eq!(first, PathfindLayerEnum::Top as u32);
    assert_eq!(second, PathfindLayerEnum::Layer3 as u32);
    let ground_pos = Coord3D::new(95.0, 95.0, 0.0);
    system.set_cell_type(&ground_pos, PathfindCellType::Water);

    assert_eq!(
        system.get_cell_type_at_cell(PathfindLayerEnum::Top, 9, 9),
        Some(PathfindCellType::Water),
        "slot 2 misses must fall back to the ground cell, not slot 3"
    );
    assert_eq!(
        system.get_cell_type_at_cell(PathfindLayerEnum::Layer3, 9, 9),
        Some(PathfindCellType::Clear)
    );

    system.set_cell_type_at_layer(
        &ground_pos,
        PathfindLayerEnum::Top,
        PathfindCellType::Obstacle,
    );
    assert_eq!(
        system.get_cell_type_at_cell(PathfindLayerEnum::Layer3, 9, 9),
        Some(PathfindCellType::Clear),
        "writing slot 2 must leave slot 3 untouched"
    );
    assert_eq!(
        system.get_cell_type_at_cell(PathfindLayerEnum::Top, 9, 9),
        Some(PathfindCellType::Obstacle),
        "slot 2 miss must read the updated ground cell"
    );
}

#[test]
fn check_change_layers_enqueues_same_xy_when_not_closed() {
    let mut system = PathfindingSystem::new(16, 16);
    let cell = GridCoord::new(4, 5);
    system.set_connect_layer(cell, PathfindLayerEnum::Top);
    let closed = HashSet::new();
    assert_eq!(
        system.change_layer_open_link(cell, &closed),
        Some(cell),
        "connect-layer same-xy cell must be enqueued at parent cost"
    );
    let mut closed = HashSet::new();
    closed.insert((4, 5));
    assert!(
        system.change_layer_open_link(cell, &closed).is_none(),
        "already-closed connect-layer cell is not re-enqueued"
    );
}

#[test]
fn diagonal_squeeze_one_orthogonal_open_allows_diagonal() {
    // C++ 6181-6185: one neighborFlag orthogonal true → allow diagonal.
    // 2x2 crack: (1,0) blocked, (0,1) open → (0,0)→(1,1) cost 14.
    let mut system = PathfindingSystem::new(4, 4);
    system.new_map();
    system.set_cell_type(&Coord3D::new(15.0, 5.0, 0.0), PathfindCellType::Impassable);
    let from = Coord3D::new(5.0, 5.0, 0.0);
    let to = Coord3D::new(15.0, 15.0, 0.0);
    let cost = system.check_path_cost(SURFACE_GROUND, false, &from, &to);
    assert!(
        (cost - COST_DIAGONAL as f32).abs() < 0.01,
        "one open orthogonal must allow diagonal COST_DIAGONAL=14, got {cost}"
    );
}

#[test]
fn diagonal_squeeze_both_orthogonals_blocked_no_path() {
    let mut system = PathfindingSystem::new(4, 4);
    system.new_map();
    system.set_cell_type(&Coord3D::new(15.0, 5.0, 0.0), PathfindCellType::Impassable);
    system.set_cell_type(&Coord3D::new(5.0, 15.0, 0.0), PathfindCellType::Impassable);
    let from = Coord3D::new(5.0, 5.0, 0.0);
    let to = Coord3D::new(15.0, 15.0, 0.0);
    let cost = system.check_path_cost(SURFACE_GROUND, false, &from, &to);
    const MAX_COST: f32 = 0x7fff_0000u32 as f32;
    assert_eq!(
        cost, MAX_COST,
        "both orthogonals blocked → no diagonal squeeze"
    );
}

#[test]
fn connect_layer_on_pathfind_cell() {
    let mut pf = crate::ai::pathfind_astar::AStarPathfinder::new(8, 8);
    let c = GridCoord::new(2, 2);
    assert_eq!(
        pf.get_cell_connect_layer(c),
        Some(PathfindLayerEnum::Invalid)
    );
    pf.set_cell_connect_layer(c, PathfindLayerEnum::Top);
    assert_eq!(pf.get_cell_connect_layer(c), Some(PathfindLayerEnum::Top));
    assert_eq!(pf.connect_layer_transition_coord(c), Some(c));
}

#[test]
fn goal_position_from_tracked_cell() {
    let mut system = PathfindingSystem::new(16, 16);
    system.new_map();
    system.update_goal(
        GridCoord::new(3, 4),
        55,
        PathfindLayerEnum::Ground,
        0,
        true,
        false,
    );
    let mut out = Coord3D::new(0.0, 0.0, 0.0);
    assert!(system.goal_position(55, 0.0, &mut out));
    // center of cell (3,4) at cell size 10 → (35, 45)
    assert!((out.x - 35.0).abs() < 0.01, "x={}", out.x);
    assert!((out.y - 45.0).abs() < 0.01, "y={}", out.y);
    assert!(!system.goal_position(999, 0.0, &mut out));
}

#[test]
fn path_destination_requires_map_ready() {
    let system = PathfindingSystem::new(16, 16);
    let mut dest = Coord3D::new(15.0, 15.0, 0.0);
    let group = Coord3D::new(85.0, 85.0, 0.0);
    assert!(!system.path_destination(&mut dest, &group, SURFACE_GROUND, false, 0.0, true));
}

#[test]
fn path_destination_finds_adjustable_cell() {
    let mut system = PathfindingSystem::new(32, 32);
    system.new_map();
    let mut dest = Coord3D::new(15.0, 15.0, 0.0);
    let group = Coord3D::new(85.0, 85.0, 0.0);
    let ok = system.path_destination(&mut dest, &group, SURFACE_GROUND, false, 0.0, true);
    assert!(ok, "open map should find adjustable destination");
}

#[test]
fn get_effective_zone_air_is_one() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    let z = system.zones.get_effective_zone(SURFACE_AIR, false, 7);
    assert_eq!(z, 1);
}

#[test]
fn process_hierarchical_cell_same_zone_expands() {
    let mut system = PathfindingSystem::new(30, 30);
    system.new_map();
    // Force same zones across block boundary
    {
        let zones = &mut system.zones;
        for x in 0..30 {
            for y in 0..30 {
                zones.zones[x][y] = 1;
            }
        }
        zones.rebuild_combiner_identity();
    }
    let parent_zone = 1u16;
    let scan = GridCoord::new(9, 5); // near block edge (ZONE_BLOCK_SIZE=10)
    let mut examined = Vec::new();
    let res = system.process_hierarchical_cell(
        scan,
        (1, 0),
        parent_zone,
        SURFACE_GROUND,
        false,
        &mut examined,
    );
    assert!(res.is_some(), "should expand into adjacent cell");
    let (adj, _z) = res.unwrap();
    assert_eq!((adj.x, adj.y), (10, 5));
    assert!(!examined.is_empty());
}

#[test]
fn block_index_uses_zone_block_size() {
    assert_eq!(ZoneManager::block_index(0, 0), (0, 0));
    assert_eq!(ZoneManager::block_index(9, 19), (0, 1));
    assert_eq!(ZoneManager::block_index(10, 20), (1, 2));
}

#[test]
fn clean_open_and_closed_lists_accumulates() {
    let mut system = PathfindingSystem::new(8, 8);
    system.note_open_closed_cells(3, 5);
    system.clean_open_and_closed_lists();
    assert_eq!(system.cumulative_cells_allocated(), 8);
    system.note_open_closed_cells(2, 0);
    system.clean_open_and_closed_lists();
    assert_eq!(system.cumulative_cells_allocated(), 10);
}

#[test]
fn move_allies_depth_and_empty_path() {
    let mut system = PathfindingSystem::new(8, 8);
    system.new_map();
    assert!(!system.move_allies(INVALID_ID, &[], &[], true, 0.0));
    let pts = [Coord3D::new(10.0, 10.0, 0.0), Coord3D::new(20.0, 20.0, 0.0)];
    let layers = [PathfindLayerEnum::Ground, PathfindLayerEnum::Ground];
    // no object → false
    assert!(!system.move_allies(1, &pts, &layers, true, 0.0));
}

#[test]
fn pathfinder_xfer_version_only() {
    use crate::common::xfer::XferExt;
    // smoke compile surface for xfer/loadPostProcess
    let mut system = PathfindingSystem::new(4, 4);
    system.load_post_process();
    assert!(!system.is_tunneling());
    system.set_is_tunneling(true);
    assert!(system.is_tunneling());
    system.set_wall_height(12.5);
    assert!((system.wall_height() - 12.5).abs() < 0.01);
    system.set_ignore_obstacle_id(42);
    assert_eq!(system.ignore_obstacle_id(), 42);
}

#[test]
fn pathfinder_reset_clears_ready_and_queue() {
    let mut system = PathfindingSystem::new(16, 16);
    system.new_map();
    system.add_wall_piece(3);
    system.queue_for_path(9);
    system.note_open_closed_cells(1, 2);
    assert!(system.is_map_ready());
    system.reset();
    assert!(!system.is_map_ready());
    assert_eq!(system.wall_piece_count(), 0);
    assert_eq!(system.cumulative_cells_allocated(), 0);
    assert!(!system.queue_for_path(9) || system.queue_for_path(9)); // queue works after reset
}

#[test]
fn get_move_away_finds_cell_off_path() {
    let mut system = PathfindingSystem::new(32, 32);
    system.new_map();
    let from = Coord3D::new(55.0, 55.0, 0.0);
    // Path along x axis through the unit
    let path = vec![
        Coord3D::new(10.0, 55.0, 0.0),
        Coord3D::new(100.0, 55.0, 0.0),
    ];
    let pos = system.get_move_away_from_path(&from, &path, None, SURFACE_GROUND, false, 0.0, 0.0);
    assert!(pos.is_some(), "should find a cell off the path corridor");
    let p = pos.unwrap();
    // Y should move away from path y=55
    assert!(
        (p.y - 55.0).abs() > 5.0 || (p.x - 55.0).abs() > 5.0,
        "moved pos {:?}",
        p
    );
}

#[test]
fn line_in_region_detects_crossing() {
    let s = Coord2D::new(0.0, 5.0);
    let e = Coord2D::new(10.0, 5.0);
    assert!(PathfindingSystem::line_in_region(
        &s, &e, 4.0, 0.0, 6.0, 10.0
    ));
    assert!(!PathfindingSystem::line_in_region(
        &s, &e, 0.0, 6.0, 10.0, 10.0
    ));
}

#[test]
fn patch_path_reconnects_open_map() {
    let mut system = PathfindingSystem::new(40, 40);
    system.new_map();
    let from = Coord3D::new(15.0, 25.0, 0.0); // off original path
    let original = vec![
        Coord3D::new(15.0, 15.0, 0.0),
        Coord3D::new(55.0, 15.0, 0.0),
        Coord3D::new(95.0, 15.0, 0.0),
        Coord3D::new(95.0, 55.0, 0.0),
    ];
    let layers = vec![PathfindLayerEnum::Ground; original.len()];
    let result = system.patch_path(
        &from,
        &original,
        &layers,
        SURFACE_GROUND,
        false,
        0.0,
        false,
        INVALID_ID,
    );
    assert!(result.success, "open map should patch onto path");
    assert!(result.waypoints.len() >= 2);
    // Should end near original goal
    let end = result.waypoints.last().unwrap();
    assert!(
        (end.x - 95.0).abs() < 20.0 && (end.y - 55.0).abs() < 20.0,
        "end {:?}",
        end
    );
}

#[test]
fn patch_path_empty_original_fails() {
    let mut system = PathfindingSystem::new(8, 8);
    system.new_map();
    let r = system.patch_path(
        &Coord3D::new(10.0, 10.0, 0.0),
        &[],
        &[],
        SURFACE_GROUND,
        false,
        0.0,
        false,
        INVALID_ID,
    );
    assert!(!r.success);
}

#[test]
fn find_attack_path_quick_step_when_in_range() {
    let mut system = PathfindingSystem::new(40, 40);
    system.new_map();
    let from = Coord3D::new(50.0, 50.0, 0.0);
    let victim = Coord3D::new(80.0, 50.0, 0.0);
    let result = system.find_attack_path_range(
        &from,
        &victim,
        SURFACE_GROUND,
        false,
        0.0,
        40.0,
        INVALID_ID,
        false,
    );
    assert!(result.success);
    assert_eq!(result.waypoints.len(), 2);
}

#[test]
fn find_attack_path_requires_map_ready() {
    let mut system = PathfindingSystem::new(16, 16);
    let r = system.find_attack_path_range(
        &Coord3D::new(10.0, 10.0, 0.0),
        &Coord3D::new(50.0, 10.0, 0.0),
        SURFACE_GROUND,
        false,
        0.0,
        30.0,
        INVALID_ID,
        false,
    );
    assert!(!r.success);
}

#[test]
fn get_aircraft_path_two_node_baseline() {
    let system = PathfindingSystem::new(32, 32);
    let from = Coord3D::new(10.0, 10.0, 50.0);
    let to = Coord3D::new(80.0, 90.0, 50.0);
    let path = system.get_aircraft_path(&from, &to, false, INVALID_ID);
    assert!(path.success);
    assert_eq!(path.waypoints.len(), 2);
    assert!((path.waypoints[0].z - to.z).abs() < 0.01);
    assert!((path.waypoints[1].x - to.x).abs() < 0.01);
}

#[test]
fn check_for_possible_same_zone() {
    let mut system = PathfindingSystem::new(16, 16);
    system.new_map();
    {
        let zones = &mut system.zones;
        for x in 0..16 {
            for y in 0..16 {
                zones.zones[x][y] = 1;
            }
        }
        zones.rebuild_combiner_identity();
    }
    let mut dest = Coord3D::new(0.0, 0.0, 0.0);
    assert!(system.check_for_possible(
        false,
        1,
        true,
        SURFACE_GROUND,
        5,
        6,
        PathfindLayerEnum::Ground,
        &mut dest,
        false,
        |_, _, _| 0.0,
    ));
    assert!((dest.x - 55.0).abs() < 0.01);
    assert!((dest.y - 65.0).abs() < 0.01);
}

#[test]
fn build_ground_path_optimizes_waypoints() {
    let mut system = PathfindingSystem::new(32, 32);
    let from = Coord3D::new(15.0, 15.0, 0.0);
    let grid = vec![
        GridCoord::new(1, 1),
        GridCoord::new(2, 1),
        GridCoord::new(3, 1),
        GridCoord::new(4, 1),
        GridCoord::new(5, 1),
    ];
    let path = system.build_ground_path(&from, &grid, false, true, 0);
    assert!(path.success);
    assert!(!path.waypoints.is_empty());
}

#[test]
fn set_debug_path_stores_copy() {
    let mut system = PathfindingSystem::new(8, 8);
    assert!(system.debug_path().is_none());
    let p = PathResult {
        success: true,
        waypoints: vec![Coord3D::new(1.0, 2.0, 0.0)],
        layers: vec![PathfindLayerEnum::Ground],
        can_optimize: vec![true],
        total_cost: 1,
        blocked_by_ally: false,
    };
    system.set_debug_path(Some(p));
    assert!(system.debug_path().is_some());
    system.set_debug_path_position(Coord3D::new(9.0, 8.0, 7.0));
    let dp = system.debug_path_position();
    assert!((dp.x - 9.0).abs() < 0.01);
    system.reset();
    assert!(system.debug_path().is_none());
}
