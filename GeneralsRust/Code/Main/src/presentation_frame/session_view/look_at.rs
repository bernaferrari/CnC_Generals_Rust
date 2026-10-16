//! Immediate W3DView::lookAt geometry against the receiving terrain.
use super::SessionView;
use gamelogic::terrain::TerrainLogic;
use glam::Vec3;

fn terrain_hit(start: Vec3, direction: Vec3, terrain: &TerrainLogic) -> Option<Vec3> {
    let cell = gamelogic::common::MAP_XY_FACTOR;
    let (width, height) = terrain.logic_height_map_extents();
    let bytes = terrain.logic_height_map_bytes();
    if width <= 0 || height <= 0 || bytes.len() < (width as usize * height as usize) {
        return None;
    }
    let border = -terrain.get_extent_including_border().lo.x / cell;
    let overhang = 64. + border;
    let min = Vec3::new(-overhang * cell, 0., -overhang * cell);
    let max = Vec3::new(
        (width as f32 + overhang) * cell,
        0.,
        (height as f32 + overhang) * cell,
    );
    // Cast_Ray uses clipped samples even at the edge; get_ground_height's
    // outside-interior nearest-sample branch is not the collision surface.
    let surface = |x: f32, z: f32| {
        let x = x / cell + border;
        let z = z / cell + border;
        let ix = x.floor() as i32;
        let iz = z.floor() as i32;
        let fx = x - x.floor();
        let fz = z - z.floor();
        let sample = |x: i32, z: i32| {
            bytes[(z.clamp(0, height - 1) * width + x.clamp(0, width - 1)) as usize] as f32
        };
        let p0 = sample(ix, iz);
        let p2 = sample(ix + 1, iz + 1);
        let h = if fz > fx {
            let p3 = sample(ix, iz + 1);
            p3 + (1. - fz) * (p0 - p3) + fx * (p2 - p3)
        } else {
            let p1 = sample(ix + 1, iz);
            p1 + fz * (p2 - p1) + (1. - fx) * (p0 - p1)
        };
        h * gamelogic::common::MAP_HEIGHT_SCALE
    };
    let mut enter = 0_f32;
    let mut exit = 1_f32;
    for (origin, delta, lo, hi) in [
        (start.x, direction.x, min.x, max.x),
        (start.z, direction.z, min.z, max.z),
    ] {
        if delta.abs() < 1e-7 {
            if origin < lo || origin > hi {
                return None;
            }
        } else {
            let a = (lo - origin) / delta;
            let b = (hi - origin) / delta;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
    }
    if enter > exit || exit < 0. || enter > 1. {
        return None;
    }
    enter = enter.max(0.);
    exit = exit.min(1.);
    let mut cuts = vec![enter, exit];
    for (origin, delta, lo, hi) in [
        (start.x, direction.x, min.x, max.x),
        (start.z, direction.z, min.z, max.z),
        (
            start.x - start.z,
            direction.x - direction.z,
            min.x - max.z,
            max.x - min.z,
        ),
    ] {
        if delta.abs() < 1e-7 {
            continue;
        }
        for index in ((lo / cell).floor() as i32)..=((hi / cell).ceil() as i32) {
            let t = (index as f32 * cell - origin) / delta;
            if t > enter && t < exit {
                cuts.push(t);
            }
        }
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup();
    for span in cuts.windows(2) {
        let a = start + direction * span[0];
        let b = start + direction * span[1];
        let da = a.y - surface(a.x, a.z);
        let db = b.y - surface(b.x, b.z);
        if !da.is_finite() || !db.is_finite() {
            return None;
        }
        if da >= 0. && db <= 0. {
            let fraction = if da == db { 0. } else { da / (da - db) };
            return Some(a + (b - a) * fraction);
        }
    }
    None
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
                if let Some(hit) = terrain_hit(requested, direction * depth, terrain) {
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
