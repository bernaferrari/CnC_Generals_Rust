//! Additional camera actions including blur, tether, fade, letterbox, and shake
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;
use crate::scripting::engine::{ScriptCameraRequest, ScriptExecutionDriver};

impl ScriptActionDispatcher<'_> {
    pub(super) fn dispatch_camera_request(
        &mut self,
        request: ScriptCameraRequest<'_>,
        driver: &mut dyn ScriptExecutionDriver,
    ) {
        let result = driver.camera(request).or_else(|| {
            self.current_script_action_handler()
                .map(|handler| request.dispatch_to(handler.as_ref()))
        });
        if let Some(Err(err)) = result {
            log::warn!("Script camera action {:?} failed: {}", request, err);
        }
        // Retain legacy callback-error continuation, including after_action.
    }

    // ============================================================================
    // ADDITIONAL CAMERA ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_move_camera_along_waypoint_path(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_path = self.get_string_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let camera_stutter_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(4).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Moving camera along waypoint path '{}' (sec: {}, stutter: {}, ease_in: {}, ease_out: {})",
            waypoint_path,
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::WaypointPath {
                waypoint_path: &waypoint_path,
                seconds,
                camera_stutter_seconds,
                ease_in_seconds,
                ease_out_seconds,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_rotate_camera(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        // C++: doRotateCamera(rotations, sec, easeIn, easeOut)
        let rotations = self.get_real_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);

        log::debug!(
            "Rotating camera by {} turns (sec: {}, ease_in: {}, ease_out: {})",
            rotations,
            seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::Rotate {
                rotations,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_move_camera_to_selection(
        &mut self,
        _action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Retargeting active camera movement to selection");

        // Prefer host-integrated selection center (Main runtime queue) when available.
        if let Some(result) = driver.camera(ScriptCameraRequest::MoveToSelection) {
            if let Err(err) = result {
                log::warn!(
                    "Script action handler move_camera_to_selection failed: {}",
                    err
                );
            }
            return Ok(ScriptActionResult::Success);
        }

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.move_camera_to_selection() {
                log::warn!(
                    "Script action handler move_camera_to_selection failed: {}",
                    err
                );
            }
            return Ok(ScriptActionResult::Success);
        }

        let local_player_id = player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(-1);
        if local_player_id < 0 {
            return Ok(ScriptActionResult::Success);
        }

        let selection_manager = crate::commands::get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return Ok(ScriptActionResult::Success);
        };

        let Some(selection) = manager.get_player_selection_ref(local_player_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let selected = selection.get_selected_objects_info();
        if selected.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let selected_len = selected.len();
        let mut sum = crate::common::Coord3D::new(0.0, 0.0, 0.0);
        for entry in &selected {
            sum += entry.position;
        }
        let center = sum / (selected_len as f32);

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.move_camera_to_selection() {
                log::warn!(
                    "Script action handler move_camera_to_selection failed: {}",
                    err
                );
            }
            return Ok(ScriptActionResult::Success);
        }

        log::info!("Script move_camera_to_selection center {:?}", center);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_move_home(
        &mut self,
        _action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Moving camera home");

        self.dispatch_camera_request(ScriptCameraRequest::MoveHome, driver);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_setup_camera(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let zoom = self.get_real_param(action, 1)?;
        let pitch = self.get_real_param(action, 2)?;
        let look_at_waypoint = self.get_string_param(action, 3)?;

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let look_at_ascii = AsciiString::from(look_at_waypoint.as_str());

        let terrain_owner_handle = get_terrain_logic();

        let Some((position, look_at)) = terrain_owner_handle.read().ok().and_then(|terrain| {
            let position = terrain.get_waypoint_by_name(&waypoint_ascii)?;
            let look_at = terrain.get_waypoint_by_name(&look_at_ascii)?;
            Some((*position.get_location(), *look_at.get_location()))
        }) else {
            log::warn!(
                "Setup camera waypoint(s) not found: '{}' / '{}'",
                waypoint,
                look_at_waypoint
            );
            return Ok(ScriptActionResult::Success);
        };

        log::debug!(
            "Setting up camera at '{}' (zoom: {}, pitch: {}, look_at: '{}')",
            waypoint,
            zoom,
            pitch,
            look_at_waypoint
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::Setup {
                x: position.x,
                y: position.y,
                z: position.z,
                zoom,
                pitch,
                look_toward_x: look_at.x,
                look_toward_y: look_at.y,
                look_toward_z: look_at.z,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_letterbox_begin(
        &mut self,
        _action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Beginning camera letterbox");
        self.dispatch_camera_request(ScriptCameraRequest::LetterboxBegin, driver);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_letterbox_end(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Ending camera letterbox");
        self.dispatch_camera_request(ScriptCameraRequest::LetterboxEnd, driver);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_zoom_camera(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let zoom = self.get_real_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Zooming camera to {} (sec: {}, ease_in: {}, ease_out: {})",
            zoom,
            seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::Zoom {
                zoom,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_pitch_camera(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let pitch = self.get_real_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);

        log::debug!(
            "Pitching camera to {} (sec: {}, ease_in: {}, ease_out: {})",
            pitch,
            seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::Pitch {
                pitch,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_oversize_terrain(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let amount = self.get_int_param(action, 0)?;
        log::debug!("Setting terrain oversize to {}", amount);

        if let Some(handler) = self.current_script_action_handler() {
            if let Err(err) = handler.oversize_terrain(amount) {
                log::warn!("Script action handler oversize_terrain failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_fade_add(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        let _ = self.context.with_engine_mut(|script_engine| {
            script_engine.set_fade_parameters(
                TFade::Add,
                min_fade,
                max_fade,
                frames_increase,
                frames_hold,
                frames_decrease,
            );
        });

        log::debug!(
            "Camera fade add from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_fade_subtract(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        let _ = self.context.with_engine_mut(|script_engine| {
            script_engine.set_fade_parameters(
                TFade::Subtract,
                min_fade,
                max_fade,
                frames_increase,
                frames_hold,
                frames_decrease,
            );
        });

        log::debug!(
            "Camera fade subtract from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_fade_saturate(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        let _ = self.context.with_engine_mut(|script_engine| {
            script_engine.set_fade_parameters(
                TFade::Saturate,
                min_fade,
                max_fade,
                frames_increase,
                frames_hold,
                frames_decrease,
            );
        });

        log::debug!(
            "Camera fade saturate from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_fade_multiply(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        let _ = self.context.with_engine_mut(|script_engine| {
            script_engine.set_fade_parameters(
                TFade::Multiply,
                min_fade,
                max_fade,
                frames_increase,
                frames_hold,
                frames_decrease,
            );
        });

        log::debug!(
            "Camera fade multiply from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_bw_mode_begin(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let frames = action.get_parameter(0).map(|p| p.get_int()).unwrap_or(0);
        log::debug!("Beginning camera B&W mode over {} frames", frames);

        self.dispatch_camera_request(
            ScriptCameraRequest::BwMode {
                enabled: true,
                frames,
            },
            driver,
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_bw_mode_end(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let frames = action.get_parameter(0).map(|p| p.get_int()).unwrap_or(0);
        log::debug!("Ending camera B&W mode over {} frames", frames);

        self.dispatch_camera_request(
            ScriptCameraRequest::BwMode {
                enabled: false,
                frames,
            },
            driver,
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_draw_skybox_begin(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Beginning skybox draw");

        self.dispatch_camera_request(ScriptCameraRequest::SkyboxEnabled { enabled: true }, driver);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_draw_skybox_end(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Ending skybox draw");

        self.dispatch_camera_request(
            ScriptCameraRequest::SkyboxEnabled { enabled: false },
            driver,
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_motion_blur(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let zoom_in = self.get_bool_param_optional(action, 0).unwrap_or(false);
        let saturate = self.get_bool_param_optional(action, 1).unwrap_or(false);
        log::debug!(
            "Camera motion blur (zoom_in: {}, saturate: {})",
            zoom_in,
            saturate
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::MotionBlur { zoom_in, saturate },
            driver,
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_motion_blur_jump(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let saturate = self.get_bool_param_optional(action, 1).unwrap_or(false);
        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let terrain_owner_handle = get_terrain_logic();
        let target = terrain_owner_handle.read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|waypoint| *waypoint.get_location())
        });

        let Some(target) = target else {
            log::warn!(
                "Camera motion blur jump failed: waypoint '{}' not found",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        log::debug!(
            "Camera motion blur jump to '{}' (saturate: {})",
            waypoint_name,
            saturate
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::MotionBlurJump {
                x: target.x,
                y: target.y,
                z: target.z,
                saturate,
            },
            driver,
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_motion_blur_follow(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let amount = self.get_int_param(action, 0)?;
        log::debug!("Camera motion blur follow amount {}", amount);

        self.dispatch_camera_request(ScriptCameraRequest::MotionBlurFollow { amount }, driver);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_motion_blur_end_follow(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Ending camera motion blur follow");

        self.dispatch_camera_request(ScriptCameraRequest::MotionBlurEndFollow, driver);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_set_audible_distance(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let distance = self.get_real_param(action, 0)?;
        log::debug!("Setting camera audible distance to {}", distance);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_tether_named(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let snap_to_unit = self.get_bool_param_optional(action, 1).unwrap_or(false);
        let play = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);

        log::debug!(
            "Camera tethering to '{}' (snap: {}, play: {})",
            unit_name,
            snap_to_unit,
            play
        );

        // C++ `TheScriptEngine->getUnitNamed` — host injects name→id into the
        // crate tracker. Empty OBJECT_REGISTRY must not skip named lookup.
        let tracker = get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&unit_name).ok().flatten();
        // Wave 284: empty dual-world → skip crate Object walk only.
        if object_id.is_none() && !dual_world_registry_unavailable() {
            let lower = unit_name.to_ascii_lowercase();
            object_id = OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find_map(|obj_ref| {
                    obj_ref.read().ok().and_then(|obj| {
                        if obj.get_name().to_ascii_lowercase() == lower {
                            Some(obj.get_id())
                        } else {
                            None
                        }
                    })
                });
        }

        let Some(object_id) = object_id else {
            log::warn!("Camera tether failed: unit '{}' not found", unit_name);
            return Ok(ScriptActionResult::Success);
        };

        self.dispatch_camera_request(
            ScriptCameraRequest::TetherObject {
                object_id,
                snap_to_unit,
                play,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_stop_tether_named(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Stopping camera tether");

        self.dispatch_camera_request(ScriptCameraRequest::StopFollow, driver);

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_set_default(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let pitch = self.get_real_param(action, 0)?;
        let angle = self.get_real_param(action, 1)?;
        let max_height = self.get_real_param(action, 2)?;

        log::debug!(
            "Setting camera default (pitch: {}, angle: {}, max_height: {})",
            pitch,
            angle,
            max_height
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::SetDefault {
                pitch,
                angle,
                max_height,
            },
            driver,
        );

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_look_toward_object(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let hold_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(4).map(|p| p.get_real()).unwrap_or(0.0);

        // C++ `TheScriptEngine->getUnitNamed` — host injects name→id into the
        // crate tracker. Empty OBJECT_REGISTRY must not skip named lookup.
        let tracker = get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&object_name).ok().flatten();
        // Wave 284: empty dual-world → skip crate Object walk only.
        if object_id.is_none() && !dual_world_registry_unavailable() {
            let lower = object_name.to_ascii_lowercase();
            object_id = OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find_map(|obj_ref| {
                    obj_ref.read().ok().and_then(|obj| {
                        if obj.get_name().to_ascii_lowercase() == lower {
                            Some(obj.get_id())
                        } else {
                            None
                        }
                    })
                });
        }

        log::debug!(
            "Camera looking toward '{}' (sec: {}, hold: {}, ease_in: {}, ease_out: {})",
            object_name,
            seconds,
            hold_seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Some(object_id) = object_id {
            self.dispatch_camera_request(
                ScriptCameraRequest::LookTowardObject {
                    object_id,
                    seconds,
                    hold_seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                },
                driver,
            );
        } else {
            log::warn!("Camera look toward object '{}' not found", object_name);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_look_toward_waypoint(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let reverse_rotation = self.get_bool_param_optional(action, 4).unwrap_or(false);

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let terrain_owner_handle = get_terrain_logic();
        let target = terrain_owner_handle.read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });

        log::debug!(
            "Camera looking toward waypoint '{}' (sec: {}, ease_in: {}, ease_out: {}, reverse: {})",
            waypoint,
            seconds,
            ease_in_seconds,
            ease_out_seconds,
            reverse_rotation
        );

        if let Some(target) = target {
            self.dispatch_camera_request(
                ScriptCameraRequest::LookTowardWaypoint {
                    x: target.x,
                    y: target.y,
                    z: target.z,
                    seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                    reverse_rotation,
                },
                driver,
            );
        } else {
            log::warn!("Camera look toward waypoint '{}' not found", waypoint);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_freeze_time(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Camera mod freeze time");
        self.dispatch_camera_request(ScriptCameraRequest::FreezeTime, driver);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_set_final_zoom(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let zoom = self.get_real_param(action, 0)?;
        let ease_in = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Camera mod set final zoom to {} (ease_in: {}, ease_out: {})",
            zoom,
            ease_in,
            ease_out
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::FinalZoom {
                zoom,
                ease_in,
                ease_out,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_set_final_pitch(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let pitch = self.get_real_param(action, 0)?;
        let ease_in = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Camera mod set final pitch to {} (ease_in: {}, ease_out: {})",
            pitch,
            ease_in,
            ease_out
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::FinalPitch {
                pitch,
                ease_in,
                ease_out,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_freeze_angle(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Camera mod freeze angle");
        self.dispatch_camera_request(ScriptCameraRequest::FreezeAngle, driver);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_set_final_speed_multiplier(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let multiplier = self.get_int_param(action, 0)?;
        log::debug!("Camera mod set final speed multiplier to {}", multiplier);
        self.dispatch_camera_request(
            ScriptCameraRequest::FinalSpeedMultiplier { multiplier },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_set_rolling_average(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let frames = self.get_int_param(action, 0)?;
        log::debug!("Camera mod set rolling average to {} frames", frames);
        self.dispatch_camera_request(ScriptCameraRequest::RollingAverage { frames }, driver);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_final_look_toward(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let terrain_owner_handle = get_terrain_logic();
        let target = terrain_owner_handle.read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });
        log::debug!("Camera mod final look toward '{}'", waypoint);

        if let Some(target) = target {
            self.dispatch_camera_request(
                ScriptCameraRequest::FinalLookToward {
                    x: target.x,
                    y: target.y,
                    z: target.z,
                },
                driver,
            );
        } else {
            log::warn!(
                "Camera mod final look toward waypoint '{}' not found",
                waypoint
            );
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_mod_look_toward(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let terrain_owner_handle = get_terrain_logic();
        let target = terrain_owner_handle.read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });
        log::debug!("Camera mod look toward '{}'", waypoint);

        if let Some(target) = target {
            self.dispatch_camera_request(
                ScriptCameraRequest::LookToward {
                    x: target.x,
                    y: target.y,
                    z: target.z,
                },
                driver,
            );
        } else {
            log::warn!("Camera mod look toward waypoint '{}' not found", waypoint);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_enable_slave_mode(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let thing_template_name = self.get_string_param(action, 0)?;
        let bone_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Enabling camera slave mode (template: '{}', bone: '{}')",
            thing_template_name,
            bone_name
        );

        self.dispatch_camera_request(
            ScriptCameraRequest::EnableSlaveMode {
                thing_template_name: &thing_template_name,
                bone_name: &bone_name,
            },
            driver,
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_disable_slave_mode(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling camera slave mode");

        self.dispatch_camera_request(ScriptCameraRequest::DisableSlaveMode, driver);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_camera_add_shaker_at(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let amplitude = self.get_real_param(action, 1)?;
        let duration_seconds = self.get_real_param(action, 2)?;
        let radius = self.get_real_param(action, 3)?;

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let terrain_owner_handle = get_terrain_logic();
        let target = terrain_owner_handle.read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });

        log::debug!(
            "Adding camera shaker at '{}' (amplitude: {}, duration: {}, radius: {})",
            waypoint,
            amplitude,
            duration_seconds,
            radius
        );

        if let Some(target) = target {
            self.dispatch_camera_request(
                ScriptCameraRequest::AddShakerAt {
                    x: target.x,
                    y: target.y,
                    z: target.z,
                    amplitude,
                    duration_seconds,
                    radius,
                },
                driver,
            );
        } else {
            log::warn!("Camera shaker waypoint '{}' not found", waypoint);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_screen_shake(
        &mut self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        let intensity = self.get_int_param(action, 0)?;
        log::debug!("Screen shake intensity {}", intensity);

        self.dispatch_camera_request(ScriptCameraRequest::ScreenShake { intensity }, driver);
        Ok(ScriptActionResult::Success)
    }
}
