use log::{debug, error, info, warn};
use std::collections::HashMap;
/// Basic audio manager implementation
///
/// This provides a foundation for audio functionality that can be used
/// by the game engine through the active backend implementation.
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::common::resource_manager::{ResourceManager, ResourceType};
use crate::common::system::subsystem_interface::{SubsystemResult, SubsystemState};

/// Audio playback state
#[derive(Debug, Clone, PartialEq)]
pub enum AudioState {
    Stopped,
    Playing,
    Paused,
    Loading,
}

/// Audio handle for tracking playing sounds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioHandle(u32);

impl AudioHandle {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn invalid() -> Self {
        Self(u32::MAX)
    }

    pub fn is_valid(&self) -> bool {
        self.0 != u32::MAX
    }
}

/// Audio source information
#[derive(Debug, Clone)]
pub struct AudioSource {
    pub handle: AudioHandle,
    pub resource_name: String,
    pub volume: f32,
    pub base_volume: f32,
    pub state: AudioState,
    pub position: Option<[f32; 3]>, // 3D position if applicable
    pub looping: bool,
    pub playback_elapsed: Duration,
    pub estimated_duration: Option<Duration>,
}

/// Basic audio manager implementation
pub struct BasicAudioManager {
    /// Master volume (0.0 to 1.0)
    master_volume: f32,
    /// Whether audio is enabled at all
    audio_enabled: bool,
    /// Currently playing audio sources
    playing_sources: HashMap<AudioHandle, AudioSource>,
    /// Next handle ID to assign
    next_handle_id: u32,
    /// Reference to resource manager for loading audio files
    resource_manager: Arc<Mutex<ResourceManager>>,
    /// Audio system state
    state: SubsystemState,
}

impl BasicAudioManager {
    fn estimate_audio_duration(
        resource_name: &str,
        data: &[u8],
        file_path: &std::path::Path,
    ) -> Option<Duration> {
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();

        if extension == "wav" {
            if let Ok(reader) = hound::WavReader::new(Cursor::new(data)) {
                let spec = reader.spec();
                if spec.sample_rate > 0 {
                    let seconds = reader.duration() as f64 / spec.sample_rate as f64;
                    if seconds.is_finite() && seconds > 0.0 {
                        return Some(Duration::from_secs_f64(seconds.min(600.0)));
                    }
                }
            }
        }

        // Fallback heuristic for non-WAV or malformed data.
        if data.is_empty() {
            return Some(Duration::from_millis(500));
        }
        let seconds = (data.len() as f64 * 8.0) / 128_000.0;
        if !seconds.is_finite() || seconds <= 0.0 {
            warn!(
                "Audio duration fallback failed for '{}'; using default 2s",
                resource_name
            );
            return Some(Duration::from_secs(2));
        }
        Some(Duration::from_secs_f64(seconds.clamp(0.2, 600.0)))
    }

    /// Create a new basic audio manager
    pub fn new() -> Self {
        Self {
            master_volume: 1.0,
            audio_enabled: true,
            playing_sources: HashMap::new(),
            next_handle_id: 1,
            resource_manager: crate::common::resource_manager::get_resource_manager(),
            state: SubsystemState::Uninitialized,
        }
    }

    /// Initialize the audio system
    pub fn init(&mut self) -> SubsystemResult<()> {
        info!("Initializing BasicAudioManager");

        // Check if audio should be disabled (e.g., in headless mode)
        if std::env::var("AUDIO_DISABLED").is_ok() || std::env::var("CI").is_ok() {
            warn!("Audio disabled by environment");
            self.audio_enabled = false;
        }

        if self.audio_enabled {
            info!(
                "Audio system enabled with master volume: {:.2}",
                self.master_volume
            );
        } else {
            info!("Audio system running in silent mode");
        }

        self.state = SubsystemState::Running;
        Ok(())
    }

    /// Update the audio system
    pub fn update(&mut self, delta_time: Duration) -> SubsystemResult<()> {
        if !self.audio_enabled {
            return Ok(());
        }

        // Advance tracked playback time and retire non-looping sounds that reached duration.
        self.playing_sources.retain(|_, source| {
            if source.state != AudioState::Playing {
                return true;
            }
            if source.looping {
                return true;
            }
            if let Some(limit) = source.estimated_duration {
                source.playback_elapsed = source.playback_elapsed.saturating_add(delta_time);
                if source.playback_elapsed >= limit {
                    debug!("Audio source '{}' finished playback", source.resource_name);
                    return false;
                }
            }
            true
        });

        Ok(())
    }

    /// Shutdown the audio system
    pub fn shutdown(&mut self) -> SubsystemResult<()> {
        info!("Shutting down BasicAudioManager");

        // Stop all playing sounds
        self.stop_all_sounds();

        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    /// Play an audio resource
    pub fn play_sound(&mut self, resource_name: &str, volume: f32, looping: bool) -> AudioHandle {
        if !self.audio_enabled {
            return AudioHandle::invalid();
        }

        // Assign a new handle
        let handle = AudioHandle::new(self.next_handle_id);
        self.next_handle_id += 1;

        // Try to load the resource
        let resource_manager = self.resource_manager.lock().unwrap();
        match resource_manager.load_resource(resource_name) {
            Ok(resource_data) => {
                if resource_data.info.resource_type != ResourceType::Audio {
                    warn!("Resource {} is not an audio file", resource_name);
                    return AudioHandle::invalid();
                }

                // Create audio source
                let base_volume = volume.clamp(0.0, 1.0);
                let source = AudioSource {
                    handle,
                    resource_name: resource_name.to_string(),
                    volume: (base_volume * self.master_volume).clamp(0.0, 1.0),
                    base_volume,
                    state: AudioState::Playing,
                    position: None,
                    looping,
                    playback_elapsed: Duration::ZERO,
                    estimated_duration: if looping {
                        None
                    } else {
                        Self::estimate_audio_duration(
                            resource_name,
                            &resource_data.data,
                            &resource_data.info.file_path,
                        )
                    },
                };

                self.playing_sources.insert(handle, source);

                debug!(
                    "Started playing audio: {} (handle: {:?}, volume: {:.2})",
                    resource_name, handle, volume
                );
            }
            Err(err) => {
                error!("Failed to load audio resource {}: {}", resource_name, err);
                return AudioHandle::invalid();
            }
        }

        handle
    }

    /// Play a 3D positioned sound
    pub fn play_sound_3d(
        &mut self,
        resource_name: &str,
        position: [f32; 3],
        volume: f32,
    ) -> AudioHandle {
        let handle = self.play_sound(resource_name, volume, false);

        if handle.is_valid() {
            if let Some(source) = self.playing_sources.get_mut(&handle) {
                source.position = Some(position);
                debug!("Playing 3D audio at position: {:?}", position);
            }
        }

        handle
    }

    /// Stop a playing sound
    pub fn stop_sound(&mut self, handle: AudioHandle) {
        if let Some(source) = self.playing_sources.get_mut(&handle) {
            source.state = AudioState::Stopped;
            debug!("Stopped audio: {}", source.resource_name);
        }
        self.playing_sources.remove(&handle);
    }

    /// Pause a playing sound
    pub fn pause_sound(&mut self, handle: AudioHandle) {
        if let Some(source) = self.playing_sources.get_mut(&handle) {
            if source.state == AudioState::Playing {
                source.state = AudioState::Paused;
                debug!("Paused audio: {}", source.resource_name);
            }
        }
    }

    /// Resume a paused sound
    pub fn resume_sound(&mut self, handle: AudioHandle) {
        if let Some(source) = self.playing_sources.get_mut(&handle) {
            if source.state == AudioState::Paused {
                source.state = AudioState::Playing;
                debug!("Resumed audio: {}", source.resource_name);
            }
        }
    }

    /// Stop all playing sounds
    pub fn stop_all_sounds(&mut self) {
        let count = self.playing_sources.len();
        for source in self.playing_sources.values_mut() {
            source.state = AudioState::Stopped;
        }
        self.playing_sources.clear();

        if count > 0 {
            info!("Stopped {} playing audio sources", count);
        }
    }

    /// Set master volume (0.0 to 1.0)
    pub fn set_master_volume(&mut self, volume: f32) {
        let new_volume = volume.clamp(0.0, 1.0);

        if (self.master_volume - new_volume).abs() > f32::EPSILON {
            info!(
                "Master volume changed: {:.2} -> {:.2}",
                self.master_volume, new_volume
            );

            self.master_volume = new_volume;

            // Update volume of all playing sources
            for source in self.playing_sources.values_mut() {
                source.volume = (source.base_volume * self.master_volume).clamp(0.0, 1.0);
            }
        }
    }

    /// Get master volume
    pub fn get_master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Set volume for a specific sound
    pub fn set_sound_volume(&mut self, handle: AudioHandle, volume: f32) {
        if let Some(source) = self.playing_sources.get_mut(&handle) {
            source.base_volume = volume.clamp(0.0, 1.0);
            source.volume = (source.base_volume * self.master_volume).clamp(0.0, 1.0);
            debug!("Set volume for {}: {:.2}", source.resource_name, volume);
        }
    }

    /// Check if a sound is currently playing
    pub fn is_playing(&self, handle: AudioHandle) -> bool {
        self.playing_sources
            .get(&handle)
            .map(|source| source.state == AudioState::Playing)
            .unwrap_or(false)
    }

    /// Get number of currently playing sounds
    pub fn get_playing_count(&self) -> usize {
        self.playing_sources.len()
    }

    /// Enable or disable audio system
    pub fn set_audio_enabled(&mut self, enabled: bool) {
        if self.audio_enabled != enabled {
            self.audio_enabled = enabled;
            info!(
                "Audio system {}",
                if enabled { "enabled" } else { "disabled" }
            );

            if !enabled {
                self.stop_all_sounds();
            }
        }
    }

    /// Check if audio is enabled
    pub fn is_audio_enabled(&self) -> bool {
        self.audio_enabled
    }

    /// Get current state
    pub fn get_state(&self) -> SubsystemState {
        self.state
    }

    /// Get list of currently playing audio resources
    pub fn get_playing_resources(&self) -> Vec<String> {
        self.playing_sources
            .values()
            .filter(|source| source.state == AudioState::Playing)
            .map(|source| source.resource_name.clone())
            .collect()
    }
}

impl Default for BasicAudioManager {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export the audio manager interface trait from game_engine.rs
pub use crate::common::game_engine::AudioManagerInterface;

// Implement the trait for BasicAudioManager
impl AudioManagerInterface for BasicAudioManager {
    fn init(&mut self) -> SubsystemResult<()> {
        BasicAudioManager::init(self)
    }

    fn update(&mut self, delta_time: Duration) -> SubsystemResult<()> {
        BasicAudioManager::update(self, delta_time)
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        BasicAudioManager::shutdown(self)
    }

    fn set_master_volume(&mut self, volume: f32) {
        BasicAudioManager::set_master_volume(self, volume)
    }

    fn get_master_volume(&self) -> f32 {
        BasicAudioManager::get_master_volume(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_test_wav(path: &std::path::Path, sample_rate: u32, samples: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for _ in 0..samples {
            writer.write_sample(0i16).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
    }

    fn unique_test_name(prefix: &str) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{}_{}.wav", prefix, now)
    }

    #[test]
    fn test_audio_manager_creation() {
        let manager = BasicAudioManager::new();
        assert_eq!(manager.get_master_volume(), 1.0);
        assert!(manager.is_audio_enabled());
        assert_eq!(manager.get_playing_count(), 0);
    }

    #[test]
    fn test_master_volume() {
        let mut manager = BasicAudioManager::new();

        manager.set_master_volume(0.5);
        assert_eq!(manager.get_master_volume(), 0.5);

        manager.set_master_volume(-0.1); // Should clamp to 0.0
        assert_eq!(manager.get_master_volume(), 0.0);

        manager.set_master_volume(1.5); // Should clamp to 1.0
        assert_eq!(manager.get_master_volume(), 1.0);
    }

    #[test]
    fn test_audio_handles() {
        let handle1 = AudioHandle::new(1);
        let handle2 = AudioHandle::new(2);
        let invalid = AudioHandle::invalid();

        assert!(handle1.is_valid());
        assert!(handle2.is_valid());
        assert!(!invalid.is_valid());

        assert_ne!(handle1, handle2);
        assert_eq!(invalid, AudioHandle::invalid());
    }

    #[test]
    fn test_audio_state() {
        let mut manager = BasicAudioManager::new();

        // Test state changes
        assert_eq!(manager.get_state(), SubsystemState::Uninitialized);

        manager.init().unwrap();
        assert_eq!(manager.get_state(), SubsystemState::Running);

        manager.shutdown().unwrap();
        assert_eq!(manager.get_state(), SubsystemState::Shutdown);
    }

    #[test]
    fn test_master_volume_recomputes_from_base_volume() {
        let mut manager = BasicAudioManager::new();
        manager.playing_sources.insert(
            AudioHandle::new(1),
            AudioSource {
                handle: AudioHandle::new(1),
                resource_name: "test.wav".to_string(),
                volume: 0.5,
                base_volume: 0.5,
                state: AudioState::Playing,
                position: None,
                looping: false,
                playback_elapsed: Duration::ZERO,
                estimated_duration: Some(Duration::from_secs(5)),
            },
        );

        manager.set_master_volume(0.5);
        let source = manager.playing_sources.get(&AudioHandle::new(1)).unwrap();
        assert!((source.volume - 0.25).abs() < f32::EPSILON);

        manager.set_master_volume(1.0);
        let source = manager.playing_sources.get(&AudioHandle::new(1)).unwrap();
        assert!((source.volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_update_retires_finished_non_looping_sound() {
        let mut manager = BasicAudioManager::new();
        manager.init().unwrap();
        manager.set_audio_enabled(true);

        let dir = tempfile::tempdir().expect("tempdir");
        let file_name = unique_test_name("short_audio");
        let file_path = dir.path().join(&file_name);
        write_test_wav(&file_path, 1_000, 100); // 100ms

        if let Ok(mut rm) = manager.resource_manager.lock() {
            rm.add_search_path(dir.path());
        }

        let handle = manager.play_sound(&file_name, 1.0, false);
        assert!(handle.is_valid());
        assert!(manager.is_playing(handle));

        manager.update(Duration::from_millis(30)).unwrap();
        assert!(manager.is_playing(handle));

        manager.update(Duration::from_millis(90)).unwrap();
        assert!(!manager.is_playing(handle));
    }
}
