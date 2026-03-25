use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/KeyboardOptionsMenu.cpp",
    "crate::gui::callbacks::menus::keyboard_options_menu",
    "Keyboard Options Menu",
    "Keyboard configuration callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "KeyboardOptionsMenu",
    "Keyboard Options",
    "Key binding and keyboard settings.",
    "Shell",
);

/// Mappable key categories matching C++ MappableKeyCategories from MetaEvent.h lines 13-24.
/// C++ enum order: CATEGORY_CONTROL=0, CATEGORY_INFORMATION, CATEGORY_INTERFACE,
/// CATEGORY_SELECTION, CATEGORY_TAUNT, CATEGORY_TEAM, CATEGORY_MISC, CATEGORY_DEBUG,
/// CATEGORY_NUM_CATEGORIES (sentinel).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MappableKeyCategory {
    Control = 0,
    Information = 1,
    Interface = 2,
    Selection = 3,
    Taunt = 4,
    Team = 5,
    Misc = 6,
    Debug = 7,
}

impl MappableKeyCategory {
    pub const NUM_CATEGORIES: usize = 8;

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Control),
            1 => Some(Self::Information),
            2 => Some(Self::Interface),
            3 => Some(Self::Selection),
            4 => Some(Self::Taunt),
            5 => Some(Self::Team),
            6 => Some(Self::Misc),
            7 => Some(Self::Debug),
            _ => None,
        }
    }

    pub fn ini_name(self) -> &'static str {
        match self {
            Self::Control => "CONTROL",
            Self::Information => "INFORMATION",
            Self::Interface => "INTERFACE",
            Self::Selection => "SELECTION",
            Self::Taunt => "TAUNT",
            Self::Team => "TEAM",
            Self::Misc => "MISC",
            Self::Debug => "DEBUG",
        }
    }
}

/// Category list names matching C++ CategoryListName[] from MetaEvent.h lines 26-37.
pub const CATEGORY_LIST_NAMES: &[(&str, MappableKeyCategory)] = &[
    ("CONTROL", MappableKeyCategory::Control),
    ("INFORMATION", MappableKeyCategory::Information),
    ("INTERFACE", MappableKeyCategory::Interface),
    ("SELECTION", MappableKeyCategory::Selection),
    ("TAUNT", MappableKeyCategory::Taunt),
    ("TEAM", MappableKeyCategory::Team),
    ("MISC", MappableKeyCategory::Misc),
    ("DEBUG", MappableKeyCategory::Debug),
];

#[deprecated(note = "Use MappableKeyCategory instead")]
pub type KeyboardCategoryPort = MappableKeyCategory;

/// MetaMap record matching C++ MetaMapRec from MetaEvent.h lines 290+.
/// Carries display name, description, category, and key binding info.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetaMapRecPort {
    pub display_name: String,
    pub description: String,
    pub category: MappableKeyCategory,
    pub key_code: u32,
    pub default_key_code: u32,
}

impl MetaMapRecPort {
    pub fn current_hotkey_display(&self) -> String {
        key_code_to_name(self.key_code)
    }
}

#[deprecated(note = "Use MetaMapRecPort instead")]
pub type KeyboardCommandPort = MetaMapRecPort;

/// Modifier key kind for doKeyDown/doKeyUp dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModifierKind {
    Alt,
    Ctrl,
    Shift,
}

/// Parsed hotkey assignment for persistence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedHotkey {
    pub key: String,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl ParsedHotkey {
    pub fn to_display_string(&self) -> String {
        let mut s = String::new();
        if self.alt {
            s.push_str("Alt+");
        }
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(&self.key);
        s
    }
}

/// Trait for preference storage, matching game_engine::common::user_preferences::UserPreferences API.
/// Allows save/load of hotkey bindings without coupling to the concrete UserPreferences type.
pub trait PreferenceStore {
    fn set_string(&mut self, key: &str, value: String);
    fn get_string(&self, key: &str) -> Option<String>;
}

/// Keyboard options menu state matching C++ KeyboardOptionsMenu.cpp static state.
/// C++ source: lines 48-91 (static window IDs, modifier booleans, UnicodeString alt/ctrl/shift).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardOptionsMenuPort {
    pub selected_category: MappableKeyCategory,
    pub commands: Vec<MetaMapRecPort>,
    pub selected_command_index: Option<usize>,
    pub assign_text: String,
    pub shadow_text: String,
    pub shift_down: bool,
    pub ctrl_down: bool,
    pub alt_down: bool,
    pub absolute: bool,
}

impl Default for KeyboardOptionsMenuPort {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardOptionsMenuPort {
    pub fn new() -> Self {
        Self {
            selected_category: MappableKeyCategory::Control,
            commands: Vec::new(),
            selected_command_index: None,
            assign_text: String::new(),
            shadow_text: String::new(),
            shift_down: false,
            ctrl_down: false,
            alt_down: false,
            absolute: false,
        }
    }

    /// Populate the category combo box.
    /// Matches C++ populateCategoryBox() lines 94-110.
    /// Iterates CATEGORY_NUM_CATEGORIES, builds "GUI:<CategoryListName[i]>" keys for GameText.
    pub fn populate_category_box_entries() -> Vec<(String, MappableKeyCategory)> {
        let mut entries = Vec::with_capacity(MappableKeyCategory::NUM_CATEGORIES);
        for &(name, cat) in CATEGORY_LIST_NAMES {
            let label = format!("GUI:{name}");
            entries.push((label, cat));
        }
        entries
    }

    /// Fill the command list box for the given category.
    /// Matches C++ fillCommandListBox(MappableKeyCategories cat) lines 124-138.
    /// Iterates all MetaMap records (TheMetaMap linked list), filters by category.
    pub fn fill_command_list_box(
        commands: &[MetaMapRecPort],
        category: MappableKeyCategory,
    ) -> Vec<&MetaMapRecPort> {
        commands
            .iter()
            .filter(|rec| rec.category == category)
            .collect()
    }

    /// Select a category. Matches C++ GCM_SELECTED for comboBoxCategoryListID lines 545-570.
    /// Resets description text, current hotkey text, clears text entry, disables text entry.
    pub fn select_category(&mut self, category: MappableKeyCategory) {
        self.selected_category = category;
        self.selected_command_index = None;
        self.assign_text.clear();
        self.shadow_text.clear();
        self.clear_modifiers();
        self.absolute = false;
    }

    /// Select a command from the list by index.
    /// Matches C++ GLM_SELECTED for listBoxCommandListID lines 575-620.
    pub fn select_command(&mut self, index: usize) -> bool {
        if index >= self.commands.len() {
            return false;
        }
        self.selected_command_index = Some(index);
        self.assign_text.clear();
        self.shadow_text.clear();
        self.clear_modifiers();
        self.absolute = false;
        true
    }

    /// Get the currently selected command record.
    pub fn selected_command(&self) -> Option<&MetaMapRecPort> {
        self.selected_command_index
            .and_then(|i| self.commands.get(i))
    }

    /// Set modifier key down state.
    /// Matches C++ setKeyDown(UnicodeString mod, Bool b) lines 113-121.
    fn set_key_down(&mut self, modifier: ModifierKind, down: bool) {
        match modifier {
            ModifierKind::Shift => self.shift_down = down,
            ModifierKind::Ctrl => self.ctrl_down = down,
            ModifierKind::Alt => self.alt_down = down,
        }
    }

    /// Localized modifier display text.
    /// C++ uses TheGameText->fetch("KEYBOARD:Alt+"), "KEYBOARD:Ctrl+", "KEYBOARD:Shift+").
    fn mod_text(kind: ModifierKind) -> &'static str {
        match kind {
            ModifierKind::Alt => "Alt+",
            ModifierKind::Ctrl => "Ctrl+",
            ModifierKind::Shift => "Shift+",
        }
    }

    /// Handle modifier key press.
    /// Matches C++ doKeyDown(EntryData *e, UnicodeString mod) lines 239-360.
    /// Builds text with Alt+Ctrl+Shift ordering as in C++.
    pub fn do_key_down(&mut self, modifier: ModifierKind) {
        let mod_str = Self::mod_text(modifier);
        if self.assign_text.len() <= 1 {
            self.assign_text = mod_str.to_string();
            self.shadow_text = mod_str.to_string();
            self.set_key_down(modifier, true);
            return;
        }

        let last_char = self.assign_text.chars().last().unwrap_or(' ');
        if last_char != '+' && self.absolute {
            self.assign_text = mod_str.to_string();
            self.shadow_text = mod_str.to_string();
            self.clear_modifiers();
            self.set_key_down(modifier, true);
            self.absolute = false;
            return;
        }

        let already_down = match modifier {
            ModifierKind::Shift => self.shift_down,
            ModifierKind::Ctrl => self.ctrl_down,
            ModifierKind::Alt => self.alt_down,
        };
        if already_down {
            return;
        }

        let alt = Self::mod_text(ModifierKind::Alt);
        let ctrl = Self::mod_text(ModifierKind::Ctrl);
        let shift = Self::mod_text(ModifierKind::Shift);

        // C++ ordering: always Alt+Ctrl+Shift (lines 286-354)
        let text = if self.alt_down && self.ctrl_down {
            // puts shift at the end of the mods (line 286-293)
            format!("{alt}{ctrl}{mod_str}")
        } else if self.alt_down && self.shift_down {
            // puts ctrl in the middle (line 296-303)
            format!("{alt}{ctrl}{shift}")
        } else if self.alt_down {
            // puts either shift or ctrl after alt (line 306-312)
            format!("{alt}{mod_str}")
        } else if self.ctrl_down && self.shift_down {
            // puts alt in front (line 315-322)
            format!("{alt}{ctrl}{shift}")
        } else if self.ctrl_down {
            // if it's alt, put it in front; else put shift after ctrl (line 325-344)
            if modifier == ModifierKind::Alt {
                format!("{mod_str}{ctrl}")
            } else {
                format!("{ctrl}{mod_str}")
            }
        } else if self.shift_down {
            // put alt or ctrl in front of shift (line 347-353)
            format!("{mod_str}{shift}")
        } else {
            mod_str.to_string()
        };

        self.assign_text = text;
        self.set_key_down(modifier, true);
    }

    /// Handle modifier key release.
    /// Matches C++ doKeyUp(EntryData *e, UnicodeString mod) lines 140-236.
    /// Replicates C++ behavior including the C++ bug on line 184 (releasing ctrl
    /// when alt+ctrl are both down sets text to ctrl instead of alt).
    pub fn do_key_up(&mut self, modifier: ModifierKind) {
        let last_char = self.assign_text.chars().last().unwrap_or(' ');
        if last_char != '+' {
            self.absolute = true;
            return;
        }

        let alt = Self::mod_text(ModifierKind::Alt);
        let ctrl = Self::mod_text(ModifierKind::Ctrl);
        let shift = Self::mod_text(ModifierKind::Shift);

        if self.alt_down && self.ctrl_down && self.shift_down {
            // All three down: make string out of other two (lines 147-172)
            let text = match modifier {
                ModifierKind::Shift => format!("{alt}{ctrl}"),
                ModifierKind::Alt => format!("{ctrl}{shift}"),
                ModifierKind::Ctrl => format!("{alt}{shift}"),
            };
            self.assign_text = text;
            self.set_key_down(modifier, false);
        } else if self.alt_down && self.ctrl_down {
            // alt and ctrl both down (lines 175-188)
            // C++ bug parity: line 184 sets text to ctrl even when releasing ctrl
            let text = match modifier {
                ModifierKind::Alt => ctrl.to_string(),
                ModifierKind::Ctrl => ctrl.to_string(),
                _ => String::new(),
            };
            self.assign_text = text;
            self.set_key_down(modifier, false);
        } else if self.alt_down && self.shift_down {
            // alt and shift both down (lines 191-204)
            let text = match modifier {
                ModifierKind::Alt => shift.to_string(),
                ModifierKind::Shift => alt.to_string(),
                _ => String::new(),
            };
            self.assign_text = text;
            self.set_key_down(modifier, false);
        } else if self.ctrl_down && self.shift_down {
            // ctrl and shift both down (lines 207-220)
            let text = match modifier {
                ModifierKind::Ctrl => shift.to_string(),
                ModifierKind::Shift => ctrl.to_string(),
                _ => String::new(),
            };
            self.assign_text = text;
            self.set_key_down(modifier, false);
        } else {
            // Only one mod, clear everything (lines 223-229)
            self.assign_text.clear();
            self.shadow_text.clear();
            self.set_key_down(modifier, false);
        }
    }

    /// Assign a key to the current modifier combination.
    /// Matches C++ GWM_IME_CHAR handling in KeyboardTextEntryInput lines 696-793.
    pub fn assign_key(&mut self, key: char) {
        if self.assign_text.len() <= 1 {
            self.assign_text.clear();
            self.assign_text.push(key);
            self.absolute = true;
            return;
        }

        let last_char = self.assign_text.chars().last().unwrap_or(' ');
        if last_char == '+' {
            // Modifiers present, append key after trailing '+'
            self.assign_text.push(key);
            self.absolute = true;
        } else if (self.shift_down || self.ctrl_down || self.alt_down) && !self.absolute {
            // Replace last key char to prevent flickering (C++ lines 762-774)
            let current_last = self.assign_text.chars().last().unwrap_or('\0');
            if current_last != key {
                self.assign_text.pop();
                self.assign_text.push(key);
            }
        } else {
            // No modifiers, bare key replaces everything (C++ lines 777-786)
            self.assign_text.clear();
            self.assign_text.push(key);
            self.absolute = true;
        }
    }

    /// Handle backspace - clears all state.
    /// Matches C++ KEY_BACKSPACE case lines 952-966.
    pub fn handle_backspace(&mut self) {
        self.assign_text.clear();
        self.clear_modifiers();
        self.absolute = false;
    }

    /// Reset all hotkeys to defaults.
    /// Matches C++ buttonResetAllID handling lines 639-663.
    pub fn reset_all(&mut self) {
        self.assign_text.clear();
        self.shadow_text.clear();
        self.clear_modifiers();
        self.absolute = false;
        self.selected_command_index = None;
    }

    fn clear_modifiers(&mut self) {
        self.shift_down = false;
        self.ctrl_down = false;
        self.alt_down = false;
    }

    /// Parse the current assign_text into a ParsedHotkey for persistence.
    pub fn parse_assignment(&self) -> Option<ParsedHotkey> {
        let text = self.assign_text.trim();
        if text.is_empty() {
            return None;
        }
        let parts: Vec<&str> = text.split('+').collect();
        let key = parts.last()?.to_string();
        if key.is_empty() {
            return None;
        }
        let has_alt = parts.iter().any(|p| p.eq_ignore_ascii_case("Alt"));
        let has_ctrl = parts.iter().any(|p| p.eq_ignore_ascii_case("Ctrl"));
        let has_shift = parts.iter().any(|p| p.eq_ignore_ascii_case("Shift"));
        Some(ParsedHotkey {
            key,
            alt: has_alt,
            ctrl: has_ctrl,
            shift: has_shift,
        })
    }

    /// Save hotkey binding for a command to preferences.
    /// Matches C++ UserPreferences hotkey persistence pattern.
    /// Stores as "KeyBinding:<display_name> = <key>:<modifier_flags>".
    pub fn save_binding_to_preferences(
        command_display_name: &str,
        parsed: &ParsedHotkey,
        preferences: &mut dyn PreferenceStore,
    ) {
        let mod_val = (parsed.alt as u32) << 2 | (parsed.ctrl as u32) << 1 | (parsed.shift as u32);
        preferences.set_string(
            &format!("KeyBinding:{command_display_name}"),
            format!("{}:{}", parsed.key, mod_val),
        );
    }

    /// Load hotkey binding for a command from preferences.
    pub fn load_binding_from_preferences(
        command_display_name: &str,
        preferences: &dyn PreferenceStore,
    ) -> Option<ParsedHotkey> {
        let stored = preferences.get_string(&format!("KeyBinding:{command_display_name}"))?;
        let (key, mod_val) = stored.split_once(':')?;
        let mod_val: u32 = mod_val.parse().ok()?;
        Some(ParsedHotkey {
            key: key.to_string(),
            alt: (mod_val & 4) != 0,
            ctrl: (mod_val & 2) != 0,
            shift: (mod_val & 1) != 0,
        })
    }

    pub fn sample() -> Self {
        Self {
            selected_category: MappableKeyCategory::Control,
            commands: vec![
                MetaMapRecPort {
                    display_name: "Attack Move".to_string(),
                    description: "Orders selected units to move and engage enemies on the path."
                        .to_string(),
                    category: MappableKeyCategory::Control,
                    key_code: b'A' as u32,
                    default_key_code: b'A' as u32,
                },
                MetaMapRecPort {
                    display_name: "Force Fire".to_string(),
                    description: "Forces units to fire on the targeted ground or object."
                        .to_string(),
                    category: MappableKeyCategory::Control,
                    key_code: b'F' as u32,
                    default_key_code: b'F' as u32,
                },
                MetaMapRecPort {
                    display_name: "Toggle Radar".to_string(),
                    description: "Shows or hides the radar display.".to_string(),
                    category: MappableKeyCategory::Interface,
                    key_code: b'R' as u32,
                    default_key_code: b'R' as u32,
                },
            ],
            selected_command_index: Some(0),
            assign_text: String::new(),
            shadow_text: String::new(),
            shift_down: false,
            ctrl_down: false,
            alt_down: false,
            absolute: false,
        }
    }
}

fn key_code_to_name(code: u32) -> String {
    match code {
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x1B => "Esc".to_string(),
        0x20 => "Space".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x2E => "Delete".to_string(),
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(code)
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| format!("0x{code:02X}")),
        0x70..=0x7B => format!("F{}", code - 0x6F),
        _ => format!("0x{code:02X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockPreferences {
        data: HashMap<String, String>,
    }

    impl MockPreferences {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }
    }

    impl PreferenceStore for MockPreferences {
        fn set_string(&mut self, key: &str, value: String) {
            self.data.insert(key.to_string(), value);
        }

        fn get_string(&self, key: &str) -> Option<String> {
            self.data.get(key).cloned()
        }
    }

    #[test]
    fn populate_category_box_returns_all_categories() {
        let entries = KeyboardOptionsMenuPort::populate_category_box_entries();
        assert_eq!(entries.len(), MappableKeyCategory::NUM_CATEGORIES);
        assert_eq!(entries[0].1, MappableKeyCategory::Control);
        assert_eq!(entries[7].1, MappableKeyCategory::Debug);
        assert_eq!(entries[0].0, "GUI:CONTROL");
        assert_eq!(entries[2].0, "GUI:INTERFACE");
    }

    #[test]
    fn fill_command_list_filters_by_category() {
        let commands = vec![
            MetaMapRecPort {
                display_name: "Attack".to_string(),
                description: "Attack move".to_string(),
                category: MappableKeyCategory::Control,
                key_code: b'A' as u32,
                default_key_code: b'A' as u32,
            },
            MetaMapRecPort {
                display_name: "Select All".to_string(),
                description: "Select all units".to_string(),
                category: MappableKeyCategory::Selection,
                key_code: b'E' as u32,
                default_key_code: b'E' as u32,
            },
            MetaMapRecPort {
                display_name: "Guard".to_string(),
                description: "Guard mode".to_string(),
                category: MappableKeyCategory::Control,
                key_code: b'G' as u32,
                default_key_code: b'G' as u32,
            },
        ];
        let filtered =
            KeyboardOptionsMenuPort::fill_command_list_box(&commands, MappableKeyCategory::Control);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].display_name, "Attack");
        assert_eq!(filtered[1].display_name, "Guard");

        let selection_filtered = KeyboardOptionsMenuPort::fill_command_list_box(
            &commands,
            MappableKeyCategory::Selection,
        );
        assert_eq!(selection_filtered.len(), 1);
        assert_eq!(selection_filtered[0].display_name, "Select All");
    }

    #[test]
    fn do_key_down_single_modifier() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        assert_eq!(menu.assign_text, "Ctrl+");
        assert!(menu.ctrl_down);
        assert!(!menu.alt_down);
        assert!(!menu.shift_down);
    }

    #[test]
    fn do_key_down_alt_ctrl_shift_ordering() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        assert_eq!(menu.assign_text, "Ctrl+");

        menu.do_key_down(ModifierKind::Shift);
        assert_eq!(menu.assign_text, "Ctrl+Shift+");

        menu.do_key_down(ModifierKind::Alt);
        // C++: altDown && ctrlDown && shiftDown -> alt goes in front
        assert_eq!(menu.assign_text, "Alt+Ctrl+Shift+");
        assert!(menu.alt_down);
    }

    #[test]
    fn do_key_down_shift_then_alt_then_ctrl() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Shift);
        assert_eq!(menu.assign_text, "Shift+");

        menu.do_key_down(ModifierKind::Alt);
        // C++: shiftDown, mod=alt -> text = alt+shift
        assert_eq!(menu.assign_text, "Alt+Shift+");

        menu.do_key_down(ModifierKind::Ctrl);
        // C++: altDown && shiftDown -> text = alt+ctrl+shift
        assert_eq!(menu.assign_text, "Alt+Ctrl+Shift+");
    }

    #[test]
    fn do_key_down_same_modifier_noop() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        let text_before = menu.assign_text.clone();
        menu.do_key_down(ModifierKind::Ctrl);
        assert_eq!(menu.assign_text, text_before);
    }

    #[test]
    fn do_key_up_all_three_releasing_shift() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.do_key_down(ModifierKind::Ctrl);
        menu.do_key_down(ModifierKind::Shift);
        assert_eq!(menu.assign_text, "Alt+Ctrl+Shift+");

        menu.do_key_up(ModifierKind::Shift);
        // C++: all three down, releasing shift -> alt+ctrl
        assert_eq!(menu.assign_text, "Alt+Ctrl+");
        assert!(!menu.shift_down);
        assert!(menu.alt_down);
        assert!(menu.ctrl_down);
    }

    #[test]
    fn do_key_up_all_three_releasing_alt() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.do_key_down(ModifierKind::Ctrl);
        menu.do_key_down(ModifierKind::Shift);
        assert_eq!(menu.assign_text, "Alt+Ctrl+Shift+");

        menu.do_key_up(ModifierKind::Alt);
        // C++: all three down, releasing alt -> ctrl+shift
        assert_eq!(menu.assign_text, "Ctrl+Shift+");
        assert!(!menu.alt_down);
        assert!(menu.ctrl_down);
        assert!(menu.shift_down);
    }

    #[test]
    fn do_key_up_all_three_releasing_ctrl() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.do_key_down(ModifierKind::Ctrl);
        menu.do_key_down(ModifierKind::Shift);
        assert_eq!(menu.assign_text, "Alt+Ctrl+Shift+");

        menu.do_key_up(ModifierKind::Ctrl);
        // C++: all three down, releasing ctrl -> alt+shift
        assert_eq!(menu.assign_text, "Alt+Shift+");
        assert!(!menu.ctrl_down);
    }

    #[test]
    fn do_key_up_two_mods_alt_ctrl_releasing_alt() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.do_key_down(ModifierKind::Ctrl);
        assert_eq!(menu.assign_text, "Alt+Ctrl+");

        menu.do_key_up(ModifierKind::Alt);
        // C++: altDown && ctrlDown, releasing alt -> text = ctrl
        assert_eq!(menu.assign_text, "Ctrl+");
        assert!(!menu.alt_down);
    }

    #[test]
    fn do_key_up_two_mods_alt_ctrl_releasing_ctrl_cxx_bug_parity() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.do_key_down(ModifierKind::Ctrl);
        assert_eq!(menu.assign_text, "Alt+Ctrl+");

        menu.do_key_up(ModifierKind::Ctrl);
        // C++ bug parity (line 184): releasing ctrl when alt+ctrl -> text = ctrl (not alt)
        assert_eq!(menu.assign_text, "Ctrl+");
        assert!(!menu.ctrl_down);
    }

    #[test]
    fn do_key_up_single_mod_clears_text() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        assert_eq!(menu.assign_text, "Alt+");

        menu.do_key_up(ModifierKind::Alt);
        // Only one mod down -> else branch: clear text
        assert_eq!(menu.assign_text, "");
        assert!(menu.shadow_text.is_empty());
        assert!(!menu.alt_down);
    }

    #[test]
    fn do_key_up_after_absolute_key_sets_absolute_flag() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        menu.assign_key('A');
        // Now text = "Ctrl+A", last char = 'A' (not '+')
        menu.do_key_up(ModifierKind::Ctrl);
        assert!(menu.absolute);
    }

    #[test]
    fn assign_key_with_modifiers() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        menu.do_key_down(ModifierKind::Shift);
        menu.assign_key('K');
        assert_eq!(menu.assign_text, "Ctrl+Shift+K");
        assert!(menu.absolute);
    }

    #[test]
    fn assign_key_bare() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.assign_key('X');
        assert_eq!(menu.assign_text, "X");
        assert!(menu.absolute);
    }

    #[test]
    fn assign_key_replaces_last_with_modifiers_held() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.assign_key('A');
        assert_eq!(menu.assign_text, "Alt+A");

        // Typing another key while modifiers held replaces the key (flicker prevention)
        menu.assign_key('B');
        assert_eq!(menu.assign_text, "Alt+B");
    }

    #[test]
    fn assign_key_same_char_no_flicker() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.assign_key('A');
        assert_eq!(menu.assign_text, "Alt+A");

        // Same char with modifiers held should not change text
        menu.assign_key('A');
        assert_eq!(menu.assign_text, "Alt+A");
    }

    #[test]
    fn backspace_clears_all() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.assign_key('X');
        assert!(!menu.assign_text.is_empty());

        menu.handle_backspace();
        assert!(menu.assign_text.is_empty());
        assert!(!menu.alt_down);
        assert!(!menu.absolute);
    }

    #[test]
    fn reset_all_clears_state() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        menu.assign_key('K');
        menu.selected_command_index = Some(0);

        menu.reset_all();
        assert!(menu.assign_text.is_empty());
        assert!(menu.shadow_text.is_empty());
        assert!(!menu.ctrl_down);
        assert!(!menu.absolute);
        assert!(menu.selected_command_index.is_none());
    }

    #[test]
    fn do_key_down_after_absolute_resets() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        menu.assign_key('A');
        assert!(menu.absolute);

        // New modifier after absolute key resets state (C++ lines 256-267)
        menu.do_key_down(ModifierKind::Shift);
        assert_eq!(menu.assign_text, "Shift+");
        assert!(!menu.ctrl_down);
        assert!(menu.shift_down);
        assert!(!menu.absolute);
    }

    #[test]
    fn save_and_load_binding_roundtrip() {
        let hotkey = ParsedHotkey {
            key: "K".to_string(),
            alt: true,
            ctrl: true,
            shift: false,
        };
        let mut prefs = MockPreferences::new();
        KeyboardOptionsMenuPort::save_binding_to_preferences("Attack Move", &hotkey, &mut prefs);

        let loaded =
            KeyboardOptionsMenuPort::load_binding_from_preferences("Attack Move", &prefs).unwrap();
        assert_eq!(loaded.key, "K");
        assert!(loaded.alt);
        assert!(loaded.ctrl);
        assert!(!loaded.shift);
    }

    #[test]
    fn load_missing_binding_returns_none() {
        let prefs = MockPreferences::new();
        let loaded = KeyboardOptionsMenuPort::load_binding_from_preferences("Nonexistent", &prefs);
        assert!(loaded.is_none());
    }

    #[test]
    fn parse_assignment_extracts_components() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Alt);
        menu.do_key_down(ModifierKind::Ctrl);
        menu.assign_key('F');

        let parsed = menu.parse_assignment().unwrap();
        assert_eq!(parsed.key, "F");
        assert!(parsed.alt);
        assert!(parsed.ctrl);
        assert!(!parsed.shift);
    }

    #[test]
    fn parse_assignment_empty_returns_none() {
        let menu = KeyboardOptionsMenuPort::new();
        assert!(menu.parse_assignment().is_none());
    }

    #[test]
    fn parse_assignment_bare_key() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.assign_key('X');
        let parsed = menu.parse_assignment().unwrap();
        assert_eq!(parsed.key, "X");
        assert!(!parsed.alt);
        assert!(!parsed.ctrl);
        assert!(!parsed.shift);
    }

    #[test]
    fn select_category_resets_state() {
        let mut menu = KeyboardOptionsMenuPort::new();
        menu.do_key_down(ModifierKind::Ctrl);
        menu.assign_key('K');

        menu.select_category(MappableKeyCategory::Interface);
        assert_eq!(menu.selected_category, MappableKeyCategory::Interface);
        assert!(menu.assign_text.is_empty());
        assert!(!menu.ctrl_down);
        assert!(menu.selected_command_index.is_none());
    }

    #[test]
    fn select_command_validates_index() {
        let mut menu = KeyboardOptionsMenuPort::sample();
        assert!(menu.select_command(0));
        assert_eq!(menu.selected_command_index, Some(0));
        assert!(!menu.select_command(999));
    }

    #[test]
    fn selected_command_returns_correct_record() {
        let menu = KeyboardOptionsMenuPort::sample();
        let cmd = menu.selected_command().unwrap();
        assert_eq!(cmd.display_name, "Attack Move");
    }

    #[test]
    fn category_from_index_roundtrip() {
        for i in 0..MappableKeyCategory::NUM_CATEGORIES {
            let cat = MappableKeyCategory::from_index(i).unwrap();
            assert_eq!(cat as usize, i);
        }
        assert!(MappableKeyCategory::from_index(8).is_none());
    }

    #[test]
    fn category_ini_names_match_cxx() {
        assert_eq!(MappableKeyCategory::Control.ini_name(), "CONTROL");
        assert_eq!(MappableKeyCategory::Debug.ini_name(), "DEBUG");
    }

    #[test]
    fn parsed_hotkey_to_display_string() {
        let hotkey = ParsedHotkey {
            key: "K".to_string(),
            alt: true,
            ctrl: false,
            shift: true,
        };
        assert_eq!(hotkey.to_display_string(), "Alt+Shift+K");
    }

    #[test]
    fn key_code_to_name_conversions() {
        assert_eq!(key_code_to_name(0x41), "A");
        assert_eq!(key_code_to_name(0x1B), "Esc");
        assert_eq!(key_code_to_name(0x70), "F1");
        assert_eq!(key_code_to_name(0x20), "Space");
    }
}
