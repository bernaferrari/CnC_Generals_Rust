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

#![warn(missing_docs)]
#![warn(clippy::all)]

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
