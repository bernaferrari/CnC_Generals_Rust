//! The real GameLogic SessionView owns native pose/HAG and scripted movement.
//! These tests cover local ownership and typed save admission, not a window,
//! renderer, native input event, or end-to-end gameplay flight.
use super::*;

fn configure_native(world: &mut GameLogic, yaw: f32, zoom: f32, ground: f32, desired: f32) {
    let camera = &mut world.session_view.camera;
    camera.camera_yaw_radians = yaw;
    camera.camera_yaw_start = yaw - 0.25;
    camera.camera_yaw_target = Some(yaw + 0.5);
    camera.camera_yaw_frame = 7;
    camera.camera_yaw_duration = 3.;
    camera.camera_zoom = zoom;
    camera.camera_zoom_start = zoom - 0.1;
    camera.camera_zoom_target = Some(zoom + 0.2);
    camera.camera_zoom_frame = 5;
    camera.camera_zoom_duration = 2.;
    camera.native_camera.initialize_map(ground, 300., 37.5, 0.);
    camera.native_camera.set_height(desired, 100., 500.);
    let (eye, look) = camera.native_camera.basis().pose(Vec3::ZERO, zoom, 0., yaw);
    camera.camera_position = eye;
    camera.view_matrix = glam::Mat4::look_at_rh(eye, look, Vec3::Y);
}

/// Immutable observation of the actual receiving native fields. Nothing is
/// installed from this receipt and no second writable camera is constructed.
fn native_receipt(world: &GameLogic) -> Vec<u8> {
    let camera = &world.session_view.camera;
    serde_json::to_vec(&(
        camera.camera_yaw_radians,
        camera.camera_yaw_start,
        camera.camera_yaw_target,
        camera.camera_yaw_frame,
        camera.camera_yaw_duration,
        camera.camera_zoom,
        camera.camera_zoom_start,
        camera.camera_zoom_target,
        camera.camera_zoom_frame,
        camera.camera_zoom_duration,
        camera.native_camera.capture_continuation(),
        camera.camera_position,
        camera.view_matrix.to_cols_array(),
        camera.projection_matrix.to_cols_array(),
    ))
    .unwrap()
}

#[test]
fn session_view_peer_reset_and_script_decode_leave_actual_native_and_script_owners_isolated() {
    let mut a = moving_world();
    configure_native(&mut a, 0.25, 0.8, 20., 260.);
    let a_script = a.capture_camera_continuation().unwrap();
    let a_native = native_receipt(&a);
    // Construction has no publication side effect on the already-running owner.
    let mut b = moving_world();
    configure_native(&mut b, -1., 1.7, 55., 410.);
    b.session_view.camera.camera_zoom_duration = 9.;
    b.session_view.camera.camera_pitch_target = Some(0.75);
    b.session_view.camera.camera_pitch_duration = 12.;
    assert_eq!(a.capture_camera_continuation().unwrap(), a_script);
    assert_eq!(native_receipt(&a), a_native);
    assert_ne!(native_receipt(&b), a_native);

    b.reset();
    assert_eq!(a.capture_camera_continuation().unwrap(), a_script);
    assert_eq!(native_receipt(&a), a_native);
    // A script-only candidate must not overwrite this receiver's actual native
    // pose, admitted axis frames, or HAG basis/desired height.
    configure_native(&mut b, -0.75, 1.4, 65., 390.);
    let b_native = native_receipt(&b);
    let candidate = GameLogic::decode_camera_continuation(&a_script).unwrap();
    assert_eq!(native_receipt(&a), a_native, "detached decode is inert");
    assert_eq!(native_receipt(&b), b_native);
    b.admit_camera_continuation(candidate).unwrap();
    assert_eq!(b.capture_camera_continuation().unwrap(), a_script);
    assert!(b.session_view.scripted.motion.active);
    assert_eq!(
        native_receipt(&b),
        a_native,
        "V2 restores one complete camera/HAG owner, not a script-only mirror"
    );
    assert_eq!(a.capture_camera_continuation().unwrap(), a_script);
    assert_eq!(native_receipt(&a), a_native);
    assert_eq!(a.session_view.camera.native_camera.desired(), 260.);
    assert_eq!(b.session_view.camera.native_camera.desired(), 260.);
    assert_eq!(
        a.session_view.camera.native_camera.basis().ground_level(),
        20.
    );
    assert_eq!(
        b.session_view.camera.native_camera.basis().ground_level(),
        20.
    );
}
