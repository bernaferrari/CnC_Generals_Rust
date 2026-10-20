//! Search-result conversion preserves the supplied layer at overlapping XY.
//! This does not establish layered optimization or movement/save ownership.
use super::*;

fn overlapping_layers(deck_height: f32, wall_height: f32) -> PathfindingSystem {
    let mut system = PathfindingSystem::new(100.0, 100.0);
    let grid = &mut system.grid;
    grid.bridge_layers.push(HostBridgeLayer {
        id: 2,
        from_left: Vec3::new(0.0, deck_height, 0.0),
        from_right: Vec3::new(40.0, deck_height, 0.0),
        to_left: Vec3::new(0.0, deck_height, 40.0),
        to_right: Vec3::new(40.0, deck_height, 40.0),
        cells: HashMap::from([((1, 1), (PathfindCellType::Clear, 0))]),
        destroyed: false,
        object_id: 10,
        ground_connect_cells: Vec::new(),
    });
    grid.flight_bridge_order.push(2);
    grid.set_wall_height(wall_height);
    grid.add_wall_piece(11, Vec3::new(15.0, 0.0, 15.0), 0.0, 30.0, 30.0);
    system
}

fn same_cell_on_each_layer() -> [(GridCoord, PathfindLayerEnum); 3] {
    let cell = GridCoord::new(1, 1);
    [
        (cell, PathfindLayerEnum::Ground),
        (cell, PathfindLayerEnum::from_u32(2)),
        (cell, PathfindLayerEnum::Wall),
    ]
}

#[test]
fn searched_layer_conversion_distinguishes_ground_bridge_and_wall_at_same_xy() {
    let system = overlapping_layers(30.0, 50.0);
    assert_eq!(system.grid.host_deck_layer_at(GridPos::new(1, 1)), Some(2));
    let points = system.crate_path_to_world(
        &same_cell_on_each_layer(),
        &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
    );
    assert_eq!(
        points.iter().map(|p| p.y).collect::<Vec<_>>(),
        vec![0.0, 30.0, 50.0]
    );
    assert_eq!((points[0].x, points[0].z), (points[1].x, points[1].z));
    assert_eq!((points[1].x, points[1].z), (points[2].x, points[2].z));
}

#[test]
fn searched_layer_conversion_reads_only_its_receiving_grid() {
    let first = overlapping_layers(30.0, 50.0);
    let expected = first.crate_path_to_world(
        &same_cell_on_each_layer(),
        &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
    );
    let mut second = overlapping_layers(70.0, 90.0);
    assert_eq!(
        second.crate_path_to_world(
            &same_cell_on_each_layer(),
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        )[1]
        .y,
        70.0
    );
    second.grid.bridge_layers.clear();
    second.grid.set_wall_height(100.0);
    assert_eq!(
        first.crate_path_to_world(
            &same_cell_on_each_layer(),
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        ),
        expected
    );
}
