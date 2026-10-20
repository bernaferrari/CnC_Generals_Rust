//! Exercise the same commit helper used by the real staged Engine restore.
//! No Window/GPU Engine is constructed by these component tests.
use super::*;
use crate::game_logic::{GameLogic, GameMode};
use crate::presentation_frame::SessionView;
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType};
use glam::Vec3;

fn receiving_view() -> SessionView {
    let mut view = SessionView::default();
    view.camera.resize_display(1920, 1080).unwrap();
    view.camera.viewport.set_rect(31, 47, 1701, 811).unwrap();
    view.camera.rebuild_projection();
    view.target = Vec3::new(80., 20., 120.);
    view.impulse.intensity = 7.;
    view.impulse.cosine = 0.6;
    view.impulse.sine = 0.8;
    view
}
fn height(view: &SessionView) -> Vec<u8> {
    bincode_legacy::serialize(&view.camera.native_camera.capture_continuation()).unwrap()
}

#[test]
fn legacy_restore_keeps_receiving_cropped_viewport_after_old_view_is_moved() {
    let mut receiver = receiving_view();
    let peer = receiving_view();
    let peer_state = peer.capture();
    let peer_viewport = peer.camera.viewport;
    let peer_projection = peer.camera.projection_matrix;
    let viewport = receiver.camera.viewport;
    let projection = receiver.camera.projection_matrix;
    let old_state = receiver.capture();
    let old_height = height(&receiver);
    let mut candidate = SessionView::default();
    candidate.pending_focus_previous_target = Some(Vec3::new(5., 6., 7.));
    candidate.scripted.script_default_camera_max_height = 0.75;
    candidate.pending_letterbox_visibility.extend([true, false]);
    candidate.letterbox_visibility_delivered_since_present = true;
    admit_staged_session_view(&mut receiver, &mut candidate, false);
    assert_eq!(candidate.camera.viewport, viewport);
    assert_eq!(candidate.camera.viewport.display_size(), (1920, 1080));
    assert_eq!(candidate.camera.viewport.origin(), (31, 47));
    assert_eq!(candidate.camera.viewport.size(), (1701, 811));
    assert_eq!(candidate.capture(), old_state);
    assert_eq!(
        height(&candidate),
        old_height,
        "viewport transfer must not advance HAG"
    );
    assert_eq!(candidate.scripted.script_default_camera_max_height, 0.75);
    assert_eq!(
        candidate.pending_focus_previous_target,
        Some(Vec3::new(5., 6., 7.))
    );
    assert!(candidate.pending_letterbox_visibility.is_empty());
    assert!(!candidate.letterbox_visibility_delivered_since_present);
    assert_eq!(candidate.camera.projection_matrix, projection);
    assert_eq!(peer.capture(), peer_state);
    assert_eq!(peer.camera.viewport, peer_viewport);
    assert_eq!(peer.camera.projection_matrix, peer_projection);
}

#[test]
fn complete_owned_restore_keeps_saved_camera_but_receiving_window_viewport() {
    let mut receiver = receiving_view();
    let receiving_state = receiver.capture();
    let viewport = receiver.camera.viewport;
    let mut candidate = SessionView::default();
    candidate.target = Vec3::new(900., 15., 300.);
    candidate.impulse.intensity = 3.;
    candidate.camera.camera_zoom_frame = 17;
    candidate.camera.camera_pitch_frame = 19;
    candidate.camera.camera_yaw_frame = 23;
    candidate.camera.camera_follow_snap_immediate = true;
    let saved_state = candidate.capture();
    let saved_height = height(&candidate);
    admit_staged_session_view(&mut receiver, &mut candidate, true);
    assert_eq!(candidate.camera.viewport, viewport);
    assert_eq!(candidate.capture(), saved_state);
    assert_eq!(height(&candidate), saved_height);
    assert_eq!(
        (
            candidate.camera.camera_zoom_frame,
            candidate.camera.camera_pitch_frame,
            candidate.camera.camera_yaw_frame
        ),
        (17, 19, 23)
    );
    assert!(candidate.camera.camera_follow_snap_immediate);
    assert_eq!(
        receiver.capture(),
        receiving_state,
        "owned camera must not move the preceding view"
    );
    assert_eq!(receiver.camera.viewport, viewport);
}

#[test]
fn unavailable_map_staging_preserves_actual_receiver_and_peer_viewports() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "unavailable_map_staging_preserves_actual_receiver_and_peer_viewports",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(directory.path());
            saves.init().unwrap();
            let mut receiver = GameLogic::new();
            receiver.start_new_game(GameMode::SinglePlayer);
            receiver.session_view = receiving_view();
            let mut peer = GameLogic::new();
            peer.session_view = receiving_view();
            peer.session_view.camera.resize_display(2560, 1440).unwrap();
            let before = receiver.session_view.capture();
            let viewport = receiver.session_view.camera.viewport;
            let projection = receiver.session_view.camera.projection_matrix;
            let peer_before = peer.session_view.capture();
            let peer_viewport = peer.session_view.camera.viewport;
            let peer_projection = peer.session_view.camera.projection_matrix;
            let info = SaveGameInfo {
                pristine_map_name: None,
                filename: "viewport_missing_map".into(),
                display_name: "viewport_missing_map".into(),
                description: "viewport staging rejection".into(),
                map_name: "No Such Native Viewport Map".into(),
                campaign_side: None,
                mission_number: None,
                save_date: std::time::UNIX_EPOCH,
                game_version: env!("CARGO_PKG_VERSION").into(),
                play_time: std::time::Duration::ZERO,
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            saves
                .save_game("viewport_missing_map", &receiver, &info)
                .unwrap();
            let result = CnCGameEngine::stage_saved_world_for_restore(
                &mut saves,
                "viewport_missing_map",
                GameMode::Shell,
                &receiver.templates,
                &receiver.weapon_definition_base,
                &receiver.projectile_definitions,
                &receiver.ocl_application_definitions(),
                receiver.ai_definitions.baseline(),
                receiver.production_energy_baseline(),
                receiver.physics_rules_baseline(),
                receiver.session_view.camera.settings,
                receiver.session_view.camera.clip_policy,
                receiver.session_view.camera.shake_policy,
                receiver.session_view.camera.movement_policy,
                receiver.session_view.camera.input_policy,
                &receiver.world_services,
            );
            assert!(matches!(result, Err(ref error) if error.contains("not available on disk")));
            assert_eq!(receiver.session_view.capture(), before);
            assert_eq!(receiver.session_view.camera.viewport, viewport);
            assert_eq!(receiver.session_view.camera.projection_matrix, projection);
            assert_eq!(peer.session_view.capture(), peer_before);
            assert_eq!(peer.session_view.camera.viewport, peer_viewport);
            assert_eq!(peer.session_view.camera.projection_matrix, peer_projection);
        },
    );
}
