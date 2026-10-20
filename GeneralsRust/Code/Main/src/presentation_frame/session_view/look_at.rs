//! Immediate W3DView::lookAt geometry against the receiving terrain.
use super::SessionView;
use gamelogic::terrain::TerrainLogic;
use glam::Vec3;

#[cfg(test)]
#[path = "terrain_ray_tests.rs"]
mod terrain_ray_tests;

fn terrain_hit(
    start: Vec3,
    direction: Vec3,
    terrain: &TerrainLogic,
) -> Result<Option<Vec3>, String> {
    // The collision component uses original XYZ (height Z). Main's completed
    // camera uses Y-up; convert once at this receiving-owner boundary.
    let source = |point: Vec3| Vec3::new(point.x, point.z, point.y);
    let cast = terrain.cast_terrain_ray(source(start), source(start + direction))?;
    Ok(cast.hit.then(|| {
        let point = cast.result.contact_point;
        Vec3::new(point.x, point.z, point.y)
    }))
}

/// Shared immediate source lookAt geometry, no clamp, state mutation or clock.
pub(crate) fn receiving_look_at_target(
    requested: Vec3,
    camera: &super::camera::SessionCamera,
    terrain: &TerrainLogic,
) -> Result<Vec3, String> {
    if !requested.is_finite() {
        return Err("nonfinite receiving LookAt position".into());
    }
    let ground = terrain.get_ground_height(requested.x, requested.z, None);
    if !ground.is_finite() {
        return Err("nonfinite receiving LookAt terrain".into());
    }
    let mut target = requested;
    if requested.y > 10. + ground {
        // Unlike calcCameraConstraints, original lookAt unprojects the
        // normalized center directly, not a truncated center pixel.
        if let Some((near, far)) = super::camera::viewport::TacticalViewport::ray_at_ndc(
            camera.view_matrix,
            camera.projection_matrix,
            glam::Vec2::ZERO,
        ) {
            let direction = (far - near).try_normalize().unwrap_or(-Vec3::Y);
            let eye = camera.view_matrix.inverse().transform_point3(Vec3::ZERO);
            let depth = (far - eye).length();
            if depth.is_finite() && depth > 0. {
                if let Some(hit) = terrain_hit(requested, direction * depth, terrain)? {
                    target.x = hit.x;
                    target.z = hit.z;
                }
            }
        }
    }
    target.y = 0.;
    Ok(target)
}

impl SessionView {
    pub(crate) fn receiving_look_at(
        &mut self,
        requested: Vec3,
        bounds: (Vec3, Vec3),
        terrain: &TerrainLogic,
    ) -> Result<Vec3, String> {
        self.receiving_look_at_with_sampler(requested, bounds, terrain, &mut |_| {
            super::camera::build::SlaveCameraSample::Unavailable(
                "standalone LookAt has no receiving native renderer",
            )
        })
    }
    pub(crate) fn receiving_look_at_with_sampler(
        &mut self,
        requested: Vec3,
        bounds: (Vec3, Vec3),
        terrain: &TerrainLogic,
        sample: &mut dyn FnMut(
            &crate::game_logic::mission_scripts::CameraSlaveModeRequest,
        ) -> super::camera::build::SlaveCameraSample,
    ) -> Result<Vec3, String> {
        if !requested.is_finite() || !bounds.0.is_finite() || !bounds.1.is_finite() {
            return Err("nonfinite receiving LookAt position/bounds".into());
        }
        let target = receiving_look_at_target(requested, &self.camera, terrain)?;
        // Source lookAt writes raw projected XY and zero Z. It does not
        // widen a path constraint or clamp before setCameraTransform builds.
        self.target = target;
        self.set_camera_transform(bounds, sample)?;
        Ok(self.target)
    }

    /// Rebuild the completed pose only. No HAG/scalar/filter frame advances.
    pub(crate) fn commit_orbit_pose(&mut self) {
        if self.camera.camera_slave_mode.is_some() {
            return;
        }
        let (eye, look) = super::camera::pose::camera_orbit_pose(
            &self.camera.native_camera,
            self.target,
            self.impulse.offset,
            self.camera.camera_zoom,
            self.camera.camera_pitch_radians,
            self.camera.camera_yaw_radians,
            self.camera.camera_fx_pitch,
            self.camera.constraint_bounds(),
        );
        self.camera.camera_position = eye;
        self.camera.view_matrix =
            super::camera::pose::camera_orbit_view(eye, look, self.camera.camera_shake_rotation);
    }
}
