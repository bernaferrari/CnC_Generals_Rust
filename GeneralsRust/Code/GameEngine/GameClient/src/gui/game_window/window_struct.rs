//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

use crate::gui::display_string::DisplayStringHandle;
use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    StaticText, TabControl, TextEntry, VerticalSlider,
};
use crate::video_buffer::VideoBufferHandle;

use super::callbacks::{
    default_input_callback, default_system_callback, legacy_default_draw_callback,
};
use super::font::{
    Color, GameFont, Image, Point2D, WindowDrawData, WindowInstanceData, WindowRegion, WindowState,
    WindowTextColors,
};
use super::messages::{WINDOW_ID_INVALID, WindowMessage, WindowMsgHandled, WindowStatus};
use super::payload::{WindowId, WindowMsgData};

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
    pub(crate) id: WindowId,
    pub(crate) status: WindowStatus,
    pub(crate) size: Point2D,
    pub(crate) region: WindowRegion,
    pub(crate) cursor_pos: Cell<Point2D>,

    // Instance data
    pub(crate) inst_data: WindowInstanceData,

    // User data
    pub(crate) user_data: Option<Box<dyn std::any::Any>>,
    pub(crate) edit_data: Option<GameWindowEditData>,

    // Hierarchy
    pub(crate) parent: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) children: Vec<Rc<RefCell<GameWindow>>>,
    pub(crate) next_sibling: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) prev_sibling: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) owner_is_self: bool,

    // Layout information
    pub(crate) next_in_layout: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) prev_in_layout: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) layout: Option<Weak<RefCell<crate::gui::WindowLayout>>>,

    // Callbacks
    pub(crate) callbacks: WindowCallbacks,

    // Optional gadget backing this window
    pub(crate) widget: Option<WindowWidget>,

    // Combo box child window references (drop-down, edit, list)
    pub(crate) combobox_links: Option<ComboBoxLinks>,

    // List box scrollbar child window references
    pub(crate) listbox_links: Option<ListBoxLinks>,

    // Slider thumb child window reference
    pub(crate) slider_thumb: Option<WindowId>,

    // Press animation state for elastic button feel
    pub(crate) press_scale: f32,
    pub(crate) press_scale_target: f32,
    pub(crate) press_scale_velocity: f32,
    pub(crate) press_spring_strength: f32,
    pub(crate) press_spring_damping: f32,
    pub(crate) press_impulse: f32,
    pub(crate) release_impulse: f32,
    pub(crate) press_was_down: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ComboBoxLinks {
    pub drop_down: WindowId,
    pub edit_box: WindowId,
    pub list_box: WindowId,
}

#[derive(Debug, Clone, Copy, Default)]
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

impl Default for GameWindow {
    fn default() -> Self {
        Self::new()
    }
}
