//! Public WorldInfo weather, save continuation, and presentation snapshot ownership.
use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode};
use generals_main::presentation_frame::PresentationFrame;
use generals_main::save_load::SnapshotBuilder;

fn map_with_weather_rule(root: &std::path::Path, name: &str, rule: &str) -> std::path::PathBuf {
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

    let map_path = directory.join("Weather.map");
    std::fs::write(&map_path, map.into_ckmp_bytes()).expect("write map chunks");
    std::fs::write(
        directory.join("Map.ini"),
        format!("GameData\n ForceModelsToFollowWeather = {rule}\nEnd\n"),
    )
    .expect("write receiving GameData lighting rule");
    map_path
}

fn world_with_world_info_weather(map: &std::path::Path, intensity: f32) -> GameLogic {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::SinglePlayer);
    assert!(logic.load_map(map.to_str().expect("map path is UTF-8")));
    logic.set_weather_state("clear", intensity, 90.0, 30.0);
    logic.apply_world_info_weather(Some(1));
    logic
}

fn published_snow(logic: &mut GameLogic) -> bool {
    PresentationFrame::build_from_logic(logic, 0)
        .world_env
        .is_snow
}

#[test]
fn world_info_weather_stays_with_each_world_through_presentation_reset_and_restore() {
    let root = tempfile::tempdir().unwrap();
    let follows_weather = map_with_weather_rule(root.path(), "follows", "Yes");
    let ignores_weather = map_with_weather_rule(root.path(), "ignores", "No");
    let mut snowy = world_with_world_info_weather(&follows_weather, 0.4);
    let mut peer = world_with_world_info_weather(&ignores_weather, 0.8);

    assert_eq!(snowy.weather_state().current_weather, "snowy");
    assert_eq!(peer.weather_state().current_weather, "snowy");
    assert_eq!(snowy.weather_state().intensity, 0.4);
    assert_eq!(peer.weather_state().intensity, 0.8);
    assert!(published_snow(&mut snowy));
    assert!(
        !published_snow(&mut peer),
        "the receiving Map.ini rule controls model weather translation"
    );

    peer.apply_world_info_weather(Some(0));
    assert_eq!(peer.weather_state().current_weather, "normal");
    assert_eq!(snowy.weather_state().current_weather, "snowy");

    let builder = SnapshotBuilder::new();
    let snowy_snapshot = builder
        .create_world_snapshot(&snowy)
        .expect("capture snowy world");
    let peer_snapshot = builder
        .create_world_snapshot(&peer)
        .expect("capture peer world");

    // Reapply WorldInfo to one session and reset it while its peer remains live.
    snowy.apply_world_info_weather(Some(0));
    assert_eq!(snowy.weather_state().current_weather, "normal");
    assert_eq!(peer.weather_state().current_weather, "normal");
    snowy.reset();
    assert_eq!(snowy.weather_state().current_weather, "clear");
    assert_eq!(peer.weather_state().current_weather, "normal");

    // Restore into receivers that have admitted the corresponding map and its
    // team definitions; a bare GameLogic is not a valid loaded-map candidate.
    let mut restored_snow = world_with_world_info_weather(&follows_weather, 0.0);
    let mut restored_peer = world_with_world_info_weather(&ignores_weather, 0.0);
    builder
        .restore_from_snapshot(&snowy_snapshot, &mut restored_snow)
        .expect("restore snowy continuation");
    builder
        .restore_from_snapshot(&peer_snapshot, &mut restored_peer)
        .expect("restore peer continuation");
    assert_eq!(restored_snow.weather_state().current_weather, "snowy");
    assert_eq!(restored_peer.weather_state().current_weather, "normal");
    assert_eq!(restored_snow.weather_state().intensity, 0.4);
    assert_eq!(restored_peer.weather_state().intensity, 0.8);
    assert!(published_snow(&mut restored_snow));
    assert!(!published_snow(&mut restored_peer));
}
