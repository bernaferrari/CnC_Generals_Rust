//! # Input Module
//!
//! Comprehensive input system providing keyboard, mouse, and gamepad support
//! with modern async patterns and cross-platform compatibility.
//!
//! ## Features
//!
//! - Cross-platform input handling using winit for window events
//! - Gamepad support via gilrs crate
//! - Key mapping and binding system
//! - Input event filtering and processing
//! - State tracking for all input devices
//! - Configurable key repeat and mouse sensitivity
//! - Touch input support (mobile platforms)
//!
//! ## Architecture
//!
//! The input system is built around several main components:
//! - [`InputManager`] - Central coordinator for all input operations
//! - [`Keyboard`] - Keyboard state and event handling
//! - [`Mouse`] - Mouse state, movement, and button tracking
//! - [`GamepadManager`] - Gamepad detection and input processing
//! - [`InputEvent`] - Unified event system for all input types
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use game_client_rust::input::{InputManager, InputEvent, KeyCode, MouseButton};
//!
//! let mut input_manager = InputManager::new();
//! input_manager.init().unwrap();
//!
//! // Process events in game loop
//! let events = input_manager.poll_events();
//! for event in events {
//!     match event {
//!         InputEvent::KeyPressed { key: KeyCode::Escape, .. } => {
//!             println!("Escape pressed - exiting game");
//!         }
//!         InputEvent::MouseButtonPressed { button: MouseButton::Left, x, y } => {
//!             println!("Left click at ({}, {})", x, y);
//!         }
//!         InputEvent::GamepadButtonPressed { gamepad_id, button } => {
//!             println!("Gamepad {} button {:?} pressed", gamepad_id, button);
//!         }
//!         _ => {}
//!     }
//! }
//! ```

pub mod events;
pub mod gamepad;
pub mod keyboard;
pub mod manager;
pub mod mouse;

use bitflags::bitflags;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::system::SubsystemInterface;

// Re-export main types for convenience
pub use events::{InputEvent, InputEventFilter, InputEventType};
pub use gamepad::{GamepadAxis, GamepadButton, GamepadId, GamepadManager, GamepadState};
pub use keyboard::{KeyCode, KeyState, Keyboard, KeyboardState};
pub use manager::InputManager;
pub use mouse::{Mouse, MouseButton, MouseDelta, MouseState};

/// Input system errors
#[derive(Error, Debug)]
pub enum InputError {
    #[error("Input system initialization failed: {0}")]
    InitializationError(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Invalid key mapping: {0}")]
    InvalidMapping(String),

    #[error("Input processing error: {0}")]
    ProcessingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Input device types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Gamepad(GamepadId),
    Touch,
}

/// Input device capabilities
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InputCapabilities: u32 {
        const KEYBOARD = 0b00000001;
        const MOUSE = 0b00000010;
        const GAMEPAD = 0b00000100;
        const TOUCH = 0b00001000;
        const FORCE_FEEDBACK = 0b00010000;
        const ACCELEROMETER = 0b00100000;
        const GYROSCOPE = 0b01000000;
    }
}

/// Input configuration settings
#[derive(Debug, Clone)]
pub struct InputConfig {
    /// Enable keyboard input
    pub keyboard_enabled: bool,

    /// Enable mouse input
    pub mouse_enabled: bool,

    /// Enable gamepad input
    pub gamepad_enabled: bool,

    /// Mouse sensitivity multiplier
    pub mouse_sensitivity: f32,

    /// Key repeat delay in milliseconds
    pub key_repeat_delay: u32,

    /// Key repeat interval in milliseconds
    pub key_repeat_interval: u32,

    /// Mouse double-click time in milliseconds
    pub double_click_time: u32,

    /// Maximum number of events to queue
    pub max_event_queue_size: usize,

    /// Enable touch input (mobile platforms)
    pub touch_enabled: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            keyboard_enabled: true,
            mouse_enabled: true,
            gamepad_enabled: true,
            mouse_sensitivity: 1.0,
            key_repeat_delay: 500,
            key_repeat_interval: 30,
            double_click_time: 500,
            max_event_queue_size: 1000,
            touch_enabled: false,
        }
    }
}

/// Key binding system for mapping keys to actions
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// Primary key for this action
    pub primary_key: KeyCode,

    /// Alternative key for this action
    pub secondary_key: Option<KeyCode>,

    /// Required modifiers (Ctrl, Alt, Shift)
    pub modifiers: KeyModifiers,

    /// Action identifier
    pub action: String,

    /// Whether this binding is enabled
    pub enabled: bool,
}

impl KeyBinding {
    pub fn new(action: impl Into<String>, primary_key: KeyCode) -> Self {
        Self {
            primary_key,
            secondary_key: None,
            modifiers: KeyModifiers::empty(),
            action: action.into(),
            enabled: true,
        }
    }

    pub fn with_secondary(mut self, key: KeyCode) -> Self {
        self.secondary_key = Some(key);
        self
    }

    pub fn with_modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Check if the given key and modifiers match this binding
    pub fn matches(&self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        if !self.enabled {
            return false;
        }

        let key_matches =
            key == self.primary_key || self.secondary_key.map_or(false, |sec| key == sec);

        key_matches && modifiers == self.modifiers
    }
}

/// Key modifier flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b00000001;
        const CTRL = 0b00000010;
        const ALT = 0b00000100;
        const META = 0b00001000; // Windows/Command key
    }
}

/// Key bindings manager
#[derive(Debug)]
pub struct KeyBindingManager {
    bindings: HashMap<String, KeyBinding>,
    enabled: bool,
}

impl KeyBindingManager {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            enabled: true,
        }
    }

    /// Add a key binding
    pub fn add_binding(&mut self, binding: KeyBinding) {
        self.bindings.insert(binding.action.clone(), binding);
    }

    /// Remove a key binding
    pub fn remove_binding(&mut self, action: &str) -> Option<KeyBinding> {
        self.bindings.remove(action)
    }

    /// Find action for the given key and modifiers
    pub fn find_action(&self, key: KeyCode, modifiers: KeyModifiers) -> Option<&str> {
        if !self.enabled {
            return None;
        }

        self.bindings
            .values()
            .find(|binding| binding.matches(key, modifiers))
            .map(|binding| binding.action.as_str())
    }

    /// Get all bindings
    pub fn bindings(&self) -> &HashMap<String, KeyBinding> {
        &self.bindings
    }

    /// Enable or disable all bindings
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Load default game key bindings
    pub fn load_default_bindings(&mut self) {
        // Movement and camera controls
        self.add_binding(KeyBinding::new("move_forward", KeyCode::W));
        self.add_binding(KeyBinding::new("move_backward", KeyCode::S));
        self.add_binding(KeyBinding::new("move_left", KeyCode::A));
        self.add_binding(KeyBinding::new("move_right", KeyCode::D));
        self.add_binding(KeyBinding::new("rotate_left", KeyCode::Q));
        self.add_binding(KeyBinding::new("rotate_right", KeyCode::E));

        // Game controls
        self.add_binding(
            KeyBinding::new("select_all", KeyCode::A).with_modifiers(KeyModifiers::CTRL),
        );
        self.add_binding(
            KeyBinding::new("deselect_all", KeyCode::D).with_modifiers(KeyModifiers::CTRL),
        );
        self.add_binding(
            KeyBinding::new("attack_move", KeyCode::A).with_modifiers(KeyModifiers::ALT),
        );
        self.add_binding(
            KeyBinding::new("force_fire", KeyCode::F).with_modifiers(KeyModifiers::CTRL),
        );
        self.add_binding(KeyBinding::new("stop", KeyCode::S).with_modifiers(KeyModifiers::ALT));

        // Interface controls
        self.add_binding(KeyBinding::new("pause_game", KeyCode::Pause));
        self.add_binding(KeyBinding::new("toggle_menu", KeyCode::Escape));
        self.add_binding(KeyBinding::new("toggle_chat", KeyCode::Enter));
        self.add_binding(KeyBinding::new("screenshot", KeyCode::F12));

        // Quick save/load
        self.add_binding(KeyBinding::new("quick_save", KeyCode::F5));
        self.add_binding(KeyBinding::new("quick_load", KeyCode::F9));

        // Group selection
        for i in 1..=10 {
            let key = match i {
                1 => KeyCode::Num1,
                2 => KeyCode::Num2,
                3 => KeyCode::Num3,
                4 => KeyCode::Num4,
                5 => KeyCode::Num5,
                6 => KeyCode::Num6,
                7 => KeyCode::Num7,
                8 => KeyCode::Num8,
                9 => KeyCode::Num9,
                10 => KeyCode::Num0,
                _ => continue,
            };

            // Create group
            self.add_binding(
                KeyBinding::new(format!("create_group_{}", i), key)
                    .with_modifiers(KeyModifiers::CTRL),
            );

            // Select group
            self.add_binding(KeyBinding::new(format!("select_group_{}", i), key));

            // Add to group
            self.add_binding(
                KeyBinding::new(format!("add_to_group_{}", i), key)
                    .with_modifiers(KeyModifiers::SHIFT),
            );
        }
    }
}

impl Default for KeyBindingManager {
    fn default() -> Self {
        let mut manager = Self::new();
        manager.load_default_bindings();
        manager
    }
}

/// Touch input data (for mobile platforms)
#[derive(Debug, Clone, Copy)]
pub struct TouchInput {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
    pub phase: TouchPhase,
}

/// Touch input phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Input statistics for debugging and optimization
#[derive(Debug, Default)]
pub struct InputStats {
    pub events_processed: u64,
    pub keyboard_events: u64,
    pub mouse_events: u64,
    pub gamepad_events: u64,
    pub touch_events: u64,
    pub events_dropped: u64,
    pub last_reset: Option<Instant>,
}

impl InputStats {
    pub fn reset(&mut self) {
        self.events_processed = 0;
        self.keyboard_events = 0;
        self.mouse_events = 0;
        self.gamepad_events = 0;
        self.touch_events = 0;
        self.events_dropped = 0;
        self.last_reset = Some(Instant::now());
    }

    pub fn events_per_second(&self) -> f64 {
        if let Some(last_reset) = self.last_reset {
            let elapsed = last_reset.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                return self.events_processed as f64 / elapsed;
            }
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_binding_creation() {
        let binding = KeyBinding::new("test_action", KeyCode::Space)
            .with_secondary(KeyCode::Enter)
            .with_modifiers(KeyModifiers::CTRL);

        assert_eq!(binding.action, "test_action");
        assert_eq!(binding.primary_key, KeyCode::Space);
        assert_eq!(binding.secondary_key, Some(KeyCode::Enter));
        assert_eq!(binding.modifiers, KeyModifiers::CTRL);
        assert!(binding.enabled);
    }

    #[test]
    fn test_key_binding_matching() {
        let binding = KeyBinding::new("test", KeyCode::A).with_modifiers(KeyModifiers::CTRL);

        assert!(binding.matches(KeyCode::A, KeyModifiers::CTRL));
        assert!(!binding.matches(KeyCode::A, KeyModifiers::empty()));
        assert!(!binding.matches(KeyCode::B, KeyModifiers::CTRL));

        let binding_with_secondary =
            KeyBinding::new("test2", KeyCode::X).with_secondary(KeyCode::Y);

        assert!(binding_with_secondary.matches(KeyCode::X, KeyModifiers::empty()));
        assert!(binding_with_secondary.matches(KeyCode::Y, KeyModifiers::empty()));
        assert!(!binding_with_secondary.matches(KeyCode::Z, KeyModifiers::empty()));
    }

    #[test]
    fn test_key_binding_manager() {
        let mut manager = KeyBindingManager::new();

        let binding = KeyBinding::new("test_action", KeyCode::Space);
        manager.add_binding(binding);

        assert_eq!(
            manager.find_action(KeyCode::Space, KeyModifiers::empty()),
            Some("test_action")
        );
        assert_eq!(
            manager.find_action(KeyCode::Enter, KeyModifiers::empty()),
            None
        );

        // Test disabled manager
        manager.set_enabled(false);
        assert_eq!(
            manager.find_action(KeyCode::Space, KeyModifiers::empty()),
            None
        );
    }

    #[test]
    fn test_input_config_defaults() {
        let config = InputConfig::default();

        assert!(config.keyboard_enabled);
        assert!(config.mouse_enabled);
        assert!(config.gamepad_enabled);
        assert_eq!(config.mouse_sensitivity, 1.0);
        assert_eq!(config.key_repeat_delay, 500);
        assert_eq!(config.key_repeat_interval, 30);
        assert_eq!(config.double_click_time, 500);
        assert!(!config.touch_enabled);
    }

    #[test]
    fn test_input_stats() {
        let mut stats = InputStats::default();
        stats.reset();

        assert!(stats.last_reset.is_some());
        assert_eq!(stats.events_processed, 0);

        stats.events_processed = 100;
        // Can't test actual EPS without waiting, but ensure the method doesn't panic
        let _eps = stats.events_per_second();
    }
}
