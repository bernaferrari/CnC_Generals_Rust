use game_client_rust::system::SubsystemInterface;
use game_client_rust::terrain::HeightMap;
use game_client_rust::terrain::chunk::TerrainStaticLighting;
use game_client_rust::terrain::terrain_visual::TerrainVisualImpl;
use game_engine::common::ini::ini_terrain::TerrainTypes;
use glam::Vec3;
use std::sync::Arc;

fn lighting(ambient: [f32; 3]) -> TerrainStaticLighting {
    TerrainStaticLighting {
        lights: vec![(Vec3::new(0.0, -1.0, 0.0), [0.05; 3])],
        ambient,
    }
}

fn visual(light: TerrainStaticLighting) -> TerrainVisualImpl {
    let mut visual = TerrainVisualImpl::new();
    visual.admit_terrain_definitions(Arc::new(TerrainTypes::new()));
    visual.admit_static_lighting(light);
    visual
        .load_heightmap_from_data(HeightMap::new(8, 8, 1.0, 10.0), None, None)
        .unwrap();
    visual.update().unwrap();
    visual
}

fn first_color(visual: &TerrainVisualImpl) -> [f32; 4] {
    visual
        .chunk_manager()
        .iter_chunks()
        .find_map(|chunk| chunk.base_colors.first().copied())
        .expect("terrain update must bake a chunk")
}

#[test]
fn interleaved_terrain_owners_keep_their_own_baked_lighting() {
    let mut first = visual(lighting([0.8, 0.4, 0.4]));
    let first_color_before = first_color(&first);
    let mut peer = visual(lighting([0.4, 0.8, 0.4]));
    let peer_color_before = first_color(&peer);
    assert!(first_color_before[0] > first_color_before[1]);
    assert!(peer_color_before[1] > peer_color_before[0]);

    assert!(peer.admit_static_lighting(lighting([0.4, 0.4, 0.8])));
    peer.update().unwrap();
    assert!(first_color(&peer)[2] > first_color(&peer)[0]);
    first.update().unwrap();
    assert_eq!(first_color(&first), first_color_before);

    peer.reset().unwrap();
    drop(peer);
    first.update().unwrap();
    assert_eq!(first_color(&first), first_color_before);
}
