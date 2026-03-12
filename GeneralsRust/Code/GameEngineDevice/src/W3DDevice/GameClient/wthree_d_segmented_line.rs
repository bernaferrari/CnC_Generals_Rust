//! Segmented line render object (port of WW3D2 SegmentedLineClass).
//!
//! Provides a thick, textured line composed of segments, used by laser and rope effects.

use cgmath::{Point3, Vector2, Vector3, InnerSpace};

/// Texture mapping mode for segmented lines (mirrors SegLineRendererClass::TextureMapMode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureMapMode {
    Stretch,
    Tiled,
}

/// Segmented line render object.
#[derive(Debug, Clone)]
pub struct SegmentedLine {
    points: Vec<Point3<f32>>,
    width: f32,
    color: Vector3<f32>,
    opacity: f32,
    texture_name: Option<String>,
    texture_map_mode: TextureMapMode,
    texture_tile_factor: f32,
    uv_offset_rate: Vector2<f32>,
    uv_offset: f32,
    visible: bool,
}

impl SegmentedLine {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            width: 1.0,
            color: Vector3::new(1.0, 1.0, 1.0),
            opacity: 1.0,
            texture_name: None,
            texture_map_mode: TextureMapMode::Stretch,
            texture_tile_factor: 1.0,
            uv_offset_rate: Vector2::new(0.0, 0.0),
            uv_offset: 0.0,
            visible: true,
        }
    }

    pub fn reset_line(&mut self) {
        self.points.clear();
    }

    pub fn set_points(&mut self, points: &[Point3<f32>]) {
        self.points.clear();
        self.points.extend_from_slice(points);
    }

    pub fn get_num_points(&self) -> usize {
        self.points.len()
    }

    pub fn set_point_location(&mut self, point_idx: usize, location: Point3<f32>) {
        if let Some(point) = self.points.get_mut(point_idx) {
            *point = location;
        }
    }

    pub fn get_point_location(&self, point_idx: usize) -> Option<Point3<f32>> {
        self.points.get(point_idx).copied()
    }

    pub fn add_point(&mut self, location: Point3<f32>) {
        self.points.push(location);
    }

    pub fn delete_point(&mut self, point_idx: usize) {
        if point_idx < self.points.len() {
            self.points.remove(point_idx);
        }
    }

    pub fn get_texture_name(&self) -> Option<&str> {
        self.texture_name.as_deref()
    }

    pub fn set_texture_name(&mut self, name: Option<String>) {
        self.texture_name = name;
    }

    pub fn get_width(&self) -> f32 {
        self.width
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.max(0.0);
    }

    pub fn get_color(&self) -> Vector3<f32> {
        self.color
    }

    pub fn set_color(&mut self, color: Vector3<f32>) {
        self.color = color;
    }

    pub fn get_opacity(&self) -> f32 {
        self.opacity
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    pub fn set_texture_mapping_mode(&mut self, mode: TextureMapMode) {
        self.texture_map_mode = mode;
    }

    pub fn get_texture_mapping_mode(&self) -> TextureMapMode {
        self.texture_map_mode
    }

    pub fn set_texture_tile_factor(&mut self, factor: f32) {
        self.texture_tile_factor = factor.max(0.0);
    }

    pub fn get_texture_tile_factor(&self) -> f32 {
        self.texture_tile_factor
    }

    pub fn set_uv_offset_rate(&mut self, rate: Vector2<f32>) {
        self.uv_offset_rate = rate;
    }

    pub fn get_uv_offset_rate(&self) -> Vector2<f32> {
        self.uv_offset_rate
    }

    pub fn advance_uv(&mut self, delta_time: f32) {
        let offset = self.uv_offset_rate.y * delta_time;
        self.uv_offset = (self.uv_offset + offset) % 1.0;
    }

    pub fn get_uv_offset(&self) -> f32 {
        self.uv_offset
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn get_points(&self) -> &[Point3<f32>] {
        &self.points
    }

    pub fn get_segment_lengths(&self) -> Vec<f32> {
        let mut lengths = Vec::new();
        for idx in 0..self.points.len().saturating_sub(1) {
            let start = self.points[idx];
            let end = self.points[idx + 1];
            lengths.push((end - start).magnitude());
        }
        lengths
    }
}

impl Default for SegmentedLine {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple helper to compute a stable perpendicular for a line given camera direction.
pub fn compute_line_perp(line_dir: Vector3<f32>, camera_dir: Vector3<f32>) -> Vector3<f32> {
    let mut perp = line_dir.cross(camera_dir);
    if perp.magnitude2() < 1e-6 {
        perp = line_dir.cross(Vector3::unit_y());
    }
    if perp.magnitude2() < 1e-6 {
        perp = line_dir.cross(Vector3::unit_x());
    }
    if perp.magnitude2() > 0.0 {
        perp = perp.normalize();
    }
    perp
}
