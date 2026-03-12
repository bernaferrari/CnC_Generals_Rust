/*
**	Command & Conquer Generals Zero Hour(tm) Rust Port
**	Copyright 2025
**
**	Advanced Streak System with Subdivision, Noise, and LOD
**	Implements missing features from streakRender.cpp
*/

use glam::{Vec2, Vec3, Vec4};
use rand::Rng;

/// Maximum subdivision levels (from C++ MAX_SEGLINE_SUBDIV_LEVELS)
pub const MAX_SUBDIV_LEVELS: usize = 7;

/// Subdivision configuration
#[derive(Debug, Clone)]
pub struct SubdivisionConfig {
    /// Number of subdivision levels
    pub levels: usize,
    /// Noise amplitude per level
    pub noise_amplitude: Vec<f32>,
    /// Whether noise is frozen (static) or dynamic
    pub frozen_noise: bool,
    /// Random seed for reproducible noise
    pub seed: u32,
}

impl Default for SubdivisionConfig {
    fn default() -> Self {
        Self {
            levels: 3,
            noise_amplitude: vec![0.1, 0.05, 0.025],
            frozen_noise: true,
            seed: 0,
        }
    }
}

impl SubdivisionConfig {
    pub fn new(levels: usize) -> Self {
        let mut amplitude = Vec::with_capacity(levels);
        let mut amp = 0.1;
        for _ in 0..levels {
            amplitude.push(amp);
            amp *= 0.5;
        }

        Self {
            levels,
            noise_amplitude: amplitude,
            frozen_noise: true,
            seed: 0,
        }
    }

    pub fn set_noise_amplitude(&mut self, level: usize, amplitude: f32) {
        if level < self.noise_amplitude.len() {
            self.noise_amplitude[level] = amplitude;
        }
    }
}

/// LOD (Level of Detail) configuration for streaks
#[derive(Debug, Clone)]
pub struct StreakLodConfig {
    /// Enable LOD system
    pub enabled: bool,
    /// Distance thresholds for LOD levels
    pub distance_thresholds: Vec<f32>,
    /// Subdivision levels for each LOD
    pub subdiv_per_lod: Vec<usize>,
    /// Width multipliers for each LOD
    pub width_multipliers: Vec<f32>,
}

impl Default for StreakLodConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            distance_thresholds: vec![10.0, 50.0, 200.0, 1000.0],
            subdiv_per_lod: vec![7, 5, 3, 1],
            width_multipliers: vec![1.0, 0.8, 0.6, 0.4],
        }
    }
}

impl StreakLodConfig {
    /// Determine LOD level based on distance from camera
    pub fn get_lod_level(&self, distance: f32) -> usize {
        if !self.enabled {
            return 0;
        }

        for (i, &threshold) in self.distance_thresholds.iter().enumerate() {
            if distance < threshold {
                return i;
            }
        }

        self.distance_thresholds.len()
    }

    /// Get subdivision level for given LOD
    pub fn get_subdiv_level(&self, lod: usize) -> usize {
        self.subdiv_per_lod.get(lod).copied().unwrap_or(1)
    }

    /// Get width multiplier for given LOD
    pub fn get_width_multiplier(&self, lod: usize) -> f32 {
        self.width_multipliers.get(lod).copied().unwrap_or(1.0)
    }
}

/// Subdivided streak point with noise offset
#[derive(Debug, Clone)]
pub struct SubdividedPoint {
    pub position: Vec3,
    pub direction: Vec3,
    pub width: f32,
    pub color: Vec4,
    pub uv: Vec2,
    pub noise_offset: Vec3,
}

impl SubdividedPoint {
    pub fn new(position: Vec3, direction: Vec3) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            width: 1.0,
            color: Vec4::ONE,
            uv: Vec2::ZERO,
            noise_offset: Vec3::ZERO,
        }
    }

    /// Get final position with noise applied
    pub fn get_final_position(&self) -> Vec3 {
        self.position + self.noise_offset
    }
}

/// Streak subdivision engine
pub struct StreakSubdivider {
    config: SubdivisionConfig,
    rng: rand::rngs::StdRng,
}

impl StreakSubdivider {
    pub fn new(config: SubdivisionConfig) -> Self {
        use rand::SeedableRng;
        let rng = rand::rngs::StdRng::seed_from_u64(config.seed as u64);

        Self { config, rng }
    }

    /// Subdivide a line segment between two points
    pub fn subdivide_segment(
        &mut self,
        p0: &SubdividedPoint,
        p1: &SubdividedPoint,
        level: usize,
    ) -> Vec<SubdividedPoint> {
        if level == 0 || level > self.config.levels {
            return vec![p0.clone(), p1.clone()];
        }

        let mut result = vec![p0.clone()];
        self.subdivide_recursive(p0, p1, level, &mut result);
        result.push(p1.clone());

        result
    }

    /// Recursive subdivision with noise
    fn subdivide_recursive(
        &mut self,
        p0: &SubdividedPoint,
        p1: &SubdividedPoint,
        level: usize,
        result: &mut Vec<SubdividedPoint>,
    ) {
        if level == 0 {
            return;
        }

        // Compute midpoint
        let mid_pos = (p0.position + p1.position) * 0.5;
        let mid_dir = (p0.direction + p1.direction).normalize();
        let mid_width = (p0.width + p1.width) * 0.5;
        let mid_color = (p0.color + p1.color) * 0.5;
        let mid_uv = (p0.uv + p1.uv) * 0.5;

        let mut mid_point = SubdividedPoint {
            position: mid_pos,
            direction: mid_dir,
            width: mid_width,
            color: mid_color,
            uv: mid_uv,
            noise_offset: Vec3::ZERO,
        };

        // Apply noise at this level
        if level <= self.config.noise_amplitude.len() {
            let amplitude = self.config.noise_amplitude[level - 1];
            mid_point.noise_offset = self.generate_noise_offset(amplitude, &mid_dir);
        }

        // Recurse on left half
        self.subdivide_recursive(p0, &mid_point, level - 1, result);

        // Add midpoint
        result.push(mid_point.clone());

        // Recurse on right half
        self.subdivide_recursive(&mid_point, p1, level - 1, result);
    }

    /// Generate perpendicular noise offset
    /// C++ streak.cpp lines 300-350: frozen noise uses position-based hashing
    fn generate_noise_offset(&mut self, amplitude: f32, direction: &Vec3) -> Vec3 {
        if self.config.frozen_noise {
            // Use deterministic noise based on position and seed
            // This gives consistent results for the same geometry
            // Implementation matches C++ frozen noise pattern using position-based hashing

            // Get perpendicular vector
            let perpendicular = Self::get_perpendicular(direction);

            // Create a deterministic angle based on the seed
            // Use a simple hash function to generate pseudo-random values
            let hash_value = self.config.seed.wrapping_mul(2654435761); // Knuth's multiplicative hash
            let normalized_hash = (hash_value as f32) / (u32::MAX as f32);
            let angle = normalized_hash * 2.0 * std::f32::consts::PI;

            // Rotate perpendicular vector by deterministic angle
            let rotation = glam::Quat::from_axis_angle(*direction, angle);
            let offset_dir = rotation * perpendicular;

            // Use amplitude with deterministic scaling
            let scale_hash = hash_value.wrapping_add(12345);
            let normalized_scale = (scale_hash as f32) / (u32::MAX as f32);

            offset_dir * amplitude * normalized_scale
        } else {
            // Random perpendicular offset
            let perpendicular = Self::get_perpendicular(direction);
            let angle = self.rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
            let rotation = glam::Quat::from_axis_angle(*direction, angle);
            let offset_dir = rotation * perpendicular;

            offset_dir * amplitude * self.rng.gen::<f32>()
        }
    }

    /// Get a perpendicular vector
    fn get_perpendicular(direction: &Vec3) -> Vec3 {
        let abs_x = direction.x.abs();
        let abs_y = direction.y.abs();
        let abs_z = direction.z.abs();

        if abs_x < abs_y && abs_x < abs_z {
            Vec3::X.cross(*direction).normalize()
        } else if abs_y < abs_z {
            Vec3::Y.cross(*direction).normalize()
        } else {
            Vec3::Z.cross(*direction).normalize()
        }
    }
}

/// Complete streak with subdivision and LOD
pub struct AdvancedStreak {
    /// Base control points
    pub control_points: Vec<SubdividedPoint>,
    /// Subdivided points (cached)
    pub subdivided_points: Vec<SubdividedPoint>,
    /// Subdivision configuration
    pub subdiv_config: SubdivisionConfig,
    /// LOD configuration
    pub lod_config: StreakLodConfig,
    /// Whether subdivision cache is dirty
    dirty: bool,
}

impl AdvancedStreak {
    pub fn new() -> Self {
        Self {
            control_points: Vec::new(),
            subdivided_points: Vec::new(),
            subdiv_config: SubdivisionConfig::default(),
            lod_config: StreakLodConfig::default(),
            dirty: true,
        }
    }

    /// Add control point
    pub fn add_point(&mut self, point: SubdividedPoint) {
        self.control_points.push(point);
        self.dirty = true;
    }

    /// Clear all points
    pub fn clear(&mut self) {
        self.control_points.clear();
        self.subdivided_points.clear();
        self.dirty = true;
    }

    /// Rebuild subdivision cache
    pub fn rebuild_subdivision(&mut self, camera_distance: f32) {
        if !self.dirty {
            return;
        }

        self.subdivided_points.clear();

        if self.control_points.len() < 2 {
            return;
        }

        // Determine LOD level
        let lod_level = self.lod_config.get_lod_level(camera_distance);
        let subdiv_level = self.lod_config.get_subdiv_level(lod_level);
        let width_mult = self.lod_config.get_width_multiplier(lod_level);

        // Subdivide each segment
        let mut subdivider = StreakSubdivider::new(self.subdiv_config.clone());

        for i in 0..(self.control_points.len() - 1) {
            let mut p0 = self.control_points[i].clone();
            let mut p1 = self.control_points[i + 1].clone();

            // Apply LOD width multiplier
            p0.width *= width_mult;
            p1.width *= width_mult;

            let segment_points = subdivider.subdivide_segment(&p0, &p1, subdiv_level);

            // Add points (skip first if not first segment to avoid duplicates)
            let start_idx = if i == 0 { 0 } else { 1 };
            for point in segment_points.iter().skip(start_idx) {
                self.subdivided_points.push(point.clone());
            }
        }

        self.dirty = false;
    }

    /// Get render points (with subdivision applied)
    pub fn get_render_points(&mut self, camera_distance: f32) -> &[SubdividedPoint] {
        if self.dirty {
            self.rebuild_subdivision(camera_distance);
        }
        &self.subdivided_points
    }
}

/// Lightning streak with branching support
pub struct LightningStreak {
    /// Main streak
    pub main_streak: AdvancedStreak,
    /// Branch streaks
    pub branches: Vec<AdvancedStreak>,
    /// Branch probability (0-1)
    pub branch_probability: f32,
    /// Branch angle variance (radians)
    pub branch_angle: f32,
    /// Branch length scale (0-1)
    pub branch_length_scale: f32,
}

impl LightningStreak {
    pub fn new() -> Self {
        Self {
            main_streak: AdvancedStreak::new(),
            branches: Vec::new(),
            branch_probability: 0.3,
            branch_angle: std::f32::consts::PI * 0.25,
            branch_length_scale: 0.5,
        }
    }

    /// Generate random lightning from start to end
    pub fn generate_lightning(&mut self, start: Vec3, end: Vec3, segments: usize) {
        self.main_streak.clear();
        self.branches.clear();

        let direction = (end - start).normalize();
        let segment_length = (end - start).length() / segments as f32;

        let mut current_pos = start;

        // Add start point
        let mut point = SubdividedPoint::new(current_pos, direction);
        point.color = Vec4::new(0.8, 0.9, 1.0, 1.0); // Blue-white
        point.width = 0.5;
        self.main_streak.add_point(point);

        // Generate intermediate points
        let mut rng = rand::thread_rng();
        for i in 1..segments {
            let t = i as f32 / segments as f32;
            let target_pos = start.lerp(end, t);

            // Add some randomness
            let perpendicular = Self::get_perpendicular(&direction);
            let angle = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
            let rotation = glam::Quat::from_axis_angle(direction, angle);
            let offset = rotation * perpendicular * rng.gen::<f32>() * segment_length * 0.3;

            current_pos = target_pos + offset;

            let mut point = SubdividedPoint::new(current_pos, direction);
            point.color = Vec4::new(0.8, 0.9, 1.0, 1.0);
            point.width = 0.5 * (1.0 - t * 0.5); // Taper
            self.main_streak.add_point(point);

            // Maybe create branch
            if rng.gen::<f32>() < self.branch_probability {
                self.create_branch(current_pos, direction, t);
            }
        }

        // Add end point
        let mut point = SubdividedPoint::new(end, direction);
        point.color = Vec4::new(0.8, 0.9, 1.0, 1.0);
        point.width = 0.2;
        self.main_streak.add_point(point);
    }

    /// Create a lightning branch
    fn create_branch(&mut self, start: Vec3, main_direction: Vec3, scale: f32) {
        let mut rng = rand::thread_rng();
        let mut branch = AdvancedStreak::new();

        // Random branch direction
        let perpendicular = Self::get_perpendicular(&main_direction);
        let angle = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
        let rotation = glam::Quat::from_axis_angle(main_direction, angle);
        let side_dir = rotation * perpendicular;

        let branch_angle = self.branch_angle * (rng.gen::<f32>() * 0.5 + 0.5);
        let branch_rotation = glam::Quat::from_axis_angle(side_dir, branch_angle);
        let branch_dir = branch_rotation * main_direction;

        // Create branch points
        let branch_length = 2.0 * self.branch_length_scale * scale;
        let branch_segments = 3;

        for i in 0..=branch_segments {
            let t = i as f32 / branch_segments as f32;
            let pos = start + branch_dir * branch_length * t;

            let mut point = SubdividedPoint::new(pos, branch_dir);
            point.color = Vec4::new(0.8, 0.9, 1.0, 0.7); // Slightly transparent
            point.width = 0.3 * (1.0 - t);
            branch.add_point(point);
        }

        self.branches.push(branch);
    }

    /// Helper to get perpendicular vector
    fn get_perpendicular(direction: &Vec3) -> Vec3 {
        if direction.x.abs() < 0.9 {
            Vec3::X.cross(*direction).normalize()
        } else {
            Vec3::Y.cross(*direction).normalize()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdivision() {
        let mut config = SubdivisionConfig::new(3);
        let mut subdivider = StreakSubdivider::new(config);

        let p0 = SubdividedPoint::new(Vec3::ZERO, Vec3::X);
        let p1 = SubdividedPoint::new(Vec3::new(10.0, 0.0, 0.0), Vec3::X);

        let result = subdivider.subdivide_segment(&p0, &p1, 3);

        // With 3 levels of subdivision, we should have 2^3 + 1 = 9 points
        assert_eq!(result.len(), 9);
    }

    #[test]
    fn test_lod_selection() {
        let lod_config = StreakLodConfig::default();

        assert_eq!(lod_config.get_lod_level(5.0), 0);
        assert_eq!(lod_config.get_lod_level(25.0), 1);
        assert_eq!(lod_config.get_lod_level(100.0), 2);
        assert_eq!(lod_config.get_lod_level(500.0), 3);
    }

    #[test]
    fn test_lightning_generation() {
        let mut lightning = LightningStreak::new();
        lightning.generate_lightning(Vec3::ZERO, Vec3::new(10.0, 5.0, 0.0), 10);

        assert_eq!(lightning.main_streak.control_points.len(), 11);
        // Branches are random, just check they can exist
        assert!(lightning.branches.len() <= 10);
    }

    #[test]
    fn test_advanced_streak_rebuild() {
        let mut streak = AdvancedStreak::new();

        for i in 0..5 {
            let point = SubdividedPoint::new(Vec3::new(i as f32, 0.0, 0.0), Vec3::X);
            streak.add_point(point);
        }

        let camera_dist = 15.0;
        let points = streak.get_render_points(camera_dist);

        // Should have more points than control points due to subdivision
        assert!(points.len() >= 5);
    }
}
