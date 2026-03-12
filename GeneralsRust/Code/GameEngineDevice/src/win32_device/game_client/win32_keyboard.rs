//! Win32 Keyboard Base Implementation
//! 
//! This module provides the base keyboard interface that can be extended by
//! specific implementations like DirectInput keyboard.
//! 
//! This serves as the foundation for all keyboard input handling in the game engine.

use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use thiserror::Error;
use tracing::{debug, info};

// Re-export keyboard types from DirectInput keyboard for compatibility
pub use super::win32_di_keyboard::{KeyboardIO, DirectInputKeyboardError};

/// Key repeat delay in frames
const KEY_REPEAT_DELAY: u32 = 10;

/// Maximum number of keyboard events to buffer
const MAX_KEYBOARD_EVENTS: usize = 128;

/// Win32 keyboard errors
#[derive(Error, Debug)]
pub enum Win32KeyboardError {
    #[error("Keyboard initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Device error: {0}")]
    DeviceError(#[from] DirectInputKeyboardError),
    #[error("Event buffer full")]
    BufferFull,
}

/// Base Win32 keyboard interface
/// 
/// This provides the common keyboard functionality that all keyboard implementations
/// must support. Specific implementations like DirectInputKeyboard extend this.
pub trait Win32Keyboard {
    /// Initialize the keyboard system
    fn init(&mut self) -> Result<(), Win32KeyboardError>;
    
    /// Reset the keyboard system
    fn reset(&mut self) -> Result<(), Win32KeyboardError>;
    
    /// Update keyboard state (called once per frame)
    fn update(&mut self) -> Result<(), Win32KeyboardError>;
    
    /// Get a keyboard event
    fn get_key(&mut self) -> Result<KeyboardIO, Win32KeyboardError>;
    
    /// Check if caps lock is active
    fn get_caps_state(&self) -> bool;
    
    /// Check if the keyboard is initialized
    fn is_initialized(&self) -> bool;
}

/// Keyboard event buffer for message-based input
#[derive(Debug, Clone)]
pub struct KeyboardEventBuffer {
    /// Event buffer
    events: VecDeque<KeyboardIO>,
    /// Maximum events to buffer
    max_events: usize,
    /// Buffer initialized flag
    initialized: AtomicBool,
}

impl KeyboardEventBuffer {
    /// Create a new keyboard event buffer
    pub fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(MAX_KEYBOARD_EVENTS),
            max_events: MAX_KEYBOARD_EVENTS,
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize the buffer
    pub fn init(&mut self) -> Result<(), Win32KeyboardError> {
        self.events.clear();
        self.initialized.store(true, Ordering::SeqCst);
        info!("Keyboard event buffer initialized");
        Ok(())
    }

    /// Add a keyboard event to the buffer
    pub fn add_event(&mut self, event: KeyboardIO) -> Result<(), Win32KeyboardError> {
        if self.events.len() >= self.max_events {
            debug!("Keyboard event buffer full, dropping oldest event");
            self.events.pop_front();
        }
        
        self.events.push_back(event);
        debug!("Added keyboard event: key=0x{:02x}, state=0x{:04x}", event.key, event.state);
        Ok(())
    }

    /// Get the next keyboard event from the buffer
    pub fn get_event(&mut self) -> Option<KeyboardIO> {
        self.events.pop_front()
    }

    /// Get the number of events in the buffer
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Clear all events from the buffer
    pub fn clear(&mut self) {
        self.events.clear();
        debug!("Keyboard event buffer cleared");
    }

    /// Check if the buffer is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Reset the buffer
    pub fn reset(&mut self) -> Result<(), Win32KeyboardError> {
        self.clear();
        debug!("Keyboard event buffer reset");
        Ok(())
    }
}

impl Default for KeyboardEventBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Keyboard state tracking for key repeat and modifiers
#[derive(Debug, Clone, Default)]
pub struct KeyboardState {
    /// Key states for repeat detection
    key_states: [bool; 256],
    /// Key press times for repeat timing
    key_times: [u32; 256],
    /// Current frame counter
    current_frame: u32,
    /// Modifier key states
    modifiers: u16,
}

impl KeyboardState {
    /// Create a new keyboard state tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Update keyboard state for a frame
    pub fn update_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    /// Update key state
    pub fn update_key(&mut self, key: u8, pressed: bool, frame: u32) {
        if (key as usize) < self.key_states.len() {
            self.key_states[key as usize] = pressed;
            if pressed {
                self.key_times[key as usize] = frame;
            }
        }
    }

    /// Check if a key is currently pressed
    pub fn is_key_pressed(&self, key: u8) -> bool {
        if (key as usize) < self.key_states.len() {
            self.key_states[key as usize]
        } else {
            false
        }
    }

    /// Check if a key should repeat
    pub fn should_key_repeat(&self, key: u8) -> bool {
        if (key as usize) < self.key_states.len() && self.key_states[key as usize] {
            let press_time = self.key_times[key as usize];
            self.current_frame > press_time && (self.current_frame - press_time) >= KEY_REPEAT_DELAY
        } else {
            false
        }
    }

    /// Set modifier state
    pub fn set_modifiers(&mut self, modifiers: u16) {
        self.modifiers = modifiers;
    }

    /// Get current modifier state
    pub fn get_modifiers(&self) -> u16 {
        self.modifiers
    }

    /// Clear all key states
    pub fn clear(&mut self) {
        self.key_states = [false; 256];
        self.key_times = [0; 256];
        self.modifiers = 0;
        debug!("Keyboard state cleared");
    }
}

/// Utility functions for keyboard handling
pub mod keyboard_utils {
    use super::*;

    /// Convert Windows virtual key code to DirectInput scancode
    pub fn vk_to_scancode(vk: u32) -> u8 {
        // This would normally use MapVirtualKey on Windows
        // For now, provide a basic mapping for common keys
        match vk {
            0x08 => 0x0E, // VK_BACK -> DIK_BACK
            0x09 => 0x0F, // VK_TAB -> DIK_TAB
            0x0D => 0x1C, // VK_RETURN -> DIK_RETURN
            0x10 => 0x2A, // VK_SHIFT -> DIK_LSHIFT
            0x11 => 0x1D, // VK_CONTROL -> DIK_LCONTROL
            0x12 => 0x38, // VK_MENU -> DIK_LALT
            0x1B => 0x01, // VK_ESCAPE -> DIK_ESCAPE
            0x20 => 0x39, // VK_SPACE -> DIK_SPACE
            // Numbers
            0x30..=0x39 => (vk - 0x30 + 0x0B) as u8,
            // Letters
            0x41..=0x5A => (vk - 0x41 + 0x1E) as u8,
            // Function keys
            0x70..=0x7B => (vk - 0x70 + 0x3B) as u8,
            _ => 0xFF, // Unknown key
        }
    }

    /// Check if a key is a modifier key
    pub fn is_modifier_key(scancode: u8) -> bool {
        matches!(scancode, 
            0x1D | 0x2A | 0x36 | 0x38 | // Ctrl, LShift, RShift, Alt
            0x9D | 0xB8 | 0x3A | 0x45 | 0x46 // RCtrl, RAlt, CapsLock, NumLock, ScrollLock
        )
    }

    /// Convert scancode to human-readable key name (for debugging)
    pub fn scancode_to_name(scancode: u8) -> &'static str {
        match scancode {
            0x01 => "Escape",
            0x0E => "Backspace",
            0x0F => "Tab",
            0x1C => "Enter",
            0x1D => "Ctrl",
            0x2A => "Shift",
            0x36 => "RShift",
            0x38 => "Alt",
            0x39 => "Space",
            0x3A => "CapsLock",
            0x1E => "A",
            0x30 => "B",
            0x2E => "C",
            0x20 => "D",
            0x12 => "E",
            0x21 => "F",
            0x22 => "G",
            0x23 => "H",
            0x17 => "I",
            0x24 => "J",
            0x25 => "K",
            0x26 => "L",
            0x32 => "M",
            0x31 => "N",
            0x18 => "O",
            0x19 => "P",
            0x10 => "Q",
            0x13 => "R",
            0x1F => "S",
            0x14 => "T",
            0x16 => "U",
            0x2F => "V",
            0x11 => "W",
            0x2D => "X",
            0x15 => "Y",
            0x2C => "Z",
            _ => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_event_buffer() {
        let mut buffer = KeyboardEventBuffer::new();
        assert!(!buffer.is_initialized());
        
        buffer.init().unwrap();
        assert!(buffer.is_initialized());
        assert_eq!(buffer.event_count(), 0);

        // Add an event
        let event = KeyboardIO {
            key: 0x1E, // 'A'
            state: 1,
            sequence: 100,
            status: 0,
        };
        buffer.add_event(event).unwrap();
        assert_eq!(buffer.event_count(), 1);

        // Get the event
        let retrieved = buffer.get_event().unwrap();
        assert_eq!(retrieved.key, 0x1E);
        assert_eq!(buffer.event_count(), 0);
    }

    #[test]
    fn test_keyboard_state() {
        let mut state = KeyboardState::new();
        
        // Test key press
        state.update_key(0x1E, true, 100); // Press 'A'
        assert!(state.is_key_pressed(0x1E));
        
        // Test key release
        state.update_key(0x1E, false, 101);
        assert!(!state.is_key_pressed(0x1E));

        // Test key repeat
        state.update_key(0x1E, true, 100);
        state.update_frame(100 + KEY_REPEAT_DELAY + 1);
        assert!(state.should_key_repeat(0x1E));
    }

    #[test]
    fn test_vk_to_scancode() {
        use keyboard_utils::*;
        
        assert_eq!(vk_to_scancode(0x41), 0x1E); // 'A' -> DIK_A
        assert_eq!(vk_to_scancode(0x1B), 0x01); // Escape -> DIK_ESCAPE
        assert_eq!(vk_to_scancode(0x20), 0x39); // Space -> DIK_SPACE
        assert_eq!(vk_to_scancode(0x999), 0xFF); // Unknown -> 0xFF
    }

    #[test]
    fn test_modifier_key_detection() {
        use keyboard_utils::*;
        
        assert!(is_modifier_key(0x1D)); // Ctrl
        assert!(is_modifier_key(0x2A)); // Shift
        assert!(is_modifier_key(0x38)); // Alt
        assert!(!is_modifier_key(0x1E)); // 'A'
    }

    #[test]
    fn test_scancode_names() {
        use keyboard_utils::*;
        
        assert_eq!(scancode_to_name(0x1E), "A");
        assert_eq!(scancode_to_name(0x01), "Escape");
        assert_eq!(scancode_to_name(0x39), "Space");
        assert_eq!(scancode_to_name(0xFF), "Unknown");
    }
}