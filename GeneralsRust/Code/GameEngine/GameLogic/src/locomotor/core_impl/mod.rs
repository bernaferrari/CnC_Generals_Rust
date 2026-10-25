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
#[path = "physics_access.rs"]
mod physics_access;
pub(crate) use physics_access::{
    LocomotorMovementObservations, LocomotorPhysicsAccess, LocomotorPoseChange,
};
include!("move_dispatch.rs");
include!("move_towards.rs");
include!("maintain.rs");
include!("flags.rs");
include!("set_store.rs");
include!("tests.rs");
