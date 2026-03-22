//! Message Stream System Module
//!
//! This module provides the complete message processing system for the game, converted from the original
//! Command & Conquer Generals MessageStream systems including translators and command processing.
//!
//! The system handles:
//! - Raw input translation to game commands
//! - Message queuing and processing pipeline
//! - Selection management and control groups
//! - Hint system for UI feedback
//! - Command rate limiting and frame management
//! - Message serialization for network transmission
//! - Message filtering and broadcast routing
//! - Logging and replay recording
//!
//! Original C++ files: GameClient/MessageStream/

pub mod command_list;
pub mod command_router;
pub mod game_message;
pub mod gui_command_translator;
pub mod hint_spy;
pub mod hot_key;
pub mod look_at_xlat;
pub mod message_filtering;
pub mod message_logging;
pub mod message_serialization;
pub mod message_stream;
pub mod meta_event;
pub mod place_event_translator;
pub mod player_state;
pub mod translators;
pub mod window_xlat;

// Input → Selection → Commands integration (C++ port).
// `translators` is the canonical command path; `command_xlat` is retained as a compatibility shim.
pub mod command_xlat;
pub mod input_processor;
pub mod selection_xlat;

// Integration tests
#[cfg(test)]
mod input_integration_tests;

// Re-export main types for convenience
pub use command_list::*;
pub use command_router::*;
pub use game_message::*;
pub use gui_command_translator::*;
pub use hint_spy::*;
pub use hot_key::*;
pub use look_at_xlat::*;
pub use message_filtering::*;
pub use message_logging::*;
pub use message_serialization::*;
pub use message_stream::*;
pub use meta_event::*;
pub use place_event_translator::*;
pub use player_state::*;
pub use translators::*;
pub use window_xlat::*;

// Re-export input processing types
pub use command_xlat::{
    CanAttackResult, CommandEvaluateType, CommandableObject,
};
pub use input_processor::{
    InputEvent, InputProcessor, InputProcessorConfig, InputProcessorStatistics,
};
pub use selection_xlat::{SelectableDrawable, SelectionState, SelectionTranslator};
