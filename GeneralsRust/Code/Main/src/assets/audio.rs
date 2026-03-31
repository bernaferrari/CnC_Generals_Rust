////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// AudioManager - Core audio management system matching C++ implementation
// This file mirrors the structure and functionality of the original C++ AudioManager

use crate::assets::archive::ArchiveFileSystem;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

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
    sink: SendSyncWrapper<Sink>,
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

/// Wrapper to make audio types Send/Sync (controlled usage matching C++ thread safety)
pub struct SendSyncWrapper<T>(T);
unsafe impl<T> Send for SendSyncWrapper<T> {}
unsafe impl<T> Sync for SendSyncWrapper<T> {}

impl<T> SendSyncWrapper<T> {
    pub fn new(value: T) -> Self {
        SendSyncWrapper(value)
    }

    pub fn get(&self) -> &T {
        &self.0
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

/// AudioManager - Main audio management class (mirrors C++ AudioManager)
/// Handles all audio operations including music, sound effects, and voice
pub struct AudioManager {
    #[allow(dead_code)] // Kept alive to prevent audio stream from dropping
    output: Option<SendSyncWrapper<OutputStream>>,
    pub handle: Option<SendSyncWrapper<OutputStreamHandle>>,

    // Multi-channel audio system matching C++ implementation
    audio_channels: HashMap<AudioAffect, Vec<AudioChannel>>,
    channel_volumes: HashMap<AudioAffect, f32>,
    channel_enabled: HashMap<AudioAffect, bool>,

    // Legacy single-channel support for backward compatibility
    background_music: Option<SendSyncWrapper<Sink>>,
    sound_effects: Vec<SendSyncWrapper<Sink>>,
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

        let (output, handle) = OutputStream::try_default()
            .map_err(|e| anyhow!("Failed to initialize audio output: {}", e))?;
        self.output = Some(SendSyncWrapper::new(output));
        self.handle = Some(SendSyncWrapper::new(handle));
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
            music.get().stop();
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
                    warn!("📋 File exists in archives but cannot be extracted - this may be a BIG file format issue");
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
            match Sink::try_new(handle.get()) {
                Ok(sink) => {
                    sink.set_volume(self.music_volume * self.master_volume);
                    sink.append(source);
                    self.background_music = Some(SendSyncWrapper::new(sink));
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

    /// Play sound effect (matches C++ playSoundEffect)
    pub async fn play_sound_effect(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        sound_name: &str,
    ) -> Result<()> {
        self.ensure_output_device()?;

        debug!("Playing sound effect: {}", sound_name);

        // Clean up finished sound effects
        self.sound_effects.retain(|sink| !sink.get().empty());

        // Enforce max concurrent sounds limit
        if self.sound_effects.len() >= self.max_concurrent_sounds {
            // Remove oldest sound effect
            if let Some(oldest) = self.sound_effects.first() {
                oldest.get().stop();
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
            match Sink::try_new(handle.get()) {
                Ok(sink) => {
                    sink.set_volume(self.sfx_volume * self.master_volume);
                    sink.append(source);
                    self.sound_effects.push(SendSyncWrapper::new(sink));
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
                    music.get().pause();
                    info!("Music paused");
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                for sink in &self.sound_effects {
                    sink.get().pause();
                }
                info!("Sound effects paused");
            }
            _ => {
                // For other types, pause all
                if let Some(ref music) = self.background_music {
                    music.get().pause();
                }
                for sink in &self.sound_effects {
                    sink.get().pause();
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
                    music.get().play();
                    info!("Music resumed");
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                for sink in &self.sound_effects {
                    sink.get().play();
                }
                info!("Sound effects resumed");
            }
            _ => {
                // For other types, resume all
                if let Some(ref music) = self.background_music {
                    music.get().play();
                }
                for sink in &self.sound_effects {
                    sink.get().play();
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
                    channel
                        .sink
                        .get()
                        .set_volume(channel.volume * self.master_volume);
                } else {
                    channel.sink.get().set_volume(0.0);
                }
            }
        }

        // Apply to legacy channels as well
        match affect {
            AudioAffect::Music => {
                if let Some(ref music) = self.background_music {
                    if enabled {
                        music
                            .get()
                            .set_volume(self.music_volume * self.master_volume);
                    } else {
                        music.get().set_volume(0.0);
                    }
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                for sink in &self.sound_effects {
                    if enabled {
                        sink.get().set_volume(self.sfx_volume * self.master_volume);
                    } else {
                        sink.get().set_volume(0.0);
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
                    channel
                        .sink
                        .get()
                        .set_volume(clamped_volume * self.master_volume);
                }
            }
        }

        // Update legacy volume settings
        match affect {
            AudioAffect::Music => {
                self.music_volume = clamped_volume;
                if let Some(ref music) = self.background_music {
                    music.get().set_volume(clamped_volume * self.master_volume);
                }
            }
            AudioAffect::Sound | AudioAffect::Sound3D => {
                self.sfx_volume = clamped_volume;
                for sink in &self.sound_effects {
                    sink.get().set_volume(clamped_volume * self.master_volume);
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
            music
                .get()
                .set_volume(self.music_volume * self.master_volume);
        }

        for sink in &self.sound_effects {
            sink.get().set_volume(self.sfx_volume * self.master_volume);
        }

        // Update channel audio as well
        for (affect, channels) in &mut self.audio_channels {
            let base_volume = self.channel_volumes.get(affect).unwrap_or(&0.7);
            for channel in &mut *channels {
                if channel.enabled {
                    channel
                        .sink
                        .get()
                        .set_volume(base_volume * self.master_volume);
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
            sink.get().stop();
        }
        self.sound_effects.clear();

        // Stop all channel audio
        for (_, channels) in &mut self.audio_channels {
            for channel in &*channels {
                channel.sink.get().stop();
            }
            channels.clear();
        }

        info!("All audio stopped");
    }

    /// Stop all sounds (matches C++ stopAllSounds)
    pub fn stop_all_sounds(&mut self) {
        for sink in &self.sound_effects {
            sink.get().stop();
        }
        self.sound_effects.clear();

        // Stop sound effects in channels
        for (affect, channels) in &mut self.audio_channels {
            if *affect == AudioAffect::Sound || *affect == AudioAffect::Sound3D {
                for channel in &*channels {
                    channel.sink.get().stop();
                }
                channels.clear();
            }
        }

        info!("All sound effects stopped");
    }

    /// Pause background music (matches C++ pauseBackgroundMusic)
    pub fn pause_background_music(&self) {
        if let Some(ref music) = self.background_music {
            music.get().pause();
            info!("Background music paused");
        }
    }

    /// Resume background music (matches C++ resumeBackgroundMusic)
    pub fn resume_background_music(&self) {
        if let Some(ref music) = self.background_music {
            music.get().play();
            info!("Background music resumed");
        }
    }

    /// Stop background music (matches C++ stopBackgroundMusic)
    pub fn stop_background_music(&mut self) {
        if let Some(ref music) = self.background_music {
            music.get().stop();
            info!("Background music stopped");
        }
        self.background_music = None;
        self.current_music_track = None;
        self.is_music_already_loaded = false;
    }

    /// Toggle background music pause/resume (matches C++ toggleBackgroundMusic)
    pub fn toggle_background_music(&self) {
        if let Some(ref music) = self.background_music {
            if music.get().is_paused() {
                music.get().play();
                info!("Background music resumed");
            } else {
                music.get().pause();
                info!("Background music paused");
            }
        }
    }

    /// Set music volume (matches C++ setMusicVolume)
    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
        if let Some(ref music) = self.background_music {
            music
                .get()
                .set_volume(self.music_volume * self.master_volume);
        }
        info!("Music volume set to: {:.2}", self.music_volume);
    }

    /// Set sound effects volume (matches C++ setSFXVolume)
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
        for sink in &self.sound_effects {
            sink.get().set_volume(self.sfx_volume * self.master_volume);
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
            !music.get().is_paused() && !music.get().empty()
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
            match Sink::try_new(handle.get()) {
                Ok(sink) => {
                    let base_volume = self.channel_volumes.get(&affect).unwrap_or(&0.7);
                    let final_volume = base_volume * self.master_volume;

                    sink.set_volume(final_volume);
                    sink.append(source);

                    let channel = AudioChannel {
                        sink: SendSyncWrapper::new(sink),
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
                                oldest.sink.get().stop();
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
                channel.sink.get().stop();
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
        for (_, channels) in &mut self.audio_channels {
            channels.retain(|channel| !channel.sink.get().empty());
        }

        // Clean up legacy sound effects
        self.sound_effects.retain(|sink| !sink.get().empty());
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

    /// Load random C&C music track (matches C++ playRandomCNCMusic)
    pub async fn play_random_cnc_music(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<()> {
        // Actual C&C music files found in the BIG archives (short filenames as in C++)
        let music_tracks = vec![
            // Faction-specific tracks (short filenames that definitely exist)
            "usa_10.mp3",
            "usa_11.mp3",
            "chi_10.mp3",
            "chi_11.mp3",
            "gla_10.mp3",
            "gla_11.mp3",
            "c_chix01.mp3",
            // Fallback to older naming convention if available
            "Music01.mp3",
            "Music02.mp3",
            "Music03.mp3",
            "Music04.mp3",
            "Music05.mp3",
            "Music06.mp3",
            "Music07.mp3",
            "Music08.mp3",
            "Music09.mp3",
            "Music10.mp3",
        ];

        // Find available tracks in archives
        let mut available_tracks = Vec::new();
        for track in &music_tracks {
            if let Some(resolved) = resolve_archive_audio_path(archive_system, track) {
                available_tracks.push(resolved);
            }
        }
        available_tracks.sort();
        available_tracks.dedup();

        if available_tracks.is_empty() {
            warn!("No C&C music tracks found in archives");
            return Err(anyhow!("No music tracks available"));
        }

        // Select random track using safer random generation
        let random_index = fastrand::usize(0..available_tracks.len());
        let selected_track = available_tracks[random_index].clone();

        info!("Selected random C&C music track: {}", selected_track);
        self.play_background_music(archive_system, &selected_track)
            .await
    }

    /// Play specific faction music (matches C++ playFactionMusic)
    pub async fn play_faction_music(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        faction: &str,
    ) -> Result<()> {
        let track_candidates = match faction.to_lowercase().as_str() {
            "usa" => vec![
                "usa_10.mp3",
                "usa_11.mp3",
                "USA01.mp3", // Fallback
            ],
            "china" => vec![
                "chi_10.mp3",
                "chi_11.mp3",
                "c_chix01.mp3",
                "China01.mp3", // Fallback
            ],
            "gla" => vec![
                "gla_10.mp3",
                "gla_11.mp3",
                "GLA01.mp3", // Fallback
            ],
            _ => vec![
                "usa_10.mp3",  // Default to USA music
                "Music01.mp3", // Ultimate fallback
            ],
        };

        // Try each track until we find one that exists
        for track_name in &track_candidates {
            if let Some(resolved) = resolve_archive_audio_path(archive_system, track_name) {
                info!("Playing faction music for {}: {}", faction, resolved);
                return self.play_background_music(archive_system, &resolved).await;
            }
        }

        // If no faction music found, try a generic track
        warn!(
            "No faction music found for {}, trying fallback tracks",
            faction
        );
        let fallback_tracks = vec![
            "Data/Audio/Tracks/USA_10.mp3",
            "Data/Audio/Tracks/CHI_10.mp3",
            "Data/Audio/Tracks/GLA_10.mp3",
            "Music01.mp3",
        ];

        for track_name in &fallback_tracks {
            if let Some(resolved) = resolve_archive_audio_path(archive_system, track_name) {
                return self.play_background_music(archive_system, &resolved).await;
            }
        }

        Err(anyhow!("No music tracks found in archives"))
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
