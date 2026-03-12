//! Key binding system for remappable actions

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    GamepadAxis, GamepadButton, InputError, InputEvent, KeyCode, ModifierKeys, MouseButton, Result,
};

/// Input source for a binding
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputBinding {
    /// Keyboard key
    Key {
        key: KeyCode,
        modifiers: ModifierKeys,
    },

    /// Mouse button
    MouseButton { button: MouseButton },

    /// Mouse axis (movement)
    MouseAxis { axis: MouseAxis },

    /// Mouse wheel
    MouseWheel { direction: WheelDirection },

    /// Gamepad button
    GamepadButton { button: GamepadButton },

    /// Gamepad axis
    GamepadAxis {
        axis: GamepadAxis,
        threshold: f32, // For converting analog to digital
    },

    /// Combination of multiple bindings (chord)
    Chord { bindings: Vec<InputBinding> },
}

/// Mouse axis for binding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseAxis {
    X,
    Y,
}

/// Mouse wheel direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WheelDirection {
    Up,
    Down,
    Left,
    Right,
}

impl InputBinding {
    /// Create a key binding
    pub fn key(key: KeyCode) -> Self {
        Self::Key {
            key,
            modifiers: ModifierKeys::empty(),
        }
    }

    /// Create a key binding with modifiers
    pub fn key_with_modifiers(key: KeyCode, modifiers: ModifierKeys) -> Self {
        Self::Key { key, modifiers }
    }

    /// Create a mouse button binding
    pub fn mouse_button(button: MouseButton) -> Self {
        Self::MouseButton { button }
    }

    /// Create a gamepad button binding
    pub fn gamepad_button(button: GamepadButton) -> Self {
        Self::GamepadButton { button }
    }

    /// Create a gamepad axis binding
    pub fn gamepad_axis(axis: GamepadAxis, threshold: f32) -> Self {
        Self::GamepadAxis { axis, threshold }
    }

    /// Check if this binding matches an input event
    pub fn matches(&self, event: &InputEvent) -> Option<f32> {
        match (self, event) {
            // Keyboard
            (
                Self::Key { key, modifiers },
                InputEvent::KeyPressed {
                    key: event_key,
                    modifiers: event_mods,
                    ..
                },
            ) => {
                if key == event_key && modifiers.contains(*event_mods) {
                    Some(1.0)
                } else {
                    None
                }
            }

            // Mouse button
            (
                Self::MouseButton { button },
                InputEvent::MouseButtonPressed {
                    button: event_button,
                    ..
                },
            ) => {
                if button == event_button {
                    Some(1.0)
                } else {
                    None
                }
            }

            // Mouse wheel
            (
                Self::MouseWheel { direction },
                InputEvent::MouseWheel {
                    delta_x, delta_y, ..
                },
            ) => match direction {
                WheelDirection::Up if *delta_y > 0.0 => Some(*delta_y),
                WheelDirection::Down if *delta_y < 0.0 => Some(delta_y.abs()),
                WheelDirection::Left if *delta_x < 0.0 => Some(delta_x.abs()),
                WheelDirection::Right if *delta_x > 0.0 => Some(*delta_x),
                _ => None,
            },

            // Gamepad button
            (
                Self::GamepadButton { button },
                InputEvent::GamepadButtonPressed {
                    button: event_button,
                    ..
                },
            ) => {
                if button == event_button {
                    Some(1.0)
                } else {
                    None
                }
            }

            // Gamepad axis
            (
                Self::GamepadAxis { axis, threshold },
                InputEvent::GamepadAxisMoved {
                    axis: event_axis,
                    value,
                    ..
                },
            ) => {
                if axis == event_axis && value.abs() >= *threshold {
                    Some(*value)
                } else {
                    None
                }
            }

            _ => None,
        }
    }

    /// Get display string for this binding
    pub fn display_string(&self) -> String {
        match self {
            Self::Key { key, modifiers } => {
                let mut parts = Vec::new();

                if modifiers.contains(ModifierKeys::CTRL) {
                    parts.push("Ctrl");
                }
                if modifiers.contains(ModifierKeys::SHIFT) {
                    parts.push("Shift");
                }
                if modifiers.contains(ModifierKeys::ALT) {
                    parts.push("Alt");
                }
                if modifiers.contains(ModifierKeys::META) {
                    parts.push("Meta");
                }

                parts.push(key.name());
                parts.join("+")
            }
            Self::MouseButton { button } => button.name().to_string(),
            Self::MouseAxis { axis } => match axis {
                MouseAxis::X => "Mouse X".to_string(),
                MouseAxis::Y => "Mouse Y".to_string(),
            },
            Self::MouseWheel { direction } => match direction {
                WheelDirection::Up => "Mouse Wheel Up".to_string(),
                WheelDirection::Down => "Mouse Wheel Down".to_string(),
                WheelDirection::Left => "Mouse Wheel Left".to_string(),
                WheelDirection::Right => "Mouse Wheel Right".to_string(),
            },
            Self::GamepadButton { button } => button.name().to_string(),
            Self::GamepadAxis { axis, .. } => match axis {
                GamepadAxis::LeftStickX => "Left Stick X".to_string(),
                GamepadAxis::LeftStickY => "Left Stick Y".to_string(),
                GamepadAxis::RightStickX => "Right Stick X".to_string(),
                GamepadAxis::RightStickY => "Right Stick Y".to_string(),
                GamepadAxis::LeftTrigger => "Left Trigger".to_string(),
                GamepadAxis::RightTrigger => "Right Trigger".to_string(),
            },
            Self::Chord { bindings } => {
                let parts: Vec<String> = bindings.iter().map(|b| b.display_string()).collect();
                parts.join(" + ")
            }
        }
    }
}

/// An action that can be bound to input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionBinding {
    /// Action name
    pub name: String,

    /// Primary binding
    pub primary: InputBinding,

    /// Secondary binding (alternative)
    pub secondary: Option<InputBinding>,

    /// Whether this action is enabled
    pub enabled: bool,

    /// Action category (for organization)
    pub category: String,

    /// Human-readable description
    pub description: String,
}

impl ActionBinding {
    /// Create a new action binding
    pub fn new(name: impl Into<String>, primary: InputBinding) -> Self {
        Self {
            name: name.into(),
            primary,
            secondary: None,
            enabled: true,
            category: "General".into(),
            description: String::new(),
        }
    }

    /// Set secondary binding
    pub fn with_secondary(mut self, secondary: InputBinding) -> Self {
        self.secondary = Some(secondary);
        self
    }

    /// Set category
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Check if this action matches an input event
    pub fn matches(&self, event: &InputEvent) -> Option<f32> {
        if !self.enabled {
            return None;
        }

        self.primary
            .matches(event)
            .or_else(|| self.secondary.as_ref().and_then(|b| b.matches(event)))
    }
}

/// Key binding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingConfig {
    /// Configuration name
    pub name: String,

    /// Action bindings
    pub actions: HashMap<String, ActionBinding>,

    /// Configuration version
    pub version: u32,
}

impl BindingConfig {
    /// Create a new binding configuration
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            actions: HashMap::new(),
            version: 1,
        }
    }

    /// Add an action binding
    pub fn add_action(&mut self, action: ActionBinding) {
        self.actions.insert(action.name.clone(), action);
    }

    /// Load configuration from file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)
            .map_err(|e| InputError::InvalidConfiguration(e.to_string()))?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| InputError::InvalidConfiguration(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get default RTS bindings
    pub fn default_rts() -> Self {
        let mut config = Self::new("Default RTS");

        // Movement
        config.add_action(
            ActionBinding::new("move", InputBinding::mouse_button(MouseButton::Right))
                .with_category("Movement")
                .with_description("Move selected units"),
        );

        config.add_action(
            ActionBinding::new("attack_move", InputBinding::key(KeyCode::A))
                .with_category("Movement")
                .with_description("Attack move"),
        );

        config.add_action(
            ActionBinding::new("stop", InputBinding::key(KeyCode::S))
                .with_category("Movement")
                .with_description("Stop units"),
        );

        // Selection
        config.add_action(
            ActionBinding::new("select", InputBinding::mouse_button(MouseButton::Left))
                .with_category("Selection")
                .with_description("Select units"),
        );

        config.add_action(
            ActionBinding::new(
                "select_all",
                InputBinding::key_with_modifiers(KeyCode::A, ModifierKeys::CTRL),
            )
            .with_category("Selection")
            .with_description("Select all units"),
        );

        // Camera
        config.add_action(
            ActionBinding::new("camera_up", InputBinding::key(KeyCode::Up))
                .with_secondary(InputBinding::key(KeyCode::W))
                .with_category("Camera")
                .with_description("Move camera up"),
        );

        config.add_action(
            ActionBinding::new("camera_down", InputBinding::key(KeyCode::Down))
                .with_secondary(InputBinding::key(KeyCode::S))
                .with_category("Camera")
                .with_description("Move camera down"),
        );

        config.add_action(
            ActionBinding::new("camera_left", InputBinding::key(KeyCode::Left))
                .with_secondary(InputBinding::key(KeyCode::A))
                .with_category("Camera")
                .with_description("Move camera left"),
        );

        config.add_action(
            ActionBinding::new("camera_right", InputBinding::key(KeyCode::Right))
                .with_secondary(InputBinding::key(KeyCode::D))
                .with_category("Camera")
                .with_description("Move camera right"),
        );

        config
    }
}

/// Key binding manager
pub struct KeyBindingManager {
    /// Current configuration
    config: BindingConfig,

    /// Binding state for analog inputs
    analog_state: HashMap<String, f32>,
}

impl KeyBindingManager {
    /// Create a new key binding manager
    pub fn new() -> Self {
        Self {
            config: BindingConfig::default_rts(),
            analog_state: HashMap::new(),
        }
    }

    /// Load configuration
    pub fn load_config(&mut self, config: BindingConfig) {
        self.config = config;
    }

    /// Save current configuration
    pub fn save_config(&self) -> BindingConfig {
        self.config.clone()
    }

    /// Bind an action to input
    pub fn bind_action(&mut self, action: String, binding: InputBinding) {
        if let Some(action_binding) = self.config.actions.get_mut(&action) {
            action_binding.primary = binding;
        } else {
            let new_action = ActionBinding::new(action.clone(), binding);
            self.config.actions.insert(action, new_action);
        }
    }

    /// Bind secondary input for action
    pub fn bind_secondary(&mut self, action: &str, binding: InputBinding) {
        if let Some(action_binding) = self.config.actions.get_mut(action) {
            action_binding.secondary = Some(binding);
        }
    }

    /// Unbind an action
    pub fn unbind_action(&mut self, action: &str) {
        self.config.actions.remove(action);
    }

    /// Check if an event triggers any action
    pub fn check_event(&mut self, event: &InputEvent) -> Option<(String, f32)> {
        for (name, action) in &self.config.actions {
            if let Some(value) = action.matches(event) {
                self.analog_state.insert(name.clone(), value);
                return Some((name.clone(), value));
            }
        }
        None
    }

    /// Get current analog value for action
    pub fn get_analog(&self, action: &str) -> f32 {
        self.analog_state.get(action).copied().unwrap_or(0.0)
    }

    /// Get all actions in category
    pub fn get_category_actions(&self, category: &str) -> Vec<&ActionBinding> {
        self.config
            .actions
            .values()
            .filter(|a| a.category == category)
            .collect()
    }

    /// Get all categories
    pub fn get_categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self
            .config
            .actions
            .values()
            .map(|a| a.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        categories
    }

    /// Enable an action
    pub fn enable_action(&mut self, action: &str) {
        if let Some(action_binding) = self.config.actions.get_mut(action) {
            action_binding.enabled = true;
        }
    }

    /// Disable an action
    pub fn disable_action(&mut self, action: &str) {
        if let Some(action_binding) = self.config.actions.get_mut(action) {
            action_binding.enabled = false;
        }
    }

    /// Check if action is enabled
    pub fn is_action_enabled(&self, action: &str) -> bool {
        self.config
            .actions
            .get(action)
            .map(|a| a.enabled)
            .unwrap_or(false)
    }

    /// Get action binding
    pub fn get_action(&self, action: &str) -> Option<&ActionBinding> {
        self.config.actions.get(action)
    }

    /// Clear all analog state
    pub fn clear_analog_state(&mut self) {
        self.analog_state.clear();
    }
}

impl Default for KeyBindingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_binding() {
        let binding = InputBinding::key(KeyCode::W);
        let event = InputEvent::KeyPressed {
            key: KeyCode::W,
            modifiers: ModifierKeys::empty(),
            timestamp: std::time::Duration::from_secs(0),
        };

        assert!(binding.matches(&event).is_some());
    }

    #[test]
    fn test_action_binding() {
        let action = ActionBinding::new("forward", InputBinding::key(KeyCode::W))
            .with_category("Movement")
            .with_description("Move forward");

        assert_eq!(action.name, "forward");
        assert_eq!(action.category, "Movement");
    }

    #[test]
    fn test_binding_config() {
        let mut config = BindingConfig::new("Test");

        let action = ActionBinding::new("test", InputBinding::key(KeyCode::T));
        config.add_action(action);

        assert!(config.actions.contains_key("test"));
    }

    #[test]
    fn test_key_binding_manager() {
        let mut manager = KeyBindingManager::new();

        manager.bind_action("test".into(), InputBinding::key(KeyCode::T));

        let event = InputEvent::KeyPressed {
            key: KeyCode::T,
            modifiers: ModifierKeys::empty(),
            timestamp: std::time::Duration::from_secs(0),
        };

        let result = manager.check_event(&event);
        assert!(result.is_some());

        if let Some((action, value)) = result {
            assert_eq!(action, "test");
            assert_eq!(value, 1.0);
        }
    }

    #[test]
    fn test_default_rts_config() {
        let config = BindingConfig::default_rts();
        assert!(!config.actions.is_empty());
        assert!(config.actions.contains_key("move"));
        assert!(config.actions.contains_key("select"));
    }
}
