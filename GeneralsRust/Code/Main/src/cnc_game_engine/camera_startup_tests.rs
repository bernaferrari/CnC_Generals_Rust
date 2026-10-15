use super::CnCGameEngine;

// Exercise the production startup receiver, not a duplicate five-sample loop.
fn startup_camera_terrain_frame(
    samples: &[(usize, usize, u8)],
) -> crate::presentation_frame::PresentationFrame {
    let mut logic = crate::game_logic::GameLogic::new();
    let mut frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, 0);
    let mut map = crate::presentation_frame::PresentationRuntimeHeightmap {
        width: 12,
        height: 12,
        heights: vec![0.0; 144],
        max_height: 159.375,
        scale: 10.0,
        border_size: 1,
        ..Default::default()
    };
    for &(x, z, byte) in samples {
        map.heights[z * 12 + x] = f32::from(byte) / 255.0;
    }
    // Deliberately unrelated playable bounds and coarse data: the original
    // BaseHeightMap camera query uses raw logical coordinates plus its border.
    frame.world_env.world_min = [100.0, 0.0, 100.0];
    frame.world_env.world_max = [110.0, 0.0, 110.0];
    frame.world_env.height_grid_w = 2;
    frame.world_env.height_grid_h = 2;
    frame.world_env.height_samples = vec![777.0; 4];
    frame.world_env.runtime_heightmap = Some(std::sync::Arc::new(map));
    frame
}

#[test]
fn startup_camera_receiver_caps_initial_center_only_and_keeps_raw_corner() {
    // (10,10) maps to (2,2); its (+40,+40) sample maps to (6,6).
    // The corner exceeds both the center and initHeightForMap's 120 cap.
    let frame = startup_camera_terrain_frame(&[(2, 2, 208), (6, 6, 255)]);
    let target = glam::Vec3::new(10.0, 999.0, 10.0);
    assert_eq!(
        CnCGameEngine::sample_startup_camera_heights(target, 888.0, Some(&frame)),
        (120.0, 159.375),
        "only the initial center is capped; raw terrain maximum must survive"
    );
    let peer = startup_camera_terrain_frame(&[(2, 2, 16), (6, 6, 32)]);
    assert_eq!(
        CnCGameEngine::sample_startup_camera_heights(target, 888.0, Some(&peer)),
        (10.0, 20.0)
    );
    assert_eq!(
        CnCGameEngine::sample_startup_camera_heights(target, 888.0, Some(&frame)),
        (120.0, 159.375),
        "sampling another frozen map must not alter the first receiver"
    );
}

#[test]
fn startup_camera_receiver_preserves_negative_border_queries_without_preclamp() {
    // floor(-0.1 / 10) + border = 0, so C++ chooses the single border
    // sample, not an interpolation toward interior (1,1), nor playable bounds.
    let frame = startup_camera_terrain_frame(&[(0, 1, 160), (1, 1, 16)]);
    assert_eq!(
        CnCGameEngine::sample_startup_camera_heights(
            glam::Vec3::new(-0.1, 999.0, 2.0),
            888.0,
            Some(&frame),
        ),
        (100.0, 100.0)
    );
    // Far outside the full map, original integer-sample clipping is still
    // owned by BaseHeightMap; no playable rectangle may relocate the query.
    assert_eq!(
        CnCGameEngine::sample_startup_camera_heights(
            glam::Vec3::new(-1000.0, 999.0, 2.0),
            888.0,
            Some(&frame),
        ),
        (100.0, 100.0)
    );
}

#[test]
fn startup_bootstrap_uses_raw_terrain_zoom_and_original_map_basis() {
    let mut frame = startup_camera_terrain_frame(&[(2, 2, 208), (6, 6, 255)]);
    frame.world_env.world_min = [0.0; 3];
    frame.world_env.world_max = [100.0; 3];
    frame.world_env.initial_camera_position = Some([10.0, 10.0, 999.0]);
    let defaults = super::StartupCameraDefaults {
        pitch_degrees: 37.5,
        yaw_degrees: 25.0,
        camera_height: 1000.0,
        max_camera_height: 400.0,
    };
    let (look, eye, zoom) =
        CnCGameEngine::bootstrap_camera_for_loaded_map(false, 0, defaults, Some(&frame));
    assert_eq!(look, glam::Vec3::new(10.0, 120.0, 10.0));
    assert_eq!(zoom, (159.375 + 400.0) / 1120.0);
    // Original normal transform scales the fixed map offset before translating.
    // Preserve the C++ float PI / double tan intermediates independently here.
    let radians = f64::from(std::f32::consts::PI) / 180.0;
    let offset_y = -(1120.0 / (37.5 * radians).tan()) as f32;
    let offset_x = -(f64::from(offset_y) * (25.0 * radians).tan()) as f32;
    let factor = (1.0 - f64::from(120.0 / (1120.0 * zoom))) as f32;
    assert_eq!(eye.x, offset_x * zoom * factor + 10.0);
    assert_eq!(eye.y, 1120.0 * zoom * factor + 120.0);
    assert_eq!(eye.z, offset_y * zoom * factor + 10.0);
}
