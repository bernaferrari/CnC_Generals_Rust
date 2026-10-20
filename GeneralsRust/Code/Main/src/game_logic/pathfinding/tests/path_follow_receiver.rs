//! CPP AIPathfind.cpp:5007 receiver occupancy exemption on path following.
use super::super::*;

fn grid() -> PathfindingGrid {
    PathfindingGrid::new(100.0, 100.0, 10.0)
}

fn stamp(grid: &mut PathfindingGrid, cell: GridPos, receiver: u32, moving: bool) {
    grid.mark_occupancy(
        cell,
        1,
        moving,
        true,
        false,
        255,
        receiver,
        PathfindLayerEnum::Ground,
    );
}

fn lead(grid: &PathfindingGrid, receiver: Option<u32>, from: GridPos, to: GridPos) -> Vec3 {
    let start = grid.grid_to_world(
        from,
        &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
    );
    let next = grid.grid_to_world(
        to,
        &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
    );
    PathfindingSystem::compute_point_on_path_for(
        start,
        &[start, next],
        Some(grid),
        SURFACE_GROUND,
        false,
        None,
        0,
        None,
        receiver,
    )
}

#[test]
fn path_follow_skips_only_own_fixed_and_moving_footprint() {
    let from = GridPos::new(2, 2);
    let next = GridPos::new(5, 2);
    for moving in [false, true] {
        let mut grid = grid();
        // A two-cell receiver footprint must not block its own line query,
        // including a fixed stamp retained from the path-admission phase.
        stamp(&mut grid, from, 7, moving);
        stamp(&mut grid, GridPos::new(3, 2), 7, moving);
        assert_eq!(
            lead(&grid, Some(7), from, next),
            grid.grid_to_world(
                next,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
        );
        if !moving {
            assert_eq!(
                lead(&grid, None, from, next),
                grid.grid_to_world(
                    from,
                    &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
                )
            );
        }
    }
}

#[test]
fn path_follow_foreign_fixed_footprint_still_blocks() {
    let mut grid = grid();
    let from = GridPos::new(2, 2);
    let next = GridPos::new(5, 2);
    stamp(&mut grid, from, 7, false);
    stamp(&mut grid, GridPos::new(3, 2), 8, false);
    assert_eq!(
        lead(&grid, Some(7), from, next),
        grid.grid_to_world(
            from,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        )
    );
    // Even a stale search identifying the blocker cannot admit the caller.
    grid.set_query_seeker_id(8);
    assert_eq!(
        lead(&grid, Some(7), from, next),
        grid.grid_to_world(
            from,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        )
    );
}

#[test]
fn path_follow_receiver_is_independent_of_interleaved_search_identity() {
    let mut grid = grid();
    let first = GridPos::new(2, 2);
    let second = GridPos::new(2, 6);
    let first_next = GridPos::new(5, 2);
    let second_next = GridPos::new(5, 6);
    stamp(&mut grid, first, 7, false);
    stamp(&mut grid, second, 8, false);
    for stale_search in [8, 7, 0, 8] {
        grid.set_query_seeker_id(stale_search);
        assert_eq!(
            lead(&grid, Some(7), first, first_next),
            grid.grid_to_world(
                first_next,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
        );
        assert_eq!(
            lead(&grid, Some(8), second, second_next),
            grid.grid_to_world(
                second_next,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
        );
        assert_eq!(
            lead(&grid, Some(8), first, first_next),
            grid.grid_to_world(
                first,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
        );
        assert_eq!(
            lead(&grid, Some(7), second, second_next),
            grid.grid_to_world(
                second,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
        );
    }
}

#[test]
fn path_follow_own_occupancy_does_not_open_terrain_or_pinched_gate() {
    let mut grid = grid();
    let from = GridPos::new(2, 2);
    let next = GridPos::new(5, 2);
    stamp(&mut grid, from, 7, false);
    grid.set_cell_type(from, PathfindCellType::Impassable);
    assert_eq!(
        lead(&grid, Some(7), from, next),
        grid.grid_to_world(
            from,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        )
    );
    grid.set_cell_type(from, PathfindCellType::Clear);
    grid.set_pinched(from, true);
    assert!(!grid.line_passable_for_receiver(
        from,
        next,
        SURFACE_GROUND,
        false,
        false,
        None,
        0,
        false,
        None,
        Some(7),
    ));
    // The path-follow route deliberately allows pinched cells, as C++ does.
    assert_eq!(
        lead(&grid, Some(7), from, next),
        grid.grid_to_world(
            next,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        )
    );
}
