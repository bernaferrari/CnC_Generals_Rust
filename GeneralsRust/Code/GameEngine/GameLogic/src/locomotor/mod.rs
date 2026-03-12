//! Locomotor System Module
//!
//! This module provides the complete locomotor (movement) system including:
//! - Core locomotor types and physics (core.rs)
//! - Path following and pathfinding integration (path_following.rs)

pub mod core;
pub mod path_following;

// Re-export main types
pub use core::*;
pub use path_following::{
    update_movement_with_pathfinding, PathFollowingController, PathFollowingState,
};
