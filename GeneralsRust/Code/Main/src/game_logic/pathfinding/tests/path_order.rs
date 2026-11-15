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

    let start = system.grid.grid_to_world(
        GridPos::new(1, 2),
        &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
    );
    let goal = system.grid.grid_to_world(
        GridPos::new(5, 2),
        &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
    );
    let path = system
        .find_closest_path(
            start,
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.2,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
            super::super::standalone_path_fixture().read(),
            crate::game_logic::ObjectId(1),
        )
        .expect("closest reachable path").coords();
    let end = system
        .grid
        .world_to_grid(*path.last().expect("path endpoint"));

    assert_eq!(
        end,
        GridPos::new(4, 3),
        "C++ FIFO order keeps the north candidate; route was {path:?}"
    );
}

#[test]
fn route_projection_uses_skipped_raw_bridge_predecessor() {
    use super::super::PathRoute;
    use glam::Vec3;

    let route = PathRoute::from_layered(
        vec![Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0), Vec3::new(80.0, 0.0, 0.0)],
        vec![2, 2, 1, 1],
    );
    let mut nodes = route.nodes().to_vec();
    nodes[0].next_optimized = Some(2);
    let route = PathRoute::from_nodes(nodes, route.blocked_by_ally).expect("valid route topology");
    let (_, ordinal, layer) = PathfindingSystem::compute_point_on_route_for(
        Vec3::new(50.0, 0.0, 0.0), &route, 0, None, SURFACE_GROUND, false, None, None,
    );
    assert_eq!(ordinal, 1);
    assert_eq!(layer, Some(1), "the raw bridge predecessor makes the close node's ground layer authoritative");
}

#[test]
fn route_projection_respects_cliff_no_optimize_flag() {
    use super::super::PathRoute;
    use glam::Vec3;

    let route = PathRoute::from_layered(
        vec![Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), Vec3::new(40.0, 0.0, 0.0)],
        vec![1, 1, 1],
    );
    let project = |route: &PathRoute| PathfindingSystem::compute_point_on_route_for(
        Vec3::new(15.0, 0.0, 0.0), route, 0, None, SURFACE_GROUND, false, None, None,
    ).0;
    assert_eq!(project(&route), Vec3::new(20.0, 0.0, 0.0));
    let mut nodes = route.nodes().to_vec();
    nodes[1].can_optimize = true;
    let route = PathRoute::from_nodes(nodes, route.blocked_by_ally).expect("valid route topology");
    assert_eq!(project(&route), Vec3::new(30.0, 0.0, 0.0));
}

#[test]
fn secondary_searches_keep_parent_chain_ally_and_optimization_metadata() {
    use super::super::{PathfindingGround, PathRoute};
    use crate::game_logic::{HostObjectStore, KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    let start = Vec3::new(55.3, 0.0, 55.0);
    let goal = Vec3::new(125.0, 0.0, 55.0);
    let ground = PathfindingGround::Flat(0.0);
    let avoided = [Vec3::new(55.0, 0.0, 55.0), Vec3::new(85.0, 0.0, 55.0)];
    let mut results: Vec<Vec<PathRoute>> = Vec::new();
    for with_ally in [false, true] {
        let mut system = PathfindingSystem::new(160.0, 120.0);
        for x in 0..16 {
            for y in 0..12 {
                if y != 5 || !(5..=12).contains(&x) {
                    system.grid.set_cell_type(GridPos::new(x, y), PathfindCellType::Impassable);
                }
            }
        }
        let mut template = ThingTemplate::new("CorridorInfantry");
        template.add_kind_of(KindOf::Infantry);
        let mut mover = Object::new(template.clone(), ObjectId(1), Team::USA);
        mover.owner_player_id = Some(0);
        mover.set_position(start);
        let mut objects = HostObjectStore::new();
        objects.admit(mover.id, mover);
        if with_ally {
            let mut ally_template = ThingTemplate::new("CorridorVehicle");
            ally_template.add_kind_of(KindOf::Vehicle);
            let mut ally = Object::new(ally_template, ObjectId(2), Team::USA);
            ally.owner_player_id = Some(0);
            ally.set_position(Vec3::new(65.0, 0.0, 55.0));
            objects.admit(ally.id, ally);
            system.grid.update_dynamic_obstacles_after_creation_events(objects.read(), &ground);
        }
        let closest = system.find_closest_path(
            start, goal, SURFACE_GROUND, false, false, 0.0, &ground, objects.read(), ObjectId(1),
        ).expect("closest corridor route");
        let safe = system.find_safe_path_from(
            start, start, start, 45.0, SURFACE_GROUND, false, false, &ground, objects.read(), ObjectId(1),
        ).expect("safe corridor route");
        let away = system.get_move_away_from_path(
            &avoided, None, &ground, objects.read(), ObjectId(1), ObjectId(1),
        ).expect("move-away corridor route");
        results.push(vec![closest, safe, away]);
    }
    for (name, clear, occupied) in [
        ("closest", &results[0][0], &results[1][0]),
        ("safe", &results[0][1], &results[1][1]),
        ("move-away", &results[0][2], &results[1][2]),
    ] {
        assert!(!clear.blocked_by_ally, "{name}: empty corridor");
        assert!(occupied.blocked_by_ally, "{name}: route crosses the allied unit");
        for route in [clear, occupied] {
            assert!(route.nodes().iter().any(|node| node.can_optimize), "{name}: raw normal cells");
            assert!(!route.nodes()[0].can_optimize, "{name}: prepended actual start");
        }
    }
}

#[test]
fn route_cached_length_tracks_topology_changes() {
    use super::super::PathRoute;
    use glam::Vec3;

    let point = |x| Vec3::new(x, 0.0, 0.0);
    let base = PathRoute::from_coords(vec![point(0.0), point(1.0), point(2.0)]);
    let mut nodes = base.nodes().to_vec();
    nodes[0].next_optimized = Some(2);
    let mut route = PathRoute::from_nodes(nodes, true).expect("valid skipped route");
    assert_eq!(route.len(), 2);
    assert_eq!(route.last(), Some(&point(2.0)));
    assert_eq!(route.coords(), vec![point(0.0), point(2.0)]);

    route.append_route(PathRoute::from_coords(vec![point(2.0), point(3.0), point(4.0)]));
    assert_eq!(route.len(), 4);
    assert_eq!(route.last(), Some(&point(4.0)));
    assert_eq!(route.coords(), vec![point(0.0), point(2.0), point(3.0), point(4.0)]);

    route.append_route(PathRoute::from_coords(vec![point(5.0), point(6.0)]));
    assert_eq!(route.len(), 6);
    assert_eq!(route.last(), Some(&point(6.0)));
    assert_eq!(route.coords(), vec![point(0.0), point(2.0), point(3.0), point(4.0), point(5.0), point(6.0)]);

    route.push(point(7.0));
    assert_eq!(route.len(), 7);
    assert_eq!(route.last(), Some(&point(7.0)));
    route.clear();
    assert_eq!(route.len(), 0);
    assert_eq!(route.last(), None);
    assert!(route.is_empty());
    assert!(!route.blocked_by_ally);
}

#[test]
fn route_rejects_invalid_optimized_links() {
    use super::super::PathRoute;
    use glam::Vec3;

    let route = PathRoute::from_coords(vec![Vec3::ZERO, Vec3::X, Vec3::Y]);
    for link in [Some(0), Some(3), None] {
        let mut nodes = route.nodes().to_vec();
        nodes[0].next_optimized = link;
        assert!(PathRoute::from_nodes(nodes, false).is_none(), "invalid link {link:?}");
    }
    let mut nodes = route.nodes().to_vec();
    nodes[1].next_optimized = Some(0);
    assert!(PathRoute::from_nodes(nodes, false).is_none());
}
