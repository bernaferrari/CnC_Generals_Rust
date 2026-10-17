use super::*;
use crate::game_logic::{GameLogic, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn receiving_movement_rule_controls_actual_headless_view_despite_foreign_global_and_reset() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_movement_rule_controls_actual_headless_view_despite_foreign_global_and_reset",
        || {
            let mut owner = GameLogic::new();
            let mut peer = GameLogic::new();
            for world in [&mut owner, &mut peer] {
                world.add_player(Player::new(0, Team::USA, "MovementOwner", true));
                world
                    .templates
                    .insert("MovementOwner".into(), ThingTemplate::new("MovementOwner"));
            }
            let owner_id = owner
                .create_object_for_player("MovementOwner", 0, Vec3::ZERO)
                .unwrap();
            let peer_id = peer
                .create_object_for_player("MovementOwner", 0, Vec3::ZERO)
                .unwrap();
            assert_eq!(owner_id, peer_id);
            owner.admit_camera_application_movement(CameraMovementRules {
                disable_camera_movement: true,
            });
            peer.admit_camera_application_movement(CameraMovementRules::default());
            owner.session_view.camera.install_zoom(2., 0.066, 0., 0.);
            peer.session_view.camera.install_zoom(2., 0.066, 0., 0.);
            let start = owner.session_view.camera.camera_zoom;
            {
                game_engine::common::global_data::write()
                    .writable
                    .disable_camera_movement = false;
            }
            assert!(owner.headless_camera_view_offer(33));
            assert_eq!(owner.session_view.camera.camera_zoom_frame, 1);
            assert_eq!(owner.session_view.camera.camera_zoom, start);
            {
                game_engine::common::global_data::write()
                    .writable
                    .disable_camera_movement = true;
            }
            assert!(peer.headless_camera_view_offer(33));
            assert_eq!(peer.session_view.camera.camera_zoom_frame, 1);
            assert_ne!(peer.session_view.camera.camera_zoom, start);
            assert!(owner.headless_camera_view_offer(33));
            assert_eq!(owner.session_view.camera.camera_zoom_target, None);
            assert_eq!(owner.session_view.camera.camera_zoom, start);
            let before = owner.capture_camera_continuation().unwrap();
            peer.reset();
            assert!(owner.camera_movement_disabled());
            assert_eq!(owner.capture_camera_continuation().unwrap(), before);
            // Actual native caller must supply the SAME policy observation.
            let native = include_str!("../../../cnc_game_engine/camera_view_effects.rs");
            assert!(native.contains("self.game_logic.camera_movement_disabled()"));
            assert!(!native.contains("global_data::read()"));
        },
    );
}

#[test]
fn movement_configuration_is_not_saved_pose_and_receiving_policy_survives_admission() {
    let mut source = GameLogic::new();
    let mut receiver = GameLogic::new();
    let ordinary = source.capture_camera_continuation().unwrap();
    source.admit_camera_application_movement(CameraMovementRules {
        disable_camera_movement: true,
    });
    assert_eq!(
        source.capture_camera_continuation().unwrap(),
        ordinary,
        "original View Xfer does not persist DisableCameraMovements"
    );
    source.session_view.camera.install_zoom(2., 1., 0., 0.);
    let bytes = source.capture_camera_continuation().unwrap();
    receiver
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&bytes).unwrap())
        .unwrap();
    assert!(!receiver.camera_movement_disabled());
    assert!(source.camera_movement_disabled());
    let unchanged = source.session_view.camera.camera_zoom;
    assert!(source.headless_camera_view_offer(33));
    assert!(receiver.headless_camera_view_offer(33));
    assert_eq!(source.session_view.camera.camera_zoom, unchanged);
    assert_ne!(receiver.session_view.camera.camera_zoom, unchanged);
}

#[test]
fn private_map_solo_overrides_are_ordered_atomic_and_reset_to_receiving_application() {
    let mut owner = GameLogic::new();
    let peer = GameLogic::new();
    owner.admit_camera_application_movement(CameraMovementRules {
        disable_camera_movement: true,
    });
    owner
        .apply_camera_movement_overrides(&["GameData\n DisableCameraMovements = No\n End\n".into()])
        .unwrap();
    assert!(!owner.camera_movement_disabled());
    let before = owner.session_view.camera.movement_policy;
    assert!(
        owner
            .apply_camera_movement_overrides(&[
                "GameData\n DisableCameraMovements = Yes\n End\n".into(),
                "GameData\n DisableCameraMovements = impossible\n End\n".into()
            ])
            .is_err()
    );
    assert_eq!(owner.session_view.camera.movement_policy, before);
    assert!(!peer.camera_movement_disabled());
    owner.session_view.camera.movement_policy.clear_map_rules();
    assert!(owner.camera_movement_disabled());
    assert!(
        owner
            .session_view
            .camera
            .movement_policy
            .reset_seed()
            .current()
            .disable_camera_movement
    );
}

#[test]
fn disabled_set_camera_lock_still_allows_clear_and_later_script_snap_mode() {
    let mut owner = GameLogic::new();
    let id = ObjectId(7);
    owner.set_camera_follow_object(Some(id));
    owner.admit_camera_application_movement(CameraMovementRules {
        disable_camera_movement: true,
    });
    owner.set_camera_follow_object(Some(ObjectId(8)));
    assert_eq!(owner.camera_follow_object_id(), Some(id));
    owner.set_camera_follow_object(None);
    assert_eq!(owner.camera_follow_object_id(), None);
    owner.set_camera_tether_object(ObjectId(8), true, 4.);
    assert_eq!(owner.camera_follow_object_id(), None);
    assert_eq!(owner.session_view.scripted.camera_tether_play, Some(4.));
    assert!(owner.session_view.camera.camera_follow_snap_immediate);
    assert_eq!(owner.session_view.target, Vec3::ZERO);
}
