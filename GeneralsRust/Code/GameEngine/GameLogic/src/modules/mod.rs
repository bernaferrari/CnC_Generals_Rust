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
