use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};

fn world(health_percent: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_combat_bonus_ini_sources([format!(
            "GameData\n HumanSoloPlayerHealthBonus_Normal = {health_percent}%\nEnd\n"
        )])
        .unwrap();
    world
        .admit_object_ini_sources([(
            "Object.ini".into(),
            "Object BonusUnit\n KindOf = INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(42, Team::USA, "BonusOwner", true));
    world
}

fn spawn(world: &mut GameLogic) -> ObjectId {
    world
        .create_object_for_player("BonusUnit", 42, glam::Vec3::ZERO)
        .unwrap()
}

fn veterancy_world(veteran_percent: u32, elite_percent: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_combat_bonus_ini_sources([format!(
            "GameData\n HealthBonus_Veteran = {veteran_percent}%\n HealthBonus_Elite = {elite_percent}%\nEnd\n"
        )])
        .unwrap();
    world
        .admit_object_ini_sources([(
            "Veterancy.ini".into(),
            "Object BonusVeteran\n KindOf = INFANTRY\n IsTrainable = Yes\n ExperienceRequired = 0 60 150 300\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
             Object BonusBornVeteran\n KindOf = INFANTRY\n IsTrainable = Yes\n ExperienceRequired = 0 60 150 300\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = VeterancyGainCreate Rank\n StartingLevel = VETERAN\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(42, Team::USA, "BonusOwner", true));
    world
}

#[test]
fn veterancy_health_uses_receiving_game_data_at_creation_and_after_load() {
    let mut first = veterancy_world(130, 160);
    let mut peer = veterancy_world(175, 220);
    let first_id = first
        .create_object_for_player("BonusVeteran", 42, glam::Vec3::ZERO)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("BonusVeteran", 42, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(first_id, peer_id);
    first.award_experience(first_id, 60.0);
    assert!((first.host_object(first_id).unwrap().health.maximum - 130.0).abs() < 0.001);
    assert_eq!(peer.host_object(peer_id).unwrap().health.maximum, 100.0);

    let born_first = first
        .create_object_for_player("BonusBornVeteran", 42, glam::Vec3::X)
        .unwrap();
    let born_peer = peer
        .create_object_for_player("BonusBornVeteran", 42, glam::Vec3::X)
        .unwrap();
    assert_eq!(born_first, born_peer);
    assert!((first.host_object(born_first).unwrap().health.maximum - 130.0).abs() < 0.001);
    assert!((peer.host_object(born_peer).unwrap().health.maximum - 175.0).abs() < 0.001);

    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut restored = veterancy_world(130, 160);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    peer.award_experience(peer_id, 60.0);
    assert!((peer.host_object(peer_id).unwrap().health.maximum - 175.0).abs() < 0.001);
    peer.reset();
    first.award_experience(first_id, 90.0);
    restored.award_experience(first_id, 90.0);
    assert!((first.host_object(first_id).unwrap().health.maximum - 160.0).abs() < 0.001);
    assert!((restored.host_object(first_id).unwrap().health.maximum - 160.0).abs() < 0.001);

    let mut crate_world = veterancy_world(145, 180);
    let picker = crate_world
        .create_object_for_player("BonusVeteran", 42, glam::Vec3::ZERO)
        .unwrap();
    let crate_id = crate_world
        .create_object_for_player("BonusVeteran", 42, glam::Vec3::X)
        .unwrap();
    crate_world.host_object_mut(crate_id).unwrap().target = Some(picker);
    assert_eq!(
        crate_world.execute_veterancy_crate_behavior(crate_id, picker, 0.0, 1),
        1
    );
    assert!((crate_world.host_object(picker).unwrap().health.maximum - 145.0).abs() < 0.001);

    // Explicit 100% is a real authored value, not a missing-INI sentinel.
    let mut unity = veterancy_world(100, 100);
    let id = unity
        .create_object_for_player("BonusVeteran", 42, glam::Vec3::ZERO)
        .unwrap();
    unity.award_experience(id, 60.0);
    assert_eq!(unity.host_object(id).unwrap().health.maximum, 100.0);
}

#[test]
fn construction_and_reset_keep_each_worlds_game_data() {
    let mut first = world(125);
    let mut peer = world(175);
    let first_id = spawn(&mut first);
    let peer_id = spawn(&mut peer);
    assert_eq!(first_id, peer_id);
    assert_eq!(first.host_object(first_id).unwrap().health.maximum, 125.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.maximum, 175.0);

    use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    saves.init().unwrap();
    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "bonus".into(),
        display_name: "Bonus continuation".into(),
        description: String::new(),
        map_name: "BonusFixture".into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    saves.save_game("bonus", &first, &info).unwrap();
    let mut restored = world(125);
    saves.load_game("bonus", &mut restored).unwrap();
    let restored_next = spawn(&mut restored);
    assert_eq!(
        restored.host_object(restored_next).unwrap().health.maximum,
        125.0
    );

    peer.start_new_game(GameMode::SinglePlayer);
    peer.add_player(Player::new(42, Team::USA, "BonusOwner", true));
    let peer_again = spawn(&mut peer);
    let first_again = spawn(&mut first);
    assert_eq!(peer.host_object(peer_again).unwrap().health.maximum, 175.0);
    assert_eq!(
        first.host_object(first_again).unwrap().health.maximum,
        125.0
    );
    map_overrides_remain_with_their_receiving_worlds();
}

fn map_overrides_remain_with_their_receiving_worlds() {
    use game_engine::common::system::DataChunkOutput;

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
    let bytes = map.into_ckmp_bytes();
    let root = tempfile::tempdir().unwrap();
    let make_map = |name: &str, percent: u32, guard_msec: u32| {
        let directory = root.path().join(name);
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("Bonus.map");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::write(
            directory.join("Map.ini"),
            format!("GameData\n HumanSoloPlayerHealthBonus_Normal = {percent}%\nEnd\nAIData\n GuardEnemyScanRate = {guard_msec}\nEnd\n"),
        )
        .unwrap();
        path
    };
    let first_map = make_map("first", 150, 1000);
    let peer_map = make_map("peer", 200, 2000);
    std::fs::write(
        first_map.parent().unwrap().join("Solo.ini"),
        "AIData\n GuardEnemyScanRate = 3500\nEnd\n",
    )
    .unwrap();
    let mut first = world(125);
    let mut peer = world(175);
    assert!(first.load_map(first_map.to_str().unwrap()));
    assert!(peer.load_map(peer_map.to_str().unwrap()));
    assert_eq!(first.host_guard_enemy_scan_rate(), 105);
    assert_eq!(peer.host_guard_enemy_scan_rate(), 60);
    first.add_player(Player::new(42, Team::USA, "BonusOwner", true));
    peer.add_player(Player::new(42, Team::USA, "BonusOwner", true));
    let first_id = spawn(&mut first);
    let peer_id = spawn(&mut peer);
    assert_eq!(first.host_object(first_id).unwrap().health.maximum, 150.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.maximum, 200.0);
    peer.reset();
    assert_eq!(first.host_guard_enemy_scan_rate(), 105);
    assert_eq!(peer.host_guard_enemy_scan_rate(), 15);
    let next = spawn(&mut first);
    assert_eq!(first.host_object(next).unwrap().health.maximum, 150.0);
    first.reset();
    assert_eq!(first.host_guard_enemy_scan_rate(), 15);
    first.add_player(Player::new(42, Team::USA, "BonusOwner", true));
    let base = spawn(&mut first);
    assert_eq!(first.host_object(base).unwrap().health.maximum, 125.0);
}

#[test]
fn authored_unity_health_bonus_remains_unity_on_easy() {
    let mut world = world(100);
    world.add_ai_opponent(77, Team::GLA, generals_main::ai::AIDifficulty::Easy);
    assert_eq!(
        world.get_difficulty(),
        generals_main::ai::AIDifficulty::Easy
    );
    let id = spawn(&mut world);
    assert_eq!(world.host_object(id).unwrap().health.maximum, 100.0);
}
