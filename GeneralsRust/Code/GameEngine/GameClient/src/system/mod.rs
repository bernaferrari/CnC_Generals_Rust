//! # System Module
//!
//! Core system types and interfaces for the GameClient

use std::error::Error;
use thiserror::Error;

pub mod anim2_d;
pub mod beacon_display;
pub mod campaign_manager;
pub mod debug_display;
pub mod debug_displayers;
pub mod image;
pub mod particle_sys;
pub mod ray_effect;
pub mod smudge;

pub use anim2_d::{Anim2D, Anim2DCollection, Anim2DStatus};
pub use beacon_display::{BeaconMarker, BeaconNotification};
pub use debug_display::{DebugDisplay, DebugTextSink};
pub use debug_displayers::audio_debug_display;
pub use particle_sys::{
    Particle, ParticleRenderer, ParticleSystem, ParticleSystemId, ParticleSystemManager,
    ParticleSystemTemplate,
};
pub use ray_effect::{RayEffect, RayEffectConfig, RayEffectId, RayEffectManager, RayType};
pub use smudge::{get_smudge_manager, Smudge, SmudgeManager, SmudgeSet, SmudgeSetHandle};

pub use crate::message_stream::{
    game_message::{Coord3D, GameMessage, GameMessageType},
    message_stream::{GameMessageDisposition, MessageStream},
};

/// Time of day enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Morning,
    Afternoon,
    Evening,
    Night,
}

/// Result type for game message operations
pub type GameMessageResult<T> = Result<T, Box<dyn Error>>;

/// Subsystem interface trait
pub trait SubsystemInterface {
    fn init(&mut self) -> Result<(), Box<dyn Error>>;
    fn update(&mut self) -> Result<(), Box<dyn Error>>;
    fn reset(&mut self) -> Result<(), Box<dyn Error>>;
}

/// Generic subsystem lifecycle states used by higher level components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemState {
    Uninitialized,
    Initializing,
    Running,
    ShuttingDown,
    Shutdown,
}

/// Generic subsystem error type used when a dedicated error is unavailable
#[derive(Debug, Error)]
pub enum SubsystemError {
    #[error("{0}")]
    Message(String),
}

impl From<Box<dyn std::error::Error>> for SubsystemError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        SubsystemError::Message(value.to_string())
    }
}
