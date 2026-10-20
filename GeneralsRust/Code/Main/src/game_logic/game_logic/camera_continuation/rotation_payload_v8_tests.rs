//! V8 complete owner state versus frozen V7 observations.
use super::*;
use crate::presentation_frame::camera::registers::{
    RotationKind, RotationRegisters, RotationRegistersState,
};
use crate::presentation_frame::camera::rotation::{RotationTime, RotationTiming};

#[derive(Serialize, Deserialize)]
struct RawRegisters {
    first: Option<u32>,
    second: Option<u32>,
    target_yz: Option<[u32; 2]>,
    kind: RotationKind,
    active: bool,
    hold_frames: u32,
}
fn raw(value: RawRegisters) -> RotationRegistersState {
    codec()
        .deserialize(&codec().serialize(&value).unwrap())
        .unwrap()
}
fn tracking() -> GameLogic {
    let mut owner = GameLogic::new();
    owner.session_view.camera.rotation =
        RotationTiming::milliseconds(132, 0., 0., RotationTime::constant(1));
    owner
        .session_view
        .camera
        .rotation_registers
        .begin_tracking(7, 0);
    owner.mission_scripts.set_camera_movement_finished(false);
    owner
}
fn encoded(value: &EnvelopeV8) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend(8u32.to_le_bytes());
    bytes.extend(codec().serialize(value).unwrap());
    bytes
}

#[test]
fn v8_unavailable_cache_and_v7_explicit_zero_remain_distinct_with_same_frozen_projection() {
    let source = tracking();
    assert_eq!(
        source
            .session_view
            .camera
            .rotation_registers
            .cached_target(),
        None
    );
    let bytes = source.capture_camera_continuation().unwrap();
    let envelope: EnvelopeV8 = codec().deserialize(&bytes[8..]).unwrap();
    let frozen = codec().serialize(&envelope.base).unwrap();
    let mut old = MAGIC.to_vec();
    old.extend(7u32.to_le_bytes());
    old.extend(&frozen);
    let mut restored_v8 = GameLogic::new();
    restored_v8
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&bytes).unwrap())
        .unwrap();
    let mut restored_v7 = GameLogic::new();
    restored_v7
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&old).unwrap())
        .unwrap();
    assert_eq!(
        restored_v8
            .session_view
            .camera
            .rotation_registers
            .cached_target(),
        None
    );
    assert_eq!(
        restored_v7
            .session_view
            .camera
            .rotation_registers
            .cached_target(),
        Some(Vec3::ZERO)
    );
    for owner in [&restored_v7, &restored_v8] {
        let recaptured = owner.capture_camera_continuation().unwrap();
        let projected: EnvelopeV8 = codec().deserialize(&recaptured[8..]).unwrap();
        assert_eq!(codec().serialize(&projected.base).unwrap(), frozen);
    }
    assert_ne!(
        restored_v7.capture_camera_continuation().unwrap(),
        restored_v8.capture_camera_continuation().unwrap()
    );
}

#[test]
fn malformed_v8_registers_and_projection_reject_before_receiving_mutation() {
    let source = tracking();
    let valid = source.capture_camera_continuation().unwrap();
    let mut receiver = GameLogic::new();
    receiver
        .session_view
        .install_rotate(0.125, 0.231, 0., 0., 1.);
    let before = receiver.capture_camera_continuation().unwrap();
    let peer = tracking();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let invalid = [
        raw(RawRegisters {
            first: Some(7),
            second: None,
            target_yz: Some([0, 0]),
            kind: RotationKind::Tracking,
            active: true,
            hold_frames: 0,
        }),
        raw(RawRegisters {
            first: Some(7),
            second: Some(f32::NAN.to_bits()),
            target_yz: Some([0, 0]),
            kind: RotationKind::Tracking,
            active: true,
            hold_frames: 0,
        }),
        raw(RawRegisters {
            first: Some(7),
            second: None,
            target_yz: None,
            kind: RotationKind::Tracking,
            active: true,
            hold_frames: u32::MAX,
        }),
        {
            let mut value = RotationRegisters::default();
            value.begin_scalar(0.5, 1.5);
            value.capture()
        },
    ];
    for registers in invalid {
        let mut wire: EnvelopeV8 = codec().deserialize(&valid[8..]).unwrap();
        wire.rotation_registers = registers.clone();
        assert!(GameLogic::decode_camera_continuation(&encoded(&wire)).is_err());
        let mut direct = GameLogic::decode_camera_continuation(&valid).unwrap();
        direct.owned_extras.rotation_registers = Some(registers);
        assert!(receiver.admit_camera_continuation(direct).is_err());
        assert_eq!(receiver.capture_camera_continuation().unwrap(), before);
    }
    let mut missing = GameLogic::decode_camera_continuation(&valid).unwrap();
    missing.owned_extras.rotation_timing = None;
    assert!(receiver.admit_camera_continuation(missing).is_err());
    assert_eq!(receiver.capture_camera_continuation().unwrap(), before);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}

#[test]
fn raw_scalar_object_id_overlay_bits_are_not_rejected_as_nonfinite_angles() {
    let state = raw(RawRegisters {
        first: Some(f32::NAN.to_bits()),
        second: Some(0),
        target_yz: Some([50f32.to_bits(), 15f32.to_bits()]),
        kind: RotationKind::Scalar,
        active: true,
        hold_frames: 0,
    });
    assert!(
        state.validate().is_ok(),
        "source partial-write union payload may contain non-angle first bits"
    );
}
