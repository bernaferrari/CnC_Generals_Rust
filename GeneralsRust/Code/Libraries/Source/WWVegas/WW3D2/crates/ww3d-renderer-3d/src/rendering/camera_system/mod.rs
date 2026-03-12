//! # Camera System Module
//!
//! This module provides comprehensive camera functionality for the WW3D engine,
//! converted from C++ to Rust with WGPU integration.
//!
//! ## Components
//!
//! - `camera`: Core camera class with projection and view matrices
//! - `viewport`: Viewport management for screen rendering
//! - `frustum`: Frustum culling and clipping planes

pub mod camera;
pub mod frustum;
pub mod viewport;

pub use camera::*;
pub use frustum::*;
pub use viewport::*;

// Type aliases for backward compatibility
pub type Camera = CameraClass;
