//! # Mouse Input Module
//!
//! Comprehensive mouse input handling with position tracking, button states,
//! scroll wheel support, and mouse sensitivity configuration.

use std::time::{Duration, Instant};
use winit::event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta};

use super::{InputError, InputStats};
use crate::system::SubsystemInterface;

/// Mouse button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

impl From<WinitMouseButton> for MouseButton {
    fn from(button: WinitMouseButton) -> Self {
        match button {
            WinitMouseButton::Left => MouseButton::Left,
            WinitMouseButton::Right => MouseButton::Right,
            WinitMouseButton::Middle => MouseButton::Middle,
            WinitMouseButton::Back => MouseButton::Other(4),
            WinitMouseButton::Forward => MouseButton::Other(5),
            WinitMouseButton::Other(id) => MouseButton::Other(id),
        }
    }
}

/// Mouse button state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Button is not pressed
    Released,
    /// Button was just pressed this frame
    JustPressed,
    /// Button is held down
    Pressed,
    /// Button was just released this frame
    JustReleased,
}

impl ButtonState {
    /// Check if the button is currently down
    pub fn is_down(self) -> bool {
        matches!(self, ButtonState::Pressed | ButtonState::JustPressed)
    }

    /// Check if the button is currently up
    pub fn is_up(self) -> bool {
        matches!(self, ButtonState::Released | ButtonState::JustReleased)
    }

    /// Check if the button was just pressed this frame
    pub fn just_pressed(self) -> bool {
        matches!(self, ButtonState::JustPressed)
    }

    /// Check if the button was just released this frame
    pub fn just_released(self) -> bool {
        matches!(self, ButtonState::JustReleased)
    }
}

/// Mouse position and movement delta
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseDelta {
    /// Change in X position
    pub dx: f32,
    /// Change in Y position
    pub dy: f32,
    /// Change in scroll wheel
    pub scroll_x: f32,
    pub scroll_y: f32,
}

impl MouseDelta {
    pub fn new(dx: f32, dy: f32) -> Self {
        Self {
            dx,
            dy,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }

    pub fn with_scroll(dx: f32, dy: f32, scroll_x: f32, scroll_y: f32) -> Self {
        Self {
            dx,
            dy,
            scroll_x,
            scroll_y,
        }
    }

    pub fn zero() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }
}

/// Click tracking for double-click detection
#[derive(Debug, Clone)]
struct ClickInfo {
    /// Time of the click
    timestamp: Instant,
    /// Position of the click
    position: (f32, f32),
    /// Number of consecutive clicks
    count: u32,
}

/// Complete mouse state tracking
#[derive(Debug)]
pub struct MouseState {
    /// Current mouse position (screen coordinates)
    position: (f32, f32),
    /// Previous mouse position
    previous_position: (f32, f32),
    /// Mouse position at start of current drag operation
    drag_start: Option<(f32, f32)>,
    /// Current button states
    button_states: [ButtonState; 8], // Support up to 8 mouse buttons
    /// Click tracking for double-click detection
    click_info: [Option<ClickInfo>; 8],
    /// Current scroll wheel position
    scroll_position: (f32, f32),
    /// Scroll delta for this frame
    scroll_delta: (f32, f32),
    /// Mouse sensitivity multiplier
    sensitivity: f32,
    /// Double-click time threshold
    double_click_time: Duration,
    /// Double-click distance threshold
    double_click_distance: f32,
    /// Whether mouse is captured/locked
    captured: bool,
    /// Whether mouse cursor is visible
    cursor_visible: bool,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            position: (0.0, 0.0),
            previous_position: (0.0, 0.0),
            drag_start: None,
            button_states: [ButtonState::Released; 8],
            click_info: [None, None, None, None, None, None, None, None],
            scroll_position: (0.0, 0.0),
            scroll_delta: (0.0, 0.0),
            sensitivity: 1.0,
            double_click_time: Duration::from_millis(500),
            double_click_distance: 5.0,
            captured: false,
            cursor_visible: true,
        }
    }

    /// Set mouse sensitivity
    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.max(0.1).min(10.0); // Clamp to reasonable range
    }

    /// Get mouse sensitivity
    pub fn sensitivity(&self) -> f32 {
        self.sensitivity
    }

    /// Set double-click configuration
    pub fn set_double_click_config(&mut self, time_ms: u32, distance: f32) {
        self.double_click_time = Duration::from_millis(time_ms as u64);
        self.double_click_distance = distance;
    }

    /// Update mouse position
    pub fn update_position(&mut self, x: f32, y: f32) {
        self.previous_position = self.position;
        self.position = (x, y);
    }

    /// Get current mouse position
    pub fn position(&self) -> (f32, f32) {
        self.position
    }

    /// Get mouse movement delta (scaled by sensitivity)
    pub fn delta(&self) -> MouseDelta {
        let raw_dx = self.position.0 - self.previous_position.0;
        let raw_dy = self.position.1 - self.previous_position.1;

        MouseDelta::with_scroll(
            raw_dx * self.sensitivity,
            raw_dy * self.sensitivity,
            self.scroll_delta.0,
            self.scroll_delta.1,
        )
    }

    /// Get raw mouse movement (not scaled by sensitivity)
    pub fn raw_delta(&self) -> (f32, f32) {
        (
            self.position.0 - self.previous_position.0,
            self.position.1 - self.previous_position.1,
        )
    }

    /// Update button state
    pub fn update_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
        timestamp: Instant,
    ) -> bool {
        let index = self.button_index(button);
        if index >= self.button_states.len() {
            return false; // Unsupported button
        }

        let current_state = self.button_states[index];

        let new_state = match (current_state, pressed) {
            (ButtonState::Released, true) | (ButtonState::JustReleased, true) => {
                // Start drag on left button press
                if button == MouseButton::Left {
                    self.drag_start = Some(self.position);
                }

                // Track click for double-click detection
                self.track_click(index, timestamp);

                ButtonState::JustPressed
            }
            (ButtonState::JustPressed, true) | (ButtonState::Pressed, true) => ButtonState::Pressed,
            (ButtonState::Pressed, false) | (ButtonState::JustPressed, false) => {
                // End drag on left button release
                if button == MouseButton::Left {
                    self.drag_start = None;
                }

                ButtonState::JustReleased
            }
            (ButtonState::Released, false) | (ButtonState::JustReleased, false) => {
                ButtonState::Released
            }
        };

        self.button_states[index] = new_state;
        current_state != new_state // Return true if state changed
    }

    /// Track click for double-click detection
    fn track_click(&mut self, button_index: usize, timestamp: Instant) {
        let current_pos = self.position;

        if let Some(ref mut click) = self.click_info[button_index] {
            let time_diff = timestamp.duration_since(click.timestamp);
            let distance = ((current_pos.0 - click.position.0).powi(2)
                + (current_pos.1 - click.position.1).powi(2))
            .sqrt();

            if time_diff <= self.double_click_time && distance <= self.double_click_distance {
                // This is a multi-click
                click.count += 1;
                click.timestamp = timestamp;
                click.position = current_pos;
            } else {
                // This is a new single click
                click.count = 1;
                click.timestamp = timestamp;
                click.position = current_pos;
            }
        } else {
            // First click
            self.click_info[button_index] = Some(ClickInfo {
                timestamp,
                position: current_pos,
                count: 1,
            });
        }
    }

    /// Get button state
    pub fn button_state(&self, button: MouseButton) -> ButtonState {
        let index = self.button_index(button);
        if index < self.button_states.len() {
            self.button_states[index]
        } else {
            ButtonState::Released
        }
    }

    /// Check if button is down
    pub fn is_button_down(&self, button: MouseButton) -> bool {
        self.button_state(button).is_down()
    }

    /// Check if button was just pressed
    pub fn is_button_just_pressed(&self, button: MouseButton) -> bool {
        self.button_state(button).just_pressed()
    }

    /// Check if button was just released
    pub fn is_button_just_released(&self, button: MouseButton) -> bool {
        self.button_state(button).just_released()
    }

    /// Get click count (for double-click detection)
    pub fn click_count(&self, button: MouseButton) -> u32 {
        let index = self.button_index(button);
        if index < self.click_info.len() {
            self.click_info[index].as_ref().map_or(0, |info| info.count)
        } else {
            0
        }
    }

    /// Check if currently dragging (left button held and moved)
    pub fn is_dragging(&self) -> bool {
        self.drag_start.is_some() && self.is_button_down(MouseButton::Left)
    }

    /// Get drag distance if currently dragging
    pub fn drag_distance(&self) -> Option<(f32, f32)> {
        if let Some(start_pos) = self.drag_start {
            if self.is_dragging() {
                Some((self.position.0 - start_pos.0, self.position.1 - start_pos.1))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Update scroll wheel
    pub fn update_scroll(&mut self, delta_x: f32, delta_y: f32) {
        self.scroll_delta = (delta_x, delta_y);
        self.scroll_position.0 += delta_x;
        self.scroll_position.1 += delta_y;
    }

    /// Get scroll delta for this frame
    pub fn scroll_delta(&self) -> (f32, f32) {
        self.scroll_delta
    }

    /// Get total scroll position
    pub fn scroll_position(&self) -> (f32, f32) {
        self.scroll_position
    }

    /// Set mouse capture state
    pub fn set_captured(&mut self, captured: bool) {
        self.captured = captured;
    }

    /// Check if mouse is captured
    pub fn is_captured(&self) -> bool {
        self.captured
    }

    /// Set cursor visibility
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    /// Check if cursor is visible
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Update state for next frame
    pub fn update_frame(&mut self) {
        // Update button states
        for state in &mut self.button_states {
            match *state {
                ButtonState::JustPressed => *state = ButtonState::Pressed,
                ButtonState::JustReleased => *state = ButtonState::Released,
                _ => {}
            }
        }

        // Clear scroll delta
        self.scroll_delta = (0.0, 0.0);

        // Clear old click info
        let now = Instant::now();
        for click in &mut self.click_info {
            if let Some(ref info) = click {
                if now.duration_since(info.timestamp) > self.double_click_time * 2 {
                    *click = None;
                }
            }
        }
    }

    /// Reset all mouse state
    pub fn reset(&mut self) {
        self.position = (0.0, 0.0);
        self.previous_position = (0.0, 0.0);
        self.drag_start = None;
        self.button_states = [ButtonState::Released; 8];
        self.click_info = [None, None, None, None, None, None, None, None];
        self.scroll_position = (0.0, 0.0);
        self.scroll_delta = (0.0, 0.0);
        self.captured = false;
    }

    /// Convert mouse button to array index
    fn button_index(&self, button: MouseButton) -> usize {
        match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(id) => 3 + (id as usize).min(4), // Support up to 5 additional buttons
        }
    }

    /// Get all currently pressed buttons
    pub fn pressed_buttons(&self) -> Vec<MouseButton> {
        let mut buttons = Vec::new();

        if self.button_states[0].is_down() {
            buttons.push(MouseButton::Left);
        }
        if self.button_states[1].is_down() {
            buttons.push(MouseButton::Right);
        }
        if self.button_states[2].is_down() {
            buttons.push(MouseButton::Middle);
        }

        for i in 3..self.button_states.len() {
            if self.button_states[i].is_down() {
                buttons.push(MouseButton::Other((i - 3) as u16));
            }
        }

        buttons
    }
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

/// Mouse input handler
pub struct Mouse {
    /// Current mouse state
    state: MouseState,
    /// Input statistics
    stats: InputStats,
    /// Whether mouse input is enabled
    enabled: bool,
}

impl Mouse {
    /// Create a new mouse handler
    pub fn new() -> Self {
        Self {
            state: MouseState::new(),
            stats: InputStats::default(),
            enabled: true,
        }
    }

    /// Enable or disable mouse input
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.state.reset();
        }
    }

    /// Check if mouse input is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Handle mouse movement
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) -> bool {
        if !self.enabled {
            return false;
        }

        self.state.update_position(x, y);
        self.stats.mouse_events += 1;
        self.stats.events_processed += 1;
        true
    }

    /// Handle mouse button input
    pub fn handle_mouse_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
        timestamp: Instant,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        let changed = self.state.update_button(button, pressed, timestamp);
        if changed {
            self.stats.mouse_events += 1;
            self.stats.events_processed += 1;
        }
        changed
    }

    /// Handle mouse scroll wheel
    pub fn handle_scroll(&mut self, delta: MouseScrollDelta) -> bool {
        if !self.enabled {
            return false;
        }

        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0), // Scale line delta
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
        };

        self.state.update_scroll(dx, dy);
        self.stats.mouse_events += 1;
        self.stats.events_processed += 1;
        true
    }

    /// Get current mouse state
    pub fn state(&self) -> &MouseState {
        &self.state
    }

    /// Get mutable mouse state
    pub fn state_mut(&mut self) -> &mut MouseState {
        &mut self.state
    }

    /// Update mouse state for current frame
    pub fn update(&mut self) {
        if self.enabled {
            self.state.update_frame();
        }
    }

    /// Get input statistics
    pub fn stats(&self) -> &InputStats {
        &self.stats
    }

    /// Reset input statistics
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }

    /// Set mouse sensitivity
    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.state.set_sensitivity(sensitivity);
    }

    /// Configure double-click settings
    pub fn set_double_click_config(&mut self, time_ms: u32, distance: f32) {
        self.state.set_double_click_config(time_ms, distance);
    }

    /// Set mouse capture
    pub fn set_captured(&mut self, captured: bool) {
        self.state.set_captured(captured);
    }

    /// Set cursor visibility
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.state.set_cursor_visible(visible);
    }
}

impl Default for Mouse {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for Mouse {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing Mouse subsystem");
        self.enabled = true;
        self.stats.reset();
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting Mouse subsystem");
        self.state.reset();
        self.stats.reset();
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_state_transitions() {
        let mut state = MouseState::new();
        let now = Instant::now();

        // Test press
        assert!(state.update_button(MouseButton::Left, true, now));
        assert_eq!(
            state.button_state(MouseButton::Left),
            ButtonState::JustPressed
        );
        assert!(state.is_button_just_pressed(MouseButton::Left));
        assert!(state.is_button_down(MouseButton::Left));

        // Update frame
        state.update_frame();
        assert_eq!(state.button_state(MouseButton::Left), ButtonState::Pressed);
        assert!(!state.is_button_just_pressed(MouseButton::Left));
        assert!(state.is_button_down(MouseButton::Left));

        // Test release
        assert!(state.update_button(MouseButton::Left, false, now));
        assert_eq!(
            state.button_state(MouseButton::Left),
            ButtonState::JustReleased
        );
        assert!(state.is_button_just_released(MouseButton::Left));
        assert!(!state.is_button_down(MouseButton::Left));

        // Update frame
        state.update_frame();
        assert_eq!(state.button_state(MouseButton::Left), ButtonState::Released);
        assert!(!state.is_button_just_released(MouseButton::Left));
        assert!(!state.is_button_down(MouseButton::Left));
    }

    #[test]
    fn test_mouse_movement() {
        let mut state = MouseState::new();

        state.update_position(10.0, 20.0);
        assert_eq!(state.position(), (10.0, 20.0));

        state.update_position(15.0, 25.0);
        assert_eq!(state.position(), (15.0, 25.0));
        assert_eq!(state.raw_delta(), (5.0, 5.0));

        let delta = state.delta();
        assert_eq!(delta.dx, 5.0); // With default sensitivity 1.0
        assert_eq!(delta.dy, 5.0);
    }

    #[test]
    fn test_sensitivity() {
        let mut state = MouseState::new();
        state.set_sensitivity(2.0);

        state.update_position(0.0, 0.0);
        state.update_position(10.0, 10.0);

        let delta = state.delta();
        assert_eq!(delta.dx, 20.0); // 10.0 * 2.0
        assert_eq!(delta.dy, 20.0);
    }

    #[test]
    fn test_dragging() {
        let mut state = MouseState::new();
        let now = Instant::now();

        state.update_position(10.0, 10.0);

        // Start drag
        state.update_button(MouseButton::Left, true, now);
        assert!(state.is_dragging());

        state.update_position(20.0, 30.0);
        if let Some((dx, dy)) = state.drag_distance() {
            assert_eq!(dx, 10.0);
            assert_eq!(dy, 20.0);
        } else {
            panic!("Expected drag distance");
        }

        // End drag
        state.update_button(MouseButton::Left, false, now);
        assert!(!state.is_dragging());
        assert!(state.drag_distance().is_none());
    }

    #[test]
    fn test_scroll_wheel() {
        let mut state = MouseState::new();

        state.update_scroll(1.0, 2.0);
        assert_eq!(state.scroll_delta(), (1.0, 2.0));
        assert_eq!(state.scroll_position(), (1.0, 2.0));

        // Test frame update clears delta
        state.update_frame();
        assert_eq!(state.scroll_delta(), (0.0, 0.0));
        assert_eq!(state.scroll_position(), (1.0, 2.0)); // Position persists
    }

    #[test]
    fn test_mouse_creation() {
        let mouse = Mouse::new();
        assert!(mouse.is_enabled());
        assert_eq!(mouse.state().position(), (0.0, 0.0));
    }

    #[test]
    fn test_mouse_enable_disable() {
        let mut mouse = Mouse::new();

        mouse.set_enabled(false);
        assert!(!mouse.is_enabled());

        // Events should be ignored when disabled
        assert!(!mouse.handle_mouse_move(10.0, 20.0));
        assert_eq!(mouse.state().position(), (0.0, 0.0));

        mouse.set_enabled(true);
        assert!(mouse.is_enabled());

        assert!(mouse.handle_mouse_move(10.0, 20.0));
        assert_eq!(mouse.state().position(), (10.0, 20.0));
    }

    #[test]
    fn test_button_conversion() {
        assert_eq!(MouseButton::from(WinitMouseButton::Left), MouseButton::Left);
        assert_eq!(
            MouseButton::from(WinitMouseButton::Right),
            MouseButton::Right
        );
        assert_eq!(
            MouseButton::from(WinitMouseButton::Middle),
            MouseButton::Middle
        );
        assert_eq!(
            MouseButton::from(WinitMouseButton::Other(5)),
            MouseButton::Other(5)
        );
    }
}
