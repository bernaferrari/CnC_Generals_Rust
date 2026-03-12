//! # Audio Module
//!
//! Audio system interfaces

use crate::message_stream::game_message::Coord3D;
use crate::system::SubsystemInterface;

/// Game audio interface
pub trait GameAudio: SubsystemInterface {
    // Play a generic EVA/radar cue at an optional world position
    fn play_event(
        &mut self,
        event: &str,
        position: Option<Coord3D>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}
