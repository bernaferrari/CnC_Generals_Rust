//! Object body subsystem.
//!
//! This module exposes the various body implementations used by objects in the
//! classic GameLogic code.  The body layer is responsible for health,
//! veterancy, death transitions, and state-dependent FX triggers.

pub mod active_body;
pub mod body_module;
pub mod highlander_body;
pub mod hive_structure_body;
pub mod immortal_body;
pub mod inactive_body;
pub mod structure_body;
pub mod undead_body;

pub use body_module::*;
