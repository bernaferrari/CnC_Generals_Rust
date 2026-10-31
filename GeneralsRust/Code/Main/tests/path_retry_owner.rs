use game_engine::common::{ini::ini_locomotor::LocomotorStore, system::DataChunkOutput};
use generals_main::game_logic::{
    GameLogic, GameMode, Object, ObjectId, Player, Team, ThingTemplate,
};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

#[test]
fn rapid_retry_uses_the_original_unsigned_frame_comparison() {
    let mut unit = Object::new(ThingTemplate::new("Retry"), ObjectId(1), Team::USA);
    unit.path_timestamp = 1;
    assert!(unit.begin_request_move_path(Vec3::X, 2));
    assert_eq!(unit.queue_for_path_frame, 0);
    assert!(!unit.begin_request_move_path(Vec3::X, 3));
    assert_eq!(unit.queue_for_path_frame, 33);
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    let mut locomotors = LocomotorStore::new_owned();
    locomotors.apply_source("Locomotor RetryWalk\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 360\n Appearance = TWO_LEGS\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world.admit_object_ini_sources([("RetryWalk.ini".into(), "Object RetryWalker\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL RetryWalk\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into())]).unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world
}

#[test]
fn delayed_request_runs_on_its_actor_deadline_after_save_and_interleaving() {
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("Retry.map");
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

    let mut original = world(&map);
    let mut restored = world(&map);
    let mut other = world(&map);
    let id = original
        .create_object_for_player("RetryWalker", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let other_id = other
        .create_object_for_player("RetryWalker", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    assert_eq!(id, other_id);
    original.set_current_frame(10);
    other.set_current_frame(90);
    original.host_object_mut(id).unwrap().path_timestamp = 10;
    other.host_object_mut(other_id).unwrap().path_timestamp = 90;
    let goal = Vec3::new(100.0, 0.0, 150.0);
    assert!(original.assign_unit_path(id, goal, &[]));
    assert!(other.assign_unit_path(other_id, Vec3::new(150.0, 0.0, 100.0), &[]));
    assert_eq!(original.host_object(id).unwrap().queue_for_path_frame, 40);
    assert_eq!(
        other.host_object(other_id).unwrap().queue_for_path_frame,
        120
    );
    assert!(original.snapshot_pending_host_paths().is_empty());

    // C++ queueForPath ignores an ID that is already queued. In particular,
    // retrying it must not discard FollowPath goals added to that request.
    let queued = other
        .create_object_for_player("RetryWalker", 1, Vec3::new(200.0, 0.0, 100.0))
        .unwrap();
    let first = Vec3::new(200.0, 0.0, 150.0);
    let appended = Vec3::new(250.0, 0.0, 150.0);
    assert!(other.assign_unit_path(queued, first, &[Vec3::new(200.0, 0.0, 125.0)]));
    assert!(other.append_unit_waypoint(queued, appended));
    other.host_object_mut(queued).unwrap().queue_for_path_frame = 90;

    let builder = SnapshotBuilder::new();
    let mut legacy = builder.create_world_snapshot(&original).unwrap();
    let version = legacy
        .lifecycle_tail
        .windows(4)
        .position(|part| part == b"TMAI")
        .unwrap()
        + 4;
    legacy.lifecycle_tail[version..version + 4].copy_from_slice(&14_u32.to_le_bytes());
    let mut legacy_receiver = world(&map);
    builder
        .restore_from_snapshot(&legacy, &mut legacy_receiver)
        .unwrap();
    // The slot contains 40 in this synthetic v14 record, meaning 40 frames
    // remaining, rather than an absolute frame 40.
    assert_eq!(
        legacy_receiver
            .host_object(id)
            .unwrap()
            .queue_for_path_frame,
        50
    );
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.host_object(id).unwrap().queue_for_path_frame, 40);

    for step in 0..30 {
        original.update_with_dt(1.0 / 30.0);
        other.update_with_dt(1.0 / 30.0);
        restored.update_with_dt(1.0 / 30.0);
        if step == 0 {
            assert_eq!(
                other
                    .host_object(queued)
                    .unwrap()
                    .movement
                    .path
                    .last()
                    .copied(),
                Some(appended)
            );
        }
        for world in [&original, &restored] {
            assert!(world.host_object(id).unwrap().waiting_for_path);
            assert_eq!(world.host_object(id).unwrap().queue_for_path_frame, 40);
            assert!(world.snapshot_pending_host_paths().is_empty());
        }
    }
    for world in [&mut original, &mut restored] {
        world.update_with_dt(1.0 / 30.0);
        let unit = world.host_object(id).unwrap();
        assert_eq!(unit.queue_for_path_frame, 0);
        assert!(
            !unit.waiting_for_path,
            "AI must enqueue and the pathfinder must run"
        );
        assert_eq!(unit.movement.path.last().copied(), Some(goal));
    }
    assert_eq!(
        original.host_object(id).unwrap().movement.path,
        restored.host_object(id).unwrap().movement.path
    );
}

#[test]
fn queued_search_uses_the_current_owners_locomotor_surfaces() {
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("LiveSurfaces.map");
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

    let mut ground = world(&map);
    let mut changed = world(&map);
    let start = Vec3::new(100.0, 0.0, 100.0);
    let goal = Vec3::new(100.0, 0.0, 150.0);
    let mut ids = Vec::new();
    for world in [&mut ground, &mut changed] {
        let id = world
            .create_object_for_player("RetryWalker", 1, start)
            .unwrap();
        assert!(world.assign_unit_path(id, goal, &[]));
        assert!(world.host_object(id).unwrap().waiting_for_path);
        ids.push(id);
    }
    assert_eq!(ids[0], ids[1]);
    changed.host_object_mut(ids[1]).unwrap().locomotor_surfaces =
        gamelogic::ai::pathfind_complete::SURFACE_WATER;
    ground.update_with_dt(1.0 / 30.0);
    changed.update_with_dt(1.0 / 30.0);
    assert!(!ground.host_object(ids[0]).unwrap().movement.path.is_empty());
    assert!(
        changed
            .host_object(ids[1])
            .unwrap()
            .movement
            .path
            .is_empty(),
        "a water-only locomotor cannot use the ground-only terrain"
    );
}
