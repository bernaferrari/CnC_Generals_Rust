//! Shared render-information types for geometry systems.
//!
//! These structs mirror the lightweight data passed around the original
//! WW3D renderer when issuing draw calls from helper geometry systems.

use glam::Vec3;

/// Minimal camera data required by legacy render helpers.
#[derive(Debug, Clone)]
pub struct CameraClass {
    pub position: Vec3,
}

impl CameraClass {
    pub fn new(position: Vec3) -> Self {
        Self { position }
    }
}

/// Placeholder lighting descriptor matching the historical API surface.
#[derive(Debug, Clone)]
pub struct LightEnvironmentClass;

/// Render context mirroring the WW3D16 RenderInfoClass payload.
#[derive(Debug, Clone)]
pub struct RenderInfoClass {
    pub camera: CameraClass,
    pub light_environment: Option<LightEnvironmentClass>,
}

impl RenderInfoClass {
    pub fn new(camera: CameraClass) -> Self {
        Self {
            camera,
            light_environment: None,
        }
    }
}
