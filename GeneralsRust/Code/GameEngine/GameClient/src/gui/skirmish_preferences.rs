//! Skirmish preference storage (Skirmish.ini).

use crate::map_util::{get_default_map, is_valid_map};
use game_engine::common::rts::player_template::get_player_template_store;
use game_network::{Money, PLAYERTEMPLATE_RANDOM};
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const SLOT_LIST_KEY: &str = "SlotList";
const USER_NAME_KEY: &str = "UserName";
const COLOR_KEY: &str = "Color";
const PLAYER_TEMPLATE_KEY: &str = "PlayerTemplate";
const USE_SYSTEM_MAP_DIR_KEY: &str = "UseSystemMapDir";
const MAP_KEY: &str = "Map";
const SUPERWEAPON_RESTRICTION_KEY: &str = "SuperweaponRestrict";
const STARTING_CASH_KEY: &str = "StartingCash";

#[derive(Debug, Default)]
pub struct SkirmishPreferences {
    data: HashMap<String, String>,
}

impl SkirmishPreferences {
    pub fn new() -> Self {
        let mut prefs = Self {
            data: HashMap::new(),
        };
        prefs.read_data();
        prefs
    }

    pub fn write(&self) {
        let path = preferences_file();
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }
        if let Ok(mut file) = File::create(&path) {
            for (key, value) in &self.data {
                let _ = writeln!(file, "{}={}", key, value);
            }
        }
    }

    fn read_data(&mut self) {
        let path = preferences_file();
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return,
        };
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Some((key, value)) = line.split_once('=') {
                self.data
                    .insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    pub fn get_slot_list(&self) -> String {
        self.data.get(SLOT_LIST_KEY).cloned().unwrap_or_default()
    }

    pub fn set_slot_list(&mut self, value: String) {
        self.data.insert(SLOT_LIST_KEY.to_string(), value);
    }

    pub fn get_user_name(&self) -> String {
        let stored = self
            .data
            .get(USER_NAME_KEY)
            .map(|value| quoted_printable_decode(value))
            .unwrap_or_default();
        let trimmed = stored.trim();
        if trimmed.is_empty() {
            get_machine_name()
        } else {
            trimmed.to_string()
        }
    }

    pub fn set_user_name(&mut self, value: String) {
        let encoded = quoted_printable_encode(&value);
        self.data.insert(USER_NAME_KEY.to_string(), encoded);
    }

    pub fn get_preferred_color(&self) -> i32 {
        self.data
            .get(COLOR_KEY)
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|value| *value >= -1)
            .unwrap_or(-1)
    }

    pub fn set_preferred_color(&mut self, value: i32) {
        self.data.insert(COLOR_KEY.to_string(), value.to_string());
    }

    pub fn get_preferred_faction(&self) -> i32 {
        let parsed = self
            .data
            .get(PLAYER_TEMPLATE_KEY)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(PLAYERTEMPLATE_RANDOM);
        if parsed <= PLAYERTEMPLATE_RANDOM {
            return PLAYERTEMPLATE_RANDOM;
        }
        let store = get_player_template_store();
        let index = parsed as usize;
        if store
            .get_nth_player_template(index)
            .map(|template| template.playable)
            == Some(true)
        {
            parsed
        } else {
            PLAYERTEMPLATE_RANDOM
        }
    }

    pub fn set_preferred_faction(&mut self, value: i32) {
        self.data
            .insert(PLAYER_TEMPLATE_KEY.to_string(), value.to_string());
    }

    pub fn uses_system_map_dir(&self) -> bool {
        self.data
            .get(USE_SYSTEM_MAP_DIR_KEY)
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(true)
    }

    pub fn set_use_system_map_dir(&mut self, value: bool) {
        self.data.insert(
            USE_SYSTEM_MAP_DIR_KEY.to_string(),
            if value { "yes" } else { "no" }.to_string(),
        );
    }

    pub fn get_preferred_map(&self) -> String {
        let stored = self
            .data
            .get(MAP_KEY)
            .map(|value| quoted_printable_decode(value))
            .unwrap_or_default();
        let trimmed = stored.trim();
        if !trimmed.is_empty() && is_valid_map(trimmed, true) {
            trimmed.to_string()
        } else {
            get_default_map(true)
        }
    }

    pub fn set_preferred_map(&mut self, value: String) {
        let encoded = quoted_printable_encode(&value);
        self.data.insert(MAP_KEY.to_string(), encoded);
    }

    pub fn get_superweapon_restricted(&self) -> bool {
        self.data
            .get(SUPERWEAPON_RESTRICTION_KEY)
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
    }

    pub fn set_superweapon_restricted(&mut self, value: bool) {
        self.data.insert(
            SUPERWEAPON_RESTRICTION_KEY.to_string(),
            if value { "yes" } else { "no" }.to_string(),
        );
    }

    pub fn get_starting_cash(&self) -> Money {
        let value = self
            .data
            .get(STARTING_CASH_KEY)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(10000);
        Money::new(value)
    }

    pub fn set_starting_cash(&mut self, value: Money) {
        self.data.insert(
            STARTING_CASH_KEY.to_string(),
            value.count_money().to_string(),
        );
    }

    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.data
            .get(key)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(default)
    }

    pub fn set_int(&mut self, key: &str, value: i32) {
        self.data.insert(key.to_string(), value.to_string());
    }
}

fn preferences_file() -> PathBuf {
    let mut path = if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
    } else if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata)
    } else {
        PathBuf::from(".")
    };
    path.push(".generals");
    path.push("Skirmish.ini");
    path
}

fn get_machine_name() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        return name;
    }
    if let Ok(name) = std::env::var("HOSTNAME") {
        return name;
    }
    if let Ok(name) = std::env::var("USER") {
        return name;
    }
    "Player".to_string()
}

fn quoted_printable_encode(input: &str) -> String {
    let mut output = String::new();
    for &byte in input.as_bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || ch == ' ' {
            output.push(ch);
        } else {
            output.push('=');
            output.push_str(&format!("{:02X}", byte));
        }
    }
    output
}

fn quoted_printable_decode(input: &str) -> String {
    let mut output: Vec<u8> = Vec::new();
    let bytes = input.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if byte == b'=' && idx + 2 < bytes.len() {
            let hi = bytes[idx + 1] as char;
            let lo = bytes[idx + 2] as char;
            if let Ok(decoded) = u8::from_str_radix(&format!("{}{}", hi, lo), 16) {
                output.push(decoded);
                idx += 3;
                continue;
            }
        }
        output.push(byte);
        idx += 1;
    }
    String::from_utf8_lossy(&output).to_string()
}
