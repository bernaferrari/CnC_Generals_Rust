//! Actual startup handoff kernel used by host_replace_game_logic, without a window.
use super::*;
#[test]
fn startup_handoff_keeps_worker_rules_and_persistent_pose_hag_with_peer_isolation() {
    let mut previous = GameLogic::new();
    let mut candidate = GameLogic::new();
    let peer = GameLogic::new();
    let rules = crate::presentation_frame::camera::settings::CameraContentRules {
        min_camera_height: 100.,
        max_camera_height: 300.,
        ..Default::default()
    };
    previous.admit_camera_application_rules(rules).unwrap();
    candidate.admit_camera_application_rules(rules).unwrap();
    previous
        .admit_camera_map_rules(
            crate::presentation_frame::camera::settings::CameraContentRules {
                camera_height: 333.,
                max_camera_height: 900.,
                ..rules
            },
        )
        .unwrap();
    candidate
        .admit_camera_map_rules(
            crate::presentation_frame::camera::settings::CameraContentRules {
                camera_height: 444.,
                camera_pitch: 41.,
                camera_yaw: 17.,
                max_camera_height: 600.,
                ..rules
            },
        )
        .unwrap();
    previous.admit_camera_application_clip(Default::default());
    candidate.admit_camera_application_clip(
        crate::presentation_frame::camera::clip::CameraClipRules {
            draw_entire_terrain: true,
        },
    );
    previous
        .admit_camera_application_shake(Default::default())
        .unwrap();
    candidate
        .admit_camera_application_shake(game_engine::common::ini::ini_game_data::CameraShakeRules {
            max_shake_range: 543.,
            ..Default::default()
        })
        .unwrap();
    previous.session_view.target = Vec3::new(91., 0., 93.);
    previous
        .session_view
        .camera
        .native_camera
        .initialize_map(23., 333., 35., 19.);
    previous
        .session_view
        .camera
        .native_camera
        .set_height(177., 100., 900.);
    previous.session_view.commit_orbit_pose();
    let target = previous.session_view.target;
    let view = previous.session_view.camera.view_matrix;
    let eye = previous.session_view.camera.camera_position;
    let hag = serde_json::to_value(
        previous
            .session_view
            .camera
            .native_camera
            .capture_continuation(),
    )
    .unwrap();
    candidate.session_view.pending_focus_previous_target = Some(Vec3::new(17., 18., 19.));
    candidate.session_view.scripted.script_default_camera_pitch = 0.75;
    candidate.session_view.pending_letterbox_visibility = vec![true, false];
    #[cfg(feature = "game_client")]
    {
        candidate
            .session_view
            .filter
            .set_filter(game_client::display::view::FilterType::MotionBlur);
    }
    let rules_before = candidate.session_view.camera.settings;
    let clip_before = candidate.session_view.camera.clip_policy;
    let shake_before = candidate.session_view.camera.shake_policy;
    let peer_before = peer.capture_camera_continuation().unwrap();
    CnCGameEngine::admit_startup_candidate_view(
        &mut previous.session_view,
        &mut candidate.session_view,
    );
    assert_eq!(candidate.session_view.camera.settings, rules_before);
    assert_eq!(candidate.session_view.camera.clip_policy, clip_before);
    assert_eq!(candidate.session_view.camera.shake_policy, shake_before);
    assert_eq!(
        candidate
            .session_view
            .camera
            .settings
            .current()
            .max_camera_height,
        600.
    );
    assert_eq!(candidate.session_view.target, target);
    assert_eq!(candidate.session_view.camera.view_matrix, view);
    assert_eq!(candidate.session_view.camera.camera_position, eye);
    assert_eq!(
        serde_json::to_value(
            candidate
                .session_view
                .camera
                .native_camera
                .capture_continuation()
        )
        .unwrap(),
        hag
    );
    assert_eq!(
        candidate.session_view.pending_focus_previous_target,
        Some(Vec3::new(17., 18., 19.))
    );
    assert_eq!(
        candidate.session_view.scripted.script_default_camera_pitch,
        0.75
    );
    assert_eq!(
        candidate.session_view.pending_letterbox_visibility,
        vec![true, false]
    );
    #[cfg(feature = "game_client")]
    assert_eq!(
        candidate.session_view.filter.filter(),
        game_client::display::view::FilterType::MotionBlur
    );
    let defaults = CnCGameEngine::startup_camera_defaults_for(&candidate);
    assert_eq!(defaults.camera_height, 444.);
    assert_eq!(defaults.pitch_degrees, 41.);
    assert_eq!(defaults.yaw_degrees, 17.);
    assert_eq!(defaults.max_camera_height, 300.);
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
    let source = include_str!("host_authority.rs");
    let body = source
        .split("fn host_replace_game_logic(")
        .nth(1)
        .unwrap()
        .split("self.game_logic = logic;")
        .next()
        .unwrap();
    assert!(body.contains("Self::admit_startup_candidate_view("));
}
