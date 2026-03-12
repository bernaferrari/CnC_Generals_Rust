//! Level of Detail (LOD) System
//!
//! Integrated LOD management for the 3D renderer providing
//! distance-based and screen-space LOD calculations.

pub mod lod_calculator;
pub mod lod_manager;
pub mod lod_object;
pub mod prototype_loader;

pub use lod_calculator::*;
pub use lod_manager::*;
pub use lod_object::*;
pub use prototype_loader::*;
