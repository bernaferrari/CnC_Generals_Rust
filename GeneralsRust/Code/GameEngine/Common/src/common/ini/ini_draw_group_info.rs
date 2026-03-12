//! INI parsing for DrawGroupInfo definitions
//!
//! This module handles parsing DrawGroupInfo entries from INI files.
//! DrawGroupInfo controls how text and graphics are drawn for UI elements.
//!
//! Author: John McDonald, October 2002
//! Rust port: 2025

use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Color representation (RGBA)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn from_u32(color: u32) -> Self {
        Self {
            a: ((color >> 24) & 0xFF) as u8,
            r: ((color >> 16) & 0xFF) as u8,
            g: ((color >> 8) & 0xFF) as u8,
            b: (color & 0xFF) as u8,
        }
    }

    pub fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    pub fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

/// Font configuration information
#[derive(Debug, Clone)]
pub struct FontInfo {
    pub name: String,
    pub size: i32,
    pub is_bold: bool,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            name: "Arial".to_string(),
            size: 12,
            is_bold: false,
        }
    }
}

/// Position offset - can be either pixels or percentage
#[derive(Debug, Clone)]
pub struct PositionOffset {
    /// The offset value
    pub value: f32,
    /// True if using pixel offset, false if using percentage
    pub using_pixel: bool,
}

impl PositionOffset {
    pub fn new_pixel(pixels: i32) -> Self {
        Self {
            value: pixels as f32,
            using_pixel: true,
        }
    }

    pub fn new_percent(percent: f32) -> Self {
        Self {
            value: percent,
            using_pixel: false,
        }
    }

    pub fn get_pixel_value(&self) -> i32 {
        if self.using_pixel {
            self.value as i32
        } else {
            0 // Would need screen dimensions to convert percentage
        }
    }

    pub fn get_percent_value(&self) -> f32 {
        if !self.using_pixel {
            self.value
        } else {
            0.0 // Would need screen dimensions to convert pixels
        }
    }
}

impl Default for PositionOffset {
    fn default() -> Self {
        Self {
            value: 0.0,
            using_pixel: true,
        }
    }
}

/// DrawGroupInfo structure for UI text and graphics drawing configuration
///
/// Contains all the information needed to draw text elements in the UI,
/// including font, colors, positioning, and drop shadow settings.
#[derive(Debug, Clone)]
pub struct DrawGroupInfo {
    /// Font information
    pub font: FontInfo,

    /// Whether to use player color for text
    pub use_player_color: bool,

    /// Color for the main text
    pub color_for_text: Color,

    /// Color for text drop shadow
    pub color_for_text_drop_shadow: Color,

    /// Drop shadow offset in X direction
    pub drop_shadow_offset_x: i32,

    /// Drop shadow offset in Y direction
    pub drop_shadow_offset_y: i32,

    /// X position offset (pixel or percentage)
    pub offset_x: PositionOffset,

    /// Y position offset (pixel or percentage)
    pub offset_y: PositionOffset,
}

impl Default for DrawGroupInfo {
    fn default() -> Self {
        Self {
            font: FontInfo::default(),
            use_player_color: false,
            color_for_text: Color::white(),
            color_for_text_drop_shadow: Color::black(),
            drop_shadow_offset_x: 1,
            drop_shadow_offset_y: 1,
            offset_x: PositionOffset::default(),
            offset_y: PositionOffset::default(),
        }
    }
}

impl DrawGroupInfo {
    /// Create a new DrawGroupInfo with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set font name
    pub fn set_font_name(&mut self, name: String) {
        self.font.name = name;
    }

    /// Set font size
    pub fn set_font_size(&mut self, size: i32) {
        self.font.size = size;
    }

    /// Set font bold flag
    pub fn set_font_bold(&mut self, is_bold: bool) {
        self.font.is_bold = is_bold;
    }

    /// Set X position in pixels
    pub fn set_draw_position_x_pixel(&mut self, pixels: i32) {
        self.offset_x = PositionOffset::new_pixel(pixels);
    }

    /// Set X position as percentage
    pub fn set_draw_position_x_percent(&mut self, percent: f32) {
        self.offset_x = PositionOffset::new_percent(percent);
    }

    /// Set Y position in pixels
    pub fn set_draw_position_y_pixel(&mut self, pixels: i32) {
        self.offset_y = PositionOffset::new_pixel(pixels);
    }

    /// Set Y position as percentage
    pub fn set_draw_position_y_percent(&mut self, percent: f32) {
        self.offset_y = PositionOffset::new_percent(percent);
    }

    /// Get effective X position in pixels (requires screen width for percentage conversion)
    pub fn get_x_pixels(&self, screen_width: i32) -> i32 {
        if self.offset_x.using_pixel {
            self.offset_x.value as i32
        } else {
            ((self.offset_x.value / 100.0) * screen_width as f32) as i32
        }
    }

    /// Get effective Y position in pixels (requires screen height for percentage conversion)
    pub fn get_y_pixels(&self, screen_height: i32) -> i32 {
        if self.offset_y.using_pixel {
            self.offset_y.value as i32
        } else {
            ((self.offset_y.value / 100.0) * screen_height as f32) as i32
        }
    }

    /// Parse from INI file.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> INIResult<()> {
        ini.init_from_ini_with_fields(self, FIELD_PARSE_TABLE)
    }
}

/// Field parsing functions (match C++ interface)
///
/// These functions handle parsing specific fields from INI files

/// Parse integer value for X pixel offset
pub fn parse_int_x(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.set_draw_position_x_pixel(INI::parse_int(value)?);
    Ok(())
}

/// Parse integer value for Y pixel offset
pub fn parse_int_y(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.set_draw_position_y_pixel(INI::parse_int(value)?);
    Ok(())
}

/// Parse percentage value for X percent offset
pub fn parse_percent_to_real_x(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.set_draw_position_x_percent(INI::parse_percent_to_real(value)?);
    Ok(())
}

/// Parse percentage value for Y percent offset
pub fn parse_percent_to_real_y(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.set_draw_position_y_percent(INI::parse_percent_to_real(value)?);
    Ok(())
}

/// Field parser definition
/// Field parse table for DrawGroupInfo (matches C++ table)
pub const FIELD_PARSE_TABLE: &[FieldParse<DrawGroupInfo>] = &[
    FieldParse {
        token: "UsePlayerColor",
        parse: parse_use_player_color,
    },
    FieldParse {
        token: "ColorForText",
        parse: parse_color_for_text,
    },
    FieldParse {
        token: "ColorForTextDropShadow",
        parse: parse_color_for_text_drop_shadow,
    },
    FieldParse {
        token: "FontName",
        parse: parse_font_name,
    },
    FieldParse {
        token: "FontSize",
        parse: parse_font_size,
    },
    FieldParse {
        token: "FontIsBold",
        parse: parse_font_is_bold,
    },
    FieldParse {
        token: "DropShadowOffsetX",
        parse: parse_drop_shadow_offset_x,
    },
    FieldParse {
        token: "DropShadowOffsetY",
        parse: parse_drop_shadow_offset_y,
    },
    FieldParse {
        token: "DrawPositionXPixel",
        parse: parse_int_x,
    },
    FieldParse {
        token: "DrawPositionXPercent",
        parse: parse_percent_to_real_x,
    },
    FieldParse {
        token: "DrawPositionYPixel",
        parse: parse_int_y,
    },
    FieldParse {
        token: "DrawPositionYPercent",
        parse: parse_percent_to_real_y,
    },
];

/// Parse boolean field for UsePlayerColor
pub fn parse_use_player_color(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.use_player_color = INI::parse_bool(value)?;
    Ok(())
}

/// Parse color field for ColorForText
pub fn parse_color_for_text(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.color_for_text = Color::from_u32(INI::parse_unsigned_int(value)?);
    Ok(())
}

/// Parse color field for ColorForTextDropShadow
pub fn parse_color_for_text_drop_shadow(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.color_for_text_drop_shadow = Color::from_u32(INI::parse_unsigned_int(value)?);
    Ok(())
}

/// Parse string field for FontName
pub fn parse_font_name(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    if args.is_empty() {
        return Err(INIError::InvalidData);
    }
    let joined = args.join(" ");
    draw_group_info.set_font_name(INI::parse_ascii_string(&joined)?);
    Ok(())
}

/// Parse integer field for FontSize
pub fn parse_font_size(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.set_font_size(INI::parse_int(value)?);
    Ok(())
}

/// Parse boolean field for FontIsBold
pub fn parse_font_is_bold(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.set_font_bold(INI::parse_bool(value)?);
    Ok(())
}

/// Parse integer field for DropShadowOffsetX
pub fn parse_drop_shadow_offset_x(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.drop_shadow_offset_x = INI::parse_int(value)?;
    Ok(())
}

/// Parse integer field for DropShadowOffsetY
pub fn parse_drop_shadow_offset_y(
    _ini: &mut INI,
    draw_group_info: &mut DrawGroupInfo,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    draw_group_info.drop_shadow_offset_y = INI::parse_int(value)?;
    Ok(())
}

/// Global DrawGroupInfo instance (thread-safe)
static DRAW_GROUP_INFO: OnceCell<Arc<RwLock<DrawGroupInfo>>> = OnceCell::new();

/// Ensure the DrawGroupInfo exists and return a handle to it
pub fn ensure_draw_group_info() -> Arc<RwLock<DrawGroupInfo>> {
    DRAW_GROUP_INFO
        .get_or_init(|| Arc::new(RwLock::new(DrawGroupInfo::new())))
        .clone()
}

/// Initialize (or reinitialize) the global DrawGroupInfo
pub fn init_global_draw_group_info() {
    let info = ensure_draw_group_info();
    *info.write() = DrawGroupInfo::new();
}

/// Get a handle to the global DrawGroupInfo if initialized
pub fn get_draw_group_info() -> Option<Arc<RwLock<DrawGroupInfo>>> {
    DRAW_GROUP_INFO.get().cloned()
}

/// INI parsing function for DrawGroupNumber definition (matches C++ interface)
///
/// This is the main entry point for parsing DrawGroupInfo definitions from INI files
pub fn parse_draw_group_number_definition(ini: &mut INI) -> INIResult<()> {
    let info_handle = ensure_draw_group_info();
    let mut draw_group_info = info_handle.write();

    // Parse using field table
    ini.init_from_ini_with_fields(&mut *draw_group_info, FIELD_PARSE_TABLE)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_group_info_creation() {
        let dgi = DrawGroupInfo::new();
        assert_eq!(dgi.font.name, "Arial");
        assert_eq!(dgi.font.size, 12);
        assert!(!dgi.font.is_bold);
        assert!(!dgi.use_player_color);
    }

    #[test]
    fn test_position_offset() {
        let pixel_offset = PositionOffset::new_pixel(100);
        assert_eq!(pixel_offset.get_pixel_value(), 100);
        assert!(pixel_offset.using_pixel);

        let percent_offset = PositionOffset::new_percent(50.0);
        assert_eq!(percent_offset.get_percent_value(), 50.0);
        assert!(!percent_offset.using_pixel);
    }

    #[test]
    fn test_draw_group_info_positioning() {
        let mut dgi = DrawGroupInfo::new();

        // Test pixel positioning
        dgi.set_draw_position_x_pixel(150);
        dgi.set_draw_position_y_pixel(200);

        assert_eq!(dgi.get_x_pixels(1000), 150);
        assert_eq!(dgi.get_y_pixels(800), 200);

        // Test percentage positioning
        dgi.set_draw_position_x_percent(25.0);
        dgi.set_draw_position_y_percent(50.0);

        assert_eq!(dgi.get_x_pixels(1000), 250);
        assert_eq!(dgi.get_y_pixels(800), 400);
    }

    #[test]
    fn test_color_creation() {
        let red = Color::new(255, 0, 0, 255);
        assert_eq!(red.r, 255);
        assert_eq!(red.g, 0);
        assert_eq!(red.b, 0);
        assert_eq!(red.a, 255);

        let blue = Color::from_rgb(0, 0, 255);
        assert_eq!(blue.b, 255);
        assert_eq!(blue.a, 255);
    }

    #[test]
    fn test_font_info() {
        let mut font = FontInfo::default();
        assert_eq!(font.name, "Arial");
        assert_eq!(font.size, 12);
        assert!(!font.is_bold);

        font.name = "Times New Roman".to_string();
        font.size = 14;
        font.is_bold = true;

        assert_eq!(font.name, "Times New Roman");
        assert_eq!(font.size, 14);
        assert!(font.is_bold);
    }

    #[test]
    fn test_global_draw_group_info() {
        init_global_draw_group_info();
        let handle = ensure_draw_group_info();

        {
            let mut dgi = handle.write();
            dgi.set_font_size(16);
        }

        let dgi = handle.read();
        assert_eq!(dgi.font.size, 16);
    }

    #[test]
    fn test_field_parse_table() {
        assert!(!FIELD_PARSE_TABLE.is_empty());

        // Check that expected fields are present
        let field_names: Vec<&str> = FIELD_PARSE_TABLE.iter().map(|f| f.token).collect();
        assert!(field_names.contains(&"UsePlayerColor"));
        assert!(field_names.contains(&"FontName"));
        assert!(field_names.contains(&"FontSize"));
        assert!(field_names.contains(&"DrawPositionXPixel"));
        assert!(field_names.contains(&"DrawPositionYPercent"));
    }
}
