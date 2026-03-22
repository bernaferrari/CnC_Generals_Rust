//! # Audio Module
//!
//! Complete audio system for the GameClient.
//!
//! Ported from the C++ Generals Zero Hour audio system:
//!   - `GameAudio.cpp`   -> `audio_engine.rs`   (core engine, kira backend)
//!   - `AudioEventRTS.cpp` -> `audio_event.rs`    (event types & request queue)
//!   - `GameMusic.cpp`    -> `music_system.rs`    (mood-based playlists, crossfade)
//!   - `GameSpeech.cpp`   -> `speech_system.rs`   (EVA & unit voice responses)
//!
//! ## Architecture
//!
//! ```text
//!  GameLogic (AudioEventRts)  ──addAudioEvent──>  TheAudio (helpers.rs)
//!       |                                                    |
//!       v                                                    v
//!  GameClient  AudioEventQueue  ──drain──>  AudioEngine (kira)
//!       |                                        |         |
//!       v                                        v         v
//!  MusicSystem                          SoundManager  SpeechSystem
//!  (mood, crossfade)                    (3D, cache)   (EVA, cooldowns)
//! ```

use crate::message_stream::game_message::Coord3D;
use crate::system::SubsystemInterface;

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

pub mod audio_engine;
pub mod audio_event;
pub mod music_system;
pub mod speech_system;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use audio_engine::{
    AudioAffect, AudioCategory, AudioControl, AudioEngine, AudioEventInfo, AudioHandle,
    AudioPosition, AudioPriority, SoundType,
};

pub use audio_event::{AudioEvent, AudioEventQueue, AudioRequest, OwnerType, PlayPortion};

pub use music_system::{MusicMood, MusicSystem, MusicTrack};

pub use speech_system::{SpeechLine, SpeechPriority, SpeechSystem};

// ---------------------------------------------------------------------------
// GameAudio trait  (compat with existing GameClient code)
// ---------------------------------------------------------------------------

/// Game audio interface.
///
/// This trait is implemented by `AudioEngine` and can also be implemented
/// by test doubles or alternative backends.
pub trait GameAudio: SubsystemInterface {
    /// Play a named audio event at an optional world position.
    fn play_event(
        &mut self,
        event: &str,
        position: Option<Coord3D>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

impl GameAudio for AudioEngine {
    fn play_event(
        &mut self,
        event: &str,
        position: Option<Coord3D>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pos = position.map(|c| AudioPosition::new(c.x, c.y, c.z));
        let _handle = AudioEngine::play_event(self, event, pos);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_audio_trait_impl() {
        let mut engine = AudioEngine::new().expect("AudioEngine::new");
        engine.init().expect("AudioEngine::init");
        // Playing a non-existent event should not panic.
        let result = engine.play_event("NonExistentEvent", None);
        assert!(result.is_ok());
    }
}
