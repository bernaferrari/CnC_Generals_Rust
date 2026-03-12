// FILE: skirmish_preferences.rs
// Author: Rust port
// Description: Skirmish preferences storage and retrieval
//
// Ported from: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp
// (SkirmishPreferences class)

use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use super::skirmish_game_options_menu::{Money, PLAYERTEMPLATE_RANDOM};
use super::skirmish_map_select_menu::{get_default_map, is_valid_map, MapCache};

const SUPERWEAPON_RESTRICTION_KEY: &str = "SuperweaponRestrict";
const STARTING_CASH_KEY: &str = "StartingCash";

// User preferences base class
pub trait UserPreferences {
    fn get_preferences_dir() -> PathBuf {
        // Get user's home directory
        if let Some(home) = std::env::var_os("HOME") {
            let mut path = PathBuf::from(home);
            path.push(".generals");
            path
        } else if let Some(appdata) = std::env::var_os("APPDATA") {
            let mut path = PathBuf::from(appdata);
            path.push("Generals");
            path
        } else {
            PathBuf::from(".")
        }
    }

    fn write(&self, filename: &str, data: &HashMap<String, String>) -> bool {
        let prefs_dir = Self::get_preferences_dir();
        if let Err(_) = create_dir_all(&prefs_dir) {
            return false;
        }

        let filepath = prefs_dir.join(filename);
        let mut file = match File::create(&filepath) {
            Ok(f) => f,
            Err(_) => return false,
        };

        for (key, value) in data {
            if let Err(_) = writeln!(file, "{}={}", key, value) {
                return false;
            }
        }

        true
    }

    fn load(&mut self, filename: &str) -> HashMap<String, String> {
        let prefs_dir = Self::get_preferences_dir();
        let filepath = prefs_dir.join(filename);

        let mut data = HashMap::new();

        if let Ok(file) = File::open(&filepath) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Some(pos) = line.find('=') {
                        let key = line[..pos].trim().to_string();
                        let value = line[pos + 1..].trim().to_string();
                        data.insert(key, value);
                    }
                }
            }
        }

        data
    }
}

// Skirmish preferences
pub struct SkirmishPreferences {
    data: HashMap<String, String>,
    filename: String,
}

impl SkirmishPreferences {
    pub fn new() -> Self {
        let mut prefs = SkirmishPreferences {
            data: HashMap::new(),
            filename: "Skirmish.ini".to_string(),
        };
        prefs.load_data();
        prefs
    }

    fn load_data(&mut self) {
        self.data = self.load(&self.filename);
    }

    pub fn write_data(&self) -> bool {
        self.write(&self.filename, &self.data)
    }

    // Get slot list (serialized game info)
    pub fn get_slot_list(&self) -> String {
        self.data.get("SlotList")
            .cloned()
            .unwrap_or_default()
    }

    // Set slot list
    pub fn set_slot_list(&mut self, slot_list: String) {
        self.data.insert("SlotList".to_string(), slot_list);
    }

    // Get user name
    pub fn get_user_name(&self) -> String {
        if let Some(name) = self.data.get("UserName") {
            let decoded = Self::quoted_printable_to_string(name);
            let trimmed = decoded.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        // Default to machine name
        Self::get_machine_name()
    }

    // Set user name
    pub fn set_user_name(&mut self, name: String) {
        let encoded = Self::string_to_quoted_printable(&name);
        self.data.insert("UserName".to_string(), encoded);
    }

    // Get preferred color
    pub fn get_preferred_color(&self) -> i32 {
        if let Some(color_str) = self.data.get("Color") {
            if let Ok(color) = color_str.parse::<i32>() {
                // Validate color range (assume 16 colors max)
                if color >= -1 && color < 16 {
                    return color;
                }
            }
        }
        -1 // Random color
    }

    // Set preferred color
    pub fn set_preferred_color(&mut self, color: i32) {
        self.data.insert("Color".to_string(), color.to_string());
    }

    // Get preferred faction (player template)
    pub fn get_preferred_faction(&self) -> i32 {
        if let Some(faction_str) = self.data.get("PlayerTemplate") {
            if let Ok(faction) = faction_str.parse::<i32>() {
                // Validate faction (assume 10 factions max including random)
                if faction >= PLAYERTEMPLATE_RANDOM && faction < 10 {
                    return faction;
                }
            }
        }
        PLAYERTEMPLATE_RANDOM
    }

    // Set preferred faction
    pub fn set_preferred_faction(&mut self, faction: i32) {
        self.data.insert("PlayerTemplate".to_string(), faction.to_string());
    }

    // Check if uses system map directory
    pub fn uses_system_map_dir(&self) -> bool {
        if let Some(value) = self.data.get("UseSystemMapDir") {
            value.eq_ignore_ascii_case("yes")
        } else {
            true // Default to system maps
        }
    }

    // Set use system map directory
    pub fn set_use_system_map_dir(&mut self, use_system: bool) {
        self.data.insert(
            "UseSystemMapDir".to_string(),
            if use_system { "yes" } else { "no" }.to_string(),
        );
    }

    // Get preferred map
    pub fn get_preferred_map(&self, cache: &MapCache) -> String {
        if let Some(map_str) = self.data.get("Map") {
            let decoded = Self::quoted_printable_to_string(map_str);
            let trimmed = decoded.trim();
            if !trimmed.is_empty() && is_valid_map(trimmed, true, cache) {
                return trimmed.to_string();
            }
        }

        // Return default map
        get_default_map(true)
    }

    // Set preferred map
    pub fn set_preferred_map(&mut self, map: String) {
        let encoded = Self::string_to_quoted_printable(&map);
        self.data.insert("Map".to_string(), encoded);
    }

    // Get superweapon restriction setting
    pub fn get_superweapon_restricted(&self) -> bool {
        if let Some(value) = self.data.get(SUPERWEAPON_RESTRICTION_KEY) {
            value.eq_ignore_ascii_case("yes")
        } else {
            false
        }
    }

    // Set superweapon restriction
    pub fn set_superweapon_restricted(&mut self, restricted: bool) {
        self.data.insert(
            SUPERWEAPON_RESTRICTION_KEY.to_string(),
            if restricted { "yes" } else { "no" }.to_string(),
        );
    }

    // Get starting cash
    pub fn get_starting_cash(&self, default_money: u32) -> Money {
        if let Some(cash_str) = self.data.get(STARTING_CASH_KEY) {
            if let Ok(amount) = cash_str.parse::<u32>() {
                let mut money = Money::new();
                money.deposit(amount, false);
                return money;
            }
        }

        let mut money = Money::new();
        money.deposit(default_money, false);
        money
    }

    // Set starting cash
    pub fn set_starting_cash(&mut self, cash: Money) {
        self.data.insert(
            STARTING_CASH_KEY.to_string(),
            cash.count_money().to_string(),
        );
    }

    // Get integer value
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        if let Some(value) = self.data.get(key) {
            value.parse::<i32>().unwrap_or(default)
        } else {
            default
        }
    }

    // Set integer value
    pub fn set_int(&mut self, key: &str, value: i32) {
        self.data.insert(key.to_string(), value.to_string());
    }

    // Get string value
    pub fn get_string(&self, key: &str, default: &str) -> String {
        self.data.get(key).cloned().unwrap_or_else(|| default.to_string())
    }

    // Set string value
    pub fn set_string(&mut self, key: &str, value: String) {
        self.data.insert(key.to_string(), value);
    }

    // Helper: Convert string to quoted-printable encoding (simplified)
    fn string_to_quoted_printable(s: &str) -> String {
        // Simplified implementation - just URL encode special characters
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                    c.to_string()
                } else {
                    format!("={:02X}", c as u8)
                }
            })
            .collect()
    }

    // Helper: Convert quoted-printable to string (simplified)
    fn quoted_printable_to_string(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '=' {
                // Try to read two hex digits
                let mut hex = String::new();
                if let Some(h1) = chars.next() {
                    hex.push(h1);
                    if let Some(h2) = chars.next() {
                        hex.push(h2);
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                            continue;
                        }
                    }
                }
                // Failed to decode, just add the =
                result.push(c);
            } else {
                result.push(c);
            }
        }

        result
    }

    // Helper: Get machine name
    fn get_machine_name() -> String {
        if let Ok(hostname) = std::env::var("HOSTNAME") {
            hostname
        } else if let Ok(computer_name) = std::env::var("COMPUTERNAME") {
            computer_name
        } else if let Ok(name) = std::env::var("USER") {
            name
        } else {
            "Player".to_string()
        }
    }
}

impl Default for SkirmishPreferences {
    fn default() -> Self {
        Self::new()
    }
}

impl UserPreferences for SkirmishPreferences {}

// Game info serialization helpers

// Convert GameInfo to ASCII string for storage
pub fn game_info_to_ascii_string(game_info: &super::skirmish_game_options_menu::SkirmishGameInfo) -> String {
    let mut result = String::new();

    // Serialize each slot
    for i in 0..super::skirmish_game_options_menu::MAX_SLOTS {
        if let Some(slot) = game_info.get_const_slot(i) {
            if i > 0 {
                result.push('|');
            }

            // Format: state,color,template,team,pos
            result.push_str(&format!(
                "{},{},{},{},{}",
                slot.get_state() as i32,
                slot.get_color(),
                slot.get_player_template(),
                slot.get_team_number(),
                slot.get_start_pos()
            ));
        }
    }

    result
}

// Parse ASCII string to GameInfo
pub fn parse_ascii_string_to_game_info(
    game_info: &mut super::skirmish_game_options_menu::SkirmishGameInfo,
    ascii_string: String,
) {
    if ascii_string.is_empty() {
        return;
    }

    let slots: Vec<&str> = ascii_string.split('|').collect();

    for (i, slot_str) in slots.iter().enumerate() {
        if i >= super::skirmish_game_options_menu::MAX_SLOTS {
            break;
        }

        let parts: Vec<&str> = slot_str.split(',').collect();
        if parts.len() >= 5 {
            if let Some(slot) = game_info.get_slot(i) {
                // Parse state
                if let Ok(state) = parts[0].parse::<i32>() {
                    slot.set_state(
                        super::skirmish_game_options_menu::SlotState::from_i32(state),
                        String::new(),
                    );
                }

                // Parse color
                if let Ok(color) = parts[1].parse::<i32>() {
                    slot.set_color(color);
                }

                // Parse template
                if let Ok(template) = parts[2].parse::<i32>() {
                    slot.set_player_template(template);
                }

                // Parse team
                if let Ok(team) = parts[3].parse::<i32>() {
                    slot.set_team_number(team);
                }

                // Parse position
                if let Ok(pos) = parts[4].parse::<i32>() {
                    slot.set_start_pos(pos);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_creation() {
        let prefs = SkirmishPreferences::new();
        assert_eq!(prefs.filename, "Skirmish.ini");
    }

    #[test]
    fn test_quoted_printable_encoding() {
        let input = "Player 1";
        let encoded = SkirmishPreferences::string_to_quoted_printable(input);
        let decoded = SkirmishPreferences::quoted_printable_to_string(&encoded);
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_quoted_printable_special_chars() {
        let input = "Test@User#123";
        let encoded = SkirmishPreferences::string_to_quoted_printable(input);
        let decoded = SkirmishPreferences::quoted_printable_to_string(&encoded);
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_get_set_user_name() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_user_name("TestPlayer".to_string());
        assert_eq!(prefs.get_user_name(), "TestPlayer");
    }

    #[test]
    fn test_get_set_color() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_preferred_color(5);
        assert_eq!(prefs.get_preferred_color(), 5);
    }

    #[test]
    fn test_get_set_faction() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_preferred_faction(3);
        assert_eq!(prefs.get_preferred_faction(), 3);
    }

    #[test]
    fn test_get_set_map() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_preferred_map("tournament_desert.map".to_string());

        // Note: This would require a valid cache to fully test
        // For now, just verify the storage works
        assert!(prefs.data.contains_key("Map"));
    }

    #[test]
    fn test_superweapon_restriction() {
        let mut prefs = SkirmishPreferences::new();

        prefs.set_superweapon_restricted(true);
        assert!(prefs.get_superweapon_restricted());

        prefs.set_superweapon_restricted(false);
        assert!(!prefs.get_superweapon_restricted());
    }

    #[test]
    fn test_starting_cash() {
        let mut prefs = SkirmishPreferences::new();
        let mut cash = Money::new();
        cash.deposit(10000, false);

        prefs.set_starting_cash(cash);
        let loaded = prefs.get_starting_cash(5000);
        assert_eq!(loaded.count_money(), 10000);
    }

    #[test]
    fn test_starting_cash_default() {
        let prefs = SkirmishPreferences::new();
        let loaded = prefs.get_starting_cash(7500);
        assert_eq!(loaded.count_money(), 7500); // Should use default
    }

    #[test]
    fn test_get_set_int() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_int("FPS", 60);
        assert_eq!(prefs.get_int("FPS", 30), 60);
        assert_eq!(prefs.get_int("NonExistent", 99), 99);
    }

    #[test]
    fn test_get_set_string() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_string("TestKey", "TestValue".to_string());
        assert_eq!(prefs.get_string("TestKey", "Default"), "TestValue");
        assert_eq!(prefs.get_string("NonExistent", "Default"), "Default");
    }

    #[test]
    fn test_use_system_map_dir() {
        let mut prefs = SkirmishPreferences::new();

        prefs.set_use_system_map_dir(false);
        assert!(!prefs.uses_system_map_dir());

        prefs.set_use_system_map_dir(true);
        assert!(prefs.uses_system_map_dir());
    }

    #[test]
    fn test_game_info_serialization() {
        use super::super::skirmish_game_options_menu::{SkirmishGameInfo, SlotState};

        let mut game_info = SkirmishGameInfo::new();

        // Set up slot 0
        if let Some(slot) = game_info.get_slot(0) {
            slot.set_state(SlotState::Player, "Player1".to_string());
            slot.set_color(1);
            slot.set_player_template(2);
            slot.set_team_number(0);
            slot.set_start_pos(0);
        }

        // Serialize
        let serialized = game_info_to_ascii_string(&game_info);
        assert!(!serialized.is_empty());

        // Deserialize to new game info
        let mut new_game_info = SkirmishGameInfo::new();
        parse_ascii_string_to_game_info(&mut new_game_info, serialized);

        // Verify
        if let Some(slot) = new_game_info.get_const_slot(0) {
            assert_eq!(slot.get_state(), SlotState::Player);
            assert_eq!(slot.get_color(), 1);
            assert_eq!(slot.get_player_template(), 2);
            assert_eq!(slot.get_team_number(), 0);
            assert_eq!(slot.get_start_pos(), 0);
        }
    }

    #[test]
    fn test_machine_name_fallback() {
        let name = SkirmishPreferences::get_machine_name();
        assert!(!name.is_empty());
    }

    #[test]
    fn test_color_validation() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_preferred_color(-1); // Random
        assert_eq!(prefs.get_preferred_color(), -1);

        prefs.set_preferred_color(0);
        assert_eq!(prefs.get_preferred_color(), 0);

        prefs.set_preferred_color(15);
        assert_eq!(prefs.get_preferred_color(), 15);
    }

    #[test]
    fn test_empty_slot_list() {
        let prefs = SkirmishPreferences::new();
        let slot_list = prefs.get_slot_list();
        assert_eq!(slot_list, "");
    }

    #[test]
    fn test_set_slot_list() {
        let mut prefs = SkirmishPreferences::new();
        prefs.set_slot_list("5,1,2,0,0|2,-1,-1,-1,-1".to_string());
        assert_eq!(prefs.get_slot_list(), "5,1,2,0,0|2,-1,-1,-1,-1");
    }
}
