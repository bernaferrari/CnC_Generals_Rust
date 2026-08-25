//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use bitflags::bitflags;

use crate::gui::gadgets::Color as GadgetColor;

use super::payload::{WindowId, WindowMsgData, write_bool_payload};

/// Result type for window operations
pub type WindowResult<T> = Result<T, WindowError>;

/// Invalid window ID constant
pub const WINDOW_ID_INVALID: WindowId = 0;

/// Undefined color constant
pub const WIN_COLOR_UNDEFINED: u32 = 0x00FF_FFFF;

/// Gadget system message IDs
pub(crate) const GGM_LEFT_DRAG: u32 = 16384;
pub(crate) const GGM_SET_LABEL: u32 = GGM_LEFT_DRAG + 1;
pub(crate) const GGM_GET_LABEL: u32 = GGM_LEFT_DRAG + 2;
pub(crate) const GGM_FOCUS_CHANGE: u32 = GGM_LEFT_DRAG + 3;
pub(crate) const GGM_RESIZED: u32 = GGM_LEFT_DRAG + 4;
pub(crate) const GBM_SET_SELECTION: u32 = GGM_LEFT_DRAG + 10;
pub const GSM_SLIDER_TRACK: u32 = GGM_LEFT_DRAG + 11;

pub(crate) const GSM_SET_SLIDER: u32 = GGM_LEFT_DRAG + 12;
pub(crate) const GSM_SET_MIN_MAX: u32 = GGM_LEFT_DRAG + 13;
pub(crate) const GLM_ADD_ENTRY: u32 = GGM_LEFT_DRAG + 15;
pub(crate) const GLM_DEL_ENTRY: u32 = GGM_LEFT_DRAG + 16;
pub(crate) const GLM_DEL_ALL: u32 = GGM_LEFT_DRAG + 17;
pub const GLM_SELECTED: u32 = GGM_LEFT_DRAG + 18;
pub const GLM_DOUBLE_CLICKED: u32 = GGM_LEFT_DRAG + 19;
pub const GLM_RIGHT_CLICKED: u32 = GGM_LEFT_DRAG + 20;
pub(crate) const GLM_SET_SELECTION: u32 = GGM_LEFT_DRAG + 21;
pub(crate) const GLM_GET_SELECTION: u32 = GGM_LEFT_DRAG + 22;
pub(crate) const GLM_TOGGLE_MULTI_SELECTION: u32 = GGM_LEFT_DRAG + 23;
pub(crate) const GLM_GET_TEXT: u32 = GGM_LEFT_DRAG + 24;
pub(crate) const GLM_SET_UP_BUTTON: u32 = GGM_LEFT_DRAG + 25;
pub(crate) const GLM_SET_DOWN_BUTTON: u32 = GGM_LEFT_DRAG + 26;
pub(crate) const GLM_SET_SLIDER: u32 = GGM_LEFT_DRAG + 27;
pub(crate) const GLM_SCROLL_BUFFER: u32 = GGM_LEFT_DRAG + 28;
pub(crate) const GLM_UPDATE_DISPLAY: u32 = GGM_LEFT_DRAG + 29;
pub(crate) const GLM_GET_ITEM_DATA: u32 = GGM_LEFT_DRAG + 30;
pub(crate) const GLM_SET_ITEM_DATA: u32 = GGM_LEFT_DRAG + 31;
pub(crate) const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;
pub const GCM_ADD_ENTRY: u32 = GGM_LEFT_DRAG + 32;
pub const GCM_DEL_ENTRY: u32 = GGM_LEFT_DRAG + 33;
pub const GCM_DEL_ALL: u32 = GGM_LEFT_DRAG + 34;
pub const GCM_SELECTED: u32 = GGM_LEFT_DRAG + 35;
pub const GCM_GET_TEXT: u32 = GGM_LEFT_DRAG + 36;
pub const GCM_SET_TEXT: u32 = GGM_LEFT_DRAG + 37;
pub const GCM_EDIT_DONE: u32 = GGM_LEFT_DRAG + 38;
pub const GCM_GET_ITEM_DATA: u32 = GGM_LEFT_DRAG + 39;
pub const GCM_SET_ITEM_DATA: u32 = GGM_LEFT_DRAG + 40;
pub const GCM_GET_SELECTION: u32 = GGM_LEFT_DRAG + 41;
pub const GCM_SET_SELECTION: u32 = GGM_LEFT_DRAG + 42;
pub const GCM_UPDATE_TEXT: u32 = GGM_LEFT_DRAG + 43;
pub(crate) const GEM_GET_TEXT: u32 = GGM_LEFT_DRAG + 44;
pub(crate) const GEM_SET_TEXT: u32 = GGM_LEFT_DRAG + 45;
pub(crate) const GEM_EDIT_DONE: u32 = GGM_LEFT_DRAG + 46;
pub(crate) const GEM_UPDATE_TEXT: u32 = GGM_LEFT_DRAG + 47;
pub(crate) const GPM_SET_PROGRESS: u32 = GGM_LEFT_DRAG + 48;

pub(crate) fn shell_color_from_packed_arg(value: WindowMsgData) -> crate::gui::shell::Color {
    let value = value as u32;
    crate::gui::shell::Color::new(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
        ((value >> 24) & 0xFF) as u8,
    )
}

pub(crate) fn gadget_color_from_shell_color(color: crate::gui::shell::Color) -> GadgetColor {
    GadgetColor::rgba(color.r, color.g, color.b, color.a)
}

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

pub(crate) const HORIZONTAL_SLIDER_THUMB_POSITION: i32 = 10;
pub(crate) const HORIZONTAL_SLIDER_THUMB_WIDTH: i32 = 13;

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
    Ignored,
    Handled,
    Value(WindowMsgData),
}

impl WindowMsgHandled {
    pub fn is_ignored(self) -> bool {
        matches!(self, Self::Ignored)
    }

    pub fn is_handled(self) -> bool {
        !self.is_ignored()
    }

    pub fn value(self) -> Option<WindowMsgData> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }

    pub fn value_i32(self) -> Option<i32> {
        self.value().map(|value| value as i32)
    }
}

pub fn write_input_focus_response(
    data1: WindowMsgData,
    data2: WindowMsgData,
    wants_focus: bool,
) -> WindowMsgHandled {
    if data1 != 0 {
        let _ = write_bool_payload(data2, wants_focus);
    }
    WindowMsgHandled::Handled
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
