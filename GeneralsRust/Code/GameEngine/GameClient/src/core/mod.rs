//! # GameClient Core Module
//!
//! This module provides the core GameClient infrastructure converted from the original
//! Command & Conquer Generals C++ codebase to Rust. The GameClient is the main singleton
//! that manages all client-side operations including drawable management, user interface,
//! subsystem coordination, and game loop management.
//!
//! ## Architecture Overview
//!
//! The GameClient serves as the central hub for:
//! - Drawable registration and lifecycle management
//! - Subsystem initialization and coordination
//! - Message dispatch and translation
//! - Game state synchronization
//! - Resource management
//!
//! ## Key Components
//!
//! - [`GameClient`] - Main client singleton struct
//! - [`GameClientMessageDispatcher`] - Message filtering and dispatch
//! - Subsystem factories and managers
//! - Drawable lookup and iteration systems
//!
//! ## Thread Safety
//!
//! The GameClient is designed to be used from the main thread only. Internal
//! synchronization is handled through Rust's ownership system and Arc/Mutex
//! patterns where necessary for subsystem communication.

pub mod game_client;
pub mod script_action_handler;
pub mod subsystems;

pub use game_client::{
    DrawableId, GameClient, GameClientError, GameClientMessageDispatcher, GameClientResult,
    SubsystemManager,
};

// Re-export commonly used types from the original codebase
pub use crate::drawable::{Drawable, DrawableStatus};
pub use crate::system::{
    GameMessage, GameMessageDisposition, MessageStream, SubsystemInterface, TimeOfDay,
};

/// Maximum number of client translators that can be registered
pub const MAX_CLIENT_TRANSLATORS: usize = 32;

/// Default drawable hash table size for performance optimization
pub const DRAWABLE_HASH_SIZE: usize = 8192;

/// Invalid drawable ID constant
pub const INVALID_DRAWABLE_ID: DrawableId = DrawableId(0);

/// Function pointer type for drawable iteration callbacks
pub type GameClientFuncPtr = fn(&dyn Drawable, *mut std::ffi::c_void);

/// Region type for spatial queries
#[derive(Debug, Clone)]
pub struct Region3D {
    pub lo: crate::system::Coord3D,
    pub hi: crate::system::Coord3D,
}

/// Time of day enumeration for lighting and visual effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDayType {
    Morning,
    Afternoon,
    Evening,
    Night,
}

/// Scorches type for terrain effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorchesType {
    Small,
    Medium,
    Large,
    Crater,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(MAX_CLIENT_TRANSLATORS, 32);
        assert_eq!(DRAWABLE_HASH_SIZE, 8192);
        assert_eq!(INVALID_DRAWABLE_ID, DrawableId(0));
    }

    #[test]
    fn test_time_of_day_values() {
        let morning = TimeOfDayType::Morning;
        let afternoon = TimeOfDayType::Afternoon;
        assert_ne!(morning, afternoon);
    }

    #[test]
    fn test_region_3d_creation() {
        let region = Region3D {
            lo: crate::system::Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            hi: crate::system::Coord3D {
                x: 100.0,
                y: 100.0,
                z: 100.0,
            },
        };

        assert_eq!(region.lo.x, 0.0);
        assert_eq!(region.hi.x, 100.0);
    }
}
