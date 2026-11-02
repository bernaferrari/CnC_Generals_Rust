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
