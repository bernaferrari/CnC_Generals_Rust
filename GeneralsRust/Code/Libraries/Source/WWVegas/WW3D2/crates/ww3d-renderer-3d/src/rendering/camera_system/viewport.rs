//! Viewport Class - Screen space rectangle for camera rendering
//!
//! This module implements the ViewportClass from the original C++ code,
//! providing viewport management for screen rendering.

use glam::Vec2;

/// Viewport Class - Defines normalized screen space rectangle
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportClass {
    /// Minimum point (normalized coordinates)
    pub min: Vec2,
    /// Maximum point (normalized coordinates)
    pub max: Vec2,
}

impl ViewportClass {
    /// Create default viewport (full screen)
    pub fn new() -> Self {
        Self {
            min: Vec2::ZERO,
            max: Vec2::ONE,
        }
    }

    /// Create viewport from min/max points
    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Create viewport from position and size
    pub fn from_position_size(position: Vec2, size: Vec2) -> Self {
        Self {
            min: position,
            max: position + size,
        }
    }

    /// Get viewport width
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// Get viewport height
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Get viewport size
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width(), self.height())
    }

    /// Get viewport center
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Check if point is inside viewport
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Convert normalized coordinates to screen coordinates
    pub fn normalized_to_screen(&self, normalized: Vec2, screen_size: Vec2) -> Vec2 {
        Vec2::new(
            self.min.x * screen_size.x + normalized.x * self.width() * screen_size.x,
            self.min.y * screen_size.y + normalized.y * self.height() * screen_size.y,
        )
    }

    /// Convert screen coordinates to normalized coordinates
    pub fn screen_to_normalized(&self, screen: Vec2, screen_size: Vec2) -> Vec2 {
        Vec2::new(
            (screen.x - self.min.x * screen_size.x) / (self.width() * screen_size.x),
            (screen.y - self.min.y * screen_size.y) / (self.height() * screen_size.y),
        )
    }

    /// Get aspect ratio
    pub fn aspect_ratio(&self) -> f32 {
        self.width() / self.height()
    }

    /// Create sub-viewport
    pub fn sub_viewport(&self, relative_min: Vec2, relative_max: Vec2) -> Self {
        let abs_min = self.min + relative_min * self.size();
        let abs_max = self.min + relative_max * self.size();
        Self::from_min_max(abs_min, abs_max)
    }
}

impl Default for ViewportClass {
    fn default() -> Self {
        Self::new()
    }
}
