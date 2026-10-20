//! The real engine-owned cache, including equal-revision foreign world owners.
use super::*;

fn loaded(value: u8) -> GameLogic {
    let logic = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 4;
    map.height = 4;
    map.heightmap = vec![value; 16];
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_geometry(map);
    logic
}

#[test]
fn actual_core_mutation_invalidates_snapshot_without_engine_boundary_call() {
    let logic = loaded(8);
    let mut cache = PresentationTerrainCache::default();
    let first = cache.runtime_heightmap_for_logic(&logic).unwrap();
    let first_bytes = first.heights.clone();
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .apply_logic_height_map_bytes(&[16; 16]);
    let changed = cache.runtime_heightmap_for_logic(&logic).unwrap();
    assert!(!Arc::ptr_eq(&first, &changed));
    assert_eq!(
        first.heights, first_bytes,
        "previous frame remains immutable"
    );
    assert_eq!((changed.heights[0] * 255.).round() as u8, 16);
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .apply_logic_height_map_bytes(&[16; 16]);
    let unchanged = cache.runtime_heightmap_for_logic(&logic).unwrap();
    assert!(Arc::ptr_eq(&changed, &unchanged));
}

#[test]
fn equal_revision_foreign_owners_never_reuse_each_others_height_payload() {
    let first = loaded(8);
    let second = loaded(24);
    assert_eq!(
        first
            .world_services
            .terrain()
            .read()
            .unwrap()
            .height_revision(),
        second
            .world_services
            .terrain()
            .read()
            .unwrap()
            .height_revision()
    );
    let mut cache = PresentationTerrainCache::default();
    let a = cache.runtime_heightmap_for_logic(&first).unwrap();
    let b = cache.runtime_heightmap_for_logic(&second).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert_eq!((a.heights[0] * 255.).round() as u8, 8);
    assert_eq!((b.heights[0] * 255.).round() as u8, 24);
    drop(second);
    let again = cache.runtime_heightmap_for_logic(&first).unwrap();
    assert_eq!(again.heights, a.heights);
    assert!(!Arc::ptr_eq(&b, &again));
}

#[test]
fn dropped_owner_weak_identity_does_not_keep_mutable_world_alive() {
    let mut cache = PresentationTerrainCache::default();
    let first = loaded(8);
    let owner = Arc::downgrade(first.world_services.terrain());
    let old_snapshot = cache.runtime_heightmap_for_logic(&first).unwrap();
    drop(first);
    assert!(owner.upgrade().is_none());
    let second = loaded(32);
    let next = cache.runtime_heightmap_for_logic(&second).unwrap();
    assert!(!Arc::ptr_eq(&old_snapshot, &next));
    assert_eq!((old_snapshot.heights[0] * 255.).round() as u8, 8);
    assert_eq!((next.heights[0] * 255.).round() as u8, 32);
}

#[test]
fn actual_core_frame_picking_uses_source_triangles_and_byte_rounding() {
    let logic = loaded(8);
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 6;
    map.height = 6;
    map.border_size = 1;
    map.heightmap = vec![8; 64];
    // Full-grid stride eight: original source cell at playable (0,0).
    map.heightmap[9] = 10;
    map.heightmap[10] = 40;
    map.heightmap[17] = 90;
    map.heightmap[18] = 210;
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_geometry(map);
    let mut cache = PresentationTerrainCache::default();
    let payload = cache.runtime_heightmap_for_logic(&logic).unwrap();
    assert!(payload.source_byte_heights);
    let mut frame =
        crate::presentation_frame::PresentationWorldEnv::from_logic_with_runtime_heightmap(
            &logic,
            Some(payload),
        );
    // Unrelated bounds must not remap original logical coordinates.
    frame.world_min = [100., 0., 100.];
    frame.world_max = [200., 0., 200.];
    assert_eq!(frame.sample_gameplay_terrain_height(2.5, 7.5), Some(62.5));
    assert_eq!(frame.sample_gameplay_terrain_height(7.5, 2.5), Some(46.875));
    // byte8/255 *159.375 is not bit-identical to source byte8*.625.
    assert_ne!((8f32 / 255. * 159.375).to_bits(), 5f32.to_bits());
    assert_eq!(frame.sample_gameplay_terrain_height(-10., -10.), Some(5.));
}

#[test]
fn actual_core_and_frozen_negative_integer_cell_use_source_fast_floor() {
    let logic = loaded(8);
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 4;
    map.height = 4;
    map.border_size = 2;
    map.heightmap = vec![8; 64];
    map.heightmap[17] = 40;
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_geometry(map);
    // Source FAST_REAL_FLOOR(-1f32) is -2 under original NEAR arithmetic.
    // Border2 therefore clips cell0 rather than sampling interior cell1.
    let authoritative = logic
        .terrain_ground_height_at(glam::Vec3::new(-10., 0., 0.))
        .unwrap();
    assert_eq!(authoritative, 5.);
    let mut cache = PresentationTerrainCache::default();
    let snapshot = cache.runtime_heightmap_for_logic(&logic).unwrap();
    let frame = crate::presentation_frame::PresentationWorldEnv::from_logic_with_runtime_heightmap(
        &logic,
        Some(snapshot),
    );
    assert_eq!(
        frame.sample_gameplay_terrain_height(-10., 0.),
        Some(authoritative)
    );
    assert_eq!(
        frame.sample_camera_terrain_height(-10., 0.),
        Some(authoritative)
    );
}
