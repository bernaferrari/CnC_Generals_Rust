//! # Input Events Module
//!
//! Unified input event system that combines keyboard, mouse, gamepad,
//! and other input types into a single event stream.

use super::{
    GamepadAxis, GamepadButton, GamepadId, KeyCode, KeyModifiers, MouseButton, TouchInput,
};
use std::time::Instant;

/// Input event types for unified input handling
#[derive(Debug, Clone)]
pub enum InputEvent {
    // Keyboard events
    KeyPressed {
        key: KeyCode,
        modifiers: KeyModifiers,
        timestamp: Instant,
    },
    KeyReleased {
        key: KeyCode,
        modifiers: KeyModifiers,
        timestamp: Instant,
    },
    KeyRepeat {
        key: KeyCode,
        modifiers: KeyModifiers,
        timestamp: Instant,
    },
    TextInput {
        text: String,
        timestamp: Instant,
    },

    // Mouse events
    MouseMoved {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        timestamp: Instant,
    },
    MouseButtonPressed {
        button: MouseButton,
        x: f32,
        y: f32,
        click_count: u32,
        timestamp: Instant,
    },
    MouseButtonReleased {
        button: MouseButton,
        x: f32,
        y: f32,
        timestamp: Instant,
    },
    MouseWheel {
        delta_x: f32,
        delta_y: f32,
        timestamp: Instant,
    },
    MouseEntered {
        timestamp: Instant,
    },
    MouseLeft {
        timestamp: Instant,
    },

    // Gamepad events
    GamepadConnected {
        gamepad_id: GamepadId,
        name: String,
        timestamp: Instant,
    },
    GamepadDisconnected {
        gamepad_id: GamepadId,
        timestamp: Instant,
    },
    GamepadButtonPressed {
        gamepad_id: GamepadId,
        button: GamepadButton,
        timestamp: Instant,
    },
    GamepadButtonReleased {
        gamepad_id: GamepadId,
        button: GamepadButton,
        timestamp: Instant,
    },
    GamepadAxisChanged {
        gamepad_id: GamepadId,
        axis: GamepadAxis,
        value: f32,
        timestamp: Instant,
    },

    // Touch events (mobile platforms)
    TouchStart {
        touch: TouchInput,
        timestamp: Instant,
    },
    TouchMove {
        touch: TouchInput,
        timestamp: Instant,
    },
    TouchEnd {
        touch: TouchInput,
        timestamp: Instant,
    },
    TouchCancel {
        touch: TouchInput,
        timestamp: Instant,
    },

    // Window/system events
    WindowResized {
        width: u32,
        height: u32,
        timestamp: Instant,
    },
    WindowFocusGained {
        timestamp: Instant,
    },
    WindowFocusLost {
        timestamp: Instant,
    },
    WindowClosed {
        timestamp: Instant,
    },

    // Custom/application events
    Custom {
        event_type: String,
        data: Vec<u8>,
        timestamp: Instant,
    },
}

impl InputEvent {
    /// Get the timestamp of this event
    pub fn timestamp(&self) -> Instant {
        match self {
            InputEvent::KeyPressed { timestamp, .. }
            | InputEvent::KeyReleased { timestamp, .. }
            | InputEvent::KeyRepeat { timestamp, .. }
            | InputEvent::TextInput { timestamp, .. }
            | InputEvent::MouseMoved { timestamp, .. }
            | InputEvent::MouseButtonPressed { timestamp, .. }
            | InputEvent::MouseButtonReleased { timestamp, .. }
            | InputEvent::MouseWheel { timestamp, .. }
            | InputEvent::MouseEntered { timestamp, .. }
            | InputEvent::MouseLeft { timestamp, .. }
            | InputEvent::GamepadConnected { timestamp, .. }
            | InputEvent::GamepadDisconnected { timestamp, .. }
            | InputEvent::GamepadButtonPressed { timestamp, .. }
            | InputEvent::GamepadButtonReleased { timestamp, .. }
            | InputEvent::GamepadAxisChanged { timestamp, .. }
            | InputEvent::TouchStart { timestamp, .. }
            | InputEvent::TouchMove { timestamp, .. }
            | InputEvent::TouchEnd { timestamp, .. }
            | InputEvent::TouchCancel { timestamp, .. }
            | InputEvent::WindowResized { timestamp, .. }
            | InputEvent::WindowFocusGained { timestamp, .. }
            | InputEvent::WindowFocusLost { timestamp, .. }
            | InputEvent::WindowClosed { timestamp, .. }
            | InputEvent::Custom { timestamp, .. } => *timestamp,
        }
    }

    /// Get the event type as a string
    pub fn event_type(&self) -> InputEventType {
        match self {
            InputEvent::KeyPressed { .. } => InputEventType::KeyPressed,
            InputEvent::KeyReleased { .. } => InputEventType::KeyReleased,
            InputEvent::KeyRepeat { .. } => InputEventType::KeyRepeat,
            InputEvent::TextInput { .. } => InputEventType::TextInput,
            InputEvent::MouseMoved { .. } => InputEventType::MouseMoved,
            InputEvent::MouseButtonPressed { .. } => InputEventType::MouseButtonPressed,
            InputEvent::MouseButtonReleased { .. } => InputEventType::MouseButtonReleased,
            InputEvent::MouseWheel { .. } => InputEventType::MouseWheel,
            InputEvent::MouseEntered { .. } => InputEventType::MouseEntered,
            InputEvent::MouseLeft { .. } => InputEventType::MouseLeft,
            InputEvent::GamepadConnected { .. } => InputEventType::GamepadConnected,
            InputEvent::GamepadDisconnected { .. } => InputEventType::GamepadDisconnected,
            InputEvent::GamepadButtonPressed { .. } => InputEventType::GamepadButtonPressed,
            InputEvent::GamepadButtonReleased { .. } => InputEventType::GamepadButtonReleased,
            InputEvent::GamepadAxisChanged { .. } => InputEventType::GamepadAxisChanged,
            InputEvent::TouchStart { .. } => InputEventType::TouchStart,
            InputEvent::TouchMove { .. } => InputEventType::TouchMove,
            InputEvent::TouchEnd { .. } => InputEventType::TouchEnd,
            InputEvent::TouchCancel { .. } => InputEventType::TouchCancel,
            InputEvent::WindowResized { .. } => InputEventType::WindowResized,
            InputEvent::WindowFocusGained { .. } => InputEventType::WindowFocusGained,
            InputEvent::WindowFocusLost { .. } => InputEventType::WindowFocusLost,
            InputEvent::WindowClosed { .. } => InputEventType::WindowClosed,
            InputEvent::Custom { .. } => InputEventType::Custom,
        }
    }

    /// Check if this is a keyboard event
    pub fn is_keyboard_event(&self) -> bool {
        matches!(
            self,
            InputEvent::KeyPressed { .. }
                | InputEvent::KeyReleased { .. }
                | InputEvent::KeyRepeat { .. }
                | InputEvent::TextInput { .. }
        )
    }

    /// Check if this is a mouse event
    pub fn is_mouse_event(&self) -> bool {
        matches!(
            self,
            InputEvent::MouseMoved { .. }
                | InputEvent::MouseButtonPressed { .. }
                | InputEvent::MouseButtonReleased { .. }
                | InputEvent::MouseWheel { .. }
                | InputEvent::MouseEntered { .. }
                | InputEvent::MouseLeft { .. }
        )
    }

    /// Check if this is a gamepad event
    pub fn is_gamepad_event(&self) -> bool {
        matches!(
            self,
            InputEvent::GamepadConnected { .. }
                | InputEvent::GamepadDisconnected { .. }
                | InputEvent::GamepadButtonPressed { .. }
                | InputEvent::GamepadButtonReleased { .. }
                | InputEvent::GamepadAxisChanged { .. }
        )
    }

    /// Check if this is a touch event
    pub fn is_touch_event(&self) -> bool {
        matches!(
            self,
            InputEvent::TouchStart { .. }
                | InputEvent::TouchMove { .. }
                | InputEvent::TouchEnd { .. }
                | InputEvent::TouchCancel { .. }
        )
    }

    /// Check if this is a window event
    pub fn is_window_event(&self) -> bool {
        matches!(
            self,
            InputEvent::WindowResized { .. }
                | InputEvent::WindowFocusGained { .. }
                | InputEvent::WindowFocusLost { .. }
                | InputEvent::WindowClosed { .. }
        )
    }
}

/// Enumeration of input event types for filtering and categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputEventType {
    // Keyboard
    KeyPressed,
    KeyReleased,
    KeyRepeat,
    TextInput,

    // Mouse
    MouseMoved,
    MouseButtonPressed,
    MouseButtonReleased,
    MouseWheel,
    MouseEntered,
    MouseLeft,

    // Gamepad
    GamepadConnected,
    GamepadDisconnected,
    GamepadButtonPressed,
    GamepadButtonReleased,
    GamepadAxisChanged,

    // Touch
    TouchStart,
    TouchMove,
    TouchEnd,
    TouchCancel,

    // Window
    WindowResized,
    WindowFocusGained,
    WindowFocusLost,
    WindowClosed,

    // Custom
    Custom,
}

impl std::fmt::Display for InputEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            InputEventType::KeyPressed => "KeyPressed",
            InputEventType::KeyReleased => "KeyReleased",
            InputEventType::KeyRepeat => "KeyRepeat",
            InputEventType::TextInput => "TextInput",
            InputEventType::MouseMoved => "MouseMoved",
            InputEventType::MouseButtonPressed => "MouseButtonPressed",
            InputEventType::MouseButtonReleased => "MouseButtonReleased",
            InputEventType::MouseWheel => "MouseWheel",
            InputEventType::MouseEntered => "MouseEntered",
            InputEventType::MouseLeft => "MouseLeft",
            InputEventType::GamepadConnected => "GamepadConnected",
            InputEventType::GamepadDisconnected => "GamepadDisconnected",
            InputEventType::GamepadButtonPressed => "GamepadButtonPressed",
            InputEventType::GamepadButtonReleased => "GamepadButtonReleased",
            InputEventType::GamepadAxisChanged => "GamepadAxisChanged",
            InputEventType::TouchStart => "TouchStart",
            InputEventType::TouchMove => "TouchMove",
            InputEventType::TouchEnd => "TouchEnd",
            InputEventType::TouchCancel => "TouchCancel",
            InputEventType::WindowResized => "WindowResized",
            InputEventType::WindowFocusGained => "WindowFocusGained",
            InputEventType::WindowFocusLost => "WindowFocusLost",
            InputEventType::WindowClosed => "WindowClosed",
            InputEventType::Custom => "Custom",
        };
        write!(f, "{}", name)
    }
}

/// Event filter for selectively processing input events
#[derive(Debug, Clone)]
pub struct InputEventFilter {
    /// Event types to include (if empty, all types are included)
    include_types: Vec<InputEventType>,
    /// Event types to exclude
    exclude_types: Vec<InputEventType>,
    /// Only include events from specific gamepads
    gamepad_filter: Option<Vec<GamepadId>>,
    /// Time window for event filtering
    time_window: Option<(Instant, Instant)>,
}

impl InputEventFilter {
    /// Create a new empty filter (allows all events)
    pub fn new() -> Self {
        Self {
            include_types: Vec::new(),
            exclude_types: Vec::new(),
            gamepad_filter: None,
            time_window: None,
        }
    }

    /// Include only specific event types
    pub fn include_types(mut self, types: Vec<InputEventType>) -> Self {
        self.include_types = types;
        self
    }

    /// Exclude specific event types
    pub fn exclude_types(mut self, types: Vec<InputEventType>) -> Self {
        self.exclude_types = types;
        self
    }

    /// Filter by specific gamepads
    pub fn gamepad_filter(mut self, gamepad_ids: Vec<GamepadId>) -> Self {
        self.gamepad_filter = Some(gamepad_ids);
        self
    }

    /// Filter by time window
    pub fn time_window(mut self, start: Instant, end: Instant) -> Self {
        self.time_window = Some((start, end));
        self
    }

    /// Check if an event passes this filter
    pub fn passes(&self, event: &InputEvent) -> bool {
        let event_type = event.event_type();

        // Check exclude list first
        if self.exclude_types.contains(&event_type) {
            return false;
        }

        // Check include list (if not empty)
        if !self.include_types.is_empty() && !self.include_types.contains(&event_type) {
            return false;
        }

        // Check gamepad filter
        if let Some(ref gamepad_filter) = self.gamepad_filter {
            match event {
                InputEvent::GamepadConnected { gamepad_id, .. }
                | InputEvent::GamepadDisconnected { gamepad_id, .. }
                | InputEvent::GamepadButtonPressed { gamepad_id, .. }
                | InputEvent::GamepadButtonReleased { gamepad_id, .. }
                | InputEvent::GamepadAxisChanged { gamepad_id, .. }
                    if !gamepad_filter.contains(gamepad_id) =>
                {
                    return false;
                }
                _ => {} // Non-gamepad events pass gamepad filter
            }
        }

        // Check time window
        if let Some((start, end)) = self.time_window {
            let timestamp = event.timestamp();
            if timestamp < start || timestamp > end {
                return false;
            }
        }

        true
    }
}

impl Default for InputEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for creating common event filters
impl InputEventFilter {
    /// Filter for keyboard events only
    pub fn keyboard_only() -> Self {
        Self::new().include_types(vec![
            InputEventType::KeyPressed,
            InputEventType::KeyReleased,
            InputEventType::KeyRepeat,
            InputEventType::TextInput,
        ])
    }

    /// Filter for mouse events only
    pub fn mouse_only() -> Self {
        Self::new().include_types(vec![
            InputEventType::MouseMoved,
            InputEventType::MouseButtonPressed,
            InputEventType::MouseButtonReleased,
            InputEventType::MouseWheel,
            InputEventType::MouseEntered,
            InputEventType::MouseLeft,
        ])
    }

    /// Filter for gamepad events only
    pub fn gamepad_only() -> Self {
        Self::new().include_types(vec![
            InputEventType::GamepadConnected,
            InputEventType::GamepadDisconnected,
            InputEventType::GamepadButtonPressed,
            InputEventType::GamepadButtonReleased,
            InputEventType::GamepadAxisChanged,
        ])
    }

    /// Filter excluding mouse movement (to reduce event noise)
    pub fn no_mouse_move() -> Self {
        Self::new().exclude_types(vec![InputEventType::MouseMoved])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_categorization() {
        let key_event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        assert!(key_event.is_keyboard_event());
        assert!(!key_event.is_mouse_event());
        assert!(!key_event.is_gamepad_event());
        assert!(!key_event.is_touch_event());
        assert!(!key_event.is_window_event());

        let mouse_event = InputEvent::MouseButtonPressed {
            button: MouseButton::Left,
            x: 10.0,
            y: 20.0,
            click_count: 1,
            timestamp: Instant::now(),
        };

        assert!(!mouse_event.is_keyboard_event());
        assert!(mouse_event.is_mouse_event());
        assert!(!mouse_event.is_gamepad_event());
    }

    #[test]
    fn test_event_filter_include() {
        let filter = InputEventFilter::keyboard_only();

        let key_event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let mouse_event = InputEvent::MouseMoved {
            x: 10.0,
            y: 20.0,
            delta_x: 1.0,
            delta_y: 2.0,
            timestamp: Instant::now(),
        };

        assert!(filter.passes(&key_event));
        assert!(!filter.passes(&mouse_event));
    }

    #[test]
    fn test_event_filter_exclude() {
        let filter = InputEventFilter::no_mouse_move();

        let mouse_move = InputEvent::MouseMoved {
            x: 10.0,
            y: 20.0,
            delta_x: 1.0,
            delta_y: 2.0,
            timestamp: Instant::now(),
        };

        let mouse_click = InputEvent::MouseButtonPressed {
            button: MouseButton::Left,
            x: 10.0,
            y: 20.0,
            click_count: 1,
            timestamp: Instant::now(),
        };

        assert!(!filter.passes(&mouse_move));
        assert!(filter.passes(&mouse_click));
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(InputEventType::KeyPressed.to_string(), "KeyPressed");
        assert_eq!(InputEventType::MouseMoved.to_string(), "MouseMoved");
        assert_eq!(
            InputEventType::GamepadConnected.to_string(),
            "GamepadConnected"
        );
    }

    #[test]
    fn test_gamepad_filter() {
        let filter = InputEventFilter::new().gamepad_filter(vec![0, 1]);

        let gamepad_0_event = InputEvent::GamepadButtonPressed {
            gamepad_id: 0,
            button: GamepadButton::A,
            timestamp: Instant::now(),
        };

        let gamepad_2_event = InputEvent::GamepadButtonPressed {
            gamepad_id: 2,
            button: GamepadButton::A,
            timestamp: Instant::now(),
        };

        let key_event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        assert!(filter.passes(&gamepad_0_event));
        assert!(!filter.passes(&gamepad_2_event));
        assert!(filter.passes(&key_event)); // Non-gamepad events pass
    }
}
