use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn empty_map(path: &std::path::Path) {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    std::fs::write(path, map.into_ckmp_bytes()).unwrap();
}

fn world(
    path: &std::path::Path,
) -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "MineClearOwner.ini".into(),
            "Object USA_Dozer\n KindOf = SELECTABLE VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject OwnerLandMine\n KindOf = SELECTABLE VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(path.to_str().unwrap()));
    world.add_player(Player::new(1, Team::China, "Dozer", true));
    world.add_player(Player::new(2, Team::USA, "Mine", true));
    let dozer = world
        .create_object_for_player("USA_Dozer", 1, glam::Vec3::new(80.0, 0.0, 80.0))
        .unwrap();
    let mine = world
        .create_object_for_player("OwnerLandMine", 2, glam::Vec3::new(110.0, 0.0, 80.0))
        .unwrap();
    (world, dozer, mine)
}

#[test]
fn mine_clear_approach_stamps_each_receiving_world_frame() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = owner_root.path().join("MineOwner.map");
    let peer_map = peer_root.path().join("MinePeer.map");
    empty_map(&owner_map);
    empty_map(&peer_map);

    let (mut owner, owner_dozer, owner_mine) = world(&owner_map);
    let (mut peer, peer_dozer, peer_mine) = world(&peer_map);
    assert_eq!(owner_dozer, peer_dozer);
    assert_eq!(owner_mine, peer_mine);
    for _ in 0..7 {
        owner.update_with_dt(1.0 / 30.0);
    }
    for _ in 0..19 {
        peer.update_with_dt(1.0 / 30.0);
    }
    let owner_frame = owner.get_frame();
    let peer_frame = peer.get_frame();
    assert_ne!(owner_frame, peer_frame);

    // Admit the mine-clear interaction only after establishing each world's
    // frame, so earlier simulation steps cannot start or finish the approach.
    for (logic, dozer, mine, frame) in [
        (&mut owner, owner_dozer, owner_mine, owner_frame),
        (&mut peer, peer_dozer, peer_mine, peer_frame),
    ] {
        logic.host_object_mut(mine).unwrap().mine_data =
            Some(generals_main::game_logic::host_mines::HostMineData::land_mine());
        logic
            .host_object_mut(dozer)
            .unwrap()
            .set_weapon_set_mine_clearing_detail_at_frame(true, frame);
    }

    owner.update_mines_and_demo_traps();
    peer.update_mines_and_demo_traps();

    for (logic, dozer, mine, frame) in [
        (&owner, owner_dozer, owner_mine, owner_frame),
        (&peer, peer_dozer, peer_mine, peer_frame),
    ] {
        let object = logic.host_object(dozer).unwrap();
        assert_eq!(
            object.ai_state(),
            generals_main::game_logic::AIState::Moving
        );
        assert_eq!(object.donut_timer, frame.wrapping_add(75));
        assert_eq!(
            object.movement.target_position,
            Some(logic.host_object(mine).unwrap().get_position())
        );
    }
}
