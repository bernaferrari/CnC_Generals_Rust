use game_engine::common::system::DataChunkOutput;
use generals_main::{
    game_logic::{GameLogic, GameMode},
    presentation_frame::PresentationFrame,
    save_load::SnapshotBuilder,
};

fn map_path(root: &std::path::Path, name: &str, draw_sky_box: bool) -> std::path::PathBuf {
    let directory = root.join(name);
    std::fs::create_dir(&directory).expect("create map directory");
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
    let path = directory.join("Skybox.map");
    std::fs::write(&path, map.into_ckmp_bytes()).expect("write map chunks");
    std::fs::write(
        directory.join("Map.ini"),
        format!(
            "GameData\n DrawSkyBox = {}\nEnd\n",
            if draw_sky_box { "Yes" } else { "No" }
        ),
    )
    .expect("write map GameData");
    path
}

fn admitted_world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().expect("map path is UTF-8")));
    world
}

fn skybox_enabled(world: &mut GameLogic) -> bool {
    PresentationFrame::build_from_logic(world, 0)
        .world_env
        .skybox_enabled
}

#[test]
fn skybox_state_is_saved_and_restored_without_changing_a_peer_world() {
    let root = tempfile::tempdir().unwrap();
    let disabled_map = map_path(root.path(), "disabled", false);
    let enabled_map = map_path(root.path(), "enabled", true);
    let solo_map = map_path(root.path(), "solo", false);
    std::fs::write(
        solo_map.parent().unwrap().join("Solo.ini"),
        "GameData\n DrawSkyBox = Yes\nEnd\n",
    )
    .unwrap();
    let mut first = admitted_world(&disabled_map);
    let mut peer = admitted_world(&enabled_map);
    let mut solo = admitted_world(&solo_map);
    assert!(!skybox_enabled(&mut first));
    assert!(skybox_enabled(&mut peer));
    assert!(skybox_enabled(&mut solo), "Solo.ini overrides Map.ini");

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&first)
        .expect("capture receiving skybox continuation");
    let mut restored = admitted_world(&disabled_map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore receiving skybox continuation");

    assert!(!restored.peek_script_skybox_enabled());
    assert!(!skybox_enabled(&mut restored));
    assert!(peer.peek_script_skybox_enabled());
    assert!(skybox_enabled(&mut peer));
}
