use super::*;
use super::*;

#[test]
fn test_grid_coord_conversion() {
    let world_pos = Coord3D::new(15.0, 25.0, 0.0);
    let grid = GridCoord::from_world(&world_pos);
    assert_eq!(grid.x, 1);
    assert_eq!(grid.y, 2);

    let world_back = grid.to_world(0.0);
    assert!((world_back.x - 15.0).abs() < 1.0);
    assert!((world_back.y - 25.0).abs() < 1.0);
    assert_eq!(world_back.z, 0.0);
}

#[test]
fn test_manhattan_distance() {
    let a = GridCoord::new(0, 0);
    let b = GridCoord::new(3, 4);
    assert_eq!(a.manhattan_distance(&b), 70); // (3+4) * 10
}

#[test]
fn test_diagonal_distance() {
    let a = GridCoord::new(0, 0);
    let b = GridCoord::new(3, 4);
    // Should be more accurate than Manhattan
    let dist = a.diagonal_distance(&b);
    assert!(dist > 0 && dist <= a.manhattan_distance(&b));
}

#[test]
fn test_simple_pathfinding() {
    let mut pathfinder = AStarPathfinder::new(10, 10);

    let start = GridCoord::new(0, 0);
    let goal = GridCoord::new(5, 5);

    let path = pathfinder
        .find_path(start, goal, 0xFFFFFFFF, false, 1000, false, None)
        .map(|(p, _)| p);
    assert!(path.is_some());

    let path = path.unwrap();
    assert_eq!(path[0], start);
    assert_eq!(path[path.len() - 1], goal);
}

#[test]
fn cliff_cost_uses_explicit_ground_heights() {
    let mut pathfinder = AStarPathfinder::new(5, 3);
    for x in 1..4 {
        pathfinder.set_cell_type(GridCoord::new(x, 1), PathfindCellType::Cliff);
    }
    let start = GridCoord::new(0, 1);
    let goal = GridCoord::new(4, 1);
    let flat_height = |_cell: GridCoord| 0.0;
    let steep_height = |cell: GridCoord| cell.x as f32 * PATHFIND_CELL_SIZE_F;

    let flat = pathfinder
        .find_path_ex_with_ground_height(
            start,
            goal,
            SURFACE_GROUND | SURFACE_CLIFF,
            false,
            1000,
            false,
            None,
            None,
            Some(&flat_height),
        )
        .expect("flat cliff route");
    let steep = pathfinder
        .find_path_ex_with_ground_height(
            start,
            goal,
            SURFACE_GROUND | SURFACE_CLIFF,
            false,
            1000,
            false,
            None,
            None,
            Some(&steep_height),
        )
        .expect("steep cliff route");

    assert!(flat.0.iter().any(|cell| cell.y != 1));
    assert!(steep.0.iter().all(|cell| cell.y == 1));
}

#[test]
fn test_pathfinding_with_obstacles() {
    let mut pathfinder = AStarPathfinder::new(10, 10);

    // Create a wall
    for y in 1..9 {
        pathfinder.set_cell_type(GridCoord::new(5, y), PathfindCellType::Obstacle);
    }

    let start = GridCoord::new(0, 5);
    let goal = GridCoord::new(9, 5);

    // Should find path around the wall
    let path = pathfinder
        .find_path(start, goal, 0x01, false, 1000, false, None)
        .map(|(p, _)| p);
    assert!(path.is_some());

    let path = path.unwrap();
    assert_eq!(path.first().copied(), Some(start));
    assert_eq!(path.last().copied(), Some(goal));
    // C++ one-open-orthogonal squeeze lets the path cut the wall end at (5,9)/(5,0).
    // Chebyshev around that gap can match the 10-cell straight-line hop count.
    assert!(
        path.iter().any(|c| c.x == 5 && (c.y == 0 || c.y == 9)),
        "ground path must detour through the wall gap, got {:?}",
        path
    );
    assert!(
        path.iter()
            .all(|c| { pathfinder.get_cell_type(*c) != Some(PathfindCellType::Obstacle) }),
        "ground path must not step on the obstacle column: {:?}",
        path
    );
}

#[test]
fn test_no_path_exists() {
    let mut pathfinder = AStarPathfinder::new(10, 10);

    // Create a complete barrier
    for y in 0..10 {
        pathfinder.set_cell_type(GridCoord::new(5, y), PathfindCellType::Impassable);
    }

    let start = GridCoord::new(0, 5);
    let goal = GridCoord::new(9, 5);

    let path = pathfinder
        .find_path(start, goal, 0x01, false, 1000, false, None)
        .map(|(p, _)| p);
    assert!(path.is_none());
}

#[test]
fn test_crusher_pathfinding() {
    // C++ validMovementPosition: crushers only enter isObstacleFence cells.
    let mut pathfinder = AStarPathfinder::new(10, 10);
    let obstacle = GridCoord::new(5, 5);
    pathfinder.set_cell_obstacle_id(obstacle, 7, false, false);

    let start = GridCoord::new(0, 5);
    let goal = GridCoord::new(9, 5);

    // Solid building: both crushers and non-crushers path around.
    let path_normal = pathfinder
        .find_path(start, goal, SURFACE_GROUND, false, 1000, false, None)
        .map(|(p, _)| p);
    assert!(path_normal.is_some());
    let path_crusher = pathfinder
        .find_path(start, goal, SURFACE_GROUND, true, 1000, false, None)
        .map(|(p, _)| p);
    assert!(path_crusher.is_some());
    assert_eq!(path_crusher.unwrap().len(), path_normal.unwrap().len());
    assert!(!pathfinder.is_passable(obstacle, SURFACE_GROUND, true));
}

#[test]
fn crusher_find_path_only_crosses_fence_obstacles_like_cpp() {
    // Tiny grid, full-height wall so the only route is through x=4.
    // C++ AIPathfind.cpp:4840-4842 + 6316-6318.
    let mut pf = AStarPathfinder::new(9, 5);
    for y in 0..5 {
        pf.set_cell_obstacle_id(GridCoord::new(4, y), 1, false, false);
    }
    let start = GridCoord::new(0, 2);
    let goal = GridCoord::new(8, 2);

    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, true, 2000, false, None)
            .is_none(),
        "crusher must not path through solid CELL_OBSTACLE buildings"
    );
    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .is_none()
    );
    assert!(!pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, true));

    for y in 0..5 {
        pf.set_cell_obstacle_id(GridCoord::new(4, y), 2, true, false);
    }
    assert!(
        pf.is_obstacle_fence(GridCoord::new(4, 2)),
        "wall must be stamped as fence"
    );
    assert!(
        !pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, false),
        "non-crusher still blocked by fence"
    );
    assert!(
        pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, true),
        "crusher may enter fence cells"
    );
    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .is_none(),
        "non-crusher must not path through a fence wall"
    );
    let (path, iters) = pf
        .find_path(start, goal, SURFACE_GROUND, true, 2000, false, None)
        .expect("crusher must path through a fence wall");
    assert!(iters >= 1);
    assert_eq!(path.first().copied(), Some(start));
    assert_eq!(path.last().copied(), Some(goal));
    assert!(
        path.iter().any(|c| c.x == 4),
        "crusher path must cross fence column: {:?}",
        path
    );
}

#[test]
fn diagonal_squeeze_one_orthogonal_open_allows_path() {
    // C++ examineNeighboringCells AIPathfind.cpp:6181-6185:
    // skip diagonal only if BOTH adjacent neighborFlags are false.
    // 2x2 crack: S X / . G — one ortho open, A* prefers S→G (cost 14).
    let mut pf = AStarPathfinder::new(2, 2);
    pf.set_cell_type(GridCoord::new(1, 0), PathfindCellType::Impassable);
    let start = GridCoord::new(0, 0);
    let goal = GridCoord::new(1, 1);
    let (path, iters) = pf
        .find_path_ex6(
            start,
            goal,
            SURFACE_GROUND,
            false,
            200,
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        )
        .expect("diagonal crack must be usable when one orthogonal is open");
    assert!(iters >= 1);
    assert_eq!(
        path,
        vec![start, goal],
        "expected direct diagonal: {:?}",
        path
    );
}

#[test]
fn diagonal_squeeze_both_orthogonals_blocked_no_path() {
    // Disable C++ line-to-goal seeding to isolate examineNeighboringCells:
    // both orthogonals blocked → neighborFlags both false → no diagonal.
    let mut pf = AStarPathfinder::new(2, 2);
    pf.set_cell_type(GridCoord::new(1, 0), PathfindCellType::Impassable);
    pf.set_cell_type(GridCoord::new(0, 1), PathfindCellType::Impassable);
    let start = GridCoord::new(0, 0);
    let goal = GridCoord::new(1, 1);
    assert!(
        pf.find_path_ex6(
            start,
            goal,
            SURFACE_GROUND,
            false,
            200,
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        )
        .is_none(),
        "must not squeeze a diagonal when both orthogonals are blocked"
    );
}

#[test]
fn line_seed_can_cross_blocked_diagonal_orthogonals() {
    // C++ examineCellsCallback directly seeds clear Bresenham cells and does
    // not consult neighborFlags, so find_path may take this direct route.
    let mut pf = AStarPathfinder::new(2, 2);
    pf.set_cell_type(GridCoord::new(1, 0), PathfindCellType::Impassable);
    pf.set_cell_type(GridCoord::new(0, 1), PathfindCellType::Impassable);
    let start = GridCoord::new(0, 0);
    let goal = GridCoord::new(1, 1);
    let (path, _) = pf
        .find_path(start, goal, SURFACE_GROUND, false, 200, false, None)
        .expect("C++ line seeding may directly seed the clear goal cell");
    assert_eq!(path, vec![start, goal]);
}

#[test]
fn air_paths_over_solid_obstacle_ground_does_not() {
    // C++ validLocomotorSurfacesForCellType(CELL_OBSTACLE) = AIR.
    // Full-height solid building wall: ground cannot go around.
    let mut pf = AStarPathfinder::new(9, 5);
    for y in 0..5 {
        pf.set_cell_obstacle_id(GridCoord::new(4, y), 1, false, false);
    }
    let start = GridCoord::new(0, 2);
    let goal = GridCoord::new(8, 2);

    assert!(
        !pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, false),
        "ground blocked by solid building"
    );
    assert!(
        !pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, true),
        "ground crusher still blocked by non-fence building"
    );
    assert!(
        pf.is_passable(GridCoord::new(4, 2), SURFACE_AIR, false),
        "AIR locomotor must enter CELL_OBSTACLE"
    );
    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .is_none(),
        "ground unit must not path through a solid building wall"
    );
    let (path, iters) = pf
        .find_path(start, goal, SURFACE_AIR, false, 2000, false, None)
        .expect("AIR locomotor must path over solid building obstacle");
    assert!(iters >= 1);
    assert_eq!(path.first().copied(), Some(start));
    assert_eq!(path.last().copied(), Some(goal));
    assert!(
        path.iter().any(|c| c.x == 4),
        "AIR path must cross obstacle column: {:?}",
        path
    );
}

#[test]
fn air_paths_over_impassable_cells_ground_does_not() {
    // C++ validLocomotorSurfacesForCellType(CELL_IMPASSABLE) = AIR.
    let mut pf = AStarPathfinder::new(9, 5);
    for y in 0..5 {
        pf.set_cell_type(GridCoord::new(4, y), PathfindCellType::Impassable);
    }
    let start = GridCoord::new(0, 2);
    let goal = GridCoord::new(8, 2);
    assert!(!pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, false));
    assert!(pf.is_passable(GridCoord::new(4, 2), SURFACE_AIR, false));
    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .is_none()
    );
    let (path, _) = pf
        .find_path(start, goal, SURFACE_AIR, false, 2000, false, None)
        .expect("AIR locomotor must path over CELL_IMPASSABLE");
    assert!(path.iter().any(|c| c.x == 4), "AIR path: {:?}", path);
}

#[test]
fn test_ignore_obstacle_allows_pass_through() {
    let mut pathfinder = AStarPathfinder::new(10, 10);
    let obstacle = GridCoord::new(5, 5);
    pathfinder.set_cell_type(obstacle, PathfindCellType::Obstacle);

    let mut ignore = HashSet::new();
    ignore.insert(obstacle);

    assert!(!pathfinder.is_passable_with_ignore(obstacle, 0x01, false, None));
    assert!(pathfinder.is_passable_with_ignore(obstacle, 0x01, false, Some(&ignore)));
}

#[test]
fn zone_impassable_adds_cost_penalty() {
    let mut pf = AStarPathfinder::new(30, 30);
    let a = GridCoord::new(2, 2);
    let (path1, cells1) = pf
        .find_path(
            a,
            GridCoord::new(25, 2),
            SURFACE_GROUND,
            false,
            8000,
            false,
            None,
        )
        .expect("path");
    assert!(path1.len() > 1);
    assert!(cells1 >= 1);
    pf.set_zone_passable(GridCoord::new(25, 2), false);
    assert!(!pf.is_zone_passable(GridCoord::new(25, 2)));
    assert!(pf.is_zone_passable(a));
    let (path2, cells2) = pf
        .find_path(
            a,
            GridCoord::new(25, 2),
            SURFACE_GROUND,
            false,
            8000,
            false,
            None,
        )
        .expect("path with zone penalty");
    assert!(path2.len() > 1);
    assert!(cells2 >= 1);
    assert!(!pf.clip_is_zone_passable(-1, 0));
    assert!(!pf.clip_is_zone_passable(0, 1000));
}

#[test]
fn hierarchical_zone_prune_marks_corridor() {
    let mut pf = AStarPathfinder::new(80, 80);
    let start = GridCoord::new(2, 2);
    let goal = GridCoord::new(75, 2);
    assert!(pf.apply_hierarchical_zone_prune(start, goal, SURFACE_GROUND, false, &[]));
    assert!(pf.is_zone_passable(start));
    assert!(pf.is_zone_passable(goal));
    assert!(pf.is_zone_passable(GridCoord::new(40, 2)));
    assert!(
        !pf.is_zone_passable(GridCoord::new(40, 70)),
        "off-corridor block must stay pruned"
    );
    let (path, _n) = pf
        .find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
        .expect("corridor A* must still reach the far cell");
    assert!(path.len() > 2);
    assert_eq!(*path.last().unwrap(), goal);
}

#[test]
fn hierarchical_zone_prune_jumps_bridge_over_water() {
    let mut pf = AStarPathfinder::new(40, 20);
    for y in 0..20 {
        pf.set_cell_type(GridCoord::new(20, y), PathfindCellType::Water);
    }
    let start = GridCoord::new(2, 10);
    let goal = GridCoord::new(35, 10);
    let near = GridCoord::new(19, 10);
    let far = GridCoord::new(21, 10);
    assert!(
        pf.apply_hierarchical_zone_prune(start, goal, SURFACE_GROUND, false, &[(near, far)]),
        "bridge jump must join river banks"
    );
    assert!(pf.is_zone_passable(start));
    assert!(pf.is_zone_passable(goal));
    assert!(
        !pf.apply_hierarchical_zone_prune(start, goal, SURFACE_GROUND, false, &[]),
        "no jump → hierarchical fail → all passable"
    );
    assert!(pf.is_zone_passable(GridCoord::new(2, 2)));
    assert!(pf.is_zone_passable(GridCoord::new(35, 18)));
}

#[test]
fn examine_cells_line_seed_half_ortho_cost() {
    let mut pf = AStarPathfinder::new(20, 20);
    for x in 0..20 {
        for y in 0..20 {
            pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
        }
    }
    let start = GridCoord::new(2, 2);
    let goal = GridCoord::new(10, 2);
    let path = pf
        .find_path_ex4(
            start, goal, 0xFFFF, false, 5000, false, None, None, false, None, None, None, true,
        )
        .expect("path");
    assert!(path.0.len() >= 2);
    assert_eq!(*path.0.first().unwrap(), start);
    assert_eq!(*path.0.last().unwrap(), goal);
    assert!(
        path.0.iter().all(|c| c.y == 2),
        "line seed should prefer straight y=2: {:?}",
        path.0
    );
}

#[test]
fn tunneling_invalid_step_allows_obstacle_with_surcharge() {
    // C++: start inside obstacle (tunneling), exit to clear goal beyond wall.
    // Tunneling clears on first valid non-pinched cell — so start must be obstacle.
    let mut pf = AStarPathfinder::new(12, 12);
    for x in 0..12 {
        for y in 0..12 {
            pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
        }
    }
    // Solid obstacle blob containing start at (3,5); goal outside at (8,5).
    for x in 2..=5 {
        for y in 3..=7 {
            pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Obstacle);
        }
    }
    let start = GridCoord::new(3, 5);
    let goal = GridCoord::new(8, 5);
    // force_passable allows start/goal validation for obstacle start.
    // Ground-only: 0xFFFF includes AIR, which can overfly CELL_OBSTACLE.
    let force = |c: GridCoord| c == start;
    assert!(
        pf.find_path_ex5(
            start,
            goal,
            SURFACE_GROUND,
            false,
            5000,
            false,
            None,
            None,
            false,
            None,
            Some(&force as &dyn Fn(GridCoord) -> bool),
            None,
            false,
            false,
            None,
        )
        .is_none()
    );
    let path = pf
        .find_path_ex5(
            start,
            goal,
            SURFACE_GROUND,
            false,
            5000,
            false,
            None,
            None,
            false,
            None,
            Some(&force as &dyn Fn(GridCoord) -> bool),
            None,
            false,
            true,
            None,
        )
        .expect("tunnel path");
    assert_eq!(*path.0.first().unwrap(), start);
    assert_eq!(*path.0.last().unwrap(), goal);
}

#[test]
fn pinched_extra_ortho_on_expand_cpp_surface() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
    assert!(
        src.contains("starts_tunneling")
            && src.contains("10 * COST_ORTHOGONAL")
            && src.contains("is_tunneling = false"),
        "expand must clear tunneling and apply C++ tunnel surcharge"
    );
}

#[test]
fn examine_neighbors_on_list_never_reopens_or_recosts() {
    // C++ examineNeighboringCells AIPathfind.cpp:6167-6180:
    // if (getOpen() || getClosed()) continue; — never update g, never reopen.
    // extra_cost is applied only after that skip, so each cell is recosted once.
    let pf = AStarPathfinder::new(5, 5);
    let start = GridCoord::new(0, 0);
    let goal = GridCoord::new(4, 4);
    let counts = std::cell::RefCell::new(HashMap::<GridCoord, u32>::new());
    let extra = |c: GridCoord| {
        *counts.borrow_mut().entry(c).or_insert(0) += 1;
        0u32
    };
    let (path, iters) = pf
        .find_path_ex(
            start,
            goal,
            SURFACE_GROUND,
            false,
            2000,
            false,
            None,
            Some(&extra as &dyn Fn(GridCoord) -> u32),
        )
        .expect("open 5x5 must path");
    assert!(iters >= 1);
    assert_eq!(path.first().copied(), Some(start));
    assert_eq!(path.last().copied(), Some(goal));
    let counts = counts.into_inner();
    assert!(
        counts.len() > 1,
        "extra_cost must run on neighbor expansion"
    );
    for (c, n) in counts.iter() {
        assert_eq!(
            *n, 1,
            "cell {:?} recosted {} times; onList must skip before cost update",
            c, n
        );
    }
    // First-visit parent is kept: straight-ish first expansion from start
    // claims (1,0) via ortho; a later cheaper reopen would not replace it.
    assert!(
        path.contains(&GridCoord::new(1, 0))
            || path.contains(&GridCoord::new(0, 1))
            || path.contains(&GridCoord::new(1, 1)),
        "first-visit neighbors from start stay on the reconstructed path: {:?}",
        path
    );
}

#[test]
fn dozer_hack_obstacle_and_no_diagonal_squeeze() {
    // C++ AIPathfind.cpp:6207-6226:
    // dozerHack lets dozers step on non-enemy CELL_OBSTACLE;
    // neighborFlags is NOT set, so diagonals cannot squeeze through.
    let mut pf = AStarPathfinder::new(2, 2);
    let start = GridCoord::new(0, 0);
    let goal = GridCoord::new(1, 1);
    let dozer_cell = GridCoord::new(1, 0);
    pf.set_cell_type(dozer_cell, PathfindCellType::Obstacle);
    pf.set_cell_type(GridCoord::new(0, 1), PathfindCellType::Impassable);

    assert!(
        pf.find_path_ex6(
            start,
            goal,
            SURFACE_GROUND,
            false,
            200,
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        )
        .is_none(),
        "find_path_ex6 with dozer_obstacle_ok=None matches non-dozer"
    );

    let ok = |c: GridCoord| c == dozer_cell;
    let (path, _) = pf
        .find_path_ex6(
            start,
            goal,
            SURFACE_GROUND,
            false,
            200,
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            false,
            false,
            None,
            Some(&ok as &dyn Fn(GridCoord) -> bool),
        )
        .expect("dozer_obstacle_ok must allow stepping on CELL_OBSTACLE");
    assert_eq!(
        path,
        vec![start, dozer_cell, goal],
        "dozer must step on the obstacle; must not diagonal-squeeze (neighborFlags false): {:?}",
        path
    );

    // Callback true on Impassable still must NOT dozerHack (C++ requires CELL_OBSTACLE).
    let always = |_c: GridCoord| true;
    let (path2, _) = pf
        .find_path_ex6(
            start,
            goal,
            SURFACE_GROUND,
            false,
            200,
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            false,
            false,
            None,
            Some(&always as &dyn Fn(GridCoord) -> bool),
        )
        .expect("dozer still paths via the obstacle cell");
    assert_eq!(path2, vec![start, dozer_cell, goal]);
    assert!(
        !pf.is_passable(GridCoord::new(0, 1), SURFACE_GROUND, false),
        "Impassable cell is not a dozerHack target"
    );
}

#[test]
fn check_change_layers_enqueues_same_xy_at_parent_cost() {
    // A populated Top cell is a distinct target returned from getCell.
    let mut pf = AStarPathfinder::new(4, 4);
    let start = GridCoord::new(1, 1);
    pf.set_cell_type_on_layer(start, PathfindLayerEnum::Top, PathfindCellType::Clear);
    pf.set_cell_connect_layer(start, PathfindLayerEnum::Top);

    let mut open_set = BinaryHeap::new();
    let mut open_members = HashSet::new();
    let mut closed_set = HashSet::new();
    closed_set.insert((start, PathfindLayerEnum::Ground));
    let mut came_from = HashMap::new();
    let mut g_scores = HashMap::new();
    let enqueued = pf.check_change_layers(
        start,
        PathfindLayerEnum::Ground,
        40,
        99,
        &mut open_set,
        &mut open_members,
        &closed_set,
        &mut came_from,
        &mut g_scores,
    );
    assert!(
        enqueued,
        "checkChangeLayers must enqueue same-xy connect layer"
    );
    let key = (start, PathfindLayerEnum::Top);
    assert!(open_members.contains(&key));
    assert_eq!(g_scores.get(&key).copied(), Some(40));
    let node = open_set.pop().expect("layered node on open heap");
    assert_eq!(node.coord, start);
    assert_eq!(node.layer, PathfindLayerEnum::Top);
    assert_eq!(node.g_score, 40);
    assert_eq!(node.f_score, 99);
    assert_eq!(node.parent, Some((start, PathfindLayerEnum::Ground)));

    // Already on open: do not re-enqueue.
    open_members.insert(key);
    assert!(
        !pf.check_change_layers(
            start,
            PathfindLayerEnum::Ground,
            40,
            99,
            &mut open_set,
            &mut open_members,
            &closed_set,
            &mut came_from,
            &mut g_scores,
        ),
        "already on open list must not re-enqueue"
    );

    // find_path still succeeds; extra same-xy expand is not a silent no-op.
    let goal = GridCoord::new(3, 1);
    let (path_layered, _) = pf
        .find_path(start, goal, SURFACE_GROUND, false, 500, false, None)
        .expect("path with connect_layer must succeed");
    assert_eq!(path_layered.first().copied(), Some(start));
    assert_eq!(path_layered.last().copied(), Some(goal));

    pf.set_cell_connect_layer(start, PathfindLayerEnum::Invalid);
    assert_eq!(
        pf.connect_layer_transition_coord(start),
        None,
        "invalid connect layer has no transition coord"
    );
    pf.set_cell_connect_layer(start, PathfindLayerEnum::Top);
    assert_eq!(
        pf.connect_layer_transition_coord(start),
        Some(start),
        "public GridCoord API stays same-xy"
    );
}

#[test]
fn check_change_layers_missing_top_falls_back_to_closed_ground() {
    // C++ Pathfinder::getCell(Top,x,y) falls back to m_map when the Top cell
    // is absent; checkChangeLayers therefore sees its closed Ground parent.
    let mut pf = AStarPathfinder::new(4, 4);
    let start = GridCoord::new(1, 1);
    pf.set_cell_connect_layer(start, PathfindLayerEnum::Top);

    let mut open_set = BinaryHeap::new();
    let mut open_members = HashSet::new();
    let mut closed_set = HashSet::new();
    closed_set.insert((start, PathfindLayerEnum::Ground));
    let mut came_from = HashMap::new();
    let mut g_scores = HashMap::new();
    assert!(!pf.check_change_layers(
        start,
        PathfindLayerEnum::Ground,
        40,
        99,
        &mut open_set,
        &mut open_members,
        &closed_set,
        &mut came_from,
        &mut g_scores,
    ));
    assert!(open_set.is_empty());
    assert!(open_members.is_empty());
    assert!(came_from.is_empty());
    assert!(g_scores.is_empty());
}

#[test]
fn ground_impassable_does_not_block_top_when_top_is_clear_and_vice_versa() {
    // Independent per-layer grids: C++ m_map vs m_layers[LAYER_TOP].
    let mut pf = AStarPathfinder::new(9, 5);
    let wall_x = 4;
    for y in 0..5 {
        let c = GridCoord::new(wall_x, y);
        pf.set_cell_type(c, PathfindCellType::Impassable);
        pf.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Clear);
    }
    let blocked = GridCoord::new(wall_x, 2);
    assert_eq!(
        pf.get_cell_type(blocked),
        Some(PathfindCellType::Impassable)
    );
    assert_eq!(
        pf.get_cell_type_on_layer(blocked, PathfindLayerEnum::Top),
        Some(PathfindCellType::Clear)
    );
    assert!(!pf.is_passable(blocked, SURFACE_GROUND, false));
    assert!(pf.is_passable_on_layer(blocked, PathfindLayerEnum::Top, SURFACE_GROUND, false));

    let start = GridCoord::new(0, 2);
    let goal = GridCoord::new(8, 2);
    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .is_none(),
        "ground Impassable wall must block LAYER_GROUND search"
    );
    let (top_path, _) = pf
        .find_path_on_layer(
            start,
            goal,
            PathfindLayerEnum::Top,
            SURFACE_GROUND,
            false,
            2000,
            false,
            None,
        )
        .expect("Top Clear cells must ignore ground Impassable");
    assert!(
        top_path.iter().any(|c| c.x == wall_x),
        "Top path must cross the ground wall: {:?}",
        top_path
    );

    // Vice versa: Top Impassable/Obstacle does not block Ground Clear.
    let mut pf2 = AStarPathfinder::new(9, 5);
    for y in 0..5 {
        let c = GridCoord::new(wall_x, y);
        pf2.set_cell_type(c, PathfindCellType::Clear);
        pf2.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Impassable);
    }
    assert_eq!(pf2.get_cell_type(blocked), Some(PathfindCellType::Clear));
    assert_eq!(
        pf2.get_cell_type_on_layer(blocked, PathfindLayerEnum::Top),
        Some(PathfindCellType::Impassable)
    );
    assert!(pf2.is_passable(blocked, SURFACE_GROUND, false));
    let (ground_path, _) = pf2
        .find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
        .expect("ground Clear must ignore Top Impassable");
    assert!(
        ground_path.iter().any(|c| c.x == wall_x),
        "ground path must cross the Top-only wall: {:?}",
        ground_path
    );
}

#[test]
fn top_obstacle_blocks_top_search_not_ground() {
    let mut pf = AStarPathfinder::new(9, 5);
    let mid = GridCoord::new(4, 2);
    pf.set_cell_type(mid, PathfindCellType::Clear);
    pf.set_cell_type_on_layer(mid, PathfindLayerEnum::Top, PathfindCellType::Obstacle);

    assert_eq!(pf.get_cell_type(mid), Some(PathfindCellType::Clear));
    assert_eq!(
        pf.get_cell_type_on_layer(mid, PathfindLayerEnum::Top),
        Some(PathfindCellType::Obstacle)
    );
    assert!(pf.is_passable(mid, SURFACE_GROUND, false));
    assert!(!pf.is_passable_on_layer(mid, PathfindLayerEnum::Top, SURFACE_GROUND, false));

    let start = GridCoord::new(0, 2);
    let goal = GridCoord::new(8, 2);
    let (ground_path, _) = pf
        .find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
        .expect("ground Clear at mid remains walkable");
    assert!(
        ground_path.contains(&mid),
        "ground path may step Clear mid: {:?}",
        ground_path
    );

    let (top_path, _) = pf
        .find_path_on_layer(
            start,
            goal,
            PathfindLayerEnum::Top,
            SURFACE_GROUND,
            false,
            2000,
            false,
            None,
        )
        .expect("Top search can go around a single Obstacle");
    assert!(
        !top_path.contains(&mid),
        "Top path must not step Top Obstacle: {:?}",
        top_path
    );
    assert_eq!(top_path.first().copied(), Some(start));
    assert_eq!(top_path.last().copied(), Some(goal));
}

#[test]
fn reset_clears_layer_grids() {
    let mut pf = AStarPathfinder::new(4, 4);
    let c = GridCoord::new(2, 2);
    pf.set_cell_type(c, PathfindCellType::Water);
    pf.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Obstacle);
    pf.set_pinched_on_layer(c, PathfindLayerEnum::Top, true);
    assert_eq!(
        pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
        Some(PathfindCellType::Obstacle)
    );
    pf.reset();
    assert_eq!(pf.get_cell_type(c), Some(PathfindCellType::Clear));
    assert_eq!(
        pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
        None,
        "reset() must drop elevated layer grids"
    );
    assert_eq!(pf.is_pinched_on_layer(c, PathfindLayerEnum::Top), None);
}

#[test]
fn missing_elevated_cell_falls_back_to_ground_like_cpp_get_cell() {
    // C++ Pathfinder::getCell(layer, x, y) (AIPathfind.h:899-917):
    //   if layer > GROUND, try m_layers[layer].getCell; if NULL, return &m_map[x][y].
    // PathfindLayer::getCell also returns NULL for CELL_IMPASSABLE (cpp:3636-3638).
    //
    // Public get_cell_type_on_layer reports the *stored* elevated type (None if
    // missing) so callers can distinguish "no Top cell" from "Top == ground".
    // Search / is_passable_on_layer use get_cell_on_layer → ground fallback.
    let mut pf = AStarPathfinder::new(5, 3);
    let c = GridCoord::new(2, 1);
    pf.set_cell_type(c, PathfindCellType::Impassable);

    assert_eq!(
        pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
        None,
        "no Top slot written → stored type is None"
    );
    assert!(
        !pf.is_passable_on_layer(c, PathfindLayerEnum::Top, SURFACE_GROUND, false),
        "C++ getCell fallback: missing Top cell uses ground Impassable"
    );

    let start = GridCoord::new(0, 1);
    let goal = GridCoord::new(4, 1);
    // Full-height ground Impassable would block; here only one cell is Impassable
    // so both searches can detour. Stamp a full wall, then only one Top Clear.
    for y in 0..3 {
        pf.set_cell_type(GridCoord::new(2, y), PathfindCellType::Impassable);
    }
    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, false, 500, false, None)
            .is_none()
    );
    assert!(
        pf.find_path_on_layer(
            start,
            goal,
            PathfindLayerEnum::Top,
            SURFACE_GROUND,
            false,
            500,
            false,
            None,
        )
        .is_none(),
        "missing Top cells fall back to ground Impassable wall"
    );

    // Writing Top Clear at the wall opens only the Top layer (no fallback).
    for y in 0..3 {
        pf.set_cell_type_on_layer(
            GridCoord::new(2, y),
            PathfindLayerEnum::Top,
            PathfindCellType::Clear,
        );
    }
    assert_eq!(
        pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
        Some(PathfindCellType::Clear)
    );
    assert!(
        pf.find_path_on_layer(
            start,
            goal,
            PathfindLayerEnum::Top,
            SURFACE_GROUND,
            false,
            500,
            false,
            None,
        )
        .is_some()
    );
    assert!(
        pf.find_path(start, goal, SURFACE_GROUND, false, 500, false, None)
            .is_none()
    );

    // C++ PathfindLayer::getCell: CELL_IMPASSABLE on the layer is treated as
    // NULL, so getCell falls back to ground (still Impassable here).
    pf.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Impassable);
    assert_eq!(
        pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
        Some(PathfindCellType::Impassable),
        "stored type remains Impassable"
    );
    assert!(
        !pf.is_passable_on_layer(c, PathfindLayerEnum::Top, SURFACE_GROUND, false),
        "Impassable Top cell → getCell NULL → fall back to ground Impassable"
    );
}
