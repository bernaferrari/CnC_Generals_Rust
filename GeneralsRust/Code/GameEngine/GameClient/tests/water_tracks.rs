use game_client_rust::terrain::{
    water_tracks::{
        decode_wak_records, encode_wak_records, water_track_strip_indices, water_track_wak_path,
        WaterTrackWakError,
    },
    WaterTrackHeightProvider, WaterTrackSaveRecord, WaterTrackType, WaterTracksRenderSystem,
    WATER_TRACK_WAVE_INFO,
};
use glam::Vec2;

struct FlatWater;

impl WaterTrackHeightProvider for FlatWater {
    fn water_height(&self, _x: f32, _y: f32) -> f32 {
        7.0
    }
}

fn approx_eq(a: f32, b: f32) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}

#[test]
fn wave_info_matches_cpp_table() {
    let ocean = WATER_TRACK_WAVE_INFO[WaterTrackType::Ocean as usize];
    assert_eq!(ocean.final_width, 55.0);
    assert_eq!(ocean.final_height, 36.0);
    assert_eq!(ocean.wave_distance, 80.0);
    assert_eq!(ocean.initial_velocity, 0.015);
    assert_eq!(ocean.fade_ms, 2000);
    assert_eq!(ocean.initial_width_fraction, 0.5);
    assert_eq!(ocean.initial_height_width_fraction, 0.18);
    assert_eq!(ocean.time_to_compress, 1000);
    assert_eq!(ocean.second_wave_time_offset, 6267);
    assert_eq!(ocean.texture_name, "wave256.tga");
    assert_eq!(ocean.wave_type_name, "Ocean");

    let stationary = WATER_TRACK_WAVE_INFO[WaterTrackType::Stationary as usize];
    assert_eq!(stationary.final_width, 0.0);
    assert_eq!(stationary.texture_name, "");
}

#[test]
fn bind_inserts_next_to_same_type_and_syncs_elapsed() {
    let mut system = WaterTracksRenderSystem::new(4);
    let pond = system.bind_track(WaterTrackType::Pond).unwrap();
    system
        .track_mut(pond)
        .unwrap()
        .init(18.0, 28.0, Vec2::ZERO, Vec2::Y, "wave256.tga", 123);
    system.track_mut(pond).unwrap().update(1000);

    let ocean = system.bind_track(WaterTrackType::Ocean).unwrap();
    let ocean2 = system.bind_track(WaterTrackType::Ocean).unwrap();

    assert_eq!(system.used_handles(), &[ocean2, ocean, pond]);
    assert_eq!(system.track(pond).unwrap().elapsed_ms(), 123.0);
}

#[test]
fn moving_wave_builds_cpp_quad_before_beach() {
    let water = FlatWater;
    let mut system = WaterTracksRenderSystem::new(1);
    let handle = system.bind_track(WaterTrackType::Pond).unwrap();
    system.track_mut(handle).unwrap().init(
        18.0,
        28.0,
        Vec2::new(10.0, 20.0),
        Vec2::new(10.0, 21.0),
        "wave256.tga",
        0,
    );

    let vertices = system.track_mut(handle).unwrap().build_vertices(&water);

    approx_eq(vertices[0].x, 9.86);
    approx_eq(vertices[0].y, -5.0504);
    approx_eq(vertices[0].z, 8.5);
    assert_eq!(vertices[0].diffuse, 0x00ff_ffff);
    assert_eq!(vertices[0].u1, 0.0);
    assert_eq!(vertices[1].u1, 1.0);
    assert_eq!(vertices[2].v1, 1.0);
}

#[test]
fn moving_wave_after_total_resets_elapsed_like_cpp_render() {
    let water = FlatWater;
    let mut system = WaterTracksRenderSystem::new(1);
    let handle = system.bind_track(WaterTrackType::Pond).unwrap();
    system.track_mut(handle).unwrap().init(
        18.0,
        28.0,
        Vec2::new(10.0, 20.0),
        Vec2::new(10.0, 21.0),
        "wave256.tga",
        0,
    );
    let total = system.track(handle).unwrap().total_ms() as i32;
    system.track_mut(handle).unwrap().update(total + 1);

    let vertices = system.track_mut(handle).unwrap().build_vertices(&water);

    assert_eq!(system.track(handle).unwrap().elapsed_ms(), 0.0);
    assert_eq!(vertices[0].diffuse, 0x00ff_ffff);
}

#[test]
fn total_ms_truncates_like_cpp_int_member() {
    let water = FlatWater;
    let mut system = WaterTracksRenderSystem::new(1);
    let handle = system.bind_track(WaterTrackType::Ocean).unwrap();
    system.track_mut(handle).unwrap().init(
        36.0,
        55.0,
        Vec2::new(0.0, 0.0),
        Vec2::new(0.0, 1.0),
        "wave256.tga",
        0,
    );

    assert_eq!(system.track(handle).unwrap().total_ms(), 12533.0);

    system.track_mut(handle).unwrap().update(12533);
    let _ = system.track_mut(handle).unwrap().build_vertices(&water);

    assert_eq!(system.track(handle).unwrap().elapsed_ms(), 0.0);
}

#[test]
fn flip_u_swaps_left_and_right_texture_coordinates() {
    let water = FlatWater;
    let mut system = WaterTracksRenderSystem::new(1);
    let handle = system.bind_track(WaterTrackType::Pond).unwrap();
    system.track_mut(handle).unwrap().init(
        18.0,
        28.0,
        Vec2::new(10.0, 20.0),
        Vec2::new(10.0, 21.0),
        "wave256.tga",
        0,
    );
    system.track_mut(handle).unwrap().set_flip_u(true);

    let vertices = system.track_mut(handle).unwrap().build_vertices(&water);

    assert_eq!(vertices[0].u1, 1.0);
    assert_eq!(vertices[1].u1, 0.0);
    assert_eq!(vertices[2].u1, 1.0);
    assert_eq!(vertices[3].u1, 0.0);
}

#[test]
fn unbind_releases_track_immediately_to_free_store() {
    let mut system = WaterTracksRenderSystem::new(1);
    let handle = system.bind_track(WaterTrackType::Pond).unwrap();
    assert_eq!(system.free_count(), 0);

    system.unbind_track(handle);

    assert!(system.used_handles().is_empty());
    assert_eq!(system.free_count(), 1);
    assert_eq!(system.track(handle).unwrap().texture_name(), "");
}

#[test]
fn load_records_adds_second_wave_and_save_skips_offsets() {
    let mut system = WaterTracksRenderSystem::new(4);
    let records = [WaterTrackSaveRecord {
        start: Vec2::new(1.0, 2.0),
        end: Vec2::new(1.0, 3.0),
        wave_type: WaterTrackType::Ocean,
    }];

    let handles = system.load_records(&records);

    assert_eq!(handles.len(), 2);
    assert_eq!(system.track(handles[0]).unwrap().init_time_offset(), 0);
    assert_eq!(system.track(handles[1]).unwrap().init_time_offset(), 6267);
    assert!(system.track(handles[0]).unwrap().flip_u());
    assert!(!system.track(handles[1]).unwrap().flip_u());
    assert_eq!(system.save_records(), records);
}

#[test]
fn wak_records_round_trip_cpp_trailing_count_format() {
    let records = [
        WaterTrackSaveRecord {
            start: Vec2::new(1.0, 2.0),
            end: Vec2::new(3.0, 4.0),
            wave_type: WaterTrackType::Pond,
        },
        WaterTrackSaveRecord {
            start: Vec2::new(-5.5, 6.25),
            end: Vec2::new(7.75, -8.125),
            wave_type: WaterTrackType::Radial,
        },
    ];

    let bytes = encode_wak_records(&records);

    assert_eq!(bytes.len(), 44);
    assert_eq!(&bytes[40..44], &2i32.to_le_bytes());
    assert_eq!(decode_wak_records(&bytes).unwrap(), records);
}

#[test]
fn wak_decoder_uses_eof_count_and_rejects_unknown_types() {
    let records = [WaterTrackSaveRecord {
        start: Vec2::new(1.0, 2.0),
        end: Vec2::new(3.0, 4.0),
        wave_type: WaterTrackType::Ocean,
    }];
    let mut bytes = encode_wak_records(&records);
    bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    bytes.extend_from_slice(&1i32.to_le_bytes());

    assert_eq!(decode_wak_records(&bytes).unwrap(), records);

    let mut bad = encode_wak_records(&records);
    bad[16..20].copy_from_slice(&99i32.to_le_bytes());
    assert_eq!(
        decode_wak_records(&bad),
        Err(WaterTrackWakError::UnknownWaveType(99))
    );
}

#[test]
fn wak_path_replaces_map_extension_like_cpp() {
    assert_eq!(
        water_track_wak_path("Data/Maps/Foo/Foo.map"),
        "Data/Maps/Foo/Foo.wak"
    );
    assert_eq!(water_track_wak_path("map"), ".wak");
}

#[test]
fn flush_uses_cpp_strip_indices_and_fixed_frame_update() {
    let water = FlatWater;
    let mut system = WaterTracksRenderSystem::new(1);
    let handle = system.bind_track(WaterTrackType::Pond).unwrap();
    system.track_mut(handle).unwrap().init(
        18.0,
        28.0,
        Vec2::new(10.0, 20.0),
        Vec2::new(10.0, 21.0),
        "wave256.tga",
        0,
    );

    let flush = system.flush(&water);

    assert_eq!(water_track_strip_indices(2, 2), vec![2, 0, 3, 1]);
    assert_eq!(flush.indices, vec![2, 0, 3, 1]);
    assert_eq!(flush.vertices.len(), 4);
    assert_eq!(flush.ranges[0].texture_name, "wave256.tga");
    assert_eq!(system.track(handle).unwrap().elapsed_ms(), 33.0);
}

#[test]
fn flush_is_suppressed_when_cpp_global_water_flags_disable_it() {
    let water = FlatWater;
    let mut system = WaterTracksRenderSystem::new(1);
    let handle = system.bind_track(WaterTrackType::Pond).unwrap();
    system.track_mut(handle).unwrap().init(
        18.0,
        28.0,
        Vec2::new(10.0, 20.0),
        Vec2::new(10.0, 21.0),
        "wave256.tga",
        0,
    );
    system.set_render_enabled(false, 1.0);

    assert!(system.flush(&water).vertices.is_empty());
}
