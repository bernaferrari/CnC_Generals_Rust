//! Script Action and Condition Executor
//!
//! This module implements the script execution engine matching C++ ScriptActions and ScriptConditions.
//! It provides the complete action and condition system for mission scripting.
//!
//! C++ Reference: ScriptActions.cpp, ScriptConditions.cpp
//! Functions: executeAction(), evaluateCondition()

//! Split into focused submodules by action/condition family.

use super::core::*;
use std::sync::atomic::Ordering;
use super::engine::{
    CanonicalScriptExecutionDriver, ScriptExecutionDriver, TFade, get_area_tracker,
    get_named_object_tracker,
};
use crate::ai::integration::{IntegratedAiPlayer, with_ai_integration_mut};
use crate::ai::{
    AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, AttitudeType, GuardMode,
};
use crate::commands::{
    Command, CommandPriority, CommandType, QueuedCommand, get_command_queue_manager,
};
use crate::common::{
    AsciiString, Color, CommandSourceType, Coord3D, INVALID_ID, LOGICFRAMES_PER_SECOND, ObjectID,
    Relationship, WaypointID,
};
use crate::control_bar::{get_control_bar_bridge, set_command_set_slot_override};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{
    TheAudio, TheGameLogic, ThePartitionManager, TheVictoryConditions, get_game_logic_random_value,
    get_game_logic_random_value_real,
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
use crate::player::{PlayerType, player_list};
use crate::system::game_logic::TheObjectFactory;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::upgrade::center::get_upgrade_center;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::audio::AudioAffect as EngineAudioAffect;
use game_engine::common::global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType, get_science_store};
use game_engine::common::system::radar::{RadarEventType, get_radar_system};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Live host drain: team/named attacks when leftover `OBJECT_REGISTRY` is empty.
/// NamedMove and TeamMove are synchronously admitted by their receiving driver.
/// C++ `ScriptActions::doAttack` / `doNamedAttack` / `doNamedAttackArea` /
/// `doNamedAttackTeam` / `doTeamAttackArea` /
/// `doTeamAttackNamed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptMoveAttackRequest {
    TeamAttackTeam {
        attacker: String,
        victim: String,
    },
    NamedAttackNamed {
        attacker: String,
        victim: String,
    },
    NamedAttackArea {
        unit: String,
        area: String,
    },
    NamedAttackTeam {
        unit: String,
        team: String,
    },
    TeamAttackArea {
        team: String,
        area: String,
    },
    TeamAttackNamed {
        team: String,
        unit: String,
    },
    /// C++ `doMoveUnitTowardsNearest` — closest template/ObjectTypes in trigger.
    NamedMoveTowardsNearest {
        unit: String,
        object_type: String,
        trigger: String,
    },
    /// C++ `doMoveTeamTowardsNearest` — same scan from team estimate, every member.
    TeamMoveTowardsNearest {
        team: String,
        object_type: String,
        trigger: String,
    },
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

/// Live host drain: NAMED HUNT, TEAM/NAMED GUARD, PLAYER_HUNT,
/// TEAM_HUNT_WITH_COMMAND_BUTTON. TEAM_HUNT is synchronously dispatched to the
/// receiving owner because its contextual team IDs must remain in scope.
/// C++ `ScriptActions::doNamedHunt` / `doTeamGuard` / `doNamedGuard` /
/// `doPlayerHunt` / `doTeamHuntWithCommandButton`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptHuntGuardRequest {
    NamedHunt { unit: String },
    TeamGuard { team: String },
    NamedGuard { unit: String },
    PlayerHunt { player: String },
    TeamHuntWithCommandButton { team: String, button: String },
}

/// Live host drain: TEAM/NAMED FOLLOW_WAYPOINTS and EXACT variants.
/// C++ `ScriptActions::doNamedFollowWaypoints` / `doNamedFollowWaypointsExact` /
/// `doTeamFollowWaypoints` / `doTeamFollowWaypointsExact`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptFollowWaypointsRequest {
    TeamFollow {
        team: String,
        waypoint: String,
        as_team: bool,
        exact: bool,
    },
    NamedFollow {
        unit: String,
        waypoint: String,
        exact: bool,
    },
}

/// Live host drain: TEAM_GUARD_POSITION / OBJECT / AREA / TUNNEL_NETWORK.
/// C++ `ScriptActions::doTeamGuardPosition` / `doTeamGuardObject` /
/// `doTeamGuardArea` / `doTeamGuardInTunnelNetwork`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptGuardVariantRequest {
    TeamGuardPosition { team: String, waypoint: String },
    TeamGuardObject { team: String, unit: String },
    TeamGuardArea { team: String, area: String },
    TeamGuardTunnel { team: String },
}

/// Live host drain: NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT / AT_NAMED.
/// C++ `ScriptActions::doNamedFireSpecialPowerAtWaypoint` /
/// `doNamedFireSpecialPowerAtNamed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptNamedFireSpecialPowerRequest {
    AtWaypoint {
        unit: String,
        power: String,
        waypoint: String,
    },
    AtNamed {
        unit: String,
        power: String,
        target: String,
    },
}

/// Live host drain: NAMED_STOP / TEAM_STOP / TEAM_STOP_AND_DISBAND.
/// C++ `ScriptActions::doNamedStop` / `doTeamStop` (`aiIdle` / `groupIdle`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptIdleRequest {
    NamedStop {
        unit: String,
    },
    TeamStop {
        team: String,
        disband: bool,
    },
    /// C++ `doIdleAllPlayerUnits` — empty player = every human player.
    IdleAll {
        player: String,
    },
    /// C++ `doResumeSupplyTruckingForIdleUnits`.
    ResumeSupply {
        player: String,
    },
}

/// Live host drain: NAMED/TEAM DELETE, KILL, DAMAGE.
/// C++ `ScriptActions::doNamedDelete` / `doNamedKill` / `doNamedDamage` /
/// `doTeamDelete` / `doTeamKill` / `doDamageTeamMembers`.
#[derive(Debug, Clone, PartialEq)]
pub enum HostScriptKillDeleteDamageRequest {
    NamedDelete {
        unit: String,
    },
    NamedKill {
        unit: String,
    },
    NamedDamage {
        unit: String,
        amount: i32,
    },
    TeamDelete {
        team: String,
        ignore_dead: bool,
    },
    TeamKill {
        team: String,
    },
    TeamDamage {
        team: String,
        amount: f32,
    },
    /// C++ `doDestroyAllContained` (`iterateContained(killTheObject)`).
    DestroyAllContained {
        unit: String,
    },
}

/// Synchronous Core-to-host script action payload for named sound actions.
/// Main consumes this only through its receiving `ScriptExecutionDriver`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptObjectSoundRequest {
    PlayNamed { sound: String, unit: String },
    Enable { unit: String, enable: bool },
}

/// Live host drain: NAMED/TEAM FACE_NAMED / FACE_WAYPOINT.
/// C++ `ScriptActions::doNamedFaceNamed` / `doNamedFaceWaypoint` /
/// `doTeamFaceNamed` / `doTeamFaceWaypoint`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptFaceRequest {
    NamedFaceNamed { unit: String, target: String },
    NamedFaceWaypoint { unit: String, waypoint: String },
    TeamFaceNamed { team: String, target: String },
    TeamFaceWaypoint { team: String, waypoint: String },
}

/// Synchronous owner request: PLAYER_SET_MONEY / PLAYER_GIVE_MONEY.
/// C++ `ScriptActions::doSetMoney` / `doGiveMoney`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptMoneyRequest {
    Set { player: String, amount: i32 },
    Give { player: String, amount: i32 },
}

/// Live host drain: PLAYER_RELATES_PLAYER.
/// C++ `ScriptActions::updatePlayerRelationTowardPlayer` (`setPlayerRelationship`).
/// Leftover writes leftover ThePlayerList; live relations live on host Player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptPlayerRelatesRequest {
    pub source: String,
    pub dest: String,
    pub relationship: Relationship,
}

/// Live host drain: NAMED_SET_BOOBYTRAPPED / TEAM_SET_BOOBYTRAPPED.
/// C++ `ScriptActions::doNamedSetBoobytrapped` / `doTeamSetBoobytrapped`
/// (`TheThingFactory->newObject` + `StickyBombUpdate::initStickyBomb`).
/// Leftover attaches via empty `OBJECT_REGISTRY`; live units live on host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptBoobytrapRequest {
    Named { thing: String, unit: String },
    Team { thing: String, team: String },
}

/// Live host drain: NAMED/TEAM SET_UNMANNED_STATUS / DELETE_ALL_UNMANNED.
/// C++ `ScriptActions::doNamedSetUnmanned` / `doTeamSetUnmanned` /
/// `deleteAllUnmanned` (`DISABLED_UNMANNED` + Neutral team / destroyObject).
/// Leftover walks empty `OBJECT_REGISTRY`; live husks live on host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptUnmannedRequest {
    Named { unit: String },
    Team { team: String },
    DeleteAll,
}

/// Live host drain: OBJECT/TEAM CREATE_RADAR_EVENT.
/// C++ `ScriptActions::doObjectRadarCreateEvent` / `doTeamRadarCreateEvent`
/// (`TheRadar->createEvent` at named unit or team estimate position).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptRadarEventRequest {
    Object { unit: String, event_type: i32 },
    Team { team: String, event_type: i32 },
}

/// Live host drain: NAMED/TEAM SET_STEALTH_ENABLED.
/// C++ `ScriptActions::doNamedEnableStealth` / `doTeamEnableStealth`
/// (`setScriptStatus(OBJECT_STATUS_SCRIPT_UNSTEALTHED, !enabled)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptStealthEnabledRequest {
    Named { unit: String, enabled: bool },
    Team { team: String, enabled: bool },
}

/// Live host drain: TECHTREE_MODIFY_BUILDABILITY_OBJECT.
/// C++ `ScriptActions::doModifyBuildableStatus` → `setBuildableStatusOverride`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptBuildableOverrideRequest {
    pub template: String,
    pub status: i32,
}

/// Live host drain: NAMED_RECEIVE_UPGRADE.
/// C++ `ScriptActions::doUnitReceiveUpgrade` (`giveUpgrade`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptNamedUpgradeRequest {
    pub unit: String,
    pub upgrade: String,
}

/// Live host drain: NAMED/TEAM FLASH / FLASH_WHITE.
/// C++ `doNamedFlash` / `doTeamFlash` (`setFlashColor` / `setFlashCount`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptFlashRequest {
    Named {
        unit: String,
        seconds: i32,
        white: bool,
    },
    Team {
        team: String,
        seconds: i32,
        white: bool,
    },
}

/// Live host drain: NAMED/TEAM SET_EMOTICON.
/// C++ `doNamedEmoticon` / `doTeamEmoticon` (`Drawable::setEmoticon`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptEmoticonRequest {
    Named {
        unit: String,
        emoticon: String,
        duration_frames: i32,
    },
    Team {
        team: String,
        emoticon: String,
        duration_frames: i32,
    },
}

/// Live host drain: NAMED_SET_HELD.
/// C++ `doNamedSetHeld` (`DISABLED_HELD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptHeldRequest {
    pub unit: String,
    pub held: bool,
}

/// Live host drain: NAMED_CUSTOM_COLOR.
/// C++ `doNamedCustomColor` (`setCustomIndicatorColor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptCustomColorRequest {
    pub unit: String,
    pub color_raw: u32,
}

/// Live host drain: NAMED_SET_ATTITUDE.
/// C++ `updateNamedSetAttitude` (`ai->setAttitude`). TEAM already has a drain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptNamedAttitudeRequest {
    pub unit: String,
    pub mood: i32,
}

/// Live host drain: NAMED/TEAM SET_REPULSOR.
/// C++ `doNamedSetRepulsor` / `doTeamSetRepulsor` (`OBJECT_STATUS_REPULSOR`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptRepulsorRequest {
    Named { unit: String, enabled: bool },
    Team { team: String, enabled: bool },
}

/// Live host drain: NAMED_SET_STOPPING_DISTANCE / SET_STOPPING_DISTANCE.
/// C++ `setCloseEnoughDist`.
#[derive(Debug, Clone, PartialEq)]
pub enum HostScriptStoppingDistanceRequest {
    Named { unit: String, distance: f32 },
    Team { team: String, distance: f32 },
}

/// Live host drain: OBJECT_FORCE_SELECT.
/// C++ `doForceObjectSelection` (`selectDrawable` + optional `moveCameraTo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptForceSelectRequest {
    pub team: String,
    pub object_type: String,
    pub center_in_view: bool,
    pub audio: String,
}

/// Live host drain: PLAYER_SELL_EVERYTHING / REPAIR_NAMED_STRUCTURE /
/// EXCLUDE_FROM_SCORE_SCREEN / SELECT_SKILLSET / PLAYER_KILL.
/// C++ `sellEverythingUnderTheSun` / `repairStructure` /
/// `setListInScoreScreen(false)` / `friend_setSkillset` / `doPlayerKill`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptPlayerMiscRequest {
    SellEverything {
        player: String,
    },
    RepairNamed {
        player: String,
        structure: String,
    },
    ExcludeFromScore {
        player: String,
    },
    SelectSkillset {
        player: String,
        skillset: i32,
    },
    /// C++ `ScriptActions::doPlayerKill` → `Player::killPlayer`.
    Kill {
        player: String,
    },
}

/// Live host drain: NAMED/TEAM USE_COMMANDBUTTON_ABILITY.
/// C++ `doNamedUseCommandButtonAbility*` / `doTeamUseCommandButtonAbility*` /
/// `doTeamUseCommandButtonOnNearest*` / `doTeamPartialUseCommandButton`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptUseCommandButtonRequest {
    Named {
        unit: String,
        button: String,
    },
    NamedOnNamed {
        unit: String,
        button: String,
        target: String,
    },
    NamedAtWaypoint {
        unit: String,
        button: String,
        waypoint: String,
    },
    NamedUsingWaypointPath {
        unit: String,
        button: String,
        path: String,
    },
    Team {
        team: String,
        button: String,
    },
    TeamOnNamed {
        team: String,
        button: String,
        target: String,
    },
    TeamAtWaypoint {
        team: String,
        button: String,
        waypoint: String,
    },
    TeamOnNearestEnemy {
        team: String,
        button: String,
    },
    TeamOnNearestGarrisonedBuilding {
        team: String,
        button: String,
    },
    TeamOnNearestKindof {
        team: String,
        button: String,
        kindof: String,
    },
    TeamOnNearestEnemyBuilding {
        team: String,
        button: String,
    },
    TeamOnNearestEnemyBuildingClass {
        team: String,
        button: String,
        kindof: String,
    },
    TeamOnNearestObjectType {
        team: String,
        button: String,
        object_type: String,
    },
}

/// Live host drain: TEAM_PARTIAL_USE_COMMANDBUTTON (`percentage/100 * count`).
#[derive(Debug, Clone, PartialEq)]
pub struct HostScriptTeamPartialCommandButtonRequest {
    pub team: String,
    pub button: String,
    pub percentage: f32,
}

/// Live host drain: SKIRMISH_FOLLOW/MOVE_TO_APPROACH_PATH.
/// C++ `doTeamFollowSkirmishApproachPath` / `doTeamMoveToSkirmishApproachPath`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptSkirmishApproachPathRequest {
    pub team: String,
    pub path_label: String,
    pub as_team: bool,
    pub follow: bool,
}

/// Live host drain: SKIRMISH_BUILD_BASE_DEFENSE_* / SKIRMISH_BUILD_STRUCTURE_*.
/// C++ `doBuildBaseDefense` / `doBuildBaseStructure`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostScriptSkirmishBaseDefenseRequest {
    pub player: String,
    pub structure: Option<String>,
    pub flank: bool,
}

/// Per-world host-script request drains (formerly process `thread_local!` queues).
/// Owned by `WorldServices` so parallel instances never share script side-effects.
pub struct HostScriptRequestQueues {
    pub(crate) skirmish_fire_special_requests: Mutex<Vec<(String, String)>>,
    pub(crate) skirmish_build_requests: Mutex<Vec<(String, String)>>,
    pub(crate) set_cave_index_requests: Mutex<Vec<(String, i32)>>,
    pub(crate) object_panel_flag_requests: Mutex<Vec<(String, String, bool)>>,
    pub(crate) team_panel_flag_requests: Mutex<Vec<(String, String, bool)>>,
    pub(crate) science_action_requests: Mutex<Vec<(String, String, bool)>>,
    pub(crate) script_move_attack_requests: Mutex<Vec<HostScriptMoveAttackRequest>>,
    pub(crate) build_team_requests: Mutex<Vec<(String, String)>>,
    pub(crate) recruit_team_requests: Mutex<Vec<(String, String, f32)>>,
    pub(crate) script_create_requests: Mutex<Vec<HostScriptCreateRequest>>,
    pub(crate) team_attitude_requests: Mutex<Vec<(String, i32)>>,
    pub(crate) ai_player_build_supply_center_requests: Mutex<Vec<(String, String, i32)>>,
    pub(crate) ai_player_build_upgrade_requests: Mutex<Vec<(String, String)>>,
    pub(crate) ai_player_build_type_nearest_team_requests: Mutex<Vec<(String, String, String)>>,
    pub(crate) guard_supply_center_requests: Mutex<Vec<(String, i32)>>,
    pub(crate) skirmish_attack_group_requests: Mutex<Vec<(String, i32, i32)>>,
    pub(crate) skirmish_cmd_button_requests: Mutex<Vec<(String, String, f32)>>,
    pub(crate) script_hunt_guard_requests: Mutex<Vec<HostScriptHuntGuardRequest>>,
    pub(crate) script_follow_waypoints_requests: Mutex<Vec<HostScriptFollowWaypointsRequest>>,
    pub(crate) script_guard_variant_requests: Mutex<Vec<HostScriptGuardVariantRequest>>,
    pub(crate) script_named_fire_special_requests: Mutex<Vec<HostScriptNamedFireSpecialPowerRequest>>,
    pub(crate) script_idle_requests: Mutex<Vec<HostScriptIdleRequest>>,
    pub(crate) script_kill_delete_damage_requests: Mutex<Vec<HostScriptKillDeleteDamageRequest>>,
    pub(crate) script_face_requests: Mutex<Vec<HostScriptFaceRequest>>,
    pub(crate) script_player_relates_requests: Mutex<Vec<HostScriptPlayerRelatesRequest>>,
    pub(crate) script_boobytrap_requests: Mutex<Vec<HostScriptBoobytrapRequest>>,
    pub(crate) script_unmanned_requests: Mutex<Vec<HostScriptUnmannedRequest>>,
    pub(crate) script_radar_event_requests: Mutex<Vec<HostScriptRadarEventRequest>>,
    pub(crate) script_stealth_enabled_requests: Mutex<Vec<HostScriptStealthEnabledRequest>>,
    pub(crate) script_buildable_override_requests: Mutex<Vec<HostScriptBuildableOverrideRequest>>,
    pub(crate) script_named_upgrade_requests: Mutex<Vec<HostScriptNamedUpgradeRequest>>,
    pub(crate) script_flash_requests: Mutex<Vec<HostScriptFlashRequest>>,
    pub(crate) script_emoticon_requests: Mutex<Vec<HostScriptEmoticonRequest>>,
    pub(crate) script_held_requests: Mutex<Vec<HostScriptHeldRequest>>,
    pub(crate) script_custom_color_requests: Mutex<Vec<HostScriptCustomColorRequest>>,
    pub(crate) script_named_attitude_requests: Mutex<Vec<HostScriptNamedAttitudeRequest>>,
    pub(crate) script_repulsor_requests: Mutex<Vec<HostScriptRepulsorRequest>>,
    pub(crate) script_stopping_distance_requests: Mutex<Vec<HostScriptStoppingDistanceRequest>>,
    pub(crate) script_force_select_requests: Mutex<Vec<HostScriptForceSelectRequest>>,
    pub(crate) script_player_misc_requests: Mutex<Vec<HostScriptPlayerMiscRequest>>,
    pub(crate) script_use_command_button_requests: Mutex<Vec<HostScriptUseCommandButtonRequest>>,
    pub(crate) team_partial_command_button_requests: Mutex<Vec<HostScriptTeamPartialCommandButtonRequest>>,
    pub(crate) set_base_construction_speed_requests: Mutex<Vec<(String, i32)>>,
    pub(crate) set_train_held_requests: Mutex<Vec<(String, bool)>>,
    pub(crate) script_topple_directions: Mutex<HashMap<String, (f32, f32)>>,
    pub(crate) skirmish_approach_path_requests: Mutex<Vec<HostScriptSkirmishApproachPathRequest>>,
    pub(crate) skirmish_base_defense_requests: Mutex<Vec<HostScriptSkirmishBaseDefenseRequest>>,
}

impl Default for HostScriptRequestQueues {
    fn default() -> Self {
        Self {
            skirmish_fire_special_requests: Mutex::new(Vec::new()),
            skirmish_build_requests: Mutex::new(Vec::new()),
            set_cave_index_requests: Mutex::new(Vec::new()),
            object_panel_flag_requests: Mutex::new(Vec::new()),
            team_panel_flag_requests: Mutex::new(Vec::new()),
            science_action_requests: Mutex::new(Vec::new()),
            script_move_attack_requests: Mutex::new(Vec::new()),
            build_team_requests: Mutex::new(Vec::new()),
            recruit_team_requests: Mutex::new(Vec::new()),
            script_create_requests: Mutex::new(Vec::new()),
            team_attitude_requests: Mutex::new(Vec::new()),
            ai_player_build_supply_center_requests: Mutex::new(Vec::new()),
            ai_player_build_upgrade_requests: Mutex::new(Vec::new()),
            ai_player_build_type_nearest_team_requests: Mutex::new(Vec::new()),
            guard_supply_center_requests: Mutex::new(Vec::new()),
            skirmish_attack_group_requests: Mutex::new(Vec::new()),
            skirmish_cmd_button_requests: Mutex::new(Vec::new()),
            script_hunt_guard_requests: Mutex::new(Vec::new()),
            script_follow_waypoints_requests: Mutex::new(Vec::new()),
            script_guard_variant_requests: Mutex::new(Vec::new()),
            script_named_fire_special_requests: Mutex::new(Vec::new()),
            script_idle_requests: Mutex::new(Vec::new()),
            script_kill_delete_damage_requests: Mutex::new(Vec::new()),
            script_face_requests: Mutex::new(Vec::new()),
            script_player_relates_requests: Mutex::new(Vec::new()),
            script_boobytrap_requests: Mutex::new(Vec::new()),
            script_unmanned_requests: Mutex::new(Vec::new()),
            script_radar_event_requests: Mutex::new(Vec::new()),
            script_stealth_enabled_requests: Mutex::new(Vec::new()),
            script_buildable_override_requests: Mutex::new(Vec::new()),
            script_named_upgrade_requests: Mutex::new(Vec::new()),
            script_flash_requests: Mutex::new(Vec::new()),
            script_emoticon_requests: Mutex::new(Vec::new()),
            script_held_requests: Mutex::new(Vec::new()),
            script_custom_color_requests: Mutex::new(Vec::new()),
            script_named_attitude_requests: Mutex::new(Vec::new()),
            script_repulsor_requests: Mutex::new(Vec::new()),
            script_stopping_distance_requests: Mutex::new(Vec::new()),
            script_force_select_requests: Mutex::new(Vec::new()),
            script_player_misc_requests: Mutex::new(Vec::new()),
            script_use_command_button_requests: Mutex::new(Vec::new()),
            team_partial_command_button_requests: Mutex::new(Vec::new()),
            set_base_construction_speed_requests: Mutex::new(Vec::new()),
            set_train_held_requests: Mutex::new(Vec::new()),
            script_topple_directions: Mutex::new(HashMap::new()),
            skirmish_approach_path_requests: Mutex::new(Vec::new()),
            skirmish_base_defense_requests: Mutex::new(Vec::new()),
        }
    }
}

fn host_script_queues() -> std::sync::Arc<crate::system::engine_stores::WorldServices> {
    crate::system::engine_stores::active_services()
}

fn try_host_script_queues() -> Option<std::sync::Arc<crate::system::engine_stores::WorldServices>> {
    crate::system::engine_stores::try_active_services()
}


/// Live host drain: `SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST` when crate
/// `OBJECT_REGISTRY` is empty (Wave 284 leftover no-op).
pub fn request_host_skirmish_fire_special(player_name: &str, power_name: &str) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().skirmish_fire_special_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((player_name.to_string(), power_name.to_string()));
    }
}

pub fn take_host_skirmish_fire_special_requests() -> Vec<(String, String)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().skirmish_fire_special_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: `SKIRMISH_BUILD_BUILDING` → `markPriorityBuild` on the
/// executing script's player (C++ `ScriptActions::doBuildBuilding` always
/// uses `TheScriptEngine->getCurrentPlayer()`).
pub fn request_host_skirmish_build_building(player_name: &str, thing_name: &str) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().skirmish_build_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((player_name.to_string(), thing_name.to_string()));
    }
}

pub fn take_host_skirmish_build_requests() -> Vec<(String, String)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().skirmish_build_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: `SET_CAVE_INDEX` → `CaveContain::tryToSetCaveIndex`.
/// Leftover crate objects are empty on the player path.
pub fn request_host_set_cave_index(cave_name: &str, cave_index: i32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().set_cave_index_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((cave_name.to_string(), cave_index));
    }
}

/// Live host drain: UNIT_AFFECT_OBJECT_PANEL_FLAGS when leftover registry is empty.
pub fn request_host_object_panel_flag(unit_name: &str, flag_name: &str, enable: bool) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().object_panel_flag_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((unit_name.to_string(), flag_name.to_string(), enable));
    }
}

pub fn take_host_object_panel_flag_requests() -> Vec<(String, String, bool)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().object_panel_flag_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TEAM_AFFECT_OBJECT_PANEL_FLAGS when leftover teams are empty.
pub fn request_host_team_panel_flag(team_name: &str, flag_name: &str, enable: bool) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().team_panel_flag_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((team_name.to_string(), flag_name.to_string(), enable));
    }
}

pub fn take_host_team_panel_flag_requests() -> Vec<(String, String, bool)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().team_panel_flag_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

pub fn take_host_set_cave_index_requests() -> Vec<(String, i32)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().set_cave_index_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: PLAYER_GRANT_SCIENCE / PLAYER_PURCHASE_SCIENCE.
/// `grant == true` → grantScience; false → attemptToPurchaseScience.
pub fn request_host_science_action(player_name: &str, science_name: &str, grant: bool) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().science_action_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((player_name.to_string(), science_name.to_string(), grant));
    }
}

pub fn take_host_science_action_requests() -> Vec<(String, String, bool)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().science_action_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TEAM/NAMED move and attack (`CMD_FROM_SCRIPT`).
pub fn request_host_script_move_attack(req: HostScriptMoveAttackRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_move_attack_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_move_attack_requests() -> Vec<HostScriptMoveAttackRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_move_attack_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: `BUILD_TEAM` → `AIPlayer::buildSpecificAITeam`.
pub fn request_host_build_team(owner_name: &str, team_name: &str) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().build_team_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((owner_name.to_string(), team_name.to_string()));
    }
}

pub fn take_host_build_team_requests() -> Vec<(String, String)> {
    let Some(services) = try_host_script_queues() else {
        return Vec::new();
    };
    std::mem::take(
        &mut *services
            .host_script_requests()
            .build_team_requests
            .lock()
            .unwrap_or_else(|p| p.into_inner()),
    )
}

/// Live host drain: `RECRUIT_TEAM` → `AIPlayer::recruitSpecificAITeam`.
pub fn request_host_recruit_team(owner_name: &str, team_name: &str, recruit_radius: f32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().recruit_team_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((
            owner_name.to_string(),
            team_name.to_string(),
            recruit_radius,
        ));
    }
}

pub fn take_host_recruit_team_requests() -> Vec<(String, String, f32)> {
    let Some(services) = try_host_script_queues() else {
        return Vec::new();
    };
    std::mem::take(
        &mut *services
            .host_script_requests()
            .recruit_team_requests
            .lock()
            .unwrap_or_else(|p| p.into_inner()),
    )
}

/// Live host drain: CREATE_OBJECT / named-at-waypoint / reinforcements.
pub fn request_host_script_create(req: HostScriptCreateRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_create_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_create_requests() -> Vec<HostScriptCreateRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_create_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TEAM_SET_ATTITUDE → `set_team_attitude_by_name`.
pub fn request_host_team_attitude(team_name: &str, mood: i32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().team_attitude_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((team_name.to_string(), mood));
    }
}

pub fn take_host_team_attitude_requests() -> Vec<(String, i32)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().team_attitude_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: `AI_PLAYER_BUILD_SUPPLY_CENTER` → `AIPlayer::buildBySupplies`.
pub fn request_host_ai_player_build_supply_center(
    player_name: &str,
    thing_name: &str,
    minimum_cash: i32,
) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().ai_player_build_supply_center_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((
            player_name.to_string(),
            thing_name.to_string(),
            minimum_cash,
        ));
    }
}

pub fn take_host_ai_player_build_supply_center_requests() -> Vec<(String, String, i32)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().ai_player_build_supply_center_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: `AI_PLAYER_BUILD_UPGRADE` → `AIPlayer::buildUpgrade`.
pub fn request_host_ai_player_build_upgrade(player_name: &str, upgrade_name: &str) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().ai_player_build_upgrade_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((player_name.to_string(), upgrade_name.to_string()));
    }
}

pub fn take_host_ai_player_build_upgrade_requests() -> Vec<(String, String)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().ai_player_build_upgrade_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: `AI_PLAYER_BUILD_TYPE_NEAREST_TEAM` →
/// `AIPlayer::buildSpecificBuildingNearestTeam`.
pub fn request_host_ai_player_build_type_nearest_team(
    player_name: &str,
    thing_name: &str,
    team_name: &str,
) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().ai_player_build_type_nearest_team_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((
            player_name.to_string(),
            thing_name.to_string(),
            team_name.to_string(),
        ));
    }
}

pub fn take_host_ai_player_build_type_nearest_team_requests() -> Vec<(String, String, String)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().ai_player_build_type_nearest_team_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: `TEAM_GUARD_SUPPLY_CENTER` → `AIPlayer::guardSupplyCenter`.
pub fn request_host_guard_supply_center(team_name: &str, min_supplies: i32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().guard_supply_center_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((team_name.to_string(), min_supplies));
    }
}

pub fn take_host_guard_supply_center_requests() -> Vec<(String, i32)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().guard_supply_center_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TEAM/NAMED HUNT, TEAM/NAMED GUARD, PLAYER_HUNT.
pub fn request_host_script_hunt_guard(req: HostScriptHuntGuardRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_hunt_guard_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_hunt_guard_requests() -> Vec<HostScriptHuntGuardRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_hunt_guard_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TEAM/NAMED FOLLOW_WAYPOINTS and EXACT.
pub fn request_host_script_follow_waypoints(req: HostScriptFollowWaypointsRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_follow_waypoints_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_follow_waypoints_requests() -> Vec<HostScriptFollowWaypointsRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_follow_waypoints_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TEAM_GUARD_POSITION / OBJECT / AREA / TUNNEL_NETWORK.
pub fn request_host_script_guard_variant(req: HostScriptGuardVariantRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_guard_variant_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_guard_variant_requests() -> Vec<HostScriptGuardVariantRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_guard_variant_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT / AT_NAMED.
pub fn request_host_script_named_fire_special(req: HostScriptNamedFireSpecialPowerRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_named_fire_special_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_named_fire_special_requests() -> Vec<HostScriptNamedFireSpecialPowerRequest>
{
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_named_fire_special_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED_STOP / TEAM_STOP (`CMD_FROM_SCRIPT` idle).
pub fn request_host_script_idle(req: HostScriptIdleRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_idle_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_idle_requests() -> Vec<HostScriptIdleRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_idle_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM DELETE / KILL / DAMAGE.
pub fn request_host_script_kill_delete_damage(req: HostScriptKillDeleteDamageRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_kill_delete_damage_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_kill_delete_damage_requests() -> Vec<HostScriptKillDeleteDamageRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_kill_delete_damage_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM FACE (`aiFaceObject` / `aiFacePosition`).
pub fn request_host_script_face(req: HostScriptFaceRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_face_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_face_requests() -> Vec<HostScriptFaceRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_face_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: PLAYER_RELATES_PLAYER (`setPlayerRelationship`).
pub fn request_host_player_relates(req: HostScriptPlayerRelatesRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_player_relates_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_player_relates_requests() -> Vec<HostScriptPlayerRelatesRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_player_relates_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM SET_BOOBYTRAPPED (`initStickyBomb`).
pub fn request_host_script_boobytrap(req: HostScriptBoobytrapRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_boobytrap_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_boobytrap_requests() -> Vec<HostScriptBoobytrapRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_boobytrap_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM SET_UNMANNED / DELETE_ALL_UNMANNED.
pub fn request_host_script_unmanned(req: HostScriptUnmannedRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_unmanned_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_unmanned_requests() -> Vec<HostScriptUnmannedRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_unmanned_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: OBJECT/TEAM CREATE_RADAR_EVENT.
pub fn request_host_script_radar_event(req: HostScriptRadarEventRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_radar_event_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_radar_event_requests() -> Vec<HostScriptRadarEventRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_radar_event_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM SET_STEALTH_ENABLED.
pub fn request_host_script_stealth_enabled(req: HostScriptStealthEnabledRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_stealth_enabled_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_stealth_enabled_requests() -> Vec<HostScriptStealthEnabledRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_stealth_enabled_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TECHTREE_MODIFY_BUILDABILITY_OBJECT.
pub fn request_host_buildable_status_override(template: &str, status: i32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().script_buildable_override_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push(HostScriptBuildableOverrideRequest {
            template: template.to_string(),
            status,
        });
    }
}

pub fn take_host_buildable_status_override_requests() -> Vec<HostScriptBuildableOverrideRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_buildable_override_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED_RECEIVE_UPGRADE.
pub fn request_host_script_named_upgrade(req: HostScriptNamedUpgradeRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_named_upgrade_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_named_upgrade_requests() -> Vec<HostScriptNamedUpgradeRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_named_upgrade_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM FLASH / FLASH_WHITE.
pub fn request_host_script_flash(req: HostScriptFlashRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_flash_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_flash_requests() -> Vec<HostScriptFlashRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_flash_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM SET_EMOTICON.
pub fn request_host_script_emoticon(req: HostScriptEmoticonRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_emoticon_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_emoticon_requests() -> Vec<HostScriptEmoticonRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_emoticon_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED_SET_HELD.
pub fn request_host_script_held(req: HostScriptHeldRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_held_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_held_requests() -> Vec<HostScriptHeldRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_held_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED_CUSTOM_COLOR.
pub fn request_host_script_custom_color(req: HostScriptCustomColorRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_custom_color_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_custom_color_requests() -> Vec<HostScriptCustomColorRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_custom_color_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED_SET_ATTITUDE.
pub fn request_host_script_named_attitude(req: HostScriptNamedAttitudeRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_named_attitude_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_named_attitude_requests() -> Vec<HostScriptNamedAttitudeRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_named_attitude_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM SET_REPULSOR.
pub fn request_host_script_repulsor(req: HostScriptRepulsorRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_repulsor_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_repulsor_requests() -> Vec<HostScriptRepulsorRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_repulsor_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM SET_STOPPING_DISTANCE.
pub fn request_host_script_stopping_distance(req: HostScriptStoppingDistanceRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_stopping_distance_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_stopping_distance_requests() -> Vec<HostScriptStoppingDistanceRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_stopping_distance_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: OBJECT_FORCE_SELECT.
pub fn request_host_script_force_select(req: HostScriptForceSelectRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_force_select_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_force_select_requests() -> Vec<HostScriptForceSelectRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_force_select_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: PLAYER_SELL_EVERYTHING / REPAIR_NAMED / SCORE / SKILLSET / KILL.
pub fn request_host_script_player_misc(req: HostScriptPlayerMiscRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_player_misc_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_player_misc_requests() -> Vec<HostScriptPlayerMiscRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_player_misc_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: NAMED/TEAM USE_COMMANDBUTTON_ABILITY.
pub fn request_host_script_use_command_button(req: HostScriptUseCommandButtonRequest) {
    { let services = host_script_queues(); services.host_script_requests().script_use_command_button_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_script_use_command_button_requests() -> Vec<HostScriptUseCommandButtonRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().script_use_command_button_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: TEAM_PARTIAL_USE_COMMANDBUTTON.
pub fn request_host_team_partial_command_button(req: HostScriptTeamPartialCommandButtonRequest) {
    { let services = host_script_queues(); services.host_script_requests().team_partial_command_button_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_team_partial_command_button_requests()
-> Vec<HostScriptTeamPartialCommandButtonRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().team_partial_command_button_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: SET_BASE_CONSTRUCTION_SPEED → `AIPlayer::setTeamDelaySeconds`.
pub fn request_host_set_base_construction_speed(player: &str, delay_seconds: i32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().set_base_construction_speed_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((player.to_string(), delay_seconds));
    }
}

pub fn take_host_set_base_construction_speed_requests() -> Vec<(String, i32)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().set_base_construction_speed_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: SET_TRAIN_HELD → `HostRailroadCar::held`.
pub fn request_host_set_train_held(unit: &str, held: bool) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().set_train_held_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((unit.to_string(), held));
    }
}

pub fn take_host_set_train_held_requests() -> Vec<(String, bool)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().set_train_held_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// C++ `ScriptEngine::setToppleDirection` live map. Consulted by host topple.
pub fn request_host_script_topple_direction(unit: &str, dx: f32, dy: f32) {
    if unit.is_empty() {
        return;
    }
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().script_topple_directions.lock().unwrap_or_else(|p| p.into_inner());
        q.insert(unit.to_string(), (dx, dy));
    }
}

pub fn host_script_topple_direction_for(unit: &str) -> Option<(f32, f32)> {
    if unit.is_empty() {
        return None;
    }
    { let services = host_script_queues(); services.host_script_requests().script_topple_directions.lock().unwrap_or_else(|p| p.into_inner()).get(unit).copied() }
}

/// Live host drain: SKIRMISH_FOLLOW/MOVE_TO_APPROACH_PATH.
pub fn request_host_skirmish_approach_path(req: HostScriptSkirmishApproachPathRequest) {
    { let services = host_script_queues(); services.host_script_requests().skirmish_approach_path_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_skirmish_approach_path_requests() -> Vec<HostScriptSkirmishApproachPathRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().skirmish_approach_path_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: SKIRMISH_BUILD_BASE_DEFENSE_* / SKIRMISH_BUILD_STRUCTURE_*.
pub fn request_host_skirmish_base_defense(req: HostScriptSkirmishBaseDefenseRequest) {
    { let services = host_script_queues(); services.host_script_requests().skirmish_base_defense_requests.lock().unwrap_or_else(|p| p.into_inner()).push(req); }
}

pub fn take_host_skirmish_base_defense_requests() -> Vec<HostScriptSkirmishBaseDefenseRequest> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().skirmish_base_defense_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE.
pub fn request_host_skirmish_attack_nearest_group(team: &str, comparison: i32, value: i32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().skirmish_attack_group_requests.lock().unwrap_or_else(|p| p.into_inner());

        q.push((team.to_string(), comparison, value));
    }
}

pub fn take_host_skirmish_attack_nearest_group_requests() -> Vec<(String, i32, i32)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().skirmish_attack_group_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Live host drain: SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT.
pub fn request_host_skirmish_command_button_most_valuable(team: &str, ability: &str, range: f32) {
    {
        let services = host_script_queues();
        let mut q = services.host_script_requests().skirmish_cmd_button_requests.lock().unwrap_or_else(|p| p.into_inner());

        q
            .push((team.to_string(), ability.to_string(), range));
    }
}

pub fn take_host_skirmish_command_button_most_valuable_requests() -> Vec<(String, String, f32)> {
    { let services = host_script_queues(); std::mem::take(&mut *services.host_script_requests().skirmish_cmd_button_requests.lock().unwrap_or_else(|p| p.into_inner())) }
}

/// Wave 284: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

fn to_radar_coord(pos: &Coord3D) -> game_engine::common::system::radar::Coord3D {
    game_engine::common::system::radar::Coord3D::new(pos.x, pos.y, pos.z)
}


#[cfg(test)]
mod host_script_queue_ownership_tests {
    use super::*;
    use crate::system::engine_stores::{new_world_services, with_world_services};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    #[test]
    fn parallel_worlds_keep_independent_host_script_queues() {
        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for seed in ["alpha", "beta"] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            let seed = seed.to_string();
            handles.push(thread::spawn(move || {
                let services = new_world_services();
                with_world_services(&services, || {
                    barrier.wait();
                    request_host_skirmish_build_building(&seed, "thing");
                    barrier.wait();
                    let drained = take_host_skirmish_build_requests();
                    results.lock().unwrap().push((seed, drained));
                });
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut got = results.lock().unwrap();
        got.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(got[0].1, vec![("alpha".into(), "thing".into())]);
        assert_eq!(got[1].1, vec![("beta".into(), "thing".into())]);
    }
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
#[derive(Clone)]
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
    /// Trigger edge state for this world context. Main supplies the owning
    /// GameLogic instance's handle; standalone executor contexts own a fresh
    /// state instead of consulting ambient TLS.
    pub host_trigger_world: Arc<Mutex<crate::scripting::HostTriggerWorld>>,
}

impl ScriptContext {
    pub fn with_session_world(
        mut self,
        world: Arc<Mutex<crate::scripting::HostTriggerWorld>>,
    ) -> Self {
        self.host_trigger_world = world;
        self
    }

    /// An installed owner is authoritative, including an installed empty
    /// map. Only standalone, uninitialized contexts retain ambient fallback.
    fn owned_trigger_area(
        &self,
        area_name: &str,
    ) -> Option<Option<crate::polygon_trigger::PolygonTrigger>> {
        let world = self
            .host_trigger_world
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        world
            .has_authored_trigger_geometry()
            .then(|| world.trigger_area_by_name(area_name))
    }

    pub fn new() -> Self {
        Self::at_frame(TheGameLogic::get_frame())
    }

    /// Construct operation state at the driving world's observed logic frame.
    /// The caller supplies the clock; construction never acquires GameLogic.
    pub fn at_frame(current_frame: u32) -> Self {
        Self {
            game_logic_id: 0,
            object_manager_id: 0,
            player_manager_id: 0,
            event_system_id: 0,
            camera_system_id: 0,
            audio_system_id: 0,
            partition_manager_id: 0,
            special_powers_id: 0,
            current_frame,
            suppress_new_windows: false,
            host_trigger_world: Arc::new(Mutex::new(Default::default())),
        }
    }
}

/// Script action dispatcher
///
/// C++ Reference: ScriptActions::executeAction()
/// This is the main entry point for executing script actions
pub struct ScriptActionDispatcher<'engine> {
    context: ExecutionContext<'engine>,
    /// Read-only name projection refreshed at each driving-owner action.
    this_team_name: crate::scripting::engine::ScriptOwnerQuery<String>,
}

impl<'engine> ScriptActionDispatcher<'engine> {
    fn current_script_player_name(&self) -> String {
        self.context
            .with_engine_ref(|engine| engine.get_current_player_name())
            .flatten()
            .unwrap_or_default()
    }

    /// Return an owned handler before crossing callbacks; no engine/state guard escapes.
    fn current_script_action_handler(
        &self,
    ) -> Option<Arc<dyn crate::scripting::engine::ScriptActionHandler>> {
        self.context
            .with_engine_ref(|engine| engine.action_handler())
            .flatten()
    }

    /// Dispatch synchronously on this engine using caller-owned operation state.
    /// Construction does not publish the engine or select an ambient world.
    pub fn new(
        engine: &'engine crate::scripting::engine::ScriptEngine,
        state: &'engine std::cell::RefCell<ScriptContext>,
    ) -> Self {
        Self {
            context: ExecutionContext::borrowed(engine, state),
            this_team_name: crate::scripting::engine::ScriptOwnerQuery::Unavailable,
        }
    }
}

/// Script condition evaluator
///
/// C++ Reference: ScriptConditions::evaluateCondition()
/// This evaluates script conditions to determine script flow
#[allow(dead_code)]
pub struct ScriptConditionEvaluator<'engine> {
    context: ExecutionContext<'engine>,
    /// Read-only projection; the engine's canonical references remain IDs.
    this_team_name: crate::scripting::engine::ScriptOwnerQuery<String>,
}

impl<'engine> ScriptConditionEvaluator<'engine> {
    /// Dispatch synchronously on this engine using caller-owned operation state.
    /// Construction does not publish the engine or select an ambient world.
    pub fn new(
        engine: &'engine crate::scripting::engine::ScriptEngine,
        state: &'engine std::cell::RefCell<ScriptContext>,
    ) -> Self {
        Self {
            context: ExecutionContext::borrowed(engine, state),
            this_team_name: crate::scripting::engine::ScriptOwnerQuery::Unavailable,
        }
    }

    pub(crate) fn with_host_trigger_world<R>(
        &self,
        f: impl FnOnce(&crate::scripting::HostTriggerWorld, u32) -> R,
    ) -> R {
        self.context.with_state(|context| {
            let world = context
                .host_trigger_world
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            f(&world, context.current_frame)
        })
    }
}

mod execution_context;
use execution_context::ExecutionContext;

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
mod borrowed_presentation;
mod dispatch;
pub(crate) use borrowed_presentation::{apply_script_drawable_flash, script_indicator_color};
mod eval_basic;
mod eval_named;
mod eval_owned_body_census;
mod eval_owned_player_area;
mod eval_owned_player_census;
mod eval_owned_skirmish;
mod eval_player_events;
mod eval_receiving_named_facts;
mod eval_skirmish;

#[cfg(test)]
mod tests;
