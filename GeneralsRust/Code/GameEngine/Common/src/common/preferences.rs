//! Preferences Module
//!
//! Provides various preference classes for different game modes:
//! - SkirmishPreferences: Single-player skirmish settings
//! - LadderPreferences: Ranked ladder match settings
//! - QuickmatchPreferences: Quick match settings
//! - CustomMatchPreferences: Custom multiplayer settings
//! - IgnorePreferences: Player ignore list settings

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::common::ini::ini_challenge_generals::get_challenge_generals;
use crate::common::ini::ini_map_cache::{get_map_cache, init_global_map_cache};
use crate::common::ini::ini_multiplayer::with_multiplayer_settings;
use crate::common::ini::ini_webpage_url::get_registry_language;
use crate::common::rts::money::Money;
use crate::common::rts::player_template::get_player_template_store;
use crate::common::system::quoted_printable::{
    ascii_string_to_quoted_printable, quoted_printable_to_ascii_string,
    quoted_printable_to_unicode_string, unicode_string_to_quoted_printable,
};
use crate::common::user_preferences::UserPreferences;
use crate::game_network::gamespy::peer_defs::get_gamespy_info;
use crate::game_network::{PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM};

/// Base preference value type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PreferenceValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
}

impl PreferenceValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PreferenceValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            PreferenceValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            PreferenceValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PreferenceValue::String(v) => Some(v.as_str()),
            _ => None,
        }
    }
}

/// Difficulty level for AI opponents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Brutal,
}

impl Default for Difficulty {
    fn default() -> Self {
        Difficulty::Medium
    }
}

/// Map size preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapSize {
    Small,
    Medium,
    Large,
    Huge,
}

impl Default for MapSize {
    fn default() -> Self {
        MapSize::Medium
    }
}

/// Starting resources setting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartingResources {
    Low,
    Standard,
    High,
    Unlimited,
}

impl Default for StartingResources {
    fn default() -> Self {
        StartingResources::Standard
    }
}

/// Victory condition type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VictoryCondition {
    Domination,
    Economic,
    TimeLimit,
    ObjectiveBased,
}

impl Default for VictoryCondition {
    fn default() -> Self {
        VictoryCondition::Domination
    }
}

/// Skirmish game preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkirmishPreferences {
    /// Selected map name
    pub map_name: String,
    /// Player's faction/side
    pub player_faction: String,
    /// Player's color index
    pub player_color: i32,
    /// AI difficulty level
    pub ai_difficulty: Difficulty,
    /// Number of AI opponents
    pub num_ai_opponents: i32,
    /// Starting resources setting
    pub starting_resources: StartingResources,
    /// Victory condition
    pub victory_condition: VictoryCondition,
    /// Time limit in minutes (0 = no limit)
    pub time_limit: i32,
    /// Enable fog of war
    pub fog_of_war: bool,
    /// Allow observers
    pub allow_observers: bool,
    /// Random map seed
    pub map_seed: i32,
    /// AI personalities
    pub ai_personalities: Vec<String>,
}

impl Default for SkirmishPreferences {
    fn default() -> Self {
        Self {
            map_name: String::from("Tournament_Desert"),
            player_faction: String::from("USA"),
            player_color: 0,
            ai_difficulty: Difficulty::default(),
            num_ai_opponents: 1,
            starting_resources: StartingResources::default(),
            victory_condition: VictoryCondition::default(),
            time_limit: 0,
            fog_of_war: true,
            allow_observers: false,
            map_seed: 0,
            ai_personalities: vec![String::from("Aggressive")],
        }
    }
}

impl SkirmishPreferences {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, data: HashMap<String, PreferenceValue>) {
        for (key, value) in data {
            match key.as_str() {
                "map_name" => {
                    if let Some(v) = value.as_str() {
                        self.map_name = v.to_string();
                    }
                }
                "player_faction" => {
                    if let Some(v) = value.as_str() {
                        self.player_faction = v.to_string();
                    }
                }
                "player_color" => {
                    if let Some(v) = value.as_int() {
                        self.player_color = v;
                    }
                }
                "num_ai_opponents" => {
                    if let Some(v) = value.as_int() {
                        self.num_ai_opponents = v;
                    }
                }
                "time_limit" => {
                    if let Some(v) = value.as_int() {
                        self.time_limit = v;
                    }
                }
                "fog_of_war" => {
                    if let Some(v) = value.as_bool() {
                        self.fog_of_war = v;
                    }
                }
                "allow_observers" => {
                    if let Some(v) = value.as_bool() {
                        self.allow_observers = v;
                    }
                }
                "map_seed" => {
                    if let Some(v) = value.as_int() {
                        self.map_seed = v;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn save(&self) -> HashMap<String, PreferenceValue> {
        let mut data = HashMap::new();
        data.insert(
            "map_name".to_string(),
            PreferenceValue::String(self.map_name.clone()),
        );
        data.insert(
            "player_faction".to_string(),
            PreferenceValue::String(self.player_faction.clone()),
        );
        data.insert(
            "player_color".to_string(),
            PreferenceValue::Int(self.player_color),
        );
        data.insert(
            "num_ai_opponents".to_string(),
            PreferenceValue::Int(self.num_ai_opponents),
        );
        data.insert(
            "time_limit".to_string(),
            PreferenceValue::Int(self.time_limit),
        );
        data.insert(
            "fog_of_war".to_string(),
            PreferenceValue::Bool(self.fog_of_war),
        );
        data.insert(
            "allow_observers".to_string(),
            PreferenceValue::Bool(self.allow_observers),
        );
        data.insert("map_seed".to_string(), PreferenceValue::Int(self.map_seed));
        data
    }
}

/// Ladder preference entry (recent ladders)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderPref {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub last_play_date: i64,
}

pub type LadderPrefMap = BTreeMap<i64, LadderPref>;

/// Ladder preferences for recent ladder history.
#[derive(Debug, Default)]
pub struct LadderPreferences {
    prefs: UserPreferences,
    ladders: LadderPrefMap,
}

impl LadderPreferences {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_profile(&mut self, profile_id: i32) -> bool {
        self.prefs.clear();
        self.ladders.clear();
        let filename = format!("GeneralsOnline/Ladders{profile_id}.ini");
        if !self.prefs.load(&filename) {
            return false;
        }

        self.rebuild_ladders_from_prefs();
        true
    }

    fn rebuild_ladders_from_prefs(&mut self) {
        self.ladders.clear();
        for (key, value) in self.prefs.entries() {
            let Some(split) = key.rfind(':') else {
                continue;
            };
            let addr_raw = &key[..split];
            let port_raw = &key[split + 1..];
            let port = port_raw.parse::<u16>().unwrap_or(0);
            let address = quoted_printable_to_ascii_string(addr_raw);

            let Some(split) = value.rfind(':') else {
                continue;
            };
            let name_raw = &value[..split];
            let time_raw = &value[split + 1..];
            let last_play_date = time_raw.parse::<i64>().unwrap_or(0);
            let name = quoted_printable_to_unicode_string(name_raw);
            let pref = LadderPref {
                name,
                address,
                port,
                last_play_date,
            };
            self.ladders.insert(pref.last_play_date, pref);
        }
    }

    pub fn write(&mut self) -> bool {
        self.prefs.clear();
        let mut count = 0;
        for (_key, pref) in self.ladders.iter() {
            if count >= 5 {
                break;
            }
            let lad_name = format!(
                "{}:{}",
                ascii_string_to_quoted_printable(&pref.address),
                pref.port
            );
            let lad_data = format!(
                "{}:{}",
                unicode_string_to_quoted_printable(&pref.name),
                pref.last_play_date
            );
            self.prefs.set_string(&lad_name, lad_data);
            count += 1;
        }
        self.prefs.write()
    }

    pub fn get_recent_ladders(&self) -> &LadderPrefMap {
        &self.ladders
    }

    pub fn add_recent_ladder(&mut self, ladder: LadderPref) {
        let mut remove_key = None;
        for (key, entry) in &self.ladders {
            if entry.address == ladder.address && entry.port == ladder.port {
                remove_key = Some(*key);
                break;
            }
        }
        if let Some(key) = remove_key {
            self.ladders.remove(&key);
        }
        self.ladders.insert(ladder.last_play_date, ladder);
    }
}

/// Quick match preferences for fast casual games.
#[derive(Debug, Default)]
pub struct QuickmatchPreferences {
    prefs: UserPreferences,
}

impl QuickmatchPreferences {
    pub fn new() -> Self {
        let profile_id = get_gamespy_info()
            .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
            .unwrap_or(0);
        let mut prefs = UserPreferences::new();
        let filename = format!("GeneralsOnline/QMPref{profile_id}.ini");
        let _ = prefs.load_from_file(&filename);
        Self { prefs }
    }

    pub fn set_map_selected(&mut self, map_name: &str, selected: bool) {
        let key = ascii_string_to_quoted_printable(map_name);
        self.prefs.set_string(
            &key,
            if selected {
                "1".to_string()
            } else {
                "0".to_string()
            },
        );
    }

    pub fn is_map_selected(&self, map_name: &str) -> bool {
        let key = ascii_string_to_quoted_printable(map_name);
        self.prefs
            .get_string(&key)
            .and_then(|value| value.parse::<i32>().ok())
            .map(|val| val != 0)
            .unwrap_or(true)
    }

    pub fn set_last_ladder(&mut self, addr: &str, port: u16) {
        self.prefs.set_string("LastLadderAddr", addr.to_string());
        self.prefs.set_string("LastLadderPort", port.to_string());
    }

    pub fn get_last_ladder_addr(&self) -> String {
        self.prefs
            .get_string("LastLadderAddr")
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_last_ladder_port(&self) -> u16 {
        self.prefs
            .get_string("LastLadderPort")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0)
    }

    pub fn set_max_disconnects(&mut self, val: i32) {
        self.prefs.set_int("MaxDisconnects", val);
    }

    pub fn get_max_disconnects(&self) -> i32 {
        self.prefs.get_int_or("MaxDisconnects", 0)
    }

    pub fn set_max_points(&mut self, val: i32) {
        self.prefs.set_int("MaxPoints", val);
    }

    pub fn get_max_points(&self) -> i32 {
        self.prefs.get_int_or("MaxPoints", 1000)
    }

    pub fn set_min_points(&mut self, val: i32) {
        self.prefs.set_int("MinPoints", val);
    }

    pub fn get_min_points(&self) -> i32 {
        self.prefs.get_int_or("MinPoints", 0)
    }

    pub fn set_wait_time(&mut self, val: i32) {
        self.prefs.set_int("WaitTime", val);
    }

    pub fn get_wait_time(&self) -> i32 {
        self.prefs.get_int_or("WaitTime", 0)
    }

    pub fn set_num_players(&mut self, val: i32) {
        self.prefs.set_int("NumPlayers", val);
    }

    pub fn get_num_players(&self) -> i32 {
        self.prefs.get_int_or("NumPlayers", 0)
    }

    pub fn set_max_ping(&mut self, val: i32) {
        self.prefs.set_int("MaxPing", val);
    }

    pub fn get_max_ping(&self) -> i32 {
        self.prefs.get_int_or("MaxPing", 5)
    }

    pub fn set_color(&mut self, val: i32) {
        self.prefs.set_int("Color", val);
    }

    pub fn get_color(&self) -> i32 {
        self.prefs.get_int_or("Color", 0)
    }

    pub fn set_side(&mut self, val: i32) {
        self.prefs.set_int("Side", val);
    }

    pub fn get_side(&self) -> i32 {
        self.prefs.get_int_or("Side", 0)
    }

    pub fn write(&mut self) -> bool {
        self.prefs.write()
    }
}

/// Custom match preferences for custom multiplayer lobbies (C++-faithful).
#[derive(Debug, Default, Clone)]
pub struct CustomMatchPreferences {
    prefs: UserPreferences,
}

impl CustomMatchPreferences {
    pub fn new() -> Self {
        let profile_id = get_gamespy_info()
            .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
            .unwrap_or(0);
        let mut prefs = UserPreferences::new();
        let filename = format!("GeneralsOnline/CustomPref{profile_id}.ini");
        let _ = prefs.load_from_file(&filename);
        Self { prefs }
    }

    pub fn write(&mut self) -> bool {
        self.prefs.write()
    }

    pub fn set_last_ladder(&mut self, addr: &str, port: u16) {
        self.prefs.set_string("LastLadderAddr", addr.to_string());
        self.prefs.set_string("LastLadderPort", port.to_string());
    }

    pub fn get_last_ladder_addr(&self) -> String {
        self.prefs
            .get_string("LastLadderAddr")
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_last_ladder_port(&self) -> u16 {
        self.prefs
            .get_string("LastLadderPort")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0)
    }

    pub fn get_preferred_color(&self) -> i32 {
        let mut ret = self.prefs.get_int_or("Color", -1);
        let max_colors = with_multiplayer_settings(|settings| settings.get_num_colors());
        if ret < -1 || ret >= max_colors {
            ret = -1;
        }
        ret
    }

    pub fn set_preferred_color(&mut self, val: i32) {
        self.prefs.set_int("Color", val);
    }

    pub fn get_chat_size_slider(&self) -> i32 {
        let mut ret = self.prefs.get_int_or("ChatSlider", 45);
        if ret < 0 || ret > 100 {
            ret = 45;
        }
        ret
    }

    pub fn set_chat_size_slider(&mut self, val: i32) {
        self.prefs.set_int("ChatSlider", val);
    }

    pub fn get_preferred_faction(&self) -> i32 {
        let ret = self
            .prefs
            .get_int_or("PlayerTemplate", PLAYERTEMPLATE_RANDOM);
        if ret == PLAYERTEMPLATE_OBSERVER || ret < PLAYERTEMPLATE_MIN {
            return PLAYERTEMPLATE_RANDOM;
        }
        let store = get_player_template_store();
        if ret >= store.len() as i32 {
            return PLAYERTEMPLATE_RANDOM;
        }
        if let Some(template) = store.get_nth_player_template(ret as usize) {
            if template.starting_building.is_empty() {
                return PLAYERTEMPLATE_RANDOM;
            }
            let generals = get_challenge_generals();
            if let Some(general) = generals.get_general_by_template_name(&template.name) {
                if !general.is_starting_enabled() {
                    return PLAYERTEMPLATE_RANDOM;
                }
            }
        } else {
            return PLAYERTEMPLATE_RANDOM;
        }
        ret
    }

    pub fn set_preferred_faction(&mut self, val: i32) {
        self.prefs.set_int("PlayerTemplate", val);
    }

    pub fn uses_system_map_dir(&self) -> bool {
        self.prefs.get_bool_or("UseSystemMapDir", true)
    }

    pub fn set_uses_system_map_dir(&mut self, val: bool) {
        self.prefs.set_bool("UseSystemMapDir", val);
    }

    pub fn uses_long_game_list(&self) -> bool {
        true
    }

    pub fn set_uses_long_game_list(&mut self, val: bool) {
        self.prefs.set_bool("UseLongGameList", val);
    }

    pub fn allows_observers(&self) -> bool {
        self.prefs.get_bool_or("AllowObservers", true)
    }

    pub fn set_allows_observers(&mut self, val: bool) {
        self.prefs.set_bool("AllowObservers", val);
    }

    pub fn get_disallow_asian_text(&self) -> bool {
        if let Some(value) = self.prefs.get_string("DisallowAsianText") {
            return value.eq_ignore_ascii_case("1");
        }
        let language = get_registry_language();
        if language.compare_no_case_str("chinese") == std::cmp::Ordering::Equal
            || language.compare_no_case_str("korean") == std::cmp::Ordering::Equal
        {
            false
        } else {
            true
        }
    }

    pub fn set_disallow_asian_text(&mut self, val: bool) {
        self.prefs.set_bool("DisallowAsianText", val);
    }

    pub fn get_disallow_non_asian_text(&self) -> bool {
        self.prefs
            .get_string("DisallowNonAsianText")
            .map(|value| value.eq_ignore_ascii_case("1"))
            .unwrap_or(false)
    }

    pub fn set_disallow_non_asian_text(&mut self, val: bool) {
        self.prefs.set_bool("DisallowNonAsianText", val);
    }

    pub fn get_preferred_map(&self) -> String {
        let raw = self.prefs.get_string_or("Map", "");
        if raw.is_empty() {
            return get_default_official_map();
        }
        let mut decoded = quoted_printable_to_ascii_string(&raw);
        decoded = decoded.trim().to_string();
        if decoded.is_empty() || !is_valid_map(&decoded, true) {
            return get_default_official_map();
        }
        if self.get_use_stats() && !is_official_map(&decoded) {
            return get_default_official_map();
        }
        decoded
    }

    pub fn set_preferred_map(&mut self, val: &str) {
        let encoded = ascii_string_to_quoted_printable(val);
        self.prefs.set_string("Map", encoded);
    }

    pub fn get_superweapon_restricted(&self) -> bool {
        self.prefs
            .get_string("SuperweaponRestrict")
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
    }

    pub fn set_superweapon_restricted(&mut self, val: bool) {
        self.prefs.set_string(
            "SuperweaponRestrict",
            if val { "Yes" } else { "No" }.to_string(),
        );
    }

    pub fn get_starting_cash(&self) -> Money {
        let value = self
            .prefs
            .get_string("StartingCash")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_else(default_starting_cash);
        Money::new_with_amount(value)
    }

    pub fn set_starting_cash(&mut self, cash: Money) {
        self.prefs
            .set_string("StartingCash", cash.count_money().to_string());
    }

    pub fn get_factions_limited(&self) -> bool {
        self.prefs
            .get_string("LimitArmies")
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
    }

    pub fn set_factions_limited(&mut self, val: bool) {
        self.prefs
            .set_string("LimitArmies", if val { "Yes" } else { "No" }.to_string());
    }

    pub fn get_use_stats(&self) -> bool {
        self.prefs
            .get_string("UseStats")
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(true)
    }

    pub fn set_use_stats(&mut self, val: bool) {
        self.prefs
            .set_string("UseStats", if val { "Yes" } else { "No" }.to_string());
    }
}

fn is_valid_map(map_name: &str, is_multiplayer: bool) -> bool {
    let name = map_name.trim();
    if name.is_empty() {
        return false;
    }
    init_global_map_cache();
    if let Some(cache) = get_map_cache() {
        cache
            .get(name)
            .map(|meta| meta.is_multiplayer == is_multiplayer)
            .unwrap_or(false)
    } else {
        false
    }
}

fn is_official_map(map_name: &str) -> bool {
    let name = map_name.trim();
    if name.is_empty() {
        return false;
    }
    init_global_map_cache();
    if let Some(cache) = get_map_cache() {
        cache
            .get(name)
            .map(|meta| meta.is_official)
            .unwrap_or(false)
    } else {
        false
    }
}

fn get_default_official_map() -> String {
    init_global_map_cache();
    if let Some(cache) = get_map_cache() {
        let mut names: Vec<_> = cache
            .iter()
            .filter(|(_, meta)| meta.is_official)
            .map(|(name, _)| name.clone())
            .collect();
        names.sort();
        return names.first().cloned().unwrap_or_default();
    }
    String::new()
}

fn default_starting_cash() -> u32 {
    with_multiplayer_settings(|settings| {
        if let Some(choice) = settings
            .starting_money_choices
            .iter()
            .find(|choice| choice.is_default)
        {
            return choice.money.count_money();
        }
        if let Some(choice) = settings.starting_money_choices.first() {
            return choice.money.count_money();
        }
        10000
    })
}

pub struct IgnorePreferences {
    /// Set of ignored player names/IDs
    pub ignored_players: HashSet<String>,
    /// Block ignored players from chat
    pub block_chat: bool,
    /// Block ignored players from joining games
    pub block_game_join: bool,
    /// Block ignored players from invites
    pub block_invites: bool,
}

impl Default for IgnorePreferences {
    fn default() -> Self {
        Self {
            ignored_players: HashSet::new(),
            block_chat: true,
            block_game_join: true,
            block_invites: true,
        }
    }
}

impl IgnorePreferences {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_player(&mut self, player_id: String) {
        self.ignored_players.insert(player_id);
    }

    pub fn remove_player(&mut self, player_id: &str) -> bool {
        self.ignored_players.remove(player_id)
    }

    pub fn is_ignored(&self, player_id: &str) -> bool {
        self.ignored_players.contains(player_id)
    }

    pub fn clear_all(&mut self) {
        self.ignored_players.clear();
    }

    pub fn load(&mut self, data: HashMap<String, PreferenceValue>) {
        for (key, value) in data {
            match key.as_str() {
                "block_chat" => {
                    if let Some(v) = value.as_bool() {
                        self.block_chat = v;
                    }
                }
                "block_game_join" => {
                    if let Some(v) = value.as_bool() {
                        self.block_game_join = v;
                    }
                }
                "block_invites" => {
                    if let Some(v) = value.as_bool() {
                        self.block_invites = v;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn save(&self) -> HashMap<String, PreferenceValue> {
        let mut data = HashMap::new();
        data.insert(
            "block_chat".to_string(),
            PreferenceValue::Bool(self.block_chat),
        );
        data.insert(
            "block_game_join".to_string(),
            PreferenceValue::Bool(self.block_game_join),
        );
        data.insert(
            "block_invites".to_string(),
            PreferenceValue::Bool(self.block_invites),
        );
        data
    }
}

/// GameSpy miscellaneous preferences (locale, cached stats, QuickMatch settings).
#[derive(Debug)]
pub struct GameSpyMiscPreferences {
    prefs: UserPreferences,
}

impl GameSpyMiscPreferences {
    pub fn new() -> Self {
        let profile_id = get_gamespy_info()
            .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
            .unwrap_or(0);
        let mut prefs = UserPreferences::new();
        let filename = format!("GeneralsOnline/GSMiscPref{profile_id}.ini");
        let _ = prefs.load_from_file(&filename);
        Self { prefs }
    }

    pub fn get_locale(&self) -> i32 {
        self.prefs.get_int("Locale").unwrap_or(0)
    }

    pub fn set_locale(&mut self, value: i32) {
        self.prefs.set_int("Locale", value);
    }

    pub fn get_cached_stats(&self) -> String {
        self.prefs
            .get_string("CachedStats")
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_cached_stats(&mut self, value: String) {
        self.prefs.set_string("CachedStats", value);
    }

    pub fn get_quick_match_res_locked(&self) -> bool {
        self.prefs.get_bool("QMResLock").unwrap_or(false)
    }

    pub fn get_max_messages_per_update(&self) -> i32 {
        self.prefs.get_int("MaxMessagesPerUpdate").unwrap_or(100)
    }

    pub fn write(&mut self) {
        let _ = self.prefs.save_to_file();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skirmish_preferences() {
        let mut prefs = SkirmishPreferences::new();
        assert_eq!(prefs.num_ai_opponents, 1);

        prefs.num_ai_opponents = 3;
        let saved = prefs.save();

        let mut loaded = SkirmishPreferences::new();
        loaded.load(saved);
        assert_eq!(loaded.num_ai_opponents, 3);
    }

    #[test]
    fn test_ladder_preferences() {
        let mut prefs = LadderPreferences::new();
        let ladder = LadderPref {
            name: "Test Ladder".to_string(),
            address: "127.0.0.1".to_string(),
            port: 1234,
            last_play_date: 1,
        };
        prefs.add_recent_ladder(ladder);
        assert_eq!(prefs.get_recent_ladders().len(), 1);
    }

    #[test]
    fn ladder_preferences_preserve_cpp_zero_fields() {
        let mut prefs = LadderPreferences::new();
        prefs.prefs.set_string(":0", ":0".to_string());

        prefs.rebuild_ladders_from_prefs();

        let ladder = prefs.get_recent_ladders().get(&0).unwrap();
        assert_eq!(ladder.address, "");
        assert_eq!(ladder.port, 0);
        assert_eq!(ladder.name, "");
        assert_eq!(ladder.last_play_date, 0);
    }

    #[test]
    fn test_quickmatch_preferences() {
        let mut prefs = QuickmatchPreferences::new();
        prefs.set_max_ping(7);
        assert_eq!(prefs.get_max_ping(), 7);
        prefs.set_map_selected("TestMap", false);
        assert!(!prefs.is_map_selected("TestMap"));
    }

    #[test]
    fn test_custom_match_preferences() {
        let mut prefs = CustomMatchPreferences::new();
        prefs.set_use_stats(true);
        prefs.set_disallow_asian_text(true);
        assert!(prefs.get_use_stats());
        assert!(prefs.get_disallow_asian_text());
    }

    #[test]
    fn custom_match_rejects_locked_challenge_general_preference() {
        use crate::common::ini::ini_challenge_generals::{
            get_challenge_generals_mut, ChallengeGenerals,
        };
        use crate::common::rts::player_template::{get_player_template_store_mut, PlayerTemplate};

        {
            let mut store = get_player_template_store_mut();
            store.clear();
            let mut template = PlayerTemplate::new("FactionLockedGeneral".to_string());
            template.starting_building = "CommandCenter".to_string();
            store.add_template(template);
        }
        {
            let mut generals = get_challenge_generals_mut();
            *generals = ChallengeGenerals::new();
            generals.positions[0].player_template_name = "FactionLockedGeneral".to_string();
            generals.positions[0].starts_enabled = false;
        }

        let mut prefs = CustomMatchPreferences::new();
        prefs.prefs.set_int("PlayerTemplate", 0);

        assert_eq!(prefs.get_preferred_faction(), PLAYERTEMPLATE_RANDOM);

        get_player_template_store_mut().clear();
        *get_challenge_generals_mut() = ChallengeGenerals::new();
    }

    #[test]
    fn test_ignore_preferences() {
        let mut prefs = IgnorePreferences::new();
        assert_eq!(prefs.ignored_players.len(), 0);

        prefs.add_player("BadPlayer123".to_string());
        assert!(prefs.is_ignored("BadPlayer123"));
        assert!(!prefs.is_ignored("GoodPlayer456"));

        prefs.remove_player("BadPlayer123");
        assert!(!prefs.is_ignored("BadPlayer123"));
    }

    #[test]
    fn test_preference_value_conversions() {
        let bool_val = PreferenceValue::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));
        assert_eq!(bool_val.as_int(), None);

        let int_val = PreferenceValue::Int(42);
        assert_eq!(int_val.as_int(), Some(42));
        assert_eq!(int_val.as_bool(), None);

        let float_val = PreferenceValue::Float(3.14);
        assert_eq!(float_val.as_float(), Some(3.14));

        let str_val = PreferenceValue::String("test".to_string());
        assert_eq!(str_val.as_str(), Some("test"));
    }
}
