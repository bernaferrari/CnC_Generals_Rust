// Main translation of the canonical ScriptActions camera movement/mod callbacks.
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
        }
        Ok(())
    }
}
