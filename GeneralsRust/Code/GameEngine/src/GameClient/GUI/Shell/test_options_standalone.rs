// Standalone test file for options_menu and keyboard_options_menu
// Run with: rustc --test test_options_standalone.rs && ./test_options_standalone

#[cfg(test)]
mod options_menu_tests {
    use std::collections::HashMap;

    // Inline the necessary types for standalone testing
    pub struct OptionPreferences {
        preferences: HashMap<String, String>,
        filename: String,
    }

    impl OptionPreferences {
        pub fn new() -> Self {
            Self {
                preferences: HashMap::new(),
                filename: "Options.ini".to_string(),
            }
        }

        pub fn get_scroll_factor(&self) -> f32 {
            if let Some(value) = self.preferences.get("ScrollFactor") {
                let mut factor = value.parse::<i32>().unwrap_or(50);
                if factor < 0 {
                    factor = 0;
                }
                if factor > 100 {
                    factor = 100;
                }
                factor as f32 / 100.0
            } else {
                0.5
            }
        }

        pub fn get_gamma_value(&self) -> f32 {
            if let Some(value) = self.preferences.get("Gamma") {
                value.parse::<f32>().unwrap_or(50.0)
            } else {
                50.0
            }
        }

        pub fn get_particle_cap(&self) -> i32 {
            if let Some(value) = self.preferences.get("MaxParticleCount") {
                let mut factor = value.parse::<i32>().unwrap_or(5000);
                if factor < 100 {
                    factor = 100;
                }
                factor
            } else {
                5000
            }
        }
    }

    #[test]
    fn test_scroll_factor() {
        let mut prefs = OptionPreferences::new();
        prefs.preferences.insert("ScrollFactor".to_string(), "50".to_string());
        assert_eq!(prefs.get_scroll_factor(), 0.5);

        prefs.preferences.insert("ScrollFactor".to_string(), "150".to_string());
        assert_eq!(prefs.get_scroll_factor(), 1.0);

        prefs.preferences.insert("ScrollFactor".to_string(), "-10".to_string());
        assert_eq!(prefs.get_scroll_factor(), 0.0);
    }

    #[test]
    fn test_particle_cap_clamping() {
        let mut prefs = OptionPreferences::new();
        prefs.preferences.insert("MaxParticleCount".to_string(), "50".to_string());
        assert_eq!(prefs.get_particle_cap(), 100);

        prefs.preferences.insert("MaxParticleCount".to_string(), "5000".to_string());
        assert_eq!(prefs.get_particle_cap(), 5000);
    }
}

#[cfg(test)]
mod keyboard_menu_tests {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct KeyModifiers {
        pub shift: bool,
        pub ctrl: bool,
        pub alt: bool,
    }

    impl KeyModifiers {
        pub fn new() -> Self {
            Self {
                shift: false,
                ctrl: false,
                alt: false,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct HotkeyBinding {
        pub key: String,
        pub modifiers: KeyModifiers,
    }

    impl HotkeyBinding {
        pub fn new(key: String, modifiers: KeyModifiers) -> Self {
            Self { key, modifiers }
        }

        pub fn to_display_string(&self) -> String {
            let mut result = String::new();
            if self.modifiers.alt {
                result.push_str("Alt+");
            }
            if self.modifiers.ctrl {
                result.push_str("Ctrl+");
            }
            if self.modifiers.shift {
                result.push_str("Shift+");
            }
            result.push_str(&self.key);
            result
        }

        pub fn from_display_string(s: &str) -> Option<Self> {
            let mut modifiers = KeyModifiers::new();
            let parts: Vec<&str> = s.split('+').collect();

            if parts.is_empty() {
                return None;
            }

            let mut key = String::new();

            for (i, part) in parts.iter().enumerate() {
                let part_lower = part.to_lowercase();
                match part_lower.as_str() {
                    "alt" => modifiers.alt = true,
                    "ctrl" => modifiers.ctrl = true,
                    "shift" => modifiers.shift = true,
                    _ => {
                        if i == parts.len() - 1 {
                            key = part.to_string();
                        } else {
                            return None;
                        }
                    }
                }
            }

            if key.is_empty() {
                None
            } else {
                Some(Self { key, modifiers })
            }
        }
    }

    #[test]
    fn test_hotkey_binding_display() {
        let mut mods = KeyModifiers::new();
        mods.ctrl = true;
        mods.shift = true;

        let binding = HotkeyBinding::new("A".to_string(), mods);
        assert_eq!(binding.to_display_string(), "Ctrl+Shift+A");
    }

    #[test]
    fn test_hotkey_binding_parse() {
        let binding = HotkeyBinding::from_display_string("Alt+Ctrl+B").unwrap();
        assert_eq!(binding.key, "B");
        assert!(binding.modifiers.alt);
        assert!(binding.modifiers.ctrl);
        assert!(!binding.modifiers.shift);
    }

    #[test]
    fn test_hotkey_binding_parse_no_modifiers() {
        let binding = HotkeyBinding::from_display_string("X").unwrap();
        assert_eq!(binding.key, "X");
        assert!(!binding.modifiers.alt);
        assert!(!binding.modifiers.ctrl);
        assert!(!binding.modifiers.shift);
    }
}

fn main() {
    println!("Options menu port standalone tests");
    println!("All tests defined - run with: cargo test");
}
