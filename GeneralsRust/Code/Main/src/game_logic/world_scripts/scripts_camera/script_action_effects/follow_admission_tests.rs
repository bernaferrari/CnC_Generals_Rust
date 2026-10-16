use super::*;
use gamelogic::scripting::engine::ScriptActionHandler;
#[test]
fn public_follow_requests_latch_snap_until_exact_receiving_view_admission() {
    let mut a = GameLogic::new();
    let peer = GameLogic::new();
    let peer_before = peer.capture_camera_continuation().unwrap();
    a.session_view.scripted.motion.active = true;
    a.session_view.scripted.path_arrived = true;
    a.session_view.camera.camera_yaw_target = Some(1.2);
    let handler = MissionScriptActionHandler::new(a.mission_scripts.clone());
    handler.camera_follow_object(42, true).unwrap();
    a.apply_script_action_requests();
    assert_eq!(a.camera_follow_object_id(), Some(ObjectId(42)));
    assert!(a.session_view.scripted.motion.active && a.session_view.scripted.path_arrived);
    assert_eq!(a.session_view.camera.camera_yaw_target, Some(1.2));
    assert_eq!(a.session_view.target, Vec3::ZERO);
    assert!(a.session_view.camera.camera_follow_snap_immediate);
    assert!(
        a.take_camera_focus_request().is_none(),
        "setCameraLock never invokes LookAt"
    );
    let facts = crate::presentation_frame::follow::FollowTargetFacts {
        id: ObjectId(42),
        position: Vec3::new(80., 37., 90.),
        airborne_orientation: Some(2.),
    };
    a.session_view
        .admit_camera_follow(Some(ObjectId(42)), Some(facts), 10., false)
        .unwrap();
    assert_eq!(a.session_view.target, Vec3::new(80., 0., 90.));
    assert!(!a.session_view.scripted.motion.active && !a.session_view.scripted.path_arrived);
    assert_eq!(
        a.session_view.camera.camera_yaw_target,
        Some(1.2),
        "source independent rotation follows lock admission"
    );
    assert!(!a.session_view.camera.camera_follow_snap_immediate);
    handler.camera_tether_object(42, true, 9.).unwrap();
    a.apply_script_action_requests();
    assert_eq!(a.peek_camera_tether_play(), Some(9.));
    handler.camera_follow_object(42, false).unwrap();
    a.apply_script_action_requests();
    assert_eq!(a.peek_camera_tether_play(), None);
    assert!(
        a.session_view.camera.camera_follow_snap_immediate,
        "false snap does not clear earlier source latch"
    );
    handler.stop_camera_follow().unwrap();
    a.apply_script_action_requests();
    assert_eq!(a.camera_follow_object_id(), None);
    assert!(a.session_view.camera.camera_follow_snap_immediate);
    handler.camera_follow_object(42, false).unwrap();
    a.apply_script_action_requests();
    assert!(a.session_view.camera.camera_follow_snap_immediate);
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
}
