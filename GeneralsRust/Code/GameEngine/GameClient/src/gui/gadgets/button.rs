//! Push Button Implementation
//!
//! This module provides interactive button controls with support for:
//! - Click handling and visual feedback
//! - Checkbox-like toggle behavior
//! - Multiple visual states (normal, hovered, pressed, disabled)
//! - Text labels and overlay images
//! - Clock/progress indicators
//! - Custom borders and theming
//!
//! # Examples
//!
//! ```rust
//! use game_client::gui::gadgets::button::*;
//!
//! // Create a simple button
//! let mut button = PushButton::new(1, 10, 10, 100, 30)
//!     .with_text("Click Me")
//!     .with_callback(Box::new(|id| println!("Button {} clicked", id)));
//!
//! // Create a toggle button (checkbox-like)
//! let mut toggle = PushButton::new(2, 10, 50, 100, 30)
//!     .with_text("Toggle")
//!     .as_checkbox(false); // Initially unchecked
//!
//! // Create a button with custom styling
//! let mut styled_button = PushButton::new(3, 10, 90, 100, 30)
//!     .with_text("Styled")
//!     .with_border_color(Color::BLUE)
//!     .with_clock_progress(75, Color::GREEN); // 75% progress indicator
//! ```

use super::*;
use std::time::{Duration, Instant};

/// Callback function type for button events
pub type ButtonCallback = Box<dyn Fn(GadgetId) + Send + Sync>;

/// Clock display mode for progress indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockMode {
    /// No clock display
    None,
    /// Normal clock (fills with progress)
    Normal,
    /// Inverse clock (empties with progress)
    Inverse,
}

/// Visual styling data for buttons
#[derive(Debug, Clone)]
pub struct ButtonStyle {
    /// Border configuration
    pub draw_border: bool,
    pub border_color: Color,

    /// Clock/progress indicator
    pub clock_mode: ClockMode,
    pub clock_progress: u8, // 0-100
    pub clock_color: Color,

    /// Overlay image (for icons, badges, etc.)
    pub overlay_image: Option<String>, // Path to overlay image

    /// Custom sound for button clicks
    pub alt_sound: Option<String>,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            draw_border: false,
            border_color: Color::BLACK,
            clock_mode: ClockMode::None,
            clock_progress: 0,
            clock_color: Color::BLUE,
            overlay_image: None,
            alt_sound: None,
        }
    }
}

/// Push button gadget with comprehensive interaction support
pub struct PushButton {
    // Base gadget properties
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,

    // Button-specific properties
    text: String,
    is_checkbox: bool,
    is_checked: bool,
    triggers_on_mouse_down: bool,

    // Visual styling
    style: ButtonStyle,

    // Event handling
    callback: Option<ButtonCallback>,
    right_click_callback: Option<ButtonCallback>,

    // State tracking
    mouse_inside: bool,
    mouse_pressed: bool,

    // Animation support
    last_click_time: Option<Instant>,
    double_click_threshold: Duration,

    // Custom user data
    user_data: Option<String>,
}

impl PushButton {
    /// Create a new push button
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,

            text: String::new(),
            is_checkbox: false,
            is_checked: false,
            triggers_on_mouse_down: false,

            style: ButtonStyle::default(),

            callback: None,
            right_click_callback: None,

            mouse_inside: false,
            mouse_pressed: false,

            last_click_time: None,
            double_click_threshold: Duration::from_millis(300),

            user_data: None,
        }
    }

    pub fn style(&self) -> &ButtonStyle {
        &self.style
    }

    /// Set the button text
    pub fn with_text<S: Into<String>>(mut self, text: S) -> Self {
        self.text = text.into();
        self
    }

    /// Set the button text (mutable)
    pub fn set_text<S: Into<String>>(&mut self, text: S) {
        self.text = text.into();
    }

    /// Get the button text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Configure as checkbox-like button with initial checked state
    pub fn as_checkbox(mut self, initially_checked: bool) -> Self {
        self.is_checkbox = true;
        self.is_checked = initially_checked;
        self.triggers_on_mouse_down = true;
        self
    }

    /// Set checkbox behavior
    pub fn set_checkbox(&mut self, is_checkbox: bool, initially_checked: bool) {
        self.is_checkbox = is_checkbox;
        self.is_checked = initially_checked;
        if is_checkbox {
            self.triggers_on_mouse_down = true;
        }
    }

    /// Check if button is in checkbox mode
    pub fn is_checkbox(&self) -> bool {
        self.is_checkbox
    }

    /// Get checkbox state (only valid for checkbox buttons)
    pub fn is_checked(&self) -> bool {
        self.is_checked
    }

    /// Set checkbox state visually (doesn't trigger callback)
    pub fn set_checked(&mut self, checked: bool) {
        if self.is_checkbox {
            self.is_checked = checked;
        }
    }

    /// Set whether button triggers on mouse down vs mouse up
    pub fn set_triggers_on_mouse_down(&mut self, on_down: bool) {
        self.triggers_on_mouse_down = on_down;
    }

    /// Set click callback
    pub fn with_callback(mut self, callback: ButtonCallback) -> Self {
        self.callback = Some(callback);
        self
    }

    /// Set click callback (mutable)
    pub fn set_callback(&mut self, callback: ButtonCallback) {
        self.callback = Some(callback);
    }

    /// Set right-click callback
    pub fn with_right_click_callback(mut self, callback: ButtonCallback) -> Self {
        self.right_click_callback = Some(callback);
        self
    }

    /// Set right-click callback (mutable)
    pub fn set_right_click_callback(&mut self, callback: ButtonCallback) {
        self.right_click_callback = Some(callback);
    }

    /// Configure border display
    pub fn with_border_color(mut self, color: Color) -> Self {
        self.style.draw_border = true;
        self.style.border_color = color;
        self
    }

    /// Set border configuration
    pub fn set_border(&mut self, draw_border: bool, color: Color) {
        self.style.draw_border = draw_border;
        self.style.border_color = color;
    }

    /// Configure clock/progress indicator
    pub fn with_clock_progress(mut self, progress: u8, color: Color) -> Self {
        self.style.clock_mode = ClockMode::Normal;
        self.style.clock_progress = progress.min(100);
        self.style.clock_color = color;
        self
    }

    /// Set clock progress (0-100)
    pub fn set_clock_progress(&mut self, progress: u8, color: Color) {
        self.style.clock_mode = ClockMode::Normal;
        self.style.clock_progress = progress.min(100);
        self.style.clock_color = color;
    }

    /// Configure inverse clock (shows remaining progress)
    pub fn with_inverse_clock(mut self, progress: u8, color: Color) -> Self {
        self.style.clock_mode = ClockMode::Inverse;
        self.style.clock_progress = progress.min(100);
        self.style.clock_color = color;
        self
    }

    /// Set inverse clock progress
    pub fn set_inverse_clock(&mut self, progress: u8, color: Color) {
        self.style.clock_mode = ClockMode::Inverse;
        self.style.clock_progress = progress.min(100);
        self.style.clock_color = color;
    }

    /// Clear clock display
    pub fn clear_clock(&mut self) {
        self.style.clock_mode = ClockMode::None;
        self.style.clock_progress = 0;
    }

    /// Set overlay image
    pub fn with_overlay_image<S: Into<String>>(mut self, image_path: S) -> Self {
        self.style.overlay_image = Some(image_path.into());
        self
    }

    /// Set overlay image (mutable)
    pub fn set_overlay_image<S: Into<String>>(&mut self, image_path: Option<S>) {
        self.style.overlay_image = image_path.map(|s| s.into());
    }

    /// Set alternative sound for clicks
    pub fn with_alt_sound<S: Into<String>>(mut self, sound: S) -> Self {
        self.style.alt_sound = Some(sound.into());
        self
    }

    /// Set alternative sound (mutable)
    pub fn set_alt_sound<S: Into<String>>(&mut self, sound: Option<S>) {
        self.style.alt_sound = sound.map(|s| s.into());
    }

    /// Set custom user data
    pub fn with_user_data<S: Into<String>>(mut self, data: S) -> Self {
        self.user_data = Some(data.into());
        self
    }

    /// Set user data (mutable)
    pub fn set_user_data<S: Into<String>>(&mut self, data: Option<S>) {
        self.user_data = data.map(|s| s.into());
    }

    /// Get user data
    pub fn user_data(&self) -> Option<&str> {
        self.user_data.as_deref()
    }

    /// Check if button is currently pressed
    pub fn is_pressed(&self) -> bool {
        matches!(self.state, GadgetState::Pressed) || (self.is_checkbox && self.is_checked)
    }

    /// Check if mouse is currently inside button
    pub fn is_mouse_inside(&self) -> bool {
        self.mouse_inside
    }

    /// Handle mouse button press
    fn handle_mouse_press(&mut self, button: MouseButton) -> Vec<GadgetMessage> {
        if !self.enabled {
            return Vec::new();
        }

        let mut messages = Vec::new();
        self.mouse_pressed = true;

        // Play click sound (would be handled by audio system)
        // This is a placeholder for actual audio integration

        match button {
            MouseButton::Left => {
                if self.is_checkbox {
                    // Toggle checkbox state
                    self.is_checked = !self.is_checked;
                    self.state = if self.is_checked {
                        GadgetState::Pressed
                    } else {
                        if self.mouse_inside {
                            GadgetState::Hovered
                        } else {
                            GadgetState::Normal
                        }
                    };
                } else {
                    self.state = GadgetState::Pressed;
                }

                if self.triggers_on_mouse_down {
                    messages.push(GadgetMessage::Clicked { gadget_id: self.id });
                    if let Some(ref callback) = self.callback {
                        callback(self.id);
                    }
                }
            }

            MouseButton::Right => {
                if self.is_checkbox {
                    // Right-click also toggles for checkboxes
                    self.is_checked = !self.is_checked;
                    self.state = if self.is_checked {
                        GadgetState::Pressed
                    } else {
                        if self.mouse_inside {
                            GadgetState::Hovered
                        } else {
                            GadgetState::Normal
                        }
                    };

                    messages.push(GadgetMessage::RightClicked { gadget_id: self.id });
                    if let Some(ref callback) = self.right_click_callback {
                        callback(self.id);
                    }
                } else {
                    self.state = GadgetState::Pressed;
                }
            }

            _ => {}
        }

        messages
    }

    /// Handle mouse button release
    fn handle_mouse_release(&mut self, button: MouseButton) -> Vec<GadgetMessage> {
        if !self.enabled {
            return Vec::new();
        }

        let mut messages = Vec::new();
        let was_pressed = self.mouse_pressed;
        self.mouse_pressed = false;

        match button {
            MouseButton::Left => {
                if !self.is_checkbox {
                    // Update state based on mouse position
                    self.state = if self.mouse_inside {
                        GadgetState::Hovered
                    } else {
                        GadgetState::Normal
                    };

                    // Trigger callback if mouse is still inside and we haven't triggered yet
                    if self.mouse_inside && was_pressed && !self.triggers_on_mouse_down {
                        messages.push(GadgetMessage::Clicked { gadget_id: self.id });
                        if let Some(ref callback) = self.callback {
                            callback(self.id);
                        }
                    }
                }
            }

            MouseButton::Right => {
                if !self.is_checkbox {
                    self.state = if self.mouse_inside {
                        GadgetState::Hovered
                    } else {
                        GadgetState::Normal
                    };

                    if self.mouse_inside && was_pressed {
                        messages.push(GadgetMessage::RightClicked { gadget_id: self.id });
                        if let Some(ref callback) = self.right_click_callback {
                            callback(self.id);
                        }
                    }
                }
            }

            _ => {}
        }

        messages
    }

    /// Handle mouse enter/leave
    fn handle_mouse_enter(&mut self) -> Vec<GadgetMessage> {
        self.mouse_inside = true;

        if self.enabled
            && !matches!(self.state, GadgetState::Pressed)
            && (!self.is_checkbox || !self.is_checked)
        {
            self.state = GadgetState::Hovered;
        }

        vec![GadgetMessage::MouseEnter { gadget_id: self.id }]
    }

    fn handle_mouse_leave(&mut self) -> Vec<GadgetMessage> {
        self.mouse_inside = false;

        // Clear selected state for non-checkbox buttons when mouse leaves
        if self.enabled && !self.is_checkbox && self.mouse_pressed {
            self.mouse_pressed = false;
        }

        if self.enabled
            && !matches!(self.state, GadgetState::Pressed)
            && (!self.is_checkbox || !self.is_checked)
        {
            self.state = GadgetState::Normal;
        }

        vec![GadgetMessage::MouseLeave { gadget_id: self.id }]
    }

    /// Get the appropriate color for current state
    fn get_current_color(&self, theme: &GadgetTheme) -> Color {
        if !self.enabled {
            return theme.disabled_color;
        }

        match self.state {
            GadgetState::Normal => theme.normal_color,
            GadgetState::Hovered => theme.hovered_color,
            GadgetState::Pressed => theme.pressed_color,
            GadgetState::Disabled => theme.disabled_color,
            GadgetState::Focused => theme.focused_color,
        }
    }

    /// Get the appropriate text color for current state
    fn get_current_text_color(&self, theme: &GadgetTheme) -> Color {
        if self.enabled {
            theme.text_color
        } else {
            theme.disabled_text_color
        }
    }
}

impl Gadget for PushButton {
    fn id(&self) -> GadgetId {
        self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn state(&self) -> GadgetState {
        self.state
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.state = GadgetState::Disabled;
            self.mouse_inside = false;
            self.mouse_pressed = false;
        } else {
            self.state = GadgetState::Normal;
        }
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.mouse_inside = false;
            self.mouse_pressed = false;
        }
    }

    fn can_focus(&self) -> bool {
        true // Buttons can always receive focus when enabled and visible
    }

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        let was_focused = self.focused;
        self.focused = focused;

        if focused
            && self.enabled
            && !matches!(self.state, GadgetState::Pressed)
            && (!self.is_checkbox || !self.is_checked)
        {
            self.state = GadgetState::Focused;
        } else if !focused && was_focused && self.enabled {
            self.state = if self.mouse_inside {
                GadgetState::Hovered
            } else {
                GadgetState::Normal
            };
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.enabled || !self.visible {
            return Vec::new();
        }

        match event {
            InputEvent::MouseDown { button, .. } => self.handle_mouse_press(*button),

            InputEvent::MouseUp { button, .. } => self.handle_mouse_release(*button),

            InputEvent::MouseDrag { button, .. } => {
                if *button == MouseButton::Left {
                    return vec![GadgetMessage::LeftDrag { gadget_id: self.id }];
                }
                Vec::new()
            }

            InputEvent::MouseEnter { .. } => self.handle_mouse_enter(),

            InputEvent::MouseLeave { .. } => self.handle_mouse_leave(),

            InputEvent::KeyDown { key, .. } => {
                if self.focused {
                    match key {
                        KeyCode::Enter | KeyCode::Space => {
                            // Simulate click
                            let mut messages = self.handle_mouse_press(MouseButton::Left);
                            messages.extend(self.handle_mouse_release(MouseButton::Left));
                            messages
                        }
                        _ => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            }

            InputEvent::FocusGained => {
                self.set_focus(true);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: true,
                }]
            }

            InputEvent::FocusLost => {
                self.set_focus(false);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: false,
                }]
            }

            _ => Vec::new(),
        }
    }

    fn update(&mut self, _delta_time: f32) {
        // Button doesn't need frame-based updates currently
        // Could add animations, hover effects, etc. here
    }

    fn render(&self, theme: &GadgetTheme) {
        // This is a placeholder for actual rendering
        // In a real implementation, this would draw the button using the graphics system

        let bg_color = self.get_current_color(theme);
        let text_color = self.get_current_text_color(theme);

        // Pseudo-rendering code (actual implementation would use graphics API)
        println!(
            "Rendering button {} at ({}, {}) {}x{}",
            self.id, self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height
        );
        println!("  Background: {:?}", bg_color);
        println!("  Text: '{}' in color {:?}", self.text, text_color);

        if self.style.draw_border {
            println!("  Border: {:?}", self.style.border_color);
        }

        match self.style.clock_mode {
            ClockMode::Normal => {
                println!(
                    "  Clock progress: {}% in {:?}",
                    self.style.clock_progress, self.style.clock_color
                );
            }
            ClockMode::Inverse => {
                println!(
                    "  Inverse clock: {}% remaining in {:?}",
                    100 - self.style.clock_progress,
                    self.style.clock_color
                );
            }
            ClockMode::None => {}
        }

        if let Some(ref overlay) = self.style.overlay_image {
            println!("  Overlay image: {}", overlay);
        }

        if self.is_checkbox {
            println!(
                "  Checkbox state: {}",
                if self.is_checked {
                    "checked"
                } else {
                    "unchecked"
                }
            );
        }
    }

    fn handle_tab(&mut self, direction: TabDirection) -> bool {
        // Buttons participate in tab navigation
        match direction {
            TabDirection::Forward | TabDirection::Backward => true,
        }
    }
}

/// Builder for creating buttons with various configurations
pub struct PushButtonBuilder {
    id: GadgetId,
    bounds: Rect,
    text: String,
    is_checkbox: bool,
    initially_checked: bool,
    callback: Option<ButtonCallback>,
    right_click_callback: Option<ButtonCallback>,
    style: ButtonStyle,
    user_data: Option<String>,
    triggers_on_mouse_down: bool,
}

impl PushButtonBuilder {
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            text: String::new(),
            is_checkbox: false,
            initially_checked: false,
            callback: None,
            right_click_callback: None,
            style: ButtonStyle::default(),
            user_data: None,
            triggers_on_mouse_down: false,
        }
    }

    pub fn text<S: Into<String>>(mut self, text: S) -> Self {
        self.text = text.into();
        self
    }

    pub fn checkbox(mut self, initially_checked: bool) -> Self {
        self.is_checkbox = true;
        self.initially_checked = initially_checked;
        self.triggers_on_mouse_down = true;
        self
    }

    pub fn callback(mut self, callback: ButtonCallback) -> Self {
        self.callback = Some(callback);
        self
    }

    pub fn right_click_callback(mut self, callback: ButtonCallback) -> Self {
        self.right_click_callback = Some(callback);
        self
    }

    pub fn border(mut self, color: Color) -> Self {
        self.style.draw_border = true;
        self.style.border_color = color;
        self
    }

    pub fn clock(mut self, progress: u8, color: Color) -> Self {
        self.style.clock_mode = ClockMode::Normal;
        self.style.clock_progress = progress.min(100);
        self.style.clock_color = color;
        self
    }

    pub fn inverse_clock(mut self, progress: u8, color: Color) -> Self {
        self.style.clock_mode = ClockMode::Inverse;
        self.style.clock_progress = progress.min(100);
        self.style.clock_color = color;
        self
    }

    pub fn overlay_image<S: Into<String>>(mut self, image_path: S) -> Self {
        self.style.overlay_image = Some(image_path.into());
        self
    }

    pub fn alt_sound<S: Into<String>>(mut self, sound: S) -> Self {
        self.style.alt_sound = Some(sound.into());
        self
    }

    pub fn user_data<S: Into<String>>(mut self, data: S) -> Self {
        self.user_data = Some(data.into());
        self
    }

    pub fn triggers_on_mouse_down(mut self, on_down: bool) -> Self {
        self.triggers_on_mouse_down = on_down;
        self
    }

    pub fn build(self) -> PushButton {
        let mut button = PushButton::new(
            self.id,
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );

        button.text = self.text;
        button.is_checkbox = self.is_checkbox;
        button.is_checked = self.initially_checked;
        button.callback = self.callback;
        button.right_click_callback = self.right_click_callback;
        button.style = self.style;
        button.user_data = self.user_data;
        button.triggers_on_mouse_down = self.triggers_on_mouse_down;

        button
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_creation() {
        let button = PushButton::new(1, 10, 20, 100, 30);
        assert_eq!(button.id(), 1);
        assert_eq!(button.bounds(), Rect::new(10, 20, 100, 30));
        assert!(button.is_enabled());
        assert!(button.is_visible());
        assert!(!button.has_focus());
        assert!(!button.is_checkbox());
        assert!(!button.is_checked());
    }

    #[test]
    fn test_checkbox_behavior() {
        let mut button = PushButton::new(1, 0, 0, 100, 30).as_checkbox(false);

        assert!(button.is_checkbox());
        assert!(!button.is_checked());

        // Simulate left click
        let messages = button.handle_input(&InputEvent::MouseDown {
            x: 50,
            y: 15,
            button: MouseButton::Left,
        });

        assert!(button.is_checked());
        assert_eq!(messages.len(), 1);

        if let GadgetMessage::Clicked { gadget_id } = &messages[0] {
            assert_eq!(*gadget_id, 1);
        } else {
            panic!("Expected Clicked message");
        }
    }

    #[test]
    fn test_button_states() {
        let mut button = PushButton::new(1, 0, 0, 100, 30);

        // Test enable/disable
        assert_eq!(button.state(), GadgetState::Normal);
        button.set_enabled(false);
        assert_eq!(button.state(), GadgetState::Disabled);
        button.set_enabled(true);
        assert_eq!(button.state(), GadgetState::Normal);

        // Test focus
        button.set_focus(true);
        assert_eq!(button.state(), GadgetState::Focused);
        assert!(button.has_focus());
    }

    #[test]
    fn test_button_text() {
        let mut button = PushButton::new(1, 0, 0, 100, 30).with_text("Test Button");

        assert_eq!(button.text(), "Test Button");

        button.set_text("Updated Text");
        assert_eq!(button.text(), "Updated Text");
    }

    #[test]
    fn test_button_builder() {
        let button = PushButtonBuilder::new(1, 10, 20, 100, 30)
            .text("Builder Button")
            .checkbox(true)
            .border(Color::BLUE)
            .clock(50, Color::GREEN)
            .build();

        assert_eq!(button.text(), "Builder Button");
        assert!(button.is_checkbox());
        assert!(button.is_checked());
    }

    #[test]
    fn test_mouse_events() {
        let mut button = PushButton::new(1, 0, 0, 100, 30);

        // Mouse enter
        let messages = button.handle_input(&InputEvent::MouseEnter { x: 50, y: 15 });
        assert_eq!(button.state(), GadgetState::Hovered);
        assert!(button.is_mouse_inside());
        assert_eq!(messages.len(), 1);

        // Mouse leave
        let messages = button.handle_input(&InputEvent::MouseLeave { x: 150, y: 15 });
        assert_eq!(button.state(), GadgetState::Normal);
        assert!(!button.is_mouse_inside());
        assert_eq!(messages.len(), 1);
    }
}
