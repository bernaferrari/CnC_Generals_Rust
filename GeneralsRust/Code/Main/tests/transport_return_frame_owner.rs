use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::host_transport_return::TransportReturnState;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn map_file(root: &std::path::Path) -> std::path::PathBuf {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let path = root.join("TransportReturnOwner.map");
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    path
}

fn world(
    path: &std::path::Path,
    frame_steps: usize,
) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source(
            "Locomotor ReturnOwnerLoco\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 120\n Appearance = FOUR_WHEELS\nEnd\n",
        )
        .unwrap();
    let mut logic = GameLogic::new();
    logic.admit_locomotor_definitions(locomotors).unwrap();
    logic
        .admit_object_ini_sources([(
            "TransportReturnOwner.ini".into(),
            "Object ReturnOwnerCarrier\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL ReturnOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = TransportContain ContainTag\n  ContainMax = 2\n  Slots = 2\n  AllowInsideKindOf = INFANTRY\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    logic.start_new_game(GameMode::SinglePlayer);
    assert!(logic.load_map(path.to_str().unwrap()));
    logic.add_player(Player::new(1, Team::USA, "Owner", true));
    for _ in 0..frame_steps {
        logic.update_with_dt(1.0 / 30.0);
    }
    let id = logic
        .create_object_for_player("ReturnOwnerCarrier", 1, glam::Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    (logic, id)
}

#[test]
fn transport_return_path_admission_preserves_each_receiving_frame() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(owner_root.path());
    let peer_map = map_file(peer_root.path());
    let (mut owner, owner_id) = world(&owner_map, 7);
    let (mut peer, peer_id) = world(&peer_map, 19);
    assert_eq!(owner_id, peer_id);
    let owner_frame = owner.get_frame();
    let peer_frame = peer.get_frame();
    assert_ne!(owner_frame, peer_frame);

    for (logic, id) in [(&mut owner, owner_id), (&mut peer, peer_id)] {
        let current = logic.host_object(id).unwrap().get_position();
        let origin = current - glam::Vec3::new(100.0, 0.0, 0.0);
        assert!(logic.unit_command_set_pending_evacuate(id, true, true, true));
        // Model the already-arrived parent state; the public unload operation
        // then runs the actual receiving return continuation.
        logic.host_object_mut(id).unwrap().transport_return_state =
            Some(TransportReturnState::MoveAndEvacuate { origin });
        let _ = logic.evacuate_container_now(id, true);
    }

    for (logic, id, frame) in [
        (&owner, owner_id, owner_frame),
        (&peer, peer_id, peer_frame),
    ] {
        let object = logic.host_object(id).unwrap();
        assert!(object.waiting_for_path || !object.movement.path.is_empty());
        assert_eq!(
            object.ai_state(),
            generals_main::game_logic::AIState::Moving
        );
        assert_eq!(object.donut_timer, frame.wrapping_add(75));
    }
}
