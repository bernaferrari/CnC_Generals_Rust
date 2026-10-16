//! W3DView's cached map limits. Logical and shake-local positions are distinct.
use glam::Vec3;
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ConstraintBounds {
    pub(crate) x_min: f32,
    pub(crate) x_max: f32,
    pub(crate) z_min: f32,
    pub(crate) z_max: f32,
}
impl ConstraintBounds {
    pub(crate) fn for_map(min: Vec3, max: Vec3, inset: f32) -> Self {
        Self {
            x_min: min.x + inset,
            x_max: max.x - inset,
            z_min: min.z + inset,
            z_max: max.z - inset,
        }
    }
    pub(crate) fn widen(mut self, widen: Option<(f32, f32, f32, f32)>) -> Self {
        if let Some((a, b, c, d)) = widen {
            self.x_min = self.x_min.min(a);
            self.x_max = self.x_max.max(b);
            self.z_min = self.z_min.min(c);
            self.z_max = self.z_max.max(d);
        }
        self
    }
    pub(crate) fn constrain(self, mut position: Vec3) -> Vec3 {
        // Source max then min intentionally keeps this order for inverted endpoints.
        position.x = position.x.max(self.x_min).min(self.x_max);
        position.z = position.z.max(self.z_min).min(self.z_max);
        position
    }
    pub(crate) fn validate(self) -> Result<(), String> {
        if [self.x_min, self.x_max, self.z_min, self.z_max]
            .into_iter()
            .all(f32::is_finite)
        {
            Ok(())
        } else {
            Err("nonfinite camera constraint bounds".into())
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CameraConstraints {
    bounds: Option<ConstraintBounds>,
}
impl CameraConstraints {
    pub(crate) fn validate(self) -> Result<(), String> {
        self.bounds.map_or(Ok(()), ConstraintBounds::validate)
    }
    pub(crate) fn invalidate(&mut self) {
        self.bounds = None;
    }
    pub(crate) fn bounds(self, widen: Option<(f32, f32, f32, f32)>) -> Option<ConstraintBounds> {
        self.bounds.map(|bounds| bounds.widen(widen))
    }
    pub(crate) fn recalculate(&mut self, min: Vec3, max: Vec3, inset: f32) -> Result<(), String> {
        let bounds = ConstraintBounds::for_map(min, max, inset);
        bounds.validate()?;
        self.bounds = Some(bounds);
        Ok(())
    }
    pub(crate) fn is_valid(self) -> bool {
        self.bounds.is_some()
    }
}
impl super::SessionCamera {
    pub(crate) fn invalidate_constraints(&mut self) {
        self.constraints.invalidate();
    }
    /// Only an already admitted cache may constrain an observer/input target.
    /// Invalid caches preserve the raw request until the actual provisional build.
    pub(crate) fn constrain_target_if_cached(&self, position: Vec3) -> Vec3 {
        self.constraint_bounds()
            .map_or(position, |bounds| bounds.constrain(position))
    }
    pub(crate) fn constraint_bounds(&self) -> Option<ConstraintBounds> {
        self.constraints
            .bounds(self.scripted_camera_constraint_widen)
    }
    /// Original forceCameraConstraintRecalc: already installed matrix, zero builds.
    pub(crate) fn force_recalculate_constraints(
        &mut self,
        bounds: (Vec3, Vec3),
    ) -> Result<(), String> {
        let inset = self.viewport.constraint_inset(
            self.view_matrix,
            self.projection_matrix,
            self.native_camera.basis().ground_level(),
        );
        self.constraints.recalculate(bounds.0, bounds.1, inset)?;
        self.scripted_camera_constraint_widen = None;
        Ok(())
    }
}
#[cfg(test)]
#[path = "constraint_tests.rs"]
mod tests;
