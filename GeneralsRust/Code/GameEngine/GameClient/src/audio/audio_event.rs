//! # Audio Event
//!
//! Defines the `AudioEvent` type used throughout the GameClient to request
//! sounds from the audio engine.  Ported from C++ `AudioEventRTS` /
//! `AudioRequest`.

use std::collections::VecDeque;

use super::audio_engine::{
    AudioAffect, AudioCategory, AudioControl, AudioEventInfo, AudioHandle,
    AudioPosition, AudioPriority, SoundType,
};

// ---------------------------------------------------------------------------
// AudioEvent
// ---------------------------------------------------------------------------

/// Represents a request to play a piece of audio.
///
/// Matches C++ `AudioEventRTS`.  An event is created, populated, and then
/// submitted to the `AudioEngine` via `AudioEngine::play_event`.
#[derive(Debug, Clone)]
pub struct AudioEvent {
    /// Name of the event (must match an entry registered from INI).
    pub event_name: String,

    /// Handle assigned by the engine once the event starts playing.
    pub playing_handle: AudioHandle,

    /// Position of the sound in world space (for 3D positional audio).
    pub position: Option<AudioPosition>,

    /// Object ID the sound is attached to (position updated each frame).
    pub object_id: u32,

    /// Drawable ID the sound is attached to.
    pub drawable_id: Option<u32>,

    /// Player index that owns this sound.
    pub player_index: Option<i32>,

    /// Volume override (0.0..=2.0).
    pub volume: Option<f32>,

    /// Priority override.
    pub priority: Option<AudioPriority>,

    /// Whether this event should fade in/out.
    pub should_fade: bool,

    /// Logical audio events (scripted) ignore shroud checks.
    pub is_logical_audio: bool,

    /// If true, this event cannot be interrupted by higher-priority sounds.
    pub uninterruptable: bool,

    /// Current pitch multiplier (1.0 = normal).
    pub pitch_shift: f32,

    /// Volume shift added on top of the event info volume.
    pub volume_shift: f32,

    /// Delay before the sound starts playing (seconds).
    pub delay: f32,

    /// Number of loops remaining (-1 = infinite).
    pub loop_count: i32,

    /// Index into the event's sounds list (for non-random sequential play).
    pub playing_audio_index: i32,

    /// Which portion of the sound to play next.
    pub portion: PlayPortion,

    /// Handle of another event that should be killed when this starts.
    pub handle_to_kill: AudioHandle,
}

/// Which portion of a sound event to play next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayPortion {
    Attack,
    Sound,
    Decay,
    Done,
}

/// Owner type for an audio event (matches C++ `OwnerType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerType {
    Positional,
    Drawable,
    Object,
    Dead,
}

impl Default for AudioEvent {
    fn default() -> Self {
        Self {
            event_name: String::new(),
            playing_handle: 0,
            position: None,
            object_id: 0,
            drawable_id: None,
            player_index: None,
            volume: None,
            priority: None,
            should_fade: false,
            is_logical_audio: false,
            uninterruptable: false,
            pitch_shift: 1.0,
            volume_shift: 0.0,
            delay: 0.0,
            loop_count: 1,
            playing_audio_index: 0,
            portion: PlayPortion::Sound,
            handle_to_kill: 0,
        }
    }
}

impl AudioEvent {
    /// Create a new event with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            event_name: name.into(),
            ..Default::default()
        }
    }

    /// Create an event with a fixed world position.
    pub fn with_position(name: impl Into<String>, pos: AudioPosition) -> Self {
        Self {
            event_name: name.into(),
            position: Some(pos),
            ..Default::default()
        }
    }

    /// Create an event attached to an object.
    pub fn with_object(name: impl Into<String>, object_id: u32) -> Self {
        Self {
            event_name: name.into(),
            object_id,
            ..Default::default()
        }
    }

    /// Create an event attached to a drawable.
    pub fn with_drawable(name: impl Into<String>, drawable_id: u32) -> Self {
        Self {
            event_name: name.into(),
            drawable_id: Some(drawable_id),
            ..Default::default()
        }
    }

    // ---- Setters (fluent API) ----

    pub fn set_event_name(&mut self, name: impl Into<String>) {
        self.event_name = name.into();
    }

    pub fn set_position(&mut self, pos: AudioPosition) {
        self.position = Some(pos);
    }

    pub fn set_object_id(&mut self, id: u32) {
        self.object_id = id;
    }

    pub fn set_drawable_id(&mut self, id: u32) {
        self.drawable_id = Some(id);
    }

    pub fn set_player_index(&mut self, idx: i32) {
        self.player_index = Some(idx);
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = Some(vol.clamp(0.0, 2.0));
    }

    pub fn set_priority(&mut self, pri: AudioPriority) {
        self.priority = Some(pri);
    }

    pub fn set_should_fade(&mut self, fade: bool) {
        self.should_fade = fade;
    }

    pub fn set_is_logical_audio(&mut self, logical: bool) {
        self.is_logical_audio = logical;
    }

    pub fn set_uninterruptable(&mut self, val: bool) {
        self.uninterruptable = val;
    }

    pub fn set_pitch_shift(&mut self, pitch: f32) {
        self.pitch_shift = pitch.clamp(0.25, 4.0);
    }

    pub fn set_volume_shift(&mut self, shift: f32) {
        self.volume_shift = shift;
    }

    pub fn set_delay(&mut self, delay: f32) {
        self.delay = delay.max(0.0);
    }

    pub fn set_loop_count(&mut self, count: i32) {
        self.loop_count = count;
    }

    pub fn set_handle_to_kill(&mut self, handle: AudioHandle) {
        self.handle_to_kill = handle;
    }

    pub fn set_playing_handle(&mut self, handle: AudioHandle) {
        self.playing_handle = handle;
    }

    // ---- Getters ----

    pub fn event_name(&self) -> &str {
        &self.event_name
    }

    pub fn is_playing(&self) -> bool {
        self.playing_handle != 0
    }

    pub fn playing_handle(&self) -> AudioHandle {
        self.playing_handle
    }

    pub fn is_positional(&self) -> bool {
        self.position.is_some()
    }

    pub fn should_fade(&self) -> bool {
        self.should_fade
    }

    pub fn is_logical_audio(&self) -> bool {
        self.is_logical_audio
    }

    pub fn is_uninterruptable(&self) -> bool {
        self.uninterruptable
    }

    /// Get the current position (either explicit position or the zero-vector).
    pub fn current_position(&self) -> AudioPosition {
        self.position.unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// AudioRequest
// ---------------------------------------------------------------------------

/// A request to the audio system (play, pause, or stop).
///
/// Matches C++ `AudioRequest`.  Requests are queued and processed each frame
/// by the audio engine.
#[derive(Debug)]
pub enum AudioRequest {
    /// Play a new audio event.
    Play {
        event: AudioEvent,
        requires_sample_check: bool,
    },
    /// Pause a playing sound.
    Pause { handle: AudioHandle },
    /// Stop a playing sound.
    Stop { handle: AudioHandle },
}

// ---------------------------------------------------------------------------
// AudioEventQueue
// ---------------------------------------------------------------------------

/// Thread-safe queue of audio requests.
///
/// Game logic (which may run on a different thread) pushes requests, and the
/// audio engine drains them once per frame during `update`.
pub struct AudioEventQueue {
    queue: VecDeque<AudioRequest>,
    max_size: usize,
}

impl AudioEventQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Enqueue a request.  Silently drops if the queue is full.
    pub fn push(&mut self, request: AudioRequest) {
        if self.queue.len() >= self.max_size {
            log::warn!("AudioEventQueue: dropped request (queue full)");
            return;
        }
        self.queue.push_back(request);
    }

    /// Drain all pending requests.
    pub fn drain(&mut self) -> Vec<AudioRequest> {
        self.queue.drain(..).collect()
    }

    /// Number of pending requests.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Clear all pending requests.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_event_builder() {
        let mut ev = AudioEvent::new("ExplosionLarge");
        ev.set_object_id(42);
        ev.set_volume(0.8);
        ev.set_uninterruptable(true);
        assert_eq!(ev.event_name(), "ExplosionLarge");
        assert_eq!(ev.object_id, 42);
        assert!(!ev.is_playing());
        assert!(ev.is_uninterruptable());
    }

    #[test]
    fn test_audio_event_with_position() {
        let pos = AudioPosition::new(100.0, 0.0, 50.0);
        let ev = AudioEvent::with_position("AmbientWind", pos);
        assert!(ev.is_positional());
        assert!((ev.current_position().x - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_event_queue() {
        let mut q = AudioEventQueue::new(16);
        assert!(q.is_empty());
        q.push(AudioRequest::Play {
            event: AudioEvent::new("Test"),
            requires_sample_check: false,
        });
        assert_eq!(q.len(), 1);
        let items = q.drain();
        assert_eq!(items.len(), 1);
        assert!(q.is_empty());
    }
}
