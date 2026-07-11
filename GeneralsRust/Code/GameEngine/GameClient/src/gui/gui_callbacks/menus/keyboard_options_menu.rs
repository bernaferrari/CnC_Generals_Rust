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

impl Default for KeyModifiers {
    fn default() -> Self {
        Self::new()
    }
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
                    if i == parts.len() - 1 {
                        key = part.to_string();
                    } else {
                        return None;
                    }
                }
            }
        }

        if key.is_empty() {
            return None;
        }

        Some(HotkeyBinding::new(key, modifiers))
    }
}

/// Keyboard options state
pub struct KeyboardOptionsState {
    bindings: HashMap<MappableKeyType, HotkeyBinding>,
    category: MappableKeyCategory,
    waiting_for_key: Bool,
    pending_key: Option<MappableKeyType>,
}

impl Default for KeyboardOptionsState {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardOptionsState {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            category: MappableKeyCategory::Control,
            waiting_for_key: false,
            pending_key: None,
        }
    }

    pub fn set_category(&mut self, category: MappableKeyCategory) {
        self.category = category;
    }

    pub fn category(&self) -> MappableKeyCategory {
        self.category
    }

    pub fn set_binding(&mut self, key_type: MappableKeyType, binding: HotkeyBinding) {
        self.bindings.insert(key_type, binding);
    }

    pub fn get_binding(&self, key_type: MappableKeyType) -> Option<&HotkeyBinding> {
        self.bindings.get(&key_type)
    }
}

pub fn keyboard_options_init(state: &mut KeyboardOptionsState) {
    state.waiting_for_key = false;
    state.pending_key = None;
    state.category = MappableKeyCategory::Control;
}

pub fn keyboard_options_request_rebind(state: &mut KeyboardOptionsState, key: MappableKeyType) {
    state.waiting_for_key = true;
    state.pending_key = Some(key);
}

pub fn keyboard_options_apply_rebind(state: &mut KeyboardOptionsState, binding: HotkeyBinding) {
    if let Some(key) = state.pending_key.take() {
        state.set_binding(key, binding);
    }
    state.waiting_for_key = false;
}
