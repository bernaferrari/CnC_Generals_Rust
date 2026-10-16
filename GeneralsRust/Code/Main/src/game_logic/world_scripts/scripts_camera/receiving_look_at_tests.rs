//! Immediate LookAt observes the receiving live camera and terrain.
use super::*;

#[test]
fn receiving_look_at_clamps_without_advancing_native_axes_or_hag() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "receiving_look_at_clamps_without_advancing_native_axes_or_hag",
        || {
            let (mut owner, _, _) = world(110.);
            let (peer, _, _) = world(210.);
            *owner.world_services.terrain().write().unwrap() = camera_terrain();
            owner.override_world_size(640., 640.);
            owner.session_view.camera.view_matrix = glam::Mat4::IDENTITY;
            owner.session_view.camera.projection_matrix = glam::Mat4::IDENTITY;
            owner.session_view.camera.install_zoom(2., 3., 0., 0.);
            let frame = owner.session_view.camera.camera_zoom_frame;
            let peer_target = peer.session_view.target;
            owner.request_camera_focus(Vec3::new(-100., 0., 1000.));
            assert_eq!(owner.session_view.target, Vec3::new(-100., 0., 320.));
            assert_eq!(owner.session_view.camera.camera_zoom_frame, frame);
            assert_eq!(owner.session_view.camera.camera_zoom_target, Some(2.));
            assert_eq!(
                owner.session_view.camera.scripted_camera_constraint_widen,
                None
            );
            assert_eq!(peer.session_view.target, peer_target);
            assert_ne!(owner.session_view.camera.view_matrix, glam::Mat4::IDENTITY);
        },
    );
}

#[test]
fn receiving_airborne_look_at_uses_receiving_flat_terrain() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "receiving_airborne_look_at_uses_receiving_flat_terrain",
        || {
            let (mut owner, _, _) = world(110.);
            *owner.world_services.terrain().write().unwrap() = camera_terrain();
            owner.override_world_size(2000., 2000.);
            let eye = Vec3::new(300., 100., 200.);
            owner.session_view.camera.view_matrix =
                glam::Mat4::look_at_rh(eye, Vec3::new(300., 0., 300.), Vec3::Y);
            owner.session_view.camera.projection_matrix =
                glam::Mat4::perspective_rh(60_f32.to_radians(), 1., 10., 12000.);
            // Preserve a broad already-admitted path constraint, so this test
            // measures airborne projection independently of the map-edge inset.
            owner.session_view.camera.scripted_camera_constraint_widen =
                Some((-2000., 2000., -2000., 2000.));
            owner.request_camera_focus(Vec3::new(300., 60., 300.));
            assert!(
                (owner.session_view.target - Vec3::new(300., 0., 350.)).length() < 0.01,
                "{:?}",
                owner.session_view.target
            );
        },
    );
}

#[test]
fn authored_home_updates_actual_receiving_target_before_transport() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_home_updates_actual_receiving_target_before_transport",
        || {
            let (mut owner, _, home) = world(110.);
            let (peer, _, _) = world(210.);
            *owner.world_services.terrain().write().unwrap() = camera_terrain();
            owner.session_view.camera.view_matrix = glam::Mat4::IDENTITY;
            owner.session_view.camera.projection_matrix = glam::Mat4::IDENTITY;
            let peer_target = peer.session_view.target;
            let mut list = ScriptList::new();
            list.append_script(script(
                "ImmediateHome",
                vec![Box::new(ScriptAction::new(
                    ScriptActionType::CameraMoveHome,
                ))],
                true,
            ));
            let retained = engine(peer.mission_scripts.clone(), list);
            let _ = run(&mut owner, retained);
            assert_eq!(owner.session_view.target, home);
            assert_eq!(peer.session_view.target, peer_target);
            assert_eq!(owner.pending_camera_focus, Some(home));
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}
