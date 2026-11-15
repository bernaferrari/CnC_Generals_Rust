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
fn quick_query_uses_each_worlds_endpoint_layers_with_same_object_ids() {
    let mut owners = bridge_approach_worlds();
    assert_eq!(owners[0].2, owners[1].2);
    for (_, finder, _) in &mut owners {
        finder.grid.set_cell_type(GridPos::new(8, 5), PathfindCellType::Cliff);
        finder.grid.rebuild_path_zones();
    }
    // The same coordinates lie on the first world's deck but at ground level
    // in the second, where the goal cell is a cliff.
    owners[1].1.grid.bridge_layers[0].from_left.y = 40.0;
    owners[1].1.grid.bridge_layers[0].from_right.y = 40.0;
    owners[1].1.grid.bridge_layers[0].to_left.y = 40.0;
    owners[1].1.grid.bridge_layers[0].to_right.y = 40.0;
    let ground = PathfindingGround::Flat(0.0);
    let from = Vec3::new(45.0, 20.0, 55.0);
    let goal = Vec3::new(85.0, 20.0, 55.0);
    let query = |finder: &PathfindingSystem| {
        finder.client_safe_quick_does_path_exist(from, goal, SURFACE_GROUND, &ground)
    };
    assert!(query(&owners[0].1));
    assert!(!query(&owners[1].1));
    assert!(query(&owners[0].1));
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
    let opt = grid.optimize_ground_path_ex_layered(
        &waypoints,
        &layers,
        SURFACE_GROUND,
        false,
        None,
        0,
        &ground,
    );
    assert!(!opt.is_empty());
    assert!(
        (0..opt.len()).all(|i| opt.optimized_layer(i) == Some(bridge as u8)),
        "carried bridge PathNode layers must survive optimize; got {opt:?}"
    );
}

#[test]
fn ground_optimizer_removes_only_one_jig_per_anchor() {
    // A blocked deck forces alternating nodes to remain separate until the
    // final jig/jog pass; absent cells would fall through to clear ground.
    let mut grid = deck_grid(0.0);
    for x in 0..20 {
        for y in 0..12 {
            grid.bridge_layers[0]
                .cells
                .insert((x, y), (PathfindCellType::BridgeImpassable, 0));
        }
    }
    let points = vec![
        Vec3::new(20.0, 0.0, 20.0),
        Vec3::new(35.0, 0.0, 20.0),
        Vec3::new(35.0, 0.0, 35.0),
        Vec3::new(50.0, 0.0, 35.0),
        Vec3::new(50.0, 0.0, 50.0),
    ];
    let layers = vec![2; points.len()];
    let optimized = grid.optimize_ground_path_ex_layered(
        &points, &layers, SURFACE_GROUND, false, None, 0, &PathfindingGround::Flat(0.0),
    );
    // AIPathfind.cpp:682-692 advances to the newly linked node after each
    // removal; it does not repeatedly remove successors of the same anchor.
    assert_eq!(optimized.coords(), vec![points[0], points[2], points[4]]);
    let object_path = grid.optimize_path_ex_layered(
        &points, &layers, SURFACE_GROUND, false, None, 0, &PathfindingGround::Flat(0.0),
    );
    assert_eq!(object_path.coords(), points, "Path::optimize does not run the jig/jog pass");
}

#[test]
fn object_path_approaches_bridge_on_starting_layer() {
    let mut grid = deck_grid(0.0);
    for x in 0..20 {
        for y in 0..12 {
            grid.bridge_layers[0]
                .cells
                .insert((x, y), (PathfindCellType::Clear, 0));
        }
    }
    grid.set_cell_type(GridPos::new(4, 2), PathfindCellType::Obstacle);
    let points = [
        Vec3::new(20.0, 0.0, 20.0),
        Vec3::new(40.0, 0.0, 40.0),
        Vec3::new(60.0, 0.0, 20.0),
    ];
    let layers = [PathfindLayerEnum::Ground as u8, 2, 2];
    let ground = PathfindingGround::Flat(0.0);
    let object_path = grid.optimize_path_ex_layered(
        &points, &layers, SURFACE_GROUND, false, None, 0, &ground,
    );
    let ground_path = grid.optimize_ground_path_ex_layered(
        &points, &layers, SURFACE_GROUND, false, None, 0, &ground,
    );
    // The object query cannot use clear bridge cells to shortcut a ground
    // obstacle before entering the bridge. The ground-only optimizer uses
    // its chosen bridge layer instead (AIPathfind.cpp:597-619).
    assert_eq!(object_path.coords(), points);
    assert_eq!(ground_path.coords(), vec![points[0], points[2]]);
}

// The original visibility check visits cells, not the continuous segment.
fn crosses_obstacle(segment: &[Vec3]) -> bool {
    let cell = |p: Vec3| ((p.x / 10.0).floor() as i32, (p.z / 10.0).floor() as i32);
    let start_cell = cell(segment[0]);
    let mut crossed = start_cell == (3, 5);
    gamelogic::ai::pathfind_astar::visit_cell_line(start_cell, cell(segment[1]), |_, to| {
        crossed |= to == (3, 5);
        !crossed
    });
    crossed
}

fn bridge_approach_worlds() -> Vec<(
    crate::game_logic::GameLogic,
    PathfindingSystem,
    crate::game_logic::ObjectId,
)> {
    use crate::game_logic::{GameLogic, HostGeometryType, KindOf, Player, Team, ThingTemplate};

    let start = Vec3::new(15.0, 0.0, 55.0);
    let mut owners = Vec::new();
    for blocked in [true, false] {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", false));
        let mut template = ThingTemplate::new("BridgeApproach");
        template.add_kind_of(KindOf::Infantry).set_health(100.0);
        template.geometry_info.geom_type = HostGeometryType::Sphere;
        template.geometry_info.major_radius = 2.0;
        template.geometry_info.authored = true;
        world.templates.insert("BridgeApproach".into(), template);
        let mover = world.create_object_for_player("BridgeApproach", 0, start).unwrap();

        let mut finder = PathfindingSystem::new(200.0, 120.0);
        finder.grid = deck_grid(20.0);
        // The deck spans the obstacle, with one entry on each bank. The
        // route must first detour on ground, then cross the elevated deck.
        let deck = &mut finder.grid.bridge_layers[0];
        deck.cells.clear();
        for x in 1..14 {
            for y in 0..12 {
                deck.cells.insert((x, y), (PathfindCellType::Clear, 0));
            }
        }
        for x in [5, 13] {
            deck.cells.insert((x, 5), (PathfindCellType::Clear, PathfindLayerEnum::Ground as u8));
        }
        deck.ground_connect_cells = vec![(5, 5), (13, 5)];
        for x in [5, 13] {
            let connect = finder.grid.bit_index(GridPos::new(x, 5)).unwrap();
            finder.grid.ground_connect[connect] = 2;
        }
        for x in 8..13 {
            for y in 0..12 {
                finder.grid.set_cell_type(GridPos::new(x, y), PathfindCellType::Impassable);
            }
        }
        if blocked {
            finder.grid.set_cell_type(GridPos::new(3, 5), PathfindCellType::Impassable);
        }
        finder.grid.terrain_gen += 1;
        finder.grid.rebuild_path_zones();
        owners.push((world, finder, mover));
    }
    owners
}

#[test]
fn searched_bridge_approach_does_not_shortcut_ground_obstacle_in_either_world() {
    use crate::game_logic::GameLogic;

    let ground = PathfindingGround::Flat(0.0);
    let start = Vec3::new(15.0, 0.0, 55.0);
    let goal = Vec3::new(155.0, 0.0, 55.0);
    let mut owners = bridge_approach_worlds();
    assert_eq!(owners[0].2, owners[1].2);
    let query = |owner: &mut (GameLogic, PathfindingSystem, crate::game_logic::ObjectId)| {
        let (world, finder, mover) = owner;
        let path = finder.find_path_ex_surfaces(
            start, goal, world.host_objects(), false, SURFACE_GROUND, false, Some(*mover), &ground,
        ).expect("ground-to-bridge route");
        (path.coords(), (0..path.len()).map(|i| path.optimized_layer(i).expect("searched node has a layer")).collect::<Vec<_>>())
    };
    let blocked = query(&mut owners[0]);
    let clear = query(&mut owners[1]);
    assert!(blocked.1.contains(&2), "search must enter the bridge: {blocked:?}");
    assert!(clear.1.contains(&2), "peer must enter the bridge: {clear:?}");
    assert_ne!(blocked.0, clear.0, "the obstacle belongs only to the first world");
    assert!(
        !blocked.0.windows(2).enumerate().any(|(i, segment)| {
            blocked.1[i] == PathfindLayerEnum::Ground as u8 && crosses_obstacle(segment)
        }),
        "ground segment crosses the obstacle under the deck: {blocked:?}"
    );
    assert!(
        crosses_obstacle(&clear.0[..2]),
        "clear peer should take the direct approach: {clear:?}"
    );
    assert_eq!(
        query(&mut owners[0]),
        blocked,
        "peer query must not alter the first world"
    );
}

#[test]
fn ground_search_reaches_bridge_destination_above_impassable_in_two_worlds() {
    use crate::game_logic::GameLogic;

    let ground = PathfindingGround::Flat(0.0);
    let start = Vec3::new(15.0, 0.0, 55.0);
    let goal = Vec3::new(105.0, 20.0, 55.0);
    let mut owners = bridge_approach_worlds();
    let query = |owner: &mut (GameLogic, PathfindingSystem, crate::game_logic::ObjectId)| {
        let (world, finder, mover) = owner;
        assert_eq!(finder.grid.cell_type(GridPos::new(10, 5)), PathfindCellType::Impassable);
        assert_eq!(finder.grid.layer_for_destination(goal, &ground) as u8, 2);
        let path = finder.find_path_ex_surfaces(
            start, goal, world.host_objects(), false, SURFACE_GROUND, false, Some(*mover), &ground,
        ).expect("a clear bridge destination must not be rejected by ground terrain below");
        assert_eq!(path.optimized_layer(path.len() - 1), Some(2));
        assert_eq!(finder.grid.world_to_grid(*path.last().unwrap()), GridPos::new(10, 5));
        (path.coords(), (0..path.len()).map(|i| path.optimized_layer(i).expect("searched node has a layer")).collect::<Vec<_>>())
    };
    let blocked = query(&mut owners[0]);
    let clear = query(&mut owners[1]);
    assert_ne!(blocked.0, clear.0, "the ground approach obstacle is world-owned");
    assert_eq!(query(&mut owners[0]), blocked, "peer search must not alter the first world");
}

#[test]
fn closest_bridge_approach_respects_each_worlds_ground_obstacle() {
    use crate::game_logic::GameLogic;

    let ground = PathfindingGround::Flat(0.0);
    let start = Vec3::new(15.0, 0.0, 55.0);
    let goal = Vec3::new(155.0, 0.0, 55.0);
    let mut owners = bridge_approach_worlds();
    assert_eq!(owners[0].2, owners[1].2);
    let query = |owner: &mut (GameLogic, PathfindingSystem, crate::game_logic::ObjectId)| {
        let (world, finder, mover) = owner;
        let path = finder.find_closest_path(
            start,
            goal,
            SURFACE_GROUND,
            false,
            false,
            0.0,
            &ground,
            world.host_objects(),
            *mover,
        ).expect("closest path across bridge");
        (path.coords(), (0..path.len()).map(|i| path.optimized_layer(i).expect("searched node has a layer")).collect::<Vec<_>>())
    };
    let blocked = query(&mut owners[0]);
    let clear = query(&mut owners[1]);
    assert!(blocked.1.contains(&2), "blocked route must use the deck: {blocked:?}");
    assert!(clear.1.contains(&2), "clear route must use the deck: {clear:?}");
    assert_ne!(blocked.0, clear.0, "the obstacle belongs only to the first world");
    assert!(
        !blocked.0.windows(2).enumerate().any(|(i, segment)| {
            blocked.1[i] == PathfindLayerEnum::Ground as u8 && crosses_obstacle(segment)
        }),
        "ground segment crosses obstacle: {blocked:?}"
    );
    assert!(crosses_obstacle(&clear.0[..2]), "clear world should take direct approach: {clear:?}");
    assert_eq!(query(&mut owners[0]), blocked, "peer query must not alter the first world");
}

#[test]
fn movement_last_path_layer_tracks_known_identity() {
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
    assert!(movement.path.nodes().iter().all(|node| node.layer.is_none()));
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
