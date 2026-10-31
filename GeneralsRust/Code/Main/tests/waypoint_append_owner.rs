use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("WaypointInfantry");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("WaypointInfantry".into(), template);
    let unit = world
        .create_object_for_player("WaypointInfantry", 0, Vec3::ZERO)
        .expect("unit");
    (world, unit)
}

#[test]
fn appending_waypoints_keeps_the_current_movement_target_in_each_world() {
    let (mut first, first_id) = world();
    let (mut second, second_id) = world();
    assert_eq!(first_id, second_id);
    let first_goal = Vec3::new(80.0, 0.0, 0.0);
    let second_goal = Vec3::new(0.0, 0.0, 90.0);
    assert!(first.append_unit_waypoint(first_id, first_goal));
    assert!(second.append_unit_waypoint(second_id, second_goal));
    let first_target = first
        .host_object(first_id)
        .unwrap()
        .movement
        .target_position;
    let second_target = second
        .host_object(second_id)
        .unwrap()
        .movement
        .target_position;
    assert!(first_target.is_some());
    assert!(second_target.is_some());
    assert_eq!(
        first.host_object(first_id).unwrap().movement.path.last(),
        Some(&first_goal)
    );
    assert_eq!(
        second.host_object(second_id).unwrap().movement.path.last(),
        Some(&second_goal)
    );
    let first_path = first.host_object(first_id).unwrap().movement.path.clone();
    let appended_goal = Vec3::new(160.0, 0.0, 0.0);
    assert!(first.append_unit_waypoint(first_id, appended_goal));
    let updated_path = &first.host_object(first_id).unwrap().movement.path;
    assert!(updated_path.starts_with(&first_path));
    assert_eq!(updated_path.last(), Some(&appended_goal));
    assert_eq!(
        first
            .host_object(first_id)
            .unwrap()
            .movement
            .target_position,
        first_target
    );
    assert_eq!(
        second
            .host_object(second_id)
            .unwrap()
            .movement
            .target_position,
        second_target
    );
}

#[test]
fn appending_to_an_unpathed_move_queues_after_the_active_goal() {
    let (mut world, id) = world();
    let current_goal = Vec3::new(80.0, 0.0, 0.0);
    let next_goal = Vec3::new(160.0, 0.0, 0.0);
    world.host_object_mut(id).unwrap().movement.target_position = Some(current_goal);

    assert!(world.append_unit_waypoint(id, next_goal));
    let movement = &world.host_object(id).unwrap().movement;
    assert_eq!(movement.target_position, Some(current_goal));
    assert_eq!(movement.path[movement.current_path_index], current_goal);
    assert_eq!(movement.path.last(), Some(&next_goal));
}

#[test]
fn first_waypoint_starts_on_the_first_path_segment() {
    let (mut world, id) = world();
    assert!(world.append_unit_waypoint(id, Vec3::new(80.0, 0.0, 0.0)));
    let movement = &world.host_object(id).unwrap().movement;
    assert_eq!(
        movement.path.get(movement.current_path_index).copied(),
        movement.target_position
    );
}

#[test]
fn waypoints_are_traversed_in_order_by_the_owning_world() {
    use game_engine::common::ini::ini_locomotor::LocomotorStore;
    use game_engine::common::system::DataChunkOutput;
    use generals_main::game_logic::GameMode;
    use generals_main::save_load::{Snapshot, WorldSnapshot, XferLoad, XferSave};
    use std::io::Cursor;

    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("WaypointWalk.map");
    let mut data = DataChunkOutput::new();
    data.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        data.write_int(value);
    }
    for _ in 0..1681 {
        data.write_byte(0);
    }
    data.close_data_chunk();
    data.open_data_chunk("ObjectsList", 3);
    data.close_data_chunk();
    std::fs::write(&map, data.into_ckmp_bytes()).unwrap();

    let make_world = || {
        let mut world = GameLogic::new();
        let mut locomotors = LocomotorStore::new_owned();
        locomotors.apply_source("Locomotor WaypointWalk\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 360\n Appearance = TWO_LEGS\nEnd\n").unwrap();
        world.admit_locomotor_definitions(locomotors).unwrap();
        world.admit_object_ini_sources([("WaypointWalk.ini".into(), "Object WaypointWalker\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL WaypointWalk\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into())]).unwrap();
        world.start_new_game(GameMode::SinglePlayer);
        assert!(world.load_map(map.to_str().unwrap()));
        world.add_player(Player::new(1, Team::USA, "Owner", true));
        world
    };
    let mut world = make_world();
    let mut restored = make_world();
    let mut reloaded = make_world();
    let id = world
        .create_object_for_player("WaypointWalker", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let first = Vec3::new(100.0, 0.0, 150.0);
    let second = Vec3::new(150.0, 0.0, 150.0);
    assert!(world.append_unit_waypoint(id, first));
    assert!(world.append_unit_waypoint(id, second));

    for _ in 0..5 {
        world.update_with_dt(1.0 / 30.0);
    }
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let mut snapshot = builder
        .create_world_snapshot(&world)
        .expect("capture route");
    let mut bytes = Cursor::new(Vec::new());
    snapshot
        .xfer(&mut XferSave::new(&mut bytes))
        .expect("write route");
    let mut snapshot = WorldSnapshot::default();
    snapshot
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .expect("read route");
    let unit = world.host_object(id).unwrap();
    let saved_motion = (
        unit.get_position(),
        unit.movement.velocity,
        unit.movement.target_position,
        unit.movement.current_path_index,
    );
    let mut expected = Vec::new();
    for _ in 0..10 {
        let timing = world.update_with_dt(1.0 / 30.0);
        let unit = world.host_object(id).expect("unit survives");
        expected.push((
            timing.frame,
            timing.steps_run,
            unit.get_position(),
            unit.movement.target_position,
            unit.movement.current_path_index,
        ));
    }
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore route");
    let unit = restored.host_object(id).unwrap();
    assert_eq!(
        (
            unit.get_position(),
            unit.movement.velocity,
            unit.movement.target_position,
            unit.movement.current_path_index
        ),
        saved_motion,
        "restore must preserve motion before stepping"
    );
    for &expected_step in &expected {
        let timing = restored.update_with_dt(1.0 / 30.0);
        let unit = restored.host_object(id).expect("restored unit survives");
        assert_eq!(
            (
                timing.frame,
                timing.steps_run,
                unit.get_position(),
                unit.movement.target_position,
                unit.movement.current_path_index,
            ),
            expected_step,
            "saved route must continue with the same movement state"
        );
    }

    builder
        .restore_from_snapshot(&snapshot, &mut reloaded)
        .expect("prepare active session");
    for _ in 0..10 {
        reloaded.update_with_dt(1.0 / 30.0);
    }
    builder
        .restore_from_snapshot(&snapshot, &mut reloaded)
        .expect("reload active session");
    let mut after_reload = builder.create_world_snapshot(&reloaded).unwrap();
    assert_eq!(
        after_reload.logic_rng_seed_words,
        snapshot.logic_rng_seed_words
    );
    for expected_step in expected {
        let timing = reloaded.update_with_dt(1.0 / 30.0);
        let unit = reloaded.host_object(id).expect("reloaded unit survives");
        assert_eq!(
            (
                timing.frame,
                timing.steps_run,
                unit.get_position(),
                unit.movement.target_position,
                unit.movement.current_path_index
            ),
            expected_step,
            "reloading an advanced session must preserve route continuation"
        );
    }

    let before_rejection = (
        reloaded.get_frame(),
        reloaded.host_object(id).unwrap().get_position(),
        reloaded.host_object(id).unwrap().movement.target_position,
    );
    let peer_before = restored.host_object(id).unwrap().get_position();
    after_reload.version = 0;
    assert!(
        builder
            .restore_from_snapshot(&after_reload, &mut reloaded)
            .is_err()
    );
    assert_eq!(
        (
            reloaded.get_frame(),
            reloaded.host_object(id).unwrap().get_position(),
            reloaded.host_object(id).unwrap().movement.target_position,
        ),
        before_rejection,
        "invalid snapshot must fail before changing the receiving timeline"
    );
    assert_eq!(
        restored.host_object(id).unwrap().get_position(),
        peer_before
    );

    reloaded.reset();
    reloaded.start_new_game(GameMode::SinglePlayer);
    assert!(reloaded.load_map(map.to_str().unwrap()));
    reloaded.add_player(Player::new(1, Team::USA, "Owner", true));
    let restarted = reloaded
        .create_object_for_player("WaypointWalker", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    assert!(reloaded.append_unit_waypoint(restarted, first));
    for _ in 0..10 {
        reloaded.update_with_dt(1.0 / 30.0);
    }
    assert!(
        reloaded
            .host_object(restarted)
            .unwrap()
            .get_position()
            .distance(Vec3::new(100.0, 0.0, 100.0))
            > 0.1,
        "reset must not postpone the restarted world's recurring movement pass"
    );

    let mut reached_first = false;
    let mut reached_second = false;
    for _ in 0..600 {
        world.update_with_dt(1.0 / 30.0);
        restored.update_with_dt(1.0 / 30.0);
        let unit = world.host_object(id).expect("unit survives");
        let peer = restored.host_object(id).expect("restored unit survives");
        assert_eq!(
            (
                unit.get_position(),
                unit.movement.velocity,
                unit.movement.target_position,
                unit.movement.current_path_index
            ),
            (
                peer.get_position(),
                peer.movement.velocity,
                peer.movement.target_position,
                peer.movement.current_path_index
            ),
            "freshly restored session must continue the same route"
        );
        let position = unit.get_position();
        if position.distance(first) < 15.0 {
            reached_first = true;
        }
        if position.distance(second) < 15.0 {
            reached_second = true;
            assert!(
                reached_first,
                "reached second goal before first: {position:?}"
            );
            break;
        }
    }
    let object = world.host_object(id).unwrap();
    assert!(
        reached_first && reached_second,
        "first={reached_first}, second={reached_second}, pos={:?}, target={:?}, path={:?}, index={}",
        object.get_position(),
        object.movement.target_position,
        object.movement.path,
        object.movement.current_path_index
    );
}
