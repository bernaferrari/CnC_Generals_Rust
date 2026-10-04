//! Traversal contracts formerly tested against the unused raw-pointer Pathfinder.
//! These requests use the current AI Pathfinder, its classified cells and normal
//! zone/build/optimization pipeline. They do not establish live UnitAI movement.
use super::*;

fn classified_grid(width: usize, height: usize) -> crate::ai::Pathfinder {
    let mut owner = crate::ai::Pathfinder::new();
    owner.reset_with_size(width, height);
    for x in 0..width {
        for y in 0..height {
            owner
                .inner
                .pathfinder
                .set_cell_type(GridCoord::new(x as i32, y as i32), PathfindCellType::Clear);
        }
    }
    owner
}

fn request(surfaces: u32, crusher: bool) -> PathRequest {
    let mut request = PathRequest::new(
        Coord3D::new(5.0, 25.0, 0.0),
        Coord3D::new(85.0, 25.0, 0.0),
        surfaces,
    );
    request.is_crusher = crusher;
    request.unit_radius = 5.0;
    request
}

fn assert_endpoints(path: &PathResult, request: &PathRequest) {
    assert!(path.success, "classified map must admit the requested path");
    assert!(!path.waypoints.is_empty());
    assert_eq!(
        GridCoord::from_world(&path.waypoints[0]),
        GridCoord::from_world(&request.from)
    );
    assert_eq!(
        GridCoord::from_world(path.waypoints.last().unwrap()),
        GridCoord::from_world(&request.to)
    );
}

fn crosses_column(points: &[Coord3D], column: i32) -> bool {
    points.windows(2).any(|pair| {
        let lo = pair[0].x.min(pair[1].x);
        let hi = pair[0].x.max(pair[1].x);
        lo < (column + 1) as f32 * PATHFIND_CELL_SIZE_F
            && hi >= column as f32 * PATHFIND_CELL_SIZE_F
    })
}

#[test]
fn classified_cliff_blocks_ground_path() {
    // Replaces steep_slope_blocks_ground_path's invented slope=.5 adapter.
    // CPP AIPathfind.cpp:4485-4504 obtains CELL_CLIFF from TerrainLogic.
    // This starts at that classification boundary; terrain height classification
    // itself is not exercised by this test.
    let mut owner = classified_grid(3, 1);
    owner
        .inner
        .pathfinder
        .set_cell_type(GridCoord::new(1, 0), PathfindCellType::Cliff);
    owner.inner.new_map_from_classified_cells();
    let request = PathRequest::new(
        Coord3D::new(5.0, 5.0, 0.0),
        Coord3D::new(25.0, 5.0, 0.0),
        SURFACE_GROUND,
    );
    assert!(!owner.find_path_result(request).success);
}

#[test]
fn crusher_fence_passability_does_not_bypass_cpp_quick_zone_gate() {
    // CPP AIPathfind.cpp:4840-4842 admits only fence obstacles for crushers.
    let mut owner = classified_grid(9, 5);
    let middle = GridCoord::new(4, 2);
    for y in 0..5 {
        owner
            .inner
            .pathfinder
            .set_cell_obstacle_id(GridCoord::new(4, y), 1, false, false);
    }
    owner.inner.new_map_from_classified_cells();
    assert!(!owner.inner.pathfinder.is_obstacle_fence(middle));
    assert!(
        !owner
            .inner
            .pathfinder
            .is_passable(middle, SURFACE_GROUND, true)
    );
    assert!(
        !owner
            .inner
            .pathfinder
            .is_passable(middle, SURFACE_GROUND, false)
    );
    assert!(
        !owner
            .find_path_result(request(SURFACE_GROUND, true))
            .success
    );
    assert!(
        !owner
            .find_path_result(request(SURFACE_GROUND, false))
            .success
    );

    for y in 0..5 {
        owner
            .inner
            .pathfinder
            .set_cell_obstacle_id(GridCoord::new(4, y), 2, true, false);
    }
    owner.inner.clear_cache();
    owner.inner.new_map_from_classified_cells();
    assert!(owner.inner.pathfinder.is_obstacle_fence(middle));
    assert!(
        !owner
            .inner
            .pathfinder
            .is_passable(middle, SURFACE_GROUND, false)
    );
    assert!(
        owner
            .inner
            .pathfinder
            .is_passable(middle, SURFACE_GROUND, true)
    );
    assert!(
        !owner
            .find_path_result(request(SURFACE_GROUND, false))
            .success
    );
    let request = request(SURFACE_GROUND, true);
    let start = GridCoord::from_world(&request.from);
    let goal = GridCoord::from_world(&request.to);
    assert!(
        owner
            .inner
            .zones
            .are_connected(start, goal, SURFACE_GROUND, true)
    );
    assert!(
        !owner
            .inner
            .zones
            .are_connected(start, goal, SURFACE_GROUND, false)
    );
    // CPP calculateZones:2639-2690 joins fence/ground only in crusherZones.
    // getEffectiveZone:3119-3171 selects that table only when crusher=true.
    // The public quick gate:7997-8050 intentionally uses false when both
    // endpoints are clear; findPath:6367 must reject this fully rezoned wall.
    assert!(!owner.inner.client_safe_quick_does_path_exist(
        SURFACE_GROUND,
        &request.from,
        &request.to
    ));
    assert!(!owner.find_path_result(request).success);
    // Existing game_pathfinding::tests::crusher_find_path_only_crosses_fence_obstacles_like_cpp
    // retains the admitted A* crossing contract. It is not a public quick-gate test.
    // CPP:6316-6318 adds 100*COST_ORTHOGONAL to base movement cost; the
    // retired raw traversal_cost's whole-cost assertion was not that contract.
}

#[test]
fn air_find_path_crosses_obstacle_impassable_and_bridge_impassable() {
    // CPP:4741-4780 permits AIR over each of these cell types.
    for cell_type in [
        PathfindCellType::Obstacle,
        PathfindCellType::Impassable,
        PathfindCellType::BridgeImpassable,
    ] {
        let mut owner = classified_grid(9, 5);
        for y in 0..5 {
            let cell = GridCoord::new(4, y);
            if cell_type == PathfindCellType::Obstacle {
                owner
                    .inner
                    .pathfinder
                    .set_cell_obstacle_id(cell, 1, false, false);
            } else {
                owner.inner.pathfinder.set_cell_type(cell, cell_type);
            }
        }
        owner.inner.new_map_from_classified_cells();
        let middle = GridCoord::new(4, 2);
        assert!(
            owner
                .inner
                .pathfinder
                .is_passable(middle, SURFACE_AIR, false)
        );
        assert!(
            !owner
                .inner
                .pathfinder
                .is_passable(middle, SURFACE_GROUND, false)
        );
        assert!(
            !owner
                .find_path_result(request(SURFACE_GROUND, false))
                .success
        );
        let request = request(SURFACE_AIR, false);
        let path = owner.find_path_result(request.clone());
        assert_endpoints(&path, &request);
        assert!(crosses_column(&path.waypoints, 4));
    }
}

#[test]
fn crusher_find_path_goes_around_solid_building_not_through() {
    let mut owner = classified_grid(9, 5);
    for x in 3..=5 {
        for y in 1..=3 {
            owner
                .inner
                .pathfinder
                .set_cell_obstacle_id(GridCoord::new(x, y), 1, false, false);
        }
    }
    owner.inner.new_map_from_classified_cells();
    assert!(
        !owner
            .inner
            .pathfinder
            .is_passable(GridCoord::new(4, 2), SURFACE_GROUND, true)
    );
    for crusher in [false, true] {
        let request = request(SURFACE_GROUND, crusher);
        let path = owner.find_path_result(request.clone());
        assert_endpoints(&path, &request);
        // Sampling the optimized segments also rejects a corner-cut through
        // the building; testing raw waypoint positions alone is insufficient.
        for pair in path.waypoints.windows(2) {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            let steps = ((dx * dx + dy * dy).sqrt() / 2.5).ceil().max(1.0) as usize;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let cell = GridCoord::from_world(&Coord3D::new(
                    pair[0].x + dx * t,
                    pair[0].y + dy * t,
                    0.0,
                ));
                assert!(
                    !(3..=5).contains(&cell.x) || !(1..=3).contains(&cell.y),
                    "path must go around solid building: {cell:?}"
                );
            }
        }
    }
}
