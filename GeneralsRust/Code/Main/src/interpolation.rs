use crate::game_logic::{Object, ObjectId};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Interpolation state for a game object
/// Stores previous and current states for smooth interpolation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpolationState {
    /// Position from previous logic frame
    pub previous_position: Vec3,
    /// Position from current logic frame
    pub current_position: Vec3,
    /// Rotation from previous logic frame (as quaternion for smooth slerp)
    pub previous_rotation: Quat,
    /// Rotation from current logic frame
    pub current_rotation: Quat,
    /// Scale from previous logic frame (for construction/destruction effects)
    pub previous_scale: Vec3,
    /// Scale from current logic frame
    pub current_scale: Vec3,
    /// Whether this object has valid interpolation data
    pub has_valid_data: bool,
}

impl Default for InterpolationState {
    fn default() -> Self {
        Self {
            previous_position: Vec3::ZERO,
            current_position: Vec3::ZERO,
            previous_rotation: Quat::IDENTITY,
            current_rotation: Quat::IDENTITY,
            previous_scale: Vec3::ONE,
            current_scale: Vec3::ONE,
            has_valid_data: false,
        }
    }
}

impl InterpolationState {
    /// Create a new interpolation state with initial values
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            previous_position: position,
            current_position: position,
            previous_rotation: rotation,
            current_rotation: rotation,
            previous_scale: scale,
            current_scale: scale,
            has_valid_data: true,
        }
    }

    /// Update the state before a logic frame (save current as previous)
    pub fn save_current_state(&mut self) {
        self.previous_position = self.current_position;
        self.previous_rotation = self.current_rotation;
        self.previous_scale = self.current_scale;
    }

    /// Update the state after a logic frame (set new current state)
    pub fn update_current_state(&mut self, position: Vec3, rotation: Quat, scale: Vec3) {
        self.current_position = position;
        self.current_rotation = rotation;
        self.current_scale = scale;
        self.has_valid_data = true;
    }

    /// Get interpolated position between previous and current
    /// Alpha should be between 0.0 and 1.0 (0 = previous, 1 = current)
    pub fn get_interpolated_position(&self, alpha: f32) -> Vec3 {
        if !self.has_valid_data {
            return self.current_position;
        }

        // Clamp alpha to prevent extrapolation
        let alpha = alpha.clamp(0.0, 1.0);

        // Linear interpolation for position
        self.previous_position.lerp(self.current_position, alpha)
    }

    /// Get interpolated rotation between previous and current
    /// Uses spherical linear interpolation (slerp) for smooth rotation
    pub fn get_interpolated_rotation(&self, alpha: f32) -> Quat {
        if !self.has_valid_data {
            return self.current_rotation;
        }

        // Clamp alpha to prevent extrapolation
        let alpha = alpha.clamp(0.0, 1.0);

        // Spherical linear interpolation for smooth rotation
        self.previous_rotation.slerp(self.current_rotation, alpha)
    }

    /// Get interpolated scale between previous and current
    pub fn get_interpolated_scale(&self, alpha: f32) -> Vec3 {
        if !self.has_valid_data {
            return self.current_scale;
        }

        // Clamp alpha to prevent extrapolation
        let alpha = alpha.clamp(0.0, 1.0);

        // Linear interpolation for scale
        self.previous_scale.lerp(self.current_scale, alpha)
    }

    /// Get complete interpolated transform matrix
    pub fn get_interpolated_transform(&self, alpha: f32) -> Mat4 {
        let position = self.get_interpolated_position(alpha);
        let rotation = self.get_interpolated_rotation(alpha);
        let scale = self.get_interpolated_scale(alpha);

        Mat4::from_scale_rotation_translation(scale, rotation, position)
    }

    /// Reset interpolation state (useful when an object teleports or is created)
    pub fn reset(&mut self, position: Vec3, rotation: Quat, scale: Vec3) {
        self.previous_position = position;
        self.current_position = position;
        self.previous_rotation = rotation;
        self.current_rotation = rotation;
        self.previous_scale = scale;
        self.current_scale = scale;
        self.has_valid_data = true;
    }
}

/// Camera interpolation state for smooth camera movement
#[derive(Debug, Clone)]
pub struct CameraInterpolationState {
    pub previous_position: Vec3,
    pub current_position: Vec3,
    pub previous_target: Vec3,
    pub current_target: Vec3,
    pub previous_zoom: f32,
    pub current_zoom: f32,
    pub has_valid_data: bool,
}

impl Default for CameraInterpolationState {
    fn default() -> Self {
        Self {
            previous_position: Vec3::ZERO,
            current_position: Vec3::ZERO,
            previous_target: Vec3::ZERO,
            current_target: Vec3::ZERO,
            previous_zoom: 1.0,
            current_zoom: 1.0,
            has_valid_data: false,
        }
    }
}

impl CameraInterpolationState {
    /// Update camera state for interpolation
    pub fn update(&mut self, position: Vec3, target: Vec3, zoom: f32) {
        self.previous_position = self.current_position;
        self.previous_target = self.current_target;
        self.previous_zoom = self.current_zoom;

        self.current_position = position;
        self.current_target = target;
        self.current_zoom = zoom;
        self.has_valid_data = true;
    }

    /// Get interpolated camera position
    pub fn get_interpolated_position(&self, alpha: f32) -> Vec3 {
        if !self.has_valid_data {
            return self.current_position;
        }

        let alpha = alpha.clamp(0.0, 1.0);
        self.previous_position.lerp(self.current_position, alpha)
    }

    /// Get interpolated camera target
    pub fn get_interpolated_target(&self, alpha: f32) -> Vec3 {
        if !self.has_valid_data {
            return self.current_target;
        }

        let alpha = alpha.clamp(0.0, 1.0);
        self.previous_target.lerp(self.current_target, alpha)
    }

    /// Get interpolated camera zoom
    pub fn get_interpolated_zoom(&self, alpha: f32) -> f32 {
        if !self.has_valid_data {
            return self.current_zoom;
        }

        let alpha = alpha.clamp(0.0, 1.0);
        self.previous_zoom * (1.0 - alpha) + self.current_zoom * alpha
    }
}

/// Projectile interpolation state for smooth projectile movement
#[derive(Debug, Clone)]
pub struct ProjectileInterpolationState {
    pub previous_position: Vec3,
    pub current_position: Vec3,
    pub previous_velocity: Vec3,
    pub current_velocity: Vec3,
    pub has_valid_data: bool,
}

impl Default for ProjectileInterpolationState {
    fn default() -> Self {
        Self {
            previous_position: Vec3::ZERO,
            current_position: Vec3::ZERO,
            previous_velocity: Vec3::ZERO,
            current_velocity: Vec3::ZERO,
            has_valid_data: false,
        }
    }
}

impl ProjectileInterpolationState {
    /// Update projectile state for interpolation
    pub fn update(&mut self, position: Vec3, velocity: Vec3) {
        self.previous_position = self.current_position;
        self.previous_velocity = self.current_velocity;

        self.current_position = position;
        self.current_velocity = velocity;
        self.has_valid_data = true;
    }

    /// Get interpolated projectile position
    /// Uses velocity for more accurate trajectory interpolation
    pub fn get_interpolated_position(&self, alpha: f32, dt: f32) -> Vec3 {
        if !self.has_valid_data {
            return self.current_position;
        }

        let alpha = alpha.clamp(0.0, 1.0);

        // Use velocity-based interpolation for more accurate projectile trajectories
        let base_position = self.previous_position.lerp(self.current_position, alpha);
        let velocity = self.previous_velocity.lerp(self.current_velocity, alpha);

        // Add velocity-based offset for smoother movement
        base_position + velocity * (dt * alpha)
    }
}

/// Main interpolation manager that coordinates all interpolated objects
pub struct InterpolationManager {
    /// Object interpolation states
    object_states: HashMap<ObjectId, InterpolationState>,
    /// Camera interpolation state
    pub camera_state: CameraInterpolationState,
    /// Projectile interpolation states
    projectile_states: HashMap<ObjectId, ProjectileInterpolationState>,
    /// Current interpolation alpha value (0.0 to 1.0)
    current_alpha: f32,
    /// Whether interpolation is enabled
    interpolation_enabled: bool,
}

impl Default for InterpolationManager {
    fn default() -> Self {
        Self {
            object_states: HashMap::new(),
            camera_state: CameraInterpolationState::default(),
            projectile_states: HashMap::new(),
            current_alpha: 0.0,
            interpolation_enabled: true,
        }
    }
}

impl InterpolationManager {
    /// Create a new interpolation manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current interpolation alpha (should be called each render frame)
    pub fn set_alpha(&mut self, alpha: f32) {
        // Clamp alpha to prevent extrapolation
        self.current_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Get the current interpolation alpha
    pub fn get_alpha(&self) -> f32 {
        self.current_alpha
    }

    /// Enable or disable interpolation
    pub fn set_interpolation_enabled(&mut self, enabled: bool) {
        self.interpolation_enabled = enabled;
    }

    /// Check if interpolation is enabled
    pub fn is_interpolation_enabled(&self) -> bool {
        self.interpolation_enabled
    }

    /// Prepare for logic update (save current states as previous)
    pub fn prepare_for_logic_update(&mut self) {
        for state in self.object_states.values_mut() {
            state.save_current_state();
        }
    }

    /// Update object state after logic update
    pub fn update_object_state(&mut self, object_id: ObjectId, object: &Object) {
        let position = object.get_position();
        let orientation_angle = object.get_orientation();
        let rotation = Quat::from_rotation_y(orientation_angle);

        // Scale based on construction progress
        let scale_factor = if object.status.under_construction {
            0.1 + 0.9 * object.construction_percent
        } else {
            1.0
        };
        let scale = Vec3::splat(scale_factor);

        let state = self.object_states.entry(object_id).or_default();
        state.update_current_state(position, rotation, scale);
    }

    /// Update camera state
    pub fn update_camera_state(&mut self, position: Vec3, target: Vec3, zoom: f32) {
        self.camera_state.update(position, target, zoom);
    }

    /// Update projectile state
    pub fn update_projectile_state(
        &mut self,
        projectile_id: ObjectId,
        position: Vec3,
        velocity: Vec3,
    ) {
        let state = self.projectile_states.entry(projectile_id).or_default();
        state.update(position, velocity);
    }

    /// Get interpolated position for an object
    pub fn get_interpolated_position(&self, object_id: ObjectId) -> Option<Vec3> {
        if !self.interpolation_enabled {
            return None;
        }

        self.object_states
            .get(&object_id)
            .map(|state| state.get_interpolated_position(self.current_alpha))
    }

    /// Get interpolated rotation for an object
    pub fn get_interpolated_rotation(&self, object_id: ObjectId) -> Option<Quat> {
        if !self.interpolation_enabled {
            return None;
        }

        self.object_states
            .get(&object_id)
            .map(|state| state.get_interpolated_rotation(self.current_alpha))
    }

    /// Get interpolated transform matrix for an object
    pub fn get_interpolated_transform(&self, object_id: ObjectId) -> Option<Mat4> {
        if !self.interpolation_enabled {
            return None;
        }

        self.object_states
            .get(&object_id)
            .map(|state| state.get_interpolated_transform(self.current_alpha))
    }

    /// Get interpolated camera position
    pub fn get_interpolated_camera_position(&self) -> Vec3 {
        if !self.interpolation_enabled {
            return self.camera_state.current_position;
        }

        self.camera_state
            .get_interpolated_position(self.current_alpha)
    }

    /// Get interpolated camera target
    pub fn get_interpolated_camera_target(&self) -> Vec3 {
        if !self.interpolation_enabled {
            return self.camera_state.current_target;
        }

        self.camera_state
            .get_interpolated_target(self.current_alpha)
    }

    /// Get interpolated camera zoom
    pub fn get_interpolated_camera_zoom(&self) -> f32 {
        if !self.interpolation_enabled {
            return self.camera_state.current_zoom;
        }

        self.camera_state.get_interpolated_zoom(self.current_alpha)
    }

    /// Get interpolated projectile position
    pub fn get_interpolated_projectile_position(
        &self,
        projectile_id: ObjectId,
        dt: f32,
    ) -> Option<Vec3> {
        if !self.interpolation_enabled {
            return None;
        }

        self.projectile_states
            .get(&projectile_id)
            .map(|state| state.get_interpolated_position(self.current_alpha, dt))
    }

    /// Remove object from interpolation tracking (when object is destroyed)
    pub fn remove_object(&mut self, object_id: ObjectId) {
        self.object_states.remove(&object_id);
        self.projectile_states.remove(&object_id);
    }

    /// Clear all interpolation states
    pub fn clear(&mut self) {
        self.object_states.clear();
        self.projectile_states.clear();
        self.camera_state = CameraInterpolationState::default();
        self.current_alpha = 0.0;
    }

    /// Get statistics about interpolation system
    pub fn get_stats(&self) -> InterpolationStats {
        InterpolationStats {
            object_count: self.object_states.len(),
            projectile_count: self.projectile_states.len(),
            current_alpha: self.current_alpha,
            interpolation_enabled: self.interpolation_enabled,
        }
    }
}

/// Statistics about the interpolation system
#[derive(Debug, Clone)]
pub struct InterpolationStats {
    pub object_count: usize,
    pub projectile_count: usize,
    pub current_alpha: f32,
    pub interpolation_enabled: bool,
}

/// Helper functions for interpolation
pub mod interpolation_utils {
    use super::*;

    /// Linear interpolation between two values
    pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a * (1.0 - t) + b * t
    }

    /// Smooth step interpolation (cubic hermite)
    pub fn smooth_step(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Smoother step interpolation (quintic hermite)
    pub fn smoother_step(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    /// Convert angle to quaternion for Y-axis rotation
    pub fn angle_to_quat_y(angle: f32) -> Quat {
        Quat::from_rotation_y(angle)
    }

    /// Extract Y-axis rotation angle from quaternion
    pub fn quat_to_angle_y(quat: Quat) -> f32 {
        let (_, y, _) = quat.to_euler(glam::EulerRot::XYZ);
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_state_creation() {
        let pos = Vec3::new(10.0, 0.0, 20.0);
        let rot = Quat::from_rotation_y(1.57); // 90 degrees
        let scale = Vec3::ONE;

        let state = InterpolationState::new(pos, rot, scale);

        assert_eq!(state.current_position, pos);
        assert_eq!(state.previous_position, pos);
        assert!(state.has_valid_data);
    }

    #[test]
    fn test_interpolation_alpha_clamping() {
        let mut state = InterpolationState::default();
        state.previous_position = Vec3::ZERO;
        state.current_position = Vec3::new(10.0, 0.0, 0.0);
        state.has_valid_data = true;

        // Test alpha clamping
        let pos_under = state.get_interpolated_position(-0.5);
        let pos_over = state.get_interpolated_position(1.5);
        let pos_valid = state.get_interpolated_position(0.5);

        assert_eq!(pos_under, Vec3::ZERO); // Should clamp to 0.0
        assert_eq!(pos_over, Vec3::new(10.0, 0.0, 0.0)); // Should clamp to 1.0
        assert_eq!(pos_valid, Vec3::new(5.0, 0.0, 0.0)); // Should interpolate normally
    }

    #[test]
    fn test_interpolation_manager() {
        let mut manager = InterpolationManager::new();
        manager.set_alpha(0.5);

        assert_eq!(manager.get_alpha(), 0.5);
        assert!(manager.is_interpolation_enabled());

        manager.set_interpolation_enabled(false);
        assert!(!manager.is_interpolation_enabled());
    }
}
