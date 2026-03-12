//! Debug Visualization System
//!
//! Provides debug drawing capabilities for visualizing game state,
//! collision boxes, pathfinding, and other development tools.

use nalgebra::{Matrix4, Point3, Vector3};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Debug shape types
#[derive(Debug, Clone)]
pub enum DebugShape {
    /// Line from start to end
    Line {
        start: Point3<f32>,
        end: Point3<f32>,
        color: [f32; 4],
    },
    /// Ray with origin and direction
    Ray {
        origin: Point3<f32>,
        direction: Vector3<f32>,
        length: f32,
        color: [f32; 4],
    },
    /// Axis-aligned bounding box
    AABB {
        min: Point3<f32>,
        max: Point3<f32>,
        color: [f32; 4],
    },
    /// Oriented bounding box
    OBB {
        center: Point3<f32>,
        half_extents: Vector3<f32>,
        rotation: Matrix4<f32>,
        color: [f32; 4],
    },
    /// Sphere
    Sphere {
        center: Point3<f32>,
        radius: f32,
        color: [f32; 4],
        segments: u32,
    },
    /// Circle (in XZ plane)
    Circle {
        center: Point3<f32>,
        radius: f32,
        color: [f32; 4],
        segments: u32,
    },
    /// Capsule
    Capsule {
        start: Point3<f32>,
        end: Point3<f32>,
        radius: f32,
        color: [f32; 4],
    },
    /// Cone
    Cone {
        tip: Point3<f32>,
        direction: Vector3<f32>,
        height: f32,
        radius: f32,
        color: [f32; 4],
    },
    /// Coordinate axes
    Axes { origin: Point3<f32>, size: f32 },
    /// Grid on XZ plane
    Grid {
        center: Point3<f32>,
        size: f32,
        divisions: u32,
        color: [f32; 4],
    },
    /// Frustum
    Frustum {
        corners: [Point3<f32>; 8],
        color: [f32; 4],
    },
    /// Text label at 3D position
    Text {
        position: Point3<f32>,
        text: String,
        color: [f32; 4],
    },
    /// Arrow from start to end
    Arrow {
        start: Point3<f32>,
        end: Point3<f32>,
        color: [f32; 4],
        head_size: f32,
    },
}

/// Debug draw command with lifetime
#[derive(Debug, Clone)]
pub struct DebugDrawCommand {
    shape: DebugShape,
    created_at: Instant,
    lifetime: Option<Duration>,
    depth_test: bool,
}

impl DebugDrawCommand {
    /// Create a new debug draw command
    pub fn new(shape: DebugShape) -> Self {
        Self {
            shape,
            created_at: Instant::now(),
            lifetime: None,
            depth_test: true,
        }
    }

    /// Set lifetime
    pub fn with_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = Some(lifetime);
        self
    }

    /// Set depth testing
    pub fn with_depth_test(mut self, depth_test: bool) -> Self {
        self.depth_test = depth_test;
        self
    }

    /// Check if command has expired
    pub fn is_expired(&self) -> bool {
        if let Some(lifetime) = self.lifetime {
            self.created_at.elapsed() > lifetime
        } else {
            false
        }
    }

    /// Get shape
    pub fn shape(&self) -> &DebugShape {
        &self.shape
    }

    /// Check if depth test is enabled
    pub fn depth_test(&self) -> bool {
        self.depth_test
    }
}

/// Debug draw system
pub struct DebugDraw {
    commands: VecDeque<DebugDrawCommand>,
    enabled: bool,
    max_commands: usize,
}

impl DebugDraw {
    /// Create a new debug draw system
    pub fn new() -> Self {
        Self {
            commands: VecDeque::new(),
            enabled: true,
            max_commands: 10000,
        }
    }

    /// Enable/disable debug drawing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear();
        }
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Add a debug draw command
    pub fn add(&mut self, command: DebugDrawCommand) {
        if !self.enabled {
            return;
        }

        if self.commands.len() >= self.max_commands {
            self.commands.pop_front();
        }

        self.commands.push_back(command);
    }

    /// Draw a line
    pub fn line(&mut self, start: Point3<f32>, end: Point3<f32>, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Line {
            start,
            end,
            color,
        }));
    }

    /// Draw a line with lifetime
    pub fn line_timed(
        &mut self,
        start: Point3<f32>,
        end: Point3<f32>,
        color: [f32; 4],
        duration: Duration,
    ) {
        self.add(
            DebugDrawCommand::new(DebugShape::Line { start, end, color }).with_lifetime(duration),
        );
    }

    /// Draw a ray
    pub fn ray(
        &mut self,
        origin: Point3<f32>,
        direction: Vector3<f32>,
        length: f32,
        color: [f32; 4],
    ) {
        self.add(DebugDrawCommand::new(DebugShape::Ray {
            origin,
            direction,
            length,
            color,
        }));
    }

    /// Draw an AABB
    pub fn aabb(&mut self, min: Point3<f32>, max: Point3<f32>, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::AABB { min, max, color }));
    }

    /// Draw a sphere
    pub fn sphere(&mut self, center: Point3<f32>, radius: f32, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Sphere {
            center,
            radius,
            color,
            segments: 16,
        }));
    }

    /// Draw a circle
    pub fn circle(&mut self, center: Point3<f32>, radius: f32, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Circle {
            center,
            radius,
            color,
            segments: 32,
        }));
    }

    /// Draw coordinate axes
    pub fn axes(&mut self, origin: Point3<f32>, size: f32) {
        self.add(DebugDrawCommand::new(DebugShape::Axes { origin, size }));
    }

    /// Draw a grid
    pub fn grid(&mut self, center: Point3<f32>, size: f32, divisions: u32, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Grid {
            center,
            size,
            divisions,
            color,
        }));
    }

    /// Draw text at 3D position
    pub fn text(&mut self, position: Point3<f32>, text: impl Into<String>, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Text {
            position,
            text: text.into(),
            color,
        }));
    }

    /// Draw an arrow
    pub fn arrow(&mut self, start: Point3<f32>, end: Point3<f32>, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Arrow {
            start,
            end,
            color,
            head_size: 0.2,
        }));
    }

    /// Draw a capsule
    pub fn capsule(&mut self, start: Point3<f32>, end: Point3<f32>, radius: f32, color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Capsule {
            start,
            end,
            radius,
            color,
        }));
    }

    /// Draw a cone
    pub fn cone(
        &mut self,
        tip: Point3<f32>,
        direction: Vector3<f32>,
        height: f32,
        radius: f32,
        color: [f32; 4],
    ) {
        self.add(DebugDrawCommand::new(DebugShape::Cone {
            tip,
            direction,
            height,
            radius,
            color,
        }));
    }

    /// Draw a frustum
    pub fn frustum(&mut self, corners: [Point3<f32>; 8], color: [f32; 4]) {
        self.add(DebugDrawCommand::new(DebugShape::Frustum {
            corners,
            color,
        }));
    }

    /// Draw OBB (oriented bounding box)
    pub fn obb(
        &mut self,
        center: Point3<f32>,
        half_extents: Vector3<f32>,
        rotation: Matrix4<f32>,
        color: [f32; 4],
    ) {
        self.add(DebugDrawCommand::new(DebugShape::OBB {
            center,
            half_extents,
            rotation,
            color,
        }));
    }

    /// Update and clean up expired commands
    pub fn update(&mut self) {
        if !self.enabled {
            return;
        }

        // Remove expired commands
        self.commands.retain(|cmd| !cmd.is_expired());
    }

    /// Get all active commands
    pub fn commands(&self) -> impl Iterator<Item = &DebugDrawCommand> {
        self.commands.iter()
    }

    /// Get number of active commands
    pub fn count(&self) -> usize {
        self.commands.len()
    }

    /// Clear all debug draw commands
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Set maximum number of commands
    pub fn set_max_commands(&mut self, max: usize) {
        self.max_commands = max;
    }
}

impl Default for DebugDraw {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper methods for common debug visualizations
impl DebugDraw {
    /// Draw a bounding box from center and size
    pub fn box_from_center(&mut self, center: Point3<f32>, size: Vector3<f32>, color: [f32; 4]) {
        let half = size * 0.5;
        let min = center - half;
        let max = center + half;
        self.aabb(min, max, color);
    }

    /// Draw a cross at position
    pub fn cross(&mut self, position: Point3<f32>, size: f32, color: [f32; 4]) {
        let half = size * 0.5;
        self.line(
            position - Vector3::new(half, 0.0, 0.0),
            position + Vector3::new(half, 0.0, 0.0),
            color,
        );
        self.line(
            position - Vector3::new(0.0, half, 0.0),
            position + Vector3::new(0.0, half, 0.0),
            color,
        );
        self.line(
            position - Vector3::new(0.0, 0.0, half),
            position + Vector3::new(0.0, 0.0, half),
            color,
        );
    }

    /// Draw a path (series of connected lines)
    pub fn path(&mut self, points: &[Point3<f32>], color: [f32; 4]) {
        for i in 0..points.len().saturating_sub(1) {
            self.line(points[i], points[i + 1], color);
        }
    }

    /// Draw a polygon
    pub fn polygon(&mut self, points: &[Point3<f32>], color: [f32; 4]) {
        for i in 0..points.len() {
            let next = (i + 1) % points.len();
            self.line(points[i], points[next], color);
        }
    }

    /// Draw a coordinate system with colored axes
    pub fn coordinate_system(&mut self, origin: Point3<f32>, size: f32) {
        // X axis (red)
        self.arrow(
            origin,
            origin + Vector3::new(size, 0.0, 0.0),
            [1.0, 0.0, 0.0, 1.0],
        );
        // Y axis (green)
        self.arrow(
            origin,
            origin + Vector3::new(0.0, size, 0.0),
            [0.0, 1.0, 0.0, 1.0],
        );
        // Z axis (blue)
        self.arrow(
            origin,
            origin + Vector3::new(0.0, 0.0, size),
            [0.0, 0.0, 1.0, 1.0],
        );
    }

    /// Draw normals for a set of points
    pub fn normals(
        &mut self,
        points: &[(Point3<f32>, Vector3<f32>)],
        length: f32,
        color: [f32; 4],
    ) {
        for (point, normal) in points {
            self.arrow(*point, *point + *normal * length, color);
        }
    }
}

/// Predefined colors for convenience
pub mod colors {
    pub const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
    pub const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
    pub const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
    pub const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
    pub const CYAN: [f32; 4] = [0.0, 1.0, 1.0, 1.0];
    pub const MAGENTA: [f32; 4] = [1.0, 0.0, 1.0, 1.0];
    pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    pub const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    pub const GRAY: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
    pub const ORANGE: [f32; 4] = [1.0, 0.5, 0.0, 1.0];
    pub const PURPLE: [f32; 4] = [0.5, 0.0, 1.0, 1.0];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_draw_creation() {
        let debug_draw = DebugDraw::new();
        assert!(debug_draw.is_enabled());
        assert_eq!(debug_draw.count(), 0);
    }

    #[test]
    fn test_add_commands() {
        let mut debug_draw = DebugDraw::new();

        debug_draw.line(Point3::origin(), Point3::new(1.0, 0.0, 0.0), colors::RED);
        debug_draw.sphere(Point3::origin(), 1.0, colors::GREEN);
        debug_draw.circle(Point3::origin(), 2.0, colors::BLUE);

        assert_eq!(debug_draw.count(), 3);
    }

    #[test]
    fn test_command_expiration() {
        let mut debug_draw = DebugDraw::new();

        debug_draw.line_timed(
            Point3::origin(),
            Point3::new(1.0, 0.0, 0.0),
            colors::RED,
            Duration::from_millis(1),
        );

        assert_eq!(debug_draw.count(), 1);

        std::thread::sleep(Duration::from_millis(10));
        debug_draw.update();

        assert_eq!(debug_draw.count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut debug_draw = DebugDraw::new();

        debug_draw.line(Point3::origin(), Point3::new(1.0, 0.0, 0.0), colors::RED);
        debug_draw.sphere(Point3::origin(), 1.0, colors::GREEN);

        assert_eq!(debug_draw.count(), 2);

        debug_draw.clear();
        assert_eq!(debug_draw.count(), 0);
    }

    #[test]
    fn test_enable_disable() {
        let mut debug_draw = DebugDraw::new();

        debug_draw.line(Point3::origin(), Point3::new(1.0, 0.0, 0.0), colors::RED);
        assert_eq!(debug_draw.count(), 1);

        debug_draw.set_enabled(false);
        assert!(!debug_draw.is_enabled());
        assert_eq!(debug_draw.count(), 0);

        debug_draw.line(Point3::origin(), Point3::new(1.0, 0.0, 0.0), colors::RED);
        assert_eq!(debug_draw.count(), 0);
    }

    #[test]
    fn test_max_commands() {
        let mut debug_draw = DebugDraw::new();
        debug_draw.set_max_commands(3);

        for i in 0..5 {
            debug_draw.line(
                Point3::origin(),
                Point3::new(i as f32, 0.0, 0.0),
                colors::RED,
            );
        }

        assert_eq!(debug_draw.count(), 3);
    }

    #[test]
    fn test_helper_methods() {
        let mut debug_draw = DebugDraw::new();

        debug_draw.cross(Point3::origin(), 1.0, colors::WHITE);
        debug_draw.coordinate_system(Point3::origin(), 2.0);
        debug_draw.box_from_center(Point3::origin(), Vector3::new(1.0, 1.0, 1.0), colors::BLUE);

        assert!(debug_draw.count() > 0);
    }
}
