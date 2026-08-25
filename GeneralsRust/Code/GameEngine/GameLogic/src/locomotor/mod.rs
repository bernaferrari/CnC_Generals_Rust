//! Locomotor System Module
//!
//! This module provides the complete locomotor (movement) system including:
//! - Core locomotor types and physics (core.rs)
//! - Path following and pathfinding integration (path_following.rs)

#[path = "core_impl/mod.rs"]
pub mod core;
pub mod ini_bridge;
pub mod path_following;

// Re-export main types
pub use core::*;
pub use path_following::{
    PathFollowingController, PathFollowingState, update_movement_with_pathfinding,
};
