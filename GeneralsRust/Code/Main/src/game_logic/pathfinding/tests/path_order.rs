use super::super::{GridPos, PathfindCellType, PathfindingSystem, SURFACE_GROUND};

/// C++ findClosestPath uses the same FIFO total-cost open list as findPath.
/// Symmetric closest cells have equal screen-distance and equal route cost;
/// `examineNeighboringCells` inserts north before south.
#[test]
fn closest_path_equal_cost_candidates_keep_cpp_fifo_order() {
    let mut system = PathfindingSystem::new(80.0, 60.0);
    for blocked in [
        GridPos::new(3, 2),
        GridPos::new(4, 2),
        GridPos::new(5, 2),
        GridPos::new(6, 2),
    ] {
        system
            .grid
            .set_cell_type(blocked, PathfindCellType::Impassable);
    }

    let start = system.grid.grid_to_world(GridPos::new(1, 2));
    let goal = system.grid.grid_to_world(GridPos::new(5, 2));
    let path = system
        .find_closest_path(start, goal, SURFACE_GROUND, false, true, 0.2)
        .expect("closest reachable path");
    let end = system
        .grid
        .world_to_grid(*path.last().expect("path endpoint"));

    assert_eq!(
        end,
        GridPos::new(4, 3),
        "C++ FIFO order keeps the north candidate; route was {path:?}"
    );
}
