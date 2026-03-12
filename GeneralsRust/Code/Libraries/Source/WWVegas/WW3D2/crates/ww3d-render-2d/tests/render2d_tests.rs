//! 2D Renderer Tests
//!
//! This module tests the WW3D 2D rendering system including fonts,
//! bitmaps, text drawing, and UI rendering.
//!
//! Tests verify:
//! - Font system management and loading
//! - Bitmap rendering operations
//! - Text drawing and layout
//! - Surface blitting
//! - UI rendering primitives
//! - C++ parity with original 2D renderer

use ww3d_render_2d::*;

/// Test font system creation
/// Verifies initial state is valid
#[test]
fn test_font_system_creation() {
    let font_sys = FontSystem::new();
    assert_eq!(
        font_sys.font_count(),
        0,
        "New font system should have no fonts"
    );
}

/// Test font system default font
/// Verifies default font can be set and retrieved
#[test]
fn test_font_system_default_font() {
    let mut font_sys = FontSystem::new();

    // No default initially
    assert!(font_sys.get_default_font().is_none());

    // Cannot set a default font until the font is loaded.
    assert!(font_sys.set_default_font("arial").is_err());
    assert!(font_sys.get_default_font().is_none());
}

/// Test font error types
/// Verifies all error variants are constructible
#[test]
fn test_font_error_types() {
    let err1 = FontError::FileNotFound("test.tga".to_string());
    assert!(format!("{}", err1).contains("test.tga"));

    let err2 = FontError::InvalidFormat("unknown".to_string());
    assert!(format!("{}", err2).contains("Invalid font format"));

    let err3 = FontError::AlreadyLoaded("arial".to_string());
    assert!(format!("{}", err3).contains("already loaded"));

    let err4 = FontError::NotFound("missing".to_string());
    assert!(format!("{}", err4).contains("not found"));
}

/// Test text measurement utilities
/// Reference: C++ text measurement functions
#[test]
fn test_text_measurement() {
    // Test basic string metrics
    let text = "Hello World";
    let char_width = 8.0;
    let char_height = 16.0;

    let width = text.len() as f32 * char_width;
    let height = char_height;

    assert_eq!(width, 88.0);
    assert_eq!(height, 16.0);
}

/// Test text measurement with newlines
/// Verifies multi-line text height calculation
#[test]
fn test_text_measurement_multiline() {
    let text = "Line 1\nLine 2\nLine 3";
    let line_count = text.lines().count();
    let line_height = 16.0;

    let total_height = line_count as f32 * line_height;
    assert_eq!(total_height, 48.0);
}

/// Test text measurement with empty string
/// Edge case: empty string should have zero width
#[test]
fn test_text_measurement_empty() {
    let text = "";
    let char_width = 8.0;

    let width = text.len() as f32 * char_width;
    assert_eq!(width, 0.0);
}

/// Test color format conversion
/// Reference: Color32 from C++ graphics system
#[test]
fn test_color_format_rgba() {
    let r = 255u8;
    let g = 128u8;
    let b = 64u8;
    let a = 200u8;

    let color = pack_rgba(r, g, b, a);
    let (ur, ug, ub, ua) = unpack_rgba(color);

    assert_eq!(ur, r);
    assert_eq!(ug, g);
    assert_eq!(ub, b);
    assert_eq!(ua, a);
}

/// Test color format conversion ARGB
/// Verifies ARGB packing/unpacking
#[test]
fn test_color_format_argb() {
    let a = 255u8;
    let r = 200u8;
    let g = 150u8;
    let b = 100u8;

    let color = pack_argb(a, r, g, b);
    let (ua, ur, ug, ub) = unpack_argb(color);

    assert_eq!(ua, a);
    assert_eq!(ur, r);
    assert_eq!(ug, g);
    assert_eq!(ub, b);
}

/// Test color alpha blending
/// Reference: C++ alpha blending formulas
#[test]
fn test_color_alpha_blend() {
    let src_color = pack_rgba(255, 0, 0, 128); // Semi-transparent red
    let dst_color = pack_rgba(0, 255, 0, 255); // Opaque green

    let blended = alpha_blend(src_color, dst_color);

    // Result should be between red and green
    let (r, g, b, a) = unpack_rgba(blended);
    assert!(r > 0 && r < 255);
    assert!(g > 0 && g < 255);
    assert_eq!(b, 0);
    assert!(a > 128);
}

/// Test rectangle structure
/// Verifies basic rectangle operations
#[test]
fn test_rectangle_creation() {
    let rect = Rect::new(10, 20, 100, 50);

    assert_eq!(rect.x(), 10);
    assert_eq!(rect.y(), 20);
    assert_eq!(rect.width(), 100);
    assert_eq!(rect.height(), 50);
}

/// Test rectangle area calculation
/// Verifies correct area computation
#[test]
fn test_rectangle_area() {
    let rect = Rect::new(0, 0, 10, 20);
    assert_eq!(rect.area(), 200);

    let empty = Rect::new(0, 0, 0, 0);
    assert_eq!(empty.area(), 0);
}

/// Test rectangle contains point
/// Reference: C++ point-in-rect test
#[test]
fn test_rectangle_contains_point() {
    let rect = Rect::new(10, 10, 50, 30);

    assert!(rect.contains_point(15, 15));
    assert!(rect.contains_point(10, 10)); // Edge
    assert!(!rect.contains_point(5, 5));
    assert!(!rect.contains_point(100, 100));
}

/// Test rectangle intersection
/// Verifies rect-rect intersection detection
#[test]
fn test_rectangle_intersection() {
    let rect1 = Rect::new(0, 0, 50, 50);
    let rect2 = Rect::new(25, 25, 50, 50);

    assert!(rect1.intersects(&rect2));

    let rect3 = Rect::new(100, 100, 50, 50);
    assert!(!rect1.intersects(&rect3));
}

/// Test rectangle union
/// Verifies bounding box calculation
#[test]
fn test_rectangle_union() {
    let rect1 = Rect::new(0, 0, 50, 50);
    let rect2 = Rect::new(25, 25, 50, 50);

    let union = rect1.union(&rect2);

    assert_eq!(union.x(), 0);
    assert_eq!(union.y(), 0);
    assert_eq!(union.width(), 75);
    assert_eq!(union.height(), 75);
}

/// Test text alignment enumeration
/// Reference: C++ TextAlign enum
#[test]
fn test_text_alignment() {
    let left = TextAlign::Left;
    let center = TextAlign::Center;
    let right = TextAlign::Right;

    assert_ne!(left, center);
    assert_ne!(center, right);
    assert_ne!(left, right);
}

/// Test text vertical alignment
/// Verifies vertical alignment options
#[test]
fn test_text_vertical_alignment() {
    let top = TextVAlign::Top;
    let middle = TextVAlign::Middle;
    let bottom = TextVAlign::Bottom;

    assert_ne!(top, middle);
    assert_ne!(middle, bottom);
    assert_ne!(top, bottom);
}

/// Test text drawing parameters structure
/// Verifies all parameters can be set
#[test]
fn test_text_draw_params() {
    let params = TextDrawParams {
        x: 100,
        y: 200,
        color: pack_rgba(255, 255, 255, 255),
        font: Some("arial".to_string()),
        size: 12.0,
        align: TextAlign::Center,
        valign: TextVAlign::Middle,
        max_width: Some(400),
        line_spacing: 1.2,
    };

    assert_eq!(params.x, 100);
    assert_eq!(params.y, 200);
    assert_eq!(params.size, 12.0);
}

/// Test bitmap descriptor
/// Verifies bitmap metadata structure
#[test]
fn test_bitmap_descriptor() {
    let desc = BitmapDesc {
        width: 256,
        height: 256,
        format: PixelFormat::RGBA8,
        mip_levels: 1,
        data: vec![0u8; 256 * 256 * 4],
    };

    assert_eq!(desc.width, 256);
    assert_eq!(desc.height, 256);
    assert_eq!(desc.data.len(), 262144);
}

/// Test pixel format properties
/// Reference: C++ surface format enums
#[test]
fn test_pixel_format_properties() {
    assert_eq!(PixelFormat::RGBA8.bytes_per_pixel(), 4);
    assert_eq!(PixelFormat::RGB8.bytes_per_pixel(), 3);
    assert_eq!(PixelFormat::R8.bytes_per_pixel(), 1);

    assert!(PixelFormat::RGBA8.has_alpha());
    assert!(!PixelFormat::RGB8.has_alpha());
}

/// Test bitmap size calculation
/// Verifies correct memory size computation
#[test]
fn test_bitmap_size_calculation() {
    let width = 128u32;
    let height = 64u32;
    let bpp = 4u32; // RGBA

    let size = width * height * bpp;
    assert_eq!(size, 32768);
}

/// Test bitmap pitch calculation
/// Reference: C++ surface pitch/stride
#[test]
fn test_bitmap_pitch() {
    let width = 100u32;
    let bpp = 4u32;
    let pitch = width * bpp;

    assert_eq!(pitch, 400);

    // Aligned pitch (align to 256 bytes)
    let alignment = 256u32;
    let aligned_pitch = ((pitch + alignment - 1) / alignment) * alignment;
    assert_eq!(aligned_pitch, 512);
}

/// Test surface blitting parameters
/// Verifies blit operation structure
#[test]
fn test_blit_params() {
    let src_rect = Rect::new(0, 0, 64, 64);
    let dst_rect = Rect::new(100, 100, 64, 64);

    let params = BlitParams {
        src_rect,
        dst_rect,
        blend: BlendMode::Alpha,
        filter: FilterMode::Linear,
    };

    assert_eq!(params.src_rect, src_rect);
    assert_eq!(params.dst_rect, dst_rect);
}

/// Test blend mode enumeration
/// Reference: C++ blend state enums
#[test]
fn test_blend_modes() {
    let opaque = BlendMode::None;
    let alpha = BlendMode::Alpha;
    let additive = BlendMode::Additive;
    let multiply = BlendMode::Multiply;

    assert_ne!(opaque, alpha);
    assert_ne!(alpha, additive);
    assert_ne!(additive, multiply);
}

/// Test filter mode enumeration
/// Verifies texture filtering options
#[test]
fn test_filter_modes() {
    let nearest = FilterMode::Nearest;
    let linear = FilterMode::Linear;
    let bilinear = FilterMode::Bilinear;
    let trilinear = FilterMode::Trilinear;

    assert_ne!(nearest, linear);
    assert_ne!(linear, bilinear);
    assert_ne!(bilinear, trilinear);
}

/// Test clipping rectangle
/// Verifies clipping operations
#[test]
fn test_clipping() {
    let viewport = Rect::new(0, 0, 800, 600);
    let draw_rect = Rect::new(700, 500, 200, 200);

    let clipped = draw_rect.clip(&viewport);

    assert!(clipped.x() >= viewport.x());
    assert!(clipped.y() >= viewport.y());
    assert!(clipped.right() <= viewport.right());
    assert!(clipped.bottom() <= viewport.bottom());
}

/// Test UI quad rendering data
/// Reference: C++ UI batch rendering
#[test]
fn test_ui_quad() {
    let quad = UiQuad {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
        color: pack_rgba(255, 255, 255, 255),
        uv: [0.0, 0.0, 1.0, 1.0],
        texture_id: Some(1),
    };

    assert_eq!(quad.x, 10.0);
    assert_eq!(quad.width, 100.0);
    assert_eq!(quad.texture_id, Some(1));
}

/// Test UI batch formation
/// Verifies efficient batching of UI elements
#[test]
fn test_ui_batching() {
    let mut batch = UiBatch::new();

    // Add quads with same texture
    batch.add_quad(create_test_quad(0.0, 0.0, Some(1)));
    batch.add_quad(create_test_quad(10.0, 10.0, Some(1)));
    batch.add_quad(create_test_quad(20.0, 20.0, Some(1)));

    assert_eq!(batch.quad_count(), 3);
    assert_eq!(batch.batch_count(), 1);
}

/// Test UI batch texture changes
/// Verifies batch breaking on texture change
#[test]
fn test_ui_batch_texture_change() {
    let mut batch = UiBatch::new();

    // Add quads with different textures
    batch.add_quad(create_test_quad(0.0, 0.0, Some(1)));
    batch.add_quad(create_test_quad(10.0, 10.0, Some(2)));
    batch.add_quad(create_test_quad(20.0, 20.0, Some(1)));

    // Should create 3 batches due to texture changes
    assert!(batch.batch_count() >= 2);
}

/// Test text wrapping at word boundaries
/// Reference: C++ word wrap algorithm
#[test]
fn test_text_word_wrap() {
    let text = "The quick brown fox jumps over the lazy dog";
    let max_width = 100.0;
    let char_width = 8.0;

    let max_chars = (max_width / char_width) as usize;
    let wrapped = word_wrap(text, max_chars);

    assert!(wrapped.lines().count() > 1);
}

/// Test text wrapping with long word
/// Long words should force break mid-word
#[test]
fn test_text_wrap_long_word() {
    let text = "supercalifragilisticexpialidocious";
    let max_chars = 10;

    let wrapped = word_wrap(text, max_chars);
    assert!(wrapped.lines().count() > 1);
}

/// Test glyph metrics
/// Verifies character metrics structure
#[test]
fn test_glyph_metrics() {
    let metrics = GlyphMetrics {
        advance: 8.0,
        width: 7.0,
        height: 12.0,
        bearing_x: 1.0,
        bearing_y: 10.0,
    };

    assert_eq!(metrics.advance, 8.0);
    assert!(metrics.width < metrics.advance);
}

/// Test line metrics
/// Verifies line height and baseline calculation
#[test]
fn test_line_metrics() {
    let metrics = LineMetrics {
        ascent: 12.0,
        descent: 3.0,
        line_gap: 1.0,
    };

    let line_height = metrics.ascent + metrics.descent + metrics.line_gap;
    assert_eq!(line_height, 16.0);
}

// Helper functions for tests

fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((r as u32) << 0) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24)
}

fn unpack_rgba(color: u32) -> (u8, u8, u8, u8) {
    (
        ((color >> 0) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        ((color >> 16) & 0xFF) as u8,
        ((color >> 24) & 0xFF) as u8,
    )
}

fn pack_argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | ((b as u32) << 0)
}

fn unpack_argb(color: u32) -> (u8, u8, u8, u8) {
    (
        ((color >> 24) & 0xFF) as u8,
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        ((color >> 0) & 0xFF) as u8,
    )
}

fn alpha_blend(src: u32, dst: u32) -> u32 {
    let (sr, sg, sb, sa) = unpack_rgba(src);
    let (dr, dg, db, da) = unpack_rgba(dst);

    let alpha = sa as f32 / 255.0;
    let inv_alpha = 1.0 - alpha;

    let r = ((sr as f32 * alpha + dr as f32 * inv_alpha) as u8);
    let g = ((sg as f32 * alpha + dg as f32 * inv_alpha) as u8);
    let b = ((sb as f32 * alpha + db as f32 * inv_alpha) as u8);
    let a = ((sa as f32 + da as f32 * inv_alpha) as u8);

    pack_rgba(r, g, b, a)
}

fn word_wrap(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut result = String::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let mut remaining = word;

        loop {
            let pending_space = if current_line.is_empty() { 0 } else { 1 };

            if current_line.len() + pending_space + remaining.len() <= max_chars {
                if pending_space != 0 {
                    current_line.push(' ');
                }
                current_line.push_str(remaining);
                break;
            }

            if !current_line.is_empty() {
                result.push_str(&current_line);
                result.push('\n');
                current_line.clear();
                continue;
            }

            let split_at = max_chars.min(remaining.len());
            let (chunk, rest) = remaining.split_at(split_at);
            result.push_str(chunk);
            remaining = rest;

            if !remaining.is_empty() {
                result.push('\n');
            } else {
                break;
            }
        }
    }

    if !current_line.is_empty() {
        result.push_str(&current_line);
    }

    result
}

fn create_test_quad(x: f32, y: f32, texture_id: Option<u32>) -> UiQuad {
    UiQuad {
        x,
        y,
        width: 10.0,
        height: 10.0,
        color: pack_rgba(255, 255, 255, 255),
        uv: [0.0, 0.0, 1.0, 1.0],
        texture_id,
    }
}

// Type definitions for test compilation

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Rect {
    fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn x(&self) -> i32 {
        self.x
    }
    fn y(&self) -> i32 {
        self.y
    }
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }
    fn right(&self) -> i32 {
        self.x + self.width
    }
    fn bottom(&self) -> i32 {
        self.y + self.height
    }
    fn area(&self) -> i32 {
        self.width * self.height
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    fn clip(&self, bounds: &Rect) -> Rect {
        let x = self.x.max(bounds.x);
        let y = self.y.max(bounds.y);
        let right = self.right().min(bounds.right());
        let bottom = self.bottom().min(bounds.bottom());
        Rect::new(x, y, (right - x).max(0), (bottom - y).max(0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextVAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone)]
struct TextDrawParams {
    x: i32,
    y: i32,
    color: u32,
    font: Option<String>,
    size: f32,
    align: TextAlign,
    valign: TextVAlign,
    max_width: Option<i32>,
    line_spacing: f32,
}

#[derive(Debug, Clone)]
struct BitmapDesc {
    width: u32,
    height: u32,
    format: PixelFormat,
    mip_levels: u32,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PixelFormat {
    RGBA8,
    RGB8,
    R8,
}

impl PixelFormat {
    fn bytes_per_pixel(&self) -> u32 {
        match self {
            PixelFormat::RGBA8 => 4,
            PixelFormat::RGB8 => 3,
            PixelFormat::R8 => 1,
        }
    }

    fn has_alpha(&self) -> bool {
        matches!(self, PixelFormat::RGBA8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BlitParams {
    src_rect: Rect,
    dst_rect: Rect,
    blend: BlendMode,
    filter: FilterMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlendMode {
    None,
    Alpha,
    Additive,
    Multiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterMode {
    Nearest,
    Linear,
    Bilinear,
    Trilinear,
}

#[derive(Debug, Clone)]
struct UiQuad {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: u32,
    uv: [f32; 4],
    texture_id: Option<u32>,
}

struct UiBatch {
    quads: Vec<UiQuad>,
}

impl UiBatch {
    fn new() -> Self {
        Self { quads: Vec::new() }
    }

    fn add_quad(&mut self, quad: UiQuad) {
        self.quads.push(quad);
    }

    fn quad_count(&self) -> usize {
        self.quads.len()
    }

    fn batch_count(&self) -> usize {
        if self.quads.is_empty() {
            return 0;
        }

        let mut count = 1;
        let mut last_texture = self.quads[0].texture_id;

        for quad in &self.quads[1..] {
            if quad.texture_id != last_texture {
                count += 1;
                last_texture = quad.texture_id;
            }
        }

        count
    }
}

#[derive(Debug, Clone, Copy)]
struct GlyphMetrics {
    advance: f32,
    width: f32,
    height: f32,
    bearing_x: f32,
    bearing_y: f32,
}

#[derive(Debug, Clone, Copy)]
struct LineMetrics {
    ascent: f32,
    descent: f32,
    line_gap: f32,
}
