//! Voice and speech playback system for unit responses and dialogue
//!
//! Matches the C++ voice playback from LogicalSound.cpp and handles:
//! - Unit voice responses (attack, move, select, etc.)
//! - Mission briefing dialogue
//! - In-game commentary
//! - Voice queuing and priority management

use crate::{
    mixer::{AudioMixer, VoiceDescriptor, VoiceHandle, VoiceParams, VoiceStopReason},
    AudioSource, Priority,
};
use log::{debug, trace, warn};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// Voice category matching C++ SOUND_TYPE enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceCategory {
    /// Unit acknowledgements and responses
    UnitResponse,
    /// Mission briefing and dialogue
    MissionDialogue,
    /// In-game announcer and events
    Announcer,
    /// Tactical voice alerts
    TacticalAlert,
    /// Generic speech
    Speech,
}

impl VoiceCategory {
    /// Get the default priority for this voice category
    pub fn default_priority(self) -> Priority {
        match self {
            VoiceCategory::TacticalAlert => Priority::Critical,
            VoiceCategory::Announcer => Priority::High,
            VoiceCategory::MissionDialogue => Priority::High,
            VoiceCategory::UnitResponse => Priority::Normal,
            VoiceCategory::Speech => Priority::Normal,
        }
    }

    /// Get the default timeout before auto-stopping (in milliseconds)
    pub fn timeout_ms(self) -> u64 {
        match self {
            VoiceCategory::TacticalAlert => 30000,   // 30 seconds
            VoiceCategory::Announcer => 20000,       // 20 seconds
            VoiceCategory::MissionDialogue => 60000, // 60 seconds
            VoiceCategory::UnitResponse => 10000,    // 10 seconds
            VoiceCategory::Speech => 15000,          // 15 seconds
        }
    }
}

/// Voice request for queued playback
#[derive(Clone)]
struct VoiceRequest {
    id: u64,
    source: Arc<AudioSource>,
    category: VoiceCategory,
    priority: Priority,
    volume: f32,
    interrupt_lower_priority: bool,
    allow_overlap: bool,
    submitted_at: Instant,
}

/// Active voice playback tracking
struct ActiveVoice {
    request_id: u64,
    voice_handle: VoiceHandle,
    category: VoiceCategory,
    priority: Priority,
    started_at: Instant,
    expires_at: Instant,
}

/// Voice queue configuration - matches C++ voice management
pub struct VoiceSystemConfig {
    /// Maximum number of simultaneously playing voices
    pub max_concurrent_voices: usize,
    /// Maximum queue size for pending voices
    pub max_queue_size: usize,
    /// Enable automatic voice timeout
    pub enable_timeout: bool,
    /// Global voice volume multiplier
    pub voice_volume: f32,
}

impl Default for VoiceSystemConfig {
    fn default() -> Self {
        Self {
            max_concurrent_voices: 4, // Matches typical C++ configuration
            max_queue_size: 16,
            enable_timeout: true,
            voice_volume: 0.9,
        }
    }
}

/// Voice playback system - matches C++ LogicalSound voice management
pub struct VoiceSystem {
    mixer: Arc<AudioMixer>,
    config: VoiceSystemConfig,

    next_request_id: AtomicU64,
    voice_queue: Arc<Mutex<VecDeque<VoiceRequest>>>,
    active_voices: Arc<Mutex<HashMap<u64, ActiveVoice>>>,

    // Voice ducking for priority management
    ducking_enabled: bool,
    ducked_voices: Arc<Mutex<HashMap<VoiceHandle, f32>>>, // Handle -> original volume
}

impl VoiceSystem {
    pub fn new(mixer: Arc<AudioMixer>, config: VoiceSystemConfig) -> Self {
        Self {
            mixer,
            config,
            next_request_id: AtomicU64::new(1),
            voice_queue: Arc::new(Mutex::new(VecDeque::new())),
            active_voices: Arc::new(Mutex::new(HashMap::new())),
            ducking_enabled: true,
            ducked_voices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Play a voice with automatic priority management
    pub fn play_voice(
        &self,
        source: Arc<AudioSource>,
        category: VoiceCategory,
        priority: Option<Priority>,
        volume: Option<f32>,
    ) -> u64 {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let priority = priority.unwrap_or_else(|| category.default_priority());
        let volume = volume.unwrap_or(self.config.voice_volume);

        let request = VoiceRequest {
            id: request_id,
            source,
            category,
            priority,
            volume,
            interrupt_lower_priority: true,
            allow_overlap: false,
            submitted_at: Instant::now(),
        };

        let mut queue = self.voice_queue.lock().unwrap();

        // Check if we should play immediately or queue
        let active_count = self.active_voices.lock().unwrap().len();

        if active_count < self.config.max_concurrent_voices {
            // Can play immediately
            drop(queue);
            self.start_voice_playback(request);
        } else {
            // Need to queue or interrupt
            if request.interrupt_lower_priority {
                // Try to stop lower priority voices
                if self.try_interrupt_lower_priority(&request) {
                    drop(queue);
                    self.start_voice_playback(request);
                } else {
                    // Queue it
                    if queue.len() < self.config.max_queue_size {
                        queue.push_back(request);
                    } else {
                        warn!("Voice queue full, dropping request {}", request_id);
                    }
                }
            } else {
                // Queue without interrupting
                if queue.len() < self.config.max_queue_size {
                    queue.push_back(request);
                } else {
                    warn!("Voice queue full, dropping request {}", request_id);
                }
            }
        }

        request_id
    }

    /// Play a unit response voice (convenience method)
    pub fn play_unit_response(&self, source: Arc<AudioSource>) -> u64 {
        self.play_voice(source, VoiceCategory::UnitResponse, None, None)
    }

    /// Play tactical alert voice (high priority)
    pub fn play_tactical_alert(&self, source: Arc<AudioSource>) -> u64 {
        self.play_voice(
            source,
            VoiceCategory::TacticalAlert,
            Some(Priority::Critical),
            None,
        )
    }

    /// Play mission dialogue
    pub fn play_mission_dialogue(&self, source: Arc<AudioSource>) -> u64 {
        self.play_voice(
            source,
            VoiceCategory::MissionDialogue,
            Some(Priority::High),
            None,
        )
    }

    /// Stop a specific voice by request ID
    pub fn stop_voice(&self, request_id: u64) {
        let mut active = self.active_voices.lock().unwrap();
        if let Some(active_voice) = active.remove(&request_id) {
            self.mixer
                .stop_voice(active_voice.voice_handle, VoiceStopReason::Command);
            debug!("Stopped voice request {}", request_id);
        }
    }

    /// Stop all voices of a specific category
    pub fn stop_category(&self, category: VoiceCategory) {
        let mut active = self.active_voices.lock().unwrap();
        let to_stop: Vec<_> = active
            .iter()
            .filter(|(_, v)| v.category == category)
            .map(|(id, v)| (*id, v.voice_handle))
            .collect();

        for (request_id, handle) in to_stop {
            self.mixer.stop_voice(handle, VoiceStopReason::Command);
            active.remove(&request_id);
            debug!("Stopped voice {} in category {:?}", request_id, category);
        }
    }

    /// Stop all active voices
    pub fn stop_all(&self) {
        let mut active = self.active_voices.lock().unwrap();
        for (request_id, voice) in active.drain() {
            self.mixer
                .stop_voice(voice.voice_handle, VoiceStopReason::Command);
            debug!("Stopped voice {}", request_id);
        }
    }

    /// Update the voice system (call from game loop)
    pub fn update(&self, _delta: Duration) {
        // Clean up completed voices
        self.cleanup_completed_voices();

        // Check for expired voices
        if self.config.enable_timeout {
            self.timeout_expired_voices();
        }

        // Process queued voices
        self.process_queue();

        // Update ducking
        if self.ducking_enabled {
            self.update_ducking();
        }
    }

    /// Set global voice volume
    pub fn set_voice_volume(&mut self, volume: f32) {
        self.config.voice_volume = volume.clamp(0.0, 1.0);
    }

    /// Get current voice volume
    pub fn voice_volume(&self) -> f32 {
        self.config.voice_volume
    }

    /// Enable or disable voice ducking
    pub fn set_ducking_enabled(&mut self, enabled: bool) {
        self.ducking_enabled = enabled;
        if !enabled {
            self.restore_all_ducked_voices();
        }
    }

    /// Check if a specific voice is currently playing
    pub fn is_voice_playing(&self, request_id: u64) -> bool {
        self.active_voices.lock().unwrap().contains_key(&request_id)
    }

    /// Get count of active voices
    pub fn active_voice_count(&self) -> usize {
        self.active_voices.lock().unwrap().len()
    }

    /// Get count of queued voices
    pub fn queued_voice_count(&self) -> usize {
        self.voice_queue.lock().unwrap().len()
    }

    // Internal methods

    fn start_voice_playback(&self, request: VoiceRequest) {
        let descriptor = VoiceDescriptor {
            source: Arc::clone(&request.source),
            params: VoiceParams {
                gain: request.volume,
                pan: 0.0,
                playback_rate: request.source.format().sample_rate.into(),
                loop_count: 1,
                start_frame: 0,
                is_culled: false,
                spatial: Default::default(),
            },
            channel_id: request.id as u32,
            handle_id: Some(request.id as u32),
        };

        let voice_handle = self.mixer.start_voice(descriptor);
        let now = Instant::now();
        let timeout_duration = Duration::from_millis(request.category.timeout_ms());

        let active_voice = ActiveVoice {
            request_id: request.id,
            voice_handle,
            category: request.category,
            priority: request.priority,
            started_at: now,
            expires_at: now + timeout_duration,
        };

        self.active_voices
            .lock()
            .unwrap()
            .insert(request.id, active_voice);

        debug!(
            "Started voice playback {} (category: {:?}, priority: {:?})",
            request.id, request.category, request.priority
        );
    }

    fn try_interrupt_lower_priority(&self, new_request: &VoiceRequest) -> bool {
        let active = self.active_voices.lock().unwrap();

        // Find lowest priority voice
        let lowest = active.iter().min_by_key(|(_, v)| v.priority);

        if let Some((request_id, voice)) = lowest {
            if voice.priority < new_request.priority {
                let request_id = *request_id;
                let handle = voice.voice_handle;
                let old_priority = voice.priority;
                drop(active);

                self.mixer.stop_voice(handle, VoiceStopReason::Command);
                self.active_voices.lock().unwrap().remove(&request_id);

                debug!(
                    "Interrupted voice {} (priority {:?}) for new voice {} (priority {:?})",
                    request_id, old_priority, new_request.id, new_request.priority
                );
                return true;
            }
        }

        false
    }

    fn cleanup_completed_voices(&self) {
        let mut active = self.active_voices.lock().unwrap();
        let mut to_remove = Vec::new();

        for (request_id, voice) in active.iter() {
            // Check if voice is still playing via mixer
            if let Some(timeline) = self.mixer.voice_timeline(voice.voice_handle) {
                if matches!(timeline.state, crate::mixer::VoicePlaybackState::Completed) {
                    to_remove.push(*request_id);
                }
            } else {
                // Voice handle is invalid, remove it
                to_remove.push(*request_id);
            }
        }

        for request_id in to_remove {
            active.remove(&request_id);
            trace!("Removed completed voice {}", request_id);
        }
    }

    fn timeout_expired_voices(&self) {
        let now = Instant::now();
        let mut active = self.active_voices.lock().unwrap();
        let mut to_stop = Vec::new();

        for (request_id, voice) in active.iter() {
            if now >= voice.expires_at {
                to_stop.push((*request_id, voice.voice_handle));
            }
        }

        for (request_id, handle) in to_stop {
            self.mixer.stop_voice(handle, VoiceStopReason::Command);
            active.remove(&request_id);
            debug!("Timed out voice {}", request_id);
        }
    }

    fn process_queue(&self) {
        let active_count = self.active_voices.lock().unwrap().len();

        if active_count < self.config.max_concurrent_voices {
            let mut queue = self.voice_queue.lock().unwrap();

            // Take one voice from queue
            if let Some(request) = queue.pop_front() {
                drop(queue);
                self.start_voice_playback(request);
            }
        }
    }

    fn update_ducking(&self) {
        let active = self.active_voices.lock().unwrap();

        // Find highest priority voice
        let highest_priority = active.values().map(|v| v.priority).max();

        if let Some(max_priority) = highest_priority {
            // Duck voices with lower priority
            for voice in active.values() {
                if voice.priority < max_priority {
                    self.duck_voice(voice.voice_handle, 0.3); // Duck to 30%
                } else {
                    self.restore_voice(voice.voice_handle);
                }
            }
        }
    }

    fn duck_voice(&self, handle: VoiceHandle, duck_volume: f32) {
        let mut ducked = self.ducked_voices.lock().unwrap();

        if !ducked.contains_key(&handle) {
            // First time ducking this voice, save original volume
            if let Some(timeline) = self.mixer.voice_timeline(handle) {
                ducked.insert(handle, 1.0); // Assume original was 1.0

                // Update voice with ducked volume
                let mut params = VoiceParams::default();
                params.gain = duck_volume;
                self.mixer.update_voice_params(handle, params);
            }
        }
    }

    fn restore_voice(&self, handle: VoiceHandle) {
        let mut ducked = self.ducked_voices.lock().unwrap();

        if let Some(original_volume) = ducked.remove(&handle) {
            let mut params = VoiceParams::default();
            params.gain = original_volume;
            self.mixer.update_voice_params(handle, params);
        }
    }

    fn restore_all_ducked_voices(&self) {
        let mut ducked = self.ducked_voices.lock().unwrap();

        for (handle, original_volume) in ducked.drain() {
            let mut params = VoiceParams::default();
            params.gain = original_volume;
            self.mixer.update_voice_params(handle, params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_category_priorities() {
        assert!(
            VoiceCategory::TacticalAlert.default_priority()
                > VoiceCategory::UnitResponse.default_priority()
        );
        assert!(
            VoiceCategory::Announcer.default_priority() > VoiceCategory::Speech.default_priority()
        );
    }

    #[test]
    fn test_voice_category_timeouts() {
        assert!(VoiceCategory::TacticalAlert.timeout_ms() > 0);
        assert!(
            VoiceCategory::MissionDialogue.timeout_ms() > VoiceCategory::UnitResponse.timeout_ms()
        );
    }

    #[test]
    fn test_voice_system_config() {
        let config = VoiceSystemConfig::default();
        assert!(config.max_concurrent_voices > 0);
        assert!(config.max_queue_size > 0);
        assert!(config.voice_volume > 0.0 && config.voice_volume <= 1.0);
    }
}
