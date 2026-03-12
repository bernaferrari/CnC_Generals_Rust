//! Font3D system for 3D text rendering
//!
//! This module provides font loading and text rendering capabilities
//! equivalent to the original WW3D Font3D subsystem while targeting the
//! modern wgpu-based renderer.

use crate::rendering::render2d::{frgba_to_u32, Rect, Render2D, Render2DGpuContext};
use crate::texture_system::{SurfaceClass, TextureClass};
use glam::{Vec2, Vec3};
use std::collections::HashMap;

/// Per-character metadata extracted from the font atlas.
#[derive(Debug, Clone)]
pub struct FontChar {
    pub u_offset: f32,
    pub v_offset: f32,
    pub u_width: f32,
    pub v_height: f32,
    pub width: u8,
    pub height: u8,
}

/// Raw font data shared by all instances.
#[derive(Debug, Clone)]
pub struct Font3DData {
    pub name: String,
    pub texture: Option<TextureClass>,
    pub char_table: HashMap<u8, FontChar>,
    pub char_height: u8,
    pub space_width: u8,
}

impl Font3DData {
    /// Create a new font data object from a source file. The current
    /// implementation fabricates a simple 16x16 ASCII atlas so the
    /// higher-level systems can be exercised.
    pub fn from_file(filename: &str) -> Self {
        let mut char_table = HashMap::new();

        // Simple monospace grid (16x16 tiles) spanning the printable ASCII range.
        let char_width = 1.0 / 16.0;
        let char_height = 1.0 / 16.0;

        for ascii in 0..256u32 {
            let row = (ascii / 16) as f32;
            let col = (ascii % 16) as f32;

            let char_data = FontChar {
                u_offset: col * char_width,
                v_offset: row * char_height,
                u_width: char_width,
                v_height: char_height,
                width: 8,
                height: 16,
            };
            char_table.insert(ascii as u8, char_data);
        }

        let mut data = Self {
            name: filename.to_string(),
            texture: None,
            char_table,
            char_height: 16,
            space_width: 8,
        };

        if let Ok(texture) = TextureClass::load_from_file(filename) {
            data.texture = Some(texture);
        }

        data
    }

    /// Construct the font data from an in-memory surface (typical for UI assets).
    pub fn from_surface(name: &str, surface: &SurfaceClass) -> Self {
        let mut data = Self::from_file(name);
        if let Ok(texture) = TextureClass::from_surface(name, surface) {
            data.texture = Some(texture);
        }
        data
    }

    /// Width of a character in texels (unscaled).
    pub fn char_width(&self, ch: char) -> u8 {
        if ch == ' ' {
            return self.space_width;
        }
        self.char_table
            .get(&(ch as u8))
            .map(|c| c.width)
            .unwrap_or(self.space_width)
    }

    /// Normalized atlas rectangle for a character.
    pub fn char_uv(&self, ch: char) -> Rect {
        let data = self
            .char_table
            .get(&(ch as u8))
            .cloned()
            .unwrap_or_else(|| self.char_table[&(b'?' as u8)].clone());
        Rect::new(
            data.u_offset,
            data.v_offset,
            data.u_offset + data.u_width,
            data.v_offset + data.v_height,
        )
    }

    /// Provide a clone of the backing texture if available.
    pub fn texture_clone(&self) -> Option<TextureClass> {
        self.texture.clone()
    }
}

/// Runtime font instance that caches scaling and spacing state.
pub struct Font3DInstance {
    pub font_data: Font3DData,
    pub render2d: Render2D,
    pub color: u32,
    pub position: Vec2,
    pub scale: f32,
    space_spacing: f32,
    inter_char_spacing: f32,
    mono_spacing: Option<f32>,
}

impl Font3DInstance {
    /// Construct a font instance from shared data.
    pub fn new(font_data: Font3DData) -> Self {
        let mut render2d = Render2D::new();
        if let Some(texture) = font_data.texture.clone() {
            render2d.set_texture(texture);
        }

        let default_space = font_data.char_width('H') as f32 * 0.5;
        Self {
            font_data,
            render2d,
            color: 0xFFFFFFFF,
            position: Vec2::ZERO,
            scale: 1.0,
            space_spacing: default_space,
            inter_char_spacing: 1.0,
            mono_spacing: None,
        }
    }

    /// Rebind the font atlas texture after loading.
    pub fn set_texture(&mut self, texture: TextureClass) {
        self.font_data.texture = Some(texture.clone());
        self.render2d.set_texture(texture);
    }

    /// Set the render colour (AARRGGBB).
    pub fn set_color(&mut self, color: u32) {
        self.color = color;
    }

    /// Convenience for specifying colour via linear RGB with implicit alpha.
    pub fn set_color_rgb(&mut self, color: Vec3, alpha: f32) {
        self.color = frgba_to_u32(color.x, color.y, color.z, alpha);
    }

    /// Position the text cursor (used mainly by higher level helpers).
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = Vec2::new(x, y);
    }

    /// Configure the global font scale.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.0);
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Toggle monospace mode. When `Some(width)` is supplied the width is measured in
    /// source texels before scaling. `None` restores proportional layout.
    pub fn set_mono_spaced(&mut self, spacing: Option<f32>) {
        self.mono_spacing = spacing;
    }

    /// Reset to proportional spacing (identical to `set_mono_spaced(None)`).
    pub fn set_proportional(&mut self) {
        self.mono_spacing = None;
    }

    /// Specify inter-character spacing in texels (unscaled).
    pub fn set_inter_char_spacing(&mut self, spacing: f32) {
        self.inter_char_spacing = spacing.max(0.0);
    }

    /// Specify custom space character width in texels (unscaled).
    pub fn set_space_spacing(&mut self, spacing: f32) {
        self.space_spacing = spacing.max(0.0);
    }

    /// Width of the glyph advance in screen units (includes inter-character spacing).
    pub fn char_spacing(&self, ch: char) -> f32 {
        let base = if ch == ' ' {
            self.space_spacing
        } else if let Some(mono) = self.mono_spacing {
            mono
        } else {
            self.font_data.char_width(ch) as f32
        };
        (base + self.inter_char_spacing) * self.scale
    }

    /// Visual width of the glyph in screen units (monospace aware).
    pub fn char_width(&self, ch: char) -> f32 {
        let base = if let Some(mono) = self.mono_spacing {
            mono
        } else {
            self.font_data.char_width(ch) as f32
        };
        base * self.scale
    }

    /// Height of the font in screen units.
    pub fn char_height(&self) -> f32 {
        self.font_data.char_height as f32 * self.scale
    }

    /// Atlas rectangle for a glyph.
    pub fn char_uv(&self, ch: char) -> Rect {
        self.font_data.char_uv(ch)
    }

    /// Clone of the backing texture, if available.
    pub fn texture_clone(&self) -> Option<TextureClass> {
        self.font_data.texture.clone()
    }

    /// Draw an individual character into the internal Render2D batch.
    pub fn draw_char(&mut self, ch: char, pen_x: f32, pen_y: f32) {
        if ch == ' ' {
            return;
        }

        let width = self.char_width(ch);
        if width.abs() < f32::EPSILON {
            return;
        }

        let spacing = self.char_spacing(ch);
        let height = self.char_height();

        // Center the glyph within its spacing just like the C++ implementation.
        let x0 = pen_x + spacing * 0.5 - width * 0.5;
        let x1 = x0 + width;
        let y0 = pen_y;
        let y1 = y0 + height;

        let screen_rect = Rect::new(x0, y0, x1, y1);
        let uv_rect = self.char_uv(ch);

        self.render2d
            .add_quad_rect(screen_rect, uv_rect, self.color);
    }

    /// Draw a UTF-8 string at the supplied location.
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32) {
        let mut pen = Vec2::new(x, y);

        for ch in text.chars() {
            if ch == '\n' {
                pen.x = x;
                pen.y += self.char_height();
                continue;
            }

            self.draw_char(ch, pen.x, pen.y);
            pen.x += self.char_spacing(ch);
        }
    }

    /// Draw the string centred around `center_x`.
    pub fn draw_centered_text(&mut self, text: &str, center_x: f32, y: f32) {
        let width = self.calculate_text_width(text);
        self.draw_text(text, center_x - width * 0.5, y);
    }

    /// Estimate the width of a string using the current spacing rules.
    pub fn calculate_text_width(&self, text: &str) -> f32 {
        let mut width = 0.0f32;
        let mut line_width = 0.0f32;

        for ch in text.chars() {
            if ch == '\n' {
                width = width.max(line_width);
                line_width = 0.0;
                continue;
            }
            line_width += self.char_spacing(ch);
        }

        width.max(line_width)
    }

    /// Submit the internal Render2D geometry.
    pub fn render<'pass>(
        &'pass mut self,
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.render2d.render(gpu, render_pass);
    }

    /// Reset the accumulated geometry.
    pub fn reset(&mut self) {
        self.render2d.reset();
    }

    /// Flush the accumulated geometry without clearing it (legacy helper).
    pub fn flush_render2d<'pass>(
        &'pass mut self,
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.render2d.render(gpu, render_pass);
    }
}

/// Legacy helper utilities that mimic the C++ TextDraw usage patterns.
pub struct TextDraw;

impl TextDraw {
    /// Draw text with the supplied font and submit immediately.
    pub fn draw_text<'pass>(
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
        font: &'pass mut Font3DInstance,
        text: &str,
        x: f32,
        y: f32,
        color: u32,
    ) {
        font.render2d.reset();
        font.set_color(color);
        font.draw_text(text, x, y);
        font.flush_render2d(gpu, render_pass);
    }

    /// Draw text centred around `center_x`.
    pub fn draw_centered_text<'pass>(
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
        font: &'pass mut Font3DInstance,
        text: &str,
        center_x: f32,
        y: f32,
        color: u32,
    ) {
        font.render2d.reset();
        font.set_color(color);
        font.draw_centered_text(text, center_x, y);
        font.flush_render2d(gpu, render_pass);
    }

    /// Draw text with a drop shadow.
    pub fn draw_text_with_shadow<'pass>(
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
        font: &'pass mut Font3DInstance,
        text: &str,
        x: f32,
        y: f32,
        color: u32,
        shadow_color: u32,
        shadow_offset: Vec2,
    ) {
        font.render2d.reset();
        font.set_color(shadow_color);
        font.draw_text(text, x + shadow_offset.x, y + shadow_offset.y);
        font.set_color(color);
        font.draw_text(text, x, y);
        font.flush_render2d(gpu, render_pass);
    }
}
