//! Wire/admission tests for the new owner record and input-only old DTOs.
use super::*;

fn captured_state() -> LogicCameraContinuation {
    let world = GameLogic::new();
    GameLogic::decode_camera_continuation(&world.capture_camera_continuation().unwrap()).unwrap()
}

fn v1_bytes(state: &LogicCameraContinuation) -> Vec<u8> {
    let payload = codec()
        .serialize(&EnvelopeV1 {
            legacy_record: codec().serialize(state).unwrap(),
            #[cfg(feature = "game_client")]
            filter: Some(codec().serialize(&state.owned_extras.filter).unwrap()),
            #[cfg(not(feature = "game_client"))]
            filter: None,
            pending_focus_previous_target: state.owned_extras.pending_focus_previous_target,
        })
        .unwrap();
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

fn v7_bytes_from_current(current: &[u8]) -> Vec<u8> {
    let current: EnvelopeV8 = codec().deserialize(&current[8..]).unwrap();
    let mut bytes = MAGIC.to_vec();
    bytes.extend(7u32.to_le_bytes());
    bytes.extend(codec().serialize(&current.base).unwrap());
    bytes
}

// Input-only historical fixtures are typed projections of the unchanged V2
// payload. V3 adds snap; V4 wraps V3 with the new constraint cache.
fn v2_bytes_from_current(current: &[u8]) -> Vec<u8> {
    assert_eq!(&current[..4], MAGIC);
    assert_eq!(&current[4..8], &8_u32.to_le_bytes());
    let current: EnvelopeV5 = codec()
        .deserialize::<EnvelopeV8>(&current[8..])
        .unwrap()
        .base
        .base
        .base;
    let current = current.base;
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend(codec().serialize(&current.base.base).unwrap());
    bytes
}
fn v3_bytes_from_current(current: &[u8]) -> Vec<u8> {
    assert_eq!(&current[..4], MAGIC);
    assert_eq!(&current[4..8], &8_u32.to_le_bytes());
    let current: EnvelopeV5 = codec()
        .deserialize::<EnvelopeV8>(&current[8..])
        .unwrap()
        .base
        .base
        .base;
    let current = current.base;
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    bytes.extend(codec().serialize(&current.base).unwrap());
    bytes
}

#[test]
fn camera_owner_v2_is_self_contained_without_a_native_block_and_leaves_peer_unchanged() {
    let mut source = GameLogic::new();
    source.session_view.target = Vec3::new(31., 42., 53.);
    source.session_view.camera.camera_yaw_radians = 0.7;
    source.session_view.camera.rotation_registers.begin_scalar(
        source
            .session_view
            .camera
            .rotation_registers
            .scalar_start()
            .unwrap_or(0.),
        1.2,
    );
    source.session_view.camera.rotation_registers =
        crate::presentation_frame::camera::registers::RotationRegisters::import_v7_scalar(
            0.7,
            source
                .session_view
                .camera
                .rotation_registers
                .scalar_target(),
        );
    source.session_view.camera.rotation =
        crate::presentation_frame::camera::rotation::RotationTiming::historical(
            2.0,
            source.session_view.camera.rotation.ease_in(),
            source.session_view.camera.rotation.ease_out(),
            source.session_view.camera.rotation.time,
        );
    source.session_view.camera.camera_yaw_frame = 17;
    source
        .session_view
        .camera
        .native_camera
        .initialize_map(19., 380., 43., 12.);
    source
        .session_view
        .camera
        .native_camera
        .set_height(165., 20., 500.);
    source
        .session_view
        .camera
        .native_camera
        .set_adjustment_enabled(true);
    source.session_view.scripted.path_arrived = true;
    let peer = GameLogic::new();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let current = source.capture_camera_continuation().unwrap();
    let v2 = v2_bytes_from_current(&current);
    let frozen_v6 = codec()
        .deserialize::<EnvelopeV8>(&current[8..])
        .unwrap()
        .base
        .base;
    let mut v6 = MAGIC.to_vec();
    v6.extend_from_slice(&6u32.to_le_bytes());
    v6.extend(codec().serialize(&frozen_v6).unwrap());
    for (version, bytes) in [
        (2_u32, v2),
        (3_u32, v3_bytes_from_current(&current)),
        (6_u32, v6),
        (7_u32, v7_bytes_from_current(&current)),
        (8_u32, current.clone()),
    ] {
        assert_eq!(&bytes[..4], MAGIC);
        assert_eq!(&bytes[4..8], &version.to_le_bytes());
        let state = GameLogic::decode_camera_continuation(&bytes).unwrap();
        assert!(state.script_camera_move_to.is_none());
        assert!(state.script_camera_path.is_none());
        assert!(!state.script_camera_freeze_time_armed);
        assert!(!state.script_camera_freeze_angle_armed);
        assert_eq!(state.script_camera_pending_final_speed_multiplier, None);
        assert!(!state.owned_follow_snap_immediate());
        let mut receiving = GameLogic::new();
        receiving.admit_camera_continuation(state).unwrap();
        let recaptured = receiving.capture_camera_continuation().unwrap();
        assert_eq!(
            v7_bytes_from_current(&recaptured),
            v7_bytes_from_current(&current)
        );
        if version == 2 {
            // Reproject the admitted state to the historical schema as well:
            // the import must preserve every old V2 field and byte.
            assert_eq!(v2_bytes_from_current(&recaptured), bytes);
        } else if version == 3 {
            assert_eq!(v3_bytes_from_current(&recaptured), bytes);
        } else if version == 6 {
            // Compare the admitted state through the frozen V6 schema, while
            // the assertion above independently checks current V7 capture.
            let projected = codec()
                .deserialize::<EnvelopeV8>(&recaptured[8..])
                .unwrap()
                .base
                .base;
            let mut historical = MAGIC.to_vec();
            historical.extend_from_slice(&6u32.to_le_bytes());
            historical.extend(codec().serialize(&projected).unwrap());
            assert_eq!(historical, bytes);
        } else if version == 7 {
            assert_eq!(v7_bytes_from_current(&recaptured), bytes);
        } else {
            assert_eq!(recaptured, bytes);
        }
        assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
        assert_eq!(
            receiving
                .session_view
                .camera
                .native_camera
                .basis_pitch_degrees(),
            43.
        );
        assert_eq!(
            receiving
                .session_view
                .camera
                .native_camera
                .basis_yaw_degrees(),
            12.
        );
        assert_eq!(receiving.session_view.camera.native_camera.desired(), 165.);
    }
}

#[test]
fn old_inactive_raw_and_v1_records_keep_bytes_and_ignore_obsolete_idle_arms() {
    let mut state = captured_state();
    state.script_camera_freeze_time_armed = true;
    state.script_camera_freeze_angle_armed = true;
    state.script_camera_pending_final_speed_multiplier = Some(0.5);
    state.script_camera_freeze_time = true;
    let legacy = codec().serialize(&state).unwrap();
    for bytes in [legacy.clone(), v1_bytes(&state)] {
        let decoded = GameLogic::decode_camera_continuation(&bytes).unwrap();
        assert_eq!(codec().serialize(&decoded).unwrap(), legacy);
        assert!(decoded.owned_camera_state().is_none());
        let mut receiving = GameLogic::new();
        receiving.admit_camera_continuation(decoded).unwrap();
        let current = GameLogic::decode_camera_continuation(
            &receiving.capture_camera_continuation().unwrap(),
        )
        .unwrap();
        assert!(current.script_camera_freeze_time);
        assert!(!current.script_camera_freeze_time_armed);
        assert!(!current.script_camera_freeze_angle_armed);
        assert_eq!(current.script_camera_pending_final_speed_multiplier, None);
        assert!(!receiving.session_view.scripted.motion.active);
    }
}

#[test]
fn old_rust_active_paths_reject_before_receiving_or_peer_mutation() {
    let mut state = captured_state();
    state.script_camera_move_to = Some(V1MoveTo {
        start: Vec3::ZERO,
        target: Vec3::X,
        ease: V1Ease {
            in_t: 0.,
            out_t: 1.,
        },
        total_time_seconds: 2.,
        elapsed_seconds: 0.25,
        shutter_frames: 1,
        cur_shutter: 0,
        last_ease: 0.,
        freeze_time: false,
        freeze_angle: false,
        look_toward: None,
        suppress_travel_look: false,
        speed_ramp_start_t: 1.,
        speed_ramp_start_multiplier: 1.,
        speed_ramp_final_multiplier: 1.,
    });
    let mut receiving = GameLogic::new();
    let peer = GameLogic::new();
    let before = receiving.capture_camera_continuation().unwrap();
    let peer_before = peer.capture_camera_continuation().unwrap();
    for bytes in [codec().serialize(&state).unwrap(), v1_bytes(&state)] {
        let result = GameLogic::decode_camera_continuation(&bytes).map(|candidate| {
            receiving.admit_camera_continuation(candidate).unwrap();
        });
        assert!(
            result
                .unwrap_err()
                .contains("unsupported old Rust active camera path")
        );
        assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
        assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    }
}

#[test]
fn camera_owner_v2_rejects_conflicting_legacy_projection_and_trailing_bytes() {
    let current = GameLogic::new().capture_camera_continuation().unwrap();
    for saved in [
        v2_bytes_from_current(&current),
        v3_bytes_from_current(&current),
        v7_bytes_from_current(&current),
        current,
    ] {
        let version = u32::from_le_bytes(saved[4..8].try_into().unwrap());
        let mut envelope = match version {
            2 => EnvelopeV4 {
                base: EnvelopeV3 {
                    base: codec().deserialize::<EnvelopeV2>(&saved[8..]).unwrap(),
                    follow_snap_immediate: false,
                },
                constraints: Default::default(),
            },
            3 => EnvelopeV4 {
                base: codec().deserialize::<EnvelopeV3>(&saved[8..]).unwrap(),
                constraints: Default::default(),
            },
            4 => codec().deserialize::<EnvelopeV4>(&saved[8..]).unwrap(),
            5 => codec().deserialize::<EnvelopeV5>(&saved[8..]).unwrap().base,
            6 => {
                codec()
                    .deserialize::<EnvelopeV6>(&saved[8..])
                    .unwrap()
                    .base
                    .base
            }
            7 => {
                codec()
                    .deserialize::<EnvelopeV7>(&saved[8..])
                    .unwrap()
                    .base
                    .base
                    .base
            }
            8 => {
                codec()
                    .deserialize::<EnvelopeV8>(&saved[8..])
                    .unwrap()
                    .base
                    .base
                    .base
                    .base
            }
            _ => panic!("unexpected fixture camera version {version}"),
        };
        let mut legacy: LogicCameraContinuation = codec()
            .deserialize(&envelope.base.base.legacy_record)
            .unwrap();
        legacy.script_camera_focus_estimate.x += 1.;
        envelope.base.base.legacy_record = codec().serialize(&legacy).unwrap();
        let payload = match version {
            2 => codec().serialize(&envelope.base.base).unwrap(),
            3 => codec().serialize(&envelope.base).unwrap(),
            4 => codec().serialize(&envelope).unwrap(),
            5 => {
                let original: EnvelopeV5 = codec().deserialize(&saved[8..]).unwrap();
                codec()
                    .serialize(&EnvelopeV5 {
                        base: envelope,
                        camera_settings: original.camera_settings,
                    })
                    .unwrap()
            }
            6 => {
                let original: EnvelopeV6 = codec().deserialize(&saved[8..]).unwrap();
                codec()
                    .serialize(&EnvelopeV6 {
                        base: EnvelopeV5 {
                            base: envelope,
                            camera_settings: original.base.camera_settings,
                        },
                        clip_policy: original.clip_policy,
                        shake_policy: original.shake_policy,
                    })
                    .unwrap()
            }
            7 => {
                let mut original: EnvelopeV7 = codec().deserialize(&saved[8..]).unwrap();
                original.base.base.base = envelope;
                codec().serialize(&original).unwrap()
            }
            8 => {
                let mut original: EnvelopeV8 = codec().deserialize(&saved[8..]).unwrap();
                original.base.base.base.base = envelope;
                codec().serialize(&original).unwrap()
            }
            _ => unreachable!(),
        };
        let mut malformed = saved[..8].to_vec();
        malformed.extend(payload);
        assert!(
            GameLogic::decode_camera_continuation(&malformed)
                .unwrap_err()
                .contains("ambiguous V2")
        );
        let mut trailing = saved.clone();
        trailing.push(0);
        assert!(GameLogic::decode_camera_continuation(&trailing).is_err());
        assert!(GameLogic::decode_camera_continuation(&saved[..saved.len() - 1]).is_err());
    }
}

#[test]
fn camera_owner_v2_preserves_tracking_turn_hold_cache_and_actual_frame_future() {
    let mut source = GameLogic::new();
    source.session_view.camera.rotation.time =
        crate::presentation_frame::camera::rotation::RotationTime { start: 1, end: 3 };
    source
        .session_view
        .camera
        .rotation_registers
        .begin_tracking(42, 4);
    source
        .session_view
        .camera
        .rotation_registers
        .observe_tracking(Some(Vec3::new(80., 15., 120.)));
    // The object has disappeared; the original rotation retains its own last
    // target observation throughout the remaining turn and hold frames.
    source.session_view.camera.camera_yaw_radians = 0.2;
    source.session_view.camera.rotation =
        crate::presentation_frame::camera::rotation::RotationTiming::historical(
            0.099,
            source.session_view.camera.rotation.ease_in(),
            source.session_view.camera.rotation.ease_out(),
            source.session_view.camera.rotation.time,
        );
    source.session_view.camera.camera_yaw_frame = 2;
    source.session_view.scripted.script_camera_freeze_time = true;
    let bytes = source.capture_camera_continuation().unwrap();
    let state = GameLogic::decode_camera_continuation(&bytes).unwrap();
    assert_eq!(state.script_look_toward_hold_seconds, 4. * 33. / 1000.);
    assert_eq!(state.script_camera_rotate_remaining, 5. * 0.033);
    let mut receiving = GameLogic::new();
    receiving.admit_camera_continuation(state).unwrap();
    for _ in 0..5 {
        source.headless_camera_view_offer(33).unwrap();
        receiving.headless_camera_view_offer(33).unwrap();
        assert_eq!(
            receiving.capture_camera_continuation().unwrap(),
            source.capture_camera_continuation().unwrap()
        );
    }
    assert_eq!(receiving.session_view.camera.camera_yaw_frame, 7);
    assert!(!receiving.session_view.camera.rotation_registers.active());
    assert!(!receiving.session_view.scripted.script_camera_freeze_time);
    assert_eq!(receiving.visual_speed_multiplier(), 3.);
    let tracking = &receiving.session_view.camera.rotation_registers;
    assert_eq!(tracking.cached_target().unwrap(), Vec3::new(80., 15., 120.));
    assert_eq!(tracking.hold_frames(), 4);
}

#[test]
fn old_object_tracking_and_invalid_v2_tracking_reject_without_admission() {
    let mut state = captured_state();
    state.script_look_toward_object_id = Some(42);
    state.script_look_toward_hold_seconds = 0.2;
    for bytes in [codec().serialize(&state).unwrap(), v1_bytes(&state)] {
        assert!(
            GameLogic::decode_camera_continuation(&bytes)
                .unwrap_err()
                .contains("unsupported old Rust active camera object tracking")
        );
    }
    // A V2 legacy ID without its typed cache/clock record is equally incomplete.
    assert!(GameLogic::decode_camera_continuation(&encode(&state).unwrap()).is_err());
    state.owned_extras.rotation_timing =
        Some(crate::presentation_frame::camera::rotation::RotationTiming::default());
    state
        .owned_extras
        .rotation_timing
        .as_mut()
        .unwrap()
        .turn_frames = 0;
    state.owned_extras.object_tracking = Some(CameraObjectTracking {
        hold_frames: 0,
        last_position: Vec3::ZERO,
    });
    assert!(GameLogic::decode_camera_continuation(&encode(&state).unwrap()).is_err());
}

#[test]
fn camera_owner_reset_cancels_actual_scalar_and_tracking_flags_without_replacing_pose_hag() {
    let mut world = GameLogic::new();
    world.session_view.camera.camera_position = Vec3::new(15., 240., 17.);
    world
        .session_view
        .camera
        .native_camera
        .initialize_map(18., 350., 43., 12.);
    world
        .session_view
        .camera
        .native_camera
        .set_height(270., 40., 500.);
    world.session_view.camera.rotation_registers.begin_scalar(
        world
            .session_view
            .camera
            .rotation_registers
            .scalar_start()
            .unwrap_or(0.),
        0.7,
    );
    world.session_view.camera.camera_pitch_target = Some(1.2);
    world.session_view.camera.camera_zoom_target = Some(0.8);
    world.session_view.camera.rotation =
        crate::presentation_frame::camera::rotation::RotationTiming::milliseconds(
            99,
            0.,
            0.,
            crate::presentation_frame::camera::rotation::RotationTime { start: 1, end: 1 },
        );
    world
        .session_view
        .camera
        .rotation_registers
        .begin_tracking(42, 4);
    world
        .session_view
        .camera
        .rotation_registers
        .observe_tracking(Some(Vec3::new(80., 15., 120.)));
    let peer = GameLogic::new();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let position = world.session_view.camera.camera_position;
    let view = world.session_view.camera.view_matrix;
    let projection = world.session_view.camera.projection_matrix;
    let hag = codec()
        .serialize(
            &world
                .session_view
                .camera
                .native_camera
                .capture_continuation(),
        )
        .unwrap();
    world.reset();
    assert!(!world.session_view.camera.rotation_registers.active());
    assert!(world.session_view.camera.camera_zoom_target.is_none());
    assert!(world.session_view.camera.camera_pitch_target.is_none());
    assert!(world.session_view.camera.rotation_registers.is_tracking());
    assert_eq!(
        world.session_view.camera.rotation_registers.tracking_id(),
        Some(42)
    );
    assert_eq!(
        world.session_view.camera.rotation_registers.cached_target(),
        Some(Vec3::new(80., 15., 120.))
    );
    assert_eq!(
        world.session_view.camera.rotation_registers.hold_frames(),
        4
    );
    assert!(!world.session_view.scripted.motion.active);
    assert_eq!(world.session_view.camera.camera_position, position);
    assert_eq!(world.session_view.camera.view_matrix, view);
    assert_eq!(world.session_view.camera.projection_matrix, projection);
    assert_eq!(
        codec()
            .serialize(
                &world
                    .session_view
                    .camera
                    .native_camera
                    .capture_continuation()
            )
            .unwrap(),
        hag
    );
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}

#[test]
fn current_v5_to_input_v2_removes_only_owned_settings_cache_and_snap_extensions() {
    let mut world = GameLogic::new();
    world
        .session_view
        .camera
        .constraints
        .recalculate(Vec3::ZERO, Vec3::new(1000., 0., 800.), 37.)
        .unwrap();
    world.session_view.camera.camera_follow_snap_immediate = true;
    let current = world.capture_camera_continuation().unwrap();
    let saved: EnvelopeV5 = codec()
        .deserialize::<EnvelopeV8>(&current[8..])
        .unwrap()
        .base
        .base
        .base;
    let saved = saved.base;
    let v2 = v2_bytes_from_current(&current);
    assert_eq!(&v2[4..8], &2_u32.to_le_bytes());
    assert_eq!(&v2[8..], codec().serialize(&saved.base.base).unwrap());
    let decoded = GameLogic::decode_camera_continuation(&v2).unwrap();
    assert!(!decoded.owned_constraints().is_valid());
    assert!(!decoded.owned_follow_snap_immediate());
    assert_eq!(
        codec().serialize(&decoded).unwrap(),
        saved.base.base.legacy_record
    );
    assert_eq!(
        codec()
            .serialize(decoded.owned_camera_state().unwrap())
            .unwrap(),
        codec().serialize(&saved.base.base.camera).unwrap()
    );
}
