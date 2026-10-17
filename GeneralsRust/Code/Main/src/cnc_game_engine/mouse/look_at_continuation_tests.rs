//! Native continuation calls the real local LookAt owner capture/admit methods.
use super::*;
use bincode_legacy::Options;

#[test]
fn bookmarks_cancel_scroll_and_input_recency_survive_detached_admission() {
    let mut original = LookAtHostModes::default();
    original.views[6] = Some(CameraViewLocation {
        pos: Vec3::new(21., -3., 90.),
        yaw: 7.0,
        pitch: -0.4,
        zoom: 0.65,
    });
    original.prev_cursor = Some("AttackObj");
    original.mouse_locked = true;
    original.scroll_type = LookAtScrollType::Rmb;
    original.mmb_original_anchor = Some((41., 52.));
    original.mmb_press_frame = 31;
    original.lookat_stamp_mouse_activity(40);
    original.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
    original.wheel_stopped_scroll = true;
    original.letterbox_os_cursor_hidden = true;
    let encoded = bincode_legacy::DefaultOptions::new()
        .serialize(&original.capture_continuation())
        .unwrap();
    let decoded: LookAtContinuation = bincode_legacy::DefaultOptions::new()
        .deserialize(&encoded)
        .unwrap();
    decoded.validate().unwrap();
    let mut restored = LookAtHostModes::default();
    restored.admit_continuation(decoded);
    let mut peer = LookAtHostModes::default();
    peer.reset_modes();
    assert_eq!(restored.views[6], original.views[6]);
    assert_eq!(restored.mmb_original_anchor, Some((41., 52.)));
    assert_eq!(restored.mmb_press_frame, 31);
    assert_eq!(restored.prev_cursor, Some("AttackObj"));
    assert!(restored.look_at_host_mouse_locked() && restored.look_at_host_is_scrolling());
    assert!(restored.wheel_stopped_scroll);
    assert!(restored.letterbox_os_cursor_hidden);
    assert!(restored.lookat_has_mouse_moved_recently(70));
    assert!(!restored.lookat_has_mouse_moved_recently(71));
    assert_eq!(
        restored.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::LookAt
    );
    assert_eq!(
        restored.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::None
    );
    assert!(peer.views.iter().all(Option::is_none));
}

#[test]
fn malformed_bookmark_or_cursor_does_not_mutate_receiving_modes() {
    let mut receiver = LookAtHostModes::default();
    receiver.lookat_stamp_mouse_activity(17);
    let mut decoded = receiver.capture_continuation();
    decoded.prev_cursor = Some("unknown".into());
    assert!(decoded.validate().is_err());
    let mut decoded = receiver.capture_continuation();
    decoded.views[0] = Some(CameraViewLocation {
        pos: Vec3::ZERO,
        yaw: f32::INFINITY,
        pitch: 0.,
        zoom: 1.,
    });
    assert!(decoded.validate().is_err());
    assert_eq!(receiver.last_mouse_move_frame, 17);
    assert!(receiver.views.iter().all(Option::is_none));
}

#[test]
fn historical_broken_lock_observation_is_ignored_without_blocking_canonical_relock() {
    let mut dto = LookAtHostModes::default().capture_continuation();
    dto.camera_follow_lock_broken = true;
    let bytes = bincode_legacy::DefaultOptions::new()
        .serialize(&dto)
        .unwrap();
    let decoded: LookAtContinuation = bincode_legacy::DefaultOptions::new()
        .deserialize(&bytes)
        .unwrap();
    decoded.validate().unwrap();
    let mut receiver = LookAtHostModes::default();
    receiver.admit_continuation(decoded);
    assert!(!receiver.capture_continuation().camera_follow_lock_broken);
    let mut world = crate::game_logic::GameLogic::new();
    let id = crate::game_logic::ObjectId(42);
    world.set_camera_follow_object(Some(id));
    world.apply_session_control_op(crate::game_logic::SessionControlOp::SetCameraFollow {
        id: None,
    });
    // Authored re-lock before any frame showing None is a new valid lock.
    world.set_camera_tether_object(id, true, 9.);
    let facts = crate::presentation_frame::follow::FollowTargetFacts {
        id,
        position: Vec3::new(80., 37., 90.),
        airborne_orientation: None,
    };
    assert_eq!(
        world
            .session_view
            .admit_camera_follow(Some(id), Some(facts), 10., false),
        Ok(crate::presentation_frame::follow::FollowAdmission::Followed)
    );
    assert_eq!(world.session_view.target, Vec3::new(80., 0., 90.));
    assert!(!world.session_view.camera.camera_follow_snap_immediate);
    let native = include_str!("../camera_view_effects.rs");
    assert!(!native.contains("look_at_player_broke_camera_lock"));
    assert!(native.contains("self.apply_camera_follow_or_tether(id, facts, paused)"));
}
