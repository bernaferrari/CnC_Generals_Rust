use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::{PathfindingGround, PathfindingSystem};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn make_world() -> (GameLogic, PathfindingSystem, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut unit = ThingTemplate::new("Mover");
    unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert("Mover".into(), unit);
    let first = world
        .create_object_for_player("Mover", 0, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    let second = world
        .create_object_for_player("Mover", 0, Vec3::new(20.0, 0.0, 50.0))
        .unwrap();
    (world, PathfindingSystem::new(200.0, 200.0), first, second)
}

#[test]
fn closest_query_recognizes_the_driving_units_own_goal_after_another_search() {
    let (world, mut finder, first, second) = make_world();
    let goal = Vec3::new(100.0, 0.0, 50.0);
    let ground = PathfindingGround::Flat(0.0);
    finder
        .grid
        .update_ground_goal_cell(second.0, 0, 1.0, false, (-1, -1), goal);
    finder.find_path_ex_surfaces(
        Vec3::new(20.0, 0.0, 20.0),
        Vec3::new(50.0, 0.0, 20.0),
        world.host_objects(),
        false,
        SURFACE_GROUND,
        false,
        Some(first),
        &ground,
    );
    let path = finder
        .find_closest_path(
            Vec3::new(20.0, 0.0, 50.0),
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &ground,
            world.host_objects(),
            second,
        )
        .expect("closest path");
    assert_eq!(
        finder.grid.world_to_grid(*path.last().unwrap()),
        finder.grid.world_to_grid(goal),
        "the mover's own goal is not another unit's reservation"
    );

    let (mut peer, mut peer_finder, peer_first, peer_second) = make_world();
    assert_eq!((first, second), (peer_first, peer_second));
    peer_finder
        .grid
        .update_ground_goal_cell(peer_first.0, 0, 1.0, false, (-1, -1), goal);
    let peer_path = peer_finder
        .find_closest_path(
            Vec3::new(20.0, 0.0, 50.0),
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &ground,
            peer.host_objects(),
            peer_second,
        )
        .expect("peer closest path");
    assert_ne!(
        peer_finder.grid.world_to_grid(*peer_path.last().unwrap()),
        peer_finder.grid.world_to_grid(goal),
        "another unit's goal stays reserved"
    );
    assert!(
        peer_finder
            .find_closest_path(
                Vec3::new(20.0, 0.0, 50.0),
                goal,
                SURFACE_GROUND,
                false,
                true,
                0.0,
                &ground,
                peer.host_objects(),
                ObjectId(999),
            )
            .is_none(),
        "a missing mover must not inherit the last search's identity"
    );

    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    let mut restored_finder = PathfindingSystem::new(200.0, 200.0);
    restored_finder
        .grid
        .update_ground_goal_cell(second.0, 0, 1.0, false, (-1, -1), goal);
    restored_finder.find_path_ex_surfaces(
        Vec3::new(20.0, 0.0, 20.0),
        Vec3::new(50.0, 0.0, 20.0),
        peer.host_objects(),
        false,
        SURFACE_GROUND,
        false,
        Some(first),
        &ground,
    );
    let restored = restored_finder
        .find_closest_path(
            Vec3::new(20.0, 0.0, 50.0),
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &ground,
            peer.host_objects(),
            second,
        )
        .expect("restored closest path");
    assert_eq!(
        restored_finder
            .grid
            .world_to_grid(*restored.last().unwrap()),
        restored_finder.grid.world_to_grid(goal),
        "restored session keeps the receiving mover's identity"
    );
    let original_again = finder
        .find_closest_path(
            Vec3::new(20.0, 0.0, 50.0),
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &ground,
            world.host_objects(),
            second,
        )
        .expect("original session after peer restore");
    assert_eq!(
        finder.grid.world_to_grid(*original_again.last().unwrap()),
        finder.grid.world_to_grid(goal),
        "restoring the peer must not change the original session"
    );
}
