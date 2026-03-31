// FILE: campaign_manager.rs
// Ported from: CampaignManager.cpp/h
// Author: Chris Huybregts (original C++), Rust port 2025
// Purpose: Campaign flow management and mission tracking

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

// Constants
pub const MAX_OBJECTIVE_LINES: usize = 5;
pub const MAX_DISPLAYED_UNITS: usize = 3;
pub const INVALID_MISSION_NUMBER: i32 = -1;

// Game difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameDifficulty {
    Easy,
    Normal,
    Hard,
}

// Audio event representation (simplified for Rust)
#[derive(Debug, Clone)]
pub struct AudioEventRTS {
    pub event_name: String,
}

impl AudioEventRTS {
    pub fn new(event_name: &str) -> Self {
        Self {
            event_name: event_name.to_string(),
        }
    }
}

// Mission data structure
#[derive(Debug, Clone)]
pub struct Mission {
    pub name: String,
    pub map_name: String,
    pub next_mission: String,
    pub movie_label: String,
    pub mission_objectives_label: [String; MAX_OBJECTIVE_LINES],
    pub briefing_voice: AudioEventRTS,
    pub location_name_label: String,
    pub unit_names: [String; MAX_DISPLAYED_UNITS],
    pub voice_length: i32,
    pub general_name: String,
}

impl Mission {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            map_name: String::new(),
            next_mission: String::new(),
            movie_label: String::new(),
            mission_objectives_label: Default::default(),
            briefing_voice: AudioEventRTS::new(""),
            location_name_label: String::new(),
            unit_names: Default::default(),
            voice_length: 0,
            general_name: String::new(),
        }
    }

    pub fn with_name(name: String) -> Self {
        let mut mission = Self::new();
        mission.name = name.to_lowercase();
        mission
    }
}

// Campaign data structure
#[derive(Debug, Clone)]
pub struct Campaign {
    pub name: String,
    pub first_mission: String,
    pub campaign_name_label: String,
    pub missions: Vec<Mission>,
    pub final_movie_name: String,
    pub is_challenge_campaign: bool,
    pub player_faction_name: String,
}

impl Campaign {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            first_mission: String::new(),
            campaign_name_label: String::new(),
            missions: Vec::new(),
            final_movie_name: String::new(),
            is_challenge_campaign: false,
            player_faction_name: String::new(),
        }
    }

    pub fn with_name(name: String) -> Self {
        let mut campaign = Self::new();
        campaign.name = name.to_lowercase();
        campaign
    }

    pub fn new_mission(&mut self, name: String) -> &mut Mission {
        let name_lower = name.to_lowercase();

        // Remove existing mission with same name
        self.missions.retain(|m| m.name != name_lower);

        // Add new mission
        self.missions.push(Mission::with_name(name_lower));
        self.missions.last_mut().unwrap()
    }

    pub fn get_mission(&self, mission_name: &str) -> Option<&Mission> {
        if mission_name.is_empty() {
            return None;
        }

        let mission_name_lower = mission_name.to_lowercase();
        self.missions.iter().find(|m| m.name == mission_name_lower)
    }

    pub fn get_next_mission(&self, current: Option<&Mission>) -> Option<&Mission> {
        let name = if let Some(current_mission) = current {
            &current_mission.next_mission
        } else {
            &self.first_mission
        };

        if name.is_empty() {
            return None;
        }

        let name_lower = name.to_lowercase();
        self.missions.iter().find(|m| m.name == name_lower)
    }

    pub fn get_final_victory_movie(&self) -> &str {
        &self.final_movie_name
    }

    pub fn is_challenge_campaign(&self) -> bool {
        self.is_challenge_campaign
    }
}

// Campaign Manager - manages all campaigns and tracks current progress
pub struct CampaignManager {
    campaign_list: Vec<Campaign>,
    current_campaign: Option<usize>,  // Index into campaign_list
    current_mission: Option<usize>,   // Index into current campaign's missions
    victorious: bool,
    current_rank_points: i32,
    difficulty: GameDifficulty,
    xfer_challenge_generals_player_template_num: i32,
}

impl CampaignManager {
    pub fn new() -> Self {
        Self {
            campaign_list: Vec::new(),
            current_campaign: None,
            current_mission: None,
            victorious: false,
            current_rank_points: 0,
            difficulty: GameDifficulty::Normal,
            xfer_challenge_generals_player_template_num: 0,
        }
    }

    pub fn init(&mut self) {
        // In C++ this loads from Data\INI\Campaign.ini
        // For Rust port, this would need INI parsing implementation
        // Placeholder for now - actual implementation would parse campaign data
    }

    pub fn new_campaign(&mut self, name: String) -> &mut Campaign {
        let name_lower = name.to_lowercase();

        // Remove existing campaign with same name
        self.campaign_list.retain(|c| c.name != name_lower);

        // Add new campaign
        self.campaign_list.push(Campaign::with_name(name_lower));
        self.campaign_list.last_mut().unwrap()
    }

    pub fn get_current_campaign(&self) -> Option<&Campaign> {
        self.current_campaign.and_then(|idx| self.campaign_list.get(idx))
    }

    pub fn get_current_campaign_mut(&mut self) -> Option<&mut Campaign> {
        self.current_campaign.and_then(move |idx| self.campaign_list.get_mut(idx))
    }

    pub fn get_current_mission(&self) -> Option<&Mission> {
        if let Some(campaign_idx) = self.current_campaign {
            if let Some(mission_idx) = self.current_mission {
                if let Some(campaign) = self.campaign_list.get(campaign_idx) {
                    return campaign.missions.get(mission_idx);
                }
            }
        }
        None
    }

    pub fn goto_next_mission(&mut self) -> Option<&Mission> {
        if self.current_campaign.is_none() || self.current_mission.is_none() {
            return None;
        }

        let campaign_idx = self.current_campaign.unwrap();
        let current_mission_idx = self.current_mission.unwrap();

        if let Some(campaign) = self.campaign_list.get(campaign_idx) {
            if let Some(current_mission) = campaign.missions.get(current_mission_idx) {
                let next_mission_name = &current_mission.next_mission;

                // Find next mission by name
                for (idx, mission) in campaign.missions.iter().enumerate() {
                    if mission.name == next_mission_name.to_lowercase() {
                        self.current_mission = Some(idx);
                        return Some(mission);
                    }
                }
            }
        }

        None
    }

    pub fn set_campaign_and_mission(&mut self, campaign_name: &str, mission_name: &str) {
        if mission_name.is_empty() {
            self.set_campaign(campaign_name);
            return;
        }

        let campaign_name_lower = campaign_name.to_lowercase();

        for (camp_idx, campaign) in self.campaign_list.iter().enumerate() {
            if campaign.name == campaign_name_lower {
                self.current_campaign = Some(camp_idx);

                // Find mission by name
                let mission_name_lower = mission_name.to_lowercase();
                for (miss_idx, mission) in campaign.missions.iter().enumerate() {
                    if mission.name == mission_name_lower {
                        self.current_mission = Some(miss_idx);
                        return;
                    }
                }
                return;
            }
        }
    }

    pub fn set_campaign(&mut self, campaign_name: &str) {
        let campaign_name_lower = campaign_name.to_lowercase();

        for (idx, campaign) in self.campaign_list.iter().enumerate() {
            if campaign.name == campaign_name_lower {
                self.current_campaign = Some(idx);

                // Set to first mission
                if !campaign.first_mission.is_empty() {
                    let first_mission_lower = campaign.first_mission.to_lowercase();
                    for (miss_idx, mission) in campaign.missions.iter().enumerate() {
                        if mission.name == first_mission_lower {
                            self.current_mission = Some(miss_idx);
                            return;
                        }
                    }
                }
                return;
            }
        }

        // Campaign not found - reset
        self.current_campaign = None;
        self.current_mission = None;
        self.current_rank_points = 0;
        self.difficulty = GameDifficulty::Normal;
    }

    pub fn get_current_map(&self) -> String {
        if let Some(mission) = self.get_current_mission() {
            mission.map_name.clone()
        } else {
            String::new()
        }
    }

    pub fn get_current_mission_number(&self) -> i32 {
        if let Some(campaign_idx) = self.current_campaign {
            if let Some(mission_idx) = self.current_mission {
                return mission_idx as i32;
            }
        }
        INVALID_MISSION_NUMBER
    }

    pub fn is_victorious(&self) -> bool {
        self.victorious
    }

    pub fn set_victorious(&mut self, victory: bool) {
        self.victorious = victory;
    }

    pub fn set_rank_points(&mut self, rank_points: i32) {
        self.current_rank_points = rank_points;
    }

    pub fn get_rank_points(&self) -> i32 {
        // All campaign missions start each map at rank 0
        // This matches the C++ behavior
        0
    }

    pub fn get_game_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    pub fn set_game_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
    }

    pub fn set_xfer_challenge_generals_player_template_num(&mut self, num: i32) {
        self.xfer_challenge_generals_player_template_num = num;
    }

    pub fn get_xfer_challenge_generals_player_template_num(&self) -> i32 {
        self.xfer_challenge_generals_player_template_num
    }

    pub fn load_post_process(&mut self) {
        // Called after loading a saved game
        // In C++ this sets TheChallengeGenerals player template number
    }
}

// Global campaign manager instance (mimics C++ singleton pattern)
static THE_CAMPAIGN_MANAGER: OnceLock<Mutex<CampaignManager>> = OnceLock::new();

pub fn init_campaign_manager() {
    let manager = THE_CAMPAIGN_MANAGER.get_or_init(|| Mutex::new(CampaignManager::new()));
    manager.lock().unwrap().init();
}

pub fn get_campaign_manager() -> Option<&'static Mutex<CampaignManager>> {
    THE_CAMPAIGN_MANAGER.get()
}

pub fn get_campaign_manager_mut() -> Option<std::sync::MutexGuard<'static, CampaignManager>> {
    THE_CAMPAIGN_MANAGER.get().and_then(|m| m.lock().ok())
}
