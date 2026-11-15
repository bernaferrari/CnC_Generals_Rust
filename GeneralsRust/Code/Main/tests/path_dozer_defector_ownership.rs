use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::{GridPos, PathfindingGround, PathfindingSystem};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn world_and_finder() -> (GameLogic, PathfindingSystem, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Builder", true));
    world.add_player(Player::new(1, Team::GLA, "Opponent", false));
    let mut dozer = ThingTemplate::new("Dozer");
    dozer
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(100.0);
    world.templates.insert("Dozer".into(), dozer);
    let mut structure = ThingTemplate::new("Wall");
    structure.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert("Wall".into(), structure);
    let mover = world
        .create_object_for_player("Dozer", 0, Vec3::new(10.0, 0.0, 10.0))
        .unwrap();
    let wall = world
        .create_object_for_player("Wall", 1, Vec3::new(50.0, 0.0, 10.0))
        .unwrap();
    let mut finder = PathfindingSystem::new(120.0, 80.0);
    for y in 0..8 {
        finder.grid.set_cell_obstacle_owned(
            GridPos::new(5, y),
            false,
            false,
            wall.0,
            Some(1),
            Some(Team::GLA),
        );
    }
    (world, finder, mover, wall)
}

fn crosses_wall(world: &GameLogic, finder: &mut PathfindingSystem, mover: ObjectId) -> bool {
    finder
        .find_path_ex_surfaces(
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(100.0, 0.0, 10.0),
            world.host_objects(),
            false,
            SURFACE_GROUND,
            false,
            Some(mover),
            &PathfindingGround::Flat(0.0),
        )
        .is_some_and(|path| path.last().is_some_and(|p| p.x > 70.0))
}

#[test]
fn dozer_obstacle_relationship_uses_current_defector_objects_per_world() {
    // GameLogic::new uninstalls WorldServices; transfer/snapshot need a bind.
    let services = gamelogic::system::engine_stores::new_world_services();
    gamelogic::system::engine_stores::with_world_services(&services, || {
    let (mut world, mut finder, mover, wall) = world_and_finder();
    let (mut peer, mut peer_finder, peer_mover, peer_wall) = world_and_finder();
    assert_eq!((mover, wall), (peer_mover, peer_wall));
    assert!(
        !crosses_wall(&world, &mut finder, mover),
        "ordinary enemy obstacle blocks the dozer"
    );
    assert!(world.transfer_object_to_player(wall, 0));
    assert!(
        crosses_wall(&world, &mut finder, mover),
        "the query must observe a same-frame change to the obstacle owner"
    );
    assert!(world.transfer_object_to_player(wall, 1));
    assert!(
        !crosses_wall(&world, &mut finder, mover),
        "a second transfer must immediately restore the enemy obstacle"
    );
    world
        .host_object_mut(mover)
        .unwrap()
        .begin_undetected_defection(0, 30, true);
    assert!(world.host_object(mover).unwrap().is_undetected_defector());
    assert!(
        crosses_wall(&world, &mut finder, mover),
        "C++ regards an undetected defector seeker as neutral"
    );
    assert!(
        !crosses_wall(&peer, &mut peer_finder, peer_mover),
        "the peer's same-ID objects remain enemies"
    );
    peer.host_object_mut(peer_wall)
        .unwrap()
        .begin_undetected_defection(0, 30, true);
    assert!(
        peer.host_object(peer_wall)
            .unwrap()
            .is_undetected_defector()
    );
    assert!(
        crosses_wall(&peer, &mut peer_finder, peer_mover),
        "C++ regards an undetected defector target as allied"
    );
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    assert!(
        peer.host_object(peer_mover)
            .unwrap()
            .is_undetected_defector()
    );
    assert!(
        crosses_wall(&peer, &mut peer_finder, peer_mover),
        "the restored dozer must retain its world-owned defector behavior"
    );
    for y in 0..8 {
        finder.grid.set_cell_obstacle_owned(
            GridPos::new(5, y),
            false,
            false,
            999,
            Some(0),
            Some(Team::USA),
        );
    }
    assert!(
        !crosses_wall(&world, &mut finder, mover),
        "a stale obstacle ID cannot grant the dozer exception"
    );
    });
}
