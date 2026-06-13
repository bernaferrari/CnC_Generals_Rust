//! INI parsing for Mouse and MouseCursor definitions
//!
//! This module handles parsing Mouse and MouseCursor blocks from INI files.
//! Mouse settings control tooltip appearance, cursor modes, and drag tolerance.
//! MouseCursor definitions configure individual cursor visuals including
//! textures, models, hotspots, and animation settings.
//!
//! C++ Reference: GeneralsMD/Code/GameEngine/Include/GameClient/Mouse.h
//! C++ Parser: GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp
//!
//! Rust port: 2025

use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of cursor animation frames
pub const MAX_2D_CURSOR_ANIM_FRAMES: usize = 21;

/// Maximum number of cursor directions (for directional cursors)
pub const MAX_2D_CURSOR_DIRECTIONS: usize = 8;

// ============================================================================
// Types
// ============================================================================

/// Integer coordinate 2D for hotspot positions
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// RGBA color as integer (matches C++ RGBAColorInt)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RGBAColorInt {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RGBAColorInt {
    pub fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Create from packed ARGB u32 value
    pub fn from_packed_argb(packed: u32) -> Self {
        Self {
            alpha: ((packed >> 24) & 0xff) as u8,
            red: ((packed >> 16) & 0xff) as u8,
            green: ((packed >> 8) & 0xff) as u8,
            blue: (packed & 0xff) as u8,
        }
    }

    /// Convert to packed ARGB u32 value
    pub fn to_packed_argb(self) -> u32 {
        ((self.alpha as u32) << 24)
            | ((self.red as u32) << 16)
            | ((self.green as u32) << 8)
            | (self.blue as u32)
    }
}

/// Cursor redraw mode (matches C++ Mouse::RedrawMode)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum RedrawMode {
    #[default]
    Windows = 0, // Default Windows cursor - very fast
    W3D = 1,     // W3D model tied to frame rate
    Polygon = 2, // Alpha blended polygon tied to frame rate
    DX8 = 3,     // Hardware cursor independent of frame rate
}

impl RedrawMode {
    /// Parse from integer value
    pub fn from_i32(value: i32) -> INIResult<Self> {
        match value {
            0 => Ok(RedrawMode::Windows),
            1 => Ok(RedrawMode::W3D),
            2 => Ok(RedrawMode::Polygon),
            3 => Ok(RedrawMode::DX8),
            _ => Err(INIError::InvalidData),
        }
    }

    /// Convert to string name (matches C++ RedrawModeName array)
    pub fn to_str(self) -> &'static str {
        match self {
            RedrawMode::Windows => "Mouse:Windows",
            RedrawMode::W3D => "Mouse:W3D",
            RedrawMode::Polygon => "Mouse:Poly",
            RedrawMode::DX8 => "Mouse:DX8",
        }
    }
}

/// Cursor information structure
///
/// Contains all the data for a single mouse cursor, including
/// visual appearance, animation, and text display settings.
/// Matches C++ CursorInfo class from Mouse.h
#[derive(Debug, Clone)]
pub struct CursorInfo {
    /// Cursor name identifier
    pub cursor_name: String,
    /// Localized text to display with cursor
    pub cursor_text: String,
    /// Color of cursor text
    pub cursor_text_color: RGBAColorInt,
    /// Color of cursor text drop shadow
    pub cursor_text_drop_color: RGBAColorInt,
    /// Texture file name for 2D cursor
    pub texture_name: String,
    /// Mapped image name for cursor
    pub image_name: String,
    /// W3D model name for 3D cursor
    pub w3d_model_name: String,
    /// W3D animation name
    pub w3d_anim_name: String,
    /// Scale factor for W3D cursor
    pub w3d_scale: f32,
    /// Whether animation should loop
    pub loop_animation: bool,
    /// Hotspot position (cursor click point)
    pub hot_spot: ICoord2D,
    /// Number of animation frames
    pub num_frames: i32,
    /// Frames per millisecond
    pub fps: f32,
    /// Number of directions (for directional cursors like scrolling)
    pub num_directions: i32,
}

impl Default for CursorInfo {
    fn default() -> Self {
        Self {
            cursor_name: String::new(),
            cursor_text: String::new(),
            cursor_text_color: RGBAColorInt::default(),
            cursor_text_drop_color: RGBAColorInt::default(),
            texture_name: String::new(),
            image_name: String::new(),
            w3d_model_name: String::new(),
            w3d_anim_name: String::new(),
            w3d_scale: 1.0,
            loop_animation: true,
            // Assume hotspot is at center of 32x32 image (C++ default)
            hot_spot: ICoord2D::new(16, 16),
            num_frames: 1,
            fps: 20.0,
            num_directions: 1,
        }
    }
}

impl CursorInfo {
    /// Create a new CursorInfo with default values
    pub fn new() -> Self {
        Self::default()
    }
}

/// Mouse settings structure
///
/// Contains global mouse configuration including tooltip settings,
/// cursor mode, and drag tolerance. Matches the tooltip-related
/// fields from the C++ Mouse class.
#[derive(Debug, Clone)]
pub struct MouseSettings {
    // Tooltip font settings
    pub tooltip_font_name: String,
    pub tooltip_font_size: i32,
    pub tooltip_font_is_bold: bool,

    // Tooltip animation settings
    pub tooltip_animate_background: bool,
    pub tooltip_fill_time: i32,  // milliseconds to animate tooltip
    pub tooltip_delay_time: i32, // milliseconds to wait before showing tooltip

    // Tooltip colors
    pub tooltip_color_text: RGBAColorInt,
    pub tooltip_color_highlight: RGBAColorInt,
    pub tooltip_color_shadow: RGBAColorInt,
    pub tooltip_color_background: RGBAColorInt,
    pub tooltip_color_border: RGBAColorInt,

    // Tooltip width (percentage of screen width)
    pub tooltip_width: f32,

    // Cursor mode
    pub cursor_mode: RedrawMode,

    // Tooltip house color settings
    pub use_tooltip_alt_text_color: bool,
    pub use_tooltip_alt_back_color: bool,
    pub adjust_tooltip_alt_color: bool,

    // 3D cursor camera settings
    pub ortho_camera: bool,
    pub ortho_zoom: f32,

    // Drag tolerance settings
    pub drag_tolerance: u32,
    pub drag_tolerance_3d: u32,
    pub drag_tolerance_ms: u32,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self {
            // Default font: Times New Roman 12pt
            tooltip_font_name: "Times New Roman".to_string(),
            tooltip_font_size: 12,
            tooltip_font_is_bold: false,

            // Default animation settings
            tooltip_animate_background: true,
            tooltip_fill_time: 50,
            tooltip_delay_time: 50,

            // Default tooltip colors (matches C++ Mouse constructor)
            tooltip_color_text: RGBAColorInt::new(220, 220, 220, 255),
            tooltip_color_highlight: RGBAColorInt::new(255, 255, 0, 255),
            tooltip_color_shadow: RGBAColorInt::new(0, 0, 0, 255),
            tooltip_color_background: RGBAColorInt::new(20, 20, 0, 127),
            tooltip_color_border: RGBAColorInt::new(0, 0, 0, 255),

            // Default tooltip width: 15% of screen
            tooltip_width: 15.0,

            // Default cursor mode: W3D
            cursor_mode: RedrawMode::W3D,

            // Default house color settings
            use_tooltip_alt_text_color: false,
            use_tooltip_alt_back_color: false,
            adjust_tooltip_alt_color: false,

            // Default 3D cursor settings
            ortho_camera: false,
            ortho_zoom: 1.0,

            // Default drag tolerance (zero = use defaults)
            drag_tolerance: 0,
            drag_tolerance_3d: 0,
            drag_tolerance_ms: 0,
        }
    }
}

impl MouseSettings {
    /// Create new MouseSettings with default values
    pub fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Global storage
// ============================================================================

/// Global mouse settings singleton
static MOUSE_SETTINGS: OnceCell<RwLock<MouseSettings>> = OnceCell::new();

/// Global cursor info collection
static CURSOR_INFO_MAP: OnceCell<RwLock<HashMap<String, CursorInfo>>> = OnceCell::new();

/// Initialize the global mouse settings
pub fn init_global_mouse_settings() {
    let _ = MOUSE_SETTINGS.get_or_init(|| RwLock::new(MouseSettings::new()));
    let _ = CURSOR_INFO_MAP.get_or_init(|| RwLock::new(HashMap::new()));
}

/// Get a read reference to the global mouse settings
pub fn get_mouse_settings() -> Option<parking_lot::RwLockReadGuard<'static, MouseSettings>> {
    MOUSE_SETTINGS.get().map(|lock| lock.read())
}

/// Get a write reference to the global mouse settings
pub fn get_mouse_settings_mut() -> Option<parking_lot::RwLockWriteGuard<'static, MouseSettings>> {
    MOUSE_SETTINGS.get().map(|lock| lock.write())
}

/// Get cursor info by name
pub fn get_cursor_info(
    name: &str,
) -> Option<parking_lot::RwLockReadGuard<'static, HashMap<String, CursorInfo>>> {
    let guard = CURSOR_INFO_MAP.get()?.read();
    // Return the guard directly - caller can look up the cursor
    drop(guard);
    CURSOR_INFO_MAP.get().map(|lock| lock.read())
}

/// Add or update cursor info
pub fn add_cursor_info(name: String, info: CursorInfo) {
    if let Some(map) = CURSOR_INFO_MAP.get() {
        map.write().insert(name, info);
    }
}

// ============================================================================
// Parsing helper functions
// ============================================================================

/// Parse an ICoord2D from tokens (format: X:value Y:value or X:value Y:value)
fn parse_icoord2d(args: &[&str]) -> INIResult<ICoord2D> {
    let mut index = 0;
    let x = parse_cpp_labeled_i32(args, &mut index, "X")?;
    let y = parse_cpp_labeled_i32(args, &mut index, "Y")?;
    if index != args.len() {
        return Err(INIError::InvalidData);
    }

    Ok(ICoord2D::new(x, y))
}

/// Parse an RGBAColorInt from tokens
/// Matches C++ INI::parseRGBAColorInt: R:value G:value B:value [A:value].
fn parse_rgba_color_int(args: &[&str]) -> INIResult<RGBAColorInt> {
    let mut index = 0;
    let r = parse_cpp_labeled_u8(args, &mut index, "R")?;
    let g = parse_cpp_labeled_u8(args, &mut index, "G")?;
    let b = parse_cpp_labeled_u8(args, &mut index, "B")?;
    let a = if index < args.len() {
        parse_cpp_labeled_u8(args, &mut index, "A")?
    } else {
        255
    };
    if index != args.len() {
        return Err(INIError::InvalidData);
    }

    Ok(RGBAColorInt::new(r, g, b, a))
}

/// Parse percent to real (0-100 -> 0.0-1.0)
fn parse_percent_to_real(token: &str) -> INIResult<f32> {
    let value: f32 = token
        .trim_end_matches('%')
        .parse()
        .map_err(|_| INIError::InvalidData)?;
    Ok(value / 100.0)
}

fn parse_cpp_yes_no_bool(token: &str) -> INIResult<bool> {
    match token.to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_cpp_labeled_i32(args: &[&str], index: &mut usize, expected: &str) -> INIResult<i32> {
    let value = parse_cpp_labeled_value(args, index, expected)?;
    INI::parse_int(value)
}

fn parse_cpp_labeled_u8(args: &[&str], index: &mut usize, expected: &str) -> INIResult<u8> {
    let value = parse_cpp_labeled_value(args, index, expected)?;
    INI::parse_unsigned_byte(value)
}

fn parse_cpp_labeled_value<'a>(
    args: &'a [&'a str],
    index: &mut usize,
    expected: &str,
) -> INIResult<&'a str> {
    let token = args.get(*index).ok_or(INIError::InvalidData)?;
    *index += 1;

    let (key, value) = token.split_once(':').ok_or(INIError::InvalidData)?;
    if !key.eq_ignore_ascii_case(expected) {
        return Err(INIError::InvalidData);
    }
    if !value.is_empty() {
        return Ok(value);
    }

    let value = args.get(*index).ok_or(INIError::InvalidData)?;
    *index += 1;
    Ok(value)
}

// ============================================================================
// MouseSettings field parse functions
// ============================================================================

fn parse_tooltip_font_name(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.tooltip_font_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_tooltip_font_size(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.tooltip_font_size = INI::parse_int(token)?;
    Ok(())
}

fn parse_tooltip_font_is_bold(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.tooltip_font_is_bold = parse_cpp_yes_no_bool(token)?;
    Ok(())
}

fn parse_tooltip_animate_background(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.tooltip_animate_background = parse_cpp_yes_no_bool(token)?;
    Ok(())
}

fn parse_tooltip_fill_time(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.tooltip_fill_time = INI::parse_int(token)?;
    Ok(())
}

fn parse_tooltip_delay_time(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.tooltip_delay_time = INI::parse_int(token)?;
    Ok(())
}

fn parse_tooltip_text_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    settings.tooltip_color_text = parse_rgba_color_int(args)?;
    Ok(())
}

fn parse_tooltip_highlight_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    settings.tooltip_color_highlight = parse_rgba_color_int(args)?;
    Ok(())
}

fn parse_tooltip_shadow_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    settings.tooltip_color_shadow = parse_rgba_color_int(args)?;
    Ok(())
}

fn parse_tooltip_background_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    settings.tooltip_color_background = parse_rgba_color_int(args)?;
    Ok(())
}

fn parse_tooltip_border_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    settings.tooltip_color_border = parse_rgba_color_int(args)?;
    Ok(())
}

fn parse_tooltip_width(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.tooltip_width = parse_percent_to_real(token)?;
    Ok(())
}

fn parse_cursor_mode(_ini: &mut INI, settings: &mut MouseSettings, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    let value = INI::parse_int(token)?;
    settings.cursor_mode = RedrawMode::from_i32(value)?;
    Ok(())
}

fn parse_use_tooltip_alt_text_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.use_tooltip_alt_text_color = parse_cpp_yes_no_bool(token)?;
    Ok(())
}

fn parse_use_tooltip_alt_back_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.use_tooltip_alt_back_color = parse_cpp_yes_no_bool(token)?;
    Ok(())
}

fn parse_adjust_tooltip_alt_color(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.adjust_tooltip_alt_color = parse_cpp_yes_no_bool(token)?;
    Ok(())
}

fn parse_ortho_camera(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.ortho_camera = parse_cpp_yes_no_bool(token)?;
    Ok(())
}

fn parse_ortho_zoom(_ini: &mut INI, settings: &mut MouseSettings, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.ortho_zoom = INI::parse_real(token)?;
    Ok(())
}

fn parse_drag_tolerance(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.drag_tolerance = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_drag_tolerance_3d(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.drag_tolerance_3d = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_drag_tolerance_ms(
    _ini: &mut INI,
    settings: &mut MouseSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.drag_tolerance_ms = INI::parse_unsigned_int(token)?;
    Ok(())
}

// ============================================================================
// MouseSettings field parse table
// ============================================================================

/// Field parse table for MouseSettings (matches C++ TheMouseFieldParseTable)
pub const MOUSE_SETTINGS_FIELD_PARSE_TABLE: &[FieldParse<MouseSettings>] = &[
    FieldParse {
        token: "TooltipFontName",
        parse: parse_tooltip_font_name,
    },
    FieldParse {
        token: "TooltipFontSize",
        parse: parse_tooltip_font_size,
    },
    FieldParse {
        token: "TooltipFontIsBold",
        parse: parse_tooltip_font_is_bold,
    },
    FieldParse {
        token: "TooltipAnimateBackground",
        parse: parse_tooltip_animate_background,
    },
    FieldParse {
        token: "TooltipFillTime",
        parse: parse_tooltip_fill_time,
    },
    FieldParse {
        token: "TooltipDelayTime",
        parse: parse_tooltip_delay_time,
    },
    FieldParse {
        token: "TooltipTextColor",
        parse: parse_tooltip_text_color,
    },
    FieldParse {
        token: "TooltipHighlightColor",
        parse: parse_tooltip_highlight_color,
    },
    FieldParse {
        token: "TooltipShadowColor",
        parse: parse_tooltip_shadow_color,
    },
    FieldParse {
        token: "TooltipBackgroundColor",
        parse: parse_tooltip_background_color,
    },
    FieldParse {
        token: "TooltipBorderColor",
        parse: parse_tooltip_border_color,
    },
    FieldParse {
        token: "TooltipWidth",
        parse: parse_tooltip_width,
    },
    FieldParse {
        token: "CursorMode",
        parse: parse_cursor_mode,
    },
    FieldParse {
        token: "UseTooltipAltTextColor",
        parse: parse_use_tooltip_alt_text_color,
    },
    FieldParse {
        token: "UseTooltipAltBackColor",
        parse: parse_use_tooltip_alt_back_color,
    },
    FieldParse {
        token: "AdjustTooltipAltColor",
        parse: parse_adjust_tooltip_alt_color,
    },
    FieldParse {
        token: "OrthoCamera",
        parse: parse_ortho_camera,
    },
    FieldParse {
        token: "OrthoZoom",
        parse: parse_ortho_zoom,
    },
    FieldParse {
        token: "DragTolerance",
        parse: parse_drag_tolerance,
    },
    FieldParse {
        token: "DragTolerance3D",
        parse: parse_drag_tolerance_3d,
    },
    FieldParse {
        token: "DragToleranceMS",
        parse: parse_drag_tolerance_ms,
    },
];

// ============================================================================
// CursorInfo field parse functions
// ============================================================================

fn parse_cursor_text(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.cursor_text = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_cursor_text_color(
    _ini: &mut INI,
    cursor: &mut CursorInfo,
    args: &[&str],
) -> INIResult<()> {
    cursor.cursor_text_color = parse_rgba_color_int(args)?;
    Ok(())
}

fn parse_cursor_text_drop_color(
    _ini: &mut INI,
    cursor: &mut CursorInfo,
    args: &[&str],
) -> INIResult<()> {
    cursor.cursor_text_drop_color = parse_rgba_color_int(args)?;
    Ok(())
}

fn parse_w3d_model(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.w3d_model_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_w3d_anim(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.w3d_anim_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_w3d_scale(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.w3d_scale = INI::parse_real(token)?;
    Ok(())
}

fn parse_loop(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.loop_animation = parse_cpp_yes_no_bool(token)?;
    Ok(())
}

fn parse_image(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.image_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_texture(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.texture_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_hot_spot(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    cursor.hot_spot = parse_icoord2d(args)?;
    Ok(())
}

fn parse_frames(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.num_frames = INI::parse_int(token)?;
    Ok(())
}

fn parse_fps(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.fps = INI::parse_real(token)?;
    Ok(())
}

fn parse_directions(_ini: &mut INI, cursor: &mut CursorInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    cursor.num_directions = INI::parse_int(token)?;
    Ok(())
}

// ============================================================================
// CursorInfo field parse table
// ============================================================================

/// Field parse table for CursorInfo (matches C++ TheMouseCursorFieldParseTable)
pub const CURSOR_INFO_FIELD_PARSE_TABLE: &[FieldParse<CursorInfo>] = &[
    FieldParse {
        token: "CursorText",
        parse: parse_cursor_text,
    },
    FieldParse {
        token: "CursorTextColor",
        parse: parse_cursor_text_color,
    },
    FieldParse {
        token: "CursorTextDropColor",
        parse: parse_cursor_text_drop_color,
    },
    FieldParse {
        token: "W3DModel",
        parse: parse_w3d_model,
    },
    FieldParse {
        token: "W3DAnim",
        parse: parse_w3d_anim,
    },
    FieldParse {
        token: "W3DScale",
        parse: parse_w3d_scale,
    },
    FieldParse {
        token: "Loop",
        parse: parse_loop,
    },
    FieldParse {
        token: "Image",
        parse: parse_image,
    },
    FieldParse {
        token: "Texture",
        parse: parse_texture,
    },
    FieldParse {
        token: "HotSpot",
        parse: parse_hot_spot,
    },
    FieldParse {
        token: "Frames",
        parse: parse_frames,
    },
    FieldParse {
        token: "FPS",
        parse: parse_fps,
    },
    FieldParse {
        token: "Directions",
        parse: parse_directions,
    },
];

// ============================================================================
// Block parser functions
// ============================================================================

/// Parse a Mouse block definition
///
/// Parses global mouse settings including tooltip configuration,
/// cursor mode, and drag tolerance settings.
pub fn parse_mouse_definition(ini: &mut INI) -> INIResult<()> {
    // Ensure storage is initialized
    init_global_mouse_settings();

    // Get mutable settings
    let mut settings = get_mouse_settings_mut().ok_or(INIError::InvalidData)?;

    // Parse fields from INI
    ini.init_from_ini_with_fields_allow_unknown(&mut *settings, MOUSE_SETTINGS_FIELD_PARSE_TABLE)?;

    Ok(())
}

/// Parse a MouseCursor block definition
///
/// Parses an individual cursor definition by name and stores it
/// in the cursor info map.
pub fn parse_mouse_cursor_definition(ini: &mut INI) -> INIResult<()> {
    // Ensure storage is initialized
    init_global_mouse_settings();

    // Get cursor name from the line
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    // Create a new cursor info with default values
    let mut cursor = CursorInfo::new();
    cursor.cursor_name = name.clone();

    // Parse fields from INI
    ini.init_from_ini_with_fields_allow_unknown(&mut cursor, CURSOR_INFO_FIELD_PARSE_TABLE)?;

    // Store the cursor info
    add_cursor_info(name, cursor);

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_color_int_default() {
        let color = RGBAColorInt::default();
        assert_eq!(color.red, 0);
        assert_eq!(color.green, 0);
        assert_eq!(color.blue, 0);
        assert_eq!(color.alpha, 0);
    }

    #[test]
    fn test_rgba_color_int_packed() {
        // ARGB: 0x80FF0000 = 128 alpha, 255 red, 0 green, 0 blue
        let packed = 0x80FF0000u32;
        let color = RGBAColorInt::from_packed_argb(packed);
        assert_eq!(color.alpha, 128);
        assert_eq!(color.red, 255);
        assert_eq!(color.green, 0);
        assert_eq!(color.blue, 0);

        // Round-trip
        let repacked = color.to_packed_argb();
        assert_eq!(packed, repacked);
    }

    #[test]
    fn test_icoord2d_default() {
        let coord = ICoord2D::default();
        assert_eq!(coord.x, 0);
        assert_eq!(coord.y, 0);
    }

    #[test]
    fn test_icoord2d_new() {
        let coord = ICoord2D::new(16, 32);
        assert_eq!(coord.x, 16);
        assert_eq!(coord.y, 32);
    }

    #[test]
    fn test_redraw_mode_default() {
        assert_eq!(RedrawMode::default(), RedrawMode::Windows);
    }

    #[test]
    fn test_redraw_mode_from_i32() {
        assert_eq!(RedrawMode::from_i32(0).unwrap(), RedrawMode::Windows);
        assert_eq!(RedrawMode::from_i32(1).unwrap(), RedrawMode::W3D);
        assert_eq!(RedrawMode::from_i32(2).unwrap(), RedrawMode::Polygon);
        assert_eq!(RedrawMode::from_i32(3).unwrap(), RedrawMode::DX8);
        assert!(RedrawMode::from_i32(4).is_err());
    }

    #[test]
    fn test_cursor_info_default() {
        let cursor = CursorInfo::default();
        assert!(cursor.cursor_name.is_empty());
        assert!(cursor.cursor_text.is_empty());
        assert_eq!(cursor.w3d_scale, 1.0);
        assert!(cursor.loop_animation);
        assert_eq!(cursor.hot_spot.x, 16);
        assert_eq!(cursor.hot_spot.y, 16);
        assert_eq!(cursor.num_frames, 1);
        assert_eq!(cursor.fps, 20.0);
        assert_eq!(cursor.num_directions, 1);
    }

    #[test]
    fn test_mouse_settings_default() {
        let settings = MouseSettings::default();
        assert_eq!(settings.tooltip_font_name, "Times New Roman");
        assert_eq!(settings.tooltip_font_size, 12);
        assert!(!settings.tooltip_font_is_bold);
        assert!(settings.tooltip_animate_background);
        assert_eq!(settings.tooltip_fill_time, 50);
        assert_eq!(settings.tooltip_delay_time, 50);
        assert_eq!(settings.tooltip_width, 15.0);
        assert_eq!(settings.cursor_mode, RedrawMode::W3D);
        assert!(!settings.use_tooltip_alt_text_color);
        assert!(!settings.use_tooltip_alt_back_color);
        assert!(!settings.adjust_tooltip_alt_color);
        assert!(!settings.ortho_camera);
        assert_eq!(settings.ortho_zoom, 1.0);
        assert_eq!(settings.drag_tolerance, 0);
        assert_eq!(settings.drag_tolerance_3d, 0);
        assert_eq!(settings.drag_tolerance_ms, 0);
    }

    #[test]
    fn test_parse_icoord2d() {
        // Test X:val Y:val format
        let result = parse_icoord2d(&["X:10", "Y:20"]).unwrap();
        assert_eq!(result.x, 10);
        assert_eq!(result.y, 20);

        // Test separate token format
        let result = parse_icoord2d(&["X:", "10", "Y:", "30"]).unwrap();
        assert_eq!(result.x, 10);
        assert_eq!(result.y, 30);
    }

    #[test]
    fn test_parse_rgba_color_int_separate() {
        let result = parse_rgba_color_int(&["R:255", "G:128", "B:64", "A:32"]).unwrap();
        assert_eq!(result.red, 255);
        assert_eq!(result.green, 128);
        assert_eq!(result.blue, 64);
        assert_eq!(result.alpha, 32);
    }

    #[test]
    fn test_parse_rgba_color_int_omitted_alpha_defaults_to_255() {
        let result = parse_rgba_color_int(&["R:64", "G:32", "B:16"]).unwrap();
        assert_eq!(result.alpha, 255);
        assert_eq!(result.red, 64);
        assert_eq!(result.green, 32);
        assert_eq!(result.blue, 16);
    }

    #[test]
    fn test_parse_rgba_color_int_rejects_non_cpp_forms() {
        assert!(parse_rgba_color_int(&["255", "128", "64", "32"]).is_err());
        assert!(parse_rgba_color_int(&["2151698960"]).is_err());
        assert!(parse_rgba_color_int(&["G:128", "R:255", "B:64"]).is_err());
        assert!(parse_rgba_color_int(&["R:255", "G:128"]).is_err());
    }

    #[test]
    fn test_parse_icoord2d_rejects_missing_cpp_subtokens() {
        assert!(parse_icoord2d(&["X:10"]).is_err());
        assert!(parse_icoord2d(&["Y:20", "X:10"]).is_err());
        assert!(parse_icoord2d(&["X:10", "Y:20", "Z:30"]).is_err());
    }

    #[test]
    fn test_parse_percent_to_real() {
        assert_eq!(parse_percent_to_real("100").unwrap(), 1.0);
        assert_eq!(parse_percent_to_real("50%").unwrap(), 0.5);
        assert_eq!(parse_percent_to_real("25").unwrap(), 0.25);
    }

    #[test]
    fn test_mouse_bool_fields_use_cpp_yes_no_tokens() {
        init_global_mouse_settings();

        let mut ini = INI::new();
        ini.with_inline_source(
            "Mouse\nTooltipFontIsBold = Yes\nTooltipAnimateBackground = No\nUseTooltipAltTextColor = Yes\nOrthoCamera = No\nEnd\n",
            |ini| ini.parse_current_file(),
        )
        .expect("valid C++ Yes/No mouse bools should parse");

        let settings = get_mouse_settings().unwrap();
        assert!(settings.tooltip_font_is_bold);
        assert!(!settings.tooltip_animate_background);
        assert!(settings.use_tooltip_alt_text_color);
        assert!(!settings.ortho_camera);
    }

    #[test]
    fn test_mouse_bool_fields_reject_non_cpp_tokens() {
        init_global_mouse_settings();

        let mut ini = INI::new();
        let result = ini.with_inline_source("Mouse\nTooltipFontIsBold = true\nEnd\n", |ini| {
            ini.parse_current_file()
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_mouse_cursor_parses_cpp_color_hotspot_and_loop() {
        init_global_mouse_settings();

        let mut ini = INI::new();
        ini.with_inline_source(
            "MouseCursor TestCursor\nCursorTextColor = R:10 G:20 B:30 A:40\nHotSpot = X: 5 Y: 7\nLoop = No\nEnd\n",
            |ini| ini.parse_current_file(),
        )
        .expect("valid C++ mouse cursor fields should parse");

        let cursors = get_cursor_info("TestCursor").unwrap();
        let cursor = cursors.get("TestCursor").unwrap();
        assert_eq!(cursor.cursor_text_color, RGBAColorInt::new(10, 20, 30, 40));
        assert_eq!(cursor.hot_spot, ICoord2D::new(5, 7));
        assert!(!cursor.loop_animation);
    }

    #[test]
    fn test_mouse_cursor_rejects_non_cpp_color_hotspot_and_loop() {
        init_global_mouse_settings();

        let mut ini = INI::new();
        let bad_color = ini.with_inline_source(
            "MouseCursor BadColor\nCursorTextColor = 10 20 30 40\nEnd\n",
            |ini| ini.parse_current_file(),
        );
        assert!(bad_color.is_err());

        let mut ini = INI::new();
        let bad_hotspot = ini
            .with_inline_source("MouseCursor BadHotSpot\nHotSpot = X: 5\nEnd\n", |ini| {
                ini.parse_current_file()
            });
        assert!(bad_hotspot.is_err());

        let mut ini = INI::new();
        let bad_loop = ini.with_inline_source("MouseCursor BadLoop\nLoop = false\nEnd\n", |ini| {
            ini.parse_current_file()
        });
        assert!(bad_loop.is_err());
    }

    #[test]
    fn test_global_storage() {
        init_global_mouse_settings();

        let settings = get_mouse_settings().unwrap();
        assert_eq!(settings.tooltip_font_name, "Times New Roman");
        drop(settings);

        // Test cursor storage
        let cursor = CursorInfo::new();
        add_cursor_info("TestCursor".to_string(), cursor);

        let cursors = get_cursor_info("TestCursor").unwrap();
        assert!(cursors.contains_key("TestCursor"));
    }
}
