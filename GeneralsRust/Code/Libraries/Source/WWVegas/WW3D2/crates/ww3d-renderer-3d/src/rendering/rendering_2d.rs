//! 2D Rendering System - Complete UI and 2D graphics rendering
//!
//! This module implements the Render2D system from the original C++ code,
//! providing comprehensive 2D rendering capabilities for UI, HUD, and 2D graphics.
//!
//! Converted from:
//! - render2d.cpp/h (main 2D rendering system)
//! - bmp2d.cpp/h (2D bitmap rendering)
//! - txt2d.cpp/h (2D text rendering)
//! - font3d.cpp/h (3D font system)
//! - textdraw.cpp/h (text drawing utilities)

use crate::core::error::{Result, W3dError};
use crate::core::wwstring::StringClass;
use crate::rendering::shader_core::ShaderClass;
use crate::rendering::texture_system::texture_base::TextureClass;
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// 2D rendering modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Render2DMode {
    /// Normal rendering
    Normal = 0,
    /// Additive blending
    Additive,
    /// Alpha blending
    Alpha,
    /// Screen blending
    Screen,
    /// Multiply blending
    Multiply,
}

/// 2D vertex structure
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Render2DVertex {
    /// Position (x, y, z)
    pub position: Vec3,
    /// Color (RGBA)
    pub color: Vec4,
    /// Texture coordinates (u, v)
    pub tex_coord: Vec2,
}

// SAFETY: Render2DVertex is repr(C) and contains only Pod types (Vec3, Vec4, Vec2)
unsafe impl bytemuck::Pod for Render2DVertex {}
unsafe impl bytemuck::Zeroable for Render2DVertex {}

impl Render2DVertex {
    /// Create new 2D vertex
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            tex_coord: Vec2::ZERO,
        }
    }

    /// Create vertex with position and color
    pub fn with_position_color(position: Vec2, color: Vec4) -> Self {
        Self {
            position: Vec3::new(position.x, position.y, 0.0),
            color,
            tex_coord: Vec2::ZERO,
        }
    }

    /// Create vertex with position, color and texture coordinates
    pub fn with_position_color_tex(position: Vec2, color: Vec4, tex_coord: Vec2) -> Self {
        Self {
            position: Vec3::new(position.x, position.y, 0.0),
            color,
            tex_coord,
        }
    }
}

/// 2D rectangle structure
#[derive(Debug, Clone, Copy)]
pub struct Rect2D {
    /// Left coordinate
    pub left: f32,
    /// Top coordinate
    pub top: f32,
    /// Right coordinate
    pub right: f32,
    /// Bottom coordinate
    pub bottom: f32,
}

impl Rect2D {
    /// Create new rectangle
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Get width
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Get height
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Get center point
    pub fn center(&self) -> Vec2 {
        Vec2::new(
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        )
    }

    /// Check if point is inside rectangle
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.left
            && point.x <= self.right
            && point.y >= self.top
            && point.y <= self.bottom
    }

    /// Create rectangle from center and size
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        let half_size = size / 2.0;
        Self::new(
            center.x - half_size.x,
            center.y - half_size.y,
            center.x + half_size.x,
            center.y + half_size.y,
        )
    }
}

/// 2D rendering state
#[derive(Debug, Clone)]
pub struct Render2DState {
    /// Current blend mode
    pub blend_mode: Render2DMode,
    /// Current texture
    pub texture: Option<Arc<TextureClass>>,
    /// Current shader
    pub shader: Option<Arc<ShaderClass>>,
    /// Current z-depth
    pub z_depth: f32,
    /// Current color multiplier
    pub color_multiplier: Vec4,
    /// Current transform matrix
    pub transform: Mat4,
}

impl Render2DState {
    /// Create default 2D state
    pub fn new() -> Self {
        Self {
            blend_mode: Render2DMode::Normal,
            texture: None,
            shader: None,
            z_depth: 0.0,
            color_multiplier: Vec4::new(1.0, 1.0, 1.0, 1.0),
            transform: Mat4::IDENTITY,
        }
    }
}

/// 2D bitmap class for rendering 2D images
#[derive(Debug)]
pub struct Bitmap2D {
    /// Texture to render
    pub texture: Arc<TextureClass>,
    /// Source rectangle (UV coordinates)
    pub source_rect: Rect2D,
    /// Color multiplier
    pub color: Vec4,
    /// Blend mode
    pub blend_mode: Render2DMode,
}

impl Bitmap2D {
    /// Create new bitmap
    pub fn new(texture: Arc<TextureClass>) -> Self {
        Self {
            texture,
            source_rect: Rect2D::new(0.0, 0.0, 1.0, 1.0),
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            blend_mode: Render2DMode::Normal,
        }
    }

    /// Set source rectangle in pixels
    pub fn set_source_rect_pixels(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        let width = self.texture.get_width() as f32;
        let height = self.texture.get_height() as f32;

        self.source_rect = Rect2D::new(left / width, top / height, right / width, bottom / height);
    }

    /// Set color
    pub fn set_color(&mut self, color: Vec4) {
        self.color = color;
    }

    /// Set blend mode
    pub fn set_blend_mode(&mut self, mode: Render2DMode) {
        self.blend_mode = mode;
    }
}

/// 2D text rendering class
#[derive(Debug)]
pub struct Text2D {
    /// Text content
    pub text: StringClass,
    /// Font texture
    pub font_texture: Arc<TextureClass>,
    /// Position
    pub position: Vec2,
    /// Color
    pub color: Vec4,
    /// Scale
    pub scale: Vec2,
    /// Character spacing
    pub char_spacing: f32,
    /// Line spacing
    pub line_spacing: f32,
    /// Blend mode
    pub blend_mode: Render2DMode,
    /// Font metrics
    pub font_metrics: FontMetrics,
}

impl Text2D {
    /// Create new 2D text
    pub fn new(font_texture: Arc<TextureClass>) -> Self {
        Self {
            text: StringClass::new(),
            font_texture,
            position: Vec2::ZERO,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            scale: Vec2::new(1.0, 1.0),
            char_spacing: 0.0,
            line_spacing: 0.0,
            blend_mode: Render2DMode::Alpha,
            font_metrics: FontMetrics::default(),
        }
    }

    /// Set text content
    pub fn set_text(&mut self, text: &str) {
        self.text = StringClass::from(text);
    }

    /// Set position
    pub fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }

    /// Set color
    pub fn set_color(&mut self, color: Vec4) {
        self.color = color;
    }

    /// Set scale
    pub fn set_scale(&mut self, scale: Vec2) {
        self.scale = scale;
    }

    /// Get text width
    pub fn get_width(&self) -> f32 {
        self.text.len() as f32 * self.font_metrics.char_width * self.scale.x
    }

    /// Get text height
    pub fn get_height(&self) -> f32 {
        self.font_metrics.char_height * self.scale.y
    }
}

/// Font metrics for text rendering
#[derive(Debug, Clone)]
pub struct FontMetrics {
    /// Character width
    pub char_width: f32,
    /// Character height
    pub char_height: f32,
    /// Characters per row in font texture
    pub chars_per_row: usize,
    /// First character ASCII code
    pub first_char: u8,
    /// Last character ASCII code
    pub last_char: u8,
}

impl FontMetrics {
    /// Create default font metrics
    pub fn default() -> Self {
        Self {
            char_width: 8.0,
            char_height: 12.0,
            chars_per_row: 16,
            first_char: 32, // Space
            last_char: 126, // ~
        }
    }

    /// Get UV coordinates for character
    pub fn get_char_uv(&self, char_code: u8) -> Rect2D {
        if char_code < self.first_char || char_code > self.last_char {
            return Rect2D::new(0.0, 0.0, 0.0, 0.0);
        }

        let char_index = (char_code - self.first_char) as usize;
        let row = char_index / self.chars_per_row;
        let col = char_index % self.chars_per_row;

        let uv_width = 1.0 / self.chars_per_row as f32;
        let uv_height =
            1.0 / ((self.last_char - self.first_char + 1) as usize / self.chars_per_row + 1) as f32;

        Rect2D::new(
            col as f32 * uv_width,
            row as f32 * uv_height,
            (col + 1) as f32 * uv_width,
            (row + 1) as f32 * uv_height,
        )
    }
}

/// Main 2D renderer class
#[derive(Debug)]
pub struct Render2DClass {
    /// Current rendering state
    pub state: Render2DState,
    /// Vertex buffer for 2D rendering
    pub vertices: Vec<Render2DVertex>,
    /// Index buffer for 2D rendering
    pub indices: Vec<u32>,
    /// WGPU vertex buffer
    pub vertex_buffer: Option<wgpu::Buffer>,
    /// WGPU index buffer
    pub index_buffer: Option<wgpu::Buffer>,
    /// Current z-depth counter
    pub z_counter: f32,
    /// Screen dimensions
    pub screen_width: f32,
    pub screen_height: f32,
    /// Renderer ID
    pub renderer_id: u32,
}

static RENDERER_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_renderer_id() -> u32 {
    RENDERER_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

impl Render2DClass {
    /// Create new 2D renderer
    pub fn new() -> Self {
        let renderer_id = next_renderer_id();

        Self {
            state: Render2DState::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
            z_counter: 0.0,
            screen_width: 1920.0,
            screen_height: 1080.0,
            renderer_id,
        }
    }

    /// Initialize renderer
    pub fn initialize(&mut self, device: &wgpu::Device) -> Result<()> {
        // Create vertex buffer
        self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("2D Vertex Buffer"),
            size: (std::mem::size_of::<Render2DVertex>() * 1024) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        // Create index buffer
        self.index_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("2D Index Buffer"),
            size: (std::mem::size_of::<u32>() * 1536) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        Ok(())
    }

    /// Set screen dimensions
    pub fn set_screen_dimensions(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Begin 2D rendering frame
    pub fn begin_frame(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.z_counter = 0.0;
        self.state = Render2DState::new();
    }

    /// End 2D rendering frame
    pub fn end_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        if self.vertices.is_empty() {
            return Ok(());
        }

        // Update vertex buffer
        if let Some(vertex_buffer) = &self.vertex_buffer {
            let vertex_data = bytemuck::cast_slice(&self.vertices);
            queue.write_buffer(vertex_buffer, 0, vertex_data);
        }

        // Update index buffer
        if let Some(index_buffer) = &self.index_buffer {
            let index_data = bytemuck::cast_slice(&self.indices);
            queue.write_buffer(index_buffer, 0, index_data);
        }

        Ok(())
    }

    /// Set blend mode
    pub fn set_blend_mode(&mut self, mode: Render2DMode) {
        self.state.blend_mode = mode;
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: Option<Arc<TextureClass>>) {
        self.state.texture = texture;
    }

    /// Set shader
    pub fn set_shader(&mut self, shader: Option<Arc<ShaderClass>>) {
        self.state.shader = shader;
    }

    /// Set color multiplier
    pub fn set_color_multiplier(&mut self, color: Vec4) {
        self.state.color_multiplier = color;
    }

    /// Set transform matrix
    pub fn set_transform(&mut self, transform: Mat4) {
        self.state.transform = transform;
    }

    /// Add quad to render list
    pub fn add_quad(&mut self, rect: Rect2D, color: Vec4, tex_coords: Option<Rect2D>) {
        let base_vertex = self.vertices.len() as u32;

        // Convert screen coordinates to clip space
        let clip_rect = self.screen_to_clip_rect(rect);

        let tex_coords = tex_coords.unwrap_or_else(|| Rect2D::new(0.0, 0.0, 1.0, 1.0));

        // Add vertices
        self.vertices.push(Render2DVertex::with_position_color_tex(
            Vec2::new(clip_rect.left, clip_rect.top),
            color,
            Vec2::new(tex_coords.left, tex_coords.top),
        ));

        self.vertices.push(Render2DVertex::with_position_color_tex(
            Vec2::new(clip_rect.right, clip_rect.top),
            color,
            Vec2::new(tex_coords.right, tex_coords.top),
        ));

        self.vertices.push(Render2DVertex::with_position_color_tex(
            Vec2::new(clip_rect.right, clip_rect.bottom),
            color,
            Vec2::new(tex_coords.right, tex_coords.bottom),
        ));

        self.vertices.push(Render2DVertex::with_position_color_tex(
            Vec2::new(clip_rect.left, clip_rect.bottom),
            color,
            Vec2::new(tex_coords.left, tex_coords.bottom),
        ));

        // Add indices
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        self.z_counter += 0.001;
    }

    /// Add triangle to render list
    pub fn add_triangle(&mut self, v1: Vec2, v2: Vec2, v3: Vec2, color: Vec4) {
        let base_vertex = self.vertices.len() as u32;

        // Convert to clip space
        let clip_v1 = self.screen_to_clip(v1);
        let clip_v2 = self.screen_to_clip(v2);
        let clip_v3 = self.screen_to_clip(v3);

        // Add vertices
        self.vertices
            .push(Render2DVertex::with_position_color(clip_v1, color));
        self.vertices
            .push(Render2DVertex::with_position_color(clip_v2, color));
        self.vertices
            .push(Render2DVertex::with_position_color(clip_v3, color));

        // Add indices
        self.indices
            .extend_from_slice(&[base_vertex, base_vertex + 1, base_vertex + 2]);

        self.z_counter += 0.001;
    }

    /// Add line to render list
    pub fn add_line(&mut self, start: Vec2, end: Vec2, color: Vec4, thickness: f32) {
        let direction = (end - start).normalize();
        let perpendicular = Vec2::new(-direction.y, direction.x) * thickness / 2.0;

        let v1 = start - perpendicular;
        let v2 = start + perpendicular;
        let v3 = end + perpendicular;
        let v4 = end - perpendicular;

        let base_vertex = self.vertices.len() as u32;

        // Convert to clip space
        let clip_v1 = self.screen_to_clip(v1);
        let clip_v2 = self.screen_to_clip(v2);
        let clip_v3 = self.screen_to_clip(v3);
        let clip_v4 = self.screen_to_clip(v4);

        // Add vertices
        self.vertices
            .push(Render2DVertex::with_position_color(clip_v1, color));
        self.vertices
            .push(Render2DVertex::with_position_color(clip_v2, color));
        self.vertices
            .push(Render2DVertex::with_position_color(clip_v3, color));
        self.vertices
            .push(Render2DVertex::with_position_color(clip_v4, color));

        // Add indices
        self.indices.extend_from_slice(&[
            base_vertex,
            base_vertex + 1,
            base_vertex + 2,
            base_vertex,
            base_vertex + 2,
            base_vertex + 3,
        ]);

        self.z_counter += 0.001;
    }

    /// Render bitmap
    pub fn render_bitmap(&mut self, bitmap: &Bitmap2D, dest_rect: Rect2D) {
        let old_blend_mode = self.state.blend_mode;
        let old_texture = self.state.texture.clone();

        self.set_blend_mode(bitmap.blend_mode);
        self.set_texture(Some(Arc::clone(&bitmap.texture)));

        let color = bitmap.color * self.state.color_multiplier;
        self.add_quad(dest_rect, color, Some(bitmap.source_rect));

        self.set_blend_mode(old_blend_mode);
        self.set_texture(old_texture);
    }

    /// Render text
    pub fn render_text(&mut self, text: &Text2D) {
        if text.text.is_empty() {
            return;
        }

        let old_blend_mode = self.state.blend_mode;
        let old_texture = self.state.texture.clone();

        self.set_blend_mode(text.blend_mode);
        self.set_texture(Some(Arc::clone(&text.font_texture)));

        let mut cursor_x = text.position.x;
        let mut cursor_y = text.position.y;

        for ch in text.text.as_str().chars() {
            if ch == '\n' {
                cursor_x = text.position.x;
                cursor_y += text.font_metrics.char_height * text.scale.y + text.line_spacing;
                continue;
            }

            let char_code = ch as u8;
            let uv_rect = text.font_metrics.get_char_uv(char_code);

            if uv_rect.width() > 0.0 {
                let char_width = text.font_metrics.char_width * text.scale.x;
                let char_height = text.font_metrics.char_height * text.scale.y;

                let dest_rect = Rect2D::new(
                    cursor_x,
                    cursor_y,
                    cursor_x + char_width,
                    cursor_y + char_height,
                );

                self.add_quad(dest_rect, text.color, Some(uv_rect));
            }

            cursor_x += text.font_metrics.char_width * text.scale.x + text.char_spacing;
        }

        self.set_blend_mode(old_blend_mode);
        self.set_texture(old_texture);
    }

    /// Render filled rectangle
    pub fn render_rect(&mut self, rect: Rect2D, color: Vec4) {
        let old_texture = self.state.texture.clone();
        self.set_texture(None);
        self.add_quad(rect, color, None);
        self.set_texture(old_texture);
    }

    /// Render outlined rectangle
    pub fn render_rect_outline(&mut self, rect: Rect2D, color: Vec4, thickness: f32) {
        let old_texture = self.state.texture.clone();
        self.set_texture(None);

        // Top line
        self.add_line(
            Vec2::new(rect.left, rect.top),
            Vec2::new(rect.right, rect.top),
            color,
            thickness,
        );

        // Right line
        self.add_line(
            Vec2::new(rect.right, rect.top),
            Vec2::new(rect.right, rect.bottom),
            color,
            thickness,
        );

        // Bottom line
        self.add_line(
            Vec2::new(rect.right, rect.bottom),
            Vec2::new(rect.left, rect.bottom),
            color,
            thickness,
        );

        // Left line
        self.add_line(
            Vec2::new(rect.left, rect.bottom),
            Vec2::new(rect.left, rect.top),
            color,
            thickness,
        );

        self.set_texture(old_texture);
    }

    /// Convert screen coordinates to clip space
    fn screen_to_clip(&self, screen_pos: Vec2) -> Vec2 {
        Vec2::new(
            (screen_pos.x / self.screen_width) * 2.0 - 1.0,
            1.0 - (screen_pos.y / self.screen_height) * 2.0,
        )
    }

    /// Convert screen rectangle to clip space
    fn screen_to_clip_rect(&self, screen_rect: Rect2D) -> Rect2D {
        Rect2D::new(
            (screen_rect.left / self.screen_width) * 2.0 - 1.0,
            1.0 - (screen_rect.top / self.screen_height) * 2.0,
            (screen_rect.right / self.screen_width) * 2.0 - 1.0,
            1.0 - (screen_rect.bottom / self.screen_height) * 2.0,
        )
    }

    /// Get vertex buffer layout for WGPU
    pub fn get_vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Render2DVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Texture coordinates
                wgpu::VertexAttribute {
                    offset: 28,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }

    /// Get vertex count
    pub fn get_vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get index count
    pub fn get_index_count(&self) -> usize {
        self.indices.len()
    }

    /// Clear render lists
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Reset renderer state
    pub fn reset_state(&mut self) {
        self.state = Render2DState::new();
    }
}

fn renderer_2d_slot() -> &'static Mutex<Option<Render2DClass>> {
    static SLOT: OnceLock<Mutex<Option<Render2DClass>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_renderer_2d_slot() -> MutexGuard<'static, Option<Render2DClass>> {
    match renderer_2d_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Scoped handle to the shared Render2D instance.
pub struct Render2DHandle<'a> {
    guard: MutexGuard<'a, Option<Render2DClass>>,
}

impl<'a> Deref for Render2DHandle<'a> {
    type Target = Render2DClass;

    fn deref(&self) -> &Self::Target {
        self.guard
            .as_ref()
            .expect("2D renderer must be initialized before use")
    }
}

impl<'a> DerefMut for Render2DHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .as_mut()
            .expect("2D renderer must be initialized before use")
    }
}

/// Initialize global 2D renderer
pub fn init_renderer_2d(device: &wgpu::Device) -> Result<()> {
    let mut renderer = Render2DClass::new();
    renderer.initialize(device)?;

    let mut guard = lock_renderer_2d_slot();
    *guard = Some(renderer);
    Ok(())
}

/// Get global 2D renderer
pub fn get_renderer_2d() -> Option<Render2DHandle<'static>> {
    let guard = lock_renderer_2d_slot();
    if guard.is_none() {
        None
    } else {
        Some(Render2DHandle { guard })
    }
}

/// Shutdown global 2D renderer
pub fn shutdown_renderer_2d() {
    let mut guard = lock_renderer_2d_slot();
    *guard = None;
}

/// Quick 2D rendering functions
pub fn begin_2d_frame() {
    if let Some(mut renderer) = get_renderer_2d() {
        renderer.begin_frame();
    }
}

pub fn end_2d_frame(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
    if let Some(mut renderer) = get_renderer_2d() {
        renderer.end_frame(device, queue)
    } else {
        Ok(())
    }
}

pub fn render_2d_quad(rect: Rect2D, color: Vec4) {
    if let Some(mut renderer) = get_renderer_2d() {
        renderer.add_quad(rect, color, None);
    }
}

pub fn render_2d_bitmap(bitmap: &Bitmap2D, dest_rect: Rect2D) {
    if let Some(mut renderer) = get_renderer_2d() {
        renderer.render_bitmap(bitmap, dest_rect);
    }
}

pub fn render_2d_text(text: &Text2D) {
    if let Some(mut renderer) = get_renderer_2d() {
        renderer.render_text(text);
    }
}

pub fn render_2d_line(start: Vec2, end: Vec2, color: Vec4, thickness: f32) {
    if let Some(mut renderer) = get_renderer_2d() {
        renderer.add_line(start, end, color, thickness);
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_rect_2d() {
        let rect = Rect2D::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(rect.width(), 20.0);
        assert_eq!(rect.height(), 20.0);
        assert_eq!(rect.center(), Vec2::new(20.0, 30.0));
        assert!(rect.contains(Vec2::new(20.0, 30.0)));
        assert!(!rect.contains(Vec2::new(5.0, 30.0)));
    }

    #[test]
    fn test_render_2d_vertex() {
        let vertex = Render2DVertex::new();
        assert_eq!(vertex.position, Vec3::ZERO);
        assert_eq!(vertex.color, Vec4::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(vertex.tex_coord, Vec2::ZERO);

        let vertex2 = Render2DVertex::with_position_color(
            Vec2::new(10.0, 20.0),
            Vec4::new(1.0, 0.0, 0.0, 1.0),
        );
        assert_eq!(vertex2.position, Vec3::new(10.0, 20.0, 0.0));
        assert_eq!(vertex2.color, Vec4::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_font_metrics() {
        let metrics = FontMetrics::default();
        assert_eq!(metrics.char_width, 8.0);
        assert_eq!(metrics.char_height, 12.0);
        assert_eq!(metrics.chars_per_row, 16);

        // Test 'A' character (ASCII 65)
        let uv = metrics.get_char_uv(b'A');
        assert!(uv.left >= 0.0 && uv.left <= 1.0);
        assert!(uv.right > uv.left);
    }

    #[test]
    fn test_render_2d_class() {
        let mut renderer = Render2DClass::new();
        assert_eq!(renderer.get_vertex_count(), 0);
        assert_eq!(renderer.get_index_count(), 0);

        renderer.begin_frame();
        renderer.add_quad(
            Rect2D::new(0.0, 0.0, 100.0, 100.0),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            None,
        );

        assert_eq!(renderer.get_vertex_count(), 4);
        assert_eq!(renderer.get_index_count(), 6);

        renderer.clear();
        assert_eq!(renderer.get_vertex_count(), 0);
        assert_eq!(renderer.get_index_count(), 0);
    }

    #[test]
    fn test_bitmap_2d() {
        let texture = Arc::new(crate::texture_system::TextureClass::new(
            "render2d_texture",
            1,
            1,
        ));
        let mut bitmap = Bitmap2D::new(Arc::clone(&texture));

        assert_eq!(bitmap.color, Vec4::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(bitmap.blend_mode, Render2DMode::Normal);

        bitmap.set_color(Vec4::new(0.5, 0.5, 0.5, 1.0));
        bitmap.set_blend_mode(Render2DMode::Additive);

        assert_eq!(bitmap.color, Vec4::new(0.5, 0.5, 0.5, 1.0));
        assert_eq!(bitmap.blend_mode, Render2DMode::Additive);
    }

    #[test]
    fn test_text_2d() {
        let font_texture = Arc::new(crate::texture_system::TextureClass::new(
            "font_texture",
            1,
            1,
        ));
        let mut text = Text2D::new(Arc::clone(&font_texture));

        text.set_text("Hello World");
        text.set_position(Vec2::new(10.0, 20.0));
        text.set_color(Vec4::new(1.0, 1.0, 0.0, 1.0));
        text.set_scale(Vec2::new(2.0, 2.0));

        assert_eq!(text.text.as_str(), "Hello World");
        assert_eq!(text.position, Vec2::new(10.0, 20.0));
        assert_eq!(text.color, Vec4::new(1.0, 1.0, 0.0, 1.0));
        assert_eq!(text.scale, Vec2::new(2.0, 2.0));

        // Test dimensions (rough approximation)
        assert!(text.get_width() > 0.0);
        assert!(text.get_height() > 0.0);
    }
}
