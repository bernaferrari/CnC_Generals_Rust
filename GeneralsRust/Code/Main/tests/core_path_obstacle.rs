use gamelogic::ai::AI;
use gamelogic::ai::pathfind_astar::GridCoord;
use gamelogic::ai::pathfinding_system::{PathfindLayerEnum, TerrainType};
use glam::Vec3;

#[test]
fn removing_a_different_object_keeps_the_original_obstacle() {
    let first = AI::new();
    let second = AI::new();
    let pos = Vec3::new(25.0, 25.0, 0.0);
    for ai in [&first, &second] {
        ai.pathfinder()
            .unwrap()
            .write()
            .unwrap()
            .reset_with_size(8, 8);
    }
    let first_facade = first.pathfinding_system().unwrap();
    let second_facade = second.pathfinding_system().unwrap();
    let mut first_facade = first_facade.write().unwrap();
    let mut second_facade = second_facade.write().unwrap();
    first_facade.add_obstacle(41, &[pos], PathfindLayerEnum::Ground);
    second_facade.add_obstacle(42, &[pos], PathfindLayerEnum::Ground);
    // C++ setTypeAsObstacle does not replace an already occupied cell.
    first_facade.add_obstacle(42, &[pos], PathfindLayerEnum::Ground);

    first_facade.remove_obstacle(42, &[pos], PathfindLayerEnum::Ground);
    assert!(!first_facade.is_cell_clear_at(&pos, gamelogic::common::PathfindLayerEnum::Ground));
    assert!(!second_facade.is_cell_clear_at(&pos, gamelogic::common::PathfindLayerEnum::Ground));

    first_facade.remove_obstacle(41, &[pos], PathfindLayerEnum::Ground);
    assert!(first_facade.is_cell_clear_at(&pos, gamelogic::common::PathfindLayerEnum::Ground));
    assert!(!second_facade.is_cell_clear_at(&pos, gamelogic::common::PathfindLayerEnum::Ground));
}

#[test]
fn removing_a_rubble_footprint_clears_the_cell() {
    let ai = AI::new();
    ai.pathfinder()
        .unwrap()
        .write()
        .unwrap()
        .reset_with_size(8, 8);
    let facade = ai.pathfinding_system().unwrap();
    let mut facade = facade.write().unwrap();
    let pos = Vec3::new(25.0, 25.0, 0.0);
    facade.set_terrain(&pos, PathfindLayerEnum::Ground, TerrainType::Rubble);
    facade.remove_obstacle(41, &[pos], PathfindLayerEnum::Ground);
    assert!(facade.is_cell_clear_at(&pos, gamelogic::common::PathfindLayerEnum::Ground));
}

#[test]
fn bridge_obstacle_removal_preserves_ground_and_owner() {
    let ai = AI::new();
    let pathfinder = ai.pathfinder().unwrap();
    let layer = {
        let mut pathfinder = pathfinder.write().unwrap();
        pathfinder.reset_with_size(8, 8);
        pathfinder.add_bridge((GridCoord::new(1, 1), GridCoord::new(4, 4)))
    };
    let layer = gamelogic::common::PathfindLayerEnum::from_u32(layer as u32);
    let facade = ai.pathfinding_system().unwrap();
    let mut facade = facade.write().unwrap();
    let pos = Vec3::new(25.0, 25.0, 0.0);
    let facade_layer = PathfindLayerEnum::from(layer);
    facade.add_obstacle(41, &[pos], facade_layer);
    assert!(!facade.is_cell_clear_at(&pos, layer));
    assert!(facade.is_cell_clear_at(&pos, gamelogic::common::PathfindLayerEnum::Ground));
    facade.remove_obstacle(42, &[pos], facade_layer);
    assert!(!facade.is_cell_clear_at(&pos, layer));
    facade.remove_obstacle(41, &[pos], facade_layer);
    assert!(facade.is_cell_clear_at(&pos, layer));
    assert!(facade.is_cell_clear_at(&pos, gamelogic::common::PathfindLayerEnum::Ground));
}
