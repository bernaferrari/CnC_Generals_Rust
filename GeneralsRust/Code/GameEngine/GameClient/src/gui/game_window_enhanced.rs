//! Enhanced GameWindow Implementation
//!
//! Complete implementation of the GameWindow system matching the original C++
//! implementation with full event handling, hierarchy management, and rendering support.

use std::collections::HashMap;
use std::sync::{Arc, Weak, RwLock, Mutex};
use std::cell::RefCell;
use std::rc::Rc;
use bitflags::bitflags;
use thiserror::Error;

use super::ui_renderer::{UIRenderer, UIRect, TextLayout, TextAlignment, VerticalAlignment};
use glam::Vec2;
use crate::display::image::get_mapped_image_collection;
use crate::gui::game_window::{
    WindowWidget, GWS_CHECK_BOX, GWS_COMBO_BOX, GWS_ENTRY_FIELD, GWS_HORZ_SLIDER, GWS_PROGRESS_BAR,
    GWS_PUSH_BUTTON, GWS_RADIO_BUTTON, GWS_SCROLL_LISTBOX, GWS_STATIC_TEXT, GWS_TAB_CONTROL,
    GWS_TAB_PANE, GWS_VERT_SLIDER,
};
use crate::gui::gadgets::{
    GadgetMessage, GadgetState, GadgetValue, InputEvent, KeyCode, KeyModifiers, MouseButton,
};

/// Enhanced GameWindow errors
#[derive(Error, Debug)]
pub enum GameWindowError {
    #[error("Invalid window ID: {0}")]
    InvalidWindowId(i32),
    #[error("Window hierarchy error: {0}")]
    HierarchyError(String),
    #[error("Event handling error: {0}")]
    EventError(String),
    #[error("Rendering error: {0}")]
    RenderError(String),
    #[error("Window creation failed: {0}")]
    CreationFailed(String),
}

impl From<crate::gui::UIRendererError> for GameWindowError {
    fn from(err: crate::gui::UIRendererError) -> Self {
        GameWindowError::RenderError(err.to_string())
    }
}

type Result<T> = std::result::Result<T, GameWindowError>;

/// Window ID type for uniquely identifying windows
pub type WindowId = i32;

/// Window message data type
pub type WindowMsgData = u32;

/// Gadget system message IDs
const GGM_LEFT_DRAG: u32 = 16384;

/// Invalid window ID constant
pub const WINDOW_ID_INVALID: WindowId = 0;
const HORIZONTAL_SLIDER_THUMB_POSITION: i32 = 10;

bitflags! {
    /// Window status flags matching C++ implementation
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Window draw data for different states
#[derive(Debug, Clone)]
pub struct WindowDrawData {
    pub enabled: Option<String>,
    pub disabled: Option<String>,
    pub hilited: Option<String>,
    pub pushed: Option<String>,
    pub enabled_color: [f32; 4],
    pub enabled_border: [f32; 4],
    pub disabled_color: [f32; 4],
    pub disabled_border: [f32; 4],
    pub hilited_color: [f32; 4],
    pub hilited_border: [f32; 4],
    pub pushed_color: [f32; 4],
    pub pushed_border: [f32; 4],
}

/// Window text color configuration
#[derive(Debug, Clone, Copy)]
pub struct WindowTextColors {
    pub enabled: [f32; 4],
    pub disabled: [f32; 4],
    pub hilited: [f32; 4],
    pub pushed: [f32; 4],
    pub enabled_border: [f32; 4],
    pub disabled_border: [f32; 4],
    pub hilited_border: [f32; 4],
    pub pushed_border: [f32; 4],
}

impl Default for WindowTextColors {
    fn default() -> Self {
        Self {
            enabled: [1.0, 1.0, 1.0, 1.0],
            disabled: [0.5, 0.5, 0.5, 1.0],
            hilited: [1.0, 1.0, 0.0, 1.0],
            pushed: [0.8, 0.8, 0.8, 1.0],
            enabled_border: [0.0, 0.0, 0.0, 0.0],
            disabled_border: [0.0, 0.0, 0.0, 0.0],
            hilited_border: [0.0, 0.0, 0.0, 0.0],
            pushed_border: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Window event callbacks
pub trait WindowCallbacks: Send + Sync {
    /// Called when the window needs to be drawn
    fn on_draw(&self, window: &EnhancedGameWindow, renderer: &mut UIRenderer) -> Result<()> {
        Ok(())
    }
    
    /// Called for input events
    fn on_input(&self, window: &EnhancedGameWindow, message: WindowMessage, wparam: WindowMsgData, lparam: WindowMsgData) -> WindowMsgHandled {
        WindowMsgHandled::Ignored
    }
    
    /// Called for system events
    fn on_system(&self, window: &EnhancedGameWindow, message: WindowMessage, wparam: WindowMsgData, lparam: WindowMsgData) -> WindowMsgHandled {
        WindowMsgHandled::Ignored
    }
    
    /// Called to show tooltip
    fn on_tooltip(&self, window: &EnhancedGameWindow, tooltip_time: u32) {}
}

/// Enhanced GameWindow implementation
pub struct EnhancedGameWindow {
    // Core properties
    id: WindowId,
    name: String,
    status: RwLock<WindowStatus>,
    window_type: RwLock<String>,
    style: RwLock<u32>,
    
    // Position and size
    position: RwLock<(i32, i32)>,
    size: RwLock<(i32, i32)>,
    
    // Hierarchy
    parent: RwLock<Option<Weak<EnhancedGameWindow>>>,
    children: RwLock<Vec<Arc<EnhancedGameWindow>>>,
    
    // Visual properties
    text: RwLock<String>,
    text_colors: RwLock<WindowTextColors>,
    draw_data: RwLock<WindowDrawData>,
    font_name: RwLock<String>,
    font_size: RwLock<i32>,
    
    // Event handling
    callbacks: RwLock<Option<Box<dyn WindowCallbacks>>>,
    
    // State tracking
    is_mouse_over: RwLock<bool>,
    is_pressed: RwLock<bool>,
    is_focused: RwLock<bool>,
    is_toggled: RwLock<bool>,
    tooltip_text: RwLock<String>,
    tooltip_delay: RwLock<u32>,

    // Optional gadget widget for script-created windows
    widget: Mutex<Option<WindowWidget>>,
    combobox_links: RwLock<Option<ComboBoxLinks>>,
    listbox_links: RwLock<Option<ListBoxLinks>>,
    slider_thumb: RwLock<Option<WindowId>>,

    // Press animation state for elastic button feel
    press_scale: RwLock<f32>,
    press_scale_target: RwLock<f32>,
    press_scale_velocity: RwLock<f32>,
    press_spring_strength: f32,
    press_spring_damping: f32,
    press_impulse: f32,
    release_impulse: f32,
    press_was_down: RwLock<bool>,

    // Render-time bounds override (used to preserve press-scale for custom draws)
    render_bounds_override: RwLock<Option<UIRect>>,
    
    // User data
    user_data: RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ComboBoxLinks {
    pub drop_down: WindowId,
    pub edit_box: WindowId,
    pub list_box: WindowId,
}

#[derive(Debug, Clone, Copy)]
pub struct ListBoxLinks {
    pub up_button: WindowId,
    pub down_button: WindowId,
    pub slider: WindowId,
    pub thumb: Option<WindowId>,
}

impl EnhancedGameWindow {
    /// Create a new enhanced game window
    pub fn new(id: WindowId, name: &str) -> Arc<Self> {
        Arc::new(Self {
            id,
            name: name.to_string(),
            status: RwLock::new(WindowStatus::NONE),
            window_type: RwLock::new(String::new()),
            style: RwLock::new(0),
            position: RwLock::new((0, 0)),
            size: RwLock::new((100, 100)),
            parent: RwLock::new(None),
            children: RwLock::new(Vec::new()),
            text: RwLock::new(String::new()),
            text_colors: RwLock::new(WindowTextColors::default()),
            draw_data: RwLock::new(WindowDrawData {
                enabled: None,
                disabled: None,
                hilited: None,
                pushed: None,
                enabled_color: [0.0, 0.0, 0.0, 0.0],
                enabled_border: [0.0, 0.0, 0.0, 0.0],
                disabled_color: [0.0, 0.0, 0.0, 0.0],
                disabled_border: [0.0, 0.0, 0.0, 0.0],
                hilited_color: [0.0, 0.0, 0.0, 0.0],
                hilited_border: [0.0, 0.0, 0.0, 0.0],
                pushed_color: [0.0, 0.0, 0.0, 0.0],
                pushed_border: [0.0, 0.0, 0.0, 0.0],
            }),
            font_name: RwLock::new("Arial".to_string()),
            font_size: RwLock::new(12),
            callbacks: RwLock::new(None),
            is_mouse_over: RwLock::new(false),
            is_pressed: RwLock::new(false),
            is_focused: RwLock::new(false),
            is_toggled: RwLock::new(false),
            tooltip_text: RwLock::new(String::new()),
            tooltip_delay: RwLock::new(1000),
            widget: Mutex::new(None),
            combobox_links: RwLock::new(None),
            listbox_links: RwLock::new(None),
            slider_thumb: RwLock::new(None),
            user_data: RwLock::new(HashMap::new()),
            press_scale: RwLock::new(1.0),
            press_scale_target: RwLock::new(1.0),
            press_scale_velocity: RwLock::new(0.0),
            press_spring_strength: 60.0,
            press_spring_damping: 10.0,
            press_impulse: -4.5,
            release_impulse: 5.5,
            press_was_down: RwLock::new(false),
            render_bounds_override: RwLock::new(None),
        })
    }
    
    // Property getters
    pub fn get_id(&self) -> WindowId {
        self.id
    }
    
    pub fn get_name(&self) -> &str {
        &self.name
    }
    
    pub fn get_status(&self) -> WindowStatus {
        *self.status.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_window_type(&self, window_type: &str) {
        *self.window_type.write().unwrap_or_else(|e| e.into_inner()) = window_type.to_string();
    }

    pub fn get_window_type(&self) -> String {
        self.window_type.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_style(&self, style: u32) {
        *self.style.write().unwrap_or_else(|e| e.into_inner()) = style;
    }

    pub fn get_style(&self) -> u32 {
        *self.style.read().unwrap_or_else(|e| e.into_inner())
    }
    
    pub fn get_position(&self) -> (i32, i32) {
        *self.position.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get_screen_position(&self) -> (i32, i32) {
        let mut x = 0;
        let mut y = 0;
        let mut current: Option<Arc<EnhancedGameWindow>> = Some(self.clone());
        while let Some(window) = current {
            let (wx, wy) = window.get_position();
            x += wx;
            y += wy;
            current = window.get_parent();
        }
        (x, y)
    }
    
    pub fn get_size(&self) -> (i32, i32) {
        *self.size.read().unwrap_or_else(|e| e.into_inner())
    }
    
    pub fn get_bounds(&self) -> UIRect {
        if let Some(bounds) = self.render_bounds_override.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            return *bounds;
        }
        let pos = self.get_position();
        let size = self.get_size();
        UIRect::new(pos.0 as f32, pos.1 as f32, size.0 as f32, size.1 as f32)
    }

    pub fn get_enabled_image_name(&self) -> Option<String> {
        let draw_data = self.draw_data.read().unwrap_or_else(|e| e.into_inner());
        draw_data.enabled.clone()
    }
    
    pub fn get_text(&self) -> String {
        self.text.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
    
    pub fn get_font_name(&self) -> String {
        self.font_name.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
    
    pub fn get_font_size(&self) -> i32 {
        *self.font_size.read().unwrap_or_else(|e| e.into_inner())
    }
    
    // Property setters
    pub fn set_status(&self, status: WindowStatus) {
        *self.status.write().unwrap_or_else(|e| e.into_inner()) = status;
    }

    pub fn set_widget(&self, widget: WindowWidget) {
        *self.widget.lock().unwrap_or_else(|e| e.into_inner()) = Some(widget);
        self.sync_widget_bounds();
        if let Some(widget) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            set_widget_visible(widget, !self.is_hidden());
            set_widget_enabled(widget, self.is_enabled());
        }
    }

    pub fn with_widget_mut<T>(&self, f: impl FnOnce(&mut WindowWidget) -> T) -> Option<T> {
        let mut guard = self.widget.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_mut().map(f)
    }

    pub fn set_combobox_links(&self, links: ComboBoxLinks) {
        *self.combobox_links.write().unwrap_or_else(|e| e.into_inner()) = Some(links);
    }

    pub fn combobox_links(&self) -> Option<ComboBoxLinks> {
        *self.combobox_links.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_listbox_links(&self, links: ListBoxLinks) {
        *self.listbox_links.write().unwrap_or_else(|e| e.into_inner()) = Some(links);
    }

    pub fn listbox_links(&self) -> Option<ListBoxLinks> {
        *self.listbox_links.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_slider_thumb(&self, thumb_id: WindowId) {
        *self.slider_thumb.write().unwrap_or_else(|e| e.into_inner()) = Some(thumb_id);
    }

    pub fn slider_thumb(&self) -> Option<WindowId> {
        *self.slider_thumb.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_position(&self, x: i32, y: i32) {
        *self.position.write().unwrap_or_else(|e| e.into_inner()) = (x, y);
        self.sync_widget_bounds();
    }
    
    pub fn set_size(&self, width: i32, height: i32) {
        *self.size.write().unwrap_or_else(|e| e.into_inner()) = (width, height);
        self.sync_widget_bounds();
    }
    
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        self.set_position(x, y);
        self.set_size(width, height);
    }

    fn sync_widget_bounds(&self) {
        let (x, y) = self.get_position();
        let (width, height) = self.get_size();
        if let Some(widget) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            set_widget_bounds(widget, x, y, width, height);
        }
    }
    
    pub fn set_text(&self, text: &str) {
        *self.text.write().unwrap_or_else(|e| e.into_inner()) = text.to_string();
    }

    pub fn set_progress_value(&self, value: f32) {
        if let Some(widget) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            if let WindowWidget::ProgressBar(bar) = widget {
                bar.set_value(value);
            }
        }
    }

    pub fn set_progress_percent(&self, percent: f32) {
        if let Some(widget) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            if let WindowWidget::ProgressBar(bar) = widget {
                bar.set_percentage(percent);
            }
        }
    }
    
    pub fn set_font(&self, name: &str, size: i32) {
        *self.font_name.write().unwrap_or_else(|e| e.into_inner()) = name.to_string();
        *self.font_size.write().unwrap_or_else(|e| e.into_inner()) = size;
    }

    pub fn set_draw_images(
        &self,
        enabled: Option<&str>,
        disabled: Option<&str>,
        hilited: Option<&str>,
        pushed: Option<&str>,
    ) {
        let mut draw_data = self.draw_data.write().unwrap_or_else(|e| e.into_inner());
        draw_data.enabled = enabled.map(|s| s.to_string());
        draw_data.disabled = disabled.map(|s| s.to_string());
        draw_data.hilited = hilited.map(|s| s.to_string());
        draw_data.pushed = pushed.map(|s| s.to_string());
    }

    pub fn set_draw_data(
        &self,
        enabled: Option<String>,
        disabled: Option<String>,
        hilited: Option<String>,
        pushed: Option<String>,
        enabled_color: [f32; 4],
        enabled_border: [f32; 4],
        disabled_color: [f32; 4],
        disabled_border: [f32; 4],
        hilited_color: [f32; 4],
        hilited_border: [f32; 4],
        pushed_color: [f32; 4],
        pushed_border: [f32; 4],
    ) {
        let mut draw_data = self.draw_data.write().unwrap_or_else(|e| e.into_inner());
        draw_data.enabled = enabled;
        draw_data.disabled = disabled;
        draw_data.hilited = hilited;
        draw_data.pushed = pushed;
        draw_data.enabled_color = enabled_color;
        draw_data.enabled_border = enabled_border;
        draw_data.disabled_color = disabled_color;
        draw_data.disabled_border = disabled_border;
        draw_data.hilited_color = hilited_color;
        draw_data.hilited_border = hilited_border;
        draw_data.pushed_color = pushed_color;
        draw_data.pushed_border = pushed_border;
    }

    pub fn set_text_colors(
        &self,
        enabled: [f32; 4],
        disabled: [f32; 4],
        hilited: [f32; 4],
        pushed: [f32; 4],
        enabled_border: [f32; 4],
        disabled_border: [f32; 4],
        hilited_border: [f32; 4],
        pushed_border: [f32; 4],
    ) {
        let mut colors = self.text_colors.write().unwrap_or_else(|e| e.into_inner());
        colors.enabled = enabled;
        colors.disabled = disabled;
        colors.hilited = hilited;
        colors.pushed = pushed;
        colors.enabled_border = enabled_border;
        colors.disabled_border = disabled_border;
        colors.hilited_border = hilited_border;
        colors.pushed_border = pushed_border;
    }
    
    pub fn set_callbacks(&self, callbacks: Box<dyn WindowCallbacks>) {
        *self.callbacks.write().unwrap_or_else(|e| e.into_inner()) = Some(callbacks);
    }
    
    pub fn set_tooltip(&self, text: &str, delay: u32) {
        *self.tooltip_text.write().unwrap_or_else(|e| e.into_inner()) = text.to_string();
        *self.tooltip_delay.write().unwrap_or_else(|e| e.into_inner()) = delay;
    }

    pub fn get_tooltip(&self) -> String {
        self.tooltip_text.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn get_tooltip_delay(&self) -> u32 {
        *self.tooltip_delay.read().unwrap_or_else(|e| e.into_inner())
    }
    
    // Status checks
    pub fn is_enabled(&self) -> bool {
        self.get_status().contains(WindowStatus::ENABLED)
    }
    
    pub fn is_hidden(&self) -> bool {
        self.get_status().contains(WindowStatus::HIDDEN)
    }
    
    pub fn is_visible(&self) -> bool {
        !self.is_hidden()
    }
    
    pub fn is_active(&self) -> bool {
        self.get_status().contains(WindowStatus::ACTIVE)
    }
    
    pub fn is_mouse_over(&self) -> bool {
        *self.is_mouse_over.read().unwrap_or_else(|e| e.into_inner())
    }
    
    pub fn is_pressed(&self) -> bool {
        *self.is_pressed.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_toggled(&self) -> bool {
        *self.is_toggled.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_input_enabled(&self) -> bool {
        let status = self.get_status();
        self.is_enabled() && !status.contains(WindowStatus::NO_INPUT) && !status.contains(WindowStatus::HIDDEN)
    }

    fn is_press_anim_enabled(&self) -> bool {
        let status = self.get_status();
        self.is_enabled() && !status.contains(WindowStatus::NO_INPUT)
    }

    fn widget_pressed_state(&self) -> Option<bool> {
        let widget_guard = self.widget.lock().unwrap_or_else(|e| e.into_inner());
        let widget = widget_guard.as_ref()?;
        Some(matches!(widget_state(widget), GadgetState::Pressed))
    }

    pub fn get_press_scale(&self) -> f32 {
        if self.is_press_anim_enabled() {
            *self.press_scale.read().unwrap_or_else(|e| e.into_inner())
        } else {
            1.0
        }
    }

    fn update_press_state(&self, pressed: bool) {
        *self.is_pressed.write().unwrap_or_else(|e| e.into_inner()) = pressed;

        if !self.is_press_anim_enabled() {
            *self.press_scale.write().unwrap_or_else(|e| e.into_inner()) = 1.0;
            *self.press_scale_target.write().unwrap_or_else(|e| e.into_inner()) = 1.0;
            *self.press_scale_velocity.write().unwrap_or_else(|e| e.into_inner()) = 0.0;
            *self.press_was_down.write().unwrap_or_else(|e| e.into_inner()) = pressed;
            return;
        }

        let mut was_down = self.press_was_down.write().unwrap_or_else(|e| e.into_inner());
        if pressed != *was_down {
            *self.press_scale_target.write().unwrap_or_else(|e| e.into_inner()) = if pressed { 0.94 } else { 1.0 };
            *self.press_scale_velocity.write().unwrap_or_else(|e| e.into_inner()) = if pressed {
                self.press_impulse
            } else {
                self.release_impulse
            };
            *was_down = pressed;
        }
    }

    pub fn update_press_animation(&self, delta_time: f32) {
        if !self.is_press_anim_enabled() {
            *self.press_scale.write().unwrap_or_else(|e| e.into_inner()) = 1.0;
            *self.press_scale_target.write().unwrap_or_else(|e| e.into_inner()) = 1.0;
            *self.press_scale_velocity.write().unwrap_or_else(|e| e.into_inner()) = 0.0;
            *self.press_was_down.write().unwrap_or_else(|e| e.into_inner()) = false;
            return;
        }

        if let Some(pressed) = self.widget_pressed_state() {
            self.update_press_state(pressed);
        }

        let dt = delta_time.max(0.0);
        if dt == 0.0 {
            return;
        }

        let target = *self.press_scale_target.read().unwrap_or_else(|e| e.into_inner());
        let mut scale = self.press_scale.write().unwrap_or_else(|e| e.into_inner());
        let mut velocity = self.press_scale_velocity.write().unwrap_or_else(|e| e.into_inner());

        let displacement = *scale - target;
        let accel = -self.press_spring_strength * displacement
            - self.press_spring_damping * *velocity;
        *velocity += accel * dt;
        *scale += *velocity * dt;

        if (*scale - target).abs() < 0.0005 && velocity.abs() < 0.0005 {
            *scale = target;
            *velocity = 0.0;
        }
    }
    
    pub fn is_focused(&self) -> bool {
        *self.is_focused.read().unwrap_or_else(|e| e.into_inner())
    }
    
    // Status modification
    pub fn enable(&self, enabled: bool) {
        let mut status = self.status.write().unwrap_or_else(|e| e.into_inner());
        if enabled {
            status.insert(WindowStatus::ENABLED);
        } else {
            status.remove(WindowStatus::ENABLED);
        }
        if let Some(widget) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            set_widget_enabled(widget, enabled);
        }
    }
    
    pub fn hide(&self, hidden: bool) {
        let mut status = self.status.write().unwrap_or_else(|e| e.into_inner());
        if hidden {
            status.insert(WindowStatus::HIDDEN);
        } else {
            status.remove(WindowStatus::HIDDEN);
        }
        if let Some(widget) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            set_widget_visible(widget, !hidden);
        }
    }
    
    pub fn activate(&self, active: bool) {
        let mut status = self.status.write().unwrap_or_else(|e| e.into_inner());
        if active {
            status.insert(WindowStatus::ACTIVE);
        } else {
            status.remove(WindowStatus::ACTIVE);
        }
    }
    
    // Hierarchy management
    pub fn add_child(self: &Arc<Self>, child: Arc<EnhancedGameWindow>) -> Result<()> {
        // Set parent reference in child
        {
            let mut child_parent = child.parent.write().unwrap_or_else(|e| e.into_inner());
            *child_parent = Some(Arc::downgrade(self));
        }
        
        // Add to children list
        self.children.write().unwrap_or_else(|e| e.into_inner()).push(child);
        
        Ok(())
    }
    
    pub fn remove_child(&self, child: &Arc<EnhancedGameWindow>) -> Result<()> {
        // Clear parent reference in child
        {
            let mut child_parent = child.parent.write().unwrap_or_else(|e| e.into_inner());
            *child_parent = None;
        }
        
        // Remove from children list
        let mut children = self.children.write().unwrap_or_else(|e| e.into_inner());
        children.retain(|c| c.get_id() != child.get_id());
        
        Ok(())
    }
    
    pub fn get_parent(&self) -> Option<Arc<EnhancedGameWindow>> {
        self.parent.read().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|weak| weak.upgrade())
    }
    
    pub fn get_children(&self) -> Vec<Arc<EnhancedGameWindow>> {
        self.children.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
    
    pub fn get_child_count(&self) -> usize {
        self.children.read().unwrap_or_else(|e| e.into_inner()).len()
    }
    
    pub fn find_child_by_name(&self, name: &str) -> Option<Arc<EnhancedGameWindow>> {
        let children = self.children.read().unwrap_or_else(|e| e.into_inner());
        for child in children.iter() {
            if child.get_name() == name {
                return Some(child.clone());
            }
            // Recursively search in children
            if let Some(found) = child.find_child_by_name(name) {
                return Some(found);
            }
        }
        None
    }
    
    pub fn find_child_by_id(&self, id: WindowId) -> Option<Arc<EnhancedGameWindow>> {
        let children = self.children.read().unwrap_or_else(|e| e.into_inner());
        for child in children.iter() {
            if child.get_id() == id {
                return Some(child.clone());
            }
            // Recursively search in children
            if let Some(found) = child.find_child_by_id(id) {
                return Some(found);
            }
        }
        None
    }
    
    // Event handling
    pub fn send_message(&self, message: WindowMessage, wparam: WindowMsgData, lparam: WindowMsgData) -> WindowMsgHandled {
        // Ensure press animation stays in sync even when input is routed directly to send_message.
        if self.is_input_enabled() {
            match message {
                WindowMessage::LeftDown => self.update_press_state(true),
                WindowMessage::LeftUp => self.update_press_state(false),
                _ => {}
            }
        }
        if let Some(callbacks) = self.callbacks.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            // Try input handler first
            let result = callbacks.on_input(self, message, wparam, lparam);
            if result.is_handled() {
                return result;
            }

            let widget_result = self.handle_widget_input(message, wparam, lparam);
            if widget_result.is_handled() {
                return widget_result;
            }
            
            // Then try system handler
            let system_result = callbacks.on_system(self, message, wparam, lparam);
            if system_result.is_handled() {
                return system_result;
            }
            self.handle_widget_system(message, wparam, lparam)
        } else {
            let result = self.handle_widget_input(message, wparam, lparam);
            if result.is_handled() {
                return result;
            }
            self.handle_widget_system(message, wparam, lparam)
        }
    }
    
    pub fn handle_mouse_event(&self, message: WindowMessage, x: i32, y: i32) -> WindowMsgHandled {
        if !self.is_input_enabled() {
            return WindowMsgHandled::Ignored;
        }
        if matches!(message, WindowMessage::RightDown | WindowMessage::RightUp) {
            if !self.get_status().contains(WindowStatus::RIGHT_CLICK) {
                return WindowMsgHandled::Ignored;
            }
        }
        let bounds = self.get_bounds();
        let is_in_bounds = bounds.contains(x as f32, y as f32);
        let status = self.get_status();
        let toggle_like = status.contains(WindowStatus::TOGGLE) || status.contains(WindowStatus::CHECK_LIKE);
        let trigger_on_mouse_down = status.contains(WindowStatus::ON_MOUSE_DOWN);
        let style = self.get_style();
        let is_button_style = (style & (GWS_PUSH_BUTTON | GWS_CHECK_BOX | GWS_RADIO_BUTTON)) != 0;
        let is_gadget_style = (style
            & (GWS_PUSH_BUTTON
                | GWS_CHECK_BOX
                | GWS_RADIO_BUTTON
                | GWS_SCROLL_LISTBOX
                | GWS_COMBO_BOX
                | GWS_ENTRY_FIELD
                | GWS_HORZ_SLIDER
                | GWS_VERT_SLIDER
                | GWS_PROGRESS_BAR
                | GWS_STATIC_TEXT
                | GWS_TAB_CONTROL
                | GWS_TAB_PANE))
            != 0;
        
        match message {
            WindowMessage::MouseEntering if is_in_bounds => {
                *self.is_mouse_over.write().unwrap_or_else(|e| e.into_inner()) = true;
                let handled = self.send_message(message, 0, pack_coords(x, y));
                if is_gadget_style {
                    let _ = self.send_message(WindowMessage::GadgetMouseEntering, 0, 0);
                }
                handled
            }
            WindowMessage::MouseLeaving => {
                *self.is_mouse_over.write().unwrap_or_else(|e| e.into_inner()) = false;
                let handled = self.send_message(message, 0, pack_coords(x, y));
                if is_gadget_style {
                    let _ = self.send_message(WindowMessage::GadgetMouseLeaving, 0, 0);
                }
                handled
            }
            WindowMessage::LeftDown if is_in_bounds => {
                self.update_press_state(true);
                if toggle_like && trigger_on_mouse_down {
                    let mut toggled = self.is_toggled.write().unwrap_or_else(|e| e.into_inner());
                    *toggled = !*toggled;
                }
                let handled = self.send_message(message, 0, pack_coords(x, y));
                if is_button_style && trigger_on_mouse_down {
                    let _ = self.send_message(WindowMessage::GadgetSelected, 0, 0);
                }
                handled
            }
            WindowMessage::LeftUp => {
                let was_pressed = self.is_pressed();
                self.update_press_state(false);
                if was_pressed && is_in_bounds {
                    if toggle_like && !trigger_on_mouse_down {
                        let mut toggled = self.is_toggled.write().unwrap_or_else(|e| e.into_inner());
                        *toggled = !*toggled;
                    }
                    let handled = self.send_message(message, 0, pack_coords(x, y));
                    if is_button_style && !trigger_on_mouse_down {
                        let _ = self.send_message(WindowMessage::GadgetSelected, 0, 0);
                    }
                    handled
                } else {
                    WindowMsgHandled::Ignored
                }
            }
            WindowMessage::RightUp if is_in_bounds => {
                let handled = self.send_message(message, 0, pack_coords(x, y));
                if is_button_style {
                    let _ = self.send_message(WindowMessage::GadgetRightClick, 0, 0);
                }
                handled
            }
            _ => {
                if is_in_bounds {
                    self.send_message(message, 0, pack_coords(x, y))
                } else {
                    WindowMsgHandled::Ignored
                }
            }
        }
    }

    fn handle_widget_input(&self, msg: WindowMessage, data1: WindowMsgData, data2: WindowMsgData) -> WindowMsgHandled {
        let mut widget_guard = self.widget.lock().unwrap_or_else(|e| e.into_inner());
        let Some(widget) = widget_guard.as_mut() else {
            return WindowMsgHandled::Ignored;
        };

        if matches!(widget, WindowWidget::ListBox(_))
            && (msg == WindowMessage::WheelUp || msg == WindowMessage::WheelDown)
        {
            let delta = if msg == WindowMessage::WheelUp { -1 } else { 1 };
            if let WindowWidget::ListBox(listbox) = widget {
                listbox.scroll_by(delta);
            }
            return WindowMsgHandled::Handled;
        }

        let (x, y) = unpack_coords(data2);
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
            WindowMessage::Char => Some(InputEvent::KeyDown {
                key: map_keycode(data1),
                modifiers: KeyModifiers::none(),
            }),
            _ => None,
        };

        let Some(event) = event else {
            return WindowMsgHandled::Ignored;
        };

        let messages = handle_widget_event(widget, &event);
        if messages.is_empty() {
            return WindowMsgHandled::Ignored;
        }

        if matches!(widget, WindowWidget::HorizontalSlider(_) | WindowWidget::VerticalSlider(_)) {
            self.update_slider_thumb();
        }

        if matches!(widget, WindowWidget::ListBox(_)) {
            self.update_listbox_scrollbar();
        }

        if matches!(widget, WindowWidget::TabControl(_)) {
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
        let target_parent = self.get_parent();
        for message in messages {
            if let GadgetMessage::ValueChanged { value: GadgetValue::Boolean(state), .. } = message {
                *self.is_toggled.write().unwrap_or_else(|e| e.into_inner()) = state;
            }

            let (msg, data1) = match message {
                GadgetMessage::Clicked { .. } => (WindowMessage::GadgetSelected, self.id as u32),
                GadgetMessage::RightClicked { .. } => {
                    if !self.get_status().contains(WindowStatus::RIGHT_CLICK) {
                        continue;
                    }
                    (WindowMessage::GadgetRightClick, self.id as u32)
                }
                GadgetMessage::LeftDrag { .. } => (WindowMessage::User(GGM_LEFT_DRAG), data1),
                GadgetMessage::ValueChanged { .. } => (WindowMessage::GadgetValueChanged, self.id as u32),
                GadgetMessage::EditingComplete { .. } => (WindowMessage::GadgetEditDone, self.id as u32),
                GadgetMessage::MouseEnter { .. } => (WindowMessage::GadgetMouseEntering, self.id as u32),
                GadgetMessage::MouseLeave { .. } => (WindowMessage::GadgetMouseLeaving, self.id as u32),
                GadgetMessage::FocusChanged { has_focus, .. } => {
                    (WindowMessage::InputFocus, if has_focus { 1 } else { 0 })
                }
                GadgetMessage::Custom { .. } => (WindowMessage::User(0x8000), self.id as u32),
            };

            let result = if let Some(ref parent) = target_parent {
                parent.send_message(msg, data1, 0)
            } else {
                self.send_message(msg, data1, 0)
            };
            if result.is_handled() {
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
        &self,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        let mut widget_guard = self.widget.lock().unwrap_or_else(|e| e.into_inner());
        let Some(widget) = widget_guard.as_mut() else {
            return WindowMsgHandled::Ignored;
        };

        if matches!(widget, WindowWidget::ComboBox(_)) {
            if let Some(links) = self.combobox_links() {
                if msg == WindowMessage::GadgetSelected && data1 == links.drop_down as u32 {
                    if let Some(list_box) = self.find_child_by_id(links.list_box) {
                        let is_hidden = list_box.is_hidden();
                        if is_hidden {
                            self.sync_combobox_listbox(&list_box);
                            self.resize_combobox_listbox(&list_box);
                            let list_height = list_box.get_size().1;
                            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                let base_height = edit_box.get_size().1;
                                let (width, _) = self.get_size();
                                self.set_size(width, base_height + list_height);
                            }
                            list_box.hide(false);
                        } else {
                            list_box.hide(true);
                            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                let base_height = edit_box.get_size().1;
                                let (width, _) = self.get_size();
                                self.set_size(width, base_height);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                }

                if msg == WindowMessage::GadgetValueChanged && data1 == links.list_box as u32 {
                    if let Some(list_box) = self.find_child_by_id(links.list_box) {
                        if let Some(selected) = list_box.with_widget_mut(|widget| {
                            if let WindowWidget::ListBox(listbox) = widget {
                                listbox.selected_indices().first().copied()
                            } else {
                                None
                            }
                        }).flatten() {
                            if let WindowWidget::ComboBox(combo) = widget {
                                let _ = combo.select_index(selected);
                            }
                        }
                        if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                            self.sync_combobox_edit_box(&edit_box);
                        }
                        let dont_hide = if let WindowWidget::ComboBox(combo) = widget {
                            combo.take_dont_hide_next()
                        } else {
                            false
                        };
                        if !dont_hide {
                            list_box.hide(true);
                            if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                                let base_height = edit_box.get_size().1;
                                let (width, _) = self.get_size();
                                self.set_size(width, base_height);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                }

                if msg == WindowMessage::GadgetEditDone && data1 == links.edit_box as u32 {
                    if let Some(edit_box) = self.find_child_by_id(links.edit_box) {
                        if let Some(text) = edit_box.with_widget_mut(|widget| {
                            if let WindowWidget::TextEntry(entry) = widget {
                                Some(entry.displayed_text().to_string())
                            } else {
                                None
                            }
                        }).flatten() {
                            if let WindowWidget::ComboBox(combo) = widget {
                                combo.set_text(text);
                            }
                        }
                        return WindowMsgHandled::Handled;
                    }
                }
            }
        }

        if matches!(widget, WindowWidget::ListBox(_)) {
            if let Some(links) = self.listbox_links() {
                if msg == WindowMessage::GadgetSelected && data1 == links.up_button as u32 {
                    if let WindowWidget::ListBox(listbox) = widget {
                        listbox.scroll_by(-1);
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }

                if msg == WindowMessage::GadgetSelected && data1 == links.down_button as u32 {
                    if let WindowWidget::ListBox(listbox) = widget {
                        listbox.scroll_by(1);
                    }
                    self.update_listbox_scrollbar();
                    return WindowMsgHandled::Handled;
                }

                if msg == WindowMessage::GadgetValueChanged && data1 == links.slider as u32 {
                    let slider_value = if let Some(slider) = self.find_child_by_id(links.slider) {
                        slider.with_widget_mut(|widget| match widget {
                            WindowWidget::VerticalSlider(slider) => Some(slider.value()),
                            WindowWidget::HorizontalSlider(slider) => Some(slider.value()),
                            _ => None,
                        }).flatten().unwrap_or(0)
                    } else {
                        0
                    };
                    if let WindowWidget::ListBox(listbox) = widget {
                        listbox.set_scroll_offset(slider_value.max(0) as usize);
                    }
                    self.update_listbox_scrollbar();
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
            let messages = handle_widget_event(widget, &event);
            return if messages.is_empty() {
                WindowMsgHandled::Ignored
            } else {
                WindowMsgHandled::Handled
            };
        }

        WindowMsgHandled::Ignored
    }

    fn sync_combobox_listbox(&self, list_box: &Arc<EnhancedGameWindow>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_ref() else {
            return;
        };
        let Some(_) = list_box.with_widget_mut(|widget| {
            if let WindowWidget::ListBox(listbox) = widget {
                listbox.clear();
                for item in combo.items() {
                    listbox.add_item(&item.text);
                }
                if let Some(selected) = combo.selected_index() {
                    let _ = listbox.select_index(selected, KeyModifiers::none());
                }
                Some(())
            } else {
                None
            }
        }).flatten() else {
            return;
        };
        list_box.update_listbox_scrollbar();
    }

    fn sync_combobox_edit_box(&self, edit_box: &Arc<EnhancedGameWindow>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_ref() else {
            return;
        };
        let _ = edit_box.with_widget_mut(|widget| {
            if let WindowWidget::TextEntry(entry) = widget {
                entry.set_text(combo.text());
            }
        });
    }

    fn resize_combobox_listbox(&self, list_box: &Arc<EnhancedGameWindow>) {
        let Some(WindowWidget::ComboBox(combo)) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_ref() else {
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
            .with_widget_mut(|widget| {
                if let WindowWidget::ListBox(listbox) = widget {
                    Some(listbox.item_height() as i32)
                } else {
                    None
                }
            })
            .flatten()
            .unwrap_or(18);
        let height = (visible as i32 * item_height).max(item_height);
        let (width, _) = list_box.get_size();
        list_box.set_size(width as i32, height);
        if let Some(links) = list_box.listbox_links() {
            if let Some(up) = list_box.find_child_by_id(links.up_button) {
                up.hide(!show_scrollbar);
            }
            if let Some(down) = list_box.find_child_by_id(links.down_button) {
                down.hide(!show_scrollbar);
            }
            if let Some(slider) = list_box.find_child_by_id(links.slider) {
                slider.hide(!show_scrollbar);
            }
        }
        list_box.update_listbox_scrollbar();
    }

    pub fn update_listbox_scrollbar(&self) {
        let Some(links) = self.listbox_links() else {
            return;
        };
        let Some(WindowWidget::ListBox(listbox)) = self.widget.lock().unwrap_or_else(|e| e.into_inner()).as_ref() else {
            return;
        };

        let bounds = listbox.bounds();
        let item_height = listbox.item_height().max(1) as usize;
        let visible = (bounds.height as usize / item_height).max(1);
        let max_offset = listbox.items().len().saturating_sub(visible);
        let scroll_offset = listbox.scroll_offset().min(max_offset);
        if scroll_offset != listbox.scroll_offset() {
            let _ = self.with_widget_mut(|widget| {
                if let WindowWidget::ListBox(listbox) = widget {
                    listbox.set_scroll_offset(scroll_offset);
                }
            });
        }

        if let Some(slider) = self.find_child_by_id(links.slider) {
            let _ = slider.with_widget_mut(|widget| match widget {
                WindowWidget::VerticalSlider(slider) => {
                    slider.set_range(0, max_offset as i32);
                    slider.set_value(scroll_offset as i32);
                }
                WindowWidget::HorizontalSlider(slider) => {
                    slider.set_range(0, max_offset as i32);
                    slider.set_value(scroll_offset as i32);
                }
                _ => {}
            });
        }

        if let Some(up_button) = self.find_child_by_id(links.up_button) {
            let enabled = max_offset > 0 && scroll_offset > 0;
            up_button.enable(enabled);
        }
        if let Some(down_button) = self.find_child_by_id(links.down_button) {
            let enabled = max_offset > 0 && scroll_offset < max_offset;
            down_button.enable(enabled);
        }
        if let Some(slider) = self.find_child_by_id(links.slider) {
            let enabled = max_offset > 0;
            slider.enable(enabled);
        }

        let mut content_width = bounds.width;
        if let Some(slider) = self.find_child_by_id(links.slider) {
            if !slider.is_hidden() {
                let (slider_width, _) = slider.get_size();
                content_width = content_width.saturating_sub(slider_width as u32 + 2);
            }
        }
        let _ = self.with_widget_mut(|widget| {
            if let WindowWidget::ListBox(listbox) = widget {
                listbox.set_content_width(content_width);
            }
        });

        if let Some(thumb_id) = links.thumb {
            if let Some(thumb) = self.find_child_by_id(thumb_id) {
                if let Some(slider) = self.find_child_by_id(links.slider) {
                    let (_, slider_height) = slider.get_size();
                    let (_, thumb_height) = thumb.get_size();
                    let available = (slider_height - thumb_height).max(0);
                    let ratio = if max_offset > 0 {
                        scroll_offset as f32 / max_offset as f32
                    } else {
                        0.0
                    };
                    let thumb_y = (ratio * available as f32).round() as i32;
                    thumb.set_position(0, thumb_y);
                    thumb.hide(max_offset == 0);
                }
            }
        }
    }

    pub fn update_slider_thumb(&self) {
        let Some(thumb_id) = self.slider_thumb() else {
            return;
        };
        let Some(thumb) = self.find_child_by_id(thumb_id) else {
            return;
        };
        let (thumb_w, thumb_h) = thumb.get_size();
        let (width, height) = self.get_size();

        let _ = self.with_widget_mut(|widget| match widget {
            WindowWidget::HorizontalSlider(slider) => {
                let (min_val, max_val) = slider.range();
                let range = (max_val - min_val).max(1);
                let track = (width - thumb_w).max(0);
                let ratio = (slider.value() - min_val) as f32 / range as f32;
                let x = (ratio * track as f32).round() as i32;
                thumb.set_position(x, HORIZONTAL_SLIDER_THUMB_POSITION);
            }
            WindowWidget::VerticalSlider(slider) => {
                let (min_val, max_val) = slider.range();
                let range = (max_val - min_val).max(1);
                let track = (height - thumb_h).max(0);
                let ratio = (slider.value() - min_val) as f32 / range as f32;
                let y = (ratio * track as f32).round() as i32;
                thumb.set_position(0, y);
            }
            _ => {}
        });
    }

    fn show_tab_pane(&self, index: usize) {
        let panes: Vec<Arc<EnhancedGameWindow>> = self
            .children
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|child| (child.get_style() & GWS_TAB_PANE) != 0)
            .cloned()
            .collect();

        if panes.is_empty() {
            return;
        }

        for pane in panes.iter() {
            pane.hide(true);
        }

        if let Some(pane) = panes.get(index) {
            pane.hide(false);
        }
    }
    
    // Rendering
    pub fn render(&self, renderer: &mut UIRenderer, parent_offset: (f32, f32)) -> Result<()> {
        if self.is_hidden() {
            return Ok(());
        }
        
        let pos = self.get_position();
        let size = self.get_size();
        let world_pos = (parent_offset.0 + pos.0 as f32, parent_offset.1 + pos.1 as f32);
        
        let mut bounds = UIRect::new(world_pos.0, world_pos.1, size.0 as f32, size.1 as f32);
        let scale = self.get_press_scale();
        if (scale - 1.0).abs() > f32::EPSILON {
            let cx = bounds.x + bounds.width * 0.5;
            let cy = bounds.y + bounds.height * 0.5;
            let scaled_width = bounds.width * scale;
            let scaled_height = bounds.height * scale;
            bounds = UIRect::new(
                cx - scaled_width * 0.5,
                cy - scaled_height * 0.5,
                scaled_width,
                scaled_height,
            );
        }
        
        // Determine current state and color
        let status = self.get_status();
        let use_disabled_colors = !self.is_enabled() && !status.contains(WindowStatus::ALWAYS_COLOR);
        let toggled = self.is_toggled();
        let pressed_or_toggled = self.is_pressed() || toggled;
        let (state_color, border_color, z_order) = if use_disabled_colors {
            let colors = self.text_colors.read().unwrap_or_else(|e| e.into_inner());
            (colors.disabled, colors.disabled_border, 0.1)
        } else if pressed_or_toggled {
            let colors = self.text_colors.read().unwrap_or_else(|e| e.into_inner());
            (colors.pushed, colors.pushed_border, 0.3)
        } else if self.is_mouse_over() {
            let colors = self.text_colors.read().unwrap_or_else(|e| e.into_inner());
            (colors.hilited, colors.hilited_border, 0.2)
        } else {
            let colors = self.text_colors.read().unwrap_or_else(|e| e.into_inner());
            (colors.enabled, colors.enabled_border, 0.1)
        };
        
        let _override_guard = RenderBoundsOverride::new(self, bounds);

        if !self.get_status().contains(WindowStatus::SEE_THRU) {
            // Call custom draw callback if available
            if let Some(callbacks) = self.callbacks.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
                if callbacks.on_draw(self, renderer).is_ok() {
                    // Custom rendering handled by callback
                } else {
                    // Default rendering
                    self.render_default(renderer, bounds, state_color, border_color, z_order)?;
                }
            } else {
                // Default rendering
                self.render_default(renderer, bounds, state_color, border_color, z_order)?;
            }
        }
        
        // Render children
        for child in self.get_children() {
            child.render(renderer, world_pos)?;
        }
        
        Ok(())
    }
    
    fn render_default(
        &self,
        renderer: &mut UIRenderer,
        bounds: UIRect,
        color: [f32; 4],
        border_color: [f32; 4],
        z_order: f32,
    ) -> Result<()> {
        // Draw background if needed
        let draw_data = self.draw_data.read().unwrap_or_else(|e| e.into_inner()).clone();
        let use_disabled_images = !self.is_enabled() && !status.contains(WindowStatus::ALWAYS_COLOR);
        let (image_name, fill_color, border_color) = if use_disabled_images {
            (
                draw_data.disabled.or(draw_data.enabled),
                draw_data.disabled_color,
                draw_data.disabled_border,
            )
        } else if pressed_or_toggled {
            (
                draw_data.pushed.or(draw_data.enabled),
                draw_data.pushed_color,
                draw_data.pushed_border,
            )
        } else if self.is_mouse_over() {
            (
                draw_data.hilited.or(draw_data.enabled),
                draw_data.hilited_color,
                draw_data.hilited_border,
            )
        } else {
            (
                draw_data.enabled,
                draw_data.enabled_color,
                draw_data.enabled_border,
            )
        };

        let mut drew_background = false;
        if let Some(name) = image_name {
            let collection = get_mapped_image_collection();
            let mut collection = collection.write();
            if let Some(mapped) = collection.find_image_by_name_mut(&name) {
                if mapped.get_gpu_texture().is_none() {
                    let _ = mapped.create_gpu_texture(renderer.device(), renderer.queue());
                }
                if let Some(gpu) = mapped.get_gpu_texture() {
                    let uv = mapped.get_uv();
                    let tex_rect = UIRect::new(uv.min.x, uv.min.y, uv.width(), uv.height());
                    renderer.draw_textured_rect(
                        bounds,
                        std::sync::Arc::new(gpu.view().clone()),
                        [1.0, 1.0, 1.0, 1.0],
                        Some(tex_rect),
                        z_order,
                    );
                    drew_background = true;
                }
            }
        }

        if !drew_background && fill_color[3] > 0.0 {
            renderer.draw_rect(bounds, fill_color, z_order);
            drew_background = true;
        }

        if !drew_background && status.contains(WindowStatus::ENABLED) {
            let bg_color = [0.2, 0.2, 0.2, 0.8]; // Dark gray fallback background
            renderer.draw_rect(bounds, bg_color, z_order);
        }

        if border_color[3] > 0.0 {
            renderer.draw_rect_outline(bounds, 1.0, border_color, z_order + 0.01);
        } else if status.contains(WindowStatus::BORDER) {
            renderer.draw_rect_outline(bounds, 1.0, [0.5, 0.5, 0.5, 1.0], z_order + 0.01);
        }
        
        // Draw text
        let raw_text = self.get_text();
        if !raw_text.is_empty() {
            let font_size = self.get_font_size() as f32;
            
            let alignment = if status.contains(WindowStatus::WRAP_CENTERED) {
                TextAlignment::Center
            } else {
                TextAlignment::Left
            };

            let (text, hotkey_index) = if status.contains(WindowStatus::HOTKEY_TEXT) {
                parse_hotkey_text(raw_text)
            } else {
                (raw_text.to_string(), None)
            };
            
            if border_color[3] > 0.0 {
                let outline_bounds = UIRect::new(bounds.x + 1.0, bounds.y + 1.0, bounds.width, bounds.height);
                let outline_layout = TextLayout {
                    text: text.clone(),
                    font_size,
                    color: border_color,
                    bounds: outline_bounds,
                    alignment,
                    vertical_alignment: VerticalAlignment::Middle,
                    word_wrap: !status.contains(WindowStatus::ONE_LINE),
                    single_line: status.contains(WindowStatus::ONE_LINE),
                };
                renderer.draw_text(&outline_layout, z_order + 0.01)?;
            }

            let text_layout = TextLayout {
                text,
                font_size,
                color,
                bounds,
                alignment,
                vertical_alignment: VerticalAlignment::Middle,
                word_wrap: !status.contains(WindowStatus::ONE_LINE),
                single_line: status.contains(WindowStatus::ONE_LINE),
            };

            renderer.draw_text(&text_layout, z_order + 0.02)?;

            if let Some(hotkey_idx) = hotkey_index {
                let single_line = status.contains(WindowStatus::ONE_LINE);
                if single_line {
                    let char_width = font_size * 0.6;
                    let text_width = text_layout.text.len() as f32 * char_width;
                    let base_x = match alignment {
                        TextAlignment::Center => bounds.x + (bounds.width - text_width) * 0.5,
                        TextAlignment::Left => bounds.x,
                        TextAlignment::Right => bounds.x + bounds.width - text_width,
                    };
                    let base_y = bounds.y + (bounds.height - font_size * 1.2) * 0.5;
                    if let Some(ch) = text_layout.text.chars().nth(hotkey_idx) {
                        let hotkey_color = self.text_colors.read().unwrap_or_else(|e| e.into_inner()).hilited;
                        let pos = Vec2::new(base_x + (hotkey_idx as f32 * char_width), base_y);
                        renderer.draw_text_simple(&ch.to_string(), pos, font_size, hotkey_color)?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    // Hit testing
    pub fn hit_test(self: &Arc<Self>, x: f32, y: f32) -> Option<Arc<EnhancedGameWindow>> {
        if self.is_hidden() || self.get_status().contains(WindowStatus::NO_INPUT) {
            return None;
        }
        
        let bounds = self.get_bounds();
        if !bounds.contains(x, y) {
            return None;
        }
        
        // Check children first (they're on top)
        for child in self.get_children().iter().rev() { // Reverse order for proper z-ordering
            if let Some(hit_child) = child.hit_test(x - bounds.x, y - bounds.y) {
                return Some(hit_child);
            }
        }
        
        // Return self if no child was hit
        Some(self.clone())
    }
    
    // User data management
    pub fn set_user_data<T: std::any::Any + Send + Sync>(&self, key: &str, value: T) {
        self.user_data.write().unwrap_or_else(|e| e.into_inner()).insert(key.to_string(), Box::new(value));
    }
    
    pub fn get_user_data<T: std::any::Any + Send + Sync>(&self, key: &str) -> Option<&T> {
        let store = self.user_data.read().unwrap_or_else(|e| e.into_inner());
        store.get(key).and_then(|value| value.downcast_ref::<T>())
    }
}

struct RenderBoundsOverride<'a> {
    window: &'a EnhancedGameWindow,
}

impl<'a> RenderBoundsOverride<'a> {
    fn new(window: &'a EnhancedGameWindow, bounds: UIRect) -> Self {
        *window.render_bounds_override.write().unwrap_or_else(|e| e.into_inner()) = Some(bounds);
        Self { window }
    }
}

impl Drop for RenderBoundsOverride<'_> {
    fn drop(&mut self) {
        *self.window.render_bounds_override.write().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

fn parse_hotkey_text(text: &str) -> (String, Option<usize>) {
    let mut out = String::new();
    let mut hotkey_index = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            if let Some('&') = chars.peek().copied() {
                out.push('&');
                chars.next();
            } else if hotkey_index.is_none() {
                hotkey_index = Some(out.chars().count());
            }
        } else {
            out.push(ch);
        }
    }
    (out, hotkey_index)
}

fn pack_coords(x: i32, y: i32) -> u32 {
    let ux = (x as u32) & 0xFFFF;
    let uy = (y as u32) & 0xFFFF;
    ux | (uy << 16)
}

fn unpack_coords(lparam: u32) -> (i32, i32) {
    let x = (lparam & 0xFFFF) as i16 as i32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn map_keycode(data: WindowMsgData) -> KeyCode {
    let key = (data & 0xFFFF) as u16;
    match key {
        8 => KeyCode::Backspace,
        9 => KeyCode::Tab,
        13 => KeyCode::Enter,
        27 => KeyCode::Escape,
        32 => KeyCode::Space,
        127 => KeyCode::Delete,
        0x1000 => KeyCode::Left,
        0x1001 => KeyCode::Right,
        0x1002 => KeyCode::Up,
        0x1003 => KeyCode::Down,
        0x1004 => KeyCode::Home,
        0x1005 => KeyCode::End,
        0x1006 => KeyCode::PageUp,
        0x1007 => KeyCode::PageDown,
        b'0' as u16 => KeyCode::Num0,
        b'1' as u16 => KeyCode::Num1,
        b'2' as u16 => KeyCode::Num2,
        b'3' as u16 => KeyCode::Num3,
        b'4' as u16 => KeyCode::Num4,
        b'5' as u16 => KeyCode::Num5,
        b'6' as u16 => KeyCode::Num6,
        b'7' as u16 => KeyCode::Num7,
        b'8' as u16 => KeyCode::Num8,
        b'9' as u16 => KeyCode::Num9,
        b'a' as u16 | b'A' as u16 => KeyCode::A,
        b'b' as u16 | b'B' as u16 => KeyCode::B,
        b'c' as u16 | b'C' as u16 => KeyCode::C,
        b'd' as u16 | b'D' as u16 => KeyCode::D,
        b'e' as u16 | b'E' as u16 => KeyCode::E,
        b'f' as u16 | b'F' as u16 => KeyCode::F,
        b'g' as u16 | b'G' as u16 => KeyCode::G,
        b'h' as u16 | b'H' as u16 => KeyCode::H,
        b'i' as u16 | b'I' as u16 => KeyCode::I,
        b'j' as u16 | b'J' as u16 => KeyCode::J,
        b'k' as u16 | b'K' as u16 => KeyCode::K,
        b'l' as u16 | b'L' as u16 => KeyCode::L,
        b'm' as u16 | b'M' as u16 => KeyCode::M,
        b'n' as u16 | b'N' as u16 => KeyCode::N,
        b'o' as u16 | b'O' as u16 => KeyCode::O,
        b'p' as u16 | b'P' as u16 => KeyCode::P,
        b'q' as u16 | b'Q' as u16 => KeyCode::Q,
        b'r' as u16 | b'R' as u16 => KeyCode::R,
        b's' as u16 | b'S' as u16 => KeyCode::S,
        b't' as u16 | b'T' as u16 => KeyCode::T,
        b'u' as u16 | b'U' as u16 => KeyCode::U,
        b'v' as u16 | b'V' as u16 => KeyCode::V,
        b'w' as u16 | b'W' as u16 => KeyCode::W,
        b'x' as u16 | b'X' as u16 => KeyCode::X,
        b'y' as u16 | b'Y' as u16 => KeyCode::Y,
        b'z' as u16 | b'Z' as u16 => KeyCode::Z,
        _ => KeyCode::Char((key as u8) as char),
    }
}

fn handle_widget_event(widget: &mut WindowWidget, event: &InputEvent) -> Vec<GadgetMessage> {
    match widget {
        WindowWidget::PushButton(gadget) => gadget.handle_input(event),
        WindowWidget::RadioButton(gadget) => gadget.handle_input(event),
        WindowWidget::CheckBox(gadget) => gadget.handle_input(event),
        WindowWidget::VerticalSlider(gadget) => gadget.handle_input(event),
        WindowWidget::HorizontalSlider(gadget) => gadget.handle_input(event),
        WindowWidget::ListBox(gadget) => gadget.handle_input(event),
        WindowWidget::TextEntry(gadget) => gadget.handle_input(event),
        WindowWidget::StaticText(gadget) => gadget.handle_input(event),
        WindowWidget::ProgressBar(gadget) => gadget.handle_input(event),
        WindowWidget::TabControl(gadget) => gadget.handle_input(event),
        WindowWidget::ComboBox(gadget) => gadget.handle_input(event),
        WindowWidget::TabPane
        | WindowWidget::User
        | WindowWidget::Animated
        | WindowWidget::MouseTrack => Vec::new(),
    }
}

fn widget_state(widget: &WindowWidget) -> GadgetState {
    match widget {
        WindowWidget::PushButton(gadget) => gadget.state(),
        WindowWidget::RadioButton(gadget) => gadget.state(),
        WindowWidget::CheckBox(gadget) => gadget.state(),
        WindowWidget::VerticalSlider(gadget) => gadget.state(),
        WindowWidget::HorizontalSlider(gadget) => gadget.state(),
        WindowWidget::ListBox(gadget) => gadget.state(),
        WindowWidget::TextEntry(gadget) => gadget.state(),
        WindowWidget::StaticText(gadget) => gadget.state(),
        WindowWidget::ProgressBar(gadget) => gadget.state(),
        WindowWidget::TabControl(gadget) => gadget.state(),
        WindowWidget::ComboBox(gadget) => gadget.state(),
        WindowWidget::TabPane
        | WindowWidget::User
        | WindowWidget::Animated
        | WindowWidget::MouseTrack => GadgetState::Normal,
    }
}

fn set_widget_visible(widget: &mut WindowWidget, visible: bool) {
    match widget {
        WindowWidget::PushButton(gadget) => gadget.set_visible(visible),
        WindowWidget::RadioButton(gadget) => gadget.set_visible(visible),
        WindowWidget::CheckBox(gadget) => gadget.set_visible(visible),
        WindowWidget::VerticalSlider(gadget) => gadget.set_visible(visible),
        WindowWidget::HorizontalSlider(gadget) => gadget.set_visible(visible),
        WindowWidget::ListBox(gadget) => gadget.set_visible(visible),
        WindowWidget::TextEntry(gadget) => gadget.set_visible(visible),
        WindowWidget::StaticText(gadget) => gadget.set_visible(visible),
        WindowWidget::ProgressBar(gadget) => gadget.set_visible(visible),
        WindowWidget::TabControl(gadget) => gadget.set_visible(visible),
        WindowWidget::ComboBox(gadget) => gadget.set_visible(visible),
        WindowWidget::TabPane
        | WindowWidget::User
        | WindowWidget::Animated
        | WindowWidget::MouseTrack => {}
    }
}

fn set_widget_enabled(widget: &mut WindowWidget, enabled: bool) {
    match widget {
        WindowWidget::PushButton(gadget) => gadget.set_enabled(enabled),
        WindowWidget::RadioButton(gadget) => gadget.set_enabled(enabled),
        WindowWidget::CheckBox(gadget) => gadget.set_enabled(enabled),
        WindowWidget::VerticalSlider(gadget) => gadget.set_enabled(enabled),
        WindowWidget::HorizontalSlider(gadget) => gadget.set_enabled(enabled),
        WindowWidget::ListBox(gadget) => gadget.set_enabled(enabled),
        WindowWidget::TextEntry(gadget) => gadget.set_enabled(enabled),
        WindowWidget::StaticText(gadget) => gadget.set_enabled(enabled),
        WindowWidget::ProgressBar(gadget) => gadget.set_enabled(enabled),
        WindowWidget::TabControl(gadget) => gadget.set_enabled(enabled),
        WindowWidget::ComboBox(gadget) => gadget.set_enabled(enabled),
        WindowWidget::TabPane
        | WindowWidget::User
        | WindowWidget::Animated
        | WindowWidget::MouseTrack => {}
    }
}

fn set_widget_bounds(widget: &mut WindowWidget, x: i32, y: i32, width: i32, height: i32) {
    let width_u = width.max(0) as u32;
    let height_u = height.max(0) as u32;
    match widget {
        WindowWidget::PushButton(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::RadioButton(gadget) => {
            let size = width.min(height).max(0) as u32;
            gadget.set_position(x, y);
            gadget.set_size(size, size);
        }
        WindowWidget::CheckBox(gadget) => {
            let size = width.min(height).max(0) as u32;
            gadget.set_position(x, y);
            gadget.set_size(size, size);
        }
        WindowWidget::VerticalSlider(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::HorizontalSlider(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::ListBox(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::TextEntry(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::StaticText(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::ProgressBar(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::TabControl(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::ComboBox(gadget) => {
            gadget.set_position(x, y);
            gadget.set_size(width_u, height_u);
        }
        WindowWidget::TabPane
        | WindowWidget::User
        | WindowWidget::Animated
        | WindowWidget::MouseTrack => {}
    }
}
