//! FILE: ini_campaign.rs
//! Ported from: CampaignManager.cpp (INI parsing section)
//! Original Author: Chris Huybregts
//! Rust port: 2025
//!
//! Purpose: INI parsing for Campaign and Mission definitions

use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use crate::common::ini::ini_misc_audio::AudioEventRTS;

// Constants matching C++ definitions
pub const MAX_OBJECTIVE_LINES: usize = 5;
pub const MAX_DISPLAYED_UNITS: usize = 3;

/// Mission data structure matching C++ Mission class
#[derive(Debug, Clone, Default)]
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

/// Campaign data structure matching C++ Campaign class
#[derive(Debug, Clone, Default)]
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

    /// Create or replace a mission with the given name
    /// Matches C++ Campaign::newMission behavior
    pub fn new_mission(&mut self, name: String) -> &mut Mission {
        let name_lower = name.to_lowercase();

        // Remove existing mission with same name (C++ behavior)
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
        let name = match current {
            Some(current_mission) => &current_mission.next_mission,
            None => &self.first_mission,
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

// ============================================================================
// Mission field parsing functions
// ============================================================================

fn parse_mission_map(_ini: &mut INI, mission: &mut Mission, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.map_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_mission_next(_ini: &mut INI, mission: &mut Mission, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.next_mission = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_mission_intro_movie(
    _ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.movie_label = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_mission_objective_line(
    index: usize,
    _ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.mission_objectives_label[index] = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_mission_objective_line0(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_objective_line(0, ini, mission, tokens)
}

fn parse_mission_objective_line1(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_objective_line(1, ini, mission, tokens)
}

fn parse_mission_objective_line2(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_objective_line(2, ini, mission, tokens)
}

fn parse_mission_objective_line3(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_objective_line(3, ini, mission, tokens)
}

fn parse_mission_objective_line4(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_objective_line(4, ini, mission, tokens)
}

fn parse_mission_briefing_voice(
    _ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.briefing_voice = AudioEventRTS::from(INI::parse_ascii_string(token)?);
    Ok(())
}

fn parse_mission_unit_name(
    index: usize,
    _ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.unit_names[index] = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_mission_unit_names0(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_unit_name(0, ini, mission, tokens)
}

fn parse_mission_unit_names1(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_unit_name(1, ini, mission, tokens)
}

fn parse_mission_unit_names2(
    ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    parse_mission_unit_name(2, ini, mission, tokens)
}

fn parse_mission_general_name(
    _ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.general_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_mission_location_name(
    _ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.location_name_label = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_mission_voice_length(
    _ini: &mut INI,
    mission: &mut Mission,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    mission.voice_length = INI::parse_int(token)?;
    Ok(())
}

/// Mission field parse table matching C++ CampaignManager::parseMissionPart
const MISSION_FIELD_PARSE_TABLE: &[FieldParse<Mission>] = &[
    FieldParse {
        token: "Map",
        parse: parse_mission_map,
    },
    FieldParse {
        token: "NextMission",
        parse: parse_mission_next,
    },
    FieldParse {
        token: "IntroMovie",
        parse: parse_mission_intro_movie,
    },
    FieldParse {
        token: "ObjectiveLine0",
        parse: parse_mission_objective_line0,
    },
    FieldParse {
        token: "ObjectiveLine1",
        parse: parse_mission_objective_line1,
    },
    FieldParse {
        token: "ObjectiveLine2",
        parse: parse_mission_objective_line2,
    },
    FieldParse {
        token: "ObjectiveLine3",
        parse: parse_mission_objective_line3,
    },
    FieldParse {
        token: "ObjectiveLine4",
        parse: parse_mission_objective_line4,
    },
    FieldParse {
        token: "BriefingVoice",
        parse: parse_mission_briefing_voice,
    },
    FieldParse {
        token: "UnitNames0",
        parse: parse_mission_unit_names0,
    },
    FieldParse {
        token: "UnitNames1",
        parse: parse_mission_unit_names1,
    },
    FieldParse {
        token: "UnitNames2",
        parse: parse_mission_unit_names2,
    },
    FieldParse {
        token: "GeneralName",
        parse: parse_mission_general_name,
    },
    FieldParse {
        token: "LocationNameLabel",
        parse: parse_mission_location_name,
    },
    FieldParse {
        token: "VoiceLength",
        parse: parse_mission_voice_length,
    },
];

// ============================================================================
// Campaign field parsing functions
// ============================================================================

/// Parse a nested Mission block within a Campaign
/// Matches C++ CampaignManager::parseMissionPart
fn parse_mission_block(ini: &mut INI, campaign: &mut Campaign, _tokens: &[&str]) -> INIResult<()> {
    // Read the mission name from the same line (e.g., "Mission Mission01")
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

    // Create a new mission in the campaign
    let mission = campaign.new_mission(name);

    // Parse the mission fields until End
    ini.init_from_ini_with_fields(mission, MISSION_FIELD_PARSE_TABLE)?;

    Ok(())
}

fn parse_campaign_first_mission(
    _ini: &mut INI,
    campaign: &mut Campaign,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    campaign.first_mission = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_campaign_name_label(
    _ini: &mut INI,
    campaign: &mut Campaign,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    campaign.campaign_name_label = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_campaign_final_movie(
    _ini: &mut INI,
    campaign: &mut Campaign,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    campaign.final_movie_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_campaign_is_challenge(
    _ini: &mut INI,
    campaign: &mut Campaign,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    campaign.is_challenge_campaign = INI::parse_bool(token)?;
    Ok(())
}

fn parse_campaign_player_faction(
    _ini: &mut INI,
    campaign: &mut Campaign,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    campaign.player_faction_name = INI::parse_ascii_string(token)?;
    Ok(())
}

/// Campaign field parse table matching C++ CampaignManager::m_campaignFieldParseTable
const CAMPAIGN_FIELD_PARSE_TABLE: &[FieldParse<Campaign>] = &[
    FieldParse {
        token: "Mission",
        parse: parse_mission_block,
    },
    FieldParse {
        token: "FirstMission",
        parse: parse_campaign_first_mission,
    },
    FieldParse {
        token: "CampaignNameLabel",
        parse: parse_campaign_name_label,
    },
    FieldParse {
        token: "FinalVictoryMovie",
        parse: parse_campaign_final_movie,
    },
    FieldParse {
        token: "IsChallengeCampaign",
        parse: parse_campaign_is_challenge,
    },
    FieldParse {
        token: "PlayerFaction",
        parse: parse_campaign_player_faction,
    },
];

// ============================================================================
// Campaign Store
// ============================================================================

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Store for all campaign definitions
pub struct CampaignStore {
    campaigns: HashMap<String, Campaign>,
    campaign_order: Vec<String>,
}

impl CampaignStore {
    pub fn new() -> Self {
        Self {
            campaigns: HashMap::new(),
            campaign_order: Vec::new(),
        }
    }

    /// Add or replace a campaign (matches C++ CampaignManager::newCampaign behavior)
    pub fn add_campaign(&mut self, campaign: Campaign) {
        let name = campaign.name.to_lowercase();
        self.campaign_order
            .retain(|existing| existing.as_str() != name.as_str());
        self.campaign_order.push(name.clone());
        self.campaigns.insert(name, campaign);
    }

    /// Get a campaign by name
    pub fn get_campaign(&self, name: &str) -> Option<&Campaign> {
        self.campaigns.get(&name.to_lowercase())
    }

    /// Get a mutable campaign by name
    pub fn get_campaign_mut(&mut self, name: &str) -> Option<&mut Campaign> {
        self.campaigns.get_mut(&name.to_lowercase())
    }

    /// Get all campaign names
    pub fn campaign_names(&self) -> Vec<&String> {
        self.campaign_order
            .iter()
            .filter(|name| self.campaigns.contains_key(*name))
            .collect()
    }

    /// Clear all campaigns
    pub fn clear(&mut self) {
        self.campaigns.clear();
        self.campaign_order.clear();
    }

    /// Get number of campaigns
    pub fn len(&self) -> usize {
        self.campaigns.len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.campaigns.is_empty()
    }
}

impl Default for CampaignStore {
    fn default() -> Self {
        Self::new()
    }
}

static CAMPAIGN_STORE: OnceCell<RwLock<CampaignStore>> = OnceCell::new();

pub fn get_campaign_store() -> RwLockReadGuard<'static, CampaignStore> {
    CAMPAIGN_STORE
        .get_or_init(|| RwLock::new(CampaignStore::new()))
        .read()
        .unwrap()
}

pub fn get_campaign_store_mut() -> RwLockWriteGuard<'static, CampaignStore> {
    CAMPAIGN_STORE
        .get_or_init(|| RwLock::new(CampaignStore::new()))
        .write()
        .unwrap()
}

pub fn init_campaign_store() {
    // Initialize the store by getting a reference to it
    let _unused = get_campaign_store();
}

// ============================================================================
// INI Block Parser
// ============================================================================

/// Parse a Campaign block from INI
/// Matches C++ INI::parseCampaignDefinition
pub fn parse_campaign_definition(ini: &mut INI) -> INIResult<()> {
    // Read the campaign name from the block header line
    // Format: "Campaign CampaignName"
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    // Create a new campaign
    let mut campaign = Campaign::with_name(name);

    // Parse all campaign fields (including nested Mission blocks)
    ini.init_from_ini_with_fields(&mut campaign, CAMPAIGN_FIELD_PARSE_TABLE)?;

    // Add the campaign to the store
    let mut store = get_campaign_store_mut();
    store.add_campaign(campaign);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_campaign_creation() {
        let campaign = Campaign::with_name("TestCampaign".to_string());
        assert_eq!(campaign.name, "testcampaign");
        assert!(campaign.missions.is_empty());
        assert!(!campaign.is_challenge_campaign);
    }

    #[test]
    fn test_mission_creation() {
        let mission = Mission::with_name("Mission1".to_string());
        assert_eq!(mission.name, "mission1");
        assert!(mission.map_name.is_empty());
    }

    #[test]
    fn test_campaign_new_mission() {
        let mut campaign = Campaign::new();
        let mission = campaign.new_mission("Mission1".to_string());

        assert_eq!(mission.name, "mission1");
        assert_eq!(campaign.missions.len(), 1);
    }

    #[test]
    fn test_campaign_new_mission_replaces() {
        let mut campaign = Campaign::new();

        campaign.new_mission("Mission1".to_string());
        assert_eq!(campaign.missions.len(), 1);

        // Adding same mission again should replace
        campaign.new_mission("Mission1".to_string());
        assert_eq!(campaign.missions.len(), 1);
    }

    #[test]
    fn test_campaign_get_mission() {
        let mut campaign = Campaign::new();
        campaign.new_mission("Mission1".to_string());

        let found = campaign.get_mission("Mission1");
        assert!(found.is_some());

        let not_found = campaign.get_mission("NonExistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_campaign_get_next_mission() {
        let mut campaign = Campaign::new();
        campaign.first_mission = "mission1".to_string();

        let m1 = campaign.new_mission("Mission1".to_string());
        m1.next_mission = "mission2".to_string();

        let m2 = campaign.new_mission("Mission2".to_string());

        // Get first mission when no current
        let first = campaign.get_next_mission(None);
        assert!(first.is_some());
        assert_eq!(first.unwrap().name, "mission1");

        // Get next mission
        let m1_ref = campaign.get_mission("Mission1").unwrap();
        let next = campaign.get_next_mission(Some(m1_ref));
        assert!(next.is_some());
        assert_eq!(next.unwrap().name, "mission2");

        // End of campaign
        let m2_ref = campaign.get_mission("Mission2").unwrap();
        let end = campaign.get_next_mission(Some(m2_ref));
        assert!(end.is_none());
    }

    #[test]
    fn test_campaign_store() {
        let mut store = CampaignStore::new();

        let mut campaign = Campaign::with_name("TestCampaign".to_string());
        campaign.first_mission = "mission1".to_string();
        campaign.new_mission("Mission1".to_string());

        store.add_campaign(campaign);

        assert_eq!(store.len(), 1);
        assert!(store.get_campaign("TestCampaign").is_some());
        assert!(store.get_campaign("testcampaign").is_some()); // case insensitive
    }

    #[test]
    fn test_campaign_store_preserves_cpp_list_order() {
        let mut store = CampaignStore::new();

        store.add_campaign(Campaign::with_name("China".to_string()));
        store.add_campaign(Campaign::with_name("USA".to_string()));
        store.add_campaign(Campaign::with_name("GLA".to_string()));

        assert_eq!(
            store
                .campaign_names()
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["china", "usa", "gla"]
        );
    }

    #[test]
    fn test_campaign_store_duplicate_moves_to_end_like_cpp_new_campaign() {
        let mut store = CampaignStore::new();

        store.add_campaign(Campaign::with_name("China".to_string()));
        store.add_campaign(Campaign::with_name("USA".to_string()));
        let mut replacement = Campaign::with_name("China".to_string());
        replacement.campaign_name_label = "GUI:ChinaReplacement".to_string();
        store.add_campaign(replacement);

        assert_eq!(store.len(), 2);
        assert_eq!(
            store
                .campaign_names()
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["usa", "china"]
        );
        assert_eq!(
            store
                .get_campaign("china")
                .expect("campaign exists")
                .campaign_name_label,
            "GUI:ChinaReplacement"
        );
    }

    #[test]
    fn test_campaign_store_clear_removes_order_entries() {
        let mut store = CampaignStore::new();

        store.add_campaign(Campaign::with_name("China".to_string()));
        store.add_campaign(Campaign::with_name("USA".to_string()));
        store.clear();

        assert!(store.is_empty());
        assert!(store.campaign_names().is_empty());
    }
}
