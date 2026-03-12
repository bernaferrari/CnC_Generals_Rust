//! Render2D system for 2D graphics and UI rendering
//!
//! This module provides comprehensive 2D rendering capabilities including:
//! - Quad rendering with textures and colors
//! - Gradient support (vertical and horizontal)
//! - Line and outline drawing
//! - Rectangle drawing with borders
//! - Coordinate system management
//! - Font rendering support

use crate::rendering::shader_system::ShaderClass;
use crate::texture_system::TextureClass;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3, Vec4};
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub mod bmp2d;
pub mod font3d;
mod gpu_context;
pub mod render2dsentence;

pub use bmp2d::*;
pub use font3d::*;
pub use gpu_context::{is_textured, screen_to_clip, Render2DGpuContext};
pub use render2dsentence::Render2DSentenceClass;

/// Color conversion macros (equivalent to C++ macros)
#[inline]
pub fn vrgb_to_u32(rgb: Vec3) -> u32 {
    ((rgb.x * 255.0) as u32) << 16
        | ((rgb.y * 255.0) as u32) << 8
        | ((rgb.z * 255.0) as u32)
        | 0xFF000000
}

#[inline]
pub fn vrgba_to_u32(rgba: Vec4) -> u32 {
    ((rgba.w * 255.0) as u32) << 24
        | ((rgba.x * 255.0) as u32) << 16
        | ((rgba.y * 255.0) as u32) << 8
        | ((rgba.z * 255.0) as u32)
}

#[inline]
pub fn rgb_to_u32(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | (b as u32) | 0xFF000000
}

#[inline]
pub fn rgba_to_u32(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | (b as u32)
}

#[inline]
pub fn frgb_to_u32(r: f32, g: f32, b: f32) -> u32 {
    ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | ((b * 255.0) as u32) | 0xFF000000
}

#[inline]
pub fn frgba_to_u32(r: f32, g: f32, b: f32, a: f32) -> u32 {
    ((a * 255.0) as u32) << 24
        | ((r * 255.0) as u32) << 16
        | ((g * 255.0) as u32) << 8
        | ((b * 255.0) as u32)
}

/// Rectangle structure for 2D coordinates
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

/// 2D Vertex structure for rendering
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex2D {
    pub position: Vec2,
    pub uv: Vec2,
    pub color: u32,
}

impl Vertex2D {
    pub fn new(position: Vec2, uv: Vec2, color: u32) -> Self {
        Self {
            position,
            uv,
            color,
        }
    }
}

/// Main Render2D class for 2D graphics rendering
pub struct Render2D {
    pub vertices: Vec<Vertex2D>,
    pub indices: Vec<u16>,
    pub texture: Option<TextureClass>,
    pub shader: ShaderClass,
    pub coordinate_range: Rect,
    pub coordinate_scale: Vec2,
    pub coordinate_offset: Vec2,
    pub z_value: f32,
    pub is_hidden: bool,
    pub is_grayscale: bool,
    pub force_alpha: Option<f32>,
    pub force_color: Option<u32>,
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    scratch_colors: Vec<u32>,
    temp_bind_groups: Vec<wgpu::BindGroup>,
    temp_pipelines: Vec<Arc<wgpu::RenderPipeline>>,
}

impl Render2D {
    /// Create a new Render2D instance
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
            shader: ShaderClass::new(),
            coordinate_range: Rect::new(0.0, 0.0, 800.0, 600.0), // Default screen size
            coordinate_scale: Vec2::new(1.0, 1.0),
            coordinate_offset: Vec2::new(0.0, 0.0),
            z_value: 0.0,
            is_hidden: false,
            is_grayscale: false,
            force_alpha: None,
            force_color: None,
            vertex_buffer: None,
            index_buffer: None,
            scratch_colors: Vec::new(),
            temp_bind_groups: Vec::new(),
            temp_pipelines: Vec::new(),
        }
    }

    /// Create Render2D with a texture
    pub fn with_texture(texture: TextureClass) -> Self {
        let mut render2d = Self::new();
        render2d.texture = Some(texture);
        render2d
    }

    /// Reset the render state
    pub fn reset(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.vertex_buffer = None;
        self.index_buffer = None;
        self.scratch_colors.clear();
        self.temp_bind_groups.clear();
    }

    /// Set coordinate range for screen mapping
    pub fn set_coordinate_range(&mut self, range: Rect) {
        self.coordinate_range = range;
        self.update_coordinate_transform();
    }

    /// Update coordinate transformation
    fn update_coordinate_transform(&mut self) {
        let width = self.coordinate_range.width();
        let height = self.coordinate_range.height();

        if width > 0.0 && height > 0.0 {
            self.coordinate_scale = Vec2::new(2.0 / width, -2.0 / height);
            self.coordinate_offset = Vec2::new(
                -1.0 - self.coordinate_scale.x * self.coordinate_range.left,
                1.0 - self.coordinate_scale.y * self.coordinate_range.top,
            );
        }
    }

    /// Convert screen coordinates to normalized device coordinates
    fn convert_vertex(&self, vertex: Vec2) -> Vec2 {
        screen_to_clip(vertex, self.coordinate_scale, self.coordinate_offset)
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: TextureClass) {
        self.texture = Some(texture);
    }

    /// Enable/disable additive blending
    pub fn enable_additive(&mut self, enabled: bool) {
        if enabled {
            self.shader
                .set_src_blend_func(crate::rendering::shader_system::SrcBlendFuncType::One);
            self.shader
                .set_dst_blend_func(crate::rendering::shader_system::DstBlendFuncType::One);
        } else {
            self.shader
                .set_src_blend_func(crate::rendering::shader_system::SrcBlendFuncType::SrcAlpha);
            self.shader
                .set_dst_blend_func(crate::rendering::shader_system::DstBlendFuncType::InvSrcAlpha);
        }
    }

    /// Enable/disable alpha blending
    pub fn enable_alpha(&mut self, enabled: bool) {
        if enabled {
            self.shader
                .set_src_blend_func(crate::rendering::shader_system::SrcBlendFuncType::SrcAlpha);
            self.shader
                .set_dst_blend_func(crate::rendering::shader_system::DstBlendFuncType::InvSrcAlpha);
        } else {
            self.shader
                .set_src_blend_func(crate::rendering::shader_system::SrcBlendFuncType::One);
            self.shader
                .set_dst_blend_func(crate::rendering::shader_system::DstBlendFuncType::Zero);
        }
    }

    /// Enable/disable grayscale rendering
    pub fn enable_grayscale(&mut self, enabled: bool) {
        self.is_grayscale = enabled;
    }

    /// Enable/disable texturing
    pub fn enable_texturing(&mut self, enabled: bool) {
        self.shader.set_texturing(if enabled {
            crate::rendering::shader_system::TexturingType::Enable
        } else {
            crate::rendering::shader_system::TexturingType::Disable
        });
    }

    /// Set Z value for depth tricks
    pub fn set_z_value(&mut self, z: f32) {
        self.z_value = z;
    }

    /// Move all vertices by an offset
    pub fn move_vertices(&mut self, offset: Vec2) {
        for vertex in &mut self.vertices {
            vertex.position += offset;
        }
        self.vertex_buffer = None; // Invalidate buffers
    }

    /// Force all vertices to use a specific alpha value
    pub fn force_alpha(&mut self, alpha: f32) {
        self.force_alpha = Some(alpha);
        self.update_vertex_colors();
    }

    /// Force all vertices to use a specific color
    pub fn force_color(&mut self, color: u32) {
        self.force_color = Some(color);
        self.update_vertex_colors();
    }

    /// Update vertex colors based on force settings
    fn update_vertex_colors(&mut self) {
        for vertex in &mut self.vertices {
            let mut color = vertex.color;

            if let Some(force_color) = self.force_color {
                color = force_color;
            }

            if let Some(alpha) = self.force_alpha {
                // Extract RGB and replace alpha
                let r = (color >> 16) & 0xFF;
                let g = (color >> 8) & 0xFF;
                let b = color & 0xFF;
                let a = (alpha * 255.0) as u32;
                color = (a << 24) | (r << 16) | (g << 8) | b;
            }

            vertex.color = color;
        }
    }

    /// Add a quad with texture coordinates
    pub fn add_quad(&mut self, v0: Vec2, v1: Vec2, v2: Vec2, v3: Vec2, uv: Rect, color: u32) {
        let base_index = self.vertices.len() as u16;

        // Add vertices
        self.vertices
            .push(Vertex2D::new(v0, Vec2::new(uv.left, uv.top), color));
        self.vertices
            .push(Vertex2D::new(v1, Vec2::new(uv.right, uv.top), color));
        self.vertices
            .push(Vertex2D::new(v2, Vec2::new(uv.right, uv.bottom), color));
        self.vertices
            .push(Vertex2D::new(v3, Vec2::new(uv.left, uv.bottom), color));

        // Add indices for two triangles
        self.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);

        self.vertex_buffer = None; // Invalidate buffers
    }

    /// Add a quad from a rectangle
    pub fn add_quad_rect(&mut self, screen: Rect, uv: Rect, color: u32) {
        let v0 = Vec2::new(screen.left, screen.top);
        let v1 = Vec2::new(screen.right, screen.top);
        let v2 = Vec2::new(screen.right, screen.bottom);
        let v3 = Vec2::new(screen.left, screen.bottom);
        self.add_quad(v0, v1, v2, v3, uv, color);
    }

    /// Add a quad with solid color (no texture)
    pub fn add_quad_solid(&mut self, screen: Rect, color: u32) {
        let uv = Rect::new(0.0, 0.0, 1.0, 1.0);
        self.add_quad_rect(screen, uv, color);
    }

    /// Add a vertical gradient quad
    pub fn add_quad_v_gradient(&mut self, screen: Rect, top_color: u32, bottom_color: u32) {
        let base_index = self.vertices.len() as u16;

        let v0 = Vec2::new(screen.left, screen.top);
        let v1 = Vec2::new(screen.right, screen.top);
        let v2 = Vec2::new(screen.right, screen.bottom);
        let v3 = Vec2::new(screen.left, screen.bottom);

        // Add vertices with gradient colors
        self.vertices
            .push(Vertex2D::new(v0, Vec2::new(0.0, 0.0), top_color));
        self.vertices
            .push(Vertex2D::new(v1, Vec2::new(1.0, 0.0), top_color));
        self.vertices
            .push(Vertex2D::new(v2, Vec2::new(1.0, 1.0), bottom_color));
        self.vertices
            .push(Vertex2D::new(v3, Vec2::new(0.0, 1.0), bottom_color));

        // Add indices
        self.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);

        self.vertex_buffer = None;
    }

    /// Add a horizontal gradient quad
    pub fn add_quad_h_gradient(&mut self, screen: Rect, left_color: u32, right_color: u32) {
        let base_index = self.vertices.len() as u16;

        let v0 = Vec2::new(screen.left, screen.top);
        let v1 = Vec2::new(screen.right, screen.top);
        let v2 = Vec2::new(screen.right, screen.bottom);
        let v3 = Vec2::new(screen.left, screen.bottom);

        // Add vertices with gradient colors
        self.vertices
            .push(Vertex2D::new(v0, Vec2::new(0.0, 0.0), left_color));
        self.vertices
            .push(Vertex2D::new(v1, Vec2::new(1.0, 1.0), right_color));
        self.vertices
            .push(Vertex2D::new(v2, Vec2::new(1.0, 0.0), right_color));
        self.vertices
            .push(Vertex2D::new(v3, Vec2::new(0.0, 1.0), left_color));

        // Add indices
        self.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);

        self.vertex_buffer = None;
    }

    /// Add a triangle
    pub fn add_triangle(
        &mut self,
        v0: Vec2,
        v1: Vec2,
        v2: Vec2,
        uv0: Vec2,
        uv1: Vec2,
        uv2: Vec2,
        color: u32,
    ) {
        let base_index = self.vertices.len() as u16;

        self.vertices.push(Vertex2D::new(v0, uv0, color));
        self.vertices.push(Vertex2D::new(v1, uv1, color));
        self.vertices.push(Vertex2D::new(v2, uv2, color));

        self.indices
            .extend_from_slice(&[base_index, base_index + 1, base_index + 2]);

        self.vertex_buffer = None;
    }

    /// Add a line
    pub fn add_line(&mut self, start: Vec2, end: Vec2, width: f32, color: u32) {
        // Simple line implementation - expand to quad
        let direction = (end - start).normalize();
        let perpendicular = Vec2::new(-direction.y, direction.x) * width * 0.5;

        let v0 = start - perpendicular;
        let v1 = start + perpendicular;
        let v2 = end + perpendicular;
        let v3 = end - perpendicular;

        let uv = Rect::new(0.0, 0.0, 1.0, 1.0);
        self.add_quad(v0, v1, v2, v3, uv, color);
    }

    /// Add a rectangle outline
    pub fn add_outline(&mut self, rect: Rect, width: f32, color: u32) {
        // Top line
        self.add_line(
            Vec2::new(rect.left, rect.top),
            Vec2::new(rect.right, rect.top),
            width,
            color,
        );
        // Bottom line
        self.add_line(
            Vec2::new(rect.left, rect.bottom),
            Vec2::new(rect.right, rect.bottom),
            width,
            color,
        );
        // Left line
        self.add_line(
            Vec2::new(rect.left, rect.top),
            Vec2::new(rect.left, rect.bottom),
            width,
            color,
        );
        // Right line
        self.add_line(
            Vec2::new(rect.right, rect.top),
            Vec2::new(rect.right, rect.bottom),
            width,
            color,
        );
    }

    /// Add a filled rectangle with border
    pub fn add_rect(&mut self, rect: Rect, border_width: f32, border_color: u32, fill_color: u32) {
        // Fill
        self.add_quad_solid(rect, fill_color);

        // Border (inset from the fill)
        let inset_rect = Rect::new(
            rect.left + border_width,
            rect.top + border_width,
            rect.right - border_width,
            rect.bottom - border_width,
        );

        if inset_rect.width() > 0.0 && inset_rect.height() > 0.0 {
            self.add_outline(inset_rect, border_width, border_color);
        }
    }

    /// Create WGPU buffers for rendering
    pub fn create_wgpu_buffers(&mut self, device: &wgpu::Device) {
        if self.vertices.is_empty() {
            return;
        }

        // Convert vertices to NDC coordinates
        let ndc_vertices: Vec<Vertex2D> = self
            .vertices
            .iter()
            .map(|v| {
                let ndc_pos = self.convert_vertex(v.position);
                Vertex2D::new(ndc_pos, v.uv, v.color)
            })
            .collect();

        // Create vertex buffer
        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render2D Vertex Buffer"),
                contents: bytemuck::cast_slice(&ndc_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );

        // Create index buffer
        self.index_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render2D Index Buffer"),
                contents: bytemuck::cast_slice(&self.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );
    }

    /// Render the 2D geometry
    pub fn render<'pass>(
        &'pass mut self,
        gpu: &mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        if self.is_hidden || self.vertices.is_empty() {
            return;
        }

        self.temp_bind_groups.clear();
        self.temp_pipelines.clear();

        // Create buffers if needed
        if self.vertex_buffer.is_none() {
            self.create_wgpu_buffers(gpu.device());
        }

        if let (Some(vertex_buffer), Some(index_buffer)) = (&self.vertex_buffer, &self.index_buffer)
        {
            let textured = is_textured(&self.shader, self.texture.is_some());
            if textured {
                let bind_group = gpu.create_texture_bind_group(self.texture.as_mut());
                self.temp_bind_groups.push(bind_group);
                let bind_ref = self
                    .temp_bind_groups
                    .last()
                    .expect("temp bind group just inserted");

                let pipeline = gpu.pipeline_for(
                    textured,
                    self.is_grayscale,
                    self.shader.get_src_blend_func(),
                    self.shader.get_dst_blend_func(),
                );
                self.temp_pipelines.push(pipeline);
                let pipeline_ref = self
                    .temp_pipelines
                    .last()
                    .expect("pipeline stored for 2D render");
                render_pass.set_pipeline(pipeline_ref.as_ref());
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.set_bind_group(0, bind_ref, &[]);
                render_pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);
            } else {
                let pipeline = gpu.pipeline_for(
                    textured,
                    self.is_grayscale,
                    self.shader.get_src_blend_func(),
                    self.shader.get_dst_blend_func(),
                );
                self.temp_pipelines.push(pipeline);
                let pipeline_ref = self
                    .temp_pipelines
                    .last()
                    .expect("pipeline stored for 2D render");
                render_pass.set_pipeline(pipeline_ref.as_ref());
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);
            }
        }
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get index count
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    /// Get color array for external modification
    pub fn color_array(&mut self) -> &mut Vec<u32> {
        self.scratch_colors.clear();
        self.scratch_colors
            .extend(self.vertices.iter().map(|vertex| vertex.color));
        &mut self.scratch_colors
    }
}

/// Screen resolution management
pub struct ScreenResolution;

impl ScreenResolution {
    pub fn set_resolution(_rect: Rect) {
        // Store screen resolution for coordinate conversion
        // This would be stored in a global or static variable
    }

    pub fn get_resolution() -> Rect {
        // Return current screen resolution
        Rect::new(0.0, 0.0, 800.0, 600.0) // Default
    }
}
