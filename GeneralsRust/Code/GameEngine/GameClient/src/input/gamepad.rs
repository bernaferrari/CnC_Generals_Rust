//! # Gamepad Input Module
//!
//! Gamepad input handling with support for multiple controllers,
//! button mapping, and analog stick processing.

use gilrs::{
    Axis as GilrsAxis, Button as GilrsButton, Event, EventType, Gamepad as GilrsGamepad, Gilrs,
};
use std::collections::HashMap;
use std::time::Instant;

use super::{InputError, InputStats};
use crate::system::SubsystemInterface;

/// Unique identifier for gamepad instances
pub type GamepadId = usize;

/// Gamepad button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    // Face buttons (Xbox naming)
    A,
    B,
    X,
    Y,

    // Shoulder buttons
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,

    // Stick buttons
    LeftStick,
    RightStick,

    // D-pad
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,

    // System buttons
    Start,
    Select,
    Home,

    // Other buttons
    Other(u8),
}

/// Gamepad axis identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
    Other(u8),
}

/// Gamepad state information
#[derive(Debug, Clone)]
pub struct GamepadState {
    /// Gamepad ID
    pub id: GamepadId,
    /// Whether the gamepad is connected
    pub connected: bool,
    /// Button states
    pub buttons: HashMap<GamepadButton, bool>,
    /// Axis values (-1.0 to 1.0)
    pub axes: HashMap<GamepadAxis, f32>,
    /// Gamepad name/model
    pub name: String,
    /// Last update time
    pub last_update: Instant,
}

impl GamepadState {
    pub fn new(id: GamepadId, name: String) -> Self {
        Self {
            id,
            connected: true,
            buttons: HashMap::new(),
            axes: HashMap::new(),
            name,
            last_update: Instant::now(),
        }
    }

    /// Check if a button is pressed
    pub fn is_button_pressed(&self, button: GamepadButton) -> bool {
        self.buttons.get(&button).copied().unwrap_or(false)
    }

    /// Get axis value
    pub fn axis_value(&self, axis: GamepadAxis) -> f32 {
        self.axes.get(&axis).copied().unwrap_or(0.0)
    }

    /// Update button state
    pub fn set_button(&mut self, button: GamepadButton, pressed: bool) {
        self.buttons.insert(button, pressed);
        self.last_update = Instant::now();
    }

    /// Update axis value
    pub fn set_axis(&mut self, axis: GamepadAxis, value: f32) {
        // Apply deadzone
        let deadzone = 0.1;
        let adjusted_value = if value.abs() < deadzone { 0.0 } else { value };

        self.axes.insert(axis, adjusted_value);
        self.last_update = Instant::now();
    }
}

/// Gamepad manager handling multiple controllers
pub struct GamepadManager {
    /// Gilrs context for gamepad handling
    gilrs: Option<Gilrs>,
    /// Connected gamepads
    gamepads: HashMap<GamepadId, GamepadState>,
    /// Input statistics
    stats: InputStats,
    /// Whether gamepad input is enabled
    enabled: bool,
}

impl GamepadManager {
    /// Create a new gamepad manager
    pub fn new() -> Self {
        Self {
            gilrs: None,
            gamepads: HashMap::new(),
            stats: InputStats::default(),
            enabled: true,
        }
    }

    /// Initialize gamepad system
    pub fn initialize(&mut self) -> Result<(), InputError> {
        match Gilrs::new() {
            Ok(gilrs) => {
                log::info!("Gamepad system initialized");

                // Detect already connected gamepads
                for (_id, gamepad) in gilrs.gamepads() {
                    let gamepad_id = _id.into();
                    let name = gamepad.name().to_string();
                    let state = GamepadState::new(gamepad_id, name.clone());
                    self.gamepads.insert(gamepad_id, state);
                    log::info!("Found gamepad: {} (ID: {})", name, gamepad_id);
                }

                self.gilrs = Some(gilrs);
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to initialize gamepad system: {}", e);
                // Don't fail if gamepads aren't available - they're optional
                Ok(())
            }
        }
    }

    /// Enable or disable gamepad input
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.gamepads.clear();
        }
    }

    /// Check if gamepad input is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Update gamepad states and process events
    pub fn update(&mut self) -> Vec<(GamepadId, GamepadEvent)> {
        if !self.enabled || self.gilrs.is_none() {
            return Vec::new();
        }

        let mut events = Vec::new();

        if let Some(ref mut gilrs) = self.gilrs {
            // Process events
            while let Some(Event { id, event, .. }) = gilrs.next_event() {
                let gamepad_id: GamepadId = id.into();

                match event {
                    EventType::Connected => {
                        let gamepad = gilrs.gamepad(id);
                        let name = gamepad.name().to_string();
                        let state = GamepadState::new(gamepad_id, name.clone());
                        self.gamepads.insert(gamepad_id, state);
                        events.push((gamepad_id, GamepadEvent::Connected));
                        log::info!("Gamepad connected: {} (ID: {})", name, gamepad_id);
                    }

                    EventType::Disconnected => {
                        self.gamepads.remove(&gamepad_id);
                        events.push((gamepad_id, GamepadEvent::Disconnected));
                        log::info!("Gamepad disconnected: ID {}", gamepad_id);
                    }

                    EventType::ButtonPressed(button, _) => {
                        if let Some(state) = self.gamepads.get_mut(&gamepad_id) {
                            let game_button = convert_button(button);
                            state.set_button(game_button, true);
                            events.push((
                                gamepad_id,
                                GamepadEvent::ButtonPressed {
                                    button: game_button,
                                },
                            ));
                        }
                    }

                    EventType::ButtonReleased(button, _) => {
                        if let Some(state) = self.gamepads.get_mut(&gamepad_id) {
                            let game_button = convert_button(button);
                            state.set_button(game_button, false);
                            events.push((
                                gamepad_id,
                                GamepadEvent::ButtonReleased {
                                    button: game_button,
                                },
                            ));
                        }
                    }

                    EventType::AxisChanged(axis, value, _) => {
                        if let Some(state) = self.gamepads.get_mut(&gamepad_id) {
                            let game_axis = convert_axis(axis);
                            state.set_axis(game_axis, value);
                            events.push((
                                gamepad_id,
                                GamepadEvent::AxisChanged {
                                    axis: game_axis,
                                    value,
                                },
                            ));
                        }
                    }

                    _ => {} // Ignore other events for now
                }

                self.stats.gamepad_events += 1;
                self.stats.events_processed += 1;
            }
        }

        events
    }

    /// Get gamepad state by ID
    pub fn gamepad_state(&self, id: GamepadId) -> Option<&GamepadState> {
        self.gamepads.get(&id)
    }

    /// Get all connected gamepad IDs
    pub fn connected_gamepads(&self) -> Vec<GamepadId> {
        self.gamepads.keys().copied().collect()
    }

    /// Get number of connected gamepads
    pub fn gamepad_count(&self) -> usize {
        self.gamepads.len()
    }

    /// Get input statistics
    pub fn stats(&self) -> &InputStats {
        &self.stats
    }

    /// Reset input statistics
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }
}

impl Default for GamepadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for GamepadManager {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing GamepadManager subsystem");
        self.initialize().map_err(|e| e.into())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting GamepadManager subsystem");
        self.gamepads.clear();
        self.stats.reset();
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update();
        Ok(())
    }
}

/// Gamepad events
#[derive(Debug, Clone)]
pub enum GamepadEvent {
    Connected,
    Disconnected,
    ButtonPressed { button: GamepadButton },
    ButtonReleased { button: GamepadButton },
    AxisChanged { axis: GamepadAxis, value: f32 },
}

/// Convert gilrs button to our button enum
fn convert_button(button: GilrsButton) -> GamepadButton {
    match button {
        GilrsButton::South => GamepadButton::A,
        GilrsButton::East => GamepadButton::B,
        GilrsButton::West => GamepadButton::X,
        GilrsButton::North => GamepadButton::Y,
        GilrsButton::LeftTrigger => GamepadButton::LeftBumper,
        GilrsButton::RightTrigger => GamepadButton::RightBumper,
        GilrsButton::LeftTrigger2 => GamepadButton::LeftTrigger,
        GilrsButton::RightTrigger2 => GamepadButton::RightTrigger,
        GilrsButton::LeftThumb => GamepadButton::LeftStick,
        GilrsButton::RightThumb => GamepadButton::RightStick,
        GilrsButton::DPadUp => GamepadButton::DPadUp,
        GilrsButton::DPadDown => GamepadButton::DPadDown,
        GilrsButton::DPadLeft => GamepadButton::DPadLeft,
        GilrsButton::DPadRight => GamepadButton::DPadRight,
        GilrsButton::Start => GamepadButton::Start,
        GilrsButton::Select => GamepadButton::Select,
        GilrsButton::Mode => GamepadButton::Home,
        _ => GamepadButton::Other(0),
    }
}

/// Convert gilrs axis to our axis enum
fn convert_axis(axis: GilrsAxis) -> GamepadAxis {
    match axis {
        GilrsAxis::LeftStickX => GamepadAxis::LeftStickX,
        GilrsAxis::LeftStickY => GamepadAxis::LeftStickY,
        GilrsAxis::RightStickX => GamepadAxis::RightStickX,
        GilrsAxis::RightStickY => GamepadAxis::RightStickY,
        GilrsAxis::LeftZ => GamepadAxis::LeftTrigger,
        GilrsAxis::RightZ => GamepadAxis::RightTrigger,
        _ => GamepadAxis::Other(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamepad_state_creation() {
        let state = GamepadState::new(0, "Test Controller".to_string());

        assert_eq!(state.id, 0);
        assert_eq!(state.name, "Test Controller");
        assert!(state.connected);
        assert!(!state.is_button_pressed(GamepadButton::A));
        assert_eq!(state.axis_value(GamepadAxis::LeftStickX), 0.0);
    }

    #[test]
    fn test_gamepad_state_updates() {
        let mut state = GamepadState::new(0, "Test".to_string());

        state.set_button(GamepadButton::A, true);
        assert!(state.is_button_pressed(GamepadButton::A));

        state.set_axis(GamepadAxis::LeftStickX, 0.5);
        assert_eq!(state.axis_value(GamepadAxis::LeftStickX), 0.5);

        // Test deadzone
        state.set_axis(GamepadAxis::LeftStickY, 0.05); // Below deadzone
        assert_eq!(state.axis_value(GamepadAxis::LeftStickY), 0.0);
    }

    #[test]
    fn test_gamepad_manager_creation() {
        let manager = GamepadManager::new();
        assert!(manager.is_enabled());
        assert_eq!(manager.gamepad_count(), 0);
    }

    #[test]
    fn test_gamepad_manager_enable_disable() {
        let mut manager = GamepadManager::new();

        manager.set_enabled(false);
        assert!(!manager.is_enabled());

        manager.set_enabled(true);
        assert!(manager.is_enabled());
    }

    #[test]
    fn test_button_conversion() {
        assert_eq!(convert_button(GilrsButton::South), GamepadButton::A);
        assert_eq!(convert_button(GilrsButton::East), GamepadButton::B);
        assert_eq!(convert_button(GilrsButton::West), GamepadButton::X);
        assert_eq!(convert_button(GilrsButton::North), GamepadButton::Y);
    }

    #[test]
    fn test_axis_conversion() {
        assert_eq!(convert_axis(GilrsAxis::LeftStickX), GamepadAxis::LeftStickX);
        assert_eq!(
            convert_axis(GilrsAxis::RightStickY),
            GamepadAxis::RightStickY
        );
    }
}
