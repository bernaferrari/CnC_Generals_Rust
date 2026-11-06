use super::*;

// THE_AUDIO → SystemStores.the_audio

pub(super) struct AnimatedSoundBridge {
    audio: Arc<Mutex<AudioManager>>,
    // THREAD: kept as Mutex - published as `Arc<dyn SoundLibraryBridge>` (Send + Sync).
    active_handles: Mutex<HashMap<String, Vec<AudioHandle>>>,
}

impl AnimatedSoundBridge {
    fn new(audio: Arc<Mutex<AudioManager>>) -> Self {
        Self {
            audio,
            active_handles: Mutex::new(HashMap::new()),
        }
    }

    fn normalized_key(name: &str) -> Option<String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_uppercase())
        }
    }

    fn record_handle(&self, sound_name: &str, handle: AudioHandle) {
        // Match classic engine: track only concrete runtime handles, not sentinel values.
        if handle < AHSV_FIRST_HANDLE
            || handle == AHSV_ERROR
            || handle == AHSV_NOT_FOR_LOCAL
            || handle == AHSV_MUTED
            || handle == AHSV_STOP_THE_MUSIC
            || handle == AHSV_STOP_THE_MUSIC_FADE
        {
            return;
        }

        if let Some(key) = Self::normalized_key(sound_name) {
            if let Ok(mut map) = self.active_handles.lock() {
                map.entry(key).or_default().push(handle);
            }
        }
    }

    fn take_handles(&self, sound_name: &str) -> Vec<AudioHandle> {
        if let Some(key) = Self::normalized_key(sound_name) {
            if let Ok(mut map) = self.active_handles.lock() {
                return map.remove(&key).unwrap_or_default();
            }
        }
        Vec::new()
    }

    fn play_internal(&self, sound_name: &str, position: Option<Coord3D>) -> W3DResult<()> {
        if sound_name.trim().is_empty() {
            return Err(W3DError::InvalidParameter(
                "sound name must not be empty".to_string(),
            ));
        }

        let mut manager = self.audio.lock().map_err(|_| W3DError::Unknown)?;

        let mut event = if let Some(pos) = position {
            AudioEventRts::with_position(sound_name, &pos)
        } else {
            AudioEventRts::with_event_name(sound_name)
        };

        let info = manager
            .find_audio_event_info(sound_name)
            .or_else(|| manager.new_audio_event_info(sound_name.to_string()))
            .ok_or_else(|| {
                W3DError::InvalidParameter(format!("audio event info '{sound_name}' not found"))
            })?;

        event.set_audio_event_info(info.clone());
        event.set_volume(info.volume);

        let handle = manager.add_audio_event(&event);
        if handle == AHSV_ERROR {
            Err(W3DError::Unknown)
        } else {
            self.record_handle(sound_name, handle);
            Ok(())
        }
    }
}

impl SoundLibraryBridge for AnimatedSoundBridge {
    fn play_3d_audio(&self, name: &str, transform: &Mat4) -> W3DResult<()> {
        let translation = transform.w_axis.truncate();
        let position = Coord3D {
            x: translation.x,
            y: translation.y,
            z: translation.z,
        };
        self.play_internal(name, Some(position))
    }

    fn play_2d_audio(&self, name: &str) -> W3DResult<()> {
        self.play_internal(name, None)
    }

    fn stop_audio(&self, name: &str) -> W3DResult<()> {
        let handles = self.take_handles(name);
        let mut manager = self.audio.lock().map_err(|_| W3DError::Unknown)?;
        if !handles.is_empty() {
            for handle in handles {
                manager.remove_audio_event(handle);
            }
        }
        Ok(())
    }
}

/// Register the audio manager with the ww3d animated sound system.
pub fn register_animation_sound_library(manager: Arc<Mutex<AudioManager>>) {
    let anim_mgr = crate::common::common_services::common_services()
        .system
        .animated_sound_mgr
        .clone();
    if let Err(err) = initialize_animated_sound_mgr::<&str>(&anim_mgr, None) {
        log::debug!("Animated sound metadata not available: {err:?}");
    }

    let bridge: Arc<dyn SoundLibraryBridge> = Arc::new(AnimatedSoundBridge::new(manager));
    set_sound_library(&anim_mgr, bridge);
}

/// Initialize the global audio manager singleton.
pub fn initialize_global_audio_manager() -> Arc<Mutex<AudioManager>> {
    let services = crate::common::common_services::common_services();
    {
        let slot = services
            .system
            .the_audio
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = slot.as_ref() {
            return existing.clone();
        }
    }

    let mut created = AudioManager::new();
    // C++ `AudioManager::init` (GameAudio.cpp) copies preferred slider volumes.
    // `new()` leaves effective music/sound/speech volumes at 0 until init.
    created.init();
    let manager = Arc::new(Mutex::new(created));
    {
        let mut slot = services
            .system
            .the_audio
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = slot.as_ref() {
            return existing.clone();
        }
        *slot = Some(manager.clone());
    }
    #[cfg(not(target_arch = "wasm32"))]
    register_rodio_playback_hook();
    #[cfg(target_arch = "wasm32")]
    register_wasm_playback_hook();
    register_animation_sound_library(manager.clone());
    // Parsers register via THE_AUDIO; load after the singleton is published.
    load_audio_event_inis();
    manager
}

/// Access the global audio manager if it has been initialised.
pub fn get_global_audio_manager() -> Option<Arc<Mutex<AudioManager>>> {
    crate::common::common_services::common_services()
        .system
        .the_audio
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// C++ `AudioManager::init` (GameAudio.cpp:186-202) loads AudioSettings.ini,
/// then Music / SoundEffects / Speech / Voice (Default then override), then
/// MiscAudio.ini. AudioSettings drive sample counts, volumes, zoom, and cache
/// size; MiscAudio names UI clicks, radar, sabotage, and other global cues.
pub fn load_audio_event_inis() {
    const AUDIO_INI_FILES: &[&str] = &[
        "Data/INI/AudioSettings.ini",
        "Data/INI/Default/Music.ini",
        "Data/INI/Music.ini",
        "Data/INI/Default/SoundEffects.ini",
        "Data/INI/SoundEffects.ini",
        "Data/INI/Default/Speech.ini",
        "Data/INI/Speech.ini",
        "Data/INI/Default/Voice.ini",
        "Data/INI/Voice.ini",
        "Data/INI/MiscAudio.ini",
    ];

    let mut ini = crate::common::ini::INI::new();
    let mut resolver = crate::common::system::install_layout::DataIniResolver::new();
    for virtual_path in AUDIO_INI_FILES {
        let Some(path) = resolver.resolve(virtual_path) else {
            continue;
        };
        if let Err(err) = ini.load(&path, crate::common::ini::INILoadType::Overwrite) {
            eprintln!("Failed to load audio INI '{}': {err}", path.display());
        }
    }

    if crate::common::ini::ini_audio_settings::get_audio_settings().is_none() {
        // Retail AudioSettings.ini ships only inside INIZH.big
        // (`Data\INI\AudioSettings.ini`). When the loose extract is absent
        // or fails to parse, re-read the archive copy through the game file
        // system — the real retail mix values (SampleCount3D = 25,
        // Relative2DVolume = -10%, ...) — instead of leaving the compiled
        // `AudioSettings::default()` table active.
        if let Some(settings) = read_audio_settings_from_game_fs() {
            let mut archive_ini = crate::common::ini::INI::new();
            if let Err(err) =
                archive_ini.with_inline_source(&settings, |ini| ini.parse_current_file())
            {
                eprintln!("Failed to parse archived AudioSettings.ini: {err}");
            }
        }
    }

    apply_loaded_audio_settings_to_manager();
}

/// Read `Data/INI/AudioSettings.ini` through the engine file system (loose
/// overrides first, then the mounted `.big` archives).
pub(super) fn read_audio_settings_from_game_fs() -> Option<String> {
    let file_system = get_file_system();
    let mut fs = file_system.lock().ok()?;
    let mut file = fs.open_file(
        "Data/INI/AudioSettings.ini",
        FileAccess::READ.combine(FileAccess::BINARY),
    )?;
    let bytes = file.read_entire_and_close().ok()?;
    String::from_utf8(bytes).ok()
}

/// Copy parsed AudioSettings.ini / MiscAudio.ini into the live AudioManager.
pub(super) fn apply_loaded_audio_settings_to_manager() {
    let Some(manager) = get_global_audio_manager() else {
        return;
    };
    let Ok(mut guard) = manager.lock() else {
        return;
    };

    if let Some(parsed) = crate::common::ini::ini_audio_settings::get_audio_settings() {
        guard.apply_receiving_audio_settings(&parsed.read());
    }

    if let Some(parsed) = crate::common::ini::ini_misc_audio::get_misc_audio() {
        guard.apply_receiving_misc_audio(&parsed.read());
    }
}
