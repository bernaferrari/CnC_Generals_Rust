use gamelogic::ai::pathfind_astar::{GridCoord, PathfindCellType};
use gamelogic::ai::pathfind_complete::{PathRequest, PathfindingSystem, SURFACE_GROUND};
use gamelogic::common::INVALID_ID;

#[test]
fn repeated_search_observes_current_world_without_affecting_peer() {
    let mut world = PathfindingSystem::new(16, 16);
    let mut peer = PathfindingSystem::new(16, 16);
    world.new_map();
    peer.new_map();
    let request = PathRequest {
        object_id: INVALID_ID,
        from: GridCoord::new(2, 2).to_world(0.0),
        to: GridCoord::new(10, 2).to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    assert!(world.find_path(request.clone()).success);
    assert!(peer.find_path(request.clone()).success);
    world.set_cell_type(&request.to, PathfindCellType::Impassable);
    assert!(
        !world.find_path(request.clone()).success,
        "changed destination must be checked again"
    );
    assert!(
        peer.find_path(request.clone()).success,
        "peer grid must not change"
    );
    world.set_cell_type(&request.to, PathfindCellType::Clear);
    assert!(
        world.find_path(request).success,
        "restored destination must be searched again"
    );
}
