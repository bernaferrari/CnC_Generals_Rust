// C++ ownership: ScriptEngine.cpp host notification queues — push/drain seams and completion tracking for every scripted side effect.

/// Notification queues drained by the GameClient tick, in C++ order.
#[derive(Default)]
struct ScriptNotificationQueues {
    pending_warehouse_set_values: Vec<(String, i32)>,
    messages: Vec<String>,
    sounds: Vec<String>,
    sound_events: Vec<ScriptSoundEvent>,
    camera_moves: Vec<Vec3>,
    camera_follows: Vec<CameraFollowRequest>,
    camera_tethers: Vec<CameraTetherRequest>,
    camera_path_moves: Vec<CameraPathRequest>,
    camera_move_to: Vec<CameraMoveToRequest>,
    camera_move_to_selection_requests: Vec<()>,
    camera_move_home_requests: Vec<()>,
    camera_resets: Vec<CameraResetRequest>,
    camera_zoom_requests: Vec<CameraZoomRequest>,
    camera_pitch_requests: Vec<CameraPitchRequest>,
    camera_rotate_requests: Vec<CameraRotateRequest>,
    camera_mod_final_zoom_requests: Vec<CameraModFinalZoomRequest>,
    camera_mod_final_pitch_requests: Vec<CameraModFinalPitchRequest>,
    camera_mod_freeze_time_requests: Vec<()>,
    camera_mod_freeze_angle_requests: Vec<()>,
    camera_mod_final_speed_multiplier_requests: Vec<CameraModFinalSpeedMultiplierRequest>,
    camera_mod_rolling_average_requests: Vec<CameraModRollingAverageRequest>,
    visual_speed_multiplier_requests: Vec<VisualSpeedMultiplierRequest>,
    script_freeze_time_requests: Vec<bool>,
    set_fps_limit_requests: Vec<SetFpsLimitRequest>,
    camera_setup_requests: Vec<CameraSetupRequest>,
    camera_look_toward_object_requests: Vec<CameraLookTowardObjectRequest>,
    camera_look_toward_waypoint_requests: Vec<CameraLookTowardWaypointRequest>,
    camera_mod_look_toward_requests: Vec<CameraModLookTowardRequest>,
    camera_mod_final_look_toward_requests: Vec<CameraModFinalLookTowardRequest>,
    camera_set_default_requests: Vec<CameraSetDefaultRequest>,
    camera_slave_mode_enable_requests: Vec<CameraSlaveModeRequest>,
    camera_slave_mode_disable_requests: Vec<()>,
    screen_shake_requests: Vec<ScreenShakeRequest>,
    camera_add_shaker_requests: Vec<CameraAddShakerRequest>,
    named_special_power_countdown_mutations: Vec<NamedSpecialPowerCountdownMutation>,
    popup_message_requests: Vec<ScriptPopupMessageRequest>,
    view_guardband_requests: Vec<ViewGuardbandRequest>,
    camera_bw_mode_requests: Vec<CameraBwModeRequest>,
    skybox_enabled_updates: Vec<bool>,
    camera_motion_blur_requests: Vec<CameraMotionBlurRequest>,
    cameo_flash_requests: Vec<CameoFlashRequest>,
    named_timer_mutations: Vec<NamedTimerMutation>,
    named_timer_display_updates: Vec<bool>,
    superweapon_display_enabled_updates: Vec<bool>,
    superweapon_object_display_mutations: Vec<SuperweaponObjectDisplayMutation>,
    cinematic_text: Vec<(String, String, i32)>,
    military_captions: Vec<MilitaryCaptionRequest>,
    letterbox_events: Vec<bool>,
    movie_requests: Vec<String>,
    radar_movie_requests: Vec<String>,
    objective_updates: Vec<ObjectiveUpdate>,
    effect_requests: Vec<ScriptEffectRequest>,
    radar_event_requests: Vec<RadarScriptEventRequest>,
    radar_enabled_updates: Vec<bool>,
    radar_forced_updates: Vec<bool>,
    weather_visibility_updates: Vec<bool>,
    music_stop_requests: Vec<()>,
    oversize_terrain_requests: Vec<i32>,
    border_shroud_levels: Vec<u8>,
}

/// Completion maps keyed by audio name.
#[derive(Default)]
struct AudioCompletionTracking {
    speech_complete_frame: HashMap<String, u64>,
    speech_handles: HashMap<String, Vec<u32>>,
    audio_complete_frame: HashMap<String, u64>,
}

/// Notifications consumed together at one existing synchronous camera-focus boundary.
/// This value owns requests; no notification guard survives into camera/view effects.
#[derive(Default)]
pub(crate) struct CameraFocusRequests {
    pub(crate) moves: Vec<Vec3>,
    pub(crate) move_to_selection: Vec<()>,
    pub(crate) move_home: Vec<()>,
    pub(crate) follows: Vec<CameraFollowRequest>,
    pub(crate) tethers: Vec<CameraTetherRequest>,
}

pub struct MissionScriptHooks {
    runtime: Mutex<MissionScriptRuntime>,
    pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    // The action handler uses `&self` while the script runtime is locked;
    // defer this per-world value until the owning GameLogic tick drains it.
    /// Every script -> host notification queue behind a single guard.
    ///
    /// THREAD: both ends of these seams run on the game thread - the script
    /// runtime and its action handler push, the GameClient tick drains. The
    /// guard survives only because `MissionScriptHooks` is handed out as an
    /// `Arc` shared by `GameLogic` and `MissionScriptActionHandler`, not
    /// because two threads ever contend. One guard also makes a whole drain
    /// atomic against pushes instead of one mutex per queue.
    notifications: Mutex<ScriptNotificationQueues>,
    /// C++ ScriptEngine completion bookkeeping: speech/audio frame stamps
    /// and the live handles that still gate HAS_FINISHED_SPEECH.
    completion: Mutex<AudioCompletionTracking>,
    camera_movement_finished: AtomicBool,
    frame_counter: AtomicU64,
}

impl MissionScriptHooks {
    /// Test seam: the completion maps were folded into one guard, so tests
    /// reach them through the same lock the drain uses.
    #[cfg(test)]
    pub(crate) fn with_completion_tracking_for_test<R>(
        &self,
        f: impl FnOnce(&mut AudioCompletionTracking) -> R,
    ) -> R {
        let mut state = self.completion.lock().expect("completion tracking lock");
        f(&mut state)
    }

    pub fn queue_warehouse_set_value(&self, name: &str, cash: i32) {
        if name.is_empty() {
            return;
        }
        if let Ok(mut queue) = self.notifications.lock() {
            queue
                .pending_warehouse_set_values
                .push((name.to_string(), cash));
        }
    }

    pub fn drain_warehouse_set_values(&self) -> Vec<(String, i32)> {
        self.notifications
            .lock()
            .map(|mut queue| queue.pending_warehouse_set_values.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn clear_warehouse_set_values(&self) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.pending_warehouse_set_values.clear();
        }
    }

    pub fn new() -> GameLogicResult<Arc<Self>> {
        Self::new_with_host_trigger_world(Arc::new(Mutex::new(Default::default())))
    }

    pub(crate) fn new_with_host_trigger_world(
        host_trigger_world: Arc<Mutex<gamelogic::scripting::HostTriggerWorld>>,
    ) -> GameLogicResult<Arc<Self>> {
        let pending_script_enabled_updates = Arc::new(Mutex::new(Vec::new()));
        Ok(Arc::new(Self {
            runtime: Mutex::new(mission_runtime_with_host_trigger_world(
                Arc::clone(&pending_script_enabled_updates),
                host_trigger_world,
            )?),
            pending_script_enabled_updates,
            notifications: Mutex::new(ScriptNotificationQueues::default()),
            completion: Mutex::new(AudioCompletionTracking::default()),
            camera_movement_finished: AtomicBool::new(true),
            frame_counter: AtomicU64::new(0),
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
        if let Ok(mut queue) = self.notifications.lock() {
            let localized = localization::localize_with_args(
                "hud.script.broadcast",
                "Transmission: {message}",
                &[("message", text.as_str())],
            );
            queue.messages.push(localized);
        }
    }

    pub fn push_sound(&self, name: String) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.sounds.push(name);
        }
    }

    pub fn push_sound_event(&self, event: ScriptSoundEvent) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.sound_events.push(event);
        }
    }

    pub fn push_camera_move(&self, position: Vec3) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_moves.push(position);
        }
    }

    pub fn push_camera_tether(&self, request: CameraTetherRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_tethers.push(request);
        }
    }

    pub fn push_camera_follow(&self, request: CameraFollowRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_follows.push(request);
        }
    }

    pub fn push_camera_path_move(&self, request: CameraPathRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_path_moves.push(request);
        }
    }

    pub fn push_camera_move_to(&self, request: CameraMoveToRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_move_to.push(request);
        }
    }

    pub fn push_camera_move_to_selection(&self) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_move_to_selection_requests.push(());
        }
    }

    pub fn push_camera_move_home(&self) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_move_home_requests.push(());
        }
    }

    pub fn push_camera_reset(&self, request: CameraResetRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_resets.push(request);
        }
    }

    pub fn push_camera_zoom(&self, request: CameraZoomRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_zoom_requests.push(request);
        }
    }

    pub fn push_camera_pitch(&self, request: CameraPitchRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_pitch_requests.push(request);
        }
    }

    pub fn push_camera_rotate(&self, request: CameraRotateRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_rotate_requests.push(request);
        }
    }

    pub fn push_camera_mod_final_zoom(&self, request: CameraModFinalZoomRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_mod_final_zoom_requests.push(request);
        }
    }

    pub fn push_camera_mod_final_pitch(&self, request: CameraModFinalPitchRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_mod_final_pitch_requests.push(request);
        }
    }

    pub fn push_camera_mod_freeze_time(&self) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_mod_freeze_time_requests.push(());
        }
    }

    pub fn push_camera_mod_freeze_angle(&self) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_mod_freeze_angle_requests.push(());
        }
    }

    pub fn push_camera_mod_final_speed_multiplier(
        &self,
        request: CameraModFinalSpeedMultiplierRequest,
    ) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue
                .camera_mod_final_speed_multiplier_requests
                .push(request);
        }
    }

    pub fn push_camera_mod_rolling_average(&self, request: CameraModRollingAverageRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_mod_rolling_average_requests.push(request);
        }
    }

    pub fn push_visual_speed_multiplier(&self, request: VisualSpeedMultiplierRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.visual_speed_multiplier_requests.push(request);
        }
    }

    pub fn push_script_freeze_time(&self, freeze: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.script_freeze_time_requests.push(freeze);
        }
    }

    pub fn push_set_fps_limit(&self, request: SetFpsLimitRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.set_fps_limit_requests.push(request);
        }
    }

    pub fn push_camera_setup(&self, request: CameraSetupRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_setup_requests.push(request);
        }
    }

    pub fn push_camera_look_toward_object(&self, request: CameraLookTowardObjectRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_look_toward_object_requests.push(request);
        }
    }

    pub fn push_camera_look_toward_waypoint(&self, request: CameraLookTowardWaypointRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_look_toward_waypoint_requests.push(request);
        }
    }

    pub fn push_camera_mod_look_toward(&self, request: CameraModLookTowardRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_mod_look_toward_requests.push(request);
        }
    }

    pub fn push_camera_mod_final_look_toward(&self, request: CameraModFinalLookTowardRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_mod_final_look_toward_requests.push(request);
        }
    }

    pub fn push_camera_set_default(&self, request: CameraSetDefaultRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_set_default_requests.push(request);
        }
    }

    pub fn push_camera_slave_mode_enable(&self, request: CameraSlaveModeRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_slave_mode_enable_requests.push(request);
        }
    }

    pub fn push_camera_slave_mode_disable(&self) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_slave_mode_disable_requests.push(());
        }
    }

    pub fn push_screen_shake(&self, request: ScreenShakeRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.screen_shake_requests.push(request);
        }
    }

    pub fn push_camera_add_shaker(&self, request: CameraAddShakerRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_add_shaker_requests.push(request);
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
        if let Ok(mut queue) = self.notifications.lock() {
            queue.cinematic_text.push((text, font, duration_seconds));
        }
    }

    pub fn push_military_caption(&self, text: String, duration_ms: i32) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue
                .military_captions
                .push(MilitaryCaptionRequest { text, duration_ms });
        }
    }

    pub fn push_letterbox(&self, enabled: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.letterbox_events.push(enabled);
        }
    }

    pub fn push_movie_request(&self, filename: String) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.movie_requests.push(filename);
        }
    }

    pub fn push_radar_movie_request(&self, filename: String) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.radar_movie_requests.push(filename);
        }
    }

    pub fn push_objective_update(&self, update: ObjectiveUpdate) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.objective_updates.push(update);
        }
    }

    pub fn push_effect_request(&self, request: ScriptEffectRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.effect_requests.push(request);
        }
    }

    pub fn push_radar_event_request(&self, request: RadarScriptEventRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.radar_event_requests.push(request);
        }
    }

    pub fn push_radar_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.radar_enabled_updates.push(enabled);
        }
    }

    pub fn push_radar_forced(&self, forced: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.radar_forced_updates.push(forced);
        }
    }

    pub fn push_weather_visible(&self, visible: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.weather_visibility_updates.push(visible);
        }
    }

    pub fn push_popup_message(&self, mut request: ScriptPopupMessageRequest) {
        // Keep this opaque and monotonic rather than deriving authority from
        // popup text/layout fields. Acknowledge only the exact live instance.
        request.popup_generation = next_live_popup_generation();
        if let Ok(mut queue) = self.notifications.lock() {
            queue.popup_message_requests.push(request);
        }
    }

    pub fn push_view_guardband(&self, request: ViewGuardbandRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.view_guardband_requests.push(request);
        }
    }

    pub fn push_camera_bw_mode(&self, request: CameraBwModeRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_bw_mode_requests.push(request);
        }
    }

    pub fn push_skybox_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.skybox_enabled_updates.push(enabled);
        }
    }

    pub fn push_camera_motion_blur(&self, request: CameraMotionBlurRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.camera_motion_blur_requests.push(request);
        }
    }

    pub fn push_cameo_flash(&self, request: CameoFlashRequest) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.cameo_flash_requests.push(request);
        }
    }

    pub fn push_named_timer_mutation(&self, request: NamedTimerMutation) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.named_timer_mutations.push(request);
        }
    }

    pub fn push_named_timer_display(&self, show: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.named_timer_display_updates.push(show);
        }
    }

    pub fn push_superweapon_display_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.superweapon_display_enabled_updates.push(enabled);
        }
    }

    pub fn push_named_special_power_countdown_mutation(
        &self,
        request: NamedSpecialPowerCountdownMutation,
    ) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.named_special_power_countdown_mutations.push(request);
        }
    }

    pub fn push_superweapon_object_display_mutation(
        &self,
        request: SuperweaponObjectDisplayMutation,
    ) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.superweapon_object_display_mutations.push(request);
        }
    }

    pub fn push_music_stop(&self) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.music_stop_requests.push(());
        }
    }

    pub fn push_oversize_terrain(&self, amount: i32) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.oversize_terrain_requests.push(amount);
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
        if let Ok(mut state) = self.completion.lock() {
            state
                .speech_complete_frame
                .insert(name.to_string(), speech_completion_frame(now, name));
            if handle != 0 {
                state
                    .speech_handles
                    .entry(name.to_string())
                    .or_default()
                    .push(handle);
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
        if let Ok(mut state) = self.completion.lock() {
            if let Some(pending) = state.speech_handles.get_mut(name) {
                match gamelogic::helpers::TheAudio::get() {
                    Some(audio) => pending.retain(|handle| audio.is_currently_playing(*handle)),
                    None => pending.clear(),
                }
                if !pending.is_empty() {
                    return false;
                }
                if flush {
                    state.speech_handles.remove(name);
                }
            }
        }
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut state) = self.completion.lock() else {
            return true;
        };
        let done_frame = match state.speech_complete_frame.get(name).copied() {
            Some(done_frame) => done_frame,
            None => {
                // C++ first HAS_FINISHED_SPEECH query starts the TheAudio timer.
                let done_frame = speech_completion_frame(now, name);
                state
                    .speech_complete_frame
                    .insert(name.to_string(), done_frame);
                done_frame
            }
        };
        let done = now >= done_frame;
        if done && flush {
            state.speech_complete_frame.remove(name);
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
        let Ok(mut state) = self.completion.lock() else {
            return false;
        };
        let done_frame = match state.audio_complete_frame.get(name).copied() {
            Some(done_frame) => done_frame,
            None => {
                let done_frame = speech_completion_frame(now, name);
                state
                    .audio_complete_frame
                    .insert(name.to_string(), done_frame);
                done_frame
            }
        };
        let done = now >= done_frame;
        if done && flush {
            state.audio_complete_frame.remove(name);
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
        self.notifications
            .lock()
            .map(|mut q| q.messages.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_sounds(&self) -> Vec<String> {
        self.notifications
            .lock()
            .map(|mut q| q.sounds.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_sound_events(&self) -> Vec<ScriptSoundEvent> {
        self.notifications
            .lock()
            .map(|mut q| q.sound_events.drain(..).collect())
            .unwrap_or_default()
    }

    /// Take only the five contiguous focus families; preserve later action drains.
    /// Producers and the Main driver run synchronously on the game thread.
    /// Retain the external shared-handler synchronization and poisoned fallback.
    pub(crate) fn take_camera_focus_requests(&self) -> CameraFocusRequests {
        self.notifications
            .lock()
            .map(|mut q| CameraFocusRequests {
                moves: q.camera_moves.drain(..).collect(),
                move_to_selection: q.camera_move_to_selection_requests.drain(..).collect(),
                move_home: q.camera_move_home_requests.drain(..).collect(),
                follows: q.camera_follows.drain(..).collect(),
                tethers: q.camera_tethers.drain(..).collect(),
            })
            .unwrap_or_default()
    }

    pub fn drain_camera_moves(&self) -> Vec<Vec3> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_moves.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_follows(&self) -> Vec<CameraFollowRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_follows.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_tethers(&self) -> Vec<CameraTetherRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_tethers.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_path_moves(&self) -> Vec<CameraPathRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_path_moves.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_to(&self) -> Vec<CameraMoveToRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_move_to.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_to_selection_requests(&self) -> Vec<()> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_move_to_selection_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_home_requests(&self) -> Vec<()> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_move_home_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_resets(&self) -> Vec<CameraResetRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_resets.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_zoom_requests(&self) -> Vec<CameraZoomRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_zoom_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_pitch_requests(&self) -> Vec<CameraPitchRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_pitch_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_rotate_requests(&self) -> Vec<CameraRotateRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_rotate_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_zoom_requests(&self) -> Vec<CameraModFinalZoomRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_mod_final_zoom_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_pitch_requests(&self) -> Vec<CameraModFinalPitchRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_mod_final_pitch_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_freeze_time_requests(&self) -> Vec<()> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_mod_freeze_time_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_freeze_angle_requests(&self) -> Vec<()> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_mod_freeze_angle_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_speed_multiplier_requests(
        &self,
    ) -> Vec<CameraModFinalSpeedMultiplierRequest> {
        self.notifications
            .lock()
            .map(|mut q| {
                q.camera_mod_final_speed_multiplier_requests
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_rolling_average_requests(&self) -> Vec<CameraModRollingAverageRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_mod_rolling_average_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_visual_speed_multiplier_requests(&self) -> Vec<VisualSpeedMultiplierRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.visual_speed_multiplier_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_script_freeze_time_requests(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.script_freeze_time_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_set_fps_limit_requests(&self) -> Vec<SetFpsLimitRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.set_fps_limit_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_setup_requests(&self) -> Vec<CameraSetupRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_setup_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_look_toward_object_requests(&self) -> Vec<CameraLookTowardObjectRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_look_toward_object_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_look_toward_waypoint_requests(
        &self,
    ) -> Vec<CameraLookTowardWaypointRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_look_toward_waypoint_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_look_toward_requests(&self) -> Vec<CameraModLookTowardRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_mod_look_toward_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_look_toward_requests(
        &self,
    ) -> Vec<CameraModFinalLookTowardRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_mod_final_look_toward_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_set_default_requests(&self) -> Vec<CameraSetDefaultRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_set_default_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_slave_mode_enable_requests(&self) -> Vec<CameraSlaveModeRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_slave_mode_enable_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_slave_mode_disable_requests(&self) -> Vec<()> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_slave_mode_disable_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_screen_shake_requests(&self) -> Vec<ScreenShakeRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.screen_shake_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_add_shaker_requests(&self) -> Vec<CameraAddShakerRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_add_shaker_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_cinematic_text(&self) -> Vec<(String, String, i32)> {
        self.notifications
            .lock()
            .map(|mut q| q.cinematic_text.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_military_captions(&self) -> Vec<MilitaryCaptionRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.military_captions.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_letterbox_events(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.letterbox_events.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_movie_requests(&self) -> Vec<String> {
        self.notifications
            .lock()
            .map(|mut q| q.movie_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_movie_requests(&self) -> Vec<String> {
        self.notifications
            .lock()
            .map(|mut q| q.radar_movie_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_objective_updates(&self) -> Vec<ObjectiveUpdate> {
        self.notifications
            .lock()
            .map(|mut q| q.objective_updates.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_effect_requests(&self) -> Vec<ScriptEffectRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.effect_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_event_requests(&self) -> Vec<RadarScriptEventRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.radar_event_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_enabled_updates(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.radar_enabled_updates.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_forced_updates(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.radar_forced_updates.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_weather_visibility_updates(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.weather_visibility_updates.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_popup_message_requests(&self) -> Vec<ScriptPopupMessageRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.popup_message_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_view_guardband_requests(&self) -> Vec<ViewGuardbandRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.view_guardband_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_bw_mode_requests(&self) -> Vec<CameraBwModeRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_bw_mode_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_skybox_enabled_updates(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.skybox_enabled_updates.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_motion_blur_requests(&self) -> Vec<CameraMotionBlurRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.camera_motion_blur_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_cameo_flash_requests(&self) -> Vec<CameoFlashRequest> {
        self.notifications
            .lock()
            .map(|mut q| q.cameo_flash_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_timer_mutations(&self) -> Vec<NamedTimerMutation> {
        self.notifications
            .lock()
            .map(|mut q| q.named_timer_mutations.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_timer_display_updates(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.named_timer_display_updates.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_superweapon_display_enabled_updates(&self) -> Vec<bool> {
        self.notifications
            .lock()
            .map(|mut q| q.superweapon_display_enabled_updates.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_special_power_countdown_mutations(
        &self,
    ) -> Vec<NamedSpecialPowerCountdownMutation> {
        self.notifications
            .lock()
            .map(|mut q| {
                q.named_special_power_countdown_mutations
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn drain_superweapon_object_display_mutations(
        &self,
    ) -> Vec<SuperweaponObjectDisplayMutation> {
        self.notifications
            .lock()
            .map(|mut q| q.superweapon_object_display_mutations.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_music_stop_requests(&self) -> Vec<()> {
        self.notifications
            .lock()
            .map(|mut q| q.music_stop_requests.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn push_border_shroud_level(&self, level: u8) {
        if let Ok(mut queue) = self.notifications.lock() {
            queue.border_shroud_levels.push(level);
        }
    }

    pub fn drain_border_shroud_levels(&self) -> Vec<u8> {
        self.notifications
            .lock()
            .map(|mut q| q.border_shroud_levels.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_oversize_terrain_requests(&self) -> Vec<i32> {
        self.notifications
            .lock()
            .map(|mut q| q.oversize_terrain_requests.drain(..).collect())
            .unwrap_or_default()
    }
}
