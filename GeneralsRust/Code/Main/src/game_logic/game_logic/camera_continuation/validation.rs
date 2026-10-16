use super::*;
pub(super) trait ValidateCamera {
    fn validate(&self) -> Result<(), String>;
}
impl ValidateCamera for f32 {
    fn validate(&self) -> Result<(), String> {
        if self.is_finite() {
            Ok(())
        } else {
            Err("nonfinite camera continuation value".into())
        }
    }
}
impl ValidateCamera for Vec3 {
    fn validate(&self) -> Result<(), String> {
        if self.is_finite() {
            Ok(())
        } else {
            Err("nonfinite camera position".into())
        }
    }
}
impl ValidateCamera for String {
    fn validate(&self) -> Result<(), String> {
        if self.len() <= MAX_ITEMS {
            Ok(())
        } else {
            Err("camera string too long".into())
        }
    }
}
impl<T: ValidateCamera> ValidateCamera for Option<T> {
    fn validate(&self) -> Result<(), String> {
        if let Some(v) = self {
            v.validate()?;
        }
        Ok(())
    }
}
impl<T: ValidateCamera> ValidateCamera for Vec<T> {
    fn validate(&self) -> Result<(), String> {
        if self.len() > MAX_ITEMS {
            return Err("camera queue too long".into());
        }
        for v in self {
            v.validate()?;
        }
        Ok(())
    }
}
impl<T: ValidateCamera, const N: usize> ValidateCamera for [T; N] {
    fn validate(&self) -> Result<(), String> {
        for v in self {
            v.validate()?;
        }
        Ok(())
    }
}
macro_rules! scalar {($($t:ty),*)=>{$(impl ValidateCamera for $t{fn validate(&self)->Result<(),String>{Ok(())}})*};}
scalar!(bool, u32, i32, usize, (), ObjectId);
impl ValidateCamera for LogicCameraContinuation {
    fn validate(&self) -> Result<(), String> {
        self.owned_extras.constraints.validate()?;
        self.owned_extras.pending_focus_previous_target.validate()?;
        self.owned_extras.motion.validate()?;
        if let Some(tracking) = &self.owned_extras.object_tracking {
            tracking.validate()?;
        }
        self.owned_extras.target.validate()?;
        match (&self.owned_extras.camera, self.owned_extras.target) {
            (Some(camera), Some(target)) => {
                camera.validate()?;
                let projected_hold = self
                    .owned_extras
                    .object_tracking
                    .as_ref()
                    .map_or(0., |tracking| tracking.hold_frames as f32 * 33. / 1000.);
                if self.script_look_toward_hold_seconds.to_bits() != projected_hold.to_bits()
                    || (self.script_look_toward_object_id.is_some()
                        && self.owned_extras.object_tracking.is_none())
                    || self.script_camera_move_to.is_some()
                    || self.script_camera_path.is_some()
                    || self.script_camera_freeze_time_armed
                    || self.script_camera_freeze_angle_armed
                    || self.script_camera_pending_final_speed_multiplier.is_some()
                    || target.to_array().map(f32::to_bits)
                        != self
                            .script_camera_focus_estimate
                            .to_array()
                            .map(f32::to_bits)
                    || camera
                        .remaining_axis_seconds(self.owned_extras.object_tracking.as_ref())
                        .map(f32::to_bits)
                        != [
                            self.script_camera_rotate_remaining,
                            self.script_camera_zoom_remaining,
                            self.script_camera_pitch_remaining,
                        ]
                        .map(f32::to_bits)
                {
                    return Err("ambiguous V2 camera continuation projection".into());
                }
            }
            (None, None) => self.reject_old_active_path()?,
            _ => return Err("incomplete V2 camera owner continuation".into()),
        }
        #[cfg(feature = "game_client")]
        self.owned_extras.filter.validate()?;
        self.pending_camera_focus.validate()?;
        self.script_camera_focus_estimate.validate()?;
        self.script_camera_move_to.validate()?;
        self.script_camera_path.validate()?;
        self.camera_follow_target.validate()?;
        self.camera_tether_play.validate()?;
        self.script_look_toward_object_id.validate()?;
        self.script_look_toward_hold_seconds.validate()?;
        self.script_default_camera_pitch.validate()?;
        self.script_default_camera_angle.validate()?;
        self.script_default_camera_max_height.validate()?;
        self.script_camera_freeze_time_armed.validate()?;
        self.script_camera_freeze_time.validate()?;
        self.script_camera_rotate_remaining.validate()?;
        self.script_camera_zoom_remaining.validate()?;
        self.script_camera_pitch_remaining.validate()?;
        self.script_camera_freeze_angle_armed.validate()?;
        self.script_camera_pending_final_speed_multiplier
            .validate()?;
        self.visual_speed_multiplier.validate()?;
        self.script_time_frozen_by_script.validate()?;
        self.pending_script_fps_limit.validate()?;
        self.pending_camera_zoom_reset.validate()?;
        self.pending_camera_zoom_reset_duration.validate()?;
        self.pending_camera_zoom_reset_ease_in.validate()?;
        self.pending_camera_zoom_reset_ease_out.validate()?;
        self.pending_camera_zoom.validate()?;
        self.pending_camera_pitch.validate()?;
        self.pending_camera_rotate.validate()?;
        self.pending_camera_look_toward.validate()?;
        self.pending_camera_slave_mode_enable.validate()?;
        self.pending_camera_slave_mode_disable.validate()?;
        self.pending_screen_shakes.validate()?;
        self.pending_camera_add_shakers.validate()?;
        self.pending_view_guardband.validate()?;
        self.native_view_guardband.validate()?;
        self.pending_camera_bw_mode.validate()?;
        self.pending_camera_motion_blur.validate()?;
        self.script_skybox_enabled.validate()?;
        self.cinematic_letterbox.validate()?;
        self.hooks.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraHookContinuation {
    fn validate(&self) -> Result<(), String> {
        self.camera_moves.validate()?;
        self.camera_follows.validate()?;
        self.camera_tethers.validate()?;
        self.camera_path_moves.validate()?;
        self.camera_move_to.validate()?;
        self.camera_move_to_selection_requests.validate()?;
        self.camera_move_home_requests.validate()?;
        self.camera_resets.validate()?;
        self.camera_zoom_requests.validate()?;
        self.camera_pitch_requests.validate()?;
        self.camera_rotate_requests.validate()?;
        self.camera_mod_final_zoom_requests.validate()?;
        self.camera_mod_final_pitch_requests.validate()?;
        self.camera_mod_freeze_time_requests.validate()?;
        self.camera_mod_freeze_angle_requests.validate()?;
        self.camera_mod_final_speed_multiplier_requests.validate()?;
        self.camera_mod_rolling_average_requests.validate()?;
        self.visual_speed_multiplier_requests.validate()?;
        self.script_freeze_time_requests.validate()?;
        self.set_fps_limit_requests.validate()?;
        self.camera_setup_requests.validate()?;
        self.camera_look_toward_object_requests.validate()?;
        self.camera_look_toward_waypoint_requests.validate()?;
        self.camera_mod_look_toward_requests.validate()?;
        self.camera_mod_final_look_toward_requests.validate()?;
        self.camera_set_default_requests.validate()?;
        self.camera_slave_mode_enable_requests.validate()?;
        self.camera_slave_mode_disable_requests.validate()?;
        self.screen_shake_requests.validate()?;
        self.camera_add_shaker_requests.validate()?;
        self.view_guardband_requests.validate()?;
        self.camera_bw_mode_requests.validate()?;
        self.skybox_enabled_updates.validate()?;
        self.camera_motion_blur_requests.validate()?;
        self.letterbox_events.validate()?;
        self.movement_finished.validate()?;
        Ok(())
    }
}
impl ValidateCamera for V1Ease {
    fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.in_t) || !(self.in_t..=1.0).contains(&self.out_t) {
            return Err("invalid camera ease interval".into());
        }
        self.in_t.validate()?;
        self.out_t.validate()?;
        Ok(())
    }
}
impl ValidateCamera for V1MoveTo {
    fn validate(&self) -> Result<(), String> {
        validate_active_time(
            self.total_time_seconds,
            self.elapsed_seconds,
            self.shutter_frames,
            self.cur_shutter,
        )?;
        self.start.validate()?;
        self.target.validate()?;
        self.ease.validate()?;
        self.total_time_seconds.validate()?;
        self.elapsed_seconds.validate()?;
        self.shutter_frames.validate()?;
        self.cur_shutter.validate()?;
        self.last_ease.validate()?;
        self.freeze_time.validate()?;
        self.freeze_angle.validate()?;
        self.look_toward.validate()?;
        self.suppress_travel_look.validate()?;
        self.speed_ramp_start_t.validate()?;
        self.speed_ramp_start_multiplier.validate()?;
        self.speed_ramp_final_multiplier.validate()?;
        Ok(())
    }
}
impl ValidateCamera for V1Path {
    fn validate(&self) -> Result<(), String> {
        validate_active_time(
            self.total_time_seconds,
            self.elapsed_seconds,
            self.shutter_frames,
            self.cur_shutter,
        )?;
        if self.points.len() < 4
            || self.points.len() > gamelogic::terrain::CAMERA_WAYPOINT_PATH_LIMIT + 3
            || self.segment_length.len() != self.points.len()
            || self.cur_segment == 0
            || self.cur_segment > self.points.len() - 2
            || self.segment_length.iter().any(|v| *v < 0.0)
            || self.total_distance < 0.0
        {
            return Err("invalid camera path continuation shape".into());
        }
        self.points.validate()?;
        self.segment_length.validate()?;
        self.total_distance.validate()?;
        self.ease.validate()?;
        self.total_time_seconds.validate()?;
        self.elapsed_seconds.validate()?;
        self.cur_segment.validate()?;
        self.cur_seg_distance.validate()?;
        self.shutter_frames.validate()?;
        self.cur_shutter.validate()?;
        self.last_ease.validate()?;
        self.freeze_time.validate()?;
        self.freeze_angle.validate()?;
        self.look_toward.validate()?;
        self.look_toward_is_final.validate()?;
        self.suppress_travel_look.validate()?;
        self.rolling_average_frames.validate()?;
        self.smoothed_focus.validate()?;
        self.speed_ramp_start_t.validate()?;
        self.speed_ramp_start_multiplier.validate()?;
        self.speed_ramp_final_multiplier.validate()?;
        self.start_angle.validate()?;
        self.frozen_to_start_angle.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraAddShakerRequest {
    fn validate(&self) -> Result<(), String> {
        self.position.validate()?;
        self.amplitude.validate()?;
        self.duration_seconds.validate()?;
        self.radius.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraBwModeRequest {
    fn validate(&self) -> Result<(), String> {
        self.enabled.validate()?;
        self.frames.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraFollowRequest {
    fn validate(&self) -> Result<(), String> {
        self.object_id.validate()?;
        self.snap_to_unit.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraLookTowardObjectRequest {
    fn validate(&self) -> Result<(), String> {
        self.object_id.validate()?;
        self.duration_seconds.validate()?;
        self.hold_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraLookTowardWaypointRequest {
    fn validate(&self) -> Result<(), String> {
        self.position.validate()?;
        self.duration_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        self.reverse_rotation.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraModFinalLookTowardRequest {
    fn validate(&self) -> Result<(), String> {
        self.position.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraModFinalPitchRequest {
    fn validate(&self) -> Result<(), String> {
        self.pitch.validate()?;
        self.ease_in.validate()?;
        self.ease_out.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraModFinalSpeedMultiplierRequest {
    fn validate(&self) -> Result<(), String> {
        self.multiplier.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraModFinalZoomRequest {
    fn validate(&self) -> Result<(), String> {
        self.zoom.validate()?;
        self.ease_in.validate()?;
        self.ease_out.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraModLookTowardRequest {
    fn validate(&self) -> Result<(), String> {
        self.position.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraModRollingAverageRequest {
    fn validate(&self) -> Result<(), String> {
        self.frames.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraMotionBlurRequest {
    fn validate(&self) -> Result<(), String> {
        if let Self::Jump { position, .. } = self {
            position.validate()?;
        }
        Ok(())
    }
}
impl ValidateCamera for CameraMoveToRequest {
    fn validate(&self) -> Result<(), String> {
        self.position.validate()?;
        self.seconds.validate()?;
        self.camera_stutter_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraPathRequest {
    fn validate(&self) -> Result<(), String> {
        self.waypoint.validate()?;
        self.seconds.validate()?;
        self.camera_stutter_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraPitchRequest {
    fn validate(&self) -> Result<(), String> {
        self.pitch.validate()?;
        self.duration_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraResetRequest {
    fn validate(&self) -> Result<(), String> {
        self.position.validate()?;
        self.duration_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraRotateRequest {
    fn validate(&self) -> Result<(), String> {
        self.rotations.validate()?;
        self.duration_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraSetDefaultRequest {
    fn validate(&self) -> Result<(), String> {
        self.pitch.validate()?;
        self.angle.validate()?;
        self.max_height.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraSetupRequest {
    fn validate(&self) -> Result<(), String> {
        self.position.validate()?;
        self.zoom.validate()?;
        self.pitch.validate()?;
        self.look_toward.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraSlaveModeRequest {
    fn validate(&self) -> Result<(), String> {
        self.thing_template_name.validate()?;
        self.bone_name.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraTetherRequest {
    fn validate(&self) -> Result<(), String> {
        self.object_id.validate()?;
        self.snap_to_unit.validate()?;
        self.play.validate()?;
        Ok(())
    }
}
impl ValidateCamera for CameraZoomRequest {
    fn validate(&self) -> Result<(), String> {
        self.zoom.validate()?;
        self.duration_seconds.validate()?;
        self.ease_in_seconds.validate()?;
        self.ease_out_seconds.validate()?;
        Ok(())
    }
}
impl ValidateCamera for ScreenShakeRequest {
    fn validate(&self) -> Result<(), String> {
        self.intensity.validate()?;
        Ok(())
    }
}
impl ValidateCamera for SetFpsLimitRequest {
    fn validate(&self) -> Result<(), String> {
        self.fps.validate()?;
        Ok(())
    }
}
impl ValidateCamera for ViewGuardbandRequest {
    fn validate(&self) -> Result<(), String> {
        self.x_bias.validate()?;
        self.y_bias.validate()?;
        Ok(())
    }
}
impl ValidateCamera for VisualSpeedMultiplierRequest {
    fn validate(&self) -> Result<(), String> {
        self.multiplier.validate()?;
        Ok(())
    }
}

fn validate_active_time(
    total: f32,
    elapsed: f32,
    shutter: u32,
    current: u32,
) -> Result<(), String> {
    if total <= 0.0 || elapsed < 0.0 || elapsed > total || shutter == 0 || current > shutter {
        return Err("invalid active camera timing".into());
    }
    Ok(())
}
