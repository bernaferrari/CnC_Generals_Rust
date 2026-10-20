//! Detached paired-record validation before any live owner or save bytes mutate.
use super::*;
use crate::game_logic::GameLogic;

fn pair() -> (GameLogic, NativeCameraContinuation) {
    let mut state = super::tests::fixture();
    let mut source = GameLogic::new();
    state.admit_session_camera_fields(&mut source.session_view.camera);
    source.session_view.camera.view_matrix = state.completed_view();
    source.session_view.target = state.camera_target;
    // Native capture makes these exact observations from the completed matrix.
    let inverse = source.session_view.camera.view_matrix.inverse();
    state.completed_eye = inverse.transform_point3(Vec3::ZERO);
    state.completed_forward = inverse.transform_vector3(Vec3::NEG_Z);
    state.completed_up = inverse.transform_vector3(Vec3::Y);
    state.validate().unwrap();
    (
        source,
        NativeCameraContinuation(state, NativeCameraExtras::default()),
    )
}

fn logic(source: &GameLogic) -> crate::game_logic::LogicCameraContinuation {
    GameLogic::decode_camera_continuation(&source.capture_camera_continuation().unwrap()).unwrap()
}

#[test]
fn paired_camera_records_validate_complete_overlap_and_leave_owner_unchanged() {
    let (source, native) = pair();
    let before = source.capture_camera_continuation().unwrap();
    native.validate_logic_continuation(&logic(&source)).unwrap();
    assert_eq!(source.capture_camera_continuation().unwrap(), before);
    let mut mismatch = native;
    mismatch.0.camera_yaw_frame += 1;
    mismatch.0.validate().unwrap();
    assert!(
        mismatch
            .validate_logic_continuation(&logic(&source))
            .unwrap_err()
            .contains("session camera continuation disagrees")
    );
    assert_eq!(source.capture_camera_continuation().unwrap(), before);
}

#[test]
fn paired_camera_records_reject_different_target_height_and_completed_pose() {
    for case in 0..3 {
        let (source, mut native) = pair();
        match case {
            0 => native.0.camera_target.x += 1.,
            1 => {
                native.0.height =
                    crate::presentation_frame::camera::height::CameraHeight::new(500., 43., 17.)
                        .capture_continuation()
            }
            2 => native.0.completed_eye.x += 1.,
            _ => unreachable!(),
        }
        native.0.validate().unwrap();
        assert!(
            native.validate_logic_continuation(&logic(&source)).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn mismatched_native_save_rejects_before_splicing_previously_captured_bytes() {
    let (source, mut native) = pair();
    let mut snapshot = crate::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let before = snapshot.lifecycle_tail.clone();
    native.0.camera_zoom = 2.;
    let bytes = codec().serialize(&native.0).unwrap();
    assert!(
        crate::save_load::snapshot::write_native_camera_continuation(
            &mut snapshot.lifecycle_tail,
            &bytes
        )
        .is_err()
    );
    assert_eq!(snapshot.lifecycle_tail, before);
}

#[test]
fn matching_native_save_rechecks_pair_on_decode_and_detects_owned_record() {
    let (source, native) = pair();
    let mut snapshot = crate::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let bytes = codec().serialize(&native.0).unwrap();
    crate::save_load::snapshot::write_native_camera_continuation(
        &mut snapshot.lifecycle_tail,
        &bytes,
    )
    .unwrap();
    assert!(
        crate::save_load::snapshot::read_owned_session_camera(&snapshot.lifecycle_tail).unwrap()
    );
    assert!(
        crate::save_load::snapshot::read_native_camera_continuation(&snapshot.lifecycle_tail)
            .unwrap()
            .is_some()
    );
}

#[test]
fn plain_world_snapshot_resumes_actual_headless_camera_owner_without_native_record() {
    use crate::presentation_frame::waypoint::{Point, WaypointMotion};
    let mut source = GameLogic::new();
    source.session_view.target = Vec3::new(4., 13., 6.);
    source
        .session_view
        .camera
        .native_camera
        .initialize_map(13., 325., 41., 17.);
    source
        .session_view
        .camera
        .native_camera
        .set_height(240., 50., 500.);
    source
        .session_view
        .camera
        .native_camera
        .set_adjustment_enabled(true);
    let points = [
        Vec3::new(60., 14., 20.),
        Vec3::new(140., 18., 100.),
        Vec3::new(240., 23., 80.),
    ]
    .map(Point::from_world);
    let mut motion = WaypointMotion::along_chain(
        &points,
        Point::from_world(source.session_view.target),
        0.35,
        13.,
        23.,
        1,
        1500,
        66,
        33,
        true,
        120.,
        210.,
        1.,
    );
    motion.rolling_average(4);
    motion.final_look_toward(Point::from_world(Vec3::new(240., 23., 200.)));
    let mut admission_speed = source.visual_speed_multiplier();
    motion.final_time_multiplier(2, &mut admission_speed);
    source
        .session_view
        .install_motion(motion, false, &mut admission_speed, false);
    // Deferred path admission does not change the receiving multiplier. Its
    // original integer ramp is applied by the actual admitted view operation.
    assert_eq!(admission_speed, source.visual_speed_multiplier());
    source.session_view.scripted.script_camera_freeze_time = true;
    for _ in 0..7 {
        source.headless_camera_view_offer(33).unwrap();
    }
    assert!(source.session_view.scripted.motion.active);
    let mut peer = GameLogic::new();
    peer.session_view.target = Vec3::splat(900.);
    peer.install_script_camera_path_for_test();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    assert!(
        crate::save_load::snapshot::read_native_camera_continuation(&snapshot.lifecycle_tail)
            .unwrap()
            .is_none()
    );
    assert!(
        crate::save_load::snapshot::read_owned_session_camera(&snapshot.lifecycle_tail).unwrap()
    );
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot: crate::save_load::WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    let mut receiving = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    assert_eq!(
        receiving.capture_camera_continuation().unwrap(),
        source.capture_camera_continuation().unwrap()
    );
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    let mut observed_segment_arrival = source.session_view.scripted.path_arrived;
    for _ in 0..65 {
        source.headless_camera_view_offer(33).unwrap();
        peer.headless_camera_view_offer(17).unwrap();
        receiving.headless_camera_view_offer(33).unwrap();
        observed_segment_arrival |= receiving.session_view.scripted.path_arrived;
        assert_eq!(
            receiving.capture_camera_continuation().unwrap(),
            source.capture_camera_continuation().unwrap()
        );
    }
    assert!(!receiving.session_view.scripted.motion.active);
    assert!(
        observed_segment_arrival,
        "source segment crossing must publish arrival"
    );
    // CPP moveAlongWaypointPath3084–3087 clears this observation on the
    // following terminal offer; it is not a latched final-completion flag.
    assert!(!receiving.session_view.scripted.path_arrived);
    assert_eq!(receiving.session_view.target, Vec3::new(240., 0., 80.));
    assert_eq!(
        receiving
            .session_view
            .camera
            .native_camera
            .basis_pitch_degrees(),
        41.
    );
    assert_eq!(
        receiving
            .session_view
            .camera
            .native_camera
            .basis_yaw_degrees(),
        17.
    );
    assert_eq!(receiving.session_view.camera.native_camera.desired(), 240.);
    assert!(!receiving.session_view.scripted.script_camera_freeze_time);
    assert_eq!(receiving.visual_speed_multiplier(), 2.);
}
