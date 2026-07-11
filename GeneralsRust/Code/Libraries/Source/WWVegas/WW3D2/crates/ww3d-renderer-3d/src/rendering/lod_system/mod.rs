//! Level of Detail (LOD) system implementation
//!
//! This module provides LOD calculation and management for meshes
//! based on distance, screen size, and other factors.

use crate::rendering::mesh_system::MeshClass;
use glam::Vec3;
use std::sync::Arc;

/// LOD level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LODLevel {
    Highest = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Lowest = 4,
}

/// LOD manager for calculating appropriate detail levels
pub struct LODManager {
    /// Distance thresholds for each LOD level
    distance_thresholds: [f32; 5],
    /// Screen size thresholds for each LOD level (in pixels)
    screen_size_thresholds: [f32; 5],
}

impl LODManager {
    /// Create a new LOD manager with default thresholds
    pub fn new() -> Self {
        Self {
            distance_thresholds: [10.0, 25.0, 50.0, 100.0, 200.0],
            screen_size_thresholds: [1000.0, 500.0, 200.0, 50.0, 10.0],
        }
    }

    /// Calculate LOD level based on distance from camera
    pub fn calculate_lod_from_distance(&self, distance: f32) -> LODLevel {
        for (i, &threshold) in self.distance_thresholds.iter().enumerate() {
            if distance <= threshold {
                return match i {
                    0 => LODLevel::Highest,
                    1 => LODLevel::High,
                    2 => LODLevel::Medium,
                    3 => LODLevel::Low,
                    _ => LODLevel::Lowest,
                };
            }
        }
        LODLevel::Lowest
    }

    /// Calculate LOD level based on screen space size
    pub fn calculate_lod_from_screen_size(&self, screen_size: f32) -> LODLevel {
        for (i, &threshold) in self.screen_size_thresholds.iter().enumerate() {
            if screen_size >= threshold {
                return match i {
                    0 => LODLevel::Highest,
                    1 => LODLevel::High,
                    2 => LODLevel::Medium,
                    3 => LODLevel::Low,
                    _ => LODLevel::Lowest,
                };
            }
        }
        LODLevel::Lowest
    }

    /// Calculate screen space size for a bounding sphere
    pub fn calculate_screen_size(
        &self,
        sphere_center: Vec3,
        sphere_radius: f32,
        camera_pos: Vec3,
        screen_height: f32,
        fov_y: f32,
    ) -> f32 {
        let distance = (sphere_center - camera_pos).length();
        if distance <= 0.0 {
            return f32::INFINITY; // Very close, use highest LOD
        }

        // Calculate the angular size in radians
        let angular_size = 2.0 * (sphere_radius / distance).atan();

        // Convert to screen space pixels

        (angular_size / fov_y) * screen_height
    }
}

/// LOD calculator for individual meshes
pub struct LODCalculator {
    pub manager: Arc<LODManager>,
}

impl LODCalculator {
    /// Create a new LOD calculator
    pub fn new(manager: Arc<LODManager>) -> Self {
        Self { manager }
    }

    /// Calculate the appropriate LOD level for a mesh
    pub fn calculate_lod(
        &self,
        mesh: &MeshClass,
        camera_pos: Vec3,
        screen_height: f32,
        fov_y: f32,
    ) -> LODLevel {
        let sphere = mesh.bounding_sphere;

        let screen_size = self.manager.calculate_screen_size(
            sphere.center,
            sphere.radius,
            camera_pos,
            screen_height,
            fov_y,
        );

        // Use screen size as the primary factor
        self.manager.calculate_lod_from_screen_size(screen_size)
    }

    /// Check if a mesh should be culled based on LOD and distance
    pub fn should_cull(&self, mesh: &MeshClass, camera_pos: Vec3, max_distance: f32) -> bool {
        let sphere_center = mesh.bounding_sphere.center;
        let distance = (sphere_center - camera_pos).length();
        distance > max_distance
    }
}

/// LOD transition manager for smooth transitions
pub struct LODTransition {
    pub current_lod: LODLevel,
    pub target_lod: LODLevel,
    pub transition_time: f32,
    pub transition_progress: f32,
}

impl LODTransition {
    /// Create a new LOD transition
    pub fn new(current_lod: LODLevel, target_lod: LODLevel) -> Self {
        Self {
            current_lod,
            target_lod,
            transition_time: 0.5, // Default transition time in seconds
            transition_progress: 0.0,
        }
    }

    /// Update transition progress
    pub fn update(&mut self, delta_time: f32) {
        if self.current_lod != self.target_lod {
            self.transition_progress += delta_time / self.transition_time;
            if self.transition_progress >= 1.0 {
                self.current_lod = self.target_lod;
                self.transition_progress = 0.0;
            }
        }
    }

    /// Get interpolated LOD value for smooth transitions
    pub fn get_interpolated_value(&self) -> f32 {
        if self.current_lod == self.target_lod {
            return self.current_lod as u32 as f32;
        }

        let start = self.current_lod as u32 as f32;
        let end = self.target_lod as u32 as f32;

        // Smooth interpolation
        let t = self.transition_progress;
        let smooth_t = t * t * (3.0 - 2.0 * t); // Smoothstep function

        start + (end - start) * smooth_t
    }
}
