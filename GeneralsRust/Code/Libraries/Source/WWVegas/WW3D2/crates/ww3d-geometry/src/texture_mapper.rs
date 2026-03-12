//! Texture Coordinate Transformation System
//!
//! This module provides animated texture coordinate transformations (mappers),
//! enabling scrolling textures, sprite sheet animations, rotating textures, and wave effects.
//! Matches C++ WW3D behavior for texture mappers.

use glam::{Mat3, Vec3};
use std::f32::consts::PI;

/// Coordinate system for texture transformations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateSystem {
    /// Local object space coordinates
    Local,
    /// World space coordinates
    World,
    /// Screen/camera space coordinates
    Screen,
    /// Camera-relative coordinates
    CameraRelative,
}

/// Base texture mapper trait for time-based UV transformations
pub trait TextureMapper: Send + Sync {
    /// Compute the UV transformation for the given time in seconds
    fn compute_transform(&self, time_seconds: f32) -> Mat3;

    /// Get the mapper type identifier
    fn mapper_type(&self) -> TextureMapperType;

    /// Clone the mapper
    fn clone_box(&self) -> Box<dyn TextureMapper>;

    /// Get coordinate system this mapper uses
    fn coordinate_system(&self) -> CoordinateSystem {
        CoordinateSystem::Local
    }
}

/// All texture mapper types available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureMapperType {
    Linear,
    Grid,
    Rotate,
    SineLinear,
    None,
}

/// Linear offset mapper - scrolls texture coordinates at constant rate
/// Useful for scrolling textures, lava, water flow, conveyor belts
#[derive(Debug, Clone)]
pub struct LinearOffsetMapper {
    /// Offset speed in U direction (texels per second)
    pub u_offset_per_sec: f32,
    /// Offset speed in V direction (texels per second)
    pub v_offset_per_sec: f32,
    /// Coordinate system for this mapper
    pub coordinate_system: CoordinateSystem,
}

impl LinearOffsetMapper {
    /// Create a new linear offset mapper
    pub fn new(u_offset: f32, v_offset: f32) -> Self {
        Self {
            u_offset_per_sec: u_offset,
            v_offset_per_sec: v_offset,
            coordinate_system: CoordinateSystem::Local,
        }
    }

    /// Set the coordinate system
    pub fn with_coordinate_system(mut self, coord_system: CoordinateSystem) -> Self {
        self.coordinate_system = coord_system;
        self
    }

    /// Create a scrolling water mapper (common preset)
    pub fn water_scroll() -> Self {
        Self {
            u_offset_per_sec: 0.1,
            v_offset_per_sec: 0.05,
            coordinate_system: CoordinateSystem::Local,
        }
    }

    /// Create a fast scroll mapper (conveyor belt effect)
    pub fn fast_scroll() -> Self {
        Self {
            u_offset_per_sec: 1.0,
            v_offset_per_sec: 0.0,
            coordinate_system: CoordinateSystem::Local,
        }
    }
}

impl TextureMapper for LinearOffsetMapper {
    fn compute_transform(&self, time_seconds: f32) -> Mat3 {
        let u_offset = self.u_offset_per_sec * time_seconds;
        let v_offset = self.v_offset_per_sec * time_seconds;

        // Translation matrix for UV coordinates
        // [1 0 u]
        // [0 1 v]
        // [0 0 1]
        Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(u_offset, v_offset, 1.0),
        )
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Linear
    }

    fn clone_box(&self) -> Box<dyn TextureMapper> {
        Box::new(self.clone())
    }

    fn coordinate_system(&self) -> CoordinateSystem {
        self.coordinate_system
    }
}

/// Grid mapper - for sprite sheet animations
/// Divides texture into grid and cycles through frames
#[derive(Debug, Clone)]
pub struct GridMapper {
    /// Number of columns in the grid
    pub columns: u32,
    /// Number of rows in the grid
    pub rows: u32,
    /// Animation speed (frames per second)
    pub fps: f32,
    /// Total number of frames (defaults to columns * rows if 0)
    pub frame_count: u32,
    /// Whether animation loops
    pub looping: bool,
    /// Coordinate system for this mapper
    pub coordinate_system: CoordinateSystem,
}

impl GridMapper {
    /// Create a new grid mapper
    pub fn new(columns: u32, rows: u32, fps: f32) -> Self {
        let frame_count = columns * rows;
        Self {
            columns,
            rows,
            fps,
            frame_count,
            looping: true,
            coordinate_system: CoordinateSystem::Local,
        }
    }

    /// Set the coordinate system
    pub fn with_coordinate_system(mut self, coord_system: CoordinateSystem) -> Self {
        self.coordinate_system = coord_system;
        self
    }

    /// Set custom frame count (for cases where not all grid cells are used)
    pub fn with_frame_count(mut self, count: u32) -> Self {
        self.frame_count = count;
        self
    }

    /// Set looping behavior
    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Create animation mapper for a typical sprite sheet
    pub fn sprite_animation(columns: u32, rows: u32) -> Self {
        Self::new(columns, rows, 10.0) // 10 FPS default
    }
}

impl TextureMapper for GridMapper {
    fn compute_transform(&self, time_seconds: f32) -> Mat3 {
        let frame_width = 1.0 / self.columns as f32;
        let frame_height = 1.0 / self.rows as f32;

        // Calculate current frame
        let mut frame_number = (time_seconds * self.fps) as u32;

        if self.looping {
            frame_number = frame_number % self.frame_count;
        } else {
            frame_number = frame_number.min(self.frame_count - 1);
        }

        // Convert frame number to grid position
        let col = frame_number % self.columns;
        let row = frame_number / self.columns;

        let u_offset = col as f32 * frame_width;
        let v_offset = row as f32 * frame_height;

        // Scale and translate matrix
        // [w  0  u]
        // [0  h  v]
        // [0  0  1]
        Mat3::from_cols(
            Vec3::new(frame_width, 0.0, 0.0),
            Vec3::new(0.0, frame_height, 0.0),
            Vec3::new(u_offset, v_offset, 1.0),
        )
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Grid
    }

    fn clone_box(&self) -> Box<dyn TextureMapper> {
        Box::new(self.clone())
    }

    fn coordinate_system(&self) -> CoordinateSystem {
        self.coordinate_system
    }
}

/// Rotation mapper - rotates texture coordinates around center
#[derive(Debug, Clone)]
pub struct RotateMapper {
    /// Rotation speed in degrees per second
    pub degrees_per_sec: f32,
    /// Rotation center U coordinate (0.0-1.0)
    pub center_u: f32,
    /// Rotation center V coordinate (0.0-1.0)
    pub center_v: f32,
    /// Coordinate system for this mapper
    pub coordinate_system: CoordinateSystem,
}

impl RotateMapper {
    /// Create a new rotation mapper
    pub fn new(degrees_per_sec: f32) -> Self {
        Self {
            degrees_per_sec,
            center_u: 0.5,
            center_v: 0.5,
            coordinate_system: CoordinateSystem::Local,
        }
    }

    /// Set rotation center
    pub fn with_center(mut self, u: f32, v: f32) -> Self {
        self.center_u = u.clamp(0.0, 1.0);
        self.center_v = v.clamp(0.0, 1.0);
        self
    }

    /// Set the coordinate system
    pub fn with_coordinate_system(mut self, coord_system: CoordinateSystem) -> Self {
        self.coordinate_system = coord_system;
        self
    }

    /// Create a slow rotation mapper (decorative effect)
    pub fn slow_rotate() -> Self {
        Self::new(45.0) // 45 degrees per second
    }

    /// Create a fast rotation mapper
    pub fn fast_rotate() -> Self {
        Self::new(180.0) // 180 degrees per second
    }
}

impl TextureMapper for RotateMapper {
    fn compute_transform(&self, time_seconds: f32) -> Mat3 {
        let angle_degrees = self.degrees_per_sec * time_seconds;
        let angle_radians = angle_degrees * PI / 180.0;

        let cos_a = angle_radians.cos();
        let sin_a = angle_radians.sin();

        // Compose: translate to center, rotate, translate back
        // First translate to center
        let to_center = Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(-self.center_u, -self.center_v, 1.0),
        );

        // Then rotate
        let rotation = Mat3::from_cols(
            Vec3::new(cos_a, sin_a, 0.0),
            Vec3::new(-sin_a, cos_a, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );

        // Then translate back
        let from_center = Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(self.center_u, self.center_v, 1.0),
        );

        // Compose matrices: from_center * rotation * to_center
        from_center * rotation * to_center
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::Rotate
    }

    fn clone_box(&self) -> Box<dyn TextureMapper> {
        Box::new(self.clone())
    }

    fn coordinate_system(&self) -> CoordinateSystem {
        self.coordinate_system
    }
}

/// Sine wave offset mapper - creates wave/ripple effects
/// Useful for water, cloth, and organic surface animations
#[derive(Debug, Clone)]
pub struct SineLinearOffsetMapper {
    /// Base linear offset speed in U direction
    pub u_base_offset: f32,
    /// Base linear offset speed in V direction
    pub v_base_offset: f32,
    /// Sine wave amplitude in U direction
    pub u_amplitude: f32,
    /// Sine wave amplitude in V direction
    pub v_amplitude: f32,
    /// Sine wave frequency (cycles per second)
    pub frequency: f32,
    /// Coordinate system for this mapper
    pub coordinate_system: CoordinateSystem,
}

impl SineLinearOffsetMapper {
    /// Create a new sine linear offset mapper
    pub fn new(u_base: f32, v_base: f32, amplitude: f32, frequency: f32) -> Self {
        Self {
            u_base_offset: u_base,
            v_base_offset: v_base,
            u_amplitude: amplitude,
            v_amplitude: amplitude,
            frequency,
            coordinate_system: CoordinateSystem::Local,
        }
    }

    /// Set individual amplitudes
    pub fn with_amplitudes(mut self, u_amp: f32, v_amp: f32) -> Self {
        self.u_amplitude = u_amp;
        self.v_amplitude = v_amp;
        self
    }

    /// Set the coordinate system
    pub fn with_coordinate_system(mut self, coord_system: CoordinateSystem) -> Self {
        self.coordinate_system = coord_system;
        self
    }

    /// Create a water wave mapper
    pub fn water_wave() -> Self {
        Self {
            u_base_offset: 0.05,
            v_base_offset: 0.05,
            u_amplitude: 0.02,
            v_amplitude: 0.02,
            frequency: 2.0,
            coordinate_system: CoordinateSystem::Local,
        }
    }

    /// Create a ripple mapper
    pub fn ripple() -> Self {
        Self {
            u_base_offset: 0.0,
            v_base_offset: 0.0,
            u_amplitude: 0.03,
            v_amplitude: 0.03,
            frequency: 3.0,
            coordinate_system: CoordinateSystem::Local,
        }
    }
}

impl TextureMapper for SineLinearOffsetMapper {
    fn compute_transform(&self, time_seconds: f32) -> Mat3 {
        // Linear component
        let u_linear = self.u_base_offset * time_seconds;
        let v_linear = self.v_base_offset * time_seconds;

        // Sine wave component
        let wave_phase = time_seconds * self.frequency * 2.0 * PI;
        let u_sine = self.u_amplitude * wave_phase.sin();
        let v_sine = self.v_amplitude * wave_phase.sin();

        // Combined offset
        let u_offset = u_linear + u_sine;
        let v_offset = v_linear + v_sine;

        // Translation matrix
        Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(u_offset, v_offset, 1.0),
        )
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::SineLinear
    }

    fn clone_box(&self) -> Box<dyn TextureMapper> {
        Box::new(self.clone())
    }

    fn coordinate_system(&self) -> CoordinateSystem {
        self.coordinate_system
    }
}

/// No-op mapper for testing and default cases
#[derive(Debug, Clone)]
pub struct NoOpMapper;

impl TextureMapper for NoOpMapper {
    fn compute_transform(&self, _time_seconds: f32) -> Mat3 {
        Mat3::IDENTITY
    }

    fn mapper_type(&self) -> TextureMapperType {
        TextureMapperType::None
    }

    fn clone_box(&self) -> Box<dyn TextureMapper> {
        Box::new(self.clone())
    }
}

/// Container for texture mapper state
pub struct TextureMapperState {
    /// The mapper implementation
    pub mapper: Box<dyn TextureMapper>,
    /// Whether this mapper is currently active
    pub enabled: bool,
}

impl TextureMapperState {
    /// Create a new mapper state
    pub fn new(mapper: Box<dyn TextureMapper>) -> Self {
        Self {
            mapper,
            enabled: true,
        }
    }

    /// Create from a specific mapper type
    pub fn from_mapper(mapper: Box<dyn TextureMapper>) -> Self {
        Self::new(mapper)
    }

    /// Enable or disable the mapper
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Compute the transformation matrix for given time
    pub fn compute_transform(&self, time_seconds: f32) -> Mat3 {
        if self.enabled {
            self.mapper.compute_transform(time_seconds)
        } else {
            Mat3::IDENTITY
        }
    }
}

impl Clone for TextureMapperState {
    fn clone(&self) -> Self {
        Self {
            mapper: self.mapper.clone_box(),
            enabled: self.enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_offset() {
        let mapper = LinearOffsetMapper::new(0.5, 0.25);
        let transform = mapper.compute_transform(2.0);

        // After 2 seconds with 0.5 u/sec offset, should be 1.0
        assert!((transform.z_axis.x - 1.0).abs() < 0.0001);
        assert!((transform.z_axis.y - 0.5).abs() < 0.0001);
    }

    #[test]
    fn test_grid_mapper() {
        let mapper = GridMapper::new(4, 4, 10.0);
        let transform = mapper.compute_transform(0.0);

        // First frame should be at origin
        assert!((transform.z_axis.x - 0.0).abs() < 0.0001);
        assert!((transform.z_axis.y - 0.0).abs() < 0.0001);

        // Check scale
        assert!((transform.x_axis.x - 0.25).abs() < 0.0001); // 1/4
        assert!((transform.y_axis.y - 0.25).abs() < 0.0001); // 1/4
    }

    #[test]
    fn test_rotate_mapper() {
        let mapper = RotateMapper::new(90.0);
        let transform = mapper.compute_transform(1.0); // 90 degrees

        // At 90 degrees, rotation matrix should have specific properties
        // Just verify it's not identity and not NaN
        assert!(transform.x_axis.x.abs() < 0.1);
        assert!(!transform.x_axis.x.is_nan());
    }

    #[test]
    fn test_sine_offset() {
        let mapper = SineLinearOffsetMapper::new(0.0, 0.0, 0.1, 1.0);
        let transform1 = mapper.compute_transform(0.0);
        let transform2 = mapper.compute_transform(0.25);

        // At t=0, sine is 0
        assert!((transform1.z_axis.x - 0.0).abs() < 0.0001);
        assert!((transform1.z_axis.y - 0.0).abs() < 0.0001);

        // At t=0.25 with frequency=1, should be near peak
        assert!(transform2.z_axis.x.abs() > 0.05);
    }

    #[test]
    fn test_mapper_types() {
        let linear: Box<dyn TextureMapper> = Box::new(LinearOffsetMapper::new(1.0, 1.0));
        assert_eq!(linear.mapper_type(), TextureMapperType::Linear);

        let grid: Box<dyn TextureMapper> = Box::new(GridMapper::new(2, 2, 10.0));
        assert_eq!(grid.mapper_type(), TextureMapperType::Grid);

        let rotate: Box<dyn TextureMapper> = Box::new(RotateMapper::new(90.0));
        assert_eq!(rotate.mapper_type(), TextureMapperType::Rotate);

        let sine: Box<dyn TextureMapper> =
            Box::new(SineLinearOffsetMapper::new(0.1, 0.1, 0.05, 2.0));
        assert_eq!(sine.mapper_type(), TextureMapperType::SineLinear);
    }

    #[test]
    fn test_linear_offset_progression() {
        let mapper = LinearOffsetMapper::new(0.1, 0.05);

        let t0 = mapper.compute_transform(0.0);
        let t1 = mapper.compute_transform(1.0);
        let t2 = mapper.compute_transform(2.0);

        // Check progression
        assert!((t0.z_axis.x - 0.0).abs() < 0.0001);
        assert!((t1.z_axis.x - 0.1).abs() < 0.0001);
        assert!((t2.z_axis.x - 0.2).abs() < 0.0001);
    }

    #[test]
    fn test_noop_mapper() {
        let mapper = NoOpMapper;
        let transform = mapper.compute_transform(100.0);

        assert_eq!(transform, Mat3::IDENTITY);
    }
}
