//! Detached validation before any owner mutation.
use super::*;

impl CameraState {
    pub(super) fn validate(&self) -> Result<(), String> {
        self.validate_with_completed_view(None)
    }

    pub(super) fn validate_with_completed_view(
        &self,
        completed_view: Option<glam::Mat4>,
    ) -> Result<(), String> {
        if let Some(matrix) = completed_view {
            let inverse =
                crate::presentation_frame::camera::pose::inverse_camera_transform(matrix)?;
            let same =
                |a: Vec3, b: Vec3| a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits);
            let eye = inverse.transform_point3(Vec3::ZERO);
            let forward = inverse.transform_vector3(Vec3::NEG_Z);
            let up = inverse.transform_vector3(Vec3::Y);
            if !eye.is_finite()
                || !forward.is_finite()
                || !up.is_finite()
                || !same(eye, self.completed_eye)
                || !same(forward, self.completed_forward)
                || !same(up, self.completed_up)
            {
                return Err("native camera matrix and pose observations disagree".into());
            }
        } else {
            let forward_length = self.completed_forward.length_squared();
            let up_length = self.completed_up.length_squared();
            let cross_length = self
                .completed_forward
                .cross(self.completed_up)
                .length_squared();
            let look = self.completed_eye + self.completed_forward;
            if !self.completed_eye.is_finite()
                || !self.completed_forward.is_finite()
                || !self.completed_up.is_finite()
                || !look.is_finite()
                || !forward_length.is_finite()
                || !up_length.is_finite()
                || !cross_length.is_finite()
                || cross_length < 1.0e-12
                || forward_length < 1.0e-12
                || up_length < 1.0e-12
                || (look - self.completed_eye).length_squared() < 1.0e-12
            {
                return Err("invalid completed camera view".into());
            }
            if !glam::Mat4::look_at_rh(self.completed_eye, look, self.completed_up).is_finite() {
                return Err("nonfinite reconstructed camera view".into());
            }
        }
        self.height.validate()?;
        self.look_at.validate()?;
        NativeDisplayState::validate_continuation(&self.display)?;
        let now = Instant::now();
        if self
            .script_fps_limit_last_tick_age
            .is_some_and(|n| now.checked_sub(Duration::from_nanos(n)).is_none())
        {
            return Err("unrepresentable camera input age".into());
        }
        if self
            .last_click_time_age
            .is_some_and(|n| now.checked_sub(Duration::from_nanos(n)).is_none())
        {
            return Err("unrepresentable camera input age".into());
        }
        if self
            .last_right_click_time_age
            .is_some_and(|n| now.checked_sub(Duration::from_nanos(n)).is_none())
        {
            return Err("unrepresentable camera input age".into());
        }
        if self
            .rmb_deselect_down_at_age
            .is_some_and(|n| now.checked_sub(Duration::from_nanos(n)).is_none())
        {
            return Err("unrepresentable camera input age".into());
        }
        if self.control_group_click.is_some_and(|(group, n)| {
            group > 9 || now.checked_sub(Duration::from_nanos(n)).is_none()
        }) || self
            .cursor
            .as_deref()
            .is_some_and(|name| cursor_name(name).is_none())
            || self
                .slave
                .as_ref()
                .is_some_and(|(template, bone)| template.len() > 4096 || bone.len() > 4096)
            || self.keys_pressed.len() > 512
            || self.selected_objects.len() > 65536
            || self.script_camera_shakers.len() > 4096
        {
            return Err("invalid camera input or collection bounds".into());
        }
        if !self.camera_position.is_finite()
            || !self.camera_target.is_finite()
            || !self.camera_zoom.is_finite()
            || self.camera_zoom_target.is_some_and(|v| !v.is_finite())
            || !self.camera_zoom_start.is_finite()
            || !self.camera_zoom_duration.is_finite()
            || !self.camera_zoom_ease_in.is_finite()
            || !self.camera_zoom_ease_out.is_finite()
            || !self.camera_pitch_radians.is_finite()
            || self.camera_pitch_target.is_some_and(|v| !v.is_finite())
            || !self.camera_pitch_start.is_finite()
            || !self.camera_pitch_duration.is_finite()
            || !self.camera_pitch_ease_in.is_finite()
            || !self.camera_pitch_ease_out.is_finite()
            || !self.camera_fx_pitch.is_finite()
            || !self.camera_yaw_radians.is_finite()
            || self.camera_yaw_target.is_some_and(|v| !v.is_finite())
            || !self.camera_yaw_start.is_finite()
            || !self.camera_yaw_duration.is_finite()
            || !self.camera_yaw_ease_in.is_finite()
            || !self.camera_yaw_ease_out.is_finite()
            || !self.camera_shake_offset.is_finite()
            || !self.camera_shake_rotation.is_finite()
            || !self.screen_shake_intensity.is_finite()
            || !self.screen_shake_angle_cos.is_finite()
            || !self.screen_shake_angle_sin.is_finite()
            || !self.camera_follow_factor.is_finite()
            || self
                .host_match_script_camera_max_height
                .is_some_and(|v| !v.is_finite())
            || self
                .host_match_script_camera_pitch
                .is_some_and(|v| !v.is_finite())
            || !finite_pair(self.mouse_position)
            || !self.mouse_world_position.is_finite()
            || self.selection_start.is_some_and(|v| !v.is_finite())
            || self.selection_start_screen.is_some_and(|v| !finite_pair(v))
            || self.last_click_position.is_some_and(|v| !finite_pair(v))
            || self
                .last_right_click_screen
                .is_some_and(|v| !finite_pair(v))
            || self.rmb_scroll_anchor.is_some_and(|v| !finite_pair(v))
            || self
                .rmb_deselect_down_screen
                .is_some_and(|v| !finite_pair(v))
            || self
                .rmb_deselect_down_camera
                .is_some_and(|v| !v.is_finite())
            || self.mmb_anchor.is_some_and(|v| !finite_pair(v))
        {
            return Err("nonfinite camera continuation".into());
        }
        if self.camera_zoom <= 0.0
            || self
                .camera_view_bookmarks
                .iter()
                .flatten()
                .any(|v| !v.is_finite())
            || self
                .host_match_camera_follow_position
                .is_some_and(|v| v.iter().any(|v| !v.is_finite()))
            || self
                .scripted_camera_constraint_widen
                .is_some_and(|(a, b, c, d)| {
                    !finite_pair((a, b)) || !finite_pair((c, d)) || a > b || c > d
                })
            || self.script_camera_shakers.iter().any(|v| {
                !v.epicenter.is_finite()
                    || !v.radius.is_finite()
                    || !v.duration_seconds.is_finite()
                    || !v.elapsed_seconds.is_finite()
                    || !v.intensity.is_finite()
                    || !v.omega.is_finite()
                    || !v.phi.is_finite()
            })
        {
            return Err("invalid camera pose, bounds or shaker".into());
        }
        Ok(())
    }
}
fn finite_pair((x, y): (f32, f32)) -> bool {
    x.is_finite() && y.is_finite()
}
