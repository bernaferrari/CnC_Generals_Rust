//! # Bounding Volumes Module
//!
//! This module provides bounding volume classes for collision detection and culling:
//! - Sphere: Spherical bounding volume
//! - AABox: Axis-aligned bounding box
//! - OBBox: Oriented bounding box
//!
//! These are essential for:
//! - View frustum culling
//! - Collision detection
//! - Spatial partitioning

pub mod aabox;
pub mod collision;
pub mod collision_detection;
pub mod obbox;
pub mod plane;
pub mod sphere;

pub use aabox::*;
pub use collision::*;
pub use collision_detection::*;
pub use obbox::*;
pub use plane::*;
pub use sphere::*;

// Type aliases for backward compatibility
pub type AABox = AABoxClass;
pub type OBBox = OBBoxClass;
pub type Sphere = SphereClass;
pub type Plane = PlaneClass;
