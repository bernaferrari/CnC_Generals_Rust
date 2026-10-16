//! Production command admission and script evaluation are different View phases.
//! These assertions also compile against the pre-migration production APIs.
use super::*;
#[test]
fn cpp_camera_scalar_is_installed_before_the_next_authored_action() {
    let mut owner = GameLogic::new();
    let peer = GameLogic::new();
    owner.scripts_loaded = true;
    owner.mission_scripts.push_camera_zoom(CameraZoomRequest {
        zoom: 0.5,
        duration_seconds: 4.,
        ease_in_seconds: 0.2,
        ease_out_seconds: 0.3,
    });
    owner.evaluate_and_execute_scripts(0.);
    assert_eq!(
        owner.session_view.camera.camera_zoom_target,
        Some(0.5),
        "source zoomCamera installs movement synchronously at the action boundary"
    );
    assert_eq!(owner.session_view.camera.camera_zoom_frame, 0);
    assert_eq!(peer.session_view.camera.camera_zoom_target, None);
}
#[test]
fn cpp_script_evaluation_does_not_offer_a_camera_view_or_queue_a_second_focus() {
    let mut owner = GameLogic::new();
    owner.scripts_loaded = true;
    owner.start_camera_move_to(CameraMoveToRequest {
        position: Vec3::new(200., 0., 100.),
        seconds: 4.,
        camera_stutter_seconds: 0.,
        ease_in_seconds: 0.,
        ease_out_seconds: 0.,
    });
    let target = owner.session_view.target;
    let yaw = owner.session_view.camera.camera_yaw_radians;
    owner.evaluate_and_execute_scripts(1.);
    assert_eq!(owner.session_view.target, target);
    assert_eq!(owner.session_view.camera.camera_yaw_radians, yaw);
    assert!(
        owner.peek_pending_camera_focus().is_none(),
        "source movement belongs to W3DView admission, never a logic-dt focus transport"
    );
}
