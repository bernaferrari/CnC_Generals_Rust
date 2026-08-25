//! Locomotor system - Unit movement and pathfinding
//!
//! This module provides the complete locomotor system for unit movement,
//! matching the C++ Locomotor implementation from Locomotor.h
//!
//! Supports all 9 locomotor types with full terrain interaction,
//! physics integration, and pathfinding capabilities.

use crate::ai::pathfinding_system::{MovementCapabilities, PathfindLayerEnum};
use crate::common::*;
use crate::helpers::{
    TheGameLogic, TheTerrainLogic, get_game_logic_random_value, get_game_logic_random_value_real,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::{PhysicsState, PhysicsType};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, RwLock};

include!("types.rs");
include!("template.rs");
include!("path.rs");
include!("locomotor.rs");
include!("move_ground.rs");
include!("path_follow.rs");
include!("thrust.rs");
include!("move_air.rs");
include!("behavior_z.rs");
include!("move_dispatch.rs");
include!("move_towards.rs");
include!("maintain.rs");
include!("flags.rs");
include!("set_store.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const LOCOMOTOR_CORE_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("flags.rs"),
    include_str!("locomotor.rs"),
    include_str!("maintain.rs"),
    include_str!("move_air.rs"),
    include_str!("move_ground.rs"),
    include_str!("move_towards.rs"),
    include_str!("move_dispatch.rs"),
    include_str!("behavior_z.rs"),
    include_str!("thrust.rs"),
    include_str!("path.rs"),
    include_str!("path_follow.rs"),
    include_str!("set_store.rs"),
    include_str!("template.rs"),
    include_str!("types.rs"),
);
