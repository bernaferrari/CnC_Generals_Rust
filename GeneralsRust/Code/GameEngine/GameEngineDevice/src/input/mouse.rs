//! Mouse device abstraction with cross-platform support

use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{InputError, Result};

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
    Button6,
    Button7,
    Button8,
}

impl MouseButton {
    /// Get the display name for this button
    pub fn name(&self) -> &'static str {
        match self {
            MouseButton::Left => "Left Button",
            MouseButton::Right => "Right Button",
            MouseButton::Middle => "Middle Button",
            MouseButton::Button4 => "Button 4",
            MouseButton::Button5 => "Button 5",
            MouseButton::Button6 => "Button 6",
            MouseButton::Button7 => "Button 7",
            MouseButton::Button8 => "Button 8",
        }
    }
}

/// Mouse cursor mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorMode {
    /// Normal visible cursor
    Normal,
    /// Hidden cursor
    Hidden,
    /// Locked cursor (FPS-style)
    Locked,
    /// Confined to window
    Confined,
}

/// Mouse state tracking position and buttons
#[derive(Debug, Clone)]
pub struct MouseState {
    /// Current X position in pixels
    position_x: i32,

    /// Current Y position in pixels
    position_y: i32,

    /// Previous X position (for delta calculation)
    prev_x: i32,

    /// Previous Y position (for delta calculation)
    prev_y: i32,

    /// Currently pressed buttons
    pressed_buttons: HashSet<MouseButton>,

    /// Scroll wheel delta X
    wheel_delta_x: f32,

    /// Scroll wheel delta Y
    wheel_delta_y: f32,

    /// Cursor mode
    cursor_mode: CursorMode,

    /// Whether cursor is inside window
    cursor_in_window: bool,
}

impl MouseState {
    /// Create a new mouse state
    pub fn new() -> Self {
        Self {
            position_x: 0,
            position_y: 0,
            prev_x: 0,
            prev_y: 0,
            pressed_buttons: HashSet::new(),
            wheel_delta_x: 0.0,
            wheel_delta_y: 0.0,
            cursor_mode: CursorMode::Normal,
            cursor_in_window: true,
        }
    }

    /// Get current X position
    pub fn x(&self) -> i32 {
        self.position_x
    }

    /// Get current Y position
    pub fn y(&self) -> i32 {
        self.position_y
    }

    /// Get position as tuple
    pub fn position(&self) -> (i32, i32) {
        (self.position_x, self.position_y)
    }

    /// Get X delta since last update
    pub fn delta_x(&self) -> i32 {
        self.position_x - self.prev_x
    }

    /// Get Y delta since last update
    pub fn delta_y(&self) -> i32 {
        self.position_y - self.prev_y
    }

    /// Get delta as tuple
    pub fn delta(&self) -> (i32, i32) {
        (self.delta_x(), self.delta_y())
    }

    /// Check if a button is currently pressed
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.pressed_buttons.contains(&button)
    }

    /// Check if any button is pressed
    pub fn any_button_pressed(&self) -> bool {
        !self.pressed_buttons.is_empty()
    }

    /// Get all currently pressed buttons
    pub fn pressed_buttons(&self) -> impl Iterator<Item = &MouseButton> {
        self.pressed_buttons.iter()
    }

    /// Get wheel delta X
    pub fn wheel_delta_x(&self) -> f32 {
        self.wheel_delta_x
    }

    /// Get wheel delta Y
    pub fn wheel_delta_y(&self) -> f32 {
        self.wheel_delta_y
    }

    /// Get cursor mode
    pub fn cursor_mode(&self) -> CursorMode {
        self.cursor_mode
    }

    /// Check if cursor is in window
    pub fn is_cursor_in_window(&self) -> bool {
        self.cursor_in_window
    }

    /// Update position
    fn set_position(&mut self, x: i32, y: i32) {
        self.prev_x = self.position_x;
        self.prev_y = self.position_y;
        self.position_x = x;
        self.position_y = y;
    }

    /// Press a button
    fn press_button(&mut self, button: MouseButton) {
        self.pressed_buttons.insert(button);
    }

    /// Release a button
    fn release_button(&mut self, button: MouseButton) {
        self.pressed_buttons.remove(&button);
    }

    /// Set wheel delta
    fn set_wheel_delta(&mut self, delta_x: f32, delta_y: f32) {
        self.wheel_delta_x = delta_x;
        self.wheel_delta_y = delta_y;
    }

    /// Clear wheel delta
    fn clear_wheel_delta(&mut self) {
        self.wheel_delta_x = 0.0;
        self.wheel_delta_y = 0.0;
    }

    /// Set cursor mode
    fn set_cursor_mode(&mut self, mode: CursorMode) {
        self.cursor_mode = mode;
    }

    /// Set cursor in window state
    fn set_cursor_in_window(&mut self, in_window: bool) {
        self.cursor_in_window = in_window;
    }

    /// Clear all state
    fn clear(&mut self) {
        self.position_x = 0;
        self.position_y = 0;
        self.prev_x = 0;
        self.prev_y = 0;
        self.pressed_buttons.clear();
        self.wheel_delta_x = 0.0;
        self.wheel_delta_y = 0.0;
        self.cursor_in_window = true;
    }
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

/// Mouse device with sensitivity and acceleration support
pub struct MouseDevice {
    /// Current mouse state
    state: MouseState,

    /// Mouse sensitivity multiplier
    sensitivity: f32,

    /// Whether to use raw input (no OS acceleration)
    raw_input: bool,

    /// Accumulated fractional movement (for sub-pixel precision)
    accumulated_x: f32,
    accumulated_y: f32,
}

impl MouseDevice {
    /// Create a new mouse device
    pub fn new(sensitivity: f32, raw_input: bool) -> Result<Self> {
        if sensitivity <= 0.0 {
            return Err(InputError::InvalidConfiguration(
                "Mouse sensitivity must be positive".into(),
            ));
        }

        Ok(Self {
            state: MouseState::new(),
            sensitivity,
            raw_input,
            accumulated_x: 0.0,
            accumulated_y: 0.0,
        })
    }

    /// Get current mouse state
    pub fn state(&self) -> MouseState {
        self.state.clone()
    }

    /// Handle mouse movement
    pub fn handle_move(&mut self, x: i32, y: i32) {
        self.state.set_position(x, y);
    }

    /// Handle raw mouse movement (delta-based)
    pub fn handle_raw_move(&mut self, delta_x: f32, delta_y: f32) {
        // Apply sensitivity
        let adjusted_x = delta_x * self.sensitivity;
        let adjusted_y = delta_y * self.sensitivity;

        // Accumulate fractional movement
        self.accumulated_x += adjusted_x;
        self.accumulated_y += adjusted_y;

        // Extract integer part
        let int_x = self.accumulated_x as i32;
        let int_y = self.accumulated_y as i32;

        // Keep fractional part
        self.accumulated_x -= int_x as f32;
        self.accumulated_y -= int_y as f32;

        // Update position
        let new_x = self.state.position_x + int_x;
        let new_y = self.state.position_y + int_y;
        self.state.set_position(new_x, new_y);
    }

    /// Handle button press
    pub fn handle_button_press(&mut self, button: MouseButton) {
        self.state.press_button(button);
    }

    /// Handle button release
    pub fn handle_button_release(&mut self, button: MouseButton) {
        self.state.release_button(button);
    }

    /// Handle mouse wheel
    pub fn handle_wheel(&mut self, delta_x: f32, delta_y: f32) {
        self.state.set_wheel_delta(delta_x, delta_y);
    }

    /// Handle cursor entering window
    pub fn handle_cursor_enter(&mut self) {
        self.state.set_cursor_in_window(true);
    }

    /// Handle cursor leaving window
    pub fn handle_cursor_leave(&mut self) {
        self.state.set_cursor_in_window(false);
    }

    /// Set cursor mode
    pub fn set_cursor_mode(&mut self, mode: CursorMode) -> Result<()> {
        self.state.set_cursor_mode(mode);
        // Platform-specific cursor mode change would happen here
        Ok(())
    }

    /// Update mouse state (call once per frame)
    pub fn update(&mut self, _delta_time: Duration) {
        // Clear wheel delta each frame
        self.state.clear_wheel_delta();
    }

    /// Get current position
    pub fn position(&self) -> (i32, i32) {
        self.state.position()
    }

    /// Get delta since last frame
    pub fn delta(&self) -> (i32, i32) {
        self.state.delta()
    }

    /// Check if button is pressed
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.state.is_button_pressed(button)
    }

    /// Get sensitivity
    pub fn sensitivity(&self) -> f32 {
        self.sensitivity
    }

    /// Set sensitivity
    pub fn set_sensitivity(&mut self, sensitivity: f32) -> Result<()> {
        if sensitivity <= 0.0 {
            return Err(InputError::InvalidConfiguration(
                "Mouse sensitivity must be positive".into(),
            ));
        }
        self.sensitivity = sensitivity;
        Ok(())
    }

    /// Check if using raw input
    pub fn is_raw_input(&self) -> bool {
        self.raw_input
    }

    /// Set raw input mode
    pub fn set_raw_input(&mut self, enabled: bool) {
        self.raw_input = enabled;
        // Platform-specific raw input registration would happen here
    }

    /// Warp mouse to position
    pub fn warp_to(&mut self, x: i32, y: i32) -> Result<()> {
        self.state.set_position(x, y);
        // Platform-specific cursor warping would happen here
        Ok(())
    }

    /// Center mouse in window
    pub fn center(&mut self, window_width: u32, window_height: u32) -> Result<()> {
        let center_x = (window_width / 2) as i32;
        let center_y = (window_height / 2) as i32;
        self.warp_to(center_x, center_y)
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.state.clear();
        self.accumulated_x = 0.0;
        self.accumulated_y = 0.0;
    }
}

/// Mouse capture state for UI interaction
#[derive(Debug, Clone)]
pub struct MouseCapture {
    /// Whether mouse is captured
    captured: bool,

    /// Capture area (x, y, width, height)
    capture_area: Option<(i32, i32, u32, u32)>,
}

impl MouseCapture {
    /// Create new mouse capture state
    pub fn new() -> Self {
        Self {
            captured: false,
            capture_area: None,
        }
    }

    /// Check if mouse is captured
    pub fn is_captured(&self) -> bool {
        self.captured
    }

    /// Capture mouse
    pub fn capture(&mut self, area: Option<(i32, i32, u32, u32)>) {
        self.captured = true;
        self.capture_area = area;
    }

    /// Release mouse
    pub fn release(&mut self) {
        self.captured = false;
        self.capture_area = None;
    }

    /// Check if position is in capture area
    pub fn contains(&self, x: i32, y: i32) -> bool {
        if !self.captured {
            return false;
        }

        if let Some((cx, cy, cw, ch)) = self.capture_area {
            x >= cx && x < (cx + cw as i32) && y >= cy && y < (cy + ch as i32)
        } else {
            true // Global capture
        }
    }
}

impl Default for MouseCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_state() {
        let mut state = MouseState::new();

        assert_eq!(state.position(), (0, 0));
        assert!(!state.any_button_pressed());

        state.set_position(100, 200);
        assert_eq!(state.position(), (100, 200));
        assert_eq!(state.delta(), (100, 200));

        state.press_button(MouseButton::Left);
        assert!(state.is_button_pressed(MouseButton::Left));
        assert!(state.any_button_pressed());

        state.release_button(MouseButton::Left);
        assert!(!state.is_button_pressed(MouseButton::Left));
        assert!(!state.any_button_pressed());
    }

    #[test]
    fn test_mouse_device() {
        let device = MouseDevice::new(1.0, true);
        assert!(device.is_ok());

        let mut device = device.unwrap();

        device.handle_move(50, 75);
        assert_eq!(device.position(), (50, 75));

        device.handle_button_press(MouseButton::Right);
        assert!(device.is_button_pressed(MouseButton::Right));

        device.handle_button_release(MouseButton::Right);
        assert!(!device.is_button_pressed(MouseButton::Right));
    }

    #[test]
    fn test_mouse_sensitivity() {
        let mut device = MouseDevice::new(2.0, true).unwrap();

        device.handle_raw_move(10.0, 10.0);
        // With 2.0 sensitivity, 10.0 delta becomes 20 pixels
        assert_eq!(device.position(), (20, 20));
    }

    #[test]
    fn test_mouse_capture() {
        let mut capture = MouseCapture::new();

        assert!(!capture.is_captured());

        capture.capture(Some((0, 0, 100, 100)));
        assert!(capture.is_captured());
        assert!(capture.contains(50, 50));
        assert!(!capture.contains(150, 150));

        capture.release();
        assert!(!capture.is_captured());
    }

    #[test]
    fn test_cursor_mode() {
        let mut device = MouseDevice::new(1.0, false).unwrap();

        assert_eq!(device.state().cursor_mode(), CursorMode::Normal);

        device.set_cursor_mode(CursorMode::Hidden).unwrap();
        assert_eq!(device.state().cursor_mode(), CursorMode::Hidden);
    }

    #[test]
    fn test_invalid_sensitivity() {
        let result = MouseDevice::new(0.0, true);
        assert!(result.is_err());

        let result = MouseDevice::new(-1.0, true);
        assert!(result.is_err());
    }
}
