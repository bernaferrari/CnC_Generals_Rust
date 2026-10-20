use super::*;

#[test]
fn camera_continuation_actual_archives_keep_pending_requests_with_each_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    a.queue_pending_camera_zoom(0.75, 3.0);
    a.queue_pending_camera_pitch(0.5, 2.0);
    a.queue_pending_screen_shake(2);
    b.queue_pending_camera_zoom(1.5, 7.0);
    let a_camera = a.capture_camera_continuation().unwrap();
    let b_camera = b.capture_camera_continuation().unwrap();
    saves
        .save_game("camera_a", &a, &fixture_save_info())
        .unwrap();
    saves
        .save_game("camera_b", &b, &fixture_save_info())
        .unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: b_save, ..
    } = saves.load_game_snapshot("camera_b").unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: a_save, ..
    } = saves.load_game_snapshot("camera_a").unwrap();
    b.reset();
    assert_eq!(a.capture_camera_continuation().unwrap(), a_camera);
    let mut restored = GameLogic::new();
    saves.restore_game_snapshot(&a_save, &mut restored).unwrap();
    assert_eq!(restored.capture_camera_continuation().unwrap(), a_camera);
    let frozen = crate::presentation_frame::PresentationFrame::build_from_logic(&mut restored, 0);
    let admitted = restored.acknowledge_frozen_camera_requests(&frozen);
    assert!(admitted.zoom && admitted.pitch);
    assert_eq!(admitted.screen_shakes, 1);
    let repeated = restored.acknowledge_frozen_camera_requests(&frozen);
    assert!(!repeated.zoom && !repeated.pitch);
    assert_eq!(repeated.screen_shakes, 0);
    saves.restore_game_snapshot(&b_save, &mut b).unwrap();
    assert_eq!(b.capture_camera_continuation().unwrap(), b_camera);
    assert_eq!(a.capture_camera_continuation().unwrap(), a_camera);
}
