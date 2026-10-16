//! Source setCameraTransform/buildCameraTransform phases on the same owner.
use crate::game_logic::mission_scripts::CameraSlaveModeRequest;
use crate::presentation_frame::SessionView;
use glam::{Mat4, Vec3};
/// Actual receiving sampler outcomes. Unavailable never means a foreign view.
pub(crate) enum SlaveCameraSample {
    MissingObject,
    MissingDrawable,
    NoInterface,
    Pose(Mat4),
    Unavailable(&'static str),
}
impl SessionView {
    pub(crate) fn set_camera_transform(
        &mut self,
        bounds: (Vec3, Vec3),
        sample: &mut dyn FnMut(&CameraSlaveModeRequest) -> SlaveCameraSample,
    ) -> Result<(), String> {
        self.camera
            .set_camera_transform(&mut self.target, self.impulse.offset, bounds, sample)
    }
}
impl super::SessionCamera {
    pub(crate) fn set_camera_transform(
        &mut self,
        target: &mut Vec3,
        impulse: Vec3,
        bounds: (Vec3, Vec3),
        sample: &mut dyn FnMut(&CameraSlaveModeRequest) -> SlaveCameraSample,
    ) -> Result<(), String> {
        if !target.is_finite()
            || !impulse.is_finite()
            || !bounds.0.is_finite()
            || !bounds.1.is_finite()
        {
            return Err("nonfinite receiving camera transform inputs".into());
        }
        self.constraints.validate()?;
        let (near, far) = super::projection::source_clip_planes(
            self.clip_policy.current().draw_entire_terrain,
            self.camera_fx_pitch,
            self.camera_zoom,
            false,
        )?;
        let projection = self.projection_for_clip_planes(near, far);
        if !projection.is_finite() {
            return Err("invalid receiving clip projection".into());
        }
        // Source Set_Clip_Planes precedes provisional transform and constraint rays.
        self.projection_matrix = projection;
        if !self.constraints.is_valid() {
            self.build_camera_transform(target, impulse, sample)?;
            self.force_recalculate_constraints(bounds)?;
        }
        if let Some(bounds) = self.constraint_bounds() {
            *target = bounds.constrain(*target);
        }
        self.build_camera_transform(target, impulse, sample)
    }
    /// One actual source build advances rotational shakers, then samples slave.
    /// Pure redraw/matrix inspection uses commit_orbit_pose instead.
    fn build_camera_transform(
        &mut self,
        target: &mut Vec3,
        impulse: Vec3,
        sample: &mut dyn FnMut(&CameraSlaveModeRequest) -> SlaveCameraSample,
    ) -> Result<(), String> {
        self.advance_transform_shakers(*target, impulse);
        let (eye, look) = super::pose::camera_orbit_pose(
            &self.native_camera,
            *target,
            impulse,
            self.camera_zoom,
            self.camera_pitch_radians,
            self.camera_yaw_radians,
            self.camera_fx_pitch,
            self.constraint_bounds(),
        );
        let view = super::pose::camera_orbit_view(eye, look, self.camera_shake_rotation);
        if let Some(mode) = self.camera_slave_mode.as_ref() {
            match sample(mode) {
                SlaveCameraSample::Pose(world) => {
                    if self.commit_native_slave_world(target, world) {
                        return Ok(());
                    }
                    return Err("invalid receiving slave camera matrix".into());
                }
                SlaveCameraSample::MissingObject | SlaveCameraSample::MissingDrawable => {
                    self.camera_slave_mode = None;
                }
                SlaveCameraSample::NoInterface => {}
                SlaveCameraSample::Unavailable(reason) => {
                    return Err(format!("receiving slave camera unavailable: {reason}"));
                }
            }
        }
        self.camera_position = eye;
        self.view_matrix = view;
        Ok(())
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
