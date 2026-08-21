//! Script Action and Condition Executor
//!
//! This module implements the script execution engine matching C++ ScriptActions and ScriptConditions.
//! It provides the complete action and condition system for mission scripting.
//!
//! C++ Reference: ScriptActions.cpp, ScriptConditions.cpp
//! Functions: executeAction(), evaluateCondition()

//! Split into focused submodules by action/condition family.

use super::core::*;
use super::engine::{
    get_area_tracker, get_named_object_tracker, get_script_engine, with_script_engine_mut,
    with_script_engine_ref, TFade,
};
use crate::ai::integration::{with_ai_integration_mut, IntegratedAiPlayer};
use crate::ai::{
    AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, AttitudeType, GuardMode,
};
use crate::commands::{
    get_command_queue_manager, Command, CommandPriority, CommandType, QueuedCommand,
};
use crate::common::{
    AsciiString, Color, CommandSourceType, Coord3D, ObjectID, Relationship, WaypointID, INVALID_ID,
    LOGICFRAMES_PER_SECOND,
};
use crate::control_bar::{get_control_bar_bridge, set_command_set_slot_override};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{
    get_game_logic_random_value, get_game_logic_random_value_real, TheAudio, TheGameLogic,
    ThePartitionManager, TheVictoryConditions,
};
use crate::modules::AIAttitudeType;
use crate::object::behavior::auto_heal_behavior::parse_kind_of;
use crate::object::object_types::ObjectTypes;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::get_special_power_store;
use crate::object::update::special_power_update::SpecialPowerCommandOption;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::object_manager::get_object_manager;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::{player_list, PlayerType};
use crate::system::game_logic::TheObjectFactory;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::upgrade::center::get_upgrade_center;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::audio::AudioAffect as EngineAudioAffect;
use game_engine::common::global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{get_science_store, ScienceType, SCIENCE_INVALID};
use game_engine::common::system::radar::{get_radar_system, RadarEventType};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::cell::RefCell;

/// Live host drain: TEAM/NAMED move and attack when leftover `OBJECT_REGISTRY` is empty.
/// C++ `ScriptActions::doMoveToWaypoint` / `doNamedMoveToWaypoint` / `doAttack` /
/// `doNamedAttack` / `doNamedAttackArea` / `doNamedAttackTeam` / `doTeamAttackArea` /
/// `doTeamAttackNamed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptMoveAttackRequest {
    TeamMove { team: String, waypoint: String },
    NamedMove { unit: String, waypoint: String },
    TeamAttackTeam { attacker: String, victim: String },
    NamedAttackNamed { attacker: String, victim: String },
    NamedAttackArea { unit: String, area: String },
    NamedAttackTeam { unit: String, team: String },
    TeamAttackArea { team: String, area: String },
    TeamAttackNamed { team: String, unit: String },
}

/// Live host drain: CREATE_OBJECT family when leftover `OBJECT_REGISTRY` is empty.
/// C++ `ScriptActions::doCreateObject` / `createUnitOnTeamAt` / `doCreateReinforcements`.
#[derive(Debug, Clone, PartialEq)]
pub enum HostScriptCreateRequest {
    Object {
        name: Option<String>,
        thing: String,
        team: String,
        x: f32,
        y: f32,
        z: f32,
        angle: f32,
    },
    ReinforcementTeam {
        team: String,
        waypoint: String,
    },
}


thread_local! {
    static HOST_SKIRMISH_FIRE_SPECIAL_REQUESTS: RefCell<Vec<(String, String)>> =
        RefCell::new(Vec::new());
    static HOST_SKIRMISH_BUILD_REQUESTS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static HOST_SET_CAVE_INDEX_REQUESTS: RefCell<Vec<(String, i32)>> = RefCell::new(Vec::new());
    static HOST_OBJECT_PANEL_FLAG_REQUESTS: RefCell<Vec<(String, String, bool)>> =
        RefCell::new(Vec::new());
    static HOST_TEAM_PANEL_FLAG_REQUESTS: RefCell<Vec<(String, String, bool)>> =
        RefCell::new(Vec::new());
    static HOST_SCIENCE_ACTION_REQUESTS: RefCell<Vec<(String, String, bool)>> =
        RefCell::new(Vec::new());
    static HOST_TEAM_LOCO_SET_REQUESTS: RefCell<Vec<(String, String, Option<String>)>> =
        RefCell::new(Vec::new());
    static HOST_UNIT_LOCO_SET_REQUESTS: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_MOVE_ATTACK_REQUESTS: RefCell<Vec<HostScriptMoveAttackRequest>> =
        RefCell::new(Vec::new());
    static HOST_BUILD_TEAM_REQUESTS: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
    static HOST_RECRUIT_TEAM_REQUESTS: RefCell<Vec<(String, String, f32)>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_CREATE_REQUESTS: RefCell<Vec<HostScriptCreateRequest>> =
        RefCell::new(Vec::new());
    static HOST_TEAM_ATTITUDE_REQUESTS: RefCell<Vec<(String, i32)>> = RefCell::new(Vec::new());
    static HOST_AI_PLAYER_BUILD_SUPPLY_CENTER_REQUESTS: RefCell<Vec<(String, String, i32)>> =
        RefCell::new(Vec::new());
    static HOST_AI_PLAYER_BUILD_UPGRADE_REQUESTS: RefCell<Vec<(String, String)>> =
        RefCell::new(Vec::new());
    static HOST_AI_PLAYER_BUILD_TYPE_NEAREST_TEAM_REQUESTS: RefCell<Vec<(String, String, String)>> =
        RefCell::new(Vec::new());
    static HOST_GUARD_SUPPLY_CENTER_REQUESTS: RefCell<Vec<(String, i32)>> =
        RefCell::new(Vec::new());
    static HOST_SKIRMISH_ATTACK_GROUP_REQUESTS: RefCell<Vec<(String, i32, i32)>> =
        RefCell::new(Vec::new());
    static HOST_SKIRMISH_CMD_BUTTON_REQUESTS: RefCell<Vec<(String, String, f32)>> =
        RefCell::new(Vec::new());




}

/// Live host drain: `SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST` when crate
/// `OBJECT_REGISTRY` is empty (Wave 284 leftover no-op).
pub fn request_host_skirmish_fire_special(player_name: &str, power_name: &str) {
    HOST_SKIRMISH_FIRE_SPECIAL_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((player_name.to_string(), power_name.to_string()));
    });
}

pub fn take_host_skirmish_fire_special_requests() -> Vec<(String, String)> {
    HOST_SKIRMISH_FIRE_SPECIAL_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `SKIRMISH_BUILD_BUILDING` → `markPriorityBuild`.
pub fn request_host_skirmish_build_building(thing_name: &str) {
    HOST_SKIRMISH_BUILD_REQUESTS.with(|q| {
        q.borrow_mut().push(thing_name.to_string());
    });
}

pub fn take_host_skirmish_build_requests() -> Vec<String> {
    HOST_SKIRMISH_BUILD_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `SET_CAVE_INDEX` → `CaveContain::tryToSetCaveIndex`.
/// Leftover crate objects are empty on the player path.
pub fn request_host_set_cave_index(cave_name: &str, cave_index: i32) {
    HOST_SET_CAVE_INDEX_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((cave_name.to_string(), cave_index));
    });
}

/// Live host drain: UNIT_AFFECT_OBJECT_PANEL_FLAGS when leftover registry is empty.
pub fn request_host_object_panel_flag(unit_name: &str, flag_name: &str, enable: bool) {
    HOST_OBJECT_PANEL_FLAG_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((unit_name.to_string(), flag_name.to_string(), enable));
    });
}

pub fn take_host_object_panel_flag_requests() -> Vec<(String, String, bool)> {
    HOST_OBJECT_PANEL_FLAG_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM_AFFECT_OBJECT_PANEL_FLAGS when leftover teams are empty.
pub fn request_host_team_panel_flag(team_name: &str, flag_name: &str, enable: bool) {
    HOST_TEAM_PANEL_FLAG_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((team_name.to_string(), flag_name.to_string(), enable));
    });
}

pub fn take_host_team_panel_flag_requests() -> Vec<(String, String, bool)> {
    HOST_TEAM_PANEL_FLAG_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}


pub fn take_host_set_cave_index_requests() -> Vec<(String, i32)> {
    HOST_SET_CAVE_INDEX_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: PLAYER_GRANT_SCIENCE / PLAYER_PURCHASE_SCIENCE.
/// `grant == true` → grantScience; false → attemptToPurchaseScience.
pub fn request_host_science_action(player_name: &str, science_name: &str, grant: bool) {
    HOST_SCIENCE_ACTION_REQUESTS.with(|q| {
        q.borrow_mut().push((
            player_name.to_string(),
            science_name.to_string(),
            grant,
        ));
    });
}

pub fn take_host_science_action_requests() -> Vec<(String, String, bool)> {
    HOST_SCIENCE_ACTION_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM_PANIC / TEAM_WANDER / TEAM_WANDER_IN_PLACE.
/// Leftover OBJECT_REGISTRY is empty on the player path.
pub fn request_host_team_loco_set(team_name: &str, set: &str, waypoint: Option<&str>) {
    HOST_TEAM_LOCO_SET_REQUESTS.with(|q| {
        q.borrow_mut().push((
            team_name.to_string(),
            set.to_string(),
            waypoint.map(str::to_string),
        ));
    });
}

pub fn take_host_team_loco_set_requests() -> Vec<(String, String, Option<String>)> {
    HOST_TEAM_LOCO_SET_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: named-unit panic residual (C++ AIPanicState via TEAM_PANIC members).
pub fn request_host_unit_loco_set(unit_name: &str, set: &str) {
    HOST_UNIT_LOCO_SET_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((unit_name.to_string(), set.to_string()));
    });
}

pub fn take_host_unit_loco_set_requests() -> Vec<(String, String)> {
    HOST_UNIT_LOCO_SET_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM/NAMED move and attack (`CMD_FROM_SCRIPT`).
pub fn request_host_script_move_attack(req: HostScriptMoveAttackRequest) {
    HOST_SCRIPT_MOVE_ATTACK_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_move_attack_requests() -> Vec<HostScriptMoveAttackRequest> {
    HOST_SCRIPT_MOVE_ATTACK_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `BUILD_TEAM` → `AIPlayer::buildSpecificAITeam`.
pub fn request_host_build_team(owner_name: &str, team_name: &str) {
    HOST_BUILD_TEAM_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((owner_name.to_string(), team_name.to_string()));
    });
}

pub fn take_host_build_team_requests() -> Vec<(String, String)> {
    HOST_BUILD_TEAM_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `RECRUIT_TEAM` → `AIPlayer::recruitSpecificAITeam`.
pub fn request_host_recruit_team(owner_name: &str, team_name: &str, recruit_radius: f32) {
    HOST_RECRUIT_TEAM_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((owner_name.to_string(), team_name.to_string(), recruit_radius));
    });
}

pub fn take_host_recruit_team_requests() -> Vec<(String, String, f32)> {
    HOST_RECRUIT_TEAM_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: CREATE_OBJECT / named-at-waypoint / reinforcements.
pub fn request_host_script_create(req: HostScriptCreateRequest) {
    HOST_SCRIPT_CREATE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_create_requests() -> Vec<HostScriptCreateRequest> {
    HOST_SCRIPT_CREATE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM_SET_ATTITUDE → `set_team_attitude_by_name`.
pub fn request_host_team_attitude(team_name: &str, mood: i32) {
    HOST_TEAM_ATTITUDE_REQUESTS.with(|q| {
        q.borrow_mut().push((team_name.to_string(), mood));
    });
}

pub fn take_host_team_attitude_requests() -> Vec<(String, i32)> {
    HOST_TEAM_ATTITUDE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `AI_PLAYER_BUILD_SUPPLY_CENTER` → `AIPlayer::buildBySupplies`.
pub fn request_host_ai_player_build_supply_center(
    player_name: &str,
    thing_name: &str,
    minimum_cash: i32,
) {
    HOST_AI_PLAYER_BUILD_SUPPLY_CENTER_REQUESTS.with(|q| {
        q.borrow_mut().push((
            player_name.to_string(),
            thing_name.to_string(),
            minimum_cash,
        ));
    });
}

pub fn take_host_ai_player_build_supply_center_requests() -> Vec<(String, String, i32)> {
    HOST_AI_PLAYER_BUILD_SUPPLY_CENTER_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `AI_PLAYER_BUILD_UPGRADE` → `AIPlayer::buildUpgrade`.
pub fn request_host_ai_player_build_upgrade(player_name: &str, upgrade_name: &str) {
    HOST_AI_PLAYER_BUILD_UPGRADE_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((player_name.to_string(), upgrade_name.to_string()));
    });
}

pub fn take_host_ai_player_build_upgrade_requests() -> Vec<(String, String)> {
    HOST_AI_PLAYER_BUILD_UPGRADE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `AI_PLAYER_BUILD_TYPE_NEAREST_TEAM` →
/// `AIPlayer::buildSpecificBuildingNearestTeam`.
pub fn request_host_ai_player_build_type_nearest_team(
    player_name: &str,
    thing_name: &str,
    team_name: &str,
) {
    HOST_AI_PLAYER_BUILD_TYPE_NEAREST_TEAM_REQUESTS.with(|q| {
        q.borrow_mut().push((
            player_name.to_string(),
            thing_name.to_string(),
            team_name.to_string(),
        ));
    });
}

pub fn take_host_ai_player_build_type_nearest_team_requests() -> Vec<(String, String, String)> {
    HOST_AI_PLAYER_BUILD_TYPE_NEAREST_TEAM_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: `TEAM_GUARD_SUPPLY_CENTER` → `AIPlayer::guardSupplyCenter`.
pub fn request_host_guard_supply_center(team_name: &str, min_supplies: i32) {
    HOST_GUARD_SUPPLY_CENTER_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((team_name.to_string(), min_supplies));
    });
}

pub fn take_host_guard_supply_center_requests() -> Vec<(String, i32)> {
    HOST_GUARD_SUPPLY_CENTER_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE.
pub fn request_host_skirmish_attack_nearest_group(team: &str, comparison: i32, value: i32) {
    HOST_SKIRMISH_ATTACK_GROUP_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((team.to_string(), comparison, value));
    });
}

pub fn take_host_skirmish_attack_nearest_group_requests() -> Vec<(String, i32, i32)> {
    HOST_SKIRMISH_ATTACK_GROUP_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT.
pub fn request_host_skirmish_command_button_most_valuable(
    team: &str,
    ability: &str,
    range: f32,
) {
    HOST_SKIRMISH_CMD_BUTTON_REQUESTS.with(|q| {
        q.borrow_mut()
            .push((team.to_string(), ability.to_string(), range));
    });
}

pub fn take_host_skirmish_command_button_most_valuable_requests() -> Vec<(String, String, f32)> {
    HOST_SKIRMISH_CMD_BUTTON_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}





/// Wave 284: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

fn to_radar_coord(pos: &Coord3D) -> game_engine::common::system::radar::Coord3D {
    game_engine::common::system::radar::Coord3D::new(pos.x, pos.y, pos.z)
}

static TRANSPORT_STATUSES: Lazy<RwLock<HashMap<ObjectID, (u32, usize)>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
static SCRIPT_TEMP_GROUP_ID: AtomicU32 = AtomicU32::new(1);

/// Take an owned handler snapshot before crossing into host/UI code.
///
/// Script actions can run from `ScriptEngine::update`, which installs the
/// active lexical engine. A global ScriptEngine lock is therefore not
/// available there. More importantly, host callbacks may synchronously enter
/// script execution again, so the scoped engine access must end before the
/// callback is invoked.
fn current_script_action_handler() -> Option<Arc<dyn crate::scripting::engine::ScriptActionHandler>>
{
    with_script_engine_ref(|script_engine| script_engine.action_handler()).flatten()
}

/// Script execution error
#[derive(Debug, Clone)]
pub enum ScriptError {
    /// Parameter not found
    ParameterNotFound(String),
    /// Invalid parameter type
    InvalidParameterType(String),
    /// Team not found
    TeamNotFound(String),
    /// Player not found
    PlayerNotFound(String),
    /// Object not found
    ObjectNotFound(String),
    /// Action execution failed
    ExecutionFailed(String),
    /// Condition evaluation failed
    EvaluationFailed(String),
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptError::ParameterNotFound(s) => write!(f, "Parameter not found: {}", s),
            ScriptError::InvalidParameterType(s) => write!(f, "Invalid parameter type: {}", s),
            ScriptError::TeamNotFound(s) => write!(f, "Team not found: {}", s),
            ScriptError::PlayerNotFound(s) => write!(f, "Player not found: {}", s),
            ScriptError::ObjectNotFound(s) => write!(f, "Object not found: {}", s),
            ScriptError::ExecutionFailed(s) => write!(f, "Execution failed: {}", s),
            ScriptError::EvaluationFailed(s) => write!(f, "Evaluation failed: {}", s),
        }
    }
}

impl std::error::Error for ScriptError {}

/// Script action execution result
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptActionResult {
    /// Action completed successfully
    Success,
    /// Action is pending completion (frames remaining)
    Pending(f32),
    /// Action failed with error message
    Failed(String),
}

/// Script condition evaluation result
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptConditionResult {
    /// Condition is true
    True,
    /// Condition is false
    False,
    /// Condition evaluation error
    Error(String),
}

/// Script execution context
///
/// C++ Reference: ScriptActions class member variables
/// This provides access to all game systems needed for script execution
pub struct ScriptContext {
    // Game system references (reserved for tighter integration points)
    pub game_logic_id: u32,
    pub object_manager_id: u32,
    pub player_manager_id: u32,
    pub event_system_id: u32,
    pub camera_system_id: u32,
    pub audio_system_id: u32,
    pub partition_manager_id: u32,
    pub special_powers_id: u32,

    // Runtime state
    pub current_frame: u32,
    pub suppress_new_windows: bool,
}

impl ScriptContext {
    pub fn new() -> Self {
        Self {
            game_logic_id: 0,
            object_manager_id: 0,
            player_manager_id: 0,
            event_system_id: 0,
            camera_system_id: 0,
            audio_system_id: 0,
            partition_manager_id: 0,
            special_powers_id: 0,
            current_frame: TheGameLogic::get_frame(),
            suppress_new_windows: false,
        }
    }
}

/// Script action dispatcher
///
/// C++ Reference: ScriptActions::executeAction()
/// This is the main entry point for executing script actions
pub struct ScriptActionDispatcher {
    context: Arc<RwLock<ScriptContext>>,
}

impl ScriptActionDispatcher {
    pub fn new(context: Arc<RwLock<ScriptContext>>) -> Self {
        Self { context }
    }
}

/// Script condition evaluator
///
/// C++ Reference: ScriptConditions::evaluateCondition()
/// This evaluates script conditions to determine script flow
#[allow(dead_code)]
pub struct ScriptConditionEvaluator {
    context: Arc<RwLock<ScriptContext>>,
}

impl ScriptConditionEvaluator {
    pub fn new(context: Arc<RwLock<ScriptContext>>) -> Self {
        Self { context }
    }
}

mod actions_attack_priority;
mod actions_camera;
mod actions_garrison;
mod actions_input_ui;
mod actions_named;
mod actions_player;
mod actions_player_display_camera;
mod actions_skirmish;
mod actions_team_build;
mod actions_team_command;
mod actions_team_relations;
mod actions_victory_team;
mod actions_world;
mod dispatch;
mod eval_basic;
mod eval_named;
mod eval_skirmish;

#[cfg(test)]
mod tests;

/// Concatenated live sources for residual `include_str!` scans.
pub const EXECUTOR_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("actions_attack_priority.rs"),
    include_str!("actions_camera.rs"),
    include_str!("actions_garrison.rs"),
    include_str!("actions_input_ui.rs"),
    include_str!("actions_named.rs"),
    include_str!("actions_player.rs"),
    include_str!("actions_player_display_camera.rs"),
    include_str!("actions_skirmish.rs"),
    include_str!("actions_team_build.rs"),
    include_str!("actions_team_command.rs"),
    include_str!("actions_team_relations.rs"),
    include_str!("actions_victory_team.rs"),
    include_str!("actions_world.rs"),
    include_str!("dispatch.rs"),
    include_str!("eval_basic.rs"),
    include_str!("eval_named.rs"),
    include_str!("eval_skirmish.rs"),
);
