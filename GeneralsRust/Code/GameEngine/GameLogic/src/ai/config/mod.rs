//! Authored AI definitions, separated from per-match AI orchestration.
//! C++ AI.cpp TAiData/SkillSet/AISideInfo; mutations require the owning AI borrow.
use super::{MAX_AI_UPGRADES, Real, ScienceType};
use crate::build_list_info::BuildListInfo;
mod ini_adapter;
mod snapshot;
pub(super) use ini_adapter::convert_ai_data;

#[derive(Debug, Clone)]
pub struct SkillSet {
    pub num_skills: i32,
    pub skills: [ScienceType; MAX_AI_UPGRADES],
}

impl Default for SkillSet {
    fn default() -> Self {
        Self {
            num_skills: 0,
            skills: [0; MAX_AI_UPGRADES],
        }
    }
}

// AI side information
#[derive(Debug, Clone)]
pub struct AiSideInfo {
    pub side: String,
    pub easy: i32,
    pub normal: i32,
    pub hard: i32,
    pub skill_set_1: SkillSet,
    pub skill_set_2: SkillSet,
    pub skill_set_3: SkillSet,
    pub skill_set_4: SkillSet,
    pub skill_set_5: SkillSet,
    pub base_defense_structure_1: String,
}

impl Default for AiSideInfo {
    fn default() -> Self {
        Self {
            side: String::new(),
            easy: 0,
            normal: 1,
            hard: 2,
            skill_set_1: SkillSet::default(),
            skill_set_2: SkillSet::default(),
            skill_set_3: SkillSet::default(),
            skill_set_4: SkillSet::default(),
            skill_set_5: SkillSet::default(),
            base_defense_structure_1: String::new(),
        }
    }
}

// AI side build list
#[derive(Debug, Clone)]
pub struct AiSideBuildList {
    pub side: String,
    pub build_list: Option<Box<BuildListInfo>>,
}

impl AiSideBuildList {
    pub fn new(side: String) -> Self {
        Self {
            side,
            build_list: None,
        }
    }

    pub fn add_info(&mut self, info: BuildListInfo) {
        self.build_list = Some(Box::new(info));
    }
}

// AI configuration data
#[derive(Debug, Clone)]
pub struct AiData {
    pub structure_seconds: Real,
    pub team_seconds: Real,
    pub resources_wealthy: i32,
    pub resources_poor: i32,
    pub force_idle_frames_count: u32,
    pub structures_wealthy_mod: Real,
    pub team_wealthy_mod: Real,
    pub structures_poor_mod: Real,
    pub team_poor_mod: Real,
    pub team_resources_to_build: Real,
    pub guard_inner_modifier_ai: Real,
    pub guard_outer_modifier_ai: Real,
    pub guard_inner_modifier_human: Real,
    pub guard_outer_modifier_human: Real,
    pub guard_chase_unit_frames: u32,
    pub guard_enemy_scan_rate: u32,
    pub guard_enemy_return_scan_rate: u32,
    pub wall_height: Real,
    pub alert_range_modifier: Real,
    pub aggressive_range_modifier: Real,
    pub attack_priority_distance_modifier: Real,
    pub skirmish_group_fudge_value: Real,
    pub max_recruit_distance: Real,
    pub skirmish_base_defense_extra_distance: Real,
    pub repulsed_distance: Real,
    pub enable_repulsors: bool,
    pub force_skirmish_ai: bool,
    pub rotate_skirmish_bases: bool,
    pub attack_uses_line_of_sight: bool,
    pub attack_ignore_insignificant_buildings: bool,
    pub min_infantry_for_group: i32,
    pub min_vehicles_for_group: i32,
    pub min_distance_for_group: Real,
    pub distance_requires_group: Real,
    pub min_clump_density: Real,
    pub infantry_pathfind_diameter: i32,
    pub vehicle_pathfind_diameter: i32,
    pub rebuild_delay_seconds: i32,
    pub supply_center_safe_radius: Real,
    pub ai_dozer_bored_radius_modifier: Real,
    pub ai_crushes_infantry: bool,
    pub max_retaliate_distance: Real,
    pub retaliate_friends_radius: Real,
    pub side_info: Vec<AiSideInfo>,
    pub side_build_lists: Vec<AiSideBuildList>,
}

impl Default for AiData {
    fn default() -> Self {
        Self {
            structure_seconds: 0.0,
            team_seconds: 0.0,
            resources_wealthy: 0,
            resources_poor: 0,
            force_idle_frames_count: 1,
            structures_wealthy_mod: 0.0,
            team_wealthy_mod: 0.0,
            structures_poor_mod: 0.0,
            team_poor_mod: 0.0,
            team_resources_to_build: 0.0,
            guard_inner_modifier_ai: 0.0,
            guard_outer_modifier_ai: 0.0,
            guard_inner_modifier_human: 0.0,
            guard_outer_modifier_human: 0.0,
            guard_chase_unit_frames: 0,
            guard_enemy_scan_rate: 15,        // LOGICFRAMES_PER_SECOND/2
            guard_enemy_return_scan_rate: 30, // LOGICFRAMES_PER_SECOND
            wall_height: 0.0,
            alert_range_modifier: 0.0,
            aggressive_range_modifier: 0.0,
            attack_priority_distance_modifier: 0.0,
            skirmish_group_fudge_value: 0.0,
            max_recruit_distance: 0.0,
            skirmish_base_defense_extra_distance: 0.0,
            repulsed_distance: 0.0,
            enable_repulsors: false,
            force_skirmish_ai: false,
            rotate_skirmish_bases: false,
            attack_uses_line_of_sight: true,
            attack_ignore_insignificant_buildings: false,
            min_infantry_for_group: 3,
            min_vehicles_for_group: 4,
            min_distance_for_group: 100.0,
            distance_requires_group: 0.0,
            min_clump_density: 0.5,
            infantry_pathfind_diameter: 6,
            vehicle_pathfind_diameter: 6,
            rebuild_delay_seconds: 10,
            supply_center_safe_radius: 250.0,
            ai_dozer_bored_radius_modifier: 2.0,
            ai_crushes_infantry: true,
            max_retaliate_distance: 210.0,
            retaliate_friends_radius: 120.0,
            side_info: Vec::new(),
            side_build_lists: Vec::new(),
        }
    }
}

impl AiData {
    pub fn add_side_info(&mut self, info: AiSideInfo) {
        self.side_info.push(info);
    }

    pub fn add_faction_build_list(&mut self, build_list: AiSideBuildList) {
        // Check if we already have a build list for this side
        for existing in &mut self.side_build_lists {
            if existing.side == build_list.side {
                existing.build_list = build_list.build_list;
                return;
            }
        }
        self.side_build_lists.push(build_list);
    }
}

#[cfg(test)]
mod tests;
