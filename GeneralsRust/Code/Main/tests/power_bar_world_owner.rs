use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode};

#[test]
fn map_power_rules_belong_to_the_receiving_world_through_reset() {
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
    let write_map = |name: &str, base: i32, intervals: i32| {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("Owner.map");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::write(
            dir.join("Map.ini"),
            format!("GameData\nPowerBarBase = {base}\nPowerBarIntervals = {intervals}\nEnd\n"),
        )
        .unwrap();
        path
    };
    let first_map = write_map("first", 11, 2);
    let peer_map = write_map("peer", 31, 6);
    let empty_map = write_map("empty", 99, 9);
    std::fs::write(empty_map.parent().unwrap().join("Map.ini"), "").unwrap();
    std::fs::write(
        first_map.parent().unwrap().join("Solo.ini"),
        "GameData\nPowerBarYellowRange = 9\nEnd\n",
    )
    .unwrap();

    let make_world = || {
        let mut world = GameLogic::new();
        world
            .admit_object_ini_sources([("Empty.ini".into(), String::new())])
            .unwrap();
        world.start_new_game(GameMode::SinglePlayer);
        world
    };
    let mut first = make_world();
    let mut peer = make_world();
    let baseline = first.power_bar_rules();
    assert!(first.load_map(first_map.to_str().unwrap()));
    assert!(peer.load_map(peer_map.to_str().unwrap()));
    assert_eq!(
        (
            first.power_bar_rules().base,
            first.power_bar_rules().intervals,
            first.power_bar_rules().yellow_range
        ),
        (11, 2.0, 9)
    );
    assert_eq!(
        (
            peer.power_bar_rules().base,
            peer.power_bar_rules().intervals,
            peer.power_bar_rules().yellow_range
        ),
        (31, 6.0, baseline.yellow_range)
    );
    peer.reset();
    assert_eq!(peer.power_bar_rules(), baseline);
    drop(peer);
    assert_eq!(first.power_bar_rules().base, 11);
    assert!(first.load_map(empty_map.to_str().unwrap()));
    assert_eq!(first.power_bar_rules(), baseline);
    first.reset();
    assert_eq!(first.power_bar_rules(), baseline);
}
