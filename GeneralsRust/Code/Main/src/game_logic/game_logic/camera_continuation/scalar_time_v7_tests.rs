use super::*;

#[test]
fn v7_scalar_time_roundtrip_and_v6_current_speed_migration_leave_historical_wire_unchanged() {
    let mut source = GameLogic::new();
    source.visual_speed_multiplier = 3.;
    source.session_view.install_rotate(0.25, 0.132, 0., 0., 3.);
    source.session_view.camera.rotation.time.end = 7;
    let bytes = source.capture_camera_continuation().unwrap();
    let current: EnvelopeV7 = codec().deserialize(&bytes[8..]).unwrap();
    assert_eq!(current.rotation_timing.time.start, 3);
    assert_eq!(current.rotation_timing.time.end, 7);
    let old_record = current.base.base.base.base.base.legacy_record.clone();
    let old_camera = codec()
        .serialize(&current.base.base.base.base.base.camera)
        .unwrap();
    let mut v6 = MAGIC.to_vec();
    v6.extend(6u32.to_le_bytes());
    v6.extend(codec().serialize(&current.base).unwrap());
    let decoded = GameLogic::decode_camera_continuation(&v6).unwrap();
    assert_eq!(codec().serialize(&decoded).unwrap(), old_record);
    assert_eq!(
        codec()
            .serialize(decoded.owned_camera_state().unwrap())
            .unwrap(),
        old_camera
    );
    let mut migrated = GameLogic::new();
    migrated.admit_camera_continuation(decoded).unwrap();
    assert_eq!(
        migrated.session_view.camera.rotation.time,
        crate::presentation_frame::camera::rotation::RotationTime::constant(3)
    );
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&bytes).unwrap())
        .unwrap();
    assert_eq!(restored.session_view.camera.rotation.time.end, 7);
    let before = restored.capture_camera_continuation().unwrap();
    for malformed in [
        &bytes[..bytes.len() - 1],
        &[bytes.as_slice(), &[0]].concat(),
    ] {
        assert!(GameLogic::decode_camera_continuation(malformed).is_err());
        assert_eq!(restored.capture_camera_continuation().unwrap(), before);
    }
}

#[test]
fn historical_scalar_raw_curve_bits_survive_old_envelope_admission() {
    // Historical validation accepted finite raw values, not just whole ms.
    // Every old wrapper retains its original positional shape.
    use crate::presentation_frame::camera::rotation::{RotationTime, RotationTiming};
    for duration in [0., -0., f32::from_bits(0x3f808001), -0.125, 1.999] {
        let mut source = GameLogic::new();
        source.session_view.camera.rotation =
            RotationTiming::historical(duration, -0.125, 0.033, RotationTime::constant(1));
        let bytes = source.capture_camera_continuation().unwrap();
        let current: EnvelopeV7 = codec().deserialize(&bytes[8..]).unwrap();
        let expected = codec()
            .serialize(&current.base.base.base.base.base.camera)
            .unwrap();
        let payloads = [
            (
                2,
                codec()
                    .serialize(&current.base.base.base.base.base)
                    .unwrap(),
            ),
            (3, codec().serialize(&current.base.base.base.base).unwrap()),
            (4, codec().serialize(&current.base.base.base).unwrap()),
            (5, codec().serialize(&current.base.base).unwrap()),
            (6, codec().serialize(&current.base).unwrap()),
        ];
        for (version, payload) in payloads {
            let mut old = MAGIC.to_vec();
            old.extend(u32::to_le_bytes(version));
            old.extend(payload);
            let decoded = GameLogic::decode_camera_continuation(&old).unwrap();
            let mut restored = GameLogic::new();
            restored.admit_camera_continuation(decoded).unwrap();
            assert_eq!(
                restored
                    .session_view
                    .camera
                    .rotation
                    .duration_seconds()
                    .to_bits(),
                duration.to_bits()
            );
            assert_eq!(
                codec()
                    .serialize(&SessionCameraState::capture(&restored.session_view.camera))
                    .unwrap(),
                expected
            );
        }
    }
}

#[test]
fn authored_1999_milliseconds_and_historical_tracking_wire_remain_exact() {
    use crate::presentation_frame::camera::rotation::{RotationTime, RotationTiming};
    let timing =
        RotationTiming::milliseconds(1999, 0.033, 0.066, RotationTime { start: 2, end: 5 });
    assert_eq!(timing.duration_ms(), 1999);
    assert_eq!(timing.turn_frames, 60);
    let runtime = CameraObjectTracking {
        hold_frames: 4,
        last_position: Vec3::new(1., 2., 3.),
    };
    let wire = TrackingV2::capture(&runtime, timing);
    let expected: Vec<u8> = [
        1999i32.to_le_bytes(),
        60u32.to_le_bytes(),
        4u32.to_le_bytes(),
        1f32.to_le_bytes(),
        2f32.to_le_bytes(),
        3f32.to_le_bytes(),
        2i32.to_le_bytes(),
        5i32.to_le_bytes(),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert_eq!(codec().serialize(&wire).unwrap(), expected);
    let mut source = GameLogic::new();
    source
        .session_view
        .install_rotate(0.25, 1.999, 0.033, 0.066, 2.);
    assert_eq!(source.session_view.camera.rotation.duration_ms(), 1999);
    let saved = source.capture_camera_continuation().unwrap();
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    assert_eq!(restored.session_view.camera.rotation.duration_ms(), 1999);
    assert_eq!(
        restored.session_view.camera.rotation,
        source.session_view.camera.rotation
    );
}

#[test]
fn near_zero_prefix_count_and_retained_curve_survive_save_and_shortened_expiry() {
    fn offer(world: &mut GameLogic) {
        world.headless_camera_view_offer(33).unwrap();
    }
    for (seconds, first_offers) in [(0.99, 1), (0.001, 10)] {
        let mut source = GameLogic::new();
        source.session_view.install_rotate(
            0.25,
            if first_offers == 1 { 0.132 } else { 0.99 },
            0.033,
            0.033,
            1.,
        );
        source.session_view.camera.rotation.time.end = 31;
        for _ in 0..first_offers {
            offer(&mut source);
        }
        let curve_before = source.session_view.camera.rotation;
        let frame_before = source.session_view.camera.camera_yaw_frame;
        let current_before = source.visual_speed_multiplier;
        let target = source.session_view.target;
        assert!(
            !source
                .session_view
                .install_look_toward(target, target, seconds, 0., 0., false, 99.)
        );
        assert_eq!(source.session_view.camera.camera_yaw_frame, frame_before);
        assert_eq!(
            source
                .session_view
                .camera
                .rotation
                .duration_seconds()
                .to_bits(),
            curve_before.duration_seconds().to_bits()
        );
        assert_eq!(
            source.session_view.camera.rotation.ease_in().to_bits(),
            curve_before.ease_in().to_bits()
        );
        assert_eq!(source.session_view.camera.rotation.time, curve_before.time);
        assert_eq!(
            source.session_view.camera.rotation.turn_frames,
            if first_offers == 1 { 30 } else { 1 }
        );
        let saved = source.capture_camera_continuation().unwrap();
        let mut restored = GameLogic::new();
        restored
            .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
            .unwrap();
        let mut peer = GameLogic::new();
        peer.session_view.install_rotate(1., 0.033, 0., 0., 17.);
        offer(&mut peer);
        offer(&mut source);
        offer(&mut restored);
        assert_eq!(
            source.capture_camera_continuation().unwrap(),
            restored.capture_camera_continuation().unwrap()
        );
        if first_offers == 1 {
            let factor =
                crate::presentation_frame::camera::motion::parabolic_ease(2. / 30., 0.25, 0.25);
            assert_eq!(
                source.visual_speed_multiplier,
                curve_before.time.sample(factor)
            );
            assert!(source.session_view.camera.camera_yaw_target.is_some());
        } else {
            assert_eq!(
                source.visual_speed_multiplier, current_before,
                "expired interpolation must not publish the endpoint"
            );
            assert!(source.session_view.camera.camera_yaw_target.is_none());
        }
    }
}

#[test]
fn malformed_owned_rotation_rejects_capture_and_direct_admission_without_mutation() {
    let mut source = GameLogic::new();
    let bytes = source.capture_camera_continuation().unwrap();
    let mut malformed = GameLogic::decode_camera_continuation(&bytes).unwrap();
    malformed
        .owned_extras
        .rotation_timing
        .as_mut()
        .unwrap()
        .turn_frames = 0;
    let mut receiving = GameLogic::new();
    let before = receiving.capture_camera_continuation().unwrap();
    assert!(receiving.admit_camera_continuation(malformed).is_err());
    assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
    source.session_view.camera.rotation.turn_frames = 0;
    assert!(source.capture_camera_continuation().is_err());
}

#[test]
fn inconsistent_tracking_timing_rejects_capture_and_direct_admission_without_mutation() {
    use crate::presentation_frame::camera::rotation::{RotationTime, RotationTiming};
    let mut source = GameLogic::new();
    source.session_view.camera.rotation =
        RotationTiming::milliseconds(132, 0., 0., RotationTime::constant(1));
    source.session_view.camera.camera_yaw_target = Some(0.2);
    source.session_view.scripted.object_tracking = Some(CameraObjectTracking {
        hold_frames: 4,
        last_position: Vec3::new(100., 0., 0.),
    });
    source.session_view.scripted.script_look_toward_object_id = Some(42);
    let saved = source.capture_camera_continuation().unwrap();
    let mut malformed = GameLogic::decode_camera_continuation(&saved).unwrap();
    malformed
        .owned_extras
        .rotation_timing
        .as_mut()
        .unwrap()
        .turn_frames = 999;
    // Keep the legacy remaining-time observation consistent with the malformed
    // count, so this specifically exercises the tracking count/curve contract.
    malformed.script_camera_rotate_remaining = (999 + 4) as f32 * 0.033;
    let mut receiving = GameLogic::new();
    let before = receiving.capture_camera_continuation().unwrap();
    assert!(
        receiving
            .admit_camera_continuation(malformed)
            .unwrap_err()
            .contains("tracking rotation timing")
    );
    assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
    source.session_view.camera.rotation.turn_frames = 999;
    assert!(
        source
            .capture_camera_continuation()
            .unwrap_err()
            .contains("tracking rotation timing")
    );
    let mut missing = GameLogic::decode_camera_continuation(&saved).unwrap();
    missing.owned_extras.rotation_timing = None;
    assert!(
        receiving
            .admit_camera_continuation(missing)
            .unwrap_err()
            .contains("missing its rotation timing")
    );
    assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
}

#[test]
fn old_tracking_integer_authority_still_admits_inconsistent_camera_float_observation() {
    use crate::presentation_frame::camera::rotation::{RotationTime, RotationTiming};
    let mut source = GameLogic::new();
    source.session_view.camera.rotation =
        RotationTiming::milliseconds(132, 0.033, 0.033, RotationTime { start: 1, end: 5 });
    source.session_view.camera.camera_yaw_target = Some(0.2);
    source.session_view.scripted.object_tracking = Some(CameraObjectTracking {
        hold_frames: 4,
        last_position: Vec3::new(100., 0., 0.),
    });
    source.session_view.scripted.script_look_toward_object_id = Some(42);
    let saved = source.capture_camera_continuation().unwrap();
    let mut current: EnvelopeV7 = codec().deserialize(&saved[8..]).unwrap();
    // V2-V6 used the explicit integer tracking payload for its actual turn,
    // despite accepting a different redundant raw camera-duration observation.
    // Construct that accepted old shape without changing its tracking record.
    source.session_view.camera.rotation =
        RotationTiming::historical(1.999, 0.033, 0.033, RotationTime { start: 1, end: 5 });
    current.base.base.base.base.base.camera =
        SessionCameraState::capture(&source.session_view.camera);
    let mut old = MAGIC.to_vec();
    old.extend(6u32.to_le_bytes());
    old.extend(codec().serialize(&current.base).unwrap());
    let decoded = GameLogic::decode_camera_continuation(&old).unwrap();
    assert_eq!(
        decoded
            .owned_camera_state()
            .unwrap()
            .historical_rotation(RotationTime::default())
            .duration_seconds()
            .to_bits(),
        1.999f32.to_bits()
    );
    let mut receiving = GameLogic::new();
    receiving.admit_camera_continuation(decoded).unwrap();
    assert_eq!(receiving.session_view.camera.rotation.duration_ms(), 132);
    assert_eq!(receiving.session_view.camera.rotation.turn_frames, 4);
    assert_eq!(
        receiving.session_view.camera.rotation.time,
        RotationTime { start: 1, end: 5 }
    );
    // Its new capture is internally consistent and decodes again.
    GameLogic::decode_camera_continuation(&receiving.capture_camera_continuation().unwrap())
        .unwrap();
}
