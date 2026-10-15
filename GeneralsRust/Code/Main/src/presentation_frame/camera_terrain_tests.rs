use super::*;

fn map() -> PresentationRuntimeHeightmap {
    PresentationRuntimeHeightmap {
        width: 6,
        height: 6,
        heights: vec![0.; 36],
        max_height: 159.375,
        scale: 10.,
        border_size: 1,
        ..Default::default()
    }
}
fn put(map: &mut PresentationRuntimeHeightmap, x: usize, z: usize, byte: u8) {
    map.heights[z * map.width as usize + x] = f32::from(byte) / 255.;
}

#[test]
fn camera_terrain_uses_asymmetric_original_triangles() {
    let mut map = map();
    put(&mut map, 1, 1, 10);
    put(&mut map, 2, 1, 40);
    put(&mut map, 1, 2, 90);
    put(&mut map, 2, 2, 210);
    // Upper: 90 + .25*(10-90) + .25*(210-90) = 100.
    assert_eq!(sample_camera_height(&map, 2.5, 7.5), Some(62.5));
    // Lower: 40 + .25*(210-40) + .25*(10-40) = 75.
    assert_eq!(sample_camera_height(&map, 7.5, 2.5), Some(46.875));
    // Equality belongs to lower triangle.
    assert_eq!(sample_camera_height(&map, 5., 5.), Some(68.75));
}

#[test]
fn camera_terrain_floors_before_border_and_clips_single_integer_sample() {
    let mut map = map();
    put(&mut map, 0, 1, 200);
    put(&mut map, 1, 1, 20);
    put(&mut map, 5, 1, 255);
    put(&mut map, 4, 1, 80);
    assert_eq!(sample_camera_height(&map, -0.1, 2.), Some(125.));
    assert_eq!(sample_camera_height(&map, -1000., 2.), Some(125.));
    // ix=4 is beyond extent-3: fractional .9 must not interpolate to255.
    assert_eq!(sample_camera_height(&map, 39., 2.), Some(50.));
    assert_eq!(sample_camera_height(&map, 1000., 2.), Some(159.375));
}

#[test]
fn camera_terrain_five_samples_keep_tall_raw_corner_and_peer_is_immutable() {
    let mut first = map();
    first.width = 12;
    first.height = 12;
    first.heights = vec![0.; 144];
    put(&mut first, 6, 6, 255); // (50,50), positive corner of(10,10).
    let mut peer = first.clone();
    put(&mut peer, 6, 6, 16);
    let positions = [
        (10., 10.),
        (50., -30.),
        (-30., -30.),
        (50., 50.),
        (-30., 50.),
    ];
    let max = positions
        .into_iter()
        .map(|(x, z)| sample_camera_height(&first, x, z).unwrap())
        .fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(max, 159.375); // no initial-ground120cap applied toterrain.
    assert_eq!(sample_camera_height(&peer, 50., 50.), Some(10.));
    assert_eq!(sample_camera_height(&first, 50., 50.), Some(159.375));
}

#[test]
fn camera_terrain_receiver_ignores_playable_bounds_and_has_no_coarse_fallback() {
    use crate::presentation_frame::PresentationWorldEnv;
    let mut grid = map();
    put(&mut grid, 1, 1, 255);
    let mut env = PresentationWorldEnv::default();
    env.world_min = [-500., 0., -700.];
    env.world_max = [900., 0., 1000.];
    assert_eq!(env.sample_camera_terrain_height(0., 0.), None);
    env.runtime_heightmap = Some(std::sync::Arc::new(grid));
    assert_eq!(env.sample_camera_terrain_height(0., 0.), Some(159.375));
    assert_eq!(env.sample_camera_terrain_height(f32::NAN, 0.), None);
}
