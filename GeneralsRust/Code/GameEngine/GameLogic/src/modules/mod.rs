//! Module interfaces and implementations
//!
//! This module provides all the module interfaces that objects use,
//! matching the C++ module system architecture.
//!
//! Split into focused submodules by interface family.

include!("core.rs");
include!("behavior.rs");
include!("contain.rs");
include!("ai_update.rs");

pub mod ai_native_world;
pub(crate) mod ai_state_runtime;
pub mod ai_world;
include!("ai_update_ext.rs");
include!("specialized_ai.rs");
include!("physics.rs");
include!("lifecycle.rs");
include!("update_sleep.rs");
include!("specialized_behavior.rs");
include!("special_power.rs");
include!("extension_traits.rs");
include!("tests.rs");

#[cfg(test)]
mod move_away_dispatch_tests;

#[cfg(test)]
mod ai_native_world_tests;
#[cfg(test)]
pub(crate) use ai_native_world_tests::{
    owned_native_explicit_goal_control, owned_native_path_save_control, owned_native_turn_control,
    owned_native_waypoint_control,
};
