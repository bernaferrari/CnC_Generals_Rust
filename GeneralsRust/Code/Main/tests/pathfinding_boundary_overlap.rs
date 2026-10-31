//! C++ PathfindCell::setTypeAsObstacle rejects replacing an existing obstacle.
use generals_main::game_logic::pathfinding::{PathfindingGrid, PathfindingGround};
use generals_main::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
use glam::Vec3;

fn structure(id: u32) -> Object {
    let mut template = ThingTemplate::new("BoundaryObstacle");
    template.add_kind_of(KindOf::Structure);
    let mut object = Object::new(template, ObjectId(id), Team::USA);
    object.set_position(Vec3::new(45.0, 0.0, 45.0));
    object.selection_radius = 20.0;
    object
}

#[test]
fn repeated_and_overlapping_boundary_classification_keeps_first_obstacle_owner() {
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let first = structure(11);
    let second = structure(12);
    let ground = PathfindingGround::Flat(0.0);
    let center = grid.world_to_grid(first.get_position());
    assert!(
        grid.classify_object_footprint(&first, false, &ground)
            .is_some()
    );
    assert!(
        grid.classify_object_footprint(&first, false, &ground)
            .is_some()
    );
    assert!(
        grid.classify_object_footprint(&second, false, &ground)
            .is_some()
    );
    assert!(!grid.clear_cell_obstacle_owned(center, second.id.0));
    assert!(grid.clear_cell_obstacle_owned(center, first.id.0));
}

#[test]
fn overlapping_fences_do_not_replace_an_existing_obstacle_owner() {
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let position = Vec3::new(45.0, 0.0, 45.0);
    let center = grid.world_to_grid(position);
    grid.set_cell_obstacle_owned(center, false, false, 11, Some(1), Some(Team::USA));
    assert!(
        grid.classify_fence_world(
            position,
            0.0,
            20.0,
            0.0,
            false,
            12,
            Some(2),
            Some(Team::GLA),
            false
        )
        .is_some()
    );
    assert!(!grid.clear_cell_obstacle_owned(center, 12));
    assert!(grid.clear_cell_obstacle_owned(center, 11));
}

#[test]
fn structure_reclassification_keeps_existing_rubble() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let object = structure(11);
    let center = grid.world_to_grid(object.get_position());
    grid.set_cell_type(center, PathfindCellType::Rubble);
    assert!(
        grid.classify_object_footprint(&object, false, &PathfindingGround::Flat(0.0))
            .is_some()
    );
    assert_eq!(grid.cell_type(center), PathfindCellType::Rubble);
}

#[test]
fn structure_classification_can_claim_an_impassable_cell() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let object = structure(11);
    let center = grid.world_to_grid(object.get_position());
    grid.set_cell_type(center, PathfindCellType::Impassable);
    assert!(
        grid.classify_object_footprint(&object, false, &PathfindingGround::Flat(0.0))
            .is_some()
    );
    assert_eq!(grid.cell_type(center), PathfindCellType::Obstacle);
    assert!(grid.clear_cell_obstacle_owned(center, object.id.0));
}
