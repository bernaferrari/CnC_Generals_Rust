use super::*;

#[cfg(feature = "game_client")]
fn world_with_sloped_terrain() -> GameLogic {
    let mut logic = GameLogic::new();
    let mut heights = game_client::terrain::height_map::HeightMap::new(129, 129, 160.0, 10.0);
    for y in 0..129 {
        for x in 0..129 {
            heights.set_height_at_index(x, y, ((x + y) % 100) as f32);
        }
    }
    logic.world_min = glam::Vec3::new(30.0, 0.0, -40.0);
    logic.world_max = glam::Vec3::new(1310.0, 160.0, 1240.0);
    let mut terrain = crate::game_logic::terrain::TerrainData::from_heightmap(
        heights,
        logic.world_min,
        logic.world_max,
        0,
    );
    terrain.water_plane_y = Some(10.0);
    logic.terrain = Some(terrain);
    store_radar_map_extent(&mut logic.radar_map, logic.world_min, logic.world_max).expect("extent");
    logic
}

#[test]
#[cfg(feature = "game_client")]
fn radar_sampling_preserves_owned_slope_water_nonzero_origin_and_grid_order() {
    let mut logic = world_with_sloped_terrain();
    let mut expected = Vec::new();
    for y in 0..128 {
        for x in 0..128 {
            let world = glam::Vec3::new(30.0 + x as f32 * 10.0, 0.0, -40.0 + y as f32 * 10.0);
            let height = logic.terrain_height_at(world).expect("owned height");
            let water = logic
                .terrain
                .as_ref()
                .unwrap()
                .is_underwater_at_world(world);
            expected.push((height, water));
        }
    }
    logic.host_radar_rescan_terrain();
    {
        let state = &logic.radar_map;
        assert_eq!(state.samples, expected);
        assert_eq!(
            state.min.z,
            expected.iter().map(|s| s.0).fold(f32::MAX, f32::min)
        );
        assert_eq!(
            state.max.z,
            expected.iter().map(|s| s.0).fold(f32::MIN, f32::max)
        );
    }
    let source = &logic.radar_map;
    let actual = source.sample_grid(Coord3D::new(30.0, -40.0, 0.0), 10.0, 10.0);
    assert_eq!(actual.len(), 128 * 128);
    for (index, (wx, height, water)) in actual.iter().enumerate() {
        assert_eq!(*wx, 30.0 + (index % 128) as f32 * 10.0);
        assert_eq!((*height, *water), expected[index]);
    }
    assert!(actual.iter().any(|s| s.2));
    assert!(actual.iter().any(|s| !s.2));
}

#[test]
#[ignore = "manual CPU benchmark, no timing assertion"]
#[cfg(feature = "game_client")]
fn benchmark_radar_sampling_cpu() {
    let mut logic = world_with_sloped_terrain();
    let mut rescan = Vec::new();
    let mut sample = Vec::new();
    for _ in 0..31 {
        let start = std::time::Instant::now();
        logic.host_radar_rescan_terrain();
        rescan.push(start.elapsed().as_micros());
        let start = std::time::Instant::now();
        std::hint::black_box(logic.radar_map.sample_grid(
            Coord3D::new(30.0, -40.0, 0.0),
            10.0,
            10.0,
        ));
        sample.push(start.elapsed().as_micros());
    }
    rescan.sort_unstable();
    sample.sort_unstable();
    eprintln!(
        "radar sampling median_us: rescan={}, source_grid={}",
        rescan[15], sample[15]
    );
}

#[test]
fn radar_sampling_empty_cache_has_no_ambient_terrain_dependency() {
    let logic = GameLogic::new();
    let grid = logic
        .radar_map
        .sample_grid(Coord3D::new(-10.0, 20.0, 0.0), 1.0, 2.0);
    assert_eq!(grid.len(), 128 * 128);
    assert!(
        grid.iter()
            .all(|(_, height, water)| *height == 0.0 && !*water)
    );
}

#[test]
fn radar_sampling_fallback_uses_driving_terrain_heightmap() {
    let mut logic = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 8;
    map.height = 8;
    map.heightmap = (0..64).collect();
    logic
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    store_radar_map_extent(&mut logic.radar_map, logic.world_min, logic.world_max).unwrap();
    logic.host_radar_rescan_terrain();
    let origin = host_to_radar_coord(logic.world_min);
    let grid = logic.radar_map.sample_grid(origin, 4.0, 4.0);
    let scalar =
        game_engine::common::system::radar::sample_radar_map_grid(origin, 4.0, 4.0, |x, y| {
            logic.radar_map.sample_cell(x, y)
        });
    assert_eq!(grid, scalar);
    assert!(grid.iter().all(|(_, height, _)| height.is_finite()));
    assert!(grid.iter().any(|(_, height, _)| *height > 0.0));
}

#[test]
#[cfg(feature = "game_client")]
fn radar_sampling_cached_grid_and_explicit_height_query_do_not_reenter_terrain_lock() {
    let mut logic = world_with_sloped_terrain();
    logic.host_radar_rescan_terrain();
    let expected = logic.radar_map.samples.clone();
    let terrain = gamelogic::terrain::TerrainLogic::new();
    let terrain_owner_handle = logic.world_services.terrain();
    let _writer = terrain_owner_handle.write().unwrap();
    let world = glam::Vec3::new(150.0, 0.0, 70.0);
    assert_eq!(
        logic.terrain_height_at_with_terrain_logic(world, Some(&terrain)),
        Some(logic.terrain.as_ref().unwrap().height_at_world(world))
    );
    let grid = logic
        .radar_map
        .sample_grid(Coord3D::new(30.0, -40.0, 0.0), 10.0, 10.0);
    assert_eq!(
        grid.into_iter().map(|(_, h, w)| (h, w)).collect::<Vec<_>>(),
        expected
    );
}
