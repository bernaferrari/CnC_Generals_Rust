use game_client_rust::terrain::{
    TerrainTrackHeightProvider, TerrainTrackLayer, TerrainTracksConfig,
    TerrainTracksRenderObjClassSystem, BRIDGE_OFFSET_FACTOR,
};
use game_engine::map_object::MAP_XY_FACTOR;
use glam::Vec3;

struct FlatTerrain;

impl TerrainTrackHeightProvider for FlatTerrain {
    fn ground_height_and_normal(&self, _x: f32, _y: f32) -> (f32, Vec3) {
        (10.0, Vec3::Z)
    }

    fn layer_height_and_normal(&self, _x: f32, _y: f32) -> (f32, Vec3) {
        (20.0, Vec3::Z)
    }
}

fn config() -> TerrainTracksConfig {
    TerrainTracksConfig {
        max_terrain_tracks: 2,
        max_tank_track_edges: 4,
        max_tank_track_opaque_edges: 2,
        max_tank_track_fade_delay: 100,
        make_track_marks: true,
    }
}

#[test]
fn first_edge_only_sets_anchor() {
    let terrain = FlatTerrain;
    let mut system = TerrainTracksRenderObjClassSystem::new(config());
    let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();

    system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);

    assert_eq!(system.track(handle).unwrap().active_edge_count(), 0);
}

#[test]
fn second_far_edge_adds_transparent_restart_edge() {
    let terrain = FlatTerrain;
    let mut system = TerrainTracksRenderObjClassSystem::new(config());
    let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
    system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
    system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);

    let edge = system.track(handle).unwrap().active_edges(4)[0];
    assert_eq!(edge.alpha, 0.0);
    assert_eq!(edge.endpoint_uv, [[0.0, 1.0], [1.0, 1.0]]);
}

#[test]
fn cap_near_last_anchor_forces_last_edge_transparent() {
    let terrain = FlatTerrain;
    let mut system = TerrainTracksRenderObjClassSystem::new(config());
    let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
    system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
    system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);
    system.add_edge_to_track(handle, &terrain, 40.0, 0.0, 2);

    system.add_cap_edge_to_track(handle, &terrain, 41.0, 0.0, 3);

    let edges = system.track(handle).unwrap().active_edges(4);
    assert_eq!(edges.last().unwrap().alpha, 0.0);
}

#[test]
fn update_fades_and_releases_unbound_tracks() {
    let terrain = FlatTerrain;
    let mut system = TerrainTracksRenderObjClassSystem::new(config());
    let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
    system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
    system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);
    system.add_edge_to_track(handle, &terrain, 40.0, 0.0, 2);
    system.unbind_track(handle);

    system.update(200);

    assert!(!system.track(handle).unwrap().bound());
    assert_eq!(system.track(handle).unwrap().active_edge_count(), 1);

    system.update(201);
    assert_eq!(system.track(handle).unwrap().active_edge_count(), 0);
}

#[test]
fn flush_builds_vertices_indices_and_resets_counter() {
    let terrain = FlatTerrain;
    let mut system = TerrainTracksRenderObjClassSystem::new(config());
    let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
    system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
    system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);
    system.add_edge_to_track(handle, &terrain, 40.0, 0.0, 2);
    system.submit_render(handle);

    let flush = system.flush(0x0012_3456);

    assert_eq!(flush.vertices.len(), 4);
    assert_eq!(flush.indices, vec![0, 1, 3, 0, 3, 2]);
    assert_eq!(flush.ranges[0].texture_name, "tracks.tga");
    assert_eq!(flush.vertices[0].diffuse & 0x00ff_ffff, 0x0012_3456);
    assert!(system.flush(0x0012_3456).vertices.is_empty());
}

#[test]
fn bridge_layer_adds_cpp_bridge_offset() {
    let terrain = FlatTerrain;
    let mut system = TerrainTracksRenderObjClassSystem::new(config());
    let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
    system
        .track_mut(handle)
        .unwrap()
        .set_owner_layer(TerrainTrackLayer::Other);

    system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
    system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);

    let edge = system.track(handle).unwrap().active_edges(4)[0];
    assert_eq!(
        edge.endpoint_pos[0].z,
        20.0 + BRIDGE_OFFSET_FACTOR + 0.2 * MAP_XY_FACTOR
    );
}

#[test]
fn set_detail_clears_tracks_and_accepts_new_edge_capacity() {
    let terrain = FlatTerrain;
    let mut system = TerrainTracksRenderObjClassSystem::new(config());
    let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
    system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
    system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);
    system.submit_render(handle);

    let mut new_config = config();
    new_config.max_tank_track_edges = 6;
    new_config.max_tank_track_opaque_edges = 3;
    new_config.max_tank_track_fade_delay = 200;
    system.set_detail(new_config);

    let track = system.track(handle).unwrap();
    assert_eq!(track.active_edge_count(), 0);
    assert!(!track.have_anchor());
    assert!(track.have_cap());
    assert!(system.flush(0x0012_3456).vertices.is_empty());

    for i in 0..7 {
        system.add_edge_to_track(handle, &terrain, i as f32 * 20.0, 0.0, i);
    }

    assert_eq!(system.track(handle).unwrap().active_edge_count(), 6);
    assert_eq!(system.track(handle).unwrap().active_edges(6).len(), 6);
}
