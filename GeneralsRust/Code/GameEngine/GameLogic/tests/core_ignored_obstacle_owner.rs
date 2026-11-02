use game_engine::system::geometry::{GeometryInfo as Shape, GeometryType};
use gamelogic::ai::pathfind_astar::{GridCoord, PathfindCellType};
use gamelogic::ai::pathfind_complete::{PathfindingSystem, SURFACE_GROUND};
use gamelogic::common::{DefaultThingTemplate, GeometryInfo, KindOf, ObjectStatusMaskType};
use gamelogic::object::Object;
use glam::Vec3;
use std::sync::Arc;

fn structure(id: u32, x: f32) -> Object {
    let mut template = DefaultThingTemplate::new("Obstacle".into());
    template.add_kind_of(KindOf::Structure);
    template.add_kind_of(KindOf::Immobile);
    let mut object = Object::new_raw(Arc::new(template), id, ObjectStatusMaskType::NONE, None);
    object.set_geometry_info(GeometryInfo {
        shape: Shape::new(GeometryType::Box, false, 10.0, 5.0, 10.0),
        ..GeometryInfo::default()
    });
    object.set_position(&Vec3::new(x, 55.0, 0.0)).unwrap();
    object
}

#[test]
fn ignored_obstacle_is_resolved_from_the_receiving_grid() {
    let first = structure(41, 55.0);
    let second = structure(41, 25.0);
    let other = structure(42, 85.0);
    let mut world = PathfindingSystem::new(12, 10);
    let mut peer = PathfindingSystem::new(12, 10);
    world.classify_object_footprint(&first).unwrap();
    world.classify_object_footprint(&other).unwrap();
    peer.classify_object_footprint(&second).unwrap();
    let first_cell = GridCoord::new(5, 5).to_world(0.0);
    let other_cell = GridCoord::new(8, 5).to_world(0.0);
    assert_eq!(
        world.get_cell_type(&first_cell),
        Some(PathfindCellType::Obstacle)
    );
    assert!(world.valid_movement_position(SURFACE_GROUND, false, &first_cell, Some(41)));
    assert!(!world.valid_movement_position(SURFACE_GROUND, false, &first_cell, Some(42)));
    assert!(!world.valid_movement_position(
        SURFACE_GROUND,
        false,
        &first_cell,
        Some(gamelogic::common::INVALID_ID),
    ));
    assert!(!world.valid_movement_position(SURFACE_GROUND, false, &other_cell, Some(41)));
    assert!(world.valid_movement_position(SURFACE_GROUND, false, &other_cell, Some(42)));
    let anonymous = GridCoord::new(10, 1).to_world(0.0);
    world.set_cell_type(&anonymous, PathfindCellType::Obstacle);
    assert!(!world.valid_movement_position(SURFACE_GROUND, false, &anonymous, Some(41)));
    let peer_cell = GridCoord::new(2, 5).to_world(0.0);
    assert!(peer.valid_movement_position(SURFACE_GROUND, false, &peer_cell, Some(41)));
    world.classify_object_footprint_ex(&first, false).unwrap();
    assert!(peer.valid_movement_position(SURFACE_GROUND, false, &peer_cell, Some(41)));
    assert!(!peer.valid_movement_position(SURFACE_GROUND, false, &peer_cell, Some(42)));
}

#[test]
fn path_search_ignores_only_the_receiving_grids_obstacle_owner() {
    use gamelogic::ai::pathfind_complete::PathRequest;
    use gamelogic::common::INVALID_ID;
    let first = structure(41, 55.0);
    let second = structure(42, 55.0);
    let mut world = PathfindingSystem::new(12, 10);
    let mut peer = PathfindingSystem::new(12, 10);
    world.new_map();
    peer.new_map();
    world.classify_object_footprint(&first).unwrap();
    peer.classify_object_footprint(&second).unwrap();
    world.new_map_from_classified_cells();
    peer.new_map_from_classified_cells();
    let request = PathRequest {
        object_id: INVALID_ID,
        from: GridCoord::new(1, 5).to_world(0.0),
        to: GridCoord::new(5, 5).to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: Some(41),
        is_human: false,
    };
    assert!(
        world.find_path(request.clone()).success,
        "the world must use its own stored obstacle identity"
    );
    assert!(
        !peer.find_path(request.clone()).success,
        "a different obstacle owner cannot be ignored"
    );
    assert!(
        world.find_path(request).success,
        "the peer query must not affect the first world"
    );
}
