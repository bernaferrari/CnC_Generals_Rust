//! hq-8ecpbu: PathNode layer identity survives search → optimize → Movement → updateGoal.
//! Overlapping bridge/ground heights must not re-pick the layer from geometry.

use super::*;
use glam::Vec3;

fn deck_grid(deck_height: f32) -> PathfindingGrid {
    let mut grid = PathfindingGrid::new(200.0, 120.0, 10.0);
    grid.bridge_layers.push(HostBridgeLayer {
        id: 2,
        from_left: Vec3::new(30.0, deck_height, 40.0),
        from_right: Vec3::new(30.0, deck_height, 60.0),
        to_left: Vec3::new(130.0, deck_height, 40.0),
        to_right: Vec3::new(130.0, deck_height, 60.0),
        cells: std::collections::HashMap::from([
            ((4, 5), (PathfindCellType::Clear, 0)),
            ((8, 5), (PathfindCellType::Clear, 0)),
            ((12, 5), (PathfindCellType::Clear, 0)),
        ]),
        destroyed: false,
        object_id: 10,
        ground_connect_cells: Vec::new(),
    });
    grid.flight_bridge_order.push(2);
    grid
}

#[test]
fn optimize_ground_path_preserves_carried_bridge_layer_over_geometry() {
    // Deck height equals Flat ground sample so geometry cannot distinguish.
    let grid = deck_grid(0.0);
    let ground = PathfindingGround::Flat(0.0);
    let bridge = PathfindLayerEnum::from_u32(2);
    assert_ne!(bridge, PathfindLayerEnum::Ground);

    let waypoints = vec![
        Vec3::new(40.0, 0.0, 50.0),
        Vec3::new(80.0, 0.0, 50.0),
        Vec3::new(120.0, 0.0, 50.0),
    ];
    let layers = vec![bridge as u8, bridge as u8, bridge as u8];
    let (opt, opt_layers) = grid.optimize_ground_path_ex_layered(
        &waypoints,
        &layers,
        SURFACE_GROUND,
        false,
        None,
        0,
        &ground,
    );
    assert!(!opt.is_empty());
    assert_eq!(opt.len(), opt_layers.len());
    assert!(
        opt_layers.iter().all(|&l| l == bridge as u8),
        "carried bridge PathNode layers must survive optimize; got {opt_layers:?}"
    );
}

#[test]
fn movement_last_path_layer_requires_parallel_identity() {
    let mut movement = crate::game_logic::host_types::Movement::default();
    movement.set_path_with_layers(
        vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
        vec![1, 2],
    );
    assert_eq!(movement.last_path_layer(), Some(2));
    movement.set_path_coords(vec![Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)]);
    assert_eq!(movement.last_path_layer(), None);
    movement.set_path_with_layers(
        vec![Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)],
        vec![1], // length mismatch
    );
    assert!(movement.path_layers.is_empty());
    assert_eq!(movement.last_path_layer(), None);
}

#[test]
fn crate_path_to_world_layered_keeps_search_layers() {
    let mut system = PathfindingSystem::new(200.0, 120.0);
    system.grid = deck_grid(20.0);
    let ground = PathfindingGround::Flat(0.0);
    let bridge = PathfindLayerEnum::from_u32(2);
    let cells = [
        (GridCoord::new(4, 5), PathfindLayerEnum::Ground),
        (GridCoord::new(8, 5), bridge),
        (GridCoord::new(12, 5), bridge),
    ];
    let (points, layers) = system.crate_path_to_world_layered(&cells, &ground);
    assert_eq!(points.len(), 3);
    assert_eq!(
        layers,
        vec![PathfindLayerEnum::Ground as u8, bridge as u8, bridge as u8]
    );
}
