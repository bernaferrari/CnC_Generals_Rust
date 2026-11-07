use super::*;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn get_rodio_stream_handle() -> Option<OutputStreamHandle> {
    type Payload = Option<(OutputStream, OutputStreamHandle)>;
    let services = crate::common::common_services::common_services();
    let mut slot = services
        .system
        .rodio_stream_slot
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if slot.is_none() {
        *slot = Some(Box::new(None as Payload));
    }
    let payload = slot
        .as_mut()
        .unwrap()
        .downcast_mut::<Payload>()
        .expect("rodio_stream_slot type");
    if payload.is_none() {
        if let Ok((stream, handle)) = OutputStream::try_default() {
            *payload = Some((stream, handle));
        }
    }
    payload.as_ref().map(|(_, handle)| handle.clone())
}
pub(super) fn live_audio_file_cache() -> Arc<AudioFileCache> {
    crate::common::common_services::common_services()
        .system
        .live_audio_file_cache
        .clone()
}

/// C++ `AudioFileCache::openFile` stereo-positional reject (MilesAudioManager.cpp:3147-3152).
pub(super) fn wav_channel_count(data: &[u8]) -> Option<u16> {
    if data.len() < 24 || &data[0..4] != b"RIFF" {
        return None;
    }
    Some(u16::from_le_bytes([data[22], data[23]]))
}

pub(super) struct CachedAudioBytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for CachedAudioBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) enum RodioVoice {
    Flat(Sink),
    Spatial(SpatialSink),
}

#[cfg(not(target_arch = "wasm32"))]
impl RodioVoice {
    fn set_volume(&self, volume: f32) {
        match self {
            Self::Flat(sink) => sink.set_volume(volume),
            Self::Spatial(sink) => sink.set_volume(volume),
        }
    }

    fn stop(&self) {
        match self {
            Self::Flat(sink) => sink.stop(),
            Self::Spatial(sink) => sink.stop(),
        }
    }

    fn pause(&self) {
        match self {
            Self::Flat(sink) => sink.pause(),
            Self::Spatial(sink) => sink.pause(),
        }
    }

    fn play(&self) {
        match self {
            Self::Flat(sink) => sink.play(),
            Self::Spatial(sink) => sink.play(),
        }
    }

    fn empty(&self) -> bool {
        match self {
            Self::Flat(sink) => sink.empty(),
            Self::Spatial(sink) => sink.empty(),
        }
    }

    fn is_paused(&self) -> bool {
        match self {
            Self::Flat(sink) => sink.is_paused(),
            Self::Spatial(sink) => sink.is_paused(),
        }
    }

    fn set_stereo_pan(&self, pan: Real) {
        if let Self::Spatial(sink) = self {
            sink.set_emitter_position([pan, 0.0, -1.0]);
            sink.set_left_ear_position([-0.1, 0.0, 0.0]);
            sink.set_right_ear_position([0.1, 0.0, 0.0]);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct RodioPlaybackHook {
    // THREAD: kept as Mutex - the hook is published as `Arc<dyn SoundPlaybackHook>`
    // (Send + Sync) and is reachable from the rodio callback thread.
    sinks: Mutex<HashMap<AudioHandle, RodioSinkState>>,
    listener_position: Mutex<Coord3D>,
    listener_orientation: Mutex<Coord3D>,
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct RodioSinkState {
    // Each voice is owned by its registry; rodio shares its internal player controls.
    sink: RodioVoice,
    base_volume: Real,
    position: Option<Coord3D>,
    min_distance: Real,
    max_distance: Real,
    is_music: bool,
    started_at: Instant,
    duration_ms: Option<Real>,
}

#[cfg(not(target_arch = "wasm32"))]
impl RodioSinkState {
    fn new(
        sink: RodioVoice,
        base_volume: Real,
        position: Option<Coord3D>,
        ranges: (Real, Real),
        is_music: bool,
        duration_ms: Option<Real>,
    ) -> Self {
        let (min_distance, max_distance) = ranges;
        Self {
            sink,
            base_volume,
            position,
            min_distance,
            max_distance,
            is_music,
            started_at: Instant::now(),
            duration_ms,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RodioPlaybackHook {
    fn new() -> Self {
        let _ = get_rodio_stream_handle();
        Self {
            sinks: Mutex::new(HashMap::new()),
            listener_position: Mutex::new(Coord3D::ZERO),
            listener_orientation: Mutex::new(Coord3D {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
        }
    }

    fn build_path_candidates(filename: &str) -> Vec<String> {
        let trimmed = filename.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let normalized = trimmed.replace('\\', "/");
        let mut candidates = vec![trimmed.to_string()];
        if normalized != trimmed {
            candidates.push(normalized.clone());
        }

        if std::path::Path::new(trimmed).extension().is_none() {
            for ext in [".wav", ".mp3", ".ogg"] {
                candidates.push(format!("{trimmed}{ext}"));
                if normalized != trimmed {
                    candidates.push(format!("{normalized}{ext}"));
                }
            }
        }

        candidates.sort();
        candidates.dedup();
        candidates
    }

    fn portion_filename(event: &AudioEventRts) -> &str {
        match event.get_next_play_portion() {
            PortionToPlay::Attack if !event.get_attack_filename().is_empty() => {
                event.get_attack_filename()
            }
            PortionToPlay::Decay if !event.get_decay_filename().is_empty() => {
                event.get_decay_filename()
            }
            _ => event.get_filename(),
        }
    }

    fn resolve_audio_data(&self, event: &AudioEventRts) -> Option<(String, Arc<Vec<u8>>)> {
        let mut names = vec![Self::portion_filename(event).to_string()];
        let fallback = event.get_filename();
        if fallback != names[0] {
            names.push(fallback.to_string());
        }
        let cache = live_audio_file_cache();
        for name in names {
            for candidate in Self::build_path_candidates(&name) {
                let Some(data) = cache.get_or_insert_named(&candidate, || {
                    AudioManager::read_from_virtual_file_system(&candidate)
                        .or_else(|| std::fs::read(&candidate).ok())
                }) else {
                    continue;
                };
                if event.is_positional_audio() {
                    if let Some(channels) = wav_channel_count(data.as_ref()) {
                        if channels > 1 {
                            cache.close_named(&candidate);
                            return None;
                        }
                    }
                }
                return Some((candidate, data));
            }
        }
        None
    }

    fn calculate_3d_volume_falloff(
        &self,
        position: &Coord3D,
        min_distance: Real,
        max_distance: Real,
    ) -> Real {
        let listener = self
            .listener_position
            .lock()
            .ok()
            .map(|l| *l)
            .unwrap_or_else(|| Coord3D::ZERO);
        let dx = position.x - listener.x;
        let dy = position.y - listener.y;
        let dz = position.z - listener.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        miles_positional_gain(distance, min_distance, max_distance)
    }

    fn effective_volume(&self, state: &RodioSinkState) -> Real {
        let base = state.base_volume.clamp(0.0, 1.0);
        if let Some(pos) = state.position.as_ref() {
            base * self.calculate_3d_volume_falloff(pos, state.min_distance, state.max_distance)
        } else {
            base
        }
    }

    fn refresh_sink_volume(&self, state: &RodioSinkState) {
        state.sink.set_volume(self.effective_volume(state));
    }

    fn listener_pose(&self) -> (Coord3D, Coord3D) {
        let position = self
            .listener_position
            .lock()
            .ok()
            .map(|l| *l)
            .unwrap_or(Coord3D::ZERO);
        let orientation = self
            .listener_orientation
            .lock()
            .ok()
            .map(|l| *l)
            .unwrap_or(Coord3D {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            });
        (position, orientation)
    }

    fn refresh_positional_pan(&self, state: &RodioSinkState) {
        let Some(source) = state.position else {
            return;
        };
        let (listener, orientation) = self.listener_pose();
        let pan = stereo_pan(&listener, orientation.x, orientation.y, &source);
        state.sink.set_stereo_pan(pan);
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RodioPlaybackHook {
    fn play_with_observation(
        &self,
        event: &AudioEventRts,
        on_screen: Option<bool>,
    ) -> Result<(), String> {
        let handle = event.get_playing_handle();
        if handle == 0 {
            return Err("No handle assigned".to_string());
        }
        let (file_path, audio_data) = self.resolve_audio_data(event).ok_or_else(|| {
            format!(
                "Could not resolve audio data for event '{}' (filename '{}')",
                event.get_event_name(),
                event.get_filename()
            )
        })?;
        let duration_ms = AudioManager::duration_ms_from_audio_data(audio_data.as_ref());
        #[cfg(feature = "audio")]
        let source: Box<dyn Source<Item = f32> + Send> =
            if super::super::retail_adpcm::is_ima_wave(&audio_data) {
                let decoded = super::super::retail_adpcm::decode(audio_data.clone())
                    .map_err(|e| format!("Failed to decode audio file '{}': {}", file_path, e))?;
                Box::new(rodio::buffer::SamplesBuffer::new(
                    std::num::NonZero::new(decoded.channels).expect("decoder validated channels"),
                    std::num::NonZero::new(decoded.sample_rate)
                        .expect("decoder validated sample rate"),
                    decoded.samples,
                ))
            } else {
                Box::new(
                    Decoder::new(Cursor::new(CachedAudioBytes(audio_data))).map_err(|e| {
                        format!("Failed to decode audio file '{}': {}", file_path, e)
                    })?,
                )
            };
        #[cfg(not(feature = "audio"))]
        let source = Decoder::new(Cursor::new(CachedAudioBytes(audio_data)))
            .map_err(|e| format!("Failed to decode audio file '{}': {}", file_path, e))?;
        let stream_handle = get_rodio_stream_handle()
            .ok_or_else(|| "Audio output stream not available".to_string())?;
        let (listener, orientation) = self.listener_pose();
        // C++ Miles getEffectiveVolume reads sliders already held by
        // AudioManager::update. Never re-lock THE_AUDIO from this hook.
        let sliders = event.playback_slider_observation().unwrap_or_else(|| {
            get_global_audio_manager()
                .and_then(|manager| manager.try_lock().ok().map(|m| m.miles_volume_sliders()))
                .unwrap_or_default()
        });
        let mut volume = miles_slider_volume(event, &sliders);
        let position = event
            .is_positional_audio()
            .then(|| miles_event_world_position(event));
        if event.is_positional_audio() {
            if let Some(info) = event.get_audio_event_info() {
                if info.low_pass_freq > 0.0 {
                    let on_screen = on_screen.unwrap_or_else(|| {
                        legacy_audio_visibility(&miles_event_world_position(event))
                    });
                    if !on_screen {
                        // C++ AIL_set_3D_sample_occlusion(sample, 1.0 - m_lowPassFreq)
                        volume *= info.low_pass_freq.clamp(0.0, 1.0);
                    }
                }
            }
        }
        let voice = if let Some(pos) = position.as_ref() {
            let pan = stereo_pan(&listener, orientation.x, orientation.y, pos);
            let spatial = SpatialSink::try_new(
                &stream_handle,
                [pan, 0.0, -1.0],
                [-0.1, 0.0, 0.0],
                [0.1, 0.0, 0.0],
            )
            .map_err(|e| format!("Failed to create spatial audio sink: {}", e))?;
            RodioVoice::Spatial(spatial)
        } else {
            let sink = Sink::try_new(&stream_handle)
                .map_err(|e| format!("Failed to create audio sink: {}", e))?;
            RodioVoice::Flat(sink)
        };
        let pitch = event.get_effective_pitch();
        // C++ MilesAudioManager::playStream (MilesAudioManager.cpp:2762-2764)
        // AIL_set_stream_loop_count(stream, INFINITE_LOOP_COUNT) for AT_Music.
        let is_music = music_repeats_source_infinitely(event);
        let pitched = (pitch - 1.0).abs() > 0.01;
        match &voice {
            RodioVoice::Flat(sink) => {
                if is_music {
                    if pitched {
                        sink.append(source.speed(pitch).repeat_infinite());
                    } else {
                        sink.append(source.repeat_infinite());
                    }
                } else if pitched {
                    sink.append(source.speed(pitch));
                } else {
                    sink.append(source);
                }
            }
            RodioVoice::Spatial(sink) => {
                if is_music {
                    if pitched {
                        sink.append(source.speed(pitch).repeat_infinite());
                    } else {
                        sink.append(source.repeat_infinite());
                    }
                } else if pitched {
                    sink.append(source.speed(pitch));
                } else {
                    sink.append(source);
                }
            }
        }
        let (min_distance, max_distance) = miles_positional_ranges(
            event.get_audio_event_info().as_deref(),
            sliders.global_min_range,
            sliders.global_max_range,
        );
        let state = RodioSinkState::new(
            voice,
            volume,
            position,
            (min_distance, max_distance),
            is_music,
            duration_ms,
        );
        self.refresh_sink_volume(&state);
        self.sinks.lock().unwrap().insert(handle, state);
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SoundPlaybackHook for RodioPlaybackHook {
    fn play(&self, event: &AudioEventRts) -> Result<(), String> {
        self.play_with_observation(event, None)
    }
    fn play_with_visibility(&self, event: &AudioEventRts, on_screen: bool) -> Result<(), String> {
        self.play_with_observation(event, Some(on_screen))
    }
    fn stop(&self, handle: AudioHandle) {
        if let Some(state) = self.sinks.lock().unwrap().remove(&handle) {
            state.sink.stop();
        }
    }

    fn pause(&self, handle: AudioHandle) {
        if let Some(state) = self.sinks.lock().unwrap().get(&handle) {
            state.sink.pause();
        }
    }
    fn set_listener_position(&self, position: &Coord3D) {
        if let Ok(mut listener) = self.listener_position.lock() {
            *listener = *position;
        }
        if let Ok(sinks) = self.sinks.lock() {
            for state in sinks.values() {
                if state.position.is_some() {
                    self.refresh_sink_volume(state);
                    self.refresh_positional_pan(state);
                }
            }
        }
    }

    fn set_listener_orientation(&self, orientation: &Coord3D) {
        if let Ok(mut stored) = self.listener_orientation.lock() {
            *stored = *orientation;
        }
        if let Ok(sinks) = self.sinks.lock() {
            for state in sinks.values() {
                if state.position.is_some() {
                    self.refresh_positional_pan(state);
                }
            }
        }
    }

    fn resume(&self, handle: AudioHandle) {
        if let Some(state) = self.sinks.lock().unwrap().get(&handle) {
            state.sink.play();
        }
    }

    fn is_playing(&self, handle: AudioHandle) -> bool {
        let mut sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get(&handle) else {
            return false;
        };

        let is_playing = !state.sink.empty();

        if !is_playing {
            sinks.remove(&handle);
        }

        is_playing
    }

    fn set_event_volume(&self, event: &AudioEventRts) {
        let handle = event.get_playing_handle();
        let mut sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get_mut(&handle) else {
            return;
        };

        let sliders = event.playback_slider_observation().unwrap_or_else(|| {
            get_global_audio_manager()
                .and_then(|manager| manager.try_lock().ok().map(|m| m.miles_volume_sliders()))
                .unwrap_or_default()
        });
        state.base_volume = miles_slider_volume(event, &sliders);
        if event.is_positional_audio() {
            state.position = Some(miles_event_world_position(event));
        }
        self.refresh_sink_volume(state);
        self.refresh_positional_pan(state);
    }

    fn set_sink_volume(&self, handle: AudioHandle, volume: Real) {
        let mut sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get_mut(&handle) else {
            return;
        };
        state.base_volume = volume.clamp(0.0, 1.0);
        state.sink.set_volume(state.base_volume);
    }

    fn music_loop_count(&self, handle: AudioHandle) -> Int {
        let sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get(&handle) else {
            return 0;
        };
        if !state.is_music {
            return 0;
        }
        let Some(duration) = state.duration_ms.filter(|ms| *ms > 0.0) else {
            return 0;
        };
        let elapsed_ms = state.started_at.elapsed().as_secs_f32() * 1000.0;
        (elapsed_ms / duration) as Int
    }

    fn is_sink_paused(&self, handle: AudioHandle) -> bool {
        let sinks = self.sinks.lock().unwrap();
        sinks
            .get(&handle)
            .map(|state| state.sink.is_paused())
            .unwrap_or(false)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn register_rodio_playback_hook() -> bool {
    let hook = Arc::new(RodioPlaybackHook::new());
    register_sound_playback_hook(hook)
}

/// Wasm has no rodio OutputStream. Playback requests still hit this hook.
#[cfg(target_arch = "wasm32")]
pub(super) struct WasmPlaybackHook;

#[cfg(target_arch = "wasm32")]
impl SoundPlaybackHook for WasmPlaybackHook {
    fn play(&self, _event: &AudioEventRts) -> Result<(), String> {
        Err("wasm audio output is not available".to_string())
    }
    fn stop(&self, _handle: AudioHandle) {}
    fn pause(&self, _handle: AudioHandle) {}
    fn resume(&self, _handle: AudioHandle) {}
    fn is_playing(&self, _handle: AudioHandle) -> bool {
        false
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn register_wasm_playback_hook() {
    let _ = register_sound_playback_hook(Arc::new(WasmPlaybackHook));
}
