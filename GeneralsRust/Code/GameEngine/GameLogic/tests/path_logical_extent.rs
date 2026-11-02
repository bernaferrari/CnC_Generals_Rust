use gamelogic::ai::pathfind_complete::{PathRequest, PathfindingSystem, SURFACE_GROUND};
use glam::{IVec2 as ICoord2D, Vec3 as Coord3D};

#[test]
fn human_path_reenters_from_an_adjacent_cell_without_admitting_outside_neighbors() {
    let mut world = PathfindingSystem::new(20, 20);
    let mut peer = PathfindingSystem::new(20, 20);
    for finder in [&mut world, &mut peer] {
        finder.new_map();
        finder.set_logical_extent(ICoord2D::new(0, 0), ICoord2D::new(9, 19));
    }
    let request = |from: f32, to: f32| {
        let mut req = PathRequest::new(
            Coord3D::new(from, 55.0, 0.0),
            Coord3D::new(to, 55.0, 0.0),
            SURFACE_GROUND,
        );
        req.is_human = true;
        req
    };
    let expected = peer.find_path(request(105.0, 85.0));
    let reentry = world.find_path(request(105.0, 85.0));
    assert!(
        reentry.success,
        "an inside neighbor is reachable from the outside seed"
    );
    assert_eq!(reentry.waypoints, expected.waypoints);
    assert!(
        !world.find_path(request(115.0, 85.0)).success,
        "a second outside cell cannot be admitted as a neighbor"
    );
    assert!(
        !world.find_path(request(85.0, 105.0)).success,
        "a human cannot step to an outside goal"
    );
    assert!(
        world.find_path(request(105.0, 105.0)).success,
        "the outside starting cell is already the goal"
    );
    assert_eq!(
        peer.find_path(request(105.0, 85.0)).waypoints,
        expected.waypoints
    );
}
