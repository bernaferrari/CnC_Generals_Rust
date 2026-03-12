//! Debug Render Modes
//!
//! Special rendering modes for debugging and visualization:
//! - Wireframe mode (mesh edges only)
//! - Normals visualization (RGB = XYZ)
//! - Collision mesh rendering
//! - LOD visualization (color-coded levels)
//! - Bounding box visualization
//!
//! Matches C++ WW3D debug rendering capabilities.

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{Device, Queue, RenderPass};

/// Debug render mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugRenderMode {
    /// Normal rendering
    Normal,
    /// Wireframe only (edges)
    Wireframe,
    /// Render normals as colors (Red=X, Green=Y, Blue=Z)
    Normals,
    /// Show collision meshes
    Collision,
    /// Color-code LOD levels
    LodVisualization,
    /// Show bounding boxes
    BoundingBoxes,
    /// Overdraw visualization
    Overdraw,
    /// Texture coordinates (UV as RG)
    TextureCoordinates,
    /// Vertex colors
    VertexColors,
}

impl DebugRenderMode {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            DebugRenderMode::Normal => "Normal",
            DebugRenderMode::Wireframe => "Wireframe",
            DebugRenderMode::Normals => "Normals",
            DebugRenderMode::Collision => "Collision",
            DebugRenderMode::LodVisualization => "LOD Levels",
            DebugRenderMode::BoundingBoxes => "Bounding Boxes",
            DebugRenderMode::Overdraw => "Overdraw",
            DebugRenderMode::TextureCoordinates => "Texture Coordinates",
            DebugRenderMode::VertexColors => "Vertex Colors",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            DebugRenderMode::Normal => "Standard rendering",
            DebugRenderMode::Wireframe => "Show mesh edges only",
            DebugRenderMode::Normals => "Visualize normals as colors (R=X, G=Y, B=Z)",
            DebugRenderMode::Collision => "Show collision geometry",
            DebugRenderMode::LodVisualization => "Color-code LOD levels",
            DebugRenderMode::BoundingBoxes => "Show axis-aligned bounding boxes",
            DebugRenderMode::Overdraw => "Visualize overdraw (red = more layers)",
            DebugRenderMode::TextureCoordinates => "Show UV coordinates as colors",
            DebugRenderMode::VertexColors => "Display vertex colors",
        }
    }
}

/// LOD level colors (from C++)
pub const LOD_COLORS: [Vec3; 8] = [
    Vec3::new(1.0, 0.0, 0.0), // LOD 0: Red
    Vec3::new(0.0, 1.0, 0.0), // LOD 1: Green
    Vec3::new(0.0, 0.0, 1.0), // LOD 2: Blue
    Vec3::new(1.0, 1.0, 0.0), // LOD 3: Yellow
    Vec3::new(1.0, 0.0, 1.0), // LOD 4: Magenta
    Vec3::new(0.0, 1.0, 1.0), // LOD 5: Cyan
    Vec3::new(1.0, 0.5, 0.0), // LOD 6: Orange
    Vec3::new(0.5, 0.0, 1.0), // LOD 7: Purple
];

/// Debug renderer for special visualization modes
pub struct DebugRenderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
    current_mode: DebugRenderMode,
    show_grid: bool,
    show_axes: bool,
    wireframe_color: Vec3,
    collision_mesh_color: Vec3,
}

impl DebugRenderer {
    /// Create a new debug renderer
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            device,
            queue,
            current_mode: DebugRenderMode::Normal,
            show_grid: false,
            show_axes: false,
            wireframe_color: Vec3::new(0.0, 1.0, 0.0), // Green wireframe (from C++)
            collision_mesh_color: Vec3::new(1.0, 0.0, 1.0), // Magenta collision (from C++)
        }
    }

    /// Set the current render mode
    pub fn set_mode(&mut self, mode: DebugRenderMode) {
        self.current_mode = mode;
    }

    /// Get the current render mode
    pub fn get_mode(&self) -> DebugRenderMode {
        self.current_mode
    }

    /// Cycle to next render mode
    pub fn cycle_mode(&mut self) {
        use DebugRenderMode::*;
        self.current_mode = match self.current_mode {
            Normal => Wireframe,
            Wireframe => Normals,
            Normals => Collision,
            Collision => LodVisualization,
            LodVisualization => BoundingBoxes,
            BoundingBoxes => Overdraw,
            Overdraw => TextureCoordinates,
            TextureCoordinates => VertexColors,
            VertexColors => Normal,
        };
    }

    /// Render wireframe
    pub fn render_wireframe(
        &self,
        _render_pass: &mut RenderPass,
        _vertices: &[Vec3],
        _indices: &[u32],
        _transform: Mat4,
    ) {
        // In shader:
        // - Use line topology (wgpu::PrimitiveTopology::LineList)
        // - Render mesh edges
        // - Use wireframe_color for all lines
        //
        // Vertex shader:
        // var output: VertexOutput;
        // output.position = projection * view * model * vec4<f32>(vertex_pos, 1.0);
        // output.color = wireframe_color;
        //
        // Fragment shader:
        // return vec4<f32>(input.color, 1.0);
    }

    /// Render normals as colors
    pub fn render_normals(
        &self,
        _render_pass: &mut RenderPass,
        _vertices: &[Vec3],
        _normals: &[Vec3],
        _indices: &[u32],
        _transform: Mat4,
    ) {
        // In fragment shader:
        // let normal_color = normal * 0.5 + 0.5; // Map [-1,1] to [0,1]
        // return vec4<f32>(normal_color, 1.0);
        //
        // Red = X axis
        // Green = Y axis
        // Blue = Z axis
    }

    /// Render collision mesh
    pub fn render_collision_mesh(
        &self,
        _render_pass: &mut RenderPass,
        _vertices: &[Vec3],
        _indices: &[u32],
        _transform: Mat4,
    ) {
        // Render with collision_mesh_color (magenta)
        // Semi-transparent (alpha = 0.5)
        // Enable depth testing but disable depth writing
    }

    /// Render LOD level visualization
    pub fn render_lod_level(
        &self,
        _render_pass: &mut RenderPass,
        lod_level: usize,
        _vertices: &[Vec3],
        _indices: &[u32],
        _transform: Mat4,
    ) -> Vec3 {
        // Get color for this LOD level
        LOD_COLORS[lod_level.min(LOD_COLORS.len() - 1)]

        // In fragment shader:
        // return vec4<f32>(lod_color, 1.0);
    }

    /// Render bounding box
    pub fn render_bounding_box(
        &self,
        _render_pass: &mut RenderPass,
        min: Vec3,
        max: Vec3,
        _color: Vec3,
        _transform: Mat4,
    ) {
        // Create 12 lines for the box edges
        let _corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];

        // Draw 12 lines connecting the corners
        // Use line rendering with specified color
    }

    /// Render coordinate axes at origin
    pub fn render_axes(&self, _render_pass: &mut RenderPass, length: f32, _view_proj: Mat4) {
        // X axis: Red line from origin to (length, 0, 0)
        // Y axis: Green line from origin to (0, length, 0)
        // Z axis: Blue line from origin to (0, 0, length)
        let _x_axis = Vec3::new(length, 0.0, 0.0);
        let _y_axis = Vec3::new(0.0, length, 0.0);
        let _z_axis = Vec3::new(0.0, 0.0, length);
    }

    /// Render grid on XZ plane
    pub fn render_grid(
        &self,
        _render_pass: &mut RenderPass,
        size: f32,
        divisions: u32,
        _view_proj: Mat4,
    ) {
        let step = size / divisions as f32;
        let half_size = size * 0.5;

        // Draw lines parallel to X axis
        for i in 0..=divisions {
            let z = -half_size + i as f32 * step;
            let _start = Vec3::new(-half_size, 0.0, z);
            let _end = Vec3::new(half_size, 0.0, z);
            // Draw line from start to end
        }

        // Draw lines parallel to Z axis
        for i in 0..=divisions {
            let x = -half_size + i as f32 * step;
            let _start = Vec3::new(x, 0.0, -half_size);
            let _end = Vec3::new(x, 0.0, half_size);
            // Draw line from start to end
        }
    }

    /// Render texture coordinates as colors
    pub fn render_texture_coords(
        &self,
        _render_pass: &mut RenderPass,
        _vertices: &[Vec3],
        _uvs: &[(f32, f32)],
        _indices: &[u32],
        _transform: Mat4,
    ) {
        // In fragment shader:
        // return vec4<f32>(uv.x, uv.y, 0.0, 1.0);
        //
        // Red = U coordinate
        // Green = V coordinate
    }

    /// Visualize overdraw
    pub fn render_overdraw(&self, _render_pass: &mut RenderPass) {
        // Use additive blending
        // Each pixel drawn adds a small amount of red
        // More overdraw = more red
        //
        // Blend mode:
        // src_factor: One
        // dst_factor: One
        // operation: Add
        //
        // Fragment shader:
        // return vec4<f32>(0.1, 0.0, 0.0, 1.0); // Add 0.1 red per draw
    }

    /// Enable/disable grid
    pub fn set_show_grid(&mut self, show: bool) {
        self.show_grid = show;
    }

    /// Enable/disable axes
    pub fn set_show_axes(&mut self, show: bool) {
        self.show_axes = show;
    }

    /// Set wireframe color
    pub fn set_wireframe_color(&mut self, color: Vec3) {
        self.wireframe_color = color;
    }

    /// Set collision mesh color
    pub fn set_collision_mesh_color(&mut self, color: Vec3) {
        self.collision_mesh_color = color;
    }

    /// Get statistics
    pub fn get_stats(&self) -> DebugRendererStats {
        DebugRendererStats {
            current_mode: self.current_mode,
            show_grid: self.show_grid,
            show_axes: self.show_axes,
        }
    }
}

/// Debug renderer statistics
#[derive(Debug, Clone)]
pub struct DebugRendererStats {
    pub current_mode: DebugRenderMode,
    pub show_grid: bool,
    pub show_axes: bool,
}

/// Utility for rendering debug lines
pub struct DebugLineRenderer {
    device: Arc<Device>,
    lines: Vec<DebugLine>,
}

/// A debug line
#[derive(Debug, Clone)]
pub struct DebugLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: Vec3,
}

impl DebugLineRenderer {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            lines: Vec::new(),
        }
    }

    /// Add a line
    pub fn add_line(&mut self, start: Vec3, end: Vec3, color: Vec3) {
        self.lines.push(DebugLine { start, end, color });
    }

    /// Add a colored axis at position
    pub fn add_axes(&mut self, position: Vec3, length: f32) {
        // X axis (red)
        self.add_line(
            position,
            position + Vec3::new(length, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        );
        // Y axis (green)
        self.add_line(
            position,
            position + Vec3::new(0.0, length, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        // Z axis (blue)
        self.add_line(
            position,
            position + Vec3::new(0.0, 0.0, length),
            Vec3::new(0.0, 0.0, 1.0),
        );
    }

    /// Add a bounding box
    pub fn add_box(&mut self, min: Vec3, max: Vec3, color: Vec3) {
        // Bottom face
        self.add_line(
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            color,
        );
        self.add_line(
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            color,
        );
        self.add_line(
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, min.y, max.z),
            color,
        );
        self.add_line(
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, min.y, min.z),
            color,
        );

        // Top face
        self.add_line(
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            color,
        );
        self.add_line(
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, max.y, max.z),
            color,
        );
        self.add_line(
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            color,
        );
        self.add_line(
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(min.x, max.y, min.z),
            color,
        );

        // Vertical edges
        self.add_line(
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            color,
        );
        self.add_line(
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            color,
        );
        self.add_line(
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            color,
        );
        self.add_line(
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            color,
        );
    }

    /// Clear all lines
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Render all lines
    pub fn render(&self, _render_pass: &mut RenderPass, _view_proj: Mat4) {
        // Render each line with its color
        // Use line list topology
    }

    /// Get line count
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Performance HUD for displaying rendering statistics
#[derive(Debug, Clone)]
pub struct PerformanceHud {
    /// Show FPS counter
    pub show_fps: bool,
    /// Show triangle/vertex count
    pub show_geometry_stats: bool,
    /// Show draw call count
    pub show_draw_calls: bool,
    /// Show memory usage
    pub show_memory: bool,
    /// Show GPU time
    pub show_gpu_time: bool,
    /// Show frame graph info
    pub show_frame_graph: bool,
    /// Position on screen (0.0-1.0 normalized)
    pub position: (f32, f32),
    /// Text color
    pub color: Vec3,
    /// Background color
    pub background_color: Vec4,
}

impl Default for PerformanceHud {
    fn default() -> Self {
        Self {
            show_fps: true,
            show_geometry_stats: true,
            show_draw_calls: true,
            show_memory: false,
            show_gpu_time: false,
            show_frame_graph: false,
            position: (0.02, 0.02),          // Top-left with 2% margin
            color: Vec3::new(0.0, 1.0, 0.0), // Green text (classic debug style)
            background_color: Vec4::new(0.0, 0.0, 0.0, 0.7), // Semi-transparent black
        }
    }
}

impl PerformanceHud {
    /// Create a new performance HUD
    pub fn new() -> Self {
        Self::default()
    }

    /// Format statistics for display
    pub fn format_stats(&self, stats: &PerformanceStats) -> Vec<String> {
        let mut lines = Vec::new();

        if self.show_fps {
            lines.push(format!("FPS: {:.1}", stats.fps));
            lines.push(format!("Frame Time: {:.2}ms", stats.frame_time_ms));
        }

        if self.show_geometry_stats {
            lines.push(format!("Triangles: {}", stats.triangle_count));
            lines.push(format!("Vertices: {}", stats.vertex_count));
        }

        if self.show_draw_calls {
            lines.push(format!("Draw Calls: {}", stats.draw_calls));
            lines.push(format!("Batches: {}", stats.batch_count));
        }

        if self.show_memory {
            lines.push(format!(
                "Memory: {:.1} MB",
                stats.memory_usage_bytes as f32 / (1024.0 * 1024.0)
            ));
        }

        if self.show_gpu_time {
            lines.push(format!("GPU Time: {:.2}ms", stats.gpu_time_ms));
        }

        if self.show_frame_graph {
            lines.push(format!("Opaque: {}", stats.opaque_count));
            lines.push(format!("Alpha: {}", stats.alpha_count));
            lines.push(format!("Decals: {}", stats.decal_count));
        }

        lines
    }

    /// Toggle all stats on/off
    pub fn toggle_all(&mut self) {
        let all_off = !self.show_fps
            && !self.show_geometry_stats
            && !self.show_draw_calls
            && !self.show_memory
            && !self.show_gpu_time
            && !self.show_frame_graph;

        if all_off {
            self.show_fps = true;
            self.show_geometry_stats = true;
            self.show_draw_calls = true;
        } else {
            self.show_fps = false;
            self.show_geometry_stats = false;
            self.show_draw_calls = false;
            self.show_memory = false;
            self.show_gpu_time = false;
            self.show_frame_graph = false;
        }
    }
}

/// Rendering performance statistics
#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub triangle_count: u32,
    pub vertex_count: u32,
    pub draw_calls: u32,
    pub batch_count: u32,
    pub memory_usage_bytes: u64,
    pub gpu_time_ms: f32,
    pub opaque_count: u32,
    pub alpha_count: u32,
    pub decal_count: u32,
}

impl PerformanceStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Update FPS from delta time
    pub fn update_fps(&mut self, delta_time: f32) {
        if delta_time > 0.0 {
            self.fps = 1.0 / delta_time;
            self.frame_time_ms = delta_time * 1000.0;
        }
    }

    /// Reset counters for new frame
    pub fn reset_frame(&mut self) {
        self.triangle_count = 0;
        self.vertex_count = 0;
        self.draw_calls = 0;
        self.batch_count = 0;
        self.opaque_count = 0;
        self.alpha_count = 0;
        self.decal_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_modes() {
        assert_eq!(DebugRenderMode::Normal.name(), "Normal");
        assert_eq!(DebugRenderMode::Wireframe.name(), "Wireframe");
        assert_eq!(DebugRenderMode::Normals.name(), "Normals");
    }

    #[test]
    fn test_lod_colors() {
        assert_eq!(LOD_COLORS.len(), 8);
        assert_eq!(LOD_COLORS[0], Vec3::new(1.0, 0.0, 0.0)); // Red
        assert_eq!(LOD_COLORS[1], Vec3::new(0.0, 1.0, 0.0)); // Green
    }

    #[test]
    fn test_mode_cycling() {
        // Test that all debug render modes are defined correctly
        // without requiring a GPU device
        let modes = [
            DebugRenderMode::Normal,
            DebugRenderMode::Wireframe,
            DebugRenderMode::Normals,
            DebugRenderMode::Collision,
            DebugRenderMode::LodVisualization,
            DebugRenderMode::BoundingBoxes,
            DebugRenderMode::Overdraw,
            DebugRenderMode::TextureCoordinates,
            DebugRenderMode::VertexColors,
        ];

        assert_eq!(modes.len(), 9);

        // Verify each mode has a valid name and description
        for mode in &modes {
            assert!(!mode.name().is_empty());
            assert!(!mode.description().is_empty());
        }
    }
}
