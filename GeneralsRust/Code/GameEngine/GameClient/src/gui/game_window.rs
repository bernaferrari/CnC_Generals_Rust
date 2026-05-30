//! GameWindow Implementation
//!
//! This module provides the `GameWindow` struct, which represents individual UI windows
//! and controls in the game's windowing system. It handles window properties, hierarchy,
//! event callbacks, and drawing.

use bitflags::bitflags;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::OnceLock;

use glam::Vec2;
use std::sync::Arc;

use crate::display::image::{ensure_client_mapped_image, get_mapped_image_collection};
use crate::game_text::GameText;
use crate::video_buffer::{VideoBufferHandle, VideoBufferType};

use super::gadgets::{
    CheckBox, ComboBox, Gadget, GadgetMessage, GadgetState, GadgetValue, HorizontalSlider,
    InputEvent, KeyCode, KeyModifiers, ListBox, MouseButton, ProgressBar, PushButton, RadioButton,
    StaticText, TabControl, TabControlData, TextEntry, VerticalSlider,
};
use super::{
    display_string::DisplayStringHandle,
    font::{get_font_library, FontDesc},
    get_display_string_manager, with_ui_renderer_mut, with_window_manager_ref, UIRect,
    MAX_DRAW_DATA, TOOLTIP_MAX_LEN,
};
use crate::gui::window_manager::{with_window_manager, TabDirection};

/// Window ID type for uniquely identifying windows
pub type WindowId = i32;

const KEY_STATE_UP: WindowMsgData = 0x0001;
const KEY_STATE_DOWN: WindowMsgData = 0x0002;
const KEY_STATE_LCONTROL: WindowMsgData = 0x0004;
const KEY_STATE_RCONTROL: WindowMsgData = 0x0008;
const KEY_STATE_LSHIFT: WindowMsgData = 0x0010;
const KEY_STATE_RSHIFT: WindowMsgData = 0x0020;
const KEY_STATE_LALT: WindowMsgData = 0x0040;
const KEY_STATE_RALT: WindowMsgData = 0x0080;
const GADGET_SIZE: i32 = 16;

/// Window message data type
pub type WindowMsgData = u32;

/// Result type for window operations
pub type WindowResult<T> = Result<T, WindowError>;

/// Invalid window ID constant
pub const WINDOW_ID_INVALID: WindowId = 0;

/// Undefined color constant
pub const WIN_COLOR_UNDEFINED: u32 = 0xFFFFFFFF;

/// Gadget system message IDs
const GGM_LEFT_DRAG: u32 = 16384;
const GGM_FOCUS_CHANGE: u32 = GGM_LEFT_DRAG + 3;
const GGM_RESIZED: u32 = GGM_LEFT_DRAG + 4;
const GBM_SET_SELECTION: u32 = GGM_LEFT_DRAG + 10;
const GSM_SET_SLIDER: u32 = GGM_LEFT_DRAG + 12;
const GSM_SET_MIN_MAX: u32 = GGM_LEFT_DRAG + 13;
const GLM_DEL_ENTRY: u32 = GGM_LEFT_DRAG + 16;
const GLM_DEL_ALL: u32 = GGM_LEFT_DRAG + 17;
pub(crate) const GPM_SET_PROGRESS: u32 = GGM_LEFT_DRAG + 48;

// Window style flags (GWS_*)
pub const GWS_PUSH_BUTTON: u32 = 0x0000_0001;
pub const GWS_RADIO_BUTTON: u32 = 0x0000_0002;
pub const GWS_CHECK_BOX: u32 = 0x0000_0004;
pub const GWS_VERT_SLIDER: u32 = 0x0000_0008;
pub const GWS_HORZ_SLIDER: u32 = 0x0000_0010;
pub const GWS_SCROLL_LISTBOX: u32 = 0x0000_0020;
pub const GWS_ENTRY_FIELD: u32 = 0x0000_0040;
pub const GWS_STATIC_TEXT: u32 = 0x0000_0080;
pub const GWS_PROGRESS_BAR: u32 = 0x0000_0100;
pub const GWS_USER_WINDOW: u32 = 0x0000_0200;
pub const GWS_MOUSE_TRACK: u32 = 0x0000_0400;
pub const GWS_ANIMATED: u32 = 0x0000_0800;
pub const GWS_TAB_STOP: u32 = 0x0000_1000;
pub const GWS_TAB_CONTROL: u32 = 0x0000_2000;
pub const GWS_TAB_PANE: u32 = 0x0000_4000;
pub const GWS_COMBO_BOX: u32 = 0x0000_8000;
pub const GWS_ALL_SLIDER: u32 = GWS_VERT_SLIDER | GWS_HORZ_SLIDER;

const HORIZONTAL_SLIDER_THUMB_POSITION: i32 = 10;
pub const GWS_GADGET_WINDOW: u32 = GWS_PUSH_BUTTON
    | GWS_RADIO_BUTTON
    | GWS_TAB_CONTROL
    | GWS_CHECK_BOX
    | GWS_VERT_SLIDER
    | GWS_HORZ_SLIDER
    | GWS_SCROLL_LISTBOX
    | GWS_ENTRY_FIELD
    | GWS_STATIC_TEXT
    | GWS_COMBO_BOX
    | GWS_PROGRESS_BAR;

bitflags! {
    /// Window status flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct WindowStatus: u32 {
        const NONE                  = 0x00000000;
        const ACTIVE               = 0x00000001;  // At the top of the window list
        const TOGGLE               = 0x00000002;  // If set, click to toggle
        const DRAGABLE             = 0x00000004;  // Window can be dragged
        const ENABLED              = 0x00000008;  // Window can receive input
        const HIDDEN               = 0x00000010;  // Window is hidden, no input
        const ABOVE                = 0x00000020;  // Window is always above others
        const BELOW                = 0x00000040;  // Window is always below others
        const IMAGE                = 0x00000080;  // Window is drawn with images
        const TAB_STOP             = 0x00000100;  // Window is a tab stop
        const NO_INPUT             = 0x00000200;  // Window does not take input
        const NO_FOCUS             = 0x00000400;  // Window does not take focus
        const DESTROYED            = 0x00000800;  // Window has been destroyed
        const BORDER               = 0x00001000;  // Window will be drawn with borders
        const SMOOTH_TEXT          = 0x00002000;  // Window text will be drawn with smoothing
        const ONE_LINE             = 0x00004000;  // Window text will be drawn on only one line
        const NO_FLUSH             = 0x00008000;  // Window images will not be unloaded when hidden
        const SEE_THRU             = 0x00010000;  // Will not draw, but is NOT hidden
        const RIGHT_CLICK          = 0x00020000;  // Window pays attention to right clicks
        const WRAP_CENTERED        = 0x00040000;  // Text will be centered on each word wrap
        const CHECK_LIKE           = 0x00080000;  // Make push buttons "check-like" with dual state
        const HOTKEY_TEXT          = 0x00100000;  // Enable hotkey text processing
        const USE_OVERLAY_STATES   = 0x00200000;  // Use automatic rendering overlay for states
        const NOT_READY            = 0x00400000;  // A disabled button that is available but not yet ready
        const FLASHING             = 0x00800000;  // Used for buttons that do cameo flashes
        const ALWAYS_COLOR         = 0x01000000;  // Never render using greyscale when disabled
        const ON_MOUSE_DOWN        = 0x02000000;  // Pushbutton triggers on mouse down
        const SHORTCUT_BUTTON      = 0x04000000;  // Special handling for shortcut buttons
    }
}

/// Window messages that can be sent to windows
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMessage {
    None = 0,
    Create,
    Destroy,
    Activate,
    Enable,
    LeftDown,
    LeftUp,
    LeftDoubleClick,
    LeftDrag,
    MiddleDown,
    MiddleUp,
    MiddleDoubleClick,
    MiddleDrag,
    RightDown,
    RightUp,
    RightDoubleClick,
    RightDrag,
    MouseEntering,
    MouseLeaving,
    WheelUp,
    WheelDown,
    Char,
    ScriptCreate,
    InputFocus,
    MousePos,
    ImeChar,
    ImeString,
    GadgetSelected = 0x0040,
    GadgetMouseEntering = 0x0041,
    GadgetMouseLeaving = 0x0042,
    GadgetEditDone = 0x0080,
    GadgetValueChanged = 0x0081,
    GadgetRightClick = 0x0082,
    // User-defined messages start at GWM_USER (32768)
    User(u32),
}

impl From<u32> for WindowMessage {
    fn from(value: u32) -> Self {
        match value {
            0 => WindowMessage::None,
            1 => WindowMessage::Create,
            2 => WindowMessage::Destroy,
            3 => WindowMessage::Activate,
            4 => WindowMessage::Enable,
            5 => WindowMessage::LeftDown,
            6 => WindowMessage::LeftUp,
            7 => WindowMessage::LeftDoubleClick,
            8 => WindowMessage::LeftDrag,
            9 => WindowMessage::MiddleDown,
            10 => WindowMessage::MiddleUp,
            11 => WindowMessage::MiddleDoubleClick,
            12 => WindowMessage::MiddleDrag,
            13 => WindowMessage::RightDown,
            14 => WindowMessage::RightUp,
            15 => WindowMessage::RightDoubleClick,
            16 => WindowMessage::RightDrag,
            17 => WindowMessage::MouseEntering,
            18 => WindowMessage::MouseLeaving,
            19 => WindowMessage::WheelUp,
            20 => WindowMessage::WheelDown,
            21 => WindowMessage::Char,
            22 => WindowMessage::ScriptCreate,
            23 => WindowMessage::InputFocus,
            24 => WindowMessage::MousePos,
            25 => WindowMessage::ImeChar,
            26 => WindowMessage::ImeString,
            0x0040 => WindowMessage::GadgetSelected,
            0x0041 => WindowMessage::GadgetMouseEntering,
            0x0042 => WindowMessage::GadgetMouseLeaving,
            0x0080 => WindowMessage::GadgetEditDone,
            0x0081 => WindowMessage::GadgetValueChanged,
            0x0082 => WindowMessage::GadgetRightClick,
            val if val >= 32768 => WindowMessage::User(val),
            _ => WindowMessage::None,
        }
    }
}

/// Return codes for input processing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowInputReturnCode {
    NotUsed = 0,
    Used,
}

/// Message handling result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMsgHandled {
    Ignored = 0,
    Handled,
}

/// Window error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowError {
    Ok = 0,
    GeneralFailure = -1,
    InvalidWindow = -2,
    InvalidParameter = -3,
    MouseCaptured = -4,
    KeyboardCaptured = -5,
    OutOfWindows = -6,
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            WindowError::Ok => "ok",
            WindowError::GeneralFailure => "general failure",
            WindowError::InvalidWindow => "invalid window",
            WindowError::InvalidParameter => "invalid parameter",
            WindowError::MouseCaptured => "mouse captured",
            WindowError::KeyboardCaptured => "keyboard captured",
            WindowError::OutOfWindows => "out of windows",
        };
        write!(f, "WindowError: {}", message)
    }
}

impl std::error::Error for WindowError {}

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

/// Draw data for different window states
#[derive(Debug, Clone, Default)]
pub struct WindowDrawData {
    pub image: Option<Image>,
    pub color: Color,
    pub border_color: Color,
}

/// Text colors for different window states
#[derive(Debug, Clone, Default)]
pub struct WindowTextColors {
    pub color: Color,
    pub border_color: Color,
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
    pub owner: Option<Weak<RefCell<GameWindow>>>,
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
            tooltip_delay: super::TOOLTIP_DELAY,
            owner: None,
            video_buffer: None,
        }
    }
}

/// Callback function types
pub type DrawCallback = Box<dyn Fn(&GameWindow, &WindowInstanceData)>;
pub type TooltipCallback = Box<dyn Fn(&GameWindow, &WindowInstanceData, u32)>;
pub type InputCallback =
    Box<dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>;
pub type SystemCallback =
    Box<dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>;

/// Data attached to each window specifically for the GUI editor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameWindowEditData {
    pub system_callback_string: String,
    pub input_callback_string: String,
    pub tooltip_callback_string: String,
    pub draw_callback_string: String,
}

/// Window callback functions
#[derive(Default)]
pub struct WindowCallbacks {
    pub draw: Option<DrawCallback>,
    pub tooltip: Option<TooltipCallback>,
    pub input: Option<InputCallback>,
    pub system: Option<SystemCallback>,
}

/// Main GameWindow struct representing a UI window or control
pub struct GameWindow {
    // Core properties
    id: WindowId,
    status: WindowStatus,
    size: Point2D,
    region: WindowRegion,
    cursor_pos: Point2D,

    // Instance data
    inst_data: WindowInstanceData,

    // User data
    user_data: Option<Box<dyn std::any::Any>>,
    edit_data: Option<GameWindowEditData>,

    // Hierarchy
    parent: Option<Weak<RefCell<GameWindow>>>,
    children: Vec<Rc<RefCell<GameWindow>>>,
    next_sibling: Option<Weak<RefCell<GameWindow>>>,
    prev_sibling: Option<Weak<RefCell<GameWindow>>>,
    owner_is_self: bool,

    // Layout information
    next_in_layout: Option<Weak<RefCell<GameWindow>>>,
    prev_in_layout: Option<Weak<RefCell<GameWindow>>>,
    layout: Option<Weak<RefCell<super::WindowLayout>>>,

    // Callbacks
    callbacks: WindowCallbacks,

    // Optional gadget backing this window
    widget: Option<WindowWidget>,

    // Combo box child window references (drop-down, edit, list)
    combobox_links: Option<ComboBoxLinks>,

    // List box scrollbar child window references
    listbox_links: Option<ListBoxLinks>,

    // Slider thumb child window reference
    slider_thumb: Option<WindowId>,

    // Press animation state for elastic button feel
    press_scale: f32,
    press_scale_target: f32,
    press_scale_velocity: f32,
    press_spring_strength: f32,
    press_spring_damping: f32,
    press_impulse: f32,
    release_impulse: f32,
    press_was_down: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ComboBoxLinks {
    pub drop_down: WindowId,
    pub edit_box: WindowId,
    pub list_box: WindowId,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ListBoxLinks {
    pub up_button: WindowId,
    pub down_button: WindowId,
    pub slider: WindowId,
    pub thumb: Option<WindowId>,
}

/// Gadget backing types for windows created from scripts.
pub enum WindowWidget {
    PushButton(PushButton),
    RadioButton(RadioButton),
    CheckBox(CheckBox),
    VerticalSlider(VerticalSlider),
    HorizontalSlider(HorizontalSlider),
    ListBox(ListBox),
    TextEntry(TextEntry),
    StaticText(StaticText),
    ProgressBar(ProgressBar),
    TabControl(TabControl),
    ComboBox(ComboBox),
    TabPane,
    User,
    Animated,
    MouseTrack,
}

impl fmt::Debug for WindowWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::PushButton(_) => "PushButton",
            Self::RadioButton(_) => "RadioButton",
            Self::CheckBox(_) => "CheckBox",
            Self::VerticalSlider(_) => "VerticalSlider",
            Self::HorizontalSlider(_) => "HorizontalSlider",
            Self::ListBox(_) => "ListBox",
            Self::TextEntry(_) => "TextEntry",
            Self::StaticText(_) => "StaticText",
            Self::ProgressBar(_) => "ProgressBar",
            Self::TabControl(_) => "TabControl",
            Self::ComboBox(_) => "ComboBox",
            Self::TabPane => "TabPane",
            Self::User => "User",
            Self::Animated => "Animated",
            Self::MouseTrack => "MouseTrack",
        };
        f.write_str(name)
    }
}

impl fmt::Debug for GameWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GameWindow")
            .field("id", &self.id)
            .field("status", &self.status)
            .field("child_count", &self.children.len())
            .finish()
    }
}

impl GameWindow {
    /// Create a new GameWindow
    pub fn new() -> Self {
        Self {
            id: WINDOW_ID_INVALID,
            status: WindowStatus::NONE,
            size: Point2D { x: 0, y: 0 },
            region: WindowRegion::default(),
            cursor_pos: Point2D { x: 0, y: 0 },
            inst_data: WindowInstanceData::default(),
            user_data: None,
            edit_data: None,
            parent: None,
            children: Vec::new(),
            next_sibling: None,
            prev_sibling: None,
            owner_is_self: false,
            next_in_layout: None,
            prev_in_layout: None,
            layout: None,
            callbacks: WindowCallbacks {
                draw: Some(Box::new(legacy_default_draw_callback)),
                tooltip: None,
                input: Some(Box::new(default_input_callback)),
                system: Some(Box::new(default_system_callback)),
            },
            widget: None,
            combobox_links: None,
            listbox_links: None,
            slider_thumb: None,
            press_scale: 1.0,
            press_scale_target: 1.0,
            press_scale_velocity: 0.0,
            press_spring_strength: 60.0,
            press_spring_damping: 10.0,
            press_impulse: -4.5,
            release_impulse: 5.5,
            press_was_down: false,
        }
    }

    /// Get window ID
    pub fn get_id(&self) -> WindowId {
        self.id
    }

    /// Get window style flags
    pub fn get_style(&self) -> u32 {
        self.inst_data.style
    }

    fn is_press_anim_enabled(&self) -> bool {
        if matches!(
            self.widget,
            Some(WindowWidget::PushButton(_))
                | Some(WindowWidget::CheckBox(_))
                | Some(WindowWidget::RadioButton(_))
        ) {
            return true;
        }
        self.inst_data.style & (GWS_PUSH_BUTTON | GWS_CHECK_BOX | GWS_RADIO_BUTTON) != 0
    }

    pub fn get_press_scale(&self) -> f32 {
        if self.is_press_anim_enabled() {
            self.press_scale
        } else {
            1.0
        }
    }

    fn sync_state_from_widget(&mut self) {
        let (pressed, hilited, has_widget) = if let Some(widget) = self.widget.as_ref() {
            let widget_state = widget.state();
            (
                matches!(widget_state, GadgetState::Pressed),
                matches!(widget_state, GadgetState::Hovered | GadgetState::Pressed),
                true,
            )
        } else {
            let pressed = self.inst_data.state.contains(WindowState::PUSHED);
            let hilited = self.inst_data.state.contains(WindowState::HILITED) || pressed;
            (pressed, hilited, false)
        };

        if has_widget {
            let mut state = self.inst_data.state;
            state.remove(WindowState::HILITED | WindowState::PUSHED);
            if hilited {
                state.insert(WindowState::HILITED);
            }
            if pressed {
                state.insert(WindowState::PUSHED);
            }
            self.inst_data.state = state;
        }

        if self.is_press_anim_enabled() && pressed != self.press_was_down {
            self.press_scale_target = if pressed { 0.94 } else { 1.0 };
            self.press_scale_velocity = if pressed {
                self.press_impulse
            } else {
                self.release_impulse
            };
            self.press_was_down = pressed;
        }
    }

    pub fn update_press_animation(&mut self, delta_time: f32) {
        if !self.is_press_anim_enabled() {
            self.press_scale = 1.0;
            self.press_scale_target = 1.0;
            self.press_scale_velocity = 0.0;
            self.press_was_down = false;
            return;
        }

        // Keep press animation in sync even if input bypassed window message routing.
        self.sync_state_from_widget();

        let dt = delta_time.max(0.0);
        if dt == 0.0 {
            return;
        }

        let displacement = self.press_scale - self.press_scale_target;
        let accel = -self.press_spring_strength * displacement
            - self.press_spring_damping * self.press_scale_velocity;
        self.press_scale_velocity += accel * dt;
        self.press_scale += self.press_scale_velocity * dt;

        if (self.press_scale - self.press_scale_target).abs() < 0.0005
            && self.press_scale_velocity.abs() < 0.0005
        {
            self.press_scale = self.press_scale_target;
            self.press_scale_velocity = 0.0;
        }
    }

    /// Get tooltip delay in milliseconds
    pub fn get_tooltip_delay(&self) -> i32 {
        self.inst_data.tooltip_delay
    }

    /// Set window ID
    pub fn set_id(&mut self, id: WindowId) {
        self.id = id;
        self.inst_data.id = id;
    }

    /// Get window size
    pub fn get_size(&self) -> (i32, i32) {
        (self.size.x, self.size.y)
    }

    /// Set window size
    pub fn set_size(&mut self, width: i32, height: i32) -> WindowResult<()> {
        self.size.x = width;
        self.size.y = height;
        self.region.high.x = self.region.low.x + width;
        self.region.high.y = self.region.low.y + height;
        let _ = self.send_system_message(
            WindowMessage::User(GGM_RESIZED),
            width as WindowMsgData,
            height as WindowMsgData,
        );
        let mut resize_tab_panes = false;
        match self.widget.as_mut() {
            Some(WindowWidget::ListBox(listbox)) => {
                listbox.set_size(width.max(0) as u32, height.max(0) as u32);
            }
            Some(WindowWidget::TabControl(tab_control)) => {
                tab_control.set_size(width.max(0) as u32, height.max(0) as u32);
                resize_tab_panes = true;
            }
            _ => {}
        }
        if resize_tab_panes {
            self.resize_tab_panes_to_content();
        }
        if self.slider_thumb.is_some() {
            self.update_slider_thumb();
        }
        if let Some(links) = self.combobox_links {
            let button_width = 21;
            let base_height = if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box.borrow().get_size().1
            } else {
                height
            };
            if let Some(drop_down) = self.find_child_by_id(links.drop_down) {
                let _ = drop_down
                    .borrow_mut()
                    .set_position((width - button_width).max(0), 0);
                let _ = drop_down.borrow_mut().set_size(button_width, base_height);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                let _ = edit_box.borrow_mut().set_position(0, 0);
                let _ = edit_box
                    .borrow_mut()
                    .set_size((width - button_width).max(0), base_height);
            }
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                let current_list_height = list_box.borrow().get_size().1;
                let list_height = if height > base_height {
                    height - base_height
                } else {
                    current_list_height
                };
                let _ = list_box.borrow_mut().set_position(0, base_height);
                let _ = list_box.borrow_mut().set_size(width, list_height);
            }
        }
        if let Some(links) = self.listbox_links {
            let button_width = 21;
            let button_height = 22;
            let has_title = !self.inst_data.text.is_empty();
            let font_height = if has_title {
                with_window_manager_ref(|manager| {
                    self.inst_data
                        .font
                        .as_ref()
                        .map(|font| manager.win_font_height(font))
                        .unwrap_or(12)
                })
            } else {
                0
            };
            let top = if has_title { font_height + 1 } else { 0 };
            let bottom = if has_title {
                height - (font_height + 1)
            } else {
                height
            };

            if let Some(up_button) = self.find_child_by_id(links.up_button) {
                let _ = up_button
                    .borrow_mut()
                    .set_position(width - button_width - 2, top + 2);
                let _ = up_button.borrow_mut().set_size(button_width, button_height);
            }
            if let Some(down_button) = self.find_child_by_id(links.down_button) {
                let _ = down_button
                    .borrow_mut()
                    .set_position(width - button_width - 2, top + bottom - button_height - 2);
                let _ = down_button
                    .borrow_mut()
                    .set_size(button_width, button_height);
            }
            if let Some(slider) = self.find_child_by_id(links.slider) {
                let slider_height = (bottom - (2 * button_height) - 6).max(0);
                let _ = slider
                    .borrow_mut()
                    .set_position(width - button_width - 2, top + button_height + 3);
                let _ = slider.borrow_mut().set_size(button_width, slider_height);
            }
            if let Some(thumb_id) = links.thumb {
                if let Some(thumb) = self.find_child_by_id(thumb_id) {
                    let _ = thumb.borrow_mut().set_size(button_width, 16);
                }
            }
            self.update_listbox_scrollbar();
        }
        Ok(())
    }

    /// Get window position
    pub fn get_position(&self) -> (i32, i32) {
        (self.region.low.x, self.region.low.y)
    }

    /// Set window position
    pub fn set_position(&mut self, x: i32, y: i32) -> WindowResult<()> {
        self.region.low.x = x;
        self.region.low.y = y;
        self.region.high.x = x + self.size.x;
        self.region.high.y = y + self.size.y;
        self.normalize_region();
        Ok(())
    }

    /// Get screen position (including parent offsets)
    pub fn get_screen_position(&self) -> (i32, i32) {
        let mut x = self.region.low.x;
        let mut y = self.region.low.y;

        let mut current_parent = self.parent.as_ref().and_then(|w| w.upgrade());
        while let Some(parent_rc) = current_parent.take() {
            if let Ok(parent) = parent_rc.try_borrow() {
                x += parent.region.low.x;
                y += parent.region.low.y;
                current_parent = parent.parent.as_ref().and_then(|w| w.upgrade());
            } else {
                let ptr = parent_rc.as_ptr();
                // SAFETY: mirrors the legacy single-threaded window tree traversal where
                // parent reads can occur while a mutable callback path already owns the window.
                let parent = unsafe { &*ptr };
                x += parent.region.low.x;
                y += parent.region.low.y;
                current_parent = parent.parent.as_ref().and_then(|w| w.upgrade());
            }
        }

        (x, y)
    }

    /// Get window region
    pub fn get_region(&self) -> WindowRegion {
        self.region
    }

    /// Set cursor position within window
    pub fn set_cursor_position(&mut self, x: i32, y: i32) -> WindowResult<()> {
        self.cursor_pos.x = x;
        self.cursor_pos.y = y;
        Ok(())
    }

    /// Get cursor position within window
    pub fn get_cursor_position(&self) -> (i32, i32) {
        (self.cursor_pos.x, self.cursor_pos.y)
    }

    /// Check if point is within window (including children)
    pub fn point_in_window(&self, x: i32, y: i32) -> bool {
        let (win_x, win_y) = self.get_screen_position();
        let (width, height) = self.get_size();

        x >= win_x && x <= win_x + width && y >= win_y && y <= win_y + height
    }

    /// Return the deepest enabled, visible child at a point, or the given window.
    pub fn point_in_child(
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Rc<RefCell<GameWindow>> {
        let children = window.borrow().children().to_vec();
        for child in children {
            let child_borrow = child.borrow();
            let contains_point = child_borrow.point_in_window(x, y);
            let hidden = child_borrow.is_hidden();
            let enabled = ignore_enabled
                || child_borrow
                    .get_status()
                    .contains(WindowStatus::ENABLED);
            drop(child_borrow);

            if contains_point && !hidden && enabled {
                return Self::point_in_child(&child, x, y, ignore_enabled);
            }
        }

        window.clone()
    }

    /// Return the child at a point regardless of enabled state, optionally skipping hidden children.
    pub fn point_in_any_child(
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_hidden: bool,
        ignore_enabled: bool,
    ) -> Rc<RefCell<GameWindow>> {
        let children = window.borrow().children().to_vec();
        for child in children {
            let child_borrow = child.borrow();
            let contains_point = child_borrow.point_in_window(x, y);
            let skip_hidden = ignore_hidden && child_borrow.is_hidden();
            drop(child_borrow);

            if contains_point && !skip_hidden {
                return Self::point_in_child(&child, x, y, ignore_enabled);
            }
        }

        window.clone()
    }

    /// Get window status flags
    pub fn get_status(&self) -> WindowStatus {
        self.status
    }

    /// Set window status flags
    pub fn set_status(&mut self, status: WindowStatus) -> WindowStatus {
        let old_status = self.status;
        self.status |= status;
        self.inst_data.status = self.status;
        old_status
    }

    /// Clear window status flags
    pub fn clear_status(&mut self, status: WindowStatus) -> WindowStatus {
        let old_status = self.status;
        self.status &= !status;
        self.inst_data.status = self.status;
        old_status
    }

    /// Enable or disable the window
    pub fn enable(&mut self, enable: bool) -> WindowResult<()> {
        if enable {
            self.status |= WindowStatus::ENABLED;
        } else {
            self.status &= !WindowStatus::ENABLED;
        }
        self.inst_data.status = self.status;
        if let Some(widget) = &mut self.widget {
            widget.set_enabled(enable);
        }

        // Enable/disable all children
        for child_rc in &self.children {
            let mut child = child_rc.borrow_mut();
            child.enable(enable)?;
        }

        Ok(())
    }

    /// Check if window is enabled (C++ parity: checks all parents too)
    pub fn is_enabled(&self) -> bool {
        if !self.status.contains(WindowStatus::ENABLED) {
            return false;
        }
        // C++ parity: isEnabled() walks up parent chain
        let mut current = self.parent.as_ref().and_then(|w| w.upgrade());
        while let Some(parent_rc) = current {
            if let Ok(parent) = parent_rc.try_borrow() {
                if !parent.status.contains(WindowStatus::ENABLED) {
                    return false;
                }
                current = parent.parent.as_ref().and_then(|w| w.upgrade());
            } else {
                // SAFETY: mirrors legacy single-threaded window tree traversal
                let parent = unsafe { &*parent_rc.as_ptr() };
                if !parent.status.contains(WindowStatus::ENABLED) {
                    return false;
                }
                current = parent.parent.as_ref().and_then(|w| w.upgrade());
            }
        }
        true
    }

    /// Hide or show the window
    pub fn hide(&mut self, hide: bool) -> WindowResult<()> {
        self.set_hidden_status(hide);
        if hide {
            let window_ptr = self as *const GameWindow;
            let children = self.children.clone();
            with_window_manager(|manager| {
                manager.window_hiding_from_direct_hide(window_ptr, children);
            });
        }
        Ok(())
    }

    pub(crate) fn hide_without_manager_side_effects(&mut self, hide: bool) -> WindowResult<()> {
        self.set_hidden_status(hide);
        Ok(())
    }

    fn set_hidden_status(&mut self, hide: bool) {
        if hide {
            // C++ parity: parent visibility suppresses child rendering/input through
            // ancestry checks in is_hidden(), rather than permanently mutating every
            // child hidden bit when the parent is toggled.
            self.status |= WindowStatus::HIDDEN;
            self.inst_data.status = self.status;
            if let Some(widget) = &mut self.widget {
                widget.set_visible(false);
            }
        } else {
            self.status &= !WindowStatus::HIDDEN;
            self.inst_data.status = self.status;
            if let Some(widget) = &mut self.widget {
                widget.set_visible(true);
            }
        }
    }

    /// Check if this window's own hidden bit is set.
    pub fn is_hidden(&self) -> bool {
        self.status.contains(WindowStatus::HIDDEN)
    }

    /// Activate the window (bring to front and show)
    pub fn activate(&mut self) -> WindowResult<()> {
        self.status |= WindowStatus::ACTIVE;
        self.inst_data.status = self.status;
        self.hide(false)?;
        Ok(())
    }

    /// Set window text
    pub fn set_text(&mut self, text: &str) -> WindowResult<()> {
        self.inst_data.text = text.to_string();
        if let Some(widget) = self.widget.as_mut() {
            match widget {
                WindowWidget::PushButton(button) => button.set_text(text),
                WindowWidget::RadioButton(radio) => radio.set_label(text),
                WindowWidget::CheckBox(checkbox) => checkbox.set_label(text),
                WindowWidget::StaticText(label) => label.set_text(text),
                WindowWidget::TextEntry(entry) => entry.set_text(text),
                WindowWidget::ProgressBar(bar) => bar.set_text(text),
                _ => {}
            }
        }
        if let Some(display) = self.ensure_display_text() {
            display.borrow_mut().set_text(text.to_string());
        }
        Ok(())
    }

    /// Get window text
    pub fn get_text(&self) -> &str {
        &self.inst_data.text
    }

    /// Get the number of characters in the window text.
    pub fn get_text_length(&self) -> usize {
        self.inst_data.text.chars().count()
    }

    pub fn get_text_label(&self) -> &str {
        &self.inst_data.text_label
    }

    /// Set tooltip text
    pub fn set_tooltip(&mut self, tooltip: &str) {
        self.inst_data.tooltip = tooltip.chars().take(TOOLTIP_MAX_LEN).collect();
        if let Some(widget) = self.widget.as_mut() {
            if let WindowWidget::ListBox(listbox) = widget {
                listbox.set_tooltip(self.inst_data.tooltip.clone());
            }
        }
        if let Some(display) = self.ensure_display_tooltip() {
            display
                .borrow_mut()
                .set_text(self.inst_data.tooltip.clone());
        }
    }

    /// Get tooltip text
    pub fn get_tooltip(&self) -> &str {
        &self.inst_data.tooltip
    }

    /// Set window font
    pub fn set_font(&mut self, font: GameFont) {
        let font_for_children = font.clone();
        self.inst_data.font = Some(font);
        if let Some(font_desc) = self.inst_data.font.as_ref().map(GameFont::to_font_desc) {
            if let Ok(font_ref) = get_font_library().get_font(&font_desc) {
                if let Some(display) = self.inst_data.display_text.as_ref() {
                    display.borrow_mut().set_font(font_ref.clone());
                }
                if let Some(display) = self.inst_data.display_tooltip.as_ref() {
                    display.borrow_mut().set_font(font_ref);
                }
            }
        }
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box.borrow_mut().set_font(font_for_children.clone());
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box.borrow_mut().set_font(font_for_children);
            }
        }
    }

    /// Get window font
    pub fn get_font(&self) -> Option<&GameFont> {
        self.inst_data.font.as_ref()
    }

    /// Set highlight state
    pub fn set_hilite_state(&mut self, state: bool) {
        if state {
            self.inst_data.state |= WindowState::HILITED;
        } else {
            self.inst_data.state &= !WindowState::HILITED;
        }
    }

    /// Set draw offset for images
    pub fn set_draw_offset(&mut self, x: i32, y: i32) {
        self.inst_data.image_offset.x = x;
        self.inst_data.image_offset.y = y;
    }

    /// Get draw offset
    pub fn get_draw_offset(&self) -> (i32, i32) {
        (self.inst_data.image_offset.x, self.inst_data.image_offset.y)
    }

    /// Set enabled image for draw data at index
    pub fn set_enabled_image(&mut self, index: usize, image: Image) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.enabled_draw_data[index].image = Some(image);
        Ok(())
    }

    /// Get enabled draw data for the specified index.
    pub fn get_enabled_draw_data(&self, index: usize) -> Option<WindowDrawData> {
        if index >= MAX_DRAW_DATA {
            return None;
        }
        Some(self.inst_data.enabled_draw_data[index].clone())
    }

    /// Get disabled draw data for the specified index.
    pub fn get_disabled_draw_data(&self, index: usize) -> Option<WindowDrawData> {
        if index >= MAX_DRAW_DATA {
            return None;
        }
        Some(self.inst_data.disabled_draw_data[index].clone())
    }

    /// Get the enabled text color.
    pub fn get_enabled_text_color(&self) -> Color {
        self.inst_data.enabled_text.color
    }

    /// Get the enabled text border color.
    pub fn get_enabled_text_border_color(&self) -> Color {
        self.inst_data.enabled_text.border_color
    }

    /// Get the disabled text color.
    pub fn get_disabled_text_color(&self) -> Color {
        self.inst_data.disabled_text.color
    }

    /// Get the disabled text border color.
    pub fn get_disabled_text_border_color(&self) -> Color {
        self.inst_data.disabled_text.border_color
    }

    /// Get the IME composite text color.
    pub fn get_ime_composite_text_color(&self) -> Color {
        self.inst_data.ime_composite_text.color
    }

    /// Get the IME composite text border color.
    pub fn get_ime_composite_text_border_color(&self) -> Color {
        self.inst_data.ime_composite_text.border_color
    }

    /// Get the hilite text color.
    pub fn get_hilite_text_color(&self) -> Color {
        self.inst_data.hilite_text.color
    }

    /// Get the hilite text border color.
    pub fn get_hilite_text_border_color(&self) -> Color {
        self.inst_data.hilite_text.border_color
    }

    /// Show the window by clearing the hidden flag.
    pub fn show(&mut self) -> WindowResult<()> {
        self.hide(false)
    }

    /// Bring the window to the front of the z-order.
    pub fn bring_to_front(&mut self) -> WindowResult<()> {
        self.status |= WindowStatus::ACTIVE;
        Ok(())
    }

    /// Find a child control by name.
    pub fn find_child<T>(&self, _name: &str) -> Option<T> {
        None
    }

    /// Find a child window by its decorated name.
    pub fn find_child_window(&self, name: &str) -> Option<Rc<RefCell<GameWindow>>> {
        if self.inst_data.decorated_name.eq_ignore_ascii_case(name) {
            if let Some(parent) = self.get_parent() {
                for child_rc in parent.borrow().children() {
                    let child = child_rc.borrow();
                    if child.inst_data.decorated_name.eq_ignore_ascii_case(name) {
                        return Some(child_rc.clone());
                    }
                }
            }
        }
        for child_rc in &self.children {
            let child = child_rc.borrow();
            if child.inst_data.decorated_name.eq_ignore_ascii_case(name) {
                return Some(child_rc.clone());
            }
            if let Some(found) = child.find_child_window(name) {
                return Some(found);
            }
        }
        None
    }

    /// Find a child window by window id.
    pub fn find_child_by_id(&self, id: WindowId) -> Option<Rc<RefCell<GameWindow>>> {
        for child_rc in &self.children {
            let child = child_rc.borrow();
            if child.id == id {
                return Some(child_rc.clone());
            }
            if let Some(found) = child.find_child_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    /// Set enabled color for draw data at index
    pub fn set_enabled_color(&mut self, index: usize, color: Color) -> WindowResult<()> {
        if index >= MAX_DRAW_DATA {
            return Err(WindowError::InvalidParameter);
        }
        self.inst_data.enabled_draw_data[index].color = color;
        Ok(())
    }

    /// Set text colors for enabled state
    pub fn set_enabled_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.enabled_text.color = color;
        self.inst_data.enabled_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_enabled_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_enabled_text_colors(color, border_color);
            }
        }
    }

    /// Set text colors for disabled state
    pub fn set_disabled_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.disabled_text.color = color;
        self.inst_data.disabled_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_disabled_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_disabled_text_colors(color, border_color);
            }
        }
    }

    /// Set text colors for hilite state
    pub fn set_hilite_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.hilite_text.color = color;
        self.inst_data.hilite_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_hilite_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_hilite_text_colors(color, border_color);
            }
        }
    }

    /// Set text colors for IME composite state
    pub fn set_ime_composite_text_colors(&mut self, color: Color, border_color: Color) {
        self.inst_data.ime_composite_text.color = color;
        self.inst_data.ime_composite_text.border_color = border_color;
        if let Some(links) = self.combobox_links {
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                list_box
                    .borrow_mut()
                    .set_ime_composite_text_colors(color, border_color);
            }
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                edit_box
                    .borrow_mut()
                    .set_ime_composite_text_colors(color, border_color);
            }
        }
    }

    /// Get parent window
    pub fn get_parent(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.parent.as_ref()?.upgrade()
    }

    /// Set parent window.
    pub fn set_parent(&mut self, parent: Option<&Rc<RefCell<GameWindow>>>) {
        self.parent = parent.map(Rc::downgrade);
    }

    /// Get the window that receives gadget notifications from this window.
    pub fn get_owner(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.inst_data.owner.as_ref()?.upgrade()
    }

    /// Set the window that receives gadget notifications from this window.
    pub fn set_owner(&mut self, owner: Option<&Rc<RefCell<GameWindow>>>) {
        self.inst_data.owner = owner.map(Rc::downgrade);
        self.owner_is_self = false;
    }

    /// Set the window owner to this window, matching C++ winSetOwner(NULL).
    pub(crate) fn set_owner_self(&mut self, self_window: &Rc<RefCell<GameWindow>>) {
        self.inst_data.owner = Some(Rc::downgrade(self_window));
        self.owner_is_self = true;
    }

    /// Return whether this window's owner is itself.
    pub fn owner_is_self(&self) -> bool {
        self.owner_is_self
    }

    /// Set the layout this window belongs to.
    pub fn set_layout(&mut self, layout: Option<&Rc<RefCell<super::WindowLayout>>>) {
        self.layout = layout.map(Rc::downgrade);
    }

    /// Get the layout this window belongs to.
    pub fn get_layout(&self) -> Option<Rc<RefCell<super::WindowLayout>>> {
        self.layout.as_ref()?.upgrade()
    }

    /// Set the next window in this window's owning layout list.
    pub(crate) fn set_next_in_layout(&mut self, next: Option<&Rc<RefCell<GameWindow>>>) {
        self.next_in_layout = next.map(Rc::downgrade);
    }

    /// Set the previous window in this window's owning layout list.
    pub(crate) fn set_prev_in_layout(&mut self, prev: Option<&Rc<RefCell<GameWindow>>>) {
        self.prev_in_layout = prev.map(Rc::downgrade);
    }

    /// Get the next window in this window's owning layout list.
    pub(crate) fn get_next_in_layout(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.next_in_layout.as_ref()?.upgrade()
    }

    /// Get the previous window in this window's owning layout list.
    pub(crate) fn get_prev_in_layout(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.prev_in_layout.as_ref()?.upgrade()
    }

    /// Set the next window in this window's sibling list.
    pub(crate) fn set_next_sibling(&mut self, next: Option<&Rc<RefCell<GameWindow>>>) {
        self.next_sibling = next.map(Rc::downgrade);
    }

    /// Set the previous window in this window's sibling list.
    pub(crate) fn set_prev_sibling(&mut self, prev: Option<&Rc<RefCell<GameWindow>>>) {
        self.prev_sibling = prev.map(Rc::downgrade);
    }

    /// Get the next window in this window's sibling list.
    pub(crate) fn get_next_sibling(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.next_sibling.as_ref()?.upgrade()
    }

    /// Get the previous window in this window's sibling list.
    pub(crate) fn get_prev_sibling(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.prev_sibling.as_ref()?.upgrade()
    }

    /// Return the first leaf in this window's root branch.
    pub fn find_first_leaf(window: &Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        let mut leaf = Self::root_of(window);
        loop {
            let child = leaf.borrow().children().first().cloned();
            if let Some(child) = child {
                leaf = child;
            } else {
                return leaf;
            }
        }
    }

    /// Return the last leaf in this window's root branch.
    pub fn find_last_leaf(window: &Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        let mut leaf = Self::root_of(window);
        loop {
            let child = leaf.borrow().children().first().cloned();
            let Some(child) = child else {
                return leaf;
            };
            leaf = Self::last_sibling(child);
        }
    }

    /// Return the previous leaf in C++ tab traversal order.
    pub fn find_prev_leaf(window: &Rc<RefCell<GameWindow>>) -> Option<Rc<RefCell<GameWindow>>> {
        let mut leaf = window.clone();
        if let Some(prev) = leaf.borrow().get_prev_sibling() {
            return Some(Self::last_leaf_from(prev));
        }

        loop {
            let parent = leaf.borrow().get_parent();
            let Some(parent) = parent else {
                return Some(Self::find_last_leaf(&leaf));
            };
            leaf = parent;
            if leaf.borrow().get_parent().is_some() {
                if let Some(prev) = leaf.borrow().get_prev_sibling() {
                    return Some(Self::last_leaf_from(prev));
                }
            }
        }
    }

    /// Return the next leaf in C++ tab traversal order.
    pub fn find_next_leaf(window: &Rc<RefCell<GameWindow>>) -> Option<Rc<RefCell<GameWindow>>> {
        let mut leaf = window.clone();
        if let Some(next) = leaf.borrow().get_next_sibling() {
            return Self::first_leaf_from(next);
        }

        loop {
            let parent = leaf.borrow().get_parent();
            let Some(parent) = parent else {
                return Some(Self::find_first_leaf(&leaf));
            };
            leaf = parent;
            if leaf.borrow().get_parent().is_some() {
                if let Some(next) = leaf.borrow().get_next_sibling() {
                    return Self::first_leaf_from(next);
                }
            }
        }
    }

    fn root_of(window: &Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        let mut root = window.clone();
        loop {
            let parent = root.borrow().get_parent();
            if let Some(parent) = parent {
                root = parent;
            } else {
                return root;
            }
        }
    }

    fn last_sibling(mut window: Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        loop {
            let next = window.borrow().get_next_sibling();
            if let Some(next) = next {
                window = next;
            } else {
                return window;
            }
        }
    }

    fn first_leaf_from(mut leaf: Rc<RefCell<GameWindow>>) -> Option<Rc<RefCell<GameWindow>>> {
        loop {
            let leaf_borrow = leaf.borrow();
            if leaf_borrow.children().is_empty()
                || leaf_borrow.get_status().contains(WindowStatus::TAB_STOP)
            {
                drop(leaf_borrow);
                return Some(leaf);
            }
            let child = leaf_borrow.children().first().cloned().unwrap();
            drop(leaf_borrow);
            leaf = child;
        }
    }

    fn last_leaf_from(mut leaf: Rc<RefCell<GameWindow>>) -> Rc<RefCell<GameWindow>> {
        loop {
            let descend = {
                let leaf_borrow = leaf.borrow();
                !leaf_borrow.get_status().contains(WindowStatus::TAB_STOP)
                    && !leaf_borrow.children().is_empty()
            };
            if !descend {
                return leaf;
            }
            let child = leaf.borrow().children().first().cloned().unwrap();
            leaf = Self::last_sibling(child);
        }
    }

    /// Add child window
    pub fn add_child(&mut self, child: Rc<RefCell<GameWindow>>) {
        self.children.insert(0, child);
        Self::sync_sibling_links(&self.children);
    }

    /// Remove child window
    pub fn remove_child(&mut self, child: &Rc<RefCell<GameWindow>>) {
        self.children.retain(|c| !Rc::ptr_eq(c, child));
        {
            let mut child = child.borrow_mut();
            child.parent = None;
            child.set_prev_sibling(None);
            child.set_next_sibling(None);
        }
        Self::sync_sibling_links(&self.children);
    }

    /// Get immutable slice of child windows
    pub fn children(&self) -> &[Rc<RefCell<GameWindow>>] {
        &self.children
    }

    /// Get mutable view of the child list
    pub fn children_mut(&mut self) -> &mut Vec<Rc<RefCell<GameWindow>>> {
        &mut self.children
    }

    /// Synchronize C++-style m_next/m_prev links for this window's child list.
    pub(crate) fn sync_child_sibling_links(&mut self) {
        Self::sync_sibling_links(&self.children);
    }

    pub(crate) fn sync_sibling_links(windows: &[Rc<RefCell<GameWindow>>]) {
        for (index, window) in windows.iter().enumerate() {
            let prev = index.checked_sub(1).and_then(|i| windows.get(i));
            let next = windows.get(index + 1);
            let mut window = window.borrow_mut();
            window.set_prev_sibling(prev);
            window.set_next_sibling(next);
        }
    }

    /// Check if a window is a child of this window
    pub fn is_child(&self, window: &GameWindow) -> bool {
        let mut parent = window.get_parent();
        while let Some(parent_rc) = parent {
            let parent_borrow = parent_rc.borrow();
            if std::ptr::eq(self, &*parent_borrow) {
                return true;
            }
            parent = parent_borrow.get_parent();
        }
        false
    }

    /// Check if a window is this window or any descendant.
    pub fn contains_descendant(&self, window: &GameWindow) -> bool {
        if std::ptr::eq(self, window) {
            return true;
        }
        for child_rc in &self.children {
            let child = child_rc.borrow();
            if child.contains_descendant(window) {
                return true;
            }
        }
        false
    }

    /// Get first child window
    pub fn get_first_child(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.children.first().cloned()
    }

    /// Get the decorated name from the instance data.
    pub fn get_name(&self) -> &str {
        &self.inst_data.decorated_name
    }

    /// Set the decorated name for lookup and debugging.
    pub fn set_name(&mut self, name: &str) {
        self.inst_data.decorated_name = name.to_string();
    }

    /// Replace the current status with an explicit value.
    pub fn set_status_exact(&mut self, status: WindowStatus) {
        self.status = status;
        self.inst_data.status = status;
        if let Some(widget) = &mut self.widget {
            widget.set_enabled(status.contains(WindowStatus::ENABLED));
            widget.set_visible(!status.contains(WindowStatus::HIDDEN));
        }
    }

    /// Mutable access to instance data for script loading.
    pub fn instance_data_mut(&mut self) -> &mut WindowInstanceData {
        &mut self.inst_data
    }

    /// Immutable access to instance data for script loading.
    pub fn instance_data(&self) -> &WindowInstanceData {
        &self.inst_data
    }

    pub fn set_video_buffer(&mut self, buffer: Option<VideoBufferHandle>) {
        self.inst_data.video_buffer = buffer;
    }

    pub fn video_buffer(&self) -> Option<VideoBufferHandle> {
        self.inst_data.video_buffer.clone()
    }

    fn ensure_display_text(&mut self) -> Option<DisplayStringHandle> {
        if self.inst_data.display_text.is_none() {
            let handle = {
                let mut manager = get_display_string_manager();
                manager.new_display_string()
            };
            if let Some(font_desc) = self.inst_data.font.as_ref().map(GameFont::to_font_desc) {
                if let Ok(font_ref) = get_font_library().get_font(&font_desc) {
                    handle.borrow_mut().set_font(font_ref);
                }
            }
            self.inst_data.display_text = Some(handle);
        }
        self.inst_data.display_text.clone()
    }

    fn ensure_display_tooltip(&mut self) -> Option<DisplayStringHandle> {
        if self.inst_data.display_tooltip.is_none() {
            let handle = {
                let mut manager = get_display_string_manager();
                manager.new_display_string()
            };
            if let Some(font_desc) = self.inst_data.font.as_ref().map(GameFont::to_font_desc) {
                if let Ok(font_ref) = get_font_library().get_font(&font_desc) {
                    handle.borrow_mut().set_font(font_ref);
                }
            }
            self.inst_data.display_tooltip = Some(handle);
        }
        self.inst_data.display_tooltip.clone()
    }

    /// Attach a gadget widget to this window.
    pub fn set_widget(&mut self, widget: WindowWidget) {
        self.widget = Some(widget);
    }

    pub fn widget(&self) -> Option<&WindowWidget> {
        self.widget.as_ref()
    }

    pub fn widget_mut(&mut self) -> Option<&mut WindowWidget> {
        self.widget.as_mut()
    }

    pub(crate) fn set_combobox_links(&mut self, links: ComboBoxLinks) {
        self.combobox_links = Some(links);
    }

    pub(crate) fn combobox_links(&self) -> Option<ComboBoxLinks> {
        self.combobox_links
    }

    pub(crate) fn set_listbox_links(&mut self, links: ListBoxLinks) {
        self.listbox_links = Some(links);
    }

    pub(crate) fn listbox_links(&self) -> Option<ListBoxLinks> {
        self.listbox_links
    }

    pub(crate) fn set_slider_thumb(&mut self, thumb: WindowId) {
        self.slider_thumb = Some(thumb);
    }

    pub(crate) fn slider_thumb(&self) -> Option<WindowId> {
        self.slider_thumb
    }

    pub fn static_text_mut(&mut self) -> Option<&mut StaticText> {
        match self.widget.as_mut() {
            Some(WindowWidget::StaticText(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn text_entry_mut(&mut self) -> Option<&mut TextEntry> {
        match self.widget.as_mut() {
            Some(WindowWidget::TextEntry(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn list_box_mut(&mut self) -> Option<&mut ListBox> {
        match self.widget.as_mut() {
            Some(WindowWidget::ListBox(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn combo_box_mut(&mut self) -> Option<&mut ComboBox> {
        match self.widget.as_mut() {
            Some(WindowWidget::ComboBox(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn set_combo_box_selected(&mut self, index: usize, dont_hide: bool) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() else {
            return;
        };
        if dont_hide {
            combo.set_dont_hide_next(true);
        }
        let _ = combo.select_index(index);
        if let Some(links) = self.combobox_links {
            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                self.sync_combobox_edit_box(&edit_box);
            }
            if let Some(list_box) = self.find_child_by_id(links.list_box) {
                self.sync_combobox_listbox(&list_box);
            }
        }
    }

    pub fn check_box_mut(&mut self) -> Option<&mut CheckBox> {
        match self.widget.as_mut() {
            Some(WindowWidget::CheckBox(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn progress_bar_mut(&mut self) -> Option<&mut ProgressBar> {
        match self.widget.as_mut() {
            Some(WindowWidget::ProgressBar(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn horizontal_slider_mut(&mut self) -> Option<&mut HorizontalSlider> {
        match self.widget.as_mut() {
            Some(WindowWidget::HorizontalSlider(widget)) => Some(widget),
            _ => None,
        }
    }

    pub fn vertical_slider_mut(&mut self) -> Option<&mut VerticalSlider> {
        match self.widget.as_mut() {
            Some(WindowWidget::VerticalSlider(widget)) => Some(widget),
            _ => None,
        }
    }

    /// Set user data
    pub fn set_user_data<T: 'static>(&mut self, data: T) {
        self.user_data = Some(Box::new(data));
    }

    /// Get user data
    pub fn get_user_data<T: 'static>(&self) -> Option<&T> {
        self.user_data.as_ref()?.downcast_ref::<T>()
    }

    /// Set GUI-editor-only metadata for this window.
    pub fn set_edit_data(&mut self, edit_data: Option<GameWindowEditData>) {
        self.edit_data = edit_data;
    }

    /// Get GUI-editor-only metadata for this window.
    pub fn get_edit_data(&self) -> Option<&GameWindowEditData> {
        self.edit_data.as_ref()
    }

    /// Get mutable GUI-editor-only metadata for this window.
    pub fn get_edit_data_mut(&mut self) -> Option<&mut GameWindowEditData> {
        self.edit_data.as_mut()
    }

    /// Set draw callback
    pub fn set_draw_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, &WindowInstanceData) + 'static,
    {
        self.callbacks.draw = Some(Box::new(callback));
    }

    /// Get draw callback.
    pub fn get_draw_callback(&self) -> Option<&dyn Fn(&GameWindow, &WindowInstanceData)> {
        self.callbacks.draw.as_deref()
    }

    /// Reset draw callback to the legacy default handler.
    pub fn reset_draw_callback(&mut self) {
        self.callbacks.draw = Some(Box::new(legacy_default_draw_callback));
    }

    /// Set input callback
    pub fn set_input_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled
            + 'static,
    {
        self.callbacks.input = Some(Box::new(callback));
    }

    /// Get input callback.
    pub fn get_input_callback(
        &self,
    ) -> Option<&dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>
    {
        self.callbacks.input.as_deref()
    }

    /// Reset input callback to the default handler.
    pub fn reset_input_callback(&mut self) {
        self.callbacks.input = Some(Box::new(default_input_callback));
    }

    /// Set system callback
    pub fn set_system_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled
            + 'static,
    {
        self.callbacks.system = Some(Box::new(callback));
    }

    /// Get system callback.
    pub fn get_system_callback(
        &self,
    ) -> Option<&dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>
    {
        self.callbacks.system.as_deref()
    }

    /// Reset system callback to the default handler.
    pub fn reset_system_callback(&mut self) {
        self.callbacks.system = Some(Box::new(default_system_callback));
    }

    /// Set tooltip callback
    pub fn set_tooltip_callback<F>(&mut self, callback: F)
    where
        F: Fn(&GameWindow, &WindowInstanceData, u32) + 'static,
    {
        self.callbacks.tooltip = Some(Box::new(callback));
    }

    /// Get tooltip callback.
    pub fn get_tooltip_callback(&self) -> Option<&dyn Fn(&GameWindow, &WindowInstanceData, u32)> {
        self.callbacks.tooltip.as_deref()
    }

    /// Clear tooltip callback.
    pub fn clear_tooltip_callback(&mut self) {
        self.callbacks.tooltip = None;
    }

    /// Set input, draw, and tooltip callbacks in one operation, like C++ winSetCallbacks.
    pub fn set_callbacks(
        &mut self,
        input: Option<InputCallback>,
        draw: Option<DrawCallback>,
        tooltip: Option<TooltipCallback>,
    ) {
        self.callbacks.input = input.or_else(|| Some(Box::new(default_input_callback)));
        self.callbacks.draw = draw.or_else(|| Some(Box::new(legacy_default_draw_callback)));
        self.callbacks.tooltip = tooltip;
    }

    /// Draw the window
    pub fn draw(&self) {
        if !self.is_hidden() {
            if let Some(ref draw_callback) = self.callbacks.draw {
                draw_callback(self, &self.inst_data);
            }
        }
    }

    /// Send input message to window
    pub fn send_input_message(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Destroy {
            if self.status.contains(WindowStatus::DESTROYED)
                || self.is_hidden()
                || !self.is_enabled()
                || self.status.contains(WindowStatus::NO_INPUT)
            {
                return WindowMsgHandled::Ignored;
            }
        }
        self.update_press_state_from_message(msg);
        if let Some(ref input_callback) = self.callbacks.input {
            let result = input_callback(self, msg, data1, data2);
            if result == WindowMsgHandled::Ignored {
                self.handle_widget_input(msg, data1, data2)
            } else {
                result
            }
        } else {
            self.handle_widget_input(msg, data1, data2)
        }
    }

    /// Send input after legacy window-manager routing has already selected the target.
    pub fn send_routed_input_message(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Destroy && self.status.contains(WindowStatus::DESTROYED) {
            return WindowMsgHandled::Ignored;
        }
        self.update_press_state_from_message(msg);
        if let Some(ref input_callback) = self.callbacks.input {
            let result = input_callback(self, msg, data1, data2);
            if result == WindowMsgHandled::Ignored {
                self.handle_widget_input(msg, data1, data2)
            } else {
                result
            }
        } else {
            self.handle_widget_input(msg, data1, data2)
        }
    }

    fn update_press_state_from_message(&mut self, msg: WindowMessage) {
        if !self.is_press_anim_enabled() {
            return;
        }
        match msg {
            WindowMessage::LeftDown => {
                if !self.press_was_down {
                    self.press_scale_target = 0.94;
                    self.press_scale_velocity = self.press_impulse;
                    self.press_was_down = true;
                }
            }
            WindowMessage::LeftUp => {
                if self.press_was_down {
                    self.press_scale_target = 1.0;
                    self.press_scale_velocity = self.release_impulse;
                    self.press_was_down = false;
                }
            }
            _ => {}
        }
    }

    /// Send system message to window
    pub fn send_system_message(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Destroy && self.status.contains(WindowStatus::DESTROYED) {
            return WindowMsgHandled::Ignored;
        }

        if let Some(ref system_callback) = self.callbacks.system {
            let result = system_callback(self, msg, data1, data2);
            if result == WindowMsgHandled::Ignored {
                self.handle_widget_system(msg, data1, data2)
            } else {
                result
            }
        } else {
            self.handle_widget_system(msg, data1, data2)
        }
    }

    fn handle_widget_input(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        let Some(widget) = self.widget.as_mut() else {
            return WindowMsgHandled::Ignored;
        };

        if matches!(widget, WindowWidget::ListBox(_))
            && (msg == WindowMessage::WheelUp || msg == WindowMessage::WheelDown)
        {
            let delta = if msg == WindowMessage::WheelUp { -1 } else { 1 };
            if let WindowWidget::ListBox(listbox) = widget {
                listbox.scroll_by(delta);
            }
            self.update_listbox_scrollbar();
            return WindowMsgHandled::Handled;
        }

        let (x, y) = (self.cursor_pos.x, self.cursor_pos.y);
        let event = match msg {
            WindowMessage::MousePos => Some(InputEvent::MouseMove { x, y }),
            WindowMessage::MouseEntering => Some(InputEvent::MouseEnter { x, y }),
            WindowMessage::MouseLeaving => Some(InputEvent::MouseLeave { x, y }),
            WindowMessage::LeftDown => Some(InputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            }),
            WindowMessage::LeftUp => Some(InputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            }),
            WindowMessage::LeftDrag => Some(InputEvent::MouseDrag {
                x,
                y,
                button: MouseButton::Left,
            }),
            WindowMessage::MiddleDown => Some(InputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Middle,
            }),
            WindowMessage::MiddleUp => Some(InputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Middle,
            }),
            WindowMessage::MiddleDrag => Some(InputEvent::MouseDrag {
                x,
                y,
                button: MouseButton::Middle,
            }),
            WindowMessage::RightDown => Some(InputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
            }),
            WindowMessage::RightUp => Some(InputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Right,
            }),
            WindowMessage::RightDrag => Some(InputEvent::MouseDrag {
                x,
                y,
                button: MouseButton::Right,
            }),
            WindowMessage::Char => char_input_event(data1, data2),
            _ => None,
        };

        let Some(event) = event else {
            return WindowMsgHandled::Ignored;
        };

        let state_before = widget.state();
        let messages = widget.handle_input(&event);
        let state_changed = widget.state() != state_before;
        self.sync_state_from_widget();
        if messages.is_empty() {
            return if state_changed {
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            };
        }

        if matches!(
            self.widget,
            Some(WindowWidget::HorizontalSlider(_)) | Some(WindowWidget::VerticalSlider(_))
        ) {
            self.update_slider_thumb();
        }

        if matches!(self.widget, Some(WindowWidget::ListBox(_))) {
            self.update_listbox_scrollbar();
        }

        if matches!(self.widget, Some(WindowWidget::TabControl(_))) {
            if let Some(selected) = messages.iter().find_map(|message| {
                if let GadgetMessage::ValueChanged { value, .. } = message {
                    if let GadgetValue::Integer(val) = value {
                        return Some(*val);
                    }
                }
                None
            }) {
                if selected >= 0 {
                    self.show_tab_pane(selected as usize);
                }
            }
        }

        let mut handled = false;
        let target_owner = if self.get_parent().is_some() && !self.owner_is_self {
            self.get_owner()
        } else {
            None
        };
        for message in messages {
            let (msg, data1) = match message {
                GadgetMessage::Clicked { .. } => (WindowMessage::GadgetSelected, self.id as u32),
                GadgetMessage::RightClicked { .. } => {
                    if !self.status.contains(WindowStatus::RIGHT_CLICK) {
                        continue;
                    }
                    (WindowMessage::GadgetRightClick, self.id as u32)
                }
                GadgetMessage::LeftDrag { .. } => (WindowMessage::User(GGM_LEFT_DRAG), data1),
                GadgetMessage::ValueChanged { .. } => {
                    (WindowMessage::GadgetValueChanged, self.id as u32)
                }
                GadgetMessage::EditingComplete { .. } => {
                    (WindowMessage::GadgetEditDone, self.id as u32)
                }
                GadgetMessage::MouseEnter { .. } => {
                    (WindowMessage::GadgetMouseEntering, self.id as u32)
                }
                GadgetMessage::MouseLeave { .. } => {
                    (WindowMessage::GadgetMouseLeaving, self.id as u32)
                }
                GadgetMessage::FocusChanged { has_focus, .. } => {
                    (WindowMessage::InputFocus, if has_focus { 1 } else { 0 })
                }
                GadgetMessage::Custom { data, .. } => {
                    if data == "tab_next" {
                        with_window_manager(|manager| manager.navigate_tab(TabDirection::Next));
                        handled = true;
                        continue;
                    }
                    if data == "tab_prev" {
                        with_window_manager(|manager| manager.navigate_tab(TabDirection::Previous));
                        handled = true;
                        continue;
                    }
                    (WindowMessage::User(0x8000), self.id as u32)
                }
            };

            let result = if let Some(ref owner) = target_owner {
                owner.borrow_mut().send_system_message(msg, data1, 0)
            } else {
                self.send_system_message(msg, data1, 0)
            };
            if result == WindowMsgHandled::Handled {
                handled = true;
            }
        }

        if handled {
            WindowMsgHandled::Handled
        } else {
            WindowMsgHandled::Ignored
        }
    }

    fn handle_widget_system(
        &mut self,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if matches!(
            msg,
            WindowMessage::RightDown | WindowMessage::RightUp | WindowMessage::RightDrag
        ) && !self.status.contains(WindowStatus::RIGHT_CLICK)
        {
            return WindowMsgHandled::Ignored;
        }

        if matches!(
            msg,
            WindowMessage::MouseEntering | WindowMessage::MouseLeaving
        ) && (self.inst_data.style & GWS_MOUSE_TRACK == 0)
        {
            return WindowMsgHandled::Ignored;
        }

        if self.widget.is_none() {
            return WindowMsgHandled::Ignored;
        }

        if matches!(self.widget, Some(WindowWidget::ComboBox(_))) {
            if let Some(links) = self.combobox_links {
                if msg == WindowMessage::GadgetSelected && data1 == links.drop_down as u32 {
                    if let Some(list_box) = self.find_child_by_id(links.list_box) {
                        let is_hidden = list_box.borrow().is_hidden();
                        if is_hidden {
                            self.sync_combobox_listbox(&list_box);
                            self.resize_combobox_listbox(&list_box);
                            let list_height = list_box.borrow().get_size().1;
                            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                let base_height = edit_box.borrow().get_size().1;
                                let (width, _) = self.get_size();
                                let _ = self.set_size(width, base_height + list_height);
                            }
                            let _ = list_box.borrow_mut().hide(false);
                        } else {
                            let _ = list_box.borrow_mut().hide(true);
                            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                let base_height = edit_box.borrow().get_size().1;
                                let (width, _) = self.get_size();
                                let _ = self.set_size(width, base_height);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                }

                if msg == WindowMessage::GadgetValueChanged && data1 == links.list_box as u32 {
                    if let Some(list_box) = self.find_child_by_id(links.list_box) {
                        if let Some(selected) =
                            list_box.borrow().widget().and_then(|widget| match widget {
                                WindowWidget::ListBox(listbox) => {
                                    listbox.selected_indices().first().copied()
                                }
                                _ => None,
                            })
                        {
                            if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
                                let _ = combo.select_index(selected);
                            }
                        }
                        if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                            self.sync_combobox_edit_box(&edit_box);
                        }
                        let dont_hide =
                            if let Some(WindowWidget::ComboBox(combo)) = self.widget.as_mut() {
                                combo.take_dont_hide_next()
                            } else {
                                false
                            };
                        if !dont_hide {
                            let _ = list_box.borrow_mut().hide(true);
                            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                let base_height = edit_box.borrow().get_size().1;
                                let (width, _) = self.get_size();
                                let _ = self.set_size(width, base_height);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                }

                if msg == WindowMessage::GadgetEditDone && data1 == links.edit_box as u32 {
                    if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                        let edit_text =
                            edit_box.borrow().widget().and_then(|widget| match widget {
                                WindowWidget::TextEntry(entry) => {
                                    Some(entry.displayed_text().to_string())
                                }
                                _ => None,
                            });
                        if let (Some(text), Some(WindowWidget::ComboBox(combo))) =
                            (edit_text, self.widget.as_mut())
                        {
                            combo.set_text(&text);
                        }
                        return WindowMsgHandled::Handled;
                    }
                }
            }
        }

        if matches!(self.widget, Some(WindowWidget::ListBox(_))) {
            if let WindowMessage::User(code) = msg {
                match code {
                    GLM_DEL_ALL => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            listbox.clear();
                        }
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Handled;
                    }
                    GLM_DEL_ENTRY => {
                        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                            let _ = listbox.remove_item(data1 as usize);
                        }
                        self.update_listbox_scrollbar();
                        return WindowMsgHandled::Handled;
                    }
                    _ => {}
                }
            }

            if let Some(links) = self.listbox_links {
                if msg == WindowMessage::GadgetSelected && data1 == links.up_button as u32 {
                    if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                        listbox.scroll_by(-1);
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }

                if msg == WindowMessage::GadgetSelected && data1 == links.down_button as u32 {
                    if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                        listbox.scroll_by(1);
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }

                if msg == WindowMessage::GadgetValueChanged && data1 == links.slider as u32 {
                    let slider_value = if let Some(slider) = self.find_child_by_id(links.slider) {
                        match slider.borrow().widget() {
                            Some(WindowWidget::VerticalSlider(slider)) => slider.value(),
                            Some(WindowWidget::HorizontalSlider(slider)) => slider.value(),
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                        listbox.set_scroll_offset(slider_value.max(0) as usize);
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }
            }
        }

        if matches!(
            self.widget,
            Some(WindowWidget::HorizontalSlider(_)) | Some(WindowWidget::VerticalSlider(_))
        ) {
            if let WindowMessage::User(code) = msg {
                match code {
                    GSM_SET_SLIDER => {
                        let new_pos = data1 as i32;
                        let mut update_thumb = false;
                        match self.widget.as_mut() {
                            Some(WindowWidget::HorizontalSlider(slider)) => {
                                let (min_val, max_val) = slider.range();
                                if (min_val..=max_val).contains(&new_pos) {
                                    slider.set_value(new_pos);
                                    update_thumb = true;
                                }
                            }
                            Some(WindowWidget::VerticalSlider(slider)) => {
                                let (min_val, max_val) = slider.range();
                                if (min_val..=max_val).contains(&new_pos) {
                                    slider.set_value(new_pos);
                                    update_thumb = true;
                                }
                            }
                            _ => {}
                        }
                        if update_thumb {
                            self.update_slider_thumb();
                        }
                        return WindowMsgHandled::Handled;
                    }
                    GSM_SET_MIN_MAX => {
                        let min_val = data1 as i32;
                        let max_val = data2 as i32;
                        match self.widget.as_mut() {
                            Some(WindowWidget::HorizontalSlider(slider)) => {
                                slider.set_range(min_val, max_val);
                                slider.set_value(min_val);
                            }
                            Some(WindowWidget::VerticalSlider(slider)) => {
                                slider.set_range(min_val, max_val);
                                slider.set_value(min_val);
                            }
                            _ => {}
                        }
                        self.update_slider_thumb();
                        return WindowMsgHandled::Handled;
                    }
                    GGM_RESIZED => {
                        if let Some(thumb_id) = self.slider_thumb {
                            if let Some(thumb) = self.find_child_by_id(thumb_id) {
                                match self.widget.as_ref() {
                                    Some(WindowWidget::HorizontalSlider(_)) => {
                                        let _ =
                                            thumb.borrow_mut().set_size(GADGET_SIZE, data2 as i32);
                                    }
                                    Some(WindowWidget::VerticalSlider(_)) => {
                                        let _ =
                                            thumb.borrow_mut().set_size(data1 as i32, GADGET_SIZE);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                    _ => {}
                }
            }
        }

        if matches!(self.widget, Some(WindowWidget::RadioButton(_))) {
            if let WindowMessage::User(code) = msg {
                if code == GBM_SET_SELECTION {
                    let mut newly_selected = false;
                    if let Some(WindowWidget::RadioButton(radio)) = self.widget.as_mut() {
                        if !radio.is_selected() {
                            radio.select();
                            newly_selected = true;
                        }
                    }
                    if newly_selected && data1 != 0 && !self.owner_is_self {
                        if let Some(owner) = self.get_owner() {
                            let _ = owner.borrow_mut().send_system_message(
                                WindowMessage::GadgetSelected,
                                self.id as u32,
                                0,
                            );
                        }
                    }
                    return WindowMsgHandled::Handled;
                }
            }
        }

        if matches!(self.widget, Some(WindowWidget::ProgressBar(_))) {
            if let WindowMessage::User(code) = msg {
                if code == GPM_SET_PROGRESS {
                    let progress = data1 as i32;
                    if (0..=100).contains(&progress) {
                        if let Some(WindowWidget::ProgressBar(progress_bar)) = self.widget.as_mut() {
                            progress_bar.set_progress(progress as f32);
                        }
                    }
                    return WindowMsgHandled::Handled;
                }
            }
        }

        if msg == WindowMessage::InputFocus {
            let focused = data1 != 0;
            let event = if focused {
                InputEvent::FocusGained
            } else {
                InputEvent::FocusLost
            };
            let messages = if let Some(widget) = self.widget.as_mut() {
                widget.handle_input(&event)
            } else {
                Vec::new()
            };
            self.set_hilite_state(focused);
            if !self.owner_is_self {
                if let Some(owner) = self.get_owner() {
                    let _ = owner.borrow_mut().send_system_message(
                        WindowMessage::User(GGM_FOCUS_CHANGE),
                        data1,
                        self.id as u32,
                    );
                }
            }
            return if messages.is_empty() {
                WindowMsgHandled::Ignored
            } else {
                WindowMsgHandled::Handled
            };
        }

        WindowMsgHandled::Ignored
    }

    fn sync_combobox_listbox(&mut self, list_box: &Rc<RefCell<GameWindow>>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_ref() else {
            return;
        };
        let mut list_box_guard = list_box.borrow_mut();
        let Some(WindowWidget::ListBox(listbox)) = list_box_guard.widget_mut() else {
            return;
        };
        listbox.clear();
        for item in combo.items() {
            listbox.add_item(&item.text);
        }
        if let Some(selected) = combo.selected_index() {
            let _ = listbox.select_index(selected, KeyModifiers::none());
        }
        drop(list_box_guard);
        list_box.borrow_mut().update_listbox_scrollbar();
    }

    fn sync_combobox_edit_box(&mut self, edit_box: &Rc<RefCell<GameWindow>>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_ref() else {
            return;
        };
        let mut edit_box_guard = edit_box.borrow_mut();
        let Some(WindowWidget::TextEntry(entry)) = edit_box_guard.widget_mut() else {
            return;
        };
        entry.set_text(combo.text());
    }

    fn resize_combobox_listbox(&mut self, list_box: &Rc<RefCell<GameWindow>>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.as_ref() else {
            return;
        };
        let count = combo.items().len().max(1);
        let max_display = combo.max_display();
        let visible = if max_display > 0 {
            count.min(max_display)
        } else {
            count
        };
        let show_scrollbar = max_display > 0 && count > max_display;
        let item_height = list_box
            .borrow()
            .widget()
            .and_then(|widget| match widget {
                WindowWidget::ListBox(listbox) => Some(listbox.item_height() as i32),
                _ => None,
            })
            .unwrap_or(18);
        let height = (visible as i32 * item_height).max(item_height);
        let (width, _) = list_box.borrow().get_size();
        let _ = list_box.borrow_mut().set_size(width as i32, height);
        if let Some(links) = list_box.borrow().listbox_links() {
            if let Some(up) = list_box.borrow().find_child_by_id(links.up_button) {
                let _ = up.borrow_mut().hide(!show_scrollbar);
            }
            if let Some(down) = list_box.borrow().find_child_by_id(links.down_button) {
                let _ = down.borrow_mut().hide(!show_scrollbar);
            }
            if let Some(slider) = list_box.borrow().find_child_by_id(links.slider) {
                let _ = slider.borrow_mut().hide(!show_scrollbar);
            }
        }
        list_box.borrow_mut().update_listbox_scrollbar();
    }

    pub(crate) fn update_listbox_scrollbar(&mut self) {
        let Some(links) = self.listbox_links else {
            return;
        };
        let Some(WindowWidget::ListBox(listbox)) = self.widget.as_ref() else {
            return;
        };

        let bounds = listbox.bounds();
        let item_height = listbox.item_height().max(1) as usize;
        let visible = (bounds.height as usize / item_height).max(1);
        let max_offset = listbox.items().len().saturating_sub(visible);
        let scroll_offset = listbox.scroll_offset().min(max_offset);
        if scroll_offset != listbox.scroll_offset() {
            if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
                listbox.set_scroll_offset(scroll_offset);
            }
        }

        if let Some(slider) = self.find_child_by_id(links.slider) {
            if let Some(WindowWidget::VerticalSlider(slider)) = slider.borrow_mut().widget_mut() {
                slider.set_range(0, max_offset as i32);
                slider.set_value(scroll_offset as i32);
            } else if let Some(WindowWidget::HorizontalSlider(slider)) =
                slider.borrow_mut().widget_mut()
            {
                slider.set_range(0, max_offset as i32);
                slider.set_value(scroll_offset as i32);
            }
        }

        if let Some(up_button) = self.find_child_by_id(links.up_button) {
            let enabled = max_offset > 0 && scroll_offset > 0;
            let _ = up_button.borrow_mut().enable(enabled);
        }
        if let Some(down_button) = self.find_child_by_id(links.down_button) {
            let enabled = max_offset > 0 && scroll_offset < max_offset;
            let _ = down_button.borrow_mut().enable(enabled);
        }
        if let Some(slider) = self.find_child_by_id(links.slider) {
            let enabled = max_offset > 0;
            let _ = slider.borrow_mut().enable(enabled);
        }

        let mut content_width = bounds.width;
        if let Some(slider) = self.find_child_by_id(links.slider) {
            if !slider.borrow().is_hidden() {
                let (slider_width, _) = slider.borrow().get_size();
                content_width = content_width.saturating_sub(slider_width.max(0) as u32 + 2);
            }
        }
        if let Some(WindowWidget::ListBox(listbox)) = self.widget.as_mut() {
            listbox.set_content_width(content_width);
        }

        if let Some(thumb_id) = links.thumb {
            if let Some(thumb) = self.find_child_by_id(thumb_id) {
                if let Some(slider) = self.find_child_by_id(links.slider) {
                    let (_, slider_height) = slider.borrow().get_size();
                    let (_, thumb_height) = thumb.borrow().get_size();
                    let available = (slider_height - thumb_height).max(0);
                    let ratio = if max_offset > 0 {
                        scroll_offset as f32 / max_offset as f32
                    } else {
                        0.0
                    };
                    let thumb_y = (ratio * available as f32).round() as i32;
                    let _ = thumb.borrow_mut().set_position(0, thumb_y);
                    let _ = thumb.borrow_mut().hide(max_offset == 0);
                }
            }
        }
    }

    pub(crate) fn update_slider_thumb(&mut self) {
        let Some(thumb_id) = self.slider_thumb else {
            return;
        };
        let Some(thumb) = self.find_child_by_id(thumb_id) else {
            return;
        };
        let (thumb_w, thumb_h) = thumb.borrow().get_size();
        let (width, height) = self.get_size();

        match self.widget.as_ref() {
            Some(WindowWidget::HorizontalSlider(slider)) => {
                let (min_val, max_val) = slider.range();
                let range = (max_val - min_val).max(1);
                let track = (width - thumb_w).max(0);
                let ratio = (slider.value() - min_val) as f32 / range as f32;
                let x = (ratio * track as f32).round() as i32;
                let _ = thumb
                    .borrow_mut()
                    .set_position(x, HORIZONTAL_SLIDER_THUMB_POSITION);
            }
            Some(WindowWidget::VerticalSlider(slider)) => {
                let (min_val, max_val) = slider.range();
                let range = (max_val - min_val).max(1);
                let track = (height - thumb_h).max(0);
                let ratio = (slider.value() - min_val) as f32 / range as f32;
                let y = (ratio * track as f32).round() as i32;
                let _ = thumb.borrow_mut().set_position(0, y);
            }
            _ => {}
        }
    }

    pub(crate) fn show_tab_pane(&mut self, index: usize) {
        let panes: Vec<Rc<RefCell<GameWindow>>> = self
            .children
            .iter()
            .rev()
            .filter(|child| {
                let child = child.borrow();
                (child.inst_data.style & GWS_TAB_PANE) != 0
            })
            .cloned()
            .collect();

        if panes.is_empty() {
            return;
        }

        let mut active_index = if panes.get(index).is_some() { index } else { 0 };
        if let Some(WindowWidget::TabControl(tab_control)) = &mut self.widget {
            let tab_count = tab_control.tab_count();
            if tab_count > 0 {
                active_index = active_index.min(tab_count - 1);
            }
            active_index = active_index.min(panes.len() - 1);
            tab_control.set_active_tab_index_silent(active_index);
        }

        for pane in panes.iter() {
            let _ = pane.borrow_mut().hide(true);
        }

        if let Some(pane) = panes.get(active_index) {
            let _ = pane.borrow_mut().hide(false);
        }
    }

    fn resize_tab_panes_to_content(&mut self) {
        let Some(WindowWidget::TabControl(tab_control)) = self.widget.as_ref() else {
            return;
        };

        let (win_width, win_height) = self.get_size();
        let mut width = win_width - (2 * tab_control.pane_border());
        let mut height = win_height - (2 * tab_control.pane_border());

        if tab_control.tab_edge() == super::gadgets::tabcontrol::TP_TOP_SIDE
            || tab_control.tab_edge() == super::gadgets::tabcontrol::TP_BOTTOM_SIDE
        {
            height -= tab_control.tab_height_px();
        }
        if tab_control.tab_edge() == super::gadgets::tabcontrol::TP_LEFT_SIDE
            || tab_control.tab_edge() == super::gadgets::tabcontrol::TP_RIGHT_SIDE
        {
            width -= tab_control.tab_width_px();
        }

        let mut x = tab_control.pane_border();
        let mut y = tab_control.pane_border();
        if tab_control.tab_edge() == super::gadgets::tabcontrol::TP_LEFT_SIDE {
            x += tab_control.tab_width_px();
        }
        if tab_control.tab_edge() == super::gadgets::tabcontrol::TP_TOP_SIDE {
            y += tab_control.tab_height_px();
        }

        let panes: Vec<Rc<RefCell<GameWindow>>> = self
            .children
            .iter()
            .rev()
            .filter(|child| {
                let child = child.borrow();
                (child.inst_data.style & GWS_TAB_PANE) != 0
            })
            .cloned()
            .collect();

        for pane in panes {
            let mut pane = pane.borrow_mut();
            let _ = pane.set_size(width.max(0), height.max(0));
            let _ = pane.set_position(x, y);
        }
    }

    /// Normalize window region (ensure low < high)
    fn normalize_region(&mut self) {
        if self.region.low.x > self.region.high.x {
            std::mem::swap(&mut self.region.low.x, &mut self.region.high.x);
        }
        if self.region.low.y > self.region.high.y {
            std::mem::swap(&mut self.region.low.y, &mut self.region.high.y);
        }
    }
}

impl Default for GameWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowWidget {
    fn set_visible(&mut self, visible: bool) {
        match self {
            WindowWidget::PushButton(widget) => widget.set_visible(visible),
            WindowWidget::RadioButton(widget) => widget.set_visible(visible),
            WindowWidget::CheckBox(widget) => widget.set_visible(visible),
            WindowWidget::VerticalSlider(widget) => widget.set_visible(visible),
            WindowWidget::HorizontalSlider(widget) => widget.set_visible(visible),
            WindowWidget::ListBox(widget) => widget.set_visible(visible),
            WindowWidget::TextEntry(widget) => widget.set_visible(visible),
            WindowWidget::StaticText(widget) => widget.set_visible(visible),
            WindowWidget::ProgressBar(widget) => widget.set_visible(visible),
            WindowWidget::TabControl(widget) => widget.set_visible(visible),
            WindowWidget::ComboBox(widget) => widget.set_visible(visible),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => {}
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        match self {
            WindowWidget::PushButton(widget) => widget.set_enabled(enabled),
            WindowWidget::RadioButton(widget) => widget.set_enabled(enabled),
            WindowWidget::CheckBox(widget) => widget.set_enabled(enabled),
            WindowWidget::VerticalSlider(widget) => widget.set_enabled(enabled),
            WindowWidget::HorizontalSlider(widget) => widget.set_enabled(enabled),
            WindowWidget::ListBox(widget) => widget.set_enabled(enabled),
            WindowWidget::TextEntry(widget) => widget.set_enabled(enabled),
            WindowWidget::StaticText(widget) => widget.set_enabled(enabled),
            WindowWidget::ProgressBar(widget) => widget.set_enabled(enabled),
            WindowWidget::TabControl(widget) => widget.set_enabled(enabled),
            WindowWidget::ComboBox(widget) => widget.set_enabled(enabled),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => {}
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        match self {
            WindowWidget::PushButton(widget) => widget.handle_input(event),
            WindowWidget::RadioButton(widget) => widget.handle_input(event),
            WindowWidget::CheckBox(widget) => widget.handle_input(event),
            WindowWidget::VerticalSlider(widget) => widget.handle_input(event),
            WindowWidget::HorizontalSlider(widget) => widget.handle_input(event),
            WindowWidget::ListBox(widget) => widget.handle_input(event),
            WindowWidget::TextEntry(widget) => widget.handle_input(event),
            WindowWidget::StaticText(widget) => widget.handle_input(event),
            WindowWidget::ProgressBar(widget) => widget.handle_input(event),
            WindowWidget::TabControl(widget) => widget.handle_input(event),
            WindowWidget::ComboBox(widget) => widget.handle_input(event),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => Vec::new(),
        }
    }

    fn state(&self) -> GadgetState {
        match self {
            WindowWidget::PushButton(widget) => widget.state(),
            WindowWidget::RadioButton(widget) => widget.state(),
            WindowWidget::CheckBox(widget) => widget.state(),
            WindowWidget::VerticalSlider(widget) => widget.state(),
            WindowWidget::HorizontalSlider(widget) => widget.state(),
            WindowWidget::ListBox(widget) => widget.state(),
            WindowWidget::TextEntry(widget) => widget.state(),
            WindowWidget::StaticText(widget) => widget.state(),
            WindowWidget::ProgressBar(widget) => widget.state(),
            WindowWidget::TabControl(widget) => widget.state(),
            WindowWidget::ComboBox(widget) => widget.state(),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => GadgetState::Normal,
        }
    }
}

// Default callback implementations
pub fn legacy_default_draw_callback(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ parity: GameWinDefaultDraw is a no-op. USER/[None]/W3DNoDraw windows
    // should not fall through into a Rust-only generic image draw path.
}

pub fn default_draw_callback(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    let video_frame = _inst_data.video_buffer.as_ref().and_then(read_video_frame);
    let _ = with_ui_renderer_mut(|renderer| {
        let (x, y) = _window.get_screen_position();
        let (width, height) = _window.get_size();
        let offset = _inst_data.image_offset;
        let mut rect = UIRect::new(
            (x + offset.x) as f32,
            (y + offset.y) as f32,
            width as f32,
            height as f32,
        );
        let scale = _window.get_press_scale();
        if (scale - 1.0).abs() > f32::EPSILON {
            let cx = rect.x + rect.width * 0.5;
            let cy = rect.y + rect.height * 0.5;
            let scaled_width = rect.width * scale;
            let scaled_height = rect.height * scale;
            rect = UIRect::new(
                cx - scaled_width * 0.5,
                cy - scaled_height * 0.5,
                scaled_width,
                scaled_height,
            );
        }

        let (draw_data, text_colors) =
            if _inst_data.state.contains(WindowState::DISABLED) || !_window.is_enabled() {
                (&_inst_data.disabled_draw_data, &_inst_data.disabled_text)
            } else if _inst_data.state.contains(WindowState::HILITED) {
                (&_inst_data.hilite_draw_data, &_inst_data.hilite_text)
            } else {
                (&_inst_data.enabled_draw_data, &_inst_data.enabled_text)
            };

        if _window.get_status().contains(WindowStatus::IMAGE) {
            // C++ parity: W3DGameWinDefaultDraw ALWAYS draws the color background,
            // then overlays the image if available. Don't skip the color fill just
            // because the image is missing.
            if let Some(entry) = draw_data.first() {
                if entry.color != WIN_COLOR_UNDEFINED {
                    renderer.draw_rect(rect, color_to_rgba(entry.color), 0.0);
                }
                if entry.border_color != WIN_COLOR_UNDEFINED {
                    renderer.draw_rect_outline(rect, 1.0, color_to_rgba(entry.border_color), 0.1);
                }
            }
            if let Some(entry) = draw_data.first() {
                if let Some(image) = &entry.image {
                    let _ = ensure_client_mapped_image(&image.name);
                    let texture = {
                        let collection = get_mapped_image_collection();
                        let mut collection = collection.write();
                        if let Some(mapped) = collection.find_image_by_name_mut(&image.name) {
                            if mapped.get_gpu_texture().is_none() {
                                let _ =
                                    mapped.create_gpu_texture(renderer.device(), renderer.queue());
                            }
                            let texture = mapped.get_gpu_texture().map(|gpu| {
                                let uv = mapped.get_uv();
                                (
                                    Arc::new(gpu.view().clone()),
                                    UIRect::new(uv.min.x, uv.min.y, uv.width(), uv.height()),
                                )
                            });
                            texture
                        } else {
                            None
                        }
                    };

                    if let Some((texture, tex_rect)) = texture {
                        renderer.draw_textured_rect(
                            rect,
                            texture,
                            [1.0, 1.0, 1.0, 1.0],
                            Some(tex_rect),
                            0.0,
                        );
                    }
                }
            }
        } else {
            if let Some(entry) = draw_data.first() {
                if entry.color != WIN_COLOR_UNDEFINED {
                    renderer.draw_rect(rect, color_to_rgba(entry.color), 0.0);
                }

                if entry.border_color != WIN_COLOR_UNDEFINED {
                    renderer.draw_rect_outline(rect, 1.0, color_to_rgba(entry.border_color), 0.1);
                }
            }
        }

        if let Some(frame) = video_frame.as_ref() {
            let video_rect = UIRect::new(x as f32, y as f32, width as f32, height as f32);
            let texture = renderer.create_texture_from_rgba(frame.width, frame.height, &frame.data);
            renderer.draw_textured_rect(video_rect, texture, [1.0, 1.0, 1.0, 1.0], None, 0.0);
        }
        // C++ parity: W3DGameWinDefaultDraw does NOT draw text here.
        // Text drawing is the responsibility of gadget-specific draw callbacks
        // (e.g., W3DGadgetPushButtonDraw, W3DGadgetStaticTextDraw) which call
        // drawButtonText() explicitly. The default draw only handles image/color
        // backgrounds and video buffers.
    });
}

pub(crate) fn resolve_window_text(raw_text: &str) -> String {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let localized = GameText::fetch(trimmed);
    if localized.is_empty() {
        trimmed.to_string()
    } else {
        localized
    }
}

pub(crate) struct VideoFrameData {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) data: Vec<u8>,
}

pub(crate) fn read_video_frame(buffer: &VideoBufferHandle) -> Option<VideoFrameData> {
    let mut guard = buffer.lock();
    if !guard.valid() {
        return None;
    }
    let width = guard.width();
    let height = guard.height();
    let pitch = guard.pitch();
    if width == 0 || height == 0 || pitch == 0 {
        return None;
    }
    let byte_len = (pitch as usize).saturating_mul(height as usize);
    let ptr = guard.lock();
    if ptr.is_null() || byte_len == 0 {
        guard.unlock();
        return None;
    }
    let src = unsafe { std::slice::from_raw_parts(ptr, byte_len) };
    let data = match guard.format() {
        VideoBufferType::X8R8G8B8 => convert_x8r8g8b8(src, width, height, pitch),
        VideoBufferType::R8G8B8 => convert_r8g8b8(src, width, height, pitch),
        VideoBufferType::R5G6B5 => convert_r5g6b5(src, width, height, pitch),
        VideoBufferType::X1R5G5B5 => convert_x1r5g5b5(src, width, height, pitch),
        VideoBufferType::Unknown => None,
    };
    guard.unlock();
    data.map(|data| VideoFrameData {
        width,
        height,
        data,
    })
}

fn convert_x8r8g8b8(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(4);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let src_idx = x * 4;
            let dst_idx = (y * width as usize + x) * 4;
            let b = row[src_idx];
            let g = row[src_idx + 1];
            let r = row[src_idx + 2];
            out[dst_idx] = r;
            out[dst_idx + 1] = g;
            out[dst_idx + 2] = b;
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

fn convert_r8g8b8(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(3);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let src_idx = x * 3;
            let dst_idx = (y * width as usize + x) * 4;
            out[dst_idx] = row[src_idx];
            out[dst_idx + 1] = row[src_idx + 1];
            out[dst_idx + 2] = row[src_idx + 2];
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

fn convert_r5g6b5(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(2);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let idx = x * 2;
            let value = u16::from_le_bytes([row[idx], row[idx + 1]]);
            let r = ((value >> 11) & 0x1F) as u8;
            let g = ((value >> 5) & 0x3F) as u8;
            let b = (value & 0x1F) as u8;
            let dst_idx = (y * width as usize + x) * 4;
            out[dst_idx] = (r << 3) | (r >> 2);
            out[dst_idx + 1] = (g << 2) | (g >> 4);
            out[dst_idx + 2] = (b << 3) | (b >> 2);
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

fn convert_x1r5g5b5(src: &[u8], width: u32, height: u32, pitch: u32) -> Option<Vec<u8>> {
    let row_bytes = (width as usize).saturating_mul(2);
    let pitch = pitch as usize;
    if pitch < row_bytes {
        return None;
    }
    let mut out = vec![
        0u8;
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    ];
    for y in 0..height as usize {
        let src_row = y.saturating_mul(pitch);
        if src_row + row_bytes > src.len() {
            return None;
        }
        let row = &src[src_row..src_row + row_bytes];
        for x in 0..width as usize {
            let idx = x * 2;
            let value = u16::from_le_bytes([row[idx], row[idx + 1]]);
            let r = ((value >> 10) & 0x1F) as u8;
            let g = ((value >> 5) & 0x1F) as u8;
            let b = (value & 0x1F) as u8;
            let dst_idx = (y * width as usize + x) * 4;
            out[dst_idx] = (r << 3) | (r >> 2);
            out[dst_idx + 1] = (g << 3) | (g >> 2);
            out[dst_idx + 2] = (b << 3) | (b >> 2);
            out[dst_idx + 3] = 255;
        }
    }
    Some(out)
}

#[derive(Default, Clone)]
struct BorderPieces {
    corner_ul: Option<Image>,
    corner_ur: Option<Image>,
    corner_ll: Option<Image>,
    corner_lr: Option<Image>,
    vertical_left: Option<Image>,
    vertical_left_short: Option<Image>,
    horizontal_top: Option<Image>,
    horizontal_top_short: Option<Image>,
    vertical_right: Option<Image>,
    vertical_right_short: Option<Image>,
    horizontal_bottom: Option<Image>,
    horizontal_bottom_short: Option<Image>,
}

fn border_pieces() -> &'static BorderPieces {
    static PIECES: OnceLock<BorderPieces> = OnceLock::new();
    PIECES.get_or_init(|| {
        with_window_manager_ref(|manager| BorderPieces {
            corner_ul: manager.win_find_image("BorderCornerUL"),
            corner_ur: manager.win_find_image("BorderCornerUR"),
            corner_ll: manager.win_find_image("BorderCornerLL"),
            corner_lr: manager.win_find_image("BorderCornerLR"),
            vertical_left: manager.win_find_image("BorderLeft"),
            vertical_left_short: manager.win_find_image("BorderLeftShort"),
            horizontal_top: manager.win_find_image("BorderTop"),
            horizontal_top_short: manager.win_find_image("BorderTopShort"),
            vertical_right: manager.win_find_image("BorderRight"),
            vertical_right_short: manager.win_find_image("BorderRightShort"),
            horizontal_bottom: manager.win_find_image("BorderBottom"),
            horizontal_bottom_short: manager.win_find_image("BorderBottomShort"),
        })
    })
}

impl GameWindow {
    /// Draw W3D border art for this window (port of W3DGameWindow::winDrawBorder).
    pub fn draw_border_w3d(&self) {
        const BORDER_CORNER_SIZE: i32 = 15;
        const BORDER_LINE_SIZE: i32 = 20;
        const OFFSET: i32 = 15;
        const OFFSET_LOWER: i32 = 5;
        const HALF_SIZE: i32 = BORDER_LINE_SIZE / 2;

        let (mut original_x, mut original_y) = self.get_screen_position();
        let (mut width, mut height) = self.get_size();

        let style = self.get_style();
        let mut found = false;

        for bit in [
            GWS_PUSH_BUTTON,
            GWS_RADIO_BUTTON,
            GWS_CHECK_BOX,
            GWS_VERT_SLIDER,
            GWS_HORZ_SLIDER,
            GWS_SCROLL_LISTBOX,
            GWS_ENTRY_FIELD,
            GWS_STATIC_TEXT,
            GWS_PROGRESS_BAR,
            GWS_USER_WINDOW,
            GWS_TAB_CONTROL,
        ] {
            if style & bit == 0 {
                continue;
            }

            match bit {
                GWS_CHECK_BOX => {
                    found = true;
                }
                GWS_ENTRY_FIELD => {
                    if !self.inst_data.text.is_empty() || !self.inst_data.text_label.is_empty() {
                        let text = if !self.inst_data.text.is_empty() {
                            self.inst_data.text.as_str()
                        } else {
                            self.inst_data.text_label.as_str()
                        };
                        let mut text_width = 0;
                        with_window_manager_ref(|manager| {
                            if let Some(font) = self.inst_data.font.as_ref() {
                                manager.win_get_text_size(
                                    font,
                                    text,
                                    Some(&mut text_width),
                                    None,
                                    0,
                                );
                            }
                        });
                        width = (width - (text_width + 6)).max(0);
                        original_x += text_width + 6;
                    }

                    self.blit_border_rect(
                        original_x,
                        original_y,
                        width,
                        height,
                        OFFSET,
                        OFFSET_LOWER,
                        BORDER_LINE_SIZE,
                        HALF_SIZE,
                        BORDER_CORNER_SIZE,
                    );
                    found = true;
                }
                GWS_VERT_SLIDER | GWS_HORZ_SLIDER => {
                    found = true;
                }
                GWS_SCROLL_LISTBOX => {
                    let slider_adjustment = 0;
                    let label_adjustment = if !self.inst_data.text.is_empty()
                        || !self.inst_data.text_label.is_empty()
                    {
                        4
                    } else {
                        0
                    };

                    self.blit_border_rect(
                        original_x - 3,
                        original_y - (3 + label_adjustment),
                        width + 3 - slider_adjustment,
                        height + 6,
                        OFFSET,
                        OFFSET_LOWER,
                        BORDER_LINE_SIZE,
                        HALF_SIZE,
                        BORDER_CORNER_SIZE,
                    );
                    found = true;
                }
                GWS_RADIO_BUTTON | GWS_STATIC_TEXT | GWS_PROGRESS_BAR | GWS_PUSH_BUTTON
                | GWS_USER_WINDOW | GWS_TAB_CONTROL => {
                    self.blit_border_rect(
                        original_x,
                        original_y,
                        width,
                        height,
                        OFFSET,
                        OFFSET_LOWER,
                        BORDER_LINE_SIZE,
                        HALF_SIZE,
                        BORDER_CORNER_SIZE,
                    );
                    found = true;
                }
                _ => {}
            }

            if found {
                break;
            }
        }
    }

    fn blit_border_rect(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        offset: i32,
        offset_lower: i32,
        line_size: i32,
        half_size: i32,
        corner_size: i32,
    ) {
        let pieces = border_pieces().clone();
        let max_x = x + width;
        let max_y = y + height;

        with_window_manager_ref(|manager| {
            let mut draw_piece = |piece: &Option<Image>, x1: i32, y1: i32, x2: i32, y2: i32| {
                if let Some(image) = piece {
                    manager.win_draw_image(image, x1, y1, x2, y2, WIN_COLOR_UNDEFINED);
                }
            };

            // Horizontal lines
            let y_top = y - offset;
            let y_bottom = max_y - offset_lower;
            let mut x_iter = x + offset_lower;
            let x_end = max_x - (offset_lower + line_size);
            while x_iter <= x_end {
                draw_piece(
                    &pieces.horizontal_top,
                    x_iter,
                    y_top,
                    x_iter + line_size,
                    y_top + line_size,
                );
                draw_piece(
                    &pieces.horizontal_bottom,
                    x_iter,
                    y_bottom,
                    x_iter + line_size,
                    y_bottom + line_size,
                );
                x_iter += line_size;
            }

            let x_end_short = max_x - 5;
            if (x_end_short - x_iter) >= half_size {
                draw_piece(
                    &pieces.horizontal_top_short,
                    x_iter,
                    y_top,
                    x_iter + half_size,
                    y_top + line_size,
                );
                draw_piece(
                    &pieces.horizontal_bottom_short,
                    x_iter,
                    y_bottom,
                    x_iter + half_size,
                    y_bottom + line_size,
                );
                x_iter += half_size;
            }

            if x_iter < x_end_short {
                x_iter -= half_size - (((x_end_short - x_iter) + 1) & !1);
                draw_piece(
                    &pieces.horizontal_top_short,
                    x_iter,
                    y_top,
                    x_iter + half_size,
                    y_top + line_size,
                );
                draw_piece(
                    &pieces.horizontal_bottom_short,
                    x_iter,
                    y_bottom,
                    x_iter + half_size,
                    y_bottom + line_size,
                );
            }

            // Vertical lines
            let x_left = x - offset;
            let x_right = max_x - offset_lower;
            let mut y_iter = y + offset_lower;
            let y_end = max_y - (offset_lower + line_size);
            while y_iter <= y_end {
                draw_piece(
                    &pieces.vertical_left,
                    x_left,
                    y_iter,
                    x_left + line_size,
                    y_iter + line_size,
                );
                draw_piece(
                    &pieces.vertical_right,
                    x_right,
                    y_iter,
                    x_right + line_size,
                    y_iter + line_size,
                );
                y_iter += line_size;
            }

            let y_end_short = max_y - offset_lower;
            if (y_end_short - y_iter) >= half_size {
                draw_piece(
                    &pieces.vertical_left_short,
                    x_left,
                    y_iter,
                    x_left + line_size,
                    y_iter + half_size,
                );
                draw_piece(
                    &pieces.vertical_right_short,
                    x_right,
                    y_iter,
                    x_right + line_size,
                    y_iter + half_size,
                );
                y_iter += half_size;
            }

            if y_iter < y_end_short {
                y_iter -= half_size - (((y_end_short - y_iter) + 1) & !1);
                draw_piece(
                    &pieces.vertical_left_short,
                    x_left,
                    y_iter,
                    x_left + line_size,
                    y_iter + half_size,
                );
                draw_piece(
                    &pieces.vertical_right_short,
                    x_right,
                    y_iter,
                    x_right + line_size,
                    y_iter + half_size,
                );
            }

            // Corners
            draw_piece(
                &pieces.corner_ul,
                x - corner_size,
                y - corner_size,
                x - corner_size + line_size,
                y - corner_size + line_size,
            );
            draw_piece(
                &pieces.corner_ur,
                max_x - 5,
                y - corner_size,
                max_x - 5 + line_size,
                y - corner_size + line_size,
            );
            draw_piece(
                &pieces.corner_ll,
                x - corner_size,
                max_y - 5,
                x - corner_size + line_size,
                max_y - 5 + line_size,
            );
            draw_piece(
                &pieces.corner_lr,
                max_x - 5,
                max_y - 5,
                max_x - 5 + line_size,
                max_y - 5 + line_size,
            );
        });
    }
}

pub(crate) fn color_to_rgba(color: Color) -> [f32; 4] {
    let a = ((color >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

pub fn default_input_callback(
    _window: &GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

pub fn default_system_callback(
    _window: &GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

pub fn default_tooltip_callback(
    _window: &GameWindow,
    _inst_data: &WindowInstanceData,
    _mouse: u32,
) {
    // Default implementation does nothing
}

fn map_keycode(data: WindowMsgData) -> KeyCode {
    let key = (data & 0xFF) as u8;
    match key {
        8 => KeyCode::Backspace,
        9 => KeyCode::Tab,
        13 => KeyCode::Enter,
        27 => KeyCode::Escape,
        32 => KeyCode::Space,
        127 => KeyCode::Delete,
        b'0' => KeyCode::Num0,
        b'1' => KeyCode::Num1,
        b'2' => KeyCode::Num2,
        b'3' => KeyCode::Num3,
        b'4' => KeyCode::Num4,
        b'5' => KeyCode::Num5,
        b'6' => KeyCode::Num6,
        b'7' => KeyCode::Num7,
        b'8' => KeyCode::Num8,
        b'9' => KeyCode::Num9,
        b'a' | b'A' => KeyCode::A,
        b'b' | b'B' => KeyCode::B,
        b'c' | b'C' => KeyCode::C,
        b'd' | b'D' => KeyCode::D,
        b'e' | b'E' => KeyCode::E,
        b'f' | b'F' => KeyCode::F,
        b'g' | b'G' => KeyCode::G,
        b'h' | b'H' => KeyCode::H,
        b'i' | b'I' => KeyCode::I,
        b'j' | b'J' => KeyCode::J,
        b'k' | b'K' => KeyCode::K,
        b'l' | b'L' => KeyCode::L,
        b'm' | b'M' => KeyCode::M,
        b'n' | b'N' => KeyCode::N,
        b'o' | b'O' => KeyCode::O,
        b'p' | b'P' => KeyCode::P,
        b'q' | b'Q' => KeyCode::Q,
        b'r' | b'R' => KeyCode::R,
        b's' | b'S' => KeyCode::S,
        b't' | b'T' => KeyCode::T,
        b'u' | b'U' => KeyCode::U,
        b'v' | b'V' => KeyCode::V,
        b'w' | b'W' => KeyCode::W,
        b'x' | b'X' => KeyCode::X,
        b'y' | b'Y' => KeyCode::Y,
        b'z' | b'Z' => KeyCode::Z,
        _ => {
            let ch = key as char;
            KeyCode::Char(ch)
        }
    }
}

fn key_modifiers_from_state(state: WindowMsgData) -> KeyModifiers {
    KeyModifiers {
        shift: (state & (KEY_STATE_LSHIFT | KEY_STATE_RSHIFT)) != 0,
        ctrl: (state & (KEY_STATE_LCONTROL | KEY_STATE_RCONTROL)) != 0,
        alt: (state & (KEY_STATE_LALT | KEY_STATE_RALT)) != 0,
    }
}

fn char_input_event(key: WindowMsgData, state: WindowMsgData) -> Option<InputEvent> {
    let key = map_keycode(key);
    let modifiers = key_modifiers_from_state(state);

    if (state & KEY_STATE_DOWN) != 0 {
        Some(InputEvent::KeyDown { key, modifiers })
    } else if (state & KEY_STATE_UP) != 0 {
        Some(InputEvent::KeyUp { key, modifiers })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::gui::gadgets::tabcontrol;
    use crate::gui::gadgets::RadioButtonGroup;
    use crate::gui::gadgets::Rect;

    use super::*;

    #[test]
    fn test_window_creation() {
        let window = GameWindow::new();
        assert_eq!(window.get_id(), WINDOW_ID_INVALID);
        assert_eq!(window.get_size(), (0, 0));
        assert_eq!(window.get_position(), (0, 0));
        assert!(!window.is_enabled());
        assert!(!window.is_hidden());
    }

    #[test]
    fn test_window_properties() {
        let mut window = GameWindow::new();

        window.set_id(123);
        assert_eq!(window.get_id(), 123);

        window.set_size(100, 200).unwrap();
        assert_eq!(window.get_size(), (100, 200));

        window.set_position(10, 20).unwrap();
        assert_eq!(window.get_position(), (10, 20));

        window.set_text("Test Window").unwrap();
        assert_eq!(window.get_text(), "Test Window");
        assert_eq!(window.get_text_length(), 11);

        window.enable(true).unwrap();
        assert!(window.is_enabled());

        window.hide(true).unwrap();
        assert!(window.is_hidden());
    }

    #[test]
    fn text_length_counts_characters_like_cpp_unicode_string() {
        let mut window = GameWindow::new();

        window.set_text("Aé中").unwrap();

        assert_eq!(window.get_text_length(), 3);
        assert_eq!(window.get_text().len(), 6);
    }

    #[test]
    fn gadget_messages_route_to_owner_not_parent_like_cpp() {
        let owner_seen = Rc::new(RefCell::new(Vec::new()));
        let parent_seen = Rc::new(RefCell::new(Vec::new()));
        let owner = Rc::new(RefCell::new(GameWindow::new()));
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        let child = Rc::new(RefCell::new(GameWindow::new()));

        {
            let owner_seen = owner_seen.clone();
            owner
                .borrow_mut()
                .set_system_callback(move |_window, msg, data1, _data2| {
                    owner_seen.borrow_mut().push((msg, data1));
                    WindowMsgHandled::Handled
                });
        }
        {
            let parent_seen = parent_seen.clone();
            parent
                .borrow_mut()
                .set_system_callback(move |_window, msg, data1, _data2| {
                    parent_seen.borrow_mut().push((msg, data1));
                    WindowMsgHandled::Handled
                });
        }

        let mut button = PushButton::new(7, 0, 0, 20, 20);
        button.set_triggers_on_mouse_down(true);

        {
            let mut child = child.borrow_mut();
            child.set_id(7);
            child.enable(true).unwrap();
            child.set_parent(Some(&parent));
            child.set_owner(Some(&owner));
            child.set_widget(WindowWidget::PushButton(button));
        }
        parent.borrow_mut().enable(true).unwrap();

        assert_eq!(
            child
                .borrow_mut()
                .send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Handled
        );

        assert_eq!(
            owner_seen.borrow().as_slice(),
            &[(WindowMessage::GadgetSelected, 7)]
        );
        assert!(parent_seen.borrow().is_empty());
    }

    #[test]
    fn gadget_messages_to_self_owner_do_not_reborrow_window() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        let child = Rc::new(RefCell::new(GameWindow::new()));

        {
            let seen = seen.clone();
            child
                .borrow_mut()
                .set_system_callback(move |_window, msg, data1, _data2| {
                    seen.borrow_mut().push((msg, data1));
                    WindowMsgHandled::Handled
                });
        }

        let mut button = PushButton::new(11, 0, 0, 20, 20);
        button.set_triggers_on_mouse_down(true);

        {
            let mut child_mut = child.borrow_mut();
            child_mut.set_id(11);
            child_mut.enable(true).unwrap();
            child_mut.set_parent(Some(&parent));
            child_mut.set_owner_self(&child);
            child_mut.set_widget(WindowWidget::PushButton(button));
        }
        parent.borrow_mut().enable(true).unwrap();

        assert_eq!(
            child
                .borrow_mut()
                .send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            seen.borrow().as_slice(),
            &[(WindowMessage::GadgetSelected, 11)]
        );
    }

    #[test]
    fn set_size_sends_resized_system_message_like_cpp() {
        let mut window = GameWindow::new();
        let seen = Rc::new(RefCell::new(Vec::new()));

        {
            let seen = Rc::clone(&seen);
            window.set_system_callback(move |_, msg, data1, data2| {
                seen.borrow_mut().push((msg, data1, data2));
                WindowMsgHandled::Handled
            });
        }

        window.set_size(123, 45).unwrap();

        assert_eq!(
            seen.borrow().as_slice(),
            &[(WindowMessage::User(GGM_RESIZED), 123, 45)]
        );
    }

    #[test]
    fn text_color_getters_return_all_state_colors_like_cpp() {
        let mut window = GameWindow::new();

        window.set_enabled_text_colors(0x01020304, 0x05060708);
        window.set_disabled_text_colors(0x11121314, 0x15161718);
        window.set_ime_composite_text_colors(0x21222324, 0x25262728);
        window.set_hilite_text_colors(0x31323334, 0x35363738);

        assert_eq!(window.get_enabled_text_color(), 0x01020304);
        assert_eq!(window.get_enabled_text_border_color(), 0x05060708);
        assert_eq!(window.get_disabled_text_color(), 0x11121314);
        assert_eq!(window.get_disabled_text_border_color(), 0x15161718);
        assert_eq!(window.get_ime_composite_text_color(), 0x21222324);
        assert_eq!(window.get_ime_composite_text_border_color(), 0x25262728);
        assert_eq!(window.get_hilite_text_color(), 0x31323334);
        assert_eq!(window.get_hilite_text_border_color(), 0x35363738);
    }

    #[test]
    fn combo_box_text_color_setters_propagate_to_sub_gadgets_like_cpp() {
        let mut combo = GameWindow::new();
        combo.set_id(1);

        let edit_box = Rc::new(RefCell::new(GameWindow::new()));
        edit_box.borrow_mut().set_id(2);
        let list_box = Rc::new(RefCell::new(GameWindow::new()));
        list_box.borrow_mut().set_id(3);
        let drop_down = Rc::new(RefCell::new(GameWindow::new()));
        drop_down.borrow_mut().set_id(4);

        combo.add_child(edit_box.clone());
        combo.add_child(list_box.clone());
        combo.add_child(drop_down.clone());
        combo.set_combobox_links(ComboBoxLinks {
            drop_down: 4,
            edit_box: 2,
            list_box: 3,
        });

        combo.set_enabled_text_colors(0x11223344, 0x55667788);
        combo.set_disabled_text_colors(0x01020304, 0x05060708);
        combo.set_hilite_text_colors(0xaabbccdd, 0x12345678);
        combo.set_ime_composite_text_colors(0x87654321, 0xddccbbaa);
        combo.set_font(GameFont {
            name: "Arial".to_string(),
            size: 18,
            bold: true,
        });

        for child in [edit_box, list_box] {
            let child = child.borrow();
            assert_eq!(child.inst_data.enabled_text.color, 0x11223344);
            assert_eq!(child.inst_data.enabled_text.border_color, 0x55667788);
            assert_eq!(child.inst_data.disabled_text.color, 0x01020304);
            assert_eq!(child.inst_data.disabled_text.border_color, 0x05060708);
            assert_eq!(child.inst_data.hilite_text.color, 0xaabbccdd);
            assert_eq!(child.inst_data.hilite_text.border_color, 0x12345678);
            assert_eq!(child.inst_data.ime_composite_text.color, 0x87654321);
            assert_eq!(child.inst_data.ime_composite_text.border_color, 0xddccbbaa);
            let font = child.get_font().unwrap();
            assert_eq!(font.name, "Arial");
            assert_eq!(font.size, 18);
            assert!(font.bold);
        }

        let drop_down = drop_down.borrow();
        assert_eq!(drop_down.inst_data.enabled_text.color, 0);
        assert_eq!(drop_down.inst_data.disabled_text.color, 0);
        assert_eq!(drop_down.inst_data.hilite_text.color, 0);
        assert_eq!(drop_down.inst_data.ime_composite_text.color, 0);
        assert!(drop_down.get_font().is_none());
    }

    #[test]
    fn test_window_status_flags() {
        let mut window = GameWindow::new();

        window.set_status(WindowStatus::ENABLED | WindowStatus::ACTIVE);
        assert!(window.get_status().contains(WindowStatus::ENABLED));
        assert!(window.get_status().contains(WindowStatus::ACTIVE));

        window.clear_status(WindowStatus::ENABLED);
        assert!(!window.get_status().contains(WindowStatus::ENABLED));
        assert!(window.get_status().contains(WindowStatus::ACTIVE));
    }

    #[test]
    fn win_is_hidden_checks_only_own_status_like_cpp() {
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        let child = Rc::new(RefCell::new(GameWindow::new()));
        child.borrow_mut().set_parent(Some(&parent));
        parent.borrow_mut().add_child(child.clone());

        parent.borrow_mut().hide(true).unwrap();
        assert!(parent.borrow().is_hidden());
        assert!(!child.borrow().is_hidden());

        parent.borrow_mut().hide(false).unwrap();
        assert!(!parent.borrow().is_hidden());
        assert!(!child.borrow().is_hidden());
    }

    #[test]
    fn win_is_child_checks_full_parent_chain_like_cpp() {
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        let child = Rc::new(RefCell::new(GameWindow::new()));
        let grandchild = Rc::new(RefCell::new(GameWindow::new()));
        let sibling = Rc::new(RefCell::new(GameWindow::new()));

        child.borrow_mut().set_parent(Some(&parent));
        parent.borrow_mut().add_child(child.clone());
        grandchild.borrow_mut().set_parent(Some(&child));
        child.borrow_mut().add_child(grandchild.clone());

        assert!(parent.borrow().is_child(&child.borrow()));
        assert!(parent.borrow().is_child(&grandchild.borrow()));
        assert!(child.borrow().is_child(&grandchild.borrow()));
        assert!(!parent.borrow().is_child(&parent.borrow()));
        assert!(!parent.borrow().is_child(&sibling.borrow()));
    }

    #[test]
    fn leaf_helpers_walk_window_tree_like_cpp() {
        let root = Rc::new(RefCell::new(GameWindow::new()));
        let trailing_leaf = Rc::new(RefCell::new(GameWindow::new()));
        let branch = Rc::new(RefCell::new(GameWindow::new()));
        let branch_leaf = Rc::new(RefCell::new(GameWindow::new()));

        trailing_leaf.borrow_mut().set_parent(Some(&root));
        root.borrow_mut().add_child(trailing_leaf.clone());
        branch.borrow_mut().set_parent(Some(&root));
        root.borrow_mut().add_child(branch.clone());
        branch_leaf.borrow_mut().set_parent(Some(&branch));
        branch.borrow_mut().add_child(branch_leaf.clone());

        assert!(Rc::ptr_eq(
            &GameWindow::find_first_leaf(&trailing_leaf),
            &branch_leaf
        ));
        assert!(Rc::ptr_eq(
            &GameWindow::find_last_leaf(&branch_leaf),
            &trailing_leaf
        ));
        assert!(Rc::ptr_eq(
            &GameWindow::find_next_leaf(&branch_leaf).unwrap(),
            &trailing_leaf
        ));
        assert!(Rc::ptr_eq(
            &GameWindow::find_prev_leaf(&trailing_leaf).unwrap(),
            &branch_leaf
        ));
        assert!(Rc::ptr_eq(
            &GameWindow::find_next_leaf(&trailing_leaf).unwrap(),
            &branch_leaf
        ));
        assert!(Rc::ptr_eq(
            &GameWindow::find_prev_leaf(&branch_leaf).unwrap(),
            &trailing_leaf
        ));
    }

    #[test]
    fn leaf_helpers_stop_descent_at_tab_stop_like_cpp() {
        let root = Rc::new(RefCell::new(GameWindow::new()));
        let tab_branch = Rc::new(RefCell::new(GameWindow::new()));
        let leading_leaf = Rc::new(RefCell::new(GameWindow::new()));
        let child_under_tab = Rc::new(RefCell::new(GameWindow::new()));

        tab_branch.borrow_mut().set_status(WindowStatus::TAB_STOP);
        tab_branch.borrow_mut().set_parent(Some(&root));
        root.borrow_mut().add_child(tab_branch.clone());
        leading_leaf.borrow_mut().set_parent(Some(&root));
        root.borrow_mut().add_child(leading_leaf.clone());
        child_under_tab.borrow_mut().set_parent(Some(&tab_branch));
        tab_branch.borrow_mut().add_child(child_under_tab.clone());

        assert!(Rc::ptr_eq(
            &GameWindow::find_next_leaf(&leading_leaf).unwrap(),
            &tab_branch
        ));
        assert!(Rc::ptr_eq(
            &GameWindow::find_prev_leaf(&leading_leaf).unwrap(),
            &child_under_tab
        ));
    }

    #[test]
    fn show_tab_pane_falls_back_and_updates_active_tab_like_cpp() {
        let mut tab_window = GameWindow::new();
        let mut tab_control = TabControl::new(7, 0, 0, 100, 80);
        tab_control.set_tab_data(TabControlData {
            tab_count: 2,
            ..Default::default()
        });
        tab_window.set_widget(WindowWidget::TabControl(tab_control));

        let first_pane = Rc::new(RefCell::new(GameWindow::new()));
        first_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;
        let second_pane = Rc::new(RefCell::new(GameWindow::new()));
        second_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;

        tab_window.add_child(first_pane.clone());
        tab_window.add_child(second_pane.clone());

        tab_window.show_tab_pane(1);
        assert!(first_pane.borrow().is_hidden());
        assert!(!second_pane.borrow().is_hidden());

        tab_window.show_tab_pane(7);
        assert!(!first_pane.borrow().is_hidden());
        assert!(second_pane.borrow().is_hidden());

        let Some(WindowWidget::TabControl(tab_control)) = tab_window.widget() else {
            panic!("expected tab control widget");
        };
        assert_eq!(tab_control.active_tab_index(), 0);
    }

    #[test]
    fn resizing_tab_control_resizes_panes_like_cpp() {
        let mut tab_window = GameWindow::new();
        let mut tab_control = TabControl::new(7, 0, 0, 100, 80);
        tab_control.set_tab_data(TabControlData {
            tab_edge: tabcontrol::TP_TOP_SIDE,
            tab_height: 20,
            tab_count: 2,
            pane_border: 3,
            ..Default::default()
        });
        tab_window.set_widget(WindowWidget::TabControl(tab_control));

        let first_pane = Rc::new(RefCell::new(GameWindow::new()));
        first_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;
        let second_pane = Rc::new(RefCell::new(GameWindow::new()));
        second_pane.borrow_mut().instance_data_mut().style |= GWS_TAB_PANE;
        tab_window.add_child(first_pane.clone());
        tab_window.add_child(second_pane.clone());

        tab_window.set_size(200, 100).unwrap();

        assert_eq!(first_pane.borrow().get_position(), (3, 23));
        assert_eq!(first_pane.borrow().get_size(), (194, 74));
        assert_eq!(second_pane.borrow().get_position(), (3, 23));
        assert_eq!(second_pane.borrow().get_size(), (194, 74));

        let Some(WindowWidget::TabControl(tab_control)) = tab_window.widget() else {
            panic!("expected tab control widget");
        };
        assert_eq!(tab_control.content_bounds(), Rect::new(3, 23, 194, 74));
    }

    #[test]
    fn progress_bar_system_message_matches_cpp_range_rules() {
        let mut window = GameWindow::new();
        window.set_widget(WindowWidget::ProgressBar(ProgressBar::new(
            42, 0, 0, 100, 10,
        )));

        assert_eq!(
            window.send_system_message(WindowMessage::User(GPM_SET_PROGRESS), 37, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(window.progress_bar_mut().unwrap().percentage(), 37.0);

        assert_eq!(
            window.send_system_message(WindowMessage::User(GPM_SET_PROGRESS), 101, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(window.progress_bar_mut().unwrap().percentage(), 37.0);

        assert_eq!(
            window.send_system_message(WindowMessage::User(GPM_SET_PROGRESS), (-1i32) as u32, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(window.progress_bar_mut().unwrap().percentage(), 37.0);
    }

    #[test]
    fn slider_system_messages_match_cpp_numeric_rules() {
        let mut window = GameWindow::new();
        window.set_widget(WindowWidget::HorizontalSlider(
            HorizontalSlider::new(7, 0, 0, 100, 20).with_range(0, 100),
        ));
        let thumb = Rc::new(RefCell::new(GameWindow::new()));
        thumb.borrow_mut().set_id(77);
        thumb.borrow_mut().set_size(13, 16).unwrap();
        window.add_child(thumb.clone());
        window.set_slider_thumb(77);

        window.set_size(100, 24).unwrap();
        assert_eq!(thumb.borrow().get_size(), (GADGET_SIZE, 24));

        assert_eq!(
            window.send_system_message(WindowMessage::User(GSM_SET_MIN_MAX), 10, 20),
            WindowMsgHandled::Handled
        );
        assert_eq!(window.horizontal_slider_mut().unwrap().range(), (10, 20));
        assert_eq!(window.horizontal_slider_mut().unwrap().value(), 10);
        assert_eq!(thumb.borrow().get_position(), (0, HORIZONTAL_SLIDER_THUMB_POSITION));

        assert_eq!(
            window.send_system_message(WindowMessage::User(GSM_SET_SLIDER), 15, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(window.horizontal_slider_mut().unwrap().value(), 15);
        let position_after_valid_set = thumb.borrow().get_position();

        assert_eq!(
            window.send_system_message(WindowMessage::User(GSM_SET_SLIDER), 21, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(window.horizontal_slider_mut().unwrap().value(), 15);
        assert_eq!(thumb.borrow().get_position(), position_after_valid_set);
    }

    #[test]
    fn radio_set_selection_system_message_matches_cpp_notify_rules() {
        let owner_seen = Rc::new(RefCell::new(Vec::new()));
        let owner = Rc::new(RefCell::new(GameWindow::new()));
        {
            let owner_seen = owner_seen.clone();
            owner
                .borrow_mut()
                .set_system_callback(move |_, msg, data1, _| {
                    owner_seen.borrow_mut().push((msg, data1));
                    WindowMsgHandled::Handled
                });
        }

        let mut silent_window = GameWindow::new();
        silent_window.set_id(17);
        silent_window.set_owner(Some(&owner));
        silent_window.set_widget(WindowWidget::RadioButton(RadioButton::new(
            17,
            0,
            0,
            16,
            RadioButtonGroup::new(2),
        )));

        assert_eq!(
            silent_window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 0, 0),
            WindowMsgHandled::Handled
        );
        assert!(matches!(
            silent_window.widget(),
            Some(WindowWidget::RadioButton(radio)) if radio.is_selected()
        ));
        assert!(owner_seen.borrow().is_empty());

        let mut notifying_window = GameWindow::new();
        notifying_window.set_id(18);
        notifying_window.set_owner(Some(&owner));
        notifying_window.set_widget(WindowWidget::RadioButton(RadioButton::new(
            18,
            0,
            0,
            16,
            RadioButtonGroup::new(3),
        )));

        assert_eq!(
            notifying_window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 1, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            owner_seen.borrow().as_slice(),
            &[(WindowMessage::GadgetSelected, 18)]
        );

        assert_eq!(
            notifying_window.send_system_message(WindowMessage::User(GBM_SET_SELECTION), 1, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(owner_seen.borrow().len(), 1);
    }

    #[test]
    fn input_focus_notifies_owner_and_updates_hilite_like_cpp_gadgets() {
        let owner_seen = Rc::new(RefCell::new(Vec::new()));
        let owner = Rc::new(RefCell::new(GameWindow::new()));
        {
            let owner_seen = owner_seen.clone();
            owner
                .borrow_mut()
                .set_system_callback(move |_, msg, data1, data2| {
                    owner_seen.borrow_mut().push((msg, data1, data2));
                    WindowMsgHandled::Handled
                });
        }

        let mut window = GameWindow::new();
        window.set_id(31);
        window.set_owner(Some(&owner));
        window.set_widget(WindowWidget::CheckBox(CheckBox::new(31, 0, 0, 16)));

        assert_eq!(
            window.send_system_message(WindowMessage::InputFocus, 1, 0),
            WindowMsgHandled::Handled
        );
        assert!(window
            .instance_data()
            .state
            .contains(WindowState::HILITED));

        assert_eq!(
            window.send_system_message(WindowMessage::InputFocus, 0, 0),
            WindowMsgHandled::Handled
        );
        assert!(!window
            .instance_data()
            .state
            .contains(WindowState::HILITED));

        assert_eq!(
            owner_seen.borrow().as_slice(),
            &[
                (WindowMessage::User(GGM_FOCUS_CHANGE), 1, 31),
                (WindowMessage::User(GGM_FOCUS_CHANGE), 0, 31),
            ]
        );
    }

    #[test]
    fn listbox_delete_system_messages_match_cpp_state_rules() {
        let mut window = GameWindow::new();
        let mut listbox = ListBox::new(42, 0, 0, 100, 60);
        listbox.add_item("alpha");
        listbox.add_item("bravo");
        listbox.add_item("charlie");
        assert!(listbox.select_index(2, KeyModifiers::none()));
        window.set_widget(WindowWidget::ListBox(listbox));

        assert_eq!(
            window.send_system_message(WindowMessage::User(GLM_DEL_ENTRY), 1, 0),
            WindowMsgHandled::Handled
        );
        let listbox = window.list_box_mut().unwrap();
        assert_eq!(listbox.items().len(), 2);
        assert_eq!(listbox.items()[1].text, "charlie");
        assert_eq!(listbox.selected_indices(), &[1]);

        assert_eq!(
            window.send_system_message(WindowMessage::User(GLM_DEL_ENTRY), 99, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(window.list_box_mut().unwrap().items().len(), 2);

        assert_eq!(
            window.send_system_message(WindowMessage::User(GLM_DEL_ALL), 0, 0),
            WindowMsgHandled::Handled
        );
        let listbox = window.list_box_mut().unwrap();
        assert!(listbox.items().is_empty());
        assert!(listbox.selected_indices().is_empty());
        assert_eq!(listbox.scroll_offset(), 0);
    }

    #[test]
    fn test_window_region() {
        let region = WindowRegion::new(10, 20, 100, 200);
        assert_eq!(region.low.x, 10);
        assert_eq!(region.low.y, 20);
        assert_eq!(region.high.x, 110);
        assert_eq!(region.high.y, 220);
        assert_eq!(region.width(), 100);
        assert_eq!(region.height(), 200);

        assert!(region.contains_point(50, 100));
        assert!(!region.contains_point(5, 100));
        assert!(!region.contains_point(50, 250));
    }

    #[test]
    fn test_point_in_window() {
        let mut window = GameWindow::new();
        window.set_position(10, 10).unwrap();
        window.set_size(100, 100).unwrap();

        assert!(window.point_in_window(50, 50));
        assert!(!window.point_in_window(5, 50));
        assert!(!window.point_in_window(150, 50));
    }

    #[test]
    fn point_in_child_returns_deepest_enabled_visible_child_like_cpp() {
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        let child = Rc::new(RefCell::new(GameWindow::new()));
        let grandchild = Rc::new(RefCell::new(GameWindow::new()));

        parent.borrow_mut().set_position(10, 10).unwrap();
        parent.borrow_mut().set_size(100, 100).unwrap();
        parent.borrow_mut().enable(true).unwrap();

        child.borrow_mut().set_position(5, 5).unwrap();
        child.borrow_mut().set_size(40, 40).unwrap();
        child.borrow_mut().enable(true).unwrap();
        child.borrow_mut().set_parent(Some(&parent));
        parent.borrow_mut().add_child(child.clone());

        grandchild.borrow_mut().set_position(4, 4).unwrap();
        grandchild.borrow_mut().set_size(10, 10).unwrap();
        grandchild.borrow_mut().enable(true).unwrap();
        grandchild.borrow_mut().set_parent(Some(&child));
        child.borrow_mut().add_child(grandchild.clone());

        let found = GameWindow::point_in_child(&parent, 20, 20, false);
        assert!(Rc::ptr_eq(&found, &grandchild));

        grandchild.borrow_mut().enable(false).unwrap();
        let found = GameWindow::point_in_child(&parent, 20, 20, false);
        assert!(Rc::ptr_eq(&found, &child));

        let found = GameWindow::point_in_child(&parent, 20, 20, true);
        assert!(Rc::ptr_eq(&found, &grandchild));
    }

    #[test]
    fn point_in_any_child_matches_hidden_and_disabled_cpp_rules() {
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        let child = Rc::new(RefCell::new(GameWindow::new()));

        parent.borrow_mut().set_position(0, 0).unwrap();
        parent.borrow_mut().set_size(100, 100).unwrap();
        parent.borrow_mut().enable(true).unwrap();

        child.borrow_mut().set_position(10, 10).unwrap();
        child.borrow_mut().set_size(20, 20).unwrap();
        child.borrow_mut().enable(false).unwrap();
        child.borrow_mut().set_parent(Some(&parent));
        parent.borrow_mut().add_child(child.clone());

        let found = GameWindow::point_in_child(&parent, 15, 15, false);
        assert!(Rc::ptr_eq(&found, &parent));

        let found = GameWindow::point_in_any_child(&parent, 15, 15, true, false);
        assert!(Rc::ptr_eq(&found, &child));

        child.borrow_mut().hide(true).unwrap();
        let found = GameWindow::point_in_any_child(&parent, 15, 15, true, false);
        assert!(Rc::ptr_eq(&found, &parent));

        let found = GameWindow::point_in_any_child(&parent, 15, 15, false, false);
        assert!(Rc::ptr_eq(&found, &child));
    }

    #[test]
    fn test_user_data() {
        let mut window = GameWindow::new();

        window.set_user_data(42i32);
        assert_eq!(window.get_user_data::<i32>(), Some(&42));
        assert_eq!(window.get_user_data::<String>(), None);

        window.set_user_data("test".to_string());
        assert_eq!(window.get_user_data::<String>(), Some(&"test".to_string()));
        assert_eq!(window.get_user_data::<i32>(), None);
    }

    #[test]
    fn edit_data_stores_gui_editor_callback_names_like_cpp() {
        let mut window = GameWindow::new();
        assert!(window.get_edit_data().is_none());

        let edit_data = GameWindowEditData {
            system_callback_string: "System".to_string(),
            input_callback_string: "Input".to_string(),
            tooltip_callback_string: "Tooltip".to_string(),
            draw_callback_string: "Draw".to_string(),
        };

        window.set_edit_data(Some(edit_data.clone()));
        assert_eq!(window.get_edit_data(), Some(&edit_data));

        window.set_edit_data(None);
        assert!(window.get_edit_data().is_none());
    }

    #[test]
    fn test_callbacks() {
        let mut window = GameWindow::new();
        window.set_status(WindowStatus::ENABLED);

        // Test input callback
        window.set_input_callback(|_win, msg, _d1, _d2| match msg {
            WindowMessage::LeftDown => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        });

        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            window.send_input_message(WindowMessage::RightDown, 0, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn callback_getters_return_installed_handlers_like_cpp() {
        let window = GameWindow::new();

        assert!(window.get_draw_callback().is_some());
        assert!(window.get_tooltip_callback().is_none());
        assert_eq!(
            window
                .get_input_callback()
                .unwrap()(&window, WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            window
                .get_system_callback()
                .unwrap()(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn callback_resets_restore_default_handlers_like_cpp_null_setters() {
        let mut window = GameWindow::new();
        window.set_status(WindowStatus::ENABLED);
        let drawn = Rc::new(RefCell::new(0));

        {
            let drawn = drawn.clone();
            window.set_draw_callback(move |_, _| {
                *drawn.borrow_mut() += 1;
            });
        }
        window.set_input_callback(|_, _, _, _| WindowMsgHandled::Handled);
        window.set_system_callback(|_, _, _, _| WindowMsgHandled::Handled);
        window.set_tooltip_callback(|_, _, _| {});

        window.draw();
        assert_eq!(*drawn.borrow(), 1);
        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            window.send_system_message(WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );

        window.reset_draw_callback();
        window.reset_input_callback();
        window.reset_system_callback();
        window.clear_tooltip_callback();

        window.draw();
        assert_eq!(*drawn.borrow(), 1);
        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            window.send_system_message(WindowMessage::Create, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert!(window.get_tooltip_callback().is_none());
    }

    #[test]
    fn set_callbacks_updates_input_draw_and_tooltip_like_cpp() {
        let mut window = GameWindow::new();
        window.set_status(WindowStatus::ENABLED);
        let drawn = Rc::new(RefCell::new(0));
        let tooltip_seen = Rc::new(RefCell::new(0));

        let draw: DrawCallback = {
            let drawn = drawn.clone();
            Box::new(move |_, _| {
                *drawn.borrow_mut() += 1;
            })
        };
        let tooltip: TooltipCallback = {
            let tooltip_seen = tooltip_seen.clone();
            Box::new(move |_, _, mouse| {
                *tooltip_seen.borrow_mut() = mouse;
            })
        };

        window.set_callbacks(
            Some(Box::new(|_, _, _, _| WindowMsgHandled::Handled)),
            Some(draw),
            Some(tooltip),
        );

        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Handled
        );
        window.draw();
        assert_eq!(*drawn.borrow(), 1);
        window
            .get_tooltip_callback()
            .unwrap()(&window, window.instance_data(), 42);
        assert_eq!(*tooltip_seen.borrow(), 42);

        window.set_callbacks(None, None, None);
        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Ignored
        );
        window.draw();
        assert_eq!(*drawn.borrow(), 1);
        assert!(window.get_tooltip_callback().is_none());
    }

    #[test]
    fn destroyed_window_ignores_non_destroy_system_messages_like_cpp() {
        let mut window = GameWindow::new();
        let seen = Rc::new(RefCell::new(Vec::new()));

        {
            let seen = Rc::clone(&seen);
            window.set_system_callback(move |_, msg, _, _| {
                seen.borrow_mut().push(msg);
                WindowMsgHandled::Handled
            });
        }
        window.set_status_exact(WindowStatus::ENABLED | WindowStatus::DESTROYED);

        assert_eq!(
            window.send_system_message(WindowMessage::Create, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            window.send_system_message(WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(seen.borrow().as_slice(), &[WindowMessage::Destroy]);
    }

    #[test]
    fn destroyed_window_ignores_non_destroy_input_messages_like_cpp() {
        let mut window = GameWindow::new();
        let seen = Rc::new(RefCell::new(Vec::new()));

        {
            let seen = Rc::clone(&seen);
            window.set_input_callback(move |_, msg, _, _| {
                seen.borrow_mut().push(msg);
                WindowMsgHandled::Handled
            });
        }
        window.set_status_exact(WindowStatus::DESTROYED);

        assert_eq!(
            window.send_input_message(WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            window.send_input_message(WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(seen.borrow().as_slice(), &[WindowMessage::Destroy]);
    }

    #[test]
    fn char_key_state_up_maps_to_widget_key_up() {
        let mut window = GameWindow::new();
        window.set_id(7);
        window.set_status(WindowStatus::ENABLED);
        window.set_widget(WindowWidget::PushButton(PushButton::new(7, 0, 0, 100, 30)));
        window.set_system_callback(|_, msg, data1, _| {
            if msg == WindowMessage::GadgetSelected && data1 == 7 {
                WindowMsgHandled::Handled
            } else {
                WindowMsgHandled::Ignored
            }
        });
        if let Some(WindowWidget::PushButton(button)) = window.widget_mut() {
            button.set_focus(true);
        }

        assert_eq!(
            window.send_input_message(WindowMessage::Char, 13, KEY_STATE_DOWN),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            window.send_input_message(WindowMessage::Char, 13, KEY_STATE_UP),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            window.send_input_message(WindowMessage::Char, 13, 0),
            WindowMsgHandled::Ignored
        );
    }
}
