//! The real LookAt owner stays local; C++ resetModes has a narrow contract.
use super::*;

fn bookmark() -> CameraViewLocation {
    CameraViewLocation {
        pos: Vec3::new(10., 20., 30.),
        yaw: 0.25,
        pitch: 0.5,
        zoom: 0.75,
    }
}

#[test]
fn construction_and_peer_operations_preserve_the_first_look_at_owner() {
    let mut first = LookAtHostModes::default();
    first.views[3] = Some(bookmark());
    first.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
    first.camera_follow_lock_broken = true;
    first.prev_cursor = Some("Move");
    first.mouse_locked = true;
    first.letterbox_os_cursor_hidden = true;
    first.lookat_stamp_mouse_activity(40);
    first.wheel_stopped_scroll = true;
    let mut peer = LookAtHostModes::default();
    assert!(peer.views.iter().all(Option::is_none));
    assert!(!peer.look_at_host_mouse_locked());
    assert!(!peer.camera_follow_lock_broken);
    peer.views[3] = Some(CameraViewLocation {
        zoom: 1.25,
        ..bookmark()
    });
    peer.lookat_note_mouse_moved(50, (8., 9.));
    peer.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
    peer.lookat_note_raw_key_activity();
    assert_eq!(first.views[3].unwrap().zoom, 0.75);
    assert_eq!(first.last_mouse_move_frame, 40);
    assert!(first.wheel_stopped_scroll);
    assert!(first.look_at_host_mouse_locked());
    assert!(first.camera_follow_lock_broken && first.letterbox_os_cursor_hidden);
    assert_eq!(first.prev_cursor, Some("Move"));
    assert!(matches!(
        first.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::LookAt
    ));
    assert!(matches!(
        peer.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::Set
    ));
    assert!(matches!(
        first.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::None
    ));
}

#[test]
fn wheel_and_mouse_activity_belong_to_the_receiver() {
    let mut first = LookAtHostModes::default();
    let mut peer = LookAtHostModes::default();
    first.last_mouse_pixel = (10., 10.);
    first.wheel_stopped_scroll = true;
    first.lookat_note_mouse_moved(50, (10., 10.));
    assert_eq!(first.last_mouse_move_frame, 0);
    assert!(
        first.wheel_stopped_scroll,
        "identical pixel is not a RAW_POSITION restart"
    );
    first.lookat_stamp_mouse_activity(50);
    assert!(first.lookat_has_mouse_moved_recently(80));
    assert!(!first.lookat_has_mouse_moved_recently(81));
    assert_eq!(peer.last_mouse_move_frame, 0);
    peer.lookat_note_mouse_moved(70, (1., 2.));
    assert!(first.wheel_stopped_scroll);
    first.lookat_note_raw_key_activity();
    assert!(!first.wheel_stopped_scroll);
    first.wheel_stopped_scroll = true;
    first.lookat_note_mouse_moved(90, (11., 10.));
    assert!(!first.wheel_stopped_scroll);
    assert_eq!(
        (peer.last_mouse_move_frame, peer.last_mouse_pixel),
        (70, (1., 2.))
    );
}

#[test]
fn reset_modes_clears_only_original_transient_modes() {
    let mut owner = LookAtHostModes::default();
    owner.mmb_original_anchor = Some((3., 4.));
    owner.mmb_press_frame = 41;
    owner.scroll_type = LookAtScrollType::Key;
    owner.views[0] = Some(bookmark());
    owner.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
    owner.lookat_stamp_mouse_activity(90);
    owner.last_mouse_pixel = (8., 7.);
    owner.wheel_stopped_scroll = true;
    owner.camera_follow_lock_broken = true;
    owner.prev_cursor = Some("Attack");
    owner.mouse_locked = true;
    owner.letterbox_os_cursor_hidden = true;
    owner.reset_modes();
    assert_eq!(owner.mmb_original_anchor, None);
    assert_eq!(owner.mmb_press_frame, 0);
    assert!(!owner.look_at_host_is_scrolling());
    assert_eq!(owner.views[0].unwrap().pos, bookmark().pos);
    assert_eq!(
        (owner.last_mouse_move_frame, owner.last_mouse_pixel),
        (90, (8., 7.))
    );
    assert!(owner.wheel_stopped_scroll && owner.camera_follow_lock_broken);
    assert_eq!(owner.prev_cursor, Some("Attack"));
    assert!(owner.mouse_locked && owner.letterbox_os_cursor_hidden);
    assert!(matches!(
        owner.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::Set
    ));
}

#[test]
fn cancellation_priority_and_scroll_resolution_do_not_share_a_latch() {
    let mut owner = LookAtHostModes::default();
    let mut peer = LookAtHostModes::default();
    owner.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
    owner.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
    owner.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::None);
    peer.note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
    owner.wheel_stopped_scroll = true;
    assert_eq!(
        lookat_resolve_scroll_type(
            owner.wheel_stopped_scroll,
            LookAtScrollType::None,
            true,
            false,
            false,
            true,
            true
        ),
        LookAtScrollType::None
    );
    assert_eq!(
        lookat_resolve_scroll_type(
            peer.wheel_stopped_scroll,
            LookAtScrollType::None,
            true,
            false,
            false,
            true,
            true
        ),
        LookAtScrollType::Key
    );
    assert!(matches!(
        owner.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::Set
    ));
    assert!(matches!(
        peer.take_scripted_camera_player_cancel(),
        ScriptedCameraPlayerCancel::LookAt
    ));
}

#[test]
fn recent_mouse_rewind_persists_without_resurrecting_old_activity() {
    let mut owner = LookAtHostModes::default();
    let mut peer = LookAtHostModes::default();
    owner.lookat_stamp_mouse_activity(100);
    peer.lookat_stamp_mouse_activity(50);
    assert!(owner.lookat_has_mouse_moved_recently(10));
    assert_eq!(owner.last_mouse_move_frame, 0);
    assert!(!owner.lookat_has_mouse_moved_recently(60));
    assert!(!owner.lookat_has_mouse_moved_recently(100));
    assert!(peer.lookat_has_mouse_moved_recently(60));
    assert_eq!(peer.last_mouse_move_frame, 50);
    owner.lookat_stamp_mouse_activity(101);
    assert!(owner.lookat_has_mouse_moved_recently(131));
    assert!(!owner.lookat_has_mouse_moved_recently(132));
}

#[test]
fn recent_mouse_deadline_uses_original_unsigned_wrapping() {
    let mut owner = LookAtHostModes::default();
    owner.lookat_stamp_mouse_activity(u32::MAX - 10);
    assert!(!owner.lookat_has_mouse_moved_recently(u32::MAX - 5));
    assert_eq!(owner.last_mouse_move_frame, u32::MAX - 10);
    // Once the clock wraps, the source branch persists the restart reset.
    assert!(owner.lookat_has_mouse_moved_recently(0));
    assert_eq!(owner.last_mouse_move_frame, 0);
    assert!(owner.lookat_has_mouse_moved_recently(30));
    assert!(!owner.lookat_has_mouse_moved_recently(31));
}
