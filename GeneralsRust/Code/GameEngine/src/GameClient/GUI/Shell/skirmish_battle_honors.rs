// FILE: skirmish_battle_honors.rs
// Author: Rust port
// Description: Skirmish battle honors tracking (achievements/statistics)
//
// Ported from: GeneralsMD/Code/GameEngine/Source/Common/SkirmishBattleHonors.h/cpp

use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use super::skirmish_preferences::UserPreferences;
use super::skirmish_game_options_menu::SlotState;

// Battle honor flags - bitfield values
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

// Difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy = 0,
    Normal = 1,
    Hard = 2,
}

// Maximum number of generals in challenge mode
pub const MAX_GLOBAL_GENERAL_TYPES: usize = 9;

// Skirmish battle honors tracker
pub struct SkirmishBattleHonors {
    data: HashMap<String, String>,
    filename: String,
    honors: u32,
    wins: i32,
    losses: i32,
    win_streak: i32,
    best_win_streak: i32,
}

impl SkirmishBattleHonors {
    pub fn new() -> Self {
        let mut honors = SkirmishBattleHonors {
            data: HashMap::new(),
            filename: "SkirmishStats.ini".to_string(),
            honors: 0,
            wins: 0,
            losses: 0,
            win_streak: 0,
            best_win_streak: 0,
        };
        honors.load_data();
        honors
    }

    fn load_data(&mut self) {
        self.data = self.load(&self.filename);

        // Load basic stats
        self.honors = self.get_int("Honors", 0) as u32;
        self.wins = self.get_int("Wins", 0);
        self.losses = self.get_int("Losses", 0);
        self.win_streak = self.get_int("WinStreak", 0);
        self.best_win_streak = self.get_int("BestWinStreak", 0);
    }

    pub fn write_data(&mut self) -> bool {
        // Save basic stats
        self.set_int("Honors", self.honors as i32);
        self.set_int("Wins", self.wins);
        self.set_int("Losses", self.losses);
        self.set_int("WinStreak", self.win_streak);
        self.set_int("BestWinStreak", self.best_win_streak);

        self.write(&self.filename, &self.data)
    }

    // Get honor flags
    pub fn get_honors(&self) -> u32 {
        self.honors
    }

    // Set honor flags
    pub fn set_honors(&mut self, honors: u32) {
        self.honors = honors;
    }

    // Award a specific honor
    pub fn award_honor(&mut self, honor: u32) {
        self.honors |= honor;
    }

    // Check if has specific honor
    pub fn has_honor(&self, honor: u32) -> bool {
        (self.honors & honor) != 0
    }

    // Get total wins
    pub fn get_wins(&self) -> i32 {
        self.wins
    }

    // Get total losses
    pub fn get_losses(&self) -> i32 {
        self.losses
    }

    // Get current win streak
    pub fn get_win_streak(&self) -> i32 {
        self.win_streak
    }

    // Get best win streak
    pub fn get_best_win_streak(&self) -> i32 {
        self.best_win_streak
    }

    // Record a win
    pub fn record_win(&mut self) {
        self.wins += 1;
        self.win_streak += 1;

        if self.win_streak > self.best_win_streak {
            self.best_win_streak = self.win_streak;
        }

        // Check for streak honors
        if self.win_streak >= 3 {
            self.award_honor(BATTLE_HONOR_STREAK);
        }

        // Check for domination honors based on total wins
        if self.wins >= 100 {
            self.award_honor(BATTLE_HONOR_DOMINATION);
        }
    }

    // Record a loss
    pub fn record_loss(&mut self) {
        self.losses += 1;
        self.win_streak = 0;
    }

    // Clear all stats (for reset button)
    pub fn clear(&mut self) {
        self.data.clear();
        self.honors = 0;
        self.wins = 0;
        self.losses = 0;
        self.win_streak = 0;
        self.best_win_streak = 0;
    }

    // Campaign completion tracking
    pub fn get_china_campaign_complete(&self, difficulty: Difficulty) -> bool {
        let key = format!("ChinaCampaign_{}", difficulty as i32);
        self.get_bool(&key, false)
    }

    pub fn set_china_campaign_complete(&mut self, difficulty: Difficulty, complete: bool) {
        let key = format!("ChinaCampaign_{}", difficulty as i32);
        self.set_bool(&key, complete);
        if complete {
            self.award_honor(BATTLE_HONOR_CAMPAIGN_CHINA);
        }
    }

    pub fn get_gla_campaign_complete(&self, difficulty: Difficulty) -> bool {
        let key = format!("GLACampaign_{}", difficulty as i32);
        self.get_bool(&key, false)
    }

    pub fn set_gla_campaign_complete(&mut self, difficulty: Difficulty, complete: bool) {
        let key = format!("GLACampaign_{}", difficulty as i32);
        self.set_bool(&key, complete);
        if complete {
            self.award_honor(BATTLE_HONOR_CAMPAIGN_GLA);
        }
    }

    pub fn get_usa_campaign_complete(&self, difficulty: Difficulty) -> bool {
        let key = format!("USACampaign_{}", difficulty as i32);
        self.get_bool(&key, false)
    }

    pub fn set_usa_campaign_complete(&mut self, difficulty: Difficulty, complete: bool) {
        let key = format!("USACampaign_{}", difficulty as i32);
        self.set_bool(&key, complete);
        if complete {
            self.award_honor(BATTLE_HONOR_CAMPAIGN_USA);
        }
    }

    // Challenge campaign completion
    pub fn get_challenge_campaign_complete(&self, general_index: usize, difficulty: Difficulty) -> bool {
        if general_index >= MAX_GLOBAL_GENERAL_TYPES {
            return false;
        }
        let key = format!("Challenge_{}_{}", general_index, difficulty as i32);
        self.get_bool(&key, false)
    }

    pub fn set_challenge_campaign_complete(&mut self, general_index: usize, difficulty: Difficulty, complete: bool) {
        if general_index >= MAX_GLOBAL_GENERAL_TYPES {
            return;
        }
        let key = format!("Challenge_{}_{}", general_index, difficulty as i32);
        self.set_bool(&key, complete);
        if complete {
            self.award_honor(BATTLE_HONOR_CHALLENGE_MODE);
        }
    }

    // Endurance medals (winning on each map against AI)
    pub fn get_endurance_medal(&self, map_name: &str, ai_difficulty: SlotState) -> i32 {
        let key = format!("Endurance_{}_{}", map_name, ai_difficulty as i32);
        self.get_int(&key, 0)
    }

    pub fn set_endurance_medal(&mut self, map_name: &str, ai_difficulty: SlotState, opponents_beaten: i32) {
        let key = format!("Endurance_{}_{}", map_name, ai_difficulty as i32);
        self.set_int(&key, opponents_beaten);
    }

    pub fn increment_endurance_medal(&mut self, map_name: &str, ai_difficulty: SlotState) {
        let current = self.get_endurance_medal(map_name, ai_difficulty);
        self.set_endurance_medal(map_name, ai_difficulty, current + 1);
    }

    // Helper functions for data storage
    fn get_int(&self, key: &str, default: i32) -> i32 {
        if let Some(value) = self.data.get(key) {
            value.parse::<i32>().unwrap_or(default)
        } else {
            default
        }
    }

    fn set_int(&mut self, key: &str, value: i32) {
        self.data.insert(key.to_string(), value.to_string());
    }

    fn get_bool(&self, key: &str, default: bool) -> bool {
        if let Some(value) = self.data.get(key) {
            value.eq_ignore_ascii_case("true") || value == "1"
        } else {
            default
        }
    }

    fn set_bool(&mut self, key: &str, value: bool) {
        self.data.insert(key.to_string(), if value { "true" } else { "false" }.to_string());
    }
}

impl Default for SkirmishBattleHonors {
    fn default() -> Self {
        Self::new()
    }
}

impl UserPreferences for SkirmishBattleHonors {}

// Battle honor display information
pub struct BattleHonorInfo {
    pub honor_flag: u32,
    pub name: &'static str,
    pub description: &'static str,
    pub tooltip: &'static str,
}

impl BattleHonorInfo {
    pub const HONORS: &'static [BattleHonorInfo] = &[
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_CAMPAIGN_CHINA,
            name: "China Campaign",
            description: "Complete the China Campaign",
            tooltip: "TOOLTIP:ChinaCampaign",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_CAMPAIGN_GLA,
            name: "GLA Campaign",
            description: "Complete the GLA Campaign",
            tooltip: "TOOLTIP:GLACampaign",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_CAMPAIGN_USA,
            name: "USA Campaign",
            description: "Complete the USA Campaign",
            tooltip: "TOOLTIP:USACampaign",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_CHALLENGE_MODE,
            name: "Challenge Mode",
            description: "Complete Challenge Mode",
            tooltip: "TOOLTIP:ChallengeMode",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_AIR_WING,
            name: "Air Wing",
            description: "Win using only air units",
            tooltip: "TOOLTIP:AirWing",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_BATTLE_TANK,
            name: "Battle Tank",
            description: "Win using only tanks",
            tooltip: "TOOLTIP:BattleTank",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_ENDURANCE,
            name: "Endurance",
            description: "Win on all official maps",
            tooltip: "TOOLTIP:Endurance",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_APOCALYPSE,
            name: "Apocalypse",
            description: "Win without building defenses",
            tooltip: "TOOLTIP:Apocalypse",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_BLITZ10,
            name: "Blitz (10 min)",
            description: "Win in under 10 minutes",
            tooltip: "TOOLTIP:Blitz10",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_BLITZ5,
            name: "Blitz (5 min)",
            description: "Win in under 5 minutes",
            tooltip: "TOOLTIP:Blitz5",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_STREAK,
            name: "Win Streak",
            description: "Win 3+ games in a row",
            tooltip: "TOOLTIP:WinStreak",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_DOMINATION,
            name: "Domination",
            description: "Win 100+ skirmish games",
            tooltip: "TOOLTIP:Domination",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_ULTIMATE,
            name: "Ultimate",
            description: "Beat brutal AI on all maps with all slots filled",
            tooltip: "TOOLTIP:Ultimate",
        },
        BattleHonorInfo {
            honor_flag: BATTLE_HONOR_OFFICERSCLUB,
            name: "Officers Club",
            description: "Pre-order bonus",
            tooltip: "TOOLTIP:OfficersClub",
        },
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_battle_honors_creation() {
        let honors = SkirmishBattleHonors::new();
        assert_eq!(honors.get_wins(), 0);
        assert_eq!(honors.get_losses(), 0);
        assert_eq!(honors.get_win_streak(), 0);
        assert_eq!(honors.get_honors(), 0);
    }

    #[test]
    fn test_record_win() {
        let mut honors = SkirmishBattleHonors::new();
        honors.record_win();

        assert_eq!(honors.get_wins(), 1);
        assert_eq!(honors.get_win_streak(), 1);
        assert_eq!(honors.get_losses(), 0);
    }

    #[test]
    fn test_record_loss() {
        let mut honors = SkirmishBattleHonors::new();
        honors.record_win();
        honors.record_win();
        honors.record_loss();

        assert_eq!(honors.get_wins(), 2);
        assert_eq!(honors.get_losses(), 1);
        assert_eq!(honors.get_win_streak(), 0);
    }

    #[test]
    fn test_win_streak() {
        let mut honors = SkirmishBattleHonors::new();

        for _ in 0..5 {
            honors.record_win();
        }

        assert_eq!(honors.get_win_streak(), 5);
        assert_eq!(honors.get_best_win_streak(), 5);
        assert!(honors.has_honor(BATTLE_HONOR_STREAK));
    }

    #[test]
    fn test_best_win_streak() {
        let mut honors = SkirmishBattleHonors::new();

        // First streak
        for _ in 0..3 {
            honors.record_win();
        }
        honors.record_loss();

        // Second, longer streak
        for _ in 0..5 {
            honors.record_win();
        }

        assert_eq!(honors.get_win_streak(), 5);
        assert_eq!(honors.get_best_win_streak(), 5);
    }

    #[test]
    fn test_award_honor() {
        let mut honors = SkirmishBattleHonors::new();

        honors.award_honor(BATTLE_HONOR_AIR_WING);
        assert!(honors.has_honor(BATTLE_HONOR_AIR_WING));
        assert!(!honors.has_honor(BATTLE_HONOR_BATTLE_TANK));

        honors.award_honor(BATTLE_HONOR_BATTLE_TANK);
        assert!(honors.has_honor(BATTLE_HONOR_AIR_WING));
        assert!(honors.has_honor(BATTLE_HONOR_BATTLE_TANK));
    }

    #[test]
    fn test_domination_honor() {
        let mut honors = SkirmishBattleHonors::new();

        for _ in 0..100 {
            honors.record_win();
        }

        assert!(honors.has_honor(BATTLE_HONOR_DOMINATION));
    }

    #[test]
    fn test_campaign_completion() {
        let mut honors = SkirmishBattleHonors::new();

        honors.set_china_campaign_complete(Difficulty::Easy, true);
        assert!(honors.get_china_campaign_complete(Difficulty::Easy));
        assert!(!honors.get_china_campaign_complete(Difficulty::Normal));
        assert!(honors.has_honor(BATTLE_HONOR_CAMPAIGN_CHINA));
    }

    #[test]
    fn test_all_campaigns() {
        let mut honors = SkirmishBattleHonors::new();

        honors.set_china_campaign_complete(Difficulty::Hard, true);
        honors.set_gla_campaign_complete(Difficulty::Hard, true);
        honors.set_usa_campaign_complete(Difficulty::Hard, true);

        assert!(honors.has_honor(BATTLE_HONOR_CAMPAIGN_CHINA));
        assert!(honors.has_honor(BATTLE_HONOR_CAMPAIGN_GLA));
        assert!(honors.has_honor(BATTLE_HONOR_CAMPAIGN_USA));
    }

    #[test]
    fn test_challenge_completion() {
        let mut honors = SkirmishBattleHonors::new();

        honors.set_challenge_campaign_complete(0, Difficulty::Easy, true);
        assert!(honors.get_challenge_campaign_complete(0, Difficulty::Easy));
        assert!(!honors.get_challenge_campaign_complete(0, Difficulty::Hard));
        assert!(honors.has_honor(BATTLE_HONOR_CHALLENGE_MODE));
    }

    #[test]
    fn test_endurance_medals() {
        let mut honors = SkirmishBattleHonors::new();

        honors.increment_endurance_medal("tournament_desert.map", SlotState::EasyAI);
        honors.increment_endurance_medal("tournament_desert.map", SlotState::EasyAI);

        assert_eq!(honors.get_endurance_medal("tournament_desert.map", SlotState::EasyAI), 2);
        assert_eq!(honors.get_endurance_medal("tournament_desert.map", SlotState::HardAI), 0);
    }

    #[test]
    fn test_clear_stats() {
        let mut honors = SkirmishBattleHonors::new();

        honors.record_win();
        honors.record_win();
        honors.award_honor(BATTLE_HONOR_AIR_WING);

        honors.clear();

        assert_eq!(honors.get_wins(), 0);
        assert_eq!(honors.get_losses(), 0);
        assert_eq!(honors.get_win_streak(), 0);
        assert_eq!(honors.get_honors(), 0);
    }

    #[test]
    fn test_battle_honor_info() {
        assert_eq!(BattleHonorInfo::HONORS.len(), 14);

        let china_honor = &BattleHonorInfo::HONORS[0];
        assert_eq!(china_honor.honor_flag, BATTLE_HONOR_CAMPAIGN_CHINA);
        assert_eq!(china_honor.name, "China Campaign");
    }

    #[test]
    fn test_multiple_difficulty_levels() {
        let mut honors = SkirmishBattleHonors::new();

        honors.set_usa_campaign_complete(Difficulty::Easy, true);
        honors.set_usa_campaign_complete(Difficulty::Normal, true);
        honors.set_usa_campaign_complete(Difficulty::Hard, true);

        assert!(honors.get_usa_campaign_complete(Difficulty::Easy));
        assert!(honors.get_usa_campaign_complete(Difficulty::Normal));
        assert!(honors.get_usa_campaign_complete(Difficulty::Hard));
    }

    #[test]
    fn test_challenge_index_validation() {
        let mut honors = SkirmishBattleHonors::new();

        // Valid index
        honors.set_challenge_campaign_complete(0, Difficulty::Easy, true);
        assert!(honors.get_challenge_campaign_complete(0, Difficulty::Easy));

        // Invalid index (should be ignored)
        honors.set_challenge_campaign_complete(99, Difficulty::Easy, true);
        assert!(!honors.get_challenge_campaign_complete(99, Difficulty::Easy));
    }
}
