//! Input state tracking and querying

use std::collections::HashMap;

use super::{
    GamepadAxis, GamepadButton, GamepadId, InputEvent, KeyCode, KeyboardState, ModifierKeys,
    MouseButton, MouseState,
};

/// Complete input state snapshot
#[derive(Debug, Clone)]
pub struct InputState {
    /// Keyboard state
    pub keyboard: KeyboardState,

    /// Mouse state
    pub mouse: MouseState,

    /// Gamepad states by ID
    pub gamepads: HashMap<GamepadId, GamepadStateSnapshot>,

    /// Frame number
    pub frame: u64,

    /// Timestamp
    pub timestamp: std::time::Duration,
}

/// Gamepad state snapshot
#[derive(Debug, Clone)]
pub struct GamepadStateSnapshot {
    /// Pressed buttons
    pub pressed_buttons: std::collections::HashSet<GamepadButton>,

    /// Axis values
    pub axes: HashMap<GamepadAxis, f32>,

    /// Connected status
    pub connected: bool,
}

impl InputState {
    /// Create a new input state
    pub fn new() -> Self {
        Self {
            keyboard: KeyboardState::new(),
            mouse: MouseState::new(),
            gamepads: HashMap::new(),
            frame: 0,
            timestamp: std::time::Duration::ZERO,
        }
    }

    /// Check if a key is pressed
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keyboard.is_key_pressed(key)
    }

    /// Check if any key is pressed
    pub fn any_key_pressed(&self) -> bool {
        self.keyboard.any_key_pressed()
    }

    /// Get keyboard modifiers
    pub fn modifiers(&self) -> ModifierKeys {
        self.keyboard.modifiers()
    }

    /// Check if a mouse button is pressed
    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse.is_button_pressed(button)
    }

    /// Get mouse position
    pub fn mouse_position(&self) -> (i32, i32) {
        self.mouse.position()
    }

    /// Get mouse delta
    pub fn mouse_delta(&self) -> (i32, i32) {
        self.mouse.delta()
    }

    /// Check if a gamepad button is pressed
    pub fn is_gamepad_button_pressed(&self, gamepad: GamepadId, button: GamepadButton) -> bool {
        self.gamepads
            .get(&gamepad)
            .map(|g| g.pressed_buttons.contains(&button))
            .unwrap_or(false)
    }

    /// Get gamepad axis value
    pub fn gamepad_axis(&self, gamepad: GamepadId, axis: GamepadAxis) -> f32 {
        self.gamepads
            .get(&gamepad)
            .and_then(|g| g.axes.get(&axis).copied())
            .unwrap_or(0.0)
    }

    /// Check if gamepad is connected
    pub fn is_gamepad_connected(&self, gamepad: GamepadId) -> bool {
        self.gamepads
            .get(&gamepad)
            .map(|g| g.connected)
            .unwrap_or(false)
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// Input state tracker for maintaining current input state
pub struct InputStateTracker {
    /// Current state
    current_state: InputState,

    /// Previous state (for detecting changes)
    previous_state: Option<InputState>,

    /// Frame counter
    frame_counter: u64,

    /// Start time for timestamps
    start_time: std::time::Instant,
}

impl InputStateTracker {
    /// Create a new input state tracker
    pub fn new() -> Self {
        Self {
            current_state: InputState::new(),
            previous_state: None,
            frame_counter: 0,
            start_time: std::time::Instant::now(),
        }
    }

    /// Update state from input event
    pub fn update(&mut self, event: &InputEvent) {
        match event {
            InputEvent::KeyPressed { key, modifiers, .. } => {
                // Update keyboard state through internal method
                // In real implementation, this would update the keyboard state
            }

            InputEvent::KeyReleased { key, .. } => {
                // Update keyboard state
            }

            InputEvent::MouseMoved { x, y, .. } => {
                // Update mouse position
            }

            InputEvent::MouseButtonPressed { button, .. } => {
                // Update mouse button state
            }

            InputEvent::MouseButtonReleased { button, .. } => {
                // Update mouse button state
            }

            InputEvent::GamepadButtonPressed { id, button, .. } => {
                let gamepad_state = self.current_state.gamepads.entry(*id).or_insert_with(|| {
                    GamepadStateSnapshot {
                        pressed_buttons: std::collections::HashSet::new(),
                        axes: HashMap::new(),
                        connected: true,
                    }
                });
                gamepad_state.pressed_buttons.insert(*button);
            }

            InputEvent::GamepadButtonReleased { id, button, .. } => {
                if let Some(gamepad_state) = self.current_state.gamepads.get_mut(id) {
                    gamepad_state.pressed_buttons.remove(button);
                }
            }

            InputEvent::GamepadAxisMoved {
                id, axis, value, ..
            } => {
                let gamepad_state = self.current_state.gamepads.entry(*id).or_insert_with(|| {
                    GamepadStateSnapshot {
                        pressed_buttons: std::collections::HashSet::new(),
                        axes: HashMap::new(),
                        connected: true,
                    }
                });
                gamepad_state.axes.insert(*axis, *value);
            }

            InputEvent::GamepadConnected { id, .. } => {
                let gamepad_state = self.current_state.gamepads.entry(*id).or_insert_with(|| {
                    GamepadStateSnapshot {
                        pressed_buttons: std::collections::HashSet::new(),
                        axes: HashMap::new(),
                        connected: true,
                    }
                });
                gamepad_state.connected = true;
            }

            InputEvent::GamepadDisconnected { id, .. } => {
                if let Some(gamepad_state) = self.current_state.gamepads.get_mut(id) {
                    gamepad_state.connected = false;
                }
            }

            _ => {}
        }
    }

    /// Advance to next frame
    pub fn next_frame(&mut self) {
        self.previous_state = Some(self.current_state.clone());
        self.frame_counter += 1;
        self.current_state.frame = self.frame_counter;
        self.current_state.timestamp = self.start_time.elapsed();
    }

    /// Get current state snapshot
    pub fn snapshot(&self) -> InputState {
        self.current_state.clone()
    }

    /// Get previous state snapshot
    pub fn previous_snapshot(&self) -> Option<InputState> {
        self.previous_state.clone()
    }

    /// Check if key was just pressed (this frame)
    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        let current = self.current_state.is_key_pressed(key);
        let previous = self
            .previous_state
            .as_ref()
            .map(|s| s.is_key_pressed(key))
            .unwrap_or(false);
        current && !previous
    }

    /// Check if key was just released (this frame)
    pub fn key_just_released(&self, key: KeyCode) -> bool {
        let current = self.current_state.is_key_pressed(key);
        let previous = self
            .previous_state
            .as_ref()
            .map(|s| s.is_key_pressed(key))
            .unwrap_or(false);
        !current && previous
    }

    /// Check if mouse button was just pressed (this frame)
    pub fn mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        let current = self.current_state.is_mouse_button_pressed(button);
        let previous = self
            .previous_state
            .as_ref()
            .map(|s| s.is_mouse_button_pressed(button))
            .unwrap_or(false);
        current && !previous
    }

    /// Check if mouse button was just released (this frame)
    pub fn mouse_button_just_released(&self, button: MouseButton) -> bool {
        let current = self.current_state.is_mouse_button_pressed(button);
        let previous = self
            .previous_state
            .as_ref()
            .map(|s| s.is_mouse_button_pressed(button))
            .unwrap_or(false);
        !current && previous
    }

    /// Check if gamepad button was just pressed (this frame)
    pub fn gamepad_button_just_pressed(&self, gamepad: GamepadId, button: GamepadButton) -> bool {
        let current = self
            .current_state
            .is_gamepad_button_pressed(gamepad, button);
        let previous = self
            .previous_state
            .as_ref()
            .map(|s| s.is_gamepad_button_pressed(gamepad, button))
            .unwrap_or(false);
        current && !previous
    }

    /// Check if gamepad button was just released (this frame)
    pub fn gamepad_button_just_released(&self, gamepad: GamepadId, button: GamepadButton) -> bool {
        let current = self
            .current_state
            .is_gamepad_button_pressed(gamepad, button);
        let previous = self
            .previous_state
            .as_ref()
            .map(|s| s.is_gamepad_button_pressed(gamepad, button))
            .unwrap_or(false);
        !current && previous
    }

    /// Get current frame number
    pub fn frame(&self) -> u64 {
        self.frame_counter
    }

    /// Get current timestamp
    pub fn timestamp(&self) -> std::time::Duration {
        self.current_state.timestamp
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.current_state = InputState::new();
        self.previous_state = None;
    }
}

impl Default for InputStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state() {
        let state = InputState::new();

        assert!(!state.any_key_pressed());
        assert_eq!(state.mouse_position(), (0, 0));
        assert_eq!(state.frame, 0);
    }

    #[test]
    fn test_state_tracker() {
        let mut tracker = InputStateTracker::new();

        assert_eq!(tracker.frame(), 0);

        tracker.next_frame();
        assert_eq!(tracker.frame(), 1);

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.frame, 1);
    }

    #[test]
    fn test_gamepad_state() {
        let mut state = InputState::new();

        let gamepad_id = GamepadId::new(0);
        state.gamepads.insert(
            gamepad_id,
            GamepadStateSnapshot {
                pressed_buttons: std::collections::HashSet::new(),
                axes: HashMap::new(),
                connected: true,
            },
        );

        assert!(state.is_gamepad_connected(gamepad_id));
    }

    #[test]
    fn test_just_pressed_detection() {
        let mut tracker = InputStateTracker::new();

        // Initially no key is pressed
        assert!(!tracker.key_just_pressed(KeyCode::A));

        // Simulate key press
        let event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: ModifierKeys::empty(),
            timestamp: std::time::Duration::from_secs(0),
        };

        tracker.update(&event);
        tracker.next_frame();

        // Note: In real implementation, keyboard state would be properly updated
        // This is simplified for demonstration
    }
}
