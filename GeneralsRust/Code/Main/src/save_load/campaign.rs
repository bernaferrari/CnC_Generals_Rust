use crate::game_logic::*;
use crate::save_load::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Campaign identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CampaignId {
    USACampaign,
    ChinaCampaign,
    GLACampaign,
    USAGeneral,
    ChinaGeneral,
    GLAGeneral,
    Challenge,
}

impl CampaignId {
    pub fn get_name(&self) -> &'static str {
        match self {
            CampaignId::USACampaign => "USA Campaign",
            CampaignId::ChinaCampaign => "China Campaign",
            CampaignId::GLACampaign => "GLA Campaign",
            CampaignId::USAGeneral => "USA General Challenge",
            CampaignId::ChinaGeneral => "China General Challenge",
            CampaignId::GLAGeneral => "GLA General Challenge",
            CampaignId::Challenge => "Special Challenge",
        }
    }

    pub fn get_faction(&self) -> Team {
        match self {
            CampaignId::USACampaign | CampaignId::USAGeneral => Team::USA,
            CampaignId::ChinaCampaign | CampaignId::ChinaGeneral => Team::China,
            CampaignId::GLACampaign | CampaignId::GLAGeneral => Team::GLA,
            CampaignId::Challenge => Team::Neutral,
        }
    }
}

/// Mission completion status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionStatus {
    Locked,
    Available,
    Completed,
    CompletedPerfect, // All objectives + bonus objectives
}

/// Mission difficulty settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionDifficulty {
    Easy,
    Normal,
    Hard,
    Brutal,
}

impl MissionDifficulty {
    pub fn get_name(&self) -> &'static str {
        match self {
            MissionDifficulty::Easy => "Easy",
            MissionDifficulty::Normal => "Normal",
            MissionDifficulty::Hard => "Hard",
            MissionDifficulty::Brutal => "Brutal",
        }
    }

    pub fn get_score_multiplier(&self) -> f32 {
        match self {
            MissionDifficulty::Easy => 0.8,
            MissionDifficulty::Normal => 1.0,
            MissionDifficulty::Hard => 1.2,
            MissionDifficulty::Brutal => 1.5,
        }
    }
}

/// Individual mission information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionInfo {
    pub id: String,
    pub campaign_id: CampaignId,
    pub mission_number: u32,
    pub name: String,
    pub description: String,
    pub map_name: String,
    pub briefing_video: Option<String>,
    pub preview_image: Option<String>,

    // Unlock conditions
    pub required_missions: Vec<String>,
    pub required_rank: Option<u32>,
    pub required_honor_points: Option<u32>,

    // Mission parameters
    pub time_limit: Option<u32>, // seconds
    pub starting_resources: Resources,
    pub starting_units: Vec<String>,
    pub tech_restrictions: Vec<String>,
    pub special_rules: Vec<String>,
    /// Optional victory rule override: "Annihilation", "NoUnits", "NoBuildings", etc.
    pub victory_rule: Option<String>,

    // Objectives
    pub primary_objectives: Vec<MissionObjective>,
    pub secondary_objectives: Vec<MissionObjective>,
    pub bonus_objectives: Vec<MissionObjective>,
}

/// Mission objective
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionObjective {
    pub id: String,
    pub description: String,
    pub objective_type: ObjectiveType,
    pub target: ObjectiveTarget,
    pub required_count: Option<u32>,
    pub current_count: u32,
    pub time_limit: Option<u32>,
    pub reward: Option<ObjectiveReward>,
}

/// Match a loaded map path/name against a mission's `map_name` field.
///
/// Accepts exact (case-insensitive) matches and path-stem matches so
/// `MD_USA01` resolves `.../Maps/MD_USA01/MD_USA01.map` and vice versa.
pub fn map_name_matches_mission(query: &str, mission_map: &str) -> bool {
    if query.is_empty() || mission_map.is_empty() {
        return false;
    }
    if query.eq_ignore_ascii_case(mission_map) {
        return true;
    }
    let stem = |s: &str| -> String {
        std::path::Path::new(s)
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or(s)
            .to_ascii_uppercase()
    };
    let q = stem(query);
    let m = stem(mission_map);
    if !q.is_empty() && q == m {
        return true;
    }
    // Path contains short mission map token as a path segment / basename.
    let upper_q = query.replace('\\', "/").to_ascii_uppercase();
    let upper_m = mission_map.replace('\\', "/").to_ascii_uppercase();
    if upper_q.contains(&format!("/{m}/"))
        || upper_q.ends_with(&format!("/{m}.MAP"))
        || upper_q.ends_with(&format!("/{m}"))
    {
        return true;
    }
    if upper_m.contains(&format!("/{q}/"))
        || upper_m.ends_with(&format!("/{q}.MAP"))
        || upper_m.ends_with(&format!("/{q}"))
    {
        return true;
    }
    false
}

/// Types of mission objectives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectiveType {
    Destroy,
    Capture,
    Defend,
    Build,
    Collect,
    Survive,
    Reach,
    Escort,
    Custom(String),
}

/// Objective targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectiveTarget {
    SpecificUnit(String),
    UnitType(String),
    Building(String),
    Area(glam::Vec3, f32), // position, radius
    Player(u32),
    Resource(String),
    Custom(String),
}

/// Objective rewards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectiveReward {
    HonorPoints(u32),
    UnlockUnit(String),
    UnlockBuilding(String),
    UnlockUpgrade(String),
    Resources(Resources),
    Reinforcements(Vec<String>),
}

/// Mission completion record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionCompletion {
    pub mission_id: String,
    pub campaign_id: CampaignId,
    pub status: MissionStatus,
    pub difficulty: MissionDifficulty,
    pub completion_time: SystemTime,
    pub play_duration: std::time::Duration,
    pub score: u32,

    // Objective completion
    pub completed_primary: Vec<String>,
    pub completed_secondary: Vec<String>,
    pub completed_bonus: Vec<String>,

    // Statistics
    pub units_built: u32,
    pub units_lost: u32,
    pub enemies_destroyed: u32,
    pub resources_gathered: u32,
    pub buildings_constructed: u32,
    pub special_powers_used: u32,

    // Performance metrics
    pub perfect_completion: bool,
    pub under_time_limit: bool,
    pub no_losses: bool,
    pub stealth_completion: bool,
}

/// Battle honors (medals/achievements)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleHonor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub honor_points: u32,
    pub unlock_condition: HonorCondition,
    pub campaign_id: Option<CampaignId>,
    pub earned_date: Option<SystemTime>,
}

/// Honor unlock conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HonorCondition {
    CompleteMission(String),
    CompleteCampaign(CampaignId),
    CompleteWithDifficulty(String, MissionDifficulty),
    KillEnemies(u32),
    BuildUnits(u32),
    GatherResources(u32),
    WinWithoutLosses(String),
    CompleteInTime(String, u32),  // mission_id, seconds
    UseSpecialPower(String, u32), // power_name, count
    Custom(String),
}

/// Player's overall campaign progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignProgress {
    pub version: u32,
    pub player_name: String,
    pub total_play_time: std::time::Duration,
    pub last_played: SystemTime,

    // Mission progress
    pub completed_missions: HashMap<String, MissionCompletion>,
    pub current_campaign: Option<CampaignId>,
    pub current_mission: Option<String>,

    // Battle honors and achievements
    pub earned_honors: HashMap<String, BattleHonor>,
    pub total_honor_points: u32,
    pub current_rank: u32,

    // Global statistics
    pub global_stats: GlobalCampaignStats,

    // Unlocked content
    pub unlocked_units: Vec<String>,
    pub unlocked_buildings: Vec<String>,
    pub unlocked_upgrades: Vec<String>,
    pub unlocked_generals: Vec<String>,

    // Settings and preferences
    pub preferred_difficulty: MissionDifficulty,
    pub show_cutscenes: bool,
    pub show_briefings: bool,
}

/// Global campaign statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalCampaignStats {
    pub missions_completed: u32,
    pub missions_perfect: u32,
    pub total_score: u64,
    pub total_kills: u64,
    pub total_built: u64,
    pub total_resources: u64,
    pub total_play_time: std::time::Duration,
    pub campaigns_completed: u32,
    pub favorite_faction: Option<Team>,
}

/// Campaign manager for progress tracking
pub struct CampaignManager {
    campaign_directory: PathBuf,
    pub mission_definitions: HashMap<String, MissionInfo>,
    honor_definitions: HashMap<String, BattleHonor>,
    player_progress: CampaignProgress,

    // Mission save state for between-mission saves
    mission_save_state: Option<MissionSaveState>,
}

/// State saved between missions (carryover units, resources, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionSaveState {
    pub campaign_id: CampaignId,
    pub current_mission: String,
    pub next_mission: Option<String>,

    // Carryover state
    pub carried_units: Vec<CarryoverUnit>,
    pub bonus_resources: Resources,
    pub active_upgrades: Vec<String>,
    pub commander_experience: f32,

    // Story state
    pub story_flags: HashMap<String, bool>,
    pub cutscene_progress: Vec<String>,
}

/// Unit that carries over between missions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarryoverUnit {
    pub template_name: String,
    pub experience: Experience,
    pub health_percentage: f32,
    pub upgrades: Vec<String>,
    pub custom_name: Option<String>,
}

impl CampaignManager {
    pub fn new() -> Self {
        let campaign_dir = SaveLoadManager::default_save_directory().join("Campaign");

        Self {
            campaign_directory: campaign_dir,
            mission_definitions: HashMap::new(),
            honor_definitions: HashMap::new(),
            player_progress: CampaignProgress::new(),
            mission_save_state: None,
        }
    }

    pub fn init(&mut self) -> SaveLoadResult<()> {
        // Create campaign directory
        std::fs::create_dir_all(&self.campaign_directory)?;

        // Load mission definitions
        self.load_mission_definitions()?;

        // Load battle honor definitions
        self.load_honor_definitions()?;

        // Load player progress
        self.load_player_progress()?;

        Ok(())
    }

    /// Find a mission definition for a loaded map path or short name.
    ///
    /// Preference order:
    /// 1. Exact map_name match with objectives
    /// 2. Exact map_name match without objectives
    /// 3. Stem/path match with objectives
    /// 4. Stem/path match without objectives
    ///
    /// Exact-first keeps path-registered residual missions (victory_rule /
    /// objectives) ahead of short-name Campaign.ini table entries.
    pub fn find_mission_for_map(&self, map_name: &str) -> Option<&MissionInfo> {
        let mut exact_with_obj: Option<&MissionInfo> = None;
        let mut exact: Option<&MissionInfo> = None;
        let mut stem_with_obj: Option<&MissionInfo> = None;
        let mut stem: Option<&MissionInfo> = None;
        for mission in self.mission_definitions.values() {
            let has_objectives = !mission.primary_objectives.is_empty()
                || !mission.secondary_objectives.is_empty()
                || !mission.bonus_objectives.is_empty();
            if mission.map_name.eq_ignore_ascii_case(map_name) {
                if has_objectives {
                    if exact_with_obj.is_none() {
                        exact_with_obj = Some(mission);
                    }
                } else if exact.is_none() {
                    exact = Some(mission);
                }
                continue;
            }
            if map_name_matches_mission(map_name, &mission.map_name) {
                if has_objectives {
                    if stem_with_obj.is_none() {
                        stem_with_obj = Some(mission);
                    }
                } else if stem.is_none() {
                    stem = Some(mission);
                }
            }
        }
        exact_with_obj.or(exact).or(stem_with_obj).or(stem)
    }

    /// Start a new campaign
    pub fn start_campaign(
        &mut self,
        campaign_id: CampaignId,
        player_name: &str,
    ) -> SaveLoadResult<()> {
        self.player_progress.player_name = player_name.to_string();
        self.player_progress.current_campaign = Some(campaign_id);

        // Find first mission of campaign
        let first_mission = self.find_first_mission(campaign_id)?;
        self.player_progress.current_mission = Some(first_mission);

        // Reset mission save state
        self.mission_save_state = None;

        self.save_player_progress()?;

        log::info!(
            "Started {} for player {}",
            campaign_id.get_name(),
            player_name
        );
        Ok(())
    }

    /// Complete a mission and update progress
    pub fn complete_mission(
        &mut self,
        mission_id: &str,
        difficulty: MissionDifficulty,
        completion_data: MissionCompletionData,
    ) -> SaveLoadResult<()> {
        let mission_info = self
            .mission_definitions
            .get(mission_id)
            .ok_or(SaveLoadError::InvalidFormat)?;

        // Create completion record
        let completion = MissionCompletion {
            mission_id: mission_id.to_string(),
            campaign_id: mission_info.campaign_id,
            status: if completion_data.perfect_completion {
                MissionStatus::CompletedPerfect
            } else {
                MissionStatus::Completed
            },
            difficulty,
            completion_time: SystemTime::now(),
            play_duration: completion_data.play_duration,
            score: completion_data.score,
            completed_primary: completion_data.completed_primary,
            completed_secondary: completion_data.completed_secondary,
            completed_bonus: completion_data.completed_bonus,
            units_built: completion_data.units_built,
            units_lost: completion_data.units_lost,
            enemies_destroyed: completion_data.enemies_destroyed,
            resources_gathered: completion_data.resources_gathered,
            buildings_constructed: completion_data.buildings_constructed,
            special_powers_used: completion_data.special_powers_used,
            perfect_completion: completion_data.perfect_completion,
            under_time_limit: completion_data.under_time_limit,
            no_losses: completion_data.no_losses,
            stealth_completion: completion_data.stealth_completion,
        };

        // Update progress
        self.player_progress
            .completed_missions
            .insert(mission_id.to_string(), completion.clone());

        // Update global stats
        self.update_global_stats(&completion);

        // Check for unlocked content
        self.check_unlocks(mission_id, &completion)?;

        // Check for earned honors
        self.check_battle_honors(mission_id, &completion)?;

        // Unlock next missions
        self.unlock_next_missions(mission_id)?;

        // Save progress
        self.save_player_progress()?;

        log::info!(
            "Mission {} completed with score {}",
            mission_id,
            completion_data.score
        );
        Ok(())
    }

    /// Save mission state for between-mission saves
    pub fn save_mission_state(&mut self, save_state: MissionSaveState) -> SaveLoadResult<()> {
        self.mission_save_state = Some(save_state.clone());

        // Save to file
        let save_path = self.campaign_directory.join("mission_state.dat");
        let data = bincode::serialize(&save_state)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        let compressed = compression::compress(&data)?;
        std::fs::write(&save_path, compressed)?;

        Ok(())
    }

    /// Load mission state for continuing campaign
    pub fn load_mission_state(&mut self) -> SaveLoadResult<Option<MissionSaveState>> {
        let save_path = self.campaign_directory.join("mission_state.dat");

        if !save_path.exists() {
            return Ok(None);
        }

        let compressed = std::fs::read(&save_path)?;
        let data = compression::decompress(&compressed)?;

        let save_state: MissionSaveState =
            bincode::deserialize(&data).map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        self.mission_save_state = Some(save_state.clone());
        Ok(Some(save_state))
    }

    /// Get mission information
    pub fn get_mission_info(&self, mission_id: &str) -> Option<&MissionInfo> {
        self.mission_definitions.get(mission_id)
    }

    /// Get mission status for player
    pub fn get_mission_status(&self, mission_id: &str) -> MissionStatus {
        if let Some(completion) = self.player_progress.completed_missions.get(mission_id) {
            completion.status
        } else if self.is_mission_available(mission_id) {
            MissionStatus::Available
        } else {
            MissionStatus::Locked
        }
    }

    /// Check if mission is available to play
    pub fn is_mission_available(&self, mission_id: &str) -> bool {
        let Some(mission_info) = self.mission_definitions.get(mission_id) else {
            return false;
        };

        // Check if all required missions are completed
        for required_mission in &mission_info.required_missions {
            if !self
                .player_progress
                .completed_missions
                .contains_key(required_mission)
            {
                return false;
            }
        }

        // Check rank requirement
        if let Some(required_rank) = mission_info.required_rank {
            if self.player_progress.current_rank < required_rank {
                return false;
            }
        }

        // Check honor points requirement
        if let Some(required_points) = mission_info.required_honor_points {
            if self.player_progress.total_honor_points < required_points {
                return false;
            }
        }

        true
    }

    /// Get list of available missions for campaign
    pub fn get_available_missions(&self, campaign_id: CampaignId) -> Vec<&MissionInfo> {
        self.mission_definitions
            .values()
            .filter(|mission| {
                mission.campaign_id == campaign_id && self.is_mission_available(&mission.id)
            })
            .collect()
    }

    /// Get campaign completion percentage
    pub fn get_campaign_completion(&self, campaign_id: CampaignId) -> f32 {
        let total_missions: Vec<_> = self
            .mission_definitions
            .values()
            .filter(|mission| mission.campaign_id == campaign_id)
            .collect();

        if total_missions.is_empty() {
            return 0.0;
        }

        let completed = total_missions
            .iter()
            .filter(|mission| {
                self.player_progress
                    .completed_missions
                    .contains_key(&mission.id)
            })
            .count();

        completed as f32 / total_missions.len() as f32
    }

    /// Return the currently active campaign id, if any.
    pub fn current_campaign_id(&self) -> Option<CampaignId> {
        self.player_progress.current_campaign
    }

    /// Return the current campaign side name used by save metadata.
    pub fn current_campaign_side_name(&self) -> Option<&'static str> {
        match self.player_progress.current_campaign {
            Some(CampaignId::USACampaign | CampaignId::USAGeneral) => Some("USA"),
            Some(CampaignId::ChinaCampaign | CampaignId::ChinaGeneral) => Some("China"),
            Some(CampaignId::GLACampaign | CampaignId::GLAGeneral) => Some("GLA"),
            Some(CampaignId::Challenge) => Some("Challenge"),
            None => None,
        }
    }

    /// Return the active mission id for the current campaign flow.
    pub fn current_mission_id(&self) -> Option<&str> {
        self.player_progress.current_mission.as_deref()
    }

    /// Return the active mission number from mission definitions.
    pub fn current_mission_number(&self) -> Option<u32> {
        let mission_id = self.player_progress.current_mission.as_deref()?;
        self.mission_definitions
            .get(mission_id)
            .map(|mission| mission.mission_number)
    }

    /// Get player's current rank based on honor points
    pub fn calculate_rank(&self) -> u32 {
        let points = self.player_progress.total_honor_points;

        // Rank progression (similar to original game)
        match points {
            0..=99 => 1,      // Private
            100..=299 => 2,   // Corporal
            300..=599 => 3,   // Sergeant
            600..=999 => 4,   // Lieutenant
            1000..=1499 => 5, // Captain
            1500..=2099 => 6, // Major
            2100..=2799 => 7, // Colonel
            _ => 8,           // General
        }
    }

    /// Get rank name
    pub fn get_rank_name(rank: u32) -> &'static str {
        match rank {
            1 => "Private",
            2 => "Corporal",
            3 => "Sergeant",
            4 => "Lieutenant",
            5 => "Captain",
            6 => "Major",
            7 => "Colonel",
            8 => "General",
            _ => "Recruit",
        }
    }

    /// Export campaign progress for sharing/backup
    pub fn export_progress(&self) -> SaveLoadResult<Vec<u8>> {
        let data = bincode::serialize(&self.player_progress)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        compression::compress(&data)
    }

    /// Import campaign progress from backup
    pub fn import_progress(&mut self, data: &[u8]) -> SaveLoadResult<()> {
        let decompressed = compression::decompress(data)?;

        let progress: CampaignProgress = bincode::deserialize(&decompressed)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        self.player_progress = progress;
        self.save_player_progress()?;

        Ok(())
    }

    // Private implementation methods

    fn load_mission_definitions(&mut self) -> SaveLoadResult<()> {
        // In a real implementation, this would load from game data files
        // For now, create some example missions

        self.add_sample_missions();

        Ok(())
    }

    pub fn iter_missions(&self) -> impl Iterator<Item = &MissionInfo> {
        self.mission_definitions.values()
    }

    fn load_honor_definitions(&mut self) -> SaveLoadResult<()> {
        // Load battle honor definitions from game data
        self.add_sample_honors();

        Ok(())
    }

    fn load_player_progress(&mut self) -> SaveLoadResult<()> {
        let progress_path = self.campaign_directory.join("progress.dat");

        if progress_path.exists() {
            let compressed = std::fs::read(&progress_path)?;
            let data = compression::decompress(&compressed)?;

            self.player_progress = bincode::deserialize(&data)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        }

        // Update rank based on honor points
        self.player_progress.current_rank = self.calculate_rank();

        Ok(())
    }

    fn save_player_progress(&self) -> SaveLoadResult<()> {
        let progress_path = self.campaign_directory.join("progress.dat");

        let data = bincode::serialize(&self.player_progress)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        let compressed = compression::compress(&data)?;
        std::fs::write(&progress_path, compressed)?;

        Ok(())
    }

    fn find_first_mission(&self, campaign_id: CampaignId) -> SaveLoadResult<String> {
        // Find mission with no prerequisites
        for mission in self.mission_definitions.values() {
            if mission.campaign_id == campaign_id && mission.required_missions.is_empty() {
                return Ok(mission.id.clone());
            }
        }

        Err(SaveLoadError::InvalidFormat)
    }

    fn update_global_stats(&mut self, completion: &MissionCompletion) {
        let stats = &mut self.player_progress.global_stats;

        stats.missions_completed += 1;
        if completion.perfect_completion {
            stats.missions_perfect += 1;
        }
        stats.total_score += completion.score as u64;
        stats.total_kills += completion.enemies_destroyed as u64;
        stats.total_built += completion.units_built as u64;
        stats.total_resources += completion.resources_gathered as u64;
        stats.total_play_time += completion.play_duration;
    }

    fn check_unlocks(
        &mut self,
        _mission_id: &str,
        _completion: &MissionCompletion,
    ) -> SaveLoadResult<()> {
        // Check objective rewards for unlocks
        // This would be more complex in a real implementation
        Ok(())
    }

    fn check_battle_honors(
        &mut self,
        mission_id: &str,
        completion: &MissionCompletion,
    ) -> SaveLoadResult<()> {
        for honor in self.honor_definitions.values() {
            if self.player_progress.earned_honors.contains_key(&honor.id) {
                continue; // Already earned
            }

            let earned = match &honor.unlock_condition {
                HonorCondition::CompleteMission(id) => id == mission_id,
                HonorCondition::CompleteWithDifficulty(id, diff) => {
                    id == mission_id && completion.difficulty == *diff
                }
                HonorCondition::WinWithoutLosses(id) => id == mission_id && completion.no_losses,
                HonorCondition::CompleteInTime(id, time_limit) => {
                    id == mission_id && completion.play_duration.as_secs() <= *time_limit as u64
                }
                _ => false, // Other conditions checked elsewhere
            };

            if earned {
                let mut earned_honor = honor.clone();
                earned_honor.earned_date = Some(SystemTime::now());

                self.player_progress.total_honor_points += earned_honor.honor_points;
                self.player_progress
                    .earned_honors
                    .insert(honor.id.clone(), earned_honor);

                log::info!(
                    "Earned battle honor: {} (+{} points)",
                    honor.name,
                    honor.honor_points
                );
            }
        }

        Ok(())
    }

    fn unlock_next_missions(&mut self, completed_mission_id: &str) -> SaveLoadResult<()> {
        // Find missions that require this mission and unlock them
        for mission in self.mission_definitions.values() {
            if mission
                .required_missions
                .contains(&completed_mission_id.to_string())
            {
                log::debug!("Mission {} now available", mission.name);
            }
        }

        Ok(())
    }

    fn add_sample_missions(&mut self) {
        // Campaign.ini residual mission table (Main CampaignManager).
        // Fail-closed: not full INI parse / GameClient CampaignManager parity —
        // host residual seeds retail map identities + chain from Campaign.ini.
        //
        // Campaign USA (Campaign.ini): MD_USA01 … MD_USA05.
        let usa_maps = [
            ("usa_01", 1, "MD_USA01", "Operation Righteous Strike"),
            ("usa_02", 2, "MD_USA02", "USA Mission 02"),
            ("usa_03", 3, "MD_USA03", "USA Mission 03"),
            ("usa_04", 4, "MD_USA04", "USA Mission 04"),
            ("usa_05", 5, "MD_USA05", "USA Mission 05"),
        ];
        for (i, (id, num, map, name)) in usa_maps.iter().enumerate() {
            let required: Vec<String> = if i == 0 {
                Vec::new()
            } else {
                vec![usa_maps[i - 1].0.to_string()]
            };
            let mission = MissionInfo {
                id: id.to_string(),
                campaign_id: CampaignId::USACampaign,
                mission_number: *num,
                name: name.to_string(),
                description: format!("Retail Campaign.ini USA map {map}"),
                map_name: map.to_string(),
                briefing_video: Some(format!("{map}.bik")),
                preview_image: Some(format!("{map}_preview.tga")),
                required_missions: required,
                required_rank: None,
                required_honor_points: None,
                time_limit: if *num == 1 { Some(1800) } else { None },
                starting_resources: Resources {
                    supplies: 10000,
                    power: 0,
                },
                starting_units: if *num == 1 {
                    vec!["PatriotMissile".to_string(), "RangerSquad".to_string()]
                } else {
                    Vec::new()
                },
                tech_restrictions: Vec::new(),
                special_rules: Vec::new(),
                victory_rule: Some("Annihilation".to_string()),
                primary_objectives: if *num == 1 {
                    vec![MissionObjective {
                        id: "destroy_gla_base".to_string(),
                        description: "Destroy the GLA base".to_string(),
                        objective_type: ObjectiveType::Destroy,
                        target: ObjectiveTarget::Building("GLACommandCenter".to_string()),
                        required_count: Some(1),
                        current_count: 0,
                        time_limit: None,
                        reward: Some(ObjectiveReward::HonorPoints(100)),
                    }]
                } else {
                    Vec::new()
                },
                secondary_objectives: Vec::new(),
                bonus_objectives: Vec::new(),
            };
            self.mission_definitions.insert(mission.id.clone(), mission);
        }

        // Campaign.ini CHALLENGE_0 residual map chain (Generals Challenge).
        let challenge_maps = [
            ("challenge_0_01", 1, "GC_ChemGeneral", "General's Challenge — Chem"),
            ("challenge_0_02", 2, "GC_NukeGeneral", "General's Challenge — Nuke"),
            (
                "challenge_0_03",
                3,
                "GC_SuperWeaponsGeneral",
                "General's Challenge — Superweapon",
            ),
            ("challenge_0_04", 4, "GC_TankGeneral", "General's Challenge — Tank"),
            ("challenge_0_05", 5, "GC_Stealth", "General's Challenge — Stealth"),
            ("challenge_0_06", 6, "GC_LaserGeneral", "General's Challenge — Laser"),
            ("challenge_0_07", 7, "GC_ChinaBoss", "General's Challenge — China Boss"),
        ];
        for (i, (id, num, map, name)) in challenge_maps.iter().enumerate() {
            let required: Vec<String> = if i == 0 {
                Vec::new()
            } else {
                vec![challenge_maps[i - 1].0.to_string()]
            };
            let challenge = MissionInfo {
                id: id.to_string(),
                campaign_id: CampaignId::USAGeneral,
                mission_number: *num,
                name: name.to_string(),
                description: format!("Retail Campaign.ini CHALLENGE_0 map {map}"),
                map_name: map.to_string(),
                briefing_video: None,
                preview_image: None,
                required_missions: required,
                required_rank: None,
                required_honor_points: None,
                time_limit: None,
                starting_resources: Resources {
                    supplies: 8000,
                    power: 0,
                },
                starting_units: Vec::new(),
                tech_restrictions: Vec::new(),
                special_rules: Vec::new(),
                victory_rule: Some("Annihilation".to_string()),
                primary_objectives: Vec::new(),
                secondary_objectives: Vec::new(),
                bonus_objectives: Vec::new(),
            };
            self.mission_definitions
                .insert(challenge.id.clone(), challenge);
        }

        // Backward-compatible alias used by golden_campaign / older residual tests.
        if let Some(first) = self.mission_definitions.get("challenge_0_01").cloned() {
            let mut alias = first;
            alias.id = "usa_gen_01".to_string();
            self.mission_definitions
                .insert(alias.id.clone(), alias);
        }
    }

    /// Residual honesty: Campaign.ini-derived USA + Challenge map table present.
    pub fn honesty_campaign_ini_table_ok(&self) -> bool {
        let usa_ok = ["usa_01", "usa_02", "usa_03", "usa_04", "usa_05"]
            .iter()
            .all(|id| {
                self.mission_definitions
                    .get(*id)
                    .map(|m| m.map_name.starts_with("MD_USA"))
                    .unwrap_or(false)
            });
        let challenge_ok = self
            .mission_definitions
            .get("challenge_0_01")
            .map(|m| m.map_name == "GC_ChemGeneral")
            .unwrap_or(false)
            && self
                .mission_definitions
                .get("challenge_0_07")
                .map(|m| m.map_name == "GC_ChinaBoss")
                .unwrap_or(false);
        usa_ok && challenge_ok && self.mission_definitions.len() >= 12
    }

    fn add_sample_honors(&mut self) {
        let honor = BattleHonor {
            id: "first_victory".to_string(),
            name: "First Victory".to_string(),
            description: "Complete your first mission".to_string(),
            icon: "honor_first_victory.tga".to_string(),
            honor_points: 50,
            unlock_condition: HonorCondition::CompleteMission("usa_01".to_string()),
            campaign_id: Some(CampaignId::USACampaign),
            earned_date: None,
        };

        self.honor_definitions.insert(honor.id.clone(), honor);
    }
}

impl CampaignProgress {
    fn new() -> Self {
        Self {
            version: SAVE_FILE_VERSION,
            player_name: "Unknown".to_string(),
            total_play_time: std::time::Duration::default(),
            last_played: SystemTime::now(),
            completed_missions: HashMap::new(),
            current_campaign: None,
            current_mission: None,
            earned_honors: HashMap::new(),
            total_honor_points: 0,
            current_rank: 1,
            global_stats: GlobalCampaignStats::default(),
            unlocked_units: Vec::new(),
            unlocked_buildings: Vec::new(),
            unlocked_upgrades: Vec::new(),
            unlocked_generals: Vec::new(),
            preferred_difficulty: MissionDifficulty::Normal,
            show_cutscenes: true,
            show_briefings: true,
        }
    }
}

/// Data passed when completing a mission
pub struct MissionCompletionData {
    pub play_duration: std::time::Duration,
    pub score: u32,
    pub completed_primary: Vec<String>,
    pub completed_secondary: Vec<String>,
    pub completed_bonus: Vec<String>,
    pub units_built: u32,
    pub units_lost: u32,
    pub enemies_destroyed: u32,
    pub resources_gathered: u32,
    pub buildings_constructed: u32,
    pub special_powers_used: u32,
    pub perfect_completion: bool,
    pub under_time_limit: bool,
    pub no_losses: bool,
    pub stealth_completion: bool,
}

impl Default for CampaignManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global campaign manager instance
lazy_static::lazy_static! {
    pub static ref CAMPAIGN_MANAGER: std::sync::Mutex<CampaignManager> =
        std::sync::Mutex::new(CampaignManager::new());
}

/// Initialize the global campaign system
pub fn init_campaign_system() -> SaveLoadResult<()> {
    let mut manager = CAMPAIGN_MANAGER.lock().unwrap_or_else(|e| e.into_inner());
    manager.init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_campaign_metadata_tracks_started_campaign() {
        let mut manager = CampaignManager::new();
        manager.init().expect("campaign manager init");
        manager
            .start_campaign(CampaignId::USACampaign, "tester")
            .expect("start campaign");

        assert_eq!(manager.current_campaign_id(), Some(CampaignId::USACampaign));
        assert_eq!(manager.current_campaign_side_name(), Some("USA"));
        assert_eq!(manager.current_mission_id(), Some("usa_01"));
        assert_eq!(manager.current_mission_number(), Some(1));
    }

    #[test]
    fn current_campaign_metadata_none_without_active_campaign() {
        let manager = CampaignManager::new();
        assert_eq!(manager.current_campaign_id(), None);
        assert_eq!(manager.current_campaign_side_name(), None);
        assert_eq!(manager.current_mission_id(), None);
        assert_eq!(manager.current_mission_number(), None);
    }

    #[test]
    fn campaign_ini_residual_mission_table() {
        let mut manager = CampaignManager::new();
        manager.init().expect("campaign manager init");
        assert!(
            manager.honesty_campaign_ini_table_ok(),
            "Campaign.ini residual USA + CHALLENGE_0 map table must seed"
        );
        assert_eq!(
            manager
                .get_mission_info("usa_05")
                .map(|m| m.map_name.as_str()),
            Some("MD_USA05")
        );
        assert_eq!(
            manager
                .get_mission_info("challenge_0_07")
                .map(|m| m.map_name.as_str()),
            Some("GC_ChinaBoss")
        );
        // Alias for older residual paths.
        assert_eq!(
            manager
                .get_mission_info("usa_gen_01")
                .map(|m| m.map_name.as_str()),
            Some("GC_ChemGeneral")
        );
    }
}
