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
fn v9_fps_continuation_wraps_v8_and_preserves_signed_enabled_state() {
    let v8 = bytes(&fixture());
    let encoded = tagged(
        codec()
            .serialize(&FpsCameraState {
                version: 9,
                camera: v8,
                limiter_enabled: true,
                limiter_rate: -17,
            })
            .unwrap(),
    )
    .unwrap();
    let decoded = decode_native_camera(&encoded).unwrap();
    assert_eq!(decoded.1.fps_limiter, Some((true, -17)));
    let mut malformed: FpsCameraState = codec().deserialize(&encoded[TAG.len()..]).unwrap();
    malformed.version = 8;
    assert!(
        decode_native_camera(&tagged(codec().serialize(&malformed).unwrap()).unwrap()).is_err()
    );
    let nested_v9 = tagged(
        codec()
            .serialize(&FpsCameraState {
                version: 9,
                camera: bytes(&fixture()),
                limiter_enabled: false,
                limiter_rate: 1,
            })
            .unwrap(),
    )
    .unwrap();
    let recursive = tagged(
        codec()
            .serialize(&FpsCameraState {
                version: 9,
                camera: nested_v9,
                limiter_enabled: true,
                limiter_rate: 2,
            })
            .unwrap(),
    )
    .unwrap();
    assert!(decode_native_camera(&recursive).is_err());
}

#[test]
fn v9_limiter_owner_rejects_mismatched_legacy_v8_rate_projection() {
    let mut wire = fixture();
    let legacy_camera: &mut Vec<u8> = &mut wire.base.base.base.base.base.camera;
    let mut projection: super::super::CameraState = codec().deserialize(legacy_camera).unwrap();
    projection.script_fps_limit = Some(27);
    *legacy_camera = codec().serialize(&projection).unwrap();
    let v8 = bytes(&wire);

    let matching = tagged(
        codec()
            .serialize(&FpsCameraState {
                version: 9,
                camera: v8.clone(),
                limiter_enabled: true,
                limiter_rate: 27,
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        decode_native_camera(&matching).unwrap().1.fps_limiter,
        Some((true, 27))
    );

    let mismatched = tagged(
        codec()
            .serialize(&FpsCameraState {
                version: 9,
                camera: v8,
                limiter_enabled: true,
                limiter_rate: 45,
            })
            .unwrap(),
    )
    .unwrap();
    assert!(decode_native_camera(&mismatched).is_err());
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
