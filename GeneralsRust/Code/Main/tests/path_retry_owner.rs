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

fn aircraft_world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    let mut locomotors = LocomotorStore::new_owned();
    locomotors.apply_source("Locomotor RetryFly\n Surfaces = AIR\n Speed = 30\n MinSpeed = 10\n MinTurnSpeed = 10\n Acceleration = 60\n TurnRate = 120\n Appearance = WINGS\nEnd\n").unwrap();
    locomotors.apply_source("Locomotor RetryGround\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world.admit_object_ini_sources([
        ("RetryFly.ini".into(), "Object RetryFlyer\n KindOf = SELECTABLE VEHICLE AIRCRAFT\n Locomotor = SET_NORMAL RetryFly\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into()),
        ("RetryWideFly.ini".into(), "Object RetryWideFlyer\n KindOf = SELECTABLE VEHICLE AIRCRAFT\n Geometry = BOX\n GeometryMajorRadius = 8\n GeometryMinorRadius = 5\n GeometryHeight = 12\n Locomotor = SET_NORMAL RetryFly\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into()),
        ("RetryGround.ini".into(), "Object RetryGroundUnit\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL RetryGround\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into()),
    ]).unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world
}

fn mark_aircraft_moving(world: &mut GameLogic, id: ObjectId) {
    use generals_main::game_logic::{AIState, ObjectAudio, SessionAudio};
    let mut audio = SessionAudio::default();
    let actor = world.host_object_mut(id).unwrap();
    actor.set_ai_state(AIState::Moving, &mut ObjectAudio::queue(&mut audio));
    assert!(actor.host_ai_is_moving());
}

#[test]
fn current_and_destination_layers_survive_snapshot_independently_per_session() {
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("ObjectLayer.map");
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

    let mut source = world(&map);
    let mut peer = world(&map);
    let position = Vec3::new(100.0, 0.0, 100.0);
    let id = source
        .create_object_for_player("RetryWalker", 1, position)
        .unwrap();
    assert_eq!(
        peer.create_object_for_player("RetryWalker", 1, position)
            .unwrap(),
        id
    );
    {
        let actor = source.host_object_mut(id).unwrap();
        actor.pathfind_layer = 3;
        actor.pathfind_goal_layer = 2;
        actor.pathfind_goal_on_ground = false;
    }
    peer.host_object_mut(id).unwrap().pathfind_layer = 4;
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = world(&map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let actor = restored.host_object(id).unwrap();
    assert_eq!(actor.pathfind_layer, 3);
    assert_eq!(actor.pathfind_goal_layer, 2);
    assert!(!actor.pathfind_goal_on_ground);
    assert_eq!(peer.host_object(id).unwrap().pathfind_layer, 4);

    let tag = snapshot
        .lifecycle_tail
        .windows(4)
        .position(|v| v == b"OLYR")
        .unwrap();
    let size = u32::from_le_bytes(
        snapshot.lifecycle_tail[tag + 8..tag + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(size, 5);
    let mut invalid_layer = builder.create_world_snapshot(&source).unwrap();
    invalid_layer.lifecycle_tail[tag + 16] = u8::MAX;
    let mut invalid_receiver = builder.create_world_snapshot(&source).unwrap();
    invalid_receiver.lifecycle_tail[tag + 12..tag + 16].copy_from_slice(&0_u32.to_le_bytes());
    let mut duplicate = builder.create_world_snapshot(&source).unwrap();
    let row = duplicate.lifecycle_tail[tag + 12..tag + 17].to_vec();
    duplicate.lifecycle_tail.splice(tag + 17..tag + 17, row);
    duplicate.lifecycle_tail[tag + 8..tag + 12].copy_from_slice(&10_u32.to_le_bytes());
    restored.set_current_frame(7);
    for invalid in [&invalid_layer, &invalid_receiver, &duplicate] {
        assert!(
            builder
                .restore_from_snapshot(invalid, &mut restored)
                .is_err()
        );
        assert_eq!(restored.get_current_frame(), 7);
        assert_eq!(restored.host_object(id).unwrap().pathfind_layer, 3);
    }
    let mut older = builder.create_world_snapshot(&source).unwrap();
    older.lifecycle_tail.drain(tag..tag + 12 + size);
    let mut older_restored = world(&map);
    builder
        .restore_from_snapshot(&older, &mut older_restored)
        .unwrap();
    assert_eq!(older_restored.host_object(id).unwrap().pathfind_layer, 1);
    assert_eq!(
        older_restored.host_object(id).unwrap().pathfind_goal_layer,
        2
    );
}

#[test]
fn loading_moving_wide_aircraft_recomputes_goal_from_saved_cell_center() {
    use generals_main::game_logic::pathfinding::{GridPos, PathfindingGrid};

    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("WideAircraft.map");
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

    let mut source = aircraft_world(&map);
    let mut peer = aircraft_world(&map);
    let position = Vec3::new(100.0, 0.0, 100.0);
    let id = source
        .create_object_for_player("RetryWideFlyer", 1, position)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("RetryWideFlyer", 1, position)
        .unwrap();
    assert_eq!(id, peer_id);
    let (radius, centered) =
        PathfindingGrid::radius_and_center(source.host_object(id).unwrap().selection_radius, 10.0);
    assert_eq!((radius, centered), (1, false));
    source.host_object_mut(id).unwrap().is_final_goal = true;
    assert!(source.assign_unit_path(id, Vec3::new(150.0, 0.0, 150.0), &[]));
    for _ in 0..2 {
        source.update_with_dt(1.0 / 30.0);
    }
    mark_aircraft_moving(&mut source, id);
    let saved = source.host_object(id).unwrap().pathfind_goal_cell;
    assert!(saved.0 >= 0 && saved.1 >= 0);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = aircraft_world(&map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(restored.host_object(id).unwrap().host_ai_is_moving());
    let expected = (saved.0 + 1, saved.1 + 1);
    assert_eq!(
        restored.host_object(id).unwrap().pathfind_goal_cell,
        expected
    );
    assert_eq!(
        restored.pathfinding_aircraft_goal(GridPos::new(expected.0, expected.1)),
        Some(id)
    );
    assert_eq!(
        restored.pathfinding_aircraft_goal(GridPos::new(saved.0 - 1, saved.1 - 1)),
        None
    );
    assert_eq!(source.host_object(id).unwrap().pathfind_goal_cell, saved);
    assert_eq!(
        peer.pathfinding_aircraft_goal(GridPos::new(expected.0, expected.1)),
        None
    );
}

#[test]
fn loading_idle_aircraft_registers_its_current_position_in_its_own_grid() {
    use generals_main::game_logic::pathfinding::GridPos;

    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("IdleAircraft.map");
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

    let mut source = aircraft_world(&map);
    let mut peer = aircraft_world(&map);
    let position = Vec3::new(100.0, 0.0, 100.0);
    let id = source
        .create_object_for_player("RetryFlyer", 1, position)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("RetryFlyer", 1, position)
        .unwrap();
    assert_eq!(id, peer_id);
    let actor = source.host_object(id).unwrap();
    assert!(actor.has_ai_update_interface());
    assert!(!actor.host_ai_is_moving());
    assert!(generals_main::game_logic::pathfinding::PathfindingGrid::is_aircraft_that_adjusts_destination(actor));
    assert_eq!(actor.pathfind_goal_cell, (-1, -1));
    let cell = GridPos::new(10, 10);
    assert_eq!(peer.pathfinding_aircraft_goal(cell), None);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = aircraft_world(&map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(id).unwrap().pathfind_goal_cell,
        (10, 10)
    );
    assert_eq!(restored.pathfinding_aircraft_goal(cell), Some(id));
    assert_eq!(peer.pathfinding_aircraft_goal(cell), None);
}

#[test]
fn aircraft_goal_overwrite_survives_rebuild_without_reviving_previous_owner() {
    use generals_main::game_logic::pathfinding::GridPos;
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("GoalOrder.map");
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

    let mut owner = aircraft_world(&map);
    let mut peer = aircraft_world(&map);
    let mut ids = Vec::new();
    for world in [&mut owner, &mut peer] {
        let low = world
            .create_object_for_player("RetryFlyer", 1, Vec3::new(100.0, 0.0, 100.0))
            .unwrap();
        let high = world
            .create_object_for_player("RetryFlyer", 1, Vec3::new(110.0, 0.0, 100.0))
            .unwrap();
        let ground_a = world
            .create_object_for_player("RetryGroundUnit", 1, Vec3::new(250.0, 0.0, 250.0))
            .unwrap();
        let ground_b = world
            .create_object_for_player("RetryGroundUnit", 1, Vec3::new(260.0, 0.0, 250.0))
            .unwrap();
        ids.push((low, high, ground_a, ground_b));
    }
    assert_eq!(ids[0], ids[1]);
    let (low, high, ground_a, ground_b) = ids[0];
    let goal = Vec3::new(150.0, 0.0, 150.0);
    for (world, order) in [(&mut owner, [high, low]), (&mut peer, [low, high])] {
        for id in order {
            world.host_object_mut(id).unwrap().is_final_goal = true;
            assert!(world.assign_unit_path(id, goal, &[]));
            for _ in 0..2 {
                world.update_with_dt(1.0 / 30.0);
            }
        }
    }
    let (x, y) = owner.host_object(low).unwrap().pathfind_goal_cell;
    assert_eq!(owner.host_object(high).unwrap().pathfind_goal_cell, (x, y));
    let cell = GridPos::new(x, y);
    assert_eq!(owner.pathfinding_aircraft_goal(cell), Some(low));
    assert_eq!(peer.pathfinding_aircraft_goal(cell), Some(high));
    for world in [&mut owner, &mut peer] {
        mark_aircraft_moving(world, low);
        mark_aircraft_moving(world, high);
    }

    // C++ xfers objects in their saved list order and schedules each AI
    // loadPostProcess immediately after reading that object's modules.
    // Loading prepends the object list, so post-process order is its reverse.
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let mut restored = aircraft_world(&map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(restored.host_object(low).unwrap().host_ai_is_moving());
    assert_eq!(restored.pathfinding_aircraft_goal(cell), Some(low));
    let peer_snapshot = builder.create_world_snapshot(&peer).unwrap();
    let mut restored_peer = aircraft_world(&map);
    builder
        .restore_from_snapshot(&peer_snapshot, &mut restored_peer)
        .unwrap();
    assert_eq!(restored_peer.pathfinding_aircraft_goal(cell), Some(low));
    assert_eq!(peer.pathfinding_aircraft_goal(cell), Some(high));

    assert!(owner.assign_unit_path(ground_a, Vec3::new(300.0, 0.0, 250.0), &[]));
    for _ in 0..3 {
        owner.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(
        owner.pathfinding_aircraft_goal(cell),
        Some(low),
        "rebuild changed registration order"
    );
    assert!(owner.unit_command_tighten_to(low, Vec3::new(190.0, 0.0, 190.0)));
    assert_eq!(owner.pathfinding_aircraft_goal(cell), None);
    assert!(owner.assign_unit_path(ground_b, Vec3::new(300.0, 0.0, 260.0), &[]));
    for _ in 0..3 {
        owner.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(
        owner.pathfinding_aircraft_goal(cell),
        None,
        "overwritten owner returned"
    );
    assert_eq!(peer.pathfinding_aircraft_goal(cell), Some(high));
    peer.destroy_object(high).unwrap();
    for _ in 0..2 {
        peer.update_with_dt(1.0 / 30.0);
    }
    assert!(peer.host_object(high).is_none());
    assert_eq!(peer.pathfinding_aircraft_goal(cell), None);
    peer.reset();
    assert_eq!(peer.pathfinding_aircraft_goal(cell), None);
    assert_eq!(owner.pathfinding_aircraft_goal(cell), None);
}

#[test]
fn removed_ground_goal_does_not_return_when_occupancy_is_rebuilt() {
    use generals_main::game_logic::pathfinding::{GridPos, PathfindingGrid, PathfindingGround};
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("GroundGoal.map");
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

    let mut owner = world(&map);
    let mut peer = world(&map);
    let start = Vec3::new(100.0, 0.0, 100.0);
    let goal = Vec3::new(150.0, 0.0, 100.0);
    let id = owner
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    assert_eq!(id, peer_id);
    for world in [&mut owner, &mut peer] {
        let unit = world.host_object_mut(id).unwrap();
        unit.selection_radius = 2.0;
        unit.pathfind_goal_cell = (15, 10);
        unit.movement.path = vec![start, goal];
        unit.movement.target_position = Some(goal);
    }
    let ground = PathfindingGround::Flat(0.0);
    let mut grid = PathfindingGrid::new(410.0, 410.0, 10.0);
    let cell = GridPos::new(15, 10);
    grid.update_dynamic_obstacles(owner.host_objects(), &ground);
    assert_eq!(grid.ground_goal_unit(cell), id.0);
    assert!(owner.unit_command_tighten_to(id, Vec3::new(150.0, 0.0, 150.0)));
    assert_eq!(owner.host_object(id).unwrap().pathfind_goal_cell, (-1, -1));
    assert_eq!(
        owner.host_object(id).unwrap().movement.path.last(),
        Some(&goal)
    );
    grid.update_dynamic_obstacles(owner.host_objects(), &ground);
    assert_eq!(
        grid.ground_goal_unit(cell),
        0,
        "old path recreated a removed goal"
    );
    grid.update_dynamic_obstacles(peer.host_objects(), &ground);
    assert_eq!(
        grid.ground_goal_unit(cell),
        id.0,
        "other session lost its goal"
    );

    let mut layered = world(&map);
    let layered_id = layered
        .create_object_for_player("RetryWalker", 1, start)
        .unwrap();
    let actor = layered.host_object_mut(layered_id).unwrap();
    actor.pathfind_goal_cell = (15, 10);
    actor.pathfind_goal_layer = 2;
    actor.pathfind_goal_on_ground = false;
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&layered).unwrap();
    let mut restored = world(&map);
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    let actor = restored.host_object(layered_id).unwrap();
    assert_eq!(actor.pathfind_goal_cell, (15, 10));
    assert_eq!(actor.pathfind_goal_layer, 2);
    assert!(!actor.pathfind_goal_on_ground);

    let mut malformed = builder.create_world_snapshot(&layered).unwrap();
    let tag = malformed
        .lifecycle_tail
        .windows(4)
        .position(|v| v == b"TMAI")
        .unwrap();
    let size = u32::from_le_bytes(
        malformed.lifecycle_tail[tag + 8..tag + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    let end = tag + 12 + size;
    assert_eq!(
        &malformed.lifecycle_tail[end - 14..end - 6],
        &1_u64.to_le_bytes()
    );
    malformed.lifecycle_tail[end - 2] = 0;
    restored.set_current_frame(7);
    assert!(
        builder
            .restore_from_snapshot(&malformed, &mut restored)
            .is_err()
    );
    assert_eq!(restored.get_current_frame(), 7);
    assert!(layered.unit_command_tighten_to(layered_id, Vec3::new(160.0, 0.0, 160.0)));
    let actor = layered.host_object(layered_id).unwrap();
    assert_eq!(actor.pathfind_goal_cell, (-1, -1));
    assert_eq!(
        actor.pathfind_goal_layer, 2,
        "C++ retains destination layer after removeGoal"
    );
    let removed = builder.create_world_snapshot(&layered).unwrap();
    let mut removed_restored = world(&map);
    builder
        .restore_from_snapshot(&removed, &mut removed_restored)
        .unwrap();
    let actor = removed_restored.host_object(layered_id).unwrap();
    assert_eq!(actor.pathfind_goal_cell, (-1, -1));
    assert_eq!(actor.pathfind_goal_layer, 2);
}

#[test]
fn aircraft_registered_goal_is_owned_by_its_session_and_survives_save() {
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("AircraftGoal.map");
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

    let mut owner = aircraft_world(&map);
    let mut peer = aircraft_world(&map);
    let start = Vec3::new(100.0, 0.0, 100.0);
    let id = owner
        .create_object_for_player("RetryFlyer", 1, start)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("RetryFlyer", 1, start)
        .unwrap();
    assert_eq!(id, peer_id);
    for world in [&mut owner, &mut peer] {
        let unit = world.host_object_mut(id).unwrap();
        unit.is_final_goal = true;
    }
    assert!(owner.assign_unit_path(id, Vec3::new(100.0, 0.0, 150.0), &[]));
    assert!(peer.assign_unit_path(id, Vec3::new(150.0, 0.0, 100.0), &[]));
    let builder = SnapshotBuilder::new();
    let pending = builder.create_world_snapshot(&owner).unwrap();
    let tag = pending
        .lifecycle_tail
        .windows(4)
        .position(|v| v == b"TMAI")
        .unwrap();
    let size = u32::from_le_bytes(
        pending.lifecycle_tail[tag + 8..tag + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    let payload_end = tag + 12 + size;
    assert_eq!(
        &pending.lifecycle_tail[payload_end - 8..payload_end],
        &[0; 8]
    );
    let end = payload_end - 8;
    assert_eq!(
        &pending.lifecycle_tail[end - 12..end - 4],
        &1_u64.to_le_bytes(),
        "quick path must remain pending until the move update"
    );
    assert_eq!(&pending.lifecycle_tail[end - 4..end], &id.0.to_le_bytes());
    let mut invalid = builder.create_world_snapshot(&owner).unwrap();
    invalid.lifecycle_tail[end - 12..end - 4].copy_from_slice(&2_u64.to_le_bytes());
    invalid.lifecycle_tail.splice(end..end, id.0.to_le_bytes());
    invalid.lifecycle_tail[tag + 8..tag + 12].copy_from_slice(&((size + 4) as u32).to_le_bytes());
    let mut rejected = aircraft_world(&map);
    rejected.set_current_frame(7);
    assert!(
        builder
            .restore_from_snapshot(&invalid, &mut rejected)
            .is_err()
    );
    assert_eq!(rejected.get_current_frame(), 7);
    let mut pending_restored = aircraft_world(&map);
    builder
        .restore_from_snapshot(&pending, &mut pending_restored)
        .unwrap();
    let mut v17 = builder.create_world_snapshot(&owner).unwrap();
    v17.lifecycle_tail.drain(payload_end - 8..payload_end);
    v17.lifecycle_tail[tag + 4..tag + 8].copy_from_slice(&17_u32.to_le_bytes());
    v17.lifecycle_tail[tag + 8..tag + 12].copy_from_slice(&((size - 8) as u32).to_le_bytes());
    let mut v17_restored = aircraft_world(&map);
    builder
        .restore_from_snapshot(&v17, &mut v17_restored)
        .unwrap();
    for _ in 0..2 {
        owner.update_with_dt(1.0 / 30.0);
        peer.update_with_dt(1.0 / 30.0);
        pending_restored.update_with_dt(1.0 / 30.0);
        v17_restored.update_with_dt(1.0 / 30.0);
    }
    let goal = owner.host_object(id).unwrap().pathfind_goal_cell;
    let peer_goal = peer.host_object(id).unwrap().pathfind_goal_cell;
    let actor = owner.host_object(id).unwrap();
    assert_ne!(
        goal,
        (-1, -1),
        "aircraft goal: disabled={} waiting={} path={:?} loco={:?} appearance={:?} surfaces={} final={} adjusts={}",
        actor.is_disabled(),
        actor.waiting_for_path,
        actor.movement.path,
        actor.cur_locomotor_name,
        actor.loco_appearance,
        actor.locomotor_surfaces,
        actor.is_final_goal,
        actor.adjust_destinations()
    );
    assert_ne!(goal, peer_goal);
    assert_eq!(
        pending_restored.host_object(id).unwrap().pathfind_goal_cell,
        goal
    );
    assert_eq!(
        v17_restored.host_object(id).unwrap().pathfind_goal_cell,
        goal
    );
    let goal_pos = generals_main::game_logic::pathfinding::GridPos::new(goal.0, goal.1);
    assert_eq!(owner.pathfinding_aircraft_goal(goal_pos), Some(id));

    mark_aircraft_moving(&mut owner, id);
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let mut restored = aircraft_world(&map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(restored.host_object(id).unwrap().host_ai_is_moving());
    assert_eq!(restored.host_object(id).unwrap().pathfind_goal_cell, goal);
    assert_eq!(restored.pathfinding_aircraft_goal(goal_pos), Some(id));
    assert!(owner.unit_command_tighten_to(id, Vec3::new(150.0, 0.0, 150.0)));
    assert_eq!(owner.host_object(id).unwrap().pathfind_goal_cell, (-1, -1));
    assert_ne!(
        owner.pathfinding_aircraft_goal(goal_pos),
        Some(id),
        "rebuild resurrected the removed claim"
    );
    assert_eq!(peer.host_object(id).unwrap().pathfind_goal_cell, peer_goal);
    assert_eq!(restored.host_object(id).unwrap().pathfind_goal_cell, goal);

    let mut cancelled = aircraft_world(&map);
    let cancelled_id = cancelled
        .create_object_for_player("RetryFlyer", 1, start)
        .unwrap();
    cancelled
        .host_object_mut(cancelled_id)
        .unwrap()
        .is_final_goal = true;
    assert!(cancelled.assign_unit_path(cancelled_id, Vec3::new(100.0, 0.0, 150.0), &[]));
    assert!(cancelled.unit_command_stop(cancelled_id).unwrap());
    for _ in 0..2 {
        cancelled.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(
        cancelled
            .host_object(cancelled_id)
            .unwrap()
            .pathfind_goal_cell,
        (-1, -1)
    );
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
    assert_eq!(&legacy.lifecycle_tail[end - 32..end], &[0; 32]);
    legacy.lifecycle_tail.drain(end - 32..end);
    legacy.lifecycle_tail[length..length + 4].copy_from_slice(&((size - 32) as u32).to_le_bytes());
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
    let goal_count = original
        .host_objects()
        .registered_values()
        .filter(|object| object.pathfind_goal_cell != (-1, -1))
        .count();
    assert_eq!(&malformed.lifecycle_tail[end - 16..end], &[0; 16]);
    let final_goals_end = end - 16 - (8 + 12 * goal_count);
    assert_eq!(
        &malformed.lifecycle_tail[final_goals_end - 12..final_goals_end - 4],
        &1_u64.to_le_bytes()
    );
    assert_eq!(
        &malformed.lifecycle_tail[final_goals_end - 4..final_goals_end],
        &id.0.to_le_bytes()
    );
    malformed.lifecycle_tail[final_goals_end - 12..final_goals_end - 4]
        .copy_from_slice(&2_u64.to_le_bytes());
    malformed
        .lifecycle_tail
        .splice(final_goals_end..final_goals_end, id.0.to_le_bytes());
    malformed.lifecycle_tail[length..length + 4]
        .copy_from_slice(&((size + 4) as u32).to_le_bytes());
    peer.set_current_frame(7);
    assert!(
        builder
            .restore_from_snapshot(&malformed, &mut peer)
            .is_err()
    );
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
    assert!(
        !boundary_restore
            .host_object(id)
            .unwrap()
            .adjust_destinations()
    );
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
