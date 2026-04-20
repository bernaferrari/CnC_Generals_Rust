// FILE: campaign_manager.rs
// Ported from: CampaignManager.cpp/h
// Author: Chris Huybregts (original C++), Rust port 2025
// Purpose: Campaign flow management and mission tracking

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use game_engine::common::ini::ini_misc_audio::AudioEventRTS;
use game_engine::common::ini::{
    self, get_campaign_store, get_campaign_store_mut, init_campaign_store, Campaign as IniCampaign,
    INILoadType, Mission as IniMission, INI,
};

pub const MAX_OBJECTIVE_LINES: usize = 5;
pub const MAX_DISPLAYED_UNITS: usize = 3;
pub const INVALID_MISSION_NUMBER: i32 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameDifficulty {
    Easy,
    Normal,
    Hard,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        Self::Normal
    }
}

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
            briefing_voice: AudioEventRTS::default(),
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

        self.missions.retain(|m| m.name != name_lower);

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

impl From<&IniCampaign> for Campaign {
    fn from(c: &IniCampaign) -> Self {
        Self {
            name: c.name.clone(),
            first_mission: c.first_mission.clone(),
            campaign_name_label: c.campaign_name_label.clone(),
            missions: c.missions.iter().map(|m| Mission::from(m)).collect(),
            final_movie_name: c.final_movie_name.clone(),
            is_challenge_campaign: c.is_challenge_campaign,
            player_faction_name: c.player_faction_name.clone(),
        }
    }
}

impl From<&IniMission> for Mission {
    fn from(m: &IniMission) -> Self {
        Self {
            name: m.name.clone(),
            map_name: m.map_name.clone(),
            next_mission: m.next_mission.clone(),
            movie_label: m.movie_label.clone(),
            mission_objectives_label: m.mission_objectives_label.clone(),
            briefing_voice: m.briefing_voice.clone(),
            location_name_label: m.location_name_label.clone(),
            unit_names: m.unit_names.clone(),
            voice_length: m.voice_length,
            general_name: m.general_name.clone(),
        }
    }
}

const XFER_VERSION: u16 = 5;

pub struct CampaignManager {
    campaign_list: Vec<Campaign>,
    current_campaign: Option<usize>,
    current_mission: Option<usize>,
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

    /// Load Campaign.ini via INI parsing.
    /// Matches C++ CampaignManager::init() which calls:
    ///   ini.load( AsciiString( "Data\\INI\\Campaign.ini" ), INI_LOAD_OVERWRITE, NULL );
    pub fn init(&mut self) {
        init_campaign_store();

        let sources = discover_campaign_ini_files();
        if sources.is_empty() {
            return;
        }

        {
            let mut store = get_campaign_store_mut();
            store.clear();
        }

        let mut ini = INI::new();
        for (idx, source) in sources.iter().enumerate() {
            let load_type = if idx == 0 {
                INILoadType::Overwrite
            } else {
                INILoadType::MultiFile
            };
            if let Err(err) = ini.load(source, load_type) {
                log::warn!(
                    "CampaignManager::init: failed to load '{}': {}",
                    source.display(),
                    err
                );
            }
        }

        self.sync_from_store();
    }

    /// Pull all parsed campaigns from the global CampaignStore into our local list.
    fn sync_from_store(&mut self) {
        let store = get_campaign_store();
        self.campaign_list = store
            .campaign_names()
            .iter()
            .filter_map(|name| store.get_campaign(name).map(|c| Campaign::from(c)))
            .collect();
    }

    /// Find a campaign by name (case-insensitive).
    /// Matches C++ CampaignManager::findCampaign behavior (linear scan).
    pub fn find_campaign(&self, campaign_name: &str) -> Option<&Campaign> {
        if campaign_name.is_empty() {
            return None;
        }
        let name_lower = campaign_name.to_lowercase();
        self.campaign_list.iter().find(|c| c.name == name_lower)
    }

    /// Set campaign difficulty. Matches C++ CampaignManager::setDifficulty().
    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
    }

    /// Alias for set_difficulty() - used by existing callers.
    pub fn set_game_difficulty(&mut self, difficulty: GameDifficulty) {
        self.set_difficulty(difficulty);
    }

    pub fn get_game_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    /// Get the next mission in the current campaign progression.
    /// Matches C++ CampaignManager::getNextMission() - returns NULL at end.
    pub fn get_next_mission(&self) -> Option<&Mission> {
        let campaign = self.get_current_campaign()?;
        let current_mission = self.get_current_mission();
        campaign.get_next_mission(current_mission)
    }

    /// Advance to the next mission. Matches C++ CampaignManager::gotoNextMission().
    pub fn goto_next_mission(&mut self) -> Option<&Mission> {
        if self.current_campaign.is_none() || self.current_mission.is_none() {
            return None;
        }

        let campaign_idx = self.current_campaign.unwrap();
        let current_mission_idx = self.current_mission.unwrap();

        if let Some(campaign) = self.campaign_list.get(campaign_idx) {
            if let Some(current_mission) = campaign.missions.get(current_mission_idx) {
                let next_name = &current_mission.next_mission;
                if next_name.is_empty() {
                    return None;
                }

                for (idx, mission) in campaign.missions.iter().enumerate() {
                    if mission.name == next_name.to_lowercase() {
                        self.current_mission = Some(idx);
                        return Some(mission);
                    }
                }
            }
        }

        None
    }

    /// Set campaign and optionally a specific mission.
    /// Matches C++ CampaignManager::setCampaignAndMission().
    pub fn set_campaign_and_mission(&mut self, campaign_name: &str, mission_name: &str) {
        if mission_name.is_empty() {
            self.set_campaign(campaign_name);
            return;
        }

        let campaign_name_lower = campaign_name.to_lowercase();

        for (camp_idx, campaign) in self.campaign_list.iter().enumerate() {
            if campaign.name == campaign_name_lower {
                self.current_campaign = Some(camp_idx);

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

    /// Set the current campaign and advance to its first mission.
    /// Matches C++ CampaignManager::setCampaign() - resets state if campaign not found.
    pub fn set_campaign(&mut self, campaign_name: &str) {
        let name_lower = campaign_name.to_lowercase();

        for (idx, campaign) in self.campaign_list.iter().enumerate() {
            if campaign.name == name_lower {
                self.current_campaign = Some(idx);

                if !campaign.first_mission.is_empty() {
                    let first_lower = campaign.first_mission.to_lowercase();
                    for (miss_idx, mission) in campaign.missions.iter().enumerate() {
                        if mission.name == first_lower {
                            self.current_mission = Some(miss_idx);
                            return;
                        }
                    }
                }
                self.current_mission = None;
                return;
            }
        }

        self.current_campaign = None;
        self.current_mission = None;
        self.current_rank_points = 0;
        self.difficulty = GameDifficulty::Normal;
    }

    pub fn new_campaign(&mut self, name: String) -> &mut Campaign {
        let name_lower = name.to_lowercase();

        self.campaign_list.retain(|c| c.name != name_lower);

        self.campaign_list.push(Campaign::with_name(name_lower));
        self.campaign_list.last_mut().unwrap()
    }

    pub fn get_current_campaign(&self) -> Option<&Campaign> {
        self.current_campaign
            .and_then(|idx| self.campaign_list.get(idx))
    }

    pub fn get_current_campaign_mut(&mut self) -> Option<&mut Campaign> {
        self.current_campaign
            .and_then(move |idx| self.campaign_list.get_mut(idx))
    }

    pub fn get_current_mission(&self) -> Option<&Mission> {
        if let Some(campaign_idx) = self.current_campaign {
            if let Some(mission_idx) = self.current_mission {
                return self
                    .campaign_list
                    .get(campaign_idx)
                    .and_then(|campaign| campaign.missions.get(mission_idx));
            }
        }
        None
    }

    pub fn get_current_mission_mut(&mut self) -> Option<&mut Mission> {
        if let Some(campaign_idx) = self.current_campaign {
            if let Some(mission_idx) = self.current_mission {
                return self
                    .campaign_list
                    .get_mut(campaign_idx)
                    .and_then(move |campaign| campaign.missions.get_mut(mission_idx));
            }
        }
        None
    }

    pub fn get_campaign(&self, campaign_name: &str) -> Option<&Campaign> {
        if campaign_name.is_empty() {
            return None;
        }

        let name_lower = campaign_name.to_lowercase();
        self.campaign_list.iter().find(|c| c.name == name_lower)
    }

    pub fn get_campaign_mut(&mut self, campaign_name: &str) -> Option<&mut Campaign> {
        if campaign_name.is_empty() {
            return None;
        }

        let name_lower = campaign_name.to_lowercase();
        self.campaign_list.iter_mut().find(|c| c.name == name_lower)
    }

    pub fn set_current_mission(&mut self, mission_name: &str) {
        if let Some(campaign_idx) = self.current_campaign {
            if let Some(mission_idx) = self.campaign_list.get(campaign_idx).and_then(|campaign| {
                campaign
                    .missions
                    .iter()
                    .position(|m| m.name == mission_name.to_lowercase())
            }) {
                self.current_mission = Some(mission_idx);
            }
        }
    }

    pub fn get_current_map(&self) -> Option<String> {
        self.get_current_mission()
            .map(|mission| mission.map_name.clone())
    }

    pub fn get_current_mission_number(&self) -> Option<i32> {
        self.current_mission.map(|idx| idx as i32)
    }

    pub fn set_victorious(&mut self, victorious: bool) {
        self.victorious = victorious;
    }

    pub fn is_victorious(&self) -> bool {
        self.victorious
    }

    pub fn set_rank_points(&mut self, points: i32) {
        self.current_rank_points = points;
    }

    pub fn get_rank_points(&self) -> i32 {
        self.current_rank_points
    }

    pub fn set_xfer_challenge_generals_player_template_num(&mut self, num: i32) {
        self.xfer_challenge_generals_player_template_num = num;
    }

    pub fn get_xfer_challenge_generals_player_template_num(&self) -> i32 {
        self.xfer_challenge_generals_player_template_num
    }

    /// Xfer (save/load) method.
    /// Matches C++ CampaignManager::xfer() version 5.
    ///
    /// When loading, reconstructs campaign/mission state from the parsed CampaignStore.
    pub fn xfer<X: XferHelper>(&mut self, xfer: &mut X) -> std::io::Result<()> {
        let mut version = XFER_VERSION;
        xfer.xfer_version(&mut version, XFER_VERSION)?;

        let mut current_campaign_name = String::new();
        if let Some(campaign) = self.get_current_campaign() {
            current_campaign_name = campaign.name.clone();
        }
        xfer.xfer_ascii_string(&mut current_campaign_name)?;

        let mut current_mission_name = String::new();
        if let Some(mission) = self.get_current_mission() {
            current_mission_name = mission.name.clone();
        }
        xfer.xfer_ascii_string(&mut current_mission_name)?;

        if version >= 2 {
            xfer.xfer_int(&mut self.current_rank_points)?;
        }

        if version >= 3 {
            xfer.xfer_user(&mut self.difficulty)?;
        }

        if xfer.is_loading() {
            self.set_campaign_and_mission(&current_campaign_name, &current_mission_name);
        }

        if version >= 4 {
            let mut is_challenge = self
                .get_current_campaign()
                .map(|c| c.is_challenge_campaign)
                .unwrap_or(false);
            xfer.xfer_bool(&mut is_challenge)?;
        }

        if version >= 5 {
            xfer.xfer_int(&mut self.xfer_challenge_generals_player_template_num)?;
        }

        Ok(())
    }

    /// Post-load processing. Matches C++ CampaignManager::loadPostProcess().
    pub fn load_post_process(&mut self) {
        if let Some(mut generals) = crate::gui::challenge_generals::get_challenge_generals_mut() {
            generals
                .set_current_player_template_num(self.xfer_challenge_generals_player_template_num);
        }
    }
}

pub trait XferHelper {
    fn xfer_version(&mut self, version: &mut u16, current: u16) -> std::io::Result<()>;
    fn xfer_ascii_string(&mut self, s: &mut String) -> std::io::Result<()>;
    fn xfer_int(&mut self, value: &mut i32) -> std::io::Result<()>;
    fn xfer_bool(&mut self, value: &mut bool) -> std::io::Result<()>;
    fn xfer_user<T>(&mut self, value: &mut T) -> std::io::Result<()>;
    fn is_loading(&self) -> bool;
}

fn discover_campaign_ini_files() -> Vec<PathBuf> {
    let mut roots = std::collections::BTreeSet::new();
    if let Ok(cwd) = env::current_dir() {
        for ancestor in cwd.ancestors() {
            roots.insert(ancestor.to_path_buf());
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            for ancestor in parent.ancestors() {
                roots.insert(ancestor.to_path_buf());
            }
        }
    }

    let mut seen = HashSet::new();
    let mut files = Vec::new();

    for root in &roots {
        push_ini_file(&mut files, &mut seen, root.join("Data/INI/Campaign.ini"));
        push_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/Default/Campaign.ini"),
        );

        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/Campaign.ini"),
            );
            push_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/Default/Campaign.ini"),
            );
        }
    }

    files
}

fn push_ini_file(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if path.is_file() {
        let key = fs::canonicalize(&path).unwrap_or(path.clone());
        if seen.insert(key) {
            files.push(path);
        }
    }
}

static CAMPAIGN_MANAGER: std::sync::OnceLock<std::sync::Mutex<CampaignManager>> =
    std::sync::OnceLock::new();

pub fn get_campaign_manager() -> std::sync::MutexGuard<'static, CampaignManager> {
    let lock = CAMPAIGN_MANAGER.get_or_init(|| std::sync::Mutex::new(CampaignManager::new()));
    lock.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_campaign_manager_creation() {
        let manager = CampaignManager::new();
        assert!(manager.get_current_campaign().is_none());
        assert_eq!(manager.get_rank_points(), 0);
        assert_eq!(manager.get_game_difficulty(), GameDifficulty::Normal);
    }

    #[test]
    fn test_campaign_creation_and_retrieval() {
        let mut manager = CampaignManager::new();

        manager.new_campaign("TestCampaign".to_string());

        let campaign = manager.get_campaign("TestCampaign").unwrap();
        assert_eq!(campaign.name, "testcampaign");
    }

    #[test]
    fn test_mission_creation() {
        let mut campaign = Campaign::new();
        let mission = campaign.new_mission("Mission1".to_string());

        assert_eq!(mission.name, "mission1");
        assert!(campaign.get_mission("Mission1").is_some());
    }

    #[test]
    fn test_find_campaign() {
        let mut manager = CampaignManager::new();

        manager.new_campaign("China".to_string());
        manager.new_campaign("USA".to_string());
        manager.new_campaign("GLA".to_string());

        assert!(manager.find_campaign("china").is_some());
        assert!(manager.find_campaign("USA").is_some());
        assert!(manager.find_campaign("Gla").is_some());
        assert!(manager.find_campaign("nonexistent").is_none());
        assert!(manager.find_campaign("").is_none());
    }

    #[test]
    fn test_set_difficulty() {
        let mut manager = CampaignManager::new();

        assert_eq!(manager.get_game_difficulty(), GameDifficulty::Normal);
        manager.set_difficulty(GameDifficulty::Hard);
        assert_eq!(manager.get_game_difficulty(), GameDifficulty::Hard);
        manager.set_difficulty(GameDifficulty::Easy);
        assert_eq!(manager.get_game_difficulty(), GameDifficulty::Easy);
    }

    #[test]
    fn test_set_campaign_resets_on_not_found() {
        let mut manager = CampaignManager::new();

        manager.set_rank_points(100);
        manager.set_difficulty(GameDifficulty::Hard);

        manager.set_campaign("NonExistent");

        assert!(manager.get_current_campaign().is_none());
        assert_eq!(manager.get_rank_points(), 0);
        assert_eq!(manager.get_game_difficulty(), GameDifficulty::Normal);
    }

    #[test]
    fn test_set_campaign_resets_on_empty() {
        let mut manager = CampaignManager::new();

        let campaign = manager.new_campaign("Test".to_string());
        campaign.first_mission = "mission1".to_string();
        campaign.new_mission("Mission1".to_string());
        manager.set_campaign("Test");
        assert!(manager.get_current_campaign().is_some());

        manager.set_rank_points(200);
        manager.set_difficulty(GameDifficulty::Hard);
        manager.set_campaign("");

        assert!(manager.get_current_campaign().is_none());
        assert!(manager.get_current_mission().is_none());
        assert_eq!(manager.get_rank_points(), 0);
        assert_eq!(manager.get_game_difficulty(), GameDifficulty::Normal);
    }

    #[test]
    fn test_set_campaign_advances_to_first_mission() {
        let mut manager = CampaignManager::new();

        let campaign = manager.new_campaign("Test".to_string());
        campaign.first_mission = "mission1".to_string();
        campaign.new_mission("Mission1".to_string());
        campaign.new_mission("Mission2".to_string());

        manager.set_campaign("Test");

        assert!(manager.get_current_campaign().is_some());
        let mission = manager.get_current_mission().unwrap();
        assert_eq!(mission.name, "mission1");
    }

    #[test]
    fn test_set_campaign_and_mission() {
        let mut manager = CampaignManager::new();

        let campaign = manager.new_campaign("Test".to_string());
        campaign.first_mission = "mission1".to_string();
        campaign.new_mission("Mission1".to_string());
        campaign.new_mission("Mission2".to_string());
        campaign.new_mission("Mission3".to_string());

        // With specific mission
        manager.set_campaign_and_mission("Test", "Mission3");
        let mission = manager.get_current_mission().unwrap();
        assert_eq!(mission.name, "mission3");

        // With empty mission falls back to first
        manager.set_campaign_and_mission("Test", "");
        let mission = manager.get_current_mission().unwrap();
        assert_eq!(mission.name, "mission1");
    }

    #[test]
    fn test_goto_next_mission() {
        let mut manager = CampaignManager::new();

        let campaign = manager.new_campaign("Test".to_string());
        campaign.first_mission = "mission1".to_string();

        let m1 = campaign.new_mission("Mission1".to_string());
        m1.next_mission = "mission2".to_string();

        let m2 = campaign.new_mission("Mission2".to_string());
        m2.next_mission = "mission3".to_string();

        let _m3 = campaign.new_mission("Mission3".to_string());

        manager.set_campaign("Test");
        assert_eq!(manager.get_current_mission().unwrap().name, "mission1");

        assert!(manager.goto_next_mission().is_some());
        assert_eq!(manager.get_current_mission().unwrap().name, "mission2");

        assert!(manager.goto_next_mission().is_some());
        assert_eq!(manager.get_current_mission().unwrap().name, "mission3");

        // End of campaign - no next mission
        assert!(manager.goto_next_mission().is_none());
        assert_eq!(manager.get_current_mission().unwrap().name, "mission3");
    }

    #[test]
    fn test_get_next_mission_without_advancing() {
        let mut manager = CampaignManager::new();

        let campaign = manager.new_campaign("Test".to_string());
        campaign.first_mission = "mission1".to_string();

        let m1 = campaign.new_mission("Mission1".to_string());
        m1.next_mission = "mission2".to_string();

        campaign.new_mission("Mission2".to_string());

        manager.set_campaign("Test");

        let next = manager.get_next_mission().unwrap();
        assert_eq!(next.name, "mission2");

        // Should not have advanced
        assert_eq!(manager.get_current_mission().unwrap().name, "mission1");
    }

    #[test]
    fn test_get_current_mission_number() {
        let mut manager = CampaignManager::new();

        assert_eq!(manager.get_current_mission_number(), None);

        let campaign = manager.new_campaign("Test".to_string());
        campaign.first_mission = "m1".to_string();

        let m1 = campaign.new_mission("M1".to_string());
        m1.next_mission = "m2".to_string();

        let m2 = campaign.new_mission("M2".to_string());
        m2.next_mission = "m3".to_string();

        campaign.new_mission("M3".to_string());

        manager.set_campaign("Test");
        assert_eq!(manager.get_current_mission_number(), Some(0));

        manager.goto_next_mission();
        assert_eq!(manager.get_current_mission_number(), Some(1));

        manager.goto_next_mission();
        assert_eq!(manager.get_current_mission_number(), Some(2));
    }

    #[test]
    fn test_victory_flag() {
        let mut manager = CampaignManager::new();
        assert!(!manager.is_victorious());

        manager.set_victorious(true);
        assert!(manager.is_victorious());

        manager.set_victorious(false);
        assert!(!manager.is_victorious());
    }

    #[test]
    fn test_challenge_generals_template_num() {
        let mut manager = CampaignManager::new();
        assert_eq!(manager.get_xfer_challenge_generals_player_template_num(), 0);

        manager.set_xfer_challenge_generals_player_template_num(42);
        assert_eq!(
            manager.get_xfer_challenge_generals_player_template_num(),
            42
        );
    }
}
