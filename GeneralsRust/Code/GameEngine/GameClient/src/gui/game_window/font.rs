//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use bitflags::bitflags;
use std::cell::RefCell;
use std::rc::Weak;

use crate::gui::MAX_DRAW_DATA;
use crate::gui::display_string::DisplayStringHandle;
use crate::gui::font::FontDesc;
use crate::video_buffer::VideoBufferHandle;

use super::messages::{WIN_COLOR_UNDEFINED, WINDOW_ID_INVALID, WindowStatus};
use super::payload::WindowId;

/// 2D coordinate point
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point2D {
    pub x: i32,
    pub y: i32,
}

/// 2D region defined by two points
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowRegion {
    pub low: Point2D,
    pub high: Point2D,
}

impl WindowRegion {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            low: Point2D { x, y },
            high: Point2D {
                x: x + width,
                y: y + height,
            },
        }
    }

    pub fn width(&self) -> i32 {
        self.high.x - self.low.x
    }

    pub fn height(&self) -> i32 {
        self.high.y - self.low.y
    }

    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.low.x && x <= self.high.x && y >= self.low.y && y <= self.high.y
    }
}

/// Color type (RGBA)
pub type Color = u32;

/// Game font descriptor used for font resolution via the font library.
#[derive(Debug, Clone)]
pub struct GameFont {
    pub name: String,
    pub size: i32,
    pub bold: bool,
}

impl GameFont {
    pub(crate) fn to_font_desc(&self) -> FontDesc {
        FontDesc::new(&self.name, self.size, self.bold)
    }
}

/// Image reference used for window draw data, resolved via the mapped image collection.
#[derive(Debug, Clone)]
pub struct Image {
    pub name: String,
    pub width: i32,
    pub height: i32,
    // Image data would be here
}

/// Draw data for different window states.
/// C++ `WinInstanceData::init` sets every slot's color/border to `WIN_COLOR_UNDEFINED`.
#[derive(Debug, Clone)]
pub struct WindowDrawData {
    pub image: Option<Image>,
    pub color: Color,
    pub border_color: Color,
}

impl Default for WindowDrawData {
    fn default() -> Self {
        Self {
            image: None,
            color: WIN_COLOR_UNDEFINED,
            border_color: WIN_COLOR_UNDEFINED,
        }
    }
}

/// Text colors for different window states.
/// C++ `WinInstanceData::init` uses `WIN_COLOR_UNDEFINED` for enabled/disabled/hilite text.
#[derive(Debug, Clone)]
pub struct WindowTextColors {
    pub color: Color,
    pub border_color: Color,
}

impl Default for WindowTextColors {
    fn default() -> Self {
        Self {
            color: WIN_COLOR_UNDEFINED,
            border_color: WIN_COLOR_UNDEFINED,
        }
    }
}

/// Window state flags for visual appearance
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct WindowState: u32 {
        const NONE = 0x00000000;
        const HILITED = 0x00000002;
        const SELECTED = 0x00000004;
        const PUSHED = Self::SELECTED.bits();
        const DISABLED = 0x00000008;
    }
}

/// Window instance data containing visual and behavioral properties
#[derive(Clone)]
pub struct WindowInstanceData {
    pub id: WindowId,
    pub style: u32,
    pub state: WindowState,
    pub status: WindowStatus,
    pub text: String,
    pub text_label: String,
    pub decorated_name: String,
    pub header_template: String,
    pub tooltip: String,
    pub font: Option<GameFont>,
    pub display_text: Option<DisplayStringHandle>,
    pub display_tooltip: Option<DisplayStringHandle>,
    pub enabled_draw_data: [WindowDrawData; MAX_DRAW_DATA],
    pub disabled_draw_data: [WindowDrawData; MAX_DRAW_DATA],
    pub hilite_draw_data: [WindowDrawData; MAX_DRAW_DATA],
    pub enabled_text: WindowTextColors,
    pub disabled_text: WindowTextColors,
    pub hilite_text: WindowTextColors,
    pub ime_composite_text: WindowTextColors,
    pub image_offset: Point2D,
    pub tooltip_delay: i32,
    pub owner: Option<Weak<RefCell<super::window_struct::GameWindow>>>,
    pub video_buffer: Option<VideoBufferHandle>,
}

impl Default for WindowInstanceData {
    fn default() -> Self {
        Self {
            id: WINDOW_ID_INVALID,
            style: 0,
            state: WindowState::NONE,
            status: WindowStatus::NONE,
            text: String::new(),
            text_label: String::new(),
            decorated_name: String::new(),
            header_template: String::new(),
            tooltip: String::new(),
            font: None,
            display_text: None,
            display_tooltip: None,
            enabled_draw_data: Default::default(),
            disabled_draw_data: Default::default(),
            hilite_draw_data: Default::default(),
            enabled_text: Default::default(),
            disabled_text: Default::default(),
            hilite_text: Default::default(),
            ime_composite_text: Default::default(),
            image_offset: Point2D { x: 0, y: 0 },
            // C++ `WinInstanceData::init`: m_tooltipDelay = -1 (use Mouse.ini).
            tooltip_delay: -1,
            owner: None,
            video_buffer: None,
        }
    }
}
