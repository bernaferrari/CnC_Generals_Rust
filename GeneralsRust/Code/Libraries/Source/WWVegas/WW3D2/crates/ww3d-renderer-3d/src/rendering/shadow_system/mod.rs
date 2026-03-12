//! Shadow Mapping System
//!
//! This module implements shadow mapping techniques for realistic lighting,
//! matching the original C++ WW3D shadow system capabilities.
//!
//! ## Features
//!
//! - Directional light shadows (Cascaded Shadow Maps)
//! - Point light shadows (Cube maps)
//! - Spot light shadows (Single shadow maps)
//! - Percentage Closer Filtering (PCF)
//! - Shadow bias and normal offset bias
//! - Soft shadows with blur
//! - Shadow acne and peter panning reduction

pub mod cascaded_shadow_map;
pub mod point_shadow_map;
pub mod shadow_map;
pub mod shadow_renderer;

pub use cascaded_shadow_map::*;
pub use point_shadow_map::*;
pub use shadow_map::*;
pub use shadow_renderer::*;
