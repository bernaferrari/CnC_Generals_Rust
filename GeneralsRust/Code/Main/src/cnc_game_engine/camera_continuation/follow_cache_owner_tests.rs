use super::*;
#[test]
fn canonical_same_id_follow_resets_tether_and_stop_observes_new_script_lock() {
    let mut a = crate::game_logic::GameLogic::new();
    let mut peer = crate::game_logic::GameLogic::new();
    let id = crate::game_logic::ObjectId(42);
    a.set_camera_tether_object(id, true, 9.);
    peer.set_camera_tether_object(id, true, 7.);
    a.apply_session_control_op(crate::game_logic::SessionControlOp::SetCameraFollow {
        id: Some(id),
    });
    assert_eq!(a.camera_follow_object_id(), Some(id));
    assert_eq!(a.peek_camera_tether_play(), None);
    assert!(a.session_view.camera.camera_follow_snap_immediate);
    // A newly authored lock must be clearable even if the old frozen observation
    // was None. No residual ID gate participates in this receiving operation.
    a.set_camera_tether_object(id, false, 3.);
    a.apply_session_control_op(crate::game_logic::SessionControlOp::SetCameraFollow { id: None });
    assert_eq!(a.camera_follow_object_id(), None);
    assert_eq!(a.peek_camera_tether_play(), None);
    assert!(a.session_view.camera.camera_follow_snap_immediate);
    assert_eq!(peer.camera_follow_object_id(), Some(id));
    assert_eq!(peer.peek_camera_tether_play(), Some(7.));
}
#[test]
fn historical_native85_stale_follow_slots_are_validated_but_cannot_reselect_owner() {
    let mut camera = fixture();
    let mut world = crate::game_logic::GameLogic::new();
    let id = crate::game_logic::ObjectId(42);
    fixture_on_owner(&mut camera, &mut world);
    world.set_camera_tether_object(id, false, 9.);
    camera.host_match_camera_follow_active = Some(false);
    camera.host_match_camera_follow_position = Some([800., 370., 900.]);
    camera.host_match_camera_follow_id = Some(None);
    let bytes = codec().serialize(&camera).unwrap();
    let stale = super::super::filter::decode_native_camera(&bytes).unwrap();
    // This is the exact canonical projection operation used by native admission;
    // the obsolete observations remain wire-only and cannot alter the lock.
    stale
        .0
        .admit_session_camera_fields(&mut world.session_view.camera);
    assert_eq!(world.camera_follow_object_id(), Some(id));
    assert_eq!(world.peek_camera_tether_play(), Some(9.));
    assert_eq!(
        codec().serialize(&stale.0).unwrap(),
        bytes,
        "Native85 field order/types remain frozen"
    );
}
