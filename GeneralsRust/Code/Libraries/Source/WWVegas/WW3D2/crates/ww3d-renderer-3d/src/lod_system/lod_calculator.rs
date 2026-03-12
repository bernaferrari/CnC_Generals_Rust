//! LOD Calculator - Integrated into 3D Renderer
//!
//! This module handles the calculation of appropriate LOD levels based on
//! distance, screen space coverage, and other factors for the 3D renderer.

use glam::{Mat4, Vec3};

/// LOD calculation methods
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LodCalculationMethod {
    Distance,    // Simple distance-based calculation
    ScreenSpace, // Screen space coverage-based
    Hybrid,      // Combination of distance and screen space
    Custom,      // User-defined calculation
}

/// LOD calculation parameters
#[derive(Debug, Clone)]
pub struct LodCalculationParams {
    pub method: LodCalculationMethod,
    pub distance_thresholds: Vec<f32>,
    pub screen_space_thresholds: Vec<f32>,
    pub hysteresis: f32,
    pub min_lod_level: u32,
    pub max_lod_level: u32,
}

impl Default for LodCalculationParams {
    fn default() -> Self {
        Self {
            method: LodCalculationMethod::Distance,
            distance_thresholds: vec![10.0, 25.0, 50.0, 100.0, 250.0],
            screen_space_thresholds: vec![0.8, 0.6, 0.4, 0.2, 0.1],
            hysteresis: 0.1,
            min_lod_level: 0,
            max_lod_level: 4,
        }
    }
}

/// LOD calculator for determining appropriate LOD levels
#[derive(Debug)]
pub struct LodCalculator {
    params: LodCalculationParams,
}

impl LodCalculator {
    /// Create a new LOD calculator with default parameters
    pub fn new() -> Self {
        Self {
            params: LodCalculationParams::default(),
        }
    }

    /// Create a LOD calculator with custom parameters
    pub fn with_params(params: LodCalculationParams) -> Self {
        Self { params }
    }

    /// Calculate the appropriate LOD level for an object
    pub fn calculate_lod_level(
        &self,
        object_position: Vec3,
        camera_position: Vec3,
        camera_projection: &Mat4,
        screen_size: (u32, u32),
        object_bounding_radius: f32,
        current_lod_level: u32,
    ) -> u32 {
        let distance = camera_position.distance(object_position);

        match self.params.method {
            LodCalculationMethod::Distance => {
                self.calculate_distance_lod(distance, current_lod_level)
            }
            LodCalculationMethod::ScreenSpace => {
                let screen_space = self.calculate_screen_space(
                    object_position,
                    camera_position,
                    camera_projection,
                    screen_size,
                    object_bounding_radius,
                );
                self.calculate_screen_space_lod(screen_space, current_lod_level)
            }
            LodCalculationMethod::Hybrid => {
                let screen_space = self.calculate_screen_space(
                    object_position,
                    camera_position,
                    camera_projection,
                    screen_size,
                    object_bounding_radius,
                );
                self.calculate_hybrid_lod(distance, screen_space, current_lod_level)
            }
            LodCalculationMethod::Custom => {
                // Default to distance-based for custom method
                self.calculate_distance_lod(distance, current_lod_level)
            }
        }
    }

    /// Calculate LOD level based on distance only
    fn calculate_distance_lod(&self, distance: f32, current_lod: u32) -> u32 {
        let mut lod_level = self.params.max_lod_level;

        for (i, &threshold) in self.params.distance_thresholds.iter().enumerate() {
            if distance <= threshold {
                lod_level = i as u32;
                break;
            }
        }

        // Apply hysteresis to prevent rapid LOD switching
        if lod_level > current_lod {
            // Switching to higher LOD (more detailed)
            let threshold = self
                .params
                .distance_thresholds
                .get(current_lod as usize)
                .copied()
                .unwrap_or(0.0);
            if distance > threshold * (1.0 + self.params.hysteresis) {
                lod_level = current_lod;
            }
        } else if lod_level < current_lod {
            // Switching to lower LOD (less detailed)
            let threshold = self
                .params
                .distance_thresholds
                .get(lod_level as usize)
                .copied()
                .unwrap_or(0.0);
            if distance < threshold * (1.0 - self.params.hysteresis) {
                lod_level = current_lod;
            }
        }

        lod_level.clamp(self.params.min_lod_level, self.params.max_lod_level)
    }

    /// Calculate LOD level based on screen space coverage
    fn calculate_screen_space_lod(&self, screen_space: f32, current_lod: u32) -> u32 {
        let mut lod_level = self.params.max_lod_level;

        for (i, &threshold) in self.params.screen_space_thresholds.iter().enumerate() {
            if screen_space >= threshold {
                lod_level = i as u32;
                break;
            }
        }

        // Apply hysteresis
        if lod_level > current_lod {
            let threshold = self
                .params
                .screen_space_thresholds
                .get(current_lod as usize)
                .copied()
                .unwrap_or(1.0);
            if screen_space < threshold * (1.0 - self.params.hysteresis) {
                lod_level = current_lod;
            }
        } else if lod_level < current_lod {
            let threshold = self
                .params
                .screen_space_thresholds
                .get(lod_level as usize)
                .copied()
                .unwrap_or(1.0);
            if screen_space > threshold * (1.0 + self.params.hysteresis) {
                lod_level = current_lod;
            }
        }

        lod_level.clamp(self.params.min_lod_level, self.params.max_lod_level)
    }

    /// Calculate LOD level using hybrid approach
    fn calculate_hybrid_lod(&self, distance: f32, screen_space: f32, current_lod: u32) -> u32 {
        let distance_lod = self.calculate_distance_lod(distance, current_lod);
        let screen_space_lod = self.calculate_screen_space_lod(screen_space, current_lod);

        // Use the more conservative (higher) LOD level
        let hybrid_lod = distance_lod.max(screen_space_lod);

        // Apply hysteresis to the hybrid result
        if hybrid_lod > current_lod {
            // Check both thresholds
            let distance_ok = if (current_lod as usize) < self.params.distance_thresholds.len() {
                distance
                    > self.params.distance_thresholds[current_lod as usize]
                        * (1.0 + self.params.hysteresis)
            } else {
                true
            };

            let screen_space_ok =
                if (current_lod as usize) < self.params.screen_space_thresholds.len() {
                    screen_space
                        < self.params.screen_space_thresholds[current_lod as usize]
                            * (1.0 - self.params.hysteresis)
                } else {
                    true
                };

            if !distance_ok || !screen_space_ok {
                return current_lod;
            }
        } else if hybrid_lod < current_lod {
            let distance_ok = if (hybrid_lod as usize) < self.params.distance_thresholds.len() {
                distance
                    < self.params.distance_thresholds[hybrid_lod as usize]
                        * (1.0 - self.params.hysteresis)
            } else {
                true
            };

            let screen_space_ok =
                if (hybrid_lod as usize) < self.params.screen_space_thresholds.len() {
                    screen_space
                        > self.params.screen_space_thresholds[hybrid_lod as usize]
                            * (1.0 + self.params.hysteresis)
                } else {
                    true
                };

            if !distance_ok || !screen_space_ok {
                return current_lod;
            }
        }

        hybrid_lod.clamp(self.params.min_lod_level, self.params.max_lod_level)
    }

    /// Calculate screen space coverage of an object
    fn calculate_screen_space(
        &self,
        object_position: Vec3,
        camera_position: Vec3,
        camera_projection: &Mat4,
        screen_size: (u32, u32),
        object_bounding_radius: f32,
    ) -> f32 {
        let distance = camera_position.distance(object_position);

        if distance <= 0.0 {
            return 1.0; // Object is at camera position
        }

        // Calculate the angular size of the object
        let angular_size = 2.0 * (object_bounding_radius / distance).atan();

        // Convert to screen space using the projection matrix vertical FOV
        let screen_height_pixels = screen_size.1 as f32;
        let scale_y = camera_projection.y_axis.y;
        let fov_radians = if scale_y.is_finite() && scale_y > 0.0 {
            2.0 * (1.0 / scale_y).atan()
        } else {
            90.0_f32.to_radians()
        };
        let screen_space = (angular_size / fov_radians) * screen_height_pixels;

        // Normalize to 0-1 range (assuming object should take up to half screen height at max detail)
        (screen_space / (screen_height_pixels * 0.5)).min(1.0)
    }

    /// Update calculation parameters
    pub fn update_params(&mut self, params: LodCalculationParams) {
        self.params = params;
    }

    /// Get current parameters
    pub fn params(&self) -> &LodCalculationParams {
        &self.params
    }

    /// Get mutable parameters
    pub fn params_mut(&mut self) -> &mut LodCalculationParams {
        &mut self.params
    }
}
