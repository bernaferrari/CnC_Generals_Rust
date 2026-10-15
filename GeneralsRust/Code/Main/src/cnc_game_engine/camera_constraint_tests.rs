use super::*;

#[test]
fn inverted_camera_constraints_preserve_cpp_sequential_clamp() {
    let constrained = constrain_camera_target_to_map(
        Vec3::new(50., 7., 50.),
        Vec3::ZERO,
        Vec3::new(100., 0., 100.),
        80.,
        None,
    );
    assert_eq!(constrained, Vec3::new(20., 7., 20.));
    let widened = constrain_camera_target_to_map(
        Vec3::new(-5., 7., 110.),
        Vec3::ZERO,
        Vec3::new(100., 0., 100.),
        80.,
        Some((-10., 120., -10., 120.)),
    );
    assert_eq!(widened, Vec3::new(-5., 7., 110.));
}

#[test]
fn camera_constraint_plane_uses_receiving_fixed_ground_not_logical_zero() {
    let mut camera = super::super::camera_height::CameraHeight::new(300., 37.5, 0.);
    camera.initialize_map(80., 300., 37.5, 0.);
    let view = Mat4::look_at_rh(Vec3::new(0., 300., 300.), Vec3::ZERO, Vec3::Y);
    let projection = Mat4::perspective_rh(50_f32.to_radians(), 4. / 3., 1., 2000.);
    let viewport = (800., 600.);
    let actual = native_camera_constraint_inset(&camera, view, projection, viewport);
    let source = w3d_camera_constraint_offset(view, projection, viewport, 80.);
    let wrong_logical_plane = w3d_camera_constraint_offset(view, projection, viewport, 0.);
    assert_eq!(actual, source);
    assert!(actual.is_finite() && actual > 0.);
    assert!((actual - wrong_logical_plane).abs() > 1.);
    let mut peer = super::super::camera_height::CameraHeight::new(300., 37.5, 0.);
    peer.initialize_map(-20., 300., 37.5, 0.);
    assert_eq!(
        native_camera_constraint_inset(&camera, view, projection, viewport),
        actual
    );
    assert_ne!(
        native_camera_constraint_inset(&peer, view, projection, viewport),
        actual
    );
}
