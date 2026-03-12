//! Hotkey system for keyboard shortcuts

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{InputEvent, KeyCode, ModifierKeys};

/// A hotkey definition combining keys and modifiers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hotkey {
    /// Main key
    pub key: KeyCode,

    /// Required modifiers
    pub modifiers: ModifierKeys,

    /// Whether all modifiers must match exactly
    pub exact_modifiers: bool,
}

impl Hotkey {
    /// Create a new hotkey
    pub fn new(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: ModifierKeys::empty(),
            exact_modifiers: false,
        }
    }

    /// Create a hotkey with modifiers
    pub fn with_modifiers(key: KeyCode, modifiers: ModifierKeys) -> Self {
        Self {
            key,
            modifiers,
            exact_modifiers: true,
        }
    }

    /// Add Ctrl modifier
    pub fn ctrl(mut self) -> Self {
        self.modifiers.insert(ModifierKeys::CTRL);
        self
    }

    /// Add Shift modifier
    pub fn shift(mut self) -> Self {
        self.modifiers.insert(ModifierKeys::SHIFT);
        self
    }

    /// Add Alt modifier
    pub fn alt(mut self) -> Self {
        self.modifiers.insert(ModifierKeys::ALT);
        self
    }

    /// Add Meta modifier
    pub fn meta(mut self) -> Self {
        self.modifiers.insert(ModifierKeys::META);
        self
    }

    /// Set exact modifier matching
    pub fn exact(mut self) -> Self {
        self.exact_modifiers = true;
        self
    }

    /// Check if this hotkey matches the given key and modifiers
    pub fn matches(&self, key: KeyCode, modifiers: ModifierKeys) -> bool {
        if self.key != key {
            return false;
        }

        if self.exact_modifiers {
            // All modifiers must match exactly
            self.modifiers == modifiers
        } else {
            // Required modifiers must be present (but others can be too)
            modifiers.contains(self.modifiers)
        }
    }

    /// Get display string for this hotkey
    pub fn display_string(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.contains(ModifierKeys::CTRL) {
            parts.push("Ctrl");
        }
        if self.modifiers.contains(ModifierKeys::SHIFT) {
            parts.push("Shift");
        }
        if self.modifiers.contains(ModifierKeys::ALT) {
            parts.push("Alt");
        }
        if self.modifiers.contains(ModifierKeys::META) {
            #[cfg(target_os = "macos")]
            parts.push("Cmd");
            #[cfg(not(target_os = "macos"))]
            parts.push("Win");
        }

        parts.push(self.key.name());

        parts.join("+")
    }

    /// Parse a hotkey from string (e.g., "Ctrl+Shift+A")
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();

        if parts.is_empty() {
            return None;
        }

        let mut modifiers = ModifierKeys::empty();
        let mut key = None;

        for part in parts {
            match part.to_uppercase().as_str() {
                "CTRL" | "CONTROL" => modifiers.insert(ModifierKeys::CTRL),
                "SHIFT" => modifiers.insert(ModifierKeys::SHIFT),
                "ALT" => modifiers.insert(ModifierKeys::ALT),
                "META" | "WIN" | "CMD" | "SUPER" => modifiers.insert(ModifierKeys::META),
                _ => {
                    if let Some(k) = KeyCode::from_name(part) {
                        key = Some(k);
                    }
                }
            }
        }

        key.map(|k| Self::with_modifiers(k, modifiers))
    }
}

/// Hotkey trigger conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyTrigger {
    /// Trigger on key press
    Press,
    /// Trigger on key release
    Release,
    /// Trigger on both press and release
    Both,
}

/// Hotkey registration entry
#[derive(Debug, Clone)]
struct HotkeyEntry {
    hotkey: Hotkey,
    trigger: HotkeyTrigger,
    enabled: bool,
}

/// Hotkey manager for registering and checking hotkeys
pub struct HotkeyManager {
    /// Registered hotkeys by name
    hotkeys: HashMap<String, HotkeyEntry>,

    /// Last triggered hotkey (for preventing double triggers)
    last_triggered: Option<(String, std::time::Instant)>,
}

impl HotkeyManager {
    /// Create a new hotkey manager
    pub fn new() -> Self {
        Self {
            hotkeys: HashMap::new(),
            last_triggered: None,
        }
    }

    /// Register a hotkey
    pub fn register(&mut self, name: String, hotkey: Hotkey) {
        self.register_with_trigger(name, hotkey, HotkeyTrigger::Press);
    }

    /// Register a hotkey with custom trigger
    pub fn register_with_trigger(&mut self, name: String, hotkey: Hotkey, trigger: HotkeyTrigger) {
        self.hotkeys.insert(
            name,
            HotkeyEntry {
                hotkey,
                trigger,
                enabled: true,
            },
        );
    }

    /// Unregister a hotkey
    pub fn unregister(&mut self, name: &str) {
        self.hotkeys.remove(name);
    }

    /// Enable a hotkey
    pub fn enable(&mut self, name: &str) {
        if let Some(entry) = self.hotkeys.get_mut(name) {
            entry.enabled = true;
        }
    }

    /// Disable a hotkey
    pub fn disable(&mut self, name: &str) {
        if let Some(entry) = self.hotkeys.get_mut(name) {
            entry.enabled = false;
        }
    }

    /// Check if an event triggers any hotkey
    pub fn check_event(&mut self, event: &InputEvent) -> Option<String> {
        let (key, modifiers, is_press) = match event {
            InputEvent::KeyPressed { key, modifiers, .. } => (*key, *modifiers, true),
            InputEvent::KeyReleased { key, modifiers, .. } => (*key, *modifiers, false),
            _ => return None,
        };

        // Check each registered hotkey
        for (name, entry) in &self.hotkeys {
            if !entry.enabled {
                continue;
            }

            // Check if hotkey matches
            if !entry.hotkey.matches(key, modifiers) {
                continue;
            }

            // Check trigger condition
            let should_trigger = match entry.trigger {
                HotkeyTrigger::Press => is_press,
                HotkeyTrigger::Release => !is_press,
                HotkeyTrigger::Both => true,
            };

            if !should_trigger {
                continue;
            }

            // Prevent double triggers
            if let Some((last_name, last_time)) = &self.last_triggered {
                if last_name == name && last_time.elapsed().as_millis() < 100 {
                    continue;
                }
            }

            // Trigger this hotkey
            self.last_triggered = Some((name.clone(), std::time::Instant::now()));
            return Some(name.clone());
        }

        None
    }

    /// Get all registered hotkeys
    pub fn list_hotkeys(&self) -> Vec<(String, Hotkey, bool)> {
        self.hotkeys
            .iter()
            .map(|(name, entry)| (name.clone(), entry.hotkey.clone(), entry.enabled))
            .collect()
    }

    /// Clear all hotkeys
    pub fn clear(&mut self) {
        self.hotkeys.clear();
        self.last_triggered = None;
    }

    /// Check if a hotkey is registered
    pub fn is_registered(&self, name: &str) -> bool {
        self.hotkeys.contains_key(name)
    }

    /// Check if a hotkey is enabled
    pub fn is_enabled(&self, name: &str) -> bool {
        self.hotkeys
            .get(name)
            .map(|entry| entry.enabled)
            .unwrap_or(false)
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Common hotkey presets
pub mod presets {
    use super::*;

    /// Create common editor hotkeys
    pub fn editor_hotkeys() -> Vec<(&'static str, Hotkey)> {
        vec![
            ("save", Hotkey::new(KeyCode::S).ctrl()),
            ("save_as", Hotkey::new(KeyCode::S).ctrl().shift()),
            ("open", Hotkey::new(KeyCode::O).ctrl()),
            ("new", Hotkey::new(KeyCode::N).ctrl()),
            ("close", Hotkey::new(KeyCode::W).ctrl()),
            ("quit", Hotkey::new(KeyCode::Q).ctrl()),
            ("undo", Hotkey::new(KeyCode::Z).ctrl()),
            ("redo", Hotkey::new(KeyCode::Y).ctrl()),
            ("cut", Hotkey::new(KeyCode::X).ctrl()),
            ("copy", Hotkey::new(KeyCode::C).ctrl()),
            ("paste", Hotkey::new(KeyCode::V).ctrl()),
            ("select_all", Hotkey::new(KeyCode::A).ctrl()),
            ("find", Hotkey::new(KeyCode::F).ctrl()),
            ("replace", Hotkey::new(KeyCode::H).ctrl()),
        ]
    }

    /// Create common game hotkeys
    pub fn game_hotkeys() -> Vec<(&'static str, Hotkey)> {
        vec![
            ("pause", Hotkey::new(KeyCode::Escape)),
            ("quicksave", Hotkey::new(KeyCode::F5)),
            ("quickload", Hotkey::new(KeyCode::F9)),
            ("screenshot", Hotkey::new(KeyCode::F12)),
            ("toggle_console", Hotkey::new(KeyCode::Grave)),
            ("toggle_fullscreen", Hotkey::new(KeyCode::Enter).alt()),
            ("toggle_fps", Hotkey::new(KeyCode::F)),
        ]
    }

    /// Create RTS-specific hotkeys
    pub fn rts_hotkeys() -> Vec<(&'static str, Hotkey)> {
        vec![
            ("select_all_units", Hotkey::new(KeyCode::A).ctrl()),
            ("stop", Hotkey::new(KeyCode::S)),
            ("attack_move", Hotkey::new(KeyCode::A)),
            ("patrol", Hotkey::new(KeyCode::P)),
            ("guard", Hotkey::new(KeyCode::G)),
            ("group_0", Hotkey::new(KeyCode::Num0).ctrl()),
            ("group_1", Hotkey::new(KeyCode::Num1).ctrl()),
            ("group_2", Hotkey::new(KeyCode::Num2).ctrl()),
            ("group_3", Hotkey::new(KeyCode::Num3).ctrl()),
            ("group_4", Hotkey::new(KeyCode::Num4).ctrl()),
            ("select_group_0", Hotkey::new(KeyCode::Num0)),
            ("select_group_1", Hotkey::new(KeyCode::Num1)),
            ("select_group_2", Hotkey::new(KeyCode::Num2)),
            ("select_group_3", Hotkey::new(KeyCode::Num3)),
            ("select_group_4", Hotkey::new(KeyCode::Num4)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_creation() {
        let hotkey = Hotkey::new(KeyCode::A).ctrl().shift();
        assert!(hotkey.modifiers.contains(ModifierKeys::CTRL));
        assert!(hotkey.modifiers.contains(ModifierKeys::SHIFT));
    }

    #[test]
    fn test_hotkey_matching() {
        let hotkey = Hotkey::new(KeyCode::S).ctrl();

        assert!(hotkey.matches(KeyCode::S, ModifierKeys::CTRL));
        assert!(!hotkey.matches(KeyCode::A, ModifierKeys::CTRL));
        assert!(!hotkey.matches(KeyCode::S, ModifierKeys::SHIFT));
    }

    #[test]
    fn test_hotkey_display() {
        let hotkey = Hotkey::new(KeyCode::S).ctrl().shift();
        let display = hotkey.display_string();
        assert!(display.contains("Ctrl"));
        assert!(display.contains("Shift"));
        assert!(display.contains("S"));
    }

    #[test]
    fn test_hotkey_from_string() {
        let hotkey = Hotkey::from_string("Ctrl+Shift+A").unwrap();
        assert_eq!(hotkey.key, KeyCode::A);
        assert!(hotkey.modifiers.contains(ModifierKeys::CTRL));
        assert!(hotkey.modifiers.contains(ModifierKeys::SHIFT));
    }

    #[test]
    fn test_hotkey_manager() {
        let mut manager = HotkeyManager::new();

        let hotkey = Hotkey::new(KeyCode::S).ctrl();
        manager.register("save".into(), hotkey);

        assert!(manager.is_registered("save"));
        assert!(manager.is_enabled("save"));

        manager.disable("save");
        assert!(!manager.is_enabled("save"));

        manager.enable("save");
        assert!(manager.is_enabled("save"));

        manager.unregister("save");
        assert!(!manager.is_registered("save"));
    }

    #[test]
    fn test_hotkey_triggering() {
        let mut manager = HotkeyManager::new();

        let hotkey = Hotkey::new(KeyCode::S).ctrl();
        manager.register("save".into(), hotkey);

        let event = InputEvent::KeyPressed {
            key: KeyCode::S,
            modifiers: ModifierKeys::CTRL,
            timestamp: std::time::Duration::from_secs(0),
        };

        let triggered = manager.check_event(&event);
        assert_eq!(triggered, Some("save".into()));
    }
}
