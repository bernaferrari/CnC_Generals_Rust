use super::*;

fn radar_on_flat_map() -> RadarSystem {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1280.0, 1280.0, 100.0),
        &[],
    );
    radar
}

#[test]
fn queued_refresh_repaints_once_after_strict_three_second_boundary() {
    let mut radar = radar_on_flat_map();
    radar.update(10);
    radar.create_event(
        &Coord3D::new(20.0, 30.0, 0.0),
        RadarEventType::Information,
        10.0,
    );
    radar.queue_terrain_refresh();
    let before = radar.terrain_generation();
    radar.update(100);
    assert_eq!(
        radar.terrain_generation(),
        before,
        "C++ delay is >90 frames"
    );
    radar.update(101);
    assert_eq!(radar.terrain_generation(), before + 1);
    for frame in 102..110 {
        radar.update(frame);
    }
    assert_eq!(
        radar.terrain_generation(),
        before + 1,
        "a consumed request must not repaint every frame"
    );
    assert_eq!(
        radar.get_active_events().len(),
        1,
        "refresh does not clear the event ring"
    );
}

#[test]
fn explicit_refresh_cancels_a_pending_delayed_refresh() {
    let mut radar = radar_on_flat_map();
    radar.update(10);
    radar.queue_terrain_refresh();
    radar.refresh_terrain();
    let after_explicit = radar.terrain_generation();
    radar.update(101);
    assert_eq!(radar.terrain_generation(), after_explicit);
}

#[test]
fn frame_zero_refresh_request_uses_cpp_inactive_sentinel() {
    let mut radar = radar_on_flat_map();
    radar.queue_terrain_refresh();
    let before = radar.terrain_generation();
    radar.update(91);
    assert_eq!(radar.terrain_generation(), before);
}

#[test]
fn later_refresh_request_restarts_delay_without_repeated_rebuilds() {
    let mut radar = radar_on_flat_map();
    radar.update(10);
    radar.queue_terrain_refresh();
    radar.update(80);
    radar.queue_terrain_refresh();
    let before = radar.terrain_generation();
    radar.update(101);
    radar.update(170);
    assert_eq!(radar.terrain_generation(), before);
    radar.update(171);
    assert_eq!(radar.terrain_generation(), before + 1);
    radar.update(172);
    assert_eq!(radar.terrain_generation(), before + 1);
}

#[test]
fn direct_texture_rebuild_does_not_consume_a_pending_refresh() {
    let mut radar = radar_on_flat_map();
    radar.update(10);
    radar.queue_terrain_refresh();
    radar.build_terrain_texture_cpp();
    let after_build = radar.terrain_generation();
    radar.update(101);
    assert_eq!(radar.terrain_generation(), after_build + 1);
}

#[test]
fn terrain_resample_preserves_cpp_new_map_averages_and_events() {
    struct ChangedTerrain;
    impl RadarMapSource for ChangedTerrain {
        fn map_extent(&self) -> Option<(Coord3D, Coord3D)> {
            None
        }
        fn sample_cell(&self, x: f32, _y: f32) -> Option<(f32, bool)> {
            Some((100.0, x >= 640.0))
        }
    }
    let mut initial = Vec::new();
    for y in 0..RADAR_CELL_HEIGHT {
        for x in 0..RADAR_CELL_WIDTH {
            let water = x >= 64;
            let height = if x % 2 == 0 && y % 2 == 0 {
                if water { 20.0 } else { 10.0 }
            } else {
                90.0
            };
            initial.push((x as f32 * 10.0, height, water));
        }
    }
    let mut radar = radar_on_flat_map();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1280.0, 1280.0, 100.0),
        &initial,
    );
    // Radar.cpp:331-369 samples every second row and column once at newMap.
    assert_eq!(radar.terrain_average_z, 10.0);
    assert_eq!(radar.water_average_z, 20.0);
    radar.update(10);
    radar.create_event(
        &Coord3D::new(40.0, 50.0, 0.0),
        RadarEventType::Construction,
        4.0,
    );
    assert!(radar.resample_terrain_with_source(&ChangedTerrain));
    assert!(radar.terrain_samples.iter().all(|s| s.height == 100.0));
    assert_eq!(
        radar.terrain_samples.iter().filter(|s| s.is_water).count(),
        128 * 64
    );
    assert_eq!(
        radar.terrain_average_z, 10.0,
        "refresh does not recompute the map shading midpoint"
    );
    assert_eq!(radar.water_average_z, 20.0);
    assert_eq!(radar.get_active_events().len(), 1);
    assert_eq!(radar.get_last_event_loc().unwrap().x, 40.0);
}

#[test]
fn resample_uses_one_source_grid_instead_of_scalar_callbacks() {
    struct BulkSource;
    impl RadarMapSource for BulkSource {
        fn map_extent(&self) -> Option<(Coord3D, Coord3D)> {
            None
        }
        fn sample_cell(&self, _x: f32, _y: f32) -> Option<(f32, bool)> {
            panic!("the bulk provider must not be sampled through per-cell callbacks");
        }
        fn sample_grid(&self, origin: Coord3D, xs: f32, ys: f32) -> Vec<(f32, f32, bool)> {
            sample_radar_map_grid(origin, xs, ys, |_, _| Some((30.0, false)))
        }
    }
    let mut radar = radar_on_flat_map();
    assert!(radar.resample_terrain_with_source(&BulkSource));
    assert_eq!(radar.terrain_samples.len(), 128 * 128);
    assert!(
        radar
            .terrain_samples
            .iter()
            .all(|s| s.height == 30.0 && !s.is_water)
    );
}

#[test]
fn default_grid_preserves_scalar_coordinates_missing_cells_and_row_order() {
    struct SparseSource;
    impl RadarMapSource for SparseSource {
        fn map_extent(&self) -> Option<(Coord3D, Coord3D)> {
            None
        }
        fn sample_cell(&self, x: f32, y: f32) -> Option<(f32, bool)> {
            if (x as i32 + y as i32) % 7 == 0 {
                None
            } else {
                Some((x - y, y < 17.0))
            }
        }
    }
    let source = SparseSource;
    let origin = Coord3D::new(-11.25, 7.5, 0.0);
    let grid = source.sample_grid(origin, 3.125, 9.75);
    assert_eq!(grid.len(), 128 * 128);
    for (i, &(wx, height, water)) in grid.iter().enumerate() {
        let x = origin.x + (i % 128) as f32 * 3.125;
        let y = origin.y + (i / 128) as f32 * 9.75;
        assert_eq!(wx, x);
        assert_eq!(
            (height, water),
            source.sample_cell(x, y).unwrap_or((0.0, false))
        );
    }
}
