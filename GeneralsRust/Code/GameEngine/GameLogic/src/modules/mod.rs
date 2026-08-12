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
include!("ai_update_ext.rs");
include!("specialized_ai.rs");
include!("physics.rs");
include!("lifecycle.rs");
include!("update_sleep.rs");
include!("specialized_behavior.rs");
include!("special_power.rs");
include!("extension_traits.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const MODULES_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("ai_update.rs"),
    include_str!("ai_update_ext.rs"),
    include_str!("behavior.rs"),
    include_str!("contain.rs"),
    include_str!("core.rs"),
    include_str!("extension_traits.rs"),
    include_str!("lifecycle.rs"),
    include_str!("physics.rs"),
    include_str!("special_power.rs"),
    include_str!("specialized_ai.rs"),
    include_str!("specialized_behavior.rs"),
    include_str!("update_sleep.rs"),
);
