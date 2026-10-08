//! Host script runtime loop and script-camera behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    pub(in crate::game_logic) fn evaluate_and_execute_scripts(&mut self, dt: f32) {
        if !self.scripts_loaded {
            return;
        }

        // Host script path: named-unit/team/area queries hit HOST objects.
        // Crate evaluator sees the name→id map + query snapshot (no crate Objects).
        self.inject_host_named_unit_map_into_crate_tracker();
        self.inject_host_supply_source_queries();

        self.update_script_camera(dt * self.visual_speed_multiplier.max(0.0));

        // Increment script frame counter
        self.mission_script_counter += 1;

        for event in script_events::drain_events() {
            match event {
                ScriptEvent::PlayerDefeated { player_id } => {
                    log::debug!(
                        "📜 Script event: player {} defeated (frame {})",
                        player_id,
                        self.frame
                    );
                    self.partition_manager.reveal_map_for_player_permanently(
                        &mut self
                            .engine_stores
                            .shroud()
                            .lock()
                            .unwrap_or_else(|e| e.into_inner()),
                        player_id,
                    );
                }
                ScriptEvent::RevealMapForPlayer { player_id } => {
                    log::debug!("📜 Script event: reveal map for player {}", player_id);
                    self.partition_manager.reveal_map_for_player(
                        &mut self
                            .engine_stores
                            .shroud()
                            .lock()
                            .unwrap_or_else(|e| e.into_inner()),
                        player_id,
                    );
                }
                ScriptEvent::CompletedSpecialPower {
                    player_id,
                    ref special_power_name,
                    creator_id,
                } => {
                    log::debug!(
                        "📜 Script event: completed special power {} player {} creator {}",
                        special_power_name,
                        player_id,
                        creator_id
                    );
                    let _ = gamelogic::scripting::engine::with_script_engine_mut(|engine| {
                        engine.notify_of_completed_special_power(
                            player_id as usize,
                            special_power_name,
                            creator_id,
                        );
                    });
                }

                ScriptEvent::AllianceStateChanged { player_id, state } => {
                    log::debug!(
                        "📜 Script event: alliance state {:?} for player {}",
                        state,
                        player_id
                    );
                }
            }
        }

        // Leftover ScriptingEngine event queue / process_events is leftover-only
        // (hq-8ta4n). Live host conditions/actions walk ScriptEngine::update.
        // C++ GameLogic.cpp:3600 — one TheScriptEngine->UPDATE() per logic frame.
        // Take the engine out of the global RwLock for the duration of update().
        // std::sync::RwLock is not re-entrant: holding write() across update()
        // deadlocks when MUSIC_SET_TRACK / MOVE_CAMERA_TO call
        // get_script_engine().read() (hang after "named cache populated").
        let taken = match gamelogic::scripting::engine::get_script_engine().write() {
            Ok(mut guard) => guard.take(),
            Err(_) => {
                log::error!("ScriptEngine::update failed: lock poisoned");
                None
            }
        };
        if let Some(engine) = taken {
            let context = gamelogic::scripting::executor::ScriptContext {
                game_logic_id: 0,
                object_manager_id: 0,
                player_manager_id: 0,
                event_system_id: 0,
                camera_system_id: 0,
                audio_system_id: 0,
                partition_manager_id: 0,
                special_powers_id: 0,
                current_frame: self.frame,
                suppress_new_windows: false,
                host_trigger_world: Arc::clone(&self.host_trigger_world),
            };
            let mut driver = script_execution_driver::HostScriptExecutionDriver::new(self);
            if let Err(err) = engine.update_with_driver(context, &mut driver) {
                log::error!("ScriptEngine::update failed: {err}");
            }
            if let Ok(mut guard) = gamelogic::scripting::engine::get_script_engine().write() {
                *guard = Some(engine);
            }
        }
        // Flush requests queued outside the action walk as well.
        self.apply_script_action_requests();

        self.script_broadcasts
            .retain(|msg| self.sim_time_seconds <= msg.expires_at);

        if self
            .cinematic_text
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.cinematic_text = None;
            self.cinematic_font = None;
        }

        if self
            .military_caption
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.military_caption = None;
        }
    }

    pub(in crate::game_logic::game_logic) fn start_camera_path_move(
        &mut self,
        request: CameraPathRequest,
    ) {
        self.script_camera_move_to = None;
        // C++ setupWaypointPath: m_doingRotateCamera = false.
        self.pending_camera_rotate = None;
        self.script_camera_rotate_remaining = 0.0;
        if let Some(move_state) =
            ScriptCameraPathMove::new(self.script_camera_focus_estimate, &request)
        {
            let mut move_state = move_state;
            if self.script_camera_freeze_time_armed {
                move_state.set_freeze_time(true);
                self.script_camera_freeze_time_armed = false;
            }
            if self.script_camera_freeze_angle_armed {
                move_state.set_freeze_angle(true);
                self.script_camera_freeze_angle_armed = false;
            }
            if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
                move_state.set_final_speed_multiplier(multiplier);
            }
            self.mission_scripts.set_camera_movement_finished(false);
            self.script_camera_path = Some(move_state);
        } else {
            self.script_camera_path = None;
            self.mark_script_camera_movement_maybe_finished();
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Camera path '{}' not found", request.waypoint),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }
    }

    pub(in crate::game_logic::game_logic) fn start_camera_move_to(
        &mut self,
        request: CameraMoveToRequest,
    ) {
        self.mission_scripts.set_camera_movement_finished(false);
        self.script_camera_path = None;
        // C++ setupWaypointPath: m_doingRotateCamera = false. RESET_CAMERA
        // and MOVE_CAMERA_TO must not leave a stale ROTATE_CAMERA ticking.
        self.pending_camera_rotate = None;
        self.script_camera_rotate_remaining = 0.0;
        let mut move_state = ScriptCameraMoveTo::new(self.script_camera_focus_estimate, &request);
        if self.script_camera_freeze_time_armed {
            move_state.set_freeze_time(true);
            self.script_camera_freeze_time_armed = false;
        }
        if self.script_camera_freeze_angle_armed {
            move_state.set_freeze_angle(true);
            self.script_camera_freeze_angle_armed = false;
        }
        if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
            move_state.set_final_speed_multiplier(multiplier);
        }
        self.script_camera_move_to = Some(move_state);
    }

    #[cfg(test)]
    pub fn script_camera_path_active(&self) -> bool {
        self.script_camera_path.is_some()
    }

    #[cfg(test)]
    pub fn install_script_camera_path_for_test(&mut self) {
        self.script_camera_path = Some(ScriptCameraPathMove::from_points_for_test(
            vec![
                Vec3::new(-10.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(100.0, 0.0, 0.0),
                Vec3::new(200.0, 0.0, 0.0),
                Vec3::new(300.0, 0.0, 0.0),
            ],
            4.0,
        ));
    }

    #[cfg(test)]
    pub fn script_camera_path_rolling_average_frames(&self) -> Option<i32> {
        self.script_camera_path
            .as_ref()
            .map(|path| path.rolling_average_frames)
    }

    #[cfg(test)]
    pub fn script_camera_move_to_target(&self) -> Option<Vec3> {
        self.script_camera_move_to.as_ref().map(|m| m.final_focus())
    }

    pub(super) fn script_camera_orientation_duration(seconds: f32) -> f32 {
        if seconds > 0.0 { seconds } else { 1.0 / 30.0 }
    }

    pub(in crate::game_logic::game_logic) fn is_script_camera_movement_finished_now(&self) -> bool {
        self.script_camera_move_to.is_none()
            && self.script_camera_path.is_none()
            && !self.script_camera_has_orientation_motion()
    }

    pub(super) fn script_camera_has_orientation_motion(&self) -> bool {
        self.script_camera_rotate_remaining > 0.0
            || self.script_camera_zoom_remaining > 0.0
            || self.script_camera_pitch_remaining > 0.0
    }

    pub(in crate::game_logic::game_logic::world_scripts) fn clear_script_camera_orientation_remaining(
        &mut self,
    ) {
        self.script_camera_rotate_remaining = 0.0;
        self.script_camera_zoom_remaining = 0.0;
        self.script_camera_pitch_remaining = 0.0;
        self.script_camera_freeze_time = false;
    }

    pub(super) fn begin_script_camera_rotate(&mut self, duration_seconds: f32) {
        self.script_camera_rotate_remaining =
            Self::script_camera_orientation_duration(duration_seconds);
        self.mission_scripts.set_camera_movement_finished(false);
    }

    pub(super) fn begin_script_camera_zoom(&mut self, duration_seconds: f32) {
        self.script_camera_zoom_remaining =
            Self::script_camera_orientation_duration(duration_seconds);
        self.mission_scripts.set_camera_movement_finished(false);
    }

    pub(super) fn begin_script_camera_pitch(&mut self, duration_seconds: f32) {
        self.script_camera_pitch_remaining =
            Self::script_camera_orientation_duration(duration_seconds);
        self.mission_scripts.set_camera_movement_finished(false);
    }

    pub(super) fn mark_script_camera_movement_maybe_finished(&mut self) {
        if self.is_script_camera_movement_finished_now() {
            self.mission_scripts.set_camera_movement_finished(true);
            self.script_camera_freeze_time = false;
            self.script_camera_freeze_time_armed = false;
        } else {
            self.mission_scripts.set_camera_movement_finished(false);
        }
    }

    pub(super) fn tick_script_camera_orientation(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }
        let had = self.script_camera_has_orientation_motion();
        if self.script_camera_rotate_remaining > 0.0 {
            self.script_camera_rotate_remaining =
                (self.script_camera_rotate_remaining - dt).max(0.0);
        }
        if self.script_camera_zoom_remaining > 0.0 {
            self.script_camera_zoom_remaining = (self.script_camera_zoom_remaining - dt).max(0.0);
        }
        if self.script_camera_pitch_remaining > 0.0 {
            self.script_camera_pitch_remaining = (self.script_camera_pitch_remaining - dt).max(0.0);
        }
        if had && !self.script_camera_has_orientation_motion() {
            self.mark_script_camera_movement_maybe_finished();
        }
    }

    pub(in crate::game_logic::game_logic) fn script_camera_remaining_seconds(&self) -> f32 {
        // C++ cameraModFinalZoom/Pitch: remaining rotate frames first, then path/move.
        if self.script_camera_rotate_remaining > 0.0 {
            return self.script_camera_rotate_remaining;
        }
        if let Some(rotate) = self.pending_camera_rotate.as_ref() {
            if rotate.duration_seconds > 0.0 {
                return rotate.duration_seconds;
            }
        }
        if let Some(move_to) = self.script_camera_move_to.as_ref() {
            return move_to.remaining_time_seconds();
        }
        if let Some(path) = self.script_camera_path.as_ref() {
            return path.remaining_time_seconds();
        }
        0.0
    }

    pub(in crate::game_logic::game_logic) fn is_script_camera_angle_frozen(&self) -> bool {
        self.script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_angle())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_angle())
                .unwrap_or(false)
    }

    /// C++ `W3DView::setDefaultView`: pitch + max-height scale; angle ignored.
    pub(in crate::game_logic::game_logic) fn apply_script_camera_default(
        &mut self,
        request: CameraSetDefaultRequest,
    ) {
        self.script_default_camera_pitch = request.pitch;
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = if request.max_height.is_finite() {
            request.max_height
        } else {
            1.0
        };
    }

    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_freeze_time(&mut self) {
        // C++ cameraModFreezeTime: m_freezeTimeForCameraMovement = true.
        self.script_camera_freeze_time = true;
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_time(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_freeze_time(true);
            applied = true;
        }
        if self.script_camera_has_orientation_motion() {
            applied = true;
        }
        if !applied {
            self.script_camera_freeze_time_armed = true;
        }
    }

    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_freeze_angle(&mut self) {
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.camera_mod_freeze_angle();
            });
        }
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_angle(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.freeze_angles_to_start();
            applied = true;
        }
        // Leftover freeze_current_angle: pin in-flight rotate start=end=current.
        if let Some(rotate) = self.pending_camera_rotate.as_mut() {
            rotate.rotations = 0.0;
            applied = true;
        } else if self.script_camera_rotate_remaining > 0.0 {
            self.pending_camera_rotate = Some(CameraRotateRequest {
                rotations: 0.0,
                duration_seconds: self.script_camera_rotate_remaining,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            applied = true;
        }
        if applied {
            // Pin the in-flight move/path. Do not leave a queued travel look.
            self.pending_camera_look_toward = None;
        }
    }

    /// C++ `cameraModLookToward` / `cameraModFinalLookToward`: rewrite the
    /// active waypoint-path (or simple moveCameraTo) look. No-op if idle.
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_look_toward(
        &mut self,
        position: Vec3,
        final_look: bool,
    ) {
        // C++ `cameraModLookToward` / `cameraModFinalLookToward` no-op while rotating.
        if self.pending_camera_rotate.is_some() {
            return;
        }
        let mut applied = false;
        let mut path_final = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_look_toward(position);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            if final_look {
                path.camera_mod_final_look_toward(position);
                path_final = true;
            } else {
                path.camera_mod_look_toward(position);
            }
            applied = true;
        }
        if !applied {
            return;
        }
        self.pending_camera_rotate = None;
        if path_final {
            // Last-segment swing is applied as the path advances. Do not retarget
            // the whole remaining duration (C++ only rewrites last 1-2 waypoints).
            return;
        }
        let remaining = self.script_camera_remaining_seconds();
        self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
            position,
            duration_seconds: remaining,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
            reverse_rotation: false,
        });
    }

    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_final_speed_multiplier(
        &mut self,
        request: &CameraModFinalSpeedMultiplierRequest,
    ) {
        let multiplier = request.multiplier as f32;
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if !applied {
            self.script_camera_pending_final_speed_multiplier = Some(multiplier.max(0.0));
        }
    }

    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_rolling_average(
        &mut self,
        request: &CameraModRollingAverageRequest,
    ) {
        // C++ cameraModRollingAverage writes m_mcwpInfo, but setupWaypointPath
        // hard-resets rollingAverageFrames=1. Leftover View applies only to an
        // in-flight camera_path and drops idle requests. Do not arm the next path.
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_rolling_average_frames(request.frames.max(1));
        }
    }

    pub(in crate::game_logic::game_logic) fn apply_visual_speed_multiplier(
        &mut self,
        request: &VisualSpeedMultiplierRequest,
    ) {
        let multiplier = request.multiplier.max(1) as f32;
        if multiplier.is_finite() {
            self.visual_speed_multiplier = multiplier;
        }
    }

    pub(in crate::game_logic::game_logic) fn apply_set_fps_limit(
        &mut self,
        request: &SetFpsLimitRequest,
    ) {
        self.pending_script_fps_limit = Some(request.fps);
    }
    pub(in crate::game_logic::game_logic) fn update_script_camera(&mut self, dt: f32) {
        self.tick_script_camera_orientation(dt);
        if let Some(object_id) = self.script_look_toward_object_id {
            if let Some(obj) = self.objects.get(&ObjectId(object_id)) {
                if let Some(look) = self.pending_camera_look_toward.as_mut() {
                    look.position = obj.get_position();
                    if look.duration_seconds > 0.0 {
                        look.duration_seconds = (look.duration_seconds - dt).max(0.0);
                    } else if self.script_look_toward_hold_seconds > 0.0 {
                        self.script_look_toward_hold_seconds =
                            (self.script_look_toward_hold_seconds - dt).max(0.0);
                    } else {
                        self.script_look_toward_object_id = None;
                    }
                }
            } else {
                self.script_look_toward_object_id = None;
            }
        }

        let move_step = self.script_camera_move_to.as_mut().map(|move_to| {
            if move_to.is_finished() {
                (true, move_to.final_focus(), false, None, 0.0)
            } else if let Some(focus) = move_to.advance(dt) {
                let look = if let Some(look) = move_to.look_toward() {
                    Some(look)
                } else if move_to.freeze_angle() || move_to.suppress_travel_look() {
                    None
                } else {
                    let dir = move_to.target - move_to.start;
                    Some(Vec3::new(focus.x + dir.x, focus.y, focus.z + dir.z))
                };
                (
                    false,
                    focus,
                    move_to.freeze_angle(),
                    look,
                    move_to.remaining_time_seconds(),
                )
            } else {
                (false, Vec3::ZERO, true, None, 0.0)
            }
        });
        if let Some((finished, focus, _freeze_angle, look, remaining)) = move_step {
            self.mission_scripts.set_camera_movement_finished(false);
            if finished {
                self.request_camera_focus(focus);
                self.script_camera_move_to = None;
                self.mark_script_camera_movement_maybe_finished();
                return;
            }
            if focus != Vec3::ZERO || look.is_some() {
                self.request_camera_focus(focus);
                if let Some(look) = look {
                    self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                        position: look,
                        duration_seconds: remaining,
                        ease_in_seconds: 0.0,
                        ease_out_seconds: 0.0,
                        reverse_rotation: false,
                    });
                }
            }
            return;
        }

        let path_step = self.script_camera_path.as_mut().map(|path_move| {
            if path_move.is_finished() {
                (true, path_move.final_focus(), None, 0.0)
            } else if let Some(focus) = path_move.advance(dt) {
                let look = if let Some(look) = path_move.frozen_start_look_toward(focus) {
                    Some(look)
                } else if let Some(look) = path_move.look_toward_for_current_segment() {
                    Some(look)
                } else if path_move.freeze_angle() || path_move.suppress_travel_look() {
                    None
                } else {
                    path_move.travel_look_toward()
                };
                (
                    false,
                    focus,
                    look,
                    path_move.remaining_time_seconds().max(0.05),
                )
            } else {
                (false, Vec3::ZERO, None, 0.0)
            }
        });
        let Some((finished, focus, look, remaining)) = path_step else {
            if !self.is_script_camera_movement_finished_now() {
                self.mission_scripts.set_camera_movement_finished(false);
            }
            return;
        };
        self.mission_scripts.set_camera_movement_finished(false);
        if finished {
            self.request_camera_focus(focus);
            self.script_camera_path = None;
            self.mark_script_camera_movement_maybe_finished();
            return;
        }
        if focus != Vec3::ZERO || look.is_some() {
            self.request_camera_focus(focus);
            if let Some(look) = look {
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: look,
                    duration_seconds: remaining,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            }
        }
    }

    pub(in crate::game_logic::game_logic) fn military_caption_duration_seconds(
        duration_ms: i32,
    ) -> f32 {
        (duration_ms as f32 / 1000.0).max(0.0)
    }
}
