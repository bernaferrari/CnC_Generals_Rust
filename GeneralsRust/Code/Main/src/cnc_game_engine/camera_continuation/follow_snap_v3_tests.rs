use super::*;
#[test]
fn native_v3_pending_snap_preserves_native85_and_v2_false_default() {
    let camera = super::super::tests::fixture();
    let completed = camera.completed_view();
    let frozen = codec().serialize(&camera).unwrap();
    let bytes = encode_follow_camera(&camera, completed, None, true).unwrap();
    let decoded = decode_native_camera(&bytes).unwrap();
    assert!(decoded.1.follow_snap_immediate);
    assert_eq!(frozen, codec().serialize(&decoded.0).unwrap());
    let v2 = encode_completed_camera(&camera, completed, None).unwrap();
    assert!(!decode_native_camera(&v2).unwrap().1.follow_snap_immediate);
    let mut invalid = bytes;
    *invalid.last_mut().unwrap() = 2;
    assert!(decode_native_camera(&invalid).is_err());
}

#[test]
fn native_logic_pending_snap_disagreement_rejects_before_receiver_mutation() {
    let camera = super::super::tests::fixture();
    let native = decode_native_camera(
        &encode_follow_camera(&camera, camera.completed_view(), None, true).unwrap(),
    )
    .unwrap();
    let world = crate::game_logic::GameLogic::new();
    let before = world.capture_camera_continuation().unwrap();
    let logic = crate::game_logic::GameLogic::decode_camera_continuation(&before).unwrap();
    assert!(
        native
            .validate_logic_continuation(&logic)
            .unwrap_err()
            .contains("pending follow snap")
    );
    assert_eq!(before, world.capture_camera_continuation().unwrap());
}
