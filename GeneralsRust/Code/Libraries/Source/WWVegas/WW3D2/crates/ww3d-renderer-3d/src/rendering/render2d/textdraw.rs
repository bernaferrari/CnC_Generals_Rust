//! Text drawing utilities mirroring the legacy WW3D TextDrawClass.
//!
//! This module emulates the behaviour of the original C++ TextDraw system while
//! emitting geometry through the modern Render2D batching layer.

use crate::rendering::render2d::{frgb_to_u32, Rect, Render2D, Render2DGpuContext};
use crate::rendering::render2d::font3d::Font3DInstance;
use crate::rendering::shader_system::{
    CullModeType, DepthCompareType, DepthMaskType, DstBlendFuncType, ShaderClass, SrcBlendFuncType,
    TexturingType,
};
use glam::{Vec2, Vec3};

/// Runtime helper for drawing 2D text and primitives using font atlases.
pub struct TextDrawClass {
    render2d: Render2D,
    default_shader: ShaderClass,
    translate_scale: Vec2,
    translate_offset: Vec2,
    pixel_size: Vec2,
    text_color: u32,
    max_chars: usize,
}

impl TextDrawClass {
    /// Create a new text drawer with capacity hints for the glyph count.
    pub fn new(max_chars: usize) -> Self {
        let mut render2d = Render2D::new();
        let mut shader = ShaderClass::new();
        shader.set_depth_mask(DepthMaskType::Disable);
        shader.set_depth_compare(DepthCompareType::Always);
        shader.set_dst_blend_func(DstBlendFuncType::InvSrcAlpha);
        shader.set_src_blend_func(SrcBlendFuncType::SrcAlpha);
        shader.set_texturing(TexturingType::Enable);
        shader.set_cull_mode(CullModeType::Disable);
        render2d.shader = shader;
        render2d.set_coordinate_range(Rect::new(-1.0, 0.75, 1.0, -0.75));

        let mut instance = Self {
            render2d,
            default_shader: shader,
            translate_scale: Vec2::new(1.0, 1.0),
            translate_offset: Vec2::ZERO,
            pixel_size: Vec2::new(1.0 / 640.0, 1.0 / 480.0),
            text_color: frgb_to_u32(1.0, 1.0, 1.0),
            max_chars,
        };

        // Match the legacy default coordinate mapping (-1..1 horizontally, +/-0.75 vertically).
        instance.set_coordinate_ranges(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 0.75),
            Vec2::new(1.0, -0.75),
        );
        instance
    }

    /// Reset the accumulated geometry and restore default shader settings.
    pub fn reset(&mut self) {
        self.render2d.reset();
        self.render2d.shader = self.default_shader;
    }

    /// Update coordinate transforms to match the legacy API.
    pub fn set_coordinate_ranges(
        &mut self,
        src_ul: Vec2,
        src_lr: Vec2,
        dest_ul: Vec2,
        dest_lr: Vec2,
    ) {
        self.translate_scale.x = (dest_lr.x - dest_ul.x) / (src_lr.x - src_ul.x);
        self.translate_scale.y = (dest_lr.y - dest_ul.y) / (src_lr.y - src_ul.y);
        self.translate_offset.x = dest_ul.x - self.translate_scale.x * src_ul.x;
        self.translate_offset.y = dest_ul.y - self.translate_scale.y * src_ul.y;

        self.pixel_size.x = (src_lr.x - src_ul.x).abs() / 640.0;
        self.pixel_size.y = (src_lr.y - src_ul.y).abs() / 480.0;

        self.render2d
            .set_coordinate_range(Rect::new(dest_ul.x, dest_ul.y, dest_lr.x, dest_lr.y));
    }

    /// Override the shader used for subsequent draw calls.
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.render2d.shader = shader;
    }

    /// Access the current shader.
    pub fn shader(&self) -> ShaderClass {
        self.render2d.shader
    }

    /// Toggle additive blending (matches `Make_Additive`).
    pub fn make_additive(&mut self) {
        let shader = &mut self.render2d.shader;
        shader.set_src_blend_func(SrcBlendFuncType::One);
        shader.set_dst_blend_func(DstBlendFuncType::One);
    }

    /// Set the default vertex colour using linear RGB.
    pub fn set_text_color(&mut self, color: Vec3) {
        self.text_color = frgb_to_u32(color.x, color.y, color.z);
    }

    /// Retrieve the current vertex colour.
    pub fn text_color(&self) -> u32 {
        self.text_color
    }

    /// Draw a textured quad with explicit coordinates.
    pub fn quad(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, u0: f32, v0: f32, u1: f32, v1: f32) {
        let (x0, x1, y0, y1) = self.transform_and_snap(x0, x1, y0, y1);

        let mut left = x0;
        let mut right = x1;
        let mut u_left = u0;
        let mut u_right = u1;
        if right < left {
            left = x1;
            right = x0;
            u_left = u1;
            u_right = u0;
        }

        let mut top = y0;
        let mut bottom = y1;
        let mut v_top = v0;
        let mut v_bottom = v1;
        if bottom < top {
            top = y1;
            bottom = y0;
            v_top = v1;
            v_bottom = v0;
        }

        let screen_rect = Rect::new(left, top, right, bottom);
        let uv_rect = Rect::new(u_left, v_top, u_right, v_bottom);
        self.render2d
            .add_quad_rect(screen_rect, uv_rect, self.text_color);
    }

    /// Draw a textured quad using helper rectangles.
    pub fn quad_rect(&mut self, rect: Rect, uv: Rect) {
        self.quad(rect.left, rect.top, rect.right, rect.bottom, uv.left, uv.top, uv.right, uv.bottom);
    }

    /// Draw a screen-space line with the requested pixel width.
    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32) {
        let mut p0 = self.transform_point(a);
        let mut p1 = self.transform_point(b);

        let mut direction = p1 - p0;
        if direction.length_squared() < f32::EPSILON {
            return;
        }
        let mut offset = Vec2::new(-direction.y, direction.x);
        offset = offset.normalize() * (width * self.translate_scale.x.abs() * 0.5);

        let v0 = p0 + offset;
        let v1 = p0 - offset;
        let v2 = p1 - offset;
        let v3 = p1 + offset;
        let uv = Rect::new(0.0, 0.0, 1.0, 1.0);
        self.render2d.add_quad(v0, v1, v2, v3, uv, self.text_color);
    }

    /// Draw the first and last `end_percent` portions of a line.
    pub fn line_ends(&mut self, a: Vec2, b: Vec2, width: f32, end_percent: f32) {
        let segment = (b - a) * end_percent;
        self.line(a, a + segment, width);
        self.line(b, b - segment, width);
    }

    /// Width of a string using the supplied font metrics.
    pub fn get_width(&self, font: &Font3DInstance, message: &str) -> f32 {
        message.chars().map(|ch| font.char_spacing(ch)).sum()
    }

    /// Width of a single character using the font metrics.
    pub fn get_char_width(&self, font: &Font3DInstance, ch: char) -> f32 {
        font.char_width(ch)
    }

    /// Inter-character spacing in screen units.
    pub fn get_inter_char_width(&self, font: &Font3DInstance) -> f32 {
        font.char_spacing(' ') - font.char_width(' ')
    }

    /// Height of the font in screen units. When a message is supplied the height spans all lines.
    pub fn get_height(&self, font: &Font3DInstance, message: Option<&str>) -> f32 {
        let lines = message
            .map(|msg| msg.chars().filter(|&c| c == '\n').count() + 1)
            .unwrap_or(1);
        font.char_height() * lines as f32
    }

    /// Print a single character. Returns the glyph advance in screen units.
    pub fn print_char(
        &mut self,
        font: &Font3DInstance,
        ch: char,
        screen_x: f32,
        screen_y: f32,
    ) -> f32 {
        let spacing = font.char_spacing(ch);
        if ch == ' ' {
            return spacing;
        }

        if let Some(texture) = font.texture_clone() {
            self.render2d.set_texture(texture);
        }

        let width = font.char_width(ch);
        if width.abs() < f32::EPSILON {
            return spacing;
        }

        let mut screen_x0 = screen_x + spacing * 0.5 - width * 0.5;
        screen_x0 = self.snap(screen_x0, self.pixel_size.x);
        let mut screen_x1 = screen_x0 + width;
        screen_x1 = self.snap(screen_x1, self.pixel_size.x);

        let mut screen_y0 = self.snap(screen_y, self.pixel_size.y);
        let y_sign = if self.translate_scale.y > 0.0 { -1.0 } else { 1.0 };
        let mut screen_y1 = screen_y0 + font.char_height() * y_sign;
        screen_y1 = self.snap(screen_y1, self.pixel_size.y);

        let uv = font.char_uv(ch);
        self.quad(screen_x0, screen_y0, screen_x1, screen_y1, uv.left, uv.top, uv.right, uv.bottom);

        spacing
    }

    /// Print an ASCII string. Returns the total advance.
    pub fn print(&mut self, font: &Font3DInstance, message: &str, mut screen_x: f32, screen_y: f32) -> f32 {
        let mut total_width = 0.0;
        for ch in message.chars() {
            let advance = self.print_char(font, ch, screen_x, screen_y);
            screen_x += advance;
            total_width += advance;
        }
        total_width
    }

    /// Render the prepared geometry.
    pub fn render<'pass>(&'pass mut self, gpu: &'pass mut Render2DGpuContext, pass: &mut wgpu::RenderPass<'pass>) {
        self.render2d.render(gpu, pass);
    }

    /// Draw the entire font texture for debugging purposes.
    pub fn show_font(&mut self, font: &Font3DInstance, screen_x: f32, screen_y: f32) {
        if let Some(texture) = font.texture_clone() {
            self.render2d.set_texture(texture);
        }

        let size_x = self.pixel_size.x * 256.0;
        let size_y = self.pixel_size.y * 256.0;
        let rect = Rect::new(screen_x, screen_y, screen_x + size_x, screen_y + size_y);
        let uv = Rect::new(0.0, 0.0, 1.0, 1.0);
        self.quad_rect(rect, uv);
    }

    fn transform_point(&self, point: Vec2) -> Vec2 {
        Vec2::new(
            point.x * self.translate_scale.x + self.translate_offset.x,
            point.y * self.translate_scale.y + self.translate_offset.y,
        )
    }

    fn transform_and_snap(&self, x0: f32, x1: f32, y0: f32, y1: f32) -> (f32, f32, f32, f32) {
        let tx0 = self.snap(x0 * self.translate_scale.x + self.translate_offset.x, self.pixel_size.x);
        let tx1 = self.snap(x1 * self.translate_scale.x + self.translate_offset.x, self.pixel_size.x);
        let ty0 = self.snap(y0 * self.translate_scale.y + self.translate_offset.y, self.pixel_size.y);
        let ty1 = self.snap(y1 * self.translate_scale.y + self.translate_offset.y, self.pixel_size.y);
        (tx0, tx1, ty0, ty1)
    }

    fn snap(&self, value: f32, step: f32) -> f32 {
        if step <= f32::EPSILON {
            value
        } else {
            (value / step).round() * step
        }
    }
}
