use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::{PathfindingGround, PathfindingSystem};
use generals_main::game_logic::{
    GameLogic, HostGeometryType, KindOf, ObjectId, Player, Team, ThingTemplate,
};
use glam::Vec3;

fn make_world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    for (name, radius) in [("Large", 10.0), ("Small", 2.0)] {
        let mut unit = ThingTemplate::new(name);
        unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
        unit.geometry_info.geom_type = HostGeometryType::Sphere;
        unit.geometry_info.major_radius = radius;
        unit.geometry_info.authored = true;
        world.templates.insert(name.into(), unit);
    }
    let large = world
        .create_object_for_player("Large", 0, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let small = world
        .create_object_for_player("Small", 0, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    (world, large, small)
}

fn seed(
    world: &GameLogic,
    finder: &mut PathfindingSystem,
    mover: ObjectId,
    ground: &PathfindingGround<'_>,
) {
    finder.find_path_ex_surfaces(
        Vec3::new(100.0, 0.0, 100.0),
        Vec3::new(130.0, 0.0, 100.0),
        world.host_objects(),
        false,
        SURFACE_GROUND,
        false,
        Some(mover),
        ground,
    );
}

fn move_away(
    world: &GameLogic,
    mover: ObjectId,
    other: ObjectId,
    finder: &mut PathfindingSystem,
    ground: &PathfindingGround<'_>,
) -> Vec<Vec3> {
    let avoided = [Vec3::new(20.0, 0.0, 100.0), Vec3::new(180.0, 0.0, 100.0)];
    finder
        .get_move_away_from_path(&avoided, None, ground, world.host_objects(), mover, other)
        .expect("move-away route")
}

#[test]
fn move_away_query_does_not_inherit_another_movers_footprint() {
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut reference = PathfindingSystem::new(400.0, 400.0);
    let mut interleaved = PathfindingSystem::new(400.0, 400.0);
    seed(&world, &mut reference, small, &ground);
    seed(&world, &mut interleaved, large, &ground);
    assert_eq!(
        move_away(&world, small, large, &mut interleaved, &ground),
        move_away(&world, small, large, &mut reference, &ground),
        "the small mover must not inherit the large mover's pathfinding state"
    );
}

#[test]
fn move_away_uses_receiving_world_after_peer_restore() {
    let (world, large, small) = make_world();
    let (mut peer, peer_large, peer_small) = make_world();
    assert_eq!((large, small), (peer_large, peer_small));
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let mut peer_finder = PathfindingSystem::new(400.0, 400.0);
    let expected = move_away(&world, small, large, &mut finder, &ground);
    seed(&peer, &mut peer_finder, peer_large, &ground);
    assert_eq!(
        move_away(&peer, peer_small, peer_large, &mut peer_finder, &ground),
        expected
    );
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    assert_eq!(
        move_away(&peer, peer_small, peer_large, &mut peer_finder, &ground),
        expected
    );
    assert_eq!(
        move_away(&world, small, large, &mut finder, &ground),
        expected
    );
    let avoided = [Vec3::new(20.0, 0.0, 100.0), Vec3::new(180.0, 0.0, 100.0)];
    assert!(
        peer_finder
            .get_move_away_from_path(
                &avoided,
                None,
                &ground,
                peer.host_objects(),
                ObjectId(999),
                peer_large,
            )
            .is_none()
    );
}

#[test]
fn move_away_clearance_uses_geometry_instead_of_selection_radius() {
    let (mut world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let expected = move_away(&world, small, large, &mut finder, &ground);
    world.host_object_mut(small).unwrap().selection_radius = 80.0;
    world.host_object_mut(large).unwrap().selection_radius = 1.0;
    assert_eq!(
        move_away(&world, small, large, &mut finder, &ground),
        expected
    );
}

#[test]
fn move_away_does_not_cut_a_closed_corner() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::pathfinding::GridPos;
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    for pos in [
        GridPos::new(9, 10),
        GridPos::new(11, 10),
        GridPos::new(10, 9),
        GridPos::new(10, 11),
    ] {
        finder.grid.set_cell_type(pos, PathfindCellType::Impassable);
    }
    let avoided = [Vec3::new(20.0, 0.0, 100.0), Vec3::new(180.0, 0.0, 100.0)];
    let path =
        finder.get_move_away_from_path(&avoided, None, &ground, world.host_objects(), small, large);
    assert!(
        path.is_none(),
        "a diagonal cannot escape between two closed sides: {path:?}"
    );
}

#[test]
fn move_away_equal_cost_uses_cpp_neighbor_order() {
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let path = move_away(&world, small, large, &mut finder, &ground);
    assert!(
        path.last().unwrap().z > 100.0,
        "C++ queues +y before -y at equal cost: {path:?}"
    );
}

#[test]
fn move_away_route_follows_open_detour_around_wall() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::pathfinding::GridPos;
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    for x in 0..=12 {
        finder
            .grid
            .set_cell_type(GridPos::new(x, 11), PathfindCellType::Impassable);
    }
    for x in 0..40 {
        finder
            .grid
            .set_cell_type(GridPos::new(x, 9), PathfindCellType::Impassable);
    }
    let path = move_away(&world, small, large, &mut finder, &ground);
    assert!(
        path.len() >= 3,
        "the route must turn around the wall: {path:?}"
    );
    assert!(
        path.iter().any(|point| point.x >= 130.0),
        "the gap is beyond the wall: {path:?}"
    );
    for segment in path.windows(2) {
        for step in 0..=200 {
            let point = segment[0].lerp(segment[1], step as f32 / 200.0);
            let cell = finder.grid.world_to_grid(point);
            assert!(
                !finder.grid.is_static_blocked(cell),
                "route crosses blocked cell {cell:?}: {path:?}"
            );
        }
    }
}

#[test]
fn move_away_queries_keep_same_id_worlds_separate() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::pathfinding::GridPos;
    let (world, large, small) = make_world();
    let (peer, peer_large, peer_small) = make_world();
    assert_eq!((large, small), (peer_large, peer_small));
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let mut peer_finder = PathfindingSystem::new(400.0, 400.0);
    let expected = move_away(&world, small, large, &mut finder, &ground);
    for pos in [
        GridPos::new(9, 10),
        GridPos::new(11, 10),
        GridPos::new(10, 9),
        GridPos::new(10, 11),
    ] {
        peer_finder
            .grid
            .set_cell_type(pos, PathfindCellType::Impassable);
    }
    let avoided = [Vec3::new(20.0, 0.0, 100.0), Vec3::new(180.0, 0.0, 100.0)];
    assert!(
        peer_finder
            .get_move_away_from_path(
                &avoided,
                None,
                &ground,
                peer.host_objects(),
                peer_small,
                peer_large
            )
            .is_none()
    );
    assert_eq!(
        move_away(&world, small, large, &mut finder, &ground),
        expected
    );
}

#[test]
fn path_through_units_does_not_enable_tunneling_through_walls() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::pathfinding::GridPos;
    let (mut world, large, small) = make_world();
    world.host_object_mut(small).unwrap().can_path_through_units = true;
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    for pos in [
        GridPos::new(9, 10),
        GridPos::new(11, 10),
        GridPos::new(10, 9),
        GridPos::new(10, 11),
    ] {
        finder.grid.set_cell_type(pos, PathfindCellType::Impassable);
    }
    let avoided = [Vec3::new(20.0, 0.0, 100.0), Vec3::new(180.0, 0.0, 100.0)];
    let path =
        finder.get_move_away_from_path(&avoided, None, &ground, world.host_objects(), small, large);
    assert!(
        path.is_none(),
        "path-through-units cannot tunnel through terrain: {path:?}"
    );
}

fn tiny_collision_world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.override_world_size(20.0, 20.0);
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("Unit");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.geometry_info.geom_type = HostGeometryType::Sphere;
    template.geometry_info.major_radius = 2.0;
    template.geometry_info.authored = true;
    world.templates.insert("Unit".into(), template);
    let mover = world
        .create_object_for_player("Unit", 0, Vec3::ZERO)
        .unwrap();
    let yielding = world
        .create_object_for_player("Unit", 0, Vec3::ZERO)
        .unwrap();
    world
        .host_object_mut(yielding)
        .unwrap()
        .is_blocked_and_stuck = true;
    (world, mover, yielding)
}

#[test]
fn collision_retry_sets_unit_flag_even_when_no_route_exists() {
    let (mut world, mover, yielding) = tiny_collision_world();
    let (mut peer, peer_mover, peer_yielding) = tiny_collision_world();
    assert_eq!((mover, yielding), (peer_mover, peer_yielding));
    world
        .host_object_mut(yielding)
        .unwrap()
        .is_blocked_and_stuck = true;
    peer.host_object_mut(peer_yielding)
        .unwrap()
        .is_blocked_and_stuck = true;
    let actor = world.host_object_mut(mover).unwrap();
    actor.allow_collide_force = false;
    actor.movement.path = vec![Vec3::new(-100.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0)];
    actor.request_other_move_away = Some(yielding);
    world.try_physics_collide(mover, yielding, 1.0).unwrap();
    let actor = world.host_object(yielding).unwrap();
    assert!(
        actor.can_path_through_units,
        "the retry flag persists even if no route is found"
    );
    assert!(
        !actor.is_blocked_and_stuck,
        "the C++ setter clears stuck state"
    );
    assert!(
        actor.movement.path.is_empty(),
        "no clear destination exists in the tiny world"
    );
    let peer_unit = peer.host_object(peer_yielding).unwrap();
    assert!(
        !peer_unit.can_path_through_units,
        "the peer's unit did not receive the retry"
    );
    assert!(
        peer_unit.is_blocked_and_stuck,
        "the peer's stuck state is independent"
    );
}

#[test]
fn occupied_start_automatically_enables_escape_tunneling() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::pathfinding::{GridPos, PathPositionFootprint};
    let (mut world, large, small) = make_world();
    world.add_player(Player::new(1, Team::GLA, "Enemy", false));
    let enemy = world
        .create_object_for_player("Small", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    finder.grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: enemy.0,
            player: 1,
            radius: 2.0,
            layer: 0,
            on_ground: true,
            infantry: false,
            crushable_level: 0,
        },
        (-1, -1),
        Vec3::new(100.0, 0.0, 100.0),
    );
    for pos in [
        GridPos::new(9, 10),
        GridPos::new(11, 10),
        GridPos::new(10, 9),
        GridPos::new(10, 11),
    ] {
        finder.grid.set_cell_type(pos, PathfindCellType::Impassable);
    }
    let avoided = [Vec3::new(20.0, 0.0, 100.0), Vec3::new(180.0, 0.0, 100.0)];
    let path =
        finder.get_move_away_from_path(&avoided, None, &ground, world.host_objects(), small, large);
    assert!(
        path.is_some(),
        "an enemy occupying the start permits the original escape search"
    );
}

#[test]
fn even_footprint_uses_shifted_index_when_start_cell_has_no_info() {
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let path = finder
        .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), large, small)
        .unwrap();
    assert_eq!(
        path,
        vec![Vec3::new(100.0, 0.0, 100.0), Vec3::new(120.5, 0.0, 100.5)]
    );
}

#[test]
fn even_footprint_keeps_existing_start_cell_info_coordinates() {
    use generals_main::game_logic::pathfinding::PathPositionFootprint;
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    finder.grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: large.0,
            player: 0,
            radius: 10.0,
            layer: 0,
            on_ground: true,
            infantry: false,
            crushable_level: 0,
        },
        (-1, -1),
        Vec3::new(100.0, 0.0, 100.0),
    );
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let path = finder
        .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), large, small)
        .unwrap();
    assert_eq!(
        path,
        vec![Vec3::new(100.0, 0.0, 100.0), Vec3::new(100.5, 0.0, 100.5)]
    );
}

#[test]
fn odd_footprint_still_forces_movement_from_start_cell() {
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let path = finder
        .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), small, large)
        .unwrap();
    assert_eq!(
        path,
        vec![Vec3::new(100.0, 0.0, 100.0), Vec3::new(115.0, 0.0, 105.0)]
    );
}

#[test]
fn even_footprint_respects_existing_goal_and_aircraft_cell_info() {
    let (mut world, large, small) = make_world();
    world.add_player(Player::new(1, Team::GLA, "Enemy", false));
    let enemy = world
        .create_object_for_player("Small", 1, Vec3::new(150.0, 0.0, 150.0))
        .unwrap();
    let ground = PathfindingGround::Flat(0.0);
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    for aircraft in [false, true] {
        let mut finder = PathfindingSystem::new(400.0, 400.0);
        if aircraft {
            finder.grid.update_aircraft_goal_cell(
                enemy.0,
                2.0,
                (-1, -1),
                0,
                Vec3::new(100.0, 0.0, 100.0),
                true,
            );
        } else {
            finder.grid.update_ground_goal_cell(
                enemy.0,
                1,
                2.0,
                false,
                (-1, -1),
                Vec3::new(100.0, 0.0, 100.0),
            );
        }
        let path = finder
            .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), large, small)
            .unwrap();
        assert_eq!(
            path,
            vec![Vec3::new(100.0, 0.0, 100.0), Vec3::new(100.5, 0.0, 100.5)],
            "aircraft={aircraft}"
        );
    }
}

#[test]
fn even_footprint_start_info_is_owned_by_each_session() {
    use generals_main::game_logic::pathfinding::PathPositionFootprint;
    let (world, large, small) = make_world();
    let (peer, peer_large, peer_small) = make_world();
    assert_eq!((large, small), (peer_large, peer_small));
    let ground = PathfindingGround::Flat(0.0);
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let mut peer_finder = PathfindingSystem::new(400.0, 400.0);
    finder.grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: large.0,
            player: 0,
            radius: 10.0,
            layer: 0,
            on_ground: true,
            infantry: false,
            crushable_level: 0,
        },
        (-1, -1),
        Vec3::new(100.0, 0.0, 100.0),
    );
    let query = |sys: &mut PathfindingSystem, world: &GameLogic, mover, other| {
        sys.get_move_away_from_path(&avoided, None, &ground, world.host_objects(), mover, other)
            .unwrap()
    };
    let expected = query(&mut finder, &world, large, small);
    let peer_path = query(&mut peer_finder, &peer, peer_large, peer_small);
    assert_ne!(
        expected, peer_path,
        "only the first session has retained start info"
    );
    assert_eq!(query(&mut finder, &world, large, small), expected);
}

#[test]
fn move_away_clips_a_start_outside_the_pathfinder_extent() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    for (name, radius) in [("Wide", 10.0), ("Narrow", 2.0)] {
        let mut template = ThingTemplate::new(name);
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        template.geometry_info.geom_type = HostGeometryType::Sphere;
        template.geometry_info.major_radius = radius;
        template.geometry_info.authored = true;
        world.templates.insert(name.into(), template);
    }
    let mover = world
        .create_object_for_player("Wide", 0, Vec3::new(-25.0, 0.0, 100.0))
        .unwrap();
    let other = world
        .create_object_for_player("Narrow", 0, Vec3::new(150.0, 0.0, 150.0))
        .unwrap();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let path = finder
        .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), mover, other)
        .unwrap();
    assert_eq!(
        path,
        vec![Vec3::new(-25.0, 0.0, 100.0), Vec3::new(10.5, 0.0, 100.5)]
    );
}

#[test]
fn even_footprint_can_accept_its_own_fixed_destination_cell() {
    use generals_main::game_logic::pathfinding::PathPositionFootprint;
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let position = Vec3::new(100.0, 0.0, 100.0);
    finder
        .grid
        .update_ground_goal_cell(large.0, 0, 10.0, false, (-1, -1), position);
    finder.grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: large.0,
            player: 0,
            radius: 10.0,
            layer: 0,
            on_ground: true,
            infantry: false,
            crushable_level: 0,
        },
        (-1, -1),
        position,
    );
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let path = finder
        .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), large, small)
        .unwrap();
    assert_eq!(path, vec![position, Vec3::new(100.5, 0.0, 100.5)]);
}

#[test]
fn even_footprint_rejects_allied_goals_and_uncrushable_enemy_occupants() {
    use generals_main::game_logic::pathfinding::PathPositionFootprint;
    let (mut world, large, small) = make_world();
    world.add_player(Player::new(1, Team::GLA, "Enemy", false));
    let enemy = world
        .create_object_for_player("Small", 1, Vec3::new(95.0, 0.0, 95.0))
        .unwrap();
    let ground = PathfindingGround::Flat(0.0);
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    for enemy_fixed in [false, true] {
        let mut finder = PathfindingSystem::new(400.0, 400.0);
        finder.grid.update_path_position_cell(
            &PathPositionFootprint {
                unit_id: large.0,
                player: 0,
                radius: 10.0,
                layer: 0,
                on_ground: true,
                infantry: false,
                crushable_level: 0,
            },
            (-1, -1),
            Vec3::new(100.0, 0.0, 100.0),
        );
        let (goal, player) = if enemy_fixed { (enemy, 1) } else { (small, 0) };
        let position = Vec3::new(95.0, 0.0, 95.0);
        finder
            .grid
            .update_ground_goal_cell(goal.0, player, 2.0, false, (-1, -1), position);
        if enemy_fixed {
            finder.grid.update_path_position_cell(
                &PathPositionFootprint {
                    unit_id: goal.0,
                    player,
                    radius: 2.0,
                    layer: 0,
                    on_ground: true,
                    infantry: false,
                    crushable_level: 255,
                },
                (-1, -1),
                position,
            );
        }
        let path = finder
            .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), large, small)
            .unwrap();
        assert_ne!(
            *path.last().unwrap(),
            Vec3::new(100.5, 0.0, 100.5),
            "enemy_fixed={enemy_fixed}: {path:?}"
        );
    }
}

#[test]
fn move_away_destination_accepts_the_movers_ignored_obstacle() {
    use generals_main::game_logic::pathfinding::GridPos;
    let (mut world, large, small) = make_world();
    world.host_object_mut(large).unwrap().ignored_obstacle_id = Some(small);
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    finder.grid.set_cell_obstacle_owned(
        GridPos::new(10, 10),
        false,
        false,
        small.0,
        Some(0),
        Some(Team::USA),
    );
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let path = finder
        .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), large, small)
        .unwrap();
    assert_eq!(
        path,
        vec![Vec3::new(100.0, 0.0, 100.0), Vec3::new(100.5, 0.0, 100.5)]
    );
}

#[test]
fn move_away_accepts_short_avoided_paths_and_checks_the_second_path() {
    let (world, other, mover) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let far = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let lone_node = [Vec3::new(100.0, 0.0, 100.0)];
    let query = |finder: &mut PathfindingSystem, primary: &[Vec3], secondary| {
        finder.get_move_away_from_path(
            primary,
            secondary,
            &ground,
            world.host_objects(),
            mover,
            other,
        )
    };
    let baseline = query(&mut finder, &far, None).expect("unobstructed move-away route");
    for short in [&[][..], &lone_node[..]] {
        let route = query(&mut finder, short, None)
            .expect("a non-null short path still triggers the search");
        assert_eq!(route, baseline);
        assert_ne!(
            route.last(),
            route.first(),
            "the mover must leave its starting cell"
        );
    }
    let crossing = [Vec3::new(20.0, 0.0, 100.0), Vec3::new(180.0, 0.0, 100.0)];
    let avoiding_both =
        query(&mut finder, &lone_node, Some(&crossing)).expect("route avoiding the second path");
    assert_ne!(
        avoiding_both.last(),
        baseline.last(),
        "the second path still restricts the destination"
    );
}

#[test]
fn collision_moves_away_from_a_single_node_path() {
    let (mut world, mover, yielding) = tiny_collision_world();
    let (peer, _, peer_yielding) = tiny_collision_world();
    let actor = world.host_object_mut(mover).unwrap();
    actor.allow_collide_force = false;
    actor.movement.path = vec![Vec3::ZERO];
    actor.request_other_move_away = Some(yielding);
    world.try_physics_collide(mover, yielding, 1.0).unwrap();
    let unit = world.host_object(yielding).unwrap();
    assert_eq!(unit.move_away_from, Some(mover));
    assert!(
        unit.movement.path.len() >= 2,
        "the unit receives a move-away route"
    );
    assert!(
        !peer
            .host_object(peer_yielding)
            .unwrap()
            .can_path_through_units
    );
}
