//! Bitmap2D class for 2D bitmap rendering
//!
//! Provides a modern Rust adaptation of the legacy Bitmap2DObjClass.

use crate::rendering::render2d::{Rect, Render2D, Render2DGpuContext};
use crate::texture_system::{SurfaceClass, TextureClass};
use glam::Vec2;

/// Immediate-mode textured quad.
pub struct Bitmap2DObj {
    render2d: Render2D,
    texture: Option<TextureClass>,
    position: Vec2,
    scale: Vec2,
    color: u32,
    center: bool,
    additive: bool,
    visible: bool,
}

impl Bitmap2DObj {
    /// Construct a bitmap using an existing texture.
    pub fn from_texture(
        texture: TextureClass,
        norm_x: f32,
        norm_y: f32,
        center: bool,
        additive: bool,
    ) -> Self {
        let mut obj = Self::new(norm_x, norm_y, center, additive);
        obj.set_texture(texture);
        obj
    }

    fn new(norm_x: f32, norm_y: f32, center: bool, additive: bool) -> Self {
        let mut render2d = Render2D::new();
        render2d.enable_additive(additive);
        Self {
            render2d,
            texture: None,
            position: Vec2::ZERO,
            scale: Vec2::new(norm_x, norm_y),
            color: 0xFFFFFFFF,
            center,
            additive,
            visible: true,
        }
    }

    /// Attach a new texture and rebuild the quad.
    pub fn set_texture(&mut self, texture: TextureClass) {
        self.texture = Some(texture.clone());
        self.render2d.set_texture(texture);
        self.update_geometry();
    }

    /// Populate the bitmap using CPU surface data (useful for UI atlases).
    pub fn set_surface(&mut self, name: &str, surface: &SurfaceClass) {
        if let Ok(texture) = TextureClass::from_surface(name, surface) {
            self.set_texture(texture);
        }
    }

    /// Access the current texture.
    pub fn texture(&self) -> Option<&TextureClass> {
        self.texture.as_ref()
    }

    /// Set the logical size (relative to the texture dimensions).
    pub fn set_scale(&mut self, width: f32, height: f32) {
        self.scale = Vec2::new(width, height);
        self.update_geometry();
    }

    /// Set the top-left position of the quad.
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = Vec2::new(x, y);
        self.update_geometry();
    }

    /// Toggle centre anchored positioning.
    pub fn set_centered(&mut self, center: bool) {
        if self.center != center {
            self.center = center;
            self.update_geometry();
        }
    }

    /// Assign a vertex colour multiplier.
    pub fn set_color(&mut self, color: u32) {
        self.color = color;
        self.update_geometry();
    }

    /// Toggle additive blending.
    pub fn set_additive(&mut self, additive: bool) {
        if self.additive != additive {
            self.additive = additive;
            self.render2d.enable_additive(additive);
        }
    }

    /// Visibility flag.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn update_geometry(&mut self) {
        self.render2d.reset();
        let texture = match self.texture.as_ref() {
            Some(texture) => texture,
            None => return,
        };

        let width = texture.width as f32 * self.scale.x;
        let height = texture.height as f32 * self.scale.y;

        let mut left = self.position.x;
        let mut top = self.position.y;
        let mut right = left + width;
        let mut bottom = top + height;

        if self.center {
            let half_w = width * 0.5;
            let half_h = height * 0.5;
            left -= half_w;
            right -= half_w;
            top -= half_h;
            bottom -= half_h;
        }

        let screen_rect = Rect::new(left, top, right, bottom);
        let uv_rect = Rect::new(0.0, 0.0, 1.0, 1.0);
        self.render2d
            .add_quad_rect(screen_rect, uv_rect, self.color);
    }

    /// Render the quad.
    pub fn render<'pass>(
        &'pass mut self,
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        if !self.visible {
            return;
        }

        if let Some(texture) = self.texture.clone() {
            self.render2d.set_texture(texture);
        }

        self.render2d.render(gpu, render_pass);
    }

    /// Create a duplicate object that shares the texture.
    pub fn duplicate(&self) -> Self {
        let mut clone = Self::new(self.scale.x, self.scale.y, self.center, self.additive);
        clone.position = self.position;
        clone.color = self.color;
        clone.visible = self.visible;
        if let Some(texture) = self.texture.clone() {
            clone.set_texture(texture);
        }
        clone
    }

    /// Legacy class identifier.
    pub fn class_id(&self) -> u32 {
        0x1000
    }
}

/// Dynamic screen mesh class for more complex 2D rendering.
pub struct DynamicScreenMesh {
    pub render2d: Render2D,
    pub vertices: Vec<Vec2>,
    pub colors: Vec<u32>,
    pub visible: bool,
}

impl DynamicScreenMesh {
    pub fn new() -> Self {
        Self {
            render2d: Render2D::new(),
            vertices: Vec::new(),
            colors: Vec::new(),
            visible: true,
        }
    }

    pub fn set_mesh_data(&mut self, vertices: Vec<Vec2>, colors: Vec<u32>) {
        self.vertices = vertices;
        self.colors = colors;
        self.update_render_data();
    }

    fn update_render_data(&mut self) {
        self.render2d.reset();
        if self.vertices.is_empty() {
            return;
        }

        for i in (0..self.vertices.len()).step_by(3) {
            if let (Some(v0), Some(v1), Some(v2)) = (
                self.vertices.get(i),
                self.vertices.get(i + 1),
                self.vertices.get(i + 2),
            ) {
                let color0 = *self.colors.get(i).unwrap_or(&0xFFFFFFFF);
                self.render2d
                    .add_triangle(*v0, *v1, *v2, *v0, *v1, *v2, color0);
            }
        }
    }

    pub fn render<'pass>(
        &'pass mut self,
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        if self.visible {
            self.render2d.render(gpu, render_pass);
        }
    }
}
