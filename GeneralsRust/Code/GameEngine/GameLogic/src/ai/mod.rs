//! AI module - Artificial Intelligence subsystem
//!
//! This module provides the AI behaviors for all game entities, pathfinding,
//! and group management functionality.
//!
//! Author: Converted from C++ by Claude, original by Michael S. Booth, November 2000

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::vec::Vec;

use self::pathfind_astar::PathfindCellType;
use self::pathfind_complete::{
    PathRequest as ClassicPathRequest, PathResult as ClassicPathResult,
    PathfindingSystem as ClassicPathfindingSystem, PATHFIND_QUEUE_LEN,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::build_list_info::BuildListInfo;
use crate::common::xfer::{Xfer, XferExt};
use crate::common::Snapshot;
use crate::common::{
    BodyDamageType, DisabledType, KindOf, ObjectID, ObjectStatusTypes, Relationship,
    FROM_BOUNDING_SPHERE_2D, INVALID_ID,
};
pub use crate::common::{
    CommandSourceType, Coord2D, Coord3D, ICoord2D, Real, SpecialPowerType, WeaponLockType,
    WeaponSetType, WeaponSlotType,
};
use crate::helpers::ThePartitionManager;
use crate::modules::AIAttitudeType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::physics::{SurfaceType, TerrainQuery};
use crate::player::PlayerType;
use crate::scripting::engine::get_script_engine;
pub use crate::scripting::engine::AttackPriorityInfo;
use crate::team::get_team_factory;
use crate::terrain::TerrainLogic;
use game_engine::common::ini::{
    get_ai_data_store, AIData as IniAIData, AiSideBuildList as IniAiSideBuildList,
    AiSideInfo as IniAiSideInfo, BuildListEntry as IniBuildListEntry, SkillSet as IniSkillSet,
};

pub type ObjectId = ObjectID;
pub type PlayerId = u32;
pub type FormationId = u32;
pub type TeamName = String;
pub type GUICommandType = u32;
pub type HackerAttackMode = u32;
pub use game_engine::common::rts::ScienceType;

pub mod enhanced_player;
pub mod groups;
pub mod integration;
pub mod native;
pub mod object_registry;
pub mod state_machine;

// Constants
pub const MAX_AI_UPGRADES: usize = 20;
pub const NO_FORMATION_ID: FormationId = 0xFFFFFFFF;

// Debug options for AI visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiDebugOptions {
    None = 0,
    Paths,
    Terrain,
    Cells,
    GroundPaths,
    Zones,
}

// Vision factor flags
pub mod vision_factors {
    pub const OWNER_TYPE: u32 = 0x01;
    pub const MOOD: u32 = 0x02;
    pub const GUARD_INNER: u32 = 0x04;
}

// AI enemy search qualifiers
pub mod search_qualifiers {
    pub const CAN_SEE: u32 = 1 << 0;
    pub const CAN_ATTACK: u32 = 1 << 1;
    pub const IGNORE_INSIGNIFICANT_BUILDINGS: u32 = 1 << 2;
    pub const ATTACK_BUILDINGS: u32 = 1 << 3;
    pub const WITHIN_ATTACK_RANGE: u32 = 1 << 4;
    pub const UNFOGGED: u32 = 1 << 5;
}

pub fn resolve_attack_priority_info_for_object(owner_id: ObjectID) -> Option<AttackPriorityInfo> {
    let mut priority_set_name = String::new();

    if let Ok(engine_lock) = get_script_engine().read() {
        if let Some(engine) = engine_lock.as_ref() {
            if let Some(name) = engine.get_object_attack_priority_set(owner_id) {
                if !name.is_empty() {
                    priority_set_name = name.to_string();
                }
            }
        }
    }

    if priority_set_name.is_empty() {
        let team_name = OBJECT_REGISTRY.get_object(owner_id).and_then(|object_arc| {
            let object = object_arc.read().ok()?;
            let team_arc = object.get_team()?;
            let team = team_arc.read().ok()?;
            Some(team.get_name().to_string())
        });

        if let Some(team_name) = team_name {
            if let Ok(factory) = get_team_factory().lock() {
                if let Some(prototype) = factory.find_team_prototype(&team_name) {
                    let name = prototype.get_attack_priority_name().as_str();
                    if !name.is_empty() {
                        priority_set_name = name.to_string();
                    }
                }
            }
        }
    }

    if priority_set_name.is_empty() {
        return None;
    }

    if let Ok(engine_lock) = get_script_engine().read() {
        if let Some(engine) = engine_lock.as_ref() {
            return engine.get_attack_info(&priority_set_name).cloned();
        }
    }

    None
}

// AI attitude behavior modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttitudeType {
    Sleep = -2,
    Passive = -1,
    Normal = 0,
    Alert = 1,
    Aggressive = 2,
    Invalid = 3,
}

/// Mood matrix actions used for behavior adjustments (matches C++ MM_Action_*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoodMatrixAction {
    Move,
    Idle,
    Attack,
    AttackMove,
}

/// Mood matrix bitflags (matches C++ MoodMatrixParameters).
pub mod mood_matrix_parameters {
    pub const CONTROLLER_PLAYER: u32 = 0x0000_0001;
    pub const CONTROLLER_AI: u32 = 0x0000_0002;
    pub const CONTROLLER_BITMASK: u32 = CONTROLLER_PLAYER | CONTROLLER_AI;

    pub const UNITTYPE_NON_TURRETED: u32 = 0x0000_0010;
    pub const UNITTYPE_TURRETED: u32 = 0x0000_0020;
    pub const UNITTYPE_AIR: u32 = 0x0000_0040;
    pub const UNITTYPE_BITMASK: u32 = UNITTYPE_NON_TURRETED | UNITTYPE_TURRETED | UNITTYPE_AIR;

    pub const MOOD_SLEEP: u32 = 0x0000_0100;
    pub const MOOD_PASSIVE: u32 = 0x0000_0200;
    pub const MOOD_NORMAL: u32 = 0x0000_0400;
    pub const MOOD_ALERT: u32 = 0x0000_0800;
    pub const MOOD_AGGRESSIVE: u32 = 0x0000_1000;
    pub const MOOD_BITMASK: u32 =
        MOOD_SLEEP | MOOD_PASSIVE | MOOD_NORMAL | MOOD_ALERT | MOOD_AGGRESSIVE;
}

/// Mood matrix action adjustment flags (matches C++ MAA_Action_*).
pub mod mood_matrix_adjustment {
    /// Matches C++ MAA_Action_Ok.
    pub const ACTION_OK: u32 = 0x0000_0001;
    /// Matches C++ MAA_Action_To_Idle.
    pub const ACTION_TO_IDLE: u32 = 0x0000_0002;
    /// Matches C++ MAA_Action_To_AttackMove.
    pub const ACTION_TO_ATTACK_MOVE: u32 = 0x0000_0004;

    /// Matches C++ MAA_Affect_Range_IgnoreAll.
    pub const AFFECT_RANGE_IGNORE_ALL: u32 = 0x0000_0010;
    /// Matches C++ MAA_Affect_Range_WaitForAttack.
    pub const AFFECT_RANGE_WAIT_FOR_ATTACK: u32 = 0x0000_0020;
    /// Matches C++ MAA_Affect_Range_Alert.
    pub const AFFECT_RANGE_ALERT: u32 = 0x0000_0040;
    /// Matches C++ MAA_Affect_Range_Aggressive.
    pub const AFFECT_RANGE_AGGRESSIVE: u32 = 0x0000_0080;
}

// AI Command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiCommandType {
    NoCommand = -1,
    MoveToPosition = 0,
    MoveToObject,
    TightenToPosition,
    MoveToPositionAndEvacuate,
    MoveToPositionAndEvacuateAndExit,
    Idle,
    FollowWaypointPath,
    FollowWaypointPathAsTeam,
    FollowUserPath,
    FollowPath,
    FollowExitProductionPath,
    AttackObject,
    ForceAttackObject,
    AttackTeam,
    AttackPosition,
    AttackMoveToPosition,
    AttackFollowWaypointPath,
    AttackFollowWaypointPathAsTeam,
    Hunt,
    Repair,
    PickUpPrisoner,
    ReturnPrisoners,
    ResumeConstruction,
    GetHealed,
    GetRepaired,
    Enter,
    Dock,
    Exit,
    Evacuate,
    ExecuteRailedTransport,
    GoProne,
    GuardPosition,
    GuardObject,
    GuardArea,
    DeployAssaultReturn,
    AttackArea,
    HackInternet,
    FaceObject,
    FacePosition,
    RappelInto,
    CombatDrop,
    CommandButtonPos,
    CommandButtonObj,
    CommandButton,
    Wander,
    WanderInPlace,
    Panic,
    Busy,
    FollowWaypointPathExact,
    FollowWaypointPathAsTeamExact,
    MoveAwayFromUnit,
    FollowPathAppend,
    MoveToPositionEvenIfSleeping,
    GuardTunnelNetwork,
    EvacuateInstantly,
    ExitInstantly,
    GuardRetaliate,
    DoSpecialPower,
    DoSpecialPowerAtObject,
    DoSpecialPowerAtLocation,
    Sell,
    ToggleOvercharge,
    Surrender,
    Cheer,
}

// Skill set structure
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
            guard_enemy_scan_rate: 30,        // LOGICFRAMES_PER_SECOND/2
            guard_enemy_return_scan_rate: 60, // LOGICFRAMES_PER_SECOND
            wall_height: 0.0,
            alert_range_modifier: 0.0,
            aggressive_range_modifier: 0.0,
            attack_priority_distance_modifier: 0.0,
            skirmish_group_fudge_value: 0.0,
            max_recruit_distance: 0.0,
            skirmish_base_defense_extra_distance: 150.0,
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
            rebuild_delay_seconds: 30,
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

impl Snapshot for AiData {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut structure_seconds = self.structure_seconds;
        let _ = xfer.xfer_real(&mut structure_seconds);
        let mut team_seconds = self.team_seconds;
        let _ = xfer.xfer_real(&mut team_seconds);
        let mut resources_wealthy = self.resources_wealthy;
        let _ = xfer.xfer_int(&mut resources_wealthy);
        let mut resources_poor = self.resources_poor;
        let _ = xfer.xfer_int(&mut resources_poor);
        let mut force_idle_frames_count = self.force_idle_frames_count;
        let _ = xfer.xfer_unsigned_int(&mut force_idle_frames_count);
        let mut structures_wealthy_mod = self.structures_wealthy_mod;
        let _ = xfer.xfer_real(&mut structures_wealthy_mod);
        let mut team_wealthy_mod = self.team_wealthy_mod;
        let _ = xfer.xfer_real(&mut team_wealthy_mod);
        let mut structures_poor_mod = self.structures_poor_mod;
        let _ = xfer.xfer_real(&mut structures_poor_mod);
        let mut team_poor_mod = self.team_poor_mod;
        let _ = xfer.xfer_real(&mut team_poor_mod);
        let mut team_resources_to_build = self.team_resources_to_build;
        let _ = xfer.xfer_real(&mut team_resources_to_build);
        let mut guard_inner_modifier_ai = self.guard_inner_modifier_ai;
        let _ = xfer.xfer_real(&mut guard_inner_modifier_ai);
        let mut guard_outer_modifier_ai = self.guard_outer_modifier_ai;
        let _ = xfer.xfer_real(&mut guard_outer_modifier_ai);
        let mut guard_inner_modifier_human = self.guard_inner_modifier_human;
        let _ = xfer.xfer_real(&mut guard_inner_modifier_human);
        let mut guard_outer_modifier_human = self.guard_outer_modifier_human;
        let _ = xfer.xfer_real(&mut guard_outer_modifier_human);
        let mut guard_chase_unit_frames = self.guard_chase_unit_frames;
        let _ = xfer.xfer_unsigned_int(&mut guard_chase_unit_frames);
        let mut guard_enemy_scan_rate = self.guard_enemy_scan_rate;
        let _ = xfer.xfer_unsigned_int(&mut guard_enemy_scan_rate);
        let mut guard_enemy_return_scan_rate = self.guard_enemy_return_scan_rate;
        let _ = xfer.xfer_unsigned_int(&mut guard_enemy_return_scan_rate);
        let mut alert_range_modifier = self.alert_range_modifier;
        let _ = xfer.xfer_real(&mut alert_range_modifier);
        let mut aggressive_range_modifier = self.aggressive_range_modifier;
        let _ = xfer.xfer_real(&mut aggressive_range_modifier);
        let mut attack_priority_distance_modifier = self.attack_priority_distance_modifier;
        let _ = xfer.xfer_real(&mut attack_priority_distance_modifier);
        let mut max_recruit_distance = self.max_recruit_distance;
        let _ = xfer.xfer_real(&mut max_recruit_distance);
        let mut skirmish_base_defense_extra_distance = self.skirmish_base_defense_extra_distance;
        let _ = xfer.xfer_real(&mut skirmish_base_defense_extra_distance);
        let mut repulsed_distance = self.repulsed_distance;
        let _ = xfer.xfer_real(&mut repulsed_distance);
        let mut enable_repulsors = self.enable_repulsors;
        let _ = xfer.xfer_bool(&mut enable_repulsors);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);
    }

    fn load_post_process(&mut self) {}
}

fn convert_skill_set(src: &IniSkillSet) -> SkillSet {
    SkillSet {
        num_skills: src.num_skills,
        skills: src.skills,
    }
}

fn convert_side_info(src: &IniAiSideInfo) -> AiSideInfo {
    AiSideInfo {
        side: src.side.clone(),
        easy: src.easy,
        normal: src.normal,
        hard: src.hard,
        skill_set_1: convert_skill_set(&src.skill_set_1),
        skill_set_2: convert_skill_set(&src.skill_set_2),
        skill_set_3: convert_skill_set(&src.skill_set_3),
        skill_set_4: convert_skill_set(&src.skill_set_4),
        skill_set_5: convert_skill_set(&src.skill_set_5),
        base_defense_structure_1: src.base_defense_structure_1.clone(),
    }
}

fn convert_build_list_entry(entry: &IniBuildListEntry) -> BuildListInfo {
    let mut info = BuildListInfo::new();
    info.set_building_name(entry.building_name.as_str().into());
    info.set_template_name(entry.template_name.as_str().into());
    info.set_location(Coord3D::new(entry.location.0, entry.location.1, 0.0));
    info.set_rally_offset(Coord2D::new(
        entry.rally_point_offset.0,
        entry.rally_point_offset.1,
    ));
    info.set_angle(entry.angle_radians);
    info.set_initially_built(entry.initially_built);
    info.set_automatic_build(entry.automatically_build);
    if entry.rebuilds < 0 {
        info.set_num_rebuilds(0);
    } else {
        info.set_num_rebuilds(entry.rebuilds as u32);
    }
    info
}

fn convert_build_list(entries: &[IniBuildListEntry]) -> Option<Box<BuildListInfo>> {
    let mut next: Option<Box<BuildListInfo>> = None;
    for entry in entries.iter().rev() {
        let mut info = convert_build_list_entry(entry);
        info.set_next_build_list_boxed(next);
        next = Some(Box::new(info));
    }
    next
}

fn convert_side_build_list(src: &IniAiSideBuildList) -> AiSideBuildList {
    let mut build_list = AiSideBuildList::new(src.side.clone());
    build_list.build_list = convert_build_list(&src.entries);
    build_list
}

fn convert_ai_data(src: &IniAIData) -> AiData {
    let mut data = AiData::default();
    data.structure_seconds = src.structure_seconds;
    data.team_seconds = src.team_seconds;
    data.resources_wealthy = src.resources_wealthy;
    data.resources_poor = src.resources_poor;
    data.force_idle_frames_count = src.force_idle_frames_count;
    data.structures_wealthy_mod = src.structures_wealthy_mod;
    data.team_wealthy_mod = src.team_wealthy_mod;
    data.structures_poor_mod = src.structures_poor_mod;
    data.team_poor_mod = src.team_poor_mod;
    data.team_resources_to_build = src.team_resources_to_build;
    data.guard_inner_modifier_ai = src.guard_inner_modifier_ai;
    data.guard_outer_modifier_ai = src.guard_outer_modifier_ai;
    data.guard_inner_modifier_human = src.guard_inner_modifier_human;
    data.guard_outer_modifier_human = src.guard_outer_modifier_human;
    data.guard_chase_unit_frames = src.guard_chase_unit_frames;
    data.guard_enemy_scan_rate = src.guard_enemy_scan_rate;
    data.guard_enemy_return_scan_rate = src.guard_enemy_return_scan_rate;
    data.wall_height = src.wall_height;
    data.alert_range_modifier = src.alert_range_modifier;
    data.aggressive_range_modifier = src.aggressive_range_modifier;
    data.attack_priority_distance_modifier = src.attack_priority_distance_modifier;
    data.skirmish_group_fudge_value = src.skirmish_group_fudge_value;
    data.max_recruit_distance = src.max_recruit_distance;
    data.skirmish_base_defense_extra_distance = src.skirmish_base_defense_extra_distance;
    data.repulsed_distance = src.repulsed_distance;
    data.enable_repulsors = src.enable_repulsors;
    data.force_skirmish_ai = src.force_skirmish_ai;
    data.rotate_skirmish_bases = src.rotate_skirmish_bases;
    data.attack_uses_line_of_sight = src.attack_uses_line_of_sight;
    data.attack_ignore_insignificant_buildings = src.attack_ignore_insignificant_buildings;
    data.min_infantry_for_group = src.min_infantry_for_group;
    data.min_vehicles_for_group = src.min_vehicles_for_group;
    data.min_distance_for_group = src.min_distance_for_group;
    data.distance_requires_group = src.distance_requires_group;
    data.min_clump_density = src.min_clump_density;
    data.infantry_pathfind_diameter = src.infantry_pathfind_diameter;
    data.vehicle_pathfind_diameter = src.vehicle_pathfind_diameter;
    data.rebuild_delay_seconds = src.rebuild_delay_seconds;
    data.supply_center_safe_radius = src.supply_center_safe_radius;
    data.ai_dozer_bored_radius_modifier = src.ai_dozer_bored_radius_modifier;
    data.ai_crushes_infantry = src.ai_crushes_infantry;
    data.max_retaliate_distance = src.max_retaliate_distance;
    data.retaliate_friends_radius = src.retaliate_friends_radius;
    data.side_info = src.side_info.iter().map(convert_side_info).collect();
    data.side_build_lists = src
        .side_build_lists
        .iter()
        .map(convert_side_build_list)
        .collect();
    data
}

// AI Command Parameters
#[derive(Debug, Clone)]
pub struct AiCommandParams {
    pub cmd: AiCommandType,
    pub cmd_source: CommandSourceType,
    pub pos: Coord3D,
    pub obj: Option<ObjectId>,
    pub other_obj: Option<ObjectId>,
    pub team: Option<TeamName>,
    pub coords: Vec<Coord3D>,
    pub waypoint: Option<WaypointId>,
    pub polygon: Option<PolygonTriggerId>,
    pub int_value: i32,
    pub damage: DamageInfo,
    pub command_button: Option<CommandButtonId>,
    pub path: Option<PathId>,
}

impl AiCommandParams {
    pub fn new(cmd: AiCommandType, cmd_source: CommandSourceType) -> Self {
        Self {
            cmd,
            cmd_source,
            pos: Coord3D::new(0.0, 0.0, 0.0),
            obj: None,
            other_obj: None,
            team: None,
            coords: Vec::new(),
            waypoint: None,
            polygon: None,
            int_value: 0,
            damage: DamageInfo::default(),
            command_button: None,
            path: None,
        }
    }
}

pub type WaypointId = crate::waypoint::WaypointId;
pub type PolygonTriggerId = crate::polygon_trigger::PolygonTriggerId;
pub type CommandButtonId = crate::command_button::CommandButtonId;
pub type PathId = crate::path::PathId;

#[derive(Debug, Clone, Default)]
pub struct DamageInfo {
    // Add damage-related fields here
}

// AI Command Interface trait
pub trait AiCommandInterface {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), AiError>;

    // Convenience methods for common commands
    fn ai_move_to_position(
        &mut self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::MoveToPosition, cmd_source);
        params.pos = *pos;
        self.ai_do_command(&params)
    }

    fn ai_move_to_object(
        &mut self,
        obj: ObjectId,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::MoveToObject, cmd_source);
        params.obj = Some(obj);
        self.ai_do_command(&params)
    }

    fn ai_tighten_to_position(
        &mut self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::TightenToPosition, cmd_source);
        params.pos = *pos;
        self.ai_do_command(&params)
    }

    fn ai_follow_exit_production_path(
        &mut self,
        path: &[Coord3D],
        ignore_object: Option<ObjectId>,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::FollowExitProductionPath, cmd_source);
        params.coords = path.to_vec();
        params.obj = ignore_object;
        self.ai_do_command(&params)
    }

    fn ai_follow_path(
        &mut self,
        path: &[Coord3D],
        ignore_object: Option<ObjectId>,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::FollowPath, cmd_source);
        params.coords = path.to_vec();
        params.obj = ignore_object;
        self.ai_do_command(&params)
    }

    fn ai_follow_path_append(
        &mut self,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::FollowPathAppend, cmd_source);
        params.pos = *pos;
        self.ai_do_command(&params)
    }

    fn ai_move_away_from_unit(
        &mut self,
        obj: ObjectId,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::MoveAwayFromUnit, cmd_source);
        params.obj = Some(obj);
        self.ai_do_command(&params)
    }

    fn ai_idle(&mut self, cmd_source: CommandSourceType) -> Result<(), AiError> {
        let params = AiCommandParams::new(AiCommandType::Idle, cmd_source);
        self.ai_do_command(&params)
    }

    fn ai_attack_object(
        &mut self,
        victim: ObjectId,
        max_shots: i32,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::AttackObject, cmd_source);
        params.obj = Some(victim);
        params.int_value = max_shots;
        self.ai_do_command(&params)
    }

    fn ai_guard_position(
        &mut self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) -> Result<(), AiError> {
        let mut params = AiCommandParams::new(AiCommandType::GuardPosition, cmd_source);
        params.pos = *pos;
        params.int_value = guard_mode.as_i32();
        self.ai_do_command(&params)
    }

    fn ai_hunt(&mut self, cmd_source: CommandSourceType) -> Result<(), AiError> {
        let params = AiCommandParams::new(AiCommandType::Hunt, cmd_source);
        self.ai_do_command(&params)
    }
}

// AI Group - collection of AI objects for group pathfinding and commands
#[allow(dead_code)]
#[derive(Debug)]
pub struct AiGroup {
    id: u32,
    member_list: Vec<ObjectId>,
    speed: Real,
    dirty: bool,
    ground_path: Option<PathId>,
}

impl AiGroup {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            member_list: Vec::new(),
            speed: 0.0,
            dirty: true,
            ground_path: None,
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn add(&mut self, obj: ObjectId) {
        if !self.member_list.contains(&obj) {
            self.member_list.push(obj);
            self.dirty = true;
        }
    }

    pub fn remove(&mut self, obj: ObjectId) -> bool {
        if let Some(pos) = self.member_list.iter().position(|&x| x == obj) {
            self.member_list.remove(pos);
            self.dirty = true;

            // Return true if group is now empty (should be destroyed)
            self.member_list.is_empty()
        } else {
            false
        }
    }

    pub fn is_member(&self, obj: ObjectId) -> bool {
        self.member_list.contains(&obj)
    }

    pub fn get_count(&self) -> usize {
        self.member_list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.member_list.is_empty()
    }

    pub fn get_all_ids(&self) -> &Vec<ObjectId> {
        &self.member_list
    }

    pub fn recompute_group_speed(&mut self) {
        self.dirty = true;
    }

    pub fn set_attitude(&mut self, attitude: AttitudeType) -> Result<(), AiError> {
        let module_attitude = to_module_attitude(attitude);
        for obj_id in &self.member_list {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let ai = obj_guard.get_ai_update_interface();
            drop(obj_guard);
            let Some(ai) = ai else {
                continue;
            };
            let Ok(mut ai_guard) = ai.lock() else {
                continue;
            };
            let _ = ai_guard.set_attitude(module_attitude);
        }
        Ok(())
    }

    pub fn get_attitude(&self) -> Result<AttitudeType, AiError> {
        for obj_id in &self.member_list {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let ai = obj_guard.get_ai_update_interface();
            drop(obj_guard);
            let Some(ai) = ai else {
                continue;
            };
            let Ok(ai_guard) = ai.lock() else {
                continue;
            };
            return Ok(from_module_attitude(ai_guard.get_attitude()));
        }
        Ok(AttitudeType::Normal)
    }

    pub fn is_idle(&self) -> bool {
        for obj_id in &self.member_list {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let ai = obj_guard.get_ai_update_interface();
            drop(obj_guard);
            let Some(ai) = ai else {
                continue;
            };
            let Ok(ai_guard) = ai.lock() else {
                continue;
            };
            if !ai_guard.is_idle() {
                return false;
            }
        }
        true
    }

    pub fn is_busy(&self) -> bool {
        for obj_id in &self.member_list {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let ai = obj_guard.get_ai_update_interface();
            drop(obj_guard);
            let Some(ai) = ai else {
                continue;
            };
            let Ok(ai_guard) = ai.lock() else {
                continue;
            };
            if ai_guard.is_busy() {
                return true;
            }
        }
        false
    }

    pub fn get_speed(&mut self) -> Real {
        if self.dirty {
            self.recompute();
        }
        self.speed
    }

    pub fn get_center(&self) -> Option<Coord3D> {
        let mut count = 0;
        let mut center = Coord3D::new(0.0, 0.0, 0.0);

        // Prefer AI-capable objects first.
        for obj_id in &self.member_list {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_disabled_by_type(DisabledType::Held) {
                continue;
            }
            if obj_guard.get_ai_update_interface().is_some() {
                let pos = obj_guard.get_position();
                center.x += pos.x;
                center.y += pos.y;
                center.z += pos.z;
                count += 1;
            }
        }

        // If none, use all members.
        if count == 0 {
            for obj_id in &self.member_list {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_disabled_by_type(DisabledType::Held) {
                    continue;
                }
                let pos = obj_guard.get_position();
                center.x += pos.x;
                center.y += pos.y;
                center.z += pos.z;
                count += 1;
            }
        }

        if count > 0 {
            center.x /= count as f32;
            center.y /= count as f32;
            center.z /= count as f32;
            Some(center)
        } else {
            None
        }
    }

    pub fn group_move_to_position(
        &self,
        position: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        let mut params = AiCommandParams::new(AiCommandType::MoveToPosition, cmd_source);
        params.pos = *position;
        params.int_value = i32::from(add_waypoint);
        self.dispatch_command_to_members(&params);
    }

    pub fn group_attack_move_to_position(&self, position: &Coord3D, cmd_source: CommandSourceType) {
        let mut params = AiCommandParams::new(AiCommandType::AttackMoveToPosition, cmd_source);
        params.pos = *position;
        self.dispatch_command_to_members(&params);
    }

    pub fn get_special_power_source_object(
        &self,
        _special_power_id: u32,
    ) -> Option<Arc<RwLock<Object>>> {
        self.member_list
            .iter()
            .find_map(|obj_id| OBJECT_REGISTRY.get_object(*obj_id))
    }

    pub fn get_command_button_source_object(
        &self,
        _command_button_id: u32,
    ) -> Option<Arc<RwLock<Object>>> {
        self.member_list
            .iter()
            .find_map(|obj_id| OBJECT_REGISTRY.get_object(*obj_id))
    }

    fn recompute(&mut self) {
        if !self.dirty {
            return;
        }

        let mut min_speed = Real::INFINITY;
        for obj_id in &self.member_list {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let ai = obj_guard.get_ai_update_interface();
            drop(obj_guard);
            if let Some(ai) = ai {
                let Ok(ai_guard) = ai.lock() else {
                    continue;
                };
                min_speed = min_speed.min(ai_guard.get_speed().max(0.0));
            }
        }
        if !min_speed.is_finite() {
            min_speed = 0.0;
        }
        self.speed = min_speed;
        self.dirty = false;
    }

    fn dispatch_command_to_members(&self, params: &AiCommandParams) {
        for obj_id in &self.member_list {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let ai = obj_guard.get_ai_update_interface();
            drop(obj_guard);
            let Some(ai) = ai else {
                continue;
            };
            let Ok(mut ai_guard) = ai.lock() else {
                continue;
            };
            let _ = ai_guard.execute_command(params);
        }
    }
}

fn to_module_attitude(attitude: AttitudeType) -> AIAttitudeType {
    match attitude {
        AttitudeType::Aggressive => AIAttitudeType::Aggressive,
        AttitudeType::Alert => AIAttitudeType::Defensive,
        AttitudeType::Sleep => AIAttitudeType::Sleep,
        AttitudeType::Passive => AIAttitudeType::Passive,
        AttitudeType::Normal | AttitudeType::Invalid => AIAttitudeType::Normal,
    }
}

fn from_module_attitude(attitude: AIAttitudeType) -> AttitudeType {
    match attitude {
        AIAttitudeType::Aggressive => AttitudeType::Aggressive,
        AIAttitudeType::Defensive => AttitudeType::Alert,
        AIAttitudeType::Passive => AttitudeType::Passive,
        AIAttitudeType::Sleep => AttitudeType::Sleep,
        AIAttitudeType::Normal => AttitudeType::Normal,
    }
}

impl AiCommandInterface for AiGroup {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), AiError> {
        for obj_id in &self.member_list {
            let dispatch_started = Instant::now();
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.execute_command(params);
                }
            }
            let elapsed = dispatch_started.elapsed();
            if elapsed >= Duration::from_millis(200) {
                log::warn!(
                    "Slow AI group command dispatch: cmd={:?} object_id={} elapsed={:?}",
                    params.cmd,
                    obj_id,
                    elapsed
                );
            }
        }
        Ok(())
    }
}

// AI error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    InvalidObject,
    EmptyGroup,
    InvalidCommand,
    PathfindingFailed,
    NoPathfinder,
    InvalidTarget,
    LockFailed,
    NotInitialized,
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::InvalidObject => write!(f, "Invalid object"),
            AiError::EmptyGroup => write!(f, "Empty group"),
            AiError::InvalidCommand => write!(f, "Invalid command"),
            AiError::PathfindingFailed => write!(f, "Pathfinding failed"),
            AiError::NoPathfinder => write!(f, "No pathfinder available"),
            AiError::InvalidTarget => write!(f, "Invalid target"),
            AiError::LockFailed => write!(f, "Lock failed"),
            AiError::NotInitialized => write!(f, "AI integration not initialized"),
        }
    }
}

impl std::error::Error for AiError {}

impl From<String> for AiError {
    fn from(_: String) -> Self {
        AiError::PathfindingFailed
    }
}

// Main AI subsystem
/// Matches C++ AI.cpp TheAI singleton
#[derive(Debug)]
pub struct AI {
    pathfinder: Option<Arc<RwLock<Pathfinder>>>,
    pathfinding_system: Option<pathfinding_system::SharedPathfindingSystem>,
    group_list: Vec<Arc<RwLock<AiGroup>>>,
    ai_data: Arc<RwLock<AiData>>,
    next_group_id: u32,
    next_formation_id: FormationId,
}

impl AI {
    /// Constructor - matches C++ AI::AI() at AI.cpp:286
    pub fn new() -> Self {
        Self {
            pathfinder: Some(Arc::new(RwLock::new(Pathfinder::new()))),
            pathfinding_system: Some(pathfinding_system::create_pathfinding_system(1000, 1000)),
            group_list: Vec::new(),
            ai_data: Arc::new(RwLock::new(AiData::default())),
            next_group_id: 0,
            next_formation_id: NO_FORMATION_ID,
        }
    }

    /// Initialize the AI system
    /// Matches C++ AI::init() at AI.cpp:296
    pub fn init(&mut self) {
        self.next_group_id = 0;

        if let Ok(mut ai_data) = self.ai_data.write() {
            if let Some(ini_data) = get_ai_data_store().get_active() {
                *ai_data = convert_ai_data(ini_data);
            }
        }

        // Initialize pathfinding system
        if let Some(ref ps) = self.pathfinding_system {
            if let Ok(mut system) = ps.write() {
                system.initialize();
            }
        }
    }

    /// Reset the AI system in preparation for a new map
    /// Matches C++ AI::reset() at AI.cpp:304
    pub fn reset(&mut self) {
        if let Some(pathfinder) = &self.pathfinder {
            if let Ok(mut pf) = pathfinder.write() {
                pf.reset();
            }
        }

        self.group_list.clear();
        self.next_group_id = 0;
        self.next_formation_id = NO_FORMATION_ID;
        self.get_next_formation_id(); // Increment past NO_FORMATION_ID
    }

    /// Update the AI system
    /// Matches C++ AI::update() at AI.cpp:332
    pub fn update(&mut self, _current_frame: u32) -> Result<(), AiError> {
        // Process legacy pathfinding (matches C++ pathfinder->processPathfindQueue())
        if let Some(pathfinder) = &self.pathfinder {
            if let Ok(mut pf) = pathfinder.write() {
                pf.process_pathfind_queue()?;
            }
        }

        // Run player updates (matches C++ ThePlayerList->UPDATE()).
        // AI player lifecycle is managed by AIManager in this port.

        Ok(())
    }

    pub fn pathfinder(&self) -> Option<Arc<RwLock<Pathfinder>>> {
        self.pathfinder.clone()
    }

    pub fn pathfinding_system(&self) -> Option<pathfinding_system::SharedPathfindingSystem> {
        self.pathfinding_system.clone()
    }

    pub fn get_ai_data(&self) -> Arc<RwLock<AiData>> {
        self.ai_data.clone()
    }

    pub fn create_group(&mut self) -> Arc<RwLock<AiGroup>> {
        let group = Arc::new(RwLock::new(AiGroup::new(self.get_next_group_id())));
        self.group_list.push(group.clone());
        group
    }

    pub fn get_group_by_id(&self, group_id: u32) -> Option<Arc<RwLock<AiGroup>>> {
        for group in &self.group_list {
            if let Ok(guard) = group.read() {
                if guard.get_id() == group_id {
                    return Some(group.clone());
                }
            }
        }
        None
    }

    pub fn destroy_group(&mut self, group_id: u32) -> Result<(), AiError> {
        self.group_list.retain(|g| {
            if let Ok(group) = g.read() {
                group.get_id() != group_id
            } else {
                true // Keep groups we can't read
            }
        });
        Ok(())
    }

    pub fn find_group(&self, id: u32) -> Option<Arc<RwLock<AiGroup>>> {
        self.group_list
            .iter()
            .find(|g| {
                if let Ok(group) = g.read() {
                    group.get_id() == id
                } else {
                    false
                }
            })
            .cloned()
    }

    pub fn get_next_formation_id(&mut self) -> FormationId {
        let next_val = self.next_formation_id;
        self.next_formation_id = self.next_formation_id.wrapping_add(1);
        next_val
    }

    fn get_next_group_id(&mut self) -> u32 {
        self.next_group_id += 1;
        self.next_group_id
    }

    pub fn find_closest_enemy(
        &self,
        me: ObjectId,
        range: Real,
        qualifiers: u32,
        info: Option<&AttackPriorityInfo>,
        optional_filter: Option<&dyn PartitionFilter>,
    ) -> Result<Option<ObjectId>, AiError> {
        let Some(me_arc) = OBJECT_REGISTRY.get_object(me) else {
            return Err(AiError::InvalidObject);
        };
        let Ok(me_guard) = me_arc.read() else {
            return Err(AiError::LockFailed);
        };

        if (qualifiers & search_qualifiers::CAN_ATTACK) != 0 && !me_guard.is_able_to_attack() {
            return Ok(None);
        }

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(None);
        };

        let candidates = partition.get_objects_in_range(me_guard.get_position(), range);
        let mut closest_id = None;
        let mut closest_dist_sqr = range * range + 1.0;

        let mut best_enemy = None;
        let mut effective_priority = 0;
        let mut actual_priority = 0;

        let use_priority = info.is_some();
        let attack_priority_modifier = self
            .ai_data
            .read()
            .unwrap()
            .attack_priority_distance_modifier;

        for target_id in candidates {
            if target_id == me {
                continue;
            }
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                continue;
            };
            let Ok(target) = target_arc.read() else {
                continue;
            };

            if target.is_effectively_dead() {
                continue;
            }

            if me_guard.is_off_map() != target.is_off_map() {
                continue;
            }

            if me_guard.relationship_to(&target) != Relationship::Enemies {
                continue;
            }

            if (qualifiers & search_qualifiers::ATTACK_BUILDINGS) == 0
                && target.is_kind_of(KindOf::Structure)
                && !target.is_able_to_attack()
            {
                continue;
            }

            if (qualifiers & search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS) != 0
                && target.is_kind_of(KindOf::Structure)
                && !target.is_kind_of(KindOf::CountsForVictory)
            {
                continue;
            }

            if (qualifiers & search_qualifiers::CAN_SEE) != 0 {
                if let Some(player_id) = me_guard.get_controlling_player_id() {
                    if !target.is_visible_to_player(player_id as u32) {
                        continue;
                    }
                }
            }

            if (qualifiers & search_qualifiers::UNFOGGED) != 0 {
                if let Some(player_id) = me_guard.get_controlling_player_id() {
                    if !target.is_visible_to_player(player_id as u32) {
                        continue;
                    }
                }
            }

            if target.is_stealthed() && !target.is_detected() {
                continue;
            }

            let attack_result = if (qualifiers
                & (search_qualifiers::CAN_ATTACK | search_qualifiers::WITHIN_ATTACK_RANGE))
                != 0
            {
                Some(me_guard.get_able_to_attack_specific_object(
                    AbleToAttackType::NewTarget,
                    &target,
                    CommandSourceType::FromAi,
                ))
            } else {
                None
            };

            if (qualifiers & search_qualifiers::CAN_ATTACK) != 0 {
                if matches!(
                    attack_result,
                    Some(CanAttackResult::NotPossible | CanAttackResult::InvalidShot)
                ) {
                    continue;
                }
            }

            if (qualifiers & search_qualifiers::WITHIN_ATTACK_RANGE) != 0 {
                if !matches!(attack_result, Some(CanAttackResult::Possible)) {
                    continue;
                }
            }

            if let Some(filter) = optional_filter {
                if !filter.allow(target_id) {
                    continue;
                }
            }

            if !use_priority {
                let dist_sqr = ThePartitionManager::get_distance_squared(
                    &me_guard,
                    &target,
                    FROM_BOUNDING_SPHERE_2D,
                );
                if dist_sqr < closest_dist_sqr {
                    closest_dist_sqr = dist_sqr;
                    closest_id = Some(target_id);
                }
                continue;
            }

            let priority_info = match info {
                Some(info) => info,
                None => {
                    continue;
                }
            };
            let template_name = target.get_template().get_name().as_str();
            let mut current_priority = priority_info.get_priority(template_name);
            if current_priority == 0 {
                continue;
            }

            // C++ AI.cpp lines 669-679: Check for garrisoned buildings/vehicles
            // and see if a higher priority unit is inside. This matches the C++
            // behavior where contained objects (garrisoned infantry) can raise the
            // effective attack priority of the container building/vehicle.
            if let Some(contain) = target.get_contain() {
                if let Ok(contain_guard) = contain.lock() {
                    for contained_id in contain_guard.get_contained_objects() {
                        if let Some(contained_arc) = OBJECT_REGISTRY.get_object(*contained_id) {
                            if let Ok(contained_obj) = contained_arc.read() {
                                let contained_template_name =
                                    contained_obj.get_template().get_name().as_str();
                                let contained_priority =
                                    priority_info.get_priority(contained_template_name);
                                if contained_priority > current_priority {
                                    current_priority = contained_priority;
                                }
                            }
                        }
                    }
                }
            }

            let dist_sqr = ThePartitionManager::get_distance_squared(
                &me_guard,
                &target,
                FROM_BOUNDING_SPHERE_2D,
            );
            let dist = dist_sqr.sqrt();
            let modifier = if attack_priority_modifier > 0.0 {
                (dist / attack_priority_modifier) as i32
            } else {
                0
            };
            let mut modified_priority = current_priority - modifier;
            if modified_priority < 1 {
                modified_priority = 1;
            }

            if modified_priority > effective_priority
                || (modified_priority == effective_priority && current_priority > actual_priority)
            {
                effective_priority = modified_priority;
                actual_priority = current_priority;
                best_enemy = Some(target_id);
            }
        }

        if use_priority {
            Ok(best_enemy)
        } else {
            Ok(closest_id)
        }
    }

    pub fn find_closest_ally(
        &self,
        me: ObjectId,
        range: Real,
        qualifiers: u32,
    ) -> Result<Option<ObjectId>, AiError> {
        let Some(me_arc) = OBJECT_REGISTRY.get_object(me) else {
            return Err(AiError::InvalidObject);
        };
        let Ok(me_guard) = me_arc.read() else {
            return Err(AiError::LockFailed);
        };

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(None);
        };

        let candidates = partition.get_objects_in_range(me_guard.get_position(), range);
        let mut closest_id = None;
        let mut closest_dist_sqr = range * range + 1.0;

        for target_id in candidates {
            if target_id == me {
                continue;
            }
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                continue;
            };
            let Ok(target) = target_arc.read() else {
                continue;
            };

            if target.is_effectively_dead() {
                continue;
            }

            if me_guard.is_off_map() != target.is_off_map() {
                continue;
            }

            if !matches!(me_guard.relationship_to(&target), Relationship::Allies) {
                continue;
            }

            if target.is_kind_of(KindOf::Structure) && !target.is_able_to_attack() {
                continue;
            }

            if (qualifiers & search_qualifiers::CAN_SEE) != 0 {
                if let Some(player_id) = me_guard.get_controlling_player_id() {
                    if !target.is_visible_to_player(player_id as u32) {
                        continue;
                    }
                }
            }

            let dist_sqr = ThePartitionManager::get_distance_squared(
                &me_guard,
                &target,
                FROM_BOUNDING_SPHERE_2D,
            );
            if dist_sqr < closest_dist_sqr {
                closest_dist_sqr = dist_sqr;
                closest_id = Some(target_id);
            }
        }

        Ok(closest_id)
    }

    pub fn find_closest_repulsor(
        &self,
        me: ObjectId,
        range: Real,
    ) -> Result<Option<ObjectId>, AiError> {
        let ai_data = self.ai_data.read().unwrap();
        if !ai_data.enable_repulsors {
            return Ok(None);
        }

        let Some(me_arc) = OBJECT_REGISTRY.get_object(me) else {
            return Err(AiError::InvalidObject);
        };
        let Ok(me_guard) = me_arc.read() else {
            return Err(AiError::LockFailed);
        };

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(None);
        };

        let candidates = partition.get_objects_in_range(me_guard.get_position(), range);
        let mut closest_id = None;
        let mut closest_dist_sqr = range * range + 1.0;

        for target_id in candidates {
            if target_id == me {
                continue;
            }
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                continue;
            };
            let Ok(target) = target_arc.read() else {
                continue;
            };

            if target.is_effectively_dead() {
                continue;
            }

            if !target.test_status(ObjectStatusTypes::Repulsor) {
                continue;
            }

            if target.is_stealthed() && !target.is_detected() {
                continue;
            }

            let dist_sqr = ThePartitionManager::get_distance_squared(
                &me_guard,
                &target,
                FROM_BOUNDING_SPHERE_2D,
            );
            if dist_sqr < closest_dist_sqr {
                closest_dist_sqr = dist_sqr;
                closest_id = Some(target_id);
            }
        }

        Ok(closest_id)
    }

    pub fn get_adjusted_vision_range_for_object(
        &self,
        object: ObjectId,
        factors_to_consider: u32,
    ) -> Result<Real, AiError> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object) else {
            return Err(AiError::InvalidObject);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Err(AiError::LockFailed);
        };

        let ai_data = self.ai_data.read().unwrap();
        let mut range = obj_guard.get_vision_range();

        let player_is_human = obj_guard
            .get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.get_player_type() == PlayerType::Human)
            })
            .unwrap_or(false);

        if (factors_to_consider & vision_factors::OWNER_TYPE) != 0 {
            if player_is_human {
                if (factors_to_consider & vision_factors::GUARD_INNER) != 0 {
                    range *= ai_data.guard_inner_modifier_human;
                } else {
                    range *= ai_data.guard_outer_modifier_human;
                }
            } else if (factors_to_consider & vision_factors::GUARD_INNER) != 0 {
                range *= ai_data.guard_inner_modifier_ai;
            } else {
                range *= ai_data.guard_outer_modifier_ai;
            }
        }

        if (factors_to_consider & vision_factors::MOOD) != 0 && !player_is_human {
            if let Some(ai_update) = obj_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai_update.lock() {
                    match ai_guard.get_attitude() {
                        AIAttitudeType::Aggressive => range *= ai_data.aggressive_range_modifier,
                        AIAttitudeType::Defensive => range *= ai_data.alert_range_modifier,
                        AIAttitudeType::Passive
                        | AIAttitudeType::Sleep
                        | AIAttitudeType::Normal => {}
                    }
                }
            }
        }

        Ok(range)
    }
}

impl Default for AI {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct Pathfinder {
    inner: ClassicPathfindingSystem,
}

impl Pathfinder {
    pub fn world_to_grid(&self, world_pos: &Coord3D) -> ICoord2D {
        let cell_size = self::pathfind_astar::PATHFIND_CELL_SIZE_F;
        ICoord2D::new(
            (world_pos.x / cell_size).floor() as i32,
            (world_pos.y / cell_size).floor() as i32,
        )
    }

    pub fn grid_to_world(&self, grid_pos: &ICoord2D) -> Coord3D {
        let cell_size = self::pathfind_astar::PATHFIND_CELL_SIZE_F;
        Coord3D::new(
            (grid_pos.x as f32 + 0.5) * cell_size,
            (grid_pos.y as f32 + 0.5) * cell_size,
            0.0,
        )
    }
}

pub(crate) fn object_footprint_positions(obj: &Object) -> Option<Vec<Coord3D>> {
    use crate::ai::pathfind_astar::{GridCoord, PathfindLayerEnum};
    use crate::common::KindOf;
    use crate::path::PATHFIND_CELL_SIZE_F;
    use std::collections::HashSet;

    if obj.is_kind_of(KindOf::Mine)
        || obj.is_kind_of(KindOf::Projectile)
        || obj.is_kind_of(KindOf::BridgeTower)
    {
        return None;
    }

    if !obj.is_kind_of(KindOf::Structure) && !obj.is_kind_of(KindOf::Barrier) {
        return None;
    }

    if obj.get_height_above_terrain() > PATHFIND_CELL_SIZE_F {
        return None;
    }

    let pos = *obj.get_position();
    let geom = obj.get_geometry_info();
    let angle = obj.get_orientation();

    let mut cells = HashSet::new();

    match obj.get_template_geometry_type() {
        Some(game_engine::system::geometry::GeometryType::Box) => {
            let halfsize_x = geom.get_major_radius();
            let halfsize_y = geom.get_minor_radius();
            let c = angle.cos();
            let s = angle.sin();
            let step = PATHFIND_CELL_SIZE_F * 0.5;
            let num_steps_x = ((2.0 * halfsize_x) / step).ceil() as i32;
            let num_steps_y = ((2.0 * halfsize_y) / step).ceil() as i32;
            let mut tl_x = pos.x - halfsize_x * c - halfsize_y * s;
            let mut tl_y = pos.y + halfsize_y * c - halfsize_x * s;
            let ydx = s * step;
            let ydy = -c * step;
            let xdx = c * step;
            let xdy = s * step;

            for _ in 0..num_steps_y {
                let mut x = tl_x;
                let mut y = tl_y;
                for _ in 0..num_steps_x {
                    let cx = ((x + 0.5) / PATHFIND_CELL_SIZE_F).floor() as i32;
                    let cy = ((y + 0.5) / PATHFIND_CELL_SIZE_F).floor() as i32;
                    if cx >= 0 && cy >= 0 {
                        cells.insert((cx, cy));
                    }
                    x += xdx;
                    y += xdy;
                }
                tl_x += ydx;
                tl_y += ydy;
            }
        }
        Some(game_engine::system::geometry::GeometryType::Sphere)
        | Some(game_engine::system::geometry::GeometryType::Cylinder)
        | None => {
            let radius = geom.get_major_radius();
            let top_left_x = ((0.5 + (pos.x - radius) / PATHFIND_CELL_SIZE_F).floor() as i32) - 1;
            let top_left_y = ((0.5 + (pos.y - radius) / PATHFIND_CELL_SIZE_F).floor() as i32) - 1;
            let mut size = radius / PATHFIND_CELL_SIZE_F;
            size += 0.4;
            let r2 = size * size;
            let bottom_right_x = top_left_x + (2.0 * size).ceil() as i32 + 2;
            let bottom_right_y = top_left_y + (2.0 * size).ceil() as i32 + 2;
            let center_x = pos.x / PATHFIND_CELL_SIZE_F;
            let center_y = pos.y / PATHFIND_CELL_SIZE_F;

            for j in top_left_y..bottom_right_y {
                for i in top_left_x..bottom_right_x {
                    let dx = i as f32 + 0.5 - center_x;
                    let dy = j as f32 + 0.5 - center_y;
                    if dx * dx + dy * dy <= r2 {
                        if i >= 0 && j >= 0 {
                            cells.insert((i, j));
                        }
                    }
                }
            }
        }
    }

    if cells.is_empty() {
        return Some(vec![pos]);
    }

    let mut positions = Vec::with_capacity(cells.len());
    for (cx, cy) in cells {
        let coord = GridCoord::new(cx, cy);
        let world = coord.to_world(PathfindLayerEnum::Ground);
        positions.push(world);
    }
    Some(positions)
}

impl Pathfinder {
    pub fn new() -> Self {
        Self {
            inner: ClassicPathfindingSystem::new(1000, 1000),
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn reset_with_size(&mut self, width: usize, height: usize) {
        self.inner = ClassicPathfindingSystem::new(width, height);
    }

    pub fn rebuild_from_terrain(&mut self, terrain: &TerrainLogic) {
        let extent = terrain.get_maximum_pathfind_extent();
        let cell_size = self::pathfind_astar::PATHFIND_CELL_SIZE_F;
        let width = (extent.hi.x / cell_size).ceil().max(1.0) as usize;
        let height = (extent.hi.y / cell_size).ceil().max(1.0) as usize;
        let lo_x = (extent.lo.x / cell_size).floor() as i32;
        let lo_y = (extent.lo.y / cell_size).floor() as i32;
        let mut hi_x = (extent.hi.x / cell_size).floor() as i32 - 1;
        let mut hi_y = (extent.hi.y / cell_size).floor() as i32 - 1;
        if hi_x < lo_x {
            hi_x = lo_x;
        }
        if hi_y < lo_y {
            hi_y = lo_y;
        }

        self.reset_with_size(width, height);

        for y in 0..height {
            for x in 0..width {
                let in_bounds = (x as i32) >= lo_x
                    && (x as i32) <= hi_x
                    && (y as i32) >= lo_y
                    && (y as i32) <= hi_y;
                let world_x = (x as f32 + 0.5) * cell_size;
                let world_y = (y as f32 + 0.5) * cell_size;
                let mut cell_type = if !in_bounds {
                    PathfindCellType::Impassable
                } else {
                    match terrain.get_surface_type(world_x, world_y) {
                        SurfaceType::Water => PathfindCellType::Water,
                        SurfaceType::Cliff => PathfindCellType::Cliff,
                        SurfaceType::Bridge => PathfindCellType::Clear,
                        _ => PathfindCellType::Clear,
                    }
                };

                let pos = Coord3D::new(world_x, world_y, 0.0);
                if in_bounds {
                    if let Some(bridge) = terrain.find_bridge_at(&pos) {
                        if bridge.get_bridge_info().cur_damage_state == BodyDamageType::Rubble {
                            cell_type = PathfindCellType::BridgeImpassable;
                        }
                    }
                }

                self.inner.set_cell_type(&pos, cell_type);
            }
        }
    }

    fn bridge_layer_from_pathfinder_id(layer_id: u32) -> crate::path::PathfindLayerEnum {
        match layer_id {
            2 => crate::path::PathfindLayerEnum::Bridge1,
            3 => crate::path::PathfindLayerEnum::Bridge2,
            4 => crate::path::PathfindLayerEnum::Bridge3,
            5 => crate::path::PathfindLayerEnum::Bridge4,
            _ => crate::path::PathfindLayerEnum::Invalid,
        }
    }

    fn pathfinder_id_from_bridge_layer(layer: crate::path::PathfindLayerEnum) -> Option<u32> {
        match layer {
            crate::path::PathfindLayerEnum::Bridge1 => Some(2),
            crate::path::PathfindLayerEnum::Bridge2 => Some(3),
            crate::path::PathfindLayerEnum::Bridge3 => Some(4),
            crate::path::PathfindLayerEnum::Bridge4 => Some(5),
            _ => None,
        }
    }

    /// Register a bridge with the pathfinder and return the assigned terrain layer.
    ///
    /// C++ parity: `Pathfinder::addBridge()` returns the dynamic bridge layer used by
    /// `TerrainLogic::addBridgeToLogic()` / `addLandmarkBridgeToLogic()`.
    pub fn add_bridge(
        &mut self,
        bounds: (pathfind_complete::GridCoord, pathfind_complete::GridCoord),
    ) -> crate::path::PathfindLayerEnum {
        self.add_bridge_ex(bounds, INVALID_ID, bounds.0, bounds.1)
    }

    pub fn add_bridge_ex(
        &mut self,
        bounds: (pathfind_complete::GridCoord, pathfind_complete::GridCoord),
        bridge_object_id: ObjectID,
        start_cell: pathfind_complete::GridCoord,
        end_cell: pathfind_complete::GridCoord,
    ) -> crate::path::PathfindLayerEnum {
        let layer_id = self
            .inner
            .add_bridge_ex(bounds, bridge_object_id, start_cell, end_cell);
        Self::bridge_layer_from_pathfinder_id(layer_id)
    }

    /// Mirror C++ `Pathfinder::changeBridgeState(layer, repaired)`.
    pub fn change_bridge_state(&mut self, layer: crate::path::PathfindLayerEnum, repaired: bool) {
        let Some(layer_id) = Self::pathfinder_id_from_bridge_layer(layer) else {
            return;
        };
        self.inner.set_bridge_destroyed(layer_id, !repaired);
    }

    /// Inspect whether a registered bridge is currently destroyed.
    pub fn bridge_is_destroyed(&self, layer: crate::path::PathfindLayerEnum) -> Option<bool> {
        let layer_id = Self::pathfinder_id_from_bridge_layer(layer)?;
        self.inner
            .bridge_by_layer_id(layer_id)
            .map(|bridge| bridge.destroyed)
    }

    pub fn process_pathfind_queue(&mut self) -> Result<(), AiError> {
        self.inner.process_queue(PATHFIND_QUEUE_LEN);
        Ok(())
    }

    /// Check whether a world position lies on the wall layer footprint.
    pub fn is_point_on_wall(&self, pos: &Coord3D) -> bool {
        self.inner
            .get_cell_type(pos)
            .map(|cell_type| {
                matches!(
                    cell_type,
                    PathfindCellType::Obstacle | PathfindCellType::Impassable
                )
            })
            .unwrap_or(false)
    }

    /// Queue a pathfinding request (matches C++ Pathfinder::queueForPath).
    pub fn queue_for_path_request(&self, request: ClassicPathRequest) -> Result<(), String> {
        self.inner.queue_path_request(request)
    }

    pub fn find_path(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        acceptable_surfaces: u32,
        is_crusher: bool,
    ) -> Option<Vec<Coord3D>> {
        let request = ClassicPathRequest {
            object_id: 0,
            from: *from,
            to: *to,
            surfaces: acceptable_surfaces,
            is_crusher,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
        };
        let result = self.inner.find_path(request);
        if result.success {
            Some(result.waypoints)
        } else {
            None
        }
    }

    pub fn find_path_for_locomotor(
        &self,
        obj: ObjectID,
        locomotor_set: &crate::locomotor::LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
    ) -> Option<Vec<Coord3D>> {
        let locomotor = locomotor_set.get_default_locomotor()?;
        let Ok(loco_guard) = locomotor.lock() else {
            return None;
        };
        let mut surfaces = loco_guard.get_legal_surfaces();
        let mut is_crusher = false;
        let mut unit_radius = 0.0;
        let mut move_allies = false;
        let mut ignore_obstacle_id = None;

        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj) {
            if let Ok(obj_guard) = obj_arc.read() {
                if obj_guard.get_crusher_level() > 0 {
                    is_crusher = true;
                    surfaces |= crate::path::SURFACE_RUBBLE;
                }
                unit_radius = obj_guard.get_geometry_info().get_major_radius();
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        move_allies = ai_guard.get_can_path_through_units();
                        let ignored = ai_guard.get_ignored_obstacle_id();
                        if ignored != INVALID_ID {
                            ignore_obstacle_id = Some(ignored);
                        }
                    }
                }
            }
        }

        let request = ClassicPathRequest {
            object_id: obj,
            from: *from,
            to: *to,
            surfaces,
            is_crusher,
            unit_radius,
            allow_partial: false,
            move_allies,
            ignore_obstacle_id,
        };
        let result = self.inner.find_path(request);
        if result.success {
            Some(result.waypoints)
        } else {
            None
        }
    }

    pub fn find_path_result(&self, request: ClassicPathRequest) -> ClassicPathResult {
        self.inner.find_path(request)
    }

    pub fn find_closest_path_result(&self, request: ClassicPathRequest) -> ClassicPathResult {
        self.inner.find_closest_path(request)
    }

    pub fn find_safe_path_result(
        &self,
        request: ClassicPathRequest,
        repulsor_pos1: &Coord3D,
        repulsor_pos2: &Coord3D,
        repulsor_radius: f32,
    ) -> ClassicPathResult {
        self.inner
            .find_safe_path(request, repulsor_pos1, repulsor_pos2, repulsor_radius)
    }

    /// C++ Pathfinder::adjustToPossibleDestination — spiral search for a reachable cell.
    pub fn adjust_to_possible_destination(
        &self,
        start: &Coord3D,
        dest: &mut Coord3D,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
    ) -> bool {
        self.inner
            .adjust_to_possible_destination(start, dest, surfaces, is_crusher, unit_radius)
    }

    /// C++ `Pathfinder::findBrokenBridge` (AIPathfind.cpp).
    ///
    /// Compute effective terrain zones at from/to; if equal, no bridge is the
    /// blocker. Else scan destroyed pathfind layers whose `connectsZones` links
    /// zone1↔zone2 and return that layer's bridge object id.
    pub fn find_broken_bridge(
        &self,
        _locomotor_set: &crate::locomotor::LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
    ) -> Option<ObjectID> {
        // C++: zone compare via zone manager (not clientSafeQuickDoesPathExist —
        // that rejects cliffs/obstacles and is a different gate).
        // C++ m_layers[i].isDestroyed() && connectsZones(zone1, zone2).
        self.inner.find_broken_bridge_layer(from, to)
    }

    /// C++ `Pathfinder::clientSafeQuickDoesPathExist` — zone connectivity only.
    pub fn client_safe_quick_does_path_exist(
        &self,
        locomotor_set: &crate::locomotor::LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        let surfaces = locomotor_set.get_valid_surfaces();
        self.inner
            .client_safe_quick_does_path_exist(surfaces, from, to)
    }

    /// Zone-based quick path that ignores one obstacle for destination validity.
    ///
    /// C++ `clientSafeQuickDoesPathExist` has no ignore variant; this keeps the
    /// host helper but uses the same zone connectivity core (not A*).
    pub fn client_safe_quick_does_path_exist_with_ignore(
        &self,
        locomotor_set: &crate::locomotor::LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let surfaces = locomotor_set.get_valid_surfaces();
        if !self
            .inner
            .valid_movement_position(surfaces, false, to, ignore_obstacle_id)
        {
            return false;
        }
        if self.inner.get_cell_type(to) == Some(PathfindCellType::Cliff) {
            return false;
        }
        // Zone connectivity only — independent of the ignored unit.
        self.inner.zones_connected_for_surfaces(surfaces, from, to)
    }

    /// C++ `Pathfinder::clientSafeQuickDoesPathExistForUI` — terrain zones only.
    pub fn client_safe_quick_does_path_exist_for_ui(
        &self,
        locomotor_set: &crate::locomotor::LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        let surfaces = locomotor_set.get_valid_surfaces();
        self.inner
            .client_safe_quick_does_path_exist_for_ui(surfaces, from, to)
    }

    pub fn is_line_passable_for_surfaces(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: u32,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        self.inner
            .is_line_passable_for_surfaces(from, to, surfaces, ignore_obstacle_id)
    }

    /// Terrain/object line-of-sight check for attack states.
    /// Mirrors Pathfinder::isAttackViewBlockedByObstacle behavior used by AI state machines.
    pub fn is_attack_view_blocked_by_obstacle(
        &self,
        attacker: &Object,
        attacker_pos: &Coord3D,
        victim: Option<&Object>,
        victim_pos: &Coord3D,
    ) -> bool {
        let attack_uses_los = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.attack_uses_line_of_sight)
            })
            .unwrap_or(false);
        if !attack_uses_los {
            return false;
        }

        if attacker.is_kind_of(KindOf::Immobile) {
            return false;
        }
        if victim.is_some_and(|v| v.is_significantly_above_terrain()) {
            return false;
        }

        let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
            return false;
        };
        if !terrain.is_clear_line_of_sight(attacker_pos, victim_pos) {
            return true;
        }

        let surfaces = crate::path::SURFACE_GROUND
            | crate::path::SURFACE_WATER
            | crate::path::SURFACE_RUBBLE
            | crate::path::SURFACE_CLIFF;
        !self.is_line_passable_for_surfaces(
            attacker_pos,
            victim_pos,
            surfaces,
            Some(attacker.get_id()),
        )
    }

    pub fn valid_movement_position(
        &self,
        locomotor_set: &crate::locomotor::LocomotorSet,
        is_crusher: bool,
        pos: &Coord3D,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let Some(locomotor) = locomotor_set.get_default_locomotor() else {
            return false;
        };
        let Ok(loco_guard) = locomotor.lock() else {
            return false;
        };
        let mut surfaces = loco_guard.get_legal_surfaces();
        if is_crusher {
            surfaces |= crate::path::SURFACE_RUBBLE;
        }
        self.inner
            .valid_movement_position(surfaces, is_crusher, pos, ignore_obstacle_id)
    }

    pub fn valid_movement_position_for_surfaces(
        &self,
        mut surfaces: u32,
        is_crusher: bool,
        pos: &Coord3D,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        if is_crusher {
            surfaces |= crate::path::SURFACE_RUBBLE;
        }
        self.inner
            .valid_movement_position(surfaces, is_crusher, pos, ignore_obstacle_id)
    }

    #[cfg(test)]
    pub fn set_cell_type_for_test(&mut self, pos: &Coord3D, cell_type: PathfindCellType) {
        self.inner.set_cell_type(pos, cell_type);
    }

    pub fn snap_position(&self, pos: &Coord3D) -> Coord3D {
        self.inner.snap_position(pos)
    }

    pub fn set_goal_cells(
        &mut self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: pathfind_astar::PathfindLayerEnum,
        do_ground: bool,
        do_layer: bool,
    ) {
        self.inner.set_goal_cells(
            unit_id,
            center_cell,
            radius,
            center_in_cell,
            layer,
            do_ground,
            do_layer,
        );
    }

    pub fn clear_goal_cells(
        &mut self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: pathfind_astar::PathfindLayerEnum,
        clear_ground: bool,
        clear_layer: bool,
    ) {
        self.inner.clear_goal_cells(
            unit_id,
            center_cell,
            radius,
            center_in_cell,
            layer,
            clear_ground,
            clear_layer,
        );
    }

    pub fn set_aircraft_goal_cells(
        &mut self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.inner
            .set_aircraft_goal_cells(unit_id, center_cell, radius, center_in_cell);
    }

    pub fn clear_aircraft_goal_cells(
        &mut self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.inner
            .clear_aircraft_goal_cells(unit_id, center_cell, radius, center_in_cell);
    }

    pub fn add_object_to_map(
        &mut self,
        object_id: ObjectID,
        positions: &[Coord3D],
        is_fence: bool,
    ) {
        let _ = is_fence;
        let mut scratch = Vec::new();
        let footprint = if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
            if let Ok(obj_guard) = obj_arc.read() {
                object_footprint_positions(&obj_guard).map(|v| {
                    scratch = v;
                    scratch.as_slice()
                })
            } else {
                None
            }
        } else {
            None
        };
        let use_positions = footprint.unwrap_or(positions);
        for pos in use_positions {
            self.inner.set_cell_type(pos, PathfindCellType::Obstacle);
        }
        self.inner.refresh_pinched_for_positions(use_positions);
    }

    pub fn remove_object_from_map(&mut self, object_id: ObjectID, positions: &[Coord3D]) {
        let mut scratch = Vec::new();
        let footprint = if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
            if let Ok(obj_guard) = obj_arc.read() {
                object_footprint_positions(&obj_guard).map(|v| {
                    scratch = v;
                    scratch.as_slice()
                })
            } else {
                None
            }
        } else {
            None
        };
        let use_positions = footprint.unwrap_or(positions);
        for pos in use_positions {
            self.inner.set_cell_type(pos, PathfindCellType::Clear);
        }
        self.inner.refresh_pinched_for_positions(use_positions);
    }

    pub fn create_wall_from_object(&mut self, obj: &Object) {
        let pos = obj.get_position();
        let radius = obj
            .get_geometry_info()
            .get_major_radius()
            .max(self::pathfind_astar::PATHFIND_CELL_SIZE_F * 0.5);
        let cell_size = self::pathfind_astar::PATHFIND_CELL_SIZE_F;
        let center_x = (pos.x / cell_size) as i32;
        let center_y = (pos.y / cell_size) as i32;
        let radius_cells = (radius / cell_size).ceil() as i32;

        for dy in -radius_cells..=radius_cells {
            for dx in -radius_cells..=radius_cells {
                let cell_x = center_x + dx;
                let cell_y = center_y + dy;
                if cell_x < 0 || cell_y < 0 {
                    continue;
                }
                let world_x = (cell_x as f32 + 0.5) * cell_size;
                let world_y = (cell_y as f32 + 0.5) * cell_size;
                let delta_x = world_x - pos.x;
                let delta_y = world_y - pos.y;
                if (delta_x * delta_x + delta_y * delta_y) > radius * radius {
                    continue;
                }
                self.inner.set_cell_type(
                    &Coord3D::new(world_x, world_y, 0.0),
                    PathfindCellType::Obstacle,
                );
            }
        }
    }

    pub fn remove_wall_from_object(&mut self, obj: &Object) {
        let pos = obj.get_position();
        let radius = obj
            .get_geometry_info()
            .get_major_radius()
            .max(self::pathfind_astar::PATHFIND_CELL_SIZE_F * 0.5);
        let cell_size = self::pathfind_astar::PATHFIND_CELL_SIZE_F;
        let center_x = (pos.x / cell_size) as i32;
        let center_y = (pos.y / cell_size) as i32;
        let radius_cells = (radius / cell_size).ceil() as i32;

        for dy in -radius_cells..=radius_cells {
            for dx in -radius_cells..=radius_cells {
                let cell_x = center_x + dx;
                let cell_y = center_y + dy;
                if cell_x < 0 || cell_y < 0 {
                    continue;
                }
                let world_x = (cell_x as f32 + 0.5) * cell_size;
                let world_y = (cell_y as f32 + 0.5) * cell_size;
                let delta_x = world_x - pos.x;
                let delta_y = world_y - pos.y;
                if (delta_x * delta_x + delta_y * delta_y) > radius * radius {
                    continue;
                }
                self.inner.set_cell_type(
                    &Coord3D::new(world_x, world_y, 0.0),
                    PathfindCellType::Clear,
                );
            }
        }
    }

    pub fn is_line_clear_between(&self, from: &Coord3D, to: &Coord3D) -> bool {
        self.inner.is_line_clear_between(from, to)
    }
}

pub trait PartitionFilter {
    fn allow(&self, obj: ObjectId) -> bool;
    fn debug_get_name(&self) -> &str;
}

// Global AI instance - using once_cell for thread-safe singleton
pub static THE_AI: Lazy<Arc<RwLock<AI>>> = Lazy::new(|| Arc::new(RwLock::new(AI::new())));

// Core AI systems
pub mod ai_core; // Complete AI system integration
pub mod ai_update; // AI update interfaces and coordination

// AI behavior modules
pub mod dock;
pub mod formations; // Formation offset calculations for group movement
pub mod group;
pub mod guard;
pub mod guard_retaliate;

// AI player systems
pub mod ai_player; // Base AIPlayer implementation
pub mod skirmish_player; // Skirmish-specific AI

pub use self::group::AIGroup;
pub use self::group::GuardMode;

// Pathfinding modules
pub mod pathfind; // Legacy pathfinding
pub mod pathfinding_system; // Production pathfinding system

// NEW: Complete pathfinding system - faithful C++ port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
// These modules provide 100% C++ compatible pathfinding with all constants matching exactly
pub mod group_pathfinding;
pub mod path_optimization; // Path smoothing (AIPathfind.cpp:450-696)
pub mod pathfind_astar; // A* algorithm (AIPathfind.cpp:6438-6694)
pub mod pathfind_complete; // Complete system (all features integrated) // Group/formation pathfinding

#[cfg(test)]
mod pathfinding_tests;

// Legacy AIPlayer implementation superseded by ai_player.
pub mod squad;
pub mod states;
pub mod tn_guard;
pub mod turret;
pub mod turret_ai;

// AI Update Modules - Specialized AI behaviors for different unit types
pub mod modules;

// Module tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::PathfindLayerEnum as LegacyPathfindLayerEnum;

    #[test]
    fn test_ai_group_creation() {
        let mut ai = AI::new();
        let group = ai.create_group();

        assert!(group.read().unwrap().is_empty());
        assert_eq!(group.read().unwrap().get_count(), 0);
    }

    #[test]
    fn test_ai_group_membership() {
        let mut ai = AI::new();
        let group = ai.create_group();
        let obj_id = 42;

        {
            let mut g = group.write().unwrap();
            g.add(obj_id);
            assert!(g.is_member(obj_id));
            assert_eq!(g.get_count(), 1);
        }

        {
            let mut g = group.write().unwrap();
            let should_destroy = g.remove(obj_id);
            assert!(should_destroy);
            assert!(!g.is_member(obj_id));
            assert_eq!(g.get_count(), 0);
        }
    }

    #[test]
    fn test_ai_command_params() {
        let params =
            AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromPlayer);

        assert_eq!(params.cmd, AiCommandType::MoveToPosition);
        assert_eq!(params.cmd_source, CommandSourceType::FromPlayer);
        assert_eq!(params.int_value, 0);
    }

    #[test]
    fn test_skill_set_default() {
        let skill_set = SkillSet::default();
        assert_eq!(skill_set.num_skills, 0);
        assert_eq!(skill_set.skills.len(), MAX_AI_UPGRADES);
    }

    #[test]
    fn test_ai_data_side_info() {
        let mut ai_data = AiData::default();
        let side_info = AiSideInfo {
            side: "USA".to_string(),
            easy: 2,
            normal: 3,
            hard: 4,
            ..Default::default()
        };

        ai_data.add_side_info(side_info);
        assert_eq!(ai_data.side_info.len(), 1);
        assert_eq!(ai_data.side_info[0].side, "USA");
    }

    #[test]
    fn find_broken_bridge_prefers_destroyed_layers_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/mod.rs"));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("pub fn find_broken_bridge")
            .expect("findBrokenBridge");
        let end = prod[i..]
            .find(
                "
    pub fn ",
            )
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 2500));
        let w = &prod[i..end];
        assert!(
            w.contains("find_broken_bridge_layer")
                && !w.contains("get_first_bridge")
                && !w.contains("is_point_on_bridge")
                && !w.contains("client_safe_quick_does_path_exist"),
            "findBrokenBridge must check pathfinder destroyed layers only (no terrain residual)"
        );
        let complete = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/pathfind_complete.rs"
        ));
        assert!(
            complete.contains("fn connects_zones")
                && complete.contains("ground_connect_cells")
                && complete.contains("bridge_object_id"),
            "BridgeLayer must expose connectsZones + ground_connect_cells + object id"
        );
    }

    #[test]
    fn test_bridge_state_mapping_matches_cpp_boolean_semantics() {
        let mut pathfinder = Pathfinder::new();
        let layer = pathfinder.add_bridge((
            pathfind_complete::GridCoord::new(10, 10),
            pathfind_complete::GridCoord::new(20, 20),
        ));

        assert_eq!(layer, LegacyPathfindLayerEnum::Bridge1);
        assert_eq!(pathfinder.bridge_is_destroyed(layer), Some(false));

        pathfinder.change_bridge_state(layer, false);
        assert_eq!(pathfinder.bridge_is_destroyed(layer), Some(true));

        pathfinder.change_bridge_state(layer, true);
        assert_eq!(pathfinder.bridge_is_destroyed(layer), Some(false));
    }
}
