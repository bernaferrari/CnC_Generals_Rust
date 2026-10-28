use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;
use std::path::Path;

fn minimal_map_bytes() -> Vec<u8> {
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
    map.into_ckmp_bytes()
}

fn configured_world(
    root: &Path,
    name: &str,
    default_rubble_height: f32,
    template_rubble_height: u8,
) -> GameLogic {
    let directory = root.join(name);
    std::fs::create_dir(&directory).unwrap();
    let map_path = directory.join("RubbleOwner.map");
    std::fs::write(&map_path, minimal_map_bytes()).unwrap();
    std::fs::write(
        directory.join("Map.ini"),
        format!("GameData\n DefaultStructureRubbleHeight = {default_rubble_height}\nEnd\n"),
    )
    .unwrap();

    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "Rubble.ini".into(),
            format!(
                "Object SameNameRubbleProbe\n KindOf = STRUCTURE SELECTABLE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 15\n GeometryHeight = 40\n StructureRubbleHeight = {template_rubble_height}\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
            ),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map_path.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Rubble Owner", false));

    world
}

fn live_world(
    root: &Path,
    name: &str,
    default_rubble_height: f32,
    template_rubble_height: u8,
) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = configured_world(root, name, default_rubble_height, template_rubble_height);
    let id = world
        .create_object_for_player("SameNameRubbleProbe", 1, Vec3::ZERO)
        .expect("create structure in this receiving world");
    (world, id)
}

fn kill_structure(world: &mut GameLogic, id: generals_main::game_logic::ObjectId) -> f32 {
    assert_eq!(world.apply_host_damage(id, 1000.0), (true, false));
    world.host_object(id).unwrap().thing().geometry.bounds_max.y
}

#[test]
fn same_id_worlds_use_their_own_map_game_data_rubble_fallback() {
    let root = tempfile::tempdir().unwrap();
    let (mut first, first_id) = live_world(root.path(), "first", 4.5, 0);
    let (mut second, second_id) = live_world(root.path(), "second", 9.25, 0);
    assert_eq!(first_id, second_id);

    // Damage the second world first to catch any process-global last-writer wins.
    assert_eq!(kill_structure(&mut second, second_id), 9.25);
    assert_eq!(
        first.host_object(first_id).unwrap().thing().geometry.bounds_max.y,
        40.0
    );
    assert_eq!(kill_structure(&mut first, first_id), 4.5);
    assert_eq!(
        second
            .host_object(second_id)
            .unwrap()
            .thing()
            .geometry
            .bounds_max.y,
        9.25
    );

    // Resetting the first session must admit the replacement map's rules
    // without changing the still-live peer.
    first.reset();
    first.start_new_game(GameMode::SinglePlayer);
    let replacement = root.path().join("second/RubbleOwner.map");
    assert!(first.load_map(replacement.to_str().unwrap()));
    first.add_player(Player::new(1, Team::USA, "Rubble Owner", false));
    let replacement_id = first
        .create_object_for_player("SameNameRubbleProbe", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(kill_structure(&mut first, replacement_id), 9.25);
    assert_eq!(second.host_object(second_id).unwrap().thing().geometry.bounds_max.y, 9.25);
}

#[test]
fn restored_object_uses_receiving_map_rules_and_source_continues_on_its_rules() {
    let root = tempfile::tempdir().unwrap();
    let (mut source, id) = live_world(root.path(), "source", 4.5, 0);
    let mut receiver = configured_world(root.path(), "receiver", 9.25, 0);

    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .expect("snapshot live source object");
    // Snapshotting does not replace the source's effective GameData.
    assert_eq!(kill_structure(&mut source, id), 4.5);

    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiver)
        .expect("restore into the receiving world's catalog");
    assert_eq!(kill_structure(&mut receiver, id), 9.25);
}

#[test]
fn rubble_fallback_preserves_zero_negative_and_authored_template_rules() {
    let root = tempfile::tempdir().unwrap();
    let (mut zero, zero_id) = live_world(root.path(), "zero", 0.0, 0);
    let (mut negative, negative_id) = live_world(root.path(), "negative", -3.5, 0);
    let (mut authored, authored_id) = live_world(root.path(), "authored", -3.5, 7);

    // ActiveBody.cpp:191-199 falls back on <= 0 template height and passes the
    // GameData Real through unchanged, including zero and negative values.
    assert_eq!(kill_structure(&mut zero, zero_id), 0.0);
    assert_eq!(kill_structure(&mut negative, negative_id), -3.5);
    assert_eq!(kill_structure(&mut authored, authored_id), 7.0);
}
