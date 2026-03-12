//! Keyboard device abstraction with cross-platform support

use std::collections::HashSet;
use std::time::{Duration, Instant};

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use super::Result;

bitflags! {
    /// Keyboard modifier keys
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ModifierKeys: u8 {
        const SHIFT = 0b0000_0001;
        const CTRL  = 0b0000_0010;
        const ALT   = 0b0000_0100;
        const META  = 0b0000_1000; // Windows/Command key
    }
}

// Manual Serialize/Deserialize implementation for ModifierKeys
impl Serialize for ModifierKeys {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(self.bits())
    }
}

impl<'de> Deserialize<'de> for ModifierKeys {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = u8::deserialize(deserializer)?;
        Ok(ModifierKeys::from_bits_truncate(bits))
    }
}

/// Virtual key codes matching common platform APIs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum KeyCode {
    // Alphanumeric keys
    A = 0x41,
    B = 0x42,
    C = 0x43,
    D = 0x44,
    E = 0x45,
    F = 0x46,
    G = 0x47,
    H = 0x48,
    I = 0x49,
    J = 0x4A,
    K = 0x4B,
    L = 0x4C,
    M = 0x4D,
    N = 0x4E,
    O = 0x4F,
    P = 0x50,
    Q = 0x51,
    R = 0x52,
    S = 0x53,
    T = 0x54,
    U = 0x55,
    V = 0x56,
    W = 0x57,
    X = 0x58,
    Y = 0x59,
    Z = 0x5A,

    // Number keys
    Num0 = 0x30,
    Num1 = 0x31,
    Num2 = 0x32,
    Num3 = 0x33,
    Num4 = 0x34,
    Num5 = 0x35,
    Num6 = 0x36,
    Num7 = 0x37,
    Num8 = 0x38,
    Num9 = 0x39,

    // Function keys
    F1 = 0x70,
    F2 = 0x71,
    F3 = 0x72,
    F4 = 0x73,
    F5 = 0x74,
    F6 = 0x75,
    F7 = 0x76,
    F8 = 0x77,
    F9 = 0x78,
    F10 = 0x79,
    F11 = 0x7A,
    F12 = 0x7B,

    // Special keys
    Escape = 0x1B,
    Tab = 0x09,
    CapsLock = 0x14,
    LeftShift = 0xA0,
    RightShift = 0xA1,
    LeftCtrl = 0xA2,
    RightCtrl = 0xA3,
    LeftAlt = 0xA4,
    RightAlt = 0xA5,
    LeftMeta = 0x5B, // Windows/Command
    RightMeta = 0x5C,
    Space = 0x20,
    Enter = 0x0D,
    Backspace = 0x08,
    Delete = 0x2E,
    Insert = 0x2D,

    // Arrow keys
    Left = 0x25,
    Up = 0x26,
    Right = 0x27,
    Down = 0x28,

    // Navigation
    Home = 0x24,
    End = 0x23,
    PageUp = 0x21,
    PageDown = 0x22,

    // Numpad
    Numpad0 = 0x60,
    Numpad1 = 0x61,
    Numpad2 = 0x62,
    Numpad3 = 0x63,
    Numpad4 = 0x64,
    Numpad5 = 0x65,
    Numpad6 = 0x66,
    Numpad7 = 0x67,
    Numpad8 = 0x68,
    Numpad9 = 0x69,
    NumpadMultiply = 0x6A,
    NumpadAdd = 0x6B,
    NumpadSubtract = 0x6D,
    NumpadDecimal = 0x6E,
    NumpadDivide = 0x6F,
    NumLock = 0x90,

    // Punctuation
    Semicolon = 0xBA,
    Equal = 0xBB,
    Comma = 0xBC,
    Minus = 0xBD,
    Period = 0xBE,
    Slash = 0xBF,
    Grave = 0xC0,
    LeftBracket = 0xDB,
    Backslash = 0xDC,
    RightBracket = 0xDD,
    Quote = 0xDE,

    // Media keys
    VolumeMute = 0xAD,
    VolumeDown = 0xAE,
    VolumeUp = 0xAF,
    MediaNextTrack = 0xB0,
    MediaPrevTrack = 0xB1,
    MediaStop = 0xB2,
    MediaPlayPause = 0xB3,

    // Lock keys
    ScrollLock = 0x91,

    // Other
    PrintScreen = 0x2C,
    Pause = 0x13,
}

impl KeyCode {
    /// Check if this key is a modifier key
    pub fn is_modifier(&self) -> bool {
        matches!(
            self,
            KeyCode::LeftShift
                | KeyCode::RightShift
                | KeyCode::LeftCtrl
                | KeyCode::RightCtrl
                | KeyCode::LeftAlt
                | KeyCode::RightAlt
                | KeyCode::LeftMeta
                | KeyCode::RightMeta
        )
    }

    /// Get the display name for this key
    pub fn name(&self) -> &'static str {
        match self {
            KeyCode::A => "A",
            KeyCode::B => "B",
            KeyCode::C => "C",
            KeyCode::D => "D",
            KeyCode::E => "E",
            KeyCode::F => "F",
            KeyCode::G => "G",
            KeyCode::H => "H",
            KeyCode::I => "I",
            KeyCode::J => "J",
            KeyCode::K => "K",
            KeyCode::L => "L",
            KeyCode::M => "M",
            KeyCode::N => "N",
            KeyCode::O => "O",
            KeyCode::P => "P",
            KeyCode::Q => "Q",
            KeyCode::R => "R",
            KeyCode::S => "S",
            KeyCode::T => "T",
            KeyCode::U => "U",
            KeyCode::V => "V",
            KeyCode::W => "W",
            KeyCode::X => "X",
            KeyCode::Y => "Y",
            KeyCode::Z => "Z",
            KeyCode::Num0 => "0",
            KeyCode::Num1 => "1",
            KeyCode::Num2 => "2",
            KeyCode::Num3 => "3",
            KeyCode::Num4 => "4",
            KeyCode::Num5 => "5",
            KeyCode::Num6 => "6",
            KeyCode::Num7 => "7",
            KeyCode::Num8 => "8",
            KeyCode::Num9 => "9",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::Escape => "Escape",
            KeyCode::Tab => "Tab",
            KeyCode::CapsLock => "CapsLock",
            KeyCode::LeftShift => "Left Shift",
            KeyCode::RightShift => "Right Shift",
            KeyCode::LeftCtrl => "Left Ctrl",
            KeyCode::RightCtrl => "Right Ctrl",
            KeyCode::LeftAlt => "Left Alt",
            KeyCode::RightAlt => "Right Alt",
            KeyCode::LeftMeta => "Left Meta",
            KeyCode::RightMeta => "Right Meta",
            KeyCode::Space => "Space",
            KeyCode::Enter => "Enter",
            KeyCode::Backspace => "Backspace",
            KeyCode::Delete => "Delete",
            KeyCode::Insert => "Insert",
            KeyCode::Left => "Left",
            KeyCode::Up => "Up",
            KeyCode::Right => "Right",
            KeyCode::Down => "Down",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "Page Up",
            KeyCode::PageDown => "Page Down",
            _ => "Unknown",
        }
    }

    /// Try to parse a key from a string name
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "A" => Some(KeyCode::A),
            "B" => Some(KeyCode::B),
            "C" => Some(KeyCode::C),
            "D" => Some(KeyCode::D),
            "E" => Some(KeyCode::E),
            "F" => Some(KeyCode::F),
            "G" => Some(KeyCode::G),
            "H" => Some(KeyCode::H),
            "I" => Some(KeyCode::I),
            "J" => Some(KeyCode::J),
            "K" => Some(KeyCode::K),
            "L" => Some(KeyCode::L),
            "M" => Some(KeyCode::M),
            "N" => Some(KeyCode::N),
            "O" => Some(KeyCode::O),
            "P" => Some(KeyCode::P),
            "Q" => Some(KeyCode::Q),
            "R" => Some(KeyCode::R),
            "S" => Some(KeyCode::S),
            "T" => Some(KeyCode::T),
            "U" => Some(KeyCode::U),
            "V" => Some(KeyCode::V),
            "W" => Some(KeyCode::W),
            "X" => Some(KeyCode::X),
            "Y" => Some(KeyCode::Y),
            "Z" => Some(KeyCode::Z),
            "SPACE" => Some(KeyCode::Space),
            "ENTER" => Some(KeyCode::Enter),
            "ESCAPE" | "ESC" => Some(KeyCode::Escape),
            "TAB" => Some(KeyCode::Tab),
            _ => None,
        }
    }
}

/// Keyboard state tracking individual key states
#[derive(Debug, Clone)]
pub struct KeyboardState {
    /// Currently pressed keys
    pressed_keys: HashSet<KeyCode>,

    /// Current modifier state
    modifiers: ModifierKeys,

    /// Key press timestamps for repeat handling
    key_press_times: std::collections::HashMap<KeyCode, Instant>,

    /// Last repeat times for each key
    last_repeat_times: std::collections::HashMap<KeyCode, Instant>,
}

impl KeyboardState {
    /// Create a new keyboard state
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            modifiers: ModifierKeys::empty(),
            key_press_times: std::collections::HashMap::new(),
            last_repeat_times: std::collections::HashMap::new(),
        }
    }

    /// Check if a key is currently pressed
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    /// Check if any key is pressed
    pub fn any_key_pressed(&self) -> bool {
        !self.pressed_keys.is_empty()
    }

    /// Get all currently pressed keys
    pub fn pressed_keys(&self) -> impl Iterator<Item = &KeyCode> {
        self.pressed_keys.iter()
    }

    /// Get current modifier state
    pub fn modifiers(&self) -> ModifierKeys {
        self.modifiers
    }

    /// Check if specific modifiers are active
    pub fn has_modifiers(&self, modifiers: ModifierKeys) -> bool {
        self.modifiers.contains(modifiers)
    }

    /// Mark a key as pressed
    fn press_key(&mut self, key: KeyCode) {
        let now = Instant::now();
        self.pressed_keys.insert(key);
        self.key_press_times.insert(key, now);

        // Update modifiers
        if key.is_modifier() {
            self.update_modifiers(key, true);
        }
    }

    /// Mark a key as released
    fn release_key(&mut self, key: KeyCode) {
        self.pressed_keys.remove(&key);
        self.key_press_times.remove(&key);
        self.last_repeat_times.remove(&key);

        // Update modifiers
        if key.is_modifier() {
            self.update_modifiers(key, false);
        }
    }

    /// Update modifier state based on key
    fn update_modifiers(&mut self, key: KeyCode, pressed: bool) {
        match key {
            KeyCode::LeftShift | KeyCode::RightShift => {
                if pressed {
                    self.modifiers.insert(ModifierKeys::SHIFT);
                } else if !self.is_key_pressed(KeyCode::LeftShift)
                    && !self.is_key_pressed(KeyCode::RightShift)
                {
                    self.modifiers.remove(ModifierKeys::SHIFT);
                }
            }
            KeyCode::LeftCtrl | KeyCode::RightCtrl => {
                if pressed {
                    self.modifiers.insert(ModifierKeys::CTRL);
                } else if !self.is_key_pressed(KeyCode::LeftCtrl)
                    && !self.is_key_pressed(KeyCode::RightCtrl)
                {
                    self.modifiers.remove(ModifierKeys::CTRL);
                }
            }
            KeyCode::LeftAlt | KeyCode::RightAlt => {
                if pressed {
                    self.modifiers.insert(ModifierKeys::ALT);
                } else if !self.is_key_pressed(KeyCode::LeftAlt)
                    && !self.is_key_pressed(KeyCode::RightAlt)
                {
                    self.modifiers.remove(ModifierKeys::ALT);
                }
            }
            KeyCode::LeftMeta | KeyCode::RightMeta => {
                if pressed {
                    self.modifiers.insert(ModifierKeys::META);
                } else if !self.is_key_pressed(KeyCode::LeftMeta)
                    && !self.is_key_pressed(KeyCode::RightMeta)
                {
                    self.modifiers.remove(ModifierKeys::META);
                }
            }
            _ => {}
        }
    }

    /// Clear all keyboard state
    fn clear(&mut self) {
        self.pressed_keys.clear();
        self.modifiers = ModifierKeys::empty();
        self.key_press_times.clear();
        self.last_repeat_times.clear();
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

/// Keyboard device with key repeat support
pub struct KeyboardDevice {
    /// Current keyboard state
    state: KeyboardState,

    /// Key repeat delay in milliseconds
    repeat_delay: Duration,

    /// Key repeat rate in milliseconds
    repeat_rate: Duration,
}

impl KeyboardDevice {
    /// Create a new keyboard device
    pub fn new(repeat_delay_ms: u64, repeat_rate_ms: u64) -> Result<Self> {
        Ok(Self {
            state: KeyboardState::new(),
            repeat_delay: Duration::from_millis(repeat_delay_ms),
            repeat_rate: Duration::from_millis(repeat_rate_ms),
        })
    }

    /// Get current keyboard state
    pub fn state(&self) -> KeyboardState {
        self.state.clone()
    }

    /// Handle key press
    pub fn handle_key_press(&mut self, key: KeyCode) {
        self.state.press_key(key);
    }

    /// Handle key release
    pub fn handle_key_release(&mut self, key: KeyCode) {
        self.state.release_key(key);
    }

    /// Update keyboard state (handle key repeats)
    pub fn update(&mut self, _delta_time: Duration) -> Vec<KeyCode> {
        let now = Instant::now();
        let mut repeat_keys = Vec::new();

        for key in self.state.pressed_keys.clone() {
            if let Some(&press_time) = self.state.key_press_times.get(&key) {
                let time_since_press = now.duration_since(press_time);

                // Check if we should start repeating
                if time_since_press >= self.repeat_delay {
                    let last_repeat = self
                        .state
                        .last_repeat_times
                        .get(&key)
                        .copied()
                        .unwrap_or(press_time + self.repeat_delay);

                    let time_since_repeat = now.duration_since(last_repeat);

                    // Check if it's time for another repeat
                    if time_since_repeat >= self.repeat_rate {
                        repeat_keys.push(key);
                        self.state.last_repeat_times.insert(key, now);
                    }
                }
            }
        }

        repeat_keys
    }

    /// Check if a key is pressed
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.state.is_key_pressed(key)
    }

    /// Get current modifiers
    pub fn modifiers(&self) -> ModifierKeys {
        self.state.modifiers()
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.state.clear();
    }

    /// Set repeat delay
    pub fn set_repeat_delay(&mut self, delay_ms: u64) {
        self.repeat_delay = Duration::from_millis(delay_ms);
    }

    /// Set repeat rate
    pub fn set_repeat_rate(&mut self, rate_ms: u64) {
        self.repeat_rate = Duration::from_millis(rate_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_state() {
        let mut state = KeyboardState::new();

        assert!(!state.is_key_pressed(KeyCode::A));
        assert!(!state.any_key_pressed());

        state.press_key(KeyCode::A);
        assert!(state.is_key_pressed(KeyCode::A));
        assert!(state.any_key_pressed());

        state.release_key(KeyCode::A);
        assert!(!state.is_key_pressed(KeyCode::A));
        assert!(!state.any_key_pressed());
    }

    #[test]
    fn test_modifiers() {
        let mut state = KeyboardState::new();

        state.press_key(KeyCode::LeftCtrl);
        assert!(state.has_modifiers(ModifierKeys::CTRL));

        state.press_key(KeyCode::LeftShift);
        assert!(state.has_modifiers(ModifierKeys::CTRL | ModifierKeys::SHIFT));

        state.release_key(KeyCode::LeftCtrl);
        assert!(!state.has_modifiers(ModifierKeys::CTRL));
        assert!(state.has_modifiers(ModifierKeys::SHIFT));
    }

    #[test]
    fn test_key_names() {
        assert_eq!(KeyCode::A.name(), "A");
        assert_eq!(KeyCode::Space.name(), "Space");
        assert_eq!(KeyCode::Enter.name(), "Enter");
    }

    #[test]
    fn test_key_from_name() {
        assert_eq!(KeyCode::from_name("A"), Some(KeyCode::A));
        assert_eq!(KeyCode::from_name("space"), Some(KeyCode::Space));
        assert_eq!(KeyCode::from_name("ENTER"), Some(KeyCode::Enter));
        assert_eq!(KeyCode::from_name("invalid"), None);
    }

    #[test]
    fn test_keyboard_device() {
        let device = KeyboardDevice::new(500, 33);
        assert!(device.is_ok());

        let mut device = device.unwrap();
        assert!(!device.is_key_pressed(KeyCode::A));

        device.handle_key_press(KeyCode::A);
        assert!(device.is_key_pressed(KeyCode::A));

        device.handle_key_release(KeyCode::A);
        assert!(!device.is_key_pressed(KeyCode::A));
    }
}
