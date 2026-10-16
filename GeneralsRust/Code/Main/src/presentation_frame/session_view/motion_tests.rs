//! The production owner receives exactly one selected View admission.
use super::camera::motion::MotionAdmission;
use super::waypoint::{Point, WaypointMotion};
use super::*;
fn admission() -> MotionAdmission {
    MotionAdmission {
        update_views: true,
        fast_camera_only: false,
        paused: false,
        debug_frozen: false,
        disable_camera_movement: false,
    }
}
fn moving() -> SessionView {
    let mut view = SessionView::default();
    let motion = WaypointMotion::move_to(
        Point {
            x: 300.,
            y: 150.,
            z: 0.,
        },
        Point::default(),
        0.25,
        20.,
        30.,
        1,
        3000,
        true,
        0.,
        0.,
    );
    view.install_motion(motion, true, &mut 1., false);
    view
}
#[test]
fn admitted_camera_owner_rejects_pause_and_advances_fast_without_a_logic_tick() {
    let mut view = moving();
    let mut speed = 1.;
    let target = view.target;
    let mut offer = admission();
    offer.paused = true;
    view.advance_camera_movements(offer, 33, &mut speed);
    assert_eq!(view.scripted.motion.elapsed_ms, 0);
    assert_eq!(view.target, target);
    offer.fast_camera_only = true;
    offer.update_views = false;
    view.advance_camera_movements(offer, 33, &mut speed);
    assert_eq!(view.scripted.motion.elapsed_ms, 33);
    assert_ne!(view.target, target);
    assert!(view.scripted.motion.active);
    // A second world is not an admitted update of this owner.
    let mut peer = moving();
    peer.advance_camera_movements(admission(), 99, &mut 3.);
    assert_eq!(view.scripted.motion.elapsed_ms, 33);
}
#[test]
fn rotation_completion_does_not_also_consume_a_path_step() {
    let mut view = moving();
    view.camera.camera_yaw_target = Some(1.);
    view.camera.camera_yaw_duration = 0.;
    let mut speed = 1.;
    view.advance_camera_movements(admission(), 33, &mut speed);
    assert!(view.camera.camera_yaw_target.is_none());
    assert_eq!(view.scripted.motion.elapsed_ms, 0);
    view.advance_camera_movements(admission(), 33, &mut speed);
    assert_eq!(view.scripted.motion.elapsed_ms, 33);
}
#[test]
fn disabled_camera_still_consumes_original_clock_without_changing_pose() {
    let mut view = moving();
    let start = view.target;
    let yaw = view.camera.camera_yaw_radians;
    let mut offer = admission();
    offer.disable_camera_movement = true;
    view.advance_camera_movements(offer, 33, &mut 1.);
    assert_eq!(view.scripted.motion.elapsed_ms, 33);
    assert_eq!(view.target, start);
    assert_eq!(view.camera.camera_yaw_radians, yaw);
}

#[test]
fn receiving_reset_cancels_actual_motion_flags_without_replacing_persistent_pose_or_peer() {
    let mut world = crate::game_logic::GameLogic::new();
    let mut peer = crate::game_logic::GameLogic::new();
    world.session_view.camera.camera_zoom = 0.8;
    world.session_view.camera.camera_zoom_target = Some(0.5);
    world.session_view.camera.camera_pitch_target = Some(0.7);
    world.session_view.camera.camera_yaw_target = Some(1.);
    peer.session_view.camera.camera_zoom_target = Some(1.5);
    let peer_before = peer.capture_camera_continuation().unwrap();
    world.reset();
    assert!(world.session_view.movement_finished());
    assert_eq!(world.session_view.camera.camera_zoom, 0.8);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}
