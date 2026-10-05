//! Apply completed script action effects synchronously to the driving Main world.
//! CPP ScriptEngine.cpp:7609–7654 dispatches every action before the next action.
use super::*;

impl GameLogic {
    pub(super) fn apply_script_action_requests(&mut self) {
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

        self.drain_warehouse_script_set_values();

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

        let camera_focus = self.mission_scripts.take_camera_focus_requests();
        for camera_target in camera_focus.moves {
            self.request_camera_focus(camera_target);
        }

        if !camera_focus.move_to_selection.is_empty() {
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

        if !camera_focus.move_home.is_empty() {
            if let Some(home) = self.local_player_camera_home_position() {
                self.camera_follow_target = None;
                self.request_camera_focus(home);
            }
        }

        if let Some(last) = camera_focus.follows.into_iter().last() {
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

        if let Some(last) = camera_focus.tethers.into_iter().last() {
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
}

#[cfg(test)]
mod camera_focus_drain_tests;
