//! Modern gameplay logic systems.
//!
//! This module tree replaces the legacy `compat` and `state_machine` layers
//! with owned, type-safe equivalents.  Each subsystem mirrors the original
//! C++ directory structure but offers a clean Rust API.

pub mod guard;
pub mod guard_registry;
pub mod state_machine;

pub use guard::{GuardBehaviour, GuardEvent, GuardParameters};
pub use guard_registry::GuardRegistry;
