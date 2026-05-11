//! Script Action and Condition Executor
//!
//! This module implements the script execution engine matching C++ ScriptActions and ScriptConditions.
//! It provides the complete action and condition system for mission scripting.
//!
//! C++ Reference: ScriptActions.cpp, ScriptConditions.cpp
//! Functions: executeAction(), evaluateCondition()

use super::core::*;
use super::engine::{get_area_tracker, get_named_object_tracker, get_script_engine, TFade};
use crate::ai::integration::{with_ai_integration_mut, IntegratedAiPlayer};
use crate::ai::{
    AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, AttitudeType, GuardMode,
};
use crate::commands::commands as cmd_api;
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

fn to_radar_coord(pos: &Coord3D) -> game_engine::common::system::radar::Coord3D {
    game_engine::common::system::radar::Coord3D::new(pos.x, pos.y, pos.z)
}

static TRANSPORT_STATUSES: Lazy<RwLock<HashMap<ObjectID, (u32, usize)>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
static SCRIPT_TEMP_GROUP_ID: AtomicU32 = AtomicU32::new(1);

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

    fn resolve_player_name_token(&self, raw: &str) -> String {
        match raw {
            THE_PLAYER | THIS_PLAYER => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                })
                .unwrap_or_else(|| raw.to_string()),
            LOCAL_PLAYER => player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| {
                    p.read()
                        .ok()
                        .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                })
                .unwrap_or_else(|| raw.to_string()),
            _ => raw.to_string(),
        }
    }

    fn resolve_team_name_token(&self, raw: &str) -> String {
        match raw {
            THIS_TEAM => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref().and_then(|e| {
                        e.get_condition_team_name()
                            .or_else(|| e.get_calling_team_name())
                            .map(|s| s.to_string())
                    })
                })
                .unwrap_or_else(|| raw.to_string()),
            TEAM_THE_PLAYER => {
                let current_player = get_script_engine().read().ok().and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                });
                let Some(player_name) = current_player else {
                    return raw.to_string();
                };

                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(&player_name))
                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                    .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                    .unwrap_or_else(|| raw.to_string())
            }
            _ => raw.to_string(),
        }
    }

    fn relation_from_script_value(&self, relation: i32) -> Relationship {
        match relation {
            0 => Relationship::Enemies, // REL_ENEMY / ENEMIES
            1 => Relationship::Neutral, // REL_NEUTRAL / NEUTRAL
            2 => Relationship::Allies,  // REL_FRIEND / ALLIES
            _ => Relationship::Neutral,
        }
    }

    fn flash_object_by_id(&self, object_id: ObjectID, time_in_seconds: i32, color: Option<Color>) {
        if time_in_seconds <= 0 {
            return;
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };

        let (drawable_arc, flash_color) = if let Ok(object_guard) = object_arc.read() {
            (
                object_guard.get_drawable(),
                color.unwrap_or_else(|| object_guard.get_indicator_color()),
            )
        } else {
            (None, Color::white())
        };

        let Some(drawable_arc) = drawable_arc else {
            return;
        };

        if let Ok(mut drawable_guard) = drawable_arc.write() {
            drawable_guard.script_flash(flash_color, time_in_seconds as f32);
        };
    }

    fn emoticon_object_by_id(&self, object_id: ObjectID, emoticon: &str, duration_frames: i32) {
        if emoticon.is_empty() || duration_frames <= 0 {
            return;
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let drawable_arc = if let Ok(object_guard) = object_arc.read() {
            object_guard.get_drawable()
        } else {
            None
        };
        let Some(drawable_arc) = drawable_arc else {
            return;
        };

        if let Ok(mut drawable_guard) = drawable_arc.write() {
            drawable_guard.script_set_emoticon(emoticon, duration_frames);
        };
    }

    fn resolve_special_power_template_name(&self, power_name: &str) -> Option<String> {
        let store = get_special_power_store()?;
        let template = store.find_special_power_template(power_name)?;
        Some(template.get_name().to_string())
    }

    fn with_named_special_power_module_mut<F>(&self, unit_name: &str, power_name: &str, func: F)
    where
        F: FnOnce(&mut dyn crate::modules::SpecialPowerModuleInterface),
    {
        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(unit_name) else {
            return;
        };
        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let Some(template_name) = self.resolve_special_power_template_name(power_name) else {
            return;
        };

        if let Ok(object_guard) = object_arc.read() {
            let _ = object_guard.with_special_power_module_mut_by_name(&template_name, func);
        };
    }

    /// Execute a script action
    ///
    /// C++ Reference: ScriptActions::executeAction(ScriptAction *pAction)
    pub fn execute_action(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let action_type = action.get_action_type();

        // Dispatch to the appropriate handler based on action type
        let result = match action_type {
            // Victory/Defeat actions
            ScriptActionType::Victory => self.do_victory(),
            ScriptActionType::Defeat => self.do_defeat(),
            ScriptActionType::Quickvictory => self.do_quick_victory(),
            ScriptActionType::Localdefeat => self.do_local_defeat(),

            // Team actions
            ScriptActionType::MoveTeamTo => self.do_move_team_to(action),
            ScriptActionType::TeamAttackTeam => self.do_team_attack_team(action),
            ScriptActionType::TeamHunt => self.do_team_hunt(action),
            ScriptActionType::TeamGuard => self.do_team_guard(action),
            ScriptActionType::TeamDelete => self.do_team_delete(action),
            ScriptActionType::TeamKill => self.do_team_kill(action),
            ScriptActionType::DamageMembersOfTeam => self.do_damage_team_members(action),
            ScriptActionType::TeamSetState => self.do_set_team_state(action),
            ScriptActionType::TeamFollowWaypoints => self.do_team_follow_waypoints(action),

            // Unit creation/deletion actions
            ScriptActionType::CreateObject => self.do_create_object(action),
            ScriptActionType::CreateNamedOnTeamAtWaypoint => {
                self.do_create_named_on_team_at_waypoint(action)
            }
            ScriptActionType::NamedDelete => self.do_named_delete(action),
            ScriptActionType::NamedKill => self.do_named_kill(action),
            ScriptActionType::NamedDamage => self.do_named_damage(action),

            // Named unit actions
            ScriptActionType::MoveNamedUnitTo => self.do_named_move_to_waypoint(action),
            ScriptActionType::NamedAttackNamed => self.do_named_attack_named(action),
            ScriptActionType::NamedHunt => self.do_named_hunt(action),
            ScriptActionType::NamedGuard => self.do_named_guard(action),
            ScriptActionType::NamedStop => self.do_named_stop(action),

            // Player actions
            ScriptActionType::PlayerSetMoney => self.do_set_money(action),
            ScriptActionType::PlayerGiveMoney => self.do_give_money(action),
            ScriptActionType::PlayerGrantScience => self.do_player_grant_science(action),
            ScriptActionType::PlayerKill => self.do_player_kill(action),
            ScriptActionType::PlayerHunt => self.do_player_hunt(action),

            // Display/UI actions
            ScriptActionType::DisplayText => self.do_display_text(action),
            ScriptActionType::DisplayCinematicText => self.do_display_cinematic_text(action),
            ScriptActionType::ShowMilitaryCaption => self.do_military_caption(action),

            // Camera actions
            ScriptActionType::MoveCameraTo => self.do_move_camera_to(action),
            ScriptActionType::CameraFollowNamed => self.do_camera_follow_named(action),
            ScriptActionType::CameraStopFollow => self.do_stop_camera_follow(),
            ScriptActionType::ResetCamera => self.do_reset_camera(action),

            // Audio actions
            ScriptActionType::PlaySoundEffect => self.do_play_sound_effect(action),
            ScriptActionType::PlaySoundEffectAt => self.do_play_sound_effect_at(action),
            ScriptActionType::SpeechPlay => self.do_speech_play(action),
            ScriptActionType::MusicSetTrack => self.do_music_track_change(action),

            // Radar actions
            ScriptActionType::RadarDisable => self.do_radar_disable(),
            ScriptActionType::RadarEnable => self.do_radar_enable(),
            ScriptActionType::MapRevealAtWaypoint => self.do_reveal_map_at_waypoint(action),
            ScriptActionType::MapShroudAtWaypoint => self.do_shroud_map_at_waypoint(action),

            // Input control
            ScriptActionType::DisableInput => self.do_disable_input(),
            ScriptActionType::EnableInput => self.do_enable_input(),

            // ============================================================================
            // COUNTER/FLAG/TIMER ACTIONS - Core scripting state
            // ============================================================================
            ScriptActionType::SetFlag => self.do_set_flag(action),
            ScriptActionType::SetCounter => self.do_set_counter(action),
            ScriptActionType::IncrementCounter => self.do_increment_counter(action),
            ScriptActionType::DecrementCounter => self.do_decrement_counter(action),
            ScriptActionType::SetTimer => self.do_set_timer(action),
            ScriptActionType::SetMillisecondTimer => self.do_set_millisecond_timer(action),
            ScriptActionType::SetRandomTimer => self.do_set_random_timer(action),
            ScriptActionType::SetRandomMsecTimer => self.do_set_random_msec_timer(action),
            ScriptActionType::StopTimer => self.do_stop_timer(action),
            ScriptActionType::RestartTimer => self.do_restart_timer(action),
            ScriptActionType::AddToMsecTimer => self.do_add_to_msec_timer(action),
            ScriptActionType::SubFromMsecTimer => self.do_sub_from_msec_timer(action),

            // ============================================================================
            // SCRIPT CONTROL ACTIONS
            // ============================================================================
            ScriptActionType::NoOp => Ok(ScriptActionResult::Success),
            ScriptActionType::EnableScript => self.do_enable_script(action),
            ScriptActionType::DisableScript => self.do_disable_script(action),
            ScriptActionType::CallSubroutine => self.do_call_subroutine(action),
            ScriptActionType::DebugMessageBox => self.do_debug_message_box(action),
            ScriptActionType::DebugString => self.do_debug_string(action),
            ScriptActionType::DebugCrashBox => self.do_debug_crash_box(action),

            // ============================================================================
            // ADDITIONAL TEAM ACTIONS
            // ============================================================================
            ScriptActionType::BuildTeam => self.do_build_team(action),
            ScriptActionType::RecruitTeam => self.do_recruit_team(action),
            ScriptActionType::CreateReinforcementTeam => self.do_create_reinforcement_team(action),
            ScriptActionType::TeamWander => self.do_team_wander(action),
            ScriptActionType::TeamWanderInPlace => self.do_team_wander_in_place(action),
            ScriptActionType::TeamPanic => self.do_team_panic(action),
            ScriptActionType::TeamStop => self.do_team_stop(action),
            ScriptActionType::TeamStopAndDisband => self.do_team_stop_and_disband(action),
            ScriptActionType::TeamAvailableForRecruitment => {
                self.do_team_available_for_recruitment(action)
            }
            ScriptActionType::TeamCollectNearbyForTeam => self.do_team_collect_nearby(action),
            ScriptActionType::TeamMergeIntoTeam => self.do_team_merge(action),
            ScriptActionType::TeamFlash => self.do_team_flash(action),
            ScriptActionType::TeamFlashWhite => self.do_team_flash_white(action),
            ScriptActionType::TeamTransferToPlayer => self.do_team_transfer_to_player(action),
            ScriptActionType::TeamSetOverrideRelationToTeam => {
                self.do_team_set_override_relation_to_team(action)
            }
            ScriptActionType::TeamRemoveOverrideRelationToTeam => {
                self.do_team_remove_override_relation_to_team(action)
            }
            ScriptActionType::TeamRemoveAllOverrideRelations => {
                self.do_team_remove_all_override_relations(action)
            }
            ScriptActionType::TeamSetOverrideRelationToPlayer => {
                self.do_team_set_override_relation_to_player(action)
            }
            ScriptActionType::TeamRemoveOverrideRelationToPlayer => {
                self.do_team_remove_override_relation_to_player(action)
            }
            ScriptActionType::TeamLoadTransports => self.do_team_load_transports(action),
            ScriptActionType::TeamEnterNamed => self.do_team_enter_named(action),
            ScriptActionType::TeamExitAll => self.do_team_exit_all(action),
            ScriptActionType::TeamGarrisonSpecificBuilding => {
                self.do_team_garrison_specific_building(action)
            }
            ScriptActionType::TeamGarrisonNearestBuilding => {
                self.do_team_garrison_nearest_building(action)
            }
            ScriptActionType::TeamExitAllBuildings => self.do_team_exit_all_buildings(action),
            ScriptActionType::TeamGuardPosition => self.do_team_guard_position(action),
            ScriptActionType::TeamGuardObject => self.do_team_guard_object(action),
            ScriptActionType::TeamGuardArea => self.do_team_guard_area(action),
            ScriptActionType::TeamGuardSupplyCenter => self.do_team_guard_supply_center(action),
            ScriptActionType::TeamGuardInTunnelNetwork => {
                self.do_team_guard_in_tunnel_network(action)
            }
            // C++ parity: ScriptActions::executeAction dispatches TEAM_GUARD_FOR_FRAMECOUNT
            // to doTeamIdleForFramecount.
            ScriptActionType::TeamGuardForFramecount => self.do_team_idle_for_framecount(action),
            ScriptActionType::TeamIdleForFramecount => self.do_team_idle_for_framecount(action),
            ScriptActionType::TeamSpinForFramecount => self.do_team_spin_for_framecount(action),
            ScriptActionType::TeamIncreasePriority => self.do_team_increase_priority(action),
            ScriptActionType::TeamDecreasePriority => self.do_team_decrease_priority(action),
            ScriptActionType::TeamFollowWaypointsExact => {
                self.do_team_follow_waypoints_exact(action)
            }
            ScriptActionType::TeamAttackArea => self.do_team_attack_area(action),
            ScriptActionType::TeamAttackNamed => self.do_team_attack_named(action),
            ScriptActionType::TeamApplyAttackPrioritySet => {
                self.do_team_apply_attack_priority_set(action)
            }
            ScriptActionType::TeamSetAttitude => self.do_team_set_attitude(action),
            ScriptActionType::TeamExecuteSequentialScript => {
                self.do_team_execute_sequential_script(action)
            }
            ScriptActionType::TeamExecuteSequentialScriptLooping => {
                self.do_team_execute_sequential_script_looping(action)
            }
            ScriptActionType::TeamStopSequentialScript => {
                self.do_team_stop_sequential_script(action)
            }
            ScriptActionType::TeamSetEmoticon => self.do_team_set_emoticon(action),
            ScriptActionType::TeamSetStealthEnabled => self.do_team_set_stealth_enabled(action),
            ScriptActionType::TeamSetRepulsor => self.do_team_set_repulsor(action),
            ScriptActionType::TeamCreateRadarEvent => self.do_team_create_radar_event(action),
            ScriptActionType::TeamDeleteLiving => self.do_team_delete_living(action),
            ScriptActionType::TeamWaitForNotContainedAll => {
                self.do_team_wait_for_not_contained_all(action)
            }
            ScriptActionType::TeamWaitForNotContainedPartial => {
                self.do_team_wait_for_not_contained_partial(action)
            }
            ScriptActionType::TeamMoveTowardsNearestObjectType => {
                self.do_team_move_towards_nearest_object_type(action)
            }
            ScriptActionType::TeamHuntWithCommandButton => {
                self.do_team_hunt_with_command_button(action)
            }
            ScriptActionType::TeamUseCommandbuttonAbilityOnNamed => {
                self.do_team_use_command_button_on_named(action)
            }
            ScriptActionType::TeamUseCommandbuttonAbilityAtWaypoint => {
                self.do_team_use_command_button_at_waypoint(action)
            }
            ScriptActionType::TeamUseCommandbuttonAbility => {
                self.do_team_use_command_button(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNamed => {
                self.do_team_all_use_command_button_on_named(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyUnit => {
                self.do_team_all_use_command_button_on_nearest_enemy_unit(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestGarrisonedBuilding => {
                self.do_team_all_use_command_button_on_nearest_garrisoned_building(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestKindof => {
                self.do_team_all_use_command_button_on_nearest_kindof(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyBuilding => {
                self.do_team_all_use_command_button_on_nearest_enemy_building(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyBuildingClass => {
                self.do_team_all_use_command_button_on_nearest_enemy_building_class(action)
            }
            ScriptActionType::TeamAllUseCommandbuttonOnNearestObjecttype => {
                self.do_team_all_use_command_button_on_nearest_object_type(action)
            }
            ScriptActionType::TeamPartialUseCommandbutton => {
                self.do_team_partial_use_command_button(action)
            }
            ScriptActionType::TeamCaptureNearestUnownedFactionUnit => {
                self.do_team_capture_nearest_unowned_faction_unit(action)
            }
            ScriptActionType::TeamAffectObjectPanelFlags => {
                self.do_team_affect_object_panel_flags(action)
            }
            ScriptActionType::TeamSetUnmannedStatus => self.do_team_set_unmanned_status(action),
            ScriptActionType::TeamSetBoobytrapped => self.do_team_set_boobytrapped(action),
            ScriptActionType::TeamFaceNamed => self.do_team_face_named(action),
            ScriptActionType::TeamFaceWaypoint => self.do_team_face_waypoint(action),

            // ============================================================================
            // ADDITIONAL NAMED UNIT ACTIONS
            // ============================================================================
            ScriptActionType::NamedEnterNamed => self.do_named_enter_named(action),
            ScriptActionType::NamedExitAll => self.do_named_exit_all(action),
            ScriptActionType::NamedFollowWaypoints => self.do_named_follow_waypoints(action),
            ScriptActionType::NamedFollowWaypointsExact => {
                self.do_named_follow_waypoints_exact(action)
            }
            ScriptActionType::NamedAttackArea => self.do_named_attack_area(action),
            ScriptActionType::NamedAttackTeam => self.do_named_attack_team(action),
            ScriptActionType::NamedApplyAttackPrioritySet => {
                self.do_named_apply_attack_priority_set(action)
            }
            ScriptActionType::NamedSetAttitude => self.do_named_set_attitude(action),
            ScriptActionType::NamedFlash => self.do_named_flash(action),
            ScriptActionType::NamedFlashWhite => self.do_named_flash_white(action),
            ScriptActionType::NamedGarrisonSpecificBuilding => {
                self.do_named_garrison_specific_building(action)
            }
            ScriptActionType::NamedGarrisonNearestBuilding => {
                self.do_named_garrison_nearest_building(action)
            }
            ScriptActionType::NamedExitBuilding => self.do_named_exit_building(action),
            ScriptActionType::NamedSetStoppingDistance => {
                self.do_named_set_stopping_distance(action)
            }
            ScriptActionType::NamedTransferOwnershipPlayer => {
                self.do_named_transfer_ownership_player(action)
            }
            ScriptActionType::NamedHideSpecialPowerDisplay => {
                self.do_named_hide_special_power_display(action)
            }
            ScriptActionType::NamedShowSpecialPowerDisplay => {
                self.do_named_show_special_power_display(action)
            }
            ScriptActionType::NamedStopSpecialPowerCountdown => {
                self.do_named_stop_special_power_countdown(action)
            }
            ScriptActionType::NamedStartSpecialPowerCountdown => {
                self.do_named_start_special_power_countdown(action)
            }
            ScriptActionType::NamedSetSpecialPowerCountdown => {
                self.do_named_set_special_power_countdown(action)
            }
            ScriptActionType::NamedAddSpecialPowerCountdown => {
                self.do_named_add_special_power_countdown(action)
            }
            ScriptActionType::NamedFireSpecialPowerAtWaypoint => {
                self.do_named_fire_special_power_at_waypoint(action)
            }
            ScriptActionType::NamedFireSpecialPowerAtNamed => {
                self.do_named_fire_special_power_at_named(action)
            }
            ScriptActionType::NamedFireWeaponFollowingWaypointPath => {
                self.do_named_fire_weapon_following_waypoint_path(action)
            }
            ScriptActionType::NamedUseCommandbuttonAbilityOnNamed => {
                self.do_named_use_command_button_on_named(action)
            }
            ScriptActionType::NamedUseCommandbuttonAbilityAtWaypoint => {
                self.do_named_use_command_button_at_waypoint(action)
            }
            ScriptActionType::NamedUseCommandbuttonAbility => {
                self.do_named_use_command_button(action)
            }
            ScriptActionType::NamedUseCommandbuttonAbilityUsingWaypointPath => {
                self.do_named_use_command_button_using_waypoint_path(action)
            }
            ScriptActionType::NamedReceiveUpgrade => self.do_named_receive_upgrade(action),
            ScriptActionType::NamedSetHeld => self.do_named_set_held(action),
            ScriptActionType::NamedSetToppleDirection => self.do_named_set_topple_direction(action),
            ScriptActionType::NamedSetRepulsor => self.do_named_set_repulsor(action),
            ScriptActionType::NamedCustomColor => self.do_named_custom_color(action),
            ScriptActionType::NamedSetStealthEnabled => self.do_named_set_stealth_enabled(action),
            ScriptActionType::NamedSetEmoticon => self.do_named_set_emoticon(action),
            ScriptActionType::NamedFaceNamed => self.do_named_face_named(action),
            ScriptActionType::NamedFaceWaypoint => self.do_named_face_waypoint(action),
            ScriptActionType::NamedSetEvacLeftOrRight => {
                self.do_named_set_evac_left_or_right(action)
            }
            ScriptActionType::NamedSetUnmannedStatus => self.do_named_set_unmanned_status(action),
            ScriptActionType::NamedSetBoobytrapped => self.do_named_set_boobytrapped(action),
            ScriptActionType::UnitExecuteSequentialScript => {
                self.do_unit_execute_sequential_script(action)
            }
            ScriptActionType::UnitExecuteSequentialScriptLooping => {
                self.do_unit_execute_sequential_script_looping(action)
            }
            ScriptActionType::UnitStopSequentialScript => {
                self.do_unit_stop_sequential_script(action)
            }
            ScriptActionType::UnitGuardForFramecount => self.do_unit_guard_for_framecount(action),
            ScriptActionType::UnitIdleForFramecount => self.do_unit_idle_for_framecount(action),
            ScriptActionType::UnitDestroyAllContained => self.do_unit_destroy_all_contained(action),
            ScriptActionType::UnitMoveTowardsNearestObjectType => {
                self.do_unit_move_towards_nearest_object_type(action)
            }
            ScriptActionType::UnitAffectObjectPanelFlags => {
                self.do_unit_affect_object_panel_flags(action)
            }
            ScriptActionType::UnitSpawnNamedLocationOrientation => {
                self.do_unit_spawn_named_location_orientation(action)
            }
            ScriptActionType::CreateUnnamedOnTeamAtWaypoint => {
                self.do_create_unnamed_on_team_at_waypoint(action)
            }

            // ============================================================================
            // ADDITIONAL PLAYER ACTIONS
            // ============================================================================
            ScriptActionType::PlayerSellEverything => self.do_player_sell_everything(action),
            ScriptActionType::PlayerDisableBaseConstruction => {
                self.do_player_disable_base_construction(action)
            }
            ScriptActionType::PlayerDisableFactories => self.do_player_disable_factories(action),
            ScriptActionType::PlayerDisableUnitConstruction => {
                self.do_player_disable_unit_construction(action)
            }
            ScriptActionType::PlayerEnableBaseConstruction => {
                self.do_player_enable_base_construction(action)
            }
            ScriptActionType::PlayerEnableFactories => self.do_player_enable_factories(action),
            ScriptActionType::PlayerEnableUnitConstruction => {
                self.do_player_enable_unit_construction(action)
            }
            ScriptActionType::PlayerTransferOwnershipPlayer => {
                self.do_player_transfer_ownership_player(action)
            }
            ScriptActionType::PlayerRelatesPlayer => self.do_player_relates_player(action),
            ScriptActionType::PlayerSetOverrideRelationToTeam => {
                self.do_player_set_override_relation_to_team(action)
            }
            ScriptActionType::PlayerRemoveOverrideRelationToTeam => {
                self.do_player_remove_override_relation_to_team(action)
            }
            ScriptActionType::PlayerGarrisonAllBuildings => {
                self.do_player_garrison_all_buildings(action)
            }
            ScriptActionType::PlayerExitAllBuildings => self.do_player_exit_all_buildings(action),
            ScriptActionType::PlayerCreateTeamFromCapturedUnits => {
                self.do_player_create_team_from_captured_units(action)
            }
            ScriptActionType::PlayerAddSkillpoints => self.do_player_add_skillpoints(action),
            ScriptActionType::PlayerAddRanklevel => self.do_player_add_ranklevel(action),
            ScriptActionType::PlayerSetRanklevel => self.do_player_set_ranklevel(action),
            ScriptActionType::PlayerSetRanklevellimit => self.do_player_set_ranklevellimit(action),
            ScriptActionType::PlayerPurchaseScience => self.do_player_purchase_science(action),
            ScriptActionType::PlayerRepairNamedStructure => {
                self.do_player_repair_named_structure(action)
            }
            ScriptActionType::PlayerAffectReceivingExperience => {
                self.do_player_affect_receiving_experience(action)
            }
            ScriptActionType::PlayerExcludeFromScoreScreen => {
                self.do_player_exclude_from_score_screen(action)
            }
            ScriptActionType::PlayerScienceAvailability => {
                self.do_player_science_availability(action)
            }
            ScriptActionType::PlayerSelectSkillset => self.do_player_select_skillset(action),

            // ============================================================================
            // ADDITIONAL CAMERA ACTIONS
            // ============================================================================
            ScriptActionType::MoveCameraAlongWaypointPath => {
                self.do_move_camera_along_waypoint_path(action)
            }
            ScriptActionType::RotateCamera => self.do_rotate_camera(action),
            ScriptActionType::MoveCameraToSelection => self.do_move_camera_to_selection(action),
            ScriptActionType::CameraMoveHome => self.do_camera_move_home(action),
            ScriptActionType::SetupCamera => self.do_setup_camera(action),
            ScriptActionType::CameraLetterboxBegin => self.do_camera_letterbox_begin(action),
            ScriptActionType::CameraLetterboxEnd => self.do_camera_letterbox_end(),
            ScriptActionType::ZoomCamera => self.do_zoom_camera(action),
            ScriptActionType::PitchCamera => self.do_pitch_camera(action),
            ScriptActionType::OversizeTerrain => self.do_oversize_terrain(action),
            ScriptActionType::CameraFadeAdd => self.do_camera_fade_add(action),
            ScriptActionType::CameraFadeSubtract => self.do_camera_fade_subtract(action),
            ScriptActionType::CameraFadeSaturate => self.do_camera_fade_saturate(action),
            ScriptActionType::CameraFadeMultiply => self.do_camera_fade_multiply(action),
            ScriptActionType::CameraBwModeBegin => self.do_camera_bw_mode_begin(action),
            ScriptActionType::CameraBwModeEnd => self.do_camera_bw_mode_end(action),
            ScriptActionType::DrawSkyboxBegin => self.do_draw_skybox_begin(),
            ScriptActionType::DrawSkyboxEnd => self.do_draw_skybox_end(),
            ScriptActionType::CameraMotionBlur => self.do_camera_motion_blur(action),
            ScriptActionType::CameraMotionBlurJump => self.do_camera_motion_blur_jump(action),
            ScriptActionType::CameraMotionBlurFollow => self.do_camera_motion_blur_follow(action),
            ScriptActionType::CameraMotionBlurEndFollow => self.do_camera_motion_blur_end_follow(),
            ScriptActionType::CameraSetAudibleDistance => {
                self.do_camera_set_audible_distance(action)
            }
            ScriptActionType::CameraTetherNamed => self.do_camera_tether_named(action),
            ScriptActionType::CameraStopTetherNamed => self.do_camera_stop_tether_named(),
            ScriptActionType::CameraSetDefault => self.do_camera_set_default(action),
            ScriptActionType::CameraLookTowardObject => self.do_camera_look_toward_object(action),
            ScriptActionType::CameraLookTowardWaypoint => {
                self.do_camera_look_toward_waypoint(action)
            }
            ScriptActionType::CameraModFreezeTime => self.do_camera_mod_freeze_time(),
            ScriptActionType::CameraModSetFinalZoom => self.do_camera_mod_set_final_zoom(action),
            ScriptActionType::CameraModSetFinalPitch => self.do_camera_mod_set_final_pitch(action),
            ScriptActionType::CameraModFreezeAngle => self.do_camera_mod_freeze_angle(),
            ScriptActionType::CameraModSetFinalSpeedMultiplier => {
                self.do_camera_mod_set_final_speed_multiplier(action)
            }
            ScriptActionType::CameraModSetRollingAverage => {
                self.do_camera_mod_set_rolling_average(action)
            }
            ScriptActionType::CameraModFinalLookToward => {
                self.do_camera_mod_final_look_toward(action)
            }
            ScriptActionType::CameraModLookToward => self.do_camera_mod_look_toward(action),
            ScriptActionType::CameraEnableSlaveMode => self.do_camera_enable_slave_mode(action),
            ScriptActionType::CameraDisableSlaveMode => self.do_camera_disable_slave_mode(),
            ScriptActionType::CameraAddShakerAt => self.do_camera_add_shaker_at(action),
            ScriptActionType::ScreenShake => self.do_screen_shake(action),

            // ============================================================================
            // ADDITIONAL AUDIO/VIDEO ACTIONS
            // ============================================================================
            ScriptActionType::SoundPlayNamed => self.do_sound_play_named(action),
            ScriptActionType::SuspendBackgroundSounds => self.do_suspend_background_sounds(),
            ScriptActionType::ResumeBackgroundSounds => self.do_resume_background_sounds(),
            ScriptActionType::SoundAmbientPause => self.do_sound_ambient_pause(),
            ScriptActionType::SoundAmbientResume => self.do_sound_ambient_resume(),
            ScriptActionType::MusicSetVolume => self.do_music_set_volume(action),
            ScriptActionType::SoundDisableType => self.do_sound_disable_type(action),
            ScriptActionType::SoundEnableType => self.do_sound_enable_type(action),
            ScriptActionType::SoundEnableAll => self.do_sound_enable_all(),
            ScriptActionType::AudioOverrideVolumeType => self.do_audio_override_volume_type(action),
            ScriptActionType::AudioRestoreVolumeType => self.do_audio_restore_volume_type(action),
            ScriptActionType::AudioRestoreVolumeAllType => self.do_audio_restore_volume_all_type(),
            ScriptActionType::SoundSetVolume => self.do_sound_set_volume(action),
            ScriptActionType::SpeechSetVolume => self.do_speech_set_volume(action),
            ScriptActionType::SoundRemoveAllDisabled => self.do_sound_remove_all_disabled(),
            ScriptActionType::SoundRemoveType => self.do_sound_remove_type(action),
            ScriptActionType::EnableObjectSound => self.do_enable_object_sound(action),
            ScriptActionType::DisableObjectSound => self.do_disable_object_sound(action),
            ScriptActionType::MoviePlayFullscreen => self.do_movie_play_fullscreen(action),
            ScriptActionType::MoviePlayRadar => self.do_movie_play_radar(action),

            // ============================================================================
            // RADAR/MAP ACTIONS
            // ============================================================================
            ScriptActionType::RadarCreateEvent => self.do_radar_create_event(action),
            ScriptActionType::RadarForceEnable => self.do_radar_force_enable(),
            ScriptActionType::RadarRevertToNormal => self.do_radar_revert_to_normal(),
            ScriptActionType::MapRevealAll => self.do_map_reveal_all(action),
            ScriptActionType::MapRevealAllPerm => self.do_map_reveal_all_perm(action),
            ScriptActionType::MapRevealAllUndoPerm => self.do_map_reveal_all_undo_perm(action),
            ScriptActionType::MapShroudAll => self.do_map_shroud_all(action),
            ScriptActionType::MapRevealPermanentlyAtWaypoint => {
                self.do_map_reveal_permanently_at_waypoint(action)
            }
            ScriptActionType::MapUndoRevealPermanentlyAtWaypoint => {
                self.do_map_undo_reveal_permanently_at_waypoint(action)
            }
            ScriptActionType::MapSwitchBorder => self.do_map_switch_border(action),
            ScriptActionType::RefreshRadar => self.do_refresh_radar(),
            ScriptActionType::ObjectCreateRadarEvent => self.do_object_create_radar_event(action),
            ScriptActionType::DisableBorderShroud => self.do_disable_border_shroud(),
            ScriptActionType::EnableBorderShroud => self.do_enable_border_shroud(),
            ScriptActionType::ResizeViewGuardband => self.do_resize_view_guardband(action),

            // ============================================================================
            // DISPLAY/UI ACTIONS
            // ============================================================================
            ScriptActionType::CameoFlash => self.do_cameo_flash(action),
            ScriptActionType::DisplayCountdownTimer => self.do_display_countdown_timer(action),
            ScriptActionType::HideCountdownTimer => self.do_hide_countdown_timer(action),
            ScriptActionType::EnableCountdownTimerDisplay => {
                self.do_enable_countdown_timer_display()
            }
            ScriptActionType::DisableCountdownTimerDisplay => {
                self.do_disable_countdown_timer_display()
            }
            ScriptActionType::DisplayCounter => self.do_display_counter(action),
            ScriptActionType::HideCounter => self.do_hide_counter(action),
            ScriptActionType::DisableSpecialPowerDisplay => self.do_disable_special_power_display(),
            ScriptActionType::EnableSpecialPowerDisplay => self.do_enable_special_power_display(),
            ScriptActionType::IngamePopupMessage => self.do_ingame_popup_message(action),
            ScriptActionType::ObjectForceSelect => self.do_object_force_select(action),

            // ============================================================================
            // TIME CONTROL
            // ============================================================================
            ScriptActionType::FreezeTime => self.do_freeze_time(),
            ScriptActionType::UnfreezeTime => self.do_unfreeze_time(),
            ScriptActionType::SetVisualSpeedMultiplier => {
                self.do_set_visual_speed_multiplier(action)
            }
            ScriptActionType::SetFpsLimit => self.do_set_fps_limit(action),

            // ============================================================================
            // ENVIRONMENT/WORLD ACTIONS
            // ============================================================================
            ScriptActionType::SetTreeSway => self.do_set_tree_sway(action),
            ScriptActionType::WaterChangeHeight => self.do_water_change_height(action),
            ScriptActionType::WaterChangeHeightOverTime => {
                self.do_water_change_height_over_time(action)
            }
            ScriptActionType::SetCaveIndex => self.do_set_cave_index(action),
            ScriptActionType::ShowWeather => self.do_show_weather(action),
            ScriptActionType::SetInfantryLightingOverride => {
                self.do_set_infantry_lighting_override(action)
            }
            ScriptActionType::ResetInfantryLightingOverride => {
                self.do_reset_infantry_lighting_override()
            }

            // ============================================================================
            // CONSTRUCTION/TECHTREE ACTIONS
            // ============================================================================
            ScriptActionType::SetBaseConstructionSpeed => {
                self.do_set_base_construction_speed(action)
            }
            ScriptActionType::TechtreeModifyBuildabilityObject => {
                self.do_techtree_modify_buildability_object(action)
            }
            ScriptActionType::WarehouseSetValue => self.do_warehouse_set_value(action),
            ScriptActionType::CommandbarRemoveButtonObjecttype => {
                self.do_command_bar_remove_button_object_type(action)
            }
            ScriptActionType::CommandbarAddButtonObjecttypeSlot => {
                self.do_command_bar_add_button_object_type_slot(action)
            }

            // ============================================================================
            // ATTACK PRIORITY ACTIONS
            // ============================================================================
            ScriptActionType::SetAttackPriorityThing => self.do_set_attack_priority_thing(action),
            ScriptActionType::SetAttackPriorityKindOf => self.do_set_attack_priority_kindof(action),
            ScriptActionType::SetDefaultAttackPriority => {
                self.do_set_default_attack_priority(action)
            }
            ScriptActionType::SetStoppingDistance => self.do_set_stopping_distance(action),

            // ============================================================================
            // OBJECT LIST ACTIONS
            // ============================================================================
            ScriptActionType::ObjectlistAddobjecttype => {
                self.do_object_list_add_object_type(action)
            }
            ScriptActionType::ObjectlistRemoveobjecttype => {
                self.do_object_list_remove_object_type(action)
            }
            ScriptActionType::ObjectAllowBonuses => self.do_object_allow_bonuses(action),
            ScriptActionType::DeleteAllUnmanned => self.do_delete_all_unmanned(action),
            ScriptActionType::ChooseVictimAlwaysUsesNormal => {
                self.do_choose_victim_always_uses_normal(action)
            }
            ScriptActionType::ScriptingOverrideHulkLifetime => {
                self.do_scripting_override_hulk_lifetime(action)
            }

            // ============================================================================
            // AI/SKIRMISH ACTIONS
            // ============================================================================
            ScriptActionType::SkirmishBuildBuilding => self.do_skirmish_build_building(action),
            ScriptActionType::SkirmishFollowApproachPath => {
                self.do_skirmish_follow_approach_path(action)
            }
            ScriptActionType::SkirmishMoveToApproachPath => {
                self.do_skirmish_move_to_approach_path(action)
            }
            ScriptActionType::SkirmishBuildBaseDefenseFront => {
                self.do_skirmish_build_base_defense_front(action)
            }
            ScriptActionType::SkirmishBuildBaseDefenseFlank => {
                self.do_skirmish_build_base_defense_flank(action)
            }
            ScriptActionType::SkirmishBuildStructureFront => {
                self.do_skirmish_build_structure_front(action)
            }
            ScriptActionType::SkirmishBuildStructureFlank => {
                self.do_skirmish_build_structure_flank(action)
            }
            ScriptActionType::SkirmishFireSpecialPowerAtMostCost => {
                self.do_skirmish_fire_special_power_at_most_cost(action)
            }
            ScriptActionType::SkirmishAttackNearestGroupWithValue => {
                self.do_skirmish_attack_nearest_group_with_value(action)
            }
            ScriptActionType::SkirmishPerformCommandbuttonOnMostValuableObject => {
                self.do_skirmish_perform_command_button_on_most_valuable_object(action)
            }
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll => {
                self.do_skirmish_wait_for_command_button_available_all(action)
            }
            ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial => {
                self.do_skirmish_wait_for_command_button_available_partial(action)
            }
            ScriptActionType::AiPlayerBuildSupplyCenter => {
                self.do_ai_player_build_supply_center(action)
            }
            ScriptActionType::AiPlayerBuildUpgrade => self.do_ai_player_build_upgrade(action),
            ScriptActionType::AiPlayerBuildTypeNearestTeam => {
                self.do_ai_player_build_type_nearest_team(action)
            }
            ScriptActionType::IdleAllUnits => self.do_idle_all_units(action),
            ScriptActionType::ResumeSupplyTrucking => self.do_resume_supply_trucking(action),

            // ============================================================================
            // EVA/MISC ACTIONS
            // ============================================================================
            ScriptActionType::EvaSetEnabledDisabled => self.do_eva_set_enabled_disabled(action),
            ScriptActionType::OptionsSetOcclusionMode => self.do_options_set_occlusion_mode(action),
            ScriptActionType::OptionsSetDrawiconUiMode => {
                self.do_options_set_draw_icon_ui_mode(action)
            }
            ScriptActionType::OptionsSetParticleCapMode => {
                self.do_options_set_particle_cap_mode(action)
            }
            ScriptActionType::ExitSpecificBuilding => self.do_exit_specific_building(action),
            ScriptActionType::EnableScoring => self.do_enable_scoring(),
            ScriptActionType::DisableScoring => self.do_disable_scoring(),
            ScriptActionType::SetTrainHeld => self.do_set_train_held(action),

            ScriptActionType::NumItems => Ok(ScriptActionResult::Success),
        };

        match result {
            Err(ScriptError::TeamNotFound(name)) => {
                log::warn!(
                    "Script action {:?} skipped because team '{}' was not found",
                    action_type,
                    name
                );
                Ok(ScriptActionResult::Success)
            }
            Err(ScriptError::ObjectNotFound(name)) => {
                log::warn!(
                    "Script action {:?} skipped because object '{}' was not found",
                    action_type,
                    name
                );
                Ok(ScriptActionResult::Success)
            }
            Err(ScriptError::PlayerNotFound(name)) => {
                log::warn!(
                    "Script action {:?} skipped because player '{}' was not found",
                    action_type,
                    name
                );
                Ok(ScriptActionResult::Success)
            }
            other => other,
        }
    }

    // ============================================================================
    // VICTORY/DEFEAT ACTIONS
    // C++ Reference: ScriptActions.cpp lines 215-276
    // ============================================================================

    /// C++ Reference: ScriptActions::doVictory() line 215
    fn do_victory(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("VICTORY!");

        let mut ctx = self.context.write().unwrap();
        ctx.suppress_new_windows = false;

        TheVictoryConditions::set_local_allied_victory(true);
        if let Ok(players) = player_list().read() {
            if let Some(local_player) = players.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(false);
                }
            }
        }
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.start_end_game_timer();
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doQuickVictory() line 193
    fn do_quick_victory(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("QUICK VICTORY!");

        let mut ctx = self.context.write().unwrap();
        ctx.suppress_new_windows = false;

        TheVictoryConditions::set_local_allied_victory(true);
        if let Ok(players) = player_list().read() {
            if let Some(local_player) = players.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(false);
                }
            }
        }
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.start_quick_end_game_timer();
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doDefeat() line 239
    fn do_defeat(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("DEFEAT!");

        let mut ctx = self.context.write().unwrap();
        ctx.suppress_new_windows = false;

        TheVictoryConditions::set_local_allied_victory(false);
        if let Ok(players) = player_list().read() {
            if let Some(local_player) = players.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(true);
                }
            }
        }
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.start_end_game_timer();
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doLocalDefeat() line 263
    fn do_local_defeat(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("LOCAL DEFEAT (multiplayer)");

        TheVictoryConditions::set_local_allied_victory(false);
        if let Ok(players) = player_list().read() {
            if let Some(local_player) = players.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(true);
                }
            }
        }
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.set_shown_mp_local_defeat_window(true);
                engine.start_close_window_timer();
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // TEAM ACTIONS
    // C++ Reference: ScriptActions.cpp lines 413-435 (move team)
    // ============================================================================

    /// C++ Reference: ScriptActions::doMoveToWaypoint() line 413
    fn do_move_team_to(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let waypoint_name = self.get_string_param(action, 1)?;

        log::info!(
            "Moving team '{}' to waypoint '{}'",
            team_name,
            waypoint_name
        );

        let destination = self.get_waypoint_position(&waypoint_name)?;
        let group_arc = self.create_ai_group_from_team(&team_name)?;

        if let Ok(group) = group_arc.read() {
            group.group_move_to_position(&destination, false, CommandSourceType::FromScript);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamAttackNamed() line (in header)
    /// C++ Reference: ScriptActions::doTeamAttackTeam()
    /// Creates AI group from attacker team and issues attack command on victim team
    fn do_team_attack_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let attacker_team = self.get_string_param(action, 0)?;
        let victim_team = self.get_string_param(action, 1)?;

        log::info!("Team '{}' attacking team '{}'", attacker_team, victim_team);

        let victim_team = self.resolve_team_name_token(&victim_team);
        if self.get_team_by_name(&victim_team).is_err() {
            log::warn!("Victim team '{}' not found for team attack", victim_team);
            return Ok(ScriptActionResult::Success);
        }

        // Create AI group from attacker team
        let group_arc = self.create_ai_group_from_team(&attacker_team)?;

        // Issue attack command to group targeting victim team
        // C++: aiGroup->groupAttackTeam(victimTeam, NO_MAX_SHOTS_LIMIT, CMD_FROM_SCRIPT)
        if let Ok(mut group) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackTeam, CommandSourceType::FromScript);
            params.team = Some(victim_team);
            params.int_value = -1; // NO_MAX_SHOTS_LIMIT
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamHunt() lines 1985-1999
    /// Creates AI group from team and issues hunt command
    fn do_team_hunt(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);

        log::info!("Team '{}' hunting", team_name);

        // Create AI group from team and issue hunt command
        // C++: theGroup->groupHunt(CMD_FROM_SCRIPT)
        let group_arc = self.create_ai_group_from_team(&team_name)?;

        if let Ok(mut group) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Hunt, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGuard() lines 1882-1900
    /// Orders team members to guard at their current positions
    fn do_team_guard(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' guarding at current positions", team_name);

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = team_arc
            .read()
            .map_err(|e| ScriptError::ExecutionFailed(format!("Failed to read team: {}", e)))?
            .get_members()
            .to_vec();

        for object_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            let position = *obj.get_position();
            let Some(ai_arc) = obj.get_ai_update_interface() else {
                continue;
            };
            drop(obj);

            if let Ok(mut ai) = ai_arc.lock() {
                let mut params = AiCommandParams::new(
                    AiCommandType::GuardPosition,
                    CommandSourceType::FromScript,
                );
                params.pos = position;
                params.int_value = GuardMode::Normal.as_i32();
                let _ = ai.execute_command(&params);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamDelete() line (in header)
    fn do_team_delete(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);

        log::info!("Deleting team '{}'", team_name);

        // C++ parity: TeamDelete delegates to Team::deleteTeam(ignoreDead=false).
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.delete_team(false);
                    log::info!("Team '{}' deleted successfully", team_name);
                }
            } else {
                log::warn!("Team '{}' not found for deletion", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamKill() line (in header)
    fn do_team_kill(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);

        log::info!("Killing team '{}'", team_name);

        // Get team by name and kill all members
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    // Kill all team members (with death effects)
                    team_guard.kill_team();
                    log::info!("Team '{}' killed successfully", team_name);
                }
            } else {
                log::warn!("Team '{}' not found for kill", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doDamageTeamMembers() line 400
    fn do_damage_team_members(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let damage_amount = self.get_real_param(action, 1)?;

        log::info!("Damaging team '{}' for {} points", team_name, damage_amount);

        // Get team by name and damage all members
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.damage_team_members(damage_amount);
                    log::info!("Team '{}' damaged for {} points", team_name, damage_amount);
                }
            } else {
                log::warn!("Team '{}' not found for damage", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetTeamState() line 492
    fn do_set_team_state(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let state_name = self.get_string_param(action, 1)?;

        log::info!("Setting team '{}' state to '{}'", team_name, state_name);

        // Get team by name and set its state
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.set_state(state_name.clone().into());
                    log::info!("Team '{}' state set to '{}'", team_name, state_name);
                }
            } else {
                log::warn!("Team '{}' not found for state change", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFollowWaypoints() line (in header)
    /// Creates AI group from team and issues follow waypoint path command
    fn do_team_follow_waypoints(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let waypoint_path_name = self.get_string_param(action, 1)?;

        log::debug!(
            "Team '{}' following waypoint path '{}'",
            team_name,
            waypoint_path_name
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let Some(team_center) = self
            .compute_team_center_and_first(&team_arc)
            .map(|(center, _)| center)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let waypoint_id = self.resolve_follow_waypoint_id(&waypoint_path_name, team_center);

        if let Some(wid) = waypoint_id {
            // Create AI group from team and issue follow waypoints command
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            // Use explicit scope to ensure proper lifetime
            {
                if let Ok(mut group) = group_arc.write() {
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowWaypointPath,
                        CommandSourceType::FromScript,
                    );
                    params.waypoint = Some(wid);
                    let _ = group.ai_do_command(&params);
                }
            }
            log::debug!(
                "Team '{}' following waypoints from '{}'",
                team_name,
                waypoint_path_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // UNIT CREATION/DELETION ACTIONS
    // C++ Reference: ScriptActions.cpp line (create object)
    // ============================================================================

    /// C++ Reference: ScriptActions::doCreateObject() line (in header)
    fn do_create_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // C++ Reference: ScriptActions.cpp switch case ScriptAction::CREATE_OBJECT
        // Parameters:
        //  0: object type (template name)
        //  1: team name
        //  2: coord3d position
        //  3: angle
        let object_type = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        let position = self.get_coord_param(action, 2)?;
        let position = crate::common::Coord3D::new(position.x, position.y, position.z);
        let angle = self.get_real_param(action, 3)?;

        log::info!(
            "Creating object of type '{}' on team '{}' at ({}, {}, {}) angle {}",
            object_type,
            team_name,
            position.x,
            position.y,
            position.z,
            angle
        );

        let team_arc = if team_name.trim().is_empty() {
            None
        } else {
            self.get_or_create_team_by_name(&team_name).ok()
        };

        let object_id = {
            let manager_arc = get_object_manager();
            let Ok(mut manager) = manager_arc.write() else {
                log::warn!("CREATE_OBJECT: failed to lock ObjectManager");
                return Ok(ScriptActionResult::Success);
            };

            match manager.create_object(
                &object_type,
                position,
                team_arc.clone(),
                crate::object_manager::ObjectCreationFlags::from_template(),
            ) {
                Ok(id) => id,
                Err(err) => {
                    log::warn!(
                        "CREATE_OBJECT: failed to create '{}' on team '{}': {}",
                        object_type,
                        team_name,
                        err
                    );
                    return Ok(ScriptActionResult::Success);
                }
            }
        };

        if let Some(team_arc) = &team_arc {
            if let Ok(mut team) = team_arc.write() {
                team.add_member(object_id);
            }
        }

        if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(mut obj) = obj_arc.write() {
                let _ = obj.set_orientation(angle);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_create_named_on_team_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let team_name = self.get_string_param(action, 2)?;
        let waypoint_name = self.get_string_param(action, 3)?;

        log::debug!(
            "Creating named unit '{}' of type '{}' on team '{}' at waypoint '{}'",
            unit_name,
            object_type,
            team_name,
            waypoint_name
        );

        let _ = self.create_unit_on_team_at_waypoint(
            Some(&unit_name),
            &object_type,
            &team_name,
            &waypoint_name,
        )?;

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doNamedDelete() line (in header)
    fn do_named_delete(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Deleting named unit '{}'", unit_name);

        // Look up object ID by name and delete
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object manager and destroy the object
            let manager_arc = get_object_manager();
            let _ = manager_arc.write().ok().map(|mut mgr_guard| {
                mgr_guard.destroy_object(object_id);
                log::info!("Named unit '{}' deleted (ID: {})", unit_name, object_id);
            });
        } else {
            log::warn!("Named unit '{}' not found for deletion", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doNamedKill() line (in header)
    fn do_named_kill(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Killing named unit '{}'", unit_name);

        // Look up object ID by name and kill
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object from manager and kill/destroy it
            let manager_arc = get_object_manager();
            let obj_arc_opt = manager_arc
                .read()
                .ok()
                .and_then(|mgr| mgr.get_object(object_id));
            if let Some(obj_arc) = obj_arc_opt {
                let _ = obj_arc.write().ok().map(|mut obj_guard| {
                    // Use destroy() which handles death with effects
                    obj_guard.destroy();
                    log::info!("Named unit '{}' killed (ID: {})", unit_name, object_id);
                });
            }
        } else {
            log::warn!("Named unit '{}' not found for kill", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doNamedDamage() line (in header)
    fn do_named_damage(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let damage_amount = self.get_int_param(action, 1)?;

        log::info!(
            "Damaging named unit '{}' for {} points",
            unit_name,
            damage_amount
        );

        // Look up object ID by name and apply damage
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object from manager and apply damage
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let _ = obj_arc.write().ok().map(|mut obj_guard| {
                    // Create damage info with script damage (unresistable type)
                    let mut damage_info = DamageInfo::with_simple(
                        damage_amount as f32,
                        0, // No source object for script damage
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    let _ = obj_guard.attempt_damage(&mut damage_info);
                    log::info!(
                        "Named unit '{}' damaged for {} points (ID: {})",
                        unit_name,
                        damage_amount,
                        object_id
                    );
                });
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for damage", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // NAMED UNIT ACTIONS
    // C++ Reference: ScriptActions.cpp line 438 (named move)
    // ============================================================================

    /// C++ Reference: ScriptActions::doNamedMoveToWaypoint() line 438
    fn do_named_move_to_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;

        log::info!(
            "Moving named unit '{}' to waypoint '{}'",
            unit_name,
            waypoint_name
        );

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get waypoint position from terrain logic
            let waypoint_name_ascii = AsciiString::from(waypoint_name.as_str());
            let waypoint_pos = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_name_ascii)
                    .map(|w| w.get_location().clone())
            });

            if let Some(position) = waypoint_pos {
                // Get the object to find player ID
                if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                    let player_id = obj_arc
                        .read()
                        .ok()
                        .and_then(|obj| obj.get_controlling_player_id().map(|id| id as i32));

                    if let Some(pid) = player_id {
                        let current_frame = TheGameLogic::get_frame();
                        if let Err(e) = cmd_api::move_objects_to_position(
                            vec![object_id],
                            position.clone(),
                            pid,
                            current_frame,
                        ) {
                            log::warn!(
                                "Failed to move '{}' to waypoint '{}': {}",
                                unit_name,
                                waypoint_name,
                                e
                            );
                        } else {
                            log::info!(
                                "Named unit '{}' moving to waypoint '{}' at ({:.1}, {:.1}, {:.1})",
                                unit_name,
                                waypoint_name,
                                position.x,
                                position.y,
                                position.z
                            );
                        }
                    } else {
                        log::warn!("Named unit '{}' has no controlling player", unit_name);
                    }
                } else {
                    log::warn!("Named unit '{}' not found in object registry", unit_name);
                }
            } else {
                log::warn!("Waypoint '{}' not found for move", waypoint_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for move to waypoint", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_attack_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let attacker_name = self.get_string_param(action, 0)?;
        let victim_name = self.get_string_param(action, 1)?;

        log::info!("Named unit '{}' attacking '{}'", attacker_name, victim_name);

        // Look up attacker and victim object IDs by name
        let tracker = get_named_object_tracker();
        let attacker_id = tracker.get_object_id(&attacker_name).ok().flatten();
        let victim_id = tracker.get_object_id(&victim_name).ok().flatten();

        match (attacker_id, victim_id) {
            (Some(attacker), Some(target)) => {
                if TheGameLogic::find_object_by_id(target).is_none() {
                    log::warn!("Victim '{}' not found in object registry", victim_name);
                    return Ok(ScriptActionResult::Success);
                }

                let Some(obj_arc) = TheGameLogic::find_object_by_id(attacker) else {
                    log::warn!("Attacker '{}' not found in object registry", attacker_name);
                    return Ok(ScriptActionResult::Success);
                };

                if let Ok(mut obj_guard) = obj_arc.write() {
                    let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                        log::warn!("Attacker '{}' has no AI update interface", attacker_name);
                        return Ok(ScriptActionResult::Success);
                    };
                    obj_guard.leave_group();
                    if let Ok(mut ai_guard) = ai_arc.lock() {
                        let _ =
                            ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                        let mut params = AiCommandParams::new(
                            AiCommandType::ForceAttackObject,
                            CommandSourceType::FromScript,
                        );
                        params.obj = Some(target);
                        params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                        let _ = ai_guard.execute_command(&params);
                        log::info!(
                            "Named unit '{}' (ID: {}) force attacking '{}' (ID: {})",
                            attacker_name,
                            attacker,
                            victim_name,
                            target
                        );
                    };
                };
            }
            (None, _) => {
                log::warn!("Attacker '{}' not found for attack", attacker_name);
            }
            (_, None) => {
                log::warn!("Victim '{}' not found for attack", victim_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_hunt(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Named unit '{}' hunting", unit_name);

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object and issue hunt command via AI interface
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());
                if let Some(ai_arc) = ai_result {
                    let hunt_params =
                        AiCommandParams::new(AiCommandType::Hunt, CommandSourceType::FromScript);
                    let _ = ai_arc.lock().ok().map(|mut ai| {
                        let _ = ai.execute_command(&hunt_params);
                        log::info!(
                            "Named unit '{}' hunt command issued (ID: {})",
                            unit_name,
                            object_id
                        );
                    });
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for hunt", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_guard(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Named unit '{}' guarding", unit_name);

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object and issue guard command via AI interface
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                // Get object's current position for guard position
                let position = obj_arc
                    .read()
                    .ok()
                    .map(|obj| obj.get_position().clone())
                    .unwrap_or_default();

                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());
                if let Some(ai_arc) = ai_result {
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard.leave_group();
                    }
                    if let Ok(mut ai) = ai_arc.lock() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    }

                    let mut guard_params = AiCommandParams::new(
                        AiCommandType::GuardPosition,
                        CommandSourceType::FromScript,
                    );
                    guard_params.pos = position;
                    guard_params.int_value = GuardMode::Normal.as_i32();
                    let _ = ai_arc.lock().ok().map(|mut ai| {
                        let _ = ai.execute_command(&guard_params);
                        log::info!(
                            "Named unit '{}' guard command issued (ID: {}) at ({:.1}, {:.1}, {:.1})",
                            unit_name, object_id, position.x, position.y, position.z
                        );
                    });
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for guard", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_stop(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;

        log::info!("Named unit '{}' stopping", unit_name);

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            // Get the object to find player ID
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let player_id = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_controlling_player_id().map(|id| id as i32));

                if let Some(pid) = player_id {
                    let current_frame = TheGameLogic::get_frame();
                    if let Err(e) = cmd_api::stop_objects(vec![object_id], pid, current_frame) {
                        log::warn!("Failed to stop named unit '{}': {}", unit_name, e);
                    } else {
                        log::info!(
                            "Named unit '{}' stop command issued (ID: {})",
                            unit_name,
                            object_id
                        );
                    }
                } else {
                    log::warn!("Named unit '{}' has no controlling player", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for stop", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // PLAYER ACTIONS
    // C++ Reference: ScriptActions.cpp line (set money)
    // ============================================================================

    /// C++ Reference: ScriptActions::doSetMoney() line (in header)
    fn do_set_money(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let money_amount = self.get_int_param(action, 1)?;

        log::info!("Setting player '{}' money to {}", player_name, money_amount);

        // Get player by name and set money
        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.get_money_mut().set_money(money_amount);
                    log::info!("Player '{}' money set to {}", player_name, money_amount);
                }
            } else {
                log::warn!("Player '{}' not found for set money", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doGiveMoney() line (in header)
    fn do_give_money(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let money_amount = self.get_int_param(action, 1)?;

        log::info!("Giving player '{}' {} money", player_name, money_amount);

        // Get player by name and add money (can be negative)
        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.get_money_mut().add_money(money_amount);
                    log::info!("Player '{}' received {} money", player_name, money_amount);
                }
            } else {
                log::warn!("Player '{}' not found for give money", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_grant_science(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};

        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let science_name = self.get_string_param(action, 1)?;

        log::info!(
            "Granting player '{}' science '{}'",
            player_name,
            science_name
        );

        // Look up the science type by name
        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.grant_science(science_type);
                    log::info!(
                        "Player '{}' granted science '{}'",
                        player_name,
                        science_name
                    );
                }
            } else {
                log::warn!("Player '{}' not found for grant science", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doPlayerKill()
    /// Kills all units and buildings belonging to a player
    fn do_player_kill(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);

        log::info!("Killing all units for player '{}' (scripted)", player_name);

        let list = player_list();
        let Ok(list_guard) = list.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(player_arc) = list_guard.find_player_by_name(&player_name) else {
            log::warn!("Player '{}' not found for kill", player_name);
            return Ok(ScriptActionResult::Success);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let player_index = player.get_player_index() as u32;
        drop(player);

        // Match C++ intent: destroy all player-owned objects.
        let obj_mgr = get_object_manager();
        let Ok(obj_mgr_guard) = obj_mgr.read() else {
            return Ok(ScriptActionResult::Success);
        };

        let owned = obj_mgr_guard.get_objects_owned_by_player(player_index);
        for object_id in owned {
            let Some(obj_arc) = obj_mgr_guard.get_object(object_id) else {
                continue;
            };
            let Ok(mut obj) = obj_arc.write() else {
                continue;
            };
            obj.destroy();
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_hunt(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);

        log::info!("Player '{}' units hunting", player_name);

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_units_should_hunt(true, CommandSourceType::FromScript);
                    log::info!("Player '{}' units now hunting", player_name);
                }
            } else {
                log::warn!("Player '{}' not found for hunt", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // DISPLAY/UI ACTIONS
    // C++ Reference: ScriptActions.cpp line (display text)
    // ============================================================================

    fn do_display_text(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let text = self.get_string_param(action, 0)?;

        log::info!("Displaying text: {}", text);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.display_text(&text) {
                        log::warn!("Script action handler display_text failed: {}", err);
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_display_cinematic_text(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let text = self.get_string_param(action, 0)?;
        let font_type = action
            .get_parameter(1)
            .map(|p| p.get_string().to_string())
            .unwrap_or_else(|| "Default".to_string());
        let duration_seconds = action.get_parameter(2).map(|p| p.get_int()).unwrap_or(0);

        log::info!(
            "Displaying cinematic text: {} (font: {}, duration: {}s)",
            text,
            font_type,
            duration_seconds
        );
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.display_cinematic_text(&text, &font_type, duration_seconds)
                    {
                        log::warn!(
                            "Script action handler display_cinematic_text failed: {}",
                            err
                        );
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_military_caption(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let briefing_text = self.get_string_param(action, 0)?;
        let mut duration_frames = self.get_int_param(action, 1)?;

        if let Ok(global) = global_data::read_safe() {
            if global.writable.disable_military_caption {
                duration_frames = 1;
            }
        }

        log::info!(
            "Showing military caption: {} (duration: {} frames)",
            briefing_text,
            duration_frames
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.military_caption(&briefing_text, duration_frames) {
                        log::warn!("Script action handler military_caption failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // CAMERA ACTIONS
    // C++ Reference: ScriptActions.cpp line (move camera)
    // ============================================================================

    fn do_move_camera_to(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // Ini scripts can call this with either:
        // - `MOVE_CAMERA_TO X:.. Y:.. Z:..` (coordinate)
        // - `MOVE_CAMERA_TO WaypointName <duration>` (waypoint + optional duration)
        let Some(param0) = action.get_parameter(0) else {
            return Err(ScriptError::ParameterNotFound(
                "Parameter 0 not found".to_string(),
            ));
        };

        let duration_seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let camera_stutter_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(4).map(|p| p.get_real()).unwrap_or(0.0);

        let target = if param0.get_parameter_type() == ParameterType::Coord3D {
            let pos = param0.get_coord();
            Some(crate::common::Coord3D::new(pos.x, pos.y, pos.z))
        } else {
            let waypoint_name = param0.get_string();
            let waypoint_ascii = AsciiString::from(waypoint_name);
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(target) = target else {
            log::warn!(
                "MOVE_CAMERA_TO: waypoint '{}' not found; action ignored",
                param0.get_string()
            );
            return Ok(ScriptActionResult::Success);
        };

        log::info!(
            "Moving camera to ({}, {}, {}) (sec: {}, stutter: {}, ease_in: {}, ease_out: {})",
            target.x,
            target.y,
            target.z,
            duration_seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_to(
                        target.x,
                        target.y,
                        target.z,
                        duration_seconds,
                        camera_stutter_seconds,
                        ease_in_seconds,
                        ease_out_seconds,
                    ) {
                        log::warn!("Script action handler move_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doCameraFollowNamed() line 468
    fn do_camera_follow_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let snap_to_unit = self.get_bool_param_optional(action, 1).unwrap_or(false);

        log::info!(
            "Camera following named unit '{}' (snap: {})",
            unit_name,
            snap_to_unit
        );

        let tracker = get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if object_id.is_none() {
            let lower = unit_name.to_ascii_lowercase();
            object_id = OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find_map(|obj_ref| {
                    obj_ref.read().ok().and_then(|obj| {
                        if obj.get_name().to_ascii_lowercase() == lower {
                            Some(obj.get_id())
                        } else {
                            None
                        }
                    })
                });
        }

        let Some(object_id) = object_id else {
            log::warn!("Camera follow failed: unit '{}' not found", unit_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_follow_object(object_id, snap_to_unit) {
                        log::warn!("Script action handler camera_follow_object failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doStopCameraFollowUnit() line 484
    fn do_stop_camera_follow(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Stopping camera follow");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.stop_camera_follow() {
                        log::warn!("Script action handler stop_camera_follow failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_reset_camera(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let duration_seconds = self.get_real_param(action, 1)?;

        log::info!(
            "Resetting camera to waypoint '{}' over {} seconds",
            waypoint_name,
            duration_seconds
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!("RESET_CAMERA: waypoint '{}' not found", waypoint_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.reset_camera_to(target.x, target.y, target.z, duration_seconds)
                    {
                        log::warn!("Script action handler reset_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // AUDIO ACTIONS
    // C++ Reference: ScriptActions.cpp line 353 (play sound)
    // ============================================================================

    /// C++ Reference: ScriptActions::doPlaySoundEffect() line 353
    fn do_play_sound_effect(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_name = self.get_string_param(action, 0)?;

        log::info!("Playing sound effect: {}", sound_name);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.play_sound_effect(&sound_name) {
                        log::warn!("Script action handler play_sound_effect failed: {}", err);
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doPlaySoundEffectAt() line 365
    fn do_play_sound_effect_at(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;

        log::info!(
            "Playing sound effect '{}' at waypoint '{}'",
            sound_name,
            waypoint_name
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "PLAY_SOUND_EFFECT_AT: waypoint '{}' not found; action ignored",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.play_sound_effect_at(&sound_name, target.x, target.y, target.z)
                    {
                        log::warn!("Script action handler play_sound_effect_at failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_speech_play(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let speech_name = self.get_string_param(action, 0)?;
        let allow_overlap = self.get_bool_param_optional(action, 1).unwrap_or(false);

        log::info!(
            "Playing speech: {} (overlap: {})",
            speech_name,
            allow_overlap
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.speech_play(&speech_name, allow_overlap) {
                        log::warn!("Script action handler speech_play failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_music_track_change(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let track_name = self.get_string_param(action, 0)?;
        let fade_out = self.get_bool_param_optional(action, 1).unwrap_or(true);
        let fade_in = self.get_bool_param_optional(action, 2).unwrap_or(true);

        log::debug!(
            "Changing music to '{}' (fade out: {}, fade in: {})",
            track_name,
            fade_out,
            fade_in
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.music_set_track(&track_name, fade_out, fade_in) {
                        log::warn!("Script action handler music_set_track failed: {}", err);
                    }
                }
            }
        }

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.set_current_track_name(track_name.clone());
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // RADAR ACTIONS
    // ============================================================================

    fn do_radar_disable(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Disabling radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.hide(true);
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_enabled(false) {
                        log::warn!(
                            "Script action handler set_radar_enabled(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_radar_enable(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Enabling radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.hide(false);
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_enabled(true) {
                        log::warn!(
                            "Script action handler set_radar_enabled(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_reveal_map_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let radius = self.get_real_param(action, 1)?;
        let player_name = self.get_string_param(action, 2)?;

        log::info!(
            "Revealing map at waypoint '{}' with radius {} for player '{}'",
            waypoint_name,
            radius,
            player_name
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "REVEAL_MAP_AT_WAYPOINT: waypoint '{}' not found; action ignored",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let player_mask = if let Some(player_arc) = players.find_player_by_name(&player_name) {
            let Ok(player) = player_arc.read() else {
                return Ok(ScriptActionResult::Success);
            };
            player.get_player_mask().bits()
        } else {
            players
                .iter()
                .filter_map(|player_arc| player_arc.read().ok())
                .filter(|player| player.get_player_type() == PlayerType::Human)
                .fold(0u32, |mask, player| mask | player.get_player_mask().bits())
        };

        if player_mask != 0 {
            let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
            if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
                shroud_mgr.do_shroud_reveal(&target, radius, player_mask);
                shroud_mgr.undo_shroud_reveal(&target, radius, player_mask);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_shroud_map_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let radius = self.get_real_param(action, 1)?;
        let player_name = self.get_string_param(action, 2)?;

        log::info!(
            "Shrouding map at waypoint '{}' with radius {} for player '{}'",
            waypoint_name,
            radius,
            player_name
        );

        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "SHROUD_MAP_AT_WAYPOINT: waypoint '{}' not found; action ignored",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let player_mask = if let Some(player_arc) = players.find_player_by_name(&player_name) {
            let Ok(player) = player_arc.read() else {
                return Ok(ScriptActionResult::Success);
            };
            player.get_player_mask().bits()
        } else {
            players
                .iter()
                .filter_map(|player_arc| player_arc.read().ok())
                .filter(|player| player.get_player_type() == PlayerType::Human)
                .fold(0u32, |mask, player| mask | player.get_player_mask().bits())
        };

        if player_mask != 0 {
            let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
            if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
                shroud_mgr.do_shroud_cover(&target, radius, player_mask);
                shroud_mgr.undo_shroud_cover(&target, radius, player_mask);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // INPUT CONTROL
    // ============================================================================

    fn do_disable_input(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Disabling user input");
        TheGameLogic::set_input_enabled(false);

        Ok(ScriptActionResult::Success)
    }

    fn do_enable_input(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Enabling user input");
        TheGameLogic::set_input_enabled(true);

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // COUNTER/FLAG/TIMER ACTION IMPLEMENTATIONS
    // C++ Reference: ScriptActions.cpp
    // ============================================================================

    /// C++ Reference: ScriptActions::doSetFlag()
    fn do_set_flag(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let flag_name = self.get_string_param(action, 0)?;
        let value = self.get_int_param(action, 1)? != 0;
        log::debug!("Setting flag '{}' to {}", flag_name, value);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.set_flag(&flag_name, value);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetCounter()
    fn do_set_counter(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        let value = self.get_int_param(action, 1)?;
        log::debug!("Setting counter '{}' to {}", counter_name, value);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.set_counter(&counter_name, value);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doIncrementCounter()
    fn do_increment_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        log::debug!("Incrementing counter '{}'", counter_name);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.increment_counter(&counter_name);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doDecrementCounter()
    fn do_decrement_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        log::debug!("Decrementing counter '{}'", counter_name);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.decrement_counter(&counter_name);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetTimer()
    fn do_set_timer(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let seconds = self.get_real_param(action, 1)?;
        log::debug!("Setting timer '{}' to {} seconds", timer_name, seconds);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.set_timer_seconds(&timer_name, seconds);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetMillisecondTimer()
    fn do_set_millisecond_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let seconds = self.get_real_param(action, 1)?;
        log::debug!(
            "Setting legacy millisecond timer '{}' to {} script-seconds",
            timer_name,
            seconds
        );

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.set_timer_millisecond_script_seconds(&timer_name, seconds);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetRandomTimer()
    fn do_set_random_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let min_seconds = self.get_int_param(action, 1)?;
        let max_seconds = self.get_int_param(action, 2)?;
        log::debug!(
            "Setting random timer '{}' between {}-{} frames",
            timer_name,
            min_seconds,
            max_seconds
        );

        let random_frames = get_game_logic_random_value(min_seconds, max_seconds);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.set_timer(&timer_name, random_frames);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSetRandomMsecTimer()
    fn do_set_random_msec_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let min_seconds = self.get_real_param(action, 1)?;
        let max_seconds = self.get_real_param(action, 2)?;
        log::debug!(
            "Setting legacy random millisecond timer '{}' between {}-{} script-seconds",
            timer_name,
            min_seconds,
            max_seconds
        );

        let random_seconds = get_game_logic_random_value_real(min_seconds, max_seconds);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.set_timer_millisecond_script_seconds(&timer_name, random_seconds);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doStopTimer()
    fn do_stop_timer(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        log::debug!("Stopping timer '{}'", timer_name);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.stop_timer(&timer_name);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doRestartTimer()
    fn do_restart_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        log::debug!("Restarting timer '{}'", timer_name);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.restart_timer(&timer_name);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doAddToMsecTimer()
    fn do_add_to_msec_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let seconds = self.get_real_param(action, 0)?;
        let timer_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Adding {} script-seconds to legacy millisecond timer '{}'",
            seconds,
            timer_name
        );

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.add_to_timer_millisecond_script_seconds(&timer_name, seconds);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doSubFromMsecTimer()
    fn do_sub_from_msec_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let seconds = self.get_real_param(action, 0)?;
        let timer_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Subtracting {} script-seconds from legacy millisecond timer '{}'",
            seconds,
            timer_name
        );

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                let _ = engine.subtract_from_timer_millisecond_script_seconds(&timer_name, seconds);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // SCRIPT CONTROL ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_enable_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let script_name = self.get_string_param(action, 0)?;
        log::debug!("Enabling script '{}'", script_name);
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                if !engine.set_script_active_by_name(&script_name, true) {
                    log::warn!("ENABLE_SCRIPT: script '{}' not found", script_name);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_disable_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let script_name = self.get_string_param(action, 0)?;
        log::debug!("Disabling script '{}'", script_name);
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                if !engine.set_script_active_by_name(&script_name, false) {
                    log::warn!("DISABLE_SCRIPT: script '{}' not found", script_name);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_call_subroutine(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let subroutine_name = self.get_string_param(action, 0)?;
        log::debug!("Calling subroutine '{}'", subroutine_name);

        let found = if let Ok(mut guard) = get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine
                    .execute_subroutine_by_name(&subroutine_name)
                    .map_err(|e| ScriptError::ExecutionFailed(e.to_string()))?
            } else {
                false
            }
        } else {
            false
        };

        if !found {
            log::warn!(
                "CALL_SUBROUTINE: subroutine '{}' not found",
                subroutine_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_debug_message_box(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        log::info!("[DEBUG MESSAGE BOX] {}", message);
        Ok(ScriptActionResult::Success)
    }

    fn do_debug_string(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        log::debug!("[DEBUG STRING] {}", message);
        Ok(ScriptActionResult::Success)
    }

    fn do_debug_crash_box(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        log::error!("[DEBUG CRASH] {}", message);
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL TEAM ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_build_team(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Building team '{}'", team_name);

        let (prototype_owner, team_owner_id) = if let Ok(mut factory) = get_team_factory().lock() {
            (
                factory.find_team_prototype(&team_name).and_then(|proto| {
                    let owner = proto.get_owner_name().to_string();
                    if owner.is_empty() {
                        None
                    } else {
                        Some(owner)
                    }
                }),
                factory.find_team(&team_name).and_then(|team| {
                    team.read()
                        .ok()
                        .and_then(|team| team.get_controlling_player_id())
                }),
            )
        } else {
            (None, None)
        };

        let owner_name = prototype_owner
            .or_else(|| {
                team_owner_id.and_then(|player_id| {
                    player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(player_id as i32).cloned())
                        .and_then(|player| {
                            player.read().ok().and_then(|player| {
                                NameKeyGenerator::key_to_name(player.get_player_name_key())
                            })
                        })
                })
            })
            .or_else(|| {
                get_script_engine().read().ok().and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                })
            });

        if let Some(owner_name) = owner_name {
            self.with_named_player_ai(&owner_name, |ai_player| {
                if let Err(err) = ai_player.build_specific_ai_team(&team_name, true) {
                    log::debug!(
                        "BuildTeam '{}' failed for player '{}': {}",
                        team_name,
                        owner_name,
                        err
                    );
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_recruit_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let recruit_radius = self.get_real_param(action, 1)?;
        log::debug!("Recruiting team '{}' radius {}", team_name, recruit_radius);

        let (prototype_owner, team_owner_id) = if let Ok(mut factory) = get_team_factory().lock() {
            (
                factory.find_team_prototype(&team_name).and_then(|proto| {
                    let owner = proto.get_owner_name().to_string();
                    if owner.is_empty() {
                        None
                    } else {
                        Some(owner)
                    }
                }),
                factory.find_team(&team_name).and_then(|team| {
                    team.read()
                        .ok()
                        .and_then(|team| team.get_controlling_player_id())
                }),
            )
        } else {
            (None, None)
        };

        let owner_name = prototype_owner
            .or_else(|| {
                team_owner_id.and_then(|player_id| {
                    player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(player_id as i32).cloned())
                        .and_then(|player| {
                            player.read().ok().and_then(|player| {
                                NameKeyGenerator::key_to_name(player.get_player_name_key())
                            })
                        })
                })
            })
            .or_else(|| {
                get_script_engine().read().ok().and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                })
            });

        if let Some(owner_name) = owner_name {
            self.with_named_player_ai(&owner_name, |ai_player| {
                if let Err(err) = ai_player.recruit_specific_ai_team(&team_name, recruit_radius) {
                    log::debug!(
                        "RecruitTeam '{}' failed for player '{}': {}",
                        team_name,
                        owner_name,
                        err
                    );
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_create_reinforcement_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let waypoint_name = self.get_string_param(action, 1)?;

        let destination = {
            let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|waypoint| *waypoint.get_location())
            })
        };

        let Some(destination) = destination else {
            log::warn!(
                "CREATE_REINFORCEMENT_TEAM: waypoint '{}' not found",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        // Keep compatibility for custom scripts that used a non-C++ extension:
        // `CREATE_REINFORCEMENT_TEAM TeamName UnitType Coord Count`.
        let has_legacy_signature = action.get_parameter(2).is_some();
        if has_legacy_signature {
            let unit_type = waypoint_name;
            let spawn_pos = action
                .get_parameter(2)
                .map(|p| {
                    if p.get_parameter_type() == ParameterType::Coord3D {
                        let pos = p.get_coord();
                        crate::common::Coord3D::new(pos.x, pos.y, pos.z)
                    } else {
                        let waypoint = AsciiString::from(p.get_string());
                        get_terrain_logic()
                            .read()
                            .ok()
                            .and_then(|terrain| {
                                terrain
                                    .get_waypoint_by_name(&waypoint)
                                    .map(|w| *w.get_location())
                            })
                            .unwrap_or(destination)
                    }
                })
                .unwrap_or(destination);
            let count = action.get_parameter(3).map(|p| p.get_int()).unwrap_or(1);

            let team_arc = match self.get_or_create_team_by_name(&team_name) {
                Ok(team) => team,
                Err(err) => {
                    log::warn!(
                        "CREATE_REINFORCEMENT_TEAM: failed to get/create team: {}",
                        err
                    );
                    return Ok(ScriptActionResult::Success);
                }
            };

            let mut created_any = false;
            for i in 0..count.max(0) {
                let offset = (i as f32) * 5.0;
                let pos =
                    crate::common::Coord3D::new(spawn_pos.x + offset, spawn_pos.y, spawn_pos.z);
                let object_id = {
                    let manager_arc = get_object_manager();
                    let Ok(mut manager) = manager_arc.write() else {
                        log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                        break;
                    };
                    match manager.create_object(
                        &unit_type,
                        pos,
                        Some(team_arc.clone()),
                        crate::object_manager::ObjectCreationFlags::from_template(),
                    ) {
                        Ok(id) => id,
                        Err(err) => {
                            log::warn!(
                                "CREATE_REINFORCEMENT_TEAM: failed to create '{}': {}",
                                unit_type,
                                err
                            );
                            continue;
                        }
                    }
                };

                if let Ok(mut team) = team_arc.write() {
                    team.add_member(object_id);
                    team.set_active();
                }
                created_any = true;
            }

            if !created_any {
                log::warn!(
                    "CREATE_REINFORCEMENT_TEAM: no units created for team '{}'",
                    team_name
                );
            }
            return Ok(ScriptActionResult::Success);
        }

        let (team_proto, team_arc) = {
            let Ok(mut factory) = get_team_factory().lock() else {
                log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock TeamFactory");
                return Ok(ScriptActionResult::Success);
            };

            let Some(proto) = factory.find_team_prototype(&team_name) else {
                log::warn!(
                    "CREATE_REINFORCEMENT_TEAM: team prototype '{}' not found",
                    team_name
                );
                return Ok(ScriptActionResult::Success);
            };

            let team = if let Some(existing) = factory.find_team(&team_name) {
                existing
            } else if let Some(created) = factory.create_inactive_team(&team_name) {
                created
            } else {
                log::warn!(
                    "CREATE_REINFORCEMENT_TEAM: failed to create inactive team '{}'",
                    team_name
                );
                return Ok(ScriptActionResult::Success);
            };

            (proto, team)
        };

        if let Ok(mut team) = team_arc.write() {
            if team.get_controlling_player_id().is_none() {
                let owner_name = team_proto.get_owner_name().to_string();
                if !owner_name.is_empty() {
                    if let Some(owner_player) = player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.find_player_by_name(&owner_name))
                    {
                        if let Ok(owner_guard) = owner_player.read() {
                            team.set_controlling_player_id(Some(
                                owner_guard.get_player_index() as u32
                            ));
                        }
                    }
                }
            }
        }

        let mut origin = destination;
        let mut need_move_to_destination = false;
        if !team_proto.get_start_reinforce_waypoint().is_empty() {
            let start_waypoint_name = team_proto.get_start_reinforce_waypoint();
            let start_waypoint_ascii = AsciiString::from(start_waypoint_name.as_str());
            if let Some(start) = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&start_waypoint_ascii)
                    .map(|waypoint| *waypoint.get_location())
            }) {
                need_move_to_destination = start.x != destination.x || start.y != destination.y;
                origin = start;
            }
        }

        let mut created_any = false;
        let mut primary_transport_id: Option<ObjectID> = None;
        let mut transport_template_for_equivalence: Option<Arc<dyn crate::common::ThingTemplate>> =
            None;
        let mut put_in_container_template: Option<Arc<dyn crate::common::ThingTemplate>> = None;
        let transport_template_name = team_proto.get_transport_unit_type().to_string();

        // C++ parity: create reinforcement transport first so we can inspect DeliverPayload behavior.
        if !transport_template_name.is_empty() {
            let transport_id = {
                let manager_arc = get_object_manager();
                let Ok(mut manager) = manager_arc.write() else {
                    log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                    return Ok(ScriptActionResult::Success);
                };
                match manager.create_object(
                    &transport_template_name,
                    origin,
                    Some(team_arc.clone()),
                    crate::object_manager::ObjectCreationFlags::from_template(),
                ) {
                    Ok(id) => id,
                    Err(err) => {
                        log::warn!(
                            "CREATE_REINFORCEMENT_TEAM: failed to create transport '{}': {}",
                            transport_template_name,
                            err
                        );
                        INVALID_ID
                    }
                }
            };

            if transport_id != INVALID_ID {
                if let Ok(mut team) = team_arc.write() {
                    team.add_member(transport_id);
                }
                primary_transport_id = Some(transport_id);
                created_any = true;

                if let Some(transport_arc) = TheGameLogic::find_object_by_id(transport_id) {
                    if let Ok(mut transport) = transport_arc.write() {
                        let _ = transport.set_position(&origin);
                        let _ = transport.set_orientation(0.0);
                        transport_template_for_equivalence = Some(transport.get_template().clone());

                        if let Some(dp_module) =
                            transport.find_update_module("DeliverPayloadAIUpdate")
                        {
                            let put_in_container_name = dp_module.with_module_data(|data| {
                                data.as_any()
                                    .downcast_ref::<crate::object::update::DeliverPayloadAIUpdateModuleData>()
                                    .and_then(|module_data| {
                                        let name = module_data.put_in_container_name.as_str();
                                        if name.is_empty() {
                                            None
                                        } else {
                                            Some(name.to_string())
                                        }
                                    })
                            });

                            if let Some(name) = put_in_container_name {
                                put_in_container_template =
                                    crate::helpers::TheThingFactory::find_template(&name);
                            }
                        }
                    }
                }
            }
        }

        // Spawn configured unit composition for the team.
        let mut row_origin = origin;
        for info in team_proto.units_info() {
            if info.unit_thing_name.is_empty() {
                continue;
            }
            let unit_count = info.max_units.max(0) as usize;
            if unit_count == 0 {
                continue;
            }

            let mut row_last_pos = row_origin;
            let mut row_last_radius = 0.0f32;
            let mut row_spawned_any = false;

            for index in 0..unit_count {
                let object_id = {
                    let manager_arc = get_object_manager();
                    let Ok(mut manager) = manager_arc.write() else {
                        log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                        break;
                    };
                    match manager.create_object(
                        info.unit_thing_name,
                        row_origin,
                        Some(team_arc.clone()),
                        crate::object_manager::ObjectCreationFlags::from_template(),
                    ) {
                        Ok(id) => id,
                        Err(err) => {
                            log::warn!(
                                "CREATE_REINFORCEMENT_TEAM: failed to create '{}': {}",
                                info.unit_thing_name,
                                err
                            );
                            continue;
                        }
                    }
                };

                if let Ok(mut team) = team_arc.write() {
                    team.add_member(object_id);
                }
                created_any = true;
                row_spawned_any = true;

                if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                    if let Ok(mut obj) = obj_arc.write() {
                        let radius = obj.get_geometry_info().get_major_radius();
                        let mut pos = row_origin;
                        pos.x = row_origin.x + 2.25 * (index as f32) * radius;
                        if let Ok(terrain) = get_terrain_logic().read() {
                            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                        }
                        let _ = obj.set_position(&pos);
                        let _ = obj.set_orientation(0.0);
                        row_last_pos = pos;
                        row_last_radius = radius;
                    }
                }
            }

            if row_spawned_any {
                row_origin.y = row_last_pos.y + 2.0 * row_last_radius;
            }
        }

        // C++ parity: if TeamStartsFull, pre-load units into transports already in the team
        // (excluding the reinforcement transport created above).
        if team_proto.get_team_starts_full() {
            let member_ids = if let Ok(team) = team_arc.read() {
                team.get_members().to_vec()
            } else {
                Vec::new()
            };

            let mut team_transports: Vec<ObjectID> = Vec::new();
            let mut loadable_units: Vec<ObjectID> = Vec::new();
            for member_id in member_ids {
                let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(member) = member_arc.read() else {
                    continue;
                };

                if Some(member_id) == primary_transport_id {
                    continue;
                }

                if member.is_kind_of(crate::common::KindOf::Transport) {
                    if member.get_contain().is_some() {
                        team_transports.push(member_id);
                    }
                } else {
                    loadable_units.push(member_id);
                }
            }

            for unit_id in loadable_units {
                let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
                    continue;
                };
                let Ok(unit_guard) = unit_arc.read() else {
                    continue;
                };

                for transport_id in &team_transports {
                    let Some(transport_arc) = TheGameLogic::find_object_by_id(*transport_id) else {
                        continue;
                    };
                    let contain_arc = transport_arc.read().ok().and_then(|t| t.get_contain());
                    let Some(contain_arc) = contain_arc else {
                        continue;
                    };
                    let Ok(mut contain_guard) = contain_arc.lock() else {
                        continue;
                    };
                    if contain_guard.is_valid_container_for(&unit_guard, true) {
                        let _ = contain_guard.add_to_contain(&unit_guard);
                        break;
                    }
                }
            }
        }

        let load_origin = destination;

        // Load remaining units into reinforcement transport(s), creating additional transports if full.
        if let Some(mut current_transport_id) = primary_transport_id {
            let mut transport_count = 1;
            let member_ids = if let Ok(team) = team_arc.read() {
                team.get_members().to_vec()
            } else {
                Vec::new()
            };

            for member_id in member_ids {
                let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let Ok(member_guard) = member_arc.read() else {
                    continue;
                };

                let is_transport_template = transport_template_for_equivalence
                    .as_ref()
                    .map(|template| {
                        member_guard
                            .get_template()
                            .is_equivalent_to(template.as_ref())
                    })
                    .unwrap_or(false);
                if is_transport_template || member_guard.get_contained_by().is_some() {
                    continue;
                }

                let Some(current_transport_arc) =
                    TheGameLogic::find_object_by_id(current_transport_id)
                else {
                    continue;
                };
                let (contains, full, transport_radius) = {
                    let Ok(transport_guard) = current_transport_arc.read() else {
                        continue;
                    };
                    let transport_radius = transport_guard.get_geometry_info().get_major_radius();
                    let Some(contain_arc) = transport_guard.get_contain() else {
                        continue;
                    };
                    let Ok(contain_guard) = contain_arc.lock() else {
                        continue;
                    };
                    (
                        contain_guard.is_valid_container_for(&member_guard, false),
                        contain_guard.is_valid_container_for(&member_guard, true),
                        transport_radius,
                    )
                };

                if !contains {
                    continue;
                }

                drop(member_guard);

                if !full {
                    let mut pos = load_origin;
                    pos.x += (transport_count as f32) * transport_radius;
                    if let Ok(terrain) = get_terrain_logic().read() {
                        pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                    }
                    let new_transport_id = {
                        let manager_arc = get_object_manager();
                        let Ok(mut manager) = manager_arc.write() else {
                            log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                            continue;
                        };
                        match manager.create_object(
                            &transport_template_name,
                            pos,
                            Some(team_arc.clone()),
                            crate::object_manager::ObjectCreationFlags::from_template(),
                        ) {
                            Ok(id) => id,
                            Err(err) => {
                                log::warn!(
                                    "CREATE_REINFORCEMENT_TEAM: failed to create overflow transport '{}': {}",
                                    transport_template_name,
                                    err
                                );
                                INVALID_ID
                            }
                        }
                    };

                    if new_transport_id != INVALID_ID {
                        if let Some(new_transport_arc) =
                            TheGameLogic::find_object_by_id(new_transport_id)
                        {
                            if let Ok(mut new_transport) = new_transport_arc.write() {
                                let _ = new_transport.set_position(&pos);
                                let _ = new_transport.set_orientation(0.0);
                            }
                        }
                        if let Ok(mut team) = team_arc.write() {
                            team.add_member(new_transport_id);
                        }
                        current_transport_id = new_transport_id;
                        transport_count += 1;
                        created_any = true;
                    }
                }

                let mut payload_object_id = member_id;
                if let Some(put_in_container_template) = put_in_container_template.as_ref() {
                    let container_pos = load_origin;
                    let container_id = {
                        let manager_arc = get_object_manager();
                        let Ok(mut manager) = manager_arc.write() else {
                            log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                            continue;
                        };
                        match manager.create_object(
                            put_in_container_template.get_name().as_str(),
                            container_pos,
                            Some(team_arc.clone()),
                            crate::object_manager::ObjectCreationFlags::from_template(),
                        ) {
                            Ok(id) => id,
                            Err(err) => {
                                log::warn!(
                                    "CREATE_REINFORCEMENT_TEAM: failed to create payload container '{}': {}",
                                    put_in_container_template.get_name().as_str(),
                                    err
                                );
                                INVALID_ID
                            }
                        }
                    };

                    if container_id != INVALID_ID {
                        if let Some(container_arc) = TheGameLogic::find_object_by_id(container_id) {
                            if let Ok(mut container) = container_arc.write() {
                                let _ = container.set_position(&container_pos);
                                let _ = container.set_orientation(0.0);
                            }
                        }
                        if let Ok(mut team) = team_arc.write() {
                            team.add_member(container_id);
                        }
                        created_any = true;

                        let inserted = if let Some(container_arc) =
                            TheGameLogic::find_object_by_id(container_id)
                        {
                            if let Some(payload_arc) = TheGameLogic::find_object_by_id(member_id) {
                                if let (Ok(container_guard), Ok(payload_guard)) =
                                    (container_arc.read(), payload_arc.read())
                                {
                                    if let Some(container_contain) = container_guard.get_contain() {
                                        if let Ok(mut container_contain_guard) =
                                            container_contain.lock()
                                        {
                                            if container_contain_guard
                                                .is_valid_container_for(&payload_guard, true)
                                            {
                                                let _ = container_contain_guard
                                                    .add_to_contain(&payload_guard);
                                                true
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if inserted {
                            payload_object_id = container_id;
                        }
                    }
                }

                let Some(payload_arc) = TheGameLogic::find_object_by_id(payload_object_id) else {
                    continue;
                };
                let Ok(payload_guard) = payload_arc.read() else {
                    continue;
                };

                let Some(transport_arc) = TheGameLogic::find_object_by_id(current_transport_id)
                else {
                    continue;
                };
                let contain_arc = transport_arc
                    .read()
                    .ok()
                    .and_then(|transport| transport.get_contain());
                let Some(contain_arc) = contain_arc else {
                    continue;
                };
                let Ok(mut contain_guard) = contain_arc.lock() else {
                    continue;
                };
                let _ = contain_guard.add_to_contain(&payload_guard);
            }
        }

        if let Ok(mut team) = team_arc.write() {
            team.set_active();
        }

        if primary_transport_id.is_some() {
            let member_ids = if let Ok(team) = team_arc.read() {
                team.get_members().to_vec()
            } else {
                Vec::new()
            };

            for member_id in member_ids {
                let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                    continue;
                };
                let (is_transport_template, is_held, ai_arc) = {
                    let Ok(member) = member_arc.read() else {
                        continue;
                    };
                    (
                        transport_template_for_equivalence
                            .as_ref()
                            .map(|template| {
                                member.get_template().is_equivalent_to(template.as_ref())
                            })
                            .unwrap_or(false),
                        member.is_disabled_by_type(crate::common::DisabledType::Held),
                        member.get_ai_update_interface(),
                    )
                };

                let Some(ai_arc) = ai_arc else {
                    continue;
                };

                if is_transport_template {
                    if let Ok(mut ai) = ai_arc.lock() {
                        let mut used_deliver_payload = false;
                        if let Some(dp) = ai.get_deliver_payload_ai_update_interface() {
                            dp.deliver_payload_via_module_data(&destination);
                            used_deliver_payload = true;
                        }

                        if !used_deliver_payload {
                            let _ =
                                ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            if team_proto.get_transports_exit() {
                                let mut params = AiCommandParams::new(
                                    AiCommandType::MoveToPositionAndEvacuateAndExit,
                                    CommandSourceType::FromScript,
                                );
                                params.pos = destination;
                                let _ = ai.execute_command(&params);
                            } else {
                                let _ = ai.ai_move_to_and_evacuate(&destination);
                            }
                        }
                    }
                } else if !is_held {
                    if let Ok(mut ai) = ai_arc.lock() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                        let mut params = AiCommandParams::new(
                            AiCommandType::MoveToPosition,
                            CommandSourceType::FromScript,
                        );
                        params.pos = destination;
                        let _ = ai.execute_command(&params);
                    }
                }
            }
        } else if created_any && need_move_to_destination {
            if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
                if let Ok(group) = group_arc.write() {
                    group.group_move_to_position(
                        &destination,
                        false,
                        CommandSourceType::FromScript,
                    );
                }
            }
        }

        if !created_any {
            log::warn!(
                "CREATE_REINFORCEMENT_TEAM: team '{}' has no units configured",
                team_name
            );
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamWander()
    /// Iterates team members, selects wander locomotor, and issues waypoint wander.
    fn do_team_wander(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        log::info!(
            "Team '{}' wandering on path '{}'",
            team_name,
            waypoint_path_label
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = if let Ok(team) = team_arc.read() {
            team.get_members().to_vec()
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to read team".to_string(),
            ));
        };

        for member_id in members {
            let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let (member_pos, ai_arc) = {
                let Ok(member) = member_arc.read() else {
                    continue;
                };
                (*member.get_position(), member.get_ai_update_interface())
            };
            let Some(ai_arc) = ai_arc else {
                continue;
            };

            let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&member_pos, &waypoint_path_label)
                    .map(|waypoint| waypoint.get_id())
            });
            let Some(waypoint_id) = waypoint_id else {
                return Ok(ScriptActionResult::Success);
            };

            if let Ok(mut ai) = ai_arc.lock() {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Wander);
                let mut params =
                    AiCommandParams::new(AiCommandType::Wander, CommandSourceType::FromScript);
                params.waypoint = Some(waypoint_id);
                let _ = ai.execute_command(&params);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamWanderInPlace()
    /// Iterates team members, selects wander locomotor, and issues wander-in-place.
    fn do_team_wander_in_place(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::info!("Team '{}' wandering in place", team_name);

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = if let Ok(team) = team_arc.read() {
            team.get_members().to_vec()
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to read team".to_string(),
            ));
        };

        for member_id in members {
            let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let ai_arc = {
                let Ok(member) = member_arc.read() else {
                    continue;
                };
                member.get_ai_update_interface()
            };
            let Some(ai_arc) = ai_arc else {
                continue;
            };

            if let Ok(mut ai) = ai_arc.lock() {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Wander);
                let params = AiCommandParams::new(
                    AiCommandType::WanderInPlace,
                    CommandSourceType::FromScript,
                );
                let _ = ai.execute_command(&params);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamPanic()
    /// Iterates team members, selects panic locomotor, and issues waypoint panic.
    fn do_team_panic(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' panicking on path '{}'",
            team_name,
            waypoint_path_label
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = if let Ok(team) = team_arc.read() {
            team.get_members().to_vec()
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to read team".to_string(),
            ));
        };

        for member_id in members {
            let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let (member_pos, ai_arc) = {
                let Ok(member) = member_arc.read() else {
                    continue;
                };
                (*member.get_position(), member.get_ai_update_interface())
            };
            let Some(ai_arc) = ai_arc else {
                continue;
            };

            let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&member_pos, &waypoint_path_label)
                    .map(|waypoint| waypoint.get_id())
            });
            let Some(waypoint_id) = waypoint_id else {
                return Ok(ScriptActionResult::Success);
            };

            if let Ok(mut ai) = ai_arc.lock() {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic);
                let mut params =
                    AiCommandParams::new(AiCommandType::Panic, CommandSourceType::FromScript);
                params.waypoint = Some(waypoint_id);
                let _ = ai.execute_command(&params);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamStop()
    /// Issues stop command to team AI group
    fn do_team_stop(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::info!("Team '{}' stopping", team_name);

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamStopAndDisband()
    /// Issues stop command and then disbands the team
    fn do_team_stop_and_disband(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::info!("Team '{}' stopping and disbanding", team_name);

        // Issue stop command
        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        // Disband the team
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let team_id = team_arc.read().ok().map(|t| t.get_id());
                if let Some(tid) = team_id {
                    factory_guard.team_about_to_be_deleted(tid);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_available_for_recruitment(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let available = self.get_int_param(action, 1)? != 0;
        log::debug!(
            "Team '{}' available for recruitment: {}",
            team_name,
            available
        );

        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.set_recruitable(available);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_collect_nearby(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::debug!("Team '{}' collecting nearby units", team_name);
        Ok(ScriptActionResult::Success)
    }

    fn do_team_merge(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let source_team = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let target_team = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        log::debug!("Merging team '{}' into '{}'", source_team, target_team);

        let (source_team_arc, target_team_arc) = if let Ok(mut factory) = get_team_factory().lock()
        {
            (
                factory.find_team(&source_team),
                factory
                    .find_team(&target_team)
                    .or_else(|| factory.create_team(&target_team)),
            )
        } else {
            (None, None)
        };
        let (Some(source_team_arc), Some(target_team_arc)) = (source_team_arc, target_team_arc)
        else {
            return Ok(ScriptActionResult::Success);
        };
        if Arc::ptr_eq(&source_team_arc, &target_team_arc) {
            return Ok(ScriptActionResult::Success);
        }

        let source_members = source_team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();

        for object_id in &source_members {
            let Some(object_arc) = TheGameLogic::find_object_by_id(*object_id) else {
                continue;
            };
            if let Ok(mut object_guard) = object_arc.write() {
                let _ = object_guard.set_team(Some(target_team_arc.clone()));
            };
        }

        if let (Ok(mut source_guard), Ok(mut target_guard)) =
            (source_team_arc.write(), target_team_arc.write())
        {
            source_guard.transfer_units_to(&mut target_guard);
            source_guard.delete_team(false);
            target_guard.set_active();
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_flash(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!("Flashing team '{}' for {}s", team_name, time_in_seconds);

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
            .and_then(|team| team.read().ok().map(|t| t.get_members().to_vec()))
            .unwrap_or_default();

        for member_id in members {
            self.flash_object_by_id(member_id, time_in_seconds, None);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_flash_white(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!(
            "Flashing team '{}' white for {}s",
            team_name,
            time_in_seconds
        );

        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
            .and_then(|team| team.read().ok().map(|t| t.get_members().to_vec()))
            .unwrap_or_default();

        for member_id in members {
            self.flash_object_by_id(member_id, time_in_seconds, Some(Color::white()));
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamTransferToPlayer()
    /// Transfers all team members to a different player's control
    fn do_team_transfer_to_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        log::info!(
            "Transferring team '{}' to player '{}'",
            team_name,
            player_name
        );

        // Find the target player by name
        let target_player_id = if let Ok(list) = player_list().read() {
            list.find_player_by_name(&player_name).and_then(|p| {
                p.read()
                    .ok()
                    .and_then(|player| Some(player.get_player_index() as u32))
            })
        } else {
            None
        };

        if let Some(player_id) = target_player_id {
            // Get team and iterate members to transfer ownership
            if let Ok(mut factory) = get_team_factory().lock() {
                if let Some(team_arc) = factory.find_team(&team_name) {
                    if let Ok(team) = team_arc.read() {
                        let object_manager = get_object_manager();
                        if let Ok(obj_manager) = object_manager.read() {
                            for obj_id in team.get_members() {
                                if let Some(obj) = obj_manager.get_object(*obj_id) {
                                    if let Ok(mut obj_write) = obj.write() {
                                        let _ =
                                            obj_write.set_controlling_player_id(Some(player_id));
                                    }
                                }
                            }
                            log::info!(
                                "Team '{}' transferred to player '{}'",
                                team_name,
                                player_name
                            );
                        };
                    }
                } else {
                    log::warn!("Team '{}' not found for transfer", team_name);
                }
            }
        } else {
            log::warn!("Player '{}' not found for team transfer", player_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_set_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let target_team = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Team '{}' override relation to team '{}' ({})",
            team_name,
            target_team,
            relation
        );

        let (team_arc, target_team_id) = if let Ok(mut factory) = get_team_factory().lock() {
            (
                factory.find_team(&team_name),
                factory
                    .find_team(&target_team)
                    .and_then(|team| team.read().ok().map(|team| team.get_id())),
            )
        } else {
            (None, None)
        };
        if let (Some(team_arc), Some(target_team_id)) = (team_arc, target_team_id) {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.set_override_team_relationship(target_team_id, relationship);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_remove_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let target_team = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Team '{}' remove override relation to team '{}'",
            team_name,
            target_team
        );

        let (team_arc, target_team_id) = if let Ok(mut factory) = get_team_factory().lock() {
            (
                factory.find_team(&team_name),
                factory
                    .find_team(&target_team)
                    .and_then(|team| team.read().ok().map(|team| team.get_id())),
            )
        } else {
            (None, None)
        };
        if let (Some(team_arc), Some(target_team_id)) = (team_arc, target_team_id) {
            if let Ok(mut team_guard) = team_arc.write() {
                let _ = team_guard.remove_override_team_relationship(target_team_id);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_remove_all_override_relations(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' remove all override relations", team_name);

        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        if let Some(team_arc) = team_arc {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.clear_override_team_relationships();
                team_guard.clear_override_player_relationships();
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_set_override_relation_to_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Team '{}' override relation to player '{}' ({})",
            team_name,
            player_name,
            relation
        );

        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        let player_index = if let Ok(players) = player_list().read() {
            players
                .find_player_by_name(&player_name)
                .and_then(|player| player.read().ok().map(|player| player.get_player_index()))
        } else {
            None
        };
        if let (Some(team_arc), Some(player_index)) = (team_arc, player_index) {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.set_override_player_relationship(player_index, relationship);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_remove_override_relation_to_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Team '{}' remove override relation to player '{}'",
            team_name,
            player_name
        );

        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        let player_index = if let Ok(players) = player_list().read() {
            players
                .find_player_by_name(&player_name)
                .and_then(|player| player.read().ok().map(|player| player.get_player_index()))
        } else {
            None
        };
        if let (Some(team_arc), Some(player_index)) = (team_arc, player_index) {
            if let Ok(mut team_guard) = team_arc.write() {
                let _ = team_guard.remove_override_player_relationship(player_index);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamLoadTransports()
    /// Team loads into their transport vehicles
    fn do_team_load_transports(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::info!("Team '{}' loading transports", team_name);

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamEnterNamed()
    /// Team enters a specific named object (building/transport)
    fn do_team_enter_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' entering '{}'", team_name, target_name);

        // Get the target object ID
        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&target_name).ok().flatten();

        if let Some(tid) = target_id {
            // Create group first, then use it
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(tid);
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Target '{}' not found for team enter", target_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamExitAll()
    /// All team members exit from containers/transports
    fn do_team_exit_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::info!("Team '{}' exiting all", team_name);

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        let write_result = group_arc.write();
        if let Ok(mut group) = write_result {
            let params =
                AiCommandParams::new(AiCommandType::Evacuate, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGarrisonSpecificBuilding()
    /// Team garrisons a specific named building
    fn do_team_garrison_specific_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let building_name = self.get_string_param(action, 1)?;
        log::info!(
            "Team '{}' garrisoning building '{}'",
            team_name,
            building_name
        );

        let team_player_mask = self
            .get_team_by_name(&team_name)
            .ok()
            .and_then(|team| team.read().ok().and_then(|t| t.get_controlling_player_id()))
            .and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as i32).cloned())
            })
            .and_then(|player| player.read().ok().map(|p| p.get_player_mask()))
            .unwrap_or_else(crate::common::PlayerMaskType::none);

        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&building_name).ok().flatten();

        if let Some(tid) = target_id {
            let Some(building_obj) = TheGameLogic::find_object_by_id(tid) else {
                return Ok(ScriptActionResult::Success);
            };
            let can_garrison = if let Ok(building_guard) = building_obj.read() {
                if !building_guard.is_kind_of(crate::common::KindOf::Structure) {
                    false
                } else if let Some(contain) = building_guard.get_contain() {
                    let entered_mask = contain
                        .lock()
                        .ok()
                        .map(|c| c.get_player_who_entered())
                        .unwrap_or_else(crate::common::PlayerMaskType::none);
                    entered_mask == crate::common::PlayerMaskType::none()
                        || entered_mask == team_player_mask
                } else {
                    false
                }
            } else {
                false
            };
            if !can_garrison {
                return Ok(ScriptActionResult::Success);
            }

            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(tid);
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Building '{}' not found for team garrison", building_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGarrisonNearestBuilding()
    /// Team finds and garrisons nearest garrisonable building
    fn do_team_garrison_nearest_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::info!("Team '{}' garrisoning nearest building", team_name);

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        let write_result = group_arc.write();
        if let Ok(mut group) = write_result {
            let params = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamExitAllBuildings()
    /// Team exits from all garrisoned buildings
    fn do_team_exit_all_buildings(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::info!("Team '{}' exiting all buildings", team_name);

        let Some(team_arc) = self.get_team_by_name(&team_name).ok() else {
            return Ok(ScriptActionResult::Success);
        };
        let members = team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();

        for member_id in members {
            let Some(member_obj) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            if let Ok(mut member_guard) = member_obj.write() {
                let Some(ai_arc) = member_guard.get_ai_update_interface() else {
                    continue;
                };
                member_guard.leave_group();
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params =
                        AiCommandParams::new(AiCommandType::Exit, CommandSourceType::FromScript);
                    let _ = ai_guard.execute_command(&params);
                };
            };
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGuardPosition()
    /// Team guards at a specified waypoint position
    fn do_team_guard_position(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' guarding position at '{}'",
            team_name,
            waypoint_name
        );

        // Get waypoint position
        let waypoint_name_ascii = AsciiString::from(waypoint_name.as_str());
        let waypoint_pos = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_name_ascii)
                .map(|w| w.get_location().clone())
        });

        if let Some(position) = waypoint_pos {
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params = AiCommandParams::new(
                    AiCommandType::GuardPosition,
                    CommandSourceType::FromScript,
                );
                params.pos = position;
                params.int_value = 0; // GUARDMODE_NORMAL
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!(
                "Waypoint '{}' not found for team guard position",
                waypoint_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamGuardObject()
    /// Team guards a specific named object
    fn do_team_guard_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let object_name = self.get_string_param(action, 1)?;
        log::debug!("Team '{}' guarding object '{}'", team_name, object_name);

        // Get the object ID from name tracker
        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&object_name).ok().flatten();

        if let Some(tid) = target_id {
            if TheGameLogic::find_object_by_id(tid).is_none() {
                log::warn!("Object '{}' object {} no longer exists", object_name, tid);
                return Ok(ScriptActionResult::Success);
            }

            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params =
                    AiCommandParams::new(AiCommandType::GuardObject, CommandSourceType::FromScript);
                params.obj = Some(tid);
                params.int_value = GuardMode::Normal.as_i32();
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Object '{}' not found for team guard", object_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_guard_area(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let area_name = self.get_string_param(action, 1)?;
        log::debug!("Team '{}' guarding area '{}'", team_name, area_name);

        let (area_center, trigger_id) = if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(&area_name) {
                (trigger.get_center_point(), trigger.get_id())
            } else {
                log::warn!("Trigger area '{}' not found for guard", area_name);
                return Ok(ScriptActionResult::Success);
            }
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ));
        };

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let mut params =
                AiCommandParams::new(AiCommandType::GuardArea, CommandSourceType::FromScript);
            params.pos = area_center;
            params.polygon = Some(trigger_id);
            params.int_value = GuardMode::Normal.as_i32();
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_guard_supply_center(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let min_supplies = self.get_int_param(action, 1)?;
        log::debug!(
            "Team '{}' guarding supply center with >= {} supplies",
            team_name,
            min_supplies
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let (members, controlling_player_id) = if let Ok(team) = team_arc.read() {
            (
                team.get_members().to_vec(),
                team.get_controlling_player_id(),
            )
        } else {
            (Vec::new(), None)
        };
        if members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let anchor = members
            .iter()
            .find_map(|&id| {
                TheGameLogic::find_object_by_id(id)
                    .and_then(|o| o.read().ok().map(|g| *g.get_position()))
            })
            .unwrap_or(Coord3D::new(0.0, 0.0, 0.0));

        let base_box_value = global_data::read_safe()
            .map(|d| d.base_value_per_supply_box.max(1))
            .unwrap_or(1) as i32;

        let controlling_player = controlling_player_id.and_then(|id| {
            player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(id as i32).cloned())
        });

        let mut best_target: Option<(f32, ObjectID)> = None;
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.is_destroyed() || obj.is_off_map() {
                continue;
            }
            if !obj.is_kind_of(crate::common::KindOf::SupplySource)
                && !obj.is_kind_of(crate::common::KindOf::ResourceNode)
                && !obj.is_kind_of(crate::common::KindOf::FSSupplyCenter)
            {
                continue;
            }

            if let (Some(owner_id), Some(controller_arc)) =
                (obj.get_controlling_player_id(), controlling_player.as_ref())
            {
                if let Some(owner_arc) = player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(owner_id as i32).cloned())
                {
                    if let (Ok(controller), Ok(owner)) = (controller_arc.read(), owner_arc.read()) {
                        if controller.get_relationship(&owner) == Relationship::Enemies {
                            continue;
                        }
                    }
                }
            }

            let mut supply_value = i32::MAX;
            if let Some(module) = obj.find_update_module("SupplyWarehouseDockUpdate") {
                let mut boxes = None;
                module.with_module_downcast::<crate::object::production::SupplyWarehouseDockUpdateModule, _, _>(|module| {
                    boxes = Some(module.behavior().get_boxes_stored());
                });
                if let Some(boxes) = boxes {
                    supply_value = boxes.saturating_mul(base_box_value);
                }
            }
            if min_supplies > 0 && supply_value < min_supplies {
                continue;
            }

            let pos = obj.get_position();
            let dx = pos.x - anchor.x;
            let dy = pos.y - anchor.y;
            let dist_sq = dx * dx + dy * dy;
            match best_target {
                Some((best_dist, _)) if dist_sq >= best_dist => {}
                _ => best_target = Some((dist_sq, obj.get_id())),
            }
        }

        if let Some((_, target_id)) = best_target {
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            if let Ok(mut group) = group_arc.write() {
                let mut params =
                    AiCommandParams::new(AiCommandType::GuardObject, CommandSourceType::FromScript);
                params.obj = Some(target_id);
                params.int_value = GuardMode::Normal.as_i32();
                let _ = group.ai_do_command(&params);
            };
        } else {
            log::debug!(
                "No qualifying supply center found for '{}'; falling back to TeamGuard",
                team_name
            );
            let _ = self.do_team_guard(action);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_guard_in_tunnel_network(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' guarding in tunnel network", team_name);

        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    let ai_arc = obj_arc
                        .read()
                        .ok()
                        .and_then(|obj| obj.get_ai_update_interface());
                    let Some(ai_arc) = ai_arc else {
                        continue;
                    };
                    if let Ok(mut ai_guard) = ai_arc.lock() {
                        let mut params = AiCommandParams::new(
                            AiCommandType::GuardTunnelNetwork,
                            CommandSourceType::FromScript,
                        );
                        params.int_value = GuardMode::Normal.as_i32();
                        let _ = ai_guard.execute_command(&params);
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    #[allow(dead_code)]
    fn do_team_guard_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Team '{}' guarding for {} frames", team_name, frames);

        // C++ parity: issue guard-at-current-position to each member.
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    for &member_id in team.get_members() {
                        let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            continue;
                        };
                        let Ok(obj) = obj_arc.read() else {
                            continue;
                        };
                        let pos = *obj.get_position();
                        let Some(ai_arc) = obj.get_ai_update_interface() else {
                            continue;
                        };
                        let mut guard_params = AiCommandParams::new(
                            AiCommandType::GuardPosition,
                            CommandSourceType::FromScript,
                        );
                        guard_params.pos = pos;
                        if let Ok(mut ai) = ai_arc.lock() {
                            let _ =
                                ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            let _ = ai.execute_command(&guard_params);
                        };
                    }
                }
            }
        }

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    fn do_team_idle_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Team '{}' idling for {} frames", team_name, frames);

        // C++ parity: idle the team through an AI group.
        if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
            if let Ok(mut group) = group_arc.write() {
                let params =
                    AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
                let _ = group.ai_do_command(&params);
            }
        }

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    fn do_team_spin_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Team '{}' spinning for {} frames", team_name, frames);

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    fn do_team_increase_priority(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(priority) = factory.increase_team_prototype_priority_for_success(&team_name)
            {
                log::debug!(
                    "Increased production priority for team '{}' to {}",
                    team_name,
                    priority
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_team_decrease_priority(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(priority) = factory.decrease_team_prototype_priority_for_failure(&team_name)
            {
                log::debug!(
                    "Decreased production priority for team '{}' to {}",
                    team_name,
                    priority
                );
            }
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFollowWaypointsExact()
    /// Team follows waypoints in exact formation (no pathfinding deviation)
    fn do_team_follow_waypoints_exact(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path = self.get_string_param(action, 1)?;
        log::info!(
            "Team '{}' following waypoints exact '{}'",
            team_name,
            waypoint_path
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let Some(team_center) = self
            .compute_team_center_and_first(&team_arc)
            .map(|(center, _)| center)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let waypoint_id = self.resolve_follow_waypoint_id(&waypoint_path, team_center);

        if let Some(wid) = waypoint_id {
            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params = AiCommandParams::new(
                    AiCommandType::FollowWaypointPathExact,
                    CommandSourceType::FromScript,
                );
                params.waypoint = Some(wid);
                let _ = group.ai_do_command(&params);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamAttackArea()
    /// Team attacks at a trigger area
    fn do_team_attack_area(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let area_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' attacking area '{}'", team_name, area_name);

        // Get trigger area center position
        let (area_center, trigger_id) = if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(&area_name) {
                (trigger.get_center_point(), trigger.get_id())
            } else {
                log::warn!("Trigger area '{}' not found", area_name);
                return Ok(ScriptActionResult::Success);
            }
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ));
        };

        // Issue AttackArea command to team AI group
        let group_arc = self.create_ai_group_from_team(&team_name)?;
        let write_result = group_arc.write();
        if let Ok(mut group) = write_result {
            let mut params =
                AiCommandParams::new(AiCommandType::AttackArea, CommandSourceType::FromScript);
            params.pos = area_center;
            params.polygon = Some(trigger_id);
            let _ = group.ai_do_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamAttackNamed()
    /// Team attacks a specific named object
    fn do_team_attack_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' attacking '{}'", team_name, target_name);

        // Get the object ID from name tracker
        let tracker = get_named_object_tracker();
        let target_id = tracker.get_object_id(&target_name).ok().flatten();

        if let Some(tid) = target_id {
            if TheGameLogic::find_object_by_id(tid).is_none() {
                log::warn!("Target '{}' object {} no longer exists", target_name, tid);
                return Ok(ScriptActionResult::Success);
            }

            let group_arc = self.create_ai_group_from_team(&team_name)?;
            let write_result = group_arc.write();
            if let Ok(mut group) = write_result {
                let mut params = AiCommandParams::new(
                    AiCommandType::AttackObject,
                    CommandSourceType::FromScript,
                );
                params.obj = Some(tid);
                params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                let _ = group.ai_do_command(&params);
            }
        } else {
            log::warn!("Target '{}' not found for team attack", target_name);
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamApplyAttackPrioritySet()
    fn do_team_apply_attack_priority_set(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let priority_set = self.get_string_param(action, 1)?;
        log::info!(
            "Team '{}' applying attack priority set '{}'",
            team_name,
            priority_set
        );

        let info_name = get_script_engine()
            .read()
            .ok()
            .and_then(|engine_guard| {
                engine_guard
                    .as_ref()
                    .and_then(|engine| engine.get_attack_info(&priority_set))
                    .map(|info| info.get_name().to_string())
            })
            .unwrap_or_default();

        let mut prototype_updated = false;
        let mut team_members = Vec::new();
        if let Ok(mut factory) = get_team_factory().lock() {
            prototype_updated =
                factory.set_team_prototype_attack_priority_name(&team_name, info_name.as_str());
            if !prototype_updated {
                if let Some(team_arc) = factory.find_team(&team_name) {
                    if let Ok(team) = team_arc.read() {
                        team_members = team.get_members().to_vec();
                    }
                }
            }
        }

        if !prototype_updated {
            if team_members.is_empty() {
                log::debug!(
                    "Team '{}' has no prototype and no live members for attack priority set '{}'",
                    team_name,
                    info_name
                );
            } else if let Ok(mut engine_guard) = get_script_engine().write() {
                if let Some(engine) = engine_guard.as_mut() {
                    for member_id in team_members {
                        if info_name.is_empty() {
                            engine.clear_object_attack_priority_set(member_id);
                        } else {
                            engine.set_object_attack_priority_set(member_id, info_name.as_str());
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamSetAttitude()
    /// Set team's combat attitude (Aggressive, Normal, Defensive, Passive)
    fn do_team_set_attitude(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let attitude_str = self.get_string_param(action, 1)?;
        log::info!(
            "Team '{}' setting attitude to '{}'",
            team_name,
            attitude_str
        );

        // Parse attitude from string - using AIAttitudeType from modules
        let attitude = match attitude_str.to_uppercase().as_str() {
            "AGGRESSIVE" | "ATTACK" => crate::modules::AIAttitudeType::Aggressive,
            "DEFENSIVE" | "GUARD" => crate::modules::AIAttitudeType::Defensive,
            "PASSIVE" => crate::modules::AIAttitudeType::Passive,
            "SLEEP" => crate::modules::AIAttitudeType::Sleep,
            _ => crate::modules::AIAttitudeType::Normal,
        };

        // Get team and iterate members to set attitude via AI interface
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let object_manager = get_object_manager();
                    if let Ok(obj_manager) = object_manager.read() {
                        for obj_id in team.get_members() {
                            if let Some(obj) = obj_manager.get_object(*obj_id) {
                                if let Ok(obj_read) = obj.read() {
                                    if let Some(ai) = obj_read.get_ai_update_interface() {
                                        if let Ok(mut ai_write) = ai.lock() {
                                            if let Err(err) = ai_write.set_attitude(attitude) {
                                                log::debug!(
                                                    "ScriptActions::do_team_set_attitude failed for object {}: {}",
                                                    obj_id,
                                                    err
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    };
                }
            } else {
                log::warn!("Team '{}' not found for set attitude", team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_execute_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let script_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' executing sequential script '{}'",
            team_name,
            script_name
        );

        let Ok(_team_arc) = self.get_team_by_name(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };

        // C++ parity: idle team before queueing sequential script.
        if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
            if let Ok(mut group) = group_arc.write() {
                let params =
                    AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
                let _ = group.ai_do_command(&params);
            }
        }

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(script) = engine.find_script_clone_by_name(&script_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let mut seq_script = super::engine::SequentialScript::new();
        seq_script.team_to_exec_on = Some(team_name.clone());
        seq_script.object_id = INVALID_ID;
        seq_script.script_to_execute_sequentially = Some(Box::new(script));
        seq_script.times_to_loop = 0;
        engine.append_sequential_script(seq_script);

        Ok(ScriptActionResult::Success)
    }

    fn do_team_execute_sequential_script_looping(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let script_name = self.get_string_param(action, 1)?;
        let loop_val = self.get_int_param(action, 2)? - 1;
        log::debug!(
            "Team '{}' executing sequential script '{}' looping ({})",
            team_name,
            script_name,
            loop_val
        );

        let Ok(_team_arc) = self.get_team_by_name(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };

        // C++ parity: idle team before queueing sequential script.
        if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
            if let Ok(mut group) = group_arc.write() {
                let params =
                    AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
                let _ = group.ai_do_command(&params);
            }
        }

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(script) = engine.find_script_clone_by_name(&script_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let mut seq_script = super::engine::SequentialScript::new();
        seq_script.team_to_exec_on = Some(team_name.clone());
        seq_script.object_id = INVALID_ID;
        seq_script.script_to_execute_sequentially = Some(Box::new(script));
        seq_script.times_to_loop = loop_val;
        engine.append_sequential_script(seq_script);

        Ok(ScriptActionResult::Success)
    }

    fn do_team_stop_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' stopping sequential script", team_name);

        let script_engine_lock = get_script_engine();
        if let Ok(mut engine_guard) = script_engine_lock.write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.remove_all_sequential_scripts_for_team(&team_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_set_emoticon(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let emoticon = self.get_string_param(action, 1)?;
        let duration_seconds = self.get_real_param(action, 2)?;
        let duration_frames = (duration_seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        log::debug!(
            "Team '{}' setting emoticon '{}' for {}s ({}f)",
            team_name,
            emoticon,
            duration_seconds,
            duration_frames
        );

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let members = team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();
        for object_id in members {
            self.emoticon_object_by_id(object_id, &emoticon, duration_frames);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_set_stealth_enabled(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Team '{}' stealth enabled: {}", team_name, enabled);

        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard.set_script_status(
                            crate::object::ObjectScriptStatusBit::ScriptUnstealthed,
                            !enabled,
                        );
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_set_repulsor(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Team '{}' repulsor: {}", team_name, enabled);

        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard
                            .set_status(crate::common::ObjectStatusMaskType::REPULSOR, enabled);
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_create_radar_event(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let event_type = self.get_int_param(action, 1)?;
        log::debug!(
            "Creating radar event for team '{}' (type {})",
            team_name,
            event_type
        );
        let team_arc = self.get_team_by_name(&team_name)?;
        if let Ok(team) = team_arc.read() {
            if !team.has_any_units() {
                return Ok(ScriptActionResult::Success);
            }
            if let Some(pos) = team.get_estimate_team_position() {
                let radar_event = Self::radar_event_type_from_int(event_type);
                if let Ok(mut radar) = get_radar_system().write() {
                    let radar_pos = to_radar_coord(&pos);
                    radar.create_event(&radar_pos, radar_event, 4.0);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_team_delete_living(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::debug!("Deleting living members of team '{}'", team_name);

        // C++ parity: TEAM_DELETE_LIVING -> doTeamDelete(team, TRUE).
        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.delete_team(true);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_wait_for_not_contained_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' waiting for not contained (all)", team_name);
        let all_contained = self.evaluate_team_is_contained(&team_name, true);
        if all_contained {
            Ok(ScriptActionResult::Pending(1.0))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    fn do_team_wait_for_not_contained_partial(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Team '{}' waiting for not contained (partial)", team_name);
        let any_contained = self.evaluate_team_is_contained(&team_name, false);
        if any_contained {
            Ok(ScriptActionResult::Pending(1.0))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    fn evaluate_team_is_contained(&self, team_name: &str, all_contained: bool) -> bool {
        let members = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(team_name))
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_members().to_vec()))
            .unwrap_or_default();
        if members.is_empty() {
            return false;
        }

        let mut any_considered = false;
        for member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            let mut is_contained = obj.get_contained_by().is_some();
            if !is_contained {
                if let Some(ai_arc) = obj.get_ai_update_interface() {
                    if let Ok(ai) = ai_arc.lock() {
                        is_contained = ai.get_current_state_id()
                            == Some(crate::ai::states::AIStateType::Exit as u32);
                    }
                }
            }

            if is_contained {
                if !all_contained {
                    return true;
                }
            } else if all_contained {
                return false;
            }

            any_considered = true;
        }

        if any_considered {
            all_contained
        } else {
            false
        }
    }

    fn do_team_move_towards_nearest_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let trigger_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' moving towards nearest '{}' in trigger '{}'",
            team_name,
            object_type,
            trigger_name
        );

        let team_name = self.resolve_team_name_token(&team_name);
        let (members, estimate_team_pos) = if let Ok(mut factory_guard) = get_team_factory().lock()
        {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                if let Ok(team_guard) = team_arc.read() {
                    (
                        team_guard.get_members().to_vec(),
                        team_guard.get_estimate_team_position(),
                    )
                } else {
                    (Vec::new(), None)
                }
            } else {
                (Vec::new(), None)
            }
        } else {
            (Vec::new(), None)
        };
        if members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let mut source_object_id = INVALID_ID;
        let mut source_off_map = false;
        let mut source_pos = estimate_team_pos.unwrap_or(Coord3D::new(0.0, 0.0, 0.0));
        for &member_id in &members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.get_ai_update_interface().is_some() {
                source_object_id = member_id;
                source_off_map = obj.is_off_map();
                if estimate_team_pos.is_none() {
                    source_pos = *obj.get_position();
                }
                break;
            }
        }
        if source_object_id == INVALID_ID {
            return Ok(ScriptActionResult::Success);
        }

        let Some(target_id) = self.find_closest_object_of_type_in_trigger(
            source_object_id,
            &source_pos,
            source_off_map,
            &object_type,
            &trigger_name,
        ) else {
            return Ok(ScriptActionResult::Success);
        };

        for &member_id in &members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let ai_arc = {
                let Ok(obj) = obj_arc.read() else {
                    continue;
                };
                let Some(ai_arc) = obj.get_ai_update_interface() else {
                    continue;
                };
                ai_arc
            };
            if let Ok(mut ai) = ai_arc.lock() {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params = AiCommandParams::new(
                    AiCommandType::MoveToObject,
                    CommandSourceType::FromScript,
                );
                params.obj = Some(target_id);
                let _ = ai.execute_command(&params);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_hunt_with_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' hunting with command button '{}'",
            team_name,
            command_button_name
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = if let Ok(team_guard) = team_arc.read() {
            team_guard.get_members().to_vec()
        } else {
            Vec::new()
        };

        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(&command_button_name)
        else {
            return Ok(ScriptActionResult::Success);
        };

        match command_button.get_command_type() {
            CommandType::DoSpecialPower => {
                let Some(_sp_template) = command_button.get_special_power_template() else {
                    return Ok(ScriptActionResult::Success);
                };
                let options = SpecialPowerCommandOption::from_bits_truncate(
                    command_button.get_options_bits(),
                );
                let needs_object_target = options.intersects(
                    SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
                );
                if !needs_object_target {
                    log::warn!(
                        "TEAM_HUNT_WITH_COMMAND_BUTTON: '{}' cannot hunt with non-object-target special power",
                        command_button_name
                    );
                    return Ok(ScriptActionResult::Success);
                }
            }
            CommandType::SwitchWeapons
            | CommandType::DoAttackObject
            | CommandType::ConvertToCarbomb
            | CommandType::Enter => {}
            // PARITY_NOTE: C++ ScriptActions.cpp doTeamHuntWithCommandButton() (lines 2047-2073)
            // explicitly rejects these GUI command types and catches all others via `default`.
            // In C++ the explicitly listed types are: GUI_COMMAND_OBJECT_UPGRADE,
            // GUI_COMMAND_PLAYER_UPGRADE, GUI_COMMAND_DOZER_CONSTRUCT,
            // GUI_COMMAND_DOZER_CONSTRUCT_CANCEL, GUI_COMMAND_UNIT_BUILD,
            // GUI_COMMAND_CANCEL_UNIT_BUILD, GUI_COMMAND_CANCEL_UPGRADE,
            // GUI_COMMAND_ATTACK_MOVE, GUI_COMMAND_GUARD, GUI_COMMAND_GUARD_WITHOUT_PURSUIT,
            // GUI_COMMAND_GUARD_FLYING_UNITS_ONLY, GUI_COMMAND_WAYPOINTS,
            // GUI_COMMAND_EXIT_CONTAINER, GUI_COMMAND_EVACUATE,
            // GUI_COMMAND_EXECUTE_RAILED_TRANSPORT, GUI_COMMAND_BEACON_DELETE,
            // GUI_COMMAND_SET_RALLY_POINT, GUI_COMMAND_SELL, GUI_COMMAND_HACK_INTERNET,
            // GUI_COMMAND_TOGGLE_OVERCHARGE (plus conditional POW_RETURN_TO_PRISON,
            // PICK_UP_PRISONER under ALLOW_SURRENDER).
            //
            // Rust maps GUI command strings to CommandType via map_gui_command_to_command_type().
            // Types below are all known-mapped types that C++ rejects (either explicitly or via
            // `default` fallthrough). We list them all explicitly to avoid silent drops.
            CommandType::QueueUpgrade              // C++: OBJECT_UPGRADE, PLAYER_UPGRADE
            | CommandType::DozerConstruct          // C++: DOZER_CONSTRUCT
            | CommandType::DozerCancelConstruct    // C++: DOZER_CONSTRUCT_CANCEL
            | CommandType::QueueUnitCreate         // C++: UNIT_BUILD
            | CommandType::CancelUnitCreate        // C++: CANCEL_UNIT_BUILD
            | CommandType::CancelUpgrade           // C++: CANCEL_UPGRADE
            | CommandType::DoAttackMoveTo          // C++: ATTACK_MOVE
            | CommandType::DoGuardPosition         // C++: GUARD, GUARD_WITHOUT_PURSUIT, GUARD_FLYING_UNITS_ONLY
            | CommandType::DoStop                  // C++: STOP (falls to default)
            | CommandType::AddWaypoint             // C++: WAYPOINTS
            | CommandType::Exit                    // C++: EXIT_CONTAINER
            | CommandType::Evacuate                // C++: EVACUATE
            | CommandType::ExecuteRailedTransport  // C++: EXECUTE_RAILED_TRANSPORT
            | CommandType::CombatDropAtLocation    // C++: COMBATDROP (falls to default)
            | CommandType::RemoveBeacon            // C++: BEACON_DELETE
            | CommandType::SetRallyPoint           // C++: SET_RALLY_POINT
            | CommandType::Sell                    // C++: SELL
            | CommandType::PurchaseScience         // C++: PURCHASE_SCIENCE (falls to default)
            | CommandType::InternetHack            // C++: HACK_INTERNET
            | CommandType::ToggleOvercharge        // C++: TOGGLE_OVERCHARGE
            | CommandType::PlaceBeacon             // C++: PLACE_BEACON (falls to default)
            | CommandType::MetaSelectMatchingUnits // C++: SELECT_ALL_UNITS_OF_TYPE (falls to default)
            => {
                log::warn!(
                    "TEAM_HUNT_WITH_COMMAND_BUTTON: '{}' is not hunt-capable (type {:?})",
                    command_button_name,
                    command_button.get_command_type()
                );
                return Ok(ScriptActionResult::Success);
            }
            // PARITY_NOTE: Unsupported/unknown GUI command strings currently map to Invalid in
            // map_gui_command_to_command_type(). C++ reports script debug errors for these.
            // Also covers conditional C++ types GUI_COMMAND_POW_RETURN_TO_PRISON and
            // GUICOMMANDMODE_PICK_UP_PRISONER (ALLOW_SURRENDER) which have no Rust mapping.
            CommandType::Invalid => {
                log::warn!(
                    "TEAM_HUNT_WITH_COMMAND_BUTTON: '{}' mapped to invalid/unsupported command type",
                    command_button_name
                );
                return Ok(ScriptActionResult::Success);
            }
            // PARITY_NOTE: CommandType variants that exist in the enum but are NOT currently
            // produced by map_gui_command_to_command_type(). These cannot appear from
            // command_button.get_command_type() today, but are listed explicitly for
            // forward-compatibility if the mapping is extended. C++ rejects all of these
            // via `default` (line 2073 of ScriptActions.cpp).
            CommandType::CaptureBuilding
            | CommandType::DisableVehicleHack
            | CommandType::StealCashHack
            | CommandType::DisableBuildingHack
            | CommandType::SnipeVehicle
            | CommandType::DoSalvage
            | CommandType::DoSpecialPowerOverrideDestination
            | CommandType::DoWeapon
            | CommandType::DoWeaponAtLocation
            | CommandType::DoWeaponAtObject
            | CommandType::DoSpecialPowerAtLocation
            | CommandType::DoSpecialPowerAtObject
            | CommandType::DoMoveTo
            | CommandType::DoForceMoveTo
            | CommandType::DoForceAttackObject
            | CommandType::DoForceAttackGround
            | CommandType::DoGuardObject
            | CommandType::DoScatter
            | CommandType::DoAttackSquad
            | CommandType::GetRepaired
            | CommandType::GetHealed
            | CommandType::DoRepair
            | CommandType::ResumeConstruction
            | CommandType::Dock
            | CommandType::DozerConstructLine
            | CommandType::DoCheer
            | CommandType::SelfDestruct
            | CommandType::CreateFormation
            | CommandType::SetMineClearingDetail
            | CommandType::EnableRetaliationMode
            | CommandType::SetBeaconText
            | CommandType::SetReplayCamera
            | CommandType::ClearInGamePopupMessage
            | CommandType::LogicCrc
            | CommandType::CreateSelectedGroup
            | CommandType::CreateSelectedGroupNoSound
            | CommandType::DestroySelectedGroup
            | CommandType::RemoveFromSelectedGroup
            | CommandType::SelectedGroupCommand
            | CommandType::AreaSelection
            | CommandType::CombatDropAtObject
            => {
                log::warn!(
                    "TEAM_HUNT_WITH_COMMAND_BUTTON: '{}' is not hunt-capable (type {:?})",
                    command_button_name,
                    command_button.get_command_type()
                );
                return Ok(ScriptActionResult::Success);
            }
            // Catch-all for any future CommandType variants not explicitly listed above.
            // PARITY_NOTE: C++ uses `default` to reject all unhandled types with an error
            // message. This arm provides the same safety net — if a new CommandType is added
            // to the enum but not handled here, it will be caught and logged rather than
            // silently proceeding to the hunt logic.
            _ => {
                log::warn!(
                    "TEAM_HUNT_WITH_COMMAND_BUTTON: unsupported command button '{}' (type {:?})",
                    command_button_name,
                    command_button.get_command_type()
                );
                return Ok(ScriptActionResult::Success);
            }
        }

        for member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            if obj_guard.get_ai_update_interface().is_none() {
                continue;
            }

            let has_matching_command = control_bar
                .find_command_set_by_name(obj_guard.get_command_set_string())
                .map(|set| {
                    set.buttons.iter().flatten().any(|button| {
                        button.get_id() == command_button.get_id()
                            || button
                                .get_name()
                                .eq_ignore_ascii_case(command_button.get_name())
                    })
                })
                .unwrap_or(false);
            if !has_matching_command {
                continue;
            }

            let Some(module) = obj_guard.find_update_module("CommandButtonHuntUpdate") else {
                log::warn!(
                    "TEAM_HUNT_WITH_COMMAND_BUTTON: object {} requires CommandButtonHuntUpdate for '{}'",
                    member_id,
                    command_button_name
                );
                continue;
            };

            let set_ok = module
                .with_module_downcast::<
                    crate::object::update::command_button_hunt_update::CommandButtonHuntUpdateModule,
                    _,
                    _,
                >(|hunt| {
                    hunt.behavior_mut()
                        .set_command_button(command_button_name.to_string());
                })
                .is_some();
            if !set_ok {
                log::warn!(
                    "TEAM_HUNT_WITH_COMMAND_BUTTON: failed to downcast hunt module for object {}",
                    member_id
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_use_command_button_on_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' using command '{}' on '{}'",
            team_name,
            command_button,
            target_name
        );

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let tracker = get_named_object_tracker();
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let can_use = {
            let Ok(src_guard) = source_obj.read() else {
                return Ok(ScriptActionResult::Success);
            };
            let Ok(target_guard) = target_obj.read() else {
                return Ok(ScriptActionResult::Success);
            };
            command_button.is_valid_to_use_on(
                &src_guard,
                Some(&target_guard),
                None,
                CommandSourceType::FromScript,
            )
        };

        if can_use {
            self.issue_group_command_button_at_object(
                &group_arc,
                command_button.get_id(),
                &target_obj,
            );
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_use_command_button_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' using command '{}' at waypoint '{}'",
            team_name,
            command_button,
            waypoint
        );

        let Some((group_arc, command_button, _source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        self.issue_group_command_button_at_position(&group_arc, command_button.get_id(), &pos);

        Ok(ScriptActionResult::Success)
    }

    fn do_team_use_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!("Team '{}' using command '{}'", team_name, command_button);

        let Some((group_arc, command_button, _source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };
        self.issue_group_command_button(&group_arc, command_button.get_id());

        Ok(ScriptActionResult::Success)
    }

    fn do_team_all_use_command_button_on_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on '{}'",
            team_name,
            command_button,
            target_name
        );

        self.do_team_use_command_button_on_named(action)
    }

    fn do_team_all_use_command_button_on_nearest_enemy_unit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest enemy unit",
            team_name,
            command_button
        );

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| source.relationship_to(candidate) == Relationship::Enemies,
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_all_use_command_button_on_nearest_garrisoned_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest garrisoned building",
            team_name,
            command_button
        );

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |_source, candidate| {
                if !candidate.is_kind_of(crate::common::KindOf::Structure) {
                    return false;
                }
                candidate
                    .get_contain()
                    .and_then(|contain| contain.lock().ok().map(|c| c.is_garrisonable()))
                    .unwrap_or(false)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_all_use_command_button_on_nearest_kindof(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let kindof = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest kindof '{}'",
            team_name,
            command_button,
            kindof
        );

        let Some(kind) = parse_kind_of(&kindof) else {
            return Ok(ScriptActionResult::Success);
        };

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                source.relationship_to(candidate) == Relationship::Enemies
                    && candidate.is_kind_of(kind)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_all_use_command_button_on_nearest_enemy_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest enemy building",
            team_name,
            command_button
        );

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                source.relationship_to(candidate) == Relationship::Enemies
                    && candidate.is_kind_of(crate::common::KindOf::Structure)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_all_use_command_button_on_nearest_enemy_building_class(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let building_class = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest enemy building class '{}'",
            team_name,
            command_button,
            building_class
        );

        let Some(kind) = parse_kind_of(&building_class) else {
            return Ok(ScriptActionResult::Success);
        };

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                source.relationship_to(candidate) == Relationship::Enemies
                    && candidate.is_kind_of(crate::common::KindOf::Structure)
                    && candidate.is_kind_of(kind)
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_all_use_command_button_on_nearest_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let object_type = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' all using command '{}' on nearest object type '{}'",
            team_name,
            command_button,
            object_type
        );

        let wanted_types = self.resolve_object_types_for_action(&object_type);
        if wanted_types.list_size() == 0 {
            return Ok(ScriptActionResult::Success);
        }

        let Some((group_arc, command_button, source_obj)) =
            self.resolve_team_command_button_context(&team_name, &command_button)?
        else {
            return Ok(ScriptActionResult::Success);
        };

        let target_id = self.find_nearest_command_button_target(
            &group_arc,
            &source_obj,
            &command_button,
            |source, candidate| {
                let rel = source.relationship_to(candidate);
                if !matches!(rel, Relationship::Enemies | Relationship::Neutral) {
                    return false;
                }
                let template_ref: &dyn crate::common::ThingTemplate =
                    candidate.get_template().as_ref();
                wanted_types.contains_template(Some(template_ref))
            },
        );

        if let Some(target_id) = target_id {
            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                self.issue_group_command_button_at_object(
                    &group_arc,
                    command_button.get_id(),
                    &target_obj,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_partial_use_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let percentage = self.get_real_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        let command_button_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Team '{}' partial use command '{}' at {}%",
            team_name,
            command_button_name,
            percentage
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = if let Ok(team_guard) = team_arc.read() {
            team_guard.get_members().to_vec()
        } else {
            Vec::new()
        };
        if members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(&command_button_name)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let command_button = command_button.clone();

        let mut valid_members = Vec::new();
        for member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if command_button.is_valid_to_use_on(
                &obj_guard,
                None,
                None,
                CommandSourceType::FromScript,
            ) {
                valid_members.push(member_id);
            }
        }

        if valid_members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let mut num_to_use = ((percentage / 100.0) * valid_members.len() as f32) as i32;
        if num_to_use <= 0 {
            return Ok(ScriptActionResult::Success);
        }
        if num_to_use > valid_members.len() as i32 {
            num_to_use = valid_members.len() as i32;
        }

        let mut count = 0;
        for member_id in valid_members {
            if count >= num_to_use {
                break;
            }
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let _ =
                obj_guard.do_command_button(command_button.get_id(), CommandSourceType::FromScript);
            count += 1;
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_capture_nearest_unowned_faction_unit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::debug!(
            "Team '{}' capturing nearest unowned faction unit",
            team_name
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let group_arc = self.create_ai_group_from_team(&team_name)?;
        let Some(group_center) = group_arc.read().ok().and_then(|group| group.get_center()) else {
            return Ok(ScriptActionResult::Success);
        };

        let controlling_player_arc = team_arc
            .read()
            .ok()
            .and_then(|team| team.get_controlling_player_id())
            .and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as i32).cloned())
            });

        let target_id = ThePartitionManager::get().and_then(|partition| {
            partition.get_closest_object_2d(&group_center, 1_000_000.0, |candidate| {
                if candidate.is_effectively_dead() || candidate.is_off_map() {
                    return false;
                }
                if !candidate.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned) {
                    return false;
                }

                let relationship = if let Some(player_arc) = &controlling_player_arc {
                    if let Ok(player_guard) = player_arc.read() {
                        if let Some(target_team_arc) = candidate.get_team() {
                            if let Ok(target_team_guard) = target_team_arc.read() {
                                player_guard.get_relationship_with_team(&target_team_guard)
                            } else {
                                Relationship::Neutral
                            }
                        } else {
                            Relationship::Neutral
                        }
                    } else {
                        Relationship::Neutral
                    }
                } else {
                    Relationship::Neutral
                };

                matches!(relationship, Relationship::Enemies | Relationship::Neutral)
            })
        });

        if let Some(target_id) = target_id {
            if let Ok(mut group) = group_arc.write() {
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(target_id);
                let _ = group.ai_do_command(&params);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn resolve_team_command_button_context(
        &self,
        team_name: &str,
        ability: &str,
    ) -> Result<
        Option<(
            Arc<RwLock<AiGroup>>,
            crate::command_button::CommandButton,
            Arc<RwLock<crate::object::Object>>,
        )>,
        ScriptError,
    > {
        let resolved_team = self.resolve_team_name_token(team_name);
        let group_arc = self.create_ai_group_from_team(&resolved_team)?;

        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(ability) else {
            return Ok(None);
        };
        let command_button = command_button.clone();

        let source_obj = if let Some(template) = command_button.get_special_power_template() {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_special_power_source_object(template.get_id()))
        } else {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_command_button_source_object(command_button.get_id()))
        };
        let Some(source_obj) = source_obj else {
            return Ok(None);
        };

        Ok(Some((group_arc, command_button, source_obj)))
    }

    fn group_member_ids(&self, group_arc: &Arc<RwLock<AiGroup>>) -> Vec<ObjectID> {
        if let Ok(group) = group_arc.read() {
            group.get_all_ids().clone()
        } else {
            Vec::new()
        }
    }

    fn issue_group_command_button(&self, group_arc: &Arc<RwLock<AiGroup>>, button_id: u32) {
        for member_id in self.group_member_ids(group_arc) {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let _ = obj_guard.do_command_button(button_id, CommandSourceType::FromScript);
        }
    }

    fn issue_group_command_button_at_position(
        &self,
        group_arc: &Arc<RwLock<AiGroup>>,
        button_id: u32,
        pos: &Coord3D,
    ) {
        for member_id in self.group_member_ids(group_arc) {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let _ = obj_guard.do_command_button_at_position(
                button_id,
                pos,
                CommandSourceType::FromScript,
            );
        }
    }

    fn issue_group_command_button_at_object(
        &self,
        group_arc: &Arc<RwLock<AiGroup>>,
        button_id: u32,
        target: &Arc<RwLock<crate::object::Object>>,
    ) {
        let Ok(target_guard) = target.read() else {
            return;
        };
        for member_id in self.group_member_ids(group_arc) {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let _ = obj_guard.do_command_button_at_object(
                button_id,
                &target_guard,
                CommandSourceType::FromScript,
            );
        }
    }

    fn find_nearest_command_button_target<F>(
        &self,
        group_arc: &Arc<RwLock<AiGroup>>,
        source_obj: &Arc<RwLock<crate::object::Object>>,
        command_button: &crate::command_button::CommandButton,
        mut extra_filter: F,
    ) -> Option<ObjectID>
    where
        F: FnMut(&crate::object::Object, &crate::object::Object) -> bool,
    {
        let group_center = group_arc.read().ok().and_then(|group| group.get_center())?;
        let source_guard = source_obj.read().ok()?;
        let source_id = source_guard.get_id();
        let source_off_map = source_guard.is_off_map();

        let partition = ThePartitionManager::get()?;
        partition.get_closest_object_2d(&group_center, 1_000_000.0, |candidate| {
            if candidate.get_id() == source_id {
                return false;
            }
            if candidate.is_effectively_dead() || candidate.is_destroyed() {
                return false;
            }
            if candidate.is_off_map() != source_off_map {
                return false;
            }
            if !extra_filter(&source_guard, candidate) {
                return false;
            }
            command_button.is_valid_to_use_on(
                &source_guard,
                Some(candidate),
                None,
                CommandSourceType::FromScript,
            )
        })
    }

    fn do_team_affect_object_panel_flags(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let flag_name = self.get_string_param(action, 1)?;
        let enable = self.get_int_param(action, 2)? != 0;
        log::debug!(
            "Team '{}' affecting object panel flag '{}' -> {}",
            team_name,
            flag_name,
            enable
        );

        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = if let Ok(team_guard) = team_arc.read() {
                    team_guard.get_members().to_vec()
                } else {
                    Vec::new()
                };
                for object_id in members {
                    if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                        if let Ok(mut obj) = obj_arc.write() {
                            self.apply_object_panel_flag_for_single_object(
                                &mut obj, &flag_name, enable,
                            );
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_set_unmanned_status(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let team_name = self.resolve_team_name_token(&team_name);
        log::debug!("Team '{}' set unmanned", team_name);

        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    self.mark_object_unmanned(object_id);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_team_set_boobytrapped(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let boobytrap_template = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        let team_name = self.resolve_team_name_token(&team_name);
        log::debug!(
            "Team '{}' set boobytrapped using template '{}'",
            team_name,
            boobytrap_template
        );

        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let _ = self.attach_boobytrap_to_object(&boobytrap_template, object_id);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFaceNamed()
    /// Makes team members face towards a named object
    fn do_team_face_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' facing '{}'", team_name, target_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            log::warn!("Target '{}' not found for team face", target_name);
            return Ok(ScriptActionResult::Success);
        };
        if TheGameLogic::find_object_by_id(target_id).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                            continue;
                        };
                        obj_guard.leave_group();
                        if let Ok(mut ai_guard) = ai_arc.lock() {
                            let _ = ai_guard
                                .choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            let mut params = AiCommandParams::new(
                                AiCommandType::FaceObject,
                                CommandSourceType::FromScript,
                            );
                            params.obj = Some(target_id);
                            let _ = ai_guard.execute_command(&params);
                        };
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamFaceWaypoint()
    /// Makes team members face towards a waypoint
    fn do_team_face_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::info!("Team '{}' facing waypoint '{}'", team_name, waypoint_name);

        let waypoint_pos = self.get_waypoint_position(&waypoint_name)?;
        let waypoint_pos =
            crate::common::Coord3D::new(waypoint_pos.x, waypoint_pos.y, waypoint_pos.z);

        let team_name = self.resolve_team_name_token(&team_name);
        if let Ok(mut factory_guard) = get_team_factory().lock() {
            if let Some(team_arc) = factory_guard.find_team(&team_name) {
                let members = team_arc
                    .read()
                    .map(|team| team.get_members().to_vec())
                    .unwrap_or_default();
                for object_id in members {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                        continue;
                    };
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                            continue;
                        };
                        obj_guard.leave_group();
                        if let Ok(mut ai_guard) = ai_arc.lock() {
                            let _ = ai_guard
                                .choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            let mut params = AiCommandParams::new(
                                AiCommandType::FacePosition,
                                CommandSourceType::FromScript,
                            );
                            params.pos = waypoint_pos;
                            let _ = ai_guard.execute_command(&params);
                        };
                    };
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL NAMED UNIT ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_named_enter_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::info!("Unit '{}' entering '{}'", unit_name, target_name);

        // Look up both objects
        let tracker = get_named_object_tracker();
        let unit_id = tracker.get_object_id(&unit_name).ok().flatten();
        let target_id = tracker.get_object_id(&target_name).ok().flatten();

        if let (Some(uid), Some(tid)) = (unit_id, target_id) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(uid) {
                if let Ok(obj) = obj_arc.read() {
                    if let Some(ai_arc) = obj.get_ai_update_interface() {
                        if let Ok(mut ai) = ai_arc.lock() {
                            let mut params = AiCommandParams::new(
                                AiCommandType::Enter,
                                CommandSourceType::FromScript,
                            );
                            params.obj = Some(tid);
                            let _ = ai.execute_command(&params);
                            log::info!(
                                "Unit '{}' enter command issued to '{}'",
                                unit_name,
                                target_name
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!(
                "Unit '{}' or target '{}' not found for enter command",
                unit_name,
                target_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_exit_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::info!("Unit '{}' exiting all contained", unit_name);

        // Look up object and issue Evacuate command
        let tracker = get_named_object_tracker();
        let object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(oid) = object_id {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(oid) {
                if let Ok(obj) = obj_arc.read() {
                    if let Some(ai_arc) = obj.get_ai_update_interface() {
                        if let Ok(mut ai) = ai_arc.lock() {
                            let params = AiCommandParams::new(
                                AiCommandType::Evacuate,
                                CommandSourceType::FromScript,
                            );
                            let _ = ai.execute_command(&params);
                            log::info!("Unit '{}' evacuate command issued", unit_name);
                        }
                    }
                }
            }
        } else {
            log::warn!("Unit '{}' not found for exit command", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_follow_waypoints(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' following waypoints '{}'",
            unit_name,
            waypoint_name
        );

        // Look up object and waypoint, issue FollowWaypointPath command
        let tracker = get_named_object_tracker();
        let object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(oid) = object_id {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(oid) {
                let reference_pos = obj_arc.read().ok().map(|obj| *obj.get_position());
                let waypoint_id = reference_pos
                    .and_then(|pos| self.resolve_follow_waypoint_id(&waypoint_name, pos));
                if let Ok(obj) = obj_arc.read() {
                    if let Some(ai_arc) = obj.get_ai_update_interface() {
                        if let Ok(mut ai) = ai_arc.lock() {
                            let mut params = AiCommandParams::new(
                                AiCommandType::FollowWaypointPath,
                                CommandSourceType::FromScript,
                            );
                            let Some(waypoint_id) = waypoint_id else {
                                return Ok(ScriptActionResult::Success);
                            };
                            params.waypoint = Some(waypoint_id);
                            let _ = ai.execute_command(&params);
                            log::debug!(
                                "Unit '{}' follow waypoints '{}' command issued",
                                unit_name,
                                waypoint_name
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Unit '{}' not found for follow waypoints", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_follow_waypoints_exact(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' following waypoints '{}' exact",
            unit_name,
            waypoint_name
        );

        // Look up object and waypoint, issue FollowWaypointPathExact command
        let tracker = get_named_object_tracker();
        let object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(oid) = object_id {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(oid) {
                let reference_pos = obj_arc.read().ok().map(|obj| *obj.get_position());
                let waypoint_id = reference_pos
                    .and_then(|pos| self.resolve_follow_waypoint_id(&waypoint_name, pos));
                if let Ok(obj) = obj_arc.read() {
                    if let Some(ai_arc) = obj.get_ai_update_interface() {
                        if let Ok(mut ai) = ai_arc.lock() {
                            let mut params = AiCommandParams::new(
                                AiCommandType::FollowWaypointPathExact,
                                CommandSourceType::FromScript,
                            );
                            let Some(waypoint_id) = waypoint_id else {
                                return Ok(ScriptActionResult::Success);
                            };
                            params.waypoint = Some(waypoint_id);
                            let _ = ai.execute_command(&params);
                            log::info!(
                                "Unit '{}' follow waypoints exact '{}' command issued",
                                unit_name,
                                waypoint_name
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Unit '{}' not found for follow waypoints exact", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_attack_area(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let area_name = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' attacking area '{}'", unit_name, area_name);

        let (area_center, trigger_id) = if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(&area_name) {
                (trigger.get_center_point(), trigger.get_id())
            } else {
                log::warn!("Trigger area '{}' not found", area_name);
                return Ok(ScriptActionResult::Success);
            }
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ));
        };

        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());
                if let Some(ai_arc) = ai_result {
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard.leave_group();
                    }
                    if let Ok(mut ai) = ai_arc.lock() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    }

                    let mut params = AiCommandParams::new(
                        AiCommandType::AttackArea,
                        CommandSourceType::FromScript,
                    );
                    params.pos = area_center;
                    params.polygon = Some(trigger_id);
                    let _ = ai_arc.lock().ok().map(|mut ai| {
                        let _ = ai.execute_command(&params);
                        log::info!(
                            "Named unit '{}' attack area '{}' command issued (ID: {})",
                            unit_name,
                            area_name,
                            object_id
                        );
                    });
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for attack area", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_attack_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 1)?);

        log::info!("Unit '{}' attacking team '{}'", unit_name, team_name);

        if self.get_team_by_name(&team_name).is_err() {
            log::warn!(
                "Target team '{}' not found for named attack team",
                team_name
            );
            return Ok(ScriptActionResult::Success);
        }

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());
                if let Some(ai_arc) = ai_result {
                    if let Ok(mut obj_guard) = obj_arc.write() {
                        obj_guard.leave_group();
                    }
                    if let Ok(mut ai) = ai_arc.lock() {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    }

                    let mut params = AiCommandParams::new(
                        AiCommandType::AttackTeam,
                        CommandSourceType::FromScript,
                    );
                    params.team = Some(team_name.clone());
                    params.int_value = -1; // NO_MAX_SHOTS_LIMIT
                    let _ = ai_arc.lock().ok().map(|mut ai| {
                        let _ = ai.execute_command(&params);
                        log::info!(
                            "Named unit '{}' attack team '{}' command issued (ID: {})",
                            unit_name,
                            team_name,
                            object_id
                        );
                    });
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for attack team", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_apply_attack_priority_set(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let priority_set = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' applying attack priority set '{}'",
            unit_name,
            priority_set
        );

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(&unit_name).ok().flatten() else {
            log::warn!(
                "Named unit '{}' not found for attack priority set '{}'",
                unit_name,
                priority_set
            );
            return Ok(ScriptActionResult::Success);
        };

        if TheGameLogic::find_object_by_id(object_id).is_none() {
            log::warn!(
                "Named unit '{}' object {} no longer exists for attack priority set '{}'",
                unit_name,
                object_id,
                priority_set
            );
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(mut engine_lock) = get_script_engine().write() {
            if let Some(engine) = engine_lock.as_mut() {
                let resolved_name = engine
                    .get_attack_info(&priority_set)
                    .map(|info| info.get_name().to_string())
                    .unwrap_or_default();
                if resolved_name.is_empty() {
                    engine.clear_object_attack_priority_set(object_id);
                } else {
                    engine.set_object_attack_priority_set(object_id, resolved_name.as_str());
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_attitude(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let attitude_str = self.get_string_param(action, 1)?;

        log::info!(
            "Unit '{}' setting attitude to '{}'",
            unit_name,
            attitude_str
        );

        // Parse attitude string to AIAttitudeType (Normal, Aggressive, Defensive, Passive)
        let attitude = match attitude_str.to_uppercase().as_str() {
            "PASSIVE" => AIAttitudeType::Passive,
            "SLEEP" => AIAttitudeType::Sleep,
            "NORMAL" => AIAttitudeType::Normal,
            "DEFENSIVE" | "ALERT" => AIAttitudeType::Defensive,
            "AGGRESSIVE" => AIAttitudeType::Aggressive,
            _ => {
                log::warn!(
                    "Unknown attitude type '{}' for unit '{}'",
                    attitude_str,
                    unit_name
                );
                return Ok(ScriptActionResult::Success);
            }
        };

        // Look up object ID by name
        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&unit_name).ok().flatten();

        if let Some(object_id) = object_id_opt {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                let ai_result = obj_arc
                    .read()
                    .ok()
                    .and_then(|obj| obj.get_ai_update_interface());
                if let Some(ai_arc) = ai_result {
                    if let Ok(mut ai) = ai_arc.lock() {
                        if let Err(err) = ai.set_attitude(attitude) {
                            log::debug!(
                                "ScriptActions::do_named_set_attitude failed for object {}: {}",
                                object_id,
                                err
                            );
                        }
                        log::info!(
                            "Named unit '{}' attitude set to {:?} (ID: {})",
                            unit_name,
                            attitude,
                            object_id
                        );
                    }
                } else {
                    log::warn!("Named unit '{}' has no AI update interface", unit_name);
                }
            } else {
                log::warn!("Named unit '{}' not found in object registry", unit_name);
            }
        } else {
            log::warn!("Named unit '{}' not found for set attitude", unit_name);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_flash(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!("Flashing unit '{}' for {}s", unit_name, time_in_seconds);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.flash_object_by_id(object_id, time_in_seconds, None);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_flash_white(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!(
            "Flashing unit '{}' white for {}s",
            unit_name,
            time_in_seconds
        );

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.flash_object_by_id(object_id, time_in_seconds, Some(Color::white()));
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_garrison_specific_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let building_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' garrisoning building '{}'",
            unit_name,
            building_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(building_id)) = tracker.get_object_id(&building_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let Some(unit_obj) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(building_obj) = TheGameLogic::find_object_by_id(building_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let can_garrison = if let (Ok(unit_guard), Ok(building_guard)) =
            (unit_obj.read(), building_obj.read())
        {
            let player_mask = unit_guard
                .get_controlling_player()
                .and_then(|p| p.read().ok().map(|player| player.get_player_mask()))
                .unwrap_or_else(crate::common::PlayerMaskType::none);

            if !building_guard.is_kind_of(crate::common::KindOf::Structure) {
                false
            } else if let Some(contain) = building_guard.get_contain() {
                let entered_mask = contain
                    .lock()
                    .ok()
                    .map(|c| c.get_player_who_entered())
                    .unwrap_or_else(crate::common::PlayerMaskType::none);
                entered_mask == crate::common::PlayerMaskType::none() || entered_mask == player_mask
            } else {
                false
            }
        } else {
            false
        };
        if !can_garrison {
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(mut unit_guard) = unit_obj.write() {
            let Some(ai_arc) = unit_guard.get_ai_update_interface() else {
                return Ok(ScriptActionResult::Success);
            };
            unit_guard.leave_group();
            if let Ok(mut ai_guard) = ai_arc.lock() {
                let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(building_id);
                let _ = ai_guard.execute_command(&params);
            };
        };

        Ok(ScriptActionResult::Success)
    }

    fn do_named_garrison_nearest_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' garrisoning nearest building", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(unit_obj) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let (unit_pos, unit_off_map, unit_is_hacker, unit_player_mask) =
            if let Ok(unit_guard) = unit_obj.read() {
                (
                    *unit_guard.get_position(),
                    unit_guard.is_off_map(),
                    unit_guard.is_kind_of(crate::common::KindOf::Hacker),
                    unit_guard
                        .get_controlling_player()
                        .and_then(|p| p.read().ok().map(|player| player.get_player_mask()))
                        .unwrap_or_else(crate::common::PlayerMaskType::none),
                )
            } else {
                return Ok(ScriptActionResult::Success);
            };

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(ScriptActionResult::Success);
        };

        let closest_building_id = partition.get_closest_object_2d(&unit_pos, 1_000_000.0, |obj| {
            if obj.get_id() == unit_id {
                return false;
            }
            if obj.is_effectively_dead() || obj.is_off_map() != unit_off_map {
                return false;
            }
            if !obj.is_kind_of(crate::common::KindOf::Structure) {
                return false;
            }

            let is_internet_center = obj.is_kind_of(crate::common::KindOf::FSInternetCenter);
            if unit_is_hacker {
                if !is_internet_center {
                    return false;
                }
            } else if is_internet_center {
                return false;
            }

            let Some(contain) = obj.get_contain() else {
                return false;
            };
            let entered_mask = contain
                .lock()
                .ok()
                .map(|c| c.get_player_who_entered())
                .unwrap_or_else(crate::common::PlayerMaskType::none);
            entered_mask == crate::common::PlayerMaskType::none()
                || entered_mask == unit_player_mask
        });

        let Some(target_id) = closest_building_id else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut unit_guard) = unit_obj.write() {
            let Some(ai_arc) = unit_guard.get_ai_update_interface() else {
                return Ok(ScriptActionResult::Success);
            };
            unit_guard.leave_group();
            if let Ok(mut ai_guard) = ai_arc.lock() {
                let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params =
                    AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                params.obj = Some(target_id);
                let _ = ai_guard.execute_command(&params);
            };
        };

        Ok(ScriptActionResult::Success)
    }

    fn do_named_exit_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' exiting building", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(unit_obj) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut unit_guard) = unit_obj.write() {
            let Some(ai_arc) = unit_guard.get_ai_update_interface() else {
                return Ok(ScriptActionResult::Success);
            };
            unit_guard.leave_group();
            if let Ok(mut ai_guard) = ai_arc.lock() {
                let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let params =
                    AiCommandParams::new(AiCommandType::Exit, CommandSourceType::FromScript);
                let _ = ai_guard.execute_command(&params);
            };
        };

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_stopping_distance(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let distance = self.get_real_param(action, 1)?;
        log::debug!(
            "Unit '{}' setting stopping distance to {}",
            unit_name,
            distance
        );

        if distance < 0.5 {
            return Ok(ScriptActionResult::Success);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(unit_obj) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let ai_arc = unit_obj
            .read()
            .ok()
            .and_then(|unit| unit.get_ai_update_interface());
        let Some(ai_arc) = ai_arc else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(ai_guard) = ai_arc.lock() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(loco_arc) = ai_guard.get_cur_locomotor() else {
            return Ok(ScriptActionResult::Success);
        };
        if let Ok(mut loco_guard) = loco_arc.lock() {
            loco_guard.set_close_enough_dist(distance);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_transfer_ownership_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Transferring unit '{}' to player '{}'",
            unit_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(unit_obj) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let destination_team = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()));
        let Some(destination_team) = destination_team else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut unit_guard) = unit_obj.write() {
            let old_owner = unit_guard.get_controlling_player();
            let _ = unit_guard.set_team(Some(destination_team));
            let new_owner = unit_guard.get_controlling_player();
            unit_guard.on_capture(old_owner, new_owner);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_hide_special_power_display(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding special power display for '{}'", unit_name);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) =
                            handler.hide_object_superweapon_display_by_script(object_id)
                        {
                            log::warn!(
                                "Script action handler hide_object_superweapon_display_by_script failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_show_special_power_display(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Showing special power display for '{}'", unit_name);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) =
                            handler.show_object_superweapon_display_by_script(object_id)
                        {
                            log::warn!(
                                "Script action handler show_object_superweapon_display_by_script failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_stop_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        log::debug!(
            "Stopping special power countdown '{}' for '{}'",
            special_power,
            unit_name
        );

        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            sp_module.pause_countdown(true);
        });

        Ok(ScriptActionResult::Success)
    }

    fn do_named_start_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        log::debug!(
            "Starting special power countdown '{}' for '{}'",
            special_power,
            unit_name
        );

        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            sp_module.pause_countdown(false);
        });

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        let countdown = self.get_int_param(action, 2)?;
        log::debug!(
            "Setting special power countdown '{}' for '{}' to {}s",
            special_power,
            unit_name,
            countdown
        );

        let frames = countdown.saturating_mul(LOGICFRAMES_PER_SECOND as i32);
        let base_frame = TheGameLogic::get_frame();
        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            let ready_frame = base_frame.saturating_add_signed(frames);
            sp_module.set_ready_frame(ready_frame);
        });

        Ok(ScriptActionResult::Success)
    }

    fn do_named_add_special_power_countdown(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let special_power = self.get_string_param(action, 1)?;
        let amount = self.get_int_param(action, 2)?;
        log::debug!(
            "Adding {}s to special power countdown '{}' for '{}'",
            amount,
            special_power,
            unit_name
        );

        let frames = amount.saturating_mul(LOGICFRAMES_PER_SECOND as i32);
        self.with_named_special_power_module_mut(&unit_name, &special_power, |sp_module| {
            let new_ready_frame = sp_module.get_ready_frame().saturating_add_signed(frames);
            sp_module.set_ready_frame(new_ready_frame);
        });

        Ok(ScriptActionResult::Success)
    }

    fn do_named_fire_special_power_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let power_name = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' firing special power '{}' at waypoint '{}'",
            unit_name,
            power_name,
            waypoint
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(waypoint_pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        let template_name = {
            let Some(store) = get_special_power_store() else {
                return Ok(ScriptActionResult::Success);
            };
            let Some(template) = store.find_special_power_template(&power_name) else {
                return Ok(ScriptActionResult::Success);
            };
            template.get_name().to_string()
        };

        if let Ok(source_guard) = source_obj.read() {
            let _ =
                source_guard.with_special_power_module_mut_by_name(&template_name, |sp_module| {
                    sp_module.do_special_power_at_location(
                        &waypoint_pos,
                        INVALID_ANGLE,
                        SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT,
                    );
                });
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_fire_special_power_at_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let power_name = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' firing special power '{}' at '{}'",
            unit_name,
            power_name,
            target_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptActionResult::Success);
        };
        if TheGameLogic::find_object_by_id(target_id).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        let template_name = {
            let Some(store) = get_special_power_store() else {
                return Ok(ScriptActionResult::Success);
            };
            let Some(template) = store.find_special_power_template(&power_name) else {
                return Ok(ScriptActionResult::Success);
            };
            template.get_name().to_string()
        };

        if let Ok(source_guard) = source_obj.read() {
            let _ =
                source_guard.with_special_power_module_mut_by_name(&template_name, |sp_module| {
                    sp_module.do_special_power_at_object(
                        target_id,
                        SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT,
                    );
                });
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_fire_weapon_following_waypoint_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint_path = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' firing weapon following waypoint path '{}'",
            unit_name,
            waypoint_path
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let (source_pos, waypoint) = if let Ok(source_guard) = source_obj.read() {
            let source_pos = *source_guard.get_position();
            let waypoint = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&source_pos, &waypoint_path)
                    .map(crate::waypoint::Waypoint::from_terrain)
            });
            (source_pos, waypoint)
        } else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(waypoint) = waypoint else {
            return Ok(ScriptActionResult::Success);
        };

        let max_object_id_before_fire = get_object_manager()
            .read()
            .ok()
            .and_then(|mgr| mgr.all_object_ids().into_iter().max())
            .unwrap_or(0);

        let fired = if let Ok(mut source_guard) = source_obj.write() {
            if let Some(weapon) = source_guard
                .weapon_set
                .find_waypoint_following_capable_weapon()
            {
                let _ = weapon.force_fire_weapon(source_id, &source_pos);
                true
            } else {
                false
            }
        } else {
            false
        };
        if !fired {
            return Ok(ScriptActionResult::Success);
        }

        let Some(projectile_id) =
            self.find_recent_projectile_fired_by(source_id, max_object_id_before_fire)
        else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(projectile_obj) = TheGameLogic::find_object_by_id(projectile_id) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut projectile_guard) = projectile_obj.write() {
            let ai = projectile_guard.get_ai_update_interface();
            projectile_guard.leave_group();
            if let Some(ai_arc) = ai {
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowWaypointPath,
                        CommandSourceType::FromScript,
                    );
                    params.waypoint = Some(waypoint.id);
                    let _ = ai_guard.execute_command(&params);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_use_command_button_on_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let target_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' using command '{}' on '{}'",
            unit_name,
            command_button,
            target_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let button_ids = if let Ok(source_guard) = source_obj.read() {
            self.matching_command_button_ids_for_object(&source_guard, &command_button)
        } else {
            Vec::new()
        };
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        if let (Ok(source_guard), Ok(target_guard)) = (source_obj.read(), target_obj.read()) {
            for button_id in button_ids {
                let _ = source_guard.do_command_button_at_object(
                    button_id,
                    &target_guard,
                    CommandSourceType::FromScript,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_use_command_button_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' using command '{}' at waypoint '{}'",
            unit_name,
            command_button,
            waypoint
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };
        let Some(waypoint_pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        let button_ids = if let Ok(source_guard) = source_obj.read() {
            self.matching_command_button_ids_for_object(&source_guard, &command_button)
        } else {
            Vec::new()
        };
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(source_guard) = source_obj.read() {
            for button_id in button_ids {
                let _ = source_guard.do_command_button_at_position(
                    button_id,
                    &waypoint_pos,
                    CommandSourceType::FromScript,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_use_command_button(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' using command '{}'", unit_name, command_button);

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let button_ids = if let Ok(source_guard) = source_obj.read() {
            self.matching_command_button_ids_for_object(&source_guard, &command_button)
        } else {
            Vec::new()
        };
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(source_guard) = source_obj.read() {
            for button_id in button_ids {
                let _ = source_guard.do_command_button(button_id, CommandSourceType::FromScript);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_use_command_button_using_waypoint_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let command_button = self.get_string_param(action, 1)?;
        let waypoint_path = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' using command '{}' along waypoint path '{}'",
            unit_name,
            command_button,
            waypoint_path
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(source_obj) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let waypoint = if let Ok(source_guard) = source_obj.read() {
            let source_pos = *source_guard.get_position();
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&source_pos, &waypoint_path)
                    .map(crate::waypoint::Waypoint::from_terrain)
            })
        } else {
            None
        };
        let Some(waypoint) = waypoint else {
            return Ok(ScriptActionResult::Success);
        };

        let button_ids = if let Ok(source_guard) = source_obj.read() {
            self.matching_command_button_ids_for_object(&source_guard, &command_button)
        } else {
            Vec::new()
        };
        if button_ids.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(source_guard) = source_obj.read() {
            for button_id in button_ids {
                let _ = source_guard.do_command_button_using_waypoints(
                    button_id,
                    &waypoint,
                    CommandSourceType::FromScript,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn matching_command_button_ids_for_object(
        &self,
        obj: &crate::object::Object,
        ability: &str,
    ) -> Vec<u32> {
        let Some(control_bar) = get_control_bar_bridge() else {
            return Vec::new();
        };
        let Some(command_set) = control_bar.find_command_set_by_name(obj.get_command_set_string())
        else {
            return Vec::new();
        };

        let matches: Vec<_> = command_set
            .buttons
            .iter()
            .flatten()
            .filter(|command_button| {
                !command_button.get_name().is_empty() && command_button.get_name() == ability
            })
            .map(|command_button| command_button.get_id())
            .collect();
        matches
    }

    fn do_named_receive_upgrade(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let upgrade_name = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' receiving upgrade '{}'", unit_name, upgrade_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let upgrade = get_upgrade_center()
            .read()
            .ok()
            .and_then(|center| center.find_upgrade(upgrade_name.as_str()));
        let Some(upgrade) = upgrade else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut obj_guard) = obj_arc.write() {
            if obj_guard.affected_by_upgrade(upgrade.as_ref()) {
                obj_guard.give_upgrade(upgrade.as_ref());
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_held(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let held = self.get_int_param(action, 1)? != 0;
        log::debug!("Unit '{}' held: {}", unit_name, held);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    let _ = obj_guard.set_disabled_held(held);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_topple_direction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let dir = self.get_coord_param(action, 1)?;
        let direction = crate::common::Coord3D::new(dir.x, dir.y, dir.z);
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.set_topple_direction(&unit_name, Some(direction));
            }
        }
        log::debug!("Setting topple direction for '{}'", unit_name);
        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_repulsor(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Unit '{}' repulsor: {}", unit_name, enabled);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    obj_guard.set_status(crate::common::ObjectStatusMaskType::REPULSOR, enabled);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_custom_color(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let color_raw = self.get_int_param(action, 1)? as u32;
        let color = crate::common::Color::new(
            (color_raw & 0xFF) as u8,
            ((color_raw >> 8) & 0xFF) as u8,
            ((color_raw >> 16) & 0xFF) as u8,
            ((color_raw >> 24) & 0xFF) as u8,
        );
        log::debug!(
            "Setting custom color for '{}': 0x{:08X}",
            unit_name,
            color_raw
        );

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    obj_guard.set_custom_indicator_color(color);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_stealth_enabled(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let enabled = self.get_int_param(action, 1)? != 0;
        log::debug!("Unit '{}' stealth enabled: {}", unit_name, enabled);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut obj_guard) = obj_arc.write() {
                    obj_guard.set_script_status(
                        crate::object::ObjectScriptStatusBit::ScriptUnstealthed,
                        !enabled,
                    );
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_emoticon(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let emoticon = self.get_string_param(action, 1)?;
        let duration_seconds = self.get_real_param(action, 2)?;
        let duration_frames = (duration_seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        log::debug!(
            "Unit '{}' emoticon '{}' for {}s ({}f)",
            unit_name,
            emoticon,
            duration_seconds,
            duration_frames
        );

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.emoticon_object_by_id(object_id, &emoticon, duration_frames);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_face_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let target_name = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' facing '{}'", unit_name, target_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(Some(target_id)) = tracker.get_object_id(&target_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };
        if TheGameLogic::find_object_by_id(target_id).is_none() {
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(mut obj_guard) = obj_arc.write() {
            let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                return Ok(ScriptActionResult::Success);
            };
            obj_guard.leave_group();
            if let Ok(mut ai_guard) = ai_arc.lock() {
                let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params =
                    AiCommandParams::new(AiCommandType::FaceObject, CommandSourceType::FromScript);
                params.obj = Some(target_id);
                let _ = ai_guard.execute_command(&params);
            };
        };

        Ok(ScriptActionResult::Success)
    }

    fn do_named_face_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let waypoint = self.get_string_param(action, 1)?;
        log::debug!("Unit '{}' facing waypoint '{}'", unit_name, waypoint);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|way| *way.get_location())
            })
        };
        let Some(waypoint_pos) = waypoint_pos else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut obj_guard) = obj_arc.write() {
            let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                return Ok(ScriptActionResult::Success);
            };
            obj_guard.leave_group();
            if let Ok(mut ai_guard) = ai_arc.lock() {
                let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let mut params = AiCommandParams::new(
                    AiCommandType::FacePosition,
                    CommandSourceType::FromScript,
                );
                params.pos = waypoint_pos;
                let _ = ai_guard.execute_command(&params);
            };
        };

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_evac_left_or_right(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let disposition = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(0);
        log::debug!(
            "Setting evac disposition for '{}' to {}",
            unit_name,
            disposition
        );

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    if let Some(contain) = obj_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            contain_guard.set_evac_disposition(disposition.max(0) as u32);
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_unmanned_status(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' set unmanned", unit_name);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            self.mark_object_unmanned(object_id);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_named_set_boobytrapped(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let boobytrap_template = self.get_string_param(action, 0)?;
        let unit_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' set boobytrapped using template '{}'",
            unit_name,
            boobytrap_template
        );

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            let _ = self.attach_boobytrap_to_object(&boobytrap_template, object_id);
        }

        Ok(ScriptActionResult::Success)
    }

    fn mark_object_unmanned(&self, object_id: ObjectID) {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        if let Ok(mut obj_guard) = obj_arc.write() {
            obj_guard.set_disabled_unmanned();
            let _ = TheGameLogic::deselect_object(&*obj_guard, crate::common::PLAYERMASK_ALL, true);
            obj_guard.set_team_to_neutral();
        };
    }

    fn attach_boobytrap_to_object(
        &self,
        boobytrap_template_name: &str,
        target_object_id: ObjectID,
    ) -> bool {
        let Some(target_obj) = TheGameLogic::find_object_by_id(target_object_id) else {
            return false;
        };
        let target_team = target_obj.read().ok().and_then(|obj| obj.get_team());

        let Some(template) = TheObjectFactory::find_template(boobytrap_template_name) else {
            return false;
        };
        let Ok(boobytrap_obj) = TheObjectFactory::new_object(template, target_team) else {
            return false;
        };

        let module = boobytrap_obj
            .read()
            .ok()
            .and_then(|obj| obj.find_update_module("StickyBombUpdate"));
        let Some(module) = module else {
            return false;
        };
        let Ok(target_guard) = target_obj.read() else {
            return false;
        };

        let mut initialized = false;
        let _ = module.with_module_downcast::<
            crate::object::behavior::sticky_bomb_update::StickyBombUpdateModule,
            _,
            _,
        >(|sticky_module| {
            sticky_module
                .behavior_mut()
                .init_sticky_bomb(Some(&*target_guard), None, None);
            initialized = true;
        });

        initialized
    }

    fn find_recent_projectile_fired_by(
        &self,
        source_object_id: ObjectID,
        minimum_new_object_id: ObjectID,
    ) -> Option<ObjectID> {
        let manager = get_object_manager();
        let ids = manager.read().ok()?.all_object_ids();

        let mut latest = None;
        for object_id in ids {
            if object_id <= minimum_new_object_id {
                continue;
            }
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_producer_id() != source_object_id {
                continue;
            }
            if !obj_guard.is_kind_of(crate::common::KindOf::Projectile) {
                continue;
            }
            if latest.is_none_or(|current| object_id > current) {
                latest = Some(object_id);
            }
        }

        latest
    }

    fn do_unit_execute_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let script_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Unit '{}' executing sequential script '{}'",
            unit_name,
            script_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(script) = engine.find_script_clone_by_name(&script_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let mut seq_script = super::engine::SequentialScript::new();
        seq_script.object_id = object_id;
        seq_script.script_to_execute_sequentially = Some(Box::new(script));
        seq_script.times_to_loop = 0;
        engine.append_sequential_script(seq_script);

        Ok(ScriptActionResult::Success)
    }

    fn do_unit_execute_sequential_script_looping(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let script_name = self.get_string_param(action, 1)?;
        let loop_val = self.get_int_param(action, 2)? - 1;
        log::debug!(
            "Unit '{}' executing sequential script '{}' looping ({})",
            unit_name,
            script_name,
            loop_val
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(script) = engine.find_script_clone_by_name(&script_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let mut seq_script = super::engine::SequentialScript::new();
        seq_script.object_id = object_id;
        seq_script.script_to_execute_sequentially = Some(Box::new(script));
        seq_script.times_to_loop = loop_val;
        engine.append_sequential_script(seq_script);

        Ok(ScriptActionResult::Success)
    }

    fn do_unit_stop_sequential_script(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Unit '{}' stopping sequential script", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let script_engine_lock = get_script_engine();
        if let Ok(mut engine_guard) = script_engine_lock.write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.remove_all_sequential_scripts_for_object(object_id);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_unit_guard_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Unit '{}' guarding for {} frames", unit_name, frames);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let pos = *obj.get_position();
        let Some(ai_arc) = obj.get_ai_update_interface() else {
            return Ok(ScriptActionResult::Success);
        };
        if let Ok(mut ai) = ai_arc.lock() {
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut guard_params =
                AiCommandParams::new(AiCommandType::GuardPosition, CommandSourceType::FromScript);
            guard_params.pos = pos;
            let _ = ai.execute_command(&guard_params);
        };

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    fn do_unit_idle_for_framecount(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let frames = self.get_int_param(action, 1)?;
        log::debug!("Unit '{}' idling for {} frames", unit_name, frames);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(ai_arc) = obj.get_ai_update_interface() else {
            return Ok(ScriptActionResult::Success);
        };
        if let Ok(mut ai) = ai_arc.lock() {
            let idle_params =
                AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = ai.execute_command(&idle_params);
        };

        if frames > 0 {
            Ok(ScriptActionResult::Pending(frames as f32))
        } else {
            Ok(ScriptActionResult::Success)
        }
    }

    fn do_unit_destroy_all_contained(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        log::debug!("Destroying all units contained in '{}'", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let contain_arc = {
            let Ok(obj) = obj_arc.read() else {
                return Ok(ScriptActionResult::Success);
            };
            obj.get_contain()
        };
        let Some(contain_arc) = contain_arc else {
            return Ok(ScriptActionResult::Success);
        };
        if let Ok(mut contain_guard) = contain_arc.lock() {
            if contain_guard.get_contained_count() > 0 {
                let _ = contain_guard.kill_all_contained();
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_unit_move_towards_nearest_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let trigger_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Unit '{}' moving towards nearest '{}' in trigger '{}'",
            unit_name,
            object_type,
            trigger_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(ai_arc) = obj.get_ai_update_interface() else {
            return Ok(ScriptActionResult::Success);
        };
        let source_pos = *obj.get_position();
        let source_off_map = obj.is_off_map();
        drop(obj);

        let Some(target_id) = self.find_closest_object_of_type_in_trigger(
            object_id,
            &source_pos,
            source_off_map,
            &object_type,
            &trigger_name,
        ) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut ai) = ai_arc.lock() {
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
            let mut params =
                AiCommandParams::new(AiCommandType::MoveToObject, CommandSourceType::FromScript);
            params.obj = Some(target_id);
            let _ = ai.execute_command(&params);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_unit_affect_object_panel_flags(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let flag_name = self.get_string_param(action, 1)?;
        let enable = self.get_int_param(action, 2)? != 0;
        log::debug!(
            "Affecting object panel flag '{}' -> {} for '{}'",
            flag_name,
            enable,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptActionResult::Success);
        };
        if let Ok(mut obj) = obj_arc.write() {
            self.apply_object_panel_flag_for_single_object(&mut obj, &flag_name, enable);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_unit_spawn_named_location_orientation(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let team_name = self.get_string_param(action, 2)?;
        let position = self.get_coord_param(action, 3)?;
        let angle = self.get_real_param(action, 4)?;
        log::debug!(
            "Spawning named '{}' type '{}' on team '{}' at ({}, {}, {}) angle {}",
            unit_name,
            object_type,
            team_name,
            position.x,
            position.y,
            position.z,
            angle
        );

        let unit_name_opt = {
            let trimmed = unit_name.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        };

        if let Some(name) = unit_name_opt {
            let tracker = get_named_object_tracker();
            if let Ok(Some(old_object_id)) = tracker.get_object_id(name) {
                if let Some(old_obj) = TheGameLogic::find_object_by_id(old_object_id) {
                    if old_obj
                        .read()
                        .ok()
                        .is_some_and(|o| !o.is_effectively_dead())
                    {
                        log::warn!(
                            "WARNING - Object with name '{}' already exists. Failed Create.",
                            name
                        );
                        return Ok(ScriptActionResult::Success);
                    }
                }
            }
        }

        let team_arc = match self.get_or_create_team_by_name(&team_name) {
            Ok(team) => team,
            Err(_) => return Ok(ScriptActionResult::Success),
        };

        let object_id = {
            let manager_arc = get_object_manager();
            let Ok(mut manager) = manager_arc.write() else {
                return Ok(ScriptActionResult::Success);
            };
            let spawn_pos = crate::common::Coord3D::new(position.x, position.y, position.z);
            match manager.create_object(
                &object_type,
                spawn_pos,
                Some(team_arc.clone()),
                crate::object_manager::ObjectCreationFlags::from_template(),
            ) {
                Ok(id) => id,
                Err(_) => return Ok(ScriptActionResult::Success),
            }
        };

        if let Ok(mut team) = team_arc.write() {
            team.add_member(object_id);
        }

        if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(mut obj) = obj_arc.write() {
                let _ = obj.set_orientation(angle);
                if let Some(name) = unit_name_opt {
                    obj.set_name(AsciiString::from(name));
                }
            }
        }

        if let Some(name) = unit_name_opt {
            let tracker = get_named_object_tracker();
            if let Ok(Some(old_object_id)) = tracker.get_object_id(name) {
                let _ = tracker.unregister_object(old_object_id);
            }
            let _ = tracker.register_named_object(name.to_string(), object_id);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_create_unnamed_on_team_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_type = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        let waypoint = self.get_string_param(action, 2)?;
        log::debug!(
            "Creating unnamed '{}' on team '{}' at waypoint '{}'",
            object_type,
            team_name,
            waypoint
        );
        let _ = self.create_unit_on_team_at_waypoint(None, &object_type, &team_name, &waypoint)?;
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL PLAYER ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_player_sell_everything(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Player '{}' selling everything", player_name);

        let object_ids = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .and_then(|player| player.read().ok().map(|p| p.get_all_objects()))
            .unwrap_or_default();

        let frame = TheGameLogic::get_frame();
        for object_id in object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let sell_obj = if let Ok(obj_guard) = obj_arc.read() {
                if obj_guard.is_effectively_dead()
                    || !obj_guard.is_kind_of(crate::common::KindOf::Structure)
                {
                    continue;
                }
                game_engine::common::system::build_assistant::Object {
                    id: obj_guard.get_id(),
                    position: game_engine::common::system::build_assistant::Coord3D {
                        x: obj_guard.get_position().x,
                        y: obj_guard.get_position().y,
                        z: obj_guard.get_position().z,
                    },
                    orientation: obj_guard.get_orientation(),
                }
            } else {
                continue;
            };

            let Some(mut assistant) =
                game_engine::common::system::build_assistant::get_build_assistant()
            else {
                break;
            };
            assistant.sell_object(&sell_obj, frame);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_disable_base_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Disabling base construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_base(false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_disable_factories(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let object_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Disabling factories '{}' for '{}'",
            object_name,
            player_name
        );

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_objects_enabled(&object_name, false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_disable_unit_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Disabling unit construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_units(false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_enable_base_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Enabling base construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_base(true);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_enable_factories(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let object_name = self.get_string_param(action, 1)?;
        log::debug!("Enabling factories '{}' for '{}'", object_name, player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_objects_enabled(&object_name, true);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_enable_unit_construction(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Enabling unit construction for '{}'", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_units(true);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_transfer_ownership_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let from_player = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let to_player = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Transferring ownership from '{}' to '{}'",
            from_player,
            to_player
        );

        let (source_player, dest_player) = if let Ok(players) = player_list().read() {
            (
                players.find_player_by_name(&from_player),
                players.find_player_by_name(&to_player),
            )
        } else {
            (None, None)
        };
        let (Some(source_player), Some(dest_player)) = (source_player, dest_player) else {
            return Ok(ScriptActionResult::Success);
        };

        let destination_team = dest_player
            .read()
            .ok()
            .and_then(|player| player.get_default_team());
        let Some(destination_team) = destination_team else {
            return Ok(ScriptActionResult::Success);
        };

        let source_object_ids = source_player
            .read()
            .ok()
            .map(|player| player.get_all_objects())
            .unwrap_or_default();

        let source_money = if let Ok(mut src_guard) = source_player.write() {
            let amount = src_guard.get_money().get_money();
            src_guard.get_money_mut().set_money(0);
            amount
        } else {
            0
        };
        if source_money != 0 {
            if let Ok(mut dst_guard) = dest_player.write() {
                dst_guard.get_money_mut().add_money(source_money);
            }
        }

        for object_id in source_object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            if let Ok(mut obj_guard) = obj_arc.write() {
                let old_owner = obj_guard.get_controlling_player();
                let _ = obj_guard.set_team(Some(destination_team.clone()));
                let new_owner = obj_guard.get_controlling_player();
                obj_guard.on_capture(old_owner, new_owner);
            };
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_relates_player(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player1 = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let player2 = self.resolve_player_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Player '{}' relation to '{}' ({})",
            player1,
            player2,
            relation
        );

        let (source_player, target_player_index) = if let Ok(players) = player_list().read() {
            (
                players.find_player_by_name(&player1),
                players
                    .find_player_by_name(&player2)
                    .and_then(|player| player.read().ok().map(|player| player.get_player_index())),
            )
        } else {
            (None, None)
        };
        if let (Some(source_player), Some(target_player_index)) =
            (source_player, target_player_index)
        {
            if let Ok(mut source_guard) = source_player.write() {
                source_guard.set_player_relationship_by_index(target_player_index, relationship);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_set_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        let relation = self.get_int_param(action, 2)?;
        let relationship = self.relation_from_script_value(relation);
        log::debug!(
            "Player '{}' override relation to team '{}' ({})",
            player_name,
            team_name,
            relation
        );

        let player_arc = if let Ok(players) = player_list().read() {
            players.find_player_by_name(&player_name)
        } else {
            None
        };
        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        if let (Some(player_arc), Some(team_arc)) = (player_arc, team_arc) {
            if let Ok(team_guard) = team_arc.read() {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_team_relationship(&team_guard, relationship);
                }
            };
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_remove_override_relation_to_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        log::debug!(
            "Player '{}' remove override relation to team '{}'",
            player_name,
            team_name
        );

        let player_arc = if let Ok(players) = player_list().read() {
            players.find_player_by_name(&player_name)
        } else {
            None
        };
        let team_arc = if let Ok(mut factory) = get_team_factory().lock() {
            factory.find_team(&team_name)
        } else {
            None
        };
        if let (Some(player_arc), Some(team_arc)) = (player_arc, team_arc) {
            if let Ok(team_guard) = team_arc.read() {
                if let Ok(mut player_guard) = player_arc.write() {
                    let _ = player_guard.remove_team_relationship(&team_guard);
                }
            };
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_garrison_all_buildings(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Player '{}' garrisoning all buildings", player_name);

        let object_ids = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .and_then(|player| player.read().ok().map(|p| p.get_all_objects()))
            .unwrap_or_default();

        for object_id in object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            if let Ok(mut obj_guard) = obj_arc.write() {
                if obj_guard.is_kind_of(crate::common::KindOf::Structure)
                    || !obj_guard.is_kind_of(crate::common::KindOf::Infantry)
                    || obj_guard.is_kind_of(crate::common::KindOf::NoGarrison)
                {
                    continue;
                }
                let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                    continue;
                };
                obj_guard.leave_group();
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params =
                        AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                    let _ = ai_guard.execute_command(&params);
                };
            };
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_exit_all_buildings(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Player '{}' exiting all buildings", player_name);

        let object_ids = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .and_then(|player| player.read().ok().map(|p| p.get_all_objects()))
            .unwrap_or_default();

        for object_id in object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            if let Ok(mut obj_guard) = obj_arc.write() {
                if obj_guard.is_kind_of(crate::common::KindOf::Structure) {
                    continue;
                }
                let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
                    continue;
                };
                obj_guard.leave_group();
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params =
                        AiCommandParams::new(AiCommandType::Exit, CommandSourceType::FromScript);
                    let _ = ai_guard.execute_command(&params);
                };
            };
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_create_team_from_captured_units(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let team_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Player '{}' creating team '{}' from captured units",
            player_name,
            team_name
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_player_add_skillpoints(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let points = self.get_int_param(action, 1)?;
        log::info!("Player '{}' adding {} skill points", player_name, points);

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.add_skill_points(points);
                    log::info!("Player '{}' skill points added", player_name);
                }
            } else {
                log::warn!("Player '{}' not found for add skill points", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_add_ranklevel(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let levels = self.get_int_param(action, 1)?;
        log::info!("Player '{}' adding {} rank levels", player_name, levels);

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    let current_level = player_guard.get_rank_level();
                    player_guard.set_rank_level(current_level + levels);
                    log::info!(
                        "Player '{}' rank level now {}",
                        player_name,
                        current_level + levels
                    );
                }
            } else {
                log::warn!("Player '{}' not found for add rank level", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_set_ranklevel(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let level = self.get_int_param(action, 1)?;
        log::info!("Player '{}' setting rank level to {}", player_name, level);

        let list = player_list();
        if let Ok(list_guard) = list.read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_rank_level(level);
                    log::info!("Player '{}' rank level set to {}", player_name, level);
                }
            } else {
                log::warn!("Player '{}' not found for set rank level", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_set_ranklevellimit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let limit = self.get_int_param(action, 0)?;
        log::debug!("Setting map rank level limit to {}", limit);
        TheGameLogic::set_rank_level_limit(limit);
        Ok(ScriptActionResult::Success)
    }

    fn do_player_purchase_science(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let science_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Player '{}' purchasing science '{}'",
            player_name,
            science_name
        );

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    let _ = player_guard.attempt_to_purchase_science(science_type);
                };
            } else {
                log::warn!("Player '{}' not found for purchase science", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_repair_named_structure(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let structure_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Player '{}' repairing structure '{}'",
            player_name,
            structure_name
        );

        let tracker = get_named_object_tracker();
        let Some(structure_id) = tracker.get_object_id(&structure_name).ok().flatten() else {
            log::warn!("Named structure '{}' not found for repair", structure_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.repair_structure(structure_id);
                };
            } else {
                log::warn!("Player '{}' not found for repair structure", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_affect_receiving_experience(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let modifier = self.get_real_param(action, 1)?;
        log::debug!(
            "Affecting experience receiving for '{}' modifier {}",
            player_name,
            modifier
        );

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_skill_points_modifier(modifier);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_exclude_from_score_screen(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Excluding '{}' from score screen", player_name);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_list_in_score_screen(false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_science_availability(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let science_name = self.get_string_param(action, 1)?;
        let availability_name = self.get_string_param(action, 2)?;
        log::debug!(
            "Setting science '{}' availability '{}' for '{}'",
            science_name,
            availability_name,
            player_name
        );

        let Some(availability_type) =
            crate::player::Player::get_science_availability_type_from_string(&availability_name)
        else {
            log::warn!(
                "Invalid science availability '{}' for '{}'",
                availability_name,
                science_name
            );
            return Ok(ScriptActionResult::Success);
        };

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            log::warn!("Science store not initialized");
            SCIENCE_INVALID
        };

        if science_type == SCIENCE_INVALID {
            log::warn!("Science '{}' not found", science_name);
            return Ok(ScriptActionResult::Success);
        }

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_science_availability(science_type, availability_type);
                };
            } else {
                log::warn!(
                    "Player '{}' not found for science availability",
                    player_name
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_player_select_skillset(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let mut skillset = self.get_int_param(action, 1)?;
        log::debug!("Player '{}' selecting skillset {}", player_name, skillset);

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    // Script uses 1-based skillset numbering; AI uses zero-based.
                    skillset -= 1;
                    player_guard.friend_set_skillset(skillset);
                };
            } else {
                log::warn!("Player '{}' not found for select skillset", player_name);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL CAMERA ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_move_camera_along_waypoint_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_path = self.get_string_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let camera_stutter_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(4).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Moving camera along waypoint path '{}' (sec: {}, stutter: {}, ease_in: {}, ease_out: {})",
            waypoint_path,
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_along_waypoint_path(
                        &waypoint_path,
                        seconds,
                        camera_stutter_seconds,
                        ease_in_seconds,
                        ease_out_seconds,
                    ) {
                        log::warn!(
                            "Script action handler move_camera_along_waypoint_path failed: {}",
                            err
                        );
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_rotate_camera(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // C++: doRotateCamera(rotations, sec, easeIn, easeOut)
        let rotations = self.get_real_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);

        log::debug!(
            "Rotating camera by {} turns (sec: {}, ease_in: {}, ease_out: {})",
            rotations,
            seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.rotate_camera(rotations, seconds, ease_in_seconds, ease_out_seconds)
                    {
                        log::warn!("Script action handler rotate_camera failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_move_camera_to_selection(
        &mut self,
        _action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Retargeting active camera movement to selection");

        // Prefer host-integrated selection center (Main runtime queue) when available.
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_to_selection() {
                        log::warn!(
                            "Script action handler move_camera_to_selection failed: {}",
                            err
                        );
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }

        let local_player_id = player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(-1);
        if local_player_id < 0 {
            return Ok(ScriptActionResult::Success);
        }

        let selection_manager = crate::commands::get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return Ok(ScriptActionResult::Success);
        };

        let Some(selection) = manager.get_player_selection_ref(local_player_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let selected = selection.get_selected_objects_info();
        if selected.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let selected_len = selected.len();
        let mut sum = crate::common::Coord3D::new(0.0, 0.0, 0.0);
        for entry in &selected {
            sum += entry.position;
        }
        let center = sum / (selected_len as f32);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_to_selection() {
                        log::warn!(
                            "Script action handler move_camera_to_selection failed: {}",
                            err
                        );
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }

        log::info!("Script move_camera_to_selection center {:?}", center);
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_move_home(
        &mut self,
        _action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Moving camera home");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_move_home() {
                        log::warn!("Script action handler camera_move_home failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_setup_camera(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let zoom = self.get_real_param(action, 1)?;
        let pitch = self.get_real_param(action, 2)?;
        let look_at_waypoint = self.get_string_param(action, 3)?;

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let look_at_ascii = AsciiString::from(look_at_waypoint.as_str());

        let Some((position, look_at)) = get_terrain_logic().read().ok().and_then(|terrain| {
            let position = terrain.get_waypoint_by_name(&waypoint_ascii)?;
            let look_at = terrain.get_waypoint_by_name(&look_at_ascii)?;
            Some((*position.get_location(), *look_at.get_location()))
        }) else {
            log::warn!(
                "Setup camera waypoint(s) not found: '{}' / '{}'",
                waypoint,
                look_at_waypoint
            );
            return Ok(ScriptActionResult::Success);
        };

        log::debug!(
            "Setting up camera at '{}' (zoom: {}, pitch: {}, look_at: '{}')",
            waypoint,
            zoom,
            pitch,
            look_at_waypoint
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.setup_camera(
                        position.x, position.y, position.z, zoom, pitch, look_at.x, look_at.y,
                        look_at.z,
                    ) {
                        log::warn!("Script action handler setup_camera failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_letterbox_begin(
        &mut self,
        _action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Beginning camera letterbox");
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_begin() {
                        log::warn!(
                            "Script action handler camera_letterbox_begin failed: {}",
                            err
                        );
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_letterbox_end(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Ending camera letterbox");
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_end() {
                        log::warn!("Script action handler camera_letterbox_end failed: {}", err);
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_zoom_camera(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let zoom = self.get_real_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Zooming camera to {} (sec: {}, ease_in: {}, ease_out: {})",
            zoom,
            seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.zoom_camera(zoom, seconds, ease_in_seconds, ease_out_seconds)
                    {
                        log::warn!("Script action handler zoom_camera failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_pitch_camera(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let pitch = self.get_real_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);

        log::debug!(
            "Pitching camera to {} (sec: {}, ease_in: {}, ease_out: {})",
            pitch,
            seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.set_camera_pitch(pitch, seconds, ease_in_seconds, ease_out_seconds)
                    {
                        log::warn!("Script action handler set_camera_pitch failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_oversize_terrain(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let amount = self.get_int_param(action, 0)?;
        log::debug!("Setting terrain oversize to {}", amount);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.oversize_terrain(amount) {
                        log::warn!("Script action handler oversize_terrain failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_fade_add(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.set_fade_parameters(
                    TFade::Add,
                    min_fade,
                    max_fade,
                    frames_increase,
                    frames_hold,
                    frames_decrease,
                );
            }
        }

        log::debug!(
            "Camera fade add from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_fade_subtract(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.set_fade_parameters(
                    TFade::Subtract,
                    min_fade,
                    max_fade,
                    frames_increase,
                    frames_hold,
                    frames_decrease,
                );
            }
        }

        log::debug!(
            "Camera fade subtract from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_fade_saturate(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.set_fade_parameters(
                    TFade::Saturate,
                    min_fade,
                    max_fade,
                    frames_increase,
                    frames_hold,
                    frames_decrease,
                );
            }
        }

        log::debug!(
            "Camera fade saturate from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_fade_multiply(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let min_fade = self.get_real_param(action, 0)?;
        let max_fade = self.get_real_param(action, 1)?;
        let frames_increase = self.get_int_param(action, 2)?;
        let frames_hold = self.get_int_param(action, 3)?;
        let frames_decrease = self.get_int_param(action, 4)?;

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.set_fade_parameters(
                    TFade::Multiply,
                    min_fade,
                    max_fade,
                    frames_increase,
                    frames_hold,
                    frames_decrease,
                );
            }
        }

        log::debug!(
            "Camera fade multiply from {} to {} (increase: {}, hold: {}, decrease: {})",
            min_fade,
            max_fade,
            frames_increase,
            frames_hold,
            frames_decrease
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_bw_mode_begin(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let frames = action.get_parameter(0).map(|p| p.get_int()).unwrap_or(0);
        log::debug!("Beginning camera B&W mode over {} frames", frames);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_camera_bw_mode(true, frames) {
                        log::warn!(
                            "Script action handler set_camera_bw_mode(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_bw_mode_end(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let frames = action.get_parameter(0).map(|p| p.get_int()).unwrap_or(0);
        log::debug!("Ending camera B&W mode over {} frames", frames);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_camera_bw_mode(false, frames) {
                        log::warn!(
                            "Script action handler set_camera_bw_mode(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_draw_skybox_begin(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Beginning skybox draw");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_skybox_enabled(true) {
                        log::warn!(
                            "Script action handler set_skybox_enabled(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_draw_skybox_end(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Ending skybox draw");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_skybox_enabled(false) {
                        log::warn!(
                            "Script action handler set_skybox_enabled(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_motion_blur(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let zoom_in = self.get_bool_param_optional(action, 0).unwrap_or(false);
        let saturate = self.get_bool_param_optional(action, 1).unwrap_or(false);
        log::debug!(
            "Camera motion blur (zoom_in: {}, saturate: {})",
            zoom_in,
            saturate
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_motion_blur(zoom_in, saturate) {
                        log::warn!("Script action handler camera_motion_blur failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_motion_blur_jump(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint_name = self.get_string_param(action, 0)?;
        let saturate = self.get_bool_param_optional(action, 1).unwrap_or(false);
        let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|waypoint| *waypoint.get_location())
        });

        let Some(target) = target else {
            log::warn!(
                "Camera motion blur jump failed: waypoint '{}' not found",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        log::debug!(
            "Camera motion blur jump to '{}' (saturate: {})",
            waypoint_name,
            saturate
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.camera_motion_blur_jump(target.x, target.y, target.z, saturate)
                    {
                        log::warn!(
                            "Script action handler camera_motion_blur_jump failed: {}",
                            err
                        );
                        let _ = handler
                            .move_camera_to(target.x, target.y, target.z, 0.0, 0.0, 0.0, 0.0);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_motion_blur_follow(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let amount = self.get_int_param(action, 0)?;
        log::debug!("Camera motion blur follow amount {}", amount);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_motion_blur_follow(amount) {
                        log::warn!(
                            "Script action handler camera_motion_blur_follow failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_motion_blur_end_follow(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Ending camera motion blur follow");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_motion_blur_end_follow() {
                        log::warn!(
                            "Script action handler camera_motion_blur_end_follow failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_set_audible_distance(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let distance = self.get_real_param(action, 0)?;
        log::debug!("Setting camera audible distance to {}", distance);
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_tether_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let unit_name = self.get_string_param(action, 0)?;
        let snap_to_unit = self.get_bool_param_optional(action, 1).unwrap_or(false);
        let play = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);

        log::debug!(
            "Camera tethering to '{}' (snap: {}, play: {})",
            unit_name,
            snap_to_unit,
            play
        );

        let tracker = get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&unit_name).ok().flatten();
        if object_id.is_none() {
            let lower = unit_name.to_ascii_lowercase();
            object_id = OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find_map(|obj_ref| {
                    obj_ref.read().ok().and_then(|obj| {
                        if obj.get_name().to_ascii_lowercase() == lower {
                            Some(obj.get_id())
                        } else {
                            None
                        }
                    })
                });
        }

        let Some(object_id) = object_id else {
            log::warn!("Camera tether failed: unit '{}' not found", unit_name);
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_tether_object(object_id, snap_to_unit, play) {
                        log::warn!("Script action handler camera_tether_object failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_stop_tether_named(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Stopping camera tether");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.stop_camera_follow() {
                        log::warn!("Script action handler stop_camera_follow failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_set_default(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let pitch = self.get_real_param(action, 0)?;
        let angle = self.get_real_param(action, 1)?;
        let max_height = self.get_real_param(action, 2)?;

        log::debug!(
            "Setting camera default (pitch: {}, angle: {}, max_height: {})",
            pitch,
            angle,
            max_height
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_set_default(pitch, angle, max_height) {
                        log::warn!("Script action handler camera_set_default failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_camera_look_toward_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let hold_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(4).map(|p| p.get_real()).unwrap_or(0.0);

        let tracker = get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&object_name).ok().flatten();
        if object_id.is_none() {
            let lower = object_name.to_ascii_lowercase();
            object_id = OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find_map(|obj_ref| {
                    obj_ref.read().ok().and_then(|obj| {
                        if obj.get_name().to_ascii_lowercase() == lower {
                            Some(obj.get_id())
                        } else {
                            None
                        }
                    })
                });
        }

        log::debug!(
            "Camera looking toward '{}' (sec: {}, hold: {}, ease_in: {}, ease_out: {})",
            object_name,
            seconds,
            hold_seconds,
            ease_in_seconds,
            ease_out_seconds
        );

        if let Some(object_id) = object_id {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.camera_look_toward_object(
                            object_id,
                            seconds,
                            hold_seconds,
                            ease_in_seconds,
                            ease_out_seconds,
                        ) {
                            log::warn!(
                                "Script action handler camera_look_toward_object failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Camera look toward object '{}' not found", object_name);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_look_toward_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let seconds = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_in_seconds = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out_seconds = action.get_parameter(3).map(|p| p.get_real()).unwrap_or(0.0);
        let reverse_rotation = self.get_bool_param_optional(action, 4).unwrap_or(false);

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });

        log::debug!(
            "Camera looking toward waypoint '{}' (sec: {}, ease_in: {}, ease_out: {}, reverse: {})",
            waypoint,
            seconds,
            ease_in_seconds,
            ease_out_seconds,
            reverse_rotation
        );

        if let Some(target) = target {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.camera_look_toward_waypoint(
                            target.x,
                            target.y,
                            target.z,
                            seconds,
                            ease_in_seconds,
                            ease_out_seconds,
                            reverse_rotation,
                        ) {
                            log::warn!(
                                "Script action handler camera_look_toward_waypoint failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Camera look toward waypoint '{}' not found", waypoint);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_freeze_time(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Camera mod freeze time");
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_mod_freeze_time() {
                        log::warn!(
                            "Script action handler camera_mod_freeze_time failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_set_final_zoom(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let zoom = self.get_real_param(action, 0)?;
        let ease_in = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Camera mod set final zoom to {} (ease_in: {}, ease_out: {})",
            zoom,
            ease_in,
            ease_out
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_mod_set_final_zoom(zoom, ease_in, ease_out) {
                        log::warn!(
                            "Script action handler camera_mod_set_final_zoom failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_set_final_pitch(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let pitch = self.get_real_param(action, 0)?;
        let ease_in = action.get_parameter(1).map(|p| p.get_real()).unwrap_or(0.0);
        let ease_out = action.get_parameter(2).map(|p| p.get_real()).unwrap_or(0.0);
        log::debug!(
            "Camera mod set final pitch to {} (ease_in: {}, ease_out: {})",
            pitch,
            ease_in,
            ease_out
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_mod_set_final_pitch(pitch, ease_in, ease_out) {
                        log::warn!(
                            "Script action handler camera_mod_set_final_pitch failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_freeze_angle(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Camera mod freeze angle");
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_mod_freeze_angle() {
                        log::warn!(
                            "Script action handler camera_mod_freeze_angle failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_set_final_speed_multiplier(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let multiplier = self.get_int_param(action, 0)?;
        log::debug!("Camera mod set final speed multiplier to {}", multiplier);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_mod_set_final_speed_multiplier(multiplier) {
                        log::warn!(
                            "Script action handler camera_mod_set_final_speed_multiplier failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_set_rolling_average(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let frames = self.get_int_param(action, 0)?;
        log::debug!("Camera mod set rolling average to {} frames", frames);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_mod_set_rolling_average(frames) {
                        log::warn!(
                            "Script action handler camera_mod_set_rolling_average failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_final_look_toward(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });
        log::debug!("Camera mod final look toward '{}'", waypoint);

        if let Some(target) = target {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) =
                            handler.camera_mod_final_look_toward(target.x, target.y, target.z)
                        {
                            log::warn!(
                                "Script action handler camera_mod_final_look_toward failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!(
                "Camera mod final look toward waypoint '{}' not found",
                waypoint
            );
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_mod_look_toward(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });
        log::debug!("Camera mod look toward '{}'", waypoint);

        if let Some(target) = target {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) =
                            handler.camera_mod_look_toward(target.x, target.y, target.z)
                        {
                            log::warn!(
                                "Script action handler camera_mod_look_toward failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Camera mod look toward waypoint '{}' not found", waypoint);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_enable_slave_mode(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let thing_template_name = self.get_string_param(action, 0)?;
        let bone_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Enabling camera slave mode (template: '{}', bone: '{}')",
            thing_template_name,
            bone_name
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.camera_enable_slave_mode(&thing_template_name, &bone_name)
                    {
                        log::warn!(
                            "Script action handler camera_enable_slave_mode failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_disable_slave_mode(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling camera slave mode");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_disable_slave_mode() {
                        log::warn!(
                            "Script action handler camera_disable_slave_mode failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_camera_add_shaker_at(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let amplitude = self.get_real_param(action, 1)?;
        let duration_seconds = self.get_real_param(action, 2)?;
        let radius = self.get_real_param(action, 3)?;

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|way| *way.get_location())
        });

        log::debug!(
            "Adding camera shaker at '{}' (amplitude: {}, duration: {}, radius: {})",
            waypoint,
            amplitude,
            duration_seconds,
            radius
        );

        if let Some(target) = target {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.camera_add_shaker_at(
                            target.x,
                            target.y,
                            target.z,
                            amplitude,
                            duration_seconds,
                            radius,
                        ) {
                            log::warn!(
                                "Script action handler camera_add_shaker_at failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Camera shaker waypoint '{}' not found", waypoint);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_screen_shake(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let intensity = self.get_int_param(action, 0)?;
        log::debug!("Screen shake intensity {}", intensity);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.screen_shake(intensity) {
                        log::warn!("Script action handler screen_shake failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL AUDIO/VIDEO ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_sound_play_named(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_name = self.get_string_param(action, 0)?;
        let unit_name = self.get_string_param(action, 1)?;
        log::debug!("Playing named sound '{}' from '{}'", sound_name, unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptActionResult::Success);
        };

        let mut event = crate::common::audio::AudioEventRts::new(sound_name.as_str());
        event.set_object_id(object_id);
        if let Some(audio) = TheAudio::get() {
            let _ = audio.add_audio_event(&event);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_suspend_background_sounds(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Suspending background sounds");
        if let Some(audio) = TheAudio::get() {
            audio.pause_audio(EngineAudioAffect::Sound);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_resume_background_sounds(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Resuming background sounds");
        if let Some(audio) = TheAudio::get() {
            audio.resume_audio(EngineAudioAffect::Sound);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_ambient_pause(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Pausing ambient sound");
        if let Some(audio) = TheAudio::get() {
            audio.pause_audio(EngineAudioAffect::Sound3D);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_ambient_resume(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Resuming ambient sound");
        if let Some(audio) = TheAudio::get() {
            audio.resume_audio(EngineAudioAffect::Sound3D);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_music_set_volume(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let volume = self.get_real_param(action, 0)?;
        log::debug!("Setting music volume to {}", volume);
        if let Some(audio) = TheAudio::get() {
            audio.set_volume((volume / 100.0).clamp(0.0, 1.0), EngineAudioAffect::Music);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_disable_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_type = self.get_string_param(action, 0)?;
        log::debug!("Disabling sound type '{}'", sound_type);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_enabled(&sound_type, false);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_enable_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_type = self.get_string_param(action, 0)?;
        log::debug!("Enabling sound type '{}'", sound_type);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_enabled(&sound_type, true);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_enable_all(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling all sounds");
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_enabled("", true);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_audio_override_volume_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let audio_type = self.get_string_param(action, 0)?;
        let volume = self.get_real_param(action, 1)?;
        log::debug!("Overriding volume for '{}' to {}", audio_type, volume);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_volume_override(&audio_type, volume / 100.0);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_audio_restore_volume_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let audio_type = self.get_string_param(action, 0)?;
        log::debug!("Restoring volume for '{}'", audio_type);
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_volume_override(&audio_type, -1.0);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_audio_restore_volume_all_type(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Restoring all audio volumes");
        if let Some(audio) = TheAudio::get() {
            audio.set_audio_event_volume_override("", -1.0);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_set_volume(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let volume = self.get_real_param(action, 0)?;
        log::debug!("Setting sound volume to {}", volume);
        let normalized = (volume / 100.0).clamp(0.0, 1.0);
        if let Some(audio) = TheAudio::get() {
            audio.set_volume(normalized, EngineAudioAffect::Sound);
            audio.set_volume(normalized, EngineAudioAffect::Sound3D);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_speech_set_volume(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let volume = self.get_real_param(action, 0)?;
        log::debug!("Setting speech volume to {}", volume);
        if let Some(audio) = TheAudio::get() {
            audio.set_volume((volume / 100.0).clamp(0.0, 1.0), EngineAudioAffect::Speech);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_remove_all_disabled(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Removing all disabled sounds");
        if let Some(audio) = TheAudio::get() {
            audio.remove_disabled_events();
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_sound_remove_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let sound_type = self.get_string_param(action, 0)?;
        log::debug!("Removing sound type '{}'", sound_type);
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event_by_name(&sound_type);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_enable_object_sound(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        log::debug!("Enabling sounds for '{}'", object_name);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    if let Some(drawable) = obj_guard.get_drawable() {
                        if let Ok(mut draw_guard) = drawable.write() {
                            draw_guard.enable_ambient_sound_from_script(true);
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_disable_object_sound(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        log::debug!("Disabling sounds for '{}'", object_name);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    if let Some(drawable) = obj_guard.get_drawable() {
                        if let Ok(mut draw_guard) = drawable.write() {
                            draw_guard.enable_ambient_sound_from_script(false);
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_movie_play_fullscreen(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let movie_name = self.get_string_param(action, 0)?;
        log::info!("Playing fullscreen movie '{}'", movie_name);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.movie_play_fullscreen(&movie_name) {
                        log::warn!(
                            "Script action handler movie_play_fullscreen failed: {}",
                            err
                        );
                    }
                    return Ok(ScriptActionResult::Success);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_movie_play_radar(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let movie_name = self.get_string_param(action, 0)?;
        log::debug!("Playing radar movie '{}'", movie_name);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.movie_play_radar(&movie_name) {
                        log::warn!("Script action handler movie_play_radar failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL RADAR/MAP ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_radar_create_event(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let position = self.get_coord_param(action, 0)?;
        let event_type = self.get_int_param(action, 1)?;
        log::debug!(
            "Creating radar event at ({}, {}, {}) type {}",
            position.x,
            position.y,
            position.z,
            event_type
        );
        let radar_event = Self::radar_event_type_from_int(event_type);
        if let Ok(mut radar) = get_radar_system().write() {
            let radar_pos = to_radar_coord(&position);
            radar.create_event(&radar_pos, radar_event, 4.0);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_radar_force_enable(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Force enabling radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.force_on(true);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_radar_revert_to_normal(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Reverting radar to normal");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.force_on(false);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_map_reveal_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Revealing all map for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr.reveal_map_for_player(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr.reveal_map_for_player(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_map_reveal_all_perm(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Permanently revealing all map for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr
                        .reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr
                        .reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_map_reveal_all_undo_perm(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Undoing permanent map reveal for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr
                        .undo_reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr
                        .undo_reveal_map_for_player_permanently(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_map_shroud_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Shrouding all map for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mut shroud_mgr = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| {
                ScriptError::ExecutionFailed("Failed to lock ShroudManager".to_string())
            })?;

        if !player_name.is_empty() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    let _ = shroud_mgr.shroud_map_for_player(player.get_player_index() as u32);
                }
                return Ok(ScriptActionResult::Success);
            }
        }

        for player_arc in players.iter() {
            if let Ok(player) = player_arc.read() {
                if player.get_player_type() == PlayerType::Human {
                    let _ = shroud_mgr.shroud_map_for_player(player.get_player_index() as u32);
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_map_reveal_permanently_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let waypoint = self.get_string_param(action, 0)?;
        let radius = self.get_real_param(action, 1)?;
        let player_name = self.get_string_param(action, 2)?;
        let reveal_name = self.get_string_param(action, 3)?;

        log::debug!(
            "Permanently revealing map '{}' at waypoint '{}' radius {} for '{}'",
            reveal_name,
            waypoint,
            radius,
            player_name
        );

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut engine) = *engine_guard {
                engine.create_named_map_reveal(&reveal_name, &waypoint, radius, &player_name);
                engine.do_named_map_reveal(&reveal_name);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_map_undo_reveal_permanently_at_waypoint(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let reveal_name = self.get_string_param(action, 0)?;
        log::debug!("Undoing permanent reveal '{}'", reveal_name);

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut engine) = *engine_guard {
                engine.undo_named_map_reveal(&reveal_name);
                engine.remove_named_map_reveal(&reveal_name);
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_map_switch_border(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let border_index = self.get_int_param(action, 0)?;
        log::debug!("Switching map border to '{}'", border_index);

        let mut observer_player_index: Option<u32> = None;
        if let Ok(players) = player_list().read() {
            if let Some(observer) = players.find_player_by_name("ReplayObserver") {
                if let Ok(observer_guard) = observer.read() {
                    observer_player_index = Some(observer_guard.get_player_index() as u32);
                }
            }
        }

        if let Some(observer_index) = observer_player_index {
            if let Ok(mut shroud_mgr) = crate::system::shroud_manager::get_shroud_manager().lock() {
                let _ = shroud_mgr.undo_reveal_map_for_player_permanently(observer_index);
            }
        }

        if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
            terrain.set_active_boundary(border_index);
        }

        if let Some(observer_index) = observer_player_index {
            if let Ok(mut shroud_mgr) = crate::system::shroud_manager::get_shroud_manager().lock() {
                let _ = shroud_mgr.reveal_map_for_player_permanently(observer_index);
            }
        }

        if let Ok(mut shroud_mgr) = crate::system::shroud_manager::get_shroud_manager().lock() {
            shroud_mgr.refresh_shroud_for_local_player();
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_refresh_radar(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Refreshing radar");
        if let Ok(mut radar) = get_radar_system().write() {
            radar.refresh_terrain();
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_object_create_radar_event(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_name = self.get_string_param(action, 0)?;
        let event_type = self.get_int_param(action, 1)?;
        log::debug!(
            "Creating radar event for object '{}' (type {})",
            object_name,
            event_type
        );

        let tracker = get_named_object_tracker();
        let object_id_opt = tracker.get_object_id(&object_name).ok().flatten();
        if let Some(object_id) = object_id_opt {
            if let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(object_guard) = object_arc.read() {
                    let pos = *object_guard.get_position();
                    let radar_event = Self::radar_event_type_from_int(event_type);
                    if let Ok(mut radar) = get_radar_system().write() {
                        let radar_pos = to_radar_coord(&pos);
                        radar.create_event(&radar_pos, radar_event, 4.0);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_disable_border_shroud(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling border shroud");
        if let Some(global) = crate::helpers::TheGlobalData::get() {
            let level = global.get_clear_alpha();
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref engine) = *engine_guard {
                    if let Some(handler) = engine.action_handler() {
                        let _ = handler.set_border_shroud_level(level);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_enable_border_shroud(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling border shroud");
        if let Some(global) = crate::helpers::TheGlobalData::get() {
            let level = global.get_shroud_alpha();
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref engine) = *engine_guard {
                    if let Some(handler) = engine.action_handler() {
                        let _ = handler.set_border_shroud_level(level);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_resize_view_guardband(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let gbx = self.get_real_param(action, 0)?;
        let gby = self.get_real_param(action, 1)?;
        log::debug!("Resizing view guardband to ({}, {})", gbx, gby);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.resize_view_guardband(gbx, gby) {
                        log::warn!(
                            "Script action handler resize_view_guardband failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ADDITIONAL DISPLAY/UI ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_cameo_flash(&mut self, action: &ScriptAction) -> Result<ScriptActionResult, ScriptError> {
        let cameo_name = self.get_string_param(action, 0)?;
        let time_in_seconds = self.get_int_param(action, 1)?;
        log::debug!("Flashing cameo '{}' for {}s", cameo_name, time_in_seconds);

        let frames = LOGICFRAMES_PER_SECOND as i32 * time_in_seconds;
        let drawable_frames_per_flash = (LOGICFRAMES_PER_SECOND as i32 / 2).max(1);
        let mut count = frames / drawable_frames_per_flash;
        // C++: ensure the cameo ends in its original visual state.
        if (count % 2) == 1 {
            count += 1;
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.cameo_flash(&cameo_name, count) {
                        log::warn!("Script action handler cameo_flash failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_display_countdown_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        let timer_text = self.get_string_param(action, 1)?;
        log::debug!(
            "Displaying countdown timer '{}' text '{}'",
            timer_name,
            timer_text
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.add_named_timer(&timer_name, &timer_text, true) {
                        log::warn!("Script action handler add_named_timer failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_hide_countdown_timer(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let timer_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding countdown timer '{}'", timer_name);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.remove_named_timer(&timer_name) {
                        log::warn!("Script action handler remove_named_timer failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_enable_countdown_timer_display(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling countdown timer display");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.show_named_timer_display(true) {
                        log::warn!(
                            "Script action handler show_named_timer_display(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_disable_countdown_timer_display(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling countdown timer display");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.show_named_timer_display(false) {
                        log::warn!(
                            "Script action handler show_named_timer_display(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_display_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        let counter_text = self.get_string_param(action, 1)?;
        log::debug!(
            "Displaying counter '{}' text '{}'",
            counter_name,
            counter_text
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.add_named_timer(&counter_name, &counter_text, false) {
                        log::warn!("Script action handler add_named_timer failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_hide_counter(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let counter_name = self.get_string_param(action, 0)?;
        log::debug!("Hiding counter '{}'", counter_name);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.remove_named_timer(&counter_name) {
                        log::warn!("Script action handler remove_named_timer failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_disable_special_power_display(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling special power display");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_superweapon_display_enabled_by_script(false) {
                        log::warn!(
                            "Script action handler set_superweapon_display_enabled_by_script(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_enable_special_power_display(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling special power display");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_superweapon_display_enabled_by_script(true) {
                        log::warn!(
                            "Script action handler set_superweapon_display_enabled_by_script(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_ingame_popup_message(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let message = self.get_string_param(action, 0)?;
        let x_percent = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(50);
        let y_percent = action.get_parameter(2).map(|p| p.get_int()).unwrap_or(50);
        let width = action.get_parameter(3).map(|p| p.get_int()).unwrap_or(400);
        let pause = action
            .get_parameter(4)
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        log::info!(
            "In-game popup: '{}' at ({}, {}) width {} pause {}",
            message,
            x_percent,
            y_percent,
            width,
            pause
        );

        // C++: TheInGameUI->popupMessage(message, x, y, width, pause, FALSE)
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.popup_message(&message, x_percent, y_percent, width, pause, false)
                    {
                        log::warn!("Script action handler popup_message failed: {}", err);
                        let _ = handler.display_text(&message);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_object_force_select(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let center_in_view = action
            .get_parameter(2)
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        let audio_to_play = action
            .get_parameter(3)
            .map(|p| p.get_string().to_string())
            .unwrap_or_default();

        log::debug!(
            "Force selecting object type '{}' on team '{}' (center_in_view: {}, audio: '{}')",
            object_type,
            team_name,
            center_in_view,
            audio_to_play
        );

        let team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name));
        let Some(team_arc) = team_arc else {
            return Ok(ScriptActionResult::Success);
        };

        let member_ids = if let Ok(team_guard) = team_arc.read() {
            team_guard.get_members().to_vec()
        } else {
            Vec::new()
        };

        let mut best_guess: Option<ObjectID> = None;
        for member_id in member_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_template_name() != object_type {
                continue;
            }
            if obj_guard.get_drawable().is_none() {
                continue;
            }
            if best_guess.is_none() || member_id < best_guess.unwrap_or(member_id) {
                best_guess = Some(member_id);
            }
        }

        let Some(selected_id) = best_guess else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(selected_obj) = TheGameLogic::find_object_by_id(selected_id) else {
            return Ok(ScriptActionResult::Success);
        };

        let local_player_mask = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_mask()))
            .unwrap_or(crate::common::PLAYERMASK_ALL);

        let mut selected_pos = Coord3D::ZERO;
        if let Ok(selected_guard) = selected_obj.read() {
            selected_pos = *selected_guard.get_position();
            let _ = TheGameLogic::select_object(&*selected_guard, true, local_player_mask, true);
        }

        if !audio_to_play.is_empty() {
            let mut audio_event = crate::common::audio::AudioEventRts::new(audio_to_play.as_str());
            if let Some(local_player) = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
            {
                if let Ok(local_guard) = local_player.read() {
                    audio_event.set_player_index(local_guard.get_player_index() as u32);
                }
            }
            if let Some(audio) = TheAudio::get() {
                let _ = audio.add_audio_event(&audio_event);
            }
        }

        if center_in_view {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        let _ = handler.move_camera_to(
                            selected_pos.x,
                            selected_pos.y,
                            selected_pos.z,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                        );
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // TIME CONTROL ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_freeze_time(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Freezing time");
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.do_freeze_time();
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.freeze_time() {
                        log::warn!("Script action handler freeze_time failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_unfreeze_time(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Unfreezing time");
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.do_unfreeze_time();
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.unfreeze_time() {
                        log::warn!("Script action handler unfreeze_time failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_set_visual_speed_multiplier(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let multiplier = self.get_int_param(action, 0)?;
        log::debug!("Setting visual speed multiplier to {}", multiplier);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_visual_speed_multiplier(multiplier) {
                        log::warn!(
                            "Script action handler set_visual_speed_multiplier failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_set_fps_limit(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let fps = self.get_int_param(action, 0)?;
        log::debug!("Setting FPS limit to {}", fps);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_fps_limit(fps) {
                        log::warn!("Script action handler set_fps_limit failed: {}", err);
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ENVIRONMENT/WORLD ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_set_tree_sway(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let direction = self.get_real_param(action, 0)?;
        let intensity = self.get_real_param(action, 1)?;
        let lean = self.get_real_param(action, 2)?;
        let breeze_period = self.get_int_param(action, 3)?;
        let randomness = self.get_real_param(action, 4)?;
        log::debug!(
            "Setting tree sway direction {} intensity {} lean {} period {} randomness {}",
            direction,
            intensity,
            lean,
            breeze_period,
            randomness
        );

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(ref mut script_engine) = *engine_guard {
                script_engine.set_breeze_info(
                    direction,
                    intensity,
                    lean,
                    breeze_period,
                    randomness,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_water_change_height(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let water_name = self.get_string_param(action, 0)?;
        let height = self.get_real_param(action, 1)?;
        log::debug!("Changing water '{}' height to {}", water_name, height);

        let water_name_ascii = AsciiString::from(water_name.as_str());
        if let Ok(mut terrain) = get_terrain_logic().write() {
            if terrain
                .get_water_handle_by_name(&water_name_ascii)
                .is_some()
            {
                terrain.set_water_height(&water_name_ascii, height, 999_999.9, true);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_water_change_height_over_time(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let water_name = self.get_string_param(action, 0)?;
        let height = self.get_real_param(action, 1)?;
        let time = self.get_real_param(action, 2)?;
        let damage = self.get_real_param(action, 3)?;
        log::debug!(
            "Changing water '{}' height to {} over {} seconds (damage {})",
            water_name,
            height,
            time,
            damage
        );

        let water_name_ascii = AsciiString::from(water_name.as_str());
        if let Ok(mut terrain) = get_terrain_logic().write() {
            terrain.change_water_height_over_time(&water_name_ascii, height, time, damage);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_set_cave_index(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let cave_name = self.get_string_param(action, 0)?;
        let cave_index = self.get_int_param(action, 1)?;
        log::debug!("Setting cave '{}' index to {}", cave_name, cave_index);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&cave_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    if let Some(contain) = obj_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            contain_guard.try_to_set_cave_index(cave_index);
                        }
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_show_weather(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let show_weather = self.get_bool_param_optional(action, 0).unwrap_or(true);
        log::debug!("Setting weather visibility to {}", show_weather);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_weather_visible(show_weather) {
                        log::warn!("Script action handler set_weather_visible failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_set_infantry_lighting_override(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let setting = self.get_real_param(action, 0)?;
        if setting != -1.0 && setting <= 0.0 {
            log::warn!(
                "Invalid infantry lighting override {}; expected -1.0 or > 0.0",
                setting
            );
        }
        if let Ok(mut gd) = global_data::write_safe() {
            gd.script_override_infantry_light_scale = setting;
        }
        log::debug!("Setting infantry lighting override to {}", setting);
        Ok(ScriptActionResult::Success)
    }

    fn do_reset_infantry_lighting_override(&mut self) -> Result<ScriptActionResult, ScriptError> {
        if let Ok(mut gd) = global_data::write_safe() {
            gd.script_override_infantry_light_scale = -1.0;
        }
        log::debug!("Resetting infantry lighting override to -1.0");
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // CONSTRUCTION/TECHTREE ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_set_base_construction_speed(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let delay_seconds = self.get_int_param(action, 1)?;
        log::debug!(
            "Setting base construction speed for '{}' to {} seconds",
            player_name,
            delay_seconds
        );

        if let Ok(list_guard) = player_list().read() {
            if let Some(player_arc) = list_guard.find_player_by_name(&player_name) {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_team_delay_seconds(delay_seconds);
                };
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_techtree_modify_buildability_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let object_type = self.get_string_param(action, 0)?;
        let buildable_status = self.get_int_param(action, 1)?;
        let Some(template) = TheObjectFactory::find_template(&object_type) else {
            log::warn!(
                "Techtree buildability change ignored; template '{}' not found",
                object_type
            );
            return Ok(ScriptActionResult::Success);
        };

        log::debug!(
            "Modifying buildability for '{}' to status {}",
            object_type,
            buildable_status
        );
        TheGameLogic::set_buildable_status_override(template.get_name().as_str(), buildable_status);
        Ok(ScriptActionResult::Success)
    }

    fn do_warehouse_set_value(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let warehouse_name = self.get_string_param(action, 0)?;
        let value = self.get_int_param(action, 1)?;
        log::debug!("Setting warehouse '{}' value to {}", warehouse_name, value);

        let tracker = get_named_object_tracker();
        let Some(warehouse_id) = tracker.get_object_id(&warehouse_name).ok().flatten() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(warehouse_arc) = TheGameLogic::find_object_by_id(warehouse_id) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(warehouse_guard) = warehouse_arc.read() {
            let Some(module) = warehouse_guard.find_update_module("SupplyWarehouseDockUpdate")
            else {
                return Ok(ScriptActionResult::Success);
            };

            module.with_module_downcast::<crate::object::production::SupplyWarehouseDockUpdateModule, _, _>(|module| {
                module.behavior_mut().set_cash_value(value);
            });
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_command_bar_remove_button_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let button_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;

        let Some(template) = TheObjectFactory::find_template(&object_type) else {
            return Ok(ScriptActionResult::Success);
        };
        let command_set_name = template.get_command_set_string().as_str().to_string();

        let slot = get_control_bar_bridge().and_then(|control_bar| {
            control_bar
                .find_command_set_by_name(command_set_name.as_str())
                .and_then(|set| {
                    set.buttons.iter().position(|button| {
                        button
                            .as_ref()
                            .map(|b| b.name.eq_ignore_ascii_case(&button_name))
                            .unwrap_or(false)
                    })
                })
        });

        if let Some(slot) = slot {
            let _ = set_command_set_slot_override(command_set_name.as_str(), slot, None);
            crate::control_bar::mark_ui_dirty();
        }

        log::debug!(
            "Removing command bar button '{}' for '{}'",
            button_name,
            object_type
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_command_bar_add_button_object_type_slot(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let button_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let slot_num = self.get_int_param(action, 2)?;

        let Some(template) = TheObjectFactory::find_template(&object_type) else {
            return Ok(ScriptActionResult::Success);
        };
        let command_set_name = template.get_command_set_string().as_str().to_string();

        let slot = slot_num - 1;
        if !(0..crate::command_button::MAX_COMMANDS_PER_SET as i32).contains(&slot) {
            return Ok(ScriptActionResult::Success);
        }

        let _ = set_command_set_slot_override(
            command_set_name.as_str(),
            slot as usize,
            Some(button_name.as_str()),
        );
        crate::control_bar::mark_ui_dirty();

        log::debug!(
            "Adding command bar button '{}' for '{}' at slot {}",
            button_name,
            object_type,
            slot_num
        );
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // ATTACK PRIORITY ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_set_attack_priority_thing(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let priority_set = self.get_string_param(action, 0)?;
        let type_or_list = self.get_string_param(action, 1)?;
        let priority = self.get_int_param(action, 2)?;
        if let Ok(mut engine_lock) = get_script_engine().write() {
            if let Some(engine) = engine_lock.as_mut() {
                let _ = engine.set_priority_thing(&priority_set, &type_or_list, priority);
            }
        }
        log::debug!(
            "Setting attack priority '{}' on '{}' to {}",
            priority_set,
            type_or_list,
            priority
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_set_attack_priority_kindof(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let priority_set = self.get_string_param(action, 0)?;
        let kind_name = self.get_string_param(action, 1)?;
        let priority = self.get_int_param(action, 2)?;
        if let Some(kind) = parse_kind_of(&kind_name) {
            if let Ok(mut engine_lock) = get_script_engine().write() {
                if let Some(engine) = engine_lock.as_mut() {
                    let _ = engine.set_priority_kind(&priority_set, kind, priority);
                }
            }
        }
        log::debug!(
            "Setting attack priority '{}' for kindof '{}' to {}",
            priority_set,
            kind_name,
            priority
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_set_default_attack_priority(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let priority_set = self.get_string_param(action, 0)?;
        let priority = self.get_int_param(action, 1)?;
        if let Ok(mut engine_lock) = get_script_engine().write() {
            if let Some(engine) = engine_lock.as_mut() {
                let _ = engine.set_priority_default(&priority_set, priority);
            }
        }
        log::debug!(
            "Setting default attack priority '{}' to {}",
            priority_set,
            priority
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_set_stopping_distance(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let distance = self.get_real_param(action, 1)?;
        log::debug!(
            "Setting team '{}' stopping distance to {}",
            team_name,
            distance
        );

        if distance < 0.5 {
            return Ok(ScriptActionResult::Success);
        }

        let Some(team_arc) = self.get_team_by_name(&team_name).ok() else {
            return Ok(ScriptActionResult::Success);
        };
        let members = team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();

        for member_id in members {
            let Some(member_obj) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let ai_arc = member_obj
                .read()
                .ok()
                .and_then(|obj| obj.get_ai_update_interface());
            let Some(ai_arc) = ai_arc else {
                return Ok(ScriptActionResult::Success);
            };
            let Ok(ai_guard) = ai_arc.lock() else {
                return Ok(ScriptActionResult::Success);
            };
            let Some(loco_arc) = ai_guard.get_cur_locomotor() else {
                return Ok(ScriptActionResult::Success);
            };
            let Ok(mut loco_guard) = loco_arc.lock() else {
                return Ok(ScriptActionResult::Success);
            };
            loco_guard.set_close_enough_dist(distance);
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // OBJECT LIST ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_object_list_add_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let list_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        if let Ok(mut engine_lock) = get_script_engine().write() {
            if let Some(engine) = engine_lock.as_mut() {
                let list_key = list_name.to_string();
                let mut list = engine
                    .get_object_types(&list_key)
                    .unwrap_or_else(|| ObjectTypes::with_list_name(AsciiString::from(&list_key)));
                list.add_object_type(AsciiString::from(object_type.as_str()));
                engine.set_object_types(list_key, list);
            }
        }
        log::debug!("Adding '{}' to object list '{}'", object_type, list_name);
        Ok(ScriptActionResult::Success)
    }

    fn do_object_list_remove_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let list_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        if let Ok(mut engine_lock) = get_script_engine().write() {
            if let Some(engine) = engine_lock.as_mut() {
                if let Some(mut list) = engine.get_object_types(&list_name) {
                    list.remove_object_type(&AsciiString::from(object_type.as_str()));
                    engine.set_object_types(list_name.to_string(), list);
                }
            }
        }
        log::debug!(
            "Removing '{}' from object list '{}'",
            object_type,
            list_name
        );
        Ok(ScriptActionResult::Success)
    }

    fn do_object_allow_bonuses(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let allow = self.get_int_param(action, 0)? != 0;
        if let Ok(mut engine_lock) = get_script_engine().write() {
            if let Some(engine) = engine_lock.as_mut() {
                engine.set_objects_should_receive_difficulty_bonus(allow);
            }
        }
        log::debug!("Object allow bonuses: {}", allow);
        Ok(ScriptActionResult::Success)
    }

    fn do_delete_all_unmanned(
        &mut self,
        _action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let mut to_destroy = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            if let Ok(guard) = obj.read() {
                if guard.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned) {
                    to_destroy.push(guard.get_id());
                }
            }
        }
        if !to_destroy.is_empty() {
            if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
                for obj_id in to_destroy {
                    logic.destroy_object(obj_id);
                }
            }
        }
        log::debug!("Deleting all unmanned");
        Ok(ScriptActionResult::Success)
    }

    fn do_choose_victim_always_uses_normal(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let use_normal = self.get_int_param(action, 0)? != 0;
        if let Ok(mut engine_lock) = get_script_engine().write() {
            if let Some(engine) = engine_lock.as_mut() {
                engine.set_choose_victim_always_uses_normal(use_normal);
            }
        }
        log::debug!("Choose victim always uses normal: {}", use_normal);
        Ok(ScriptActionResult::Success)
    }

    fn do_scripting_override_hulk_lifetime(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let lifetime = self.get_int_param(action, 0)?;
        log::debug!("Scripting override hulk lifetime to {}", lifetime);
        TheGameLogic::set_hulk_max_lifetime_override(lifetime);
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // AI/SKIRMISH ACTION IMPLEMENTATIONS
    // ============================================================================

    fn with_current_player_ai<F>(&mut self, f: F)
    where
        F: FnOnce(&mut crate::ai::integration::IntegratedAiPlayer),
    {
        let current_player = get_script_engine().read().ok().and_then(|g| {
            g.as_ref()
                .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
        });
        let Some(player_name) = current_player else {
            log::warn!("Skirmish action: current player not available");
            return;
        };

        let Ok(list) = player_list().read() else {
            return;
        };
        let Some(player_arc) = list.find_player_by_name(&player_name) else {
            log::warn!("Skirmish action: player '{}' not found", player_name);
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };

        let player_id = player_guard.get_player_index() as u32;
        let _difficulty = player_guard.get_player_difficulty();

        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                f(ai_player);
            })
        });
    }

    fn with_named_player_ai<F>(&mut self, player_name: &str, f: F)
    where
        F: FnOnce(&mut crate::ai::integration::IntegratedAiPlayer),
    {
        let Ok(list) = player_list().read() else {
            return;
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            log::warn!("Skirmish action: player '{}' not found", player_name);
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };

        let player_id = player_guard.get_player_index() as u32;
        let _difficulty = player_guard.get_player_difficulty();

        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| {
                f(ai_player);
            })
        });
    }

    fn get_skirmish_enemy_player(&self) -> Option<Arc<RwLock<crate::player::Player>>> {
        let current_player_name = get_script_engine().read().ok().and_then(|g| {
            g.as_ref()
                .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
        })?;

        let list = player_list().read().ok()?;
        let current_player = list.find_player_by_name(&current_player_name)?;
        let current_guard = current_player.read().ok()?;

        if let Some(enemy_index) = current_guard.get_current_enemy_player_index() {
            if let Some(enemy_arc) = list.get_player(enemy_index).cloned() {
                let is_non_neutral = enemy_arc
                    .read()
                    .ok()
                    .map(|enemy_guard| enemy_guard.get_player_type() != PlayerType::Neutral)
                    .unwrap_or(false);
                if is_non_neutral {
                    return Some(enemy_arc);
                }
            }
        }

        for player_arc in list.iter() {
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };
            if player_guard.get_player_type() == PlayerType::Human {
                return Some(player_arc.clone());
            }
        }

        None
    }

    fn compute_team_center_and_first(
        &self,
        team_arc: &Arc<RwLock<crate::team::Team>>,
    ) -> Option<(Coord3D, Arc<RwLock<crate::object::Object>>)> {
        let team_guard = team_arc.read().ok()?;
        let members = team_guard.get_members();
        let mut sum = Coord3D::new(0.0, 0.0, 0.0);
        let mut count = 0.0;
        let mut first_unit: Option<Arc<RwLock<crate::object::Object>>> = None;

        for &member_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let pos = obj_guard.get_position();
            sum.x += pos.x;
            sum.y += pos.y;
            sum.z += pos.z;
            count += 1.0;
            if first_unit.is_none() {
                first_unit = Some(obj_arc.clone());
            }
        }

        let Some(first_unit) = first_unit else {
            return None;
        };
        if count == 0.0 {
            return None;
        }

        let center = Coord3D::new(sum.x / count, sum.y / count, sum.z / count);
        Some((center, first_unit))
    }

    fn resolve_follow_waypoint_id(
        &self,
        waypoint_name_or_path: &str,
        reference_pos: Coord3D,
    ) -> Option<WaypointID> {
        get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_closest_waypoint_on_path(&reference_pos, waypoint_name_or_path)
                .map(|waypoint| waypoint.get_id())
        })
    }

    fn check_bridges_for_waypoint(
        &self,
        player_id: u32,
        unit: &Arc<RwLock<crate::object::Object>>,
        target: Coord3D,
    ) {
        let unit_pos = {
            let Ok(unit_guard) = unit.read() else {
                return;
            };
            *unit_guard.get_position()
        };
        let delta = Coord3D::new(
            target.x - unit_pos.x,
            target.y - unit_pos.y,
            target.z - unit_pos.z,
        );
        let dist_sq = delta.x * delta.x + delta.y * delta.y;
        if dist_sq < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
            return;
        }

        let Ok(terrain_guard) = get_terrain_logic().read() else {
            return;
        };
        let mut bridge_opt = terrain_guard.get_first_bridge();
        while let Some(bridge) = bridge_opt {
            let bridge_id = bridge.get_bridge_info().bridge_object_id;
            if bridge_id == INVALID_ID {
                bridge_opt = bridge.get_next();
                continue;
            }

            let broken = match TheGameLogic::find_object_by_id(bridge_id) {
                Some(obj) => obj
                    .read()
                    .ok()
                    .map(|guard| guard.is_destroyed())
                    .unwrap_or(true),
                None => true,
            };
            if !broken {
                bridge_opt = bridge.get_next();
                continue;
            }

            let dist = dist_sq.sqrt().max(PATHFIND_CELL_SIZE_F);
            let steps = (dist / PATHFIND_CELL_SIZE_F).ceil() as i32;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let sample = Coord3D::new(
                    unit_pos.x + delta.x * t,
                    unit_pos.y + delta.y * t,
                    unit_pos.z + delta.z * t,
                );
                if bridge.is_point_on_bridge(&sample) {
                    let _ = with_ai_integration_mut(|manager| {
                        manager.with_ai_player_mut(player_id, |ai_player| {
                            let _ = ai_player.repair_structure(bridge_id);
                        })
                    });
                    return;
                }
            }

            bridge_opt = bridge.get_next();
        }
    }

    fn do_skirmish_build_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let building_type = self.get_string_param(action, 0)?;
        log::debug!("Skirmish building '{}'", building_type);
        let building = building_type.clone();
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_specific_building(&building);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_follow_approach_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        let as_team = self.get_int_param(action, 2)? != 0;
        log::debug!(
            "Skirmish team '{}' following approach path '{}' as_team={}",
            team_name,
            waypoint_path_label,
            as_team
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let Some((center, first_unit)) = self.compute_team_center_and_first(&team_arc) else {
            return Ok(ScriptActionResult::Success);
        };

        let enemy_player = self.get_skirmish_enemy_player();
        let Some(enemy_player) = enemy_player else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(enemy_guard) = enemy_player.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mp_index = enemy_guard.get_mp_start_index() + 1;

        let path_label = format!("{}{}", waypoint_path_label, mp_index);
        let (waypoint_id, waypoint_pos) =
            match get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&center, &path_label)
                    .map(|way| (way.get_id(), *way.get_location()))
            }) {
                Some(result) => result,
                None => return Ok(ScriptActionResult::Success),
            };

        let current_player_name = get_script_engine().read().ok().and_then(|g| {
            g.as_ref()
                .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
        });
        if let Some(current_player_name) = current_player_name {
            if let Ok(list) = player_list().read() {
                if let Some(player_arc) = list.find_player_by_name(&current_player_name) {
                    if let Ok(player_guard) = player_arc.read() {
                        let player_id = player_guard.get_player_index() as u32;
                        self.check_bridges_for_waypoint(player_id, &first_unit, waypoint_pos);
                    }
                }
            }
        }

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let command_type = if as_team {
                AiCommandType::FollowWaypointPathAsTeam
            } else {
                AiCommandType::FollowWaypointPath
            };
            let mut params = AiCommandParams::new(command_type, CommandSourceType::FromScript);
            params.waypoint = Some(waypoint_id);
            let _ = group.ai_do_command(&params);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_move_to_approach_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        log::debug!(
            "Skirmish team '{}' moving to approach path '{}'",
            team_name,
            waypoint_path_label
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let Some((center, _first_unit)) = self.compute_team_center_and_first(&team_arc) else {
            return Ok(ScriptActionResult::Success);
        };

        let enemy_player = self.get_skirmish_enemy_player();
        let Some(enemy_player) = enemy_player else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(enemy_guard) = enemy_player.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mp_index = enemy_guard.get_mp_start_index() + 1;

        let path_label = format!("{}{}", waypoint_path_label, mp_index);
        let waypoint_pos = match get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_closest_waypoint_on_path(&center, &path_label)
                .map(|way| *way.get_location())
        }) {
            Some(pos) => pos,
            None => return Ok(ScriptActionResult::Success),
        };

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(group) = group_arc.read() {
            group.group_move_to_position(&waypoint_pos, false, CommandSourceType::FromScript);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_build_base_defense_front(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        if action.num_parms > 0 {
            let _ = self.get_string_param(action, 0);
        }
        log::debug!("Skirmish building base defense front");
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense(false);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_build_base_defense_flank(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        if action.num_parms > 0 {
            let _ = self.get_string_param(action, 0);
        }
        log::debug!("Skirmish building base defense flank");
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense(true);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_build_structure_front(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let structure_type = self.get_string_param(action, 0)?;
        log::debug!("Skirmish building structure front '{}'", structure_type);
        let structure = structure_type.clone();
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense_structure(&structure, false);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_build_structure_flank(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let structure_type = self.get_string_param(action, 0)?;
        log::debug!("Skirmish building structure flank '{}'", structure_type);
        let structure = structure_type.clone();
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense_structure(&structure, true);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_fire_special_power_at_most_cost(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let power_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Skirmish player '{}' firing special power '{}' at most cost",
            player_name,
            power_name
        );

        let Some(enemy_player) = self.get_skirmish_enemy_player() else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(enemy_guard) = enemy_player.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let enemy_player_index = enemy_guard.get_player_index();

        let (power_template, template_name, radius) = {
            let Some(store) = get_special_power_store() else {
                return Ok(ScriptActionResult::Success);
            };
            let Some(template) = store.find_special_power_template(&power_name) else {
                return Ok(ScriptActionResult::Success);
            };

            (
                template.clone(),
                template.get_name().to_string(),
                template.get_radius_cursor_radius().max(50.0),
            )
        };

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
        else {
            log::warn!("Skirmish action: player '{}' not found", player_name);
            return Ok(ScriptActionResult::Success);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let player_id = player_guard.get_player_index() as u32;

        let mut target_location: Option<Coord3D> = None;
        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                IntegratedAiPlayer::Skirmish(skirmish_ai) => {
                    let mut location = Coord3D::ZERO;
                    if skirmish_ai.compute_superweapon_target(
                        &power_template,
                        &mut location,
                        enemy_player_index,
                        radius,
                    ) {
                        target_location = Some(location);
                    }
                }
                IntegratedAiPlayer::Standard(standard_ai) => {
                    if let Ok(Some(location)) =
                        standard_ai.compute_superweapon_target(power_template.get_name(), radius)
                    {
                        target_location = Some(location);
                    }
                }
            })
        });

        let Some(target_location) = target_location else {
            return Ok(ScriptActionResult::Success);
        };
        if target_location.x == 0.0 && target_location.y == 0.0 && target_location.z == 0.0 {
            return Ok(ScriptActionResult::Success);
        }

        let mut fired = false;
        for object_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.is_destroyed() {
                continue;
            }
            let Some(owner_id) = object_guard.get_controlling_player_id() else {
                continue;
            };
            if owner_id as u32 != player_id {
                continue;
            }

            let is_ready = object_guard
                .with_special_power_module_interface_by_name(&template_name, |sp_module| {
                    sp_module.is_ready()
                })
                .unwrap_or(false);
            if !is_ready {
                continue;
            }

            let fired_here =
                object_guard.with_special_power_module_mut_by_name(&template_name, |sp_module| {
                    sp_module.do_special_power_at_location(
                        &target_location,
                        INVALID_ANGLE,
                        SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT,
                    );
                    true
                });
            if fired_here.unwrap_or(false) {
                fired = true;
                break;
            }
        }

        if !fired {
            log::debug!(
                "Skirmish special power '{}' not fired: no ready module found for '{}'",
                power_name,
                player_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_attack_nearest_group_with_value(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let comparison = self.get_int_param(action, 1)?;
        let value = self.get_int_param(action, 2)?;
        log::debug!(
            "Skirmish team '{}' attacking nearest group with comparison {} value {}",
            team_name,
            comparison,
            value
        );

        let group_arc = self.create_ai_group_from_team(&team_name)?;

        let team_arc = self.get_team_by_name(&team_name)?;
        let controlling_player_id = team_arc
            .read()
            .ok()
            .and_then(|team| team.get_controlling_player_id())
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Skirmish team has no controlling player".to_string())
            })?;

        let player_list_guard = player_list()
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to lock player list".to_string()))?;
        let controlling_player = player_list_guard
            .get_player(controlling_player_id as i32)
            .cloned()
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Skirmish team player not found".to_string())
            })?;
        let controlling_player_guard = controlling_player.read().map_err(|_| {
            ScriptError::ExecutionFailed("Failed to read skirmish player".to_string())
        })?;

        let group_center = group_arc
            .read()
            .ok()
            .and_then(|group| group.get_center())
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Failed to get group center".to_string())
            })?;

        let comparison_type = match comparison {
            0 => ComparisonType::LessThan,
            1 => ComparisonType::LessEqual,
            2 => ComparisonType::Equal,
            3 => ComparisonType::GreaterEqual,
            4 => ComparisonType::Greater,
            5 => ComparisonType::NotEqual,
            _ => ComparisonType::Equal,
        };

        let mut target_loc = group_center;
        {
            if let Ok(manager) = get_object_manager().read() {
                let mut best_dist = f32::MAX;
                let mut best_pos = None;

                for obj_id in manager.all_object_ids() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_destroyed() {
                        continue;
                    }
                    if obj_guard
                        .get_status_bits()
                        .test(crate::common::ObjectStatusTypes::UnderConstruction)
                    {
                        continue;
                    }
                    let Some(obj_player_id) = obj_guard.get_controlling_player_id() else {
                        continue;
                    };
                    if obj_player_id == controlling_player_id {
                        continue;
                    }

                    let Some(target_player_arc) =
                        player_list_guard.get_player(obj_player_id as i32).cloned()
                    else {
                        continue;
                    };
                    let Ok(target_player_guard) = target_player_arc.read() else {
                        continue;
                    };
                    if controlling_player_guard.get_relationship(&target_player_guard)
                        != Relationship::Enemies
                    {
                        continue;
                    }

                    let build_cost = obj_guard.get_build_cost();
                    let meets_value = match comparison_type {
                        ComparisonType::LessThan => build_cost < value,
                        ComparisonType::LessEqual => build_cost <= value,
                        ComparisonType::Equal => build_cost == value,
                        ComparisonType::GreaterEqual => build_cost >= value,
                        ComparisonType::Greater => build_cost > value,
                        ComparisonType::NotEqual => build_cost != value,
                    };
                    if !meets_value {
                        continue;
                    }

                    let pos = obj_guard.get_position();
                    let dx = pos.x - group_center.x;
                    let dy = pos.y - group_center.y;
                    let dist = dx * dx + dy * dy;
                    if dist < best_dist {
                        best_dist = dist;
                        best_pos = Some(*pos);
                    }
                }

                if let Some(pos) = best_pos {
                    target_loc = pos;
                }
            }
        }

        if let Ok(group) = group_arc.read() {
            group.group_attack_move_to_position(&target_loc, CommandSourceType::FromScript);
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_perform_command_button_on_most_valuable_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let ability = self.get_string_param(action, 1)?;
        let range = self.get_real_param(action, 2)?;
        let _all_team_members = self.get_bool_param_optional(action, 3).unwrap_or(false);

        log::debug!(
            "Skirmish team '{}' performing command '{}' on most valuable object (range {})",
            team_name,
            ability,
            range
        );

        let group_arc = self.create_ai_group_from_team(&team_name)?;

        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(&ability) else {
            return Ok(ScriptActionResult::Success);
        };

        let source_obj = if let Some(template) = command_button.get_special_power_template() {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_special_power_source_object(template.get_id()))
        } else {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_command_button_source_object(command_button.get_id()))
        };

        let Some(source_obj) = source_obj else {
            return Ok(ScriptActionResult::Success);
        };

        let source_guard = match source_obj.read() {
            Ok(guard) => guard,
            Err(_) => return Ok(ScriptActionResult::Success),
        };

        let group_center = group_arc
            .read()
            .ok()
            .and_then(|group| group.get_center())
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Failed to get group center".to_string())
            })?;

        let target_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&group_center, range))
            .unwrap_or_default();

        let options =
            SpecialPowerCommandOption::from_bits_truncate(command_button.get_options_bits());
        let requires_object_target = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );

        let mut best_target: Option<Arc<RwLock<crate::object::Object>>> = None;
        let mut best_cost = i32::MIN;

        for obj_id in target_ids {
            let Some(target_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(target_guard) = target_arc.read() else {
                continue;
            };
            if target_guard.is_destroyed() {
                continue;
            }
            if target_guard
                .get_status_bits()
                .test(crate::common::ObjectStatusTypes::UnderConstruction)
            {
                continue;
            }
            if target_guard.is_off_map() != source_guard.is_off_map() {
                continue;
            }

            let relationship = source_guard.relationship_to(&target_guard);
            let relationship_ok = if requires_object_target {
                (options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
                    && relationship == Relationship::Enemies)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT)
                        && relationship == Relationship::Neutral)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
                        && matches!(relationship, Relationship::Allies))
                    || (!options.intersects(
                        SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
                    ) && relationship == Relationship::Enemies)
            } else {
                relationship == Relationship::Enemies
            };
            if !relationship_ok {
                continue;
            }

            if options.contains(SpecialPowerCommandOption::NEED_TARGET_PRISONER)
                && !target_guard.is_captured()
            {
                continue;
            }

            let cost = target_guard.get_build_cost();
            if cost > best_cost {
                best_cost = cost;
                best_target = Some(target_arc.clone());
            }
        }

        if let Some(target_arc) = best_target {
            if let Ok(target_guard) = target_arc.read() {
                let _ = source_guard.do_command_button_at_object(
                    command_button.get_id(),
                    &target_guard,
                    CommandSourceType::FromScript,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_skirmish_wait_for_command_button_available_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 1)?;
        let command_button = self.get_string_param(action, 2)?;
        log::debug!(
            "Skirmish waiting for command '{}' available (all) on team '{}'",
            command_button,
            team_name
        );

        let ready =
            self.eval_skirmish_command_button_ready_by_name(&team_name, &command_button, true)?;
        if ready {
            Ok(ScriptActionResult::Success)
        } else {
            Ok(ScriptActionResult::Pending(1.0))
        }
    }

    fn do_skirmish_wait_for_command_button_available_partial(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 1)?;
        let command_button = self.get_string_param(action, 2)?;
        log::debug!(
            "Skirmish waiting for command '{}' available (partial) on team '{}'",
            command_button,
            team_name
        );

        let ready =
            self.eval_skirmish_command_button_ready_by_name(&team_name, &command_button, false)?;
        if ready {
            Ok(ScriptActionResult::Success)
        } else {
            Ok(ScriptActionResult::Pending(1.0))
        }
    }

    fn do_ai_player_build_supply_center(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let building_type = self.get_string_param(action, 1)?;
        let cash = self.get_int_param(action, 2)?;
        log::debug!(
            "AI player '{}' building supply center '{}' with cash {}",
            player_name,
            building_type,
            cash
        );
        let building = building_type.clone();
        self.with_named_player_ai(&player_name, |ai_player| {
            let _ = ai_player.build_by_supplies(cash, &building);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_ai_player_build_upgrade(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let upgrade_name = self.get_string_param(action, 1)?;
        log::debug!(
            "AI player '{}' building upgrade '{}'",
            player_name,
            upgrade_name
        );
        let upgrade = upgrade_name.clone();
        self.with_named_player_ai(&player_name, |ai_player| {
            let _ = ai_player.build_upgrade(&upgrade);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_ai_player_build_type_nearest_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let build_type = self.get_string_param(action, 1)?;
        let team_name = self.get_string_param(action, 2)?;
        log::debug!(
            "AI player '{}' building '{}' nearest team '{}'",
            player_name,
            build_type,
            team_name
        );

        let team_factory = get_team_factory();
        let team_arc = team_factory
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name));
        let Some(team_arc) = team_arc else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(location) = team_guard.get_estimate_team_position() else {
            return Ok(ScriptActionResult::Success);
        };

        let building = build_type.clone();
        self.with_named_player_ai(&player_name, |ai_player| {
            let _ = ai_player.build_specific_building_near_location(&building, location);
        });
        Ok(ScriptActionResult::Success)
    }

    fn do_idle_all_units(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Idling all units for '{}'", player_name);
        if let Ok(list) = player_list().read() {
            if !player_name.is_empty() {
                if let Some(player_arc) = list.find_player_by_name(&player_name) {
                    if let Ok(mut player_guard) = player_arc.write() {
                        player_guard
                            .set_units_should_idle_or_resume(true, CommandSourceType::FromScript);
                    }
                }
            } else {
                for player_arc in list.iter() {
                    if let Ok(mut player_guard) = player_arc.write() {
                        if player_guard.get_player_type() == PlayerType::Human {
                            player_guard.set_units_should_idle_or_resume(
                                true,
                                CommandSourceType::FromScript,
                            );
                        }
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_resume_supply_trucking(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Resuming supply trucking for '{}'", player_name);
        if let Ok(list) = player_list().read() {
            if !player_name.is_empty() {
                if let Some(player_arc) = list.find_player_by_name(&player_name) {
                    if let Ok(mut player_guard) = player_arc.write() {
                        player_guard
                            .set_units_should_idle_or_resume(false, CommandSourceType::FromScript);
                    }
                }
            } else {
                for player_arc in list.iter() {
                    if let Ok(mut player_guard) = player_arc.write() {
                        if player_guard.get_player_type() == PlayerType::Human {
                            player_guard.set_units_should_idle_or_resume(
                                false,
                                CommandSourceType::FromScript,
                            );
                        }
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // EVA/MISC ACTION IMPLEMENTATIONS
    // ============================================================================

    fn do_eva_set_enabled_disabled(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let enabled = self.get_int_param(action, 0)? != 0;
        log::debug!("EVA enabled: {}", enabled);
        if let Err(err) = crate::helpers::TheEva::set_enabled(enabled) {
            log::warn!("Failed to update EVA enabled state: {}", err);
        }
        Ok(ScriptActionResult::Success)
    }

    fn do_options_set_occlusion_mode(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let mode = self.get_int_param(action, 0)?;
        TheGameLogic::set_show_behind_building_markers(mode != 0);
        log::debug!("Setting occlusion mode to {}", mode);
        Ok(ScriptActionResult::Success)
    }

    fn do_options_set_draw_icon_ui_mode(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let mode = self.get_int_param(action, 0)?;
        TheGameLogic::set_draw_icon_ui(mode != 0);
        log::debug!("Setting draw icon UI mode to {}", mode);
        Ok(ScriptActionResult::Success)
    }

    fn do_options_set_particle_cap_mode(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let mode = self.get_int_param(action, 0)?;
        TheGameLogic::set_show_dynamic_lod(mode != 0);
        log::debug!("Setting particle cap mode to {}", mode);
        Ok(ScriptActionResult::Success)
    }

    fn do_exit_specific_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let building_name = self.get_string_param(action, 0)?;
        log::debug!("Exiting specific building '{}'", building_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(building_id)) = tracker.get_object_id(&building_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(building_obj) = TheGameLogic::find_object_by_id(building_id) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut building_guard) = building_obj.write() {
            if !building_guard.is_kind_of(crate::common::KindOf::Structure) {
                return Ok(ScriptActionResult::Success);
            }

            if let Some(ai_arc) = building_guard.get_ai_update_interface() {
                let _ = building_guard.leave_group();
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params = AiCommandParams::new(
                        AiCommandType::Evacuate,
                        CommandSourceType::FromScript,
                    );
                    let _ = ai_guard.execute_command(&params);
                }
                return Ok(ScriptActionResult::Success);
            }

            if let Some(contain) = building_guard.get_contain() {
                if let Ok(mut contain_guard) = contain.lock() {
                    let _ = contain_guard.remove_all_contained(false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    fn do_enable_scoring(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling scoring");
        TheGameLogic::set_scoring_enabled(true);
        Ok(ScriptActionResult::Success)
    }

    fn do_disable_scoring(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling scoring");
        TheGameLogic::set_scoring_enabled(false);
        Ok(ScriptActionResult::Success)
    }

    fn do_set_train_held(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let loco_name = self.get_string_param(action, 0)?;
        let held = self.get_int_param(action, 1)? != 0;
        log::debug!("Setting train '{}' held: {}", loco_name, held);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&loco_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    if let Some(module) = obj_guard.find_update_module("RailroadBehavior") {
                        let _ = module.with_module_downcast::<crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModule, _, _>(|module| {
                            module.behavior_mut().set_held(held);
                        });
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // HELPER METHODS FOR PARAMETER EXTRACTION
    // ============================================================================

    fn get_string_param(&self, action: &ScriptAction, index: usize) -> Result<String, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_string().to_string())
    }

    fn get_int_param(&self, action: &ScriptAction, index: usize) -> Result<i32, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_int())
    }

    fn get_real_param(&self, action: &ScriptAction, index: usize) -> Result<f32, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_real())
    }

    fn get_coord_param(&self, action: &ScriptAction, index: usize) -> Result<Coord3D, ScriptError> {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| {
                let c = p.get_coord();
                Coord3D::new(c.x, c.y, c.z)
            })
    }

    fn get_bool_param_optional(&self, action: &ScriptAction, index: usize) -> Option<bool> {
        action.get_parameter(index).map(|p| p.get_int() != 0)
    }

    fn radar_event_type_from_int(event_type: i32) -> RadarEventType {
        match event_type {
            1 => RadarEventType::Construction,
            2 => RadarEventType::Upgrade,
            3 => RadarEventType::UnderAttack,
            4 => RadarEventType::Information,
            5 => RadarEventType::BeaconPulse,
            6 => RadarEventType::Infiltration,
            7 => RadarEventType::BattlePlan,
            8 => RadarEventType::StealthDiscovered,
            9 => RadarEventType::StealthNeutralized,
            10 => RadarEventType::Fake,
            _ => RadarEventType::Invalid,
        }
    }

    /// C++ parity helper for ScriptActions::changeObjectPanelFlagForSingleObject.
    fn apply_object_panel_flag_for_single_object(
        &self,
        obj: &mut crate::object::Object,
        flag_to_change: &str,
        new_val: bool,
    ) {
        let normalized = flag_to_change
            .chars()
            .filter(|c| !c.is_ascii_whitespace() && *c != '_')
            .collect::<String>()
            .to_ascii_lowercase();

        match normalized.as_str() {
            "enabled" => {
                obj.set_script_status(
                    crate::object::ObjectScriptStatusBit::ScriptDisabled,
                    !new_val,
                );
            }
            "powered" => {
                obj.set_script_status(
                    crate::object::ObjectScriptStatusBit::ScriptUnderpowered,
                    !new_val,
                );
            }
            "indestructible" => {
                if let Some(body) = obj.get_body_module() {
                    if let Ok(mut body_guard) = body.lock() {
                        let _ = body_guard.set_indestructible(new_val);
                    }
                }
            }
            "unsellable" => {
                obj.set_script_status(crate::object::ObjectScriptStatusBit::Unsellable, new_val);
            }
            "selectable" => {
                if obj.is_selectable() != new_val {
                    obj.set_selectable(new_val);
                }
            }
            "airecruitable" => {
                if let Some(ai) = obj.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_is_recruitable(new_val);
                    }
                }
            }
            "playertargetable" => {
                obj.set_script_status(
                    crate::object::ObjectScriptStatusBit::ScriptTargetable,
                    new_val,
                );
            }
            _ => {
                log::warn!("Unknown object panel flag '{}'", flag_to_change);
            }
        }
    }

    fn resolve_object_types_for_action(&self, type_or_list_name: &str) -> ObjectTypes {
        let mut types = ObjectTypes::new();
        if type_or_list_name.is_empty() {
            return types;
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(found) = engine.get_object_types(type_or_list_name) {
                    return found;
                }
            }
        }

        types.add_object_type(AsciiString::from(type_or_list_name));
        types
    }

    fn find_closest_object_of_type_in_trigger(
        &self,
        source_object_id: ObjectID,
        source_pos: &Coord3D,
        source_off_map: bool,
        type_or_list_name: &str,
        trigger_name: &str,
    ) -> Option<ObjectID> {
        let trigger = self.get_trigger_area(trigger_name).ok()?;
        let wanted_types = self.resolve_object_types_for_action(type_or_list_name);
        let max_search_radius = 1_000_000.0;

        let partition = ThePartitionManager::get()?;
        partition.get_closest_object_2d(source_pos, max_search_radius, |candidate| {
            if candidate.get_id() == source_object_id {
                return false;
            }
            if candidate.is_effectively_dead() {
                return false;
            }
            if candidate.is_off_map() != source_off_map {
                return false;
            }

            let pos = candidate.get_position();
            let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
            if !trigger.point_in_trigger_int(&point) {
                return false;
            }

            let template_ref: &dyn crate::common::ThingTemplate = candidate.get_template().as_ref();
            wanted_types.contains_template(Some(template_ref))
        })
    }

    // ============================================================================
    // TEAM AND OBJECT LOOKUP HELPERS
    // C++ Reference: TheScriptEngine->getTeamNamed(), TheAI->createGroup()
    // ============================================================================

    /// Get team by name from TeamFactory
    /// C++ Reference: TheScriptEngine->getTeamNamed(teamName)
    fn get_team_by_name(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<crate::team::Team>>, ScriptError> {
        let team_name = self.resolve_team_name_token(team_name);
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            factory_guard
                .find_team(&team_name)
                .ok_or_else(|| ScriptError::TeamNotFound(team_name.to_string()))
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock team factory".to_string(),
            ))
        }
    }

    /// Get a team by name, creating it if missing (matches ScriptActions::createUnitOnTeamAt).
    fn get_or_create_team_by_name(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<crate::team::Team>>, ScriptError> {
        let team_name = self.resolve_team_name_token(team_name);
        let factory = get_team_factory();
        let Ok(mut factory_guard) = factory.lock() else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to lock team factory".to_string(),
            ));
        };

        if let Some(team) = factory_guard.find_team(&team_name) {
            return Ok(team);
        }

        factory_guard.create_team(&team_name).ok_or_else(|| {
            ScriptError::ExecutionFailed(format!("Failed to create team '{}'", team_name))
        })
    }

    /// Create a unit on a team at a waypoint (C++: ScriptActions::createUnitOnTeamAt).
    ///
    /// Returns `Ok(Some(object_id))` when a unit is created, or `Ok(None)` when the action
    /// intentionally does nothing (e.g. unit already exists and is alive).
    fn create_unit_on_team_at_waypoint(
        &mut self,
        unit_name: Option<&str>,
        object_type: &str,
        team_name: &str,
        waypoint_name: &str,
    ) -> Result<Option<crate::common::ObjectID>, ScriptError> {
        let unit_name = unit_name.and_then(|name| {
            let trimmed = name.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        });

        let tracker = get_named_object_tracker();
        if let Some(unit_name) = unit_name {
            if let Ok(Some(old_object_id)) = tracker.get_object_id(unit_name) {
                if let Some(old_obj) = TheGameLogic::find_object_by_id(old_object_id) {
                    if old_obj
                        .read()
                        .ok()
                        .is_some_and(|o| !o.is_effectively_dead())
                    {
                        log::warn!(
                            "WARNING - Object with name '{}' already exists. Failed Create.",
                            unit_name
                        );
                        return Ok(None);
                    }
                }
            }
        }

        let team_arc = match self.get_or_create_team_by_name(team_name) {
            Ok(team) => team,
            Err(err) => {
                log::warn!("CREATE_UNIT: team '{}' unavailable: {}", team_name, err);
                return Ok(None);
            }
        };

        let waypoint_pos = {
            let waypoint_ascii = AsciiString::from(waypoint_name);
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
        };

        let position = if let Some(pos) = waypoint_pos {
            pos
        } else {
            log::warn!("CREATE_UNIT: waypoint '{}' not found", waypoint_name);
            crate::common::Coord3D::new(0.0, 0.0, 0.0)
        };

        let object_id = {
            let manager_arc = get_object_manager();
            let Ok(mut manager) = manager_arc.write() else {
                log::warn!("CREATE_UNIT: failed to lock ObjectManager");
                return Ok(None);
            };

            match manager.create_object(
                object_type,
                position,
                Some(team_arc.clone()),
                crate::object_manager::ObjectCreationFlags::from_template(),
            ) {
                Ok(id) => id,
                Err(err) => {
                    log::warn!(
                        "CREATE_UNIT: failed to create '{}' for team '{}': {}",
                        object_type,
                        team_name,
                        err
                    );
                    return Ok(None);
                }
            }
        };

        if let Ok(mut team) = team_arc.write() {
            team.add_member(object_id);
        }

        if let Some(unit_name) = unit_name {
            if let Ok(Some(old_object_id)) = tracker.get_object_id(unit_name) {
                let _ = tracker.unregister_object(old_object_id);
            }

            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut obj) = obj_arc.write() {
                    obj.set_name(AsciiString::from(unit_name));
                }
            }

            if let Err(err) = tracker.register_named_object(unit_name.to_string(), object_id) {
                log::warn!(
                    "CREATE_UNIT: failed to register named object '{}' -> {}: {}",
                    unit_name,
                    object_id,
                    err
                );
            }
        }

        Ok(Some(object_id))
    }

    /// Create an AI group and populate it with team members
    /// C++ Reference: TheAI->createGroup(); theTeam->getTeamAsAIGroup(theGroup);
    fn create_ai_group_from_team(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<AiGroup>>, ScriptError> {
        // Get team
        let team_arc = self.get_team_by_name(team_name)?;
        let members = if let Ok(team) = team_arc.read() {
            team.get_members().to_vec()
        } else {
            return Err(ScriptError::ExecutionFailed(
                "Failed to read team".to_string(),
            ));
        };

        // C++ script actions use short-lived groups; avoid contending on global AI write lock.
        let group_id = SCRIPT_TEMP_GROUP_ID.fetch_add(1, Ordering::Relaxed);
        let group = Arc::new(RwLock::new(AiGroup::new(group_id)));

        if let Ok(mut group_guard) = group.write() {
            for member_id in members {
                if TheGameLogic::find_object_by_id(member_id).is_some() {
                    group_guard.add(member_id);
                }
            }
        }

        Ok(group)
    }

    /// Issue a command to all members of a team through their AI interfaces
    /// C++ Reference: Matches pattern in doTeamGuard where we iterate team members
    #[allow(dead_code)] // C++ parity: script engine helper, will be wired to script actions
    fn issue_command_to_team_members(
        &self,
        team_name: &str,
        _command: AiCommandType,
        params: &AiCommandParams,
    ) -> Result<(), ScriptError> {
        let team_name = self.resolve_team_name_token(team_name);
        log::debug!(
            "issue_command_to_team_members called for team '{}'",
            team_name
        );

        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let object_manager = get_object_manager();
                    if let Ok(obj_manager) = object_manager.read() {
                        for obj_id in team.get_members() {
                            if let Some(obj) = obj_manager.get_object(*obj_id) {
                                if let Ok(obj_read) = obj.read() {
                                    if let Some(ai) = obj_read.get_ai_update_interface() {
                                        if let Ok(mut ai_write) = ai.lock() {
                                            let _ = ai_write.execute_command(params);
                                        }
                                    }
                                }
                            }
                        }
                    };
                }
            } else {
                return Err(ScriptError::TeamNotFound(team_name.to_string()));
            }
        }

        Ok(())
    }

    /// Get object by ID from ObjectManager
    #[allow(dead_code)] // C++ parity: script engine helper, will be wired to script actions
    fn get_object_by_id(
        &self,
        object_id: u32,
    ) -> Result<Arc<RwLock<crate::object_manager::GameObjectInstance>>, ScriptError> {
        let obj_mgr = get_object_manager();
        let obj_mgr_guard = obj_mgr.read().map_err(|_| {
            ScriptError::ExecutionFailed("Failed to lock object manager".to_string())
        })?;
        obj_mgr_guard
            .get_object(object_id)
            .ok_or_else(|| ScriptError::ObjectNotFound(format!("Object {} not found", object_id)))
    }

    /// Get waypoint position from terrain logic
    #[allow(dead_code)] // C++ parity: script engine helper, will be wired to script actions
    fn get_waypoint_position(&self, waypoint_name: &str) -> Result<Coord3D, ScriptError> {
        let waypoint_name_ascii = AsciiString::from(waypoint_name);
        if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(waypoint) = terrain.get_waypoint_by_name(&waypoint_name_ascii) {
                let loc = waypoint.get_location();
                Ok(Coord3D::new(loc.x, loc.y, loc.z))
            } else {
                Err(ScriptError::ObjectNotFound(format!(
                    "Waypoint '{}' not found",
                    waypoint_name
                )))
            }
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ))
        }
    }

    /// Get trigger area by name
    /// Returns a clone of the trigger area for use in script execution
    #[allow(dead_code)] // C++ parity: script engine helper, will be wired to script actions
    fn get_trigger_area(
        &self,
        area_name: &str,
    ) -> Result<crate::polygon_trigger::PolygonTrigger, ScriptError> {
        if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(area_name) {
                Ok(trigger.clone())
            } else {
                Err(ScriptError::ObjectNotFound(format!(
                    "Trigger area '{}' not found",
                    area_name
                )))
            }
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ))
        }
    }

    fn eval_skirmish_command_button_ready_by_name(
        &self,
        team_name: &str,
        command_button_name: &str,
        _all_ready: bool,
    ) -> Result<bool, ScriptError> {
        let _ = self.get_team_by_name(team_name)?;
        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        Ok(control_bar
            .find_command_button_by_name(command_button_name)
            .is_some())
    }
}

// ============================================================================
// SCRIPT CONDITION EVALUATOR
// ============================================================================

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

    fn resolve_string_token(&self, raw: &str) -> String {
        match raw {
            THE_PLAYER | THIS_PLAYER => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                })
                .unwrap_or_else(|| raw.to_string()),
            LOCAL_PLAYER => player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| {
                    p.read()
                        .ok()
                        .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                })
                .unwrap_or_else(|| raw.to_string()),
            THIS_TEAM => get_script_engine()
                .read()
                .ok()
                .and_then(|g| {
                    g.as_ref().and_then(|e| {
                        e.get_condition_team_name()
                            .or_else(|| e.get_calling_team_name())
                            .map(|s| s.to_string())
                    })
                })
                .unwrap_or_else(|| raw.to_string()),
            TEAM_THE_PLAYER => {
                let current_player = get_script_engine().read().ok().and_then(|g| {
                    g.as_ref()
                        .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
                });
                let Some(player_name) = current_player else {
                    return raw.to_string();
                };

                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(&player_name))
                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                    .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                    .unwrap_or_else(|| raw.to_string())
            }
            _ => raw.to_string(),
        }
    }

    fn get_team_by_name(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<crate::team::Team>>, ScriptError> {
        let team_name = self.resolve_string_token(team_name);
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            factory_guard
                .find_team(&team_name)
                .ok_or_else(|| ScriptError::TeamNotFound(team_name.to_string()))
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock team factory".to_string(),
            ))
        }
    }

    fn get_trigger_area(
        &self,
        area_name: &str,
    ) -> Result<crate::polygon_trigger::PolygonTrigger, ScriptError> {
        if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(area_name) {
                Ok(trigger.clone())
            } else {
                Err(ScriptError::ObjectNotFound(format!(
                    "Trigger area '{}' not found",
                    area_name
                )))
            }
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ))
        }
    }

    /// Evaluate a script condition
    ///
    /// C++ Reference: ScriptConditions::evaluateCondition(Condition *pCondition)
    pub fn evaluate_condition(
        &mut self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let condition_type = condition.get_condition_type();

        // Dispatch to the appropriate handler based on condition type
        match condition_type {
            // ============================================================================
            // BASIC CONDITIONS
            // ============================================================================
            ConditionType::ConditionFalse => Ok(ScriptConditionResult::False),
            ConditionType::ConditionTrue => Ok(ScriptConditionResult::True),
            ConditionType::Counter => self.eval_counter(condition),
            ConditionType::Flag => self.eval_flag(condition),
            ConditionType::TimerExpired => self.eval_timer_expired(condition),

            // ============================================================================
            // PLAYER CONDITIONS
            // ============================================================================
            ConditionType::PlayerAllDestroyed => self.eval_player_all_destroyed(condition),
            ConditionType::PlayerAllBuildfacilitiesDestroyed => {
                self.eval_player_all_buildfacilities_destroyed(condition)
            }
            ConditionType::PlayerHasCredits => self.eval_player_has_credits(condition),
            ConditionType::PlayerHasNOrFewerBuildings => {
                self.eval_player_has_n_or_fewer_buildings(condition)
            }
            ConditionType::PlayerHasPower => self.eval_player_has_power(condition),
            ConditionType::PlayerHasNoPower => self.eval_player_has_no_power(condition),
            ConditionType::PlayerHasNOrFewerFactionBuildings => {
                self.eval_player_has_n_or_fewer_faction_buildings(condition)
            }
            ConditionType::PlayerPowerComparePercent => {
                self.eval_player_power_compare_percent(condition)
            }
            ConditionType::PlayerExcessPowerCompareValue => {
                self.eval_player_excess_power_compare_value(condition)
            }
            ConditionType::PlayerAcquiredScience => self.eval_player_acquired_science(condition),
            ConditionType::PlayerHasSciencepurchasepoints => {
                self.eval_player_has_science_purchase_points(condition)
            }
            ConditionType::PlayerCanPurchaseScience => {
                self.eval_player_can_purchase_science(condition)
            }
            ConditionType::PlayerLostObjectType => self.eval_player_lost_object_type(condition),
            ConditionType::PlayerDestroyedNBuildingsPlayer => {
                self.eval_player_destroyed_n_buildings_player(condition)
            }
            ConditionType::PlayerHasObjectComparison => {
                self.eval_player_has_object_comparison(condition)
            }

            // ============================================================================
            // TEAM CONDITIONS
            // ============================================================================
            ConditionType::TeamInsideAreaPartially => {
                self.eval_team_inside_area_partially(condition)
            }
            ConditionType::TeamDestroyed => self.eval_team_destroyed(condition),
            ConditionType::TeamHasUnits => self.eval_team_has_units(condition),
            ConditionType::TeamStateIs => self.eval_team_state_is(condition),
            ConditionType::TeamStateIsNot => self.eval_team_state_is_not(condition),
            ConditionType::TeamInsideAreaEntirely => self.eval_team_inside_area_entirely(condition),
            ConditionType::TeamOutsideAreaEntirely => {
                self.eval_team_outside_area_entirely(condition)
            }
            ConditionType::TeamAttackedByObjecttype => {
                self.eval_team_attacked_by_object_type(condition)
            }
            ConditionType::TeamAttackedByPlayer => self.eval_team_attacked_by_player(condition),
            ConditionType::TeamCreated => self.eval_team_created(condition),
            ConditionType::TeamDiscovered => self.eval_team_discovered(condition),
            ConditionType::TeamOwnedByPlayer => self.eval_team_owned_by_player(condition),
            ConditionType::TeamReachedWaypointsEnd => {
                self.eval_team_reached_waypoints_end(condition)
            }
            ConditionType::TeamEnteredAreaEntirely => {
                self.eval_team_entered_area_entirely(condition)
            }
            ConditionType::TeamEnteredAreaPartially => {
                self.eval_team_entered_area_partially(condition)
            }
            ConditionType::TeamExitedAreaEntirely => self.eval_team_exited_area_entirely(condition),
            ConditionType::TeamExitedAreaPartially => {
                self.eval_team_exited_area_partially(condition)
            }
            ConditionType::TeamCompletedSequentialExecution => {
                self.eval_team_completed_sequential_execution(condition)
            }
            ConditionType::TeamAllHasObjectStatus => {
                self.eval_team_all_has_object_status(condition)
            }
            ConditionType::TeamSomeHaveObjectStatus => {
                self.eval_team_some_have_object_status(condition)
            }

            // ============================================================================
            // NAMED OBJECT CONDITIONS
            // ============================================================================
            ConditionType::NamedInsideArea => self.eval_named_inside_area(condition),
            ConditionType::NamedOutsideArea => self.eval_named_outside_area(condition),
            ConditionType::NamedDestroyed => self.eval_named_destroyed(condition),
            ConditionType::NamedNotDestroyed => self.eval_named_not_destroyed(condition),
            ConditionType::NamedAttackedByObjecttype => {
                self.eval_named_attacked_by_object_type(condition)
            }
            ConditionType::NamedAttackedByPlayer => self.eval_named_attacked_by_player(condition),
            ConditionType::NamedCreated => self.eval_named_created(condition),
            ConditionType::NamedDiscovered => self.eval_named_discovered(condition),
            ConditionType::NamedOwnedByPlayer => self.eval_named_owned_by_player(condition),
            ConditionType::NamedReachedWaypointsEnd => {
                self.eval_named_reached_waypoints_end(condition)
            }
            ConditionType::NamedSelected => self.eval_named_selected(condition),
            ConditionType::NamedEnteredArea => self.eval_named_entered_area(condition),
            ConditionType::NamedExitedArea => self.eval_named_exited_area(condition),
            ConditionType::NamedDying => self.eval_named_dying(condition),
            ConditionType::NamedTotallyDead => self.eval_named_totally_dead(condition),
            ConditionType::NamedBuildingIsEmpty => self.eval_named_building_is_empty(condition),
            ConditionType::NamedHasFreeContainerSlots => {
                self.eval_named_has_free_container_slots(condition)
            }

            // ============================================================================
            // UNIT CONDITIONS
            // ============================================================================
            ConditionType::UnitHealth => self.eval_unit_health(condition),
            ConditionType::UnitCompletedSequentialExecution => {
                self.eval_unit_completed_sequential_execution(condition)
            }
            ConditionType::UnitEmptied => self.eval_unit_emptied(condition),
            ConditionType::UnitHasObjectStatus => self.eval_unit_has_object_status(condition),

            // ============================================================================
            // CAMERA CONDITIONS
            // ============================================================================
            ConditionType::CameraMovementFinished => self.eval_camera_movement_finished(condition),

            // ============================================================================
            // BUILDING CONDITIONS
            // ============================================================================
            ConditionType::BuiltByPlayer => self.eval_built_by_player(condition),
            ConditionType::BuildingEnteredByPlayer => {
                self.eval_building_entered_by_player(condition)
            }
            ConditionType::BridgeRepaired => self.eval_bridge_repaired(condition),
            ConditionType::BridgeBroken => self.eval_bridge_broken(condition),

            // ============================================================================
            // SPECIAL POWER CONDITIONS
            // ============================================================================
            ConditionType::PlayerTriggeredSpecialPower => {
                self.eval_player_triggered_special_power(condition)
            }
            ConditionType::PlayerCompletedSpecialPower => {
                self.eval_player_completed_special_power(condition)
            }
            ConditionType::PlayerMidwaySpecialPower => {
                self.eval_player_midway_special_power(condition)
            }
            ConditionType::PlayerTriggeredSpecialPowerFromNamed => {
                self.eval_player_triggered_special_power_from_named(condition)
            }
            ConditionType::PlayerCompletedSpecialPowerFromNamed => {
                self.eval_player_completed_special_power_from_named(condition)
            }
            ConditionType::PlayerMidwaySpecialPowerFromNamed => {
                self.eval_player_midway_special_power_from_named(condition)
            }

            // ============================================================================
            // UPGRADE CONDITIONS
            // ============================================================================
            ConditionType::PlayerBuiltUpgrade => self.eval_player_built_upgrade(condition),
            ConditionType::PlayerBuiltUpgradeFromNamed => {
                self.eval_player_built_upgrade_from_named(condition)
            }

            // ============================================================================
            // MULTIPLAYER CONDITIONS
            // ============================================================================
            ConditionType::MultiplayerAlliedVictory => {
                self.eval_multiplayer_allied_victory(condition)
            }
            ConditionType::MultiplayerAlliedDefeat => {
                self.eval_multiplayer_allied_defeat(condition)
            }
            ConditionType::MultiplayerPlayerDefeat => {
                self.eval_multiplayer_player_defeat(condition)
            }

            // ============================================================================
            // MEDIA CONDITIONS
            // ============================================================================
            ConditionType::HasFinishedVideo => self.eval_has_finished_video(condition),
            ConditionType::HasFinishedSpeech => self.eval_has_finished_speech(condition),
            ConditionType::HasFinishedAudio => self.eval_has_finished_audio(condition),
            ConditionType::MusicTrackHasCompleted => self.eval_music_track_has_completed(condition),

            // ============================================================================
            // MISCELLANEOUS CONDITIONS
            // ============================================================================
            ConditionType::EnemySighted => self.eval_enemy_sighted(condition),
            ConditionType::TypeSighted => self.eval_type_sighted(condition),
            ConditionType::MissionAttempts => self.eval_mission_attempts(condition),
            ConditionType::SupplySourceSafe => self.eval_supply_source_safe(condition),
            ConditionType::SupplySourceAttacked => self.eval_supply_source_attacked(condition),
            ConditionType::StartPositionIs => self.eval_start_position_is(condition),

            // ============================================================================
            // SKIRMISH CONDITIONS
            // ============================================================================
            ConditionType::SkirmishSpecialPowerReady => {
                self.eval_skirmish_special_power_ready(condition)
            }
            ConditionType::SkirmishValueInArea => self.eval_skirmish_value_in_area(condition),
            ConditionType::SkirmishPlayerFaction => self.eval_skirmish_player_faction(condition),
            ConditionType::SkirmishSuppliesValueWithinDistance => {
                self.eval_skirmish_supplies_value_within_distance(condition)
            }
            ConditionType::SkirmishTechBuildingWithinDistance => {
                self.eval_skirmish_tech_building_within_distance(condition)
            }
            ConditionType::SkirmishCommandButtonReadyAll => {
                self.eval_skirmish_command_button_ready_all(condition)
            }
            ConditionType::SkirmishCommandButtonReadyPartial => {
                self.eval_skirmish_command_button_ready_partial(condition)
            }
            ConditionType::SkirmishUnownedFactionUnitExists => {
                self.eval_skirmish_unowned_faction_unit_exists(condition)
            }
            ConditionType::SkirmishPlayerHasPrerequisiteToBuild => {
                self.eval_skirmish_player_has_prerequisite_to_build(condition)
            }
            ConditionType::SkirmishPlayerHasComparisonGarrisoned => {
                self.eval_skirmish_player_has_comparison_garrisoned(condition)
            }
            ConditionType::SkirmishPlayerHasComparisonCapturedUnits => {
                self.eval_skirmish_player_has_comparison_captured_units(condition)
            }
            ConditionType::SkirmishNamedAreaExist => self.eval_skirmish_named_area_exist(condition),
            ConditionType::SkirmishPlayerHasUnitsInArea => {
                self.eval_skirmish_player_has_units_in_area(condition)
            }
            ConditionType::SkirmishPlayerHasBeenAttackedByPlayer => {
                self.eval_skirmish_player_has_been_attacked_by_player(condition)
            }
            ConditionType::SkirmishPlayerIsOutsideArea => {
                self.eval_skirmish_player_is_outside_area(condition)
            }
            ConditionType::SkirmishPlayerHasDiscoveredPlayer => {
                self.eval_skirmish_player_has_discovered_player(condition)
            }

            // ============================================================================
            // AREA CONDITIONS
            // ============================================================================
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea => {
                self.eval_player_has_comparison_unit_type_in_trigger_area(condition)
            }
            ConditionType::PlayerHasComparisonUnitKindInTriggerArea => {
                self.eval_player_has_comparison_unit_kind_in_trigger_area(condition)
            }

            // ============================================================================
            // OBSOLETE/DEFUNCT CONDITIONS
            // ============================================================================
            ConditionType::ObsoleteScript1 => Ok(ScriptConditionResult::False),
            ConditionType::ObsoleteScript2 => Ok(ScriptConditionResult::False),
            ConditionType::DefunctPlayerSelectedGeneral => Ok(ScriptConditionResult::False),
            ConditionType::DefunctPlayerSelectedGeneralFromNamed => {
                Ok(ScriptConditionResult::False)
            }

            ConditionType::NumItems => Ok(ScriptConditionResult::False),
        }
    }

    /// Evaluate an OR condition (disjunction of AND conditions)
    pub fn evaluate_or_condition(
        &mut self,
        or_condition: &mut OrCondition,
    ) -> Result<bool, ScriptError> {
        // Iterate through all OR branches
        let mut current_or = Some(or_condition);
        while let Some(or_cond) = current_or {
            // Evaluate the AND chain for this OR branch
            if let Some(and_cond) = or_cond.first_and.as_deref_mut() {
                if self.evaluate_and_chain(and_cond)? {
                    return Ok(true);
                }
            }
            current_or = or_cond.next_or.as_deref_mut();
        }
        Ok(false)
    }

    /// Evaluate an AND chain of conditions
    fn evaluate_and_chain(&mut self, condition: &mut Condition) -> Result<bool, ScriptError> {
        let mut current = Some(condition);
        while let Some(cond) = current {
            match self.evaluate_condition(cond)? {
                ScriptConditionResult::True => {
                    // Continue to next AND condition
                    current = cond.next_and_condition.as_deref_mut();
                }
                ScriptConditionResult::False => {
                    // AND chain failed
                    return Ok(false);
                }
                ScriptConditionResult::Error(msg) => {
                    return Err(ScriptError::EvaluationFailed(msg));
                }
            }
        }
        // All conditions in the AND chain passed
        Ok(true)
    }

    // ============================================================================
    // BASIC CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptEngine::evaluateCounter() line 6319-6332
    fn eval_counter(&self, condition: &Condition) -> Result<ScriptConditionResult, ScriptError> {
        let counter_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_value = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating counter '{}' {:?} {}",
            counter_name,
            comparison,
            target_value
        );

        // Get counter value from script engine
        let counter_value = if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                engine
                    .get_counter(&counter_name)
                    .map(|c| c.value)
                    .unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        // Perform comparison matching C++ ScriptEngine::evaluateCounter()
        let result = match comparison {
            ComparisonType::LessThan => counter_value < target_value,
            ComparisonType::LessEqual => counter_value <= target_value,
            ComparisonType::Equal => counter_value == target_value,
            ComparisonType::GreaterEqual => counter_value >= target_value,
            ComparisonType::Greater => counter_value > target_value,
            ComparisonType::NotEqual => counter_value != target_value,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptEngine::evaluateFlag() line 6442-6450
    fn eval_flag(&self, condition: &Condition) -> Result<ScriptConditionResult, ScriptError> {
        let flag_name = self.get_condition_string_param(condition, 0)?;
        let expected = self.get_condition_bool_param(condition, 1)?;
        log::debug!("Evaluating flag '{}' == {}", flag_name, expected);

        // Get flag value from script engine
        let flag_value = if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                engine
                    .get_flag(&flag_name)
                    .map(|f| f.value)
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        };

        // Compare flag value with expected (C++ compares boolFlag == value)
        Ok(if flag_value == expected {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptEngine::evaluateTimerExpired() line 6700-6710
    /// Timers are counters with is_countdown_timer=true. Expired when value <= 0.
    fn eval_timer_expired(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let timer_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if timer '{}' expired", timer_name);

        // Get counter (timers are counters with is_countdown_timer flag)
        let is_expired = if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(counter) = engine.get_counter(&timer_name) {
                    // C++: If not a countdown timer, return false
                    // Timer is expired when is_countdown_timer && value <= 0
                    counter.is_countdown_timer && counter.value <= 0
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        Ok(if is_expired {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // PLAYER CONDITION HANDLERS
    // ============================================================================

    fn eval_player_all_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if all of player '{}' destroyed", player_name);

        // Parser compatibility: `PLAYER_ALIVE player TRUE/FALSE` is mapped onto this condition with
        // an optional second boolean that inverts the meaning (TRUE => player alive).
        let wants_alive = condition.get_parameter(1).map(|p| p.get_int() != 0);

        // Look up the player and check if they have any units
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // Player is all destroyed if they have no units
                    let all_destroyed = !player.has_any_units();
                    let result = match wants_alive {
                        Some(true) => !all_destroyed,
                        Some(false) => all_destroyed,
                        None => all_destroyed,
                    };
                    return Ok(if result {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // Player not found - consider as destroyed
        Ok(if wants_alive == Some(true) {
            ScriptConditionResult::False
        } else {
            ScriptConditionResult::True
        })
    }

    fn eval_player_all_buildfacilities_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if all build facilities of player '{}' destroyed",
            player_name
        );

        // Look up the player and check if they have any build facilities
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // All build facilities are destroyed if player has none
                    return Ok(if !player.has_any_build_facility() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // Player not found - consider build facilities as destroyed
        Ok(ScriptConditionResult::True)
    }

    fn eval_player_has_credits(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_credits = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' credits {:?} {}",
            player_name,
            comparison,
            target_credits
        );

        // Look up the player and get their credits
        let current_credits = if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    player.get_money().get_money()
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let result = match comparison {
            ComparisonType::LessThan => current_credits < target_credits,
            ComparisonType::LessEqual => current_credits <= target_credits,
            ComparisonType::Equal => current_credits == target_credits,
            ComparisonType::GreaterEqual => current_credits >= target_credits,
            ComparisonType::Greater => current_credits > target_credits,
            ComparisonType::NotEqual => current_credits != target_credits,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_player_has_n_or_fewer_buildings(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let count = self.get_condition_int_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' has {} or fewer buildings",
            player_name,
            count
        );

        // C++ parity: ScriptConditions::evaluatePlayerHasNOrFewerBuildings
        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let mut building_count: i32 = 0;
        for object_id in player.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.is_effectively_dead() || obj.is_destroyed() {
                continue;
            }
            if obj.is_kind_of(crate::common::KindOf::Structure) {
                building_count = building_count.saturating_add(1);
            }
        }

        Ok(if count >= building_count {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_player_has_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if player '{}' has power", player_name);

        // Look up the player and check their power status
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // C++ parity: Energy::hasSufficientPower
                    return Ok(if player.get_energy().has_sufficient_power() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // If player doesn't exist, default to no power
        Ok(ScriptConditionResult::False)
    }

    fn eval_player_has_no_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if player '{}' has no power", player_name);

        // Invert the has_power check
        match self.eval_player_has_power(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    fn eval_player_has_n_or_fewer_faction_buildings(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let count = self.get_condition_int_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' has {} or fewer faction buildings",
            player_name,
            count
        );

        // C++ parity: ScriptConditions::evaluatePlayerHasNOrFewerFactionBuildings
        // Uses KINDOF_MP_COUNT_FOR_VICTORY + KINDOF_STRUCTURE.
        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let mut building_count: i32 = 0;
        for object_id in player.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.is_effectively_dead() || obj.is_destroyed() {
                continue;
            }
            if obj.is_kind_of(crate::common::KindOf::Structure)
                && obj.is_kind_of(crate::common::KindOf::CountsForVictory)
            {
                building_count = building_count.saturating_add(1);
            }
        }

        Ok(if count >= building_count {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_player_power_compare_percent(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let percent = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating player '{}' power percent {:?} {}",
            player_name,
            comparison,
            percent
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let power_ratio = player.get_energy().supply_ratio();
        let test_ratio = percent as f32 / 100.0;
        Ok(if Self::compare_f32(comparison, power_ratio, test_ratio) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_player_excess_power_compare_value(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let desired_excess = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating player '{}' excess power {:?} {}",
            player_name,
            comparison,
            desired_excess
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let energy = player.get_energy();
        let actual_excess = energy.production() - energy.consumption();
        Ok(
            if Self::compare_i32(comparison, actual_excess, desired_excess) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    /// C++ Reference: ScriptConditions::evaluateScienceAcquired() line 1543-1553
    fn eval_player_acquired_science(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let science_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' acquired science '{}'",
            player_name,
            science_name
        );

        let science = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            log::warn!("Science '{}' not found in store", science_name);
            return Ok(ScriptConditionResult::False);
        }

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = match player_arc.read() {
            Ok(player) => player.get_player_index() as usize,
            Err(_) => return Ok(ScriptConditionResult::False),
        };

        let script_engine = get_script_engine();
        let Ok(mut engine_guard) = script_engine.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if engine.is_science_acquired(player_index, science, true) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_player_has_science_purchase_points(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_points = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' science points {:?} {}",
            player_name,
            comparison,
            target_points
        );

        // Look up the player and get their science purchase points
        let current_points = if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    player.get_science_purchase_points()
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let result = match comparison {
            ComparisonType::LessThan => current_points < target_points,
            ComparisonType::LessEqual => current_points <= target_points,
            ComparisonType::Equal => current_points == target_points,
            ComparisonType::GreaterEqual => current_points >= target_points,
            ComparisonType::Greater => current_points > target_points,
            ComparisonType::NotEqual => current_points != target_points,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::evaluateCanPurchaseScience() line 1559-1568
    fn eval_player_can_purchase_science(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let science_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' can purchase science '{}'",
            player_name,
            science_name
        );

        // Look up science type from name using science store
        let science = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            SCIENCE_INVALID
        };

        if science == SCIENCE_INVALID {
            log::warn!("Science '{}' not found in store", science_name);
            return Ok(ScriptConditionResult::False);
        }

        // Look up the player and check if they can purchase the science
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // Check prerequisites for this science
                    return Ok(if player.has_prereqs_for_science(science) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluatePlayerLostObjectType() - requires event tracking
    fn eval_player_lost_object_type(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let object_type = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' lost object type '{}'",
            player_name,
            object_type
        );

        let Some(player_index) = player_list()
            .read()
            .ok()
            .and_then(|players| players.find_player_by_name(&player_name))
            .and_then(|player_arc| {
                player_arc
                    .read()
                    .ok()
                    .map(|player| player.get_player_index())
            })
        else {
            return Ok(ScriptConditionResult::False);
        };

        let current_count = crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| {
                engine
                    .as_ref()
                    .map(|engine| engine.get_object_count(player_index, &object_type))
            })
            .unwrap_or(0);

        let object_manager = get_object_manager();
        let sum_of_objs = object_manager
            .read()
            .ok()
            .map(|manager| {
                manager
                    .all_object_ids()
                    .into_iter()
                    .filter(|object_id| {
                        let Some(obj_arc) = manager.get_object(*object_id) else {
                            return false;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            return false;
                        };
                        if obj_guard.is_destroyed() {
                            return false;
                        }
                        let owner = obj_guard
                            .base
                            .read()
                            .ok()
                            .and_then(|base| base.get_controlling_player_id())
                            .map(|id| id as i32)
                            .unwrap_or(-1);
                        if owner != player_index {
                            return false;
                        }
                        obj_guard
                            .template
                            .as_ref()
                            .map(|template| template.get_name().as_str() == object_type.as_str())
                            .unwrap_or(false)
                    })
                    .count() as i32
            })
            .unwrap_or(0);

        if sum_of_objs != current_count {
            if let Ok(mut engine_guard) = crate::scripting::engine::get_script_engine().write() {
                if let Some(ref mut engine) = *engine_guard {
                    engine.set_object_count(player_index, &object_type, sum_of_objs);
                }
            }
        }

        Ok(if sum_of_objs < current_count {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::evaluatePlayerDestroyedNBuildingsPlayer()
    /// Check if player has destroyed N buildings belonging to another player
    fn eval_player_destroyed_n_buildings_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;
        let _target_player = self.get_condition_string_param(condition, 3).ok();
        log::debug!(
            "Evaluating if player '{}' destroyed {:?} {} buildings",
            player_name,
            comparison,
            target_count
        );

        // Get player's score keeper to check buildings destroyed
        let buildings_destroyed = if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    player.get_score_keeper().get_buildings_destroyed()
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let result = match comparison {
            ComparisonType::LessThan => buildings_destroyed < target_count,
            ComparisonType::LessEqual => buildings_destroyed <= target_count,
            ComparisonType::Equal => buildings_destroyed == target_count,
            ComparisonType::GreaterEqual => buildings_destroyed >= target_count,
            ComparisonType::Greater => buildings_destroyed > target_count,
            ComparisonType::NotEqual => buildings_destroyed != target_count,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::evaluatePlayerHasObjectComparison()
    /// Check if player has N objects of a specific type
    fn eval_player_has_object_comparison(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ parameter order: [player, comparison, count, type_or_list]
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;
        let object_type = self.get_condition_string_param(condition, 3)?;
        log::debug!(
            "Evaluating player '{}' has {:?} {} of type '{}'",
            player_name,
            comparison,
            target_count,
            object_type
        );

        if condition.custom_data != 0 {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    if engine.get_frame_object_count_changed() == condition.custom_frame {
                        return Ok(if condition.custom_data == 1 {
                            ScriptConditionResult::True
                        } else {
                            ScriptConditionResult::False
                        });
                    }
                }
            }
        }

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let types = self.resolve_object_types_param(&object_type);
        let mut object_count = 0i32;
        for object_id in player.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_effectively_dead() {
                continue;
            }
            if types.contains_template(Some(obj_guard.get_template().as_ref())) {
                object_count += 1;
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => object_count < target_count,
            ComparisonType::LessEqual => object_count <= target_count,
            ComparisonType::Equal => object_count == target_count,
            ComparisonType::GreaterEqual => object_count >= target_count,
            ComparisonType::Greater => object_count > target_count,
            ComparisonType::NotEqual => object_count != target_count,
        };

        condition.custom_data = if result { 1 } else { -1 };
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                condition.custom_frame = engine.get_frame_object_count_changed();
            }
        }

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // TEAM CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptConditions::evaluateTeamInsideAreaPartially() line 378-392
    /// C++ pattern: theTeam->someInsideSomeOutside(pTrig, type) || theTeam->allInside(pTrig, type)
    /// Returns true if ANY team member is inside the area
    fn eval_team_inside_area_partially(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' is partially inside area '{}'",
            team_name,
            area_name
        );

        // Get team members and check if ANY are inside the area
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ANY team member is in the area
                        for &member_id in members {
                            if objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::True);
                            }
                        }
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if team '{}' is destroyed", team_name);

        // Look up the team and check if it has no members
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Team is destroyed if it has no members
                    return Ok(if team.get_member_count() == 0 {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // If team doesn't exist, consider it destroyed
        Ok(ScriptConditionResult::True)
    }

    fn eval_team_has_units(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if team '{}' has units", team_name);

        // Look up the team and check if it has members
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if team.get_member_count() > 0 {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // If team doesn't exist, it has no units
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_state_is(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let expected_state = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' state is '{}'",
            team_name,
            expected_state
        );

        // Look up the team and check its state
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let current_state = team.get_state();
                    return Ok(if current_state.as_str() == expected_state {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateTeamStateIsNot() line 608-620
    fn eval_team_state_is_not(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let expected_state = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' state is not '{}'",
            team_name,
            expected_state
        );

        // Look up the team and check its state
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let current_state = team.get_state();
                    return Ok(if current_state.as_str() != expected_state {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // C++: return false; // Non existent team isn't in any state.
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateTeamInsideAreaEntirely() line 632-649
    /// C++ pattern: theTeam->allInside(pTrig, type)
    /// Returns true if ALL team members are inside the area
    fn eval_team_inside_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' is entirely inside area '{}'",
            team_name,
            area_name
        );

        // Get team members and check if ALL are inside the area
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ALL team members are in the area
                        for &member_id in members {
                            if !objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::False);
                            }
                        }
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateTeamOutsideAreaEntirely() line 652-658
    /// C++ pattern: return !(evaluateTeamInsideAreaEntirely(...) || evaluateTeamInsideAreaPartially(...));
    fn eval_team_outside_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating team outside area entirely (using C++ pattern)");

        // C++ pattern: return !(evaluateTeamInsideAreaEntirely(...) || evaluateTeamInsideAreaPartially(...));
        let entirely_inside = self.eval_team_inside_area_entirely(condition)?;
        let partially_inside = self.eval_team_inside_area_partially(condition)?;

        // If either entirely or partially inside, team is NOT entirely outside
        let any_inside = matches!(entirely_inside, ScriptConditionResult::True)
            || matches!(partially_inside, ScriptConditionResult::True);

        Ok(if any_inside {
            ScriptConditionResult::False
        } else {
            ScriptConditionResult::True
        })
    }

    fn eval_team_attacked_by_object_type(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateTeamAttackedByType
        let team_name = self.get_condition_string_param(condition, 0)?;
        let types_param = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' attacked by object type '{}'",
            team_name,
            types_param
        );

        let wanted_types: Vec<&str> = types_param
            .split(|c| c == ',' || c == '|' || c == ';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if wanted_types.is_empty() {
            return Ok(ScriptConditionResult::False);
        }

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let Some(body) = obj.get_body_module() else {
                continue;
            };
            let Ok(body_guard) = body.lock() else {
                continue;
            };
            let Some(last) = body_guard.get_last_damage_info() else {
                continue;
            };

            if let Some(template) = &last.input.source_template {
                if wanted_types
                    .iter()
                    .any(|wanted| template.get_name().as_str() == *wanted)
                {
                    return Ok(ScriptConditionResult::True);
                }
            } else {
                // Old system: consult the attacker object template if the source template wasn't set.
                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    // C++ explicitly continues here so other team members can still satisfy.
                    continue;
                };
                let Ok(attacker) = attacker_arc.read() else {
                    continue;
                };
                let attacker_template = attacker.get_template();
                if wanted_types
                    .iter()
                    .any(|wanted| attacker_template.get_name().as_str() == *wanted)
                {
                    return Ok(ScriptConditionResult::True);
                }
            }
        }

        Ok(ScriptConditionResult::False)
    }

    fn eval_team_attacked_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateTeamAttackedByPlayer
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' attacked by player '{}'",
            team_name,
            player_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(victim_player) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(victim_guard) = victim_player.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let victim_index = victim_guard.get_player_index();

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let Some(body) = obj.get_body_module() else {
                continue;
            };
            let Ok(body_guard) = body.lock() else {
                continue;
            };
            let Some(last) = body_guard.get_last_damage_info() else {
                continue;
            };

            let attacker_id = last.input.source_id;
            let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                continue;
            };
            let Ok(attacker) = attacker_arc.read() else {
                continue;
            };
            let Some(attacker_owner) = attacker.get_controlling_player_id() else {
                continue;
            };

            if attacker_owner as i32 == victim_index {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    fn eval_team_created(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if team '{}' was created", team_name);

        // Look up the team and check if it was just created
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if team.is_created() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_discovered(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateTeamDiscovered
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' was discovered by player '{}'",
            team_name,
            player_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_id: u32 = match player.get_player_index().try_into() {
            Ok(value) => value,
            Err(_) => return Ok(ScriptConditionResult::False),
        };

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        let Ok(shroud_mgr) = shroud_mgr.lock() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            // We are held, so we are not visible.
            if obj.is_disabled_by_type(crate::common::DisabledType::Held) {
                continue;
            }

            // If we are stealthed we are not visible (unless DETECTED or DISGUISED).
            let status = obj.get_status_bits();
            if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
            {
                continue;
            }

            let shroud_state = shroud_mgr.get_shroud_state(player_id, obj.get_position());
            if matches!(
                shroud_state,
                crate::system::shroud_manager::ShroudState::Visible
                    | crate::system::shroud_manager::ShroudState::Explored
            ) {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    fn eval_team_owned_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' is owned by player '{}'",
            team_name,
            player_name
        );

        // Get the team and its controlling player ID
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    if let Some(controlling_player_id) = team.get_controlling_player_id() {
                        // Find the player and compare names
                        if let Ok(players) = player_list().read() {
                            if let Some(player_arc) =
                                players.get_player(controlling_player_id as i32)
                            {
                                if let Ok(player) = player_arc.read() {
                                    return Ok(
                                        if player.get_player_name_key()
                                            == NameKeyGenerator::name_to_key(&player_name)
                                        {
                                            ScriptConditionResult::True
                                        } else {
                                            ScriptConditionResult::False
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_reached_waypoints_end(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let waypoint_path = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' reached waypoints end for path '{}'",
            team_name,
            waypoint_path
        );

        // C++ parity: ScriptConditions::evaluateTeamReachedWaypointsEnd
        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };

        let mut any_at_end = false;
        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let Some(ai_arc) = obj.get_ai_update_interface() else {
                // C++: no AI -> continue (e.g. rocks/trees in team)
                continue;
            };
            let Ok(ai) = ai_arc.lock() else {
                continue;
            };
            let Some(completed_waypoint_id) = ai.get_completed_waypoint_id() else {
                continue;
            };
            let Some(target_waypoint) = terrain.get_waypoint_by_id(completed_waypoint_id) else {
                continue;
            };

            let found = target_waypoint.get_path_label1().as_str() == waypoint_path
                || target_waypoint.get_path_label2().as_str() == waypoint_path
                || target_waypoint.get_path_label3().as_str() == waypoint_path;
            if found {
                any_at_end = true;
            }
        }

        Ok(if any_at_end {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_team_entered_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' entered area '{}' entirely",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and is now entirely inside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ALL team members are now in the area
                        for &member_id in members {
                            if !objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::False);
                            }
                        }
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_entered_area_partially(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' entered area '{}' partially",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and at least one member is now inside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ANY team member is now in the area
                        for &member_id in members {
                            if objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::True);
                            }
                        }
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_exited_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' exited area '{}' entirely",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and is now entirely outside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        // Empty team considered to have exited
                        return Ok(ScriptConditionResult::True);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if NO team members are in the area
                        for &member_id in members {
                            if objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::False);
                            }
                        }
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_exited_area_partially(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' exited area '{}' partially",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and at least one member is now outside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ANY team member is now outside the area
                        for &member_id in members {
                            if !objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::True);
                            }
                        }
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_completed_sequential_execution(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if team '{}' completed sequential execution",
            team_name
        );
        Ok(ScriptConditionResult::False)
    }

    fn eval_team_all_has_object_status(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let status_mask = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?
            .get_object_status();
        log::debug!(
            "Evaluating if all of team '{}' has object status",
            team_name
        );

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                return Ok(ScriptConditionResult::False);
            };
            let Ok(obj) = obj_arc.read() else {
                return Ok(ScriptConditionResult::False);
            };
            if !obj.get_status_bits().intersects(status_mask) {
                return Ok(ScriptConditionResult::False);
            }
        }

        Ok(ScriptConditionResult::True)
    }

    fn eval_team_some_have_object_status(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let status_mask = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?
            .get_object_status();
        log::debug!(
            "Evaluating if some of team '{}' have object status",
            team_name
        );

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.get_status_bits().intersects(status_mask) {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    // ============================================================================
    // NAMED OBJECT CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptConditions::evaluateNamedInsideArea() line 395-415
    /// C++ pattern: Gets object position and calls pTrig->pointInTrigger(iCoord)
    fn eval_named_inside_area(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' is inside area '{}'",
            object_name,
            area_name
        );

        // Look up the named object using the tracker
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            // Check if object is in the area using the area tracker
            let area_tracker = get_area_tracker();
            if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                return Ok(if objects_in_area.contains(&object_id) {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateNamedOutsideArea() line 625
    /// C++ simply returns !evaluateNamedInsideArea(...)
    fn eval_named_outside_area(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating named outside area (inverting inside check per C++)");

        // C++ pattern: return !evaluateNamedInsideArea(pUnitParm, pTriggerParm);
        match self.eval_named_inside_area(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    fn eval_named_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is destroyed", object_name);

        // Look up the named object using the tracker
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            // Check if the object exists and is destroyed
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    return Ok(if obj.is_destroyed() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
            // Object ID exists but object not found - considered destroyed
            return Ok(ScriptConditionResult::True);
        }
        // Object not in tracker - considered destroyed
        Ok(ScriptConditionResult::True)
    }

    fn eval_named_not_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is not destroyed", object_name);

        // Invert the destroyed check
        match self.eval_named_destroyed(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    fn eval_named_attacked_by_object_type(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateNamedAttackedByType
        let object_name = self.get_condition_string_param(condition, 0)?;
        let types_param = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' attacked by object type '{}'",
            object_name,
            types_param
        );

        let wanted_types: Vec<&str> = types_param
            .split(|c| c == ',' || c == '|' || c == ';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if wanted_types.is_empty() {
            return Ok(ScriptConditionResult::False);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(body) = obj.get_body_module() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(ScriptConditionResult::False);
        };

        if let Some(template) = &last.input.source_template {
            return Ok(
                if wanted_types
                    .iter()
                    .any(|wanted| template.get_name().as_str() == *wanted)
                {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                },
            );
        }

        // Old system: consult attacker object if source template wasn't set.
        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(attacker) = attacker_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let attacker_template = attacker.get_template();

        Ok(
            if wanted_types
                .iter()
                .any(|wanted| attacker_template.get_name().as_str() == *wanted)
            {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    fn eval_named_attacked_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateNamedAttackedByPlayer
        let object_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' attacked by player '{}'",
            object_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(body) = obj.get_body_module() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(victim_player) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(victim_guard) = victim_player.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let victim_index = victim_guard.get_player_index();

        // Prefer the source player mask if present (C++ does this first).
        let mask_bits = last.input.source_player_mask.bits();
        if mask_bits != 0 {
            let masked_index = mask_bits.trailing_zeros() as i32;
            if masked_index == victim_index {
                return Ok(ScriptConditionResult::True);
            }
        }

        // Fallback to attacker object controlling player.
        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(attacker) = attacker_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(attacker_owner) = attacker.get_controlling_player_id() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if attacker_owner as i32 == victim_index {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::evaluateNamedCreated() line 900-907
    /// Note: the original implementation checks whether the named unit exists, not whether it was
    /// created this frame.
    fn eval_named_created(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if '{}' was created (checking existence per C++)",
            object_name
        );

        // C++ pattern: return (TheScriptEngine->getUnitNamed(pUnitParm->getString()) != NULL);
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            // Verify object actually exists
            if TheGameLogic::find_object_by_id(object_id).is_some() {
                return Ok(ScriptConditionResult::True);
            }
        }
        Ok(ScriptConditionResult::False)
    }

    fn eval_named_discovered(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateNamedDiscovered
        let object_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' was discovered by player '{}'",
            object_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_id: u32 = match player.get_player_index().try_into() {
            Ok(value) => value,
            Err(_) => return Ok(ScriptConditionResult::False),
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        // We are held, so we are not visible.
        if obj.is_disabled_by_type(crate::common::DisabledType::Held) {
            return Ok(ScriptConditionResult::False);
        }

        // If we are stealthed we are not visible (unless DETECTED or DISGUISED).
        let status = obj.get_status_bits();
        if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
            && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
            && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
        {
            return Ok(ScriptConditionResult::False);
        }

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        let Ok(shroud_mgr) = shroud_mgr.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let shroud_state = shroud_mgr.get_shroud_state(player_id, obj.get_position());

        Ok(
            if matches!(
                shroud_state,
                crate::system::shroud_manager::ShroudState::Visible
                    | crate::system::shroud_manager::ShroudState::Explored
            ) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    fn eval_named_owned_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' owned by player '{}'",
            object_name,
            player_name
        );

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Get controlling player and compare display name
                    if let Some(controlling_player) = obj.get_controlling_player() {
                        if let Ok(player) = controlling_player.read() {
                            return Ok(
                                if player.get_player_name_key()
                                    == NameKeyGenerator::name_to_key(&player_name)
                                {
                                    ScriptConditionResult::True
                                } else {
                                    ScriptConditionResult::False
                                },
                            );
                        }
                    }
                }
            }
        }
        // Object not found or has no owner - condition is false
        Ok(ScriptConditionResult::False)
    }

    fn eval_named_reached_waypoints_end(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let waypoint_path = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' reached waypoints end for path '{}'",
            object_name,
            waypoint_path
        );

        // C++ parity: ScriptConditions::evaluateNamedReachedWaypointsEnd
        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(ai_arc) = obj.get_ai_update_interface() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(ai) = ai_arc.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(completed_waypoint_id) = ai.get_completed_waypoint_id() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(target_waypoint) = terrain.get_waypoint_by_id(completed_waypoint_id) else {
            return Ok(ScriptConditionResult::False);
        };

        let reached = target_waypoint.get_path_label1().as_str() == waypoint_path
            || target_waypoint.get_path_label2().as_str() == waypoint_path
            || target_waypoint.get_path_label3().as_str() == waypoint_path;
        Ok(if reached {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_named_selected(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is selected", object_name);

        let game_logic = crate::system::game_logic::get_game_logic();
        if game_logic
            .lock()
            .ok()
            .is_some_and(|logic| logic.is_in_multiplayer_game())
        {
            return Ok(ScriptConditionResult::False);
        }

        let Ok(list) = crate::player::player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let local_player_id = list.get_local_player_index();
        if local_player_id < 0 {
            return Ok(ScriptConditionResult::False);
        }

        let selection_manager = crate::commands::get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let selection_changed_frame = manager.get_frame_selection_changed();
        let mut any_changes = condition.custom_data == 0;
        if selection_changed_frame != condition.custom_frame {
            any_changes = true;
        }

        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let Some(selection) = manager.get_player_selection_ref(local_player_id) else {
            condition.custom_data = -1;
            condition.custom_frame = selection_changed_frame;
            return Ok(ScriptConditionResult::False);
        };

        let wanted = crate::common::AsciiString::from(object_name.as_str());
        let mut is_selected = false;
        for object_id in selection.get_selected_objects() {
            let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };
            if guard.get_name() == &wanted {
                is_selected = true;
                break;
            }
        }

        condition.custom_data = if is_selected { 1 } else { -1 };
        condition.custom_frame = selection_changed_frame;
        Ok(if is_selected {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_named_entered_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' entered area '{}'",
            object_name,
            area_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let area_tracker = crate::scripting::engine::get_area_tracker();
        let last_enter = area_tracker.get_last_enter_frame(&area_name, object_id);
        let last_seen = condition.custom_frame;

        let entered = last_enter.is_some_and(|frame| frame > last_seen);
        if entered {
            condition.custom_frame = last_enter.unwrap_or(last_seen);
        }

        Ok(if entered {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_named_exited_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' exited area '{}'",
            object_name,
            area_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let area_tracker = crate::scripting::engine::get_area_tracker();
        let last_exit = area_tracker.get_last_exit_frame(&area_name, object_id);
        let last_seen = condition.custom_frame;

        let exited = last_exit.is_some_and(|frame| frame > last_seen);
        if exited {
            condition.custom_frame = last_exit.unwrap_or(last_seen);
        }

        Ok(if exited {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_named_dying(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is dying", object_name);

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Object is dying if destroyed but not yet effectively dead
                    let is_dying = obj.is_destroyed() && !obj.is_effectively_dead();
                    return Ok(if is_dying {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // Object not found - not dying
        Ok(ScriptConditionResult::False)
    }

    fn eval_named_totally_dead(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is totally dead", object_name);

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    return Ok(if obj.is_effectively_dead() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
            // Object ID exists but object not found - considered totally dead
            return Ok(ScriptConditionResult::True);
        }
        // Object not in tracker - considered totally dead
        Ok(ScriptConditionResult::True)
    }

    /// C++ Reference: ScriptConditions::evaluateIsBuildingEmpty() line 1008-1024
    fn eval_named_building_is_empty(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' building is empty", object_name);

        // Look up the building object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // C++ pattern: get contain module, check if count > 0
                    if let Some(contain_arc) = obj.get_contain() {
                        if let Ok(contain_guard) = contain_arc.lock() {
                            let count = contain_guard.get_contained_count();
                            return Ok(if count == 0 {
                                ScriptConditionResult::True
                            } else {
                                ScriptConditionResult::False
                            });
                        }
                    }
                    // No contain module = false per C++
                    return Ok(ScriptConditionResult::False);
                }
            }
        }
        // Building not found = false per C++
        Ok(ScriptConditionResult::False)
    }

    fn eval_named_has_free_container_slots(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' has free container slots", object_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(contain_arc) = obj.get_contain() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(contain) = contain_arc.lock() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if contain.get_contained_count() < contain.get_max_capacity() {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    // ============================================================================
    // UNIT CONDITION HANDLERS
    // ============================================================================

    fn eval_unit_health(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_health = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating unit '{}' health {:?} {}",
            unit_name,
            comparison,
            target_health
        );

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Get health percentage (0-100 scale)
                    let health_percent = (obj.get_health_percentage() * 100.0) as i32;

                    let result = match comparison {
                        ComparisonType::LessThan => health_percent < target_health,
                        ComparisonType::LessEqual => health_percent <= target_health,
                        ComparisonType::Equal => health_percent == target_health,
                        ComparisonType::GreaterEqual => health_percent >= target_health,
                        ComparisonType::Greater => health_percent > target_health,
                        ComparisonType::NotEqual => health_percent != target_health,
                    };

                    return Ok(if result {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // Object not found - consider health check as false
        Ok(ScriptConditionResult::False)
    }

    fn eval_unit_completed_sequential_execution(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if unit '{}' completed sequential execution",
            unit_name
        );
        Ok(ScriptConditionResult::False)
    }

    fn eval_unit_emptied(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if unit '{}' emptied", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let num_peeps = obj
            .get_contain()
            .and_then(|contain| contain.lock().ok().map(|c| c.get_contained_count()))
            .unwrap_or(0);
        let frame = TheGameLogic::get_frame();

        let Ok(mut statuses) = TRANSPORT_STATUSES.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let entry = statuses.entry(object_id).or_insert((frame, num_peeps));

        if entry.0 == frame.saturating_sub(1) && entry.1 > 0 && num_peeps == 0 {
            // Match C++: do not update this frame so repeated checks remain true.
            return Ok(ScriptConditionResult::True);
        }

        *entry = (frame, num_peeps);
        Ok(ScriptConditionResult::False)
    }

    fn eval_unit_has_object_status(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let status_mask = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?
            .get_object_status();
        log::debug!("Evaluating if unit '{}' has object status", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if obj.get_status_bits().intersects(status_mask) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // CAMERA CONDITION HANDLERS
    // ============================================================================

    fn eval_camera_movement_finished(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating if camera movement finished");
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    return Ok(if handler.is_camera_movement_finished() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::True)
    }

    // ============================================================================
    // BUILDING CONDITION HANDLERS
    // ============================================================================

    fn eval_built_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let type_or_list_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' has built object type/list '{}'",
            player_name,
            type_or_list_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let types = self.resolve_object_types_param(&type_or_list_name);
        if types.list_size() == 0 {
            return Ok(ScriptConditionResult::False);
        }

        for obj_arc in player.get_objects() {
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if types.contains_template(Some(obj.get_template())) {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    fn eval_building_entered_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let building_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if building '{}' entered by player '{}'",
            building_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&building_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(contain_arc) = obj.get_contain() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(contain) = contain_arc.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let entered_mask = contain.get_player_who_entered();
        if entered_mask == crate::common::PlayerMaskType::none() {
            return Ok(ScriptConditionResult::False);
        }

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if entered_mask == player.get_player_mask() {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_bridge_repaired(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let bridge_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if bridge '{}' repaired", bridge_name);
        let tracker = get_named_object_tracker();
        let Ok(Some(bridge_id)) = tracker.get_object_id(&bridge_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        Ok(if terrain.is_bridge_repaired(bridge_id) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_bridge_broken(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let bridge_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if bridge '{}' broken", bridge_name);
        let tracker = get_named_object_tracker();
        let Ok(Some(bridge_id)) = tracker.get_object_id(&bridge_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        Ok(if terrain.is_bridge_broken(bridge_id) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // SPECIAL POWER CONDITION HANDLERS
    // ============================================================================

    fn eval_player_triggered_special_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' triggered special power '{}'",
            player_name,
            power_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if engine.is_special_power_triggered(player_index, &power_name, true, INVALID_ID) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    fn eval_player_completed_special_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' completed special power '{}'",
            player_name,
            power_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if engine.is_special_power_complete(player_index, &power_name, true, INVALID_ID) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    fn eval_player_midway_special_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' midway special power '{}'",
            player_name,
            power_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if engine.is_special_power_midway(player_index, &power_name, true, INVALID_ID) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    fn eval_player_triggered_special_power_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' triggered special power '{}' from '{}'",
            player_name,
            power_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if engine.is_special_power_triggered(player_index, &power_name, true, source_id) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    fn eval_player_completed_special_power_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' completed special power '{}' from '{}'",
            player_name,
            power_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if engine.is_special_power_complete(player_index, &power_name, true, source_id) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    fn eval_player_midway_special_power_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' midway special power '{}' from '{}'",
            player_name,
            power_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let script_engine_lock = get_script_engine();
        let Ok(mut engine_guard) = script_engine_lock.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(engine) = engine_guard.as_mut() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if engine.is_special_power_midway(player_index, &power_name, true, source_id) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    // ============================================================================
    // UPGRADE CONDITION HANDLERS
    // ============================================================================

    fn eval_player_built_upgrade(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let upgrade_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' built upgrade '{}'",
            player_name,
            upgrade_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        let player_has_upgrade = {
            let mask = crate::upgrade::upgrade_mask_for_name(&upgrade_name);
            let completed = player.get_completed_upgrade_mask();
            completed.intersects(crate::common::UpgradeMaskType::from_bits_retain(
                mask.to_bits(),
            ))
        };
        drop(player);
        drop(players);

        let event_hit = if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.is_upgrade_complete(player_index, &upgrade_name, true, INVALID_ID)
            } else {
                false
            }
        } else {
            false
        };

        Ok(if event_hit || player_has_upgrade {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_player_built_upgrade_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let upgrade_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' built upgrade '{}' from '{}'",
            player_name,
            upgrade_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as usize;
        let player_has_upgrade = {
            let mask = crate::upgrade::upgrade_mask_for_name(&upgrade_name);
            let completed = player.get_completed_upgrade_mask();
            completed.intersects(crate::common::UpgradeMaskType::from_bits_retain(
                mask.to_bits(),
            ))
        };
        drop(player);
        drop(players);

        let event_hit = if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.is_upgrade_complete(player_index, &upgrade_name, true, source_id)
            } else {
                false
            }
        } else {
            false
        };

        Ok(if event_hit || player_has_upgrade {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // MULTIPLAYER CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptConditions::checkMultiplayerAlliedVictory()
    /// Checks if all allied players have won
    fn eval_multiplayer_allied_victory(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating multiplayer allied victory");
        // C++ uses TheVictoryConditions for local allied victory checks.
        Ok(if TheVictoryConditions::is_local_allied_victory() {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::checkMultiplayerAlliedDefeat()
    /// Checks if all allied players have been defeated
    fn eval_multiplayer_allied_defeat(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating multiplayer allied defeat");
        let players = player_list();
        let Ok(players_lock) = players.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Some(local_player_arc) = players_lock.get_local_player() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(local_player) = local_player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let mut allied_count = 0usize;
        for player_arc in players_lock.iter() {
            if Arc::ptr_eq(player_arc, &local_player_arc) {
                allied_count += 1;
                if !local_player.is_defeated() {
                    return Ok(ScriptConditionResult::False);
                }
                continue;
            }

            let Ok(player) = player_arc.read() else {
                continue;
            };
            if local_player.is_allied_with_player(&player) {
                allied_count += 1;
                if !player.is_defeated() {
                    return Ok(ScriptConditionResult::False);
                }
            }
        }

        Ok(if allied_count > 0 {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::checkMultiplayerPlayerDefeat()
    /// Checks if a specific player has been defeated
    fn eval_multiplayer_player_defeat(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating multiplayer player '{}' defeat", player_name);

        // Look up player by name and check their defeat status
        let players = player_list();
        if let Ok(players_lock) = players.read() {
            for player_arc in players_lock.iter() {
                if let Ok(player) = player_arc.read() {
                    if player.get_player_name_key() == NameKeyGenerator::name_to_key(&player_name) {
                        // Check if player has been defeated
                        if player.is_player_dead() {
                            return Ok(ScriptConditionResult::True);
                        }
                        break;
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    // ============================================================================
    // MEDIA CONDITION HANDLERS
    // ============================================================================

    fn eval_has_finished_video(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let name = self.get_condition_string_param(_condition, 0)?;
        log::debug!("Evaluating if video '{}' finished", name);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    return Ok(if handler.is_video_complete(&name, true) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::True)
    }

    fn eval_has_finished_speech(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let name = self.get_condition_string_param(_condition, 0)?;
        log::debug!("Evaluating if speech '{}' finished", name);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    return Ok(if handler.is_speech_complete(&name, true) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::True)
    }

    fn eval_has_finished_audio(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let name = self.get_condition_string_param(_condition, 0)?;
        log::debug!("Evaluating if audio '{}' finished", name);
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    return Ok(if handler.is_audio_complete(&name, true) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::True)
    }

    fn eval_music_track_has_completed(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let track = self.get_condition_string_param(_condition, 0)?;
        let param = self.get_condition_int_param(_condition, 1).unwrap_or(0);
        log::debug!(
            "Evaluating if music track '{}' completed (param: {})",
            track,
            param
        );
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    return Ok(if handler.has_music_track_completed(&track, param) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::True)
    }

    // ============================================================================
    // MISCELLANEOUS CONDITION HANDLERS
    // ============================================================================

    fn eval_enemy_sighted(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let alliance = self.get_condition_int_param(condition, 1)?;
        let player_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if unit '{}' has sighted alliance {} unit from player '{}'",
            unit_name,
            alliance,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(unit) = unit_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(target_player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let src_pos = *unit.get_position();
        let vision_range = unit.get_vision_range();
        let src_off_map = unit.is_off_map();
        let Some(partition) = ThePartitionManager::get() else {
            return Ok(ScriptConditionResult::False);
        };

        for obj_id in partition.get_objects_in_range(&src_pos, vision_range) {
            if obj_id == unit_id {
                continue;
            }
            let Some(candidate_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(candidate) = candidate_arc.read() else {
                continue;
            };

            if candidate.is_effectively_dead() {
                continue;
            }
            if candidate.is_off_map() != src_off_map {
                continue;
            }

            let status = candidate.get_status_bits();
            if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
            {
                continue;
            }

            let relationship = unit.relationship_to(&candidate);
            let relation_ok = match alliance {
                0 => relationship == Relationship::Enemies, // REL_ENEMY
                1 => relationship == Relationship::Neutral, // REL_NEUTRAL
                2 => matches!(relationship, Relationship::Allies), // REL_FRIEND
                _ => false,
            };
            if !relation_ok {
                continue;
            }

            if let Some(owner) = candidate.get_controlling_player() {
                if Arc::ptr_eq(&owner, &target_player_arc) {
                    return Ok(ScriptConditionResult::True);
                }
            }
        }

        Ok(ScriptConditionResult::False)
    }

    fn eval_type_sighted(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let type_or_list_name = self.get_condition_string_param(condition, 1)?;
        let player_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if unit '{}' has sighted type/list '{}' from player '{}'",
            unit_name,
            type_or_list_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(unit) = unit_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(target_player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let wanted_types = self.resolve_object_types_param(&type_or_list_name);
        if wanted_types.list_size() == 0 {
            return Ok(ScriptConditionResult::False);
        }

        let src_pos = *unit.get_position();
        let vision_range = unit.get_vision_range();
        let src_off_map = unit.is_off_map();
        let Some(partition) = ThePartitionManager::get() else {
            return Ok(ScriptConditionResult::False);
        };

        for obj_id in partition.get_objects_in_range(&src_pos, vision_range) {
            if obj_id == unit_id {
                continue;
            }
            let Some(candidate_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(candidate) = candidate_arc.read() else {
                continue;
            };

            if candidate.is_effectively_dead() {
                continue;
            }
            if candidate.is_off_map() != src_off_map {
                continue;
            }

            let status = candidate.get_status_bits();
            if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
            {
                continue;
            }

            let Some(owner) = candidate.get_controlling_player() else {
                continue;
            };
            if !Arc::ptr_eq(&owner, &target_player_arc) {
                continue;
            }
            if !wanted_types.contains_template(Some(candidate.get_template())) {
                continue;
            }
            return Ok(ScriptConditionResult::True);
        }

        Ok(ScriptConditionResult::False)
    }

    fn eval_mission_attempts(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let comparison = self.get_condition_comparison_param(condition, 0)?;
        let value = self.get_condition_int_param(condition, 1)?;
        log::debug!("Evaluating mission attempts {:?} {}", comparison, value);
        Ok(ScriptConditionResult::False)
    }

    fn eval_supply_source_safe(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let min_supply_amount = self.get_condition_int_param(condition, 1)?;
        log::debug!("Evaluating if supply source safe for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_id = player.get_player_index() as u32;
        drop(player);
        drop(players);

        let is_safe = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                crate::ai::integration::IntegratedAiPlayer::Standard(ai) => {
                    ai.is_supply_source_safe(min_supply_amount)
                }
                crate::ai::integration::IntegratedAiPlayer::Skirmish(ai) => {
                    ai.is_supply_source_safe(min_supply_amount)
                }
            })
        })
        .flatten()
        .unwrap_or(false);

        Ok(if is_safe {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_supply_source_attacked(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if supply source attacked for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_id = player.get_player_index() as u32;
        drop(player);
        drop(players);

        let attacked = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                crate::ai::integration::IntegratedAiPlayer::Standard(ai) => {
                    ai.is_supply_source_attacked()
                }
                crate::ai::integration::IntegratedAiPlayer::Skirmish(ai) => {
                    ai.is_supply_source_attacked()
                }
            })
        })
        .flatten()
        .unwrap_or(false);

        Ok(if attacked {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_start_position_is(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let start_position = self.get_condition_int_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' start position is {}",
            player_name,
            start_position
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        // C++ expects external start positions as 1-based indices.
        let expected_index = start_position - 1;
        Ok(if player.get_mp_start_index() == expected_index {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // SKIRMISH CONDITION HANDLERS
    // ============================================================================

    fn eval_skirmish_special_power_ready(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish special power '{}' ready for '{}'",
            power_name,
            player_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let player_id = player.get_player_index() as crate::common::ObjectID;
        let power_name_lower = power_name.to_ascii_lowercase();

        for power in crate::special_power_module::registry::get_player_powers(player_id) {
            let Ok(power) = power.lock() else {
                continue;
            };
            if power.get_data().name.to_string().to_ascii_lowercase() != power_name_lower {
                continue;
            }

            if power.is_ready() {
                return Ok(ScriptConditionResult::True);
            }

            return Ok(ScriptConditionResult::False);
        }

        Ok(ScriptConditionResult::False)
    }

    fn eval_skirmish_value_in_area(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: evaluateSkirmishValueInArea(SIDE, COMPARISON, INT, TRIGGER)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let compare_value = self.get_condition_int_param(condition, 2)?;
        let area_name = self.get_condition_string_param(condition, 3)?;
        log::debug!(
            "Evaluating skirmish value in area '{}' for '{}' {:?} {}",
            area_name,
            player_name,
            comparison,
            compare_value
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as i32;

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .map_err(|e| ScriptError::EvaluationFailed(e.to_string()))?;

        let mut total_cost: i32 = 0;
        for object_id in objects_in_area {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            // C++ excludes effectively-dead objects.
            if obj.is_effectively_dead() || obj.is_destroyed() {
                continue;
            }
            // C++ ignores inert/projectiles; we can at least ignore projectiles.
            if obj.is_kind_of(crate::common::KindOf::Projectile) {
                continue;
            }

            let owner = obj
                .get_controlling_player_id()
                .map(|id| id as i32)
                .unwrap_or(-1);
            if owner != player_index {
                continue;
            }

            total_cost = total_cost.saturating_add(obj.get_template().get_build_cost());
        }

        let result = match comparison {
            ComparisonType::LessThan => total_cost < compare_value,
            ComparisonType::LessEqual => total_cost <= compare_value,
            ComparisonType::Equal => total_cost == compare_value,
            ComparisonType::GreaterEqual => total_cost >= compare_value,
            ComparisonType::Greater => total_cost > compare_value,
            ComparisonType::NotEqual => total_cost != compare_value,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_player_faction(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: evaluateSkirmishPlayerIsFaction(SIDE, FACTION)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let faction = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating skirmish player '{}' faction == '{}'",
            player_name,
            faction
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if player.get_side() == &faction {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_supplies_value_within_distance(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let distance = self.get_condition_real_param(condition, 1)?;
        let area_name = self.get_condition_string_param(condition, 2)?;
        let compare_value = self.get_condition_real_param(condition, 3)?;

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;

        let trigger = self.get_trigger_area(&area_name)?;
        let center = trigger.get_center_point();
        let radius = trigger.get_radius() + distance;

        let base_value = global_data::read_safe()
            .map(|data| data.base_value_per_supply_box.max(1))
            .unwrap_or(1) as f32;

        let mut max_value = 0.0;
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            for obj_id in partition.get_objects_in_range(&center, radius) {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() || obj_guard.is_off_map() {
                    continue;
                }
                if !obj_guard.is_kind_of(crate::common::KindOf::Structure) {
                    continue;
                }

                let allow_affiliation =
                    if let Some(owner_id) = obj_guard.get_controlling_player_id() {
                        if owner_id == player_guard.get_player_index() as u32 {
                            true
                        } else if let Some(owner_arc) = player_list()
                            .read()
                            .ok()
                            .and_then(|list| list.get_player(owner_id as i32).cloned())
                        {
                            if let Ok(owner_guard) = owner_arc.read() {
                                player_guard.get_relationship(&owner_guard) == Relationship::Neutral
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                if !allow_affiliation {
                    continue;
                }

                let Some(module) = obj_guard.find_update_module("SupplyWarehouseDockUpdate") else {
                    continue;
                };
                let mut boxes = None;
                module.with_module_downcast::<crate::object::production::SupplyWarehouseDockUpdateModule, _, _>(|module| {
                    boxes = Some(module.behavior().get_boxes_stored());
                });
                let Some(boxes) = boxes else {
                    continue;
                };

                let value = base_value * boxes as f32;
                if value > max_value {
                    max_value = value;
                }
            }
        }

        Ok(if max_value > compare_value {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_tech_building_within_distance(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let distance = self.get_condition_real_param(condition, 1)?;
        let area_name = self.get_condition_string_param(condition, 2)?;

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;

        let trigger = self.get_trigger_area(&area_name)?;
        let center = trigger.get_center_point();
        let radius = trigger.get_radius() + distance;

        let mut found = false;
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            for obj_id in partition.get_objects_in_range(&center, radius) {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() || obj_guard.is_off_map() {
                    continue;
                }
                if !obj_guard.is_kind_of(crate::common::KindOf::TechBuilding) {
                    continue;
                }

                let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                    continue;
                };
                if owner_id == player_guard.get_player_index() as u32 {
                    continue;
                }
                if let Some(owner_arc) = player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(owner_id as i32).cloned())
                {
                    if let Ok(owner_guard) = owner_arc.read() {
                        let rel = player_guard.get_relationship(&owner_guard);
                        if matches!(rel, Relationship::Allies) {
                            continue;
                        }
                    }
                }

                found = true;
                break;
            }
        }

        Ok(if found {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_command_button_ready_all(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        self.eval_skirmish_command_button_ready(condition, true)
    }

    fn eval_skirmish_command_button_ready_partial(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        self.eval_skirmish_command_button_ready(condition, false)
    }

    fn eval_skirmish_command_button_ready(
        &self,
        condition: &Condition,
        all_ready: bool,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 1)?;
        let command_button = self.get_condition_string_param(condition, 2)?;
        let ready = self.eval_skirmish_command_button_ready_by_name(
            &team_name,
            &command_button,
            all_ready,
        )?;
        Ok(if ready {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_command_button_ready_by_name(
        &self,
        team_name: &str,
        command_button_name: &str,
        all_ready: bool,
    ) -> Result<bool, ScriptError> {
        let team_arc = self.get_team_by_name(team_name)?;
        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(command_button_name)
        else {
            return Ok(false);
        };

        let members = team_arc
            .read()
            .map(|team| team.get_members().to_vec())
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read team".to_string()))?;

        for obj_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            let Some(is_ready) = self.command_button_ready_for_object(&obj_guard, command_button)
            else {
                continue;
            };

            if is_ready {
                if !all_ready {
                    return Ok(true);
                }
            } else if all_ready {
                return Ok(false);
            }
        }

        Ok(all_ready)
    }

    fn command_button_ready_for_object(
        &self,
        obj: &crate::object::Object,
        command_button: &crate::command_button::CommandButton,
    ) -> Option<bool> {
        if let Some(template) = command_button.get_special_power_template() {
            if !obj.has_special_power(template.get_special_power_type()) {
                return None;
            }
            return obj
                .with_special_power_module_interface_by_name(template.get_name(), |sp_module| {
                    sp_module.is_ready()
                })
                .or(Some(false));
        }

        let Some(upgrade) = command_button.get_upgrade_template() else {
            return None;
        };

        if upgrade.get_upgrade_type() == crate::upgrade::UpgradeType::Object {
            if obj.has_upgrade(upgrade) || !obj.affected_by_upgrade(upgrade) {
                return Some(false);
            }
        }

        if !obj.can_produce_upgrade(upgrade) {
            return Some(false);
        }

        let player_id = obj.get_controlling_player_id()?;
        let player_arc = {
            let list = player_list().read().ok()?;
            list.get_player(player_id as i32).cloned()?
        };
        let player_guard = player_arc.read().ok()?;

        if player_guard.has_upgrade_complete(upgrade)
            || player_guard.has_upgrade_in_production(upgrade)
        {
            return Some(false);
        }

        Some(true)
    }

    fn eval_skirmish_unowned_faction_unit_exists(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;

        let neutral_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player())
            .ok_or_else(|| ScriptError::ExecutionFailed("Neutral player not found".to_string()))?;
        let neutral_guard = neutral_player.read().map_err(|_| {
            ScriptError::ExecutionFailed("Failed to read neutral player".to_string())
        })?;
        let neutral_id = neutral_guard.get_player_index() as u32;

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().unwrap_or(u32::MAX) != neutral_id {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned)
                    {
                        count += 1;
                    }
                }
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => count < target_count,
            ComparisonType::LessEqual => count <= target_count,
            ComparisonType::Equal => count == target_count,
            ComparisonType::GreaterEqual => count >= target_count,
            ComparisonType::Greater => count > target_count,
            ComparisonType::NotEqual => count != target_count,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_player_has_prerequisite_to_build(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let object_type = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?;

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;

        let mut types = crate::object::object_types::ObjectTypes::new();
        let type_name = object_type.get_string();
        if !type_name.is_empty() {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    if let Some(found) = engine.get_object_types(type_name) {
                        types = found;
                    } else {
                        types.add_object_type(AsciiString::from(type_name));
                    }
                } else {
                    types.add_object_type(AsciiString::from(type_name));
                }
            } else {
                types.add_object_type(AsciiString::from(type_name));
            }
        }

        let can_build = types.can_build_any(&player_guard);
        Ok(if can_build {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_player_has_comparison_garrisoned(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;
        let player_id = player_guard.get_player_index() as u32;

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().unwrap_or(u32::MAX) != player_id {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let Some(contain) = obj_guard.get_contain() else {
                        continue;
                    };
                    let Ok(contain_guard) = contain.lock() else {
                        continue;
                    };
                    if contain_guard.is_garrisonable() && contain_guard.get_contained_count() > 0 {
                        count += 1;
                    }
                }
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => count < target_count,
            ComparisonType::LessEqual => count <= target_count,
            ComparisonType::Equal => count == target_count,
            ComparisonType::GreaterEqual => count >= target_count,
            ComparisonType::Greater => count > target_count,
            ComparisonType::NotEqual => count != target_count,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_player_has_comparison_captured_units(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;
        let player_id = player_guard.get_player_index() as u32;

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().unwrap_or(u32::MAX) != player_id {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_captured() {
                        count += 1;
                    }
                }
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => count < target_count,
            ComparisonType::LessEqual => count <= target_count,
            ComparisonType::Equal => count == target_count,
            ComparisonType::GreaterEqual => count >= target_count,
            ComparisonType::Greater => count > target_count,
            ComparisonType::NotEqual => count != target_count,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_named_area_exist(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ ignores parameter 0 here and uses the trigger-name parameter.
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!("Evaluating if skirmish named area '{}' exists", area_name);

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let exists = terrain.get_trigger_area_by_name(&area_name).is_some();

        Ok(if exists {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_player_has_units_in_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish player '{}' has units in area '{}'",
            player_name,
            area_name
        );

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(trigger) = terrain.get_trigger_area_by_name(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index();

        let mut any_changes = condition.custom_data == 0;
        if !any_changes {
            if let Ok(factory) = get_team_factory().lock() {
                for team_arc in factory.get_all_teams() {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    if team_guard.get_controlling_player_id().map(|id| id as i32)
                        != Some(player_index)
                    {
                        continue;
                    }
                    if team_guard.did_enter_or_exit() {
                        any_changes = true;
                        break;
                    }
                }
            }
        }

        if !any_changes {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    if engine.get_frame_object_count_changed() > condition.custom_frame {
                        any_changes = true;
                    }
                }
            }
        }

        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().map(|id| id as i32) != Some(player_index)
                {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let pos = obj_guard.get_position();
                    let point =
                        crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
                    if trigger.point_in_trigger_int(&point) {
                        if !(obj_guard.is_effectively_dead()
                            || obj_guard.is_kind_of(crate::common::KindOf::Inert)
                            || obj_guard.is_kind_of(crate::common::KindOf::Projectile))
                        {
                            count += 1;
                        }
                    }
                }
            }
        }

        let comparison = count > 0;
        condition.custom_data = if comparison { 1 } else { -1 };
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                condition.custom_frame = engine.get_frame_object_count_changed();
            }
        }

        Ok(if comparison {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_player_has_been_attacked_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: evaluateSkirmishPlayerHasBeenAttackedByPlayer(SIDE, SIDE)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let attacked_by_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish player '{}' has been attacked by '{}'",
            player_name,
            attacked_by_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(src_arc) = players.find_player_by_name(&attacked_by_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(src) = src_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let attacked = player.get_attacked_by(src.get_player_index() as i32);
        Ok(if attacked {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_skirmish_player_is_outside_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: return !evaluateSkirmishPlayerHasUnitsInArea(...)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        if players.find_player_by_name(&player_name).is_none() {
            return Ok(ScriptConditionResult::False);
        }
        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        if terrain.get_trigger_area_by_name(&area_name).is_none() {
            return Ok(ScriptConditionResult::False);
        }

        match self.eval_skirmish_player_has_units_in_area(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    fn eval_skirmish_player_has_discovered_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateSkirmishPlayerHasDiscoveredPlayer(SIDE, SIDE)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let discovered_by_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish player '{}' has been discovered by '{}'",
            player_name,
            discovered_by_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(discovered_by_arc) = players.find_player_by_name(&discovered_by_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(discovered_by) = discovered_by_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let player_index = player.get_player_index();
        let discovered_by_index = discovered_by.get_player_index();

        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().map(|id| id as i32) != Some(player_index)
                {
                    continue;
                }

                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let shroud_status = obj_guard.get_shrouded_status(discovered_by_index);
                    if matches!(
                        shroud_status,
                        crate::common::ObjectShroudStatus::Clear
                            | crate::common::ObjectShroudStatus::PartialClear
                    ) {
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }

        Ok(ScriptConditionResult::False)
    }

    // ============================================================================
    // AREA CONDITION HANDLERS
    // ============================================================================

    fn eval_player_has_comparison_unit_type_in_trigger_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;
        let type_name = self.get_condition_string_param(condition, 3)?;
        let trigger_name = self.get_condition_string_param(condition, 4)?;
        log::debug!(
            "Evaluating player '{}' has unit type '{}' in area '{}' {:?} {}",
            player_name,
            type_name,
            trigger_name,
            comparison,
            target_count
        );

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(trigger) = terrain.get_trigger_area_by_name(&trigger_name).cloned() else {
            return Ok(ScriptConditionResult::False);
        };
        drop(terrain);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index();
        let player_object_ids = player.get_all_objects();
        drop(player);
        drop(players);

        let mut any_changes = condition.custom_data == 0;
        if !any_changes {
            if let Ok(factory) = get_team_factory().lock() {
                for team_arc in factory.get_all_teams() {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    if team_guard.get_controlling_player_id().map(|id| id as i32)
                        != Some(player_index)
                    {
                        continue;
                    }
                    if team_guard.did_enter_or_exit() {
                        any_changes = true;
                        break;
                    }
                }
            }
        }
        if !any_changes {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    if engine.get_frame_object_count_changed() > condition.custom_frame {
                        any_changes = true;
                    }
                }
            }
        }
        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let types = self.resolve_object_types_param(&type_name);
        let mut count = 0i32;
        for object_id in player_object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !types.contains_template(Some(obj_guard.get_template().as_ref())) {
                continue;
            }
            let pos = obj_guard.get_position();
            let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
            if !trigger.point_in_trigger_int(&point) {
                continue;
            }

            // C++ includes crates even though they can be effectively dead/inert.
            let include = !(obj_guard.is_effectively_dead()
                || obj_guard.is_kind_of(crate::common::KindOf::Inert))
                || obj_guard.is_kind_of(crate::common::KindOf::Crate);
            if include {
                count += 1;
            }
        }

        let comparison_result = Self::compare_i32(comparison, count, target_count);
        condition.custom_data = if comparison_result { 1 } else { -1 };
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                condition.custom_frame = engine.get_frame_object_count_changed();
            }
        }

        Ok(if comparison_result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    fn eval_player_has_comparison_unit_kind_in_trigger_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;
        let kind_param = condition
            .get_parameter(3)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 3 not found".to_string()))?;
        let trigger_name = self.get_condition_string_param(condition, 4)?;
        log::debug!(
            "Evaluating player '{}' has kind '{}' in area '{}' {:?} {}",
            player_name,
            kind_param.get_int(),
            trigger_name,
            comparison,
            target_count
        );

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(trigger) = terrain.get_trigger_area_by_name(&trigger_name).cloned() else {
            return Ok(ScriptConditionResult::False);
        };
        drop(terrain);

        let kind = if kind_param.get_int() >= 0 {
            crate::common::ALL_KIND_OF
                .get(kind_param.get_int() as usize)
                .copied()
        } else {
            None
        }
        .or_else(|| parse_kind_of(kind_param.get_string()));
        let Some(kind) = kind else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index();
        let player_object_ids = player.get_all_objects();
        drop(player);
        drop(players);

        let mut any_changes = condition.custom_data == 0;
        if !any_changes {
            if let Ok(factory) = get_team_factory().lock() {
                for team_arc in factory.get_all_teams() {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    if team_guard.get_controlling_player_id().map(|id| id as i32)
                        != Some(player_index)
                    {
                        continue;
                    }
                    if team_guard.did_enter_or_exit() {
                        any_changes = true;
                        break;
                    }
                }
            }
        }
        if !any_changes {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    if engine.get_frame_object_count_changed() > condition.custom_frame {
                        any_changes = true;
                    }
                }
            }
        }
        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let mut count = 0i32;
        for object_id in player_object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !obj_guard.is_kind_of(kind) {
                continue;
            }
            let pos = obj_guard.get_position();
            let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
            if !trigger.point_in_trigger_int(&point) {
                continue;
            }
            if !(obj_guard.is_effectively_dead()
                || obj_guard.is_kind_of(crate::common::KindOf::Inert))
            {
                count += 1;
            }
        }

        let comparison_result = Self::compare_i32(comparison, count, target_count);

        // Match C++ behavior: this writes frame object count into custom_data (legacy quirk).
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                condition.custom_data = engine.get_frame_object_count_changed() as i32;
            }
        }

        Ok(if comparison_result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // PARAMETER HELPERS
    // ============================================================================

    fn resolve_object_types_param(&self, type_or_list_name: &str) -> ObjectTypes {
        let mut types = ObjectTypes::new();
        if type_or_list_name.is_empty() {
            return types;
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(found) = engine.get_object_types(type_or_list_name) {
                    return found;
                }
            }
        }

        types.add_object_type(AsciiString::from(type_or_list_name));
        types
    }

    fn get_condition_string_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<String, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| self.resolve_string_token(p.get_string()))
    }

    fn get_condition_int_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<i32, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_int())
    }

    fn get_condition_real_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<f32, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_real())
    }

    fn get_condition_bool_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<bool, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_int() != 0)
    }

    fn get_condition_comparison_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<ComparisonType, ScriptError> {
        let value = condition
            .get_parameter(index)
            .ok_or_else(|| {
                ScriptError::ParameterNotFound(format!("Parameter {} not found", index))
            })?
            .get_int();
        match value {
            0 => Ok(ComparisonType::LessThan),
            1 => Ok(ComparisonType::LessEqual),
            2 => Ok(ComparisonType::Equal),
            3 => Ok(ComparisonType::GreaterEqual),
            4 => Ok(ComparisonType::Greater),
            5 => Ok(ComparisonType::NotEqual),
            _ => Ok(ComparisonType::Equal),
        }
    }

    fn compare_i32(comparison: ComparisonType, lhs: i32, rhs: i32) -> bool {
        match comparison {
            ComparisonType::LessThan => lhs < rhs,
            ComparisonType::LessEqual => lhs <= rhs,
            ComparisonType::Equal => lhs == rhs,
            ComparisonType::GreaterEqual => lhs >= rhs,
            ComparisonType::Greater => lhs > rhs,
            ComparisonType::NotEqual => lhs != rhs,
        }
    }

    fn compare_f32(comparison: ComparisonType, lhs: f32, rhs: f32) -> bool {
        match comparison {
            ComparisonType::LessThan => lhs < rhs,
            ComparisonType::LessEqual => lhs <= rhs,
            ComparisonType::Equal => lhs == rhs,
            ComparisonType::GreaterEqual => lhs >= rhs,
            ComparisonType::Greater => lhs > rhs,
            ComparisonType::NotEqual => lhs != rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::LocomotorSetType;
    use crate::modules::AIUpdateInterface;
    use crate::object_manager::ObjectCreationFlags;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        commands: Arc<
            Mutex<
                Vec<(
                    AiCommandType,
                    Option<ObjectID>,
                    Option<String>,
                    i32,
                    CommandSourceType,
                )>,
            >,
        >,
        locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn choose_locomotor_set(
            &mut self,
            set: LocomotorSetType,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.locomotors.lock().unwrap().push(set);
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.commands.lock().unwrap().push((
                command.cmd,
                command.obj,
                command.team.clone(),
                command.int_value,
                command.cmd_source,
            ));
            Ok(())
        }
    }

    #[test]
    fn executor_named_attack_named_leaves_group_and_dispatches_force_attack() {
        get_object_manager().write().unwrap().reset();
        get_named_object_tracker().clear().unwrap();

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let attacker_id = 8450;
        let target_id = 8451;
        let attacker = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                attacker_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test attacker instance"),
        ));
        let target = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                target_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test target instance"),
        ));

        {
            let instance = attacker.write().unwrap();
            let mut base = instance.base.write().unwrap();
            base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                commands: Arc::clone(&commands),
                locomotors: Arc::clone(&locomotors),
            }))));
            base.enter_group(&crate::ai::AIGroup::new(91));
            assert_eq!(base.get_group_id(), Some(91));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(attacker.clone(), Coord3D::new(12.0, 4.0, 0.0))
            .unwrap();
        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(target, Coord3D::new(20.0, 4.0, 0.0))
            .unwrap();
        get_named_object_tracker()
            .register_named_object("ExecutorAttacker".to_string(), attacker_id)
            .unwrap();
        get_named_object_tracker()
            .register_named_object("ExecutorVictim".to_string(), target_id)
            .unwrap();

        let mut action = ScriptAction::new(ScriptActionType::NamedAttackNamed);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ExecutorAttacker".to_string(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ExecutorVictim".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_named_attack_named(&action).unwrap();

        assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
        assert_eq!(
            *commands.lock().unwrap(),
            vec![(
                AiCommandType::ForceAttackObject,
                Some(target_id),
                None,
                -1,
                CommandSourceType::FromScript,
            )]
        );
        assert_eq!(
            attacker.read().unwrap().base.read().unwrap().get_group_id(),
            None
        );
    }

    #[test]
    fn executor_team_attack_team_dispatches_attack_team() {
        get_object_manager().write().unwrap().reset();
        get_team_factory().lock().unwrap().reset();

        {
            let mut factory = get_team_factory().lock().unwrap();
            factory.init_team(
                AsciiString::from("ExecutorAttackers"),
                AsciiString::default(),
                false,
                None,
            );
            factory.init_team(
                AsciiString::from("ExecutorVictims"),
                AsciiString::default(),
                false,
                None,
            );
            factory
                .create_team("ExecutorAttackers")
                .expect("attacker team should be created");
            factory
                .create_team("ExecutorVictims")
                .expect("victim team should be created");
        }

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let attacker_id = 8460;
        let attacker = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                attacker_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test attacker instance"),
        ));

        {
            let instance = attacker.write().unwrap();
            instance
                .base
                .write()
                .unwrap()
                .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                    commands: Arc::clone(&commands),
                    locomotors: Arc::clone(&locomotors),
                }))));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(attacker, Coord3D::new(14.0, 4.0, 0.0))
            .unwrap();
        {
            let factory = get_team_factory();
            let mut factory_guard = factory.lock().unwrap();
            factory_guard
                .find_team("ExecutorAttackers")
                .unwrap()
                .write()
                .unwrap()
                .add_member(attacker_id);
        }

        let mut action = ScriptAction::new(ScriptActionType::TeamAttackTeam);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "ExecutorAttackers".to_string(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "ExecutorVictims".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_team_attack_team(&action).unwrap();

        assert_eq!(locomotors.lock().unwrap().len(), 0);
        assert_eq!(
            *commands.lock().unwrap(),
            vec![(
                AiCommandType::AttackTeam,
                None,
                Some("ExecutorVictims".to_string()),
                -1,
                CommandSourceType::FromScript,
            )]
        );
    }

    #[test]
    fn executor_named_attack_area_leaves_group_and_selects_normal_locomotor() {
        get_object_manager().write().unwrap().reset();
        get_named_object_tracker().clear().unwrap();
        get_terrain_logic().write().unwrap().reset();

        get_terrain_logic().write().unwrap().add_trigger_area(
            crate::polygon_trigger::PolygonTrigger::new(
                8470,
                AsciiString::from("ExecutorAttackArea"),
                vec![
                    crate::common::ICoord3D::new(0, 0, 0),
                    crate::common::ICoord3D::new(20, 0, 0),
                    crate::common::ICoord3D::new(20, 20, 0),
                    crate::common::ICoord3D::new(0, 20, 0),
                ],
            ),
        );

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let attacker_id = 8471;
        let attacker = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                attacker_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test attacker instance"),
        ));

        {
            let instance = attacker.write().unwrap();
            let mut base = instance.base.write().unwrap();
            base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                commands: Arc::clone(&commands),
                locomotors: Arc::clone(&locomotors),
            }))));
            base.enter_group(&crate::ai::AIGroup::new(92));
            assert_eq!(base.get_group_id(), Some(92));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(attacker.clone(), Coord3D::new(4.0, 4.0, 0.0))
            .unwrap();
        get_named_object_tracker()
            .register_named_object("ExecutorAreaAttacker".to_string(), attacker_id)
            .unwrap();

        let mut action = ScriptAction::new(ScriptActionType::NamedAttackArea);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ExecutorAreaAttacker".to_string(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::TriggerArea,
                "ExecutorAttackArea".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_named_attack_area(&action).unwrap();

        assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
        assert_eq!(
            commands.lock().unwrap()[0],
            (
                AiCommandType::AttackArea,
                None,
                None,
                0,
                CommandSourceType::FromScript,
            )
        );
        assert_eq!(
            attacker.read().unwrap().base.read().unwrap().get_group_id(),
            None
        );
    }

    #[test]
    fn executor_named_attack_team_validates_team_and_sets_max_shots() {
        get_object_manager().write().unwrap().reset();
        get_named_object_tracker().clear().unwrap();
        get_team_factory().lock().unwrap().reset();

        {
            let mut factory = get_team_factory().lock().unwrap();
            factory.init_team(
                AsciiString::from("ExecutorTargetTeam"),
                AsciiString::default(),
                false,
                None,
            );
            factory
                .create_team("ExecutorTargetTeam")
                .expect("target team should be created");
        }

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let attacker_id = 8480;
        let attacker = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                attacker_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test attacker instance"),
        ));

        {
            let instance = attacker.write().unwrap();
            let mut base = instance.base.write().unwrap();
            base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                commands: Arc::clone(&commands),
                locomotors: Arc::clone(&locomotors),
            }))));
            base.enter_group(&crate::ai::AIGroup::new(93));
            assert_eq!(base.get_group_id(), Some(93));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(attacker.clone(), Coord3D::new(8.0, 4.0, 0.0))
            .unwrap();
        get_named_object_tracker()
            .register_named_object("ExecutorTeamAttacker".to_string(), attacker_id)
            .unwrap();

        let mut action = ScriptAction::new(ScriptActionType::NamedAttackTeam);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ExecutorTeamAttacker".to_string(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "ExecutorTargetTeam".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_named_attack_team(&action).unwrap();

        assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
        assert_eq!(
            *commands.lock().unwrap(),
            vec![(
                AiCommandType::AttackTeam,
                None,
                Some("ExecutorTargetTeam".to_string()),
                -1,
                CommandSourceType::FromScript,
            )]
        );
        assert_eq!(
            attacker.read().unwrap().base.read().unwrap().get_group_id(),
            None
        );
    }

    #[test]
    fn executor_team_attack_named_ignores_stale_target_tracker_id() {
        get_object_manager().write().unwrap().reset();
        get_named_object_tracker().clear().unwrap();
        get_team_factory().lock().unwrap().reset();

        {
            let mut factory = get_team_factory().lock().unwrap();
            factory.init_team(
                AsciiString::from("ExecutorSourceTeam"),
                AsciiString::default(),
                false,
                None,
            );
            factory
                .create_team("ExecutorSourceTeam")
                .expect("source team should be created");
        }

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let member_id = 8490;
        let member = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                member_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test team member instance"),
        ));

        {
            let instance = member.write().unwrap();
            instance
                .base
                .write()
                .unwrap()
                .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                    commands: Arc::clone(&commands),
                    locomotors: Arc::clone(&locomotors),
                }))));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(member, Coord3D::new(8.0, 8.0, 0.0))
            .unwrap();
        get_team_factory()
            .lock()
            .unwrap()
            .find_team("ExecutorSourceTeam")
            .unwrap()
            .write()
            .unwrap()
            .add_member(member_id);
        get_named_object_tracker()
            .register_named_object("MissingExecutorVictim".to_string(), 8491)
            .unwrap();

        let mut action = ScriptAction::new(ScriptActionType::TeamAttackNamed);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "ExecutorSourceTeam".to_string(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "MissingExecutorVictim".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_team_attack_named(&action).unwrap();

        assert!(commands.lock().unwrap().is_empty());
        assert!(locomotors.lock().unwrap().is_empty());
    }

    #[test]
    fn executor_named_guard_leaves_group_selects_locomotor_and_sets_guard_mode() {
        get_object_manager().write().unwrap().reset();
        get_named_object_tracker().clear().unwrap();

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let guard_id = 8500;
        let guard = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                guard_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test guard instance"),
        ));

        {
            let instance = guard.write().unwrap();
            let mut base = instance.base.write().unwrap();
            base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                commands: Arc::clone(&commands),
                locomotors: Arc::clone(&locomotors),
            }))));
            base.enter_group(&crate::ai::AIGroup::new(94));
            assert_eq!(base.get_group_id(), Some(94));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(guard.clone(), Coord3D::new(9.0, 5.0, 0.0))
            .unwrap();
        get_named_object_tracker()
            .register_named_object("ExecutorGuard".to_string(), guard_id)
            .unwrap();

        let mut action = ScriptAction::new(ScriptActionType::NamedGuard);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ExecutorGuard".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_named_guard(&action).unwrap();

        assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
        assert_eq!(
            *commands.lock().unwrap(),
            vec![(
                AiCommandType::GuardPosition,
                None,
                None,
                GuardMode::Normal.as_i32(),
                CommandSourceType::FromScript,
            )]
        );
        assert_eq!(
            guard.read().unwrap().base.read().unwrap().get_group_id(),
            None
        );
    }

    #[test]
    fn executor_team_guard_dispatches_direct_ai_without_player_owner() {
        get_object_manager().write().unwrap().reset();
        get_team_factory().lock().unwrap().reset();

        {
            let mut factory = get_team_factory().lock().unwrap();
            factory.init_team(
                AsciiString::from("ExecutorGuardTeam"),
                AsciiString::default(),
                false,
                None,
            );
            factory
                .create_team("ExecutorGuardTeam")
                .expect("guard team should be created");
        }

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let member_id = 8510;
        let member = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                member_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test guard team member instance"),
        ));

        {
            let instance = member.write().unwrap();
            instance
                .base
                .write()
                .unwrap()
                .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                    commands: Arc::clone(&commands),
                    locomotors: Arc::clone(&locomotors),
                }))));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(member, Coord3D::new(10.0, 10.0, 0.0))
            .unwrap();
        get_team_factory()
            .lock()
            .unwrap()
            .find_team("ExecutorGuardTeam")
            .unwrap()
            .write()
            .unwrap()
            .add_member(member_id);

        let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "ExecutorGuardTeam".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_team_guard(&action).unwrap();

        assert!(locomotors.lock().unwrap().is_empty());
        assert_eq!(
            *commands.lock().unwrap(),
            vec![(
                AiCommandType::GuardPosition,
                None,
                None,
                GuardMode::Normal.as_i32(),
                CommandSourceType::FromScript,
            )]
        );
    }

    #[test]
    fn executor_team_guard_object_ignores_stale_target_tracker_id() {
        get_object_manager().write().unwrap().reset();
        get_named_object_tracker().clear().unwrap();
        get_team_factory().lock().unwrap().reset();

        {
            let mut factory = get_team_factory().lock().unwrap();
            factory.init_team(
                AsciiString::from("ExecutorObjectGuardTeam"),
                AsciiString::default(),
                false,
                None,
            );
            factory
                .create_team("ExecutorObjectGuardTeam")
                .expect("object guard team should be created");
        }

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
        let member_id = 8520;
        let member = Arc::new(RwLock::new(
            crate::object_manager::GameObjectInstance::new(
                member_id,
                None,
                None,
                ObjectCreationFlags::new(),
            )
            .expect("test object guard team member instance"),
        ));

        {
            let instance = member.write().unwrap();
            instance
                .base
                .write()
                .unwrap()
                .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                    commands: Arc::clone(&commands),
                    locomotors: Arc::clone(&locomotors),
                }))));
        }

        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(member, Coord3D::new(11.0, 11.0, 0.0))
            .unwrap();
        get_team_factory()
            .lock()
            .unwrap()
            .find_team("ExecutorObjectGuardTeam")
            .unwrap()
            .write()
            .unwrap()
            .add_member(member_id);
        get_named_object_tracker()
            .register_named_object("MissingGuardTarget".to_string(), 8521)
            .unwrap();

        let mut action = ScriptAction::new(ScriptActionType::TeamGuardObject);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "ExecutorObjectGuardTeam".to_string(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "MissingGuardTarget".to_string(),
            ))
            .unwrap();

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_team_guard_object(&action).unwrap();

        assert!(commands.lock().unwrap().is_empty());
        assert!(locomotors.lock().unwrap().is_empty());
    }
}
