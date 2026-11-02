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
        .get_move_away_from_path(
            &avoided,
            None,
            false,
            ground,
            world.host_objects(),
            mover,
            other,
        )
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
                false,
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
