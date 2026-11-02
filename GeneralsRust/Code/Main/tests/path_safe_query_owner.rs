use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::{PathfindingGround, PathfindingSystem};
use generals_main::game_logic::{
    GameLogic, HostGeometryType, KindOf, ObjectId, Player, Team, ThingTemplate,
};
use glam::Vec3;

fn make_world() -> (GameLogic, ObjectId, ObjectId) {
    make_world_at(Vec3::new(100.0, 0.0, 100.0))
}

fn make_world_at(position: Vec3) -> (GameLogic, ObjectId, ObjectId) {
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
        .create_object_for_player("Large", 0, position)
        .unwrap();
    let small = world
        .create_object_for_player("Small", 0, position)
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

fn flee(
    world: &GameLogic,
    finder: &mut PathfindingSystem,
    mover: ObjectId,
    ground: &PathfindingGround<'_>,
) -> Vec<Vec3> {
    finder
        .find_safe_path_from(
            Vec3::new(100.0, 0.0, 100.0),
            Vec3::new(100.0, 0.0, 100.0),
            Vec3::new(160.0, 0.0, 100.0),
            40.0,
            SURFACE_GROUND,
            false,
            true,
            ground,
            world.host_objects(),
            mover,
        )
        .expect("safe route")
}

#[test]
fn safe_query_does_not_inherit_another_movers_footprint() {
    let (world, large, small) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let mut reference = PathfindingSystem::new(400.0, 400.0);
    let mut interleaved = PathfindingSystem::new(400.0, 400.0);
    seed(&world, &mut reference, small, &ground);
    seed(&world, &mut interleaved, large, &ground);
    let expected = flee(&world, &mut reference, small, &ground);
    assert_eq!(
        flee(&world, &mut interleaved, small, &ground),
        expected,
        "the small mover must not inherit the large mover's footprint"
    );
    let (mut peer, peer_large, peer_small) = make_world();
    assert_eq!((large, small), (peer_large, peer_small));
    let mut peer_finder = PathfindingSystem::new(400.0, 400.0);
    seed(&peer, &mut peer_finder, peer_large, &ground);
    assert_eq!(
        flee(&peer, &mut peer_finder, peer_small, &ground),
        expected,
        "the receiving peer resolves its own mover"
    );
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    seed(&peer, &mut peer_finder, peer_large, &ground);
    assert_eq!(
        flee(&peer, &mut peer_finder, peer_small, &ground),
        expected,
        "restored queries still use the selected mover"
    );
    assert_eq!(
        flee(&world, &mut interleaved, small, &ground),
        expected,
        "restoring the peer must not change the first world"
    );
    assert!(
        peer_finder
            .find_safe_path_from(
                Vec3::new(100.0, 0.0, 100.0),
                Vec3::new(100.0, 0.0, 100.0),
                Vec3::new(160.0, 0.0, 100.0),
                40.0,
                SURFACE_GROUND,
                false,
                true,
                &ground,
                peer.host_objects(),
                ObjectId(999),
            )
            .is_none(),
        "a missing mover cannot inherit an earlier query"
    );
}

#[test]
fn safe_query_uses_its_own_human_extent_policy() {
    use generals_main::game_logic::pathfinding::GridPos;
    let (mut world, _, small) = make_world();
    world.get_player_mut(0).unwrap().is_human = false;
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    finder.set_human_player_mask(0);
    finder
        .grid
        .set_logical_extent(GridPos::new(0, 0), GridPos::new(5, 5));
    finder.grid.set_query_is_human(true);
    let route = finder.find_safe_path_from(
        Vec3::new(100.0, 0.0, 100.0),
        Vec3::new(100.0, 0.0, 100.0),
        Vec3::new(160.0, 0.0, 100.0),
        40.0,
        SURFACE_GROUND,
        false,
        false,
        &ground,
        world.host_objects(),
        small,
    );
    assert!(
        route.is_some(),
        "an AI mover must not inherit the previous human extent restriction"
    );
}

#[test]
fn safe_search_accepts_the_last_reachable_cell_inside_the_repulsor() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::pathfinding::GridPos;
    let from = Vec3::new(105.0, 0.0, 105.0);
    let (world, _, mover) = make_world_at(from);
    let (peer, _, peer_mover) = make_world_at(from);
    assert_eq!(mover, peer_mover);
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let mut peer_finder = PathfindingSystem::new(400.0, 400.0);
    for f in [&mut finder, &mut peer_finder] {
        for x in 9..=12 {
            for y in 9..=11 {
                if (x, y) != (10, 10) && (x, y) != (11, 10) {
                    f.grid
                        .set_cell_type(GridPos::new(x, y), PathfindCellType::Impassable);
                }
            }
        }
    }
    let query = |f: &mut PathfindingSystem, w: &GameLogic, id| {
        f.find_safe_path_from(
            from,
            from,
            from,
            100.0,
            SURFACE_GROUND,
            false,
            true,
            &ground,
            w.host_objects(),
            id,
        )
        .expect("the original accepts the last valid reachable cell")
    };
    let expected = query(&mut peer_finder, &peer, peer_mover);
    let result = query(&mut finder, &world, mover);
    assert_eq!(
        finder.grid.world_to_grid(*result.last().unwrap()),
        GridPos::new(11, 10)
    );
    assert!((*result.last().unwrap() - from).length() < 100.0);
    assert_eq!(query(&mut peer_finder, &peer, peer_mover), expected);
}

#[test]
fn safe_search_can_return_the_already_safe_start() {
    let from = Vec3::new(105.0, 0.0, 105.0);
    let (world, _, mover) = make_world_at(from);
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    let repulsor = Vec3::new(10.0, 0.0, 10.0);
    let route = finder
        .find_safe_path_from(
            Vec3::new(305.0, 0.0, 305.0),
            repulsor,
            repulsor,
            20.0,
            SURFACE_GROUND,
            false,
            true,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("the original builds a short path when the start is already safe");
    assert_eq!(route, vec![from]);
}

#[test]
fn single_repulsor_wrapper_uses_the_receiving_computers_extent_policy() {
    use generals_main::game_logic::pathfinding::GridPos;
    let (mut world, _, mover) = make_world();
    world.get_player_mut(0).unwrap().is_human = false;
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    finder.set_human_player_mask(0);
    finder
        .grid
        .set_logical_extent(GridPos::new(0, 0), GridPos::new(5, 5));
    let from = Vec3::new(100.0, 0.0, 100.0);
    let route = finder.find_safe_path(
        from,
        from,
        40.0,
        SURFACE_GROUND,
        false,
        &PathfindingGround::Flat(0.0),
        world.host_objects(),
        mover,
    );
    assert!(
        route.is_some(),
        "computer movement may search outside the human logical extent"
    );
}

#[test]
fn safe_search_does_not_walk_through_an_uncrushable_enemy() {
    use generals_main::game_logic::pathfinding::{GridPos, PathPositionFootprint};
    let from = Vec3::new(105.0, 0.0, 105.0);
    let (mut world, _, mover) = make_world_at(from);
    world.add_player(Player::new(1, Team::GLA, "Enemy", false));
    let enemy_pos = Vec3::new(115.0, 0.0, 105.0);
    let enemy = world
        .create_object_for_player("Small", 1, enemy_pos)
        .unwrap();
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    finder
        .grid
        .set_logical_extent(GridPos::new(10, 10), GridPos::new(11, 10));
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
        enemy_pos,
    );
    let route = finder.find_safe_path_from(
        from,
        from,
        from,
        100.0,
        SURFACE_GROUND,
        false,
        true,
        &PathfindingGround::Flat(0.0),
        world.host_objects(),
        mover,
    );
    assert!(
        route.is_none(),
        "the only neighbor is occupied by an uncrushable enemy: {route:?}"
    );
}
