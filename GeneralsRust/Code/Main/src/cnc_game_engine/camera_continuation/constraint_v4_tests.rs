use super::*;
#[test]
fn native_constraint_v4_roundtrip_and_v3_input_preserve_native85() {
    let camera = super::super::tests::fixture();
    let mut cache = crate::presentation_frame::camera::constraints::CameraConstraints::default();
    cache
        .recalculate(Vec3::ZERO, Vec3::new(1000., 0., 800.), 37.)
        .unwrap();
    let bytes =
        encode_constrained_camera(&camera, camera.completed_view(), None, true, cache).unwrap();
    let decoded = decode_native_camera(&bytes).unwrap();
    assert_eq!(decoded.1.constraints, cache);
    assert!(decoded.1.follow_snap_immediate);
    assert_eq!(
        codec().serialize(&decoded.0).unwrap(),
        codec().serialize(&camera).unwrap()
    );
    let old = decode_native_camera(
        &encode_follow_camera(&camera, camera.completed_view(), None, true).unwrap(),
    )
    .unwrap();
    assert!(!old.1.constraints.is_valid());
    assert!(old.1.follow_snap_immediate);
    let mut invalid = bytes;
    invalid.truncate(invalid.len() - 1);
    assert!(decode_native_camera(&invalid).is_err());
}

#[test]
fn native_logic_constraint_disagreement_and_nonfinite_wire_reject_detached() {
    let camera = super::super::tests::fixture();
    let mut cache = crate::presentation_frame::camera::constraints::CameraConstraints::default();
    cache
        .recalculate(Vec3::ZERO, Vec3::new(1000., 0., 800.), 37.)
        .unwrap();
    let bytes =
        encode_constrained_camera(&camera, camera.completed_view(), None, false, cache).unwrap();
    let native = decode_native_camera(&bytes).unwrap();
    let world = crate::game_logic::GameLogic::new();
    let before = world.capture_camera_continuation().unwrap();
    let logic = crate::game_logic::GameLogic::decode_camera_continuation(&before).unwrap();
    assert!(
        native
            .validate_logic_continuation(&logic)
            .unwrap_err()
            .contains("constraint")
    );
    let mut invalid = bytes;
    let len = invalid.len();
    invalid[len - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(decode_native_camera(&invalid).is_err());
    assert_eq!(before, world.capture_camera_continuation().unwrap());
}
