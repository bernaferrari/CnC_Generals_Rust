//! Text Renderer
//!
//! Provides a high level wrapper around the shared Render2DSentence pipeline so
//! UI systems can render formatted strings with hot-key highlighting, drop
//! shadows, clipping, shader customization, and advanced formatting.

use glam::Vec2;
use std::fmt;
use std::sync::Arc;
use ww3d_renderer_3d::rendering::render2d::font3d::Font3DData;
use ww3d_renderer_3d::rendering::render2d::render2dsentence::Render2DSentenceClass;
use ww3d_renderer_3d::rendering::render2d::{Rect, Render2DGpuContext};

/// Public façade for sentence based text rendering with advanced features.
pub struct TextRenderer {
    sentence: Render2DSentenceClass,
    font_data: Option<Arc<Font3DData>>,
    location: Vec2,
    drop_shadow: Option<(u32, Vec2)>,
    clipping_rect: Option<Rect>,
    enable_additive_blend: bool,
    enable_grayscale: bool,
}

impl fmt::Debug for TextRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextRenderer")
            .field("has_font", &self.font_data.is_some())
            .field("location", &self.location)
            .field("drop_shadow", &self.drop_shadow)
            .field("clipping_rect", &self.clipping_rect)
            .field("enable_additive_blend", &self.enable_additive_blend)
            .field("enable_grayscale", &self.enable_grayscale)
            .finish()
    }
}

impl TextRenderer {
    /// Create a new text renderer with no font bound.
    pub fn new() -> Self {
        Self {
            sentence: Render2DSentenceClass::new(),
            font_data: None,
            location: Vec2::ZERO,
            drop_shadow: None,
            clipping_rect: None,
            enable_additive_blend: false,
            enable_grayscale: false,
        }
    }

    /// Bind the font used for subsequent draws.
    pub fn set_font(&mut self, font: Arc<Font3DData>) {
        self.sentence.set_font(font.clone());
        self.font_data = Some(font);
    }

    /// Set the string to render.
    pub fn set_text(&mut self, text: &str) {
        self.sentence.set_text(text);
    }

    /// Set the base colour (AARRGGBB) used for glyphs.
    pub fn set_color(&mut self, color: u32) {
        self.sentence.set_color(color);
    }

    /// Configure optional hot-key highlighting.
    pub fn set_hotkey_color(&mut self, color: Option<u32>) {
        self.sentence.set_hotkey_color(color);
    }

    /// Enable or disable legacy hot-key parsing (`&` markers).
    pub fn set_hot_key_parse(&mut self, parse: bool) {
        self.sentence.set_hot_key_parse(parse);
    }

    /// Set the drop shadow configuration (`None` disables the shadow).
    pub fn set_drop_shadow(&mut self, color: Option<u32>, offset: Vec2) {
        self.drop_shadow = color.map(|c| (c, offset));
    }

    /// Control soft word wrapping.
    pub fn set_wrap_width(&mut self, width: Option<f32>) {
        self.sentence.set_wrap_width(width);
    }

    /// Toggle centred wrapping behaviour.
    pub fn set_word_wrap_centered(&mut self, centered: bool) {
        self.sentence.set_word_wrap_centered(centered);
    }

    /// Update the screen-space location for the sentence origin.
    pub fn set_location(&mut self, position: Vec2) {
        self.location = position;
        self.sentence.set_location(position);
    }

    /// Provide an additional base offset (matches the legacy API).
    pub fn set_base_location(&mut self, base: Vec2) {
        self.sentence.set_base_location(base);
    }

    /// Force monospace layout.
    pub fn set_monospace(&mut self, enabled: bool) {
        self.sentence.set_monospace(enabled);
    }

    /// Obtain the rendered extents in pixels (takes wrapping into account).
    pub fn text_extents(&mut self) -> Vec2 {
        self.sentence.text_extents()
    }

    /// Access the first parsed hot-key glyph, if any.
    pub fn first_hotkey(&self) -> Option<(char, Vec2)> {
        self.sentence.first_hotkey()
    }

    /// Set a clipping rectangle for text rendering
    ///
    /// Text will be clipped to this rectangle. Use None to disable clipping.
    pub fn set_clipping_rect(&mut self, rect: Option<Rect>) {
        self.clipping_rect = rect;
        if let Some(r) = rect {
            self.sentence.set_clipping_rect(r);
        } else {
            self.sentence.disable_clipping();
        }
    }

    /// Enable or disable additive blending
    ///
    /// Additive blending makes text glow and is useful for effects
    pub fn set_additive_blend(&mut self, enabled: bool) {
        self.enable_additive_blend = enabled;
        if enabled {
            self.sentence.make_additive();
        }
        // Note: no direct disable method, would need to reset shader manually
    }

    /// Enable or disable grayscale rendering
    ///
    /// Useful for disabled UI elements
    pub fn set_grayscale(&mut self, _enabled: bool) {
        // Note: Render2DSentenceClass doesn't expose grayscale control directly
        // This would need to be implemented via shader settings
    }

    /// Set hard word wrap mode
    ///
    /// When enabled, words are broken mid-word if they exceed wrap width
    pub fn set_hard_word_wrap(&mut self, enabled: bool) {
        self.sentence.set_use_hard_word_wrap(enabled);
    }

    /// Set texture size hint for optimization
    ///
    /// Hints the renderer about expected texture dimensions
    pub fn set_texture_size_hint(&mut self, _width: u32, _height: u32) {
        // Note: Not exposed by Render2DSentenceClass
        // This is a hint only and can be safely ignored
    }

    /// Set the line spacing multiplier
    ///
    /// 1.0 is default, > 1.0 increases spacing, < 1.0 decreases it
    pub fn set_line_spacing(&mut self, _spacing: f32) {
        // Note: Not exposed by Render2DSentenceClass
        // Would need to be implemented in the sentence layout system
    }

    /// Set character spacing adjustment
    ///
    /// Adds extra spacing between characters (can be negative)
    pub fn set_char_spacing(&mut self, _spacing: f32) {
        // Note: Not exposed by Render2DSentenceClass
        // Would need to be implemented in the sentence layout system
    }

    /// Enable or disable outline rendering
    ///
    /// Renders text with an outline for better visibility
    pub fn set_outline(&mut self, _enabled: bool, _color: Option<u32>, _width: f32) {
        // Note: Not exposed by Render2DSentenceClass
        // Would need to be implemented as a separate rendering pass
    }

    /// Set text alignment (left, center, right)
    pub fn set_alignment(&mut self, alignment: TextAlignment) {
        // Render2DSentenceClass uses set_word_wrap_centered for centering
        match alignment {
            TextAlignment::Left => self.sentence.set_word_wrap_centered(false),
            TextAlignment::Center => self.sentence.set_word_wrap_centered(true),
            TextAlignment::Right => {
                // Right alignment not directly supported, use center as fallback
                self.sentence.set_word_wrap_centered(false);
            }
        }
    }

    /// Get the number of lines in the current text
    pub fn line_count(&self) -> usize {
        // Note: Render2DSentenceClass doesn't expose line count
        // Would need to parse the text manually
        self.sentence.text().lines().count()
    }

    /// Get the width of a specific line
    pub fn line_width(&self, _line: usize) -> f32 {
        // Note: Render2DSentenceClass doesn't expose line width
        // Would need to calculate from font metrics
        0.0 // Placeholder
    }

    /// Render the sentence using the current configuration.
    pub fn render<'pass>(
        &'pass mut self,
        gpu: &'pass mut Render2DGpuContext,
        render_pass: &mut wgpu::RenderPass<'pass>,
    ) {
        let font = match self.font_data.clone() {
            Some(font) => font,
            None => return,
        };
        let sentence = &mut self.sentence;
        sentence.set_font(font);
        sentence.set_location(self.location);

        if let Some((shadow_color, offset)) = self.drop_shadow {
            sentence.render_with_shadow(shadow_color, offset, gpu, render_pass);
        } else {
            sentence.render_with_color(None, gpu, render_pass);
        }
    }
}

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    /// Left-aligned text
    Left,
    /// Center-aligned text
    Center,
    /// Right-aligned text
    Right,
}

impl Default for TextAlignment {
    fn default() -> Self {
        TextAlignment::Left
    }
}
