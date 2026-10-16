//! Native85 keeps historical observations without creating another defaults owner.
use super::*;
use crate::game_logic::GameLogic;

#[test]
fn stale_native85_default_observations_do_not_override_receiving_world_defaults() {
    let mut source = GameLogic::new();
    let mut native = super::tests::fixture();
    super::tests::fixture_on_owner(&mut native, &mut source);
    source.session_view.scripted.script_default_camera_pitch = 0.625;
    source
        .session_view
        .scripted
        .script_default_camera_max_height = 1.75;
    // Deliberately stale historical cache observations still decode as before.
    native.host_match_script_camera_max_height = Some(0.125);
    native.host_match_script_camera_pitch = Some(-0.5);
    let bytes = codec().serialize(&native).unwrap();
    let decoded = CnCGameEngine::decode_camera_continuation(&bytes).unwrap();
    assert_eq!(codec().serialize(&decoded.0).unwrap(), bytes);
    let logic =
        GameLogic::decode_camera_continuation(&source.capture_camera_continuation().unwrap())
            .unwrap();
    decoded.validate_logic_continuation(&logic).unwrap();
    let builder = crate::save_load::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    crate::save_load::snapshot::write_native_camera_continuation(
        &mut snapshot.lifecycle_tail,
        &bytes,
    )
    .unwrap();
    let mut receiving = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    let restored =
        crate::save_load::snapshot::read_native_camera_continuation(&snapshot.lifecycle_tail)
            .unwrap()
            .unwrap();
    // This is the same field projection used by live native admission. Script
    // defaults belong to the Logic continuation, outside this historical DTO.
    restored
        .0
        .admit_session_camera_fields(&mut receiving.session_view.camera);
    assert_eq!(receiving.script_default_camera_max_height(), 1.75);
    assert_eq!(receiving.script_default_camera_pitch(), 0.625);
    assert_eq!(restored.0.host_match_script_camera_max_height, Some(0.125));
    assert_eq!(restored.0.host_match_script_camera_pitch, Some(-0.5));
    // Empty observations are also a valid old wire record; they must not
    // supply the former native cold fallback of zero for max-height scale.
    native.host_match_script_camera_max_height = None;
    native.host_match_script_camera_pitch = None;
    let empty =
        CnCGameEngine::decode_camera_continuation(&codec().serialize(&native).unwrap()).unwrap();
    let mut fresh = GameLogic::new();
    empty
        .0
        .admit_session_camera_fields(&mut fresh.session_view.camera);
    assert_eq!(fresh.script_default_camera_max_height(), 1.0);
    assert_eq!(fresh.script_default_camera_pitch(), 0.0);
}

#[test]
fn malformed_historical_native85_observations_still_reject_before_admission() {
    let owner = GameLogic::new();
    let before = owner.capture_camera_continuation().unwrap();
    for (max_height, pitch) in [(Some(f32::NAN), None), (None, Some(f32::INFINITY))] {
        let mut native = super::tests::fixture();
        native.host_match_script_camera_max_height = max_height;
        native.host_match_script_camera_pitch = pitch;
        assert!(
            CnCGameEngine::decode_camera_continuation(&codec().serialize(&native).unwrap())
                .is_err()
        );
        assert_eq!(owner.capture_camera_continuation().unwrap(), before);
    }
}
