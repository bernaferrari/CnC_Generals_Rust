//! Authored callback -> actual receiving scalar owner -> admitted View update.
use super::*;
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_camera_actions_use_receiving_movement_rule_under_foreign_global_publish() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_camera_actions_use_receiving_movement_rule_under_foreign_global_publish",
        || {
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            first.admit_camera_application_movement(
                game_engine::common::ini::ini_game_data::CameraMovementRules {
                    disable_camera_movement: true,
                    ..Default::default()
                },
            );
            let first_engine = authored(0.75, second.mission_scripts.clone());
            let second_engine = authored(1.25, first.mission_scripts.clone());
            let first_engine = walk(&mut first, first_engine);
            let second_engine = walk(&mut second, second_engine);
            first.install_owned_script_engine(first_engine);
            second.install_owned_script_engine(second_engine);
            assert_eq!(first.session_view.camera.camera_zoom_target, Some(0.75));
            assert_eq!(second.session_view.camera.camera_zoom_target, Some(1.25));
            let first_angle = first.session_view.camera.camera_yaw_radians;
            let first_zoom = first.session_view.camera.camera_zoom;
            let second_angle = second.session_view.camera.camera_yaw_radians;
            let second_zoom = second.session_view.camera.camera_zoom;
            {
                game_engine::common::global_data::write()
                    .writable
                    .disable_camera_movement = false;
            }
            assert!(first.headless_camera_view_offer(33));
            assert_eq!(first.session_view.camera.camera_yaw_frame, 1);
            assert_eq!(first.session_view.camera.camera_zoom_frame, 1);
            assert_eq!(first.session_view.camera.camera_yaw_radians, first_angle);
            assert_eq!(first.session_view.camera.camera_zoom, first_zoom);
            {
                game_engine::common::global_data::write()
                    .writable
                    .disable_camera_movement = true;
            }
            assert!(second.headless_camera_view_offer(33));
            assert_eq!(second.session_view.camera.camera_yaw_frame, 1);
            assert_eq!(second.session_view.camera.camera_zoom_frame, 1);
            assert_ne!(second.session_view.camera.camera_yaw_radians, second_angle);
            assert_ne!(second.session_view.camera.camera_zoom, second_zoom);
            assert!(first.camera_movement_disabled());
            assert!(!second.camera_movement_disabled());
        },
    );
}
