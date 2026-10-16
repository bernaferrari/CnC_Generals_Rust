//! Immutable camera observation for the native post-scene effect draw.
//! The session has already applied terrain/HAG, FX pitch and camera shake.
use game_client::display::shadow_pass::VisibleBox;
use game_client::effects::particle_renderer::ParticleUniforms;
use glam::{Mat4, Vec3};

#[derive(Clone, Copy)]
pub(super) struct ParticleCamera {
    pub(super) uniforms: ParticleUniforms,
    pub(super) visible: VisibleBox,
}

impl ParticleCamera {
    /// Convert legacy Z-up positions into the receiving native Y-up view.
    /// C++ W3DParticleSys::doParticles uses rinfo.Camera, including its frustum;
    /// BaseHeightMap::getMaximumVisibleBox clips its four depth edges to terrain.
    pub(super) fn capture(
        view: Mat4,
        projection: Mat4,
        camera_position: Vec3,
        time: f32,
        min_height: f32,
    ) -> Option<Self> {
        let axis = super::terrain_to_main_axis_matrix();
        let particle_view = view * axis;
        if !camera_position.is_finite() {
            return None;
        }
        let visible = game_client::display::shadow_pass::clipped_visible_box_z_up(
            particle_view,
            projection,
            min_height,
        )?;
        let camera = axis.transform_point3(camera_position);
        Some(Self {
            uniforms: ParticleUniforms {
                view_matrix: particle_view.to_cols_array_2d(),
                projection_matrix: projection.to_cols_array_2d(),
                camera_position: camera.to_array(),
                time,
                source_z_up: 1,
                ..ParticleUniforms::default()
            },
            visible,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_terrain_pass_selects_receiving_y_up_snow_camera_explicitly() {
        let source = include_str!("pipeline_prewarm.rs")
            .split("pub(super) fn update_and_enqueue_terrain_pass(")
            .nth(1)
            .unwrap();
        assert!(source.contains(".render_native_y_up("));
        assert!(source.contains("snow_min_height"));
        assert!(source.contains("frame.world_env.runtime_heightmap.as_ref()"));
        assert!(!source.contains("snow_terrain_visible_xy"));
    }

    fn observation(eye: Vec3, target: Vec3, ground: f32) -> ParticleCamera {
        ParticleCamera::capture(
            Mat4::look_at_rh(eye, target, Vec3::Y),
            Mat4::perspective_rh(0.8, 1.6, 10., 12_000.),
            eye,
            7.,
            ground,
        )
        .unwrap()
    }

    #[test]
    fn legacy_particles_project_through_the_exact_receiving_camera() {
        let eye = Vec3::new(120., 475., -240.);
        let target = Vec3::new(170., 85., 130.);
        let view = Mat4::look_at_rh(eye, target, Vec3::Y);
        let projection = Mat4::perspective_rh(0.8, 1.6, 10., 12_000.);
        let captured = observation(eye, target, 85.);
        let particle_view = Mat4::from_cols_array_2d(&captured.uniforms.view_matrix);
        let axis = super::super::terrain_to_main_axis_matrix();
        for point in [
            target,
            Vec3::new(-50., 110., 30.),
            eye + Vec3::new(20., -70., 60.),
        ] {
            let legacy = axis.transform_point3(point);
            let expected = projection * view * point.extend(1.);
            let actual = projection * particle_view * legacy.extend(1.);
            assert!(
                actual.abs_diff_eq(expected, 0.001),
                "{actual:?} != {expected:?}"
            );
        }
        assert_eq!(captured.uniforms.camera_position, [120., -240., 475.]);
        assert_eq!(captured.uniforms.source_z_up, 1);
        // The axis reflection is intentional. Billboard axes remain orthonormal
        // and map to camera X/Y; the particle pipeline disables backface culling.
        let inverse = particle_view.inverse();
        assert!(
            particle_view
                .transform_vector3(inverse.x_axis.truncate())
                .abs_diff_eq(Vec3::X, 1e-5)
        );
        assert!(
            particle_view
                .transform_vector3(inverse.y_axis.truncate())
                .abs_diff_eq(Vec3::Y, 1e-5)
        );
    }

    #[test]
    fn captured_particle_camera_survives_peer_changes_and_keeps_terrain_plane() {
        let a = observation(
            Vec3::new(120., 475., -240.),
            Vec3::new(170., 85., 130.),
            85.,
        );
        let before = (a.uniforms, a.visible);
        let peer = observation(
            Vec3::new(-1500., 800., 1400.),
            Vec3::new(-1600., 210., 1000.),
            210.,
        );
        assert_ne!(a.uniforms.view_matrix, peer.uniforms.view_matrix);
        assert_ne!(a.visible.center, peer.visible.center);
        assert_eq!(a.uniforms, before.0);
        assert_eq!(a.visible.center, before.1.center);
        assert_eq!(a.visible.extent, before.1.extent);
        let minimum_z = a.visible.center[2] - a.visible.extent[2];
        assert!((minimum_z - 85.).abs() < 0.01, "{minimum_z}");
        assert_eq!(a.uniforms.time, 7.);
    }

    #[test]
    fn receiving_session_hag_fx_and_encoded_restore_feed_the_same_draw_observation() {
        use crate::presentation_frame::{SessionView, camera::continuation::SessionCameraState};
        fn capture(view: &SessionView) -> ParticleCamera {
            ParticleCamera::capture(
                view.camera.view_matrix,
                view.camera.projection_matrix,
                view.camera.camera_position,
                9.,
                85.,
            )
            .unwrap()
        }
        let mut view = SessionView::default();
        view.target = Vec3::new(170., 85., 130.);
        view.camera
            .native_camera
            .initialize_map(85., 390., 37.5, 0.);
        view.camera.projection_matrix = Mat4::perspective_rh(0.8, 1.6, 10., 12_000.);
        view.camera.camera_zoom = 0.9;
        view.camera.camera_fx_pitch = 0.7;
        view.camera.camera_shake_rotation = Vec3::new(0.02, -0.03, 0.01);
        view.commit_orbit_pose();
        let before = capture(&view);
        let saved_view = view.capture();
        let saved_camera =
            serde_json::to_string(&SessionCameraState::capture(&view.camera)).unwrap();
        let mut peer = SessionView::default();
        peer.target = Vec3::new(-300., 40., 500.);
        peer.camera
            .native_camera
            .initialize_map(40., 800., 45., 15.);
        peer.commit_orbit_pose();
        assert_ne!(
            capture(&peer).uniforms.view_matrix,
            before.uniforms.view_matrix
        );
        peer = SessionView::default();
        assert_ne!(
            capture(&peer).uniforms.view_matrix,
            before.uniforms.view_matrix
        );
        let mut restored = SessionView::default();
        restored.admit(saved_view).unwrap();
        serde_json::from_str::<SessionCameraState>(&saved_camera)
            .unwrap()
            .admit(&mut restored.camera);
        let after = capture(&restored);
        assert_eq!(after.uniforms, before.uniforms);
        assert_eq!(after.visible.center, before.visible.center);
        assert_eq!(after.visible.extent, before.visible.extent);
        let unchanged =
            serde_json::to_string(&SessionCameraState::capture(&restored.camera)).unwrap();
        for _ in 0..120 {
            assert_eq!(capture(&restored).uniforms, after.uniforms);
        }
        assert_eq!(
            serde_json::to_string(&SessionCameraState::capture(&restored.camera)).unwrap(),
            unchanged
        );
    }

    #[test]
    fn singular_particle_camera_is_rejected_without_an_ambient_fallback() {
        assert!(ParticleCamera::capture(Mat4::ZERO, Mat4::IDENTITY, Vec3::ZERO, 0., 0.).is_none());
    }
}
