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
            let requested = Vec3::new(-100., 0., 1000.);
            // Original calcCameraConstraints reads the current terrain extent,
            // independently of the deliberately different cached host bounds.
            let extent = owner.world_services.terrain().read().unwrap().get_extent();
            let min = Vec3::new(extent.lo.x, extent.lo.z, extent.lo.y);
            let max = Vec3::new(extent.hi.x, extent.hi.z, extent.hi.y);
            assert_eq!((min.x, min.z, max.x, max.z), (0., 0., 640., 640.));
            assert_ne!((min, max), owner.world_bounds());
            let camera = &owner.session_view.camera;
            assert_eq!(camera.viewport.size(), (640, 480));
            let hag_before =
                serde_json::to_vec(&camera.native_camera.capture_continuation()).unwrap();
            // Source calcCameraConstraints uses the newly installed provisional
            // transform, not the previous identity matrix in this fixture.
            // Independently reconstruct its two integer-pixel ground-plane
            // intersections (center320,240; bottom320,456), retaining ground Z
            // in Coord3D::length exactly as W3DView.cpp490–502 does.
            let (eye, look) = camera.native_camera.basis().pose(
                requested,
                camera.camera_zoom,
                camera.camera_pitch_radians,
                camera.camera_yaw_radians,
            );
            let provisional = glam::Mat4::look_at_rh(eye, look, Vec3::Y);
            // W3DView549-573 installs this source lens BEFORE the provisional
            // constraint query. The hostile old IDENTITY projection above is
            // not an input to calcCameraConstraints. View.cpp53 stores50deg
            // horizontal FOV; CameraClass257-261 converts using aspect640/480.
            assert_eq!(camera.camera_zoom, 1.);
            assert_eq!(camera.camera_fx_pitch, 1.);
            let aspect = 640_f32 / 480_f32;
            let half_horizontal = 50_f32.to_radians() * 0.5;
            let vertical = 2. * (half_horizontal.tan() / aspect).atan();
            let f = 1. / (vertical * 0.5).tan();
            let near = 10_f32;
            let far = 1200_f32;
            let projection = glam::Mat4::from_cols(
                glam::Vec4::new(f / aspect, 0., 0., 0.),
                glam::Vec4::new(0., f, 0., 0.),
                glam::Vec4::new(0., 0., far / (near - far), -1.),
                glam::Vec4::new(0., 0., near * far / (near - far), 0.),
            );
            let inverse = (projection * provisional).inverse();
            let ground = camera.native_camera.basis().ground_level();
            let ground_hit = |pixel_x: f32, pixel_y: f32| {
                let x = (2. * pixel_x) / 640. - 1.;
                let y = -((2. * pixel_y) / 480. - 1.);
                let a = inverse * glam::Vec4::new(x, y, 0., 1.);
                let b = inverse * glam::Vec4::new(x, y, 1., 1.);
                let a = a.truncate() / a.w;
                let b = b.truncate() / b.w;
                a + (b - a) * ((ground - a.y) / (b.y - a.y))
            };
            let center = ground_hit(320., 240.);
            let bottom = ground_hit(320., 456.);
            let dx = center.x - bottom.x;
            let dz = center.z - bottom.z;
            let inset = f64::from(dx * dx + dz * dz + ground * ground).sqrt() as f32;
            assert!(inset > 0.);
            let expected = Vec3::new(
                requested.x.max(min.x + inset).min(max.x - inset),
                0.,
                requested.z.max(min.z + inset).min(max.z - inset),
            );
            assert!(
                expected.z < max.z,
                "provisional geometry must change the old zero-inset expectation"
            );
            let (expected_eye, expected_look) = camera.native_camera.basis().pose(
                expected,
                camera.camera_zoom,
                camera.camera_pitch_radians,
                camera.camera_yaw_radians,
            );
            owner.session_view.camera.script_camera_shakers.push(
                crate::presentation_frame::camera::ScriptCameraShaker::new(
                    Vec3::ZERO,
                    10000.,
                    30.,
                    0.,
                ),
            );
            owner.request_camera_focus(requested);
            assert_eq!(owner.session_view.camera.projection_matrix, projection);
            assert_eq!(owner.session_view.target, expected);
            assert_eq!(owner.session_view.camera.camera_position, expected_eye);
            assert_eq!(
                owner.session_view.camera.view_matrix,
                glam::Mat4::look_at_rh(expected_eye, expected_look, Vec3::Y)
            );
            assert_eq!(
                owner.session_view.camera.script_camera_shakers[0].elapsed_seconds,
                2. * (1. / 30.)
            );
            assert_eq!(
                serde_json::to_vec(
                    &owner
                        .session_view
                        .camera
                        .native_camera
                        .capture_continuation()
                )
                .unwrap(),
                hag_before
            );
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
            // Invalid caches correctly discard prior path widening on rebuild.
            owner
                .session_view
                .camera
                .constraints
                .recalculate(Vec3::ZERO, Vec3::new(640., 0., 640.), 0.)
                .unwrap();
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
