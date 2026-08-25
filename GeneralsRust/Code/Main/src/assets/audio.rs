////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// AudioManager - Core audio management system matching C++ implementation
// This file mirrors the structure and functionality of the original C++ AudioManager

use crate::assets::archive::ArchiveFileSystem;
use anyhow::{Result, anyhow};
use log::{debug, error, info, warn};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

/// Audio affect types (matches C++ AudioAffect enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioAffect {
    Music,
    Sound,
    Sound3D,
    Speech,
    UI,
    Ambient,
}

/// Audio file formats supported by C&C Generals
#[derive(Debug, Clone, Copy)]
pub enum AudioFormat {
    WAV,
    OGG,
    MP3,
    Unknown,
}

/// Audio channel information (matches C++ AudioChannel structure)
struct AudioChannel {
    sink: Sink,
    affect_type: AudioAffect,
    volume: f32,
    enabled: bool,
    priority: i32,
}

impl std::fmt::Debug for AudioChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioChannel")
            .field("affect_type", &self.affect_type)
            .field("volume", &self.volume)
            .field("enabled", &self.enabled)
            .field("priority", &self.priority)
            .field("sink", &"<Sink>")
            .finish()
    }
}

impl AudioFormat {
    pub fn from_filename(filename: &str) -> Self {
        let filename_lower = filename.to_lowercase();
        if filename_lower.ends_with(".wav") {
            AudioFormat::WAV
        } else if filename_lower.ends_with(".ogg") {
            AudioFormat::OGG
        } else if filename_lower.ends_with(".mp3") {
            AudioFormat::MP3
        } else {
            AudioFormat::Unknown
        }
    }
}

/// Owns a rodio `OutputStream` on the thread that created it.
///
/// rodio 0.17: `Sink`, `SpatialSink`, and `OutputStreamHandle` are already
/// `Send + Sync`. `OutputStream` is not — it contains `cpal::Stream`, which
/// cpal marks `!Send + !Sync` on every platform via
/// `NotSendSyncAcrossAllPlatforms` (Android AAudio is not thread-safe).
///
/// `AudioManager` lives in `Mutex<AssetManager>` / `Mutex<SubsystemManager>`
/// and can be moved across threads, so the stream is created and dropped on a
/// dedicated owner thread. Only the already-Send handle is stored here.
/// No blanket `unsafe impl Send/Sync` for arbitrary `T`.
struct OutputStreamKeepalive {
    shutdown: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for OutputStreamKeepalive {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn spawn_output_stream_owner() -> Result<(OutputStreamKeepalive, OutputStreamHandle)> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let thread = thread::Builder::new()
        .name("rodio-output-stream".into())
        .spawn(move || match OutputStream::try_default() {
            Ok((output, handle)) => {
                if ready_tx.send(Ok(handle)).is_err() {
                    return;
                }
                let _output = output;
                let _ = shutdown_rx.recv();
            }
            Err(err) => {
                let _ = ready_tx.send(Err(err.to_string()));
            }
        })
        .map_err(|e| anyhow!("Failed to spawn audio stream thread: {}", e))?;

    let handle = match ready_rx.recv() {
        Ok(Ok(handle)) => handle,
        Ok(Err(err)) => {
            let _ = thread.join();
            return Err(anyhow!("Failed to initialize audio output: {}", err));
        }
        Err(_) => {
            let _ = thread.join();
            return Err(anyhow!(
                "Audio stream thread exited before reporting output device status"
            ));
        }
    };

    Ok((
        OutputStreamKeepalive {
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        },
        handle,
    ))
}

/// Build leftover `AudioEventRts` for TheAudio, keeping world pose or object.
///
/// `position_host_yup` is Main Y-up `(x, height, z_ground)`. Leftover / C++
/// Coord3D is Z-up `(x, y_ground, z_height)` so Miles `playSample3D` pans.
/// `object_id` is C++ `setObjectID` — `OwnerType::Object` so leftover follows
/// the unit each frame. Object wins over a static pose (`setPosition` is a
/// no-op once the owner is `OT_Object`).
pub fn leftover_world_sfx_event(
    event_name: &str,
    position_host_yup: Option<(f32, f32, f32)>,
    object_id: Option<u32>,
) -> gamelogic::common::audio::AudioEventRts {
    let mut event = gamelogic::common::audio::AudioEventRts::new(event_name);
    if let Some(id) = object_id.filter(|&id| id != 0) {
        event.set_object_id(id);
    } else if let Some((x, y, z)) = position_host_yup {
        event.set_position(&(x, z, y));
    }
    event
}

/// Leftover Miles is constructed (`THE_AUDIO`). Culled events stay silent.
pub fn leftover_the_audio_is_live() -> bool {
    game_engine::common::audio::game_audio::get_global_audio_manager().is_some()
}

/// C++ `TheAudio->addAudioEvent` (`AudioManager::addAudioEvent`, GameAudio.cpp).
///
/// Live UI/SFX play API. Routes only when Common `AudioManager` already has a
/// live handle (`get_global_audio_manager`). Special values
/// (`AHSV_NO_SOUND`/`AHSV_ERROR`/…) are treated as “no handle”.
pub fn play_sound_through_the_audio(event_name: &str) -> Option<u32> {
    play_sound_through_the_audio_at(event_name, None, None)
}

/// Same as [`play_sound_through_the_audio`] with a world pose / object owner.
pub fn play_sound_through_the_audio_at(
    event_name: &str,
    position_host_yup: Option<(f32, f32, f32)>,
    object_id: Option<u32>,
) -> Option<u32> {
    game_engine::common::audio::game_audio::get_global_audio_manager()?;
    let audio = gamelogic::helpers::TheAudio::get()?;
    let event = leftover_world_sfx_event(event_name, position_host_yup, object_id);
    let handle = audio.add_audio_event(&event);
    // C++ `AHSV_FIRST_HANDLE` is 1000; sentinels occupy the 0xFFFF_FFF0 band.
    if (1000..0xFFFF_FFF0).contains(&handle) {
        Some(handle)
    } else {
        None
    }
}

/// C++ `MilesAudioManager::getEffectiveVolume` for the live host drain.
///
/// `position_host_yup` is Main Y-up `(x, height, z)`. Leftover listener /
/// event math is C++ Z-up `(x, y_ground, z_height)`.
pub fn live_gameplay_sfx_volume(
    event_name: &str,
    position_host_yup: Option<(f32, f32, f32)>,
) -> f32 {
    let (listener, sliders, info) = leftover_volume_inputs(event_name);
    live_gameplay_sfx_volume_with(event_name, position_host_yup, listener, &sliders, info)
}

/// Same Miles product with an explicit listener / slider / INI snapshot.
pub fn live_gameplay_sfx_volume_with(
    event_name: &str,
    position_host_yup: Option<(f32, f32, f32)>,
    listener_leftover: game_engine::common::audio::Coord3D,
    sliders: &game_engine::common::audio::MilesVolumeSliders,
    info: Option<std::sync::Arc<game_engine::common::audio::AudioEventInfo>>,
) -> f32 {
    use game_engine::common::audio::{AudioEventRts, Coord3D, miles_get_effective_volume};

    let mut event = if let Some((x, y, z)) = position_host_yup {
        let leftover = Coord3D { x, y: z, z: y };
        AudioEventRts::with_position(event_name, &leftover)
    } else {
        AudioEventRts::with_event_name(event_name)
    };
    // C++ `generatePlayInfo` with VolumeShift 0 → multiplier 1.0.
    event.set_volume_shift(1.0);
    if let Some(info) = info {
        event.set_audio_event_info(info);
    }
    miles_get_effective_volume(&event, &listener_leftover, sliders)
}

fn leftover_volume_inputs(
    event_name: &str,
) -> (
    game_engine::common::audio::Coord3D,
    game_engine::common::audio::MilesVolumeSliders,
    Option<std::sync::Arc<game_engine::common::audio::AudioEventInfo>>,
) {
    use game_engine::common::audio::{Coord3D, MilesVolumeSliders};
    let defaults = (Coord3D::new(), MilesVolumeSliders::default(), None);
    let Some(mgr) = game_engine::common::audio::game_audio::get_global_audio_manager() else {
        return defaults;
    };
    let Ok(guard) = mgr.try_lock() else {
        return defaults;
    };
    (
        *guard.get_listener_position(),
        guard.miles_volume_sliders(),
        guard.find_audio_event_info(event_name),
    )
}

/// AudioManager - Main audio management class (mirrors C++ AudioManager)
/// Handles all audio operations including music, sound effects, and voice
pub struct AudioManager {
    /// Keeps the cpal/rodio output stream alive on its owner thread.
    #[allow(dead_code)]
    output: Option<OutputStreamKeepalive>,
    /// Already `Send + Sync` rodio mixer handle used to create sinks.
    pub handle: Option<OutputStreamHandle>,

    // Multi-channel audio system matching C++ implementation
    audio_channels: HashMap<AudioAffect, Vec<AudioChannel>>,
    channel_volumes: HashMap<AudioAffect, f32>,
    channel_enabled: HashMap<AudioAffect, bool>,

    // Legacy single-channel support for backward compatibility
    background_music: Option<Sink>,
    sound_effects: Vec<Sink>,
    current_music_track: Option<String>,

    // Global audio settings (matching C++ member variables)
    master_volume: f32,
    music_volume: f32,
    sfx_volume: f32,
    speech_volume: f32,
    ui_volume: f32,
    ambient_volume: f32,

    // Audio system state (matching C++ state management)
    is_music_already_loaded: bool,
    max_concurrent_sounds: usize,
    last_update_time: f32,
    cleanup_accumulator: f32,
}

impl AudioManager {
    /// Initialize AudioManager (matches C++ constructor)
    pub fn new() -> Result<Self> {
        let mut audio_channels = HashMap::new();
        let mut channel_volumes = HashMap::new();
        let mut channel_enabled = HashMap::new();

        // Initialize audio channels matching C++ pattern
        for affect in [
            AudioAffect::Music,
            AudioAffect::Sound,
            AudioAffect::Sound3D,
            AudioAffect::Speech,
            AudioAffect::UI,
            AudioAffect::Ambient,
        ] {
            audio_channels.insert(affect, Vec::new());
            channel_volumes.insert(affect, 0.7);
            channel_enabled.insert(affect, true);
        }

        Ok(Self {
            // Keep construction lightweight. The original C++ startup path does not block the
            // shell on immediate device activation, and opening the host audio device here can
            // stall first-frame startup badly on some platforms.
            output: None,
            handle: None,
            audio_channels,
            channel_volumes,
            channel_enabled,
            background_music: None,
            sound_effects: Vec::new(),
            current_music_track: None,
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 0.8,
            speech_volume: 0.8,
            ui_volume: 0.9,
            ambient_volume: 0.6,
            is_music_already_loaded: false,
            max_concurrent_sounds: 32, // Match typical C++ limits
            last_update_time: 0.0,
            cleanup_accumulator: 0.0,
        })
    }

    fn ensure_output_device(&mut self) -> Result<()> {
        if self.handle.is_some() {
            return Ok(());
        }

        let (output, handle) = spawn_output_stream_owner()?;
        self.output = Some(output);
        self.handle = Some(handle);
        info!("Audio output device activated");
        Ok(())
    }

    /// Play background music (matches C++ playBackgroundMusic)
    pub async fn play_background_music(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        track_name: &str,
    ) -> Result<()> {
        self.ensure_output_device()?;

        let resolved_track = resolve_archive_audio_path(archive_system, track_name)
            .unwrap_or_else(|| track_name.to_string());
        info!(
            "Loading background music: {} (resolved: {})",
            track_name, resolved_track
        );

        // Stop current music if playing
        if let Some(ref music) = self.background_music {
            music.stop();
        }

        // Try to load from archive with better diagnostics
        let audio_data = match archive_system.open_file(&resolved_track).await {
            Ok(data) => {
                info!(
                    "✅ Successfully loaded audio file: {} ({} bytes)",
                    resolved_track,
                    data.len()
                );
                data
            }
            Err(e) => {
                error!(
                    "❌ Failed to load audio file {} (requested: {}): {}",
                    resolved_track, track_name, e
                );

                // Try to provide helpful diagnostics
                if archive_system.does_file_exist(&resolved_track) {
                    warn!(
                        "📋 File exists in archives but cannot be extracted - this may be a BIG file format issue"
                    );
                } else {
                    warn!("📋 File not found in any loaded archives");

                    // Show available music files for debugging
                    let all_files = archive_system.list_all_files();
                    let music_files: Vec<_> = all_files
                        .iter()
                        .filter(|f| {
                            f.to_lowercase().contains("audio")
                                && (f.ends_with(".mp3")
                                    || f.ends_with(".ogg")
                                    || f.ends_with(".wav"))
                        })
                        .collect();

                    if !music_files.is_empty() {
                        warn!("📋 Available audio files in archives:");
                        for (i, file) in music_files.iter().take(10).enumerate() {
                            warn!("   {}: {}", i + 1, file);
                        }
                        if music_files.len() > 10 {
                            warn!("   ... and {} more audio files", music_files.len() - 10);
                        }
                    }
                }

                return Err(anyhow!(
                    "Failed to load audio file {} (requested {}): {}",
                    resolved_track,
                    track_name,
                    e
                ));
            }
        };

        // Create cursor for audio data
        let cursor = Cursor::new(audio_data);

        // Create decoder based on file format
        let format = AudioFormat::from_filename(&resolved_track);
        debug!("Audio format detected: {:?}", format);

        // Create decoder with proper error handling to prevent audio noise
        let source = match Decoder::new(cursor) {
            Ok(decoder) => {
                // Convert to f32 samples to prevent audio corruption and noise
                let f32_source = decoder.convert_samples::<f32>();
                f32_source.repeat_infinite()
            }
            Err(e) => {
                error!("Failed to decode audio file {}: {}", resolved_track, e);
                return Err(anyhow!("Failed to decode audio file: {}", e));
            }
        };

        // Create sink and play
        if let Some(ref handle) = self.handle {
            match Sink::try_new(handle) {
                Ok(sink) => {
                    sink.set_volume(self.music_volume * self.master_volume);
                    sink.append(source);
                    self.background_music = Some(sink);
                    self.current_music_track = Some(resolved_track.clone());
                    self.is_music_already_loaded = true;
                    info!("Started playing background music: {}", resolved_track);
                }
                Err(e) => {
                    error!("Failed to create audio sink: {}", e);
                    return Err(anyhow!("Failed to create audio sink: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Play sound effect (matches C++ `TheAudio->addAudioEvent`).
    ///
    /// Local archive/rodio decode is leftover and used only when Common
    /// TheAudio / AudioManager produced no live handle.
    pub async fn play_sound_effect(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        sound_name: &str,
    ) -> Result<()> {
        self.play_sound_effect_scaled(archive_system, sound_name, 1.0)
            .await
    }

    /// Rodio leftover path with a Miles effective-volume scale.
    ///
    /// `volume_scale` is C++ `getEffectiveVolume` (INI Volume × 3D slider /
    /// zoom × inverse-distance). Zero mutes at max range without decoding.
    pub async fn play_sound_effect_scaled(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        sound_name: &str,
        volume_scale: f32,
    ) -> Result<()> {
        if volume_scale <= 0.0 {
            return Ok(());
        }
        if play_sound_through_the_audio(sound_name).is_some() {
            return Ok(());
        }

        self.ensure_output_device()?;

        debug!("Playing sound effect: {}", sound_name);

        // Clean up finished sound effects
        self.sound_effects.retain(|sink| !sink.empty());

        // Enforce max concurrent sounds limit
        if self.sound_effects.len() >= self.max_concurrent_sounds {
            // Remove oldest sound effect
            if let Some(oldest) = self.sound_effects.first() {
                oldest.stop();
            }
            self.sound_effects.remove(0);
        }

        // Load sound from archive
        let audio_data = archive_system
            .open_file(sound_name)
            .await
            .map_err(|e| anyhow!("Failed to load sound effect {}: {}", sound_name, e))?;

        // Create cursor and decoder
        let cursor = Cursor::new(audio_data);

        // Create decoder with proper noise prevention
        let source = Decoder::new(cursor)
            .map_err(|e| anyhow!("Failed to decode sound effect {}: {}", sound_name, e))?
            .convert_samples::<f32>(); // Convert to f32 to prevent audio noise

        // Create sink and play
        if let Some(ref handle) = self.handle {
            match Sink::try_new(handle) {
                Ok(sink) => {
                    sink.set_volume(
                        (self.sfx_volume * self.master_volume * volume_scale).clamp(0.0, 1.0),
                    );
                    sink.append(source);
                    self.sound_effects.push(sink);
                    debug!("Started playing sound effect: {}", sound_name);
                }
                Err(e) => {
                    error!("Failed to create sound effect sink: {}", e);
                    return Err(anyhow!("Failed to create sound effect sink: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Pause audio (matches C++ pauseAudio)
    pub fn pause_audio(&self, affect: AudioAffect) {
        match affect {
            AudioAffect::Music => {
                if let Some(ref music) = self.background_music {
                    music.pause();
                    info!("Music paused");
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                for sink in &self.sound_effects {
                    sink.pause();
                }
                info!("Sound effects paused");
            }
            _ => {
                // For other types, pause all
                if let Some(ref music) = self.background_music {
                    music.pause();
                }
                for sink in &self.sound_effects {
                    sink.pause();
                }
                info!("All audio paused for affect: {:?}", affect);
            }
        }
    }

    /// Resume audio (matches C++ resumeAudio)
    pub fn resume_audio(&self, affect: AudioAffect) {
        match affect {
            AudioAffect::Music => {
                if let Some(ref music) = self.background_music {
                    music.play();
                    info!("Music resumed");
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                for sink in &self.sound_effects {
                    sink.play();
                }
                info!("Sound effects resumed");
            }
            _ => {
                // For other types, resume all
                if let Some(ref music) = self.background_music {
                    music.play();
                }
                for sink in &self.sound_effects {
                    sink.play();
                }
                info!("All audio resumed for affect: {:?}", affect);
            }
        }
    }

    /// Set audio affect on/off (matches C++ AudioManager::setOn)
    pub fn set_on(&mut self, enabled: bool, affect: AudioAffect) {
        self.channel_enabled.insert(affect, enabled);

        // Apply to existing channels
        if let Some(channels) = self.audio_channels.get_mut(&affect) {
            for channel in &mut *channels {
                channel.enabled = enabled;
                if enabled {
                    channel.sink.set_volume(channel.volume * self.master_volume);
                } else {
                    channel.sink.set_volume(0.0);
                }
            }
        }

        // Apply to legacy channels as well
        match affect {
            AudioAffect::Music => {
                if let Some(ref music) = self.background_music {
                    if enabled {
                        music.set_volume(self.music_volume * self.master_volume);
                    } else {
                        music.set_volume(0.0);
                    }
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                for sink in &self.sound_effects {
                    if enabled {
                        sink.set_volume(self.sfx_volume * self.master_volume);
                    } else {
                        sink.set_volume(0.0);
                    }
                }
            }
            _ => {}
        }

        info!("Audio affect {:?} set to: {}", affect, enabled);
    }

    /// Check if music is already loaded (matches C++ AudioManager::isMusicAlreadyLoaded)
    pub fn is_music_already_loaded(&self) -> bool {
        self.is_music_already_loaded
    }

    /// Set music loaded state (matches C++ setMusicLoaded)
    pub fn set_music_loaded(&mut self, loaded: bool) {
        self.is_music_already_loaded = loaded;
    }

    /// Set volume for specific audio affect type (matches C++ setVolume)
    pub fn set_volume(&mut self, affect: AudioAffect, volume: f32) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        self.channel_volumes.insert(affect, clamped_volume);

        // Update existing channels
        if let Some(channels) = self.audio_channels.get_mut(&affect) {
            for channel in &mut *channels {
                channel.volume = clamped_volume;
                if channel.enabled {
                    channel.sink.set_volume(clamped_volume * self.master_volume);
                }
            }
        }

        // Update legacy volume settings
        match affect {
            AudioAffect::Music => {
                self.music_volume = clamped_volume;
                if let Some(ref music) = self.background_music {
                    music.set_volume(clamped_volume * self.master_volume);
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                self.sfx_volume = clamped_volume;
                for sink in &self.sound_effects {
                    sink.set_volume(clamped_volume * self.master_volume);
                }
            }
            AudioAffect::Speech => self.speech_volume = clamped_volume,
            AudioAffect::UI => self.ui_volume = clamped_volume,
            AudioAffect::Ambient => self.ambient_volume = clamped_volume,
        }

        info!("Volume for {:?} set to: {:.2}", affect, clamped_volume);
    }

    /// Get volume for specific audio affect type (matches C++ getVolume)
    pub fn get_volume(&self, affect: AudioAffect) -> f32 {
        match affect {
            AudioAffect::Music => self.music_volume,
            AudioAffect::Sound | AudioAffect::Sound3D => self.sfx_volume,
            AudioAffect::Speech => self.speech_volume,
            AudioAffect::UI => self.ui_volume,
            AudioAffect::Ambient => self.ambient_volume,
        }
    }

    /// Set master volume (matches C++ setMasterVolume)
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);

        // Update all active audio with new master volume
        if let Some(ref music) = self.background_music {
            music.set_volume(self.music_volume * self.master_volume);
        }

        for sink in &self.sound_effects {
            sink.set_volume(self.sfx_volume * self.master_volume);
        }

        // Update channel audio as well
        for (affect, channels) in &mut self.audio_channels {
            let base_volume = self.channel_volumes.get(affect).unwrap_or(&0.7);
            for channel in &mut *channels {
                if channel.enabled {
                    channel.sink.set_volume(base_volume * self.master_volume);
                }
            }
        }

        info!("Master volume set to: {:.2}", self.master_volume);
    }

    /// Get master volume (matches C++ getMasterVolume)
    pub fn get_master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Stop all audio (matches C++ stopAllAudio)
    pub fn stop_all_audio(&mut self) {
        self.stop_background_music();

        for sink in &self.sound_effects {
            sink.stop();
        }
        self.sound_effects.clear();

        // Stop all channel audio
        for channels in self.audio_channels.values_mut() {
            for channel in &*channels {
                channel.sink.stop();
            }
            channels.clear();
        }

        info!("All audio stopped");
    }

    /// Stop all sounds (matches C++ stopAllSounds)
    pub fn stop_all_sounds(&mut self) {
        for sink in &self.sound_effects {
            sink.stop();
        }
        self.sound_effects.clear();

        // Stop sound effects in channels
        for (affect, channels) in &mut self.audio_channels {
            if *affect == AudioAffect::Sound || *affect == AudioAffect::Sound3D {
                for channel in &*channels {
                    channel.sink.stop();
                }
                channels.clear();
            }
        }

        info!("All sound effects stopped");
    }

    /// Pause background music (matches C++ pauseBackgroundMusic)
    pub fn pause_background_music(&self) {
        if let Some(ref music) = self.background_music {
            music.pause();
            info!("Background music paused");
        }
    }

    /// Resume background music (matches C++ resumeBackgroundMusic)
    pub fn resume_background_music(&self) {
        if let Some(ref music) = self.background_music {
            music.play();
            info!("Background music resumed");
        }
    }

    /// Stop background music (matches C++ stopBackgroundMusic)
    pub fn stop_background_music(&mut self) {
        if let Some(ref music) = self.background_music {
            music.stop();
            info!("Background music stopped");
        }
        self.background_music = None;
        self.current_music_track = None;
        self.is_music_already_loaded = false;
    }

    /// Toggle background music pause/resume (matches C++ toggleBackgroundMusic)
    pub fn toggle_background_music(&self) {
        if let Some(ref music) = self.background_music {
            if music.is_paused() {
                music.play();
                info!("Background music resumed");
            } else {
                music.pause();
                info!("Background music paused");
            }
        }
    }

    /// Set music volume (matches C++ setMusicVolume)
    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
        if let Some(ref music) = self.background_music {
            music.set_volume(self.music_volume * self.master_volume);
        }
        info!("Music volume set to: {:.2}", self.music_volume);
    }

    /// Set sound effects volume (matches C++ setSFXVolume)
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
        for sink in &self.sound_effects {
            sink.set_volume(self.sfx_volume * self.master_volume);
        }
        info!("Sound effects volume set to: {:.2}", self.sfx_volume);
    }

    /// Get current music volume (matches C++ getMusicVolume)
    pub fn get_music_volume(&self) -> f32 {
        self.music_volume
    }

    /// Get current sound effects volume (matches C++ getSFXVolume)
    pub fn get_sfx_volume(&self) -> f32 {
        self.sfx_volume
    }

    /// Get currently playing track name (matches C++ getCurrentTrack)
    pub fn get_current_track(&self) -> Option<&str> {
        self.current_music_track.as_deref()
    }

    /// Check if background music is playing (matches C++ isMusicPlaying)
    pub fn is_music_playing(&self) -> bool {
        if let Some(ref music) = self.background_music {
            !music.is_paused() && !music.empty()
        } else {
            false
        }
    }

    /// Play audio with specific affect type (matches C++ playAudioWithAffect)
    pub async fn play_audio_with_affect(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        sound_name: &str,
        affect: AudioAffect,
        priority: i32,
    ) -> Result<()> {
        self.ensure_output_device()?;

        // Check if this affect type is enabled
        if !self.channel_enabled.get(&affect).unwrap_or(&true) {
            debug!(
                "Audio affect {:?} is disabled, skipping {}",
                affect, sound_name
            );
            return Ok(());
        }

        // Load audio data
        let audio_data = archive_system
            .open_file(sound_name)
            .await
            .map_err(|e| anyhow!("Failed to load audio file {}: {}", sound_name, e))?;

        let cursor = Cursor::new(audio_data);

        // Create decoder with noise prevention
        let source = Decoder::new(cursor)
            .map_err(|e| anyhow!("Failed to decode audio file {}: {}", sound_name, e))?
            .convert_samples::<f32>(); // Convert to f32 to prevent audio noise

        // Create sink
        if let Some(ref handle) = self.handle {
            match Sink::try_new(handle) {
                Ok(sink) => {
                    let base_volume = self.channel_volumes.get(&affect).unwrap_or(&0.7);
                    let final_volume = base_volume * self.master_volume;

                    sink.set_volume(final_volume);
                    sink.append(source);

                    let channel = AudioChannel {
                        sink,
                        affect_type: affect,
                        volume: *base_volume,
                        enabled: true,
                        priority,
                    };

                    // Add to appropriate channel list
                    if let Some(channels) = self.audio_channels.get_mut(&affect) {
                        // Enforce max concurrent sounds
                        if affect == AudioAffect::Sound
                            && channels.len() >= self.max_concurrent_sounds
                        {
                            // Remove oldest sound
                            if let Some(oldest) = channels.first() {
                                oldest.sink.stop();
                            }
                            channels.remove(0);
                        }
                        channels.push(channel);
                    }

                    debug!("Started playing {} with affect {:?}", sound_name, affect);
                }
                Err(e) => {
                    error!("Failed to create audio sink: {}", e);
                    return Err(anyhow!("Failed to create audio sink: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Stop all sounds of a specific affect type (matches C++ stopAffect)
    pub fn stop_affect(&mut self, affect: AudioAffect) {
        if let Some(channels) = self.audio_channels.get_mut(&affect) {
            for channel in &*channels {
                channel.sink.stop();
            }
            channels.clear();
        }

        // Handle legacy channels as well
        match affect {
            AudioAffect::Music => self.stop_background_music(),
            AudioAffect::Sound | AudioAffect::Sound3D => self.stop_all_sounds(),
            _ => {}
        }

        info!("Stopped all audio for affect: {:?}", affect);
    }

    fn cleanup_finished_sounds(&mut self) {
        // Clean up finished sounds in all channels
        for channels in self.audio_channels.values_mut() {
            channels.retain(|channel| !channel.sink.empty());
        }

        // Clean up legacy sound effects
        self.sound_effects.retain(|sink| !sink.empty());
    }

    /// Update audio system (matches C++ update) - call every frame
    pub fn update(&mut self) {
        self.cleanup_finished_sounds();
        self.cleanup_accumulator = 0.0;
    }

    /// Update the audio system using authoritative timing from the WW3D engine.
    pub fn update_with_time(&mut self, delta_time: f32, total_time: f32) {
        self.cleanup_accumulator += delta_time.max(0.0);
        if self.cleanup_accumulator >= 0.016 {
            self.cleanup_finished_sounds();
            self.cleanup_accumulator = 0.0;
        }
        self.last_update_time = total_time;
    }

    /// Play boot/menu music through TheAudio authored event (C++ Music.ini `Shell`).
    ///
    /// C++ has no random 17-track rodio sink. All music is `TheAudio->addAudioEvent`.
    /// If TheAudio is missing or Music is off (`-nomusic`), stay silent until a
    /// script queues a track.
    pub async fn play_random_cnc_music(
        &mut self,
        _archive_system: &mut ArchiveFileSystem,
    ) -> Result<()> {
        if let Some(music) = self.background_music.take() {
            music.stop();
        }
        if play_sound_through_the_audio("Shell").is_some() {
            info!("Playing boot/menu music through TheAudio event Shell");
        } else {
            info!("Boot/menu music silent until scripted TheAudio event");
        }
        Ok(())
    }

    /// Play specific faction music (matches C++ playFactionMusic)
    pub async fn play_faction_music(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        faction: &str,
    ) -> Result<()> {
        let Some(track_candidates) = faction_music_candidates(faction) else {
            return Err(anyhow!("Unknown faction music set: {faction}"));
        };

        // Try each track until we find one that exists
        for track_name in track_candidates {
            if let Some(resolved) = resolve_archive_audio_path(archive_system, track_name) {
                info!("Playing faction music for {}: {}", faction, resolved);
                return self.play_background_music(archive_system, &resolved).await;
            }
        }

        Err(anyhow!("No {faction} music tracks found in archives"))
    }
}

fn faction_music_candidates(faction: &str) -> Option<&'static [&'static str]> {
    match faction.to_lowercase().as_str() {
        "usa" => Some(&["usa_10.mp3", "usa_11.mp3", "USA01.mp3"]),
        "china" => Some(&["chi_10.mp3", "chi_11.mp3", "c_chix01.mp3", "China01.mp3"]),
        "gla" => Some(&["gla_10.mp3", "gla_11.mp3", "GLA01.mp3"]),
        _ => None,
    }
}

fn build_audio_track_candidates(track_name: &str) -> Vec<String> {
    let normalized = track_name.replace('\\', "/");
    let trimmed = normalized.trim_matches('/').to_string();
    let file_name = trimmed
        .rsplit('/')
        .next()
        .map(str::to_string)
        .unwrap_or_else(|| trimmed.clone());

    let mut raw = vec![
        trimmed.clone(),
        trimmed.to_lowercase(),
        file_name.clone(),
        file_name.to_lowercase(),
        format!("Data/Audio/Tracks/{}", file_name),
        format!("data/audio/tracks/{}", file_name),
        format!("Audio/Tracks/{}", file_name),
        format!("audio/tracks/{}", file_name),
    ];

    if trimmed.contains('/') {
        raw.push(format!("Data/{}", trimmed));
        raw.push(format!("data/{}", trimmed.to_lowercase()));
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for candidate in raw {
        let canonical = candidate.replace('\\', "/");
        if seen.insert(canonical.to_lowercase()) {
            deduped.push(canonical);
        }
    }

    deduped
}

fn resolve_archive_audio_path(
    archive_system: &ArchiveFileSystem,
    track_name: &str,
) -> Option<String> {
    let candidates = build_audio_track_candidates(track_name);
    for candidate in &candidates {
        if archive_system.does_file_exist(candidate) {
            return Some(candidate.clone());
        }
    }

    let all_files = archive_system.list_all_files();
    let wanted: Vec<String> = candidates.into_iter().map(|c| c.to_lowercase()).collect();
    for file in all_files {
        let normalized = file.replace('\\', "/");
        let lower = normalized.to_lowercase();
        for suffix in &wanted {
            if lower == *suffix || lower.ends_with(&format!("/{}", suffix)) {
                return Some(normalized);
            }
        }
    }

    None
}

/// Utility functions for audio file discovery

/// Load and list available music tracks from archives (matches C++ getAvailableMusicTracks)
pub fn get_available_music_tracks(archive_system: &ArchiveFileSystem) -> Vec<String> {
    let mut tracks = Vec::new();
    let all_files = archive_system.list_all_files();

    for file in all_files {
        let file_lower = file.to_lowercase();
        if (file_lower.contains("music") || file_lower.contains("audio"))
            && (file_lower.ends_with(".mp3")
                || file_lower.ends_with(".ogg")
                || file_lower.ends_with(".wav"))
        {
            tracks.push(file);
        }
    }

    tracks.sort();
    tracks
}

/// Load and list available sound effects from archives (matches C++ getAvailableSoundEffects)
pub fn get_available_sound_effects(archive_system: &ArchiveFileSystem) -> Vec<String> {
    let mut sounds = Vec::new();
    let all_files = archive_system.list_all_files();

    for file in all_files {
        let file_lower = file.to_lowercase();
        if (file_lower.contains("sound")
            || file_lower.contains("sfx")
            || file_lower.contains("audio"))
            && (file_lower.ends_with(".mp3")
                || file_lower.ends_with(".ogg")
                || file_lower.ends_with(".wav"))
        {
            sounds.push(file);
        }
    }

    sounds.sort();
    sounds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faction_music_candidates_do_not_cross_fallback_to_other_factions() {
        let usa = faction_music_candidates("USA").expect("usa candidates");
        assert!(usa.iter().all(|track| {
            let lower = track.to_ascii_lowercase();
            lower.starts_with("usa") || lower.starts_with("c_usa")
        }));

        let china = faction_music_candidates("china").expect("china candidates");
        assert!(china.iter().all(|track| {
            let lower = track.to_ascii_lowercase();
            lower.starts_with("chi") || lower.starts_with("c_chi") || lower.starts_with("china")
        }));

        let gla = faction_music_candidates("gla").expect("gla candidates");
        assert!(
            gla.iter()
                .all(|track| track.to_ascii_lowercase().starts_with("gla"))
        );

        assert!(faction_music_candidates("unknown").is_none());
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn send_sync_wrapper_is_gone_and_audio_types_are_honestly_send() {
        // The old generic wrapper marked every T as thread-safe. That type must stay gone.
        let src = include_str!("audio.rs");
        let production_src = src
            .split("#[cfg(test)]")
            .next()
            .expect("audio.rs should keep a test module after production code");
        assert!(
            !production_src.contains("struct SendSyncWrapper"),
            "SendSyncWrapper must remain removed; do not restore a generic Send/Sync lie"
        );
        assert!(
            !production_src.contains("unsafe impl<T>"),
            "blanket unsafe Send/Sync for arbitrary T is forbidden"
        );

        // rodio 0.17: control types are already Send + Sync. OutputStream is not
        // (cpal::Stream / NotSendSyncAcrossAllPlatforms); it lives on a dedicated
        // owner thread so AudioManager can sit in Mutex<AssetManager>.
        assert_send::<rodio::Sink>();
        assert_send::<rodio::SpatialSink>();
        assert_send::<rodio::OutputStreamHandle>();
        assert_sync::<rodio::Sink>();
        assert_sync::<rodio::SpatialSink>();
        assert_sync::<rodio::OutputStreamHandle>();
        assert_send::<AudioManager>();
        assert_send::<OutputStreamKeepalive>();

        let manager = AudioManager::new().expect("AudioManager constructs without a device");
        assert!(manager.handle.is_none());
        assert!(!manager.is_music_playing());
        assert_eq!(manager.get_master_volume(), 1.0);
    }

    #[test]
    fn play_sound_through_the_audio_requires_live_common_handle() {
        // C++ AudioManager::addAudioEvent (GameAudio.cpp) via TheAudio.
        // Pre-fix Main assets::audio opened its own rodio sink and never
        // consulted Common THE_AUDIO. A missing handle must not initialize
        // Miles/rodio from the UI play wrapper.
        if game_engine::common::audio::game_audio::get_global_audio_manager().is_none() {
            assert!(
                play_sound_through_the_audio("UnitSelect").is_none(),
                "UI SFX must not invent a TheAudio handle"
            );
        }

        // Same addAudioEvent contract the live wrapper uses once a handle exists.
        // Local manager: no global init, no OutputStream::try_default hang.
        let mut audio = game_engine::common::audio::AudioManager::new();
        let before = audio.pending_play_request_count();
        let _ = audio.new_audio_event_info("UnitSelect".to_string());
        let event = game_engine::common::audio::AudioEventRts::with_event_name("UnitSelect");
        let handle = audio.add_audio_event(&event);
        assert!(
            handle >= 1000,
            "C++ AHSV_FIRST_HANDLE is 1000; got {handle}"
        );
        let after = audio.pending_play_request_count();
        assert!(
            after > before,
            "Common AudioManager must queue AR_Play (before={before}, after={after})"
        );
    }

    #[test]
    fn live_gameplay_sfx_volume_applies_miles_inverse_falloff() {
        use game_engine::common::audio::{AudioEventInfo, Coord3D, MilesVolumeSliders, ST_WORLD};

        let sliders = MilesVolumeSliders::default();
        let listener = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let info = std::sync::Arc::new(AudioEventInfo {
            audio_name: "AmericaTankFire".to_string(),
            volume: 1.0,
            min_distance: 25.0,
            max_distance: 1000.0,
            type_field: ST_WORLD,
            ..Default::default()
        });

        let at_camera = live_gameplay_sfx_volume_with(
            "AmericaTankFire",
            Some((0.0, 0.0, 0.0)),
            listener,
            &sliders,
            Some(info.clone()),
        );
        let mid = live_gameplay_sfx_volume_with(
            "AmericaTankFire",
            Some((50.0, 0.0, 0.0)),
            listener,
            &sliders,
            Some(info.clone()),
        );
        let silent = live_gameplay_sfx_volume_with(
            "AmericaTankFire",
            Some((1000.0, 0.0, 0.0)),
            listener,
            &sliders,
            Some(info),
        );

        assert!(at_camera > mid, "near camera must be louder than mid range");
        assert!(mid > 0.0);
        assert_eq!(silent, 0.0, "C++ mutes at objDistance >= objMaxDistance");
        // 50/25 → gain 0.5; sound3DVolume default 0.75 → 0.375
        assert!((mid - 0.375).abs() < 1.0e-5);
    }

    #[test]
    fn leftover_world_sfx_event_keeps_host_pose_as_cpp_z_up() {
        use gamelogic::common::audio::LeftoverAudioOwner;
        let event = leftover_world_sfx_event("Explosion", Some((10.0, 4.0, 20.0)), None);
        assert_eq!(event.owner_type, LeftoverAudioOwner::Positional);
        assert_eq!(event.position, Some((10.0, 20.0, 4.0)));
        let ui = leftover_world_sfx_event("GUIClick", None, None);
        assert_eq!(ui.owner_type, LeftoverAudioOwner::Invalid);
        assert!(ui.position.is_none());
    }

    #[test]
    fn leftover_world_sfx_event_stamps_object_id_as_owner() {
        use gamelogic::common::audio::LeftoverAudioOwner;
        let event = leftover_world_sfx_event("VoiceSelect", Some((10.0, 4.0, 20.0)), Some(42));
        assert_eq!(event.owner_type, LeftoverAudioOwner::Object);
        assert_eq!(event.object_id, 42);
        assert!(
            event.position.is_none(),
            "C++ setPosition is a no-op once owner is OT_Object"
        );
        let zero = leftover_world_sfx_event("Explosion", Some((10.0, 4.0, 20.0)), Some(0));
        assert_eq!(zero.owner_type, LeftoverAudioOwner::Positional);
        assert_eq!(zero.object_id, 0);
    }

    #[test]
    fn leftover_the_audio_is_live_matches_global_manager() {
        assert_eq!(
            leftover_the_audio_is_live(),
            game_engine::common::audio::game_audio::get_global_audio_manager().is_some()
        );
    }
}
