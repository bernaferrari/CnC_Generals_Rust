//! Gadget UI Control System
//!
//! This module provides the foundation for GUI gadgets in the Command & Conquer Generals
//! game client. It includes reusable UI components with consistent behavior, theming,
//! and state management.
//!
//! # Core Components
//!
//! - **Gadget Trait**: Base trait for all UI controls providing common functionality
//! - **PushButton**: Interactive buttons with click handling and visual states
//! - **StaticText**: Text display with formatting and alignment options
//! - **TextEntry**: Text input fields with validation and keyboard navigation
//! - **Slider**: Range controls for numeric value selection
//!
//! # Features
//!
//! - Consistent event handling and callback system
//! - Theming support with enabled/disabled/highlighted states
//! - Focus management and keyboard navigation
//! - Input validation and filtering
//! - Comprehensive documentation and examples
//!
//! # Example Usage
//!
//! ```rust
//! use game_client::gui::gadgets::*;
//!
//! // Create a button with callback
//! let button = PushButton::new(10, 10, 100, 30)
//!     .with_text("Click Me")
//!     .with_callback(|_| println!("Button clicked!"));
//!
//! // Create a text entry field
//! let entry = TextEntry::new(10, 50, 200, 25)
//!     .with_max_length(32)
//!     .with_validation(ValidationMode::AlphanumericOnly);
//!
//! // Create a slider
//! let slider = Slider::new(10, 90, 150, 20)
//!     .with_range(0, 100)
//!     .with_value(50);
//! ```

pub mod button;
pub mod checkbox;
pub mod combobox;
pub mod gadget_check_box;
pub mod gadget_combo_box;
pub mod gadget_horizontal_slider;
pub mod gadget_list_box;
pub mod gadget_progress_bar;
pub mod gadget_push_button;
pub mod gadget_radio_button;
pub mod gadget_static_text;
pub mod gadget_tab_control;
pub mod gadget_text_entry;
pub mod gadget_vertical_slider;
pub mod listbox;
pub mod progressbar;
pub mod radiobutton;
pub mod slider;
pub mod tabcontrol;
pub mod text;

pub use button::{
    register_button_audio_hook, ButtonAudioHook, ButtonCallback, ButtonStyle, ClockMode,
    PushButton, PushButtonBuilder, PushButtonRenderCommand,
};
pub use checkbox::{
    CheckBox, CheckBoxBuilder, CheckBoxCallback, CheckBoxConfig, CheckBoxRenderCommand,
    CheckBoxStyle,
};
pub use combobox::{ComboBox, ComboBoxCallback, ComboBoxItem, ComboBoxRenderCommand};
pub use listbox::{
    ListBox, ListBoxAddEntry, ListBoxCallback, ListBoxItem, ListBoxItemData, ListBoxRenderCommand,
    ListBoxRightClick, ListBoxSelection, ListBoxTextAndColor, SelectionMode,
};
pub use progressbar::{
    ProgressBar, ProgressBarBuilder, ProgressBarConfig, ProgressBarOrientation,
    ProgressBarRenderCommand, ProgressBarStyle,
};
pub use radiobutton::{
    RadioButton, RadioButtonBuilder, RadioButtonCallback, RadioButtonGroup,
    RadioButtonRenderCommand,
};
pub use slider::{
    HorizontalSlider, SliderCallback, SliderConfig, SliderOrientation, SliderRenderCommand,
    SliderStyle, VerticalSlider,
};
pub use tabcontrol::{Tab, TabCallback, TabControl, TabControlData, TabControlRenderCommand};
pub use text::{
    StaticText, TextAlignment, TextConfig, TextEntry, TextEntryCallback, TextRenderCommand,
    ValidationMode, VerticalAlignment,
};

use std::collections::HashMap;
use std::fmt;

/// Unique identifier for gadget instances
pub type GadgetId = u32;

/// Screen coordinates and dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Create a new rectangle
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is within this rectangle
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }

    /// Get the center point of the rectangle
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width / 2) as i32,
            self.y + (self.height / 2) as i32,
        )
    }
}

/// Color representation using RGBA components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color from RGB components (full opacity)
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a new color from RGBA components
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create a darker version of this color
    pub fn darken(&self, amount: u8) -> Self {
        Self {
            r: self.r.saturating_sub(amount),
            g: self.g.saturating_sub(amount),
            b: self.b.saturating_sub(amount),
            a: self.a,
        }
    }

    /// Create a lighter version of this color
    pub fn lighten(&self, amount: u8) -> Self {
        Self {
            r: self.r.saturating_add(amount).min(255),
            g: self.g.saturating_add(amount).min(255),
            b: self.b.saturating_add(amount).min(255),
            a: self.a,
        }
    }

    /// Common color constants
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const GREEN: Color = Color {
        r: 0,
        g: 255,
        b: 0,
        a: 255,
    };
    pub const BLUE: Color = Color {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    pub const GRAY: Color = Color {
        r: 128,
        g: 128,
        b: 128,
        a: 255,
    };
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
}

/// Visual state of a gadget
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GadgetState {
    /// Normal state
    Normal,
    /// Mouse is hovering over the gadget
    Hovered,
    /// Gadget is pressed/selected
    Pressed,
    /// Gadget is disabled
    Disabled,
    /// Gadget has keyboard focus
    Focused,
}

/// Mouse button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Key codes for keyboard input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Tab,
    Enter,
    Escape,
    Space,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    /// Character input (for text entry)
    Char(char),
}

/// Keyboard modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl KeyModifiers {
    pub fn none() -> Self {
        Self {
            shift: false,
            ctrl: false,
            alt: false,
        }
    }
}

/// Input events that gadgets can receive
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Mouse moved to position (x, y)
    MouseMove { x: i32, y: i32 },
    /// Mouse button pressed
    MouseDown { x: i32, y: i32, button: MouseButton },
    /// Mouse button released
    MouseUp { x: i32, y: i32, button: MouseButton },
    /// Mouse entered gadget area
    MouseEnter { x: i32, y: i32 },
    /// Mouse left gadget area
    MouseLeave { x: i32, y: i32 },
    /// Mouse dragged while button is pressed
    MouseDrag { x: i32, y: i32, button: MouseButton },
    /// Key pressed
    KeyDown {
        key: KeyCode,
        modifiers: KeyModifiers,
    },
    /// Key released
    KeyUp {
        key: KeyCode,
        modifiers: KeyModifiers,
    },
    /// Focus gained
    FocusGained,
    /// Focus lost
    FocusLost,
    /// Text input for text entry fields
    TextInput { text: String },
}

/// Messages that gadgets can send to their owners
#[derive(Debug, Clone)]
pub enum GadgetMessage {
    /// Gadget was clicked
    Clicked {
        gadget_id: GadgetId,
    },
    /// Gadget was right-clicked
    RightClicked {
        gadget_id: GadgetId,
    },
    /// Gadget value changed (for sliders, text fields, etc.)
    ValueChanged {
        gadget_id: GadgetId,
        value: GadgetValue,
    },
    /// Text entry editing completed
    EditingComplete {
        gadget_id: GadgetId,
        text: String,
    },
    /// Focus changed
    FocusChanged {
        gadget_id: GadgetId,
        has_focus: bool,
    },
    /// Mouse entered/left gadget
    MouseEnter {
        gadget_id: GadgetId,
    },
    MouseLeave {
        gadget_id: GadgetId,
    },
    /// Left mouse drag over gadget
    LeftDrag {
        gadget_id: GadgetId,
    },
    /// Generic message with custom data
    Custom {
        gadget_id: GadgetId,
        data: String,
    },
}

/// Values that gadgets can hold
#[derive(Debug, Clone)]
pub enum GadgetValue {
    Integer(i32),
    Float(f32),
    String(String),
    Boolean(bool),
}

impl fmt::Display for GadgetValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GadgetValue::Integer(v) => write!(f, "{}", v),
            GadgetValue::Float(v) => write!(f, "{}", v),
            GadgetValue::String(v) => write!(f, "{}", v),
            GadgetValue::Boolean(v) => write!(f, "{}", v),
        }
    }
}

/// Theme configuration for gadget appearance
#[derive(Debug, Clone)]
pub struct GadgetTheme {
    /// Colors for different states
    pub normal_color: Color,
    pub hovered_color: Color,
    pub pressed_color: Color,
    pub disabled_color: Color,
    pub focused_color: Color,

    /// Border colors
    pub border_color: Color,
    pub border_width: u32,

    /// Text colors
    pub text_color: Color,
    pub disabled_text_color: Color,

    /// Font information (simplified)
    pub font_size: u32,
}

impl Default for GadgetTheme {
    fn default() -> Self {
        Self {
            normal_color: Color::rgb(200, 200, 200),
            hovered_color: Color::rgb(220, 220, 220),
            pressed_color: Color::rgb(160, 160, 160),
            disabled_color: Color::rgb(128, 128, 128),
            focused_color: Color::rgb(180, 200, 255),
            border_color: Color::rgb(64, 64, 64),
            border_width: 1,
            text_color: Color::BLACK,
            disabled_text_color: Color::rgb(96, 96, 96),
            font_size: 12,
        }
    }
}

/// Tab navigation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDirection {
    Forward,
    Backward,
}

/// Base trait that all gadgets must implement
pub trait Gadget {
    /// Get the unique identifier for this gadget
    fn id(&self) -> GadgetId;

    /// Get the gadget's bounding rectangle
    fn bounds(&self) -> Rect;

    /// Set the gadget's position
    fn set_position(&mut self, x: i32, y: i32);

    /// Set the gadget's size
    fn set_size(&mut self, width: u32, height: u32);

    /// Get the gadget's current state
    fn state(&self) -> GadgetState;

    /// Check if the gadget is enabled
    fn is_enabled(&self) -> bool;

    /// Enable or disable the gadget
    fn set_enabled(&mut self, enabled: bool);

    /// Check if the gadget is visible
    fn is_visible(&self) -> bool;

    /// Show or hide the gadget
    fn set_visible(&mut self, visible: bool);

    /// Check if the gadget can receive focus
    fn can_focus(&self) -> bool;

    /// Check if the gadget has focus
    fn has_focus(&self) -> bool;

    /// Set focus state
    fn set_focus(&mut self, focused: bool);

    /// Handle input events
    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage>;

    /// Update gadget state (called each frame)
    fn update(&mut self, delta_time: f32);

    /// Render the gadget.
    fn render(&self, theme: &GadgetTheme);

    /// Get tooltip text if any
    fn tooltip(&self) -> Option<&str> {
        None
    }

    /// Handle tab navigation
    #[allow(unused_variables)]
    fn handle_tab(&mut self, direction: TabDirection) -> bool {
        false // Default: doesn't handle tab navigation
    }
}

/// Manager for handling multiple gadgets
pub struct GadgetManager {
    gadgets: HashMap<GadgetId, Box<dyn Gadget>>,
    focused_gadget: Option<GadgetId>,
    next_id: GadgetId,
    theme: GadgetTheme,
}

impl GadgetManager {
    /// Create a new gadget manager
    pub fn new() -> Self {
        Self {
            gadgets: HashMap::new(),
            focused_gadget: None,
            next_id: 1,
            theme: GadgetTheme::default(),
        }
    }

    /// Generate a new unique gadget ID
    pub fn generate_id(&mut self) -> GadgetId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Add a gadget to the manager
    pub fn add_gadget(&mut self, gadget: Box<dyn Gadget>) {
        let id = gadget.id();
        self.gadgets.insert(id, gadget);
    }

    /// Remove a gadget from the manager
    pub fn remove_gadget(&mut self, id: GadgetId) -> Option<Box<dyn Gadget>> {
        if self.focused_gadget == Some(id) {
            self.focused_gadget = None;
        }
        self.gadgets.remove(&id)
    }

    /// Get a reference to a gadget
    pub fn get_gadget(&self, id: GadgetId) -> Option<&dyn Gadget> {
        self.gadgets.get(&id).map(|g| g.as_ref())
    }

    /// Get a mutable reference to a gadget
    pub fn get_gadget_mut(&mut self, id: GadgetId) -> Option<&mut (dyn Gadget + 'static)> {
        self.gadgets.get_mut(&id).map(move |g| g.as_mut())
    }

    /// Handle input for all gadgets
    pub fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        let mut messages = Vec::new();

        match event {
            InputEvent::MouseMove { x, y }
            | InputEvent::MouseDown { x, y, .. }
            | InputEvent::MouseUp { x, y, .. } => {
                // Find gadget under mouse
                for gadget in self.gadgets.values_mut() {
                    if gadget.is_visible() && gadget.is_enabled() {
                        if gadget.bounds().contains_point(*x, *y) {
                            messages.extend(gadget.handle_input(event));
                        }
                    }
                }
            }

            InputEvent::KeyDown { key, .. } => {
                // Send keyboard events to focused gadget
                if let Some(focused_id) = self.focused_gadget {
                    if let Some(gadget) = self.gadgets.get_mut(&focused_id) {
                        if gadget.is_visible() && gadget.is_enabled() {
                            messages.extend(gadget.handle_input(event));
                        }
                    }
                }

                let (tab_direction, key_handled) = Self::take_control_messages(&mut messages);
                if let Some(direction) = tab_direction.or_else(|| match key {
                    KeyCode::Tab | KeyCode::Right | KeyCode::Down
                        if messages.is_empty() && !key_handled =>
                    {
                        Some(TabDirection::Forward)
                    }
                    KeyCode::Left | KeyCode::Up if messages.is_empty() && !key_handled => {
                        Some(TabDirection::Backward)
                    }
                    _ => None,
                }) {
                    self.handle_tab_navigation(direction);
                }
            }

            _ => {
                // Send keyboard events to focused gadget
                if let Some(focused_id) = self.focused_gadget {
                    if let Some(gadget) = self.gadgets.get_mut(&focused_id) {
                        if gadget.is_visible() && gadget.is_enabled() {
                            messages.extend(gadget.handle_input(event));
                        }
                    }
                }
            }
        }

        messages
    }

    fn take_control_messages(messages: &mut Vec<GadgetMessage>) -> (Option<TabDirection>, bool) {
        let mut direction = None;
        let mut key_handled = false;
        messages.retain(|message| {
            if let GadgetMessage::Custom { data, .. } = message {
                match data.as_str() {
                    "tab_next" => {
                        direction = Some(TabDirection::Forward);
                        return false;
                    }
                    "tab_prev" => {
                        direction = Some(TabDirection::Backward);
                        return false;
                    }
                    "key_handled" => {
                        key_handled = true;
                        return false;
                    }
                    _ => {}
                }
            }
            true
        });
        (direction, key_handled)
    }

    /// Set focus to a specific gadget
    pub fn set_focus(&mut self, id: Option<GadgetId>) -> bool {
        // Clear current focus
        if let Some(current_id) = self.focused_gadget {
            if let Some(gadget) = self.gadgets.get_mut(&current_id) {
                gadget.set_focus(false);
            }
        }

        // Set new focus
        if let Some(new_id) = id {
            if let Some(gadget) = self.gadgets.get_mut(&new_id) {
                if gadget.can_focus() && gadget.is_visible() && gadget.is_enabled() {
                    gadget.set_focus(true);
                    self.focused_gadget = Some(new_id);
                    return true;
                }
            }
        } else {
            self.focused_gadget = None;
        }

        false
    }

    /// Handle tab navigation between focusable gadgets
    pub fn handle_tab_navigation(&mut self, direction: TabDirection) {
        let mut focusable_ids: Vec<GadgetId> = self
            .gadgets
            .iter()
            .filter(|(_, g)| g.can_focus() && g.is_visible() && g.is_enabled())
            .map(|(id, _)| *id)
            .collect();
        focusable_ids.sort_unstable();

        if focusable_ids.is_empty() {
            return;
        }

        let next_index = match self.focused_gadget {
            Some(current_id) => {
                if let Some(current_index) = focusable_ids.iter().position(|&id| id == current_id) {
                    match direction {
                        TabDirection::Forward => (current_index + 1) % focusable_ids.len(),
                        TabDirection::Backward => {
                            if current_index == 0 {
                                focusable_ids.len() - 1
                            } else {
                                current_index - 1
                            }
                        }
                    }
                } else {
                    0 // Current focused gadget not found, start from beginning
                }
            }
            None => 0, // No current focus, start from beginning
        };

        if let Some(&next_id) = focusable_ids.get(next_index) {
            self.set_focus(Some(next_id));
        }
    }

    /// Update all gadgets
    pub fn update(&mut self, delta_time: f32) {
        for gadget in self.gadgets.values_mut() {
            gadget.update(delta_time);
        }
    }

    /// Render all visible gadgets
    pub fn render(&self) {
        for gadget in self.gadgets.values() {
            if gadget.is_visible() {
                gadget.render(&self.theme);
            }
        }
    }

    /// Set the theme for all gadgets
    pub fn set_theme(&mut self, theme: GadgetTheme) {
        self.theme = theme;
    }

    /// Get the current theme
    pub fn theme(&self) -> &GadgetTheme {
        &self.theme
    }

    /// Get all gadget IDs
    pub fn gadget_ids(&self) -> Vec<GadgetId> {
        self.gadgets.keys().copied().collect()
    }

    /// Clear all gadgets
    pub fn clear(&mut self) {
        self.gadgets.clear();
        self.focused_gadget = None;
    }
}

impl Default for GadgetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_contains_point() {
        let rect = Rect::new(10, 20, 100, 50);

        assert!(rect.contains_point(10, 20)); // Top-left corner
        assert!(rect.contains_point(50, 40)); // Center
        assert!(rect.contains_point(109, 69)); // Bottom-right inside

        assert!(!rect.contains_point(9, 20)); // Left of rect
        assert!(!rect.contains_point(110, 40)); // Right of rect
        assert!(!rect.contains_point(50, 19)); // Above rect
        assert!(!rect.contains_point(50, 70)); // Below rect
    }

    #[test]
    fn test_color_operations() {
        let color = Color::rgb(100, 150, 200);

        let darker = color.darken(20);
        assert_eq!(darker.r, 80);
        assert_eq!(darker.g, 130);
        assert_eq!(darker.b, 180);

        let lighter = color.lighten(20);
        assert_eq!(lighter.r, 120);
        assert_eq!(lighter.g, 170);
        assert_eq!(lighter.b, 220);
    }

    #[test]
    fn test_gadget_manager() {
        let mut manager = GadgetManager::new();

        // Test ID generation
        let id1 = manager.generate_id();
        let id2 = manager.generate_id();
        assert_ne!(id1, id2);
        assert!(id2 > id1);
    }

    #[test]
    fn test_keyboard_navigation_matches_push_button_arrow_parity() {
        let mut manager = GadgetManager::new();
        manager.add_gadget(Box::new(PushButton::new(10, 0, 0, 20, 20)));
        manager.add_gadget(Box::new(PushButton::new(20, 30, 0, 20, 20)));
        manager.add_gadget(Box::new(PushButton::new(30, 60, 0, 20, 20)));

        assert!(manager.set_focus(Some(10)));

        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Right,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(20));

        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Down,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(30));

        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Left,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(20));

        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Up,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(10));

        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Tab,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(20));
    }

    #[test]
    fn test_focused_listbox_keeps_arrow_selection_before_tab_fallback() {
        let mut manager = GadgetManager::new();
        let mut listbox = ListBox::new(10, 0, 0, 120, 80);
        listbox.add_item_with_id(100, "Alpha");
        listbox.add_item_with_id(200, "Bravo");
        manager.add_gadget(Box::new(listbox));
        manager.add_gadget(Box::new(PushButton::new(20, 140, 0, 20, 20)));

        assert!(manager.set_focus(Some(10)));

        let messages = manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Down,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(10));
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::ValueChanged {
                gadget_id: 10,
                value: GadgetValue::Integer(0)
            }]
        ));

        let messages = manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Down,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(10));
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::ValueChanged {
                gadget_id: 10,
                value: GadgetValue::Integer(1)
            }]
        ));

        let messages = manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Down,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(10));
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::ValueChanged {
                gadget_id: 10,
                value: GadgetValue::Integer(1)
            }]
        ));

        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Right,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(20));
    }

    #[test]
    fn test_focused_listbox_enter_and_space_emit_double_click() {
        let mut manager = GadgetManager::new();
        let mut listbox = ListBox::new(10, 0, 0, 120, 80);
        listbox.add_item_with_id(100, "Alpha");
        manager.add_gadget(Box::new(listbox));

        assert!(manager.set_focus(Some(10)));

        let messages = manager.handle_input(&InputEvent::KeyUp {
            key: KeyCode::Enter,
            modifiers: KeyModifiers::none(),
        });
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::Custom { gadget_id: 10, data } ] if data == "double_click"
        ));

        let messages = manager.handle_input(&InputEvent::KeyUp {
            key: KeyCode::Space,
            modifiers: KeyModifiers::none(),
        });
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::Custom { gadget_id: 10, data } ] if data == "double_click"
        ));
    }

    #[test]
    fn test_focused_slider_perpendicular_keys_use_cpp_tab_navigation() {
        let mut manager = GadgetManager::new();
        manager.add_gadget(Box::new(
            HorizontalSlider::new(10, 0, 0, 120, 20)
                .with_range(0, 10)
                .with_value(5),
        ));
        manager.add_gadget(Box::new(PushButton::new(20, 140, 0, 20, 20)));
        manager.add_gadget(Box::new(
            VerticalSlider::new(30, 170, 0, 20, 120)
                .with_range(0, 10)
                .with_value(5),
        ));

        assert!(manager.set_focus(Some(10)));
        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Down,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(20));

        assert!(manager.set_focus(Some(30)));
        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Right,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(10));

        assert!(manager.set_focus(Some(30)));
        manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Left,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(manager.focused_gadget, Some(20));
    }

    #[test]
    fn test_focused_slider_boundary_key_does_not_fall_through_to_tab_navigation() {
        let mut manager = GadgetManager::new();
        manager.add_gadget(Box::new(
            HorizontalSlider::new(10, 0, 0, 120, 20)
                .with_range(0, 10)
                .with_value(0)
                .with_step_size(1),
        ));
        manager.add_gadget(Box::new(PushButton::new(20, 140, 0, 20, 20)));

        assert!(manager.set_focus(Some(10)));
        let messages = manager.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Right,
            modifiers: KeyModifiers::none(),
        });

        assert_eq!(manager.focused_gadget, Some(10));
        assert!(messages.is_empty());
    }
}
