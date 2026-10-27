use game_client_rust::terrain::terrain_visual::TerrainVisualImpl;
use game_engine::common::ini::ini_road::TerrainRoadCollection;

#[test]
fn same_named_bridges_use_each_visuals_admitted_art() {
    let mut first_roads = TerrainRoadCollection::new();
    let bridge = first_roads.new_bridge("SharedBridge".into());
    bridge.bridge_model_name = "FirstModel".into();
    bridge.bridge_scale = 1.0;
    let mut peer_roads = TerrainRoadCollection::new();
    peer_roads.new_bridge("SharedBridge".into());

    let span = [(
        [0.0, 0.0, 0.0],
        [30.0, 0.0, 0.0],
        8.0,
        "SharedBridge".into(),
    )];
    let mut first = TerrainVisualImpl::new();
    first.admit_road_presentation(first_roads.presentation());
    first.set_runtime_bridge_segments(&span).unwrap();
    let expected = first.minimap_road_samples(2);
    assert!(!expected.is_empty());

    let mut peer = TerrainVisualImpl::new();
    peer.admit_road_presentation(peer_roads.presentation());
    peer.set_runtime_bridge_segments(&span).unwrap();
    assert!(peer.minimap_road_samples(2).is_empty());
    drop(peer);
    assert_eq!(first.minimap_road_samples(2).len(), expected.len());
}
