//! Full installed camera matrices are detached save data, not another owner.
//! No GPU/window or unresolved animated bone producer is invoked by this suite.
use super::*;
use crate::game_logic::GameLogic;
use glam::{Mat4, Vec4};

fn reflected_view() -> Mat4 {
    // Arbitrary finite native view, with reflection, roll and nonuniform scale.
    // This is a codec retention fixture, not a D3DX bit-accuracy claim for
    // CameraClass::Update_Frustum's general inverse. Full bone matrix copy and
    // general inversion are source intent; no look-at is equivalent.
    Mat4::from_cols(
        Vec4::new(0.0, 0.25, 0.0, 0.0),
        Vec4::new(-0.5, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, -0.125, 0.0),
        Vec4::new(16.0, -4.0, 6.0, 1.0),
    )
}

fn fixture(view: Mat4) -> CameraState {
    let mut state = super::super::tests::fixture();
    let inverse = view.inverse();
    state.completed_eye = inverse.transform_point3(Vec3::ZERO);
    state.completed_forward = inverse.transform_vector3(Vec3::NEG_Z);
    state.completed_up = inverse.transform_vector3(Vec3::Y);
    state
}

fn bits(matrix: Mat4) -> [u32; 16] {
    matrix.to_cols_array().map(f32::to_bits)
}

fn filter() -> Option<(glam::Vec2, Vec3)> {
    #[cfg(feature = "game_client")]
    {
        Some((glam::Vec2::new(0.125, -0.25), Vec3::new(11.0, 13.0, 17.0)))
    }
    #[cfg(not(feature = "game_client"))]
    {
        None
    }
}

#[test]
fn native_v2_preserves_literal_reflection_roll_and_nonuniform_scale() {
    let view = reflected_view();
    let state = fixture(view);
    assert!(view.determinant() < 0.0);
    assert_eq!(state.completed_eye, Vec3::new(16.0, 32.0, 48.0));
    assert_eq!(state.completed_forward, Vec3::new(0.0, 0.0, 8.0));
    assert_eq!(state.completed_up, Vec3::new(4.0, 0.0, 0.0));
    let historical = codec().serialize(&state).unwrap();
    let bytes = encode_completed_camera(&state, view, filter()).unwrap();
    assert_eq!(&bytes[TAG.len()..TAG.len() + 4], &2u32.to_le_bytes());
    let decoded = decode_native_camera(&bytes).unwrap();
    assert_eq!(bits(decoded.completed_view()), bits(view));
    assert_eq!(decoded.1.filter, filter());
    assert_eq!(codec().serialize(&decoded.0).unwrap(), historical);
    let raw = decode_native_camera(&historical).unwrap();
    assert!(raw.completed_view().determinant() > 0.0);
    assert_ne!(bits(raw.completed_view()), bits(view));
}

#[test]
fn native_v2_preserves_rigid_reflection_and_roll() {
    // For this rigid component fixture the general inverse is R^T,-R^T*t.
    // Camera-world rows: [0,1,0,16],[-1,0,0,32],[0,0,-1,48].
    // CPP CameraClass::Update_Frustum559 calls Get_Inverse, not the separate
    // Get_Orthogonal_Inverse routine. Reflection is valid in either arithmetic.
    let view = Mat4::from_cols(
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(-1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, -1.0, 0.0),
        Vec4::new(32.0, -16.0, 48.0, 1.0),
    );
    let state = fixture(view);
    assert_eq!(view.determinant(), -1.0);
    assert_eq!(state.completed_eye, Vec3::new(16.0, 32.0, 48.0));
    assert_eq!(state.completed_up, Vec3::X);
    assert_eq!(state.completed_forward, Vec3::Z);
    let bytes = encode_completed_camera(&state, view, None).unwrap();
    let decoded = decode_native_camera(&bytes).unwrap();
    assert_eq!(bits(decoded.completed_view()), bits(view));
    assert_ne!(bits(decoded.0.completed_view()), bits(view));
}

#[test]
fn full_matrix_validation_does_not_require_a_reconstructible_look_at() {
    // Finite invertible affine matrix, but eye+forward rounds back to eye.
    // V2 stores its matrix and never asks look_at_rh to reconstruct this pose.
    let view = Mat4::from_translation(Vec3::new(0.0, 0.0, -1.0e20));
    let state = fixture(view);
    assert_eq!(
        state.completed_eye + state.completed_forward,
        state.completed_eye
    );
    assert!(
        state.validate().is_err(),
        "historical normalized-pose restriction remains"
    );
    let bytes = encode_completed_camera(&state, view, None).unwrap();
    assert_eq!(
        bits(decode_native_camera(&bytes).unwrap().completed_view()),
        bits(view)
    );
}

#[test]
fn paired_world_snapshot_preserves_full_completed_view_without_peer_or_hag_step() {
    let view = reflected_view();
    let state = fixture(view);
    let mut source = GameLogic::new();
    state.admit_session_camera_fields(&mut source.session_view.camera);
    source.session_view.camera.view_matrix = view;
    source.session_view.target = state.camera_target;
    #[cfg(feature = "game_client")]
    {
        let (scroll, previous) = filter().unwrap();
        source.session_view.camera.native_filter_scroll = scroll;
        source.session_view.camera.native_filter_previous_target = previous;
    }
    let source_before = source.capture_camera_continuation().unwrap();
    let native = encode_completed_camera(&state, view, filter()).unwrap();
    let decoded = decode_native_camera(&native).unwrap();
    let logic = GameLogic::decode_camera_continuation(&source_before).unwrap();
    decoded.validate_logic_continuation(&logic).unwrap();
    let builder = crate::save_load::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    crate::save_load::snapshot::write_native_camera_continuation(
        &mut snapshot.lifecycle_tail,
        &native,
    )
    .unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot: crate::save_load::WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    let mut peer = GameLogic::new();
    peer.session_view.camera.view_matrix = Mat4::from_translation(Vec3::new(101.0, 103.0, 107.0));
    let peer_before = peer.capture_camera_continuation().unwrap();
    let mut receiving = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    let received =
        crate::save_load::snapshot::read_native_camera_continuation(&snapshot.lifecycle_tail)
            .unwrap()
            .unwrap();
    assert_eq!(bits(received.completed_view()), bits(view));
    assert_eq!(bits(receiving.session_view.camera.view_matrix), bits(view));
    assert_eq!(
        receiving.capture_camera_continuation().unwrap(),
        source_before
    );
    assert_eq!(source.capture_camera_continuation().unwrap(), source_before);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    peer.reset();
    source.reset();
    assert_eq!(bits(receiving.session_view.camera.view_matrix), bits(view));
    assert_eq!(receiving.session_view.camera.native_camera.desired(), 470.0);
    assert_eq!(receiving.session_view.camera.camera_yaw_frame, 0);
}

#[test]
fn v2_rejects_malformed_and_observation_mismatched_matrices_detached() {
    let owner = GameLogic::new();
    let before = owner.capture_camera_continuation().unwrap();
    let view = reflected_view();
    let state = fixture(view);
    let mut nonfinite = view;
    nonfinite.x_axis.x = f32::NAN;
    let mut infinity = view;
    infinity.y_axis.z = f32::INFINITY;
    let mut mismatch = view;
    mismatch.w_axis.x += 1.0;
    for matrix in [Mat4::ZERO, nonfinite, infinity, mismatch] {
        // Bypass the writer intentionally to exercise untrusted on-disk bytes.
        let bytes = tagged(
            codec()
                .serialize(&CompletedCameraState {
                    version: 2,
                    camera: codec().serialize(&state).unwrap(),
                    completed_view: matrix,
                    filter: None,
                })
                .unwrap(),
        )
        .unwrap();
        assert!(decode_native_camera(&bytes).is_err());
        assert_eq!(owner.capture_camera_continuation().unwrap(), before);
    }
    let bytes = encode_completed_camera(&state, view, None).unwrap();
    for end in [TAG.len(), TAG.len() + 3, bytes.len() - 1] {
        assert!(decode_native_camera(&bytes[..end]).is_err());
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(decode_native_camera(&trailing).is_err());
    assert_eq!(owner.capture_camera_continuation().unwrap(), before);
}

#[test]
fn full_matrix_pairing_detects_handedness_that_eye_forward_up_cannot() {
    let source = GameLogic::new();
    let source_before = source.capture_camera_continuation().unwrap();
    let reflected = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let mut state = super::super::tests::fixture();
    // Both views have identical semantic observations; their X basis differs.
    let mut fixture_owner = GameLogic::new();
    super::super::tests::fixture_on_owner(&mut state, &mut fixture_owner);
    fixture_owner.session_view.camera.view_matrix = Mat4::IDENTITY;
    state.completed_eye = Vec3::ZERO;
    state.completed_forward = Vec3::NEG_Z;
    state.completed_up = Vec3::Y;
    // Match signed-zero observations produced by actual capture.
    let inverse = reflected.inverse();
    state.completed_eye = inverse.transform_point3(Vec3::ZERO);
    state.completed_forward = inverse.transform_vector3(Vec3::NEG_Z);
    state.completed_up = inverse.transform_vector3(Vec3::Y);
    let bytes = encode_completed_camera(&state, reflected, None).unwrap();
    let decoded = decode_native_camera(&bytes).unwrap();
    let owned = GameLogic::decode_camera_continuation(
        &fixture_owner.capture_camera_continuation().unwrap(),
    )
    .unwrap();
    assert!(
        decoded
            .validate_logic_continuation(&owned)
            .unwrap_err()
            .contains("completed camera matrix disagrees")
    );
    let mut snapshot = crate::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&fixture_owner)
        .unwrap();
    let before = snapshot.lifecycle_tail.clone();
    assert!(
        crate::save_load::snapshot::write_native_camera_continuation(
            &mut snapshot.lifecycle_tail,
            &bytes
        )
        .is_err()
    );
    assert_eq!(snapshot.lifecycle_tail, before);
    assert_eq!(source.capture_camera_continuation().unwrap(), source_before);
}

#[test]
#[cfg(feature = "game_client")]
fn exact_native_v1_envelope_still_decodes_its_original_pose_and_filter() {
    let state = super::super::tests::fixture();
    let legacy = codec().serialize(&state).unwrap();
    let scroll = glam::Vec2::new(0.2, -0.3);
    let previous = Vec3::new(3.0, 5.0, 7.0);
    let bytes = encode_filter_camera(legacy.clone(), scroll, previous).unwrap();
    assert_eq!(&bytes[TAG.len()..TAG.len() + 4], &1u32.to_le_bytes());
    let decoded = decode_native_camera(&bytes).unwrap();
    assert!(decoded.1.completed_view.is_none());
    assert_eq!(decoded.1.filter, Some((scroll, previous)));
    assert_eq!(codec().serialize(&decoded.0).unwrap(), legacy);
    assert_eq!(bits(decoded.completed_view()), bits(state.completed_view()));
}

#[test]
fn projective_native_view_rejects_before_affine_observation_math() {
    let owner = GameLogic::new();
    let before = owner.capture_camera_continuation().unwrap();
    let state = fixture(Mat4::IDENTITY);
    let mut tiny_perspective = Mat4::IDENTITY;
    tiny_perspective.x_axis.w = f32::MIN_POSITIVE;
    let mut perspective = Mat4::IDENTITY;
    perspective.z_axis.w = 0.25;
    let mut wrong_homogeneous_scale = Mat4::IDENTITY;
    wrong_homogeneous_scale.w_axis.w = 2.0;
    for matrix in [tiny_perspective, perspective, wrong_homogeneous_scale] {
        assert!(matrix.is_finite() && matrix.determinant() != 0.0);
        // Never call transform_point3/vector3 on these invalid views. The
        // shared validator rejects their representation before those helpers.
        let bytes = tagged(
            codec()
                .serialize(&CompletedCameraState {
                    version: 2,
                    camera: codec().serialize(&state).unwrap(),
                    completed_view: matrix,
                    filter: None,
                })
                .unwrap(),
        )
        .unwrap();
        assert!(
            decode_native_camera(&bytes)
                .err()
                .unwrap()
                .contains("non-affine")
        );
        assert!(
            encode_completed_camera(&state, matrix, None)
                .unwrap_err()
                .contains("non-affine")
        );
        assert_eq!(owner.capture_camera_continuation().unwrap(), before);
    }
}

#[test]
fn actual_general_affine_inverse_roundoff_is_retained_without_matrix_repair() {
    use glam::Quat;
    for index in 1..=64 {
        let rotation = Quat::from_euler(
            glam::EulerRot::XYZ,
            index as f32 * 0.013,
            index as f32 * -0.027,
            index as f32 * 0.041,
        );
        for scale in [
            Vec3::new(1.3, 2.7, 4.1),
            Vec3::new(-0.7, 5.3, 1.9),
            Vec3::new(7.1, 0.3, -2.1),
        ] {
            let camera_world = Mat4::from_scale_rotation_translation(
                scale,
                rotation,
                Vec3::new(17.3, -21.1, 13.7),
            );
            let view = camera_world.inverse();
            assert_eq!([view.x_axis.w, view.y_axis.w, view.z_axis.w], [0.0; 3]);
            assert!((view.w_axis.w - 1.0).abs() <= 1.0e-6);
            let state = fixture(view);
            let bytes = encode_completed_camera(&state, view, None).unwrap();
            assert_eq!(
                bits(decode_native_camera(&bytes).unwrap().completed_view()),
                bits(view)
            );
        }
    }
    // A concrete representation of reciprocal-multiply roundoff, independent
    // of whether a particular SIMD backend happens to round this corpus to1.
    let mut rounded = Mat4::IDENTITY;
    rounded.w_axis.w = f32::from_bits(1.0f32.to_bits() - 1);
    let bytes = encode_completed_camera(&fixture(rounded), rounded, None).unwrap();
    assert_eq!(
        bits(decode_native_camera(&bytes).unwrap().completed_view()),
        bits(rounded)
    );
}

#[test]
fn canonical_view_rejects_projective_candidate_but_projection_remains_projective() {
    let mut receiving = GameLogic::new();
    receiving.session_view.camera.projection_matrix = Mat4::perspective_rh(1.0, 1.5, 0.1, 1000.0);
    assert_ne!(
        receiving.session_view.camera.projection_matrix.z_axis.w,
        0.0
    );
    let before = receiving.capture_camera_continuation().unwrap();
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&before).unwrap())
        .unwrap();
    assert_eq!(
        bits(restored.session_view.camera.projection_matrix),
        bits(receiving.session_view.camera.projection_matrix)
    );
    let mut candidate = GameLogic::new();
    candidate.session_view.camera.view_matrix = receiving.session_view.camera.projection_matrix;
    assert!(
        candidate
            .capture_camera_continuation()
            .unwrap_err()
            .contains("non-affine")
    );
    let detached = crate::presentation_frame::camera::continuation::SessionCameraState::capture(
        &candidate.session_view.camera,
    );
    assert!(detached.validate().unwrap_err().contains("non-affine"));
    assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
    assert_eq!(restored.capture_camera_continuation().unwrap(), before);
}
