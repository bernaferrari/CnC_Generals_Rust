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
    let services = gamelogic::system::engine_stores::new_world_services();
    let snapshot = gamelogic::system::engine_stores::with_world_services(&services, || {
        builder.create_world_snapshot(&world).unwrap()
    });
    gamelogic::system::engine_stores::with_world_services(&services, || {
        builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    });
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

#[test]
fn closest_search_uses_current_controller_extent_after_another_query() {
    use generals_main::game_logic::pathfinding::GridPos;
    let (mut world, mut finder, human, computer) = make_world();
    let (mut peer, mut peer_finder, peer_human, peer_computer) = make_world();
    assert_eq!((human, computer), (peer_human, peer_computer));
    for (w, id) in [(&mut world, computer), (&mut peer, peer_computer)] {
        w.add_player(Player::new(1, Team::GLA, "Computer", false));
        w.host_object_mut(id).unwrap().owner_player_id = Some(1);
    }
    for f in [&mut finder, &mut peer_finder] {
        f.grid
            .set_logical_extent(GridPos::new(0, 0), GridPos::new(9, 19));
    }
    let ground = PathfindingGround::Flat(0.0);
    let from = Vec3::new(25.0, 0.0, 55.0);
    let goal = Vec3::new(145.0, 0.0, 55.0);
    let seed_human = |f: &mut PathfindingSystem, w: &GameLogic, id| {
        f.find_path_ex_surfaces(
            from,
            Vec3::new(65.0, 0.0, 55.0),
            w.host_objects(),
            false,
            SURFACE_GROUND,
            false,
            Some(id),
            &ground,
        )
        .expect("human seed path");
    };
    let query = |f: &mut PathfindingSystem, w: &GameLogic, id, is_human| {
        f.find_closest_path(
            from,
            goal,
            SURFACE_GROUND,
            false,
            is_human,
            0.0,
            &ground,
            w.host_objects(),
            id,
        )
        .expect("closest path")
    };
    let baseline = query(&mut peer_finder, &peer, peer_computer, false);
    assert_eq!(
        peer_finder.grid.world_to_grid(*baseline.last().unwrap()),
        GridPos::new(14, 5)
    );
    seed_human(&mut finder, &world, human);
    let after_human = query(&mut finder, &world, computer, false);
    assert_eq!(
        finder.grid.world_to_grid(*after_human.last().unwrap()),
        GridPos::new(14, 5),
        "computer can traverse beyond the human extent"
    );
    let human_path = query(&mut finder, &world, human, true);
    assert!(
        finder.grid.world_to_grid(*human_path.last().unwrap()).x <= 9,
        "human remains inside the logical extent"
    );
    assert_eq!(
        query(&mut peer_finder, &peer, peer_computer, false),
        baseline,
        "peer finder stays independent"
    );
}

#[test]
fn closest_goal_checks_the_receiving_units_whole_footprint() {
    use generals_main::game_logic::HostGeometryType;
    use generals_main::game_logic::pathfinding::GridPos;
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    for (name, radius) in [("Wide", 10.0), ("Ally", 2.0)] {
        let mut unit = ThingTemplate::new(name);
        unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
        unit.geometry_info.geom_type = HostGeometryType::Sphere;
        unit.geometry_info.major_radius = radius;
        unit.geometry_info.authored = true;
        world.templates.insert(name.into(), unit);
    }
    let mover = world
        .create_object_for_player("Wide", 0, Vec3::new(25.0, 0.0, 55.0))
        .unwrap();
    let ally = world
        .create_object_for_player("Ally", 0, Vec3::new(180.0, 0.0, 180.0))
        .unwrap();
    let mut finder = PathfindingSystem::new(200.0, 200.0);
    let from = Vec3::new(25.0, 0.0, 55.0);
    let goal = Vec3::new(100.0, 0.0, 50.0);
    finder.grid.update_ground_goal_cell(
        ally.0,
        0,
        2.0,
        false,
        (-1, -1),
        Vec3::new(95.0, 0.0, 45.0),
    );
    let path = finder
        .find_closest_path(
            from,
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("a nearby reachable candidate");
    assert_ne!(
        finder.grid.world_to_grid(*path.last().unwrap()),
        GridPos::new(10, 5),
        "the allied goal occupies the receiving unit's destination footprint"
    );
}

#[test]
fn human_closest_path_reenters_from_an_adjacent_outside_cell() {
    use generals_main::game_logic::HostGeometryType;
    use generals_main::game_logic::pathfinding::GridPos;
    let make = || {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut template = ThingTemplate::new("Unit");
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        template.geometry_info.geom_type = HostGeometryType::Sphere;
        template.geometry_info.major_radius = 2.0;
        template.geometry_info.authored = true;
        world.templates.insert("Unit".into(), template);
        let mover = world
            .create_object_for_player("Unit", 0, Vec3::new(105.0, 0.0, 55.0))
            .unwrap();
        (world, PathfindingSystem::new(200.0, 200.0), mover)
    };
    let (world, mut finder, mover) = make();
    let (peer, mut peer_finder, peer_mover) = make();
    assert_eq!(mover, peer_mover);
    for f in [&mut finder, &mut peer_finder] {
        f.grid
            .set_logical_extent(GridPos::new(0, 0), GridPos::new(9, 19));
    }
    let ground = PathfindingGround::Flat(0.0);
    let from = Vec3::new(105.0, 0.0, 55.0);
    let goal = Vec3::new(85.0, 0.0, 55.0);
    let direct = finder
        .find_path_ex_surfaces(
            from,
            goal,
            world.host_objects(),
            false,
            SURFACE_GROUND,
            false,
            Some(mover),
            &ground,
        )
        .expect("ordinary path search can reenter from the outside start");
    assert_eq!(direct.first(), Some(&from));
    assert_eq!(
        finder.grid.world_to_grid(*direct.last().unwrap()),
        GridPos::new(8, 5)
    );
    assert!(
        finder
            .find_path_ex_surfaces(
                Vec3::new(115.0, 0.0, 55.0),
                goal,
                world.host_objects(),
                false,
                SURFACE_GROUND,
                false,
                Some(mover),
                &ground,
            )
            .is_none(),
        "human neighbors outside the logical extent remain forbidden"
    );
    let query = |f: &mut PathfindingSystem, w: &GameLogic, id| {
        f.find_closest_path(
            from,
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &ground,
            w.host_objects(),
            id,
        )
        .expect("human should reenter from an adjacent cell")
    };
    let expected = query(&mut peer_finder, &peer, peer_mover);
    let path = query(&mut finder, &world, mover);
    assert_eq!(
        path.first(),
        Some(&from),
        "the true start is not silently relocated"
    );
    assert_eq!(
        finder.grid.world_to_grid(*path.last().unwrap()),
        GridPos::new(8, 5)
    );
    assert_eq!(query(&mut peer_finder, &peer, peer_mover), expected);
    let same = finder
        .find_closest_path(
            from,
            from,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &ground,
            world.host_objects(),
            mover,
        )
        .expect("the starting cell can itself be the goal");
    assert_eq!(
        finder.grid.world_to_grid(*same.last().unwrap()),
        GridPos::new(10, 5)
    );
}

#[test]
fn ordinary_path_reads_current_player_type_without_frame_refresh() {
    use generals_main::game_logic::pathfinding::GridPos;
    let (mut world, mut finder, mover, _) = make_world();
    world.get_player_mut(0).unwrap().is_human = false;
    finder
        .grid
        .set_logical_extent(GridPos::new(0, 0), GridPos::new(9, 19));
    let from = Vec3::new(25.0, 0.0, 55.0);
    let goal = Vec3::new(145.0, 0.0, 55.0);
    let ground = PathfindingGround::Flat(0.0);
    let query = |finder: &mut PathfindingSystem, world: &GameLogic| {
        finder.find_path_ex_surfaces(
            from,
            goal,
            world.host_objects(),
            false,
            SURFACE_GROUND,
            false,
            Some(mover),
            &ground,
        )
    };
    let computer = query(&mut finder, &world);
    world.get_player_mut(0).unwrap().is_human = true;
    let human = query(&mut finder, &world);
    let computer_end = *computer.expect("computer path").last().unwrap();
    let human_end = *human.expect("human path").last().unwrap();
    assert_eq!(
        computer_end, goal,
        "computer can reach outside the human extent"
    );
    assert!(
        finder
            .grid
            .in_logical_extent(finder.grid.world_to_grid(human_end)),
        "the next query must respect the current human controller"
    );
}

#[test]
fn closest_search_tunnels_from_an_invalid_surface_until_goal_or_valid_exit() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let (mut world, mut finder, mover, _) = make_world();
    let surfaces = gamelogic::ai::pathfind_complete::SURFACE_WATER;
    world.host_object_mut(mover).unwrap().locomotor_surfaces = surfaces;
    let from = Vec3::new(25.0, 0.0, 55.0);
    let goal = Vec3::new(75.0, 0.0, 55.0);
    let ground = PathfindingGround::Flat(0.0);
    let query = |finder: &mut PathfindingSystem| {
        finder.find_closest_path(
            from,
            goal,
            surfaces,
            false,
            true,
            0.0,
            &ground,
            world.host_objects(),
            mover,
        )
    };
    let invalid_start = query(&mut finder).expect("C++ tunnels out of an invalid starting surface");
    assert_eq!(invalid_start.last().copied(), Some(goal));

    let mut valid_start = PathfindingSystem::new(200.0, 200.0);
    let start_cell = valid_start.grid.world_to_grid(from);
    valid_start
        .grid
        .set_cell_type(start_cell, PathfindCellType::Water);
    let water_start =
        query(&mut valid_start).expect("the starting water cell is a valid closest result");
    assert_ne!(
        water_start.last().copied(),
        Some(goal),
        "without tunneling a water-only unit cannot cross ground cells"
    );
}

#[test]
fn closest_same_cell_preserves_original_start_before_cell_center() {
    let make = |position| {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut template = ThingTemplate::new("Mover");
        template.add_kind_of(KindOf::Infantry).set_health(100.0);
        world.templates.insert("Mover".into(), template);
        let mover = world
            .create_object_for_player("Mover", 0, position)
            .unwrap();
        (world, PathfindingSystem::new(200.0, 200.0), mover)
    };
    let from = Vec3::new(21.0, 0.0, 52.0);
    let (world, mut finder, mover) = make(from);
    let goal = Vec3::new(22.0, 0.0, 53.0);
    let path = finder
        .find_closest_path(
            from,
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("same-cell closest path");
    assert_eq!(
        path,
        vec![from, Vec3::new(25.0, 0.0, 55.0)],
        "original prependCells keeps the actual start before the cell center"
    );

    let centered = Vec3::new(25.0, 0.0, 55.0);
    let (world, mut finder, mover) = make(centered);
    let path = finder
        .find_closest_path(
            centered,
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("centered closest path");
    assert_eq!(path, vec![centered]);
}

#[test]
fn closest_same_cell_uses_even_footprint_inset() {
    use generals_main::game_logic::HostGeometryType;
    let from = Vec3::new(21.0, 0.0, 52.0);
    let goal = Vec3::new(22.0, 0.0, 53.0);
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("WideMover");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.geometry_info.geom_type = HostGeometryType::Sphere;
    template.geometry_info.major_radius = 8.0;
    template.geometry_info.authored = true;
    world.templates.insert("WideMover".into(), template);
    let mover = world
        .create_object_for_player("WideMover", 0, from)
        .unwrap();
    let mut finder = PathfindingSystem::new(200.0, 200.0);
    let path = finder
        .find_closest_path(
            from,
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("same-cell closest path");
    assert_eq!(path, vec![from, Vec3::new(20.5, 0.0, 50.5)]);
}

#[test]
fn closest_path_backs_off_a_pinched_goal_to_its_open_parent() {
    let (world, mut finder, mover, _) = make_world();
    let from = Vec3::new(25.0, 0.0, 55.0);
    let goal = Vec3::new(75.0, 0.0, 55.0);
    let goal_cell = finder.grid.world_to_grid(goal);
    finder.grid.set_pinched(goal_cell, true);
    let path = finder
        .find_closest_path(
            from,
            goal,
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("closest path to pinched goal");
    let end_cell = finder.grid.world_to_grid(*path.last().unwrap());
    assert_ne!(
        end_cell, goal_cell,
        "C++ buildActualPath backs off a pinched goal"
    );
    assert!(
        !finder.grid.is_pinched(end_cell),
        "the selected parent is not pinched"
    );
}

#[test]
fn closest_search_clips_or_clamps_outside_endpoints_like_cpp() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("ClipMover");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("ClipMover".into(), template);
    let mover = world
        .create_object_for_player("ClipMover", 0, Vec3::new(25.0, 0.0, 55.0))
        .unwrap();
    let ground = PathfindingGround::Flat(0.0);
    for (name, from, goal, expected_end) in [
        (
            "goal-right",
            Vec3::new(25.0, 0.0, 55.0),
            Vec3::new(250.0, 0.0, 55.0),
            Vec3::new(195.0, 0.0, 55.0),
        ),
        (
            "start-left",
            Vec3::new(-25.0, 0.0, 55.0),
            Vec3::new(75.0, 0.0, 55.0),
            Vec3::new(75.0, 0.0, 55.0),
        ),
        (
            "diagonal",
            Vec3::new(-30.0, 0.0, 20.0),
            Vec3::new(230.0, 0.0, 160.0),
            Vec3::new(195.0, 0.0, 145.0),
        ),
        (
            "exterior",
            Vec3::new(-50.0, 0.0, 55.0),
            Vec3::new(-20.0, 0.0, 75.0),
            Vec3::new(5.0, 0.0, 75.0),
        ),
    ] {
        let mut finder = PathfindingSystem::new(200.0, 200.0);
        let path = finder
            .find_closest_path(
                from,
                goal,
                SURFACE_GROUND,
                false,
                true,
                0.0,
                &ground,
                world.host_objects(),
                mover,
            )
            .unwrap_or_else(|| panic!("{name}: closest path"));
        assert_eq!(
            path.first().copied(),
            Some(from),
            "{name}: preserve original start"
        );
        assert_eq!(
            path.last().copied(),
            Some(expected_end),
            "{name}: C++ clipped goal cell"
        );
    }
}

#[test]
fn closest_clipping_uses_the_unshifted_start_for_even_footprints() {
    use generals_main::game_logic::HostGeometryType;
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("WideClipMover");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.geometry_info.geom_type = HostGeometryType::Sphere;
    template.geometry_info.major_radius = 8.0;
    template.geometry_info.authored = true;
    world.templates.insert("WideClipMover".into(), template);
    let from = Vec3::new(25.0, 0.0, 55.0);
    let mover = world
        .create_object_for_player("WideClipMover", 0, from)
        .unwrap();
    let mut finder = PathfindingSystem::new(200.0, 200.0);
    let path = finder
        .find_closest_path(
            from,
            Vec3::new(250.0, 0.0, 55.0),
            SURFACE_GROUND,
            false,
            true,
            0.0,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("even-footprint clipped path");
    assert_eq!(path.first().copied(), Some(from));
    assert_eq!(path.last().copied(), Some(Vec3::new(190.5, 0.0, 60.5)));
}

#[test]
fn closest_clipping_rejects_empty_grids_and_nonfinite_coordinates() {
    let (world, mut finder, mover, _) = make_world();
    let ground = PathfindingGround::Flat(0.0);
    let from = Vec3::new(25.0, 0.0, 55.0);
    let goal = Vec3::new(75.0, 0.0, 55.0);
    assert!(
        finder
            .find_closest_path(
                from,
                Vec3::splat(f32::NAN),
                SURFACE_GROUND,
                false,
                true,
                0.0,
                &ground,
                world.host_objects(),
                mover
            )
            .is_none()
    );
    let mut empty = PathfindingSystem::new(0.0, 0.0);
    assert!(
        empty
            .find_closest_path(
                from,
                goal,
                SURFACE_GROUND,
                false,
                true,
                0.0,
                &ground,
                world.host_objects(),
                mover
            )
            .is_none()
    );
}

#[test]
fn closest_path_distinguishes_ground_and_bridge_cells_at_the_same_coordinates() {
    use generals_main::game_logic::HostGeometryType;

    let ground = PathfindingGround::Flat(0.0);
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut unit = ThingTemplate::new("Mover");
    unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
    unit.geometry_info.geom_type = HostGeometryType::Sphere;
    unit.geometry_info.major_radius = 2.0;
    unit.geometry_info.authored = true;
    world.templates.insert("Mover".into(), unit);
    let mover = world
        .create_object_for_player("Mover", 0, Vec3::new(85.0, 0.0, 55.0))
        .unwrap();
    let mut finder = PathfindingSystem::new(200.0, 200.0);
    finder.grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let on_ground = Vec3::new(85.0, 0.0, 55.0);
    let on_bridge = Vec3::new(85.0, 20.0, 55.0);
    for (layer, from, to) in [(1, on_ground, on_bridge), (2, on_bridge, on_ground)] {
        let object = world.host_object_mut(mover).unwrap();
        object.pathfind_layer = layer;
        let path = finder
            .find_closest_path(
                from,
                to,
                SURFACE_GROUND,
                false,
                true,
                0.0,
                &ground,
                world.host_objects(),
                mover,
            )
            .expect("path between bridge and ground");
        assert_eq!(*path.last().unwrap(), to, "layer {layer}: {path:?}");
        assert!(
            path.len() > 1,
            "changing layers requires travel to a connection: {path:?}"
        );
    }
}

#[test]
fn closest_search_rejects_a_locomotor_without_valid_surfaces() {
    let (world, mut finder, mover, _) = make_world();
    let from = Vec3::new(25.0, 0.0, 25.0);
    let goal = Vec3::new(75.0, 0.0, 25.0);
    assert!(
        finder
            .find_closest_path(
                from,
                goal,
                0,
                false,
                true,
                0.0,
                &PathfindingGround::Flat(0.0),
                world.host_objects(),
                mover,
            )
            .is_none(),
        "an immobile locomotor must not tunnel to the goal"
    );
}

#[test]
fn closest_search_does_not_readmit_an_already_open_neighbor() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::HostGeometryType;
    use generals_main::game_logic::pathfinding::GridPos;

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut unit = ThingTemplate::new("Mover");
    unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
    unit.geometry_info.geom_type = HostGeometryType::Sphere;
    unit.geometry_info.major_radius = 2.0;
    unit.geometry_info.authored = true;
    world.templates.insert("Mover".into(), unit);
    let from = Vec3::new(25.0, 0.0, 25.0);
    let mover = world.create_object_for_player("Mover", 0, from).unwrap();
    // The original skips line seeding for downhill-only locomotors. Keep
    // this test focused on ordinary neighbor admission on flat ground.
    world.host_object_mut(mover).unwrap().downhill_only = true;
    let mut finder = PathfindingSystem::new(140.0, 120.0);
    for (x, y) in [(3, 3), (4, 2), (10, 8)] {
        finder
            .grid
            .set_cell_type(GridPos::new(x, y), PathfindCellType::Impassable);
    }
    let path = finder
        .find_closest_path(
            from,
            Vec3::new(105.0, 0.0, 85.0),
            SURFACE_GROUND,
            false,
            true,
            0.2,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("a nearby reachable destination");
    assert_eq!(
        finder.grid.world_to_grid(*path.last().unwrap()),
        GridPos::new(7, 8),
        "{path:?}"
    );
}

#[test]
fn closest_search_seeds_the_original_line_toward_a_blocked_goal() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::HostGeometryType;
    use generals_main::game_logic::pathfinding::GridPos;

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut unit = ThingTemplate::new("Mover");
    unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
    unit.geometry_info.geom_type = HostGeometryType::Sphere;
    unit.geometry_info.major_radius = 2.0;
    unit.geometry_info.authored = true;
    world.templates.insert("Mover".into(), unit);
    let from = Vec3::new(25.0, 0.0, 25.0);
    let mover = world.create_object_for_player("Mover", 0, from).unwrap();
    let mut finder = PathfindingSystem::new(140.0, 120.0);
    finder
        .grid
        .set_cell_type(GridPos::new(10, 8), PathfindCellType::Impassable);
    let path = finder
        .find_closest_path(
            from,
            Vec3::new(105.0, 0.0, 85.0),
            SURFACE_GROUND,
            false,
            true,
            0.2,
            &PathfindingGround::Flat(0.0),
            world.host_objects(),
            mover,
        )
        .expect("a nearby reachable destination");
    assert_eq!(
        finder.grid.world_to_grid(*path.last().unwrap()),
        GridPos::new(9, 7),
        "{path:?}"
    );
}

#[test]
fn closest_seed_respects_receiving_world_occupants() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::HostGeometryType;
    use generals_main::game_logic::pathfinding::GridPos;

    let mut results = Vec::new();
    for (fixed, crusher, unmanned) in [(false, 0, false), (true, 2, false), (true, 2, true)] {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world.add_player(Player::new(1, Team::GLA, "Enemy", false));
        let mut unit = ThingTemplate::new("Mover");
        unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
        unit.geometry_info.geom_type = HostGeometryType::Sphere;
        unit.geometry_info.major_radius = 2.0;
        unit.geometry_info.authored = true;
        world.templates.insert("Mover".into(), unit);
        let from = Vec3::new(25.0, 0.0, 25.0);
        let enemy_pos = Vec3::new(35.0, 0.0, 35.0);
        let mover = world.create_object_for_player("Mover", 0, from).unwrap();
        let enemy = world
            .create_object_for_player("Mover", 1, enemy_pos)
            .unwrap();
        world.host_object_mut(mover).unwrap().crusher_level = crusher;
        world
            .host_object_mut(mover)
            .unwrap()
            .status
            .disabled_unmanned = unmanned;
        world.host_object_mut(enemy).unwrap().crushable_level = 1;
        let mut finder = PathfindingSystem::new(140.0, 120.0);
        for (x, y) in [
            (3, 6),
            (4, 3),
            (4, 7),
            (5, 6),
            (6, 5),
            (7, 3),
            (8, 4),
            (10, 8),
        ] {
            finder
                .grid
                .set_cell_type(GridPos::new(x, y), PathfindCellType::Impassable);
        }
        let footprint = finder
            .grid
            .path_position_footprint(world.host_object(enemy).unwrap());
        finder
            .grid
            .update_path_position_cell(&footprint, (-1, -1), enemy_pos);
        if fixed {
            finder
                .grid
                .update_ground_goal_cell(enemy.0, 1, 2.0, false, (-1, -1), enemy_pos);
        }
        let path = finder
            .find_closest_path(
                from,
                Vec3::new(105.0, 0.0, 85.0),
                SURFACE_GROUND,
                false,
                true,
                0.2,
                &PathfindingGround::Flat(0.0),
                world.host_objects(),
                mover,
            )
            .expect("a nearby destination");
        results.push(finder.grid.world_to_grid(*path.last().unwrap()));
    }
    assert_eq!(
        results[0],
        GridPos::new(9, 7),
        "a moving enemy does not stop the line seed"
    );
    assert_eq!(
        results[1],
        GridPos::new(9, 7),
        "a crushable fixed enemy does not stop the line seed"
    );
    assert_eq!(
        results[2],
        GridPos::new(10, 7),
        "an unmanned unit cannot crush the fixed enemy"
    );
}
