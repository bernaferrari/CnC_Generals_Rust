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
                    self.partition_manager
                        .reveal_map_for_player_permanently(player_id);
                }
                ScriptEvent::RevealMapForPlayer { player_id } => {
                    log::debug!("📜 Script event: reveal map for player {}", player_id);
                    self.partition_manager.reveal_map_for_player(player_id);
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
            if let Err(err) = engine.update() {
                log::error!("ScriptEngine::update failed: {err}");
            }
            if let Ok(mut guard) = gamelogic::scripting::engine::get_script_engine().write() {
                *guard = Some(engine);
            }
        }
        self.apply_host_skirmish_script_requests();
        self.apply_host_set_base_construction_speed_requests();
        self.apply_host_set_train_held_requests();
        self.apply_host_money_script_requests();
        self.apply_host_can_build_script_requests();
        self.apply_host_buildable_override_script_requests();
        self.apply_host_rank_script_requests();
        self.apply_host_transfer_script_requests();
        self.apply_host_player_relates_script_requests();
        self.apply_host_team_override_relation_script_requests();

        self.apply_host_loco_set_script_requests();
        self.apply_host_face_script_requests();

        self.apply_host_move_attack_script_requests();
        self.apply_host_hunt_guard_script_requests();
        self.apply_host_garrison_enter_exit_script_requests();
        self.apply_host_idle_script_requests();
        self.apply_host_kill_delete_damage_script_requests();

        self.apply_host_follow_waypoints_script_requests();
        self.apply_host_skirmish_approach_path_script_requests();

        self.apply_host_create_script_requests();
        self.apply_host_boobytrap_script_requests();
        self.apply_host_unmanned_script_requests();
        self.apply_host_radar_event_script_requests();
        self.apply_host_stealth_enabled_script_requests();
        self.apply_host_team_attitude_script_requests();
        self.apply_host_script_visual_status_requests();
        self.apply_host_guard_supply_center_script_requests();
        self.apply_host_guard_variant_script_requests();
        self.apply_host_named_fire_special_script_requests();
        self.apply_host_use_command_button_script_requests();
        self.apply_host_object_sound_script_requests();

        self.apply_host_skirmish_fight_script_requests();

        self.mission_scripts.note_logic_frame(self.frame as u64);

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

        for msg in self.mission_scripts.drain_messages() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: msg.clone(),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
            self.new_script_messages.push(msg);
        }

        for sound in self.mission_scripts.drain_sounds() {
            self.play_ui_sound(&sound);
        }

        for sound in self.mission_scripts.drain_sound_events() {
            let translated = translate_audio_event(&sound.sound_name);
            let mut event = AudioEventRequest::new(translated);
            if let Some(pos) = sound.position {
                event = event.with_position(pos);
            }
            self.queue_audio_event(event);
        }

        for camera_target in self.mission_scripts.drain_camera_moves() {
            self.request_camera_focus(camera_target);
        }

        if !self
            .mission_scripts
            .drain_camera_move_to_selection_requests()
            .is_empty()
        {
            // C++ doModCameraMoveToSelection → cameraModFinalMoveTo: path modifier,
            // not a new lookAt. No-op during rotate; no-op if no path/move.
            if self.pending_camera_rotate.is_none() {
                if let Some(center) = self.selected_objects_center_for_local_player() {
                    if let Some(path) = self.script_camera_path.as_mut() {
                        path.camera_mod_final_move_to(center);
                    }
                    if let Some(move_to) = self.script_camera_move_to.as_mut() {
                        move_to.camera_mod_final_move_to(center);
                    }
                    #[cfg(feature = "game_client")]
                    {
                        game_client::display::view::with_tactical_view(|view| {
                            view.camera_mod_final_move_to(
                                &game_client::display::view::Point3::new(
                                    center.x, center.z, center.y,
                                ),
                            );
                        });
                    }
                }
            }
        }

        if !self
            .mission_scripts
            .drain_camera_move_home_requests()
            .is_empty()
        {
            if let Some(home) = self.local_player_camera_home_position() {
                self.camera_follow_target = None;
                self.request_camera_focus(home);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_follows()
            .into_iter()
            .last()
        {
            if last.object_id == 0 {
                self.camera_follow_target = None;
                self.camera_tether_play = None;
            } else {
                self.script_camera_move_to = None;
                self.script_camera_path = None;
                self.camera_tether_play = None;
                self.camera_follow_target = Some(ObjectId(last.object_id));
                if last.snap_to_unit {
                    if let Some(obj) = self.objects.get(&ObjectId(last.object_id)) {
                        self.request_camera_focus(obj.get_position());
                    }
                }
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_tethers()
            .into_iter()
            .last()
        {
            self.script_camera_move_to = None;
            self.script_camera_path = None;
            self.set_camera_tether_object(ObjectId(last.object_id), last.snap_to_unit, last.play);
        }

        if !self
            .mission_scripts
            .drain_camera_mod_freeze_time_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_time();
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_final_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_rolling_average_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_rolling_average(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_visual_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_visual_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_script_freeze_time_requests()
            .into_iter()
            .last()
        {
            self.script_time_frozen_by_script = last;
        }

        if let Some(last) = self
            .mission_scripts
            .drain_set_fps_limit_requests()
            .into_iter()
            .last()
        {
            self.apply_set_fps_limit(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_move_to()
            .into_iter()
            .last()
        {
            self.start_camera_move_to(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_path_moves()
            .into_iter()
            .last()
        {
            self.start_camera_path_move(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_set_default_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_default(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_slave_mode_enable_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_slave_mode_enable = Some(last);
            self.pending_camera_slave_mode_disable = false;
        }

        if !self
            .mission_scripts
            .drain_camera_slave_mode_disable_requests()
            .is_empty()
        {
            self.pending_camera_slave_mode_enable = None;
            self.pending_camera_slave_mode_disable = true;
        }

        let screen_shakes = self.mission_scripts.drain_screen_shake_requests();
        if !screen_shakes.is_empty() {
            self.pending_screen_shakes.extend(screen_shakes);
        }

        let camera_shakers = self.mission_scripts.drain_camera_add_shaker_requests();
        if !camera_shakers.is_empty() {
            self.pending_camera_add_shakers.extend(camera_shakers);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_resets()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            // C++ setupWaypointPath ends with m_doingRotateCamera = false.
            // Leftover reset_camera replaces camera_rotate so a prior ROTATE
            // cannot keep peeking into presentation after the reset.
            self.pending_camera_rotate = None;
            self.script_camera_rotate_remaining = 0.0;
            self.pending_camera_zoom_reset = true;
            self.pending_camera_zoom_reset_duration = last.duration_seconds.max(0.0);
            self.pending_camera_zoom_reset_ease_in = last.ease_in_seconds.max(0.0);
            self.pending_camera_zoom_reset_ease_out = last.ease_out_seconds.max(0.0);
            let request = CameraMoveToRequest {
                position: last.position,
                seconds: last.duration_seconds,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: last.ease_in_seconds.max(0.0),
                ease_out_seconds: last.ease_out_seconds.max(0.0),
            };
            self.start_camera_move_to(request);
            if let Some(move_to) = self.script_camera_move_to.as_mut() {
                move_to.set_suppress_travel_look(true);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_zoom_requests()
            .into_iter()
            .last()
        {
            self.begin_script_camera_zoom(last.duration_seconds);
            self.pending_camera_zoom = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_pitch_requests()
            .into_iter()
            .last()
        {
            self.begin_script_camera_pitch(last.duration_seconds);
            self.pending_camera_pitch = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_rotate_requests()
            .into_iter()
            .last()
        {
            // C++ rotateCamera replaces any current animation. FREEZE_ANGLE only
            // pins the in-flight move/path and must not swallow later rotates.
            self.begin_script_camera_rotate(last.duration_seconds);
            self.pending_camera_rotate = Some(last);
        }

        // C++ mods apply to the in-flight animation. Drain MOVE/PATH/RESET/ROTATE
        // first so same-frame ROTATE_CAMERA + CAMERA_MOD_FREEZE_ANGLE pins yaw.
        if !self
            .mission_scripts
            .drain_camera_mod_freeze_angle_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_angle();
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_zoom_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.camera_mod_final_zoom(last.zoom, last.ease_in, last.ease_out);
                });
            }
            // Leftover/C++ cameraModFinalZoom: idle (no rotate/path/move) is a no-op.
            let remaining = self.script_camera_remaining_seconds();
            if remaining > 0.0 {
                let max_zoom = (320.0 + 300.0) / 320.0;
                self.begin_script_camera_zoom(remaining);
                self.pending_camera_zoom = Some(CameraZoomRequest {
                    zoom: last.zoom * max_zoom,
                    duration_seconds: remaining,
                    ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                    ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
                });
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_pitch_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.camera_mod_final_pitch(last.pitch, last.ease_in, last.ease_out);
                });
            }
            // Leftover/C++ cameraModFinalPitch: idle (no rotate/path/move) is a no-op.
            let remaining = self.script_camera_remaining_seconds();
            if remaining > 0.0 {
                self.begin_script_camera_pitch(remaining);
                self.pending_camera_pitch = Some(CameraPitchRequest {
                    pitch: last.pitch,
                    duration_seconds: remaining,
                    ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                    ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
                });
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_setup_requests()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            // C++ moveCameraTo → setupWaypointPath rebuilds m_mcwpInfo and
            // sets m_doingRotateCamera = false. Leftover setup_camera →
            // look_at cancels camera_move / camera_path / camera_rotate.
            self.script_camera_move_to = None;
            self.script_camera_path = None;
            self.script_look_toward_object_id = None;
            self.script_look_toward_hold_seconds = 0.0;
            self.script_camera_rotate_remaining = 0.0;
            self.request_camera_focus(last.position);
            let max_zoom = (320.0 + 300.0) / 320.0;
            self.pending_camera_zoom = Some(CameraZoomRequest {
                zoom: last.zoom * max_zoom,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            self.pending_camera_pitch = Some(CameraPitchRequest {
                pitch: last.pitch,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            self.pending_camera_rotate = None;
            self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                position: last.look_toward,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
                reverse_rotation: false,
            });
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_waypoint_requests()
            .into_iter()
            .last()
        {
            // C++ rotateCameraTowardPosition: m_doingMoveCameraOnWaypointPath = false.
            self.script_camera_move_to = None;
            self.script_camera_path = None;
            self.pending_camera_rotate = None;
            self.begin_script_camera_rotate(last.duration_seconds);
            self.pending_camera_look_toward = Some(last);
        }
        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_object_requests()
            .into_iter()
            .last()
        {
            if let Some(position) = self
                .objects
                .get(&ObjectId(last.object_id))
                .map(|obj| obj.get_position())
            {
                // C++ rotateCameraTowardObject: m_doingMoveCameraOnWaypointPath = false.
                self.script_camera_move_to = None;
                self.script_camera_path = None;
                self.pending_camera_rotate = None;
                self.begin_script_camera_rotate(last.duration_seconds + last.hold_seconds.max(0.0));
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position,
                    duration_seconds: last.duration_seconds,
                    ease_in_seconds: last.ease_in_seconds,
                    ease_out_seconds: last.ease_out_seconds,
                    reverse_rotation: false,
                });
                self.script_look_toward_object_id = Some(last.object_id);
                self.script_look_toward_hold_seconds = last.hold_seconds.max(0.0);
            } else {
                log::warn!(
                    "Camera look toward object request ignored; object {} not found",
                    last.object_id
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_look_toward_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_look_toward(last.position, false);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_look_toward_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_look_toward(last.position, true);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_letterbox_events()
            .last()
            .copied()
        {
            self.cinematic_letterbox = last;
            // C++ ScriptActions::doLetterBoxMode HideControlBar(TRUE)/ShowControlBar(FALSE).
            #[cfg(feature = "game_client")]
            {
                if last {
                    let _ =
                        game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(true);
                } else {
                    let _ =
                        game_client::gui::callbacks::control_bar_callbacks::show_control_bar(false);
                }
            }
        }

        if let Some((text, font, duration_seconds)) = self
            .mission_scripts
            .drain_cinematic_text()
            .into_iter()
            .last()
        {
            let duration = (duration_seconds as f32).max(0.0);
            self.cinematic_text = Some((text, self.sim_time_seconds + duration));
            self.cinematic_font = if font.is_empty() { None } else { Some(font) };
        }

        if let Some(last) = self
            .mission_scripts
            .drain_military_captions()
            .into_iter()
            .last()
        {
            let duration = Self::military_caption_duration_seconds(last.duration_ms);
            self.military_caption = Some((last.text, self.sim_time_seconds + duration));
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_movie = Some(movie.clone());
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Movie requested: {}", movie),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_radar_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_radar_movie = Some(movie);
        }

        let objective_updates = self.mission_scripts.drain_objective_updates();
        if !objective_updates.is_empty() {
            for update in objective_updates {
                let status = if update.completed {
                    ObjectiveStatus::Completed
                } else {
                    ObjectiveStatus::Active
                };

                let updated_existing = self.with_objective_mut(&update.name, |objective| {
                    objective.title = update.name.clone();
                    objective.description = update.description.clone();
                    objective.status = status;
                });

                if !updated_existing {
                    self.mission_objectives.push(ObjectiveDisplay::new(
                        Some(update.name.clone()),
                        update.name.clone(),
                        update.description.clone(),
                        ObjectiveCategory::Primary,
                    ));
                    let idx = self.mission_objectives.len().saturating_sub(1);
                    self.objective_lookup
                        .insert(update.name.to_ascii_lowercase(), idx);
                }
            }
        }

        for effect in self.mission_scripts.drain_effect_requests() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!(
                    "Effect '{}' at ({:.0}, {:.0}, {:.0})",
                    effect.effect_type, effect.position.x, effect.position.y, effect.position.z
                ),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        for radar_event in self.mission_scripts.drain_radar_event_requests() {
            self.queue_script_radar_event(radar_event);
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_radar_enabled_updates()
            .into_iter()
            .last()
        {
            self.radar_enabled = enabled;
        }

        if let Some(forced) = self
            .mission_scripts
            .drain_radar_forced_updates()
            .into_iter()
            .last()
        {
            self.radar_forced = forced;
        }

        if let Some(visible) = self
            .mission_scripts
            .drain_weather_visibility_updates()
            .into_iter()
            .last()
        {
            self.set_weather_visible(visible);
        }

        let popup_messages = self.mission_scripts.drain_popup_message_requests();
        if !popup_messages.is_empty() {
            // C++ InGameUI owns one popup layout: every new popup replaces the
            // previously visible one.  Keep only the newest presentation
            // residual; MissionScriptHooks itself remains the future-event
            // queue and is already drained above.
            let active_popup = popup_messages.last().cloned();
            #[cfg(feature = "game_client")]
            if let Some(popup) = active_popup.as_ref() {
                // C++ clears/replaces the single InGameUI popup layout. Send
                // only its newest request to GameClient and retain its opaque
                // identity so a delayed ButtonOk/Esc cannot dismiss a later
                // replacement popup in Main.
                game_client::core::script_action_handler::script_popup_message_with_host_generation(
                    &popup.message,
                    popup.x_percent,
                    popup.y_percent,
                    popup.width,
                    popup.pause,
                    popup.pause_music,
                    Some(popup.popup_generation),
                );
            }

            for popup in popup_messages {
                if popup.pause_music {
                    self.pending_music_stop = true;
                }
                self.script_broadcasts.push(ScriptBroadcast {
                    text: popup.message.clone(),
                    expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
                });
                self.new_script_messages.push(popup.message.clone());
            }

            self.pending_popup_messages.clear();
            if let Some(active_popup) = active_popup {
                self.pending_popup_messages.push(active_popup);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_view_guardband_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_resize_view_guardband(
                last.x_bias,
                last.y_bias,
            );
            self.pending_view_guardband = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_bw_mode_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_camera_bw_mode(
                last.enabled,
                last.frames,
            );
            self.pending_camera_bw_mode = Some(last);
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_skybox_enabled_updates()
            .into_iter()
            .last()
        {
            self.script_skybox_enabled = enabled;
            {
                let mut global = game_engine::common::global_data::write();
                global.draw_sky_box = enabled;
            }
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_skybox_enabled(enabled);
        }

        for request in self.mission_scripts.drain_camera_motion_blur_requests() {
            #[cfg(feature = "game_client")]
            match &request {
                CameraMotionBlurRequest::Basic { zoom_in, saturate } => {
                    game_client::core::script_action_handler::script_camera_motion_blur(
                        *zoom_in, *saturate,
                    );
                }
                CameraMotionBlurRequest::Jump { position, saturate } => {
                    // C++ doCameraMotionBlurJump: leftover set filter+pos only.
                    // lookAt / request_cam only if leftover filter failed.
                    let passed =
                        game_client::core::script_action_handler::script_camera_motion_blur_jump(
                            position.x, position.z, position.y, *saturate,
                        );
                    if !passed {
                        self.camera_follow_target = None;
                        self.request_camera_focus(*position);
                    }
                }
                CameraMotionBlurRequest::Follow { amount } => {
                    game_client::core::script_action_handler::script_camera_motion_blur_follow(
                        *amount,
                    );
                }
                CameraMotionBlurRequest::EndFollow => {
                    game_client::core::script_action_handler::script_camera_motion_blur_end_follow(
                    );
                }
            }
            #[cfg(not(feature = "game_client"))]
            if let CameraMotionBlurRequest::Jump { position, .. } = &request {
                self.camera_follow_target = None;
                self.request_camera_focus(*position);
            }
            self.pending_camera_motion_blur.push(request);
        }

        for flash in self.mission_scripts.drain_cameo_flash_requests() {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_cameo_flash(
                &flash.command_button_name,
                flash.flash_count,
            );
            self.script_cameo_flash_count
                .insert(flash.command_button_name, flash.flash_count);
        }

        for mutation in self.mission_scripts.drain_named_timer_mutations() {
            match mutation {
                NamedTimerMutation::Add {
                    name,
                    text,
                    countdown,
                } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_add_named_timer(
                        &name, &text, countdown,
                    );
                    self.script_named_timers.insert(name, (text, countdown));
                }
                NamedTimerMutation::Remove { name } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_remove_named_timer(&name);
                    self.script_named_timers.remove(&name);
                }
            }
        }

        if let Some(show) = self
            .mission_scripts
            .drain_named_timer_display_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_show_named_timer_display(show);
            self.script_named_timer_display_shown = show;
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_superweapon_display_enabled_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_superweapon_display_enabled(
                enabled,
            );
            self.script_superweapon_display_enabled = enabled;
        }

        for mutation in self
            .mission_scripts
            .drain_superweapon_object_display_mutations()
        {
            match mutation {
                SuperweaponObjectDisplayMutation::Hide { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_hide_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .insert(ObjectId(object_id));
                }
                SuperweaponObjectDisplayMutation::Show { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_show_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .remove(&ObjectId(object_id));
                }
            }
        }

        for mutation in self
            .mission_scripts
            .drain_named_special_power_countdown_mutations()
        {
            let _ = self.script_named_special_power_countdown(
                &mutation.unit_name,
                &mutation.power_name,
                mutation.op,
                mutation.seconds,
            );
        }

        if !self.mission_scripts.drain_music_stop_requests().is_empty() {
            self.pending_music_stop = true;
        }

        #[cfg(feature = "game_client")]
        {
            if let Some(amount) = self
                .mission_scripts
                .drain_oversize_terrain_requests()
                .into_iter()
                .last()
            {
                if let Ok(mut terrain_guard) =
                    game_client::terrain::terrain_visual::get_terrain_visual()
                {
                    if let Some(visual) = terrain_guard.as_mut() {
                        visual.oversize_terrain(amount);
                    }
                }
            }

            if let Some(level) = self
                .mission_scripts
                .drain_border_shroud_levels()
                .into_iter()
                .last()
            {
                if !game_client::core::script_action_handler::set_script_display_border_shroud_level(
                    level,
                ) {
                    log::warn!(
                        "Border shroud level script request not applied: display bridge unavailable"
                    );
                }
            }
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
        if seconds > 0.0 {
            seconds
        } else {
            1.0 / 30.0
        }
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
