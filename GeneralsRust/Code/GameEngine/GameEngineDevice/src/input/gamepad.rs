//! Gamepad device abstraction with cross-platform support

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{GamepadAxis, InputError, Result};

/// Gamepad identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GamepadId(pub u32);

impl GamepadId {
    /// Create a new gamepad ID
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw ID value
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// Standard gamepad buttons (Xbox/PlayStation layout)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadButton {
    // Face buttons
    South, // A / Cross
    East,  // B / Circle
    West,  // X / Square
    North, // Y / Triangle

    // D-Pad
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,

    // Shoulder buttons
    LeftShoulder,  // LB / L1
    RightShoulder, // RB / R1
    LeftTrigger,   // LT / L2
    RightTrigger,  // RT / R2

    // Stick buttons
    LeftStick,  // L3
    RightStick, // R3

    // Menu buttons
    Start,
    Select,
    Guide, // Xbox button / PS button

    // Extra buttons
    Extra1,
    Extra2,
    Extra3,
    Extra4,
}

impl GamepadButton {
    /// Get the display name for this button
    pub fn name(&self) -> &'static str {
        match self {
            GamepadButton::South => "A / Cross",
            GamepadButton::East => "B / Circle",
            GamepadButton::West => "X / Square",
            GamepadButton::North => "Y / Triangle",
            GamepadButton::DPadUp => "D-Pad Up",
            GamepadButton::DPadDown => "D-Pad Down",
            GamepadButton::DPadLeft => "D-Pad Left",
            GamepadButton::DPadRight => "D-Pad Right",
            GamepadButton::LeftShoulder => "Left Shoulder",
            GamepadButton::RightShoulder => "Right Shoulder",
            GamepadButton::LeftTrigger => "Left Trigger",
            GamepadButton::RightTrigger => "Right Trigger",
            GamepadButton::LeftStick => "Left Stick",
            GamepadButton::RightStick => "Right Stick",
            GamepadButton::Start => "Start",
            GamepadButton::Select => "Select",
            GamepadButton::Guide => "Guide",
            GamepadButton::Extra1 => "Extra 1",
            GamepadButton::Extra2 => "Extra 2",
            GamepadButton::Extra3 => "Extra 3",
            GamepadButton::Extra4 => "Extra 4",
        }
    }
}

/// Gamepad state tracking buttons and axes
#[derive(Debug, Clone)]
pub struct GamepadState {
    /// Gamepad ID
    id: GamepadId,

    /// Gamepad name
    name: String,

    /// Currently pressed buttons
    pressed_buttons: std::collections::HashSet<GamepadButton>,

    /// Axis values (-1.0 to 1.0)
    axes: HashMap<GamepadAxis, f32>,

    /// Dead zone for analog sticks
    dead_zone: f32,

    /// Whether gamepad is connected
    connected: bool,

    /// Vibration state (left motor, right motor)
    vibration: (f32, f32),
}

impl GamepadState {
    /// Create a new gamepad state
    pub fn new(id: GamepadId, name: String, dead_zone: f32) -> Self {
        let mut axes = HashMap::new();
        axes.insert(GamepadAxis::LeftStickX, 0.0);
        axes.insert(GamepadAxis::LeftStickY, 0.0);
        axes.insert(GamepadAxis::RightStickX, 0.0);
        axes.insert(GamepadAxis::RightStickY, 0.0);
        axes.insert(GamepadAxis::LeftTrigger, 0.0);
        axes.insert(GamepadAxis::RightTrigger, 0.0);

        Self {
            id,
            name,
            pressed_buttons: std::collections::HashSet::new(),
            axes,
            dead_zone,
            connected: true,
            vibration: (0.0, 0.0),
        }
    }

    /// Get gamepad ID
    pub fn id(&self) -> GamepadId {
        self.id
    }

    /// Get gamepad name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if a button is currently pressed
    pub fn is_button_pressed(&self, button: GamepadButton) -> bool {
        self.pressed_buttons.contains(&button)
    }

    /// Check if any button is pressed
    pub fn any_button_pressed(&self) -> bool {
        !self.pressed_buttons.is_empty()
    }

    /// Get all currently pressed buttons
    pub fn pressed_buttons(&self) -> impl Iterator<Item = &GamepadButton> {
        self.pressed_buttons.iter()
    }

    /// Get axis value with dead zone applied
    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        let value = self.axes.get(&axis).copied().unwrap_or(0.0);
        self.apply_dead_zone(value)
    }

    /// Get raw axis value without dead zone
    pub fn axis_raw(&self, axis: GamepadAxis) -> f32 {
        self.axes.get(&axis).copied().unwrap_or(0.0)
    }

    /// Get left stick position (x, y)
    pub fn left_stick(&self) -> (f32, f32) {
        (
            self.axis(GamepadAxis::LeftStickX),
            self.axis(GamepadAxis::LeftStickY),
        )
    }

    /// Get right stick position (x, y)
    pub fn right_stick(&self) -> (f32, f32) {
        (
            self.axis(GamepadAxis::RightStickX),
            self.axis(GamepadAxis::RightStickY),
        )
    }

    /// Get trigger values (left, right)
    pub fn triggers(&self) -> (f32, f32) {
        (
            self.axis(GamepadAxis::LeftTrigger),
            self.axis(GamepadAxis::RightTrigger),
        )
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get current vibration state
    pub fn vibration(&self) -> (f32, f32) {
        self.vibration
    }

    /// Apply dead zone to axis value
    fn apply_dead_zone(&self, value: f32) -> f32 {
        if value.abs() < self.dead_zone {
            0.0
        } else {
            // Scale from dead zone to 1.0
            let sign = value.signum();
            let scaled = (value.abs() - self.dead_zone) / (1.0 - self.dead_zone);
            sign * scaled
        }
    }

    /// Press a button
    fn press_button(&mut self, button: GamepadButton) {
        self.pressed_buttons.insert(button);
    }

    /// Release a button
    fn release_button(&mut self, button: GamepadButton) {
        self.pressed_buttons.remove(&button);
    }

    /// Set axis value
    fn set_axis(&mut self, axis: GamepadAxis, value: f32) {
        self.axes.insert(axis, value.clamp(-1.0, 1.0));
    }

    /// Set connected state
    fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    /// Set vibration
    fn set_vibration(&mut self, left: f32, right: f32) {
        self.vibration = (left.clamp(0.0, 1.0), right.clamp(0.0, 1.0));
    }

    /// Clear all state
    fn clear(&mut self) {
        self.pressed_buttons.clear();
        for axis in self.axes.values_mut() {
            *axis = 0.0;
        }
        self.vibration = (0.0, 0.0);
    }
}

/// Gamepad device with rumble support
pub struct GamepadDevice {
    /// Current gamepad state
    state: GamepadState,

    /// Whether rumble is supported
    rumble_supported: bool,

    /// Last update time for tracking connection
    last_update: Option<std::time::Instant>,
}

impl GamepadDevice {
    /// Create a new gamepad device
    pub fn new(id: GamepadId, name: String, dead_zone: f32) -> Result<Self> {
        if !(0.0..=1.0).contains(&dead_zone) {
            return Err(InputError::InvalidConfiguration(
                "Dead zone must be between 0.0 and 1.0".into(),
            ));
        }

        Ok(Self {
            state: GamepadState::new(id, name, dead_zone),
            rumble_supported: false, // Would be detected from platform
            last_update: None,
        })
    }

    /// Get current gamepad state
    pub fn state(&self) -> GamepadState {
        self.state.clone()
    }

    /// Get gamepad ID
    pub fn id(&self) -> GamepadId {
        self.state.id()
    }

    /// Get gamepad name
    pub fn name(&self) -> &str {
        self.state.name()
    }

    /// Handle button press
    pub fn handle_button_press(&mut self, button: GamepadButton) {
        self.state.press_button(button);
        self.last_update = Some(std::time::Instant::now());
    }

    /// Handle button release
    pub fn handle_button_release(&mut self, button: GamepadButton) {
        self.state.release_button(button);
        self.last_update = Some(std::time::Instant::now());
    }

    /// Handle axis movement
    pub fn handle_axis(&mut self, axis: GamepadAxis, value: f32) {
        self.state.set_axis(axis, value);
        self.last_update = Some(std::time::Instant::now());
    }

    /// Set connected state
    pub fn set_connected(&mut self, connected: bool) {
        self.state.set_connected(connected);
    }

    /// Check if button is pressed
    pub fn is_button_pressed(&self, button: GamepadButton) -> bool {
        self.state.is_button_pressed(button)
    }

    /// Get axis value
    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        self.state.axis(axis)
    }

    /// Get left stick position
    pub fn left_stick(&self) -> (f32, f32) {
        self.state.left_stick()
    }

    /// Get right stick position
    pub fn right_stick(&self) -> (f32, f32) {
        self.state.right_stick()
    }

    /// Get trigger values
    pub fn triggers(&self) -> (f32, f32) {
        self.state.triggers()
    }

    /// Set vibration/rumble
    pub fn set_rumble(&mut self, left: f32, right: f32) -> Result<()> {
        if !self.rumble_supported {
            return Err(InputError::PlatformError(
                "Rumble not supported on this device".into(),
            ));
        }

        self.state.set_vibration(left, right);
        // Platform-specific rumble command would be sent here
        Ok(())
    }

    /// Stop rumble
    pub fn stop_rumble(&mut self) -> Result<()> {
        self.state.set_vibration(0.0, 0.0);
        // Platform-specific rumble stop command would be sent here
        Ok(())
    }

    /// Check if rumble is supported
    pub fn is_rumble_supported(&self) -> bool {
        self.rumble_supported
    }

    /// Set dead zone
    pub fn set_dead_zone(&mut self, dead_zone: f32) -> Result<()> {
        if !(0.0..=1.0).contains(&dead_zone) {
            return Err(InputError::InvalidConfiguration(
                "Dead zone must be between 0.0 and 1.0".into(),
            ));
        }
        self.state.dead_zone = dead_zone;
        Ok(())
    }

    /// Get dead zone
    pub fn dead_zone(&self) -> f32 {
        self.state.dead_zone
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.state.is_connected()
    }

    /// Update gamepad state (call once per frame)
    pub fn update(&mut self, _delta_time: Duration) -> Result<()> {
        // Check for timeout (device disconnected)
        if let Some(last_update) = self.last_update {
            if last_update.elapsed() > Duration::from_secs(5) {
                self.state.set_connected(false);
            }
        }

        Ok(())
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.state.clear();
    }
}

/// Gamepad mapping profile for custom button layouts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamepadMapping {
    /// Profile name
    pub name: String,

    /// Button remapping
    pub button_map: HashMap<GamepadButton, GamepadButton>,

    /// Axis remapping
    pub axis_map: HashMap<GamepadAxis, GamepadAxis>,

    /// Invert axis flags
    pub axis_inverted: HashMap<GamepadAxis, bool>,
}

impl GamepadMapping {
    /// Create a default mapping
    pub fn default_mapping() -> Self {
        Self {
            name: "Default".into(),
            button_map: HashMap::new(),
            axis_map: HashMap::new(),
            axis_inverted: HashMap::new(),
        }
    }

    /// Create Xbox controller mapping
    pub fn xbox_mapping() -> Self {
        Self {
            name: "Xbox".into(),
            button_map: HashMap::new(),
            axis_map: HashMap::new(),
            axis_inverted: HashMap::new(),
        }
    }

    /// Create PlayStation controller mapping
    pub fn playstation_mapping() -> Self {
        Self {
            name: "PlayStation".into(),
            button_map: HashMap::new(),
            axis_map: HashMap::new(),
            axis_inverted: HashMap::new(),
        }
    }

    /// Map a button
    pub fn map_button(&mut self, from: GamepadButton, to: GamepadButton) {
        self.button_map.insert(from, to);
    }

    /// Map an axis
    pub fn map_axis(&mut self, from: GamepadAxis, to: GamepadAxis) {
        self.axis_map.insert(from, to);
    }

    /// Set axis inversion
    pub fn set_axis_inverted(&mut self, axis: GamepadAxis, inverted: bool) {
        self.axis_inverted.insert(axis, inverted);
    }

    /// Apply mapping to button
    pub fn apply_button(&self, button: GamepadButton) -> GamepadButton {
        self.button_map.get(&button).copied().unwrap_or(button)
    }

    /// Apply mapping to axis
    pub fn apply_axis(&self, axis: GamepadAxis, value: f32) -> (GamepadAxis, f32) {
        let mapped_axis = self.axis_map.get(&axis).copied().unwrap_or(axis);
        let inverted = self
            .axis_inverted
            .get(&mapped_axis)
            .copied()
            .unwrap_or(false);
        let mapped_value = if inverted { -value } else { value };
        (mapped_axis, mapped_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamepad_state() {
        let mut state = GamepadState::new(GamepadId::new(0), "Test Gamepad".into(), 0.15);

        assert!(!state.any_button_pressed());
        assert!(!state.is_button_pressed(GamepadButton::South));

        state.press_button(GamepadButton::South);
        assert!(state.is_button_pressed(GamepadButton::South));
        assert!(state.any_button_pressed());

        state.release_button(GamepadButton::South);
        assert!(!state.is_button_pressed(GamepadButton::South));
    }

    #[test]
    fn test_gamepad_axes() {
        let mut state = GamepadState::new(GamepadId::new(0), "Test Gamepad".into(), 0.15);

        state.set_axis(GamepadAxis::LeftStickX, 0.5);
        assert_eq!(state.axis_raw(GamepadAxis::LeftStickX), 0.5);

        // Test dead zone
        state.set_axis(GamepadAxis::LeftStickX, 0.1);
        assert_eq!(state.axis(GamepadAxis::LeftStickX), 0.0); // Below dead zone
    }

    #[test]
    fn test_gamepad_device() {
        let device = GamepadDevice::new(GamepadId::new(0), "Test Gamepad".into(), 0.15);
        assert!(device.is_ok());

        let mut device = device.unwrap();
        assert_eq!(device.id(), GamepadId::new(0));
        assert!(device.is_connected());

        device.handle_button_press(GamepadButton::North);
        assert!(device.is_button_pressed(GamepadButton::North));

        device.handle_axis(GamepadAxis::LeftStickX, 0.8);
        assert!(device.axis(GamepadAxis::LeftStickX) > 0.0);
    }

    #[test]
    fn test_dead_zone() {
        let device = GamepadDevice::new(GamepadId::new(0), "Test".into(), 0.2);
        assert!(device.is_ok());

        let mut device = device.unwrap();

        // Below dead zone
        device.handle_axis(GamepadAxis::LeftStickX, 0.15);
        assert_eq!(device.axis(GamepadAxis::LeftStickX), 0.0);

        // Above dead zone
        device.handle_axis(GamepadAxis::LeftStickX, 0.5);
        assert!(device.axis(GamepadAxis::LeftStickX) > 0.0);
    }

    #[test]
    fn test_gamepad_mapping() {
        let mut mapping = GamepadMapping::default_mapping();

        mapping.map_button(GamepadButton::South, GamepadButton::East);
        assert_eq!(
            mapping.apply_button(GamepadButton::South),
            GamepadButton::East
        );

        mapping.set_axis_inverted(GamepadAxis::LeftStickY, true);
        let (axis, value) = mapping.apply_axis(GamepadAxis::LeftStickY, 0.5);
        assert_eq!(axis, GamepadAxis::LeftStickY);
        assert_eq!(value, -0.5);
    }

    #[test]
    fn test_invalid_dead_zone() {
        let result = GamepadDevice::new(GamepadId::new(0), "Test".into(), 1.5);
        assert!(result.is_err());

        let result = GamepadDevice::new(GamepadId::new(0), "Test".into(), -0.1);
        assert!(result.is_err());
    }
}
