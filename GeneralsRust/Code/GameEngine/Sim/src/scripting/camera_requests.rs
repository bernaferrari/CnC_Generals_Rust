//! Translate typed script camera requests for an installed presentation callback.

use super::presentation::ScriptActionHandler;
use super::world::ScriptCameraRequest;
use crate::GameLogicResult;

impl ScriptCameraRequest<'_> {
    /// Compatibility adapter for standalone engines, after releasing engine borrows.
    pub fn dispatch_to(self, handler: &dyn ScriptActionHandler) -> GameLogicResult<()> {
        match self {
            Self::MoveTo {
                x,
                y,
                z,
                seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => handler.move_camera_to(
                x,
                y,
                z,
                seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            ),
            Self::WaypointPath {
                waypoint_path,
                seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => handler.move_camera_along_waypoint_path(
                waypoint_path,
                seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            ),
            Self::Rotate {
                rotations,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => handler.rotate_camera(rotations, seconds, ease_in_seconds, ease_out_seconds),
            Self::MoveToSelection => handler.move_camera_to_selection(),
            Self::MoveHome => handler.camera_move_home(),
            Self::Zoom {
                zoom,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => handler.zoom_camera(zoom, seconds, ease_in_seconds, ease_out_seconds),
            Self::Pitch {
                pitch,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => handler.set_camera_pitch(pitch, seconds, ease_in_seconds, ease_out_seconds),
            Self::FreezeTime => handler.camera_mod_freeze_time(),
            Self::FreezeAngle => handler.camera_mod_freeze_angle(),
            Self::FinalZoom {
                zoom,
                ease_in,
                ease_out,
            } => handler.camera_mod_set_final_zoom(zoom, ease_in, ease_out),
            Self::FinalPitch {
                pitch,
                ease_in,
                ease_out,
            } => handler.camera_mod_set_final_pitch(pitch, ease_in, ease_out),
            Self::FinalSpeedMultiplier { multiplier } => {
                handler.camera_mod_set_final_speed_multiplier(multiplier)
            }
            Self::RollingAverage { frames } => handler.camera_mod_set_rolling_average(frames),
            Self::FollowObject {
                object_id,
                snap_to_unit,
            } => handler.camera_follow_object(object_id, snap_to_unit),
            Self::TetherObject {
                object_id,
                snap_to_unit,
                play,
            } => handler.camera_tether_object(object_id, snap_to_unit, play),
            Self::StopFollow => handler.stop_camera_follow(),
            Self::ResetTo {
                x,
                y,
                z,
                duration_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => handler.reset_camera_to(
                x,
                y,
                z,
                duration_seconds,
                ease_in_seconds,
                ease_out_seconds,
            ),
            Self::Setup {
                x,
                y,
                z,
                zoom,
                pitch,
                look_toward_x,
                look_toward_y,
                look_toward_z,
            } => handler.setup_camera(
                x,
                y,
                z,
                zoom,
                pitch,
                look_toward_x,
                look_toward_y,
                look_toward_z,
            ),
            Self::LookTowardObject {
                object_id,
                seconds,
                hold_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => handler.camera_look_toward_object(
                object_id,
                seconds,
                hold_seconds,
                ease_in_seconds,
                ease_out_seconds,
            ),
            Self::LookTowardWaypoint {
                x,
                y,
                z,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
                reverse_rotation,
            } => handler.camera_look_toward_waypoint(
                x,
                y,
                z,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
                reverse_rotation,
            ),
            Self::LookToward { x, y, z } => handler.camera_mod_look_toward(x, y, z),
            Self::FinalLookToward { x, y, z } => handler.camera_mod_final_look_toward(x, y, z),
            Self::LetterboxBegin => handler.camera_letterbox_begin(),
            Self::LetterboxEnd => handler.camera_letterbox_end(),
            Self::SetDefault {
                pitch,
                angle,
                max_height,
            } => handler.camera_set_default(pitch, angle, max_height),
            Self::EnableSlaveMode {
                thing_template_name,
                bone_name,
            } => handler.camera_enable_slave_mode(thing_template_name, bone_name),
            Self::DisableSlaveMode => handler.camera_disable_slave_mode(),
            Self::ScreenShake { intensity } => handler.screen_shake(intensity),
            Self::AddShakerAt {
                x,
                y,
                z,
                amplitude,
                duration_seconds,
                radius,
            } => handler.camera_add_shaker_at(x, y, z, amplitude, duration_seconds, radius),
            Self::ViewGuardband { gbx, gby } => handler.resize_view_guardband(gbx, gby),
            Self::BwMode { enabled, frames } => handler.set_camera_bw_mode(enabled, frames),
            Self::SkyboxEnabled { enabled } => handler.set_skybox_enabled(enabled),
            Self::MotionBlur { zoom_in, saturate } => handler.camera_motion_blur(zoom_in, saturate),
            Self::MotionBlurJump { x, y, z, saturate } => {
                match handler.camera_motion_blur_jump(x, y, z, saturate) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        log::warn!(
                            "Script action handler camera_motion_blur_jump failed: {}",
                            err
                        );
                        handler.move_camera_to(x, y, z, 0.0, 0.0, 0.0, 0.0)
                    }
                }
            }
            Self::MotionBlurFollow { amount } => handler.camera_motion_blur_follow(amount),
            Self::MotionBlurEndFollow => handler.camera_motion_blur_end_follow(),
        }
    }
}
