//! Source-derived `Pathfinder::updatePos` / `removePos` cell transitions.
use generals_main::game_logic::pathfinding::{GridPos, PathPositionFootprint, PathfindingGrid};
use glam::Vec3;

fn footprint(unit_id: u32, radius: f32) -> PathPositionFootprint {
    PathPositionFootprint {
        unit_id,
        player: 0,
        radius,
        layer: 1,
        on_ground: true,
        infantry: false,
        crushable_level: 255,
    }
}

#[test]
fn movement_keeps_only_current_cell_owners_in_each_grid() {
    let mut first = PathfindingGrid::new(200.0, 200.0, 10.0);
    let mut second = PathfindingGrid::new(200.0, 200.0, 10.0);
    let p = Vec3::new(50.0, 0.0, 50.0);
    let first_cell = first.update_path_position_cell(&footprint(7, 1.0), (-1, -1), p);
    let peer_cell = second.update_path_position_cell(&footprint(7, 1.0), (-1, -1), p);
    assert_eq!(first_cell, (5, 5));
    assert_eq!(peer_cell, first_cell);
    first.update_path_position_cell(&footprint(8, 1.0), (-1, -1), p);
    assert_eq!(first.ground_position_unit(GridPos::new(5, 5)), 8);
    let moved =
        first.update_path_position_cell(&footprint(7, 1.0), first_cell, Vec3::new(60.0, 0.0, 50.0));
    assert_eq!(moved, (6, 5));
    assert_eq!(
        first.ground_position_unit(GridPos::new(5, 5)),
        8,
        "moving the older actor must not erase the newer owner"
    );
    assert_eq!(first.ground_position_unit(GridPos::new(6, 5)), 7);
    assert_eq!(
        second.ground_position_unit(GridPos::new(5, 5)),
        7,
        "same numeric ID in another grid is independent"
    );
    first.clear_path_position_square(8, 1.0, first_cell, 1);
    assert_eq!(first.ground_position_unit(GridPos::new(5, 5)), 0);
    assert_eq!(second.ground_position_unit(GridPos::new(5, 5)), 7);
}

#[test]
fn same_cell_keeps_old_footprint_until_the_actor_changes_cells() {
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let position = Vec3::new(50.0, 0.0, 50.0);
    let cell = grid.update_path_position_cell(&footprint(7, 1.0), (-1, -1), position);
    // A box-sized geometry changes the cell rounding too, so choose a point
    // that still maps to the same cell for both center conventions.
    let same = grid.update_path_position_cell(&footprint(7, 20.0), cell, position);
    assert_eq!(same, cell);
    assert_eq!(grid.ground_position_unit(GridPos::new(6, 5)), 0);
    let moved =
        grid.update_path_position_cell(&footprint(7, 20.0), cell, Vec3::new(70.0, 0.0, 50.0));
    assert_ne!(moved, cell);
    assert_eq!(grid.ground_position_unit(GridPos::new(8, 5)), 7);
    grid.clear_path_position_square(7, 20.0, moved, 1);
    assert_eq!(grid.ground_position_unit(GridPos::new(8, 5)), 0);
}

#[test]
fn negative_previous_cell_does_not_remove_edge_claims() {
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let previous =
        grid.update_path_position_cell(&footprint(7, 20.0), (-1, -1), Vec3::new(-10.0, 0.0, 20.0));
    assert!(previous.0 < 0);
    assert_eq!(grid.ground_position_unit(GridPos::new(0, 2)), 7);
    grid.clear_path_position_square(7, 20.0, previous, 1);
    assert_eq!(
        grid.ground_position_unit(GridPos::new(0, 2)),
        7,
        "the original does not visit a negative old cell"
    );
    grid.clear_dynamic_blocks();
    assert_eq!(grid.ground_position_unit(GridPos::new(0, 2)), 0);
}

#[test]
fn bridge_position_updates_remove_the_old_ground_and_layer_claims() {
    use gamelogic::ai::pathfind_astar::PathfindLayerEnum;
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    use generals_main::game_logic::pathfinding::{PathGoalUpdate, PathfindingGround};

    let ground = PathfindingGround::Flat(0.0);
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let start = Vec3::new(40.0, 20.0, 50.0);
    let end = Vec3::new(80.0, 20.0, 50.0);
    let layer = grid.layer_for_destination(end, &ground);
    assert_ne!(layer, PathfindLayerEnum::Ground);
    let mut shape = footprint(7, 1.0);
    shape.layer = layer as u8;
    shape.on_ground = true;
    let old = grid.update_path_position_cell(&shape, (-1, -1), start);
    assert_eq!(grid.ground_position_unit(GridPos::new(4, 5)), 7);
    shape.on_ground = false;
    let new = grid.update_path_position_cell(&shape, old, end);
    assert_eq!(grid.ground_position_unit(GridPos::new(4, 5)), 0);
    assert_eq!(grid.ground_position_unit(GridPos::new(8, 5)), 0);
    grid.update_path_goal_cell(PathGoalUpdate {
        unit_id: 7,
        player: 0,
        radius: 1.0,
        old_cell: (-1, -1),
        old_layer: 1,
        new_layer: layer as u8,
        new_on_ground: false,
        goal: end,
    });
    grid.set_query_seeker_id(7);
    let cell = GridPos::new(8, 5);
    let destination = |grid: &PathfindingGrid| {
        grid.adjust_destination_on_layer(
            cell,
            SURFACE_GROUND,
            false,
            400,
            Some(0),
            0,
            layer,
            &ground,
        )
    };
    assert_ne!(destination(&grid), Some(cell));
    grid.clear_path_position_square(7, 1.0, new, layer as u8);
    assert_eq!(destination(&grid), Some(cell));
}
