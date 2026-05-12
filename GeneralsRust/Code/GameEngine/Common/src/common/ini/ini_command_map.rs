//! INI parser for CommandMap (MetaMap) key bindings
//!
//! Corresponds to C++ INI::parseMetaMapDefinition in MetaEvent.cpp
//! Parses keyboard command mappings for game actions.

use crate::common::ini::{ini, FieldParse, INIError, INIResult, LookupListRec, INI};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Modifier key flags (matching C++ ModifierNames / KEY_STATE_* bits)
pub const MODIFIER_NONE: i32 = 0;
pub const MODIFIER_SHIFT: i32 = 0x0010;
pub const MODIFIER_CTRL: i32 = 0x0004;
pub const MODIFIER_ALT: i32 = 0x0040;

/// Transition types (matching C++ TransitionNames)
pub const TRANSITION_DOWN: i32 = 0;
pub const TRANSITION_UP: i32 = 1;
pub const TRANSITION_DOUBLEDOWN: i32 = 2;
pub const TRANSITION_REPEAT: i32 = TRANSITION_DOUBLEDOWN;

/// Command usable in locations (matching C++ TheCommandUsableInNames)
pub static COMMAND_USABLE_IN_NAMES: &[&str] = &[
    "SHELL", "GAME", NULL, // terminator
];

const NULL: &str = "\0";

/// Category types (matching C++ CategoryListName)
pub static CATEGORY_NAMES: &[&str] = &[
    "CONTROL",
    "INFORMATION",
    "INTERFACE",
    "SELECTION",
    "TAUNT",
    "TEAM",
    "MISC",
    "DEBUG",
    NULL,
];

/// Key names lookup table (matching C++ KeyNames / DirectInput scan codes)
pub static KEY_NAMES: &[LookupListRec] = &[
    LookupListRec {
        name: "KEY_NONE",
        value: 0,
    },
    LookupListRec {
        name: "KEY_ESC",
        value: 1,
    },
    LookupListRec {
        name: "KEY_ESCAPE",
        value: 1,
    },
    LookupListRec {
        name: "KEY_BACKSPACE",
        value: 14,
    },
    LookupListRec {
        name: "KEY_ENTER",
        value: 28,
    },
    LookupListRec {
        name: "KEY_RETURN",
        value: 28,
    },
    LookupListRec {
        name: "KEY_SPACE",
        value: 57,
    },
    LookupListRec {
        name: "KEY_TAB",
        value: 15,
    },
    LookupListRec {
        name: "KEY_F1",
        value: 59,
    },
    LookupListRec {
        name: "KEY_F2",
        value: 60,
    },
    LookupListRec {
        name: "KEY_F3",
        value: 61,
    },
    LookupListRec {
        name: "KEY_F4",
        value: 62,
    },
    LookupListRec {
        name: "KEY_F5",
        value: 63,
    },
    LookupListRec {
        name: "KEY_F6",
        value: 64,
    },
    LookupListRec {
        name: "KEY_F7",
        value: 65,
    },
    LookupListRec {
        name: "KEY_F8",
        value: 66,
    },
    LookupListRec {
        name: "KEY_F9",
        value: 67,
    },
    LookupListRec {
        name: "KEY_F10",
        value: 68,
    },
    LookupListRec {
        name: "KEY_F11",
        value: 87,
    },
    LookupListRec {
        name: "KEY_F12",
        value: 88,
    },
    LookupListRec {
        name: "KEY_1",
        value: 2,
    },
    LookupListRec {
        name: "KEY_2",
        value: 3,
    },
    LookupListRec {
        name: "KEY_3",
        value: 4,
    },
    LookupListRec {
        name: "KEY_4",
        value: 5,
    },
    LookupListRec {
        name: "KEY_5",
        value: 6,
    },
    LookupListRec {
        name: "KEY_6",
        value: 7,
    },
    LookupListRec {
        name: "KEY_7",
        value: 8,
    },
    LookupListRec {
        name: "KEY_8",
        value: 9,
    },
    LookupListRec {
        name: "KEY_9",
        value: 10,
    },
    LookupListRec {
        name: "KEY_0",
        value: 11,
    },
    LookupListRec {
        name: "KEY_A",
        value: 30,
    },
    LookupListRec {
        name: "KEY_B",
        value: 48,
    },
    LookupListRec {
        name: "KEY_C",
        value: 46,
    },
    LookupListRec {
        name: "KEY_D",
        value: 32,
    },
    LookupListRec {
        name: "KEY_E",
        value: 18,
    },
    LookupListRec {
        name: "KEY_F",
        value: 33,
    },
    LookupListRec {
        name: "KEY_G",
        value: 34,
    },
    LookupListRec {
        name: "KEY_H",
        value: 35,
    },
    LookupListRec {
        name: "KEY_I",
        value: 23,
    },
    LookupListRec {
        name: "KEY_J",
        value: 36,
    },
    LookupListRec {
        name: "KEY_K",
        value: 37,
    },
    LookupListRec {
        name: "KEY_L",
        value: 38,
    },
    LookupListRec {
        name: "KEY_M",
        value: 50,
    },
    LookupListRec {
        name: "KEY_N",
        value: 49,
    },
    LookupListRec {
        name: "KEY_O",
        value: 24,
    },
    LookupListRec {
        name: "KEY_P",
        value: 25,
    },
    LookupListRec {
        name: "KEY_Q",
        value: 16,
    },
    LookupListRec {
        name: "KEY_R",
        value: 19,
    },
    LookupListRec {
        name: "KEY_S",
        value: 31,
    },
    LookupListRec {
        name: "KEY_T",
        value: 20,
    },
    LookupListRec {
        name: "KEY_U",
        value: 22,
    },
    LookupListRec {
        name: "KEY_V",
        value: 47,
    },
    LookupListRec {
        name: "KEY_W",
        value: 17,
    },
    LookupListRec {
        name: "KEY_X",
        value: 45,
    },
    LookupListRec {
        name: "KEY_Y",
        value: 21,
    },
    LookupListRec {
        name: "KEY_Z",
        value: 44,
    },
    LookupListRec {
        name: "KEY_KP1",
        value: 79,
    },
    LookupListRec {
        name: "KEY_KP2",
        value: 80,
    },
    LookupListRec {
        name: "KEY_KP3",
        value: 81,
    },
    LookupListRec {
        name: "KEY_KP4",
        value: 75,
    },
    LookupListRec {
        name: "KEY_KP5",
        value: 76,
    },
    LookupListRec {
        name: "KEY_KP6",
        value: 77,
    },
    LookupListRec {
        name: "KEY_KP7",
        value: 71,
    },
    LookupListRec {
        name: "KEY_KP8",
        value: 72,
    },
    LookupListRec {
        name: "KEY_KP9",
        value: 73,
    },
    LookupListRec {
        name: "KEY_KP0",
        value: 82,
    },
    LookupListRec {
        name: "KEY_MINUS",
        value: 12,
    },
    LookupListRec {
        name: "KEY_EQUAL",
        value: 13,
    },
    LookupListRec {
        name: "KEY_LBRACKET",
        value: 26,
    },
    LookupListRec {
        name: "KEY_RBRACKET",
        value: 27,
    },
    LookupListRec {
        name: "KEY_SEMICOLON",
        value: 39,
    },
    LookupListRec {
        name: "KEY_APOSTROPHE",
        value: 40,
    },
    LookupListRec {
        name: "KEY_TICK",
        value: 41,
    },
    LookupListRec {
        name: "KEY_BACKSLASH",
        value: 43,
    },
    LookupListRec {
        name: "KEY_COMMA",
        value: 51,
    },
    LookupListRec {
        name: "KEY_PERIOD",
        value: 52,
    },
    LookupListRec {
        name: "KEY_SLASH",
        value: 53,
    },
    LookupListRec {
        name: "KEY_INSERT",
        value: 210,
    },
    LookupListRec {
        name: "KEY_INS",
        value: 210,
    },
    LookupListRec {
        name: "KEY_DELETE",
        value: 211,
    },
    LookupListRec {
        name: "KEY_DEL",
        value: 211,
    },
    LookupListRec {
        name: "KEY_HOME",
        value: 199,
    },
    LookupListRec {
        name: "KEY_END",
        value: 207,
    },
    LookupListRec {
        name: "KEY_PAGEUP",
        value: 201,
    },
    LookupListRec {
        name: "KEY_PGUP",
        value: 201,
    },
    LookupListRec {
        name: "KEY_PAGEDOWN",
        value: 209,
    },
    LookupListRec {
        name: "KEY_PGDN",
        value: 209,
    },
    LookupListRec {
        name: "KEY_UP",
        value: 200,
    },
    LookupListRec {
        name: "KEY_DOWN",
        value: 208,
    },
    LookupListRec {
        name: "KEY_LEFT",
        value: 203,
    },
    LookupListRec {
        name: "KEY_RIGHT",
        value: 205,
    },
    LookupListRec {
        name: "KEY_KPSLASH",
        value: 181,
    },
];

/// Transition names lookup
pub static TRANSITION_NAMES: &[LookupListRec] = &[
    LookupListRec {
        name: "DOWN",
        value: TRANSITION_DOWN,
    },
    LookupListRec {
        name: "UP",
        value: TRANSITION_UP,
    },
    LookupListRec {
        name: "DOUBLEDOWN",
        value: TRANSITION_DOUBLEDOWN,
    },
];

/// Modifier names lookup
pub static MODIFIER_NAMES: &[LookupListRec] = &[
    LookupListRec {
        name: "NONE",
        value: MODIFIER_NONE,
    },
    LookupListRec {
        name: "SHIFT",
        value: MODIFIER_SHIFT,
    },
    LookupListRec {
        name: "CTRL",
        value: MODIFIER_CTRL,
    },
    LookupListRec {
        name: "ALT",
        value: MODIFIER_ALT,
    },
    LookupListRec {
        name: "SHIFT_CTRL",
        value: MODIFIER_SHIFT | MODIFIER_CTRL,
    },
    LookupListRec {
        name: "SHIFT_ALT",
        value: MODIFIER_SHIFT | MODIFIER_ALT,
    },
    LookupListRec {
        name: "CTRL_ALT",
        value: MODIFIER_CTRL | MODIFIER_ALT,
    },
    LookupListRec {
        name: "SHIFT_ALT_CTRL",
        value: MODIFIER_SHIFT | MODIFIER_CTRL | MODIFIER_ALT,
    },
];

/// Category names lookup
pub static CATEGORY_NAMES_LOOKUP: &[LookupListRec] = &[
    LookupListRec {
        name: "CONTROL",
        value: 0,
    },
    LookupListRec {
        name: "INFORMATION",
        value: 1,
    },
    LookupListRec {
        name: "INTERFACE",
        value: 2,
    },
    LookupListRec {
        name: "SELECTION",
        value: 3,
    },
    LookupListRec {
        name: "TAUNT",
        value: 4,
    },
    LookupListRec {
        name: "TEAM",
        value: 5,
    },
    LookupListRec {
        name: "MISC",
        value: 6,
    },
    LookupListRec {
        name: "DEBUG",
        value: 7,
    },
];

/// Single command map record
/// Matches C++ MetaMapRec
#[derive(Debug, Clone, Default)]
pub struct MetaMapRec {
    pub key: i32,
    pub transition: i32,
    pub mod_state: i32,
    pub usable_in: u32,
    pub category: i32,
    pub description: String,
    pub display_name: String,
    pub action_name: String,
}

/// MetaMap (CommandMap) storage singleton
static META_MAP: OnceLock<RwLock<MetaMap>> = OnceLock::new();

/// MetaMap structure holding all key bindings
#[derive(Debug, Clone, Default)]
pub struct MetaMap {
    /// All command mappings by name
    pub mappings: HashMap<String, MetaMapRec>,
    /// Quick lookup by key+modifiers
    pub key_bindings: HashMap<(i32, i32, i32), String>, // (key, transition, modifiers) -> action_name
}

impl MetaMap {
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
            key_bindings: HashMap::new(),
        }
    }

    /// Add a command mapping
    pub fn add_mapping(&mut self, name: String, rec: MetaMapRec) {
        // Add to key bindings lookup
        let key = (rec.key, rec.transition, rec.mod_state);
        self.key_bindings.insert(key, name.clone());

        // Add to mappings
        self.mappings.insert(name, rec);
    }

    /// Find action by key combination
    pub fn find_action(&self, key: i32, transition: i32, modifiers: i32) -> Option<&String> {
        self.key_bindings.get(&(key, transition, modifiers))
    }

    /// Get mapping by name
    pub fn get_mapping(&self, name: &str) -> Option<&MetaMapRec> {
        self.mappings.get(name)
    }
}

/// Field parse table for MetaMapRec
/// Matches C++ TheMetaMapFieldParseTable
const META_MAP_FIELD_PARSE_TABLE: &[FieldParse<MetaMapRec>] = &[
    FieldParse {
        token: "Key",
        parse: parse_key,
    },
    FieldParse {
        token: "Transition",
        parse: parse_transition,
    },
    FieldParse {
        token: "Modifiers",
        parse: parse_modifiers,
    },
    FieldParse {
        token: "UseableIn",
        parse: parse_usable_in,
    },
    FieldParse {
        token: "Category",
        parse: parse_category,
    },
    FieldParse {
        token: "Description",
        parse: parse_description,
    },
    FieldParse {
        token: "DisplayName",
        parse: parse_display_name,
    },
];

fn parse_key(ini: &mut INI, target: &mut MetaMapRec, _tokens: &[&str]) -> INIResult<()> {
    let token = ini.get_next_token()?;
    target.key = INI::parse_lookup_list(&token, KEY_NAMES)?;
    Ok(())
}

fn parse_transition(ini: &mut INI, target: &mut MetaMapRec, _tokens: &[&str]) -> INIResult<()> {
    let token = ini.get_next_token()?;
    target.transition = INI::parse_lookup_list(&token, TRANSITION_NAMES)?;
    Ok(())
}

fn parse_modifiers(ini: &mut INI, target: &mut MetaMapRec, _tokens: &[&str]) -> INIResult<()> {
    let token = ini.get_next_token()?;
    target.mod_state = INI::parse_lookup_list(&token, MODIFIER_NAMES)?;
    Ok(())
}

fn parse_usable_in(ini: &mut INI, target: &mut MetaMapRec, _tokens: &[&str]) -> INIResult<()> {
    // Parse bit string for usable locations
    target.usable_in = ini.parse_flags_with_list(COMMAND_USABLE_IN_NAMES)?;
    Ok(())
}

fn parse_category(ini: &mut INI, target: &mut MetaMapRec, _tokens: &[&str]) -> INIResult<()> {
    let token = ini.get_next_token()?;
    target.category = INI::parse_lookup_list(&token, CATEGORY_NAMES_LOOKUP)?;
    Ok(())
}

fn parse_description(ini: &mut INI, target: &mut MetaMapRec, _tokens: &[&str]) -> INIResult<()> {
    target.description = ini.parse_and_translate_label()?;
    Ok(())
}

fn parse_display_name(ini: &mut INI, target: &mut MetaMapRec, _tokens: &[&str]) -> INIResult<()> {
    target.display_name = ini.parse_and_translate_label()?;
    Ok(())
}

/// Initialize the MetaMap singleton
pub fn init_meta_map() {
    META_MAP.get_or_init(|| RwLock::new(MetaMap::new()));
}

/// Get a read reference to the MetaMap
pub fn get_meta_map() -> Option<std::sync::RwLockReadGuard<'static, MetaMap>> {
    META_MAP.get()?.read().ok()
}

/// Get a write reference to the MetaMap
pub fn get_meta_map_mut() -> Option<std::sync::RwLockWriteGuard<'static, MetaMap>> {
    META_MAP.get()?.write().ok()
}

/// Parse a CommandMap definition block
/// C++ equivalent: INI::parseMetaMapDefinition
pub fn parse_meta_map_definition(ini: &mut INI) -> INIResult<()> {
    init_meta_map();

    // Get the command name
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

    let mut rec = MetaMapRec::default();
    rec.action_name = name.clone();

    ini.init_from_ini_with_fields_allow_unknown(&mut rec, META_MAP_FIELD_PARSE_TABLE)?;

    if let Some(mut guard) = get_meta_map_mut() {
        guard.add_mapping(name, rec);
    }

    Ok(())
}

/// Register this parser with the INI system
pub fn register_command_map_parser() -> bool {
    crate::common::ini::register_block_parser("CommandMap", parse_meta_map_definition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_map_creation() {
        let mut map = MetaMap::new();
        let rec = MetaMapRec {
            key: 30,
            transition: TRANSITION_DOWN,
            mod_state: MODIFIER_CTRL,
            usable_in: 0xFFFFFFFF,
            category: 0,
            description: "Select All".to_string(),
            display_name: "Select All".to_string(),
            action_name: "SELECT_ALL".to_string(),
        };

        map.add_mapping("SELECT_ALL".to_string(), rec.clone());

        assert!(map.get_mapping("SELECT_ALL").is_some());
        assert!(map
            .find_action(30, TRANSITION_DOWN, MODIFIER_CTRL)
            .is_some());
    }

    #[test]
    fn test_lookup_lists_match_cpp_meta_event_tables() {
        assert_eq!(INI::parse_lookup_list("KEY_A", KEY_NAMES).unwrap(), 30);
        assert_eq!(INI::parse_lookup_list("KEY_F1", KEY_NAMES).unwrap(), 59);
        assert_eq!(INI::parse_lookup_list("KEY_KP2", KEY_NAMES).unwrap(), 80);
        assert_eq!(
            INI::parse_lookup_list("KEY_KPSLASH", KEY_NAMES).unwrap(),
            181
        );
        assert_eq!(
            INI::parse_lookup_list("DOWN", TRANSITION_NAMES).unwrap(),
            TRANSITION_DOWN
        );
        assert_eq!(
            INI::parse_lookup_list("SHIFT_CTRL", MODIFIER_NAMES).unwrap(),
            MODIFIER_SHIFT | MODIFIER_CTRL
        );
        assert_eq!(
            INI::parse_lookup_list("TEAM", CATEGORY_NAMES_LOOKUP).unwrap(),
            5
        );
    }

    #[test]
    fn test_command_usable_in_bits_match_cpp_order() {
        assert_eq!(
            INI::parse_bit_string_32(&["SHELL"], COMMAND_USABLE_IN_NAMES).unwrap(),
            1
        );
        assert_eq!(
            INI::parse_bit_string_32(&["GAME"], COMMAND_USABLE_IN_NAMES).unwrap(),
            2
        );
        assert_eq!(
            INI::parse_bit_string_32(&["GAME", "SHELL"], COMMAND_USABLE_IN_NAMES).unwrap(),
            3
        );
    }

    #[test]
    fn test_case_insensitive_legacy_names_still_parse() {
        assert_eq!(
            INI::parse_lookup_list("Down", TRANSITION_NAMES).unwrap(),
            TRANSITION_DOWN
        );
        assert_eq!(
            INI::parse_lookup_list("Ctrl", MODIFIER_NAMES).unwrap(),
            MODIFIER_CTRL
        );
    }
}
