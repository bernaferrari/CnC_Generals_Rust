//! Complete audio event system and volume control
//!
//! Matches C++ AudioEvents.h and WWAudioClass volume management with:
//! - Audio event callbacks (EOS, playback state changes)
//! - Volume control with mixing buses
//! - Category-based volume management
//! - Fade in/out effects

use crate::{
    error::Result,
    mixer::{AudioMixer, VoiceHandle, VoiceParams, VoiceStopReason},
    sound_scene_obj::SoundObjectId,
    AudioSource,
};
use log::{debug, trace};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// Audio event types matching C++ AudioEvents.h
#[derive(Debug, Clone, PartialEq)]
pub enum AudioEvent {
    /// Sound started playing
    Started {
        sound_id: SoundObjectId,
        voice_handle: VoiceHandle,
    },
    /// Sound stopped playing
    Stopped {
        sound_id: SoundObjectId,
        voice_handle: VoiceHandle,
        reason: AudioStopReason,
    },
    /// Sound paused
    Paused {
        sound_id: SoundObjectId,
        voice_handle: VoiceHandle,
    },
    /// Sound resumed
    Resumed {
        sound_id: SoundObjectId,
        voice_handle: VoiceHandle,
    },
    /// End of stream reached
    EndOfStream {
        sound_id: SoundObjectId,
        voice_handle: VoiceHandle,
    },
    /// Volume changed for a category
    VolumeChanged {
        category: AudioCategory,
        new_volume: f32,
    },
    /// Audio system error
    Error {
        sound_id: Option<SoundObjectId>,
        message: String,
    },
}

/// Reason a sound stopped - matches C++ stop reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStopReason {
    /// Stopped by user command
    Command,
    /// Reached end of audio
    Completed,
    /// Stopped due to error
    Error,
    /// Stopped due to priority management
    Interrupted,
    /// Stopped due to resource limits
    ResourceLimited,
}

impl From<VoiceStopReason> for AudioStopReason {
    fn from(reason: VoiceStopReason) -> Self {
        match reason {
            VoiceStopReason::Command => Self::Command,
            VoiceStopReason::Completed => Self::Completed,
        }
    }
}

/// Audio event callback - matches C++ callback signatures
pub type AudioEventCallback = Arc<dyn Fn(&AudioEvent) + Send + Sync>;

/// Audio categories for volume control - matches C++ sound types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioCategory {
    /// Master volume (affects all audio)
    Master,
    /// Music tracks
    Music,
    /// Sound effects
    SoundEffects,
    /// Voice and speech
    Voice,
    /// Ambient sounds
    Ambient,
    /// UI sounds
    UI,
}

impl AudioCategory {
    /// Get default volume for category
    pub fn default_volume(self) -> f32 {
        match self {
            AudioCategory::Master => 1.0,
            AudioCategory::Music => 0.8,
            AudioCategory::SoundEffects => 0.9,
            AudioCategory::Voice => 1.0,
            AudioCategory::Ambient => 0.7,
            AudioCategory::UI => 0.8,
        }
    }
}

/// Volume control for a single category
#[derive(Debug, Clone)]
struct CategoryVolume {
    current: f32,
    target: f32,
    fade_duration: Duration,
    fade_started: Option<Instant>,
}

impl CategoryVolume {
    fn new(volume: f32) -> Self {
        Self {
            current: volume,
            target: volume,
            fade_duration: Duration::ZERO,
            fade_started: None,
        }
    }

    fn set_immediate(&mut self, volume: f32) {
        self.current = volume.clamp(0.0, 1.0);
        self.target = self.current;
        self.fade_started = None;
    }

    fn set_with_fade(&mut self, volume: f32, duration: Duration) {
        self.target = volume.clamp(0.0, 1.0);
        self.fade_duration = duration;
        self.fade_started = Some(Instant::now());
    }

    fn update(&mut self) -> bool {
        if let Some(started) = self.fade_started {
            let elapsed = started.elapsed();

            if elapsed >= self.fade_duration {
                self.current = self.target;
                self.fade_started = None;
                return true;
            }

            let progress = elapsed.as_secs_f32() / self.fade_duration.as_secs_f32();
            self.current = self.current + (self.target - self.current) * progress;
            return false;
        }

        self.current == self.target
    }

    fn is_fading(&self) -> bool {
        self.fade_started.is_some()
    }
}

/// Audio mixing bus - combines multiple sources with volume control
#[derive(Debug)]
pub struct AudioBus {
    name: String,
    category: AudioCategory,
    volume: CategoryVolume,
    mute: bool,
    solo: bool,
    voices: Vec<VoiceHandle>,
}

impl AudioBus {
    pub fn new(name: impl Into<String>, category: AudioCategory) -> Self {
        Self {
            name: name.into(),
            category,
            volume: CategoryVolume::new(category.default_volume()),
            mute: false,
            solo: false,
            voices: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn category(&self) -> AudioCategory {
        self.category
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume.set_immediate(volume);
    }

    pub fn fade_volume(&mut self, volume: f32, duration: Duration) {
        self.volume.set_with_fade(volume, duration);
    }

    pub fn volume(&self) -> f32 {
        if self.mute {
            0.0
        } else {
            self.volume.current
        }
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.mute = mute;
    }

    pub fn is_muted(&self) -> bool {
        self.mute
    }

    pub fn set_solo(&mut self, solo: bool) {
        self.solo = solo;
    }

    pub fn is_solo(&self) -> bool {
        self.solo
    }

    pub fn add_voice(&mut self, handle: VoiceHandle) {
        if !self.voices.contains(&handle) {
            self.voices.push(handle);
        }
    }

    pub fn remove_voice(&mut self, handle: VoiceHandle) {
        self.voices.retain(|h| *h != handle);
    }

    pub fn clear_voices(&mut self) {
        self.voices.clear();
    }

    pub fn voices(&self) -> &[VoiceHandle] {
        &self.voices
    }

    fn update(&mut self) -> bool {
        self.volume.update()
    }
}

/// Complete audio event and volume management system
pub struct AudioEventSystem {
    mixer: Arc<AudioMixer>,
    next_callback_id: AtomicU64,
    callbacks: Arc<Mutex<HashMap<u64, AudioEventCallback>>>,
    event_queue: Arc<Mutex<Vec<AudioEvent>>>,

    // Volume and bus management
    buses: Arc<Mutex<HashMap<AudioCategory, AudioBus>>>,
    master_volume: Arc<Mutex<CategoryVolume>>,

    // Voice to bus mapping
    voice_to_bus: Arc<Mutex<HashMap<VoiceHandle, AudioCategory>>>,
}

impl AudioEventSystem {
    pub fn new(mixer: Arc<AudioMixer>) -> Self {
        let mut buses = HashMap::new();

        // Create default buses for each category
        for category in [
            AudioCategory::Music,
            AudioCategory::SoundEffects,
            AudioCategory::Voice,
            AudioCategory::Ambient,
            AudioCategory::UI,
        ] {
            let bus_name = format!("{:?}", category);
            buses.insert(category, AudioBus::new(bus_name, category));
        }

        Self {
            mixer,
            next_callback_id: AtomicU64::new(1),
            callbacks: Arc::new(Mutex::new(HashMap::new())),
            event_queue: Arc::new(Mutex::new(Vec::new())),
            buses: Arc::new(Mutex::new(buses)),
            master_volume: Arc::new(Mutex::new(CategoryVolume::new(1.0))),
            voice_to_bus: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register an event callback
    pub fn register_callback(&self, callback: AudioEventCallback) -> u64 {
        let id = self.next_callback_id.fetch_add(1, Ordering::Relaxed);
        self.callbacks.lock().unwrap().insert(id, callback);
        debug!("Registered audio event callback {}", id);
        id
    }

    /// Unregister an event callback
    pub fn unregister_callback(&self, callback_id: u64) {
        self.callbacks.lock().unwrap().remove(&callback_id);
        debug!("Unregistered audio event callback {}", callback_id);
    }

    /// Fire an audio event
    pub fn fire_event(&self, event: AudioEvent) {
        trace!("Firing audio event: {:?}", event);

        // Add to event queue
        self.event_queue.lock().unwrap().push(event.clone());

        // Call all registered callbacks
        let callbacks = self.callbacks.lock().unwrap();
        for callback in callbacks.values() {
            callback(&event);
        }
    }

    /// Drain pending events
    pub fn drain_events(&self) -> Vec<AudioEvent> {
        self.event_queue.lock().unwrap().drain(..).collect()
    }

    /// Set master volume (affects all audio)
    pub fn set_master_volume(&self, volume: f32) {
        self.master_volume.lock().unwrap().set_immediate(volume);
        self.fire_event(AudioEvent::VolumeChanged {
            category: AudioCategory::Master,
            new_volume: volume,
        });
    }

    /// Fade master volume over duration
    pub fn fade_master_volume(&self, volume: f32, duration: Duration) {
        self.master_volume
            .lock()
            .unwrap()
            .set_with_fade(volume, duration);
    }

    /// Get current master volume
    pub fn master_volume(&self) -> f32 {
        self.master_volume.lock().unwrap().current
    }

    /// Set volume for a category
    pub fn set_category_volume(&self, category: AudioCategory, volume: f32) {
        if let Some(bus) = self.buses.lock().unwrap().get_mut(&category) {
            bus.set_volume(volume);
            self.fire_event(AudioEvent::VolumeChanged {
                category,
                new_volume: volume,
            });

            // Update all voices on this bus
            self.update_bus_voices(category);
        }
    }

    /// Fade category volume over duration
    pub fn fade_category_volume(&self, category: AudioCategory, volume: f32, duration: Duration) {
        if let Some(bus) = self.buses.lock().unwrap().get_mut(&category) {
            bus.fade_volume(volume, duration);
        }
    }

    /// Get volume for a category
    pub fn category_volume(&self, category: AudioCategory) -> f32 {
        self.buses
            .lock()
            .unwrap()
            .get(&category)
            .map(|bus| bus.volume())
            .unwrap_or(1.0)
    }

    /// Mute/unmute a category
    pub fn set_category_mute(&self, category: AudioCategory, mute: bool) {
        if let Some(bus) = self.buses.lock().unwrap().get_mut(&category) {
            bus.set_mute(mute);
            self.update_bus_voices(category);
        }
    }

    /// Check if category is muted
    pub fn is_category_muted(&self, category: AudioCategory) -> bool {
        self.buses
            .lock()
            .unwrap()
            .get(&category)
            .map(|bus| bus.is_muted())
            .unwrap_or(false)
    }

    /// Assign a voice to a bus/category
    pub fn assign_voice(&self, handle: VoiceHandle, category: AudioCategory) {
        self.voice_to_bus.lock().unwrap().insert(handle, category);

        if let Some(bus) = self.buses.lock().unwrap().get_mut(&category) {
            bus.add_voice(handle);
        }

        // Apply current bus volume to voice
        self.update_voice_volume(handle, category);
    }

    /// Remove voice from its bus
    pub fn unassign_voice(&self, handle: VoiceHandle) {
        if let Some(category) = self.voice_to_bus.lock().unwrap().remove(&handle) {
            if let Some(bus) = self.buses.lock().unwrap().get_mut(&category) {
                bus.remove_voice(handle);
            }
        }
    }

    /// Update system (call from game loop)
    pub fn update(&self, _delta: Duration) {
        // Update master volume fade
        self.master_volume.lock().unwrap().update();

        // Update all bus volume fades
        // First collect categories that completed fading
        let completed_categories: Vec<AudioCategory> = {
            let mut buses = self.buses.lock().unwrap();
            buses
                .values_mut()
                .filter_map(|bus| {
                    if bus.update() {
                        Some(bus.category())
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Then update voices for completed categories
        for category in completed_categories {
            self.update_bus_voices(category);
        }
    }

    /// Get calculated volume for a voice (master * category)
    pub fn get_voice_volume(&self, handle: VoiceHandle) -> f32 {
        let master = self.master_volume.lock().unwrap().current;

        if let Some(category) = self.voice_to_bus.lock().unwrap().get(&handle) {
            let category_vol = self.category_volume(*category);
            master * category_vol
        } else {
            master
        }
    }

    // Internal methods

    fn update_bus_voices(&self, category: AudioCategory) {
        let buses = self.buses.lock().unwrap();
        if let Some(bus) = buses.get(&category) {
            let voices: Vec<_> = bus.voices().to_vec();
            drop(buses);

            for handle in voices {
                self.update_voice_volume(handle, category);
            }
        }
    }

    fn update_voice_volume(&self, handle: VoiceHandle, category: AudioCategory) {
        let final_volume = self.get_voice_volume(handle);

        // Update voice params with new volume
        if let Some(_timeline) = self.mixer.voice_timeline(handle) {
            let mut params = VoiceParams::default();
            params.gain = final_volume;
            self.mixer.update_voice_params(handle, params);
        }
    }
}

/// Audio fade helper utility
pub struct AudioFader {
    start_volume: f32,
    end_volume: f32,
    duration: Duration,
    started_at: Option<Instant>,
}

impl AudioFader {
    pub fn new(start_volume: f32, end_volume: f32, duration: Duration) -> Self {
        Self {
            start_volume: start_volume.clamp(0.0, 1.0),
            end_volume: end_volume.clamp(0.0, 1.0),
            duration,
            started_at: None,
        }
    }

    pub fn start(&mut self) {
        self.started_at = Some(Instant::now());
    }

    pub fn current_volume(&self) -> f32 {
        if let Some(started) = self.started_at {
            let elapsed = started.elapsed();

            if elapsed >= self.duration {
                return self.end_volume;
            }

            let progress = elapsed.as_secs_f32() / self.duration.as_secs_f32();
            self.start_volume + (self.end_volume - self.start_volume) * progress
        } else {
            self.start_volume
        }
    }

    pub fn is_complete(&self) -> bool {
        if let Some(started) = self.started_at {
            started.elapsed() >= self.duration
        } else {
            false
        }
    }

    pub fn progress(&self) -> f32 {
        if let Some(started) = self.started_at {
            (started.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_category_defaults() {
        assert_eq!(AudioCategory::Master.default_volume(), 1.0);
        assert!(AudioCategory::Music.default_volume() > 0.0);
        assert!(AudioCategory::Voice.default_volume() > 0.0);
    }

    #[test]
    fn test_audio_bus_volume() {
        let mut bus = AudioBus::new("Test", AudioCategory::Music);
        assert_eq!(bus.volume(), AudioCategory::Music.default_volume());

        bus.set_volume(0.5);
        assert!((bus.volume() - 0.5).abs() < 0.01);

        bus.set_mute(true);
        assert_eq!(bus.volume(), 0.0);

        bus.set_mute(false);
        assert!((bus.volume() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_audio_fader() {
        let mut fader = AudioFader::new(1.0, 0.0, Duration::from_secs(1));
        assert_eq!(fader.current_volume(), 1.0);
        assert!(!fader.is_complete());

        fader.start();
        assert!(fader.current_volume() <= 1.0);
        assert!(fader.progress() >= 0.0);
    }

    #[test]
    fn test_category_volume_clamping() {
        let mut vol = CategoryVolume::new(0.5);
        vol.set_immediate(2.0);
        assert_eq!(vol.current, 1.0);

        vol.set_immediate(-0.5);
        assert_eq!(vol.current, 0.0);
    }
}
