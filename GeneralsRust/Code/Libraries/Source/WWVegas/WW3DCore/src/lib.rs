//! ww3d_core - Foundational types/constants for the Westwood 3D ecosystem
//!
//! This crate extracts engine-agnostic parts of the original WW3D2 crate:
//! - Error and result types
//! - W3D file format chunk IDs and helpers
//! - Minimal portable W3D structs used by loaders
//!
//! Higher level systems (renderer, scene, animation) should depend on this
//! crate instead of duplicating format or error definitions.

pub mod error;
pub mod w3d_chunks;
pub mod w3d_types;

// Re-exports for convenience
pub use error::{Result, W3dError};
pub use w3d_chunks::*;
pub use w3d_types::*;
