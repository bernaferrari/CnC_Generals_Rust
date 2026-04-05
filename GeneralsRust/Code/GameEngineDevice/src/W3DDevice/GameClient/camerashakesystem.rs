//! # Camera Shake System Module
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/camerashakesystem.h
//!
//! This module provides the camera shake system for simulating explosions, earthquakes, etc.
//!
//! ## Implementation
//!
//! The full implementation is in `/Code/GameEngine/GameClient/src/display/cinematic_camera.rs`.
//! This module provides a compatibility wrapper for the W3D device layer.
//!
//! ## C++ References
//! - `camerashakesystem.h` - System interface
//! - `camerashakesystem.cpp` - Implementation with sinusoidal shake patterns

use glam::Vec3;
use std::sync::{Mutex, OnceLock};

// Re-export the full cinematic camera system
pub use game_client::display::cinematic_camera::{
    CameraShakeSystem, CameraShakeType, CinematicCameraSystem,
};

/// Camera shake system wrapper for W3D device layer
///
/// Provides a simple interface for adding camera shakes from the device layer.
/// The actual implementation is in the GameClient cinematic camera system.
pub struct CameraShakeSystemWrapper {
    /// Internal shake system
    shake_system: CameraShakeSystem,
}

impl CameraShakeSystemWrapper {
    /// Create a new camera shake system
    pub fn new() -> Self {
        Self {
            shake_system: CameraShakeSystem::new(),
        }
    }

    /// Add a camera shake effect
    ///
    /// C++ Reference: camerashakesystem.cpp lines 164-183
    ///
    /// # Arguments
    /// * `position` - World position of shake epicenter
    /// * `radius` - Radius of effect in world units (default: 50.0)
    /// * `duration` - Duration in seconds (default: 1.5)
    /// * `power` - Power in degrees of amplitude (default: 1.0)
    pub fn add_camera_shake(&mut self, position: Vec3, radius: f32, duration: f32, power: f32) {
        self.shake_system
            .add_camera_shake(position, radius, duration, power);
    }

    /// Update all active shakers
    ///
    /// C++ Reference: camerashakesystem.cpp lines 201-225
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    pub fn timestep(&mut self, dt: f32) {
        self.shake_system.timestep(dt);
    }

    /// Check if camera is currently shaking
    ///
    /// C++ Reference: camerashakesystem.cpp lines 185-198
    pub fn is_camera_shaking(&self) -> bool {
        self.shake_system.is_camera_shaking()
    }

    /// Compute accumulated shake angles for camera
    ///
    /// C++ Reference: camerashakesystem.cpp lines 227-263
    ///
    /// # Arguments
    /// * `camera_position` - Current camera position in world space
    ///
    /// # Returns
    /// Shake angles (pitch, yaw, roll) in radians
    pub fn update_camera_shaker(&self, camera_position: Vec3) -> Vec3 {
        self.shake_system.update_camera_shaker(camera_position)
    }

    /// Get reference to internal shake system
    pub fn shake_system(&self) -> &CameraShakeSystem {
        &self.shake_system
    }

    /// Get mutable reference to internal shake system
    pub fn shake_system_mut(&mut self) -> &mut CameraShakeSystem {
        &mut self.shake_system
    }
}

impl Default for CameraShakeSystemWrapper {
    fn default() -> Self {
        Self::new()
    }
}

// Global camera shake system instance
// C++ Reference: camerashakesystem.cpp line 266
static GLOBAL_CAMERA_SHAKER: OnceLock<Mutex<CameraShakeSystemWrapper>> = OnceLock::new();

/// Get the global camera shake system
pub fn get_camera_shaker_system() -> std::sync::MutexGuard<'static, CameraShakeSystemWrapper> {
    GLOBAL_CAMERA_SHAKER
        .get_or_init(|| Mutex::new(CameraShakeSystemWrapper::new()))
        .lock()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_shake_wrapper() {
        let mut system = CameraShakeSystemWrapper::new();

        // Add a shake
        system.add_camera_shake(Vec3::ZERO, 50.0, 1.5, 1.0);
        assert!(system.is_camera_shaking());

        // Should produce shake angles
        let angles = system.update_camera_shaker(Vec3::ZERO);
        assert!(angles.length() > 0.0);
    }

    #[test]
    fn test_global_system() {
        let system = get_camera_shaker_system();
        system.add_camera_shake(Vec3::ZERO, 50.0, 1.5, 1.0);
        assert!(system.is_camera_shaking());
    }
}
