//! Ladder definitions and parsing (C++ LadderDefs.cpp parity).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use game_engine::common::ascii_string::AsciiString;
use game_engine::common::rts::player_template::get_player_template_store;
use game_engine::common::system::encrypt::encrypt_string;

use super::config::GameSpyConfig;
use crate::config::MAX_SLOTS;

#[derive(Debug, Clone)]
pub struct LadderInfo {
    pub name: String,
    pub description: String,
    pub location: String,
    pub players_per_team: i32,
    pub min_wins: i32,
    pub max_wins: i32,
    pub random_maps: bool,
    pub random_factions: bool,
    pub valid_qm: bool,
    pub valid_custom: bool,
    pub valid_maps: Vec<AsciiString>,
    pub valid_factions: Vec<AsciiString>,
    pub crypted_password: AsciiString,
    pub address: AsciiString,
    pub port: u16,
    pub homepage_url: AsciiString,
    pub submit_replay: bool,
    pub index: i32,
}

impl Default for LadderInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            location: String::new(),
            players_per_team: 1,
            min_wins: 0,
            max_wins: 0,
            random_maps: true,
            random_factions: true,
            valid_qm: true,
            valid_custom: false,
            valid_maps: Vec::new(),
            valid_factions: Vec::new(),
            crypted_password: AsciiString::new(),
            address: AsciiString::new(),
            port: 0,
            homepage_url: AsciiString::new(),
            submit_replay: false,
            index: -1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LadderMapMeta {
    pub display_name: String,
    pub num_players: i32,
}

pub trait LadderMapProvider: Send + Sync {
    fn map_dir(&self) -> String;
    fn find_map(&self, map_path: &str) -> Option<LadderMapMeta>;
}

static MAP_PROVIDER: OnceLock<Arc<dyn LadderMapProvider>> = OnceLock::new();

pub fn set_ladder_map_provider(provider: Arc<dyn LadderMapProvider>) {
    let _ = MAP_PROVIDER.set(provider);
}

fn map_provider() -> Option<Arc<dyn LadderMapProvider>> {
    MAP_PROVIDER.get().cloned()
}

#[derive(Default)]
pub struct LadderList {
    local_ladders: Vec<LadderInfo>,
    special_ladders: Vec<LadderInfo>,
    standard_ladders: Vec<LadderInfo>,
}

impl LadderList {
    pub fn new(config: &GameSpyConfig) -> Self {
        let mut list = LadderList::default();
        list.parse_ladders(config.leftover_config(), config.get_qm_maps());
        list.load_local_ladders(config.get_qm_maps());
        list
    }

    pub fn new_from_config_file() -> Self {
        let config = GameSpyConfig::new_sync();
        Self::new(&config)
    }

    pub fn get_local_ladders(&self) -> &[LadderInfo] {
        &self.local_ladders
    }

    pub fn get_special_ladders(&self) -> &[LadderInfo] {
        &self.special_ladders
    }

    pub fn get_standard_ladders(&self) -> &[LadderInfo] {
        &self.standard_ladders
    }

    pub fn find_ladder(&self, addr: &AsciiString, port: u16) -> Option<&LadderInfo> {
        self.special_ladders
            .iter()
            .chain(self.standard_ladders.iter())
            .chain(self.local_ladders.iter())
            .find(|lad| lad.address == *addr && lad.port == port)
    }

    pub fn find_ladder_by_index(&self, index: i32) -> Option<&LadderInfo> {
        if index == 0 {
            return None;
        }
        self.special_ladders
            .iter()
            .chain(self.standard_ladders.iter())
            .chain(self.local_ladders.iter())
            .find(|lad| lad.index == index)
    }

    fn parse_ladders(&mut self, raw: &str, qm_maps: &[String]) {
        let mut in_ladders = false;
        let mut in_special_ladders = false;
        let mut in_ladder = false;
        let mut raw_ladder = String::new();
        let mut index = 1;

        for line in raw.lines() {
            let line = line.trim_end_matches('\r').trim();
            if line.is_empty() {
                continue;
            }

            if !in_ladders && line == "<Ladders>" {
                in_ladders = true;
                raw_ladder.clear();
                continue;
            }
            if in_ladders && line == "</Ladders>" {
                in_ladders = false;
                continue;
            }
            if !in_special_ladders && line == "<SpecialLadders>" {
                in_special_ladders = true;
                raw_ladder.clear();
                continue;
            }
            if in_special_ladders && line == "</SpecialLadders>" {
                in_special_ladders = false;
                continue;
            }

            if in_ladders || in_special_ladders {
                if line.starts_with("<Ladder ") && !in_ladder {
                    in_ladder = true;
                    raw_ladder.clear();
                    raw_ladder.push_str(line);
                    raw_ladder.push('\n');
                } else if line == "</Ladder>" && in_ladder {
                    in_ladder = false;
                    raw_ladder.push_str(line);
                    raw_ladder.push('\n');
                    if let Some(mut lad) = parse_ladder(&raw_ladder, qm_maps) {
                        lad.index = index;
                        index += 1;
                        if in_ladders {
                            self.standard_ladders.push(lad);
                        } else {
                            self.special_ladders.push(lad);
                        }
                    }
                    raw_ladder.clear();
                } else if in_ladder {
                    raw_ladder.push_str(line);
                    raw_ladder.push('\n');
                }
            }
        }
    }

    fn load_local_ladders(&mut self, qm_maps: &[String]) {
        let Some(base_dir) = user_data_dir() else {
            return;
        };
        let ladders_dir = base_dir.join("GeneralsOnline").join("Ladders");
        let Ok(entries) = fs::read_dir(&ladders_dir) else {
            return;
        };
        let mut index = -1;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("ini") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Some(mut lad) = parse_ladder(&raw, qm_maps) {
                    if lad.address.is_empty() || lad.port == 0 || lad.valid_maps.is_empty() {
                        continue;
                    }
                    lad.index = index;
                    index -= 1;
                    lad.valid_qm = false;
                    lad.valid_custom = false;
                    self.local_ladders.push(lad);
                }
            }
        }
    }
}

static THE_LADDER_LIST: OnceLock<Arc<RwLock<LadderList>>> = OnceLock::new();

pub fn init_ladder_list(config: &GameSpyConfig) -> Arc<RwLock<LadderList>> {
    THE_LADDER_LIST
        .get_or_init(|| Arc::new(RwLock::new(LadderList::new(config))))
        .clone()
}

pub fn get_ladder_list() -> Option<Arc<RwLock<LadderList>>> {
    THE_LADDER_LIST.get().cloned()
}

fn parse_ladder(raw: &str, qm_maps: &[String]) -> Option<LadderInfo> {
    let mut ladder = None::<LadderInfo>;

    for line in raw.lines() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }

        if ladder.is_none() && line.starts_with("<Ladder ") {
            if let Some((name, addr, port, homepage)) = parse_ladder_header(line) {
                let mut info = LadderInfo::default();
                info.name = name;
                info.address = AsciiString::from(addr.as_str());
                info.port = port;
                info.homepage_url = AsciiString::from(homepage.as_str());
                ladder = Some(info);
            }
            continue;
        }

        let Some(info) = ladder.as_mut() else {
            continue;
        };

        if line == "</Ladder>" {
            if info.players_per_team < 1 || info.players_per_team > (MAX_SLOTS as i32 / 2) {
                return None;
            }
            ensure_valid_factions(info);
            ensure_valid_maps(info, qm_maps);
            return Some(info.clone());
        }

        if let Some(value) = line.strip_prefix("Name ") {
            info.name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Desc ") {
            info.description = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Loc ") {
            info.location = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("TeamSize ") {
            info.players_per_team = value.trim().parse().unwrap_or(info.players_per_team);
        } else if let Some(value) = line.strip_prefix("RandomMaps ") {
            info.random_maps = value.trim() != "0";
        } else if let Some(value) = line.strip_prefix("RandomFactions ") {
            info.random_factions = value.trim() != "0";
        } else if let Some(value) = line.strip_prefix("Faction ") {
            let faction = value.trim();
            if !faction.is_empty() {
                add_faction(info, faction);
            }
        } else if let Some(value) = line.strip_prefix("MinWins ") {
            info.min_wins = value.trim().parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("MaxWins ") {
            info.max_wins = value.trim().parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("CryptedPass ") {
            info.crypted_password = AsciiString::from(value.trim());
        } else if let Some(value) = line.strip_prefix("Map ") {
            add_map(info, value.trim(), qm_maps);
        } else {
            return None;
        }
    }

    None
}

fn parse_ladder_header(line: &str) -> Option<(String, String, u16, String)> {
    let trimmed = line.trim().trim_end_matches('>');
    let content = trimmed.strip_prefix("<Ladder ")?;
    let mut remainder = content.trim();
    let mut name = String::new();

    if remainder.starts_with('"') {
        remainder = &remainder[1..];
        if let Some(pos) = remainder.find('"') {
            name = remainder[..pos].to_string();
            remainder = remainder[pos + 1..].trim();
        }
    }

    let mut parts = remainder.split_whitespace();
    let addr = parts.next()?.to_string();
    let port = parts.next()?.parse().ok()?;
    let homepage = parts.next().unwrap_or("").to_string();
    Some((name, addr, port, homepage))
}

fn add_faction(info: &mut LadderInfo, faction: &str) {
    let store = get_player_template_store();
    let valid = store
        .iter()
        .any(|template| template.is_playable_side() && template.get_side() == faction);
    if !valid {
        return;
    }
    if !info
        .valid_factions
        .iter()
        .any(|entry| entry.as_str() == faction)
    {
        info.valid_factions.push(AsciiString::from(faction));
    }
}

fn ensure_valid_factions(info: &mut LadderInfo) {
    if !info.valid_factions.is_empty() {
        return;
    }
    let store = get_player_template_store();
    for template in store.iter() {
        if template.is_playable_side() {
            info.valid_factions
                .push(AsciiString::from(template.get_side()));
        }
    }
}

fn ensure_valid_maps(info: &mut LadderInfo, qm_maps: &[String]) {
    if !info.valid_maps.is_empty() {
        return;
    }
    let map_provider = map_provider();
    let min_players = info.players_per_team * 2;
    for map in qm_maps {
        let normalized = normalize_map_path(map);
        if let Some(provider) = map_provider.as_ref() {
            if let Some(meta) = provider.find_map(&normalized) {
                if meta.num_players >= min_players {
                    info.valid_maps.push(AsciiString::from(normalized.as_str()));
                }
            }
        } else {
            info.valid_maps.push(AsciiString::from(normalized.as_str()));
        }
    }
}

fn add_map(info: &mut LadderInfo, map_name: &str, qm_maps: &[String]) {
    if map_name.is_empty() {
        return;
    }

    let map_provider = map_provider();
    let mut path = map_name.to_string();
    if !map_name.ends_with(".map") {
        if let Some(provider) = map_provider.as_ref() {
            path = format!("{}\\{}\\{}.map", provider.map_dir(), map_name, map_name);
        }
    }
    let normalized = normalize_map_path(&path);
    let qm_normalized: Vec<String> = qm_maps.iter().map(|m| normalize_map_path(m)).collect();
    if !qm_normalized.iter().any(|m| m == &normalized) {
        return;
    }

    if let Some(provider) = map_provider.as_ref() {
        if let Some(meta) = provider.find_map(&normalized) {
            if meta.num_players >= info.players_per_team * 2 {
                info.valid_maps.push(AsciiString::from(normalized.as_str()));
            }
        }
    } else {
        info.valid_maps.push(AsciiString::from(normalized.as_str()));
    }
}

fn normalize_map_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn user_data_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(Path::new(&home).join(".generals"))
}

pub fn encrypt_ladder_password(password: &str) -> String {
    encrypt_string(password)
}
