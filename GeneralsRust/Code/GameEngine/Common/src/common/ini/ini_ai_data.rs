//! INI AI Data parsing module
//! Author: John Ahlquist, March 2002
//! Desc: Parsing AIData INI entries

use once_cell::sync::OnceCell;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::ini::{FieldParse, INIError, INILoadType, INIResult, INI};
use crate::common::rts::{get_science_store, ScienceType, SCIENCE_INVALID};

pub const MAX_AI_UPGRADES: usize = 20;

#[derive(Debug, Clone)]
pub struct SkillSet {
    pub num_skills: i32,
    pub skills: [ScienceType; MAX_AI_UPGRADES],
}

impl Default for SkillSet {
    fn default() -> Self {
        Self {
            num_skills: 0,
            skills: [SCIENCE_INVALID; MAX_AI_UPGRADES],
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct BuildListEntry {
    pub building_name: String,
    pub template_name: String,
    pub location: (f32, f32),
    pub rebuilds: i32,
    pub angle_radians: f32,
    pub initially_built: bool,
    pub rally_point_offset: (f32, f32),
    pub automatically_build: bool,
}

impl Default for BuildListEntry {
    fn default() -> Self {
        Self {
            building_name: String::new(),
            template_name: String::new(),
            location: (0.0, 0.0),
            rebuilds: 0,
            angle_radians: 0.0,
            initially_built: false,
            rally_point_offset: (0.0, 0.0),
            automatically_build: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiSideBuildList {
    pub side: String,
    pub entries: Vec<BuildListEntry>,
}

impl AiSideBuildList {
    pub fn new(side: String) -> Self {
        Self {
            side,
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: BuildListEntry) {
        self.entries.push(entry);
    }
}

/// AI Data configuration structure (matches C++ TAiData fields)
#[derive(Debug, Clone)]
pub struct AIData {
    pub structure_seconds: f32,
    pub team_seconds: f32,
    pub resources_wealthy: i32,
    pub resources_poor: i32,
    pub force_idle_frames_count: u32,
    pub structures_wealthy_mod: f32,
    pub team_wealthy_mod: f32,
    pub structures_poor_mod: f32,
    pub team_poor_mod: f32,
    pub team_resources_to_build: f32,
    pub guard_inner_modifier_ai: f32,
    pub guard_outer_modifier_ai: f32,
    pub guard_inner_modifier_human: f32,
    pub guard_outer_modifier_human: f32,
    pub guard_chase_unit_frames: u32,
    pub guard_enemy_scan_rate: u32,
    pub guard_enemy_return_scan_rate: u32,
    pub wall_height: f32,
    pub alert_range_modifier: f32,
    pub aggressive_range_modifier: f32,
    pub attack_priority_distance_modifier: f32,
    pub skirmish_group_fudge_value: f32,
    pub max_recruit_distance: f32,
    pub skirmish_base_defense_extra_distance: f32,
    pub repulsed_distance: f32,
    pub enable_repulsors: bool,
    pub force_skirmish_ai: bool,
    pub rotate_skirmish_bases: bool,
    pub attack_uses_line_of_sight: bool,
    pub attack_ignore_insignificant_buildings: bool,
    pub min_infantry_for_group: i32,
    pub min_vehicles_for_group: i32,
    pub min_distance_for_group: f32,
    pub distance_requires_group: f32,
    pub min_clump_density: f32,
    pub infantry_pathfind_diameter: i32,
    pub vehicle_pathfind_diameter: i32,
    pub rebuild_delay_seconds: i32,
    pub supply_center_safe_radius: f32,
    pub ai_dozer_bored_radius_modifier: f32,
    pub ai_crushes_infantry: bool,
    pub max_retaliate_distance: f32,
    pub retaliate_friends_radius: f32,
    pub side_info: Vec<AiSideInfo>,
    pub side_build_lists: Vec<AiSideBuildList>,
}

impl Default for AIData {
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
            guard_enemy_scan_rate: 30,
            guard_enemy_return_scan_rate: 60,
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

impl AIData {
    pub fn add_side_info(&mut self, info: AiSideInfo) {
        self.side_info.push(info);
    }

    pub fn add_faction_build_list(&mut self, build_list: AiSideBuildList) {
        for existing in &mut self.side_build_lists {
            if existing.side == build_list.side {
                existing.entries = build_list.entries;
                return;
            }
        }
        self.side_build_lists.push(build_list);
    }
}

#[derive(Debug, Default)]
pub struct AIDataStore {
    entries: Vec<AIData>,
}

impl AIDataStore {
    pub fn get_active(&self) -> Option<&AIData> {
        self.entries.last()
    }

    pub fn get_active_mut(&mut self) -> Option<&mut AIData> {
        self.entries.last_mut()
    }

    pub fn ensure_base(&mut self) {
        if self.entries.is_empty() {
            self.entries.push(AIData::default());
        }
    }

    pub fn push_override(&mut self) {
        if let Some(active) = self.entries.last().cloned() {
            self.entries.push(active);
        } else {
            self.entries.push(AIData::default());
        }
    }
}

static AI_DATA_STORE: OnceCell<RwLock<AIDataStore>> = OnceCell::new();

pub fn get_ai_data_store() -> RwLockReadGuard<'static, AIDataStore> {
    AI_DATA_STORE
        .get_or_init(|| RwLock::new(AIDataStore::default()))
        .read()
        .expect("AI data store read lock")
}

pub fn get_ai_data_store_mut() -> RwLockWriteGuard<'static, AIDataStore> {
    AI_DATA_STORE
        .get_or_init(|| RwLock::new(AIDataStore::default()))
        .write()
        .expect("AI data store write lock")
}

fn parse_real_field(tokens: &[&str]) -> INIResult<f32> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_int_field(tokens: &[&str]) -> INIResult<i32> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_int(token)
}

fn parse_bool_field(tokens: &[&str]) -> INIResult<bool> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_bool(token)
}

fn parse_duration_field(tokens: &[&str]) -> INIResult<u32> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_ai_data_structure_seconds(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.structure_seconds = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_team_seconds(_ini: &mut INI, data: &mut AIData, tokens: &[&str]) -> INIResult<()> {
    data.team_seconds = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_resources_wealthy(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.resources_wealthy = parse_int_field(tokens)?;
    Ok(())
}

fn parse_ai_data_resources_poor(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.resources_poor = parse_int_field(tokens)?;
    Ok(())
}

fn parse_ai_data_force_idle_msec(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.force_idle_frames_count = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_ai_data_structures_wealthy_mod(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.structures_wealthy_mod = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_team_wealthy_mod(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.team_wealthy_mod = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_structures_poor_mod(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.structures_poor_mod = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_team_poor_mod(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.team_poor_mod = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_team_resources_to_build(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.team_resources_to_build = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_guard_inner_modifier_ai(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.guard_inner_modifier_ai = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_guard_outer_modifier_ai(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.guard_outer_modifier_ai = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_guard_inner_modifier_human(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.guard_inner_modifier_human = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_guard_outer_modifier_human(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.guard_outer_modifier_human = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_guard_chase_unit_frames(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.guard_chase_unit_frames = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_ai_data_guard_enemy_scan_rate(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.guard_enemy_scan_rate = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_ai_data_guard_enemy_return_scan_rate(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.guard_enemy_return_scan_rate = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_ai_data_skirmish_group_fudge_value(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.skirmish_group_fudge_value = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_repulsed_distance(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.repulsed_distance = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_enable_repulsors(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.enable_repulsors = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_ai_data_alert_range_modifier(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.alert_range_modifier = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_aggressive_range_modifier(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.aggressive_range_modifier = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_attack_priority_distance_modifier(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.attack_priority_distance_modifier = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_max_recruit_radius(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.max_recruit_distance = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_skirmish_base_defense_extra_distance(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.skirmish_base_defense_extra_distance = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_wall_height(_ini: &mut INI, data: &mut AIData, tokens: &[&str]) -> INIResult<()> {
    data.wall_height = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_force_skirmish_ai(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.force_skirmish_ai = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_ai_data_rotate_skirmish_bases(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.rotate_skirmish_bases = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_ai_data_attack_uses_line_of_sight(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.attack_uses_line_of_sight = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_ai_data_attack_ignore_insignificant_buildings(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.attack_ignore_insignificant_buildings = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_ai_data_min_infantry_for_group(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.min_infantry_for_group = parse_int_field(tokens)?;
    Ok(())
}

fn parse_ai_data_min_vehicles_for_group(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.min_vehicles_for_group = parse_int_field(tokens)?;
    Ok(())
}

fn parse_ai_data_min_distance_for_group(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.min_distance_for_group = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_distance_requires_group(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.distance_requires_group = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_min_clump_density(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.min_clump_density = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_infantry_pathfind_diameter(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.infantry_pathfind_diameter = parse_int_field(tokens)?;
    Ok(())
}

fn parse_ai_data_vehicle_pathfind_diameter(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.vehicle_pathfind_diameter = parse_int_field(tokens)?;
    Ok(())
}

fn parse_ai_data_rebuild_delay_seconds(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.rebuild_delay_seconds = parse_int_field(tokens)?;
    Ok(())
}

fn parse_ai_data_supply_center_safe_radius(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.supply_center_safe_radius = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_ai_dozer_bored_radius_modifier(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.ai_dozer_bored_radius_modifier = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_ai_crushes_infantry(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.ai_crushes_infantry = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_ai_data_max_retaliation_distance(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.max_retaliate_distance = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_retaliation_friends_radius(
    _ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    data.retaliate_friends_radius = parse_real_field(tokens)?;
    Ok(())
}

fn parse_ai_data_side_info(ini: &mut INI, data: &mut AIData, tokens: &[&str]) -> INIResult<()> {
    let side = tokens.first().ok_or(INIError::InvalidData)?.to_string();
    let mut info = AiSideInfo::default();
    info.side = side;
    ini.init_from_ini_with_fields(&mut info, AI_SIDE_INFO_FIELDS)?;
    data.add_side_info(info);
    Ok(())
}

fn parse_ai_data_skirmish_build_list(
    ini: &mut INI,
    data: &mut AIData,
    tokens: &[&str],
) -> INIResult<()> {
    let side = tokens.first().ok_or(INIError::InvalidData)?.to_string();
    let mut build_list = AiSideBuildList::new(side);
    ini.init_from_ini_with_fields(&mut build_list, AI_BUILD_LIST_FIELDS)?;
    data.add_faction_build_list(build_list);
    Ok(())
}

fn parse_skill_set(ini: &mut INI, skillset: &mut SkillSet, _tokens: &[&str]) -> INIResult<()> {
    *skillset = SkillSet::default();
    ini.init_from_ini_with_fields(skillset, AI_SKILL_SET_FIELDS)?;
    Ok(())
}

fn parse_science_skill(_ini: &mut INI, skillset: &mut SkillSet, tokens: &[&str]) -> INIResult<()> {
    let name = tokens.first().ok_or(INIError::InvalidData)?;
    let Some(store) = get_science_store() else {
        return Err(INIError::InvalidData);
    };
    let science = store.get_science_from_internal_name(name);
    if science == SCIENCE_INVALID {
        return Err(INIError::InvalidData);
    }
    if store.get_science_purchase_cost(science) == 0 {
        return Ok(());
    }
    if skillset.num_skills as usize >= MAX_AI_UPGRADES {
        return Err(INIError::InvalidData);
    }
    skillset.skills[skillset.num_skills as usize] = science;
    skillset.num_skills += 1;
    Ok(())
}

fn parse_build_list_structure(
    ini: &mut INI,
    build_list: &mut AiSideBuildList,
    tokens: &[&str],
) -> INIResult<()> {
    let template_name = tokens.first().ok_or(INIError::InvalidData)?.to_string();
    let mut entry = BuildListEntry::default();
    entry.template_name = template_name;
    ini.init_from_ini_with_fields(&mut entry, BUILD_LIST_STRUCTURE_FIELDS)?;
    build_list.add_entry(entry);
    Ok(())
}

fn parse_build_list_name(
    _ini: &mut INI,
    entry: &mut BuildListEntry,
    tokens: &[&str],
) -> INIResult<()> {
    entry.building_name = tokens.first().ok_or(INIError::InvalidData)?.to_string();
    Ok(())
}

fn parse_build_list_location(
    _ini: &mut INI,
    entry: &mut BuildListEntry,
    tokens: &[&str],
) -> INIResult<()> {
    entry.location = INI::parse_coord_2d(tokens)?;
    Ok(())
}

fn parse_build_list_rebuilds(
    _ini: &mut INI,
    entry: &mut BuildListEntry,
    tokens: &[&str],
) -> INIResult<()> {
    entry.rebuilds = parse_int_field(tokens)?;
    Ok(())
}

fn parse_build_list_angle(
    _ini: &mut INI,
    entry: &mut BuildListEntry,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    entry.angle_radians = INI::parse_angle_real(token)?;
    Ok(())
}

fn parse_build_list_initially_built(
    _ini: &mut INI,
    entry: &mut BuildListEntry,
    tokens: &[&str],
) -> INIResult<()> {
    entry.initially_built = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_build_list_rally_point_offset(
    _ini: &mut INI,
    entry: &mut BuildListEntry,
    tokens: &[&str],
) -> INIResult<()> {
    entry.rally_point_offset = INI::parse_coord_2d(tokens)?;
    Ok(())
}

fn parse_build_list_automatically_build(
    _ini: &mut INI,
    entry: &mut BuildListEntry,
    tokens: &[&str],
) -> INIResult<()> {
    entry.automatically_build = parse_bool_field(tokens)?;
    Ok(())
}

const AI_DATA_FIELDS: &[FieldParse<AIData>] = &[
    FieldParse {
        token: "StructureSeconds",
        parse: parse_ai_data_structure_seconds,
    },
    FieldParse {
        token: "TeamSeconds",
        parse: parse_ai_data_team_seconds,
    },
    FieldParse {
        token: "Wealthy",
        parse: parse_ai_data_resources_wealthy,
    },
    FieldParse {
        token: "Poor",
        parse: parse_ai_data_resources_poor,
    },
    FieldParse {
        token: "ForceIdleMSEC",
        parse: parse_ai_data_force_idle_msec,
    },
    FieldParse {
        token: "StructuresWealthyRate",
        parse: parse_ai_data_structures_wealthy_mod,
    },
    FieldParse {
        token: "TeamsWealthyRate",
        parse: parse_ai_data_team_wealthy_mod,
    },
    FieldParse {
        token: "StructuresPoorRate",
        parse: parse_ai_data_structures_poor_mod,
    },
    FieldParse {
        token: "TeamsPoorRate",
        parse: parse_ai_data_team_poor_mod,
    },
    FieldParse {
        token: "TeamResourcesToStart",
        parse: parse_ai_data_team_resources_to_build,
    },
    FieldParse {
        token: "GuardInnerModifierAI",
        parse: parse_ai_data_guard_inner_modifier_ai,
    },
    FieldParse {
        token: "GuardOuterModifierAI",
        parse: parse_ai_data_guard_outer_modifier_ai,
    },
    FieldParse {
        token: "GuardInnerModifierHuman",
        parse: parse_ai_data_guard_inner_modifier_human,
    },
    FieldParse {
        token: "GuardOuterModifierHuman",
        parse: parse_ai_data_guard_outer_modifier_human,
    },
    FieldParse {
        token: "GuardChaseUnitsDuration",
        parse: parse_ai_data_guard_chase_unit_frames,
    },
    FieldParse {
        token: "GuardEnemyScanRate",
        parse: parse_ai_data_guard_enemy_scan_rate,
    },
    FieldParse {
        token: "GuardEnemyReturnScanRate",
        parse: parse_ai_data_guard_enemy_return_scan_rate,
    },
    FieldParse {
        token: "SkirmishGroupFudgeDistance",
        parse: parse_ai_data_skirmish_group_fudge_value,
    },
    FieldParse {
        token: "RepulsedDistance",
        parse: parse_ai_data_repulsed_distance,
    },
    FieldParse {
        token: "EnableRepulsors",
        parse: parse_ai_data_enable_repulsors,
    },
    FieldParse {
        token: "AlertRangeModifier",
        parse: parse_ai_data_alert_range_modifier,
    },
    FieldParse {
        token: "AggressiveRangeModifier",
        parse: parse_ai_data_aggressive_range_modifier,
    },
    FieldParse {
        token: "ForceSkirmishAI",
        parse: parse_ai_data_force_skirmish_ai,
    },
    FieldParse {
        token: "RotateSkirmishBases",
        parse: parse_ai_data_rotate_skirmish_bases,
    },
    FieldParse {
        token: "AttackUsesLineOfSight",
        parse: parse_ai_data_attack_uses_line_of_sight,
    },
    FieldParse {
        token: "AttackIgnoreInsignificantBuildings",
        parse: parse_ai_data_attack_ignore_insignificant_buildings,
    },
    FieldParse {
        token: "AttackPriorityDistanceModifier",
        parse: parse_ai_data_attack_priority_distance_modifier,
    },
    FieldParse {
        token: "MaxRecruitRadius",
        parse: parse_ai_data_max_recruit_radius,
    },
    FieldParse {
        token: "SkirmishBaseDefenseExtraDistance",
        parse: parse_ai_data_skirmish_base_defense_extra_distance,
    },
    FieldParse {
        token: "WallHeight",
        parse: parse_ai_data_wall_height,
    },
    FieldParse {
        token: "SideInfo",
        parse: parse_ai_data_side_info,
    },
    FieldParse {
        token: "SkirmishBuildList",
        parse: parse_ai_data_skirmish_build_list,
    },
    FieldParse {
        token: "MinInfantryForGroup",
        parse: parse_ai_data_min_infantry_for_group,
    },
    FieldParse {
        token: "MinVehiclesForGroup",
        parse: parse_ai_data_min_vehicles_for_group,
    },
    FieldParse {
        token: "MinDistanceForGroup",
        parse: parse_ai_data_min_distance_for_group,
    },
    FieldParse {
        token: "DistanceRequiresGroup",
        parse: parse_ai_data_distance_requires_group,
    },
    FieldParse {
        token: "MinClumpDensity",
        parse: parse_ai_data_min_clump_density,
    },
    FieldParse {
        token: "InfantryPathfindDiameter",
        parse: parse_ai_data_infantry_pathfind_diameter,
    },
    FieldParse {
        token: "VehiclePathfindDiameter",
        parse: parse_ai_data_vehicle_pathfind_diameter,
    },
    FieldParse {
        token: "RebuildDelayTimeSeconds",
        parse: parse_ai_data_rebuild_delay_seconds,
    },
    FieldParse {
        token: "SupplyCenterSafeRadius",
        parse: parse_ai_data_supply_center_safe_radius,
    },
    FieldParse {
        token: "AIDozerBoredRadiusModifier",
        parse: parse_ai_data_ai_dozer_bored_radius_modifier,
    },
    FieldParse {
        token: "AICrushesInfantry",
        parse: parse_ai_data_ai_crushes_infantry,
    },
    FieldParse {
        token: "MaxRetaliationDistance",
        parse: parse_ai_data_max_retaliation_distance,
    },
    FieldParse {
        token: "RetaliationFriendsRadius",
        parse: parse_ai_data_retaliation_friends_radius,
    },
];

const AI_SIDE_INFO_FIELDS: &[FieldParse<AiSideInfo>] = &[
    FieldParse {
        token: "ResourceGatherersEasy",
        parse: |_, info, tokens| {
            info.easy = parse_int_field(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ResourceGatherersNormal",
        parse: |_, info, tokens| {
            info.normal = parse_int_field(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ResourceGatherersHard",
        parse: |_, info, tokens| {
            info.hard = parse_int_field(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "BaseDefenseStructure1",
        parse: |_, info, tokens| {
            info.base_defense_structure_1 =
                tokens.first().ok_or(INIError::InvalidData)?.to_string();
            Ok(())
        },
    },
    FieldParse {
        token: "SkillSet1",
        parse: |ini, info, tokens| parse_skill_set(ini, &mut info.skill_set_1, tokens),
    },
    FieldParse {
        token: "SkillSet2",
        parse: |ini, info, tokens| parse_skill_set(ini, &mut info.skill_set_2, tokens),
    },
    FieldParse {
        token: "SkillSet3",
        parse: |ini, info, tokens| parse_skill_set(ini, &mut info.skill_set_3, tokens),
    },
    FieldParse {
        token: "SkillSet4",
        parse: |ini, info, tokens| parse_skill_set(ini, &mut info.skill_set_4, tokens),
    },
    FieldParse {
        token: "SkillSet5",
        parse: |ini, info, tokens| parse_skill_set(ini, &mut info.skill_set_5, tokens),
    },
];

const AI_SKILL_SET_FIELDS: &[FieldParse<SkillSet>] = &[FieldParse {
    token: "Science",
    parse: parse_science_skill,
}];

const AI_BUILD_LIST_FIELDS: &[FieldParse<AiSideBuildList>] = &[FieldParse {
    token: "Structure",
    parse: parse_build_list_structure,
}];

const BUILD_LIST_STRUCTURE_FIELDS: &[FieldParse<BuildListEntry>] = &[
    FieldParse {
        token: "Name",
        parse: parse_build_list_name,
    },
    FieldParse {
        token: "Location",
        parse: parse_build_list_location,
    },
    FieldParse {
        token: "Rebuilds",
        parse: parse_build_list_rebuilds,
    },
    FieldParse {
        token: "Angle",
        parse: parse_build_list_angle,
    },
    FieldParse {
        token: "InitiallyBuilt",
        parse: parse_build_list_initially_built,
    },
    FieldParse {
        token: "RallyPointOffset",
        parse: parse_build_list_rally_point_offset,
    },
    FieldParse {
        token: "AutomaticallyBuild",
        parse: parse_build_list_automatically_build,
    },
];

/// Parse AI Data definition from INI file
pub fn parse_ai_data_definition(ini: &mut INI) -> INIResult<()> {
    let mut store = get_ai_data_store_mut();
    store.ensure_base();

    if ini.get_load_type() == INILoadType::CreateOverrides {
        store.push_override();
    }

    let Some(active) = store.get_active_mut() else {
        return Err(INIError::InvalidData);
    };

    ini.init_from_ini_with_fields(active, AI_DATA_FIELDS)?;
    Ok(())
}
