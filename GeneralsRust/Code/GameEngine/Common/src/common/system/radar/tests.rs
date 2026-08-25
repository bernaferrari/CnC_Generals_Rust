use super::*;

#[test]
fn test_radar_system_creation() {
    let radar = RadarSystem::new();
    assert!(!radar.is_radar_hidden());
    assert!(!radar.is_radar_forced());
}

#[test]
fn test_is_temporarily_hidden_matches_cpp_stealth_look() {
    let mut own = RadarObject::new(1);
    own.is_stealth = true;
    own.is_enemy = false;
    own.stealth_revealed = false;
    assert!(
        !own.is_temporarily_hidden(),
        "own stealth is STEALTHLOOK_VISIBLE_FRIENDLY"
    );

    let mut undetected = RadarObject::new(2);
    undetected.is_stealth = true;
    undetected.is_enemy = true;
    undetected.stealth_revealed = false;
    assert!(
        undetected.is_temporarily_hidden(),
        "enemy undetected undisguised is STEALTHLOOK_INVISIBLE"
    );

    let mut detected = undetected.clone();
    detected.is_detected = true;
    assert!(
        !detected.is_temporarily_hidden(),
        "OBJECT_STATUS_DETECTED enemy blips"
    );

    let mut revealed = undetected.clone();
    revealed.stealth_revealed = true;
    assert!(
        !revealed.is_temporarily_hidden(),
        "detector-range stealth_revealed enemy blips"
    );

    let mut disguised = undetected.clone();
    disguised.is_disguised = true;
    assert!(!disguised.is_temporarily_hidden(), "DISGUISED_ENEMY blips");

    let mut hijacker = RadarObject::new(6);
    hijacker.is_stealth = false;
    hijacker.is_enemy = false;
    hijacker.drawable_hidden = true;
    assert!(
        hijacker.is_temporarily_hidden(),
        "C++ isDrawableEffectivelyHidden drops hijacker/script-hidden drawables"
    );

    let mut stealth_hidden = RadarObject::new(7);
    stealth_hidden.is_stealth = false;
    stealth_hidden.is_enemy = false;
    stealth_hidden.hidden_by_stealth = true;
    assert!(
        stealth_hidden.is_temporarily_hidden(),
        "C++ m_hiddenByStealth hides even without STEALTHLOOK_INVISIBLE"
    );
}

#[test]
fn test_radar_priority_visibility() {
    assert!(!RadarPriorityType::Invalid.is_visible());
    assert!(!RadarPriorityType::NotOnRadar.is_visible());
    assert!(RadarPriorityType::Structure.is_visible());
    assert!(RadarPriorityType::Unit.is_visible());
    assert!(RadarPriorityType::LocalUnitOnly.is_visible());
}

#[test]
fn test_set_shroud_level_from_partition_cell_maps_to_radar() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1280.0, 1280.0, 100.0),
        &[],
    );
    // 40wu partition cell / (1280/128=10) = 4 radar pixels.
    radar.set_shroud_level_from_partition_cell(0, 0, CellShroudStatus::Clear, 40.0, 40.0);
    assert_eq!(radar.get_shroud_level(0, 0), CellShroudStatus::Clear);
    assert_eq!(radar.get_shroud_level(3, 3), CellShroudStatus::Clear);
    radar.set_shroud_level_from_partition_cell(10, 10, CellShroudStatus::Fogged, 40.0, 40.0);
    assert_eq!(radar.get_shroud_level(40, 40), CellShroudStatus::Fogged);
    assert_ne!(
        radar.get_shroud_level(10, 10),
        CellShroudStatus::Fogged,
        "partition cell 10 is not radar cell 10"
    );
}

#[test]
fn test_world_to_radar_conversion() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let world = Coord3D::new(512.0, 512.0, 0.0);
    let radar_pos = radar.world_to_radar(&world).unwrap();
    assert_eq!(radar_pos.x, 64); // Middle of 128x128 radar
    assert_eq!(radar_pos.y, 64);
}

#[test]
fn test_radar_to_world_conversion() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let radar_pos = ICoord2D::new(64, 64);
    let world = radar.radar_to_world(&radar_pos).unwrap();
    assert!((world.x - 512.0).abs() < 1.0);
    assert!((world.y - 512.0).abs() < 1.0);
}

#[test]
fn test_new_map_builds_initial_terrain_texture_like_w3d() {
    let mut radar = RadarSystem::new();

    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    assert!(!radar.is_terrain_dirty());
    assert!(
        radar
            .get_terrain_texture()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 255)
    );
}

#[test]
fn test_build_terrain_texture_uses_cpp_call_site_height_order() {
    // C++ W3DRadar.cpp:1177 passes (z, getTerrainAverageZ(), mapHi.z, mapLo.z).
    // Retail therefore darkens from map-max, not around the average.
    let mut radar = RadarSystem::new();
    let samples: Vec<(f32, f32, bool)> = (0..(RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize)
        .map(|_| (0.0, 50.0, false))
        .collect();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &samples,
    );

    let tan = [0.45, 0.42, 0.32];
    let retail = interpolate_color_for_height(tan, 50.0, 50.0, 100.0, 0.0);
    let old_leftover = interpolate_color_for_height(tan, 50.0, 100.0, 50.0, 0.0);
    let px = &radar.get_terrain_texture()[0..3];
    let got = [
        px[0] as f32 / 255.0,
        px[1] as f32 / 255.0,
        px[2] as f32 / 255.0,
    ];
    assert!(
        (got[0] - retail[0]).abs() < 0.01
            && (got[1] - retail[1]).abs() < 0.01
            && (got[2] - retail[2]).abs() < 0.01,
        "expected C++ (avg, hi, lo) shade {retail:?}, got {got:?}"
    );
    assert!(
        (got[0] - old_leftover[0]).abs() > 0.02,
        "must not use leftover (hi, avg, lo) order {old_leftover:?}"
    );
}

#[test]
fn test_legal_radar_point_matches_w3d_bounds() {
    assert!(legal_radar_point(0, 0));
    assert!(legal_radar_point(
        RADAR_CELL_WIDTH as i32 - 1,
        RADAR_CELL_HEIGHT as i32 - 1
    ));
    assert!(!legal_radar_point(-1, 0));
    assert!(!legal_radar_point(0, -1));
    assert!(!legal_radar_point(RADAR_CELL_WIDTH as i32, 0));
    assert!(!legal_radar_point(0, RADAR_CELL_HEIGHT as i32));
}

#[test]
fn test_radar_to_pixel_matches_w3d_inverted_y_mapping() {
    let upper_left_x = 10;
    let upper_left_y = 20;
    let width = 256;
    let height = 128;

    assert_eq!(
        radar_to_pixel(
            &ICoord2D::new(0, 0),
            upper_left_x,
            upper_left_y,
            width,
            height
        ),
        ICoord2D::new(10, 147)
    );
    assert_eq!(
        radar_to_pixel(
            &ICoord2D::new(127, 127),
            upper_left_x,
            upper_left_y,
            width,
            height
        ),
        ICoord2D::new(264, 20)
    );
    assert_eq!(
        radar_to_pixel(
            &ICoord2D::new(64, 64),
            upper_left_x,
            upper_left_y,
            width,
            height
        ),
        ICoord2D::new(138, 83)
    );
}

#[test]
fn test_radar_draw_positions_letterboxes_wide_maps() {
    let extent = Region3D {
        lo: Coord3D::new(0.0, 0.0, 0.0),
        hi: Coord3D::new(2000.0, 1000.0, 0.0),
    };

    let (ul, lr) = radar_draw_positions(10, 20, 100, 100, extent);

    assert_eq!(ul, ICoord2D::new(10, 45));
    assert_eq!(lr, ICoord2D::new(110, 95));
}

#[test]
fn test_radar_draw_positions_pillarboxes_tall_maps() {
    let extent = Region3D {
        lo: Coord3D::new(0.0, 0.0, 0.0),
        hi: Coord3D::new(1000.0, 2000.0, 0.0),
    };

    let (ul, lr) = radar_draw_positions(10, 20, 100, 100, extent);

    assert_eq!(ul, ICoord2D::new(35, 20));
    assert_eq!(lr, ICoord2D::new(85, 120));
}

#[test]
fn test_interpolate_color_for_height_matches_w3d_lighting() {
    let base = [0.2, 0.4, 0.6];

    let lighter = interpolate_color_for_height(base, 75.0, 100.0, 50.0, 0.0);
    assert!((lighter[0] - 0.58).abs() < 0.0001);
    assert!((lighter[1] - 0.685).abs() < 0.0001);
    assert!((lighter[2] - 0.79).abs() < 0.0001);

    let darker = interpolate_color_for_height(base, 25.0, 100.0, 50.0, 0.0);
    assert!((darker[0] - 0.14).abs() < 0.0001);
    assert!((darker[1] - 0.28).abs() < 0.0001);
    assert!((darker[2] - 0.42).abs() < 0.0001);
}

#[test]
fn test_interpolate_color_for_height_handles_flat_w3d_ranges() {
    let color = interpolate_color_for_height([0.5, 0.5, 0.5], 10.0, 10.0, 10.0, 10.0);
    assert!(color.iter().all(|channel| channel.is_finite()));
    assert!(color.iter().all(|channel| (0.0..=1.0).contains(channel)));
}

#[test]
fn test_generic_radar_event_marker_matches_w3d_triangle_at_create_frame() {
    let event = RadarEvent {
        active: true,
        create_frame: 10,
        die_frame: 100,
        fade_frame: 90,
        color1: RGBAColorInt::new(255, 0, 0, 200),
        color2: RGBAColorInt::new(255, 255, 0, 180),
        radar_loc: ICoord2D::new(64, 64),
        ..RadarEvent::default()
    };

    let marker = radar_event_marker(&event, 10, 10, 20, 100, 100, RadarEventMarkerKind::Generic);

    assert_eq!(marker.size, 50);
    assert_eq!(
        marker.points,
        [
            ICoord2D::new(99, 69),
            ICoord2D::new(39, 35),
            ICoord2D::new(39, 103),
        ]
    );
    assert_eq!(marker.color1.a, 200);
    assert_eq!(marker.color2.a, 180);
}

#[test]
fn test_beacon_radar_event_marker_matches_w3d_size_and_fade() {
    let event = RadarEvent {
        active: true,
        create_frame: 10,
        die_frame: 100,
        fade_frame: 90,
        color1: RGBAColorInt::new(255, 0, 0, 200),
        color2: RGBAColorInt::new(255, 255, 0, 180),
        radar_loc: ICoord2D::new(64, 64),
        ..RadarEvent::default()
    };

    let marker = radar_event_marker(&event, 95, 10, 20, 100, 100, RadarEventMarkerKind::Beacon);

    assert_eq!(marker.size, 6);
    assert_eq!(marker.color1.a, 100);
    assert_eq!(marker.color2.a, 90);
}

#[test]
fn test_world_to_radar_respects_nonzero_map_origin() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(256.0, 128.0, 0.0),
        Coord3D::new(1280.0, 1152.0, 100.0),
        &[],
    );

    let world = Coord3D::new(768.0, 640.0, 0.0);
    let radar_pos = radar.world_to_radar(&world).unwrap();
    assert_eq!(radar_pos.x, 64);
    assert_eq!(radar_pos.y, 64);
}

#[test]
fn test_radar_to_world_uses_sampled_cell_height_when_available() {
    let mut radar = RadarSystem::new();
    let mut terrain = Vec::with_capacity((RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize);
    for _ in 0..(RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) {
        terrain.push((0.0, 10.0, false));
    }
    let target_x = 64u32;
    let target_y = 64u32;
    let idx = (target_y * RADAR_CELL_WIDTH + target_x) as usize;
    terrain[idx] = (0.0, 77.0, false);

    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &terrain,
    );

    let world = radar
        .radar_to_world(&ICoord2D::new(target_x as i32, target_y as i32))
        .unwrap();
    assert!((world.z - 77.0).abs() < f32::EPSILON);
}

#[test]
fn test_radar_event_creation() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let world_loc = Coord3D::new(100.0, 100.0, 0.0);
    radar.create_event(&world_loc, RadarEventType::UnderAttack, 4.0);

    let active_events = radar.get_active_events();
    assert_eq!(active_events.len(), 1);
    assert_eq!(active_events[0].event_type, RadarEventType::UnderAttack);
}

#[test]
fn last_event_loc_skips_beacon_pulse_like_cpp() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let attack = Coord3D::new(111.0, 222.0, 7.0);
    radar.create_event(&attack, RadarEventType::UnderAttack, 4.0);
    radar.create_event(
        &Coord3D::new(9.0, 8.0, 1.0),
        RadarEventType::BeaconPulse,
        0.5,
    );

    let last = radar.get_last_event_loc().expect("last real event");
    assert!((last.x - 111.0).abs() < f32::EPSILON);
    assert!((last.y - 222.0).abs() < f32::EPSILON);
    assert!((last.z - 7.0).abs() < f32::EPSILON);
}

#[test]
fn test_radar_event_expiration() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let world_loc = Coord3D::new(100.0, 100.0, 0.0);
    radar.create_event(&world_loc, RadarEventType::Information, 1.0);

    // Event should be active initially
    assert_eq!(radar.get_active_events().len(), 1);

    // Update past expiration (1 second = 30 frames)
    radar.update(35);
    assert_eq!(radar.get_active_events().len(), 0);
}

#[test]
fn queued_terrain_refresh_ignores_earlier_frame_without_underflow() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );
    radar.refresh_terrain();
    assert!(!radar.is_terrain_dirty());

    radar.update(100);
    radar.queue_terrain_refresh();
    radar.update(50);

    assert!(!radar.is_terrain_dirty());
}

#[test]
fn test_add_remove_object() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let mut obj = RadarObject::new(1);
    obj.color = 0xFF0000FF;
    obj.world_pos = Coord3D::new(100.0, 100.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.is_local = false;

    radar.add_object(obj);
    assert_eq!(radar.object_list.len(), 1);

    radar.remove_object(1);
    assert_eq!(radar.object_list.len(), 0);
}

#[test]
fn test_remove_object_searches_local_and_regular_lists() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let mut local = RadarObject::new(1);
    local.world_pos = Coord3D::new(100.0, 100.0, 0.0);
    local.priority = RadarPriorityType::Unit;
    local.is_local = true;
    radar.add_object(local);

    let mut regular = RadarObject::new(2);
    regular.world_pos = Coord3D::new(200.0, 200.0, 0.0);
    regular.priority = RadarPriorityType::Unit;
    radar.add_object(regular);

    assert!(radar.remove_object(2));
    assert_eq!(radar.local_object_list.len(), 1);
    assert_eq!(radar.object_list.len(), 0);

    assert!(radar.remove_object(1));
    assert_eq!(radar.local_object_list.len(), 0);
    assert_eq!(radar.object_list.len(), 0);
}

#[test]
fn test_remove_object_survives_priority_insert_shifts() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let mut unit = RadarObject::new(1);
    unit.world_pos = Coord3D::new(100.0, 100.0, 0.0);
    unit.priority = RadarPriorityType::Unit;
    radar.add_object(unit);

    let mut structure = RadarObject::new(2);
    structure.world_pos = Coord3D::new(200.0, 200.0, 0.0);
    structure.priority = RadarPriorityType::Structure;
    radar.add_object(structure);

    assert_eq!(radar.object_list[0].object_id, 2);
    assert_eq!(radar.object_list[1].object_id, 1);

    assert!(radar.remove_object(1));
    assert_eq!(radar.object_list.len(), 1);
    assert_eq!(radar.object_list[0].object_id, 2);
}

#[test]
fn test_event_colors() {
    let (color1, _color2) = RadarEventType::UnderAttack.get_colors();
    assert_eq!(color1.r, 255);
    assert_eq!(color1.g, 0);
    assert_eq!(color1.b, 0);
}

#[test]
fn test_shroud_management() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Initially all shrouded
    assert_eq!(radar.get_shroud_level(64, 64), CellShroudStatus::Shrouded);

    // Set to clear
    radar.set_shroud_level(64, 64, CellShroudStatus::Clear);
    assert_eq!(radar.get_shroud_level(64, 64), CellShroudStatus::Clear);

    // Clear all shroud
    radar.clear_shroud();
    assert_eq!(radar.get_shroud_level(64, 64), CellShroudStatus::Clear);
    assert!(radar.is_shroud_cleared());
}

#[test]
fn test_shroud_texture_matches_w3d_alpha_levels() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );
    radar.set_shroud_level(0, 0, CellShroudStatus::Clear);
    radar.set_shroud_level(1, 0, CellShroudStatus::Fogged);
    radar.set_shroud_level(2, 0, CellShroudStatus::Shrouded);

    let texture = radar.build_shroud_texture_rgba();

    assert_eq!(
        texture.len(),
        (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT * 4) as usize
    );
    assert_eq!(&texture[0..4], &[0, 0, 0, 0]);
    assert_eq!(&texture[4..8], &[0, 0, 0, 127]);
    assert_eq!(&texture[8..12], &[0, 0, 0, 255]);
}

#[test]
fn test_object_overlay_texture_matches_w3d_2x2_blip_shape() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();
    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.color = 0xAA112233;
    radar.add_object(obj);

    let texture = radar.build_object_overlay_texture_rgba();
    let pixel = |x: u32, y: u32| -> &[u8] {
        let idx = ((y * RADAR_CELL_WIDTH + x) * 4) as usize;
        &texture[idx..idx + 4]
    };

    assert_eq!(pixel(10, 20), &[0x11, 0x22, 0x33, 0xAA]);
    assert_eq!(pixel(10, 21), &[0x11, 0x22, 0x33, 0xAA]);
    assert_eq!(pixel(11, 21), &[0x11, 0x22, 0x33, 0xAA]);
    assert_eq!(pixel(11, 20), &[0x11, 0x22, 0x33, 0xAA]);
    assert_eq!(pixel(9, 20), &[0, 0, 0, 0]);
}

#[test]
fn test_object_overlay_texture_skips_fogged_and_nonlocal_local_only_blips() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut fogged = RadarObject::new(1);
    fogged.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    fogged.priority = RadarPriorityType::Unit;
    fogged.color = 0xFFFF0000;
    radar.add_object(fogged);
    radar.set_shroud_level(10, 20, CellShroudStatus::Fogged);

    let mut nonlocal_local_only = RadarObject::new(2);
    nonlocal_local_only.world_pos = Coord3D::new(30.0, 40.0, 0.0);
    nonlocal_local_only.priority = RadarPriorityType::LocalUnitOnly;
    nonlocal_local_only.color = 0xFF00FF00;
    nonlocal_local_only.is_local = false;
    radar.add_object(nonlocal_local_only);

    let mut local_only = RadarObject::new(3);
    local_only.world_pos = Coord3D::new(50.0, 60.0, 0.0);
    local_only.priority = RadarPriorityType::LocalUnitOnly;
    local_only.color = 0xFF0000FF;
    local_only.is_local = true;
    radar.add_object(local_only);

    let texture = radar.build_object_overlay_texture_rgba();
    let pixel = |x: u32, y: u32| -> &[u8] {
        let idx = ((y * RADAR_CELL_WIDTH + x) * 4) as usize;
        &texture[idx..idx + 4]
    };

    assert_eq!(pixel(10, 20), &[0, 0, 0, 0]);
    assert_eq!(pixel(30, 40), &[0, 0, 0, 0]);
    assert_eq!(pixel(50, 60), &[0, 0x00, 0xFF, 0xFF]);
}

#[test]
fn test_object_overlay_texture_twinkles_revealed_stealth_blips() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.color = 0xFF112233;
    obj.is_stealth = true;
    obj.stealth_revealed = true;
    radar.add_object(obj);

    let pixel_at_frame = |frame| {
        let texture = radar.build_object_overlay_texture_rgba_at_frame(frame);
        let idx = ((20 * RADAR_CELL_WIDTH + 10) * 4) as usize;
        texture[idx..idx + 4].to_vec()
    };

    assert_eq!(pixel_at_frame(0), vec![0x11, 0x22, 0x33, 32]);
    assert_eq!(pixel_at_frame(15), vec![0x11, 0x22, 0x33, 32]);
    assert_eq!(pixel_at_frame(29), vec![0x11, 0x22, 0x33, 240]);
}

#[test]
fn test_object_overlay_texture_skips_enemy_undetected_undisguised_stealth_blip() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.color = 0xFF112233;
    obj.is_stealth = true;
    obj.stealth_revealed = false;
    obj.is_enemy = true;
    radar.add_object(obj);

    let texture = radar.build_object_overlay_texture_rgba_at_frame(0);
    let idx = ((20 * RADAR_CELL_WIDTH + 10) * 4) as usize;
    assert_eq!(&texture[idx..idx + 4], &[0, 0, 0, 0]);
}

#[test]
fn test_object_overlay_texture_draws_detected_or_disguised_enemy_stealth_blips() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut detected = RadarObject::new(1);
    detected.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    detected.priority = RadarPriorityType::Unit;
    detected.color = 0xFF112233;
    detected.is_stealth = true;
    detected.stealth_revealed = true;
    detected.is_enemy = true;
    detected.is_detected = true;
    radar.add_object(detected);

    let mut disguised = RadarObject::new(2);
    disguised.world_pos = Coord3D::new(30.0, 40.0, 0.0);
    disguised.priority = RadarPriorityType::Unit;
    disguised.color = 0xFF445566;
    disguised.is_stealth = true;
    disguised.stealth_revealed = true;
    disguised.is_enemy = true;
    disguised.is_disguised = true;
    radar.add_object(disguised);

    let texture = radar.build_object_overlay_texture_rgba_at_frame(0);
    let pixel = |x: u32, y: u32| -> &[u8] {
        let idx = ((y * RADAR_CELL_WIDTH + x) * 4) as usize;
        &texture[idx..idx + 4]
    };

    assert_eq!(pixel(10, 20), &[0x11, 0x22, 0x33, 32]);
    assert_eq!(pixel(30, 40), &[0x44, 0x55, 0x66, 32]);
}

#[test]
fn test_object_overlay_texture_draws_own_unrevealed_stealth_blip() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.color = 0xFF112233;
    obj.is_stealth = true;
    obj.stealth_revealed = false;
    obj.is_enemy = false;
    radar.add_object(obj);

    let texture = radar.build_object_overlay_texture_rgba_at_frame(0);
    let idx = ((20 * RADAR_CELL_WIDTH + 10) * 4) as usize;
    assert_eq!(&texture[idx..idx + 4], &[0x11, 0x22, 0x33, 32]);
}

#[test]
fn test_object_overlay_texture_draws_detector_revealed_enemy_stealth_blip() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.color = 0xFF112233;
    obj.is_stealth = true;
    obj.is_enemy = true;
    obj.is_detected = false;
    obj.is_disguised = false;
    obj.stealth_revealed = true;
    radar.add_object(obj);

    let texture = radar.build_object_overlay_texture_rgba_at_frame(0);
    let idx = ((20 * RADAR_CELL_WIDTH + 10) * 4) as usize;
    assert_eq!(&texture[idx..idx + 4], &[0x11, 0x22, 0x33, 32]);
}

#[test]
fn test_object_overlay_texture_draws_non_enemy_revealed_stealth_blip() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.color = 0xFF112233;
    obj.is_stealth = true;
    obj.stealth_revealed = true;
    radar.add_object(obj);

    let texture = radar.build_object_overlay_texture_rgba_at_frame(0);
    let idx = ((20 * RADAR_CELL_WIDTH + 10) * 4) as usize;
    assert_eq!(&texture[idx..idx + 4], &[0x11, 0x22, 0x33, 32]);
}

#[test]
fn test_w3d_object_overlay_refresh_cadence_matches_cpp() {
    assert!(should_refresh_w3d_object_overlay(0));
    assert!(!should_refresh_w3d_object_overlay(1));
    assert!(!should_refresh_w3d_object_overlay(5));
    assert!(should_refresh_w3d_object_overlay(6));
    assert!(should_refresh_w3d_object_overlay(12));
    assert!(!should_refresh_w3d_object_overlay(17));
}

#[test]
fn test_draw_events_chirp_requires_radar_online() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.create_event(
        &Coord3D::new(10.0, 10.0, 0.0),
        RadarEventType::UnderAttack,
        4.0,
    );

    // C++ `W3DRadar::drawEvents` is only reached after the hasRadar gate.
    // Leftover `Radar::update` must not consume the chirp while offline.
    radar.update(1);
    let offline = radar.draw_events();
    assert!(
        offline.iter().any(|e| e.active && !e.sound_played),
        "RadarEvent chirp must wait until the radar is on screen"
    );

    radar.set_local_has_radar(true);
    let online = radar.draw_events();
    assert!(
        online.iter().any(|e| e.active && e.sound_played),
        "RadarEvent chirp plays on the first visible drawEvents frame"
    );
}

#[test]
fn test_object_overlay_draws_partial_clear_fog_edge_blips() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    obj.color = 0xFFFF0000;
    obj.object_shroud = ObjectShroudStatus::PartialClear;
    radar.add_object(obj);
    // Cell fog must not drop an object whose getShroudedStatus is PARTIAL_CLEAR.
    radar.set_shroud_level(10, 20, CellShroudStatus::Fogged);

    let texture = radar.build_object_overlay_texture_rgba();
    let idx = ((20 * RADAR_CELL_WIDTH + 10) * 4) as usize;
    assert_eq!(
        &texture[idx..idx + 4],
        &[0xFF, 0x00, 0x00, 0xFF],
        "C++ skip only when getShroudedStatus > OBJECTSHROUD_PARTIAL_CLEAR"
    );
}

#[test]
fn test_object_overlay_unhides_local_unit_only_for_defeated_or_observer() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut nonlocal = RadarObject::new(2);
    nonlocal.world_pos = Coord3D::new(30.0, 40.0, 0.0);
    nonlocal.priority = RadarPriorityType::LocalUnitOnly;
    nonlocal.color = 0xFF00FF00;
    nonlocal.is_local = false;
    radar.add_object(nonlocal);

    let pixel = |radar: &RadarSystem| {
        let texture = radar.build_object_overlay_texture_rgba();
        let idx = ((40 * RADAR_CELL_WIDTH + 30) * 4) as usize;
        texture[idx..idx + 4].to_vec()
    };

    radar.set_local_player_active(true);
    assert_eq!(
        pixel(&radar),
        vec![0, 0, 0, 0],
        "active local still skips non-local LOCAL_UNIT_ONLY"
    );

    // C++ `isPlayerActive` exemption: observers / defeated see every blip.
    radar.set_local_player_active(false);
    assert_eq!(
        pixel(&radar),
        vec![0x00, 0xFF, 0x00, 0xFF],
        "defeated/observer local must unhide LOCAL_UNIT_ONLY"
    );
}

#[test]
fn test_object_overlay_unhides_enemy_stealth_for_defeated_or_observer() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut stealthed = RadarObject::new(3);
    stealthed.world_pos = Coord3D::new(12.0, 18.0, 0.0);
    stealthed.priority = RadarPriorityType::Unit;
    stealthed.color = 0xFFFF0000;
    stealthed.is_stealth = true;
    stealthed.is_enemy = true;
    stealthed.is_detected = false;
    stealthed.is_disguised = false;
    stealthed.stealth_revealed = false;
    stealthed.hidden_by_stealth = false;
    radar.add_object(stealthed);

    let pixel = |radar: &RadarSystem| {
        let texture = radar.build_object_overlay_texture_rgba();
        let idx = ((18 * RADAR_CELL_WIDTH + 12) * 4) as usize;
        texture[idx..idx + 4].to_vec()
    };

    radar.set_local_player_active(true);
    assert_eq!(
        pixel(&radar),
        vec![0, 0, 0, 0],
        "active local still hides undetected enemy stealth"
    );

    radar.set_local_player_active(false);
    assert_eq!(
        pixel(&radar),
        vec![0xFF, 0x00, 0x00, 0xFF],
        "defeated/observer local must see enemy stealth as VISIBLE_FRIENDLY"
    );
}

#[test]
fn test_hero_reticle_rects_match_w3d_icon_positioning() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );

    let mut hero = RadarObject::new(1);
    hero.world_pos = Coord3D::new(64.0, 64.0, 0.0);
    hero.priority = RadarPriorityType::Unit;
    hero.is_local = true;
    hero.is_hero = true;
    radar.add_object(hero);

    let rects = radar.build_hero_reticle_rects(10, 20, 128, 128, 20, 10);

    assert_eq!(
        rects,
        vec![RadarHeroReticleRect {
            x1: 65,
            y1: 78,
            x2: 85,
            y2: 88,
        }]
    );
}

#[test]
fn test_cached_hero_reticle_ids_use_current_position_like_w3d_pointers() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );

    let mut hero = RadarObject::new(1);
    hero.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    hero.priority = RadarPriorityType::Unit;
    hero.is_local = true;
    hero.is_hero = true;
    radar.add_object(hero.clone());

    let hero_object_ids = radar.build_hero_reticle_object_ids();
    hero.world_pos = Coord3D::new(64.0, 64.0, 0.0);
    radar.examine_object(1, hero);

    let rects =
        radar.build_hero_reticle_rects_for_objects(&hero_object_ids, 10, 20, 128, 128, 20, 10);

    assert_eq!(rects[0].x1, 65);
    assert_eq!(rects[0].y1, 78);
}

#[test]
fn test_hero_reticle_rects_only_use_visible_local_heroes() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );

    let mut local_hero = RadarObject::new(1);
    local_hero.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    local_hero.priority = RadarPriorityType::Unit;
    local_hero.is_local = true;
    local_hero.is_hero = true;
    radar.add_object(local_hero);

    let mut nonlocal_hero = RadarObject::new(2);
    nonlocal_hero.world_pos = Coord3D::new(30.0, 40.0, 0.0);
    nonlocal_hero.priority = RadarPriorityType::Unit;
    nonlocal_hero.is_hero = true;
    radar.add_object(nonlocal_hero);

    let mut hidden_local_hero = RadarObject::new(3);
    hidden_local_hero.world_pos = Coord3D::new(50.0, 60.0, 0.0);
    hidden_local_hero.priority = RadarPriorityType::Unit;
    hidden_local_hero.is_local = true;
    hidden_local_hero.is_hero = true;
    hidden_local_hero.is_stealth = true;
    hidden_local_hero.stealth_revealed = false;
    hidden_local_hero.is_enemy = true;
    radar.add_object(hidden_local_hero);

    let rects = radar.build_hero_reticle_rects(0, 0, 128, 128, 20, 10);

    assert_eq!(rects.len(), 1);
}

#[test]
fn test_view_box_lines_match_w3d_radar_conversion_and_colors() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );

    let lines = radar.build_view_box_lines(
        Coord3D::new(32.0, 32.0, 0.0),
        [
            Coord3D::new(32.0, 32.0, 0.0),
            Coord3D::new(96.0, 32.0, 0.0),
            Coord3D::new(96.0, 96.0, 0.0),
            Coord3D::new(32.0, 96.0, 0.0),
        ],
        0,
        0,
        128,
        128,
        0,
        0,
        128,
        128,
    );

    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].start, ICoord2D::new(32, 95));
    assert_eq!(lines[0].end, ICoord2D::new(96, 95));
    assert_eq!(lines[0].start_color, RGBAColorInt::new(225, 225, 0, 255));
    assert_eq!(lines[0].end_color, RGBAColorInt::new(225, 225, 0, 255));
    assert_eq!(lines[1].start, ICoord2D::new(96, 95));
    assert_eq!(lines[1].end, ICoord2D::new(96, 31));
    assert_eq!(lines[1].start_color, RGBAColorInt::new(225, 225, 0, 255));
    assert_eq!(lines[1].end_color, RGBAColorInt::new(158, 158, 0, 255));
    assert_eq!(lines[2].start, ICoord2D::new(96, 31));
    assert_eq!(lines[2].end, ICoord2D::new(32, 31));
    assert_eq!(lines[3].start, ICoord2D::new(32, 31));
    assert_eq!(lines[3].end, ICoord2D::new(32, 95));
}

#[test]
fn test_view_box_lines_clip_to_full_radar_window() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );

    let lines = radar.build_view_box_lines(
        Coord3D::new(-16.0, 64.0, 0.0),
        [
            Coord3D::new(-16.0, 64.0, 0.0),
            Coord3D::new(16.0, 64.0, 0.0),
            Coord3D::new(16.0, 96.0, 0.0),
            Coord3D::new(-16.0, 96.0, 0.0),
        ],
        0,
        0,
        128,
        128,
        0,
        0,
        128,
        128,
    );

    assert!(lines.iter().all(|line| {
        line.start.x >= 0
            && line.end.x >= 0
            && line.start.y >= 0
            && line.end.y >= 0
            && line.start.x <= 128
            && line.end.x <= 128
            && line.start.y <= 128
            && line.end.y <= 128
    }));
}

#[test]
fn test_shroud_circle() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let center = Coord3D::new(512.0, 512.0, 0.0);
    let radius = 100.0;

    radar.set_shroud_circle(&center, radius, CellShroudStatus::Clear);

    // Center should be clear
    let center_radar = radar.world_to_radar(&center).unwrap();
    assert_eq!(
        radar.get_shroud_level(center_radar.x, center_radar.y),
        CellShroudStatus::Clear
    );
}

#[test]
fn test_stealth_detection() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Add stealth detection radar
    let mut detector = RadarObject::new(1);
    detector.color = 0xFFFFFFFF;
    detector.world_pos = Coord3D::new(500.0, 500.0, 0.0);
    detector.priority = RadarPriorityType::Structure;
    detector.radar_range = 200.0;
    detector.can_detect_stealth = true;
    radar.add_object(detector);

    // Add stealth unit nearby
    let mut stealth_unit = RadarObject::new(2);
    stealth_unit.color = 0xFF0000FF;
    stealth_unit.world_pos = Coord3D::new(550.0, 550.0, 0.0);
    stealth_unit.priority = RadarPriorityType::Unit;
    stealth_unit.is_stealth = true;
    radar.add_object(stealth_unit);

    // Update stealth detection
    radar.update_stealth_detection();

    // Verify stealth unit is revealed
    let revealed_count = radar
        .get_all_objects()
        .filter(|obj| obj.is_stealth && obj.stealth_revealed)
        .count();
    assert_eq!(revealed_count, 1);
}

#[test]
fn test_radar_range_detection() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Add radar with range
    let mut radar_obj = RadarObject::new(1);
    radar_obj.color = 0xFFFFFFFF;
    radar_obj.world_pos = Coord3D::new(500.0, 500.0, 0.0);
    radar_obj.priority = RadarPriorityType::Structure;
    radar_obj.radar_range = 100.0;
    radar.add_object(radar_obj);

    // Position within range
    let in_range = Coord3D::new(550.0, 550.0, 0.0);
    assert!(radar.is_position_in_radar_range(&in_range, 0));

    // Position out of range
    let out_of_range = Coord3D::new(700.0, 700.0, 0.0);
    assert!(!radar.is_position_in_radar_range(&out_of_range, 0));
}

#[test]
fn test_stealth_events() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let world_loc = Coord3D::new(100.0, 100.0, 0.0);

    // First event should succeed
    assert!(radar.try_stealth_discovered_event(&world_loc));

    // Second event at same location should fail (too soon)
    assert!(!radar.try_stealth_discovered_event(&world_loc));

    // C++ precedence quirk: far-away same-type events still throttle for 10s.
    let far_loc = Coord3D::new(500.0, 500.0, 0.0);
    assert!(!radar.try_stealth_discovered_event(&far_loc));
}

#[test]
fn test_visible_objects_filtering() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Add visible object
    let mut visible = RadarObject::new(1);
    visible.world_pos = Coord3D::new(100.0, 100.0, 0.0);
    visible.priority = RadarPriorityType::Unit;
    radar.add_object(visible);

    // Add hidden stealth object
    let mut hidden = RadarObject::new(2);
    hidden.world_pos = Coord3D::new(200.0, 200.0, 0.0);
    hidden.priority = RadarPriorityType::Unit;
    hidden.is_stealth = true;
    radar.add_object(hidden);

    // Should only get visible objects
    assert_eq!(radar.get_visible_objects().len(), 1);
    assert_eq!(radar.get_all_objects().count(), 2);
}

#[test]
fn test_refresh_terrain_uses_sampled_height_gradient() {
    let mut radar = RadarSystem::new();
    let sample_count = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
    let mut samples = Vec::with_capacity(sample_count);
    for y in 0..RADAR_CELL_HEIGHT {
        for x in 0..RADAR_CELL_WIDTH {
            let height = x as f32 / (RADAR_CELL_WIDTH - 1) as f32 * 80.0 + y as f32 * 0.01;
            samples.push((x as f32, height, false));
        }
    }

    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &samples,
    );
    radar.refresh_terrain();
    let tex = radar.get_terrain_texture();
    let left = &tex[0..4];
    let right_offset = ((RADAR_CELL_WIDTH - 1) * 4) as usize;
    let right = &tex[right_offset..right_offset + 4];

    assert_ne!(left[0], right[0]);
    assert_ne!(left[1], right[1]);
}

#[test]
fn test_refresh_terrain_tints_water_cells_blue() {
    let mut radar = RadarSystem::new();
    let sample_count = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
    let mut samples = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let is_water = i == 0;
        samples.push((0.0, 10.0, is_water));
    }

    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &samples,
    );
    radar.refresh_terrain();
    let tex = radar.get_terrain_texture();
    let first = &tex[0..4];

    assert!(first[2] > first[0]);
}

#[test]
fn test_examine_object() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Add object
    let mut obj = RadarObject::new(1);
    obj.world_pos = Coord3D::new(100.0, 100.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    radar.add_object(obj.clone());

    // Update object position
    let mut updated = obj;
    updated.world_pos = Coord3D::new(200.0, 200.0, 0.0);
    radar.examine_object(1, updated);

    // Should still have 1 object
    assert_eq!(radar.get_all_objects().count(), 1);
}

// ===== New comprehensive radar tests =====

#[test]
fn test_gps_satellite_activation() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // GPS not active initially
    assert!(!radar.is_gps_active());

    // Activate GPS for 900 frames (30 seconds)
    radar.activate_gps_satellite(900);
    assert!(radar.is_gps_active());

    // All shroud should be clear
    assert!(
        radar
            .get_shroud_grid()
            .iter()
            .all(|&cell| cell == CellShroudStatus::Clear)
    );

    // Update to expiration
    radar.update(900);
    assert!(!radar.is_gps_active());
}

#[test]
fn test_radar_scan_activation() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let scan_location = Coord3D::new(500.0, 500.0, 0.0);
    let scan_radius = 300.0;
    let player_id = 1;

    // Activate radar scan
    radar.activate_radar_scan(scan_location, scan_radius, 300, player_id);

    // Check that scan is active
    let scans = radar.get_active_radar_scans(player_id);
    assert_eq!(scans.len(), 1);

    // Position inside scan should be revealed
    let inside_pos = Coord3D::new(550.0, 550.0, 0.0);
    assert!(radar.is_position_in_radar_scan(&inside_pos, player_id));

    // Position outside scan should not be revealed
    let outside_pos = Coord3D::new(900.0, 900.0, 0.0);
    assert!(!radar.is_position_in_radar_scan(&outside_pos, player_id));

    // Update past expiration
    radar.update(301);
    assert_eq!(radar.get_active_radar_scans(player_id).len(), 0);
}

#[test]
fn test_radar_jamming() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let jammer_location = Coord3D::new(500.0, 500.0, 0.0);
    let jamming_radius = 200.0;
    let jammer_player_id = 2; // Enemy player

    // Add jamming source
    radar.add_jamming_source(100, jammer_location, jamming_radius, jammer_player_id);

    // Position inside jamming radius should be jammed for player 1
    let jammed_pos = Coord3D::new(550.0, 550.0, 0.0);
    assert!(radar.is_position_jammed(&jammed_pos, 1));

    // Position outside jamming radius should not be jammed
    let clear_pos = Coord3D::new(800.0, 800.0, 0.0);
    assert!(!radar.is_position_jammed(&clear_pos, 1));

    // Disable jammer
    radar.set_jamming_source_active(100, false);
    assert!(!radar.is_position_jammed(&jammed_pos, 1));

    // Remove jammer
    radar.remove_jamming_source(100);
    assert!(radar.jamming_sources.is_empty());
}

#[test]
fn test_radar_power_states() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Add radar provider
    let mut radar_obj = RadarObject::new(1);
    radar_obj.world_pos = Coord3D::new(500.0, 500.0, 0.0);
    radar_obj.priority = RadarPriorityType::Structure;
    radar_obj.radar_range = 200.0;
    radar_obj.is_radar_provider = true;
    radar_obj.is_powered = true;
    radar.add_object(radar_obj);

    // Radar should be operational
    assert!(radar.has_operational_radar(0));

    // Disable power
    radar.set_radar_powered(1, false);
    assert!(!radar.has_operational_radar(0));

    // Re-enable power
    radar.set_radar_powered(1, true);
    assert!(radar.has_operational_radar(0));
}

#[test]
fn test_radar_emp_disable() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Add radar provider
    let mut radar_obj = RadarObject::new(1);
    radar_obj.world_pos = Coord3D::new(500.0, 500.0, 0.0);
    radar_obj.priority = RadarPriorityType::Structure;
    radar_obj.radar_range = 200.0;
    radar_obj.is_radar_provider = true;
    radar.add_object(radar_obj);

    // Radar operational initially
    assert!(radar.has_operational_radar(0));

    // Apply EMP
    radar.disable_radar_object(1);
    assert!(!radar.has_operational_radar(0));

    // EMP expires
    radar.enable_radar_object(1);
    assert!(radar.has_operational_radar(0));
}

#[test]
fn test_radar_object_operational_check() {
    let mut obj = RadarObject::new(1);
    obj.is_radar_provider = true;
    obj.is_powered = true;
    obj.is_disabled = false;

    // Should be operational
    assert!(obj.is_radar_operational());

    // Not powered
    obj.set_powered(false);
    assert!(!obj.is_radar_operational());
    obj.set_powered(true);

    // Disabled by EMP
    obj.disable_radar();
    assert!(!obj.is_radar_operational());
    obj.enable_radar();
    assert!(obj.is_radar_operational());
}

#[test]
fn test_radar_scan_expiration() {
    let scan = RadarScan::new(
        Coord3D::new(100.0, 100.0, 0.0),
        300.0,
        100, // duration
        1,   // player_id
        0,   // current_frame
    );

    assert!(!scan.is_expired(50));
    assert!(!scan.is_expired(99));
    assert!(scan.is_expired(100));
    assert!(scan.is_expired(150));
}

#[test]
fn test_jamming_source_range() {
    let jammer = JammingSource::new(1, Coord3D::new(500.0, 500.0, 0.0), 200.0, 1);

    // Position inside radius
    let inside = Coord3D::new(550.0, 550.0, 0.0);
    assert!(jammer.is_position_jammed(&inside));

    // Position outside radius
    let outside = Coord3D::new(800.0, 800.0, 0.0);
    assert!(!jammer.is_position_jammed(&outside));
}

#[test]
fn test_jamming_update_on_objects() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    // Add object
    let mut obj = RadarObject::new(10);
    obj.world_pos = Coord3D::new(550.0, 550.0, 0.0);
    obj.priority = RadarPriorityType::Unit;
    radar.add_object(obj);

    // Add jammer affecting object
    radar.add_jamming_source(1, Coord3D::new(500.0, 500.0, 0.0), 200.0, 2);

    // Update jamming status
    radar.update_jamming_status();

    // Object should be jammed
    let obj_jammed = radar
        .get_all_objects()
        .find(|o| o.object_id == 10)
        .map(|o| o.is_jammed)
        .unwrap_or(false);
    assert!(obj_jammed);
}

#[test]
fn test_shroud_status_visibility() {
    assert!(CellShroudStatus::Clear.is_visible());
    assert!(CellShroudStatus::Fogged.is_visible());
    assert!(!CellShroudStatus::Shrouded.is_visible());

    assert!(CellShroudStatus::Clear.is_explored());
    assert!(CellShroudStatus::Fogged.is_explored());
    assert!(!CellShroudStatus::Shrouded.is_explored());
}

#[test]
fn try_event_throttles_inactive_history_for_ten_seconds() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );
    let loc = Coord3D::new(100.0, 100.0, 0.0);
    assert!(radar.try_event(RadarEventType::UnderAttack, &loc));
    radar.update(150);
    assert_eq!(radar.get_active_events().len(), 0);
    assert!(!radar.try_event(RadarEventType::UnderAttack, &loc));
    radar.update(400);
    assert!(radar.try_event(RadarEventType::UnderAttack, &loc));
}

#[test]
fn try_event_cpp_precedence_throttles_map_wide() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );
    let near = Coord3D::new(10.0, 10.0, 0.0);
    let far = Coord3D::new(800.0, 800.0, 0.0);
    assert!(radar.try_event(RadarEventType::UnderAttack, &near));
    // C++ 250² check is a no-op; same type anywhere within 10s is rejected.
    assert!(!radar.try_event(RadarEventType::UnderAttack, &far));
    radar.update(400);
    assert!(radar.try_event(RadarEventType::UnderAttack, &far));
}

#[test]
fn add_object_inserts_at_head_of_priority_section() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );

    let mut first = RadarObject::new(1);
    first.world_pos = Coord3D::new(100.0, 100.0, 0.0);
    first.priority = RadarPriorityType::Unit;
    radar.add_object(first);

    let mut second = RadarObject::new(2);
    second.world_pos = Coord3D::new(200.0, 200.0, 0.0);
    second.priority = RadarPriorityType::Unit;
    radar.add_object(second);

    // C++ inserts at the head of the Unit section.
    assert_eq!(radar.object_list[0].object_id, 2);
    assert_eq!(radar.object_list[1].object_id, 1);
}

#[test]
fn overlay_draws_local_last_so_own_blips_win_shared_cells() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(128.0, 128.0, 0.0),
        &[],
    );
    radar.clear_shroud();

    let mut enemy = RadarObject::new(1);
    enemy.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    enemy.priority = RadarPriorityType::Unit;
    enemy.color = 0xFFFF0000;
    enemy.is_local = false;
    radar.add_object(enemy);

    let mut local = RadarObject::new(2);
    local.world_pos = Coord3D::new(10.0, 20.0, 0.0);
    local.priority = RadarPriorityType::Unit;
    local.color = 0xFF0000FF;
    local.is_local = true;
    radar.add_object(local);

    let texture = radar.build_object_overlay_texture_rgba();
    let idx = ((20 * RADAR_CELL_WIDTH + 10) * 4) as usize;
    assert_eq!(&texture[idx..idx + 4], &[0x00, 0x00, 0xFF, 0xFF]);
}

#[test]
fn try_infiltration_without_local_victim_is_fail_closed() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );
    let loc = Coord3D::new(40.0, 50.0, 0.0);
    radar.try_infiltration_event(&loc);
    assert!(radar.get_last_event_loc().is_none());

    let remote = RadarVictimInfo {
        is_local_player: false,
        player_index: 2,
        ..Default::default()
    };
    radar.try_infiltration_event_for(&loc, Some(&remote));
    assert!(radar.get_last_event_loc().is_none());

    let local = RadarVictimInfo {
        is_local_player: true,
        player_index: 0,
        ..Default::default()
    };
    radar.try_infiltration_event_for(&loc, Some(&local));
    assert!(radar.get_last_event_loc().is_some());
}

#[test]
fn try_under_attack_creates_under_attack_event() {
    let mut radar = RadarSystem::new();
    radar.new_map(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1024.0, 1024.0, 100.0),
        &[],
    );
    let loc = Coord3D::new(12.0, 34.0, 0.0);
    assert!(radar.try_under_attack_event(&loc));
    assert_eq!(radar.get_last_event_loc(), Some(loc));
    assert!(!radar.try_under_attack_event(&loc));
}
