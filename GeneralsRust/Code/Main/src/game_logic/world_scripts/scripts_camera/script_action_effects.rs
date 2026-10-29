//! Apply completed script action effects synchronously to the driving Main world.
//! CPP ScriptEngine.cpp:7609–7654 dispatches every action before the next action.
use super::*;

impl GameLogic {
    /// Older output from an unmigrated callback must precede the next borrowed
    /// invocation. Drain it before any new script action can observe the UI.
    pub(super) fn deliver_pending_script_ui(
        &mut self,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let upgrade_observer = self.receiving_upgrade_tooltip_observer();
        let game_ending = ui.script_events.map_or_else(
            || self.owned_script_game_ending(),
            |recipient| recipient.is_game_ending(),
        );
        let Some(ui) = ui.ui.as_mut() else {
            return;
        };
        // Catch up detached/headless intent on the next receiving Client loan.
        ui.weather_visible(self.weather_state.visible);
        for enabled in std::mem::take(&mut self.session_view.pending_letterbox_visibility) {
            ui.letterbox(
                &mut self.session_view.camera,
                enabled,
                matches!(self.game_mode, GameMode::Replay),
                game_engine::common::thing::receiving_module_content::UpgradeTooltipSource::Receiving(&upgrade_observer),
                game_ending,
            );
            self.session_view
                .letterbox_visibility_delivered_since_present = true;
        }
    }

    fn admit_script_letterbox_state(&mut self, enabled: bool) {
        // W3DDisplay.cpp:2092: only a transition changes the zoom limit. This
        // runs at the authored action, before another action may move the view.
        if self.cinematic_letterbox != enabled {
            self.session_view
                .camera
                .native_camera
                .set_zoom_limited(!enabled);
        }
        self.cinematic_letterbox = enabled;
    }

    pub(super) fn apply_script_action_requests(&mut self) {
        self.apply_script_action_requests_with_interpreter(None);
    }

    pub(super) fn apply_script_action_requests_with_interpreter(
        &mut self,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        if let Err(error) =
            self.apply_script_action_requests_with_effects(interpreter, Default::default())
        {
            log::error!("script presentation effects: {error}");
        }
    }

    pub(super) fn apply_script_action_requests_with_effects(
        &mut self,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        mut ui: crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(interpreter) = interpreter else {
            return self
                .with_owned_script_engine(|engine, world| {
                    world.apply_script_action_requests_with_effects(Some(engine), ui)
                })
                .expect("script action delivery owns its receiving interpreter");
        };
        // This boundary can be reached without a HostScriptExecutionDriver.
        // Preserve the exact supplied interpreter through every nested effect.
        let mut receiving_ui = ui.reborrow();
        receiving_ui.script_events =
            Some(crate::game_logic::ScriptEventRecipient::new(interpreter));
        let mut ui = receiving_ui;
        if let Some(native_ui) = ui.ui.as_deref_mut() {
            self.mission_scripts
                .deliver_win_lose_window_actions(native_ui)?;
        } else {
            // Headless interpreter calls have no native window owner; discard
            // only these presentation requests, without retaining stale work.
            drop(self.mission_scripts.drain_win_lose_window_actions());
        }
        self.apply_host_skirmish_script_requests();
        self.apply_host_set_base_construction_speed_requests();
        self.apply_host_set_train_held_requests();
        self.apply_host_player_relates_script_requests();

        self.apply_host_loco_set_script_requests_with_effects(&mut ui)?;
        self.apply_host_face_script_requests_with_effects(&mut ui)?;

        self.apply_host_move_attack_script_requests_with_interpreter_and_effects(
            interpreter,
            &mut ui,
        )?;
        self.apply_host_hunt_guard_script_requests(&mut ui)?;
        self.apply_host_idle_script_requests_with_effects(&mut ui);
        self.apply_host_kill_delete_damage_script_requests_with_effects(&mut ui);

        self.apply_host_follow_waypoints_script_requests();
        self.apply_host_skirmish_approach_path_script_requests();

        self.apply_host_create_script_requests_with_effects(Some(interpreter), &mut ui);
        self.apply_host_boobytrap_script_requests_with_effects(Some(interpreter), &mut ui);
        self.apply_host_unmanned_script_requests_with_effects(&mut ui);
        self.apply_host_radar_event_script_requests();
        self.apply_host_stealth_enabled_script_requests();
        self.apply_host_team_attitude_script_requests();
        self.apply_host_script_visual_status_requests_with_effects(&mut ui);
        self.apply_host_guard_supply_center_script_requests();
        self.apply_host_guard_variant_script_requests_with_effects(&mut ui);
        self.apply_host_named_fire_special_script_requests();
        self.apply_host_use_command_button_script_requests_with_policy(interpreter);
        self.apply_host_skirmish_fight_script_requests_with_policy(&mut ui, interpreter);

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
            self.request_camera_focus_with_effects(camera_target, &mut ui)?;
        }

        if !camera_focus.move_to_selection.is_empty() {
            // C++ cameraModFinalMoveTo does not alter an active rotate.
            if !self.session_view.camera.rotation_registers.active() {
                if let Some(center) = self.selected_objects_center_for_local_player() {
                    self.session_view.scripted.motion.final_move_to(
                        crate::presentation_frame::waypoint::Point::from_world(center),
                    );
                }
            }
        }

        if !camera_focus.move_home.is_empty() {
            if let Some(home) = self.local_player_camera_home_position() {
                self.session_view.scripted.camera_follow_target = None;
                self.request_camera_focus_with_effects(home, &mut ui)?;
            }
        }

        if let Some(last) = camera_focus.follows.into_iter().last() {
            self.set_camera_follow_object(
                (last.object_id != 0).then_some(ObjectId(last.object_id)),
            );
            if last.object_id != 0 && last.snap_to_unit {
                self.session_view.camera.camera_follow_snap_immediate = true;
            }
        }
        if let Some(last) = camera_focus.tethers.into_iter().last() {
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
            self.apply_script_camera_default(last)
                .map_err(gamelogic::GameLogicError::Configuration)?;
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_slave_mode_enable_requests()
            .into_iter()
            .last()
        {
            self.session_view.camera.camera_slave_mode = Some(last.clone());
            self.pending_camera_slave_mode_enable = Some(last);
            self.pending_camera_slave_mode_disable = false;
        }

        if !self
            .mission_scripts
            .drain_camera_slave_mode_disable_requests()
            .is_empty()
        {
            self.session_view.camera.camera_slave_mode = None;
            self.pending_camera_slave_mode_enable = None;
            self.pending_camera_slave_mode_disable = true;
        }

        // Retained standalone hooks meet the same receiving action boundary.
        // New borrowed ScreenShake already applied directly and queues nothing.
        for request in self.mission_scripts.drain_screen_shake_requests() {
            self.admit_script_screen_shake(request.intensity);
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
            self.admit_camera_reset_motion(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_zoom_requests()
            .into_iter()
            .last()
        {
            self.session_view.camera.install_zoom(
                last.zoom,
                last.duration_seconds,
                last.ease_in_seconds,
                last.ease_out_seconds,
            );
            self.pending_camera_zoom = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_pitch_requests()
            .into_iter()
            .last()
        {
            self.session_view.camera.install_pitch(
                last.pitch,
                last.duration_seconds,
                last.ease_in_seconds,
                last.ease_out_seconds,
            );
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

            self.session_view.install_rotate(
                last.rotations,
                last.duration_seconds,
                last.ease_in_seconds,
                last.ease_out_seconds,
                self.visual_speed_multiplier,
            );
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
            self.install_owned_final_zoom(last.zoom, last.ease_in, last.ease_out);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_pitch_requests()
            .into_iter()
            .last()
        {
            self.install_owned_final_pitch(last.pitch, last.ease_in, last.ease_out);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_setup_requests()
            .into_iter()
            .last()
        {
            self.session_view.scripted.camera_follow_target = None;
            // ScriptActions::doSetupCamera performs the original zero-ms
            // waypoint admission, then modifies that same live path in order.
            self.start_camera_move_to(CameraMoveToRequest {
                position: last.position,
                seconds: 0.,
                camera_stutter_seconds: 0.,
                ease_in_seconds: 0.,
                ease_out_seconds: 0.,
            });
            self.apply_script_camera_mod_look_toward(last.look_toward, false);
            self.install_owned_final_pitch(last.pitch, 0., 0.);
            self.install_owned_final_zoom(last.zoom, 0., 0.);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_waypoint_requests()
            .into_iter()
            .last()
        {
            if self.session_view.install_look_toward(
                self.session_view.target,
                last.position,
                last.duration_seconds,
                last.ease_in_seconds,
                last.ease_out_seconds,
                last.reverse_rotation,
                self.visual_speed_multiplier,
            ) {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(last);
            }
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
                self.pending_camera_rotate = None;
                self.install_camera_object_tracking(
                    last.object_id,
                    last.duration_seconds,
                    last.hold_seconds,
                    last.ease_in_seconds,
                    last.ease_out_seconds,
                );
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position,
                    duration_seconds: last.duration_seconds,
                    ease_in_seconds: last.ease_in_seconds,
                    ease_out_seconds: last.ease_out_seconds,
                    reverse_rotation: false,
                });
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

        self.deliver_pending_script_ui(&mut ui);
        for last in self.mission_scripts.drain_letterbox_events() {
            if let Some(ui) = ui.ui.as_mut() {
                let upgrade_observer = self.receiving_upgrade_tooltip_observer();
                ui.letterbox(
                    &mut self.session_view.camera,
                    last,
                    matches!(self.game_mode, GameMode::Replay),
                    game_engine::common::thing::receiving_module_content::UpgradeTooltipSource::Receiving(&upgrade_observer),
                    interpreter.is_game_ending(),
                );
                self.session_view
                    .letterbox_visibility_delivered_since_present = true;
                self.admit_script_letterbox_state(last);
                continue;
            }
            // Unmigrated callback roots still deliver through the explicit
            // pending output. Do not replay synchronously delivered effects.
            self.session_view.pending_letterbox_visibility.push(last);
            self.admit_script_letterbox_state(last);
            // A detached operation has no WND capability. Its existing pending
            // presentation output is admitted by the receiving UI boundary;
            // no peer tree may authorize a synchronous camera-height mutation.
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
            if let Some(ui) = ui.ui.as_mut() {
                ui.weather_visible(visible);
            }
        }

        let popup_messages = self.mission_scripts.drain_popup_message_requests();
        if !popup_messages.is_empty() {
            // C++ InGameUI owns one popup layout: every new popup replaces the
            // previously visible one.  Keep only the newest presentation
            // residual; MissionScriptHooks itself remains the future-event
            // queue and is already drained above.
            let active_popup = popup_messages.last().cloned();
            if let (Some(popup), Some(recipient)) = (active_popup.as_ref(), ui.ui.as_deref_mut()) {
                recipient.popup_message(popup)?;
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

        // These consecutive consumers only mutate concrete view/presentation
        // values; none reenter the hook producers. Preserve the original late
        // capture boundary and guardband -> BW -> skybox -> blur effect order.
        let camera_view = self.mission_scripts.take_camera_view_requests();
        if let Some(last) = camera_view.guardbands.into_iter().last() {
            self.queue_pending_view_guardband(last.x_bias, last.y_bias);
        }

        if let Some(last) = camera_view.bw_modes.into_iter().last() {
            #[cfg(feature = "game_client")]
            self.session_view.apply_bw(last.enabled, last.frames);
            // Compatibility notification only; GPU observes session_view.filter.
            self.pending_camera_bw_mode = Some(last);
        }

        if let Some(enabled) = camera_view.skybox_enabled.into_iter().last() {
            self.script_skybox_enabled = enabled;
        }

        for request in camera_view.motion_blur {
            #[cfg(feature = "game_client")]
            if let Some(position) = self.session_view.apply_motion_blur(&request) {
                self.session_view.scripted.camera_follow_target = None;
                self.request_camera_focus_with_effects(position, &mut ui)?;
            }
            #[cfg(not(feature = "game_client"))]
            if let CameraMotionBlurRequest::Jump { position, .. } = &request {
                self.session_view.scripted.camera_follow_target = None;
                self.request_camera_focus_with_effects(*position, &mut ui)?;
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
                    if let Some(native_ui) = ui.ui.as_deref_mut() {
                        let counters = self
                            .script_named_timers
                            .keys()
                            .chain(std::iter::once(&name))
                            .filter_map(|name| {
                                interpreter
                                    .get_counter(name)
                                    .map(|counter| (name.clone(), counter.value))
                            })
                            .collect();
                        native_ui.observe_named_timer_counters(counters);
                        native_ui.add_named_timer(&name, &text, countdown);
                    }
                    self.script_named_timers.insert(name, (text, countdown));
                }
                NamedTimerMutation::Remove { name } => {
                    if let Some(native_ui) = ui.ui.as_deref_mut() {
                        native_ui.remove_named_timer(&name);
                    }
                    self.script_named_timers.remove(&name);
                }
            }
        }

        if let Some(native_ui) = ui.ui.as_deref_mut() {
            let counters = self
                .script_named_timers
                .keys()
                .filter_map(|name| {
                    interpreter
                        .get_counter(name)
                        .map(|counter| (name.clone(), counter.value))
                })
                .collect();
            native_ui.observe_named_timer_counters(counters);
        }

        if let Some(show) = self
            .mission_scripts
            .drain_named_timer_display_updates()
            .into_iter()
            .last()
        {
            if let Some(native_ui) = ui.ui.as_deref_mut() {
                native_ui.show_named_timer_display(show);
            }
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
                if let Some(terrain) = ui.terrain.as_mut() {
                    terrain.oversize_terrain(amount);
                }
            }

            if let Some(level) = self
                .mission_scripts
                .drain_border_shroud_levels()
                .into_iter()
                .last()
            {
                self.session_view.border_shroud_level = Some(level);
            }
        }
        Ok(())
    }

    fn install_owned_final_zoom(&mut self, factor: f32, ease_in: f32, ease_out: f32) {
        if !self.session_view.scripted.motion.active
            && !self.session_view.camera.rotation_registers.active()
        {
            return;
        }
        // Source tests the active flags, not a positive remaining time. An
        // instant path still admits final axes after its terminal crossing;
        // scalar admission performs the original minimum-one-ms handling.
        let remaining = self.script_camera_remaining_seconds();
        let target = if self.session_view.scripted.motion.active {
            self.session_view.scripted.motion.points
                [self.session_view.scripted.motion.num_waypoints]
                .world()
        } else {
            self.session_view.target
        };
        let Some(max_zoom) = self.owned_script_camera_max_zoom_at(target) else {
            return;
        };
        let request = CameraZoomRequest {
            zoom: factor * max_zoom,
            duration_seconds: remaining.max(0.),
            ease_in_seconds: remaining * ease_in,
            ease_out_seconds: remaining * ease_out,
        };
        self.session_view.camera.install_zoom(
            request.zoom,
            request.duration_seconds,
            request.ease_in_seconds,
            request.ease_out_seconds,
        );
        self.pending_camera_zoom = Some(request);
    }
    fn install_owned_final_pitch(&mut self, pitch: f32, ease_in: f32, ease_out: f32) {
        if !self.session_view.scripted.motion.active
            && !self.session_view.camera.rotation_registers.active()
        {
            return;
        }
        // Source tests the active flags, not a positive remaining time. An
        // instant path still admits final axes after its terminal crossing;
        // scalar admission performs the original minimum-one-ms handling.
        let remaining = self.script_camera_remaining_seconds();
        let request = CameraPitchRequest {
            pitch,
            duration_seconds: remaining.max(0.),
            ease_in_seconds: remaining * ease_in,
            ease_out_seconds: remaining * ease_out,
        };
        self.session_view.camera.install_pitch(
            request.pitch,
            request.duration_seconds,
            request.ease_in_seconds,
            request.ease_out_seconds,
        );
        self.pending_camera_pitch = Some(request);
    }
}

#[cfg(test)]
mod camera_focus_drain_tests;

#[cfg(test)]
#[path = "script_action_effects/follow_admission_tests.rs"]
mod follow_admission_tests;
