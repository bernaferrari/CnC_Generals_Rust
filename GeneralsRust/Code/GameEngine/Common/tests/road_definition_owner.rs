use game_engine::common::ini::ini::{INI, INIError, INILoadType};
use game_engine::common::ini::ini_road::{TerrainRoadCollection, get_terrain_roads};

fn overlay(base: &TerrainRoadCollection, text: &str) -> Result<TerrainRoadCollection, INIError> {
    let mut ini = INI::new();
    ini.set_terrain_roads_target(base.clone());
    ini.with_inline_source_and_load_type(text, INILoadType::Overwrite, |ini| {
        ini.parse_current_file()
    })?;
    Ok(ini.take_terrain_roads_target().unwrap())
}

#[test]
fn road_and_bridge_overrides_stay_with_the_receiving_catalog() {
    let base = overlay(
        &TerrainRoadCollection::new(),
        "Road DefaultRoad\n Texture = base.tga\n RoadWidth = 10\nEnd\n\
         Road SharedRoad\nEnd\n\
         Bridge DefaultBridge\n BridgeModelName = BaseModel\n BridgeScale = 2\nEnd\n\
         Bridge SharedBridge\nEnd\n",
    )
    .unwrap();
    let first = overlay(
        &base,
        "Road SharedRoad\n RoadWidth = 21\nEnd\n\
         Bridge SharedBridge\n BridgeModelName = FirstModel\nEnd\n",
    )
    .unwrap();
    let peer = overlay(
        &base,
        "Road SharedRoad\n RoadWidth = 34\nEnd\n\
         Bridge SharedBridge\n BridgeModelName = PeerModel\nEnd\n",
    )
    .unwrap();

    let first_road = first.find_road("SharedRoad").unwrap();
    let peer_road = peer.find_road("SharedRoad").unwrap();
    assert_eq!((first_road.id, peer_road.id), (2, 2));
    assert_eq!((first_road.road_width, peer_road.road_width), (21.0, 34.0));
    assert_eq!(base.find_road("SharedRoad").unwrap().road_width, 10.0);
    assert_eq!(first_road.texture.as_str(), "base.tga");
    assert_eq!(first.find_bridge("SharedBridge").unwrap().id, 4);
    assert_eq!(first.find_bridge("SharedBridge").unwrap().bridge_scale, 2.0);
    assert_eq!(
        first
            .find_bridge("SharedBridge")
            .unwrap()
            .bridge_model_name
            .as_str(),
        "FirstModel"
    );
    assert_eq!(
        peer.find_bridge("SharedBridge")
            .unwrap()
            .bridge_model_name
            .as_str(),
        "PeerModel"
    );

    assert!(overlay(&first, "Road SharedRoad\n RoadWidth = invalid\nEnd\n").is_err());
    assert_eq!(first.find_road("SharedRoad").unwrap().road_width, 21.0);
    assert!(get_terrain_roads().find_road("SharedRoad").is_none());
    drop(first);
    assert_eq!(peer.find_road("SharedRoad").unwrap().road_width, 34.0);
}

#[test]
fn supplied_retail_road_sources_parse_into_an_owned_catalog() {
    let Ok(base_path) = std::env::var("GENERALS_TEST_ROAD_BASE") else {
        return;
    };
    let retail_path = std::env::var("GENERALS_TEST_ROAD_RETAIL").unwrap();
    let base = std::fs::read_to_string(base_path).unwrap();
    let retail = std::fs::read_to_string(retail_path).unwrap();
    let definitions = overlay(&TerrainRoadCollection::new(), &base).unwrap();
    let definitions = overlay(&definitions, &retail).unwrap();
    let road = definitions.find_road("TwoLane").unwrap();
    assert_eq!(road.texture.as_str(), "TRTwoLane.tga");
    assert_eq!(road.road_width, 35.0);
    assert_eq!(
        definitions
            .find_bridge("Concrete")
            .unwrap()
            .bridge_model_name
            .as_str(),
        "CBBridgeSt"
    );
}
