use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_road::get_terrain_roads;
use std::{fs, path::PathBuf};

#[test]
fn retail_roads_parse_and_can_be_reapplied_as_overrides() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../windows_game/extracted_big_files_v2/INI/Roads.ini");
    let Ok(source) = fs::read_to_string(&path) else {
        return;
    };

    for layer in ["preload", "map loader"] {
        let mut ini = INI::new();
        ini.with_inline_source(&source, |ini| ini.parse_current_file())
            .unwrap_or_else(|error| {
                panic!(
                    "retail {} must parse during {layer}: {error:?}",
                    path.display()
                )
            });
    }

    let roads = get_terrain_roads();
    let two_lane = roads.find_road("TwoLane").expect("retail road stored");
    assert_eq!(two_lane.texture.to_str(), "TRTwoLane.tga");
    assert_eq!(two_lane.road_width, 35.0);

    let concrete = roads.find_bridge("Concrete").expect("retail bridge stored");
    assert_eq!(concrete.bridge_model_name.to_str(), "CBBridgeSt");
}
