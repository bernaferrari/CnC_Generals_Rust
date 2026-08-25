//! Audio, movie, radar, map reveal/shroud, UI countdown, time, weather, and command-bar actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

fn resolve_script_named_object_id(unit_name: &str) -> Option<u32> {
    get_named_object_tracker()
        .get_object_id(unit_name)
        .ok()
        .flatten()
        .or_else(|| crate::scripting::host_script_named_unit_id(unit_name))
}

/// C++ `ScriptActions::doEnableObjectSound` leftover drawable + live-host queue.
fn enable_or_disable_object_sound(object_name: &str, enable: bool) {
    super::request_host_script_object_sound(super::HostScriptObjectSoundRequest::Enable {
        unit: object_name.to_string(),
        enable,
    });
    if let Some(handler) = current_script_action_handler() {
        if let Err(err) = handler.enable_object_sound(object_name, enable) {
            log::warn!(
                "Script action handler enable_object_sound({}) failed: {}",
                enable,
                err
            );
        }
    }
    let Some(object_id) = resolve_script_named_object_id(object_name) else {
        return;
    };
    if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(obj_guard) = obj_arc.read() {
            if let Some(drawable) = obj_guard.get_drawable() {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.enable_ambient_sound_from_script(enable);
                }
            }
        }
    }
}

impl ScriptActionDispatcher {
    // ============================================================================
    // ADDITIONAL AUDIO/VIDEO ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_sound_play_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_name = self.get_string_param(action, 0)?;
        let unit_name = self.get_string_param(action, 1)?;
        log::debug!("Playing named sound '{}' from '{}'", sound_name, unit_name);

        // C++ doSoundPlayFromNamed: one TheAudio->addAudioEvent. Do not also
        // enqueue HostScriptObjectSoundRequest::PlayNamed — live drain would
        // play the same name a second time through TheAudio.

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.sound_play_named(&sound_name, &unit_name) {
                log::warn!("Script action handler sound_play_named failed: {}", err);
            }
            return Ok(ScriptActionResult::Success);
        }

        let Some(object_id) = resolve_script_named_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };

        // C++ AudioEventRTS(soundName, pUnit->getID()); setIsLogicalAudio(true).
        let mut event = crate::common::audio::AudioEventRts::new(sound_name.as_str());
        event.set_object_id(object_id);
        event.set_is_logical_audio(true);
        if let Some(audio) = TheAudio::get() {
            let _ = audio.add_audio_event(&event);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_suspend_background_sounds(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Suspending background sounds");
        if let Some(audio) = TheAudio::get() {
            audio.pause_audio(EngineAudioAffect::Sound);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_resume_background_sounds(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Resuming background sounds");
        if let Some(audio) = TheAudio::get() {
            audio.resume_audio(EngineAudioAffect::Sound);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_ambient_pause(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Pausing ambient sound");
        if let Some(audio) = TheAudio::get() {
            audio.pause_audio(EngineAudioAffect::Sound3D);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_ambient_resume(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Resuming ambient sound");
        if let Some(audio) = TheAudio::get() {
            audio.resume_audio(EngineAudioAffect::Sound3D);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_music_set_volume(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let volume = self.get_real_param(action, 0)?;
        log::debug!("Setting music volume to {}", volume);
        if let Some(audio) = TheAudio::get() {
            audio.set_volume((volume / 100.0).clamp(0.0, 1.0), EngineAudioAffect::Music);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_disable_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_type = self.get_string_param(action, 0)?;
        log::debug!("Disabling sound type '{}'", sound_type);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_enabled(&sound_type, false);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_enable_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_type = self.get_string_param(action, 0)?;
        log::debug!("Enabling sound type '{}'", sound_type);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_enabled(&sound_type, true);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_enable_all(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling all sounds");
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_enabled("", true);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_audio_override_volume_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let audio_type = self.get_string_param(action, 0)?;
        let volume = self.get_real_param(action, 1)?;
        log::debug!("Overriding volume for '{}' to {}", audio_type, volume);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_volume_override(&audio_type, volume / 100.0);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_audio_restore_volume_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let audio_type = self.get_string_param(action, 0)?;
        log::debug!("Restoring volume for '{}'", audio_type);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_volume_override(&audio_type, -1.0);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_audio_restore_volume_all_type(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Restoring all audio volumes");
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_volume_override("", -1.0);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_set_volume(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let volume = self.get_real_param(action, 0)?;
        log::debug!("Setting sound volume to {}", volume);
        let normalized = (volume / 100.0).clamp(0.0, 1.0);
        if let Some(audio) = TheAudio::get() {
            audio.set_volume(normalized, EngineAudioAffect::Sound);
            audio.set_volume(normalized, EngineAudioAffect::Sound3D);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_speech_set_volume(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let volume = self.get_real_param(action, 0)?;
        log::debug!("Setting speech volume to {}", volume);
        if let Some(audio) = TheAudio::get() {
            audio.set_volume((volume / 100.0).clamp(0.0, 1.0), EngineAudioAffect::Speech);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_remove_all_disabled(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Removing all disabled sounds");
        if let Some(audio) = TheAudio::get() {
            audio.remove_disabled_events();
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_sound_remove_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_type = self.get_string_param(action, 0)?;
        log::debug!("Removing sound type '{}'", sound_type);
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event_by_name(&sound_type);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_enable_object_sound(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        log::debug!("Enabling sounds for '{}'", object_name);
        enable_or_disable_object_sound(&object_name, true);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_object_sound(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        log::debug!("Disabling sounds for '{}'", object_name);
        enable_or_disable_object_sound(&object_name, false);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_movie_play_fullscreen(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let movie_name = self.get_string_param(action, 0)?;
        log::info!("Playing fullscreen movie '{}'", movie_name);
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.movie_play_fullscreen(&movie_name) {
                log::warn!(
                    "Script action handler movie_play_fullscreen failed: {}",
                    err
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_movie_play_radar(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let movie_name = self.get_string_param(action, 0)?;
        log::debug!("Playing radar movie '{}'", movie_name);
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.movie_play_radar(&movie_name) {
                log::warn!("Script action handler movie_play_radar failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL RADAR/MAP ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_radar_create_event(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let position = self.get_coord_param(action, 0)?;
        let event_type = self.get_int_param(action, 1)?;
        log::debug!(
            "Creating radar event at ({}, {}, {}) type {}",
            position.x,
            position.y,
            position.z,
            event_type
        );
        let radar_event = Self::radar_event_type_from_int(event_type);
        if let Ok(mut radar) = get_radar_system().write() {
            let radar_pos = to_radar_coord(&position);
            radar.create_event(&radar_pos, radar_event, 4.0);
        }
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) =
                handler.create_radar_event(position.x, position.y, position.z, event_type)
            {
                log::warn!("Script action handler create_radar_event failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_radar_force_enable(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Force enabling radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.force_on(true);
        }
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_radar_forced(true) {
                log::warn!(
                    "Script action handler set_radar_forced(true) failed: {}",
                    err
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_radar_revert_to_normal(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Reverting radar to normal");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.force_on(false);
        }
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_radar_forced(false) {
                log::warn!(
                    "Script action handler set_radar_forced(false) failed: {}",
                    err
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_map_reveal_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Revealing all map for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr.reveal_map_for_player(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr.reveal_map_for_player(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_map_reveal_all_perm(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Permanently revealing all map for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr
                        .reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr
                        .reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_map_reveal_all_undo_perm(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Undoing permanent map reveal for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr
                        .undo_reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr
                        .undo_reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_map_shroud_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Shrouding all map for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr.shroud_map_for_player(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr.shroud_map_for_player(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_map_reveal_permanently_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let radius = self.get_real_param(action, 1)?;
        let player_name = self.get_string_param(action, 2)?;
        let reveal_name = self.get_string_param(action, 3)?;

        log::debug!(
            "Permanently revealing map '{}' at waypoint '{}' radius {} for '{}'",
            reveal_name,
            waypoint,
            radius,
            player_name
        );

        let _ = with_script_engine_mut(|engine| {
            engine.create_named_map_reveal(&reveal_name, &waypoint, radius, &player_name);
            engine.do_named_map_reveal(&reveal_name);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_map_undo_reveal_permanently_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let reveal_name = self.get_string_param(action, 0)?;
        log::debug!("Undoing permanent reveal '{}'", reveal_name);

        let _ = with_script_engine_mut(|engine| {
            engine.undo_named_map_reveal(&reveal_name);
            engine.remove_named_map_reveal(&reveal_name);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_map_switch_border(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let border_index = self.get_int_param(action, 0)?;
        log::debug!("Switching map border to '{}'", border_index);

        let mut observer_player_index: Option<u32> = None;
        if let Ok(players) = player_list().read() {
            if let Some(observer) = players.find_player_by_name("ReplayObserver") {
                if let Ok(observer_guard) = observer.read() {
                    observer_player_index = Some(observer_guard.get_player_index() as u32);
                }
            }
        }

        if let Some(observer_index) = observer_player_index {
            if let Ok(mut shroud_mgr) = crate::system::shroud_manager::get_shroud_manager().lock() {
                let _ = shroud_mgr.undo_reveal_map_for_player_permanently(observer_index);
            }
        }

        if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
            terrain.set_active_boundary(border_index);
        }

        if let Some(observer_index) = observer_player_index {
            if let Ok(mut shroud_mgr) = crate::system::shroud_manager::get_shroud_manager().lock() {
                let _ = shroud_mgr.reveal_map_for_player_permanently(observer_index);
            }
        }

        if let Ok(mut shroud_mgr) = crate::system::shroud_manager::get_shroud_manager().lock() {
            shroud_mgr.refresh_shroud_for_local_player();
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_refresh_radar(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Refreshing radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.refresh_terrain();
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_object_create_radar_event(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        let event_type = self.get_int_param(action, 1)?;
        log::debug!(
            "Creating radar event for object '{}' (type {})",
            object_name,
            event_type
        );
        // Wave 284: empty dual-world → live host drain.
        if dual_world_registry_unavailable() {
            super::request_host_script_radar_event(super::HostScriptRadarEventRequest::Object {
                unit: object_name,
                event_type,
            });
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&object_name).ok().flatten();
        if let Some(object_id) = object_id_opt {
            if let Some(pos) =
                OBJECT_REGISTRY.with_object(object_id, |object_guard| *object_guard.get_position())
            {
                let radar_event = Self::radar_event_type_from_int(event_type);
                if let Ok(mut radar) = get_radar_system().write() {
                    let radar_pos = to_radar_coord(&pos);
                    radar.create_event(&radar_pos, radar_event, 4.0);
                }
                if let Some(handler) = current_script_action_handler() {
                    if let Err(err) = handler.create_radar_event(pos.x, pos.y, pos.z, event_type) {
                        log::warn!("Script action handler create_radar_event failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_border_shroud(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling border shroud");
        if let Some(global) = crate::helpers::TheGlobalData::get() {
            let level = global.get_clear_alpha();
            if let Some(handler) = current_script_action_handler() {
                let _ = handler.set_border_shroud_level(level);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_enable_border_shroud(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling border shroud");
        if let Some(global) = crate::helpers::TheGlobalData::get() {
            let level = global.get_shroud_alpha();
            if let Some(handler) = current_script_action_handler() {
                let _ = handler.set_border_shroud_level(level);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_resize_view_guardband(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let gbx = self.get_real_param(action, 0)?;
        let gby = self.get_real_param(action, 1)?;
        log::debug!("Resizing view guardband to ({}, {})", gbx, gby);

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.resize_view_guardband(gbx, gby) {
                log::warn!(
                    "Script action handler resize_view_guardband failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL DISPLAY/UI ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_cameo_flash(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let cameo_name = self.get_string_param(action, 0)?;
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!("Flashing cameo '{}' for {}s", cameo_name, time_in_seconds);

        let frames = LOGICFRAMES_PER_SECOND as i32 * time_in_seconds;
        let drawable_frames_per_flash = (LOGICFRAMES_PER_SECOND as i32 / 2).max(1);
        let mut count = frames / drawable_frames_per_flash;
        // C++: ensure the cameo ends in its original visual state.
        if (count % 2) == 1 {
            count += 1;
        }

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.cameo_flash(&cameo_name, count) {
                log::warn!("Script action handler cameo_flash failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_display_countdown_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let timer_text = self.get_string_param(action, 1)?;
        log::debug!(
            "Displaying countdown timer '{}' text '{}'",
            timer_name,
            timer_text
        );

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.add_named_timer(&timer_name, &timer_text, true) {
                log::warn!("Script action handler add_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_hide_countdown_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding countdown timer '{}'", timer_name);

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.remove_named_timer(&timer_name) {
                log::warn!("Script action handler remove_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_enable_countdown_timer_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling countdown timer display");

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.show_named_timer_display(true) {
                log::warn!(
                    "Script action handler show_named_timer_display(true) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_countdown_timer_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling countdown timer display");

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.show_named_timer_display(false) {
                log::warn!(
                    "Script action handler show_named_timer_display(false) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_display_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        let counter_text = self.get_string_param(action, 1)?;
        log::debug!(
            "Displaying counter '{}' text '{}'",
            counter_name,
            counter_text
        );

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.add_named_timer(&counter_name, &counter_text, false) {
                log::warn!("Script action handler add_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_hide_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding counter '{}'", counter_name);

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.remove_named_timer(&counter_name) {
                log::warn!("Script action handler remove_named_timer failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_special_power_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling special power display");

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_superweapon_display_enabled_by_script(false) {
                log::warn!(
                    "Script action handler set_superweapon_display_enabled_by_script(false) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_enable_special_power_display(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling special power display");

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_superweapon_display_enabled_by_script(true) {
                log::warn!(
                    "Script action handler set_superweapon_display_enabled_by_script(true) failed: {}",
                    err
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_ingame_popup_message(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        let x_percent = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(50);
        let y_percent = action.get_parameter(2).map(|p| p.get_int()).unwrap_or(50);
        let width = action.get_parameter(3).map(|p| p.get_int()).unwrap_or(400);
        let pause = action
            .get_parameter(4)
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        log::info!(
            "In-game popup: '{}' at ({}, {}) width {} pause {}",
            message,
            x_percent,
            y_percent,
            width,
            pause
        );

        // C++: TheInGameUI->popupMessage(message, x, y, width, pause, FALSE)
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) =
                handler.popup_message(&message, x_percent, y_percent, width, pause, false)
            {
                log::warn!("Script action handler popup_message failed: {}", err);
                let _ = handler.display_text(&message);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_object_force_select(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let center_in_view = action
            .get_parameter(2)
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        let audio_to_play = action
            .get_parameter(3)
            .map(|p| p.get_string().to_string())
            .unwrap_or_default();

        log::debug!(
            "Force selecting object type '{}' on team '{}' (center_in_view: {}, audio: '{}')",
            object_type,
            team_name,
            center_in_view,
            audio_to_play
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_force_select(super::HostScriptForceSelectRequest {
                team: team_name,
                object_type,
                center_in_view,
                audio: audio_to_play,
            });
            return Ok(ScriptActionResult::Success);
        }

        let team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name));
        let Some(team_arc) = team_arc else {
            return Ok(ScriptActionResult::Success);
        };

        let member_ids = if let Ok(team_guard) = team_arc.read() {
            team_guard.get_members().to_vec()
        } else {
            Vec::new()
        };

        let mut best_guess: Option<ObjectID> = None;
        for member_id in member_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_template_name() != object_type {
                continue;
            }
            if obj_guard.get_drawable().is_none() {
                continue;
            }
            if best_guess.is_none() || member_id < best_guess.unwrap_or(member_id) {
                best_guess = Some(member_id);
            }
        }

        let Some(selected_id) = best_guess else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(selected_obj) = TheGameLogic::find_object_by_id(selected_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let local_player_mask = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_mask()))
            .unwrap_or(crate::common::PLAYERMASK_ALL);

        let mut selected_pos = Coord3D::ZERO;
        if let Ok(selected_guard) = selected_obj.read() {
            selected_pos = *selected_guard.get_position();
            let _ = TheGameLogic::select_object(&*selected_guard, true, local_player_mask, true);
        }

        if !audio_to_play.is_empty() {
            let mut audio_event = crate::common::audio::AudioEventRts::new(audio_to_play.as_str());
            if let Some(local_player) = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
            {
                if let Ok(local_guard) = local_player.read() {
                    audio_event.set_player_index(local_guard.get_player_index() as u32);
                }
            }
            if let Some(audio) = TheAudio::get() {
                let _ = audio.add_audio_event(&audio_event);
            }
        }

        if center_in_view {
            if let Some(handler) = current_script_action_handler() {
                let _ = handler.move_camera_to(
                    selected_pos.x,
                    selected_pos.y,
                    selected_pos.z,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // TIME CONTROL ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_freeze_time(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Freezing time");
        let _ = with_script_engine_mut(|script_engine| script_engine.do_freeze_time());
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.freeze_time() {
                log::warn!("Script action handler freeze_time failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_unfreeze_time(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Unfreezing time");
        let _ = with_script_engine_mut(|script_engine| script_engine.do_unfreeze_time());
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.unfreeze_time() {
                log::warn!("Script action handler unfreeze_time failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_visual_speed_multiplier(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let multiplier = self.get_int_param(action, 0)?;
        log::debug!("Setting visual speed multiplier to {}", multiplier);
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_visual_speed_multiplier(multiplier) {
                log::warn!(
                    "Script action handler set_visual_speed_multiplier failed: {}",
                    err
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_fps_limit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let fps = self.get_int_param(action, 0)?;
        log::debug!("Setting FPS limit to {}", fps);
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_fps_limit(fps) {
                log::warn!("Script action handler set_fps_limit failed: {}", err);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ENVIRONMENT/WORLD ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_set_tree_sway(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let direction = self.get_real_param(action, 0)?;
        let intensity = self.get_real_param(action, 1)?;
        let lean = self.get_real_param(action, 2)?;
        let breeze_period = self.get_int_param(action, 3)?;
        let randomness = self.get_real_param(action, 4)?;
        log::debug!(
            "Setting tree sway direction {} intensity {} lean {} period {} randomness {}",
            direction,
            intensity,
            lean,
            breeze_period,
            randomness
        );

        let _ = with_script_engine_mut(|script_engine| {
            script_engine.set_breeze_info(direction, intensity, lean, breeze_period, randomness);
        });

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_water_change_height(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let water_name = self.get_string_param(action, 0)?;
        let height = self.get_real_param(action, 1)?;
        log::debug!("Changing water '{}' height to {}", water_name, height);

        let water_name_ascii = AsciiString::from(water_name.as_str());
        if let Ok(mut terrain) = get_terrain_logic().write() {
            if terrain
                .get_water_handle_by_name(&water_name_ascii)
                .is_some()
            {
                terrain.set_water_height(&water_name_ascii, height, 999_999.9, true);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_water_change_height_over_time(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let water_name = self.get_string_param(action, 0)?;
        let height = self.get_real_param(action, 1)?;
        let time = self.get_real_param(action, 2)?;
        let damage = self.get_real_param(action, 3)?;
        log::debug!(
            "Changing water '{}' height to {} over {} seconds (damage {})",
            water_name,
            height,
            time,
            damage
        );

        let water_name_ascii = AsciiString::from(water_name.as_str());
        if let Ok(mut terrain) = get_terrain_logic().write() {
            terrain.change_water_height_over_time(&water_name_ascii, height, time, damage);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_cave_index(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let cave_name = self.get_string_param(action, 0)?;
        let cave_index = self.get_int_param(action, 1)?;
        log::debug!("Setting cave '{}' index to {}", cave_name, cave_index);

        // Live host: leftover OBJECT_REGISTRY is empty. C++ doSetCaveIndex
        // still looks up the named cave and tryToSetCaveIndex.
        if super::dual_world_registry_unavailable() {
            super::request_host_set_cave_index(&cave_name, cave_index);
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&cave_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    if let Some(contain) = obj_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            contain_guard.try_to_set_cave_index(cave_index);
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_show_weather(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let show_weather = self.get_bool_param_optional(action, 0).unwrap_or(true);
        log::debug!("Setting weather visibility to {}", show_weather);

        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_weather_visible(show_weather) {
                log::warn!("Script action handler set_weather_visible failed: {}", err);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_infantry_lighting_override(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let setting = self.get_real_param(action, 0)?;
        if setting != -1.0 && setting <= 0.0 {
            log::warn!(
                "Invalid infantry lighting override {}; expected -1.0 or > 0.0",
                setting
            );
        }
        if let Ok(mut gd) = global_data::write_safe() {
            gd.script_override_infantry_light_scale = setting;
        }
        log::debug!("Setting infantry lighting override to {}", setting);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_reset_infantry_lighting_override(
        &mut self,
    ) -> Result<ScriptActionResult, ScriptError> {
        if let Ok(mut gd) = global_data::write_safe() {
            gd.script_override_infantry_light_scale = -1.0;
        }
        log::debug!("Resetting infantry lighting override to -1.0");
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // CONSTRUCTION/TECHTREE ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_set_base_construction_speed(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let delay_seconds = self.get_int_param(action, 1)?;
        log::debug!(
            "Setting base construction speed for '{}' to {} seconds",
            player_name,
            delay_seconds
        );
        super::request_host_set_base_construction_speed(&player_name, delay_seconds);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_team_delay_seconds(delay_seconds);
                };
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_techtree_modify_buildability_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_type = self.get_string_param(action, 0)?;
        let buildable_status = self.get_int_param(action, 1)?;
        // Live leftover ThingFactory is empty. Always write the override by
        // the script object-type name so host can_make_unit can read it.
        TheGameLogic::set_buildable_status_override(&object_type, buildable_status);
        if let Some(template) = TheObjectFactory::find_template(&object_type) {
            TheGameLogic::set_buildable_status_override(
                template.get_name().as_str(),
                buildable_status,
            );
        }
        crate::scripting::executor::request_host_buildable_status_override(
            &object_type,
            buildable_status,
        );
        log::debug!(
            "Modifying buildability for '{}' to status {}",
            object_type,
            buildable_status
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_warehouse_set_value(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let warehouse_name = self.get_string_param(action, 0)?;
        let value = self.get_int_param(action, 1)?;
        log::debug!("Setting warehouse '{}' value to {}", warehouse_name, value);

        // Live host: leftover OBJECT_REGISTRY is empty. Always push the
        // C++ setCashValue through MissionScriptActionHandler.
        if let Some(handler) = current_script_action_handler() {
            if let Err(err) = handler.set_warehouse_value(&warehouse_name, value) {
                log::warn!("Script action handler set_warehouse_value failed: {}", err);
            }
        }

        let tracker = get_named_object_tracker();
        let Some(warehouse_id) = tracker.get_object_id(&warehouse_name).ok().flatten() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(warehouse_arc) = TheGameLogic::find_object_by_id(warehouse_id) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(warehouse_guard) = warehouse_arc.read() {
            let Some(module) = warehouse_guard.find_update_module("SupplyWarehouseDockUpdate")
            else {
                return Ok(ScriptActionResult::Success);
            };

            module.with_module(|module| {
                if let Some(warehouse) = module.get_supply_warehouse_dock_interface() {
                    warehouse.set_cash_value(value);
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_command_bar_remove_button_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let button_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;

        let Some(template) = TheObjectFactory::find_template(&object_type) else {
            return Ok(ScriptActionResult::Success);
        };
        let command_set_name = template.get_command_set_string().as_str().to_string();

        let slot = get_control_bar_bridge().and_then(|control_bar| {
            control_bar
                .find_command_set_by_name(command_set_name.as_str())
                .and_then(|set| {
                    set.buttons.iter().position(|button| {
                        button
                            .as_ref()
                            .map(|b| b.name.eq_ignore_ascii_case(&button_name))
                            .unwrap_or(false)
                    })
                })
        });

        if let Some(slot) = slot {
            let _ = set_command_set_slot_override(command_set_name.as_str(), slot, None);
            crate::control_bar::mark_ui_dirty();
        }

        log::debug!(
            "Removing command bar button '{}' for '{}'",
            button_name,
            object_type
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_command_bar_add_button_object_type_slot(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let button_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let slot_num = self.get_int_param(action, 2)?;

        let Some(template) = TheObjectFactory::find_template(&object_type) else {
            return Ok(ScriptActionResult::Success);
        };
        let command_set_name = template.get_command_set_string().as_str().to_string();

        let slot = slot_num - 1;
        if !(0..crate::command_button::MAX_COMMANDS_PER_SET as i32).contains(&slot) {
            return Ok(ScriptActionResult::Success);
        }

        let _ = set_command_set_slot_override(
            command_set_name.as_str(),
            slot as usize,
            Some(button_name.as_str()),
        );
        crate::control_bar::mark_ui_dirty();

        log::debug!(
            "Adding command bar button '{}' for '{}' at slot {}",
            button_name,
            object_type,
            slot_num
        );
        Ok(ScriptActionResult::Success)
    }
}
