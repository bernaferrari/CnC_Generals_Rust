use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team, VeterancyLevel};
use glam::Vec3;

const SOURCE: &str = "OclRandomOwnerSource";
const CHILD: &str = "OclRandomOwnerChild";
const OCL: &str = "OCL_RandomOwner";

fn map_ini() -> String {
    format!(
        "ObjectCreationList {OCL}\n\
         CreateObject\n ObjectNames = {CHILD}\n Count = 1\n\
         MinHealth = 0.2\n MaxHealth = 0.8\n\
         Disposition = ON_GROUND_ALIGNED\n End\nEnd\n"
    )
}

fn map_file(directory: &std::path::Path) -> std::path::PathBuf {
    std::fs::create_dir_all(directory).unwrap();
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
    let path = directory.join("RandomOwner.map");
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    std::fs::write(directory.join("Map.ini"), map_ini()).unwrap();
    path
}

fn world(map: &std::path::Path) -> (GameLogic, ObjectId) {
    let source = format!(
        "Object {SOURCE}\n KindOf = VEHICLE SELECTABLE\n\
         Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
         Object {CHILD}\n KindOf = VEHICLE SELECTABLE\n\
         Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    );
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([("Object.ini".into(), source)])
        .unwrap();
    logic.start_new_game(GameMode::SinglePlayer);
    assert!(logic.load_map(map.to_str().unwrap()));
    logic.add_player(Player::new(42, Team::USA, "Owner", true));
    let source = logic
        .create_object_for_player(SOURCE, 42, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    (logic, source)
}

fn dispatch(logic: &mut GameLogic, source: ObjectId) -> ObjectId {
    logic
        .execute_parsed_weapon_ocl_at(
            OCL,
            Some(source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(50.0, 0.0, 50.0),
        )
        .into_iter()
        .next()
        .expect("receiving map OCL creates one child")
}

fn child_state(logic: &GameLogic, child: ObjectId) -> (f32, f32) {
    let child = logic
        .host_object(child)
        .expect("child belongs to this world");
    (child.health.current, child.get_orientation())
}

#[test]
fn interleaved_world_ocl_draws_do_not_change_the_receiving_world_child() {
    let root = tempfile::tempdir().unwrap();
    let control_map = map_file(&root.path().join("control"));
    let first_map = map_file(&root.path().join("first"));
    let peer_map = map_file(&root.path().join("peer"));
    let (mut control, control_source) = world(&control_map);
    let (mut first, first_source) = world(&first_map);
    let (mut peer, peer_source) = world(&peer_map);
    assert_eq!(control_source, first_source);
    assert_eq!(control_source, peer_source);

    // The control and interleaved run begin with identical global fallback
    // state. Under the old implementation, peer health/yaw draws advance that
    // fallback before first's child is created.
    let initial_global_seed = [0xA11CE, 0xB0B, 0xC0FFEE, 4, 5, 6];
    game_engine::common::random_value::set_game_logic_random_seed_state(initial_global_seed);
    let control_child = dispatch(&mut control, control_source);
    let expected = child_state(&control, control_child);

    game_engine::common::random_value::set_game_logic_random_seed_state(initial_global_seed);
    let peer_child = dispatch(&mut peer, peer_source);
    let first_child = dispatch(&mut first, first_source);
    let actual = child_state(&first, first_child);
    assert_eq!(child_state(&peer, peer_child), expected);

    assert!(
        (actual.0 - expected.0).abs() < 0.001,
        "child health changed: {actual:?} vs {expected:?}"
    );
    assert!(
        (actual.1 - expected.1).abs() < 0.001,
        "child yaw changed: {actual:?} vs {expected:?}"
    );
}
