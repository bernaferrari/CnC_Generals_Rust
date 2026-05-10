//! # Input Manager Module
//!
//! Central coordinator for all input operations, combining keyboard, mouse,
//! gamepad, and other input types into a unified system.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use winit::event::{DeviceEvent, ElementState, WindowEvent};
use winit::keyboard::PhysicalKey;

use super::{
    GamepadId, GamepadManager, InputConfig, InputError, InputEvent, InputEventFilter,
    InputEventType, InputStats, KeyBinding, KeyBindingManager, KeyCode, KeyModifiers, Keyboard,
    Mouse, MouseButton,
};
use crate::system::SubsystemInterface;
use game_engine::common::game_engine::get_game_engine;

/// Central input manager that coordinates all input systems
pub struct InputManager {
    /// Keyboard input handler
    keyboard: Keyboard,
    /// Mouse input handler  
    mouse: Mouse,
    /// Gamepad manager
    gamepad_manager: GamepadManager,
    /// Key binding system
    key_bindings: KeyBindingManager,
    /// Event queue for processed input events
    event_queue: VecDeque<InputEvent>,
    /// Input configuration
    config: InputConfig,
    /// Combined input statistics
    stats: InputStats,
    /// Whether the input system is enabled
    enabled: bool,
    /// Window focus state
    window_focused: bool,
    /// Action callbacks
    action_callbacks: HashMap<String, Box<dyn Fn() + Send>>,
}

impl InputManager {
    /// Create a new input manager
    pub fn new() -> Self {
        Self {
            keyboard: Keyboard::new(),
            mouse: Mouse::new(),
            gamepad_manager: GamepadManager::new(),
            key_bindings: KeyBindingManager::default(),
            event_queue: VecDeque::new(),
            config: InputConfig::default(),
            stats: InputStats::default(),
            enabled: true,
            window_focused: true,
            action_callbacks: HashMap::new(),
        }
    }

    /// Create input manager with custom configuration
    pub fn with_config(config: InputConfig) -> Self {
        let mut manager = Self::new();
        manager.set_config(config);
        manager
    }

    /// Set input configuration
    pub fn set_config(&mut self, config: InputConfig) {
        self.config = config.clone();

        // Apply configuration to subsystems
        self.keyboard.set_enabled(config.keyboard_enabled);
        self.mouse.set_enabled(config.mouse_enabled);
        self.gamepad_manager.set_enabled(config.gamepad_enabled);

        self.mouse.set_sensitivity(config.mouse_sensitivity);
        self.mouse
            .set_double_click_config(config.double_click_time, 5.0);
        self.keyboard
            .set_key_repeat(config.key_repeat_delay, config.key_repeat_interval);

        // Ensure event queue doesn't exceed maximum size
        while self.event_queue.len() > config.max_event_queue_size {
            self.event_queue.pop_front();
            self.stats.events_dropped += 1;
        }
    }

    /// Get current input configuration
    pub fn config(&self) -> &InputConfig {
        &self.config
    }

    /// Enable or disable the entire input system
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear_events();
        }
    }

    /// Check if input system is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Process a winit window event
    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        if !self.enabled {
            return;
        }

        let timestamp = Instant::now();

        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if self.config.keyboard_enabled && self.window_focused {
                    if let PhysicalKey::Code(winit_key) = event.physical_key {
                        let key = KeyCode::from(winit_key);
                        let pressed = event.state == ElementState::Pressed;
                        let modifiers = self.keyboard.state().modifiers();

                        // Update keyboard state
                        self.keyboard.handle_key_event(event, timestamp);

                        // Generate input event
                        let input_event = if pressed {
                            InputEvent::KeyPressed {
                                key,
                                modifiers,
                                timestamp,
                            }
                        } else {
                            InputEvent::KeyReleased {
                                key,
                                modifiers,
                                timestamp,
                            }
                        };

                        self.add_event(input_event);

                        // Check for key bindings
                        if pressed {
                            if let Some(action) = self.key_bindings.find_action(key, modifiers) {
                                self.trigger_action(action);
                            }
                        }
                    }
                }
            }

            WindowEvent::Ime(ime_event) => {
                if self.config.keyboard_enabled && self.window_focused {
                    match ime_event {
                        winit::event::Ime::Commit(text) => {
                            self.keyboard.handle_text_input(text);
                            let input_event = InputEvent::TextInput {
                                text: text.clone(),
                                timestamp,
                            };
                            self.add_event(input_event);
                        }
                        _ => {} // Handle other IME events if needed
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.config.mouse_enabled && self.window_focused {
                    let x = position.x as f32;
                    let y = position.y as f32;

                    self.mouse.handle_mouse_move(x, y);
                    let delta = self.mouse.state().delta();

                    let input_event = InputEvent::MouseMoved {
                        x,
                        y,
                        delta_x: delta.dx,
                        delta_y: delta.dy,
                        timestamp,
                    };
                    self.add_event(input_event);
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if self.config.mouse_enabled && self.window_focused {
                    let mouse_button = MouseButton::from(*button);
                    let pressed = *state == ElementState::Pressed;
                    let position = self.mouse.state().position();

                    self.mouse
                        .handle_mouse_button(mouse_button, pressed, timestamp);

                    let input_event = if pressed {
                        let click_count = self.mouse.state().click_count(mouse_button);
                        InputEvent::MouseButtonPressed {
                            button: mouse_button,
                            x: position.0,
                            y: position.1,
                            click_count,
                            timestamp,
                        }
                    } else {
                        InputEvent::MouseButtonReleased {
                            button: mouse_button,
                            x: position.0,
                            y: position.1,
                            timestamp,
                        }
                    };

                    self.add_event(input_event);
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if self.config.mouse_enabled && self.window_focused {
                    self.mouse.handle_scroll(*delta);
                    let scroll_delta = self.mouse.state().scroll_delta();

                    let input_event = InputEvent::MouseWheel {
                        delta_x: scroll_delta.0,
                        delta_y: scroll_delta.1,
                        timestamp,
                    };
                    self.add_event(input_event);
                }
            }

            WindowEvent::CursorEntered { .. } => {
                if self.config.mouse_enabled {
                    let input_event = InputEvent::MouseEntered { timestamp };
                    self.add_event(input_event);
                }
            }

            WindowEvent::CursorLeft { .. } => {
                if self.config.mouse_enabled {
                    let input_event = InputEvent::MouseLeft { timestamp };
                    self.add_event(input_event);
                }
            }

            WindowEvent::Resized(size) => {
                let input_event = InputEvent::WindowResized {
                    width: size.width,
                    height: size.height,
                    timestamp,
                };
                self.add_event(input_event);
            }

            WindowEvent::Focused(focused) => {
                self.window_focused = *focused;

                let input_event = if *focused {
                    InputEvent::WindowFocusGained { timestamp }
                } else {
                    InputEvent::WindowFocusLost { timestamp }
                };
                self.add_event(input_event);

                // Clear input state when losing focus to prevent stuck commands.
                if !focused {
                    self.keyboard.state_mut().reset();
                    self.mouse.state_mut().reset();
                }
            }

            WindowEvent::CloseRequested => {
                let input_event = InputEvent::WindowClosed { timestamp };
                self.add_event(input_event);
                if let Some(engine) = get_game_engine() {
                    engine.lock().set_quitting(true);
                }
            }

            _ => {} // Ignore other window events
        }
    }

    /// Process a winit device event
    pub fn handle_device_event(&mut self, _event: &DeviceEvent) {
        // Device events can be used for raw mouse input, etc.
        // For now, we'll stick with window events for simplicity
    }

    /// Update input systems and process events
    pub fn update(&mut self) {
        if !self.enabled {
            return;
        }

        let timestamp = Instant::now();

        // Update keyboard (handle key repeat)
        let repeated_keys = self.keyboard.update();
        for key in repeated_keys {
            let modifiers = self.keyboard.state().modifiers();
            let input_event = InputEvent::KeyRepeat {
                key,
                modifiers,
                timestamp,
            };
            self.add_event(input_event);

            // Trigger key bindings for repeated keys too
            if let Some(action) = self.key_bindings.find_action(key, modifiers) {
                self.trigger_action(action);
            }
        }

        // Update mouse
        self.mouse.update();

        // Update gamepad manager
        let gamepad_events = self.gamepad_manager.update();
        for (gamepad_id, gamepad_event) in gamepad_events {
            let input_event = match gamepad_event {
                super::gamepad::GamepadEvent::Connected => {
                    if let Some(state) = self.gamepad_manager.gamepad_state(gamepad_id) {
                        InputEvent::GamepadConnected {
                            gamepad_id,
                            name: state.name.clone(),
                            timestamp,
                        }
                    } else {
                        continue;
                    }
                }
                super::gamepad::GamepadEvent::Disconnected => InputEvent::GamepadDisconnected {
                    gamepad_id,
                    timestamp,
                },
                super::gamepad::GamepadEvent::ButtonPressed { button } => {
                    InputEvent::GamepadButtonPressed {
                        gamepad_id,
                        button,
                        timestamp,
                    }
                }
                super::gamepad::GamepadEvent::ButtonReleased { button } => {
                    InputEvent::GamepadButtonReleased {
                        gamepad_id,
                        button,
                        timestamp,
                    }
                }
                super::gamepad::GamepadEvent::AxisChanged { axis, value } => {
                    InputEvent::GamepadAxisChanged {
                        gamepad_id,
                        axis,
                        value,
                        timestamp,
                    }
                }
            };

            self.add_event(input_event);
        }

        // Update statistics
        self.update_stats();
    }

    /// Add an event to the queue
    fn add_event(&mut self, event: InputEvent) {
        // Check if queue is full
        if self.event_queue.len() >= self.config.max_event_queue_size {
            self.event_queue.pop_front();
            self.stats.events_dropped += 1;
        }

        self.event_queue.push_back(event);
        self.stats.events_processed += 1;
    }

    /// Get all pending events
    pub fn poll_events(&mut self) -> Vec<InputEvent> {
        self.event_queue.drain(..).collect()
    }

    /// Get filtered events
    pub fn poll_events_filtered(&mut self, filter: &InputEventFilter) -> Vec<InputEvent> {
        let all_events: Vec<InputEvent> = self.event_queue.drain(..).collect();
        all_events
            .into_iter()
            .filter(|event| filter.passes(event))
            .collect()
    }

    /// Peek at events without consuming them
    pub fn peek_events(&self) -> &VecDeque<InputEvent> {
        &self.event_queue
    }

    /// Clear all pending events
    pub fn clear_events(&mut self) {
        let dropped_count = self.event_queue.len();
        self.event_queue.clear();
        self.stats.events_dropped += dropped_count as u64;
    }

    /// Get keyboard state
    pub fn keyboard(&self) -> &Keyboard {
        &self.keyboard
    }

    /// Get mutable keyboard state
    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        &mut self.keyboard
    }

    /// Get mouse state
    pub fn mouse(&self) -> &Mouse {
        &self.mouse
    }

    /// Get mutable mouse state
    pub fn mouse_mut(&mut self) -> &mut Mouse {
        &mut self.mouse
    }

    /// Get gamepad manager
    pub fn gamepad_manager(&self) -> &GamepadManager {
        &self.gamepad_manager
    }

    /// Get mutable gamepad manager
    pub fn gamepad_manager_mut(&mut self) -> &mut GamepadManager {
        &mut self.gamepad_manager
    }

    /// Get key bindings manager
    pub fn key_bindings(&self) -> &KeyBindingManager {
        &self.key_bindings
    }

    /// Get mutable key bindings manager
    pub fn key_bindings_mut(&mut self) -> &mut KeyBindingManager {
        &mut self.key_bindings
    }

    /// Register an action callback
    pub fn register_action_callback<F>(&mut self, action: &str, callback: F)
    where
        F: Fn() + Send + 'static,
    {
        self.action_callbacks
            .insert(action.to_string(), Box::new(callback));
    }

    /// Trigger an action (called internally when key bindings match)
    fn trigger_action(&self, action: &str) {
        if let Some(callback) = self.action_callbacks.get(action) {
            callback();
        }
    }

    /// Check if a specific key is currently down
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.keyboard.state().is_key_down(key)
    }

    /// Check if a mouse button is currently down
    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.mouse.state().is_button_down(button)
    }

    /// Get current mouse position
    pub fn mouse_position(&self) -> (f32, f32) {
        self.mouse.state().position()
    }

    /// Get gamepad state by ID
    pub fn gamepad_state(&self, gamepad_id: GamepadId) -> Option<&super::gamepad::GamepadState> {
        self.gamepad_manager.gamepad_state(gamepad_id)
    }

    /// Get all connected gamepad IDs
    pub fn connected_gamepads(&self) -> Vec<GamepadId> {
        self.gamepad_manager.connected_gamepads()
    }

    /// Get combined input statistics
    pub fn stats(&self) -> &InputStats {
        &self.stats
    }

    /// Reset all input statistics
    pub fn reset_stats(&mut self) {
        self.stats.reset();
        self.keyboard.reset_stats();
        self.mouse.reset_stats();
        self.gamepad_manager.reset_stats();
    }

    /// Update combined statistics
    fn update_stats(&mut self) {
        let kb_stats = self.keyboard.stats();
        let mouse_stats = self.mouse.stats();
        let gamepad_stats = self.gamepad_manager.stats();

        self.stats.keyboard_events = kb_stats.keyboard_events;
        self.stats.mouse_events = mouse_stats.mouse_events;
        self.stats.gamepad_events = gamepad_stats.gamepad_events;
        self.stats.events_processed = kb_stats.events_processed
            + mouse_stats.events_processed
            + gamepad_stats.events_processed;
    }

    /// Check if window has focus
    pub fn has_window_focus(&self) -> bool {
        self.window_focused
    }

    /// Get event queue size
    pub fn event_queue_size(&self) -> usize {
        self.event_queue.len()
    }
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for InputManager {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing InputManager subsystem");

        // Initialize all subsystems
        self.keyboard.init()?;
        self.mouse.init()?;
        self.gamepad_manager.init()?;

        self.enabled = true;
        self.reset_stats();

        log::info!("InputManager initialization complete");
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting InputManager subsystem");

        // Reset all subsystems
        self.keyboard.reset()?;
        self.mouse.reset()?;
        self.gamepad_manager.reset()?;

        // Clear events and callbacks
        self.clear_events();
        self.action_callbacks.clear();

        self.reset_stats();

        log::info!("InputManager reset complete");
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_input_manager_creation() {
        let manager = InputManager::new();
        assert!(manager.is_enabled());
        assert_eq!(manager.event_queue_size(), 0);
        assert!(manager.has_window_focus());
    }

    #[test]
    fn test_input_manager_with_config() {
        let mut config = InputConfig::default();
        config.keyboard_enabled = false;
        config.mouse_sensitivity = 2.0;

        let manager = InputManager::with_config(config);
        assert!(!manager.keyboard().is_enabled());
        assert_eq!(manager.mouse().state().sensitivity(), 2.0);
    }

    #[test]
    fn test_action_callbacks() {
        let mut manager = InputManager::new();
        let action_triggered = Arc::new(Mutex::new(false));
        let action_triggered_clone = action_triggered.clone();

        manager.register_action_callback("test_action", move || {
            *action_triggered_clone
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = true;
        });

        // Trigger action manually
        manager.trigger_action("test_action");

        assert!(*action_triggered.lock().unwrap_or_else(|e| e.into_inner()));
    }

    #[test]
    fn test_event_queue_management() {
        let mut manager = InputManager::new();

        // Test adding events
        let event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        manager.add_event(event);
        assert_eq!(manager.event_queue_size(), 1);

        // Test polling events
        let events = manager.poll_events();
        assert_eq!(events.len(), 1);
        assert_eq!(manager.event_queue_size(), 0); // Events consumed

        // Test clearing events
        manager.add_event(InputEvent::KeyPressed {
            key: KeyCode::B,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        });
        manager.clear_events();
        assert_eq!(manager.event_queue_size(), 0);
    }

    #[test]
    fn test_focus_loss_clears_held_keyboard_and_mouse_state() {
        let mut manager = InputManager::new();
        let now = Instant::now();

        manager
            .keyboard_mut()
            .state_mut()
            .update_key(KeyCode::LeftCtrl, true, now);
        manager
            .mouse_mut()
            .handle_mouse_button(MouseButton::Left, true, now);

        assert!(manager.is_key_down(KeyCode::LeftCtrl));
        assert!(manager.is_mouse_button_down(MouseButton::Left));

        manager.handle_window_event(&WindowEvent::Focused(false));

        assert!(!manager.has_window_focus());
        assert!(!manager.is_key_down(KeyCode::LeftCtrl));
        assert!(!manager.is_mouse_button_down(MouseButton::Left));
        assert!(manager.keyboard().state().pressed_keys().is_empty());
        assert!(manager.mouse().state().pressed_buttons().is_empty());
    }

    #[test]
    fn test_enable_disable() {
        let mut manager = InputManager::new();

        manager.set_enabled(false);
        assert!(!manager.is_enabled());

        manager.set_enabled(true);
        assert!(manager.is_enabled());
    }

    #[test]
    fn test_stats_tracking() {
        let mut manager = InputManager::new();

        // Initial stats should be clean
        let stats = manager.stats();
        assert_eq!(stats.events_processed, 0);

        // Reset should work
        manager.reset_stats();
        let stats = manager.stats();
        assert!(stats.last_reset.is_some());
    }
}
