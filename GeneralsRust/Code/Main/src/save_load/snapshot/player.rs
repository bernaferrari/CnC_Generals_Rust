//! Player and team snapshot types and Xfer residual.

use super::xfer_helpers::{xfer_hashmap_default, xfer_vec_default};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Player state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: u32,
    pub name: String,
    pub team: Team,
    pub is_human: bool,
    pub is_active: bool,

    pub resources: Resources,
    pub population: PopulationInfo,
    pub tech_tree: TechTreeSnapshot,
    pub upgrades: Vec<String>,

    pub build_queue: Vec<String>,
    pub research_queue: Vec<String>,

    pub statistics: PlayerStatisticsSnapshot,
}

/// Stable, exact identity of an offline player selection. `template_index` is
/// signed because the C++ PlayerTemplate store is indexed by an `int`.
/// Restoration must validate the name/index pair and never resolve Random.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerTemplateBindingSnapshot {
    pub player_id: u32,
    pub template_name: String,
    pub template_index: i32,
}

/// C++ `Player::xfer` rank/skill/science purchase points
/// (`Player.cpp` 4268-4275: `m_rankLevel` / `m_skillPoints` / `m_sciencePurchasePoints`).
/// Kept as a world tail so historical nested `PlayerSnapshot` records stay aligned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerRankSnapshot {
    pub player_id: u32,
    pub rank_level: u32,
    pub skill_points: i32,
    pub science_purchase_points: i32,
}

/// C++ `Energy::xfer` v3 (`Energy.cpp:258-262`) `m_powerSabotagedTillFrame`.
/// World tail so nested `PlayerSnapshot` records stay aligned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEnergySnapshot {
    pub player_id: u32,
    pub power_sabotaged_till_frame: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationInfo {
    pub current: u32,
    pub maximum: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechTreeSnapshot {
    pub unlocked_units: Vec<String>,
    pub unlocked_buildings: Vec<String>,
    pub unlocked_upgrades: Vec<String>,
    pub research_progress: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStatisticsSnapshot {
    pub units_built: u32,
    pub units_lost: u32,
    pub buildings_built: u32,
    pub buildings_lost: u32,
    pub damage_dealt: f32,
    pub damage_received: f32,
    pub resources_gathered: u32,
    pub experience_gained: f32,
}

/// Team snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSnapshot {
    pub team: Team,
    pub players: Vec<u32>,
    pub allied_teams: Vec<Team>,
    pub is_defeated: bool,
    pub shared_vision: bool,
    pub shared_control: bool,
}

impl XferData for PlayerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerSnapshot")?;

        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;

        xfer.xfer_marker_label("Name")?;
        self.name.xfer(xfer)?;

        xfer.xfer_marker_label("Team")?;
        self.team.xfer(xfer)?;

        xfer.xfer_marker_label("IsHuman")?;
        xfer.xfer_bool(&mut self.is_human)?;

        xfer.xfer_marker_label("IsActive")?;
        xfer.xfer_bool(&mut self.is_active)?;

        xfer.xfer_marker_label("Resources")?;
        self.resources.xfer(xfer)?;

        xfer.xfer_marker_label("Population")?;
        self.population.xfer(xfer)?;

        xfer.xfer_marker_label("TechTree")?;
        self.tech_tree.xfer(xfer)?;

        xfer.xfer_marker_label("Upgrades")?;
        xfer.xfer_vec_string(&mut self.upgrades)?;

        xfer.xfer_marker_label("BuildQueue")?;
        xfer.xfer_vec_string(&mut self.build_queue)?;

        xfer.xfer_marker_label("ResearchQueue")?;
        xfer.xfer_vec_string(&mut self.research_queue)?;

        xfer.xfer_marker_label("Statistics")?;
        self.statistics.xfer(xfer)?;

        Ok(())
    }
}

impl XferData for PlayerTemplateBindingSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerTemplateBindingSnapshot")?;
        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;
        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;
        xfer.xfer_marker_label("TemplateIndex")?;
        xfer.xfer_i32(&mut self.template_index)?;
        Ok(())
    }
}

impl XferData for PlayerRankSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerRankSnapshot")?;
        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;
        xfer.xfer_marker_label("RankLevel")?;
        xfer.xfer_u32(&mut self.rank_level)?;
        xfer.xfer_marker_label("SkillPoints")?;
        xfer.xfer_i32(&mut self.skill_points)?;
        xfer.xfer_marker_label("SciencePurchasePoints")?;
        xfer.xfer_i32(&mut self.science_purchase_points)?;
        Ok(())
    }
}

impl XferData for PlayerEnergySnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerEnergySnapshot")?;
        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;
        xfer.xfer_marker_label("PowerSabotagedTillFrame")?;
        xfer.xfer_u32(&mut self.power_sabotaged_till_frame)?;
        Ok(())
    }
}

impl XferData for PopulationInfo {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PopulationInfo")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_u32(&mut self.current)?;
        xfer.xfer_marker_label("Maximum")?;
        xfer.xfer_u32(&mut self.maximum)?;
        Ok(())
    }
}

impl XferData for TechTreeSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TechTreeSnapshot")?;
        xfer.xfer_marker_label("UnlockedUnits")?;
        xfer.xfer_vec_string(&mut self.unlocked_units)?;
        xfer.xfer_marker_label("UnlockedBuildings")?;
        xfer.xfer_vec_string(&mut self.unlocked_buildings)?;
        xfer.xfer_marker_label("UnlockedUpgrades")?;
        xfer.xfer_vec_string(&mut self.unlocked_upgrades)?;
        xfer.xfer_marker_label("ResearchProgress")?;
        xfer_hashmap_default(xfer, &mut self.research_progress, String::new(), 0.0f32)?;
        Ok(())
    }
}

impl XferData for PlayerStatisticsSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerStatisticsSnapshot")?;
        xfer.xfer_marker_label("UnitsBuilt")?;
        xfer.xfer_u32(&mut self.units_built)?;
        xfer.xfer_marker_label("UnitsLost")?;
        xfer.xfer_u32(&mut self.units_lost)?;
        xfer.xfer_marker_label("BuildingsBuilt")?;
        xfer.xfer_u32(&mut self.buildings_built)?;
        xfer.xfer_marker_label("BuildingsLost")?;
        xfer.xfer_u32(&mut self.buildings_lost)?;
        xfer.xfer_marker_label("DamageDealt")?;
        xfer.xfer_f32(&mut self.damage_dealt)?;
        xfer.xfer_marker_label("DamageReceived")?;
        xfer.xfer_f32(&mut self.damage_received)?;
        xfer.xfer_marker_label("ResourcesGathered")?;
        xfer.xfer_u32(&mut self.resources_gathered)?;
        xfer.xfer_marker_label("ExperienceGained")?;
        xfer.xfer_f32(&mut self.experience_gained)?;
        Ok(())
    }
}

impl XferData for TeamSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TeamSnapshot")?;
        xfer.xfer_marker_label("Team")?;
        self.team.xfer(xfer)?;
        xfer.xfer_marker_label("Players")?;
        xfer.xfer_vec_u32(&mut self.players)?;
        xfer.xfer_marker_label("AlliedTeams")?;
        xfer_vec_default(xfer, &mut self.allied_teams, Team::Neutral)?;
        xfer.xfer_marker_label("IsDefeated")?;
        xfer.xfer_bool(&mut self.is_defeated)?;
        xfer.xfer_marker_label("SharedVision")?;
        xfer.xfer_bool(&mut self.shared_vision)?;
        xfer.xfer_marker_label("SharedControl")?;
        xfer.xfer_bool(&mut self.shared_control)?;
        Ok(())
    }
}
