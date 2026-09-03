//! Shared imports for the GameLogic facade split. Visibility is parent-only.
#![allow(unused_imports)]

pub(super) use super::super::mission_scripts::{
    CameoFlashRequest, CameraAddShakerRequest, CameraBwModeRequest,
    CameraLookTowardWaypointRequest, CameraModFinalSpeedMultiplierRequest,
    CameraModRollingAverageRequest, CameraMotionBlurRequest, CameraMoveToRequest,
    CameraPathRequest, CameraPitchRequest, CameraRotateRequest, CameraSetDefaultRequest,
    CameraSlaveModeRequest, CameraZoomRequest, MissionScriptActionHandler, MissionScriptHooks,
    NamedTimerMutation, RadarScriptEventRequest, ScreenShakeRequest, ScriptPopupMessageRequest,
    SetFpsLimitRequest, SuperweaponObjectDisplayMutation, ViewGuardbandRequest,
    VisualSpeedMultiplierRequest,
};
pub(super) use super::super::partition_manager::PartitionManager;
pub(super) use super::super::radar_notifications::{self, RadarEntry, RadarNotifications};
pub(super) use super::super::script_events::{self, ScriptEvent};
pub(super) use super::super::victory::{
    PlayerOutcome, PlayerResult, VictoryCondition, VictorySummary,
};
pub(super) use super::super::victory_conditions::{
    AllianceNotification, VictoryConditions, victory_rules_for_map,
};
pub(super) use super::super::*;
pub(super) use crate::ai::*;
pub(super) use crate::assets::{ObjectDefinition, get_asset_manager};
pub(super) use crate::localization;
pub(super) use crate::save_load::campaign::CampaignManager;
pub(super) use crate::save_load::campaign::MissionObjective;
pub(super) use crate::save_load::game_state::global_campaign_manager;
pub(super) use crate::ui::audio::translate_audio_event;
pub(super) use crate::ui::color_for_player;
pub(super) use crate::ui::objectives::{ObjectiveCategory, ObjectiveDisplay, ObjectiveStatus};
pub(super) use game_engine::common::dict::Dict;
pub(super) use game_engine::common::name_key_generator::NameKeyGenerator;
pub(super) use game_engine::common::rts::player_template::{
    get_player_template_store, try_get_player_template_store,
};
pub(super) use game_engine::common::system::build_assistant::get_build_assistant;
pub(super) use game_engine::common::well_known_keys::{
    key_multiplayer_start_index, key_player_allies, key_player_color, key_player_display_name,
    key_player_enemies, key_player_faction, key_player_is_human, key_player_is_skirmish,
    key_player_name, key_player_night_color, key_player_start_money, key_team_is_singleton,
    key_team_name, key_team_owner,
};
pub(super) use gamelogic::ai::the_ai;
pub(super) use gamelogic::ai::integration::{initialize_ai_integration, with_ai_integration_mut};
pub(super) use gamelogic::common::CommandSourceType;
pub(super) use gamelogic::modules::AIUpdateInterfaceExt;
pub(super) use gamelogic::player::{
    GameDifficulty as LogicGameDifficulty, Player as LogicPlayer, PlayerList as LogicPlayerList,
    PlayerTemplate as LogicPlayerTemplate, PlayerType as LogicPlayerType, ThePlayerList,
};
pub(super) use gamelogic::scripting::core::ScriptList;
pub(super) use gamelogic::scripting::engine::ScriptActionHandler;
pub(super) use gamelogic::scripting::{
    ScriptEvent as MissionScriptEvent, ScriptPriority, ScriptValue, ScriptingEngine,
};
pub(super) use gamelogic::sides_list::get_sides_list;
pub(super) use gamelogic::special_power_module::update as update_special_powers;
pub(super) use gamelogic::system::beacon_manager::snapshot_beacons;
pub(super) use gamelogic::system::game_logic::RadarEventType;
pub(super) use gamelogic::system::map_loader::MapLoader as LogicMapLoader;
pub(super) use gamelogic::system::radar_notifier;
pub(super) use gamelogic::system::shroud_manager::get_shroud_manager;
pub(super) use gamelogic::team::get_team_factory;
pub(super) use gamelogic::update_game_logic;
pub(super) use gamelogic::weapon::with_weapon_store_mut;
pub(super) use glam::{Vec2, Vec3};
pub(super) use std::collections::{HashMap, HashSet, VecDeque};
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
pub(super) use std::sync::{Arc, Mutex, OnceLock, RwLock};
pub(super) use std::time::{Duration, Instant, SystemTime};
pub(super) use ww3d_engine::FrameTiming;

pub(super) const SCRIPT_BROADCAST_DURATION: f32 = 6.0;
pub(super) const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
pub(super) const LOGIC_FRAME_TIMESTEP: f32 = 1.0 / LOGIC_FRAMES_PER_SECOND;
