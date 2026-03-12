//! WW3D Effects System
//!
//! This crate implements the complete effects system from the original WW3D engine,
//! including dazzle effects, ring objects, sphere objects, streak/trail rendering,
//! line rendering, and various other visual effects.

pub mod effects;

pub mod shattersystem;
pub use effects::*;

// Re-export commonly used types
pub use effects::{
    DazzleManager, MeshRenderObj, RingManager, SegLineRenderObj, SphereRenderObj, StreakRenderer,
};

use glam::{Vec3, Vec4};

/// Main effects manager that coordinates all effect systems
#[derive(Debug)]
pub struct EffectsManager {
    pub dazzle_manager: DazzleManager,
    pub ring_manager: RingManager,
    pub streak_renderer: StreakRenderer,
}

impl EffectsManager {
    /// Create a new effects manager
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            dazzle_manager: DazzleManager::new(device, queue),
            ring_manager: RingManager::new(device, queue),
            streak_renderer: StreakRenderer::new(device, queue),
        }
    }

    /// Update all effects
    pub fn update(
        &mut self,
        camera: &ww3d_renderer_3d::rendering::camera_system::camera::CameraClass,
        delta_time: f32,
    ) {
        self.dazzle_manager.update(camera, delta_time);
        self.ring_manager.update(delta_time);
        self.streak_renderer.update(delta_time);
    }

    /// Render all effects
    pub fn render(
        &mut self,
        render_info: &ww3d_renderer_3d::render_object_system::RenderInfoClass,
    ) -> Result<(), ww3d_core::errors::W3DError> {
        // Render effects in proper order for transparency
        self.ring_manager.render(render_info)?;
        self.streak_renderer.render(render_info)?;
        self.dazzle_manager.render(render_info)?;
        Ok(())
    }

    /// Create a screen flash effect
    pub fn create_screen_flash(&mut self, color: Vec3, intensity: f32, duration: f32) {
        self.dazzle_manager
            .create_screen_flash(color, intensity, duration);
    }

    /// Create a ring explosion effect
    pub fn create_ring_explosion(
        &mut self,
        position: Vec3,
        max_radius: f32,
        duration: f32,
        color: Vec4,
    ) {
        self.ring_manager
            .create_explosion_ring(position, max_radius, duration, color);
    }

    /// Create a missile trail effect
    pub fn create_missile_trail(&mut self, start: Vec3, end: Vec3, color: Vec4, width: f32) {
        self.streak_renderer.create_trail(start, end, color, width);
    }

    /// Create a laser line effect
    pub fn create_laser_line(&mut self, start: Vec3, end: Vec3, color: Vec4, width: f32) {
        self.streak_renderer.create_laser(start, end, color, width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effects_manager_creation() {
        // Would need actual device/queue for real test
        // This is just a placeholder to ensure the structure compiles
    }
}
