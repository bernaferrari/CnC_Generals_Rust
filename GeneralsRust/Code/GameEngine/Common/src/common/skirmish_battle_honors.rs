//! Skirmish battle honors and campaign completion tracking.

use crate::common::ini::ini_game_data::get_global_data;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

pub const BATTLE_HONOR_LADDER_CHAMP: u32 = 0x00000001;
pub const BATTLE_HONOR_STREAK: u32 = 0x00000002;
pub const BATTLE_HONOR_LOYALTY_USA: u32 = 0x00000020;
pub const BATTLE_HONOR_LOYALTY_CHINA: u32 = 0x00000040;
pub const BATTLE_HONOR_BATTLE_TANK: u32 = 0x00000080;
pub const BATTLE_HONOR_AIR_WING: u32 = 0x00000100;
pub const BATTLE_HONOR_LOYALTY_GLA: u32 = 0x00000200;
pub const BATTLE_HONOR_ENDURANCE: u32 = 0x00000400;
pub const BATTLE_HONOR_CAMPAIGN_USA: u32 = 0x00000800;
pub const BATTLE_HONOR_CAMPAIGN_CHINA: u32 = 0x00001000;
pub const BATTLE_HONOR_CAMPAIGN_GLA: u32 = 0x00002000;
pub const BATTLE_HONOR_BLITZ5: u32 = 0x00004000;
pub const BATTLE_HONOR_BLITZ10: u32 = 0x00008000;
pub const BATTLE_HONOR_FAIR_PLAY: u32 = 0x00010000;
pub const BATTLE_HONOR_APOCALYPSE: u32 = 0x00020000;
pub const BATTLE_HONOR_OFFICERSCLUB: u32 = 0x00040000;
pub const BATTLE_HONOR_DOMINATION: u32 = 0x00080000;
pub const BATTLE_HONOR_CHALLENGE_MODE: u32 = 0x00100000;
pub const BATTLE_HONOR_ULTIMATE: u32 = 0x00200000;
pub const BATTLE_HONOR_GLOBAL_GENERAL: u32 = 0x00400000;
pub const BATTLE_HONOR_DOMINATION_ONLINE: u32 = 0x00800000;
pub const BATTLE_HONOR_STREAK_ONLINE: u32 = 0x01000000;
pub const BATTLE_HONOR_CHALLENGE: u32 = 0x02000000;
pub const BATTLE_HONOR_NOT_GAINED: u32 = 0x08000000;

pub const BH_CHALLENGE_MASK_1: u32 = 0x0001;
pub const BH_CHALLENGE_MASK_2: u32 = 0x0002;
pub const BH_CHALLENGE_MASK_3: u32 = 0x0004;
pub const BH_CHALLENGE_MASK_4: u32 = 0x0008;
pub const BH_CHALLENGE_MASK_5: u32 = 0x0010;
pub const BH_CHALLENGE_MASK_6: u32 = 0x0020;
pub const BH_CHALLENGE_MASK_7: u32 = 0x0040;

pub const MAX_BATTLE_HONOR_COLUMNS: u32 = 4;
pub const MAX_BATTLE_HONOR_IMAGE_WIDTH: u32 = 40;
pub const MAX_BATTLE_HONOR_IMAGE_HEIGHT: u32 = 41;

const MAX_GLOBAL_GENERAL_TYPES: usize = 9;

#[derive(Debug, Clone)]
pub struct SkirmishBattleHonors {
    data: HashMap<String, String>,
    filename: String,
    honors: u32,
    wins: i32,
    losses: i32,
    win_streak: i32,
    best_win_streak: i32,
    last_general: String,
    num_games_loyal: i32,
}

impl Default for SkirmishBattleHonors {
    fn default() -> Self {
        Self::new()
    }
}

impl SkirmishBattleHonors {
    pub fn new() -> Self {
        let mut honors = Self {
            data: HashMap::new(),
            filename: "SkirmishStats.ini".to_string(),
            honors: 0,
            wins: 0,
            losses: 0,
            win_streak: 0,
            best_win_streak: 0,
            last_general: String::new(),
            num_games_loyal: 0,
        };
        honors.load_data();
        honors
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.honors = 0;
        self.wins = 0;
        self.losses = 0;
        self.win_streak = 0;
        self.best_win_streak = 0;
        self.last_general.clear();
        self.num_games_loyal = 0;
    }

    pub fn write(&mut self) -> bool {
        self.set_int("Honors", self.honors as i32);
        self.set_int("Wins", self.wins);
        self.set_int("Losses", self.losses);
        self.set_int("WinStreak", self.win_streak);
        self.set_int("BestWinStreak", self.best_win_streak);
        let last_general = self.last_general.clone();
        self.set_string("LastGeneral", &last_general);
        self.set_int("NumGamesLoyal", self.num_games_loyal);
        self.write_file()
    }

    pub fn get_honors(&self) -> u32 {
        self.honors
    }

    pub fn set_honors(&mut self, honors: i32) {
        self.honors = honors.max(0) as u32;
    }

    pub fn award_honor(&mut self, honor: u32) {
        self.honors |= honor;
    }

    pub fn get_wins(&self) -> i32 {
        self.wins
    }

    pub fn set_wins(&mut self, wins: i32) {
        self.wins = wins;
    }

    pub fn get_losses(&self) -> i32 {
        self.losses
    }

    pub fn set_losses(&mut self, losses: i32) {
        self.losses = losses;
    }

    pub fn get_win_streak(&self) -> i32 {
        self.win_streak
    }

    pub fn set_win_streak(&mut self, win_streak: i32) {
        self.win_streak = win_streak;
    }

    pub fn get_best_win_streak(&self) -> i32 {
        self.best_win_streak
    }

    pub fn set_best_win_streak(&mut self, best_win_streak: i32) {
        self.best_win_streak = best_win_streak;
    }

    pub fn get_last_general(&self) -> String {
        self.last_general.clone()
    }

    pub fn set_last_general(&mut self, general: String) {
        self.last_general = general;
    }

    pub fn get_num_games_loyal(&self) -> i32 {
        self.num_games_loyal
    }

    pub fn set_num_games_loyal(&mut self, games: i32) {
        self.num_games_loyal = games;
    }

    pub fn get_china_campaign_complete(&self, difficulty: i32) -> bool {
        self.get_bool(&format!("ChinaCampaign_{}", difficulty), false)
    }

    pub fn set_china_campaign_complete(&mut self, difficulty: i32) {
        self.set_bool(&format!("ChinaCampaign_{}", difficulty), true);
        self.award_honor(BATTLE_HONOR_CAMPAIGN_CHINA);
    }

    pub fn get_gla_campaign_complete(&self, difficulty: i32) -> bool {
        self.get_bool(&format!("GLACampaign_{}", difficulty), false)
    }

    pub fn set_gla_campaign_complete(&mut self, difficulty: i32) {
        self.set_bool(&format!("GLACampaign_{}", difficulty), true);
        self.award_honor(BATTLE_HONOR_CAMPAIGN_GLA);
    }

    pub fn get_usa_campaign_complete(&self, difficulty: i32) -> bool {
        self.get_bool(&format!("USACampaign_{}", difficulty), false)
    }

    pub fn set_usa_campaign_complete(&mut self, difficulty: i32) {
        self.set_bool(&format!("USACampaign_{}", difficulty), true);
        self.award_honor(BATTLE_HONOR_CAMPAIGN_USA);
    }

    pub fn get_challenge_campaign_complete(&self, general_index: usize, difficulty: i32) -> bool {
        if general_index >= MAX_GLOBAL_GENERAL_TYPES {
            return false;
        }
        self.get_bool(
            &format!("Challenge_{}_{}", general_index, difficulty),
            false,
        )
    }

    pub fn set_challenge_campaign_complete(&mut self, general_index: usize, difficulty: i32) {
        if general_index >= MAX_GLOBAL_GENERAL_TYPES {
            return;
        }
        self.set_bool(&format!("Challenge_{}_{}", general_index, difficulty), true);
        self.award_honor(BATTLE_HONOR_CHALLENGE_MODE);
    }

    pub fn get_endurance_medal(&self, map_name: &str, ai_difficulty: i32) -> i32 {
        self.get_int(&format!("Endurance_{}_{}", map_name, ai_difficulty), 0)
    }

    pub fn set_endurance_medal(
        &mut self,
        map_name: &str,
        ai_difficulty: i32,
        opponents_beaten: i32,
    ) {
        self.set_int(
            &format!("Endurance_{}_{}", map_name, ai_difficulty),
            opponents_beaten,
        );
    }

    pub fn increment_endurance_medal(&mut self, map_name: &str, ai_difficulty: i32) {
        let current = self.get_endurance_medal(map_name, ai_difficulty);
        self.set_endurance_medal(map_name, ai_difficulty, current + 1);
    }

    fn load_data(&mut self) {
        self.data = self.load_file();
        self.honors = self.get_int("Honors", 0) as u32;
        self.wins = self.get_int("Wins", 0);
        self.losses = self.get_int("Losses", 0);
        self.win_streak = self.get_int("WinStreak", 0);
        self.best_win_streak = self.get_int("BestWinStreak", 0);
        self.last_general = self.get_string("LastGeneral", "");
        self.num_games_loyal = self.get_int("NumGamesLoyal", 0);
    }

    fn get_int(&self, key: &str, default: i32) -> i32 {
        self.data
            .get(key)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(default)
    }

    fn get_bool(&self, key: &str, default: bool) -> bool {
        self.data
            .get(key)
            .and_then(|value| value.parse::<i32>().ok())
            .map(|value| value != 0)
            .unwrap_or(default)
    }

    fn get_string(&self, key: &str, default: &str) -> String {
        self.data
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    fn set_int(&mut self, key: &str, value: i32) {
        self.data.insert(key.to_string(), value.to_string());
    }

    fn set_bool(&mut self, key: &str, value: bool) {
        self.data
            .insert(key.to_string(), if value { "1" } else { "0" }.to_string());
    }

    fn set_string(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }

    fn load_file(&self) -> HashMap<String, String> {
        let mut data = HashMap::new();
        let path = self.stats_path();
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return data,
        };
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                continue;
            }
            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            if !key.is_empty() {
                data.insert(key.to_string(), value.to_string());
            }
        }
        data
    }

    fn write_file(&self) -> bool {
        let path = self.stats_path();
        if let Some(parent) = path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                eprintln!("Failed to create stats dir {}: {}", parent.display(), err);
                return false;
            }
        }
        let mut entries: Vec<_> = self.data.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let mut file = match File::create(&path) {
            Ok(file) => file,
            Err(err) => {
                eprintln!("Failed to create stats file {}: {}", path.display(), err);
                return false;
            }
        };
        for (key, value) in entries {
            if writeln!(file, "{}={}", key, value).is_err() {
                return false;
            }
        }
        true
    }

    fn stats_path(&self) -> PathBuf {
        if let Some(global) = get_global_data() {
            let data = global.read();
            let dir = data.get_path_user_data();
            if !dir.is_empty() {
                return PathBuf::from(dir).join(&self.filename);
            }
        }
        PathBuf::from(&self.filename)
    }
}
