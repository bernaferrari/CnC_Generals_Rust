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
    frame.world_env.height_samples_from_terrain = true;
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
    assert_eq!(look, glam::Vec3::new(10.0, 0.0, 10.0));
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

fn native_startup_frame(
    samples: &[(usize, usize, u8)],
) -> crate::presentation_frame::PresentationFrame {
    let mut frame = startup_camera_terrain_frame(samples);
    frame.world_env.world_min = [0.0, 0.0, 0.0];
    frame.world_env.world_max = [80.0, 0.0, 80.0];
    frame.world_env.initial_camera_position = Some([10.0, 10.0, 0.0]);
    frame
}

fn native_startup_defaults() -> super::StartupCameraDefaults {
    super::StartupCameraDefaults {
        camera_height: 100.0,
        max_camera_height: 300.0,
        pitch_degrees: 45.0,
        yaw_degrees: 0.0,
    }
}

#[test]
fn native_startup_retains_fixed_map_basis_default_goal_and_enabled_owner() {
    use super::camera_height::{HeightRules, HeightStep};
    let frame = native_startup_frame(&[(2, 2, 208), (6, 6, 255)]);
    let (target, eye, mut zoom, mut owner) = CnCGameEngine::bootstrap_native_camera_for_loaded_map(
        false,
        0,
        native_startup_defaults(),
        Some(&frame),
    );
    assert_eq!(target, glam::Vec3::new(10.0, 0.0, 10.0));
    assert_eq!(zoom, (159.375 + 300.0) / 220.0);
    assert_eq!(owner.desired(), 300.0);
    assert_eq!(owner.basis().pose(target, zoom, 0.0, 0.0).0, eye);
    // Destination terrain changes the numerator only, never initial-map offsetZ.
    assert_eq!(owner.default_zoom(20.0, 300.0), 320.0 / 220.0);
    owner.set_height(400.0, 0.0, 1000.0);
    let peer = native_startup_frame(&[(2, 2, 16), (6, 6, 32)]);
    let (_, _, peer_zoom, peer_owner) = CnCGameEngine::bootstrap_native_camera_for_loaded_map(
        false,
        0,
        native_startup_defaults(),
        Some(&peer),
    );
    assert_eq!(peer_zoom, 320.0 / 110.0);
    assert_eq!(peer_owner.desired(), 300.0);
    assert_eq!(owner.desired(), 400.0);
    let previous_zoom = zoom;
    assert!(
        owner.advance(
            &mut zoom,
            HeightStep {
                terrain: 159.375,
                paused: false,
                scrolling: false,
                scroll_amount: 0.0,
                scripted: false,
                replay_camera: false,
                rules: HeightRules {
                    min: 0.0,
                    max: 1000.0,
                    enforce_max: false,
                    adjust_speed: 0.5,
                    scroll_cutoff: 0.0
                },
            }
        ),
        "map admission enables HAG adjustment on its retained owner"
    );
    assert!(zoom > previous_zoom);
}

#[test]
fn native_saved_pose_uses_initial_map_basis_and_admits_visible_height() {
    use super::camera_height::{HeightRules, HeightStep};
    let frame = native_startup_frame(&[(2, 2, 208), (6, 6, 255)]);
    let (_, _, _, mut owner) = CnCGameEngine::bootstrap_native_camera_for_loaded_map(
        false,
        0,
        native_startup_defaults(),
        Some(&frame),
    );
    let target = glam::Vec3::new(40.0, 0.0, 30.0);
    let mut zoom = 2.5;
    let heading = 0.7;
    let pitch = 0.2;
    let (eye, _) = owner.basis().pose(target, zoom, pitch, heading);
    let recovered = owner
        .basis()
        .pitch_from_pose(target, eye, zoom, heading)
        .unwrap();
    assert!((recovered - pitch).abs() < 0.000001);
    owner.admit_restored_pose_height(zoom, 20.0);
    assert_eq!(owner.desired(), 530.0);
    assert_eq!(
        owner.default_zoom(20.0, 300.0),
        320.0 / 220.0,
        "saved eye and destination cannot recalibrate initial-map offsetZ"
    );
    assert!(
        !owner.advance(
            &mut zoom,
            HeightStep {
                terrain: 20.0,
                paused: false,
                scrolling: false,
                scroll_amount: 0.0,
                scripted: false,
                replay_camera: false,
                rules: HeightRules {
                    min: 0.0,
                    max: 1000.0,
                    enforce_max: false,
                    adjust_speed: 0.5,
                    scroll_cutoff: 0.0
                },
            }
        ),
        "native restored pose must not drift toward unrelated startup HAG"
    );
    assert_eq!(zoom, 2.5);
}
