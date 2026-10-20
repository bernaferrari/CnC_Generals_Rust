//! The real Main coordinate adapter and receiving terrain owner.
use super::{TerrainLogic, Vec3, terrain_hit};

fn terrain(width: u32, height: u32, bytes: Vec<u8>) -> TerrainLogic {
    let mut data = gamelogic::system::map_loader::MapData::new();
    data.width = width;
    data.height = height;
    data.heightmap = bytes;
    let mut terrain = TerrainLogic::new();
    terrain.load_map_geometry(data);
    terrain
}

#[test]
fn camera_ray_uses_receiving_height_bytes_with_interleaved_terrain_owners() {
    let first = terrain(4, 4, vec![8; 16]);
    let start = Vec3::new(15., 100., 15.);
    let delta = Vec3::new(0., -120., 0.);
    let before = terrain_hit(start, delta, &first).unwrap().unwrap();
    let second = terrain(4, 4, vec![16; 16]);
    assert_eq!(terrain_hit(start, delta, &second).unwrap().unwrap().y, 10.);
    assert_eq!(terrain_hit(start, delta, &first).unwrap(), Some(before));
    assert_eq!(
        before.to_array().map(f32::to_bits),
        [1097859072, 1084227584, 1097859072]
    );
    drop(second);
    assert_eq!(terrain_hit(start, delta, &first).unwrap(), Some(before));
}

#[test]
fn camera_ray_preserves_original_first_grid_hit_instead_of_nearest_hit() {
    let terrain = terrain(6, 3, [0, 0, 160, 0, 160, 0].repeat(3));
    // Executed original case review_three_intersections_reverse_x. Original
    // Cast_Ray visits ascending cells and short circuits after its first hit.
    let point = terrain_hit(
        Vec3::new(55., 80., 5.),
        Vec3::new(-50., -100., 0.),
        &terrain,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        point.to_array().map(f32::to_bits),
        [1104936960, 1103626240, 1084227584]
    );
}

#[test]
fn camera_ray_accepts_original_upward_contact_and_unloaded_map_miss() {
    let terrain = terrain(4, 4, vec![8; 16]);
    let start = Vec3::new(15., -20., 15.);
    let delta = Vec3::new(0., 120., 0.);
    assert_eq!(terrain_hit(start, delta, &terrain).unwrap().unwrap().y, 5.);
    assert_eq!(
        terrain_hit(start, delta, &TerrainLogic::new()).unwrap(),
        None
    );
}
