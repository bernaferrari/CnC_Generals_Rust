//! Native V8 codec uses complete canonical payload, without a GPU fixture.
use super::*;
use crate::presentation_frame::camera::registers::RotationRegisters;

fn fixture() -> RegisteredCameraState {
    let mut state = super::super::tests::fixture();
    let mut owner = crate::game_logic::GameLogic::new();
    super::super::tests::fixture_on_owner(&mut state, &mut owner);
    // Historical fields and canonical timing come from the same admitted owner.
    let mut defined = defined_camera(
        &state,
        state.completed_view(),
        None,
        false,
        owner.session_view.camera.constraints,
        owner.session_view.camera.settings,
        owner.session_view.camera.clip_policy,
        owner.session_view.camera.shake_policy,
    )
    .unwrap();
    defined.base.base.base.base.version = 8;
    RegisteredCameraState {
        base: ScalarCameraState {
            base: defined,
            rotation_timing: owner.session_view.camera.rotation,
        },
        rotation_registers: owner.session_view.camera.rotation_registers.capture(),
    }
}
fn bytes(value: &RegisteredCameraState) -> Vec<u8> {
    tagged(codec().serialize(value).unwrap()).unwrap()
}

#[test]
fn standalone_native_v8_codec_roundtrip_retains_complete_register_state() {
    let wire = fixture();
    let expected = wire.rotation_registers.clone();
    let encoded = bytes(&wire);
    let decoded = decode_native_camera(&encoded).unwrap();
    assert_eq!(decoded.1.rotation_registers, Some(expected));
    assert_eq!(decoded.1.rotation_timing, Some(wire.base.rotation_timing));
    let registered: RegisteredCameraState = codec().deserialize(&encoded[TAG.len()..]).unwrap();
    assert_eq!(
        bytes(&registered),
        encoded,
        "standalone native format is lossless"
    );
    for malformed in [
        &encoded[..encoded.len() - 1],
        &[encoded.as_slice(), &[0]].concat(),
    ] {
        assert!(decode_native_camera(malformed).is_err());
    }
}

#[test]
fn native_v8_mismatched_projection_or_timing_rejects_without_touching_receiving_owner() {
    let receiver = crate::game_logic::GameLogic::new();
    let before = receiver.capture_camera_continuation().unwrap();
    let mut wire = fixture();
    let mut wrong = RotationRegisters::default();
    wrong.begin_scalar(1.25, 2.5);
    wire.rotation_registers = wrong.capture();
    assert!(decode_native_camera(&bytes(&wire)).is_err());
    let mut wire = fixture();
    wire.base.rotation_timing.turn_frames = 0;
    assert!(decode_native_camera(&bytes(&wire)).is_err());
    assert_eq!(receiver.capture_camera_continuation().unwrap(), before);
}
