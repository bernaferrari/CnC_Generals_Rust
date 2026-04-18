//! Concrete implementation of `GameplayAudioDispatch` for the Main crate.
//!
//! Routes gameplay audio events (weapon fire, unit death, EVA) through the
//! existing `AudioManagerSubsystem` which uses the `SoundEffectsTable` to
//! resolve INI event names to concrete sound file paths and plays them
//! through the rodio audio backend.

use game_engine::common::audio::GameplayAudioDispatch;
use std::sync::{Arc, Mutex};

/// Concrete dispatch that queues audio events for the `AudioManagerSubsystem`
/// to process on the next frame.
///
/// This avoids calling async audio directly from gameplay code (which runs on
/// the logic thread) and instead feeds events into the same queue that
/// `GameLogic::process_audio_events()` uses.
pub struct MainAudioDispatch {
    events: Mutex<Vec<GameplayAudioEvent>>,
}

/// An audio event queued for playback.
#[derive(Debug, Clone)]
pub struct GameplayAudioEvent {
    pub event_name: String,
    pub position: Option<(f32, f32, f32)>,
}

impl MainAudioDispatch {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    /// Drain all queued events (called from the subsystem update).
    pub fn drain_events(&self) -> Vec<GameplayAudioEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect()
    }
}

impl GameplayAudioDispatch for MainAudioDispatch {
    fn play_positional_sound(&self, event_name: &str, x: f32, y: f32, z: f32) {
        let event = GameplayAudioEvent {
            event_name: event_name.to_string(),
            position: Some((x, y, z)),
        };
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(event);
    }

    fn play_2d_sound(&self, event_name: &str) {
        let event = GameplayAudioEvent {
            event_name: event_name.to_string(),
            position: None,
        };
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(event);
    }
}
