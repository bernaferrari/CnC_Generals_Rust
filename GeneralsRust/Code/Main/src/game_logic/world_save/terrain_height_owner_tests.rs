//! Actual receiving Core bytes, with the detached Main map intentionally stale.
use super::*;

fn install(logic: &GameLogic, raw: &[u8], width: u32, height: u32) {
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = width;
    map.height = height;
    map.heightmap = raw.to_vec();
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_geometry(map);
}

#[test]
fn loaded_core_ground_and_raw_ground_ignore_detached_height_copies() {
    let mut logic = GameLogic::new();
    logic.override_world_size(30., 30.);
    assert!(logic.restore_terrain_heights_from_grid(4, 4, &[99.; 16]));
    install(&logic, &[8; 16], 4, 4);
    let position = Vec3::new(15., 0., 15.);
    assert_eq!(logic.terrain_ground_height_at(position), Some(5.));
    assert_eq!(logic.raw_terrain_ground_height_at(position), 5.);
    assert_eq!(logic.terrain_height_at(position), Some(5.));
    assert_eq!(logic.surface_ht_at(position), Some(5.));
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .apply_logic_height_map_bytes(&[16; 16]);
    assert_eq!(logic.terrain_ground_height_at(position), Some(10.));
    assert_eq!(logic.raw_terrain_ground_height_at(position), 10.);
    assert_eq!(logic.surface_ht_at(position), Some(10.));
}

#[test]
fn provided_terrain_loan_never_reenters_driving_owner_lock() {
    let logic = GameLogic::new();
    let mut other = gamelogic::terrain::TerrainLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 4;
    map.height = 4;
    map.heightmap = vec![24; 16];
    other.load_map_geometry(map);
    let _writer = logic.world_services.terrain().write().unwrap();
    assert_eq!(
        logic.terrain_height_at_with_terrain_logic(Vec3::new(15., 0., 15.), Some(&other)),
        Some(15.)
    );
}

#[test]
fn poisoned_receiving_lock_recovers_its_loaded_ground_instead_of_falling_back() {
    let logic = GameLogic::new();
    install(&logic, &[8; 16], 4, 4);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _writer = logic.world_services.terrain().write().unwrap();
        panic!("poison only this owned terrain");
    }));
    assert!(outcome.is_err());
    assert_eq!(
        logic.terrain_ground_height_at(Vec3::new(15., 0., 15.)),
        Some(5.)
    );
    assert_eq!(
        logic.raw_terrain_ground_height_at(Vec3::new(15., 0., 15.)),
        5.
    );
}

#[test]
#[cfg(feature = "game_client")]
fn snapshot_preserves_all_source_bytes_and_only_matching_material_metadata() {
    let mut logic = GameLogic::new();
    let mut material = game_client::terrain::height_map::HeightMap::new(16, 16, 999., 2.);
    material.heights.fill(1.);
    material.tile_ndxes[7] = 42;
    material.blend_tile_ndxes[7] = 17;
    material.extra_blend_tile_ndxes[7] = 23;
    logic.terrain = Some(crate::game_logic::terrain::TerrainData::from_heightmap(
        material,
        Vec3::ZERO,
        Vec3::new(150., 0., 150.),
        0,
    ));
    let bytes: Vec<u8> = (0..=255).collect();
    install(&logic, &bytes, 16, 16);
    let snapshot = logic.terrain_heightmap_snapshot().unwrap();
    assert_eq!(snapshot.max_height, 255. * 0.625);
    assert_eq!(snapshot.scale, 10.);
    assert_eq!(snapshot.border_size, 0);
    assert_eq!(snapshot.tile_ndxes[7], 42);
    assert_eq!(snapshot.blend_tile_ndxes[7], 17);
    assert_eq!(snapshot.extra_blend_tile_ndxes[7], 23);
    assert_eq!(
        snapshot
            .heights
            .iter()
            .map(|height| (height * 255.).round() as u8)
            .collect::<Vec<_>>(),
        bytes
    );
    // The existing source camera sampler recovers bytes before interpolation;
    // normalized*max_height alone is not an exact world-height bit identity.
    let frame = crate::presentation_frame::PresentationRuntimeHeightmap::from_height_map(&snapshot);
    assert_eq!(frame.heights, snapshot.heights);
    assert_eq!(snapshot.min_height, 0.);
    assert_eq!(snapshot.height_range, 255. * 0.625);
    let env = crate::presentation_frame::PresentationWorldEnv::from_logic_with_runtime_heightmap(
        &logic,
        Some(std::sync::Arc::new(frame)),
    );
    let terrain = logic.world_services.terrain().read().unwrap();
    for z in 0..16 {
        for x in 0..16 {
            let (world_x, world_z) = (x as f32 * 10., z as f32 * 10.);
            assert_eq!(
                env.sample_camera_terrain_height(world_x, world_z)
                    .unwrap()
                    .to_bits(),
                terrain.get_ground_height(world_x, world_z, None).to_bits(),
                "raw-byte recovery at ({x},{z})",
            );
        }
    }
}

#[test]
#[cfg(feature = "game_client")]
fn malformed_map_admission_keeps_the_receiving_snapshot_valid_and_unchanged() {
    let logic = GameLogic::new();
    install(&logic, &[8; 16], 4, 4);
    let before = logic.terrain_heightmap_snapshot().unwrap();
    let mut bad = gamelogic::system::map_loader::MapData::new();
    bad.width = 4;
    bad.height = 4;
    bad.heightmap = vec![99; 3];
    let mut terrain = logic.world_services.terrain().write().unwrap();
    assert!(terrain.try_load_map_geometry(bad).is_err());
    terrain.validate_height_map().unwrap();
    drop(terrain);
    assert_eq!(
        logic.terrain_heightmap_snapshot().unwrap().heights,
        before.heights
    );
}

#[test]
#[cfg(feature = "game_client")]
fn no_core_map_retains_detached_fixture_ground_and_snapshot() {
    let mut logic = GameLogic::new();
    logic.override_world_size(30., 30.);
    assert!(logic.restore_terrain_heights_from_grid(4, 4, &[99.; 16]));
    logic.world_services.terrain().write().unwrap().reset();
    let position = Vec3::new(15., 0., 15.);
    assert_eq!(logic.terrain_ground_height_at(position), Some(99.));
    assert_eq!(logic.raw_terrain_ground_height_at(position), 99.);
    let frozen = logic.terrain_heightmap_snapshot().unwrap();
    assert_eq!(frozen.max_height, 99.);
}

#[test]
#[cfg(feature = "game_client")]
fn actual_chunky_zero_and_positive_borders_survive_core_freeze_and_native_admission() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_chunky_zero_and_positive_borders_survive_core_freeze_and_native_admission",
        || {
            let directory = tempfile::tempdir().unwrap();
            for border in [0, 70] {
                let size = 8 + 2 * border;
                let count = size * size;
                let mut data = game_engine::common::system::DataChunkOutput::new();
                data.open_data_chunk("HeightMapData", 3);
                for value in [size, size, border, count] {
                    data.write_int(value);
                }
                for _ in 0..count {
                    data.write_byte(40);
                }
                data.close_data_chunk();
                let path = directory.path().join(format!("owned-border-{border}.map"));
                std::fs::write(&path, data.into_ckmp_bytes()).unwrap();
                let mut world = crate::game_logic::GameLogic::new();
                assert!(world.load_map(path.to_str().unwrap()));
                assert_eq!(
                    world
                        .terrain
                        .as_ref()
                        .unwrap()
                        .heightmap_clone()
                        .border_size,
                    border
                );
                let env = crate::presentation_frame::PresentationWorldEnv::from_logic(&world);
                let frozen = env.runtime_heightmap.as_ref().unwrap().to_height_map();
                assert_eq!(frozen.border_size, border);
                let mut native =
                    crate::graphics::render_pipeline::RenderPipeline::prepare_native_terrain_cpu(
                        &env,
                    )
                    .unwrap()
                    .unwrap();
                let peer =
                    crate::graphics::render_pipeline::RenderPipeline::prepare_native_terrain_cpu(
                        &env,
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(native.get_raw_map_height(1, 1), 40);
                world
                    .world_services
                    .terrain()
                    .write()
                    .unwrap()
                    .set_raw_map_height_with_effects(1, 1, -1, &mut |change| {
                        assert_eq!((change.sample_x, change.sample_y), (border + 1, border + 1));
                        native.apply_accepted_height(
                            change.sample_x,
                            change.sample_y,
                            change.raw_height,
                        );
                    });
                assert_eq!(
                    native.get_raw_map_height(1, 1),
                    255,
                    "authored border={border}"
                );
                assert_eq!(peer.get_raw_map_height(1, 1), 40);
                assert_eq!(
                    world
                        .world_services
                        .terrain()
                        .read()
                        .unwrap()
                        .logic_height_map_border_size(),
                    border
                );
                // Saved canonical-grid admission validates the exact preserved native shape.
                let terrain = world.world_services.terrain().read().unwrap();
                native
                    .admit_canonical_height_grid(
                        size,
                        size,
                        border,
                        terrain.logic_height_map_bytes(),
                    )
                    .unwrap();
                assert_eq!(native.get_raw_map_height(1, 1), 255);
            }
        },
    );
}
