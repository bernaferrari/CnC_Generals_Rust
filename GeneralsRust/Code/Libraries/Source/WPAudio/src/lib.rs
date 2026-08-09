#![allow(missing_docs)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(clippy::cargo)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(deprecated)]
#![allow(nonstandard_style)]
#![allow(unconditional_recursion)]
#![allow(mismatched_lifetime_syntaxes)]
#![allow(unexpected_cfgs)]
#![allow(private_interfaces)]

//! # Shared Utilities for WestWood Studios Library Conversions
//!
//! This crate provides common utilities and data structures shared across
//! multiple WestWood Studios library conversions (WPAudio, WWVegas, etc.).
//!
//! ## Modules
//!
//! - [`error`] - Common error types and utilities
//! - [`memory`] - Memory management utilities  
//! - [`collections`] - Specialized data structures
//! - [`threading`] - Thread synchronization primitives
//! - [`time`] - Cross-platform timing utilities
//! - [`platform`] - Platform-specific abstractions

pub mod collections;
pub mod error;
pub mod memory;
pub mod platform;
pub mod threading;
pub mod time;

/// Common result type using shared error
pub type Result<T> = std::result::Result<T, error::SharedError>;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub mod aud__stream_buffering;
pub mod aud_d_sound_driver;
