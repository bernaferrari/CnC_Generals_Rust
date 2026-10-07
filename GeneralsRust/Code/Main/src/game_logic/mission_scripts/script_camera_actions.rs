// Main translation of the canonical ScriptActions camera/view callbacks.
// The borrowed driver remains the owner boundary; this adapter preserves the
// existing ordered hook queue representation for standalone handler callers.

impl MissionScriptHooks {
    pub(crate) fn apply_camera_request(
        &self,
        request: ScriptCameraRequest<'_>,
    ) -> GameLogicResult<()> {
        match request {
            ScriptCameraRequest::MoveTo {
                x,
                y,
                z,
                seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => {
                static DEBUG_CAMERA_MOVE_TO_LOGS: AtomicUsize = AtomicUsize::new(0);
                let position = camera_coord3d_to_world(x, y, z);
                if DEBUG_CAMERA_MOVE_TO_LOGS.fetch_add(1, Ordering::Relaxed) < 16 {
                    eprintln!(
                        "DEBUG_SHELL_CAMERA_ACTION: move_camera_to raw=({x:.3}, {y:.3}, {z:.3}) world={position:?} seconds={seconds:.3}"
                    );
                }
                if seconds <= 0.0 {
                    self.push_camera_move(position);
                    return Ok(());
                }
                self.push_camera_move_to(CameraMoveToRequest {
                    position,
                    seconds,
                    camera_stutter_seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                });
            }
            ScriptCameraRequest::WaypointPath {
                waypoint_path,
                seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => {
                self.push_camera_path_move(CameraPathRequest {
                    waypoint: waypoint_path.to_string(),
                    seconds,
                    camera_stutter_seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                });
            }
            ScriptCameraRequest::Rotate {
                rotations,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => {
                self.push_camera_rotate(CameraRotateRequest {
                    rotations,
                    duration_seconds: seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                });
            }
            ScriptCameraRequest::MoveToSelection => self.push_camera_move_to_selection(),
            ScriptCameraRequest::MoveHome => self.push_camera_move_home(),
            ScriptCameraRequest::Zoom {
                zoom,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => {
                self.push_camera_zoom(CameraZoomRequest {
                    zoom,
                    duration_seconds: seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                });
            }
            ScriptCameraRequest::Pitch {
                pitch,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => {
                self.push_camera_pitch(CameraPitchRequest {
                    pitch,
                    duration_seconds: seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                });
            }
            ScriptCameraRequest::FreezeTime => self.push_camera_mod_freeze_time(),
            ScriptCameraRequest::FreezeAngle => self.push_camera_mod_freeze_angle(),
            ScriptCameraRequest::FinalZoom {
                zoom,
                ease_in,
                ease_out,
            } => self.push_camera_mod_final_zoom(CameraModFinalZoomRequest {
                zoom,
                ease_in,
                ease_out,
            }),
            ScriptCameraRequest::FinalPitch {
                pitch,
                ease_in,
                ease_out,
            } => self.push_camera_mod_final_pitch(CameraModFinalPitchRequest {
                pitch,
                ease_in,
                ease_out,
            }),
            ScriptCameraRequest::FinalSpeedMultiplier { multiplier } => {
                self.push_camera_mod_final_speed_multiplier(CameraModFinalSpeedMultiplierRequest {
                    multiplier,
                });
            }
            ScriptCameraRequest::RollingAverage { frames } => {
                self.push_camera_mod_rolling_average(CameraModRollingAverageRequest { frames });
            }
            ScriptCameraRequest::FollowObject {
                object_id,
                snap_to_unit,
            } => {
                self.push_camera_follow(CameraFollowRequest {
                    object_id,
                    snap_to_unit,
                });
            }
            ScriptCameraRequest::TetherObject {
                object_id,
                snap_to_unit,
                play,
            } => {
                self.push_camera_tether(CameraTetherRequest {
                    object_id,
                    snap_to_unit,
                    play,
                });
            }
            ScriptCameraRequest::StopFollow => {
                self.push_camera_follow(CameraFollowRequest {
                    object_id: 0,
                    snap_to_unit: false,
                });
            }
            ScriptCameraRequest::ResetTo {
                x,
                y,
                z,
                duration_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => {
                self.push_camera_reset(CameraResetRequest {
                    position: camera_coord3d_to_world(x, y, z),
                    duration_seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                });
            }
            ScriptCameraRequest::Setup {
                x,
                y,
                z,
                zoom,
                pitch,
                look_toward_x,
                look_toward_y,
                look_toward_z,
            } => {
                self.push_camera_setup(CameraSetupRequest {
                    position: camera_coord3d_to_world(x, y, z),
                    zoom,
                    pitch,
                    look_toward: camera_coord3d_to_world(
                        look_toward_x,
                        look_toward_y,
                        look_toward_z,
                    ),
                });
            }
            ScriptCameraRequest::LookTowardObject {
                object_id,
                seconds,
                hold_seconds,
                ease_in_seconds,
                ease_out_seconds,
            } => {
                self.push_camera_look_toward_object(CameraLookTowardObjectRequest {
                    object_id,
                    duration_seconds: seconds,
                    hold_seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                });
            }
            ScriptCameraRequest::LookTowardWaypoint {
                x,
                y,
                z,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
                reverse_rotation,
            } => {
                self.push_camera_look_toward_waypoint(CameraLookTowardWaypointRequest {
                    position: camera_coord3d_to_world(x, y, z),
                    duration_seconds: seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                    reverse_rotation,
                });
            }
            ScriptCameraRequest::LookToward { x, y, z } => {
                self.push_camera_mod_look_toward(CameraModLookTowardRequest {
                    position: camera_coord3d_to_world(x, y, z),
                });
            }
            ScriptCameraRequest::FinalLookToward { x, y, z } => {
                self.push_camera_mod_final_look_toward(CameraModFinalLookTowardRequest {
                    position: camera_coord3d_to_world(x, y, z),
                });
            }
            ScriptCameraRequest::LetterboxBegin => {
                self.push_letterbox(true);
            }
            ScriptCameraRequest::LetterboxEnd => {
                self.push_letterbox(false);
            }
            ScriptCameraRequest::SetDefault {
                pitch,
                angle,
                max_height,
            } => {
                self.push_camera_set_default(CameraSetDefaultRequest {
                    pitch,
                    angle,
                    max_height,
                });
            }
            ScriptCameraRequest::EnableSlaveMode {
                thing_template_name,
                bone_name,
            } => {
                self.push_camera_slave_mode_enable(CameraSlaveModeRequest {
                    thing_template_name: thing_template_name.to_string(),
                    bone_name: bone_name.to_string(),
                });
            }
            ScriptCameraRequest::DisableSlaveMode => {
                self.push_camera_slave_mode_disable();
            }
            ScriptCameraRequest::ScreenShake { intensity } => {
                self.push_screen_shake(ScreenShakeRequest { intensity });
            }
            ScriptCameraRequest::AddShakerAt {
                x,
                y,
                z,
                amplitude,
                duration_seconds,
                radius,
            } => {
                self.push_camera_add_shaker(CameraAddShakerRequest {
                    position: camera_coord3d_to_world(x, y, z),
                    amplitude,
                    duration_seconds,
                    radius,
                });
            }
            ScriptCameraRequest::ViewGuardband { gbx, gby } => {
                self.push_view_guardband(ViewGuardbandRequest {
                    x_bias: gbx,
                    y_bias: gby,
                });
            }
            ScriptCameraRequest::BwMode { enabled, frames } => {
                self.push_camera_bw_mode(CameraBwModeRequest { enabled, frames });
            }
            ScriptCameraRequest::SkyboxEnabled { enabled } => {
                self.push_skybox_enabled(enabled);
            }
            ScriptCameraRequest::MotionBlur { zoom_in, saturate } => {
                self.push_camera_motion_blur(CameraMotionBlurRequest::Basic { zoom_in, saturate });
            }
            ScriptCameraRequest::MotionBlurJump { x, y, z, saturate } => {
                self.push_camera_motion_blur(CameraMotionBlurRequest::Jump {
                    position: camera_coord3d_to_world(x, y, z),
                    saturate,
                });
            }
            ScriptCameraRequest::MotionBlurFollow { amount } => {
                self.push_camera_motion_blur(CameraMotionBlurRequest::Follow { amount });
            }
            ScriptCameraRequest::MotionBlurEndFollow => {
                self.push_camera_motion_blur(CameraMotionBlurRequest::EndFollow);
            }
        }
        Ok(())
    }
}
