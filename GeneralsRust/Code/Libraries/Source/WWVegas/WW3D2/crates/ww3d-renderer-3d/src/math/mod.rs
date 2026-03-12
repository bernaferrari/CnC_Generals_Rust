//! Mathematics module for WW3D renderer

pub mod matrix4;
pub mod vector3;

// Re-export commonly used types
pub use glam::{Mat4, Vec4};
pub use matrix4::{Matrix4, Vector4};
pub use vector3::Vec3 as Vector3;

// Additional math types
pub type Vector2 = glam::Vec2;
