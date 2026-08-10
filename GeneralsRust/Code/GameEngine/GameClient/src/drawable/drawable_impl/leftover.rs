//! Leftover drawable public types that do not belong with the core impl.

use super::*;

/// Specific drawable types for different objects
#[derive(Debug, Clone)]
pub enum DrawableType {
    /// 3D Model drawable
    Model {
        model_name: String,
        position: Vector3,
        scale: f32,
        animation_state: String,
    },
    /// 2D Sprite drawable
    Sprite {
        texture_name: String,
        position: Vector3,
        size: Vector3,
        uv_coordinates: [f32; 4], // u1, v1, u2, v2
    },
    /// Particle system drawable
    Particle {
        system_name: String,
        position: Vector3,
        scale: f32,
        lifetime: f32,
    },
    /// UI Element drawable
    UI {
        element_type: String,
        position: Vector3,
        size: Vector3,
        text: Option<String>,
    },
}
