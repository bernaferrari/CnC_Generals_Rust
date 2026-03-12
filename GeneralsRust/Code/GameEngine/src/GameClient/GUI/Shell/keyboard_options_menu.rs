// FILE: keyboard_options_menu.rs
// Author: Ported from C++ (Chris Brue, July 2002)
// Description: Keyboard options window control for keybinding management
//
// This is a faithful port from:
// GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/KeyboardOptionsMenu.cpp

use std::collections::HashMap;

// Type aliases to match C++ naming
type Bool = bool;
type Int = i32;

/// Mappable key categories
/// Matches C++ MappableKeyCategories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MappableKeyCategory {
    Control = 0,
    Selection,
    Team,
    Beacon,
    Camera,
    Scripting,
    Interface,
    Development,
    NumCategories,
}

/// Category list names - matches C++ CategoryListName
pub const CATEGORY_NAMES: [&str; 8] = [
    "Control",
    "Selection",
    "Team",
    "Beacon",
    "Camera",
    "Scripting",
    "Interface",
    "Development",
];

/// Mappable key types
/// Matches C++ MappableKeyType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MappableKeyType {
    // Control keys
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Stop,
    Attack,
    ForceAttack,
    Scatter,
    Guard,
    Waypoint,

    // Selection keys
    SelectAll,
    SelectAllCombat,
    SelectAllDamaged,
    SelectNextUnit,
    SelectPrevUnit,

    // Team keys
    CreateTeam0,
    CreateTeam1,
    CreateTeam2,
    CreateTeam3,
    CreateTeam4,
    CreateTeam5,
    CreateTeam6,
    CreateTeam7,
    CreateTeam8,
    CreateTeam9,
    SelectTeam0,
    SelectTeam1,
    SelectTeam2,
    SelectTeam3,
    SelectTeam4,
    SelectTeam5,
    SelectTeam6,
    SelectTeam7,
    SelectTeam8,
    SelectTeam9,

    // Camera keys
    CameraBookmark1,
    CameraBookmark2,
    CameraBookmark3,
    CameraBookmark4,
    CameraGoto1,
    CameraGoto2,
    CameraGoto3,
    CameraGoto4,

    // Beacon keys
    BeaconAttack,
    BeaconDefend,
    BeaconGather,

    // Interface keys
    ToggleFullscreen,
    Screenshot,
    ShowFramerate,

    // Development keys
    DebugMode,

    // Sentinel
    NumMappableKeys,
}

/// Key modifier flags
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

    pub fn none() -> Self {
        Self::new()
    }

    pub fn has_any(&self) -> bool {
        self.shift || self.ctrl || self.alt
    }

    pub fn clear(&mut self) {
        self.shift = false;
        self.ctrl = false;
        self.alt = false;
    }
}

/// Hotkey binding information
#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub key: String,
    pub modifiers: KeyModifiers,
}

impl HotkeyBinding {
    pub fn new(key: String, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    /// Convert to display string
    /// Matches C++ text entry format with modifiers
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

    /// Parse from display string
    /// Matches C++ text entry parsing logic
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
                    // This is the key itself (should be last part)
                    if i == parts.len() - 1 {
                        key = part.to_string();
                    } else {
                        // Invalid format
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

/// Meta map record - describes a mappable command
/// Matches C++ MetaMapRec structure
#[derive(Debug, Clone)]
pub struct MetaMapRec {
    pub key: MappableKeyType,
    pub category: MappableKeyCategory,
    pub display_name: String,
    pub description: String,
    pub default_binding: Option<HotkeyBinding>,
}

/// The meta map - defines all mappable commands
/// Matches C++ TheMetaMap global
pub struct MetaMap {
    records: Vec<MetaMapRec>,
    bindings: HashMap<MappableKeyType, HotkeyBinding>,
}

impl MetaMap {
    pub fn new() -> Self {
        let mut map = Self {
            records: Vec::new(),
            bindings: HashMap::new(),
        };
        map.initialize_defaults();
        map
    }

    /// Initialize default key mappings
    /// This would typically be loaded from configuration
    fn initialize_defaults(&mut self) {
        // Control category
        self.add_record(
            MappableKeyType::MoveUp,
            MappableKeyCategory::Control,
            "Move Up",
            "Move camera or units up",
            Some(HotkeyBinding::new("Up".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::MoveDown,
            MappableKeyCategory::Control,
            "Move Down",
            "Move camera or units down",
            Some(HotkeyBinding::new("Down".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::MoveLeft,
            MappableKeyCategory::Control,
            "Move Left",
            "Move camera or units left",
            Some(HotkeyBinding::new("Left".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::MoveRight,
            MappableKeyCategory::Control,
            "Move Right",
            "Move camera or units right",
            Some(HotkeyBinding::new("Right".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::Stop,
            MappableKeyCategory::Control,
            "Stop",
            "Stop selected units",
            Some(HotkeyBinding::new("S".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::Attack,
            MappableKeyCategory::Control,
            "Attack",
            "Attack move",
            Some(HotkeyBinding::new("A".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::Guard,
            MappableKeyCategory::Control,
            "Guard",
            "Guard mode",
            Some(HotkeyBinding::new("G".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::Scatter,
            MappableKeyCategory::Control,
            "Scatter",
            "Scatter units",
            Some(HotkeyBinding::new("X".to_string(), KeyModifiers::none())),
        );

        // Selection category
        self.add_record(
            MappableKeyType::SelectAll,
            MappableKeyCategory::Selection,
            "Select All",
            "Select all units on screen",
            Some(HotkeyBinding::new("E".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::SelectAllCombat,
            MappableKeyCategory::Selection,
            "Select All Combat Units",
            "Select all combat units on screen",
            Some(HotkeyBinding::new("Q".to_string(), KeyModifiers::none())),
        );

        // Team creation keys (0-9)
        for i in 0..10 {
            let key_type = match i {
                0 => MappableKeyType::CreateTeam0,
                1 => MappableKeyType::CreateTeam1,
                2 => MappableKeyType::CreateTeam2,
                3 => MappableKeyType::CreateTeam3,
                4 => MappableKeyType::CreateTeam4,
                5 => MappableKeyType::CreateTeam5,
                6 => MappableKeyType::CreateTeam6,
                7 => MappableKeyType::CreateTeam7,
                8 => MappableKeyType::CreateTeam8,
                9 => MappableKeyType::CreateTeam9,
                _ => continue,
            };

            let mut mods = KeyModifiers::new();
            mods.ctrl = true;

            self.add_record(
                key_type,
                MappableKeyCategory::Team,
                &format!("Create Team {}", i),
                &format!("Create team {}", i),
                Some(HotkeyBinding::new(i.to_string(), mods)),
            );
        }

        // Team selection keys (0-9)
        for i in 0..10 {
            let key_type = match i {
                0 => MappableKeyType::SelectTeam0,
                1 => MappableKeyType::SelectTeam1,
                2 => MappableKeyType::SelectTeam2,
                3 => MappableKeyType::SelectTeam3,
                4 => MappableKeyType::SelectTeam4,
                5 => MappableKeyType::SelectTeam5,
                6 => MappableKeyType::SelectTeam6,
                7 => MappableKeyType::SelectTeam7,
                8 => MappableKeyType::SelectTeam8,
                9 => MappableKeyType::SelectTeam9,
                _ => continue,
            };

            self.add_record(
                key_type,
                MappableKeyCategory::Team,
                &format!("Select Team {}", i),
                &format!("Select team {}", i),
                Some(HotkeyBinding::new(i.to_string(), KeyModifiers::none())),
            );
        }

        // Camera bookmarks (F5-F8)
        self.add_record(
            MappableKeyType::CameraBookmark1,
            MappableKeyCategory::Camera,
            "Set Camera 1",
            "Set camera bookmark 1",
            Some(HotkeyBinding::new("F5".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::CameraBookmark2,
            MappableKeyCategory::Camera,
            "Set Camera 2",
            "Set camera bookmark 2",
            Some(HotkeyBinding::new("F6".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::CameraBookmark3,
            MappableKeyCategory::Camera,
            "Set Camera 3",
            "Set camera bookmark 3",
            Some(HotkeyBinding::new("F7".to_string(), KeyModifiers::none())),
        );
        self.add_record(
            MappableKeyType::CameraBookmark4,
            MappableKeyCategory::Camera,
            "Set Camera 4",
            "Set camera bookmark 4",
            Some(HotkeyBinding::new("F8".to_string(), KeyModifiers::none())),
        );

        // Interface keys
        self.add_record(
            MappableKeyType::ToggleFullscreen,
            MappableKeyCategory::Interface,
            "Toggle Fullscreen",
            "Toggle fullscreen mode",
            Some(HotkeyBinding::new("F".to_string(), {
                let mut mods = KeyModifiers::new();
                mods.alt = true;
                mods
            })),
        );
        self.add_record(
            MappableKeyType::Screenshot,
            MappableKeyCategory::Interface,
            "Screenshot",
            "Take screenshot",
            Some(HotkeyBinding::new("F12".to_string(), KeyModifiers::none())),
        );
    }

    fn add_record(
        &mut self,
        key: MappableKeyType,
        category: MappableKeyCategory,
        display_name: &str,
        description: &str,
        default_binding: Option<HotkeyBinding>,
    ) {
        let rec = MetaMapRec {
            key,
            category,
            display_name: display_name.to_string(),
            description: description.to_string(),
            default_binding: default_binding.clone(),
        };

        // Set initial binding to default
        if let Some(binding) = default_binding {
            self.bindings.insert(key, binding);
        }

        self.records.push(rec);
    }

    pub fn get_records_for_category(&self, category: MappableKeyCategory) -> Vec<&MetaMapRec> {
        self.records
            .iter()
            .filter(|r| r.category == category)
            .collect()
    }

    pub fn get_binding(&self, key: MappableKeyType) -> Option<&HotkeyBinding> {
        self.bindings.get(&key)
    }

    pub fn set_binding(&mut self, key: MappableKeyType, binding: HotkeyBinding) {
        self.bindings.insert(key, binding);
    }

    pub fn reset_to_defaults(&mut self) {
        self.bindings.clear();
        for rec in &self.records {
            if let Some(ref default_binding) = rec.default_binding {
                self.bindings.insert(rec.key, default_binding.clone());
            }
        }
    }

    pub fn get_all_records(&self) -> &[MetaMapRec] {
        &self.records
    }
}

/// Keyboard options menu state
/// Matches C++ keyboard options menu management from KeyboardOptionsMenu.cpp
pub struct KeyboardOptionsMenu {
    pub meta_map: MetaMap,
    pub selected_category: MappableKeyCategory,
    pub selected_command: Option<MappableKeyType>,
    pub text_entry_value: String,
    pub current_modifiers: KeyModifiers,

    // UI state flags
    pub shift_down: bool,
    pub alt_down: bool,
    pub ctrl_down: bool,
    pub absolute: bool, // Whether a complete key (not just modifiers) has been entered
}

impl KeyboardOptionsMenu {
    /// Create new keyboard options menu
    /// Matches C++ KeyboardOptionsMenuInit() line 366-427
    pub fn new() -> Self {
        Self {
            meta_map: MetaMap::new(),
            selected_category: MappableKeyCategory::Control,
            selected_command: None,
            text_entry_value: String::new(),
            current_modifiers: KeyModifiers::new(),
            shift_down: false,
            alt_down: false,
            ctrl_down: false,
            absolute: false,
        }
    }

    /// Get commands for currently selected category
    /// Matches C++ fillCommandListBox() line 124-138
    pub fn get_commands_for_current_category(&self) -> Vec<&MetaMapRec> {
        self.meta_map.get_records_for_category(self.selected_category)
    }

    /// Select a category
    /// Matches C++ GCM_SELECTED for comboBoxCategoryListID line 545-570
    pub fn select_category(&mut self, category: MappableKeyCategory) {
        self.selected_category = category;
        self.selected_command = None;
        self.text_entry_value.clear();
        self.clear_modifiers();
    }

    /// Select a command
    /// Matches C++ GLM_SELECTED for listBoxCommandListID line 575-620
    pub fn select_command(&mut self, command: MappableKeyType) {
        self.selected_command = Some(command);
        self.text_entry_value.clear();
        self.clear_modifiers();
    }

    /// Get current hotkey for selected command
    pub fn get_current_hotkey(&self) -> Option<String> {
        if let Some(cmd) = self.selected_command {
            if let Some(binding) = self.meta_map.get_binding(cmd) {
                return Some(binding.to_display_string());
            }
        }
        None
    }

    /// Handle modifier key down
    /// Matches C++ doKeyDown() line 239-360
    pub fn handle_modifier_down(&mut self, modifier: &str) {
        // If we have a complete key entered, start fresh
        if self.absolute && self.text_entry_value.len() > 1 {
            let last_char = self.text_entry_value.chars().last().unwrap_or('+');
            if last_char != '+' {
                self.text_entry_value.clear();
                self.clear_modifiers();
                self.absolute = false;
            }
        }

        // Add modifier if not already present
        match modifier.to_lowercase().as_str() {
            "shift" => {
                if !self.shift_down {
                    self.shift_down = true;
                    self.update_text_entry();
                }
            }
            "ctrl" => {
                if !self.ctrl_down {
                    self.ctrl_down = true;
                    self.update_text_entry();
                }
            }
            "alt" => {
                if !self.alt_down {
                    self.alt_down = true;
                    self.update_text_entry();
                }
            }
            _ => {}
        }
    }

    /// Handle modifier key up
    /// Matches C++ doKeyUp() line 140-236
    pub fn handle_modifier_up(&mut self, modifier: &str) {
        let last_char = self.text_entry_value.chars().last().unwrap_or(' ');

        // Only remove modifiers if we haven't entered a complete key yet
        if last_char == '+' {
            match modifier.to_lowercase().as_str() {
                "shift" => {
                    if self.shift_down {
                        self.shift_down = false;
                        self.update_text_entry();
                    }
                }
                "ctrl" => {
                    if self.ctrl_down {
                        self.ctrl_down = false;
                        self.update_text_entry();
                    }
                }
                "alt" => {
                    if self.alt_down {
                        self.alt_down = false;
                        self.update_text_entry();
                    }
                }
                _ => {}
            }
        } else {
            // We have a complete binding
            self.absolute = true;
        }
    }

    /// Handle regular key press
    /// Matches C++ GWM_IME_CHAR and default case handling line 696-793
    pub fn handle_key_press(&mut self, key: &str) {
        // If modifiers are present, append key
        if self.shift_down || self.ctrl_down || self.alt_down {
            self.text_entry_value.clear();
            self.update_text_entry();
            self.text_entry_value.push_str(key);
            self.absolute = true;
        } else {
            // Just the key
            self.text_entry_value = key.to_string();
            self.absolute = true;
        }
    }

    /// Handle backspace
    /// Matches C++ KEY_BACKSPACE case line 952-966
    pub fn handle_backspace(&mut self) {
        self.text_entry_value.clear();
        self.clear_modifiers();
        self.absolute = false;
    }

    /// Update text entry display based on current modifiers
    fn update_text_entry(&mut self) {
        self.text_entry_value.clear();

        // Add modifiers in order: Alt, Ctrl, Shift
        if self.alt_down {
            self.text_entry_value.push_str("Alt+");
        }
        if self.ctrl_down {
            self.text_entry_value.push_str("Ctrl+");
        }
        if self.shift_down {
            self.text_entry_value.push_str("Shift+");
        }
    }

    /// Clear all modifiers
    fn clear_modifiers(&mut self) {
        self.shift_down = false;
        self.ctrl_down = false;
        self.alt_down = false;
        self.current_modifiers.clear();
    }

    /// Assign current text entry to selected command
    pub fn assign_hotkey(&mut self) -> bool {
        if let Some(cmd) = self.selected_command {
            if let Some(binding) = HotkeyBinding::from_display_string(&self.text_entry_value) {
                self.meta_map.set_binding(cmd, binding);
                return true;
            }
        }
        false
    }

    /// Reset all hotkeys to defaults
    /// Matches C++ buttonResetAllID handling line 639-663
    pub fn reset_all_to_defaults(&mut self) {
        self.meta_map.reset_to_defaults();
        self.selected_category = MappableKeyCategory::Control;
        self.selected_command = None;
        self.text_entry_value.clear();
        self.clear_modifiers();
    }

    /// Save hotkey bindings to preferences
    pub fn save_bindings(&self) {
        // In a real implementation, this would write to a file
        // For now, this is a placeholder
    }

    /// Load hotkey bindings from preferences
    pub fn load_bindings(&mut self) {
        // In a real implementation, this would read from a file
        // For now, this is a placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_meta_map_initialization() {
        let meta_map = MetaMap::new();
        assert!(!meta_map.records.is_empty());

        // Check that we have bindings for basic controls
        assert!(meta_map.get_binding(MappableKeyType::Stop).is_some());
        assert!(meta_map.get_binding(MappableKeyType::Attack).is_some());
    }

    #[test]
    fn test_category_filtering() {
        let meta_map = MetaMap::new();
        let control_records = meta_map.get_records_for_category(MappableKeyCategory::Control);
        assert!(!control_records.is_empty());

        // All returned records should be in Control category
        for rec in control_records {
            assert_eq!(rec.category, MappableKeyCategory::Control);
        }
    }

    #[test]
    fn test_keyboard_menu_category_selection() {
        let mut menu = KeyboardOptionsMenu::new();
        menu.select_category(MappableKeyCategory::Team);

        assert_eq!(menu.selected_category, MappableKeyCategory::Team);
        assert!(menu.selected_command.is_none());
        assert!(menu.text_entry_value.is_empty());
    }

    #[test]
    fn test_modifier_handling() {
        let mut menu = KeyboardOptionsMenu::new();

        menu.handle_modifier_down("ctrl");
        assert!(menu.ctrl_down);
        assert_eq!(menu.text_entry_value, "Ctrl+");

        menu.handle_modifier_down("shift");
        assert!(menu.shift_down);
        assert_eq!(menu.text_entry_value, "Ctrl+Shift+");

        menu.handle_key_press("A");
        assert_eq!(menu.text_entry_value, "Ctrl+Shift+A");
        assert!(menu.absolute);
    }

    #[test]
    fn test_backspace_clears_entry() {
        let mut menu = KeyboardOptionsMenu::new();
        menu.handle_modifier_down("alt");
        menu.handle_key_press("X");

        assert!(!menu.text_entry_value.is_empty());

        menu.handle_backspace();
        assert!(menu.text_entry_value.is_empty());
        assert!(!menu.alt_down);
        assert!(!menu.absolute);
    }

    #[test]
    fn test_reset_to_defaults() {
        let mut menu = KeyboardOptionsMenu::new();

        // Change a binding
        menu.selected_command = Some(MappableKeyType::Stop);
        menu.text_entry_value = "Ctrl+Q".to_string();
        menu.assign_hotkey();

        // Reset
        menu.reset_all_to_defaults();

        // Should be back to default
        let binding = menu.meta_map.get_binding(MappableKeyType::Stop).unwrap();
        assert_eq!(binding.key, "S");
        assert!(!binding.modifiers.has_any());
    }

    #[test]
    fn test_assign_hotkey() {
        let mut menu = KeyboardOptionsMenu::new();
        menu.selected_command = Some(MappableKeyType::Attack);
        menu.text_entry_value = "Shift+X".to_string();

        assert!(menu.assign_hotkey());

        let binding = menu.meta_map.get_binding(MappableKeyType::Attack).unwrap();
        assert_eq!(binding.key, "X");
        assert!(binding.modifiers.shift);
    }
}
