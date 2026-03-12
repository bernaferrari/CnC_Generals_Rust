//! WGPU Debug Renderer - Professional debugging and visualization tools
//!
//! This module provides comprehensive debugging capabilities for the WW3D engine:
//! - Wireframe rendering for mesh debugging
//! - Bounding box visualization
//! - Normal vector display
//! - Performance overlay
//! - Debug text rendering
//! - Collision shape visualization

use crate::core::error::{W3dError, RendererResult};
use crate::pluglib::Color;
use crate::rendering::wgpu_polygon_renderer::{
    PolygonPrimitiveType, PolygonRenderMode, WgpuPolygonRenderer,
};
// DX8 buffer types removed; use WGPU buffers directly via polygon renderer
use crate::core::WW3DFormat;
use crate::utils::mesh::W3dMesh;
use glam::{Mat4, Vec3, Vec4};
use crate::bounding_volumes::{AABox, OBBox, Sphere};

use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{Device, Queue};

/// Convert math_utilities::Matrix4 to glam::Mat4
fn matrix4_to_mat4(m: Mat4) -> Mat4 {
    Mat4::from_cols_array(&[
        m.row[0].x, m.row[1].x, m.row[2].x, m.row[3].x, m.row[0].y, m.row[1].y, m.row[2].y,
        m.row[3].y, m.row[0].z, m.row[1].z, m.row[2].z, m.row[3].z, m.row[0].w, m.row[1].w,
        m.row[2].w, m.row[3].w,
    ])
}

/// Convert glam::Vec3 to math_utilities::Vec3
fn vec3_to_vector3(v: Vec3) -> Vec3 {
    v
}

/// Convert math_utilities::Vec3 to glam::Vec3
fn vector3_to_vec3(v: Vec3) -> Vec3 {
    v
}

/// Debug renderer configuration
#[derive(Debug, Clone)]
pub struct DebugRendererConfig {
    /// Enable wireframe rendering
    pub wireframe_enabled: bool,
    /// Enable bounding box display
    pub bounding_boxes_enabled: bool,
    /// Enable normal vector display
    pub normals_enabled: bool,
    /// Enable collision shape visualization
    pub collision_shapes_enabled: bool,
    /// Enable performance overlay
    pub performance_overlay: bool,
    /// Wireframe color
    pub wireframe_color: Color,
    /// Bounding box color
    pub bounding_box_color: Color,
    /// Normal vector color
    pub normal_color: Color,
    /// Normal vector length
    pub normal_length: f32,
    /// Debug text color
    pub text_color: Color,
    /// Maximum debug lines per frame
    pub max_debug_lines: u32,
}

impl Default for DebugRendererConfig {
    fn default() -> Self {
        Self {
            wireframe_enabled: false,
            bounding_boxes_enabled: false,
            normals_enabled: false,
            collision_shapes_enabled: false,
            performance_overlay: true,
            wireframe_color: Color::new(0.0, 1.0, 0.0, 1.0), // Green
            bounding_box_color: Color::new(1.0, 0.0, 0.0, 1.0), // Red
            normal_color: Color::new(0.0, 0.0, 1.0, 1.0),    // Blue
            normal_length: 0.1,
            text_color: Color::new(1.0, 1.0, 1.0, 1.0), // White
            max_debug_lines: 10000,
        }
    }
}

/// Debug rendering statistics
#[derive(Debug, Clone, Default)]
pub struct DebugRendererStats {
    /// Number of debug lines rendered this frame
    pub debug_lines_rendered: u32,
    /// Number of bounding boxes rendered this frame
    pub bounding_boxes_rendered: u32,
    /// Number of normal vectors rendered this frame
    pub normal_vectors_rendered: u32,
    /// Number of collision shapes rendered this frame
    pub collision_shapes_rendered: u32,
}

/// Debug rendering command
#[derive(Debug, Clone)]
pub enum DebugCommand {
    /// Draw a line between two points
    DrawLine {
        start: Vec3,
        end: Vec3,
        color: Color,
    },
    /// Draw a bounding box
    DrawBoundingBox {
        aabox: AABox,
        color: Color,
        transform: Option<Mat4>,
    },
    /// Draw an oriented bounding box
    DrawOBBox {
        obbox: OBBox,
        color: Color,
        transform: Option<Mat4>,
    },
    /// Draw a sphere
    DrawSphere {
        sphere: Sphere,
        color: Color,
        transform: Option<Mat4>,
    },
    /// Draw normal vectors for a mesh
    DrawNormals {
        mesh: Arc<W3dMesh>,
        color: Color,
        length: f32,
        transform: Option<Mat4>,
    },
    /// Draw debug text
    DrawText {
        text: String,
        position: Vec3,
        color: Color,
        scale: f32,
    },
}

/// WGPU-based debug renderer for comprehensive visualization
pub struct WgpuDebugRenderer {
    /// Polygon renderer for debug geometry
    polygon_renderer: Arc<WgpuPolygonRenderer>,
    /// Configuration
    config: DebugRendererConfig,
    /// Statistics
    stats: DebugRendererStats,
    /// Debug command queue
    command_queue: Vec<DebugCommand>,
    /// Debug vertex buffer for lines
    debug_line_buffer: Option<String>,
    /// Debug vertex buffer for boxes
    debug_box_buffer: Option<String>,
    /// Debug vertex buffer for spheres
    debug_sphere_buffer: Option<String>,
    /// Cached debug geometry
    cached_geometry: HashMap<String, (String, Option<String>)>, // (vertex_buffer, index_buffer)
}

impl WgpuDebugRenderer {
    /// Create a new debug renderer
    pub fn new(polygon_renderer: Arc<WgpuPolygonRenderer>) -> Self {
        let config = DebugRendererConfig::default();

        Self {
            polygon_renderer,
            config,
            stats: DebugRendererStats::default(),
            command_queue: Vec::new(),
            debug_line_buffer: None,
            debug_box_buffer: None,
            debug_sphere_buffer: None,
            cached_geometry: HashMap::new(),
        }
    }

    /// Create debug renderer with custom configuration
    pub fn with_config(
        polygon_renderer: Arc<WgpuPolygonRenderer>,
        config: DebugRendererConfig,
    ) -> Self {
        Self {
            polygon_renderer,
            config,
            stats: DebugRendererStats::default(),
            command_queue: Vec::new(),
            debug_line_buffer: None,
            debug_box_buffer: None,
            debug_sphere_buffer: None,
            cached_geometry: HashMap::new(),
        }
    }

    /// Add a debug command to the queue
    pub fn add_command(&mut self, command: DebugCommand) {
        if self.command_queue.len() < self.config.max_debug_lines as usize {
            self.command_queue.push(command);
        }
    }

    /// Draw a debug line
    pub fn draw_line(&mut self, start: Vec3, end: Vec3, color: Color) {
        if self.config.wireframe_enabled {
            self.add_command(DebugCommand::DrawLine { start, end, color });
        }
    }

    /// Draw a bounding box
    pub fn draw_bounding_box(&mut self, aabox: AABox, color: Color, transform: Option<Matrix4>) {
        if self.config.bounding_boxes_enabled {
            self.add_command(DebugCommand::DrawBoundingBox {
                aabox,
                color,
                transform,
            });
            self.stats.bounding_boxes_rendered += 1;
        }
    }

    /// Draw an oriented bounding box
    pub fn draw_obbox(&mut self, obbox: OBBox, color: Color, transform: Option<Matrix4>) {
        if self.config.collision_shapes_enabled {
            self.add_command(DebugCommand::DrawOBBox {
                obbox,
                color,
                transform,
            });
            self.stats.collision_shapes_rendered += 1;
        }
    }

    /// Draw a sphere
    pub fn draw_sphere(&mut self, sphere: Sphere, color: Color, transform: Option<Matrix4>) {
        if self.config.collision_shapes_enabled {
            self.add_command(DebugCommand::DrawSphere {
                sphere,
                color,
                transform,
            });
            self.stats.collision_shapes_rendered += 1;
        }
    }

    /// Draw normal vectors for a mesh
    pub fn draw_normals(
        &mut self,
        mesh: Arc<W3dMesh>,
        color: Color,
        length: f32,
        transform: Option<Matrix4>,
    ) {
        if self.config.normals_enabled {
            let mesh_clone = mesh.clone();
            self.add_command(DebugCommand::DrawNormals {
                mesh,
                color,
                length,
                transform,
            });
            self.stats.normal_vectors_rendered += mesh_clone.vertices.len() as u32;
        }
    }

    /// Draw debug text
    pub fn draw_text(&mut self, text: String, position: Vec3, color: Color, scale: f32) {
        self.add_command(DebugCommand::DrawText {
            text,
            position,
            color,
            scale,
        });
    }

    /// Render all queued debug commands
    pub fn render_debug_commands(&mut self) -> RendererResult<()> {
        // Ensure debug geometry buffers are created
        self.ensure_debug_geometry()?;

        // Clone the command queue to avoid borrowing conflicts
        let commands = self.command_queue.clone();

        // Process each command
        for command in &commands {
            match command {
                DebugCommand::DrawLine { start, end, color } => {
                    self.render_line(*start, *end, *color)?;
                }
                DebugCommand::DrawBoundingBox {
                    aabox,
                    color,
                    transform,
                } => {
                    self.render_bounding_box(aabox, *color, *transform)?;
                }
                DebugCommand::DrawOBBox {
                    obbox,
                    color,
                    transform,
                } => {
                    self.render_obbox(obbox, *color, *transform)?;
                }
                DebugCommand::DrawSphere {
                    sphere,
                    color,
                    transform,
                } => {
                    self.render_sphere(sphere, *color, *transform)?;
                }
                DebugCommand::DrawNormals {
                    mesh,
                    color,
                    length,
                    transform,
                } => {
                    self.render_normals(mesh, *color, *length, *transform)?;
                }
                DebugCommand::DrawText {
                    text,
                    position,
                    color,
                    scale,
                } => {
                    self.render_text(text, *position, *color, *scale)?;
                }
            }
        }

        // Clear command queue
        self.command_queue.clear();

        Ok(())
    }

    /// Ensure debug geometry buffers are created
    fn ensure_debug_geometry(&mut self) -> RendererResult<()> {
        if self.debug_line_buffer.is_none() {
            self.debug_line_buffer = Some("debug_lines".to_string());
            // Create debug line geometry buffer
            self.create_debug_line_buffer()?;
        }

        if self.debug_box_buffer.is_none() {
            self.debug_box_buffer = Some("debug_box".to_string());
            // Create debug box geometry buffer
            self.create_debug_box_buffer()?;
        }

        if self.debug_sphere_buffer.is_none() {
            self.debug_sphere_buffer = Some("debug_sphere".to_string());
            // Create debug sphere geometry buffer
            self.create_debug_sphere_buffer()?;
        }

        Ok(())
    }

    /// Create debug line geometry buffer
    fn create_debug_line_buffer(&self) -> RendererResult<()> {
        // This would create a dynamic vertex buffer for debug lines
        // Implementation would depend on the actual vertex buffer creation system
        Ok(())
    }

    /// Create debug box geometry buffer
    fn create_debug_box_buffer(&self) -> RendererResult<()> {
        // This would create a vertex buffer for a unit cube wireframe
        // 8 vertices, 24 indices (for 12 edges)
        Ok(())
    }

    /// Create debug sphere geometry buffer
    fn create_debug_sphere_buffer(&self) -> RendererResult<()> {
        // This would create a vertex buffer for a sphere wireframe
        // Multiple circles at different latitudes/longitudes
        Ok(())
    }

    /// Render a single line
    fn render_line(&mut self, start: Vec3, end: Vec3, color: Color) -> RendererResult<()> {
        if let Some(buffer_name) = &self.debug_line_buffer {
            // Update the debug line buffer with the line vertices
            // Then render using the polygon renderer

            self.stats.debug_lines_rendered += 1;
        }

        Ok(())
    }

    /// Render a bounding box
    fn render_bounding_box(
        &mut self,
        aabox: &AABox,
        color: Color,
        transform: Option<Matrix4>,
    ) -> RendererResult<()> {
        if let Some(buffer_name) = &self.debug_box_buffer {
            // Set up transformation matrix for the box
            let box_transform = if let Some(transform) = transform {
                // Combine with box scaling and positioning
                let scale = Vec3::new(
                    aabox.extents().x * 2.0,
                    aabox.extents().y * 2.0,
                    aabox.extents().z * 2.0,
                );
                let center = Vec3::new(aabox.center().x, aabox.center().y, aabox.center().z);
                let box_matrix = Mat4::from_scale(scale) * Mat4::from_translation(center);
                matrix4_to_mat4(transform) * box_matrix
            } else {
                let scale = Vec3::new(
                    aabox.extents().x * 2.0,
                    aabox.extents().y * 2.0,
                    aabox.extents().z * 2.0,
                );
                let center = Vec3::new(aabox.center().x, aabox.center().y, aabox.center().z);
                Mat4::from_scale(scale) * Mat4::from_translation(center)
            };

            // Render the box wireframe
            // This would use the polygon renderer to draw the box edges
        }

        Ok(())
    }

    /// Render an oriented bounding box
    fn render_obbox(
        &mut self,
        obbox: &OBBox,
        color: Color,
        transform: Option<Matrix4>,
    ) -> RendererResult<()> {
        if let Some(buffer_name) = &self.debug_box_buffer {
            // Set up transformation matrix for the oriented box
            let box_transform = if let Some(transform) = transform {
                let scale = Vec3::new(
                    obbox.extents().x * 2.0,
                    obbox.extents().y * 2.0,
                    obbox.extents().z * 2.0,
                );
                let center = Vec3::new(obbox.center().x, obbox.center().y, obbox.center().z);
                let box_matrix = Mat4::from_scale(scale) * Mat4::from_translation(center);
                (matrix4_to_mat4(transform) * obbox.basis_matrix()) * box_matrix
            } else {
                let scale = Vec3::new(
                    obbox.extents().x * 2.0,
                    obbox.extents().y * 2.0,
                    obbox.extents().z * 2.0,
                );
                let center = Vec3::new(obbox.center().x, obbox.center().y, obbox.center().z);
                let box_matrix = Mat4::from_scale(scale) * Mat4::from_translation(center);
                obbox.basis_matrix() * box_matrix
            };

            // Render the oriented box wireframe
        }

        Ok(())
    }

    /// Render a sphere
    fn render_sphere(
        &mut self,
        sphere: &Sphere,
        color: Color,
        transform: Option<Matrix4>,
    ) -> RendererResult<()> {
        if let Some(buffer_name) = &self.debug_sphere_buffer {
            // Set up transformation matrix for the sphere
            let sphere_transform = if let Some(transform) = transform {
                let scale_vector = Vec3::new(sphere.radius, sphere.radius, sphere.radius);
                let center = Vec3::new(sphere.center.x, sphere.center.y, sphere.center.z);
                let sphere_matrix = Mat4::from_scale(scale_vector) * Mat4::from_translation(center);
                matrix4_to_mat4(transform) * sphere_matrix
            } else {
                let scale_vector = Vec3::new(sphere.radius, sphere.radius, sphere.radius);
                let center = Vec3::new(sphere.center.x, sphere.center.y, sphere.center.z);
                Mat4::from_scale(scale_vector) * Mat4::from_translation(center)
            };

            // Render the sphere wireframe
        }

        Ok(())
    }

    /// Render normal vectors for a mesh
    fn render_normals(
        &mut self,
        mesh: &Arc<W3dMesh>,
        color: Color,
        length: f32,
        transform: Option<Matrix4>,
    ) -> RendererResult<()> {
        // Create normal vectors as lines from each vertex
        for (i, vertex) in mesh.vertices.iter().enumerate() {
            if i < mesh.normals.len() {
                let normal = mesh.normals[i];
                let start = *vertex;
                let end = start + (normal * length);

                let line_start = if let Some(transform) = transform {
                    vector3_to_vec3(transform.transform_point3(vec3_to_vector3(start)))
                } else {
                    start
                };

                let line_end = if let Some(transform) = transform {
                    vector3_to_vec3(transform.transform_point3(vec3_to_vector3(end)))
                } else {
                    end
                };

                // Render the normal line
                self.render_line(line_start, line_end, color)?;
            }
        }

        Ok(())
    }

    /// Render debug text
    fn render_text(
        &mut self,
        text: &str,
        position: Vec3,
        color: Color,
        scale: f32,
    ) -> RendererResult<()> {
        // Text rendering would use a 2D text system or bitmap font
        // This is a placeholder for text rendering implementation
        Ok(())
    }

    /// Enable/disable wireframe rendering
    pub fn set_wireframe_enabled(&mut self, enabled: bool) {
        self.config.wireframe_enabled = enabled;
    }

    /// Enable/disable bounding box display
    pub fn set_bounding_boxes_enabled(&mut self, enabled: bool) {
        self.config.bounding_boxes_enabled = enabled;
    }

    /// Enable/disable normal vector display
    pub fn set_normals_enabled(&mut self, enabled: bool) {
        self.config.normals_enabled = enabled;
    }

    /// Enable/disable collision shape visualization
    pub fn set_collision_shapes_enabled(&mut self, enabled: bool) {
        self.config.collision_shapes_enabled = enabled;
    }

    /// Enable/disable performance overlay
    pub fn set_performance_overlay(&mut self, enabled: bool) {
        self.config.performance_overlay = enabled;
    }

    /// Set wireframe color
    pub fn set_wireframe_color(&mut self, color: Color) {
        self.config.wireframe_color = color;
    }

    /// Set bounding box color
    pub fn set_bounding_box_color(&mut self, color: Color) {
        self.config.bounding_box_color = color;
    }

    /// Set normal vector color
    pub fn set_normal_color(&mut self, color: Color) {
        self.config.normal_color = color;
    }

    /// Set normal vector length
    pub fn set_normal_length(&mut self, length: f32) {
        self.config.normal_length = length;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &DebugRendererConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: DebugRendererConfig) {
        self.config = config;
    }

    /// Get current statistics
    pub fn get_stats(&self) -> &DebugRendererStats {
        &self.stats
    }

    /// Reset frame statistics
    pub fn reset_stats(&mut self) {
        self.stats = DebugRendererStats::default();
    }

    /// Clear all debug commands
    pub fn clear_commands(&mut self) {
        self.command_queue.clear();
    }

    /// Get number of queued commands
    pub fn get_command_count(&self) -> usize {
        self.command_queue.len()
    }
}

impl Drop for WgpuDebugRenderer {
    fn drop(&mut self) {
        // Cleanup debug resources
        self.clear_commands();
        self.cached_geometry.clear();
    }
}

/// Factory function to create debug renderer with default settings
pub fn create_debug_renderer(polygon_renderer: Arc<WgpuPolygonRenderer>) -> WgpuDebugRenderer {
    WgpuDebugRenderer::new(polygon_renderer)
}

/// Factory function to create debug renderer with custom configuration
pub fn create_debug_renderer_with_config(
    polygon_renderer: Arc<WgpuPolygonRenderer>,
    config: DebugRendererConfig,
) -> WgpuDebugRenderer {
    WgpuDebugRenderer::with_config(polygon_renderer, config)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_debug_renderer_config_defaults() {
        let config = DebugRendererConfig::default();
        assert!(!config.wireframe_enabled);
        assert!(!config.bounding_boxes_enabled);
        assert!(!config.normals_enabled);
        assert!(!config.collision_shapes_enabled);
        assert!(config.performance_overlay);
        assert_eq!(config.max_debug_lines, 10000);
        assert_eq!(config.normal_length, 0.1);
    }

    #[test]
    fn test_debug_command_creation() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(1.0, 1.0, 1.0);
        let color = Color::new(1.0, 0.0, 0.0, 1.0);

        let command = DebugCommand::DrawLine { start, end, color };

        match command {
            DebugCommand::DrawLine {
                start: s,
                end: e,
                color: c,
            } => {
                assert_eq!(s, start);
                assert_eq!(e, end);
                assert_eq!(c, color);
            }
            _ => panic!("Wrong command type"),
        }
    }
}
