//! Real archive and Native V6 component admission, not a constructed GPU Engine.
use super::*;
use crate::game_logic::GameLogic;
use crate::presentation_frame::camera::shake_rules::CameraShakeRules;
use crate::save_load::snapshot::{ClientDrawableWorldSnapshot, ClientSaveState, SnapshotBuilder};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};

#[test]
fn actual_archive_preserves_immediate_shake_active_native_state_and_next_logic_admission() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_archive_preserves_immediate_shake_active_native_state_and_next_logic_admission",
        || {
            let mut source = GameLogic::new();
            source.set_current_frame(713);
            source
                .admit_camera_application_shake(CameraShakeRules {
                    shake_cine_extreme_intensity: 8.,
                    max_shake_intensity: 100.,
                    ..Default::default()
                })
                .unwrap();
            source.admit_script_screen_shake(4);
            assert_eq!(source.session_view.impulse.intensity, 8.);
            source.session_view.advance_impulse();
            let mut state = super::super::tests::fixture();
            super::super::tests::fixture_on_owner(&mut state, &mut source);
            let saved_view = source.session_view.capture();
            state.screen_shake_intensity = saved_view.intensity;
            state.screen_shake_angle_cos = saved_view.cosine;
            state.screen_shake_angle_sin = saved_view.sine;
            state.camera_shake_offset = saved_view.offset;
            let native_bytes = encode_defined_camera(
                &state,
                source.session_view.camera.view_matrix,
                None,
                source.session_view.camera.camera_follow_snap_immediate,
                source.session_view.camera.constraints,
                source.session_view.camera.settings,
                source.session_view.camera.clip_policy,
                source.session_view.camera.shake_policy,
            )
            .unwrap();
            let directory = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(directory.path());
            let info = SaveGameInfo {
                filename: "immediate_shake".into(),
                display_name: "immediate_shake".into(),
                description: "source immediate impulse".into(),
                map_name: "component".into(),
                pristine_map_name: None,
                campaign_side: None,
                mission_number: None,
                save_date: std::time::UNIX_EPOCH,
                game_version: "test".into(),
                play_time: std::time::Duration::ZERO,
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            let mut client = game_client::core::game_client::GameClient::new().unwrap();
            saves
                .save_game_with_client_save_state(
                    "immediate_shake",
                    &source,
                    ClientDrawableWorldSnapshot::default(),
                    &mut client,
                    &info,
                    ClientSaveState {
                        camera: None,
                        camera_continuation: Some(native_bytes),
                        control_groups: None,
                    },
                )
                .unwrap();
            let crate::save_load::DecodedGameSave { snapshot, .. } =
                saves.load_game_snapshot("immediate_shake").unwrap();
            let native = crate::save_load::snapshot::read_native_camera_continuation(
                &snapshot.lifecycle_tail,
            )
            .unwrap()
            .unwrap();
            let mut restored = GameLogic::new();
            let peer = GameLogic::new();
            let peer_before = peer.capture_camera_continuation().unwrap();
            SnapshotBuilder::new()
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();
            let saved = native.0;
            // Exact existing native receiving owner admission, no second serializer
            // or callback replay. The full Engine/window wrapper is not executed.
            restored
                .session_view
                .admit(crate::presentation_frame::SessionViewState {
                    target: saved.camera_target,
                    intensity: saved.screen_shake_intensity,
                    cosine: saved.screen_shake_angle_cos,
                    sine: saved.screen_shake_angle_sin,
                    offset: saved.camera_shake_offset,
                })
                .unwrap();
            assert_eq!(restored.session_view.capture(), saved_view);
            assert_eq!(
                restored.session_view.camera.shake_policy,
                source.session_view.camera.shake_policy
            );
            assert!(
                restored.peek_pending_screen_shakes().is_empty(),
                "saved applied action has no replay intent"
            );
            for _ in 0..4 {
                assert_eq!(
                    source.session_view.advance_impulse(),
                    restored.session_view.advance_impulse()
                );
                source.admit_script_screen_shake(4);
                restored.admit_script_screen_shake(4);
                assert_eq!(
                    restored.session_view.capture(),
                    source.session_view.capture()
                );
            }
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
        },
    );
}
