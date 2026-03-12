//! Distance-Based Level of Detail (LOD) System
//!
//! Port of distlod.cpp implementing automatic LOD selection based on camera distance.
//! Key algorithms:
//! - Distance calculation (lines 1100-1103)
//! - LOD selection thresholds (lines 1105-1109)
//! - LOD transitions (lines 1125-1163)

use glam::Vec3;
use std::sync::Arc;

/// Maximum LOD levels supported
pub const MAX_LOD_LEVELS: usize = 8;

/// LOD level definition
/// Port of C++ LODNodeClass (distlod.cpp lines 394-405)
#[derive(Debug, Clone)]
pub struct LodLevel {
    /// Maximum distance for this LOD (switch to next LOD beyond this)
    pub max_distance: f32,
    /// Minimum distance for this LOD (switch to previous LOD below this)
    pub min_distance: f32,
    /// Polygon count for this LOD (for statistics)
    pub polygon_count: usize,
    /// LOD index (0 = highest detail)
    pub level_index: usize,
}

impl LodLevel {
    /// Create new LOD level
    /// Port of distlod.cpp lines 402-405
    pub fn new(
        level_index: usize,
        min_distance: f32,
        max_distance: f32,
        polygon_count: usize,
    ) -> Self {
        Self {
            max_distance,
            min_distance,
            polygon_count,
            level_index,
        }
    }

    /// Check if distance is within this LOD's range
    pub fn contains_distance(&self, distance: f32) -> bool {
        distance >= self.min_distance && distance < self.max_distance
    }
}

/// Distance LOD controller
/// Port of C++ DistLODClass (distlod.cpp lines 389-443)
pub struct DistanceLodController {
    /// LOD levels (sorted by distance, 0 = highest detail)
    lod_levels: Vec<LodLevel>,
    /// Current active LOD index
    current_lod: usize,
    /// Object position for distance calculation
    object_position: Vec3,
    /// Bounding sphere radius for distance calculation
    bounding_radius: f32,
    /// Enable smooth transitions
    enable_smooth_transitions: bool,
    /// Transition progress (0.0 to 1.0) when transitioning between LODs
    transition_progress: f32,
}

impl DistanceLodController {
    /// Create new LOD controller
    pub fn new() -> Self {
        Self {
            lod_levels: Vec::new(),
            current_lod: 0,
            object_position: Vec3::ZERO,
            bounding_radius: 1.0,
            enable_smooth_transitions: false,
            transition_progress: 1.0,
        }
    }

    /// Add LOD level
    pub fn add_lod_level(
        &mut self,
        min_distance: f32,
        max_distance: f32,
        polygon_count: usize,
    ) {
        let level_index = self.lod_levels.len();
        let level = LodLevel::new(level_index, min_distance, max_distance, polygon_count);
        self.lod_levels.push(level);

        // Keep sorted by distance
        self.lod_levels
            .sort_by(|a, b| a.min_distance.partial_cmp(&b.min_distance).unwrap());

        // Update indices after sorting
        for (i, level) in self.lod_levels.iter_mut().enumerate() {
            level.level_index = i;
        }
    }

    /// Set object position
    pub fn set_position(&mut self, position: Vec3) {
        self.object_position = position;
    }

    /// Set bounding radius
    pub fn set_bounding_radius(&mut self, radius: f32) {
        self.bounding_radius = radius;
    }

    /// Update LOD based on camera distance
    /// Port of distlod.cpp Update_Lod (lines 1097-1110)
    pub fn update_lod(&mut self, camera_position: Vec3) {
        // Calculate distance from camera to object
        // Port of distlod.cpp lines 1100-1103
        let delta = camera_position - self.object_position;
        let distance = delta.length();

        // Get current LOD threshold
        if self.lod_levels.is_empty() {
            return;
        }

        let current = &self.lod_levels[self.current_lod];

        // Port of distlod.cpp lines 1105-1109
        if distance < current.min_distance && self.current_lod > 0 {
            // Increment LOD (higher detail)
            self.increment_lod();
        } else if distance > current.max_distance
            && self.current_lod < self.lod_levels.len() - 1
        {
            // Decrement LOD (lower detail)
            self.decrement_lod();
        }
    }

    /// Move to higher detail LOD
    /// Port of distlod.cpp Increment_Lod (lines 1125-1137)
    fn increment_lod(&mut self) {
        if self.current_lod > 0 {
            self.current_lod -= 1;
            self.transition_progress = 0.0;
        }
    }

    /// Move to lower detail LOD
    /// Port of distlod.cpp Decrement_Lod (lines 1152-1163)
    fn decrement_lod(&mut self) {
        if self.current_lod < self.lod_levels.len() - 1 {
            self.current_lod += 1;
            self.transition_progress = 0.0;
        }
    }

    /// Get current LOD index
    pub fn get_current_lod(&self) -> usize {
        self.current_lod
    }

    /// Get current LOD level
    pub fn get_current_level(&self) -> Option<&LodLevel> {
        self.lod_levels.get(self.current_lod)
    }

    /// Get LOD level by index
    pub fn get_lod_level(&self, index: usize) -> Option<&LodLevel> {
        self.lod_levels.get(index)
    }

    /// Get LOD count
    pub fn get_lod_count(&self) -> usize {
        self.lod_levels.len()
    }

    /// Select LOD by distance (doesn't change current LOD, just returns index)
    /// Port of distlod.cpp select_lod logic (lines 1097-1110)
    pub fn select_lod_by_distance(&self, camera_position: Vec3) -> usize {
        let delta = camera_position - self.object_position;
        let distance = delta.length();

        for (i, level) in self.lod_levels.iter().enumerate() {
            if level.contains_distance(distance) {
                return i;
            }
        }

        // Return lowest LOD if beyond all ranges
        self.lod_levels.len().saturating_sub(1)
    }

    /// Get recommended LOD for distance
    pub fn recommend_lod_for_distance(&self, distance: f32) -> usize {
        for (i, level) in self.lod_levels.iter().enumerate() {
            if distance < level.max_distance {
                return i;
            }
        }
        self.lod_levels.len().saturating_sub(1)
    }

    /// Enable smooth LOD transitions
    pub fn set_smooth_transitions(&mut self, enable: bool) {
        self.enable_smooth_transitions = enable;
    }

    /// Update transition progress (for smooth LOD blending)
    pub fn update_transition(&mut self, delta_time: f32, transition_speed: f32) {
        if self.transition_progress < 1.0 {
            self.transition_progress =
                (self.transition_progress + delta_time * transition_speed).min(1.0);
        }
    }

    /// Get transition progress
    pub fn get_transition_progress(&self) -> f32 {
        self.transition_progress
    }

    /// Force specific LOD
    pub fn force_lod(&mut self, lod_index: usize) {
        if lod_index < self.lod_levels.len() {
            self.current_lod = lod_index;
            self.transition_progress = 1.0;
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> LodStats {
        let current_polys = self
            .get_current_level()
            .map(|l| l.polygon_count)
            .unwrap_or(0);

        let max_polys = self
            .lod_levels
            .first()
            .map(|l| l.polygon_count)
            .unwrap_or(0);

        LodStats {
            current_lod: self.current_lod,
            lod_count: self.lod_levels.len(),
            current_poly_count: current_polys,
            max_poly_count: max_polys,
            poly_reduction_ratio: if max_polys > 0 {
                current_polys as f32 / max_polys as f32
            } else {
                1.0
            },
        }
    }
}

impl Default for DistanceLodController {
    fn default() -> Self {
        Self::new()
    }
}

/// LOD statistics
#[derive(Debug, Clone, Copy)]
pub struct LodStats {
    pub current_lod: usize,
    pub lod_count: usize,
    pub current_poly_count: usize,
    pub max_poly_count: usize,
    pub poly_reduction_ratio: f32,
}

/// LOD Builder helper for constructing common LOD configurations
pub struct LodBuilder {
    base_distance: f32,
    distance_multiplier: f32,
    levels: Vec<(f32, f32, usize)>,
}

impl LodBuilder {
    /// Create new LOD builder
    pub fn new(base_distance: f32) -> Self {
        Self {
            base_distance,
            distance_multiplier: 2.0,
            levels: Vec::new(),
        }
    }

    /// Set distance multiplier between LODs
    pub fn with_distance_multiplier(mut self, multiplier: f32) -> Self {
        self.distance_multiplier = multiplier;
        self
    }

    /// Add LOD level with polygon count
    pub fn add_level(mut self, polygon_count: usize) -> Self {
        let level_idx = self.levels.len();
        let min_dist = if level_idx == 0 {
            0.0
        } else {
            self.base_distance * self.distance_multiplier.powi(level_idx as i32 - 1)
        };
        let max_dist = if level_idx == 0 {
            self.base_distance
        } else {
            self.base_distance * self.distance_multiplier.powi(level_idx as i32)
        };

        self.levels.push((min_dist, max_dist, polygon_count));
        self
    }

    /// Build LOD controller
    pub fn build(self) -> DistanceLodController {
        let mut controller = DistanceLodController::new();

        for (min_dist, max_dist, poly_count) in self.levels {
            controller.add_lod_level(min_dist, max_dist, poly_count);
        }

        controller
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lod_level_creation() {
        let level = LodLevel::new(0, 0.0, 100.0, 5000);
        assert_eq!(level.level_index, 0);
        assert_eq!(level.min_distance, 0.0);
        assert_eq!(level.max_distance, 100.0);
        assert_eq!(level.polygon_count, 5000);
    }

    #[test]
    fn test_lod_level_contains_distance() {
        let level = LodLevel::new(0, 0.0, 100.0, 5000);
        assert!(level.contains_distance(50.0));
        assert!(!level.contains_distance(150.0));
        assert!(level.contains_distance(0.0));
        assert!(!level.contains_distance(100.0)); // Exclusive upper bound
    }

    #[test]
    fn test_distance_lod_controller_creation() {
        let controller = DistanceLodController::new();
        assert_eq!(controller.get_lod_count(), 0);
        assert_eq!(controller.get_current_lod(), 0);
    }

    #[test]
    fn test_distance_lod_controller_add_levels() {
        let mut controller = DistanceLodController::new();

        controller.add_lod_level(0.0, 50.0, 5000);
        controller.add_lod_level(50.0, 100.0, 2500);
        controller.add_lod_level(100.0, 200.0, 1000);

        assert_eq!(controller.get_lod_count(), 3);
    }

    #[test]
    fn test_distance_lod_selection() {
        let mut controller = DistanceLodController::new();

        controller.add_lod_level(0.0, 50.0, 5000);
        controller.add_lod_level(50.0, 100.0, 2500);
        controller.add_lod_level(100.0, 200.0, 1000);

        controller.set_position(Vec3::ZERO);

        // Close camera should select LOD 0
        let lod = controller.select_lod_by_distance(Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(lod, 0);

        // Medium camera should select LOD 1
        let lod = controller.select_lod_by_distance(Vec3::new(75.0, 0.0, 0.0));
        assert_eq!(lod, 1);

        // Far camera should select LOD 2
        let lod = controller.select_lod_by_distance(Vec3::new(150.0, 0.0, 0.0));
        assert_eq!(lod, 2);
    }

    #[test]
    fn test_distance_lod_update() {
        let mut controller = DistanceLodController::new();

        controller.add_lod_level(0.0, 50.0, 5000);
        controller.add_lod_level(50.0, 100.0, 2500);
        controller.add_lod_level(100.0, 200.0, 1000);

        controller.set_position(Vec3::ZERO);

        // Start at LOD 0
        assert_eq!(controller.get_current_lod(), 0);

        // Move camera far away
        controller.update_lod(Vec3::new(75.0, 0.0, 0.0));
        assert_eq!(controller.get_current_lod(), 1);

        controller.update_lod(Vec3::new(150.0, 0.0, 0.0));
        assert_eq!(controller.get_current_lod(), 2);

        // Move camera close again
        controller.update_lod(Vec3::new(25.0, 0.0, 0.0));
        assert_eq!(controller.get_current_lod(), 0);
    }

    #[test]
    fn test_lod_force() {
        let mut controller = DistanceLodController::new();

        controller.add_lod_level(0.0, 50.0, 5000);
        controller.add_lod_level(50.0, 100.0, 2500);
        controller.add_lod_level(100.0, 200.0, 1000);

        controller.force_lod(2);
        assert_eq!(controller.get_current_lod(), 2);

        controller.force_lod(0);
        assert_eq!(controller.get_current_lod(), 0);
    }

    #[test]
    fn test_lod_stats() {
        let mut controller = DistanceLodController::new();

        controller.add_lod_level(0.0, 50.0, 5000);
        controller.add_lod_level(50.0, 100.0, 2500);
        controller.add_lod_level(100.0, 200.0, 1000);

        let stats = controller.get_stats();
        assert_eq!(stats.current_lod, 0);
        assert_eq!(stats.lod_count, 3);
        assert_eq!(stats.current_poly_count, 5000);
        assert_eq!(stats.max_poly_count, 5000);
        assert_eq!(stats.poly_reduction_ratio, 1.0);

        controller.force_lod(2);
        let stats = controller.get_stats();
        assert_eq!(stats.current_poly_count, 1000);
        assert_eq!(stats.poly_reduction_ratio, 0.2); // 1000/5000
    }

    #[test]
    fn test_lod_builder() {
        let controller = LodBuilder::new(50.0)
            .with_distance_multiplier(2.0)
            .add_level(5000)
            .add_level(2500)
            .add_level(1000)
            .build();

        assert_eq!(controller.get_lod_count(), 3);

        let level0 = controller.get_lod_level(0).unwrap();
        assert_eq!(level0.min_distance, 0.0);
        assert_eq!(level0.max_distance, 50.0);

        let level1 = controller.get_lod_level(1).unwrap();
        assert_eq!(level1.min_distance, 50.0);
        assert_eq!(level1.max_distance, 100.0);

        let level2 = controller.get_lod_level(2).unwrap();
        assert_eq!(level2.min_distance, 100.0);
        assert_eq!(level2.max_distance, 200.0);
    }

    #[test]
    fn test_smooth_transitions() {
        let mut controller = DistanceLodController::new();
        controller.add_lod_level(0.0, 50.0, 5000);
        controller.add_lod_level(50.0, 100.0, 2500);

        controller.set_smooth_transitions(true);

        // Trigger transition
        controller.set_position(Vec3::ZERO);
        controller.update_lod(Vec3::new(75.0, 0.0, 0.0));

        assert_eq!(controller.get_transition_progress(), 0.0);

        controller.update_transition(0.5, 1.0);
        assert_eq!(controller.get_transition_progress(), 0.5);

        controller.update_transition(1.0, 1.0);
        assert_eq!(controller.get_transition_progress(), 1.0);
    }

    #[test]
    fn test_recommend_lod_for_distance() {
        let mut controller = DistanceLodController::new();
        controller.add_lod_level(0.0, 50.0, 5000);
        controller.add_lod_level(50.0, 100.0, 2500);
        controller.add_lod_level(100.0, 200.0, 1000);

        assert_eq!(controller.recommend_lod_for_distance(25.0), 0);
        assert_eq!(controller.recommend_lod_for_distance(75.0), 1);
        assert_eq!(controller.recommend_lod_for_distance(150.0), 2);
        assert_eq!(controller.recommend_lod_for_distance(300.0), 2); // Beyond all ranges
    }
}
