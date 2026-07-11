//! 2D Render Sentence System
//!
//! This module implements the legacy WW3D Render2DSentence system on top of the
//! modern wgpu-based Render2D helpers. The goal is to preserve the original C++
//! behaviour (word wrapping, clipping, shader control, batching per texture)
//! while exposing a safe, ergonomic Rust API.

use crate::rendering::render2d::font3d::Font3DData;
use crate::rendering::render2d::{Rect, Render2D, Render2DGpuContext};
use crate::rendering::shader_system::{
    DstBlendFuncType, PriGradientType, SecGradientType, ShaderClass, SrcBlendFuncType,
};
use glam::Vec2;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct SentenceChunk {
    screen_rect: Rect,
    uv_rect: Rect,
    texture_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HotkeyGlyph {
    pub screen_rect: Rect,
    pub uv_rect: Rect,
    pub texture_key: Option<String>,
    pub character: char,
    pub baseline_offset: Vec2,
    pub line_index: usize,
}

struct RendererRecord {
    texture_key: Option<String>,
    render2d: Render2D,
}

pub struct Render2DSentenceClass {
    font: Option<Arc<Font3DData>>,
    shader: ShaderClass,
    location: Vec2,
    base_location: Vec2,
    cursor: Vec2,
    wrap_width: Option<f32>,
    centered: bool,
    parse_hot_key: bool,
    use_hard_word_wrap: bool,
    clip_rect: Option<Rect>,
    draw_extents: Rect,
    sentence_data: Vec<SentenceChunk>,
    hotkey_glyphs: Vec<HotkeyGlyph>,
    renderers: Vec<RendererRecord>,
    text: String,
    color: u32,
    monospace: bool,
    dirty: bool,
    hotkey_color: Option<u32>,
}

impl Default for Render2DSentenceClass {
    fn default() -> Self {
        Self::new()
    }
}

impl Render2DSentenceClass {
    /// Create an empty sentence instance.
    pub fn new() -> Self {
        Self {
            font: None,
            shader: ShaderClass::new(),
            location: Vec2::ZERO,
            base_location: Vec2::ZERO,
            cursor: Vec2::ZERO,
            wrap_width: None,
            centered: false,
            parse_hot_key: false,
            use_hard_word_wrap: false,
            clip_rect: None,
            draw_extents: Rect::new(0.0, 0.0, 0.0, 0.0),
            sentence_data: Vec::new(),
            hotkey_glyphs: Vec::new(),
            renderers: Vec::new(),
            text: String::new(),
            color: 0xFFFFFFFF,
            monospace: false,
            dirty: true,
            hotkey_color: None,
        }
    }

    /// Assign a font atlas to the sentence (shared reference via `Arc`).
    pub fn set_font(&mut self, font: Arc<Font3DData>) {
        if self
            .font
            .as_ref()
            .map(|f| Arc::ptr_eq(f, &font))
            .unwrap_or(false)
        {
            return;
        }
        self.font = Some(font);
        self.mark_dirty();
    }

    /// Remove the current font association.
    pub fn clear_font(&mut self) {
        if self.font.take().is_some() {
            self.mark_dirty();
        }
    }

    /// Set the world-space location offset this sentence should render at.
    pub fn set_location(&mut self, location: Vec2) {
        self.location = location;
    }

    /// Set the base location used for cursor calculations.
    pub fn set_base_location(&mut self, location: Vec2) {
        self.base_location = location;
        // Legacy implementation shifts existing renderers; in this port we simply
        // rebuild the sentence geometry the next time it is drawn.
        self.mark_dirty();
    }

    /// Replace the sentence text.
    pub fn set_text(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);
        self.mark_dirty();
    }

    /// Access the raw sentence text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Specify the default RGBA color (0xAARRGGBB) for the sentence.
    pub fn set_color(&mut self, color: u32) {
        if self.color != color {
            self.color = color;
            self.mark_dirty();
        }
    }

    /// Retrieve the default sentence color.
    pub fn color(&self) -> u32 {
        self.color
    }

    /// Enable or disable monospace layout. When enabled, every glyph uses the
    /// font's nominal space width instead of its individual advance.
    pub fn set_monospace(&mut self, enabled: bool) {
        if self.monospace != enabled {
            self.monospace = enabled;
            self.mark_dirty();
        }
    }

    /// Configure soft wrapping width in pixels. Use `None` to disable wrapping.
    pub fn set_wrap_width(&mut self, width: Option<f32>) {
        if self.wrap_width != width {
            self.wrap_width = width;
            self.mark_dirty();
        }
    }

    /// Mirror the C++ `Set_Word_Wrap_Centered` behaviour.
    pub fn set_word_wrap_centered(&mut self, centered: bool) {
        if self.centered != centered {
            self.centered = centered;
            self.mark_dirty();
        }
    }

    /// Enable parsing of legacy hot-key markers (ampersand sequences).
    pub fn set_hot_key_parse(&mut self, parse: bool) {
        if self.parse_hot_key != parse {
            self.parse_hot_key = parse;
            self.mark_dirty();
        }
    }

    /// Specify the colour used when rendering hot-key glyphs. `None` falls back to the base sentence colour.
    pub fn set_hotkey_color(&mut self, color: Option<u32>) {
        self.hotkey_color = color;
    }

    /// Retrieve the first parsed hot-key character and its baseline offset if present.
    pub fn first_hotkey(&self) -> Option<(char, Vec2)> {
        self.hotkey_glyphs
            .first()
            .map(|glyph| (glyph.character, glyph.baseline_offset))
    }

    /// Expose all parsed hot-key glyphs for advanced rendering paths.
    pub fn hotkey_glyphs(&self) -> &[HotkeyGlyph] {
        &self.hotkey_glyphs
    }

    /// Mirror the legacy hard word wrap toggle.
    pub fn set_use_hard_word_wrap(&mut self, use_hard: bool) {
        if self.use_hard_word_wrap != use_hard {
            self.use_hard_word_wrap = use_hard;
            self.mark_dirty();
        }
    }

    /// Apply a clipping rectangle in screen space.
    pub fn set_clipping_rect(&mut self, rect: Rect) {
        self.clip_rect = Some(rect);
    }

    /// Remove any active clip rectangle.
    pub fn disable_clipping(&mut self) {
        self.clip_rect = None;
    }

    /// Query whether clipping is enabled.
    pub fn is_clipping_enabled(&self) -> bool {
        self.clip_rect.is_some()
    }

    /// Request additive blending, matching `Make_Additive` from the C++ engine.
    pub fn make_additive(&mut self) {
        self.shader.set_src_blend_func(SrcBlendFuncType::One);
        self.shader.set_dst_blend_func(DstBlendFuncType::One);
        self.shader.set_pri_gradient(PriGradientType::Modulate);
        self.shader.set_sec_gradient(SecGradientType::Disable);
        self.propagate_shader();
    }

    /// Explicitly override the shader used when drawing the sentence.
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.shader = shader;
        self.propagate_shader();
    }

    /// Retrieve the current shader configuration.
    pub fn shader(&self) -> ShaderClass {
        self.shader
    }

    /// Reset all cached Render2D geometry while keeping renderer objects.
    pub fn reset_polys(&mut self) {
        for record in &mut self.renderers {
            record.render2d.reset();
            record.render2d.shader = self.shader;
        }
    }

    /// Release all internal state (fonts remain untouched).
    pub fn reset(&mut self) {
        self.reset_polys();
        self.renderers.clear();
        self.sentence_data.clear();
        self.hotkey_glyphs.clear();
        self.cursor = Vec2::ZERO;
        self.draw_extents = Rect::new(0.0, 0.0, 0.0, 0.0);
        self.dirty = true;
    }

    /// Compute the text extents in local space after rebuilding if necessary.
    pub fn text_extents(&mut self) -> Vec2 {
        self.ensure_sentence();
        Vec2::new(self.draw_extents.width(), self.draw_extents.height())
    }

    /// Draw the sentence geometry into internal Render2D batches.
    pub fn draw_sentence(&mut self, override_color: Option<u32>) {
        self.ensure_sentence();

        self.reset_polys();
        self.draw_extents = Rect::new(0.0, 0.0, 0.0, 0.0);

        let base_color = override_color.unwrap_or(self.color);
        let highlight_color = if override_color.is_none() {
            self.hotkey_color.or(Some(base_color))
        } else {
            None
        };

        let mut bounds_min = Vec2::new(f32::MAX, f32::MAX);
        let mut bounds_max = Vec2::new(f32::MIN, f32::MIN);

        for i in 0..self.sentence_data.len() {
            let (screen_rect, uv_rect, texture_key) = {
                let chunk = &self.sentence_data[i];
                (chunk.screen_rect, chunk.uv_rect, chunk.texture_key.clone())
            };

            let offset_rect = offset_rect(screen_rect, self.location + self.base_location);
            let (screen_rect, uv_rect) = if let Some(clip) = self.clip_rect {
                match clip_quad(offset_rect, uv_rect, clip) {
                    Some((clipped_screen, clipped_uv)) => (clipped_screen, clipped_uv),
                    None => continue,
                }
            } else {
                (offset_rect, uv_rect)
            };

            bounds_min.x = bounds_min.x.min(screen_rect.left);
            bounds_min.y = bounds_min.y.min(screen_rect.top);
            bounds_max.x = bounds_max.x.max(screen_rect.right);
            bounds_max.y = bounds_max.y.max(screen_rect.bottom);

            if let Some(renderer) = self.acquire_renderer(texture_key.as_ref()) {
                renderer
                    .render2d
                    .add_quad_rect(screen_rect, uv_rect, base_color);
            }
        }

        if let Some(color) = highlight_color {
            for i in 0..self.hotkey_glyphs.len() {
                let (screen_rect, uv_rect, texture_key) = {
                    let glyph = &self.hotkey_glyphs[i];
                    (glyph.screen_rect, glyph.uv_rect, glyph.texture_key.clone())
                };

                let offset_rect = offset_rect(screen_rect, self.location + self.base_location);
                let (screen_rect, uv_rect) = if let Some(clip) = self.clip_rect {
                    match clip_quad(offset_rect, uv_rect, clip) {
                        Some((clipped_screen, clipped_uv)) => (clipped_screen, clipped_uv),
                        None => continue,
                    }
                } else {
                    (offset_rect, uv_rect)
                };

                bounds_min.x = bounds_min.x.min(screen_rect.left);
                bounds_min.y = bounds_min.y.min(screen_rect.top);
                bounds_max.x = bounds_max.x.max(screen_rect.right);
                bounds_max.y = bounds_max.y.max(screen_rect.bottom);

                if let Some(renderer) = self.acquire_renderer(texture_key.as_ref()) {
                    renderer.render2d.add_quad_rect(screen_rect, uv_rect, color);
                }
            }
        }

        if bounds_min.x <= bounds_max.x && bounds_min.y <= bounds_max.y {
            self.draw_extents = Rect::new(bounds_min.x, bounds_min.y, bounds_max.x, bounds_max.y);
        }
    }

    /// Submit the prepared quads to the GPU render pass.
    pub fn render_with_color<'pass>(
        &'pass mut self,
        override_color: Option<u32>,
        gpu: &mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.render_internal(override_color, None, gpu, render_pass);
    }

    pub fn render<'pass>(
        &'pass mut self,
        gpu: &mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.render_internal(None, None, gpu, render_pass);
    }

    pub fn render_with_shadow<'pass>(
        &'pass mut self,
        shadow_color: u32,
        offset: Vec2,
        gpu: &mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        self.render_internal(None, Some((shadow_color, offset)), gpu, render_pass);
    }

    fn render_internal<'pass>(
        &'pass mut self,
        override_color: Option<u32>,
        shadow: Option<(u32, Vec2)>,
        gpu: &mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        let original_location = self.location;

        if let Some((shadow_color, offset)) = shadow {
            self.location = original_location + offset;
            self.draw_sentence(Some(shadow_color));
            unsafe {
                let ptr = self.renderers.as_mut_ptr();
                for idx in 0..self.renderers.len() {
                    (*ptr.add(idx)).render2d.render(gpu, render_pass);
                }
            }
            self.location = original_location;
        }

        self.draw_sentence(override_color);
        unsafe {
            let ptr = self.renderers.as_mut_ptr();
            for idx in 0..self.renderers.len() {
                (*ptr.add(idx)).render2d.render(gpu, render_pass);
            }
        }
    }

    /// Access the sentence draw extents (in screen space) from the previous draw.
    pub fn draw_extents(&self) -> &Rect {
        &self.draw_extents
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn ensure_sentence(&mut self) {
        if !self.dirty {
            return;
        }
        self.rebuild_sentence();
        self.dirty = false;
    }

    fn rebuild_sentence(&mut self) {
        self.sentence_data.clear();
        self.hotkey_glyphs.clear();
        self.cursor = Vec2::ZERO;

        let font = match self.font.as_ref() {
            Some(font) => font.clone(),
            None => return,
        };

        let line_height = font.char_height as f32;
        let space_advance = font.space_width as f32;
        let wrap_limit = self.wrap_width;

        let mut current_line_start = 0usize;
        let mut current_line_width = 0.0f32;
        let mut lines: Vec<(usize, usize, f32)> = Vec::new();
        let mut chunk_line_map: Vec<usize> = Vec::new();
        let mut line_index = 0usize;

        let mut cursor = Vec2::ZERO;
        let mut chars = self.text.chars().peekable();
        let mut pending_hotkey = false;
        let mut last_break_state: Option<(usize, usize, Vec2, f32)> = None;

        while let Some(ch) = chars.next() {
            if ch == '\r' {
                continue;
            }

            if self.parse_hot_key && ch == '&' {
                if let Some(next) = chars.peek().copied() {
                    if next != ' ' && next != '\n' && next != '\r' {
                        pending_hotkey = true;
                        continue;
                    }
                }
            }

            if ch == '\n' {
                lines.push((
                    current_line_start,
                    self.sentence_data.len(),
                    current_line_width,
                ));
                current_line_start = self.sentence_data.len();
                current_line_width = 0.0;
                line_index += 1;
                cursor.x = 0.0;
                cursor.y += line_height;
                pending_hotkey = false;
                last_break_state = None;
                continue;
            }

            let is_hotkey = std::mem::take(&mut pending_hotkey);

            let advance = if self.monospace {
                space_advance
            } else if ch == ' ' {
                space_advance
            } else {
                font.char_width(ch) as f32
            };

            if let Some(limit) = wrap_limit {
                let limit = limit.max(0.0);
                let projected = cursor.x + advance;
                if projected > limit && current_line_width > 0.0 {
                    if !self.use_hard_word_wrap {
                        if let Some((sentence_len, hotkey_len, break_cursor, break_width)) =
                            last_break_state.take()
                        {
                            lines.push((current_line_start, sentence_len, break_width));
                            self.sentence_data.truncate(sentence_len);
                            self.hotkey_glyphs.truncate(hotkey_len);
                            current_line_start = sentence_len;
                            current_line_width = 0.0;
                            line_index += 1;
                            cursor = Vec2::new(0.0, break_cursor.y + line_height);
                        } else {
                            lines.push((
                                current_line_start,
                                self.sentence_data.len(),
                                current_line_width,
                            ));
                            current_line_start = self.sentence_data.len();
                            current_line_width = 0.0;
                            line_index += 1;
                            cursor.x = 0.0;
                            cursor.y += line_height;
                            last_break_state = None;
                        }
                    } else {
                        lines.push((
                            current_line_start,
                            self.sentence_data.len(),
                            current_line_width,
                        ));
                        current_line_start = self.sentence_data.len();
                        current_line_width = 0.0;
                        line_index += 1;
                        cursor.x = 0.0;
                        cursor.y += line_height;
                        last_break_state = None;
                    }
                }
            }

            let screen_rect = Rect::new(
                cursor.x,
                cursor.y,
                cursor.x + advance,
                cursor.y + line_height,
            );

            let uv_rect = glyph_uv_rect(&font, ch);

            if is_hotkey {
                self.hotkey_glyphs.push(HotkeyGlyph {
                    screen_rect,
                    uv_rect,
                    texture_key: font.texture.as_ref().map(|tex| tex.name.clone()),
                    character: ch,
                    baseline_offset: Vec2::new(cursor.x, cursor.y),
                    line_index,
                });
            } else {
                self.sentence_data.push(SentenceChunk {
                    screen_rect,
                    uv_rect,
                    texture_key: font.texture.as_ref().map(|tex| tex.name.clone()),
                });
                chunk_line_map.push(line_index);
            }

            cursor.x += advance;
            current_line_width = current_line_width.max(cursor.x);
        }
        lines.push((
            current_line_start,
            self.sentence_data.len(),
            current_line_width,
        ));
        self.cursor = cursor;

        if self.centered {
            if let Some(limit) = wrap_limit {
                center_lines(
                    &mut self.sentence_data,
                    &chunk_line_map,
                    &mut self.hotkey_glyphs,
                    &lines,
                    limit,
                );
            }
        }

        self.draw_extents = Rect::new(0.0, 0.0, cursor.x, cursor.y + line_height);
    }

    fn acquire_renderer(&mut self, texture_key: Option<&String>) -> Option<&mut RendererRecord> {
        let key = texture_key.cloned();
        let idx = self
            .renderers
            .iter()
            .position(|record| record.texture_key == key);

        if let Some(i) = idx {
            return self.renderers.get_mut(i);
        }

        let mut render2d = Render2D::new();
        render2d.shader = self.shader;

        if let Some(font) = self.font.as_ref() {
            if let Some(texture) = font.texture.as_ref() {
                render2d.set_texture(texture.clone());
            }
        }

        self.renderers.push(RendererRecord {
            texture_key: key.clone(),
            render2d,
        });
        self.renderers.last_mut()
    }

    fn propagate_shader(&mut self) {
        for record in &mut self.renderers {
            record.render2d.shader = self.shader;
        }
    }
}

fn offset_rect(rect: Rect, offset: Vec2) -> Rect {
    Rect::new(
        rect.left + offset.x,
        rect.top + offset.y,
        rect.right + offset.x,
        rect.bottom + offset.y,
    )
}

fn clip_quad(screen: Rect, uv: Rect, clip: Rect) -> Option<(Rect, Rect)> {
    let inter = Rect::new(
        clip.left.max(screen.left),
        clip.top.max(screen.top),
        clip.right.min(screen.right),
        clip.bottom.min(screen.bottom),
    );

    if inter.left >= inter.right || inter.top >= inter.bottom {
        return None;
    }

    let screen_width = screen.width();
    let screen_height = screen.height();
    if screen_width <= 0.0 || screen_height <= 0.0 {
        return None;
    }

    let u_scale = uv.width() / screen_width;
    let v_scale = uv.height() / screen_height;

    let u_offset = (inter.left - screen.left) * u_scale;
    let v_offset = (inter.top - screen.top) * v_scale;

    let clipped_uv = Rect::new(
        uv.left + u_offset,
        uv.top + v_offset,
        uv.left + u_offset + inter.width() * u_scale,
        uv.top + v_offset + inter.height() * v_scale,
    );

    Some((inter, clipped_uv))
}

fn glyph_uv_rect(font: &Font3DData, ch: char) -> Rect {
    let uv = font.char_uv(ch);
    Rect::new(uv.left, uv.top, uv.right, uv.bottom)
}

fn center_lines(
    chunks: &mut [SentenceChunk],
    chunk_line_map: &[usize],
    hotkeys: &mut [HotkeyGlyph],
    lines: &[(usize, usize, f32)],
    wrap_width: f32,
) {
    for (line_idx, &(start, end, width)) in lines.iter().enumerate() {
        if end <= start {
            continue;
        }

        let offset = (wrap_width - width).max(0.0) * 0.5;
        for (chunk, &chunk_line) in chunks.iter_mut().zip(chunk_line_map.iter()) {
            if chunk_line == line_idx {
                chunk.screen_rect.left += offset;
                chunk.screen_rect.right += offset;
            }
        }

        for glyph in hotkeys.iter_mut().filter(|g| g.line_index == line_idx) {
            glyph.screen_rect.left += offset;
            glyph.screen_rect.right += offset;
            glyph.baseline_offset.x += offset;
        }
    }
}
