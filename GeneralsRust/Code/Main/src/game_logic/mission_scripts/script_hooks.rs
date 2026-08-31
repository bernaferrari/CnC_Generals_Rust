// C++ ownership: ScriptEngine.cpp host notification queues — push/drain seams and completion tracking for every scripted side effect.

pub struct MissionScriptHooks {
    runtime: Mutex<MissionScriptRuntime>,
    pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    messages: Mutex<Vec<String>>,
    sounds: Mutex<Vec<String>>,
    sound_events: Mutex<Vec<ScriptSoundEvent>>,
    camera_moves: Mutex<Vec<Vec3>>,
    camera_follows: Mutex<Vec<CameraFollowRequest>>,
    camera_tethers: Mutex<Vec<CameraTetherRequest>>,
    camera_path_moves: Mutex<Vec<CameraPathRequest>>,
    camera_move_to: Mutex<Vec<CameraMoveToRequest>>,
    camera_move_to_selection_requests: Mutex<Vec<()>>,
    camera_move_home_requests: Mutex<Vec<()>>,
    camera_resets: Mutex<Vec<CameraResetRequest>>,
    camera_zoom_requests: Mutex<Vec<CameraZoomRequest>>,
    camera_pitch_requests: Mutex<Vec<CameraPitchRequest>>,
    camera_rotate_requests: Mutex<Vec<CameraRotateRequest>>,
    camera_mod_final_zoom_requests: Mutex<Vec<CameraModFinalZoomRequest>>,
    camera_mod_final_pitch_requests: Mutex<Vec<CameraModFinalPitchRequest>>,
    camera_mod_freeze_time_requests: Mutex<Vec<()>>,
    camera_mod_freeze_angle_requests: Mutex<Vec<()>>,
    camera_mod_final_speed_multiplier_requests: Mutex<Vec<CameraModFinalSpeedMultiplierRequest>>,
    camera_mod_rolling_average_requests: Mutex<Vec<CameraModRollingAverageRequest>>,
    visual_speed_multiplier_requests: Mutex<Vec<VisualSpeedMultiplierRequest>>,
    script_freeze_time_requests: Mutex<Vec<bool>>,
    set_fps_limit_requests: Mutex<Vec<SetFpsLimitRequest>>,
    camera_setup_requests: Mutex<Vec<CameraSetupRequest>>,
    camera_look_toward_object_requests: Mutex<Vec<CameraLookTowardObjectRequest>>,
    camera_look_toward_waypoint_requests: Mutex<Vec<CameraLookTowardWaypointRequest>>,
    camera_mod_look_toward_requests: Mutex<Vec<CameraModLookTowardRequest>>,
    camera_mod_final_look_toward_requests: Mutex<Vec<CameraModFinalLookTowardRequest>>,
    camera_set_default_requests: Mutex<Vec<CameraSetDefaultRequest>>,
    camera_slave_mode_enable_requests: Mutex<Vec<CameraSlaveModeRequest>>,
    camera_slave_mode_disable_requests: Mutex<Vec<()>>,
    screen_shake_requests: Mutex<Vec<ScreenShakeRequest>>,
    camera_add_shaker_requests: Mutex<Vec<CameraAddShakerRequest>>,
    named_special_power_countdown_mutations: Mutex<Vec<NamedSpecialPowerCountdownMutation>>,

    popup_message_requests: Mutex<Vec<ScriptPopupMessageRequest>>,
    view_guardband_requests: Mutex<Vec<ViewGuardbandRequest>>,
    camera_bw_mode_requests: Mutex<Vec<CameraBwModeRequest>>,
    skybox_enabled_updates: Mutex<Vec<bool>>,
    camera_motion_blur_requests: Mutex<Vec<CameraMotionBlurRequest>>,
    cameo_flash_requests: Mutex<Vec<CameoFlashRequest>>,
    named_timer_mutations: Mutex<Vec<NamedTimerMutation>>,
    named_timer_display_updates: Mutex<Vec<bool>>,
    superweapon_display_enabled_updates: Mutex<Vec<bool>>,
    superweapon_object_display_mutations: Mutex<Vec<SuperweaponObjectDisplayMutation>>,
    cinematic_text: Mutex<Vec<(String, String, i32)>>,
    military_captions: Mutex<Vec<MilitaryCaptionRequest>>,
    letterbox_events: Mutex<Vec<bool>>,
    movie_requests: Mutex<Vec<String>>,
    radar_movie_requests: Mutex<Vec<String>>,
    objective_updates: Mutex<Vec<ObjectiveUpdate>>,
    effect_requests: Mutex<Vec<ScriptEffectRequest>>,
    radar_event_requests: Mutex<Vec<RadarScriptEventRequest>>,
    radar_enabled_updates: Mutex<Vec<bool>>,
    radar_forced_updates: Mutex<Vec<bool>>,
    weather_visibility_updates: Mutex<Vec<bool>>,
    music_stop_requests: Mutex<Vec<()>>,
    oversize_terrain_requests: Mutex<Vec<i32>>,
    border_shroud_levels: Mutex<Vec<u8>>,
    camera_movement_finished: AtomicBool,
    frame_counter: AtomicU64,
    speech_complete_frame: Mutex<HashMap<String, u64>>,
    speech_handles: Mutex<HashMap<String, Vec<u32>>>,
    audio_complete_frame: Mutex<HashMap<String, u64>>,
}

impl MissionScriptHooks {
    pub fn new() -> GameLogicResult<Arc<Self>> {
        let pending_script_enabled_updates = Arc::new(Mutex::new(Vec::new()));
        Ok(Arc::new(Self {
            runtime: Mutex::new(
                MissionScriptRuntime::new_with_pending_script_enabled_updates(Arc::clone(
                    &pending_script_enabled_updates,
                ))?,
            ),
            pending_script_enabled_updates,
            messages: Mutex::new(Vec::new()),
            sounds: Mutex::new(Vec::new()),
            sound_events: Mutex::new(Vec::new()),
            camera_moves: Mutex::new(Vec::new()),
            camera_follows: Mutex::new(Vec::new()),
            camera_tethers: Mutex::new(Vec::new()),
            camera_path_moves: Mutex::new(Vec::new()),
            camera_move_to: Mutex::new(Vec::new()),
            camera_move_to_selection_requests: Mutex::new(Vec::new()),
            camera_move_home_requests: Mutex::new(Vec::new()),
            camera_resets: Mutex::new(Vec::new()),
            camera_zoom_requests: Mutex::new(Vec::new()),
            camera_pitch_requests: Mutex::new(Vec::new()),
            camera_rotate_requests: Mutex::new(Vec::new()),
            camera_mod_final_zoom_requests: Mutex::new(Vec::new()),
            camera_mod_final_pitch_requests: Mutex::new(Vec::new()),
            camera_mod_freeze_time_requests: Mutex::new(Vec::new()),
            camera_mod_freeze_angle_requests: Mutex::new(Vec::new()),
            camera_mod_final_speed_multiplier_requests: Mutex::new(Vec::new()),
            camera_mod_rolling_average_requests: Mutex::new(Vec::new()),
            visual_speed_multiplier_requests: Mutex::new(Vec::new()),
            script_freeze_time_requests: Mutex::new(Vec::new()),
            set_fps_limit_requests: Mutex::new(Vec::new()),
            camera_setup_requests: Mutex::new(Vec::new()),
            camera_look_toward_object_requests: Mutex::new(Vec::new()),
            camera_look_toward_waypoint_requests: Mutex::new(Vec::new()),
            camera_mod_look_toward_requests: Mutex::new(Vec::new()),
            camera_mod_final_look_toward_requests: Mutex::new(Vec::new()),
            camera_set_default_requests: Mutex::new(Vec::new()),
            camera_slave_mode_enable_requests: Mutex::new(Vec::new()),
            camera_slave_mode_disable_requests: Mutex::new(Vec::new()),
            screen_shake_requests: Mutex::new(Vec::new()),
            camera_add_shaker_requests: Mutex::new(Vec::new()),
            named_special_power_countdown_mutations: Mutex::new(Vec::new()),

            popup_message_requests: Mutex::new(Vec::new()),
            view_guardband_requests: Mutex::new(Vec::new()),
            camera_bw_mode_requests: Mutex::new(Vec::new()),
            skybox_enabled_updates: Mutex::new(Vec::new()),
            camera_motion_blur_requests: Mutex::new(Vec::new()),
            cameo_flash_requests: Mutex::new(Vec::new()),
            named_timer_mutations: Mutex::new(Vec::new()),
            named_timer_display_updates: Mutex::new(Vec::new()),
            superweapon_display_enabled_updates: Mutex::new(Vec::new()),
            superweapon_object_display_mutations: Mutex::new(Vec::new()),
            cinematic_text: Mutex::new(Vec::new()),
            military_captions: Mutex::new(Vec::new()),
            letterbox_events: Mutex::new(Vec::new()),
            movie_requests: Mutex::new(Vec::new()),
            radar_movie_requests: Mutex::new(Vec::new()),
            objective_updates: Mutex::new(Vec::new()),
            effect_requests: Mutex::new(Vec::new()),
            radar_event_requests: Mutex::new(Vec::new()),
            radar_enabled_updates: Mutex::new(Vec::new()),
            radar_forced_updates: Mutex::new(Vec::new()),
            weather_visibility_updates: Mutex::new(Vec::new()),
            music_stop_requests: Mutex::new(Vec::new()),
            oversize_terrain_requests: Mutex::new(Vec::new()),
            border_shroud_levels: Mutex::new(Vec::new()),
            camera_movement_finished: AtomicBool::new(true),
            frame_counter: AtomicU64::new(0),
            speech_complete_frame: Mutex::new(HashMap::new()),
            speech_handles: Mutex::new(HashMap::new()),
            audio_complete_frame: Mutex::new(HashMap::new()),
        }))
    }

    pub fn install_lists(&self, lists: &[ScriptList]) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.install_lists(lists);
        }
    }

    /// C++ `ScriptEngine::newMap` fade-in from black (33-frame `FADE_MULTIPLY`).
    /// Live map load calls this after leftover `reset()` so the overlay starts
    /// even when the crate engine handle is taken out for `update()`.
    pub fn start_new_map_fade(&self) {
        if let Ok(mut engine_guard) = gamelogic::scripting::engine::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.new_map();
            }
        }
    }

    /// Advance hook completion clocks without walking scripts.
    ///
    /// C++ GameLogic.cpp:3600 has one `TheScriptEngine->UPDATE()` per logic
    /// frame.  Live host evaluation is crate `ScriptEngine::update`; this only
    /// stamps `frame_counter` so video/speech/audio/music completion queries
    /// stay frame-accurate after the second walker was removed (hq-fxq1).
    pub fn note_logic_frame(&self, frame: u64) {
        self.frame_counter.store(frame, Ordering::Relaxed);
    }

    pub fn update(&self, frame: u64) -> GameLogicResult<()> {
        self.update_budgeted(frame, None)
    }

    pub fn update_budgeted(
        &self,
        frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.frame_counter.store(frame, Ordering::Relaxed);
        let mut runtime = self.runtime.lock().map_err(|_| {
            GameLogicError::Configuration("Mission script runtime mutex poisoned".to_string())
        })?;
        runtime.update_budgeted(frame, max_scripts_per_frame)?;
        Ok(())
    }

    pub fn set_script_enabled(&self, name: &str, enabled: bool) -> GameLogicResult<()> {
        let mut queue = self.pending_script_enabled_updates.lock().map_err(|_| {
            GameLogicError::Configuration("Mission script enable queue mutex poisoned".to_string())
        })?;
        queue.push((name.to_string(), enabled));
        Ok(())
    }

    pub fn push_message(&self, text: String) {
        if let Ok(mut queue) = self.messages.lock() {
            let localized = localization::localize_with_args(
                "hud.script.broadcast",
                "Transmission: {message}",
                &[("message", text.as_str())],
            );
            queue.push(localized);
        }
    }

    pub fn push_sound(&self, name: String) {
        if let Ok(mut queue) = self.sounds.lock() {
            queue.push(name);
        }
    }

    pub fn push_sound_event(&self, event: ScriptSoundEvent) {
        if let Ok(mut queue) = self.sound_events.lock() {
            queue.push(event);
        }
    }

    pub fn push_camera_move(&self, position: Vec3) {
        if let Ok(mut queue) = self.camera_moves.lock() {
            queue.push(position);
        }
    }

    pub fn push_camera_tether(&self, request: CameraTetherRequest) {
        if let Ok(mut queue) = self.camera_tethers.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_follow(&self, request: CameraFollowRequest) {
        if let Ok(mut queue) = self.camera_follows.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_path_move(&self, request: CameraPathRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_path_moves.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_move_to(&self, request: CameraMoveToRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_move_to.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_move_to_selection(&self) {
        if let Ok(mut queue) = self.camera_move_to_selection_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_move_home(&self) {
        if let Ok(mut queue) = self.camera_move_home_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_reset(&self, request: CameraResetRequest) {
        if let Ok(mut queue) = self.camera_resets.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_zoom(&self, request: CameraZoomRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_zoom_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_pitch(&self, request: CameraPitchRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_pitch_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_rotate(&self, request: CameraRotateRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_rotate_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_final_zoom(&self, request: CameraModFinalZoomRequest) {
        if let Ok(mut queue) = self.camera_mod_final_zoom_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_final_pitch(&self, request: CameraModFinalPitchRequest) {
        if let Ok(mut queue) = self.camera_mod_final_pitch_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_freeze_time(&self) {
        if let Ok(mut queue) = self.camera_mod_freeze_time_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_mod_freeze_angle(&self) {
        if let Ok(mut queue) = self.camera_mod_freeze_angle_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_mod_final_speed_multiplier(
        &self,
        request: CameraModFinalSpeedMultiplierRequest,
    ) {
        if let Ok(mut queue) = self.camera_mod_final_speed_multiplier_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_rolling_average(&self, request: CameraModRollingAverageRequest) {
        if let Ok(mut queue) = self.camera_mod_rolling_average_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_visual_speed_multiplier(&self, request: VisualSpeedMultiplierRequest) {
        if let Ok(mut queue) = self.visual_speed_multiplier_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_script_freeze_time(&self, freeze: bool) {
        if let Ok(mut queue) = self.script_freeze_time_requests.lock() {
            queue.push(freeze);
        }
    }

    pub fn push_set_fps_limit(&self, request: SetFpsLimitRequest) {
        if let Ok(mut queue) = self.set_fps_limit_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_setup(&self, request: CameraSetupRequest) {
        if let Ok(mut queue) = self.camera_setup_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_look_toward_object(&self, request: CameraLookTowardObjectRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_look_toward_object_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_look_toward_waypoint(&self, request: CameraLookTowardWaypointRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_look_toward_waypoint_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_look_toward(&self, request: CameraModLookTowardRequest) {
        if let Ok(mut queue) = self.camera_mod_look_toward_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_final_look_toward(&self, request: CameraModFinalLookTowardRequest) {
        if let Ok(mut queue) = self.camera_mod_final_look_toward_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_set_default(&self, request: CameraSetDefaultRequest) {
        if let Ok(mut queue) = self.camera_set_default_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_slave_mode_enable(&self, request: CameraSlaveModeRequest) {
        if let Ok(mut queue) = self.camera_slave_mode_enable_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_slave_mode_disable(&self) {
        if let Ok(mut queue) = self.camera_slave_mode_disable_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_screen_shake(&self, request: ScreenShakeRequest) {
        if let Ok(mut queue) = self.screen_shake_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_add_shaker(&self, request: CameraAddShakerRequest) {
        if let Ok(mut queue) = self.camera_add_shaker_requests.lock() {
            queue.push(request);
        }
    }

    pub fn set_camera_movement_finished(&self, finished: bool) {
        self.camera_movement_finished
            .store(finished, Ordering::Relaxed);
    }

    pub fn is_camera_movement_finished(&self) -> bool {
        self.camera_movement_finished.load(Ordering::Relaxed)
    }

    pub fn push_cinematic_text(&self, text: String, font: String, duration_seconds: i32) {
        if let Ok(mut queue) = self.cinematic_text.lock() {
            queue.push((text, font, duration_seconds));
        }
    }

    pub fn push_military_caption(&self, text: String, duration_ms: i32) {
        if let Ok(mut queue) = self.military_captions.lock() {
            queue.push(MilitaryCaptionRequest { text, duration_ms });
        }
    }

    pub fn push_letterbox(&self, enabled: bool) {
        if let Ok(mut queue) = self.letterbox_events.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_movie_request(&self, filename: String) {
        if let Ok(mut queue) = self.movie_requests.lock() {
            queue.push(filename);
        }
    }

    pub fn push_radar_movie_request(&self, filename: String) {
        if let Ok(mut queue) = self.radar_movie_requests.lock() {
            queue.push(filename);
        }
    }

    pub fn push_objective_update(&self, update: ObjectiveUpdate) {
        if let Ok(mut queue) = self.objective_updates.lock() {
            queue.push(update);
        }
    }

    pub fn push_effect_request(&self, request: ScriptEffectRequest) {
        if let Ok(mut queue) = self.effect_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_radar_event_request(&self, request: RadarScriptEventRequest) {
        if let Ok(mut queue) = self.radar_event_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_radar_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.radar_enabled_updates.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_radar_forced(&self, forced: bool) {
        if let Ok(mut queue) = self.radar_forced_updates.lock() {
            queue.push(forced);
        }
    }

    pub fn push_weather_visible(&self, visible: bool) {
        if let Ok(mut queue) = self.weather_visibility_updates.lock() {
            queue.push(visible);
        }
    }

    pub fn push_popup_message(&self, mut request: ScriptPopupMessageRequest) {
        // Keep this opaque and monotonic rather than deriving authority from
        // popup text/layout fields. Acknowledge only the exact live instance.
        request.popup_generation = next_live_popup_generation();
        if let Ok(mut queue) = self.popup_message_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_view_guardband(&self, request: ViewGuardbandRequest) {
        if let Ok(mut queue) = self.view_guardband_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_bw_mode(&self, request: CameraBwModeRequest) {
        if let Ok(mut queue) = self.camera_bw_mode_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_skybox_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.skybox_enabled_updates.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_camera_motion_blur(&self, request: CameraMotionBlurRequest) {
        if let Ok(mut queue) = self.camera_motion_blur_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_cameo_flash(&self, request: CameoFlashRequest) {
        if let Ok(mut queue) = self.cameo_flash_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_named_timer_mutation(&self, request: NamedTimerMutation) {
        if let Ok(mut queue) = self.named_timer_mutations.lock() {
            queue.push(request);
        }
    }

    pub fn push_named_timer_display(&self, show: bool) {
        if let Ok(mut queue) = self.named_timer_display_updates.lock() {
            queue.push(show);
        }
    }

    pub fn push_superweapon_display_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.superweapon_display_enabled_updates.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_named_special_power_countdown_mutation(
        &self,
        request: NamedSpecialPowerCountdownMutation,
    ) {
        if let Ok(mut queue) = self.named_special_power_countdown_mutations.lock() {
            queue.push(request);
        }
    }

    pub fn push_superweapon_object_display_mutation(
        &self,
        request: SuperweaponObjectDisplayMutation,
    ) {
        if let Ok(mut queue) = self.superweapon_object_display_mutations.lock() {
            queue.push(request);
        }
    }

    pub fn push_music_stop(&self) {
        if let Ok(mut queue) = self.music_stop_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_oversize_terrain(&self, amount: i32) {
        if let Ok(mut queue) = self.oversize_terrain_requests.lock() {
            queue.push(amount);
        }
    }

    pub fn note_speech_started(&self, name: &str) {
        self.note_speech_started_with_handle(name, 0);
    }

    pub fn note_speech_started_with_handle(&self, name: &str, handle: u32) {
        if name.trim().is_empty() {
            return;
        }
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.speech_complete_frame.lock() {
            map.insert(name.to_string(), speech_completion_frame(now, name));
        }
        if handle != 0 {
            if let Ok(mut handles) = self.speech_handles.lock() {
                handles.entry(name.to_string()).or_default().push(handle);
            }
        }
    }

    pub fn note_audio_started(&self, name: &str) {
        // C++ isAudioComplete starts the TheAudio length timer on first query,
        // not on play. Do not stamp now+1 (that made HAS_FINISHED_AUDIO true
        // next frame).
        let _ = name;
    }

    pub fn note_music_started(&self, name: &str) {
        // C++ MUSIC_TRACK_HAS_COMPLETED is TheAudio loop count, not a frame stamp.
        let _ = name;
    }

    pub fn mark_music_stopped(&self) {
        // C++ stop-music does not mark hasMusicTrackCompleted; Miles walks
        // playing streams only. Stopping a track makes the condition false.
    }

    pub fn is_video_complete(&self, name: &str, flush: bool) -> bool {
        // C++ ScriptEngine::isVideoComplete: true only if name is on
        // m_completedVideo. Untracked / never-finished names stay false.
        gamelogic::scripting::engine::with_script_engine_ref(|engine| {
            engine.is_video_complete(name, flush)
        })
        .unwrap_or(false)
    }

    pub fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        if name.trim().is_empty() {
            return false;
        }
        // Leftover GameClient `is_named_audio_complete`: a live Miles/rodio
        // handle is still playing, so the line is not finished yet.
        if let Ok(mut handles) = self.speech_handles.lock() {
            if let Some(pending) = handles.get_mut(name) {
                match gamelogic::helpers::TheAudio::get() {
                    Some(audio) => pending.retain(|handle| audio.is_currently_playing(*handle)),
                    None => pending.clear(),
                }
                if !pending.is_empty() {
                    return false;
                }
                if flush {
                    handles.remove(name);
                }
            }
        }
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.speech_complete_frame.lock() else {
            return true;
        };
        let done_frame = match map.get(name).copied() {
            Some(done_frame) => done_frame,
            None => {
                // C++ first HAS_FINISHED_SPEECH query starts the TheAudio timer.
                let done_frame = speech_completion_frame(now, name);
                map.insert(name.to_string(), done_frame);
                done_frame
            }
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(name);
        }
        done
    }

    pub fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        if name.trim().is_empty() {
            return false;
        }
        // C++ ScriptEngine::isAudioComplete: first query starts leftover
        // TheAudio length timer; true only after that frame. Use the live
        // frame clock — leftover TheGameLogic::get_frame is not the host.
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.audio_complete_frame.lock() else {
            return false;
        };
        let done_frame = match map.get(name).copied() {
            Some(done_frame) => done_frame,
            None => {
                let done_frame = speech_completion_frame(now, name);
                map.insert(name.to_string(), done_frame);
                done_frame
            }
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(name);
        }
        done
    }

    pub fn has_music_track_completed(&self, track: &str, times: i32) -> bool {
        let key = track.trim();
        if key.is_empty() {
            return false;
        }
        // C++ TheAudio->hasMusicTrackCompleted(track, N). Unplayed / missing = false.
        gamelogic::helpers::TheAudio::get()
            .map(|audio| audio.has_music_track_completed(key, times))
            .unwrap_or(false)
    }

    pub fn drain_messages(&self) -> Vec<String> {
        self.messages
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_sounds(&self) -> Vec<String> {
        self.sounds
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_sound_events(&self) -> Vec<ScriptSoundEvent> {
        self.sound_events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_moves(&self) -> Vec<Vec3> {
        self.camera_moves
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_follows(&self) -> Vec<CameraFollowRequest> {
        self.camera_follows
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_tethers(&self) -> Vec<CameraTetherRequest> {
        self.camera_tethers
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_path_moves(&self) -> Vec<CameraPathRequest> {
        self.camera_path_moves
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_to(&self) -> Vec<CameraMoveToRequest> {
        self.camera_move_to
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_to_selection_requests(&self) -> Vec<()> {
        self.camera_move_to_selection_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_home_requests(&self) -> Vec<()> {
        self.camera_move_home_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_resets(&self) -> Vec<CameraResetRequest> {
        self.camera_resets
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_zoom_requests(&self) -> Vec<CameraZoomRequest> {
        self.camera_zoom_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_pitch_requests(&self) -> Vec<CameraPitchRequest> {
        self.camera_pitch_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_rotate_requests(&self) -> Vec<CameraRotateRequest> {
        self.camera_rotate_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_zoom_requests(&self) -> Vec<CameraModFinalZoomRequest> {
        self.camera_mod_final_zoom_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_pitch_requests(&self) -> Vec<CameraModFinalPitchRequest> {
        self.camera_mod_final_pitch_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_freeze_time_requests(&self) -> Vec<()> {
        self.camera_mod_freeze_time_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_freeze_angle_requests(&self) -> Vec<()> {
        self.camera_mod_freeze_angle_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_speed_multiplier_requests(
        &self,
    ) -> Vec<CameraModFinalSpeedMultiplierRequest> {
        self.camera_mod_final_speed_multiplier_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_rolling_average_requests(&self) -> Vec<CameraModRollingAverageRequest> {
        self.camera_mod_rolling_average_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_visual_speed_multiplier_requests(&self) -> Vec<VisualSpeedMultiplierRequest> {
        self.visual_speed_multiplier_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_script_freeze_time_requests(&self) -> Vec<bool> {
        self.script_freeze_time_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_set_fps_limit_requests(&self) -> Vec<SetFpsLimitRequest> {
        self.set_fps_limit_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_setup_requests(&self) -> Vec<CameraSetupRequest> {
        self.camera_setup_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_look_toward_object_requests(&self) -> Vec<CameraLookTowardObjectRequest> {
        self.camera_look_toward_object_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_look_toward_waypoint_requests(
        &self,
    ) -> Vec<CameraLookTowardWaypointRequest> {
        self.camera_look_toward_waypoint_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_look_toward_requests(&self) -> Vec<CameraModLookTowardRequest> {
        self.camera_mod_look_toward_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_look_toward_requests(
        &self,
    ) -> Vec<CameraModFinalLookTowardRequest> {
        self.camera_mod_final_look_toward_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_set_default_requests(&self) -> Vec<CameraSetDefaultRequest> {
        self.camera_set_default_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_slave_mode_enable_requests(&self) -> Vec<CameraSlaveModeRequest> {
        self.camera_slave_mode_enable_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_slave_mode_disable_requests(&self) -> Vec<()> {
        self.camera_slave_mode_disable_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_screen_shake_requests(&self) -> Vec<ScreenShakeRequest> {
        self.screen_shake_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_add_shaker_requests(&self) -> Vec<CameraAddShakerRequest> {
        self.camera_add_shaker_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_cinematic_text(&self) -> Vec<(String, String, i32)> {
        self.cinematic_text
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_military_captions(&self) -> Vec<MilitaryCaptionRequest> {
        self.military_captions
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_letterbox_events(&self) -> Vec<bool> {
        self.letterbox_events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_movie_requests(&self) -> Vec<String> {
        self.movie_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_movie_requests(&self) -> Vec<String> {
        self.radar_movie_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_objective_updates(&self) -> Vec<ObjectiveUpdate> {
        self.objective_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_effect_requests(&self) -> Vec<ScriptEffectRequest> {
        self.effect_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_event_requests(&self) -> Vec<RadarScriptEventRequest> {
        self.radar_event_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_enabled_updates(&self) -> Vec<bool> {
        self.radar_enabled_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_forced_updates(&self) -> Vec<bool> {
        self.radar_forced_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_weather_visibility_updates(&self) -> Vec<bool> {
        self.weather_visibility_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_popup_message_requests(&self) -> Vec<ScriptPopupMessageRequest> {
        self.popup_message_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_view_guardband_requests(&self) -> Vec<ViewGuardbandRequest> {
        self.view_guardband_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_bw_mode_requests(&self) -> Vec<CameraBwModeRequest> {
        self.camera_bw_mode_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_skybox_enabled_updates(&self) -> Vec<bool> {
        self.skybox_enabled_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_motion_blur_requests(&self) -> Vec<CameraMotionBlurRequest> {
        self.camera_motion_blur_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_cameo_flash_requests(&self) -> Vec<CameoFlashRequest> {
        self.cameo_flash_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_timer_mutations(&self) -> Vec<NamedTimerMutation> {
        self.named_timer_mutations
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_timer_display_updates(&self) -> Vec<bool> {
        self.named_timer_display_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_superweapon_display_enabled_updates(&self) -> Vec<bool> {
        self.superweapon_display_enabled_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_special_power_countdown_mutations(
        &self,
    ) -> Vec<NamedSpecialPowerCountdownMutation> {
        self.named_special_power_countdown_mutations
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_superweapon_object_display_mutations(
        &self,
    ) -> Vec<SuperweaponObjectDisplayMutation> {
        self.superweapon_object_display_mutations
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_music_stop_requests(&self) -> Vec<()> {
        self.music_stop_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn push_border_shroud_level(&self, level: u8) {
        if let Ok(mut queue) = self.border_shroud_levels.lock() {
            queue.push(level);
        }
    }

    pub fn drain_border_shroud_levels(&self) -> Vec<u8> {
        self.border_shroud_levels
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_oversize_terrain_requests(&self) -> Vec<i32> {
        self.oversize_terrain_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
}
