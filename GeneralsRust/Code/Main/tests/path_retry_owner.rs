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
    let length = version + 4;
    let size = u32::from_le_bytes(
        legacy.lifecycle_tail[length..length + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let end = length + 4 + size;
    assert_eq!(&legacy.lifecycle_tail[end - 8..end], &[0; 8]);
    legacy.lifecycle_tail.drain(end - 8..end);
    legacy.lifecycle_tail[length..length + 4].copy_from_slice(&((size - 8) as u32).to_le_bytes());
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

#[test]
fn tighten_continuation_is_actor_owned_and_survives_pending_save() {
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("Tighten.map");
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
    let mut peer = world(&map);
    let start = Vec3::new(100.0, 0.0, 100.0);
    let id = original
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    assert_eq!(id, peer_id);
    original.host_object_mut(id).unwrap().pathfind_goal_cell = (3, 3);
    assert!(original.unit_command_tighten_to(id, Vec3::new(100.0, 0.0, 150.0)));
    assert!(original.host_object(id).unwrap().waiting_for_path);
    assert!(
        original.host_object(id).unwrap().is_approach_path,
        "C++ tighten starts with requestApproachPath"
    );
    assert_eq!(
        original.host_object(id).unwrap().pathfind_goal_cell,
        (-1, -1)
    );
    assert!(!original.host_object(id).unwrap().adjust_destinations());
    assert!(original.host_object(id).unwrap().is_final_goal);
    assert!(peer.host_object(peer_id).unwrap().adjust_destinations());
    let builder = SnapshotBuilder::new();
    let mut throttled = world(&map);
    let throttled_id = throttled
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    throttled.set_current_frame(10);
    throttled
        .host_object_mut(throttled_id)
        .unwrap()
        .path_timestamp = 10;
    assert!(throttled.unit_command_tighten_to(throttled_id, Vec3::new(100.0, 0.0, 150.0)));
    let actor = throttled.host_object(throttled_id).unwrap();
    assert!(actor.is_approach_path);
    assert_eq!(
        actor.queue_for_path_frame, 70,
        "C++ approach retries wait two seconds"
    );
    assert!(throttled.snapshot_pending_host_paths().is_empty());
    let delayed_save = builder.create_world_snapshot(&throttled).unwrap();
    let mut delayed_restore = world(&map);
    builder
        .restore_from_snapshot(&delayed_save, &mut delayed_restore)
        .unwrap();
    let actor = delayed_restore.host_object(throttled_id).unwrap();
    assert!(actor.is_approach_path);
    assert_eq!(actor.queue_for_path_frame, 70);
    for world in [&mut throttled, &mut delayed_restore] {
        world.set_current_frame(70);
        world.update_with_dt(1.0 / 30.0);
        let actor = world.host_object(throttled_id).unwrap();
        assert!(!actor.waiting_for_path);
        assert!(actor.is_approach_path);
        assert!(!actor.movement.path.is_empty());
    }
    assert_eq!(
        throttled.host_object(throttled_id).unwrap().movement.path,
        delayed_restore
            .host_object(throttled_id)
            .unwrap()
            .movement
            .path
    );

    // requestApproachPath queues even when this actor can later take a quick
    // flying path. The switch happens when doPathfind resolves the actor.
    let mut flying = world(&map);
    let flying_id = flying
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    let aircraft = flying.host_object_mut(flying_id).unwrap();
    aircraft.object_type = generals_main::game_logic::ObjectType::Aircraft;
    aircraft.locomotor_surfaces = gamelogic::ai::pathfind_complete::SURFACE_AIR;
    assert!(flying.unit_command_tighten_to(flying_id, Vec3::new(100.0, 0.0, 150.0)));
    let aircraft = flying.host_object(flying_id).unwrap();
    assert!(aircraft.waiting_for_path && aircraft.is_approach_path);
    assert_eq!(flying.snapshot_pending_host_paths().len(), 1);
    flying.update_with_dt(1.0 / 30.0);
    let aircraft = flying.host_object(flying_id).unwrap();
    assert!(!aircraft.waiting_for_path);
    assert!(!aircraft.is_approach_path);
    assert!(!aircraft.movement.path.is_empty());

    // A disabled unit retains the request kind until it can move, including
    // across save/load; waking it must not turn the approach into a move.
    let mut disabled = world(&map);
    let disabled_id = disabled
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    disabled
        .host_object_mut(disabled_id)
        .unwrap()
        .set_status_disabled_emp(true);
    let ignored_obstacle = disabled
        .create_object_for_player("RetryWalker", 1, Vec3::new(250.0, 0.0, 250.0))
        .unwrap();
    disabled
        .host_object_mut(disabled_id)
        .unwrap()
        .ignored_obstacle_id = Some(ignored_obstacle);
    assert!(disabled.unit_command_tighten_to(disabled_id, Vec3::new(100.0, 0.0, 150.0)));
    let actor = disabled.host_object(disabled_id).unwrap();
    assert!(actor.pending_move.is_some() && actor.is_approach_path);
    let disabled_save = builder.create_world_snapshot(&disabled).unwrap();
    let mut disabled_restore = world(&map);
    builder
        .restore_from_snapshot(&disabled_save, &mut disabled_restore)
        .unwrap();
    for world in [&mut disabled, &mut disabled_restore] {
        world
            .host_object_mut(disabled_id)
            .unwrap()
            .set_status_disabled_emp(false);
        world.update_with_dt(1.0 / 30.0);
        let actor = world.host_object(disabled_id).unwrap();
        assert!(actor.is_approach_path);
        assert_eq!(actor.ignored_obstacle_id, Some(ignored_obstacle));
        assert!(!actor.waiting_for_path && !actor.movement.path.is_empty());
    }
    assert_eq!(
        disabled.host_object(disabled_id).unwrap().movement.path,
        disabled_restore
            .host_object(disabled_id)
            .unwrap()
            .movement
            .path
    );
    // Reject duplicate V16 final-goal receivers before mutating the peer.
    let mut malformed = builder.create_world_snapshot(&original).unwrap();
    let tag = malformed
        .lifecycle_tail
        .windows(4)
        .position(|v| v == b"TMAI")
        .unwrap();
    let length = tag + 8;
    let size = u32::from_le_bytes(
        malformed.lifecycle_tail[length..length + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let end = tag + 12 + size;
    assert_eq!(
        &malformed.lifecycle_tail[end - 12..end - 4],
        &1_u64.to_le_bytes()
    );
    assert_eq!(&malformed.lifecycle_tail[end - 4..end], &id.0.to_le_bytes());
    malformed.lifecycle_tail[end - 12..end - 4].copy_from_slice(&2_u64.to_le_bytes());
    malformed
        .lifecycle_tail
        .splice(end..end, id.0.to_le_bytes());
    malformed.lifecycle_tail[length..length + 4]
        .copy_from_slice(&((size + 4) as u32).to_le_bytes());
    peer.set_current_frame(7);
    assert!(builder
        .restore_from_snapshot(&malformed, &mut peer)
        .is_err());
    assert_eq!(peer.get_current_frame(), 7);
    assert!(peer.host_object(peer_id).unwrap().adjust_destinations());
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();

    for world in [&mut original, &mut restored] {
        world.update_with_dt(1.0 / 30.0);
        let actor = world.host_object(id).unwrap();
        assert!(!actor.waiting_for_path);
        assert!(!actor.movement.path.is_empty());
        assert!(
            !actor.adjust_destinations(),
            "C++ waits for the following AI update"
        );
    }
    assert_eq!(
        original.host_object(id).unwrap().movement.path,
        restored.host_object(id).unwrap().movement.path
    );
    let between_frames = builder.create_world_snapshot(&original).unwrap();
    let mut boundary_restore = world(&map);
    builder
        .restore_from_snapshot(&between_frames, &mut boundary_restore)
        .unwrap();
    assert!(!boundary_restore
        .host_object(id)
        .unwrap()
        .adjust_destinations());
    peer.update_with_dt(1.0 / 30.0);
    assert!(peer.host_object(peer_id).unwrap().adjust_destinations());
    for world in [&mut original, &mut restored, &mut boundary_restore] {
        world.update_with_dt(1.0 / 30.0);
        assert!(
            world.host_object(id).unwrap().adjust_destinations(),
            "C++ enables adjustment in the tighten-state update"
        );
    }

    // A new ordinary move replaces the pending tighten state, even though
    // both commands project to the host's Moving state.
    let mut interrupted = world(&map);
    let interrupted_id = interrupted
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    assert!(interrupted.unit_command_tighten_to(interrupted_id, Vec3::new(100.0, 0.0, 150.0)));
    // An ultra-accurate move keeps adjustment disabled; a stale tighten
    // continuation would incorrectly enable it after the replacement.
    interrupted
        .host_object_mut(interrupted_id)
        .unwrap()
        .ultra_accurate = true;
    assert!(interrupted.unit_command_move_to(interrupted_id, Vec3::new(150.0, 0.0, 100.0)));
    let adjustment_after_order = interrupted
        .host_object(interrupted_id)
        .unwrap()
        .adjust_destinations();
    assert!(!adjustment_after_order);
    for _ in 0..2 {
        interrupted.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(
        interrupted
            .host_object(interrupted_id)
            .unwrap()
            .adjust_destinations(),
        adjustment_after_order
    );
}
