// C++ ownership: ScriptActions.cpp action dispatch — audio, camera, named timers/counters, UI, and victory operations.

pub struct MissionScriptActionHandler {
    hooks: Arc<MissionScriptHooks>,
}

impl MissionScriptActionHandler {
    pub fn new(hooks: Arc<MissionScriptHooks>) -> Self {
        Self { hooks }
    }

    pub fn hooks(&self) -> Arc<MissionScriptHooks> {
        Arc::clone(&self.hooks)
    }

    fn local_player_index() -> Option<u32> {
        let players = gamelogic::player::player_list().read().ok()?;
        let index = players.get_local_player_index();
        (index >= 0).then_some(index as u32)
    }

    /// C++ `ScriptActions::doMusicTrackChange` (ScriptActions.cpp:3271-3286):
    /// `TheAudio->removeAudioEvent(AHSV_StopTheMusic[Fade])` then
    /// `TheAudio->addAudioEvent` of the named track (GameMusic / MusicManager).
    fn play_music_track_through_the_audio(track: &str, fade_out: bool, fade_in: bool) {
        const AHSV_STOP_THE_MUSIC: u32 = 0xFFFF_FFF0;
        const AHSV_STOP_THE_MUSIC_FADE: u32 = 0xFFFF_FFF1;

        let Some(audio) = gamelogic::helpers::TheAudio::get() else {
            return;
        };
        audio.remove_audio_event(if fade_out {
            AHSV_STOP_THE_MUSIC_FADE
        } else {
            AHSV_STOP_THE_MUSIC
        });

        let mut event = gamelogic::common::audio::AudioEventRts::new(track);
        event.set_should_fade(fade_in);
        if let Some(player_index) = Self::local_player_index() {
            event.set_player_index(player_index);
        }
        let _handle = audio.add_audio_event(&event);
    }

    /// C++ `ScriptActions::doSpeechPlay` (ScriptActions.cpp:2743-2764):
    /// `AudioEventRTS` + `setIsLogicalAudio(true)` + local player index +
    /// `setUninterruptable(!allowOverlap)` + `TheAudio->addAudioEvent`.
    fn play_speech_through_the_audio(name: &str, allow_overlap: bool) -> u32 {
        let Some(audio) = gamelogic::helpers::TheAudio::get() else {
            return 0;
        };
        let mut event = gamelogic::common::audio::AudioEventRts::new(name);
        event.set_is_logical_audio(true);
        event.set_uninterruptable(!allow_overlap);
        if let Some(player_index) = Self::local_player_index() {
            event.set_player_index(player_index);
        }
        audio.add_audio_event(&event)
    }

    /// C++ `ScriptActions::doSoundPlayFromNamed` (ScriptActions.cpp:2723-2733):
    /// `AudioEventRTS(soundName, pUnit->getID())` + `setIsLogicalAudio(true)`.
    fn play_named_sound_through_the_audio(name: &str, object_id: u32) -> u32 {
        let Some(audio) = gamelogic::helpers::TheAudio::get() else {
            return 0;
        };
        let mut event = gamelogic::common::audio::AudioEventRts::new(name);
        event.set_object_id(object_id);
        event.set_is_logical_audio(true);
        audio.add_audio_event(&event)
    }
}

impl ScriptActionHandler for MissionScriptActionHandler {
    fn enable_script(&self, name: &str, enabled: bool) -> GameLogicResult<()> {
        self.hooks.set_script_enabled(name, enabled)
    }

    fn display_text(&self, text: &str) -> GameLogicResult<()> {
        self.hooks.push_message(text.to_string());
        Ok(())
    }

    fn display_cinematic_text(
        &self,
        text: &str,
        font_type: &str,
        duration_seconds: i32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_cinematic_text(text.to_string(), font_type.to_string(), duration_seconds);
        Ok(())
    }

    fn set_border_shroud_level(&self, level: u8) -> GameLogicResult<()> {
        self.hooks.push_border_shroud_level(level);
        Ok(())
    }

    fn oversize_terrain(&self, amount: i32) -> GameLogicResult<()> {
        self.hooks.push_oversize_terrain(amount);
        Ok(())
    }

    fn military_caption(&self, text: &str, duration_ms: i32) -> GameLogicResult<()> {
        self.hooks
            .push_military_caption(text.to_string(), duration_ms);
        Ok(())
    }

    fn play_sound_effect(&self, sound: &str) -> GameLogicResult<()> {
        // Live GAME_SHELL installs this handler (initialize_scripts), not
        // GameClientScriptActionHandler. C++ PLAY_SOUND_EFFECT always reaches
        // TheAudio via doPlaySoundEffect (setIsLogicalAudio + local player).
        // Do not drain a second unlocal leftover_world_sfx_event / rodio play.
        let result = game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .play_sound_effect(sound);
        self.hooks.note_audio_started(sound);
        result
    }

    fn play_sound_effect_at(&self, sound: &str, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        let result = game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .play_sound_effect_at(sound, x, y, z);
        self.hooks.note_audio_started(sound);
        result
    }

    fn move_camera(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        static DEBUG_CAMERA_MOVE_LOGS: AtomicUsize = AtomicUsize::new(0);
        let position = camera_coord3d_to_world(x, y, z);
        if DEBUG_CAMERA_MOVE_LOGS.fetch_add(1, Ordering::Relaxed) < 16 {
            eprintln!(
                "DEBUG_SHELL_CAMERA_ACTION: move_camera raw=({x:.3}, {y:.3}, {z:.3}) world={position:?}"
            );
        }
        self.hooks.push_camera_move(position);
        Ok(())
    }

    fn move_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        static DEBUG_CAMERA_MOVE_TO_LOGS: AtomicUsize = AtomicUsize::new(0);
        let position = camera_coord3d_to_world(x, y, z);
        if DEBUG_CAMERA_MOVE_TO_LOGS.fetch_add(1, Ordering::Relaxed) < 16 {
            eprintln!(
                "DEBUG_SHELL_CAMERA_ACTION: move_camera_to raw=({x:.3}, {y:.3}, {z:.3}) world={position:?} seconds={seconds:.3}"
            );
        }
        if seconds <= 0.0 {
            self.hooks.push_camera_move(position);
            return Ok(());
        }
        self.hooks.push_camera_move_to(CameraMoveToRequest {
            position,
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn move_camera_along_waypoint_path(
        &self,
        waypoint_path: &str,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_path_move(CameraPathRequest {
            waypoint: waypoint_path.to_string(),
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn move_camera_to_selection(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_move_to_selection();
        Ok(())
    }

    fn camera_move_home(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_move_home();
        Ok(())
    }

    fn is_camera_movement_finished(&self) -> bool {
        self.hooks.is_camera_movement_finished()
    }

    fn camera_follow_object(
        &self,
        object_id: gamelogic::common::ObjectID,
        snap_to_unit: bool,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_follow(CameraFollowRequest {
            object_id,
            snap_to_unit,
        });
        Ok(())
    }

    fn camera_tether_object(
        &self,
        object_id: gamelogic::common::ObjectID,
        snap_to_unit: bool,
        play: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_tether(CameraTetherRequest {
            object_id,
            snap_to_unit,
            play,
        });
        Ok(())
    }

    fn stop_camera_follow(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_follow(CameraFollowRequest {
            object_id: 0,
            snap_to_unit: false,
        });
        Ok(())
    }

    fn reset_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        duration_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_reset(CameraResetRequest {
            position: camera_coord3d_to_world(x, y, z),
            duration_seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn set_camera_zoom(&self, zoom: f32, duration_seconds: f32) -> GameLogicResult<()> {
        self.hooks.push_camera_zoom(CameraZoomRequest {
            zoom,
            duration_seconds,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
        Ok(())
    }

    fn zoom_camera(
        &self,
        zoom: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_zoom(CameraZoomRequest {
            zoom,
            duration_seconds: seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn set_camera_pitch(
        &self,
        pitch: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_pitch(CameraPitchRequest {
            pitch,
            duration_seconds: seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn rotate_camera(
        &self,
        rotations: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_rotate(CameraRotateRequest {
            rotations,
            duration_seconds: seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn camera_mod_set_final_zoom(
        &self,
        zoom: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_zoom(CameraModFinalZoomRequest {
                zoom,
                ease_in,
                ease_out,
            });
        Ok(())
    }

    fn camera_mod_set_final_pitch(
        &self,
        pitch: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_pitch(CameraModFinalPitchRequest {
                pitch,
                ease_in,
                ease_out,
            });
        Ok(())
    }

    fn camera_mod_freeze_time(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_mod_freeze_time();
        Ok(())
    }

    fn camera_mod_freeze_angle(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_mod_freeze_angle();
        Ok(())
    }

    fn camera_mod_set_final_speed_multiplier(&self, multiplier: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_speed_multiplier(CameraModFinalSpeedMultiplierRequest {
                multiplier,
            });
        Ok(())
    }

    fn camera_mod_set_rolling_average(&self, frames: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_rolling_average(CameraModRollingAverageRequest { frames });
        Ok(())
    }

    fn set_visual_speed_multiplier(&self, multiplier: i32) -> GameLogicResult<()> {
        self.hooks
            .push_visual_speed_multiplier(VisualSpeedMultiplierRequest { multiplier });
        Ok(())
    }

    fn freeze_time(&self) -> GameLogicResult<()> {
        self.hooks.push_script_freeze_time(true);
        Ok(())
    }

    fn unfreeze_time(&self) -> GameLogicResult<()> {
        self.hooks.push_script_freeze_time(false);
        Ok(())
    }

    fn set_fps_limit(&self, fps: i32) -> GameLogicResult<()> {
        self.hooks.push_set_fps_limit(SetFpsLimitRequest { fps });
        Ok(())
    }

    fn popup_message(
        &self,
        message: &str,
        x_percent: i32,
        y_percent: i32,
        width: i32,
        pause: bool,
        pause_music: bool,
    ) -> GameLogicResult<()> {
        self.hooks.push_popup_message(ScriptPopupMessageRequest {
            message: message.to_string(),
            x_percent,
            y_percent,
            width,
            pause,
            pause_music,
            popup_generation: 0,
        });
        Ok(())
    }

    fn resize_view_guardband(&self, gbx: f32, gby: f32) -> GameLogicResult<()> {
        self.hooks.push_view_guardband(ViewGuardbandRequest {
            x_bias: gbx,
            y_bias: gby,
        });
        Ok(())
    }

    fn set_camera_bw_mode(&self, enabled: bool, frames: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_bw_mode(CameraBwModeRequest { enabled, frames });
        Ok(())
    }

    fn set_skybox_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        self.hooks.push_skybox_enabled(enabled);
        Ok(())
    }

    fn camera_motion_blur(&self, zoom_in: bool, saturate: bool) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::Basic { zoom_in, saturate });
        Ok(())
    }

    fn camera_motion_blur_jump(
        &self,
        x: f32,
        y: f32,
        z: f32,
        saturate: bool,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::Jump {
                position: camera_coord3d_to_world(x, y, z),
                saturate,
            });
        Ok(())
    }

    fn camera_motion_blur_follow(&self, amount: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::Follow { amount });
        Ok(())
    }

    fn camera_motion_blur_end_follow(&self) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::EndFollow);
        Ok(())
    }

    fn cameo_flash(&self, command_button_name: &str, flash_count: i32) -> GameLogicResult<()> {
        self.hooks.push_cameo_flash(CameoFlashRequest {
            command_button_name: command_button_name.to_string(),
            flash_count: flash_count.max(0),
        });
        Ok(())
    }

    fn add_named_timer(&self, name: &str, text: &str, countdown: bool) -> GameLogicResult<()> {
        self.hooks
            .push_named_timer_mutation(NamedTimerMutation::Add {
                name: name.to_string(),
                text: text.to_string(),
                countdown,
            });
        Ok(())
    }

    fn remove_named_timer(&self, name: &str) -> GameLogicResult<()> {
        self.hooks
            .push_named_timer_mutation(NamedTimerMutation::Remove {
                name: name.to_string(),
            });
        Ok(())
    }

    fn show_named_timer_display(&self, show: bool) -> GameLogicResult<()> {
        self.hooks.push_named_timer_display(show);
        Ok(())
    }

    fn set_superweapon_display_enabled_by_script(&self, enabled: bool) -> GameLogicResult<()> {
        self.hooks.push_superweapon_display_enabled(enabled);
        Ok(())
    }

    fn hide_object_superweapon_display_by_script(
        &self,
        object_id: gamelogic::common::ObjectID,
    ) -> GameLogicResult<()> {
        self.hooks.push_superweapon_object_display_mutation(
            SuperweaponObjectDisplayMutation::Hide { object_id },
        );
        Ok(())
    }

    fn show_object_superweapon_display_by_script(
        &self,
        object_id: gamelogic::common::ObjectID,
    ) -> GameLogicResult<()> {
        self.hooks.push_superweapon_object_display_mutation(
            SuperweaponObjectDisplayMutation::Show { object_id },
        );
        Ok(())
    }

    fn pause_named_special_power_countdown(
        &self,
        unit_name: &str,
        power_name: &str,
        pause: bool,
    ) -> GameLogicResult<()> {
        self.hooks.push_named_special_power_countdown_mutation(
            NamedSpecialPowerCountdownMutation {
                unit_name: unit_name.to_string(),
                power_name: power_name.to_string(),
                op: if pause {
                    crate::game_logic::NamedSpecialPowerCountdownOp::Stop
                } else {
                    crate::game_logic::NamedSpecialPowerCountdownOp::Start
                },
                seconds: 0,
            },
        );
        Ok(())
    }

    fn set_named_special_power_countdown(
        &self,
        unit_name: &str,
        power_name: &str,
        seconds: i32,
    ) -> GameLogicResult<()> {
        self.hooks.push_named_special_power_countdown_mutation(
            NamedSpecialPowerCountdownMutation {
                unit_name: unit_name.to_string(),
                power_name: power_name.to_string(),
                op: crate::game_logic::NamedSpecialPowerCountdownOp::Set,
                seconds,
            },
        );
        Ok(())
    }

    fn add_named_special_power_countdown(
        &self,
        unit_name: &str,
        power_name: &str,
        seconds: i32,
    ) -> GameLogicResult<()> {
        self.hooks.push_named_special_power_countdown_mutation(
            NamedSpecialPowerCountdownMutation {
                unit_name: unit_name.to_string(),
                power_name: power_name.to_string(),
                op: crate::game_logic::NamedSpecialPowerCountdownOp::Add,
                seconds,
            },
        );
        Ok(())
    }

    fn setup_camera(
        &self,
        x: f32,
        y: f32,
        z: f32,
        zoom: f32,
        pitch: f32,
        look_toward_x: f32,
        look_toward_y: f32,
        look_toward_z: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_setup(CameraSetupRequest {
            position: camera_coord3d_to_world(x, y, z),
            zoom,
            pitch,
            look_toward: camera_coord3d_to_world(look_toward_x, look_toward_y, look_toward_z),
        });
        Ok(())
    }

    fn camera_look_toward_object(
        &self,
        object_id: gamelogic::common::ObjectID,
        seconds: f32,
        hold_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_look_toward_object(CameraLookTowardObjectRequest {
                object_id,
                duration_seconds: seconds,
                hold_seconds,
                ease_in_seconds,
                ease_out_seconds,
            });
        Ok(())
    }

    fn camera_look_toward_waypoint(
        &self,
        x: f32,
        y: f32,
        z: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
        reverse_rotation: bool,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_look_toward_waypoint(CameraLookTowardWaypointRequest {
                position: camera_coord3d_to_world(x, y, z),
                duration_seconds: seconds,
                ease_in_seconds,
                ease_out_seconds,
                reverse_rotation,
            });
        Ok(())
    }

    fn camera_mod_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_look_toward(CameraModLookTowardRequest {
                position: camera_coord3d_to_world(x, y, z),
            });
        Ok(())
    }

    fn camera_mod_final_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_look_toward(CameraModFinalLookTowardRequest {
                position: camera_coord3d_to_world(x, y, z),
            });
        Ok(())
    }

    fn camera_letterbox_begin(&self) -> GameLogicResult<()> {
        self.hooks.push_letterbox(true);
        Ok(())
    }

    fn camera_letterbox_end(&self) -> GameLogicResult<()> {
        self.hooks.push_letterbox(false);
        Ok(())
    }

    fn camera_set_default(&self, pitch: f32, angle: f32, max_height: f32) -> GameLogicResult<()> {
        self.hooks.push_camera_set_default(CameraSetDefaultRequest {
            pitch,
            angle,
            max_height,
        });
        Ok(())
    }

    fn camera_enable_slave_mode(
        &self,
        thing_template_name: &str,
        bone_name: &str,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_slave_mode_enable(CameraSlaveModeRequest {
                thing_template_name: thing_template_name.to_string(),
                bone_name: bone_name.to_string(),
            });
        Ok(())
    }

    fn camera_disable_slave_mode(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_slave_mode_disable();
        Ok(())
    }

    fn screen_shake(&self, intensity: i32) -> GameLogicResult<()> {
        self.hooks
            .push_screen_shake(ScreenShakeRequest { intensity });
        Ok(())
    }

    fn camera_add_shaker_at(
        &self,
        x: f32,
        y: f32,
        z: f32,
        amplitude: f32,
        duration_seconds: f32,
        radius: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_add_shaker(CameraAddShakerRequest {
            position: camera_coord3d_to_world(x, y, z),
            amplitude,
            duration_seconds,
            radius,
        });
        Ok(())
    }

    fn movie_play_fullscreen(&self, filename: &str) -> GameLogicResult<()> {
        self.hooks.push_movie_request(filename.to_string());
        Ok(())
    }

    fn movie_play_radar(&self, filename: &str) -> GameLogicResult<()> {
        self.hooks.push_radar_movie_request(filename.to_string());
        Ok(())
    }

    fn is_video_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_video_complete(name, flush)
    }

    fn speech_play(&self, name: &str, allow_overlap: bool) -> GameLogicResult<()> {
        // Live GAME_SHELL installs this handler (initialize_scripts), not
        // GameClientScriptActionHandler. C++ SPEECH_PLAY always reaches
        // TheAudio via doSpeechPlay — do not leave this as a UI SFX.
        let handle = Self::play_speech_through_the_audio(name, allow_overlap);
        self.hooks.note_speech_started_with_handle(name, handle);
        if let Some(label) = speech_subtitle_label_if_displayable(name, localization::translate) {
            self.hooks
                .push_military_caption(label, SPEECH_SUBTITLE_DURATION_MS);
        }
        Ok(())
    }

    fn sound_play_named(&self, sound: &str, unit_name: &str) -> GameLogicResult<()> {
        let Some(object_id) =
            gamelogic::scripting::host_script_named_unit_id(unit_name).or_else(|| {
                gamelogic::scripting::engine::get_named_object_tracker()
                    .get_object_id(unit_name)
                    .ok()
                    .flatten()
            })
        else {
            return Ok(());
        };
        let handle = Self::play_named_sound_through_the_audio(sound, object_id);
        self.hooks.note_audio_started(sound);
        let _ = handle;
        Ok(())
    }

    fn enable_object_sound(&self, _unit_name: &str, _enable: bool) -> GameLogicResult<()> {
        // Leftover dispatcher already queued HostScriptObjectSoundRequest.
        Ok(())
    }

    fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_speech_complete(name, flush)
    }

    fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_audio_complete(name, flush)
    }

    fn music_set_track(&self, track: &str, fade_out: bool, fade_in: bool) -> GameLogicResult<()> {
        // Live GAME_SHELL installs this handler (initialize_scripts), not
        // GameClientScriptActionHandler. C++ MUSIC_SET_TRACK always reaches
        // TheAudio via doMusicTrackChange — do not leave this as a UI note.
        Self::play_music_track_through_the_audio(track, fade_out, fade_in);
        self.hooks.note_music_started(track);
        // C++ doMusicTrackChange: TheAudio only — no InGameUI / broadcast.
        Ok(())
    }

    fn has_music_track_completed(&self, track: &str, param: i32) -> bool {
        self.hooks.has_music_track_completed(track, param)
    }

    fn stop_music(&self) -> GameLogicResult<()> {
        const AHSV_STOP_THE_MUSIC_FADE: u32 = 0xFFFF_FFF1;
        if let Some(audio) = gamelogic::helpers::TheAudio::get() {
            audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
        }
        self.hooks.mark_music_stopped();
        self.hooks.push_music_stop();
        Ok(())
    }

    fn set_radar_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        self.hooks.push_radar_enabled(enabled);
        Ok(())
    }

    fn set_radar_forced(&self, forced: bool) -> GameLogicResult<()> {
        self.hooks.push_radar_forced(forced);
        Ok(())
    }

    fn create_radar_event(&self, x: f32, y: f32, z: f32, event_type: i32) -> GameLogicResult<()> {
        self.hooks
            .push_radar_event_request(RadarScriptEventRequest {
                position: Vec3::new(x, y, z),
                event_type,
            });
        Ok(())
    }

    fn set_weather_visible(&self, visible: bool) -> GameLogicResult<()> {
        self.hooks.push_weather_visible(visible);
        Ok(())
    }

    fn set_objective(&self, name: &str, description: &str, completed: bool) -> GameLogicResult<()> {
        self.hooks.push_objective_update(ObjectiveUpdate {
            name: name.to_string(),
            description: description.to_string(),
            completed,
        });
        Ok(())
    }

    fn spawn_effect(&self, effect_type: &str, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        // Generals Coord3D: x/y on map plane, z height. Main uses x/z plane.
        let position = camera_coord3d_to_world(x, y, z);
        self.hooks.push_effect_request(ScriptEffectRequest {
            effect_type: effect_type.to_string(),
            position,
        });
        Ok(())
    }

    fn set_campaign_victorious(&self, victorious: bool) -> GameLogicResult<()> {
        game_client::gui::campaign_manager::get_campaign_manager().set_victorious(victorious);
        Ok(())
    }

    fn create_win_lose_window(&self, layout_filename: &str) -> GameLogicResult<()> {
        // C++ ScriptActions.cpp:201/204/225/228/247 TheWindowManager->winCreateFromScript.
        // Live host initialize_scripts overwrites GameClientScriptActionHandler; the
        // trait default is a no-op, so forward to the GameClient load_window path.
        game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .create_win_lose_window(layout_filename)
    }

    fn destroy_win_lose_window(&self) -> GameLogicResult<()> {
        // C++ ScriptActions.cpp:160-162 TheWindowManager->winDestroy(m_messageWindow).
        game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .destroy_win_lose_window()
    }

    fn close_game_windows(&self) -> GameLogicResult<()> {
        // C++ GameLogic::closeWindows GameLogicDispatch.cpp:202-219.
        game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .close_game_windows()
    }

    fn set_warehouse_value(&self, warehouse_name: &str, cash_value: i32) -> GameLogicResult<()> {
        crate::game_logic::host_supply_gather::queue_warehouse_set_value(
            warehouse_name,
            cash_value,
        );
        Ok(())
    }
}
