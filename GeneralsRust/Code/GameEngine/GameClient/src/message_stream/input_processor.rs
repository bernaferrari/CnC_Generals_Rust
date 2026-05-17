//! Input Processor - Complete integration of input handling
//!
//! This module orchestrates the full input → selection → command flow:
//! 1. Raw mouse/keyboard input from hardware
//! 2. Selection handling (box select, click select, control groups)
//! 3. Command generation (move, attack, etc.)
//! 4. Message stream injection
//!
//! Architecture matches C++ InGameUI.cpp message processing pipeline

use log::{debug, info, warn};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use super::game_message::*;
use super::message_stream::{take_emitted_messages, GameMessageDisposition, GameMessageTranslator};
use super::player_state::get_local_player_id;
use super::selection_xlat::SelectionTranslator;
use super::translators::CommandTranslator;
use crate::helpers::TheInGameUI;
use crate::input::{KeyCode, KeyModifiers, KeyboardState, MouseButton, MouseState};

const KEY_STATE_UP: u32 = 0x0001;
const KEY_STATE_DOWN: u32 = 0x0002;
const KEY_STATE_CONTROL: u32 = 0x0004 | 0x0008;
const KEY_STATE_SHIFT: u32 = 0x0010 | 0x0020 | 0x0400;
const KEY_STATE_ALT: u32 = 0x0040 | 0x0080;

/// Input event from hardware
/// Port of C++ input message types
#[derive(Debug, Clone)]
pub enum InputEvent {
    // Mouse events
    MouseMove {
        x: f32,
        y: f32,
        timestamp: Instant,
    },
    MouseButtonDown {
        button: MouseButton,
        x: f32,
        y: f32,
        timestamp: Instant,
    },
    MouseButtonUp {
        button: MouseButton,
        x: f32,
        y: f32,
        timestamp: Instant,
    },
    MouseScroll {
        delta_x: f32,
        delta_y: f32,
        timestamp: Instant,
    },

    // Keyboard events
    KeyDown {
        key: KeyCode,
        modifiers: KeyModifiers,
        timestamp: Instant,
    },
    KeyUp {
        key: KeyCode,
        modifiers: KeyModifiers,
        timestamp: Instant,
    },
    KeyRepeat {
        key: KeyCode,
        modifiers: KeyModifiers,
        timestamp: Instant,
    },

    // Special events
    FocusGained,
    FocusLost,
}

/// Input processor configuration
/// Matches C++ InGameUI configuration
#[derive(Debug, Clone)]
pub struct InputProcessorConfig {
    /// Enable/disable input processing
    pub enabled: bool,

    /// Mouse sensitivity (1.0 = normal)
    pub mouse_sensitivity: f32,

    /// Double-click time window (milliseconds)
    pub double_click_time_ms: u32,

    /// Drag tolerance (pixels)
    pub drag_tolerance: i32,

    /// Maximum selection count
    pub max_selection_count: usize,

    /// Enable alternate mouse mode
    pub alternate_mouse_mode: bool,

    /// Enable debug logging
    pub debug_logging: bool,
}

impl Default for InputProcessorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mouse_sensitivity: 1.0,
            double_click_time_ms: 500,
            drag_tolerance: 5,
            max_selection_count: 40,
            alternate_mouse_mode: false,
            debug_logging: false,
        }
    }
}

/// Complete input processing pipeline
/// Integrates C++ InGameUI, SelectionTranslator, and the canonical command translator.
pub struct InputProcessor {
    // Configuration
    config: InputProcessorConfig,

    // Input state
    mouse_state: MouseState,
    keyboard_state: KeyboardState,

    // Translators
    selection_translator: SelectionTranslator,
    command_translator: CommandTranslator,

    // Message queue (for generated messages)
    message_queue: VecDeque<GameMessage>,

    // Performance tracking
    events_processed: u64,
    messages_generated: u64,
    last_update_time: Instant,
}

impl InputProcessor {
    /// Create a new input processor
    pub fn new(config: InputProcessorConfig) -> Self {
        Self {
            config,
            mouse_state: MouseState::new(),
            keyboard_state: KeyboardState::new(),
            selection_translator: SelectionTranslator::new(),
            command_translator: CommandTranslator::new(),
            message_queue: VecDeque::new(),
            events_processed: 0,
            messages_generated: 0,
            last_update_time: Instant::now(),
        }
    }

    /// Create with default configuration
    pub fn with_default_config() -> Self {
        Self::new(InputProcessorConfig::default())
    }

    /// Process a raw input event
    /// Port of C++ InGameUI::ProcessRawInput() message dispatch
    pub fn process_input_event(&mut self, event: InputEvent) -> Vec<GameMessage> {
        if !self.config.enabled || !TheInGameUI::get_input_enabled() {
            return Vec::new();
        }

        self.events_processed += 1;

        // Convert input event to game messages
        let raw_messages = self.convert_input_to_messages(event);

        // Match C++ message-stream behavior more closely: keep track of every message generated
        // by this input event (raw + translator-emitted follow-ups), while still allowing each
        // translator to consume/replace messages for downstream processing.
        let mut processed_messages = Vec::new();
        let mut messages_to_process: VecDeque<GameMessage> = raw_messages.into_iter().collect();

        // Clear any stale emitted messages before this event's pipeline run.
        let _ = take_emitted_messages();

        while let Some(msg) = messages_to_process.pop_front() {
            processed_messages.push(msg.clone());

            let selection_result = self.selection_translator.translate_game_message(&msg);
            if matches!(selection_result, GameMessageDisposition::KeepMessage) {
                let _ = self.command_translator.translate_game_message(&msg);
            }

            let emitted = take_emitted_messages();
            if !emitted.is_empty() {
                for new_msg in emitted.into_iter().rev() {
                    messages_to_process.push_front(new_msg);
                }
            }
        }

        self.messages_generated = self
            .messages_generated
            .saturating_add(processed_messages.len() as u64);

        // Log if debug enabled
        if self.config.debug_logging && !processed_messages.is_empty() {
            debug!(
                "Input processor generated {} messages",
                processed_messages.len()
            );
        }

        processed_messages
    }

    /// Convert raw input event to game messages
    /// Port of C++ raw input message creation
    fn convert_input_to_messages(&mut self, event: InputEvent) -> Vec<GameMessage> {
        let mut messages = Vec::new();
        let player_id = get_local_player_id();

        match event {
            InputEvent::MouseMove { x, y, timestamp } => {
                self.mouse_state.update_position(x, y);

                let pos = ICoord2D {
                    x: x as i32,
                    y: y as i32,
                };

                messages.push(GameMessage::with_player(
                    GameMessageType::RawMousePosition(pos),
                    player_id,
                ));
            }

            InputEvent::MouseButtonDown {
                button,
                x,
                y,
                timestamp,
            } => {
                self.mouse_state.update_button(button, true, timestamp);

                let pos = ICoord2D {
                    x: x as i32,
                    y: y as i32,
                };

                let time = timestamp.elapsed().as_millis() as u32;
                let modifiers = u32::from(self.keyboard_state.modifiers().bits());

                let msg_type = match button {
                    MouseButton::Left => {
                        GameMessageType::RawMouseLeftButtonDown(pos.clone(), modifiers, time)
                    }
                    MouseButton::Right => {
                        GameMessageType::RawMouseRightButtonDown(pos.clone(), modifiers, time)
                    }
                    MouseButton::Middle => {
                        GameMessageType::RawMouseMiddleButtonDown(pos.clone(), modifiers, time)
                    }
                    _ => return messages, // Other buttons ignored
                };

                messages.push(GameMessage::with_player(msg_type, player_id));

                // Check for double-click
                if button == MouseButton::Left && self.mouse_state.click_count(button) >= 2 {
                    let region = IRegion2D {
                        x: pos.x,
                        y: pos.y,
                        width: 0,
                        height: 0,
                    };

                    messages.push(GameMessage::with_player(
                        GameMessageType::MouseLeftDoubleClick(region, modifiers),
                        player_id,
                    ));
                }
            }

            InputEvent::MouseButtonUp {
                button,
                x,
                y,
                timestamp,
            } => {
                self.mouse_state.update_button(button, false, timestamp);

                let pos = ICoord2D {
                    x: x as i32,
                    y: y as i32,
                };

                let time = timestamp.elapsed().as_millis() as u32;
                let modifiers = u32::from(self.keyboard_state.modifiers().bits());

                let msg_type = match button {
                    MouseButton::Left => {
                        GameMessageType::RawMouseLeftButtonUp(pos.clone(), modifiers, time)
                    }
                    MouseButton::Right => {
                        GameMessageType::RawMouseRightButtonUp(pos.clone(), modifiers, time)
                    }
                    MouseButton::Middle => {
                        GameMessageType::RawMouseMiddleButtonUp(pos.clone(), modifiers, time)
                    }
                    _ => return messages,
                };

                messages.push(GameMessage::with_player(msg_type, player_id));
            }

            InputEvent::MouseScroll {
                delta_x,
                delta_y,
                timestamp,
            } => {
                // Scroll events would be processed for camera control
                // Not part of selection/command system
            }

            InputEvent::KeyDown {
                key,
                modifiers,
                timestamp,
            } => {
                self.keyboard_state.update_key(key, true, timestamp);

                let key_code = self.key_to_virtual_code(key);

                let mut raw_key_msg =
                    GameMessage::with_player(GameMessageType::RawKeyDown(key_code), player_id);
                raw_key_msg.append_integer_argument(key_code as i32);
                raw_key_msg.append_integer_argument(self.build_key_state(modifiers, true) as i32);
                messages.push(raw_key_msg);

                match key {
                    KeyCode::LeftCtrl | KeyCode::RightCtrl => messages.push(
                        GameMessage::with_player(GameMessageType::MetaBeginForceAttack, player_id),
                    ),
                    KeyCode::LeftAlt | KeyCode::RightAlt => messages.push(
                        GameMessage::with_player(GameMessageType::MetaBeginWaypoints, player_id),
                    ),
                    KeyCode::LeftShift | KeyCode::RightShift => {
                        messages.push(GameMessage::with_player(
                            GameMessageType::MetaBeginPreferSelection,
                            player_id,
                        ));
                    }
                    _ => {}
                }

                if key == KeyCode::Escape {
                    TheInGameUI::clear_pending_command();
                    TheInGameUI::clear_pending_special_power();
                    TheInGameUI::place_build_available(None, None);
                    TheInGameUI::clear_attack_move_to_mode();
                    TheInGameUI::set_force_attack_mode(false);
                    TheInGameUI::set_force_move_to_mode(false);
                    TheInGameUI::set_prefer_selection_mode(false);
                }

                // Handle control group shortcuts (Ctrl+0-9)
                if modifiers.contains(KeyModifiers::CTRL) {
                    if let Some(group) = self.key_to_number_group(key) {
                        messages.push(GameMessage::with_player(
                            GameMessageType::MetaCreateTeam(group),
                            player_id,
                        ));
                    }
                }

                // Handle control group selection (0-9 alone)
                if modifiers.contains(KeyModifiers::SHIFT)
                    && !modifiers.contains(KeyModifiers::CTRL)
                    && !modifiers.contains(KeyModifiers::ALT)
                {
                    if let Some(group) = self.key_to_number_group(key) {
                        messages.push(GameMessage::with_player(
                            GameMessageType::MetaAddTeam(group),
                            player_id,
                        ));
                    }
                } else if !modifiers.contains(KeyModifiers::CTRL)
                    && !modifiers.contains(KeyModifiers::ALT)
                {
                    if let Some(group) = self.key_to_number_group(key) {
                        messages.push(GameMessage::with_player(
                            GameMessageType::MetaSelectTeam(group),
                            player_id,
                        ));
                    }
                }
            }

            InputEvent::KeyUp {
                key,
                modifiers,
                timestamp,
            } => {
                self.keyboard_state.update_key(key, false, timestamp);

                let key_code = self.key_to_virtual_code(key);

                let mut raw_key_msg =
                    GameMessage::with_player(GameMessageType::RawKeyUp(key_code), player_id);
                raw_key_msg.append_integer_argument(key_code as i32);
                raw_key_msg.append_integer_argument(self.build_key_state(modifiers, false) as i32);
                messages.push(raw_key_msg);

                match key {
                    KeyCode::LeftCtrl | KeyCode::RightCtrl => messages.push(
                        GameMessage::with_player(GameMessageType::MetaEndForceAttack, player_id),
                    ),
                    KeyCode::LeftAlt | KeyCode::RightAlt => messages.push(
                        GameMessage::with_player(GameMessageType::MetaEndWaypoints, player_id),
                    ),
                    KeyCode::LeftShift | KeyCode::RightShift => {
                        messages.push(GameMessage::with_player(
                            GameMessageType::MetaEndPreferSelection,
                            player_id,
                        ));
                    }
                    _ => {}
                }
            }

            InputEvent::KeyRepeat {
                key,
                modifiers,
                timestamp,
            } => {
                // Key repeat events (not used for most commands)
            }

            InputEvent::FocusGained => {
                info!("Input focus gained");
            }

            InputEvent::FocusLost => {
                info!("Input focus lost - clearing input state");
                self.clear_input_state();
            }
        }

        messages
    }

    /// Convert KeyCode to virtual key code
    /// Matches C++ virtual key code system
    fn key_to_virtual_code(&self, key: KeyCode) -> u32 {
        match key {
            KeyCode::A => 0x41,
            KeyCode::B => 0x42,
            KeyCode::C => 0x43,
            KeyCode::D => 0x44,
            KeyCode::E => 0x45,
            KeyCode::F => 0x46,
            KeyCode::G => 0x47,
            KeyCode::H => 0x48,
            KeyCode::I => 0x49,
            KeyCode::J => 0x4A,
            KeyCode::K => 0x4B,
            KeyCode::L => 0x4C,
            KeyCode::M => 0x4D,
            KeyCode::N => 0x4E,
            KeyCode::O => 0x4F,
            KeyCode::P => 0x50,
            KeyCode::Q => 0x51,
            KeyCode::R => 0x52,
            KeyCode::S => 0x53,
            KeyCode::T => 0x54,
            KeyCode::U => 0x55,
            KeyCode::V => 0x56,
            KeyCode::W => 0x57,
            KeyCode::X => 0x58,
            KeyCode::Y => 0x59,
            KeyCode::Z => 0x5A,
            KeyCode::LeftCtrl | KeyCode::RightCtrl => 0x11,
            KeyCode::LeftAlt | KeyCode::RightAlt => 0x12,
            KeyCode::LeftShift | KeyCode::RightShift => 0x10,
            KeyCode::Num0 => 0x30,
            KeyCode::Num1 => 0x31,
            KeyCode::Num2 => 0x32,
            KeyCode::Num3 => 0x33,
            KeyCode::Num4 => 0x34,
            KeyCode::Num5 => 0x35,
            KeyCode::Num6 => 0x36,
            KeyCode::Num7 => 0x37,
            KeyCode::Num8 => 0x38,
            KeyCode::Num9 => 0x39,
            KeyCode::Space => 0x20,
            KeyCode::Escape => 0x1B,
            KeyCode::Enter => 0x0D,
            KeyCode::Tab => 0x09,
            KeyCode::Backspace => 0x08,
            KeyCode::Delete => 0x2E,
            KeyCode::Insert => 0x2D,
            KeyCode::Home => 0x24,
            KeyCode::End => 0x23,
            KeyCode::PageUp => 0x21,
            KeyCode::PageDown => 0x22,
            KeyCode::Left => 0x25,
            KeyCode::Up => 0x26,
            KeyCode::Right => 0x27,
            KeyCode::Down => 0x28,
            KeyCode::F1 => 0x70,
            KeyCode::F2 => 0x71,
            KeyCode::F3 => 0x72,
            KeyCode::F4 => 0x73,
            KeyCode::F5 => 0x74,
            KeyCode::F6 => 0x75,
            KeyCode::F7 => 0x76,
            KeyCode::F8 => 0x77,
            KeyCode::F9 => 0x78,
            KeyCode::F10 => 0x79,
            KeyCode::F11 => 0x7A,
            KeyCode::F12 => 0x7B,
            KeyCode::Minus => 0xBD,
            KeyCode::Plus => 0xBB,
            KeyCode::LeftBracket => 0xDB,
            KeyCode::RightBracket => 0xDD,
            KeyCode::Semicolon => 0xBA,
            KeyCode::Quote => 0xDE,
            KeyCode::Grave => 0xC0,
            KeyCode::Backslash => 0xDC,
            KeyCode::Comma => 0xBC,
            KeyCode::Period => 0xBE,
            KeyCode::Slash => 0xBF,
            KeyCode::NumPad0 => 0x60,
            KeyCode::NumPad1 => 0x61,
            KeyCode::NumPad2 => 0x62,
            KeyCode::NumPad3 => 0x63,
            KeyCode::NumPad4 => 0x64,
            KeyCode::NumPad5 => 0x65,
            KeyCode::NumPad6 => 0x66,
            KeyCode::NumPad7 => 0x67,
            KeyCode::NumPad8 => 0x68,
            KeyCode::NumPad9 => 0x69,
            KeyCode::NumPadMultiply => 0x6A,
            KeyCode::NumPadAdd => 0x6B,
            KeyCode::NumPadSubtract => 0x6D,
            KeyCode::NumPadDecimal => 0x6E,
            KeyCode::NumPadDivide => 0x6F,
            KeyCode::NumPadEnter => 0x0D,
            _ => 0,
        }
    }

    fn build_key_state(&self, modifiers: KeyModifiers, is_down: bool) -> u32 {
        let mut state = if is_down {
            KEY_STATE_DOWN
        } else {
            KEY_STATE_UP
        };

        if modifiers.contains(KeyModifiers::CTRL) {
            state |= KEY_STATE_CONTROL;
        }
        if modifiers.contains(KeyModifiers::SHIFT) {
            state |= KEY_STATE_SHIFT;
        }
        if modifiers.contains(KeyModifiers::ALT) {
            state |= KEY_STATE_ALT;
        }

        state
    }

    /// Convert KeyCode to control group number
    /// Returns Some(0-9) for number keys, None otherwise
    fn key_to_number_group(&self, key: KeyCode) -> Option<u8> {
        match key {
            KeyCode::Num0 => Some(0),
            KeyCode::Num1 => Some(1),
            KeyCode::Num2 => Some(2),
            KeyCode::Num3 => Some(3),
            KeyCode::Num4 => Some(4),
            KeyCode::Num5 => Some(5),
            KeyCode::Num6 => Some(6),
            KeyCode::Num7 => Some(7),
            KeyCode::Num8 => Some(8),
            KeyCode::Num9 => Some(9),
            _ => None,
        }
    }

    /// Clear all input state (on focus loss)
    fn clear_input_state(&mut self) {
        self.mouse_state.reset();
        self.keyboard_state.reset();
        self.message_queue.clear();
    }

    /// Update input processor (called every frame)
    /// Port of C++ InGameUI::Update() input section
    pub fn update(&mut self) {
        // Update mouse state
        self.mouse_state.update_frame();

        // Update keyboard state
        self.keyboard_state.update_frame();

        // Update performance tracking
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update_time);

        if elapsed.as_secs() >= 1 {
            if self.config.debug_logging {
                debug!(
                    "Input processor: {} events/sec, {} messages/sec",
                    self.events_processed, self.messages_generated
                );
            }

            self.events_processed = 0;
            self.messages_generated = 0;
            self.last_update_time = now;
        }
    }

    /// Get current mouse position
    pub fn mouse_position(&self) -> (f32, f32) {
        self.mouse_state.position()
    }

    /// Check if mouse button is down
    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.mouse_state.is_button_down(button)
    }

    /// Check if key is down
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.keyboard_state.is_key_down(key)
    }

    /// Get current keyboard modifiers
    pub fn keyboard_modifiers(&self) -> KeyModifiers {
        self.keyboard_state.modifiers()
    }

    /// Set mouse sensitivity
    pub fn set_mouse_sensitivity(&mut self, sensitivity: f32) {
        self.config.mouse_sensitivity = sensitivity;
        self.mouse_state.set_sensitivity(sensitivity);
    }

    /// Enable/disable input processing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;

        if !enabled {
            self.clear_input_state();
        }
    }

    /// Get statistics
    pub fn get_statistics(&self) -> InputProcessorStatistics {
        InputProcessorStatistics {
            events_processed: self.events_processed,
            messages_generated: self.messages_generated,
            mouse_position: self.mouse_state.position(),
            keys_pressed: self.keyboard_state.pressed_keys().len() as u32,
        }
    }

    /// Access selection translator directly
    pub fn selection_translator(&self) -> &SelectionTranslator {
        &self.selection_translator
    }

    /// Access selection translator mutably
    pub fn selection_translator_mut(&mut self) -> &mut SelectionTranslator {
        &mut self.selection_translator
    }

    /// Access command translator directly
    pub fn command_translator(&self) -> &CommandTranslator {
        &self.command_translator
    }

    /// Access command translator mutably
    pub fn command_translator_mut(&mut self) -> &mut CommandTranslator {
        &mut self.command_translator
    }
}

/// Input processor statistics
#[derive(Debug, Clone)]
pub struct InputProcessorStatistics {
    pub events_processed: u64,
    pub messages_generated: u64,
    pub mouse_position: (f32, f32),
    pub keys_pressed: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_stream::player_state::set_local_player_id;
    use std::sync::Mutex;

    static LOCAL_PLAYER_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_input_processor_creation() {
        let processor = InputProcessor::with_default_config();
        assert!(processor.config.enabled);
        assert_eq!(processor.events_processed, 0);
        assert_eq!(processor.messages_generated, 0);
    }

    #[test]
    fn test_mouse_move_processing() {
        let mut processor = InputProcessor::with_default_config();

        let event = InputEvent::MouseMove {
            x: 100.0,
            y: 200.0,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(event);

        assert!(messages.len() >= 1);
        assert_eq!(processor.mouse_position(), (100.0, 200.0));
    }

    #[test]
    fn test_input_messages_use_current_local_player_id() {
        let _guard = LOCAL_PLAYER_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set_local_player_id(3);
        let mut processor = InputProcessor::with_default_config();

        let messages = processor.process_input_event(InputEvent::MouseMove {
            x: 64.0,
            y: 96.0,
            timestamp: Instant::now(),
        });

        assert!(!messages.is_empty());
        assert!(messages
            .iter()
            .all(|message| message.get_player_index() == 3));

        set_local_player_id(0);
    }

    #[test]
    fn test_mouse_button_processing() {
        let mut processor = InputProcessor::with_default_config();

        // Mouse down
        let down_event = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(down_event);
        assert!(messages.len() >= 1);
        assert!(processor.is_mouse_button_down(MouseButton::Left));

        // Mouse up
        let up_event = InputEvent::MouseButtonUp {
            button: MouseButton::Left,
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(up_event);
        assert!(messages.len() >= 1);
    }

    #[test]
    fn test_keyboard_processing() {
        let mut processor = InputProcessor::with_default_config();

        // Key down
        let down_event = InputEvent::KeyDown {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(down_event);
        assert!(messages.len() >= 1);
        assert!(processor.is_key_down(KeyCode::A));

        // Key up
        let up_event = InputEvent::KeyUp {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(up_event);
        assert!(messages.len() >= 1);
    }

    #[test]
    fn test_control_group_shortcut() {
        let mut processor = InputProcessor::with_default_config();

        // Ctrl+1 to create control group
        let event = InputEvent::KeyDown {
            key: KeyCode::Num1,
            modifiers: KeyModifiers::CTRL,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(event);

        // Should generate both RawKeyDown and MetaCreateTeam messages
        assert!(messages.len() >= 2);

        let has_create_team = messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaCreateTeam(1)));

        assert!(has_create_team);
    }

    #[test]
    fn test_number_key_conversion() {
        let processor = InputProcessor::with_default_config();

        assert_eq!(processor.key_to_number_group(KeyCode::Num0), Some(0));
        assert_eq!(processor.key_to_number_group(KeyCode::Num1), Some(1));
        assert_eq!(processor.key_to_number_group(KeyCode::Num9), Some(9));
        assert_eq!(processor.key_to_number_group(KeyCode::A), None);
    }

    #[test]
    fn test_virtual_key_conversion() {
        let processor = InputProcessor::with_default_config();

        assert_eq!(processor.key_to_virtual_code(KeyCode::A), 0x41);
        assert_eq!(processor.key_to_virtual_code(KeyCode::S), 0x53);
        assert_eq!(processor.key_to_virtual_code(KeyCode::LeftCtrl), 0x11);
        assert_eq!(processor.key_to_virtual_code(KeyCode::Space), 0x20);
    }

    #[test]
    fn test_focus_loss_clears_state() {
        let mut processor = InputProcessor::with_default_config();

        // Simulate mouse down
        let down_event = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };

        processor.process_input_event(down_event);
        assert!(processor.is_mouse_button_down(MouseButton::Left));

        // Lose focus
        processor.process_input_event(InputEvent::FocusLost);

        // State should be cleared
        assert!(!processor.is_mouse_button_down(MouseButton::Left));
    }

    #[test]
    fn test_enable_disable() {
        let mut processor = InputProcessor::with_default_config();

        processor.set_enabled(false);
        assert!(!processor.config.enabled);

        // Events should not be processed when disabled
        let event = InputEvent::MouseMove {
            x: 100.0,
            y: 200.0,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(event);
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_statistics() {
        let mut processor = InputProcessor::with_default_config();

        // Process some events
        for i in 0..10 {
            let event = InputEvent::MouseMove {
                x: (i * 10) as f32,
                y: (i * 10) as f32,
                timestamp: Instant::now(),
            };

            processor.process_input_event(event);
        }

        let stats = processor.get_statistics();
        assert_eq!(stats.events_processed, 10);
        assert!(stats.messages_generated >= 10);
    }
}
