//! # Keyboard Input Module
//!
//! Comprehensive keyboard input handling with state tracking, key mapping,
//! and event processing for the Command & Conquer Generals Zero Hour game.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode as WinitKeyCode, KeyLocation, PhysicalKey};

use super::{InputError, InputStats, KeyModifiers};
use crate::system::SubsystemInterface;

/// Key codes matching the original game's key definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Number row
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Num0,

    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Navigation keys
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,

    // Special keys
    Space,
    Enter,
    Tab,
    Backspace,
    Delete,
    Insert,
    Escape,
    Pause,
    PrintScreen,

    // Modifier keys
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftMeta,
    RightMeta, // Windows/Cmd key

    // Numpad
    NumPad0,
    NumPad1,
    NumPad2,
    NumPad3,
    NumPad4,
    NumPad5,
    NumPad6,
    NumPad7,
    NumPad8,
    NumPad9,
    NumPadAdd,
    NumPadSubtract,
    NumPadMultiply,
    NumPadDivide,
    NumPadDecimal,
    NumPadEnter,

    // Lock keys
    CapsLock,
    NumLock,
    ScrollLock,

    // Punctuation and symbols
    Minus,
    Plus,
    LeftBracket,
    RightBracket,
    Semicolon,
    Quote,
    Grave,
    Backslash,
    Slash,
    Comma,
    Period,

    // Unknown/unmapped key
    Unknown,
}

impl From<WinitKeyCode> for KeyCode {
    fn from(winit_key: WinitKeyCode) -> Self {
        match winit_key {
            WinitKeyCode::F1 => KeyCode::F1,
            WinitKeyCode::F2 => KeyCode::F2,
            WinitKeyCode::F3 => KeyCode::F3,
            WinitKeyCode::F4 => KeyCode::F4,
            WinitKeyCode::F5 => KeyCode::F5,
            WinitKeyCode::F6 => KeyCode::F6,
            WinitKeyCode::F7 => KeyCode::F7,
            WinitKeyCode::F8 => KeyCode::F8,
            WinitKeyCode::F9 => KeyCode::F9,
            WinitKeyCode::F10 => KeyCode::F10,
            WinitKeyCode::F11 => KeyCode::F11,
            WinitKeyCode::F12 => KeyCode::F12,

            WinitKeyCode::Digit1 => KeyCode::Num1,
            WinitKeyCode::Digit2 => KeyCode::Num2,
            WinitKeyCode::Digit3 => KeyCode::Num3,
            WinitKeyCode::Digit4 => KeyCode::Num4,
            WinitKeyCode::Digit5 => KeyCode::Num5,
            WinitKeyCode::Digit6 => KeyCode::Num6,
            WinitKeyCode::Digit7 => KeyCode::Num7,
            WinitKeyCode::Digit8 => KeyCode::Num8,
            WinitKeyCode::Digit9 => KeyCode::Num9,
            WinitKeyCode::Digit0 => KeyCode::Num0,

            WinitKeyCode::KeyA => KeyCode::A,
            WinitKeyCode::KeyB => KeyCode::B,
            WinitKeyCode::KeyC => KeyCode::C,
            WinitKeyCode::KeyD => KeyCode::D,
            WinitKeyCode::KeyE => KeyCode::E,
            WinitKeyCode::KeyF => KeyCode::F,
            WinitKeyCode::KeyG => KeyCode::G,
            WinitKeyCode::KeyH => KeyCode::H,
            WinitKeyCode::KeyI => KeyCode::I,
            WinitKeyCode::KeyJ => KeyCode::J,
            WinitKeyCode::KeyK => KeyCode::K,
            WinitKeyCode::KeyL => KeyCode::L,
            WinitKeyCode::KeyM => KeyCode::M,
            WinitKeyCode::KeyN => KeyCode::N,
            WinitKeyCode::KeyO => KeyCode::O,
            WinitKeyCode::KeyP => KeyCode::P,
            WinitKeyCode::KeyQ => KeyCode::Q,
            WinitKeyCode::KeyR => KeyCode::R,
            WinitKeyCode::KeyS => KeyCode::S,
            WinitKeyCode::KeyT => KeyCode::T,
            WinitKeyCode::KeyU => KeyCode::U,
            WinitKeyCode::KeyV => KeyCode::V,
            WinitKeyCode::KeyW => KeyCode::W,
            WinitKeyCode::KeyX => KeyCode::X,
            WinitKeyCode::KeyY => KeyCode::Y,
            WinitKeyCode::KeyZ => KeyCode::Z,

            WinitKeyCode::ArrowLeft => KeyCode::Left,
            WinitKeyCode::ArrowRight => KeyCode::Right,
            WinitKeyCode::ArrowUp => KeyCode::Up,
            WinitKeyCode::ArrowDown => KeyCode::Down,
            WinitKeyCode::Home => KeyCode::Home,
            WinitKeyCode::End => KeyCode::End,
            WinitKeyCode::PageUp => KeyCode::PageUp,
            WinitKeyCode::PageDown => KeyCode::PageDown,

            WinitKeyCode::Space => KeyCode::Space,
            WinitKeyCode::Enter => KeyCode::Enter,
            WinitKeyCode::Tab => KeyCode::Tab,
            WinitKeyCode::Backspace => KeyCode::Backspace,
            WinitKeyCode::Delete => KeyCode::Delete,
            WinitKeyCode::Insert => KeyCode::Insert,
            WinitKeyCode::Escape => KeyCode::Escape,
            WinitKeyCode::Pause => KeyCode::Pause,
            WinitKeyCode::PrintScreen => KeyCode::PrintScreen,

            WinitKeyCode::ShiftLeft => KeyCode::LeftShift,
            WinitKeyCode::ShiftRight => KeyCode::RightShift,
            WinitKeyCode::ControlLeft => KeyCode::LeftCtrl,
            WinitKeyCode::ControlRight => KeyCode::RightCtrl,
            WinitKeyCode::AltLeft => KeyCode::LeftAlt,
            WinitKeyCode::AltRight => KeyCode::RightAlt,
            WinitKeyCode::SuperLeft => KeyCode::LeftMeta,
            WinitKeyCode::SuperRight => KeyCode::RightMeta,
            WinitKeyCode::Meta => KeyCode::LeftMeta,

            WinitKeyCode::Numpad0 => KeyCode::NumPad0,
            WinitKeyCode::Numpad1 => KeyCode::NumPad1,
            WinitKeyCode::Numpad2 => KeyCode::NumPad2,
            WinitKeyCode::Numpad3 => KeyCode::NumPad3,
            WinitKeyCode::Numpad4 => KeyCode::NumPad4,
            WinitKeyCode::Numpad5 => KeyCode::NumPad5,
            WinitKeyCode::Numpad6 => KeyCode::NumPad6,
            WinitKeyCode::Numpad7 => KeyCode::NumPad7,
            WinitKeyCode::Numpad8 => KeyCode::NumPad8,
            WinitKeyCode::Numpad9 => KeyCode::NumPad9,
            WinitKeyCode::NumpadAdd => KeyCode::NumPadAdd,
            WinitKeyCode::NumpadSubtract => KeyCode::NumPadSubtract,
            WinitKeyCode::NumpadMultiply => KeyCode::NumPadMultiply,
            WinitKeyCode::NumpadDivide => KeyCode::NumPadDivide,
            WinitKeyCode::NumpadDecimal => KeyCode::NumPadDecimal,
            WinitKeyCode::NumpadEnter => KeyCode::NumPadEnter,

            WinitKeyCode::CapsLock => KeyCode::CapsLock,
            WinitKeyCode::NumLock => KeyCode::NumLock,
            WinitKeyCode::ScrollLock => KeyCode::ScrollLock,

            WinitKeyCode::Minus => KeyCode::Minus,
            WinitKeyCode::Equal => KeyCode::Plus,
            WinitKeyCode::BracketLeft => KeyCode::LeftBracket,
            WinitKeyCode::BracketRight => KeyCode::RightBracket,
            WinitKeyCode::Semicolon => KeyCode::Semicolon,
            WinitKeyCode::Quote => KeyCode::Quote,
            WinitKeyCode::Backquote => KeyCode::Grave,
            WinitKeyCode::Backslash => KeyCode::Backslash,
            WinitKeyCode::Slash => KeyCode::Slash,
            WinitKeyCode::Comma => KeyCode::Comma,
            WinitKeyCode::Period => KeyCode::Period,

            _ => KeyCode::Unknown,
        }
    }
}

fn map_winit_key_code(winit_key: WinitKeyCode, location: KeyLocation) -> KeyCode {
    match winit_key {
        WinitKeyCode::Meta => match location {
            KeyLocation::Right => KeyCode::RightMeta,
            KeyLocation::Left | KeyLocation::Standard | KeyLocation::Numpad => KeyCode::LeftMeta,
        },
        _ => KeyCode::from(winit_key),
    }
}

/// Key state information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Key is not pressed
    Released,
    /// Key was just pressed this frame
    JustPressed,
    /// Key is held down
    Pressed,
    /// Key was just released this frame
    JustReleased,
}

impl KeyState {
    /// Check if the key is currently down (pressed or just pressed)
    pub fn is_down(self) -> bool {
        matches!(self, KeyState::Pressed | KeyState::JustPressed)
    }

    /// Check if the key is currently up (released or just released)
    pub fn is_up(self) -> bool {
        matches!(self, KeyState::Released | KeyState::JustReleased)
    }

    /// Check if the key was just pressed this frame
    pub fn just_pressed(self) -> bool {
        matches!(self, KeyState::JustPressed)
    }

    /// Check if the key was just released this frame
    pub fn just_released(self) -> bool {
        matches!(self, KeyState::JustReleased)
    }
}

/// Complete keyboard state tracking
#[derive(Debug)]
pub struct KeyboardState {
    /// Current state of all keys
    key_states: HashMap<KeyCode, KeyState>,
    /// Time when each key was last pressed (for repeat handling)
    key_press_times: HashMap<KeyCode, Instant>,
    /// Keys that are repeating
    repeating_keys: HashMap<KeyCode, Instant>,
    /// Current modifier key state
    modifiers: KeyModifiers,
    /// Key repeat configuration
    repeat_delay: Duration,
    repeat_interval: Duration,
    /// Last key that was pressed (for hotkey detection)
    last_key_pressed: Option<KeyCode>,
}

impl KeyboardState {
    /// Create a new keyboard state
    pub fn new() -> Self {
        Self {
            key_states: HashMap::new(),
            key_press_times: HashMap::new(),
            repeating_keys: HashMap::new(),
            modifiers: KeyModifiers::empty(),
            repeat_delay: Duration::from_millis(500),
            repeat_interval: Duration::from_millis(30),
            last_key_pressed: None,
        }
    }

    /// Set key repeat configuration
    pub fn set_repeat_config(&mut self, delay: Duration, interval: Duration) {
        self.repeat_delay = delay;
        self.repeat_interval = interval;
    }

    /// Update key state from input
    pub fn update_key(&mut self, key: KeyCode, pressed: bool, timestamp: Instant) {
        let current_state = self
            .key_states
            .get(&key)
            .copied()
            .unwrap_or(KeyState::Released);

        let new_state = match (current_state, pressed) {
            (KeyState::Released, true) | (KeyState::JustReleased, true) => {
                self.key_press_times.insert(key, timestamp);
                self.repeating_keys.remove(&key);
                // Track last key pressed for hotkey detection
                self.last_key_pressed = Some(key);
                KeyState::JustPressed
            }
            (KeyState::JustPressed, true) | (KeyState::Pressed, true) => KeyState::Pressed,
            (KeyState::Pressed, false) | (KeyState::JustPressed, false) => {
                self.key_press_times.remove(&key);
                self.repeating_keys.remove(&key);
                KeyState::JustReleased
            }
            (KeyState::Released, false) | (KeyState::JustReleased, false) => KeyState::Released,
        };

        self.key_states.insert(key, new_state);

        // Update modifier keys
        self.update_modifiers(key, pressed);
    }

    /// Update modifier key state
    fn update_modifiers(&mut self, key: KeyCode, pressed: bool) {
        match key {
            KeyCode::LeftShift | KeyCode::RightShift => {
                if pressed {
                    self.modifiers.insert(KeyModifiers::SHIFT);
                } else if !self.is_key_down(KeyCode::LeftShift)
                    && !self.is_key_down(KeyCode::RightShift)
                {
                    self.modifiers.remove(KeyModifiers::SHIFT);
                }
            }
            KeyCode::LeftCtrl | KeyCode::RightCtrl => {
                if pressed {
                    self.modifiers.insert(KeyModifiers::CTRL);
                } else if !self.is_key_down(KeyCode::LeftCtrl)
                    && !self.is_key_down(KeyCode::RightCtrl)
                {
                    self.modifiers.remove(KeyModifiers::CTRL);
                }
            }
            KeyCode::LeftAlt | KeyCode::RightAlt => {
                if pressed {
                    self.modifiers.insert(KeyModifiers::ALT);
                } else if !self.is_key_down(KeyCode::LeftAlt)
                    && !self.is_key_down(KeyCode::RightAlt)
                {
                    self.modifiers.remove(KeyModifiers::ALT);
                }
            }
            KeyCode::LeftMeta | KeyCode::RightMeta => {
                if pressed {
                    self.modifiers.insert(KeyModifiers::META);
                } else if !self.is_key_down(KeyCode::LeftMeta)
                    && !self.is_key_down(KeyCode::RightMeta)
                {
                    self.modifiers.remove(KeyModifiers::META);
                }
            }
            _ => {}
        }
    }

    /// Get the state of a specific key
    pub fn key_state(&self, key: KeyCode) -> KeyState {
        self.key_states
            .get(&key)
            .copied()
            .unwrap_or(KeyState::Released)
    }

    /// Check if a key is currently down
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.key_state(key).is_down()
    }

    /// Check if a key was just pressed this frame
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.key_state(key).just_pressed()
    }

    /// Check if a key was just released this frame
    pub fn is_key_just_released(&self, key: KeyCode) -> bool {
        self.key_state(key).just_released()
    }

    /// Get current modifier key state
    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    /// Get the last key that was pressed (for hotkey detection)
    pub fn get_last_key_pressed(&self) -> Option<KeyCode> {
        self.last_key_pressed
    }

    /// Clear the last key pressed tracking (call after processing)
    pub fn clear_last_key_pressed(&mut self) {
        self.last_key_pressed = None;
    }

    /// Check if CTRL key is currently pressed
    pub fn is_ctrl_pressed(&self) -> bool {
        self.modifiers.contains(KeyModifiers::CTRL)
    }

    /// Check if SHIFT key is currently pressed
    pub fn is_shift_pressed(&self) -> bool {
        self.modifiers.contains(KeyModifiers::SHIFT)
    }

    /// Check if ALT key is currently pressed
    pub fn is_alt_pressed(&self) -> bool {
        self.modifiers.contains(KeyModifiers::ALT)
    }

    /// Update key repeat handling
    pub fn update_repeat(&mut self, now: Instant) -> Vec<KeyCode> {
        let mut repeated_keys = Vec::new();

        for (&key, &press_time) in &self.key_press_times {
            if self.is_key_down(key) {
                let elapsed = now.duration_since(press_time);

                if let Some(&last_repeat) = self.repeating_keys.get(&key) {
                    // Key is already repeating
                    if now.duration_since(last_repeat) >= self.repeat_interval {
                        self.repeating_keys.insert(key, now);
                        repeated_keys.push(key);
                    }
                } else if elapsed >= self.repeat_delay {
                    // Start repeating
                    self.repeating_keys.insert(key, now);
                    repeated_keys.push(key);
                }
            }
        }

        repeated_keys
    }

    /// Update state for next frame (convert Just* states to stable states)
    pub fn update_frame(&mut self) {
        for state in self.key_states.values_mut() {
            match *state {
                KeyState::JustPressed => *state = KeyState::Pressed,
                KeyState::JustReleased => *state = KeyState::Released,
                _ => {}
            }
        }
    }

    /// Get all currently pressed keys
    pub fn pressed_keys(&self) -> Vec<KeyCode> {
        self.key_states
            .iter()
            .filter(|(_, state)| state.is_down())
            .map(|(&key, _)| key)
            .collect()
    }

    /// Reset all key states
    pub fn reset(&mut self) {
        self.key_states.clear();
        self.key_press_times.clear();
        self.repeating_keys.clear();
        self.modifiers = KeyModifiers::empty();
        self.last_key_pressed = None;
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Key name translation — matches C++ Keyboard::initKeyNames / getPrintableKey
// ---------------------------------------------------------------------------

/// Maximum key states in C++ (std, shifted, shifted2).
pub const MAX_KEY_STATES: usize = 3;

/// Entry in the key-name table.
///
/// C++ parity: `m_keyNames[z].stdKey`, `m_keyNames[z].shifted`,
/// `m_keyNames[z].shifted2`.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyNameEntry {
    pub std_key: char,
    pub shifted: char,
    pub shifted2: char,
}

/// Total number of key-code slots in the name table (matches C++ KEY_NAMES_COUNT).
pub const KEY_NAMES_COUNT: usize = 256;

/// Build the default US-layout key name table.
///
/// C++ parity: `Keyboard::initKeyNames()` for `LANGUAGE_ID_US`.
fn build_us_key_names() -> [KeyNameEntry; KEY_NAMES_COUNT] {
    let mut names = [KeyNameEntry::default(); KEY_NAMES_COUNT];

    let set = |table: &mut [KeyNameEntry; KEY_NAMES_COUNT],
               idx: usize,
               std: char,
               shifted: char,
               shifted2: char| {
        if idx < KEY_NAMES_COUNT {
            table[idx].std_key = std;
            table[idx].shifted = shifted;
            table[idx].shifted2 = shifted2;
        }
    };

    // --- navigation / special keys (no printable output) ---
    set(&mut names, KeyCode::Up as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Down as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Left as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Right as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Home as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::End as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::PageUp as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::PageDown as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Insert as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Delete as usize, '\u{8}', '\u{8}', '\0');
    set(
        &mut names,
        KeyCode::Backspace as usize,
        '\u{8}',
        '\u{8}',
        '\0',
    );
    set(&mut names, KeyCode::Escape as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Tab as usize, '\t', '\t', '\0');
    set(&mut names, KeyCode::CapsLock as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::Enter as usize, '\n', '\n', '\0');

    // modifiers
    set(&mut names, KeyCode::RightAlt as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::RightCtrl as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::RightShift as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::LeftAlt as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::LeftCtrl as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::LeftShift as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::NumLock as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::ScrollLock as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::PrintScreen as usize, '\0', '\0', '\0');

    // function keys
    set(&mut names, KeyCode::F1 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F2 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F3 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F4 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F5 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F6 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F7 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F8 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F9 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F10 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F11 as usize, '\0', '\0', '\0');
    set(&mut names, KeyCode::F12 as usize, '\0', '\0', '\0');

    // numpad digits
    set(&mut names, KeyCode::NumPad1 as usize, '1', '1', '\0');
    set(&mut names, KeyCode::NumPad2 as usize, '2', '2', '\0');
    set(&mut names, KeyCode::NumPad3 as usize, '3', '3', '\0');
    set(&mut names, KeyCode::NumPad4 as usize, '4', '4', '\0');
    set(&mut names, KeyCode::NumPad5 as usize, '5', '5', '\0');
    set(&mut names, KeyCode::NumPad6 as usize, '6', '6', '\0');
    set(&mut names, KeyCode::NumPad7 as usize, '7', '7', '\0');
    set(&mut names, KeyCode::NumPad8 as usize, '8', '8', '\0');
    set(&mut names, KeyCode::NumPad9 as usize, '9', '9', '\0');
    set(&mut names, KeyCode::NumPad0 as usize, '0', '0', '\0');

    // numpad operators
    set(&mut names, KeyCode::NumPadSubtract as usize, '-', '-', '\0');
    set(&mut names, KeyCode::NumPadAdd as usize, '+', '+', '\0');
    set(&mut names, KeyCode::NumPadEnter as usize, '\n', '\n', '\0');
    set(&mut names, KeyCode::NumPadDivide as usize, '/', '/', '\0');
    set(&mut names, KeyCode::NumPadDecimal as usize, '.', '.', '\0');
    set(&mut names, KeyCode::NumPadMultiply as usize, '*', '*', '\0');

    // space
    set(&mut names, KeyCode::Space as usize, ' ', ' ', '\0');

    // --- US-layout printable keys ---
    // letters
    set(&mut names, KeyCode::A as usize, 'a', 'A', '\0');
    set(&mut names, KeyCode::B as usize, 'b', 'B', '\0');
    set(&mut names, KeyCode::C as usize, 'c', 'C', '\0');
    set(&mut names, KeyCode::D as usize, 'd', 'D', '\0');
    set(&mut names, KeyCode::E as usize, 'e', 'E', '\0');
    set(&mut names, KeyCode::F as usize, 'f', 'F', '\0');
    set(&mut names, KeyCode::G as usize, 'g', 'G', '\0');
    set(&mut names, KeyCode::H as usize, 'h', 'H', '\0');
    set(&mut names, KeyCode::I as usize, 'i', 'I', '\0');
    set(&mut names, KeyCode::J as usize, 'j', 'J', '\0');
    set(&mut names, KeyCode::K as usize, 'k', 'K', '\0');
    set(&mut names, KeyCode::L as usize, 'l', 'L', '\0');
    set(&mut names, KeyCode::M as usize, 'm', 'M', '\0');
    set(&mut names, KeyCode::N as usize, 'n', 'N', '\0');
    set(&mut names, KeyCode::O as usize, 'o', 'O', '\0');
    set(&mut names, KeyCode::P as usize, 'p', 'P', '\0');
    set(&mut names, KeyCode::Q as usize, 'q', 'Q', '\0');
    set(&mut names, KeyCode::R as usize, 'r', 'R', '\0');
    set(&mut names, KeyCode::S as usize, 's', 'S', '\0');
    set(&mut names, KeyCode::T as usize, 't', 'T', '\0');
    set(&mut names, KeyCode::U as usize, 'u', 'U', '\0');
    set(&mut names, KeyCode::V as usize, 'v', 'V', '\0');
    set(&mut names, KeyCode::W as usize, 'w', 'W', '\0');
    set(&mut names, KeyCode::X as usize, 'x', 'X', '\0');
    set(&mut names, KeyCode::Y as usize, 'y', 'Y', '\0');
    set(&mut names, KeyCode::Z as usize, 'z', 'Z', '\0');

    // number row
    set(&mut names, KeyCode::Num1 as usize, '1', '!', '\0');
    set(&mut names, KeyCode::Num2 as usize, '2', '@', '\0');
    set(&mut names, KeyCode::Num3 as usize, '3', '#', '\0');
    set(&mut names, KeyCode::Num4 as usize, '4', '$', '\0');
    set(&mut names, KeyCode::Num5 as usize, '5', '%', '\0');
    set(&mut names, KeyCode::Num6 as usize, '6', '^', '\0');
    set(&mut names, KeyCode::Num7 as usize, '7', '&', '\0');
    set(&mut names, KeyCode::Num8 as usize, '8', '*', '\0');
    set(&mut names, KeyCode::Num9 as usize, '9', '(', '\0');
    set(&mut names, KeyCode::Num0 as usize, '0', ')', '\0');

    // punctuation
    set(&mut names, KeyCode::Comma as usize, ',', '<', '\0');
    set(&mut names, KeyCode::Period as usize, '.', '>', '\0');
    set(&mut names, KeyCode::Slash as usize, '/', '?', '\0');
    set(&mut names, KeyCode::LeftBracket as usize, '[', '{', '\0');
    set(&mut names, KeyCode::RightBracket as usize, ']', '}', '\0');
    set(&mut names, KeyCode::Semicolon as usize, ';', ':', '\0');
    set(&mut names, KeyCode::Quote as usize, '\'', '"', '\0');
    set(&mut names, KeyCode::Grave as usize, '`', '~', '\0');
    set(&mut names, KeyCode::Backslash as usize, '\\', '|', '\0');
    set(&mut names, KeyCode::Minus as usize, '-', '_', '\0');
    set(&mut names, KeyCode::Plus as usize, '=', '+', '\0');

    names
}

/// Lazy-initialized US key name table.
static US_KEY_NAMES: std::sync::OnceLock<[KeyNameEntry; KEY_NAMES_COUNT]> =
    std::sync::OnceLock::new();

fn us_key_names() -> &'static [KeyNameEntry; KEY_NAMES_COUNT] {
    US_KEY_NAMES.get_or_init(build_us_key_names)
}

/// Get the printable character for a key and state.
///
/// C++ parity: `WideChar Keyboard::getPrintableKey(UnsignedByte key, Int state)`.
///
/// * `state == 0` → `stdKey`
/// * `state == 1` → `shifted`
/// * `state == 2` → `shifted2`
///
/// Returns `'\0'` if the key has no printable representation or is out of range.
pub fn get_printable_key(key: KeyCode, state: usize) -> char {
    let idx = key as usize;
    if idx >= KEY_NAMES_COUNT || state >= MAX_KEY_STATES {
        return '\0';
    }
    let names = us_key_names();
    match state {
        0 => names[idx].std_key,
        1 => names[idx].shifted,
        2 => names[idx].shifted2,
        _ => '\0',
    }
}

/// Translate a key code to a printable character, respecting shift and caps-lock.
///
/// C++ parity: `WideChar Keyboard::translateKey(WideChar keyCode)`.
///
/// Returns `Some(char)` for printable keys, `None` for non-printable / modifier keys.
pub fn translate_key(key: KeyCode, shift: bool, caps_lock: bool) -> Option<char> {
    let idx = key as usize;
    if idx >= KEY_NAMES_COUNT {
        return None;
    }

    let names = us_key_names();
    let std = names[idx].std_key;

    // Modifier keys return None
    if std == '\0' {
        return None;
    }

    // C++ parity: if shift is held OR (caps-lock and the key is alphabetic),
    // return the shifted form.
    let is_alpha = std.is_ascii_alphabetic();
    if shift || (caps_lock && is_alpha) {
        let shifted = names[idx].shifted;
        if shifted != '\0' {
            return Some(shifted);
        }
    }

    Some(std)
}

/// Keyboard input handler
pub struct Keyboard {
    /// Current keyboard state
    state: KeyboardState,
    /// Input statistics
    stats: InputStats,
    /// Whether keyboard input is enabled
    enabled: bool,
    /// Text input buffer for character input
    text_input: String,
}

impl Keyboard {
    /// Create a new keyboard handler
    pub fn new() -> Self {
        Self {
            state: KeyboardState::new(),
            stats: InputStats::default(),
            enabled: true,
            text_input: String::new(),
        }
    }

    /// Enable or disable keyboard input
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.state.reset();
            self.text_input.clear();
        }
    }

    /// Check if keyboard input is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Process a winit keyboard event
    pub fn handle_key_event(&mut self, event: &KeyEvent, timestamp: Instant) -> Vec<KeyCode> {
        if !self.enabled {
            return Vec::new();
        }

        self.stats.keyboard_events += 1;
        self.stats.events_processed += 1;

        let mut events = Vec::new();

        if let PhysicalKey::Code(winit_key) = event.physical_key {
            let key = map_winit_key_code(winit_key, event.location);
            let pressed = event.state == ElementState::Pressed;

            // Update key state
            let old_state = self.state.key_state(key);
            self.state.update_key(key, pressed, timestamp);
            let new_state = self.state.key_state(key);

            // Generate events for state changes
            if old_state != new_state {
                events.push(key);
            }
        }

        events
    }

    /// Process text input
    pub fn handle_text_input(&mut self, text: &str) {
        if !self.enabled {
            return;
        }

        self.text_input.push_str(text);
        self.stats.keyboard_events += 1;
        self.stats.events_processed += 1;
    }

    /// Get and clear accumulated text input
    pub fn take_text_input(&mut self) -> String {
        std::mem::take(&mut self.text_input)
    }

    /// Get current keyboard state
    pub fn state(&self) -> &KeyboardState {
        &self.state
    }

    /// Get mutable keyboard state
    pub fn state_mut(&mut self) -> &mut KeyboardState {
        &mut self.state
    }

    /// Update keyboard state for current frame
    pub fn update(&mut self) -> Vec<KeyCode> {
        if !self.enabled {
            return Vec::new();
        }

        // Handle key repeat
        let repeated_keys = self.state.update_repeat(Instant::now());

        // Update frame state
        self.state.update_frame();

        repeated_keys
    }

    /// Get input statistics
    pub fn stats(&self) -> &InputStats {
        &self.stats
    }

    /// Reset input statistics
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }

    /// Configure key repeat settings
    pub fn set_key_repeat(&mut self, delay_ms: u32, interval_ms: u32) {
        self.state.set_repeat_config(
            Duration::from_millis(delay_ms as u64),
            Duration::from_millis(interval_ms as u64),
        );
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for Keyboard {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing Keyboard subsystem");
        self.enabled = true;
        self.stats.reset();
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting Keyboard subsystem");
        self.state.reset();
        self.text_input.clear();
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
    fn test_key_state_transitions() {
        let mut state = KeyboardState::new();
        let now = Instant::now();

        // Test press
        state.update_key(KeyCode::A, true, now);
        assert_eq!(state.key_state(KeyCode::A), KeyState::JustPressed);
        assert!(state.is_key_just_pressed(KeyCode::A));
        assert!(state.is_key_down(KeyCode::A));

        // Update frame
        state.update_frame();
        assert_eq!(state.key_state(KeyCode::A), KeyState::Pressed);
        assert!(!state.is_key_just_pressed(KeyCode::A));
        assert!(state.is_key_down(KeyCode::A));

        // Test release
        state.update_key(KeyCode::A, false, now);
        assert_eq!(state.key_state(KeyCode::A), KeyState::JustReleased);
        assert!(state.is_key_just_released(KeyCode::A));
        assert!(!state.is_key_down(KeyCode::A));

        // Update frame
        state.update_frame();
        assert_eq!(state.key_state(KeyCode::A), KeyState::Released);
        assert!(!state.is_key_just_released(KeyCode::A));
        assert!(!state.is_key_down(KeyCode::A));
    }

    #[test]
    fn test_modifier_tracking() {
        let mut state = KeyboardState::new();
        let now = Instant::now();

        // Test Ctrl key
        state.update_key(KeyCode::LeftCtrl, true, now);
        assert!(state.modifiers().contains(KeyModifiers::CTRL));

        state.update_key(KeyCode::LeftCtrl, false, now);
        assert!(!state.modifiers().contains(KeyModifiers::CTRL));

        // Test both shift keys
        state.update_key(KeyCode::LeftShift, true, now);
        assert!(state.modifiers().contains(KeyModifiers::SHIFT));

        state.update_key(KeyCode::RightShift, true, now);
        assert!(state.modifiers().contains(KeyModifiers::SHIFT));

        state.update_key(KeyCode::LeftShift, false, now);
        assert!(state.modifiers().contains(KeyModifiers::SHIFT)); // Right shift still down

        state.update_key(KeyCode::RightShift, false, now);
        assert!(!state.modifiers().contains(KeyModifiers::SHIFT)); // Both released
    }

    #[test]
    fn test_keyboard_creation() {
        let keyboard = Keyboard::new();
        assert!(keyboard.is_enabled());
        assert_eq!(keyboard.state().pressed_keys().len(), 0);
    }

    #[test]
    fn test_keyboard_enable_disable() {
        let mut keyboard = Keyboard::new();

        keyboard.set_enabled(false);
        assert!(!keyboard.is_enabled());

        keyboard.set_enabled(true);
        assert!(keyboard.is_enabled());
    }

    #[test]
    fn test_text_input() {
        let mut keyboard = Keyboard::new();

        keyboard.handle_text_input("Hello");
        keyboard.handle_text_input(" World");

        let text = keyboard.take_text_input();
        assert_eq!(text, "Hello World");

        // Text should be cleared after taking
        let text2 = keyboard.take_text_input();
        assert_eq!(text2, "");
    }

    #[test]
    fn test_keycode_conversion() {
        assert_eq!(KeyCode::from(WinitKeyCode::KeyA), KeyCode::A);
        assert_eq!(KeyCode::from(WinitKeyCode::F1), KeyCode::F1);
        assert_eq!(KeyCode::from(WinitKeyCode::Space), KeyCode::Space);
        assert_eq!(KeyCode::from(WinitKeyCode::Escape), KeyCode::Escape);
    }

    #[test]
    fn test_meta_key_location_mapping() {
        assert_eq!(
            map_winit_key_code(WinitKeyCode::Meta, KeyLocation::Left),
            KeyCode::LeftMeta
        );
        assert_eq!(
            map_winit_key_code(WinitKeyCode::Meta, KeyLocation::Right),
            KeyCode::RightMeta
        );
        assert_eq!(
            map_winit_key_code(WinitKeyCode::Meta, KeyLocation::Standard),
            KeyCode::LeftMeta
        );
    }

    #[test]
    fn test_get_printable_key_us_letters() {
        assert_eq!(get_printable_key(KeyCode::A, 0), 'a');
        assert_eq!(get_printable_key(KeyCode::A, 1), 'A');
        assert_eq!(get_printable_key(KeyCode::Z, 0), 'z');
        assert_eq!(get_printable_key(KeyCode::Z, 1), 'Z');
    }

    #[test]
    fn test_get_printable_key_us_numbers() {
        assert_eq!(get_printable_key(KeyCode::Num1, 0), '1');
        assert_eq!(get_printable_key(KeyCode::Num1, 1), '!');
        assert_eq!(get_printable_key(KeyCode::Num0, 0), '0');
        assert_eq!(get_printable_key(KeyCode::Num0, 1), ')');
    }

    #[test]
    fn test_get_printable_key_us_punctuation() {
        assert_eq!(get_printable_key(KeyCode::Comma, 0), ',');
        assert_eq!(get_printable_key(KeyCode::Comma, 1), '<');
        assert_eq!(get_printable_key(KeyCode::Semicolon, 0), ';');
        assert_eq!(get_printable_key(KeyCode::Semicolon, 1), ':');
        assert_eq!(get_printable_key(KeyCode::Minus, 0), '-');
        assert_eq!(get_printable_key(KeyCode::Minus, 1), '_');
        assert_eq!(get_printable_key(KeyCode::Plus, 0), '=');
        assert_eq!(get_printable_key(KeyCode::Plus, 1), '+');
    }

    #[test]
    fn test_get_printable_key_numpad() {
        assert_eq!(get_printable_key(KeyCode::NumPad5, 0), '5');
        assert_eq!(get_printable_key(KeyCode::NumPadAdd, 0), '+');
        assert_eq!(get_printable_key(KeyCode::NumPadEnter, 0), '\n');
    }

    #[test]
    fn test_get_printable_key_special_keys() {
        assert_eq!(get_printable_key(KeyCode::Space, 0), ' ');
        assert_eq!(get_printable_key(KeyCode::Tab, 0), '\t');
        assert_eq!(get_printable_key(KeyCode::Enter, 0), '\n');
    }

    #[test]
    fn test_get_printable_key_non_printable() {
        assert_eq!(get_printable_key(KeyCode::F1, 0), '\0');
        assert_eq!(get_printable_key(KeyCode::Escape, 0), '\0');
        assert_eq!(get_printable_key(KeyCode::LeftShift, 0), '\0');
        assert_eq!(get_printable_key(KeyCode::Up, 0), '\0');
    }

    #[test]
    fn test_translate_key_no_modifiers() {
        assert_eq!(translate_key(KeyCode::A, false, false), Some('a'));
        assert_eq!(translate_key(KeyCode::Num1, false, false), Some('1'));
        assert_eq!(translate_key(KeyCode::Space, false, false), Some(' '));
    }

    #[test]
    fn test_translate_key_shift() {
        assert_eq!(translate_key(KeyCode::A, true, false), Some('A'));
        assert_eq!(translate_key(KeyCode::Num1, true, false), Some('!'));
        assert_eq!(translate_key(KeyCode::Comma, true, false), Some('<'));
    }

    #[test]
    fn test_translate_key_caps_lock() {
        assert_eq!(translate_key(KeyCode::A, false, true), Some('A'));
        assert_eq!(translate_key(KeyCode::Num1, false, true), Some('1'));
    }

    #[test]
    fn test_translate_key_non_printable() {
        assert_eq!(translate_key(KeyCode::F1, false, false), None);
        assert_eq!(translate_key(KeyCode::Escape, false, false), None);
        assert_eq!(translate_key(KeyCode::LeftShift, false, false), None);
    }
}
