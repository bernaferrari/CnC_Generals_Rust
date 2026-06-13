use game_client_rust::terrain::{height_map::HeightMap, terrain_visual::TerrainVisualImpl};

fn loaded_visual_with_border() -> TerrainVisualImpl {
    let mut heightmap = HeightMap::new(6, 6, 255.0, 1.0);
    heightmap.border_size = 1;
    heightmap.set_raw_height(2, 3, 100);
    heightmap.set_raw_height(3, 3, 120);

    let mut visual = TerrainVisualImpl::new();
    visual
        .load_heightmap_from_data(heightmap, None, None)
        .expect("runtime heightmap should load");
    visual
}

#[test]
fn raw_map_height_only_lowers_and_reads_logic_height() {
    let mut visual = loaded_visual_with_border();

    assert_eq!(visual.get_raw_map_height(1, 2), 100);

    visual.set_raw_map_height(1, 2, 130);
    assert_eq!(visual.get_raw_map_height(1, 2), 100);

    visual.set_raw_map_height(1, 2, 80);
    assert_eq!(visual.get_raw_map_height(1, 2), 80);

    assert_eq!(visual.get_raw_map_height(2, 2), 120);
}

#[test]
fn raw_map_height_returns_zero_without_loaded_logic_map() {
    let mut visual = TerrainVisualImpl::new();

    assert_eq!(visual.get_raw_map_height(1, 2), 0);
    visual.set_raw_map_height(1, 2, 80);
    assert_eq!(visual.get_raw_map_height(1, 2), 0);
}
