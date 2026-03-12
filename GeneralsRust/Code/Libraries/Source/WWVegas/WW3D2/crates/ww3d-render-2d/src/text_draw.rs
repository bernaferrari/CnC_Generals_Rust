//! TextDraw - Alternative Text Rendering API
//!
//! This module provides a simple, immediate-mode text drawing API similar to
//! the C++ TextDrawClass. Ideal for quick debug text and simple UI elements.

use glam::{Vec2, Vec3};
use std::sync::Arc;
use ww3d_renderer_3d::rendering::render2d::font3d::Font3DData;
use ww3d_renderer_3d::rendering::render2d::{Rect, Render2D, Render2DGpuContext};

/// Simple immediate-mode text drawing utility
pub struct TextDraw {
    render2d: Render2D,
    font: Option<Arc<Font3DData>>,
    color: u32,
    scale: f32,
    coordinate_scale: Vec2,
    coordinate_offset: Vec2,
}

impl TextDraw {
    /// Create a new TextDraw instance
    pub fn new() -> Self {
        Self {
            render2d: Render2D::new(),
            font: None,
            color: 0xFFFFFFFF,
            scale: 1.0,
            coordinate_scale: Vec2::ONE,
            coordinate_offset: Vec2::ZERO,
        }
    }

    /// Set the coordinate range transformation
    ///
    /// This allows mapping from arbitrary coordinate spaces to screen space
    /// Similar to C++ Set_Coordinate_Ranges
    pub fn set_coordinate_ranges(
        &mut self,
        param_ul: Vec2,
        param_lr: Vec2,
        dest_ul: Vec2,
        dest_lr: Vec2,
    ) {
        let param_size = param_lr - param_ul;
        let dest_size = dest_lr - dest_ul;

        self.coordinate_scale = Vec2::new(dest_size.x / param_size.x, dest_size.y / param_size.y);

        self.coordinate_offset = dest_ul - param_ul * self.coordinate_scale;
    }

    /// Set the font to use for text rendering
    pub fn set_font(&mut self, font: Arc<Font3DData>) {
        if let Some(texture) = font.texture.clone() {
            self.render2d.set_texture(texture);
        }
        self.font = Some(font);
    }

    /// Set the text color (AARRGGBB format)
    pub fn set_text_color(&mut self, color: u32) {
        self.color = color;
    }

    /// Set the text color from RGB vector
    pub fn set_text_color_rgb(&mut self, color: Vec3) {
        self.color = ((color.x * 255.0) as u32) << 16
            | ((color.y * 255.0) as u32) << 8
            | ((color.z * 255.0) as u32)
            | 0xFF000000;
    }

    /// Set the text scale
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.01);
    }

    /// Get the width of a single character
    pub fn get_char_width(&self, ch: char) -> f32 {
        if let Some(ref font) = self.font {
            font.char_width(ch) as f32 * self.scale
        } else {
            8.0 * self.scale
        }
    }

    /// Get the width of a string
    pub fn get_width(&self, message: &str) -> f32 {
        if let Some(ref font) = self.font {
            let mut width = 0.0;
            for ch in message.chars() {
                width += font.char_width(ch) as f32 * self.scale;
            }
            width
        } else {
            message.len() as f32 * 8.0 * self.scale
        }
    }

    /// Get the inter-character spacing width
    pub fn get_inter_char_width(&self) -> f32 {
        1.0 * self.scale
    }

    /// Get the height of text
    pub fn get_height(&self, _message: Option<&str>) -> f32 {
        if let Some(ref font) = self.font {
            font.char_height as f32 * self.scale
        } else {
            16.0 * self.scale
        }
    }

    /// Print a single character at the given screen position
    ///
    /// Returns the width of the drawn character
    pub fn print_char(&mut self, ch: char, screen_x: f32, screen_y: f32) -> f32 {
        let font = match self.font.as_ref() {
            Some(f) => f,
            None => return 0.0,
        };

        if ch == ' ' {
            return font.space_width as f32 * self.scale;
        }

        let char_width = font.char_width(ch) as f32 * self.scale;
        let char_height = font.char_height as f32 * self.scale;

        // Transform coordinates
        let x = screen_x * self.coordinate_scale.x + self.coordinate_offset.x;
        let y = screen_y * self.coordinate_scale.y + self.coordinate_offset.y;

        // Get UV coordinates
        let uv_rect = font.char_uv(ch);

        // Create screen rectangle
        let screen_rect = Rect::new(x, y, x + char_width, y + char_height);

        // Draw the character
        self.render2d
            .add_quad_rect(screen_rect, uv_rect, self.color);

        char_width
    }

    /// Print a string at the given screen position
    ///
    /// Returns the total width of the drawn text
    pub fn print(&mut self, message: &str, screen_x: f32, screen_y: f32) -> f32 {
        let mut x = screen_x;
        let mut total_width = 0.0;

        for ch in message.chars() {
            if ch == '\n' {
                // Newline support
                return total_width;
            }

            let char_width = self.print_char(ch, x, screen_y);
            x += char_width + self.get_inter_char_width();
            total_width += char_width + self.get_inter_char_width();
        }

        total_width
    }

    /// Print text centered at a given x position
    pub fn print_centered(&mut self, message: &str, center_x: f32, screen_y: f32) -> f32 {
        let width = self.get_width(message);
        self.print(message, center_x - width * 0.5, screen_y)
    }

    /// Draw a filled quad at screen coordinates
    pub fn quad(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.quad_uv(x0, y0, x1, y1, 0.0, 0.0, 1.0, 1.0);
    }

    /// Draw a filled quad with UV coordinates
    pub fn quad_uv(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) {
        let screen_rect = Rect::new(
            x0 * self.coordinate_scale.x + self.coordinate_offset.x,
            y0 * self.coordinate_scale.y + self.coordinate_offset.y,
            x1 * self.coordinate_scale.x + self.coordinate_offset.x,
            y1 * self.coordinate_scale.y + self.coordinate_offset.y,
        );

        let uv_rect = Rect::new(u0, v0, u1, v1);

        self.render2d
            .add_quad_rect(screen_rect, uv_rect, self.color);
    }

    /// Draw a line from point A to point B with given width
    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32) {
        let a_transformed = Vec2::new(
            a.x * self.coordinate_scale.x + self.coordinate_offset.x,
            a.y * self.coordinate_scale.y + self.coordinate_offset.y,
        );

        let b_transformed = Vec2::new(
            b.x * self.coordinate_scale.x + self.coordinate_offset.x,
            b.y * self.coordinate_scale.y + self.coordinate_offset.y,
        );

        self.render2d
            .add_line(a_transformed, b_transformed, width, self.color);
    }

    /// Draw a line with tapered ends
    pub fn line_ends(&mut self, a: Vec2, b: Vec2, width: f32, end_percent: f32) {
        // Calculate line direction
        let dir = (b - a).normalize();
        let _normal = Vec2::new(-dir.y, dir.x) * width * 0.5;

        let length = (b - a).length();
        let taper_length = length * end_percent;

        // Draw main body
        let start_main = a + dir * taper_length;
        let end_main = b - dir * taper_length;
        self.line(start_main, end_main, width);

        // Draw tapered start
        if taper_length > 0.0 {
            let start_center = a + dir * taper_length * 0.5;
            self.line(a, start_center, width * 0.5);
        }

        // Draw tapered end
        if taper_length > 0.0 {
            let end_center = b - dir * taper_length * 0.5;
            self.line(end_center, b, width * 0.5);
        }
    }

    /// Display the font texture for debugging
    pub fn show_font(&mut self, screen_x: f32, screen_y: f32, size: f32) {
        self.quad_uv(
            screen_x,
            screen_y,
            screen_x + size,
            screen_y + size,
            0.0,
            0.0,
            1.0,
            1.0,
        );
    }

    /// Reset all geometry
    pub fn reset(&mut self) {
        self.render2d.reset();
    }

    /// Render all queued geometry
    pub fn render<'pass>(
        &'pass mut self,
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.render2d.render(gpu, render_pass);
    }
}

impl Default for TextDraw {
    fn default() -> Self {
        Self::new()
    }
}
