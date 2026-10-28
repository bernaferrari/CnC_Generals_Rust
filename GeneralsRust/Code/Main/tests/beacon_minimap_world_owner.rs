use game_engine::common::system::DataChunkOutput;
use generals_main::command_executor::CommandExecutor;
use generals_main::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use std::time::SystemTime;

fn minimal_map(root: &std::path::Path) -> std::path::PathBuf {
    let mut output = DataChunkOutput::new();
    output.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        output.write_int(value);
    }
    for _ in 0..1681 {
        output.write_byte(0);
    }
    output.close_data_chunk();
    output.open_data_chunk("ObjectsList", 3);
    output.close_data_chunk();
    let path = root.join("BeaconMinimapOwner.map");
    std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "BeaconMinimapOwner.ini".into(),
            "Object AmericaBeacon\nKindOf = SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 1\nInitialHealth = 1\nEnd\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Local", true).with_script_identity("Local"));
    world
}

fn place(world: &mut GameLogic, position: glam::Vec3) -> ObjectId {
    let command = GameCommand {
        command_type: CommandType::PlaceBeacon {
            location: position,
            text: String::new(),
        },
        player_id: 1,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        CommandExecutor::new(world, 1)
            .execute_command(command)
            .unwrap(),
        CommandResult::Success
    );
    *world.host_objects().registered_keys().next().unwrap()
}

fn remove(world: &mut GameLogic, id: ObjectId) {
    let command = GameCommand {
        command_type: CommandType::RemoveBeacon,
        player_id: 1,
        command_id: 2,
        timestamp: SystemTime::now(),
        selected_units: vec![id],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        CommandExecutor::new(world, 1)
            .execute_command(command)
            .unwrap(),
        CommandResult::Success
    );
}

#[test]
fn minimap_beacons_use_receiving_objects_with_same_ids_and_conflicting_global_markers() {
    let first_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let first_map = minimal_map(first_root.path());
    let peer_map = minimal_map(peer_root.path());
    let mut first = world(&first_map);
    let mut peer = world(&peer_map);

    let first_position = glam::Vec3::new(-500.0, 0.0, 300.0);
    let first_nearby_position = first_position + glam::Vec3::new(1.0, 0.0, 0.0);
    let peer_position = glam::Vec3::new(700.0, 0.0, -400.0);
    let first_id = place(&mut first, first_position);
    let first_nearby_id = place(&mut first, first_nearby_position);
    let peer_id = place(&mut peer, peer_position);
    assert_eq!(
        first_id, peer_id,
        "worlds deliberately reuse the beacon ObjectId"
    );

    // Seed a marker owned by the Core singleton without a matching receiving
    // Object. Neither hosted map may inherit it.
    {
        let mut global = gamelogic::system::beacon_manager::get_beacon_manager()
            .lock()
            .unwrap();
        global.place_beacon(0, gamelogic::common::Coord3D::new(1400.0, 0.0, 1400.0), 99);
    }

    let bounds = first.world_bounds();
    assert_eq!(bounds, peer.world_bounds());
    let first_position = first.host_object(first_id).unwrap().get_position();
    let peer_position = peer.host_object(peer_id).unwrap().get_position();
    assert_ne!(first_position, peer_position);
    let expected = |position: glam::Vec3| {
        (
            ((position.x - bounds.0.x) / (bounds.1.x - bounds.0.x).max(1.0)).clamp(0.0, 1.0),
            ((position.z - bounds.0.z) / (bounds.1.z - bounds.0.z).max(1.0)).clamp(0.0, 1.0),
        )
    };
    let first_expected = expected(first_position);
    let first_nearby_actual = first.host_object(first_nearby_id).unwrap().get_position();
    let first_nearby_expected = expected(first_nearby_actual);
    let peer_expected = expected(peer_position);
    let first_state = first.update_ui_state(1);
    let peer_state = peer.update_ui_state(1);

    assert_eq!(first_state.minimap_beacons.len(), 2);
    assert_eq!(peer_state.minimap_beacons.len(), 1);
    let first_dots: Vec<_> = first_state
        .minimap_beacons
        .iter()
        .map(|dot| dot.position)
        .collect();
    let peer_dot = peer_state.minimap_beacons[0].position;
    assert!(first_dots.iter().any(|dot| {
        (dot.x - first_expected.0).abs() < 0.001 && (dot.y - first_expected.1).abs() < 0.001
    }));
    assert!(first_dots.iter().any(|dot| {
        (dot.x - first_nearby_expected.0).abs() < 0.001
            && (dot.y - first_nearby_expected.1).abs() < 0.001
    }));
    assert!((peer_dot.x - peer_expected.0).abs() < 0.001);
    assert!((peer_dot.y - peer_expected.1).abs() < 0.001);

    // C++ removes the selected live Object; it does not collapse nearby
    // placements into one BeaconManager location.
    remove(&mut first, first_id);
    let first_after_remove = first.update_ui_state(1);
    let peer_after_remove = peer.update_ui_state(1);
    assert_eq!(first_after_remove.minimap_beacons.len(), 1);
    assert_eq!(peer_after_remove.minimap_beacons.len(), 1);
    assert_eq!(
        first.host_object(first_nearby_id).unwrap().get_position(),
        first_nearby_actual
    );
    assert!(peer.host_object(peer_id).is_some());
}
