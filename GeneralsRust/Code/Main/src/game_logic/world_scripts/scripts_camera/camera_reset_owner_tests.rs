//! Actual canonical reset admission and future View offers, not a DTO-only test.
use super::*;
fn owner() -> GameLogic {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 64 * 64];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world.session_view.target = Vec3::new(200., 0., 300.);
    let camera = &mut world.session_view.camera;
    camera.camera_yaw_radians = 0.6;
    camera.camera_pitch_radians = 0.2;
    camera.camera_fx_pitch = 0.4;
    camera.camera_zoom = 0.7;
    camera.rebuild_projection();
    camera
        .constraints
        .recalculate(Vec3::ZERO, Vec3::new(1000., 0., 1000.), 20.)
        .unwrap();
    camera
        .script_camera_shakers
        .push(crate::presentation_frame::camera::ScriptCameraShaker::new(
            Vec3::ZERO,
            10000.,
            30.,
            0.1,
        ));
    world.session_view.commit_orbit_pose();
    world.session_view.scripted.camera_follow_target = Some(ObjectId(42));
    world.session_view.scripted.camera_tether_play = Some(30.);
    world
}
#[test]
fn one_millisecond_reset_preserves_pose_hag_and_follow_until_actual_view_offer_and_save_continues()
{
    let mut world = owner();
    let peer = owner();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let target = world.session_view.target;
    let view = world.session_view.camera.view_matrix;
    let eye = world.session_view.camera.camera_position;
    let hag = serde_json::to_vec(
        &world
            .session_view
            .camera
            .native_camera
            .capture_continuation(),
    )
    .unwrap();
    let cache = world.session_view.camera.constraints;
    world.admit_camera_reset_motion(CameraResetRequest {
        position: target,
        duration_seconds: 0.001,
        ease_in_seconds: 0.,
        ease_out_seconds: 0.,
    });
    let camera = &world.session_view.camera;
    assert_eq!(world.session_view.target, target);
    assert_eq!(camera.camera_yaw_radians, 0.6);
    assert_eq!(
        camera.camera_pitch_radians, 0.2,
        "reset targets FXpitch, not orbit pitch"
    );
    assert_eq!(camera.camera_fx_pitch, 0.4);
    assert_eq!(camera.camera_pitch_target, Some(1.));
    assert_eq!(camera.camera_zoom, 0.7);
    assert!(camera.camera_zoom_target.is_some());
    assert_eq!(
        (camera.camera_pitch_frame, camera.camera_zoom_frame),
        (0, 0)
    );
    assert_eq!(
        (camera.camera_pitch_duration, camera.camera_zoom_duration),
        (0.001, 0.001)
    );
    assert_eq!(camera.view_matrix, view);
    assert_eq!(camera.camera_position, eye);
    assert_eq!(camera.script_camera_shakers[0].elapsed_seconds, 0.);
    assert_eq!(
        serde_json::to_vec(&camera.native_camera.capture_continuation()).unwrap(),
        hag
    );
    assert_eq!(
        camera.constraints, cache,
        "reset does not invalidate the source constraint cache"
    );
    assert!(
        world.session_view.scripted.motion.active,
        "source rearms after the instant one-ms substep"
    );
    assert!(!world.session_view.scripted.path_arrived);
    assert_eq!(world.session_view.scripted.motion.angles[2], 0.);
    assert_eq!(
        world.session_view.scripted.camera_follow_target,
        Some(ObjectId(42))
    );
    assert_eq!(world.session_view.scripted.camera_tether_play, Some(30.));
    let saved = world.capture_camera_continuation().unwrap();
    let mut restored = owner();
    restored.admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap());
    for _ in 0..3 {
        world.headless_camera_view_offer(33);
        restored.headless_camera_view_offer(33);
        assert_eq!(
            world.capture_camera_continuation().unwrap(),
            restored.capture_camera_continuation().unwrap()
        );
    }
    assert_eq!(world.session_view.camera.camera_fx_pitch, 1.);
    assert!(world.session_view.camera.camera_zoom_target.is_none());
    assert!(world.session_view.camera.camera_pitch_target.is_none());
    assert_eq!(world.session_view.camera.camera_pitch_radians, 0.2);
    assert_eq!(
        world.session_view.camera.script_camera_shakers[0].elapsed_seconds,
        0.
    );
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}
#[test]
fn authored_reset_intake_and_direct_reset_share_motion_and_do_not_clear_follow() {
    let mut direct = owner();
    let mut authored = owner();
    let request = CameraResetRequest {
        position: Vec3::new(220., 0., 320.),
        duration_seconds: 2.5,
        ease_in_seconds: 0.2,
        ease_out_seconds: 0.4,
    };
    direct.admit_camera_reset_motion(request.clone());
    authored.mission_scripts.push_camera_reset(request);
    authored.apply_script_action_requests();
    assert_eq!(
        direct.capture_camera_continuation().unwrap(),
        authored.capture_camera_continuation().unwrap()
    );
    assert_eq!(
        authored.session_view.scripted.camera_follow_target,
        Some(ObjectId(42))
    );
    assert_eq!(authored.session_view.camera.camera_pitch_target, Some(1.));
    assert_eq!(authored.session_view.camera.camera_pitch_radians, 0.2);
    assert_eq!(authored.session_view.camera.camera_fx_pitch, 0.4);
    assert_eq!(authored.session_view.scripted.motion.angles[2], 0.);
}
