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

#[test]
fn camera_owner_v2_is_self_contained_without_a_native_block_and_leaves_peer_unchanged() {
    let mut source = GameLogic::new();
    source.session_view.target = Vec3::new(31., 42., 53.);
    source.session_view.camera.camera_yaw_radians = 0.7;
    source.session_view.camera.camera_yaw_target = Some(1.2);
    source.session_view.camera.camera_yaw_start = 0.7;
    source.session_view.camera.camera_yaw_duration = 2.0;
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
    let bytes = source.capture_camera_continuation().unwrap();
    assert_eq!(&bytes[..4], MAGIC);
    assert_eq!(&bytes[4..8], &2_u32.to_le_bytes());
    let state = GameLogic::decode_camera_continuation(&bytes).unwrap();
    assert!(state.script_camera_move_to.is_none());
    assert!(state.script_camera_path.is_none());
    assert!(!state.script_camera_freeze_time_armed);
    assert!(!state.script_camera_freeze_angle_armed);
    assert_eq!(state.script_camera_pending_final_speed_multiplier, None);
    let mut receiving = GameLogic::new();
    receiving.admit_camera_continuation(state);
    assert_eq!(receiving.capture_camera_continuation().unwrap(), bytes);
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
        receiving.admit_camera_continuation(decoded);
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
            receiving.admit_camera_continuation(candidate);
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
    let saved = GameLogic::new().capture_camera_continuation().unwrap();
    let envelope: EnvelopeV2 = codec().deserialize(&saved[8..]).unwrap();
    let mut legacy: LogicCameraContinuation = codec().deserialize(&envelope.legacy_record).unwrap();
    legacy.script_camera_focus_estimate.x += 1.;
    let payload = codec()
        .serialize(&EnvelopeV2 {
            legacy_record: codec().serialize(&legacy).unwrap(),
            ..envelope
        })
        .unwrap();
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

#[test]
fn camera_owner_v2_preserves_tracking_turn_hold_cache_and_actual_frame_future() {
    let mut source = GameLogic::new();
    source.session_view.scripted.object_tracking = Some(CameraObjectTracking {
        duration_ms: 99,
        turn_frames: 3,
        hold_frames: 4,
        last_position: Vec3::new(80., 15., 120.),
        start_time_multiplier: 1,
        end_time_multiplier: 3,
    });
    // The object has disappeared; the original rotation retains its own last
    // target observation throughout the remaining turn and hold frames.
    source.session_view.scripted.script_look_toward_object_id = Some(42);
    source.session_view.camera.camera_yaw_target = Some(0.2);
    source.session_view.camera.camera_yaw_radians = 0.2;
    source.session_view.camera.camera_yaw_duration = 0.099;
    source.session_view.camera.camera_yaw_frame = 2;
    source.session_view.scripted.script_camera_freeze_time = true;
    let bytes = source.capture_camera_continuation().unwrap();
    let state = GameLogic::decode_camera_continuation(&bytes).unwrap();
    assert_eq!(state.script_look_toward_hold_seconds, 4. * 33. / 1000.);
    assert_eq!(state.script_camera_rotate_remaining, 5. * 0.033);
    let mut receiving = GameLogic::new();
    receiving.admit_camera_continuation(state);
    for _ in 0..5 {
        source.headless_camera_view_offer(33);
        receiving.headless_camera_view_offer(33);
        assert_eq!(
            receiving.capture_camera_continuation().unwrap(),
            source.capture_camera_continuation().unwrap()
        );
    }
    assert_eq!(receiving.session_view.camera.camera_yaw_frame, 7);
    assert!(receiving.session_view.camera.camera_yaw_target.is_none());
    assert!(!receiving.session_view.scripted.script_camera_freeze_time);
    assert_eq!(receiving.visual_speed_multiplier(), 3.);
    let tracking = receiving
        .session_view
        .scripted
        .object_tracking
        .as_ref()
        .unwrap();
    assert_eq!(tracking.last_position, Vec3::new(80., 15., 120.));
    assert_eq!(tracking.hold_frames, 4);
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
    state.owned_extras.object_tracking = Some(CameraObjectTracking {
        duration_ms: 0,
        turn_frames: 0,
        hold_frames: 0,
        last_position: Vec3::ZERO,
        start_time_multiplier: 1,
        end_time_multiplier: 1,
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
    world.session_view.camera.camera_yaw_target = Some(0.7);
    world.session_view.camera.camera_pitch_target = Some(1.2);
    world.session_view.camera.camera_zoom_target = Some(0.8);
    world.session_view.scripted.object_tracking = Some(CameraObjectTracking {
        duration_ms: 99,
        turn_frames: 3,
        hold_frames: 4,
        last_position: Vec3::new(80., 15., 120.),
        start_time_multiplier: 1,
        end_time_multiplier: 1,
    });
    world.session_view.scripted.script_look_toward_object_id = Some(42);
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
    assert!(world.session_view.camera.camera_yaw_target.is_none());
    assert!(world.session_view.camera.camera_zoom_target.is_none());
    assert!(world.session_view.camera.camera_pitch_target.is_none());
    assert!(world.session_view.scripted.object_tracking.is_none());
    assert!(
        world
            .session_view
            .scripted
            .script_look_toward_object_id
            .is_none()
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
