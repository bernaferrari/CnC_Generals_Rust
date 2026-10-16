use super::*;

fn moving_world() -> GameLogic {
    let mut world = GameLogic::new();
    let request = CameraMoveToRequest {
        position: Vec3::new(100.0, 20.0, 50.0),
        seconds: 3.0,
        camera_stutter_seconds: 0.1,
        ease_in_seconds: 0.25,
        ease_out_seconds: 0.5,
    };
    use crate::presentation_frame::waypoint::{Point, WaypointMotion};
    world.session_view.target = Vec3::new(1., 2., 3.);
    let mut movement = WaypointMotion::move_to(
        Point::from_world(request.position),
        Point::from_world(world.session_view.target),
        0.,
        0.,
        0.,
        1,
        3000,
        true,
        250.,
        500.,
    );
    movement.current_shutter = 3;
    movement.shutter = 3;
    movement.final_time_multiplier(2, &mut world.visual_speed_multiplier);
    world.session_view.scripted.motion = movement;
    world.session_view.scripted.script_camera_freeze_time = true;
    world.session_view.camera.camera_zoom_target = Some(0.8);
    world.session_view.camera.camera_zoom_duration = 2.;
    world.session_view.scripted.camera_tether_play = Some(12.0);
    world.native_view_guardband = [3.0, 4.0];
    world
}

#[test]
fn logic_camera_continuation_retains_future_move_and_shutter_steps() {
    let mut uninterrupted = moving_world();
    for _ in 0..7 {
        uninterrupted.headless_camera_view_offer(33);
    }
    let bytes = uninterrupted.capture_camera_continuation().unwrap();
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&bytes).unwrap())
        .unwrap();
    for _ in 0..50 {
        uninterrupted.headless_camera_view_offer(33);
        restored.headless_camera_view_offer(33);
        assert_eq!(
            restored.session_view.target,
            uninterrupted.session_view.target
        );
        assert_eq!(
            restored.session_view.camera.camera_yaw_radians,
            uninterrupted.session_view.camera.camera_yaw_radians
        );
        assert_eq!(
            restored.capture_camera_continuation().unwrap(),
            uninterrupted.capture_camera_continuation().unwrap()
        );
    }
}

#[test]
fn logic_camera_continuation_preserves_pending_hook_fifo_without_draining_source() {
    let source = moving_world();
    for zoom in [0.25, 0.5, 0.75] {
        source.mission_scripts.push_camera_zoom(CameraZoomRequest {
            zoom,
            duration_seconds: 1.0,
            ease_in_seconds: 0.1,
            ease_out_seconds: 0.2,
        });
    }
    source
        .mission_scripts
        .push_camera_move(Vec3::new(42.0, 0.0, 43.0));
    let bytes = source.capture_camera_continuation().unwrap();
    assert_eq!(source.capture_camera_continuation().unwrap(), bytes);
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&bytes).unwrap())
        .unwrap();
    assert!(!restored.mission_scripts.is_camera_movement_finished());
    let zooms = restored.mission_scripts.drain_camera_zoom_requests();
    assert_eq!(
        zooms.iter().map(|r| r.zoom).collect::<Vec<_>>(),
        [0.25, 0.5, 0.75]
    );
    assert!(
        restored
            .mission_scripts
            .drain_camera_zoom_requests()
            .is_empty()
    );
    assert_eq!(source.mission_scripts.drain_camera_zoom_requests().len(), 3);
}

#[test]
fn logic_camera_continuation_peer_reset_and_malformed_candidates_leave_owner_unchanged() {
    let a = moving_world();
    let baseline = a.capture_camera_continuation().unwrap();
    let mut b = moving_world();
    b.pending_camera_focus = Some(Vec3::splat(99.0));
    let peer = b.capture_camera_continuation().unwrap();
    assert_ne!(peer, baseline);
    b.reset();
    b.admit_camera_continuation(GameLogic::decode_camera_continuation(&peer).unwrap())
        .unwrap();
    assert_eq!(a.capture_camera_continuation().unwrap(), baseline);
    let mut trailing = baseline.clone();
    trailing.push(0);
    assert!(GameLogic::decode_camera_continuation(&trailing).is_err());
    assert!(GameLogic::decode_camera_continuation(&baseline[..baseline.len() - 1]).is_err());
    let mut invalid = GameLogic::decode_camera_continuation(&baseline).unwrap();
    invalid.script_camera_focus_estimate.x = f32::NAN;
    assert!(GameLogic::decode_camera_continuation(&codec().serialize(&invalid).unwrap()).is_err());
    let mut too_many = GameLogic::decode_camera_continuation(&baseline).unwrap();
    too_many.hooks.camera_move_home_requests = vec![(); MAX_ITEMS + 1];
    assert!(GameLogic::decode_camera_continuation(&codec().serialize(&too_many).unwrap()).is_err());
    assert_eq!(a.capture_camera_continuation().unwrap(), baseline);
}

#[test]
fn logic_camera_continuation_retains_path_segments_rolling_and_pending_admission() {
    let mut world = moving_world();
    use crate::presentation_frame::waypoint::{Point, WaypointMotion};
    let points = [
        Vec3::ZERO,
        Vec3::X * 20.,
        Vec3::new(40., 0., 20.),
        Vec3::new(60., 0., 20.),
        Vec3::new(80., 0., 40.),
    ]
    .map(Point::from_world);
    let mut path = WaypointMotion::along_chain(
        &points,
        Point::default(),
        0.,
        0.,
        0.,
        1,
        3000,
        33,
        33,
        true,
        0.,
        0.,
        1.,
    );
    path.rolling_average(7);
    path.freeze_angle();
    world.session_view.scripted.motion = path;
    world.pending_camera_pitch = Some(CameraPitchRequest {
        pitch: 0.75,
        duration_seconds: 3.0,
        ease_in_seconds: 0.5,
        ease_out_seconds: 0.25,
    });
    let bytes = world.capture_camera_continuation().unwrap();
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&bytes).unwrap())
        .unwrap();
    assert_eq!(restored.capture_camera_continuation().unwrap(), bytes);
    for _ in 0..3 {
        restored.headless_camera_view_offer(33);
        world.headless_camera_view_offer(33);
        assert_eq!(restored.session_view.target, world.session_view.target);
    }
    assert_eq!(
        restored.capture_camera_continuation().unwrap(),
        world.capture_camera_continuation().unwrap()
    );
    let mut broken = GameLogic::decode_camera_continuation(&bytes).unwrap();
    broken.owned_extras.motion.segment = usize::MAX;
    let malformed = owned_envelope::encode(&broken).unwrap();
    assert!(GameLogic::decode_camera_continuation(&malformed).is_err());
}

#[path = "session_view_ownership_tests.rs"]
mod session_view_ownership_tests;
