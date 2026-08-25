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
    TFade, get_area_tracker, get_named_object_tracker, get_script_engine, with_script_engine_mut,
    with_script_engine_ref,
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
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

/// Live host drain: TEAM/NAMED move and attack when leftover `OBJECT_REGISTRY` is empty.
/// C++ `ScriptActions::doMoveToWaypoint` / `doNamedMoveToWaypoint` / `doAttack` /
/// `doNamedAttack` / `doNamedAttackArea` / `doNamedAttackTeam` / `doTeamAttackArea` /
/// `doTeamAttackNamed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptMoveAttackRequest {
    TeamMove {
        team: String,
        waypoint: String,
    },
    NamedMove {
        unit: String,
        waypoint: String,
    },
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

/// Live host drain: TEAM/NAMED HUNT, TEAM/NAMED GUARD, PLAYER_HUNT,
/// TEAM_HUNT_WITH_COMMAND_BUTTON.
/// C++ `ScriptActions::doNamedHunt` / `doTeamHunt` / `doNamedGuard` /
/// `doTeamGuard` / `doPlayerHunt` / `doTeamHuntWithCommandButton`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptHuntGuardRequest {
    TeamHunt { team: String },
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

/// Live host drain: SOUND_PLAY_NAMED / ENABLE_OBJECT_SOUND / DISABLE_OBJECT_SOUND.
/// C++ `ScriptActions::doSoundPlayFromNamed` / `doEnableObjectSound`.
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

/// Live host drain: PLAYER_SET_MONEY / PLAYER_GIVE_MONEY.
/// C++ `ScriptActions::doSetMoney` / `doGiveMoney`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptMoneyRequest {
    Set { player: String, amount: i32 },
    Give { player: String, amount: i32 },
}

/// Live host drain: TEAM/PLAYER/NAMED TRANSFER ownership.
/// C++ `ScriptActions::doTransferTeamToPlayer` / `doPlayerTransferAssetsToPlayer` /
/// `doNamedTransferAssetsToPlayer` (`transferObjectToPlayer` / `transferAssetsFromThat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptTransferRequest {
    Player { from: String, to: String },
    Named { unit: String, player: String },
    Team { team: String, player: String },
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

/// Live host drain: PLAYER_ADD_SKILLPOINTS / ADD_RANKLEVEL / SET_RANKLEVEL /
/// SET_RANKLEVELLIMIT / AFFECT_RECEIVING_EXPERIENCE.
/// C++ `doPlayerAddSkillPoints` / `doPlayerAddRankLevels` / `doPlayerSetRankLevel` /
/// `doMapSetRankLevelLimit` / `doAffectSkillPointsModifier`.
/// Leftover writes leftover player_list / leftover GameLogic; live rank lives on host.
#[derive(Debug, Clone, PartialEq)]
pub enum HostScriptRankRequest {
    AddSkillPoints { player: String, delta: i32 },
    AddRankLevel { player: String, delta: i32 },
    SetRankLevel { player: String, level: i32 },
    SetRankLevelLimit { limit: i32 },
    AffectReceivingExperience { player: String, modifier: f32 },
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

/// Live host drain: PLAYER_DISABLE/ENABLE_UNIT_CONSTRUCTION,
/// PLAYER_DISABLE/ENABLE_BASE_CONSTRUCTION, PLAYER_DISABLE/ENABLE_FACTORIES.
/// C++ `Player::setCanBuildUnits` / `setCanBuildBase` / `setObjectsEnabled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptCanBuildRequest {
    Units {
        player: String,
        enable: bool,
    },
    Base {
        player: String,
        enable: bool,
    },
    Factories {
        player: String,
        template: String,
        enable: bool,
    },
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
    static HOST_SCRIPT_HUNT_GUARD_REQUESTS: RefCell<Vec<HostScriptHuntGuardRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_FOLLOW_WAYPOINTS_REQUESTS: RefCell<Vec<HostScriptFollowWaypointsRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_GUARD_VARIANT_REQUESTS: RefCell<Vec<HostScriptGuardVariantRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_NAMED_FIRE_SPECIAL_REQUESTS:
        RefCell<Vec<HostScriptNamedFireSpecialPowerRequest>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_IDLE_REQUESTS: RefCell<Vec<HostScriptIdleRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_KILL_DELETE_DAMAGE_REQUESTS:
        RefCell<Vec<HostScriptKillDeleteDamageRequest>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_OBJECT_SOUND_REQUESTS: RefCell<Vec<HostScriptObjectSoundRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_FACE_REQUESTS: RefCell<Vec<HostScriptFaceRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_MONEY_REQUESTS: RefCell<Vec<HostScriptMoneyRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_TRANSFER_REQUESTS: RefCell<Vec<HostScriptTransferRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_PLAYER_RELATES_REQUESTS: RefCell<Vec<HostScriptPlayerRelatesRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_BOOBYTRAP_REQUESTS: RefCell<Vec<HostScriptBoobytrapRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_RANK_REQUESTS: RefCell<Vec<HostScriptRankRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_UNMANNED_REQUESTS: RefCell<Vec<HostScriptUnmannedRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_RADAR_EVENT_REQUESTS: RefCell<Vec<HostScriptRadarEventRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_STEALTH_ENABLED_REQUESTS: RefCell<Vec<HostScriptStealthEnabledRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_CAN_BUILD_REQUESTS: RefCell<Vec<HostScriptCanBuildRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_BUILDABLE_OVERRIDE_REQUESTS:
        RefCell<Vec<HostScriptBuildableOverrideRequest>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_NAMED_UPGRADE_REQUESTS: RefCell<Vec<HostScriptNamedUpgradeRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_FLASH_REQUESTS: RefCell<Vec<HostScriptFlashRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_EMOTICON_REQUESTS: RefCell<Vec<HostScriptEmoticonRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_HELD_REQUESTS: RefCell<Vec<HostScriptHeldRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_CUSTOM_COLOR_REQUESTS: RefCell<Vec<HostScriptCustomColorRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_NAMED_ATTITUDE_REQUESTS: RefCell<Vec<HostScriptNamedAttitudeRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_REPULSOR_REQUESTS: RefCell<Vec<HostScriptRepulsorRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_STOPPING_DISTANCE_REQUESTS:
        RefCell<Vec<HostScriptStoppingDistanceRequest>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_FORCE_SELECT_REQUESTS: RefCell<Vec<HostScriptForceSelectRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_PLAYER_MISC_REQUESTS: RefCell<Vec<HostScriptPlayerMiscRequest>> =
        RefCell::new(Vec::new());
    static HOST_SCRIPT_USE_COMMAND_BUTTON_REQUESTS:
        RefCell<Vec<HostScriptUseCommandButtonRequest>> = RefCell::new(Vec::new());
    static HOST_TEAM_PARTIAL_COMMAND_BUTTON_REQUESTS:
        RefCell<Vec<HostScriptTeamPartialCommandButtonRequest>> = RefCell::new(Vec::new());
    static HOST_SET_BASE_CONSTRUCTION_SPEED_REQUESTS: RefCell<Vec<(String, i32)>> =
        RefCell::new(Vec::new());
    static HOST_SET_TRAIN_HELD_REQUESTS: RefCell<Vec<(String, bool)>> = RefCell::new(Vec::new());
    static HOST_SCRIPT_TOPPLE_DIRECTIONS: RefCell<HashMap<String, (f32, f32)>> =
        RefCell::new(HashMap::new());
    static HOST_SKIRMISH_APPROACH_PATH_REQUESTS:
        RefCell<Vec<HostScriptSkirmishApproachPathRequest>> = RefCell::new(Vec::new());
    static HOST_SKIRMISH_BASE_DEFENSE_REQUESTS:
        RefCell<Vec<HostScriptSkirmishBaseDefenseRequest>> = RefCell::new(Vec::new());












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
        q.borrow_mut().push((cave_name.to_string(), cave_index));
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
        q.borrow_mut()
            .push((player_name.to_string(), science_name.to_string(), grant));
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
        q.borrow_mut().push((
            owner_name.to_string(),
            team_name.to_string(),
            recruit_radius,
        ));
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
        q.borrow_mut().push((team_name.to_string(), min_supplies));
    });
}

pub fn take_host_guard_supply_center_requests() -> Vec<(String, i32)> {
    HOST_GUARD_SUPPLY_CENTER_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM/NAMED HUNT, TEAM/NAMED GUARD, PLAYER_HUNT.
pub fn request_host_script_hunt_guard(req: HostScriptHuntGuardRequest) {
    HOST_SCRIPT_HUNT_GUARD_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_hunt_guard_requests() -> Vec<HostScriptHuntGuardRequest> {
    HOST_SCRIPT_HUNT_GUARD_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM/NAMED FOLLOW_WAYPOINTS and EXACT.
pub fn request_host_script_follow_waypoints(req: HostScriptFollowWaypointsRequest) {
    HOST_SCRIPT_FOLLOW_WAYPOINTS_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_follow_waypoints_requests() -> Vec<HostScriptFollowWaypointsRequest> {
    HOST_SCRIPT_FOLLOW_WAYPOINTS_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM_GUARD_POSITION / OBJECT / AREA / TUNNEL_NETWORK.
pub fn request_host_script_guard_variant(req: HostScriptGuardVariantRequest) {
    HOST_SCRIPT_GUARD_VARIANT_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_guard_variant_requests() -> Vec<HostScriptGuardVariantRequest> {
    HOST_SCRIPT_GUARD_VARIANT_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT / AT_NAMED.
pub fn request_host_script_named_fire_special(req: HostScriptNamedFireSpecialPowerRequest) {
    HOST_SCRIPT_NAMED_FIRE_SPECIAL_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_named_fire_special_requests() -> Vec<HostScriptNamedFireSpecialPowerRequest>
{
    HOST_SCRIPT_NAMED_FIRE_SPECIAL_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED_STOP / TEAM_STOP (`CMD_FROM_SCRIPT` idle).
pub fn request_host_script_idle(req: HostScriptIdleRequest) {
    HOST_SCRIPT_IDLE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_idle_requests() -> Vec<HostScriptIdleRequest> {
    HOST_SCRIPT_IDLE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM DELETE / KILL / DAMAGE.
pub fn request_host_script_kill_delete_damage(req: HostScriptKillDeleteDamageRequest) {
    HOST_SCRIPT_KILL_DELETE_DAMAGE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_kill_delete_damage_requests() -> Vec<HostScriptKillDeleteDamageRequest> {
    HOST_SCRIPT_KILL_DELETE_DAMAGE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SOUND_PLAY_NAMED / ENABLE / DISABLE_OBJECT_SOUND.
pub fn request_host_script_object_sound(req: HostScriptObjectSoundRequest) {
    HOST_SCRIPT_OBJECT_SOUND_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_object_sound_requests() -> Vec<HostScriptObjectSoundRequest> {
    HOST_SCRIPT_OBJECT_SOUND_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM FACE (`aiFaceObject` / `aiFacePosition`).
pub fn request_host_script_face(req: HostScriptFaceRequest) {
    HOST_SCRIPT_FACE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_face_requests() -> Vec<HostScriptFaceRequest> {
    HOST_SCRIPT_FACE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: PLAYER_SET_MONEY / PLAYER_GIVE_MONEY.
/// C++ `doSetMoney` withdraws all then deposits; `doGiveMoney` deposits or
/// withdraws a signed amount.
pub fn request_host_money(req: HostScriptMoneyRequest) {
    HOST_SCRIPT_MONEY_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_money_requests() -> Vec<HostScriptMoneyRequest> {
    HOST_SCRIPT_MONEY_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM/PLAYER/NAMED TRANSFER (`setControllingPlayer` /
/// `transferAssetsFromThat` / `setTeam(defaultTeam)`).
pub fn request_host_script_transfer(req: HostScriptTransferRequest) {
    HOST_SCRIPT_TRANSFER_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_transfer_requests() -> Vec<HostScriptTransferRequest> {
    HOST_SCRIPT_TRANSFER_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: PLAYER_RELATES_PLAYER (`setPlayerRelationship`).
pub fn request_host_player_relates(req: HostScriptPlayerRelatesRequest) {
    HOST_SCRIPT_PLAYER_RELATES_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_player_relates_requests() -> Vec<HostScriptPlayerRelatesRequest> {
    HOST_SCRIPT_PLAYER_RELATES_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM SET_BOOBYTRAPPED (`initStickyBomb`).
pub fn request_host_script_boobytrap(req: HostScriptBoobytrapRequest) {
    HOST_SCRIPT_BOOBYTRAP_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_boobytrap_requests() -> Vec<HostScriptBoobytrapRequest> {
    HOST_SCRIPT_BOOBYTRAP_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: PLAYER_ADD_SKILLPOINTS / ADD_RANKLEVEL / SET_RANKLEVEL /
/// SET_RANKLEVELLIMIT / AFFECT_RECEIVING_EXPERIENCE.
pub fn request_host_rank(req: HostScriptRankRequest) {
    HOST_SCRIPT_RANK_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_rank_requests() -> Vec<HostScriptRankRequest> {
    HOST_SCRIPT_RANK_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM SET_UNMANNED / DELETE_ALL_UNMANNED.
pub fn request_host_script_unmanned(req: HostScriptUnmannedRequest) {
    HOST_SCRIPT_UNMANNED_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_unmanned_requests() -> Vec<HostScriptUnmannedRequest> {
    HOST_SCRIPT_UNMANNED_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: OBJECT/TEAM CREATE_RADAR_EVENT.
pub fn request_host_script_radar_event(req: HostScriptRadarEventRequest) {
    HOST_SCRIPT_RADAR_EVENT_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_radar_event_requests() -> Vec<HostScriptRadarEventRequest> {
    HOST_SCRIPT_RADAR_EVENT_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM SET_STEALTH_ENABLED.
pub fn request_host_script_stealth_enabled(req: HostScriptStealthEnabledRequest) {
    HOST_SCRIPT_STEALTH_ENABLED_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_stealth_enabled_requests() -> Vec<HostScriptStealthEnabledRequest> {
    HOST_SCRIPT_STEALTH_ENABLED_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: PLAYER_DISABLE/ENABLE unit/base/factory construction.
pub fn request_host_can_build(req: HostScriptCanBuildRequest) {
    HOST_SCRIPT_CAN_BUILD_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_can_build_requests() -> Vec<HostScriptCanBuildRequest> {
    HOST_SCRIPT_CAN_BUILD_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TECHTREE_MODIFY_BUILDABILITY_OBJECT.
pub fn request_host_buildable_status_override(template: &str, status: i32) {
    HOST_SCRIPT_BUILDABLE_OVERRIDE_REQUESTS.with(|q| {
        q.borrow_mut().push(HostScriptBuildableOverrideRequest {
            template: template.to_string(),
            status,
        });
    });
}

pub fn take_host_buildable_status_override_requests() -> Vec<HostScriptBuildableOverrideRequest> {
    HOST_SCRIPT_BUILDABLE_OVERRIDE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED_RECEIVE_UPGRADE.
pub fn request_host_script_named_upgrade(req: HostScriptNamedUpgradeRequest) {
    HOST_SCRIPT_NAMED_UPGRADE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_named_upgrade_requests() -> Vec<HostScriptNamedUpgradeRequest> {
    HOST_SCRIPT_NAMED_UPGRADE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM FLASH / FLASH_WHITE.
pub fn request_host_script_flash(req: HostScriptFlashRequest) {
    HOST_SCRIPT_FLASH_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_flash_requests() -> Vec<HostScriptFlashRequest> {
    HOST_SCRIPT_FLASH_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM SET_EMOTICON.
pub fn request_host_script_emoticon(req: HostScriptEmoticonRequest) {
    HOST_SCRIPT_EMOTICON_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_emoticon_requests() -> Vec<HostScriptEmoticonRequest> {
    HOST_SCRIPT_EMOTICON_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED_SET_HELD.
pub fn request_host_script_held(req: HostScriptHeldRequest) {
    HOST_SCRIPT_HELD_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_held_requests() -> Vec<HostScriptHeldRequest> {
    HOST_SCRIPT_HELD_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED_CUSTOM_COLOR.
pub fn request_host_script_custom_color(req: HostScriptCustomColorRequest) {
    HOST_SCRIPT_CUSTOM_COLOR_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_custom_color_requests() -> Vec<HostScriptCustomColorRequest> {
    HOST_SCRIPT_CUSTOM_COLOR_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED_SET_ATTITUDE.
pub fn request_host_script_named_attitude(req: HostScriptNamedAttitudeRequest) {
    HOST_SCRIPT_NAMED_ATTITUDE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_named_attitude_requests() -> Vec<HostScriptNamedAttitudeRequest> {
    HOST_SCRIPT_NAMED_ATTITUDE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM SET_REPULSOR.
pub fn request_host_script_repulsor(req: HostScriptRepulsorRequest) {
    HOST_SCRIPT_REPULSOR_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_repulsor_requests() -> Vec<HostScriptRepulsorRequest> {
    HOST_SCRIPT_REPULSOR_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM SET_STOPPING_DISTANCE.
pub fn request_host_script_stopping_distance(req: HostScriptStoppingDistanceRequest) {
    HOST_SCRIPT_STOPPING_DISTANCE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_stopping_distance_requests() -> Vec<HostScriptStoppingDistanceRequest> {
    HOST_SCRIPT_STOPPING_DISTANCE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: OBJECT_FORCE_SELECT.
pub fn request_host_script_force_select(req: HostScriptForceSelectRequest) {
    HOST_SCRIPT_FORCE_SELECT_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_force_select_requests() -> Vec<HostScriptForceSelectRequest> {
    HOST_SCRIPT_FORCE_SELECT_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: PLAYER_SELL_EVERYTHING / REPAIR_NAMED / SCORE / SKILLSET / KILL.
pub fn request_host_script_player_misc(req: HostScriptPlayerMiscRequest) {
    HOST_SCRIPT_PLAYER_MISC_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_player_misc_requests() -> Vec<HostScriptPlayerMiscRequest> {
    HOST_SCRIPT_PLAYER_MISC_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: NAMED/TEAM USE_COMMANDBUTTON_ABILITY.
pub fn request_host_script_use_command_button(req: HostScriptUseCommandButtonRequest) {
    HOST_SCRIPT_USE_COMMAND_BUTTON_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_script_use_command_button_requests() -> Vec<HostScriptUseCommandButtonRequest> {
    HOST_SCRIPT_USE_COMMAND_BUTTON_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: TEAM_PARTIAL_USE_COMMANDBUTTON.
pub fn request_host_team_partial_command_button(req: HostScriptTeamPartialCommandButtonRequest) {
    HOST_TEAM_PARTIAL_COMMAND_BUTTON_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_team_partial_command_button_requests()
-> Vec<HostScriptTeamPartialCommandButtonRequest> {
    HOST_TEAM_PARTIAL_COMMAND_BUTTON_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SET_BASE_CONSTRUCTION_SPEED → `AIPlayer::setTeamDelaySeconds`.
pub fn request_host_set_base_construction_speed(player: &str, delay_seconds: i32) {
    HOST_SET_BASE_CONSTRUCTION_SPEED_REQUESTS.with(|q| {
        q.borrow_mut().push((player.to_string(), delay_seconds));
    });
}

pub fn take_host_set_base_construction_speed_requests() -> Vec<(String, i32)> {
    HOST_SET_BASE_CONSTRUCTION_SPEED_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SET_TRAIN_HELD → `HostRailroadCar::held`.
pub fn request_host_set_train_held(unit: &str, held: bool) {
    HOST_SET_TRAIN_HELD_REQUESTS.with(|q| {
        q.borrow_mut().push((unit.to_string(), held));
    });
}

pub fn take_host_set_train_held_requests() -> Vec<(String, bool)> {
    HOST_SET_TRAIN_HELD_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// C++ `ScriptEngine::setToppleDirection` live map. Consulted by host topple.
pub fn request_host_script_topple_direction(unit: &str, dx: f32, dy: f32) {
    if unit.is_empty() {
        return;
    }
    HOST_SCRIPT_TOPPLE_DIRECTIONS.with(|m| {
        m.borrow_mut().insert(unit.to_string(), (dx, dy));
    });
}

pub fn host_script_topple_direction_for(unit: &str) -> Option<(f32, f32)> {
    if unit.is_empty() {
        return None;
    }
    HOST_SCRIPT_TOPPLE_DIRECTIONS.with(|m| m.borrow().get(unit).copied())
}

/// Live host drain: SKIRMISH_FOLLOW/MOVE_TO_APPROACH_PATH.
pub fn request_host_skirmish_approach_path(req: HostScriptSkirmishApproachPathRequest) {
    HOST_SKIRMISH_APPROACH_PATH_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_skirmish_approach_path_requests() -> Vec<HostScriptSkirmishApproachPathRequest> {
    HOST_SKIRMISH_APPROACH_PATH_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SKIRMISH_BUILD_BASE_DEFENSE_* / SKIRMISH_BUILD_STRUCTURE_*.
pub fn request_host_skirmish_base_defense(req: HostScriptSkirmishBaseDefenseRequest) {
    HOST_SKIRMISH_BASE_DEFENSE_REQUESTS.with(|q| q.borrow_mut().push(req));
}

pub fn take_host_skirmish_base_defense_requests() -> Vec<HostScriptSkirmishBaseDefenseRequest> {
    HOST_SKIRMISH_BASE_DEFENSE_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE.
pub fn request_host_skirmish_attack_nearest_group(team: &str, comparison: i32, value: i32) {
    HOST_SKIRMISH_ATTACK_GROUP_REQUESTS.with(|q| {
        q.borrow_mut().push((team.to_string(), comparison, value));
    });
}

pub fn take_host_skirmish_attack_nearest_group_requests() -> Vec<(String, i32, i32)> {
    HOST_SKIRMISH_ATTACK_GROUP_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// Live host drain: SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT.
pub fn request_host_skirmish_command_button_most_valuable(team: &str, ability: &str, range: f32) {
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

pub(super) fn current_script_player_name() -> String {
    with_script_engine_ref(|engine| engine.get_current_player_name())
        .flatten()
        .unwrap_or_default()
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
