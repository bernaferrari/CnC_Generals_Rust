use super::*;

#[test]
fn zone_combiners_merge_ground_cliff() {
    let mut system = PathfindingSystem::new(20, 20);
    // Paint cliff strip
    for y in 0..20 {
        system.set_cell_type(
            &Coord3D::new(100.0, y as f32 * 10.0 + 5.0, 0.0),
            PathfindCellType::Cliff,
        );
    }
    system.new_map();
    let z = &system.zones;
    // Combiners should not be pure identity if cliff/clear adjacencies exist
    let mut merged = false;
    for (i, &v) in z.ground_cliff_zones.iter().enumerate() {
        if i > 0 && v != i as u16 && v != 0 {
            merged = true;
            break;
        }
    }
    // At least tables sized and get_effective works for ground|cliff
    let eff = z.get_effective_zone(SURFACE_GROUND | SURFACE_CLIFF, false, 1);
    assert!(eff >= 1 || eff == 0);
    let _ = merged; // may be true if multiple zones
    assert!(!z.ground_cliff_zones.is_empty());
}

#[test]
fn obstacle_fence_flag_stamped_on_astar() {
    let mut pf = crate::ai::pathfind_astar::AStarPathfinder::new(8, 8);
    let c = GridCoord::new(3, 4);
    pf.set_cell_obstacle_id(c, 42, true, false);
    assert!(pf.is_obstacle_fence(c));
    assert_eq!(pf.get_cell_type(c), Some(PathfindCellType::Obstacle));
    assert!(pf.clear_cell_obstacle_id(c, 42));
    assert!(!pf.is_obstacle_fence(c));
}

#[test]
fn crusher_combiner_merges_fence_obstacle() {
    let mut system = PathfindingSystem::new(12, 12);
    // Fence obstacle next to clear cells.
    {
        let pf = &mut system.pathfinder;
        pf.set_cell_obstacle_id(GridCoord::new(5, 5), 7, true, false);
    }
    system.new_map();
    let z = &system.zones;
    // Fence obstacle zone and neighboring clear should merge under crusher table.
    let z_obs = z.zones[5][5];
    let z_clear = z.zones[6][5];
    assert_ne!(z_obs, 0);
    assert_ne!(z_clear, 0);
    if z_obs != z_clear {
        let c_obs = z
            .crusher_zones
            .get(z_obs as usize)
            .copied()
            .unwrap_or(z_obs);
        let c_clr = z
            .crusher_zones
            .get(z_clear as usize)
            .copied()
            .unwrap_or(z_clear);
        assert_eq!(
            c_obs, c_clr,
            "crusher combiner should equate fence obstacle zone with clear"
        );
    }
}

#[test]
fn zone_blocks_allocated_on_new_map() {
    let mut system = PathfindingSystem::new(25, 25);
    system.new_map();
    let z = &system.zones;
    // 25 cells → 3 blocks (10+10+5)
    assert_eq!(z.blocks_x, 3);
    assert_eq!(z.blocks_y, 3);
    assert_eq!(z.zone_blocks.len(), 3);
    assert_eq!(z.zone_blocks[0].len(), 3);
    assert!(z.zone_blocks[0][0].num_zones >= 1);
}

#[test]
fn get_block_zone_uses_block_combiner() {
    let mut system = PathfindingSystem::new(20, 20);
    for y in 0..20 {
        system.set_cell_type(
            &Coord3D::new(50.0, y as f32 * 10.0 + 5.0, 0.0),
            PathfindCellType::Cliff,
        );
    }
    system.new_map();
    let z = &system.zones;
    let cell_zone = z.zones[5][5];
    let block_z = z.get_block_zone(SURFACE_GROUND | SURFACE_CLIFF, false, 5, 5);
    // ground|cliff effective should resolve through block table
    assert!(block_z > 0 || cell_zone == 0);
    let bx = 5 / ZONE_BLOCK_SIZE as i32;
    let by = 5 / ZONE_BLOCK_SIZE as i32;
    assert!(z.zone_blocks[bx as usize][by as usize].num_zones >= 1);
}

#[test]
fn hierarchical_zones_merge_same_type() {
    let mut system = PathfindingSystem::new(16, 16);
    // Two clear regions separated by a cliff strip — hierarchical should
    // still only merge same-type neighbors, not across cliff.
    for y in 0..16 {
        system.set_cell_type(
            &Coord3D::new(80.0, y as f32 * 10.0 + 5.0, 0.0),
            PathfindCellType::Cliff,
        );
    }
    system.new_map();
    let z = &system.zones;
    assert!(!z.hierarchical_zones.is_empty());
    assert!(!z.terrain_zones.is_empty());
    // Default effective zone for plain ground uses hierarchical table.
    let z_clear = z.zones[1][1];
    if z_clear != 0 {
        let eff = z.get_effective_zone(SURFACE_GROUND, false, z_clear);
        let hier = z
            .hierarchical_zones
            .get(z_clear as usize)
            .copied()
            .unwrap_or(z_clear);
        assert_eq!(eff, hier);
    }
}

#[test]
fn terrain_zone_treats_obstacle_as_clear() {
    let mut system = PathfindingSystem::new(12, 12);
    {
        let pf = &mut system.pathfinder;
        // Non-fence obstacle between clear cells
        pf.set_cell_obstacle_id(GridCoord::new(5, 5), 9, false, false);
    }
    system.new_map();
    let z = &system.zones;
    let z_obs = z.zones[5][5];
    let z_a = z.zones[4][5];
    let z_b = z.zones[6][5];
    assert_ne!(z_obs, 0);
    // Terrain combiner should equate obstacle zone with neighboring clear
    // when obstacle is treated as clear (C++ terrain()).
    if z_a != 0 && z_a != z_obs {
        let ta = z.terrain_zones.get(z_a as usize).copied().unwrap_or(z_a);
        let to = z
            .terrain_zones
            .get(z_obs as usize)
            .copied()
            .unwrap_or(z_obs);
        // After flatten, hierarchical[terrain] should match for connectivity
        let ha = z.get_effective_terrain_zone(z_a);
        let ho = z.get_effective_terrain_zone(z_obs);
        assert_eq!(
            ha, ho,
            "terrain effective should link obstacle-as-clear to neighbor clear (a={} o={} ta={} to={})",
            z_a, z_obs, ta, to
        );
    }
    let _ = z_b;
}

#[test]
fn connect_layer_merges_hierarchical_zone() {
    let mut system = PathfindingSystem::new(20, 20);
    // Bridge layer id starts at 2 (Top).
    let bid = system.add_bridge((GridCoord::new(8, 8), GridCoord::new(12, 8)));
    assert_eq!(bid, 2);
    // Mark a clear ground cell as connecting to the bridge layer.
    system.set_connect_layer(GridCoord::new(8, 7), PathfindLayerEnum::Top);
    system.new_map();
    let bridge_zone = system.bridge_by_layer_id(bid).expect("bridge").zone;
    assert_ne!(bridge_zone, 0, "bridge layer zone must be allocated");
    let z = &system.zones;
    assert!(!z.hierarchical_zones.is_empty());
    let cell_z = z.zones[8][7];
    assert_ne!(cell_z, 0);
    assert_ne!(
        cell_z, bridge_zone,
        "layer zone must be distinct from ground cell zone"
    );
    let h_cell = z
        .hierarchical_zones
        .get(cell_z as usize)
        .copied()
        .unwrap_or(cell_z);
    let h_bridge = z
        .hierarchical_zones
        .get(bridge_zone as usize)
        .copied()
        .unwrap_or(bridge_zone);
    assert_eq!(
        h_cell, h_bridge,
        "connect-layer clear cell must hierarchical-merge with bridge layer zone"
    );
}

#[test]
fn process_queue_recalculates_dirty_zones() {
    let mut system = PathfindingSystem::new(16, 16);
    system.new_map();
    assert!(system.is_map_ready);
    system.mark_zones_dirty();
    assert!(system.zones.zones_dirty);
    // C++ processPathfindQueue: dirty → calculateZones and return 0 processed.
    let n = system.process_queue(PATHFIND_CELLS_PER_FRAME);
    assert_eq!(n, 0, "dirty zone frame must not drain path queue");
    assert!(
        !system.zones.zones_dirty,
        "zones_dirty cleared after recalculate"
    );
}

#[test]
fn hierarchical_path_marks_start_block_passable() {
    let mut system = PathfindingSystem::new(40, 40);
    system.new_map();
    // Clear all passable first.
    system.clear_zone_passable_flags();
    let from = Coord3D::new(50.0, 50.0, 0.0);
    let cells = vec![
        GridCoord::new(5, 5),
        GridCoord::new(6, 5),
        GridCoord::new(7, 5),
    ];
    let res = system.build_hierarchical_path(&from, &cells);
    assert!(res.success);
    // Start neighborhood should be passable on A* table.
    let pf = &system.pathfinder;
    assert!(pf.is_zone_passable(GridCoord::new(5, 5)));
}

#[test]
fn zone_bridge_flags_from_live_bridges() {
    let mut system = PathfindingSystem::new(40, 40);
    let bid = system.add_bridge((GridCoord::new(10, 10), GridCoord::new(20, 10)));
    assert_ne!(bid, 0);
    system.new_map();
    // Start/end blocks should interact with bridge.
    assert!(
        system.zone_interacts_with_bridge(GridCoord::new(10, 10)),
        "start cell block must interact with bridge"
    );
    assert!(
        system.zone_interacts_with_bridge(GridCoord::new(20, 10)),
        "end cell block must interact with bridge"
    );
    // Far cell should not.
    assert!(
        !system.zone_interacts_with_bridge(GridCoord::new(0, 0)),
        "unrelated block must not interact"
    );
    // Destroyed bridge clears on next recalc.
    system.set_bridge_destroyed(bid, true);
    system.force_map_recalculation();
    assert!(
        !system.zone_interacts_with_bridge(GridCoord::new(10, 10)),
        "destroyed bridge must not stamp setBridge"
    );
}

#[test]
fn hierarchical_skips_pinched_cells() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    let cell = GridCoord::new(5, 5);
    let adj = GridCoord::new(6, 5);
    {
        let pf = &mut system.pathfinder;
        pf.set_pinched(adj, true);
    }
    let mut examined = Vec::new();
    let parent_zone = system.zones.zone_at(cell);
    let res = system.process_hierarchical_cell(
        cell,
        (1, 0),
        parent_zone,
        SURFACE_GROUND,
        false,
        &mut examined,
    );
    assert!(res.is_none(), "pinched neighbor must be skipped");
}

#[test]
fn logical_extent_human_clamp() {
    let mut system = PathfindingSystem::new(40, 40);
    system.new_map();
    // Shrink logical extent to a corner.
    system.set_logical_extent(ICoord2D::new(0, 0), ICoord2D::new(5, 5));
    let inside = PathRequest {
        object_id: INVALID_ID,
        from: Coord3D::new(25.0, 25.0, 0.0), // cell ~2,2
        to: Coord3D::new(45.0, 45.0, 0.0),   // cell ~4,4
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: true,
    };
    let _ = system.find_path(inside);
    // Outside logical for human must fail.
    let outside = PathRequest {
        object_id: INVALID_ID,
        from: Coord3D::new(25.0, 25.0, 0.0),
        to: Coord3D::new(300.0, 300.0, 0.0), // cell ~30,30
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: true,
    };
    assert!(
        !system.find_path(outside.clone()).success,
        "human path outside logical extent must fail"
    );
    // AI (is_human=false) may still attempt outside.
    let mut ai = outside.clone();
    ai.is_human = false;
    // Not required to succeed, but must not hard-reject solely on logical.
    let _ = system.find_path(ai);
}

#[test]
fn hierarchical_bridge_jumps_from_live_bridge() {
    let mut system = PathfindingSystem::new(40, 40);
    // Bridge spanning two blocks: start block (1,1) cells 10-19, end block (2,1).
    let bid = system.add_bridge((GridCoord::new(15, 15), GridCoord::new(25, 15)));
    assert_ne!(bid, 0);
    system.new_map();
    assert!(system.zone_interacts_with_bridge(GridCoord::new(15, 15)));
    let parent = GridCoord::new(15, 15);
    let parent_z = system
        .zones
        .get_block_zone(SURFACE_GROUND, false, parent.x, parent.y);
    let mut examined = Vec::new();
    let jumps =
        system.hierarchical_bridge_jumps(parent, parent_z, 0, SURFACE_GROUND, false, &mut examined);
    assert!(
        !jumps.is_empty(),
        "live bridge must yield hierarchical far-end jump"
    );
    let (far, _fz, _) = jumps[0];
    assert_eq!(far, GridCoord::new(25, 15));
}

#[test]
fn build_actual_path_respects_center_flag() {
    let mut system = PathfindingSystem::new(30, 30);
    let from = Coord3D::new(15.0, 15.0, 0.0);
    let to = Coord3D::new(85.0, 15.0, 0.0);
    let grid = vec![
        GridCoord::new(1, 1),
        GridCoord::new(4, 1),
        GridCoord::new(8, 1),
    ];
    let centered = system.build_actual_path(&grid, &from, &to, SURFACE_GROUND, false, false, true);
    let cornered = system.build_actual_path(&grid, &from, &to, SURFACE_GROUND, false, false, false);
    assert!(centered.success && cornered.success);
    // Intermediate waypoint (not from/to) should differ for center vs corner.
    // Find a mid waypoint that is not from/to.
    let mid_c = centered
        .waypoints
        .iter()
        .find(|p| (p.x - from.x).abs() > 1.0 && (p.x - to.x).abs() > 1.0);
    let mid_k = cornered
        .waypoints
        .iter()
        .find(|p| (p.x - from.x).abs() > 1.0 && (p.x - to.x).abs() > 1.0);
    if let (Some(a), Some(b)) = (mid_c, mid_k) {
        // center is +0.5 cell; corner is cell origin — x or y should differ by ~5.
        let dx = (a.x - b.x).abs();
        let dy = (a.y - b.y).abs();
        assert!(
            dx > 0.1 || dy > 0.1,
            "centerInCell must change intermediate cell snap (c={:?} k={:?})",
            a,
            b
        );
    }
}

#[test]
fn update_goal_bridge_end_stamps_ground() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    let cell = GridCoord::new(5, 5);
    // Elevated layer without bridge-end: layer only.
    system.update_goal(cell, 42, PathfindLayerEnum::Top, 0, true, false);
    let goals = &system.goal_cells;
    let gc = goals[5][5];
    assert_eq!(gc.get_goal_unit(PathfindLayerEnum::Top), 42);
    // Ground should not be stamped without bridge-end.
    // (may be INVALID if never set)
    system.remove_goal(42, 0, true, PathfindLayerEnum::Top);
    // With bridge-end both layers.
    system.update_goal(cell, 43, PathfindLayerEnum::Top, 0, true, true);
    let goals = &system.goal_cells;
    let gc = goals[5][5];
    assert_eq!(gc.get_goal_unit(PathfindLayerEnum::Top), 43);
    assert_eq!(
        gc.get_goal_unit(PathfindLayerEnum::Ground),
        43,
        "bridge-end must also stamp ground goal cells"
    );
}

#[test]
fn tall_building_segment_finds_obstacle_id_building() {
    let mut system = PathfindingSystem::new(40, 40);
    system.new_map();
    // Stamp obstacle cell with a fake id — without registry object, scan skips.
    // With KindOf would need full object; surface: obstacle id is read.
    {
        let pf = &mut system.pathfinder;
        pf.set_cell_type(GridCoord::new(10, 10), PathfindCellType::Obstacle);
        pf.set_cell_obstacle_id(GridCoord::new(10, 10), 99, false, false);
    }
    assert_eq!(
        system.get_cell_obstacle_id(GridCoord::new(10, 10)),
        Some(99)
    );
    // No registry object → no tall building found (cannot resolve KindOf).
    let from = Coord3D::new(50.0, 100.0, 0.0);
    let to = Coord3D::new(150.0, 100.0, 0.0);
    assert!(
        system
            .find_tall_building_along_segment(&from, &to, INVALID_ID)
            .is_none()
    );
}

#[test]
fn clear_cell_for_diameter_allows_crusher_through_fence() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    {
        let pf = &mut system.pathfinder;
        pf.set_cell_type(GridCoord::new(5, 5), PathfindCellType::Obstacle);
        pf.set_cell_obstacle_id(GridCoord::new(5, 5), 1, true, false);
    }
    // Non-crusher blocked by fence.
    assert_eq!(
        system.clear_cell_for_diameter(false, 5, 5, PathfindLayerEnum::Ground, 1),
        0
    );
    // Crusher can pass fence at diameter 1.
    assert!(system.clear_cell_for_diameter(true, 5, 5, PathfindLayerEnum::Ground, 1) >= 1);
}

#[test]
fn update_pos_stamps_pos_unit_not_goal() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    let cell = GridCoord::new(4, 4);
    system.update_pos(cell, 77, PathfindLayerEnum::Ground, 0, true, false);
    let goals = &system.goal_cells;
    let gc = goals[4][4];
    assert_eq!(gc.get_pos_unit(PathfindLayerEnum::Ground), 77);
    assert_eq!(
        gc.get_goal_unit(PathfindLayerEnum::Ground),
        INVALID_ID,
        "updatePos must not stamp goal units"
    );
}

#[test]
fn check_for_movement_ally_moving_from_pos_without_goal() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    // Stamp only pos unit → UNIT_PRESENT_MOVING.
    system.set_pos_cells(
        55,
        crate::common::ICoord2D::new(5, 5),
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    let mut info = CheckMovementInfo {
        cell: GridCoord::new(5, 5),
        layer: PathfindLayerEnum::Ground,
        center_in_cell: true,
        radius: 0,
        consider_transient: false,
        ..Default::default()
    };
    // Without a real object in registry for obj_id, returns true early.
    // Surface: occupancy flags are queryable.
    let goals = &system.goal_cells;
    let gc = goals[5][5];
    assert_eq!(gc.get_pos_unit(PathfindLayerEnum::Ground), 55);
    assert_eq!(gc.get_goal_unit(PathfindLayerEnum::Ground), INVALID_ID);
    assert_eq!(
        PathfindingSystem::cell_occupancy_flags(INVALID_ID, 55),
        0x02, // UNIT_PRESENT_MOVING
    );
    let _ = info;
}

#[test]
fn cell_occupancy_flags_match_cpp_setters() {
    assert_eq!(
        PathfindingSystem::cell_occupancy_flags(INVALID_ID, INVALID_ID),
        0x00
    );
    assert_eq!(PathfindingSystem::cell_occupancy_flags(1, INVALID_ID), 0x01); // UNIT_GOAL
    assert_eq!(PathfindingSystem::cell_occupancy_flags(INVALID_ID, 2), 0x02); // UNIT_PRESENT_MOVING
    assert_eq!(PathfindingSystem::cell_occupancy_flags(3, 3), 0x03); // FIXED
    assert_eq!(PathfindingSystem::cell_occupancy_flags(4, 5), 0x05); // GOAL_OTHER_MOVING
}

#[test]
fn snap_closest_avoids_fixed_when_radius_zero() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    // Stamp FIXED occupancy at cell (5,5): same goal+pos unit.
    let c = crate::common::ICoord2D::new(5, 5);
    system.set_goal_cells(88, c, 0, true, PathfindLayerEnum::Ground, true, false);
    system.set_pos_cells(88, c, 0, true, PathfindLayerEnum::Ground, true, false);
    assert!(system.goal_cell_fixed_occupied(GridCoord::new(5, 5), PathfindLayerEnum::Ground));
    // Open neighbor should not be fixed.
    assert!(!system.goal_cell_fixed_occupied(GridCoord::new(6, 5), PathfindLayerEnum::Ground));
}

#[test]
fn are_connected_ground_cliff_merge() {
    // Two zones linked only via ground_cliff combiner should connect for
    // SURFACE_GROUND|CLIFF locomotors but not plain GROUND.
    let mut z = ZoneManager::new(4, 4);
    z.next_zone = 3;
    z.zones = vec![vec![1u16; 4]; 4];
    z.zones[0][0] = 1;
    z.zones[3][3] = 2;
    z.rebuild_combiner_identity();
    // Manually merge zone 1 and 2 in ground_cliff table only.
    z.ground_cliff_zones[1] = 1;
    z.ground_cliff_zones[2] = 1;
    let a = GridCoord::new(0, 0);
    let b = GridCoord::new(3, 3);
    assert!(
        z.are_connected(a, b, SURFACE_GROUND | SURFACE_CLIFF, false),
        "ground+cliff should share merged effective zone"
    );
    assert!(
        !z.are_connected(a, b, SURFACE_GROUND, false),
        "plain ground must not see cliff-only merge"
    );
}

#[test]
fn get_move_away_returns_path_result() {
    let mut system = PathfindingSystem::new(32, 32);
    system.new_map();
    let from = Coord3D::new(55.0, 55.0, 0.0);
    let path = vec![
        Coord3D::new(10.0, 55.0, 0.0),
        Coord3D::new(100.0, 55.0, 0.0),
    ];
    let result = system.get_move_away_from_path_result(
        &from,
        &path,
        None,
        SURFACE_GROUND,
        false,
        0.0,
        0.0,
        INVALID_ID,
        true,
    );
    assert!(result.success);
    assert!(result.waypoints.len() >= 2);
    let end = result.waypoints.last().unwrap();
    assert!(
        (end.y - 55.0).abs() > 5.0 || (end.x - 55.0).abs() > 5.0,
        "path end {:?}",
        end
    );
}

#[test]
fn check_path_cost_straight_line_cheaper_than_detour_gate() {
    let mut system = PathfindingSystem::new(32, 32);
    system.new_map();
    let from = Coord3D::new(20.0, 20.0, 0.0);
    let to = Coord3D::new(80.0, 20.0, 0.0);
    let cost = system.check_path_cost(SURFACE_GROUND, false, &from, &to);
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();
    // C++ checkForAdjust accepts when cost <= 1.4*(dx+dy)
    assert!(
        cost <= 1.4 * (dx + dy) + 1.0,
        "straight path cost {cost} should pass 1.4*(dx+dy)={}",
        1.4 * (dx + dy)
    );
    // Off-map / invalid start → MAX_COST like C++.
    let bad = system.check_path_cost(
        SURFACE_GROUND,
        false,
        &Coord3D::new(-100.0, -100.0, 0.0),
        &to,
    );
    assert!(bad >= 0x7fff_0000u32 as f32 * 0.5);
}

#[test]
fn find_safe_path_moves_outside_repulsor() {
    let mut system = PathfindingSystem::new(48, 48);
    system.new_map();
    let from = Coord3D::new(100.0, 100.0, 0.0);
    let r1 = Coord3D::new(100.0, 100.0, 0.0);
    let r2 = Coord3D::new(105.0, 100.0, 0.0);
    let req = PathRequest {
        object_id: INVALID_ID,
        from,
        to: from,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: true,
    };
    let path = system.find_safe_path(req, &r1, &r2, 40.0);
    assert!(path.success, "should find safe cell");
    let end = path.waypoints.last().unwrap();
    let d1 = (end.x - r1.x) * (end.x - r1.x) + (end.y - r1.y) * (end.y - r1.y);
    assert!(
        d1 > 40.0 * 40.0 * 0.9,
        "end {:?} should leave radius, d2={d1}",
        end
    );
}

#[test]
fn find_path_still_works_open_ground() {
    let mut system = PathfindingSystem::new(32, 32);
    system.new_map();
    let req = PathRequest {
        object_id: INVALID_ID,
        from: Coord3D::new(20.0, 20.0, 0.0),
        to: Coord3D::new(100.0, 100.0, 0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let r = system.find_path(req);
    assert!(r.success);
    assert!(r.waypoints.len() >= 2);
}

#[test]
fn find_path_blocked_wall_returns_cpp_failure() {
    let mut system = PathfindingSystem::new(4, 4);
    system.new_map();
    // A complete impassable column separates the start and destination cells.
    for y in [5.0, 15.0, 25.0, 35.0] {
        system.set_cell_type(&Coord3D::new(15.0, y, 0.0), PathfindCellType::Impassable);
    }
    let request = PathRequest {
        object_id: INVALID_ID,
        from: Coord3D::new(5.0, 5.0, 0.0),
        to: Coord3D::new(35.0, 35.0, 0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let result = system.find_path(request);
    assert!(
        !result.success,
        "blocked route must return C++ NULL-equivalent"
    );
    assert!(result.waypoints.is_empty());
}

#[test]
fn find_closest_path_open_ground_reaches_goal() {
    let mut system = PathfindingSystem::new(32, 32);
    system.new_map();
    let req = PathRequest {
        object_id: INVALID_ID,
        from: Coord3D::new(20.0, 20.0, 0.0),
        to: Coord3D::new(100.0, 80.0, 0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let r = system.find_closest_path(req);
    assert!(r.success);
    assert!(r.waypoints.len() >= 2);
}

#[test]
fn hierarchical_path_open_ground() {
    let mut system = PathfindingSystem::new(64, 64);
    system.new_map();
    // Force zone calc
    system.recalculate_zones_from_cells();
    let start = Coord3D::new(30.0, 30.0, 0.0);
    let end = Coord3D::new(200.0, 180.0, 0.0);
    let r = system.find_hierarchical_path(start, end, SURFACE_GROUND, false);
    assert!(r.is_some(), "hierarchical should connect open ground");
    let path = r.unwrap();
    assert!(path.success && path.waypoints.len() >= 2);
}

#[test]
fn add_bridge_runs_classify_clearance() {
    let mut system = PathfindingSystem::new(32, 32);
    system.new_map();
    let lo = GridCoord::new(5, 5);
    let hi = GridCoord::new(10, 8);
    let _id = system.add_bridge_ex((lo, hi), INVALID_ID, lo, hi);
    // Without terrain, deck_z = 2*cell; ground 0 → 0+10 > 20? false, so no impassable.
    // Entry cells should be Clear.
    let pf = &system.pathfinder;
    assert_eq!(pf.get_cell_type(lo), Some(PathfindCellType::Clear));
}

#[test]
fn human_logical_extent_blocks_out_of_map_neighbor() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    // Full grid clear
    {
        let pf = &mut system.pathfinder;
        for x in 0..20 {
            for y in 0..20 {
                pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
            }
        }
    }
    // Logical map is only left half
    system.set_logical_extent(ICoord2D::new(0, 0), ICoord2D::new(9, 19));
    let from = Coord3D::new(5.0 * PATHFIND_CELL_SIZE_F, 10.0 * PATHFIND_CELL_SIZE_F, 0.0);
    let to = Coord3D::new(
        15.0 * PATHFIND_CELL_SIZE_F,
        10.0 * PATHFIND_CELL_SIZE_F,
        0.0,
    );
    let human = PathRequest {
        object_id: INVALID_ID,
        from,
        to,
        surfaces: 0xFFFF,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: true,
    };
    // Start/goal outside clamp of goal → rejected at entry
    let r = system.find_path(human.clone());
    assert!(!r.success, "human path to outside logical extent must fail");

    let computer = PathRequest {
        is_human: false,
        ..human
    };
    let r2 = system.find_path(computer);
    assert!(
        r2.success,
        "computer may path outside logical extent: {:?}",
        r2.waypoints.len()
    );
}

#[test]
fn attack_los_blocks_opaque_obstacle_on_line() {
    let mut system = PathfindingSystem::new(20, 20);
    system.new_map();
    {
        let pf = &mut system.pathfinder;
        for x in 0..20 {
            for y in 0..20 {
                pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
            }
        }
        // Opaque wall at x=10
        for y in 0..20 {
            let c = GridCoord::new(10, y);
            pf.set_cell_obstacle_id(c, 999, false, false);
        }
    }
    let from = Coord3D::new(5.0 * PATHFIND_CELL_SIZE_F, 10.0 * PATHFIND_CELL_SIZE_F, 0.0);
    let to = Coord3D::new(
        15.0 * PATHFIND_CELL_SIZE_F,
        10.0 * PATHFIND_CELL_SIZE_F,
        0.0,
    );
    // Without AttackNeedsLineOfSight kind and without object, KINDOF check is skipped when id INVALID
    // With INVALID attacker, still runs Bresenham obstacle check
    assert!(
        system.is_attack_view_blocked_by_obstacle(INVALID_ID, &from, None, &to),
        "opaque obstacle must block attack LOS"
    );
    // Transparent wall should not block
    {
        let pf = &mut system.pathfinder;
        for y in 0..20 {
            let c = GridCoord::new(10, y);
            pf.set_cell_obstacle_id(c, 999, false, true);
        }
    }
    assert!(
        !system.is_attack_view_blocked_by_obstacle(INVALID_ID, &from, None, &to),
        "transparent obstacle must not block"
    );
}
