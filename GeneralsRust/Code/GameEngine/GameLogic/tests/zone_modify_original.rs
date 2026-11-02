use game_pathfinding::{AStarPathfinder, GridCoord, PathfindCellType};
use gamelogic::ai::pathfind_complete::{UNINITIALIZED_ZONE, ZONE_BLOCK_SIZE};

#[path = "../src/ai/pathfind_complete/zone_modify.rs"]
mod zone_modify;

#[test]
fn modified_zones_follow_the_original_two_pass_block_order() {
    let mut grid = AStarPathfinder::new(20, 10);
    let mut zones = vec![vec![1; 10]; 20];
    for column in &mut zones[10..] {
        column.fill(2);
    }
    let mut bridges = [true, true];
    let global = (GridCoord::new(0, 0), GridCoord::new(19, 9));
    let bounds = (GridCoord::new(4, 4), GridCoord::new(5, 4));
    for x in [4, 5] {
        grid.set_cell_type(GridCoord::new(x, 4), PathfindCellType::Obstacle);
        zones[x as usize][4] = 0;
    }
    zone_modify::update_modified_zones(&mut zones, &grid, global, bounds, |x, _| {
        bridges[x] = false
    });
    assert_eq!((zones[4][4], zones[5][4], zones[6][4]), (0, 0, 1));
    assert_eq!(bridges, [false, true]);

    grid.set_cell_type(GridCoord::new(4, 4), PathfindCellType::Clear);
    zones[4][4] = 0;
    zone_modify::update_modified_zones(&mut zones, &grid, global, bounds, |x, _| {
        bridges[x] = false
    });
    assert_eq!((zones[4][4], zones[5][4]), (1, 0));

    zones[9][5] = 0;
    zones[10][5] = 0;
    let crossing = (GridCoord::new(9, 5), GridCoord::new(10, 5));
    zone_modify::update_modified_zones(&mut zones, &grid, global, crossing, |x, _| {
        bridges[x] = false
    });
    assert_eq!((zones[9][5], zones[10][5]), (1, 2));
    assert_eq!(bridges, [false, false]);

    for (x, column) in zones.iter_mut().enumerate() {
        for (y, zone) in column.iter_mut().enumerate() {
            grid.set_cell_type(
                GridCoord::new(x as i32, y as i32),
                PathfindCellType::Obstacle,
            );
            *zone = 3;
        }
    }
    for (x, y, zone) in [(4, 4, 0), (5, 4, 0), (5, 3, 1)] {
        grid.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
        zones[x as usize][y as usize] = zone;
    }
    let diagonal = (GridCoord::new(3, 3), GridCoord::new(5, 5));
    zone_modify::update_modified_zones(&mut zones, &grid, global, diagonal, |_, _| {});
    assert_eq!((zones[4][4], zones[5][4]), (1, 1));
    zones[4][4] = 0;
    grid.set_cell_type(GridCoord::new(5, 4), PathfindCellType::Obstacle);
    zones[5][4] = 3;
    zone_modify::update_modified_zones(&mut zones, &grid, global, diagonal, |_, _| {});
    assert_eq!(zones[4][4], 0);
}
