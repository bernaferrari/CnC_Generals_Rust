#![allow(non_snake_case)]

/*
** Command & Conquer Generals Zero Hour(tm) - Game Logic System
** Copyright 2025 Electronic Arts Inc.
**
** Main GameLogic singleton - manages all objects, simulation, and game state
** Ported from GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
*/

use super::mission_scripts::{
    CameoFlashRequest, CameraAddShakerRequest, CameraBwModeRequest,
    CameraLookTowardWaypointRequest, CameraModFinalSpeedMultiplierRequest,
    CameraModRollingAverageRequest, CameraMotionBlurRequest, CameraMoveToRequest,
    CameraPathRequest, CameraPitchRequest, CameraRotateRequest, CameraSetDefaultRequest,
    CameraSlaveModeRequest, CameraZoomRequest, MissionScriptActionHandler, MissionScriptHooks,
    NamedTimerMutation, ScreenShakeRequest, ScriptPopupMessageRequest, SetFpsLimitRequest,
    SuperweaponObjectDisplayMutation, ViewGuardbandRequest, VisualSpeedMultiplierRequest,
};
use super::partition_manager::PartitionManager;
use super::radar_notifications::{self, RadarEntry, RadarNotifications};
use super::script_events::{self, ScriptEvent};
use super::victory::{PlayerOutcome, PlayerResult, VictoryCondition, VictorySummary};
use super::victory_conditions::{victory_rules_for_map, AllianceNotification, VictoryConditions};
use super::*;
use crate::ai::*;
use crate::assets::{get_asset_manager, ObjectDefinition};
use crate::localization;
use crate::save_load::campaign::CampaignManager;
use crate::save_load::campaign::MissionObjective;
use crate::save_load::game_state::global_campaign_manager;
use crate::ui::audio::translate_audio_event;
use crate::ui::color_for_player;
use crate::ui::objectives::{ObjectiveCategory, ObjectiveDisplay, ObjectiveStatus};
use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::get_player_template_store;
use game_engine::common::well_known_keys::{
    key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
};
use gamelogic::player::{
    GameDifficulty as LogicGameDifficulty, Player as LogicPlayer, PlayerList as LogicPlayerList,
    PlayerTemplate as LogicPlayerTemplate, PlayerType as LogicPlayerType, ThePlayerList,
};
use gamelogic::scripting::core::ScriptList;
use gamelogic::scripting::engine::ScriptActionHandler;
use gamelogic::scripting::{
    ScriptEvent as MissionScriptEvent, ScriptPriority, ScriptValue, ScriptingEngine,
};
use game_engine::common::system::build_assistant::get_build_assistant;
use gamelogic::sides_list::get_sides_list;
use gamelogic::special_power_module::update as update_special_powers;
use gamelogic::system::beacon_manager::snapshot_beacons;
use gamelogic::system::game_logic::RadarEventType;
use gamelogic::system::map_loader::MapLoader as LogicMapLoader;
use gamelogic::system::radar_notifier;
use gamelogic::system::shroud_manager::get_shroud_manager;
use gamelogic::team::get_team_factory;
use gamelogic::weapon::{update_dot_effects, update_projectiles, with_weapon_store_mut};
use gamelogic::ai::THE_AI;
use gamelogic::ai::integration::{initialize_ai_integration, with_ai_integration_mut};
use gamelogic::update_game_logic;
use glam::{Vec2, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use ww3d_engine::FrameTiming;

const SCRIPT_BROADCAST_DURATION: f32 = 6.0;
const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
const LOGIC_FRAME_TIMESTEP: f32 = 1.0 / LOGIC_FRAMES_PER_SECOND;
const SHELL_MISSION_SCRIPT_BUDGET: usize = 8;

/// Tick the gamelogic crate's full C++-parity update pipeline.
/// This runs AI players, production/build assistant, weapon store (delayed damage),
/// partition manager, death cleanup, locomotor store, victory conditions, and
/// disabled-status checks — all phases from C++ GameLogic::update().
pub fn tick_gamelogic_crate() -> Result<(), String> {
    update_game_logic()
}

/// AI command structure for parallel processing
#[derive(Debug)]
pub enum AICommand {
    AttackTarget {
        object_id: ObjectId,
        target_id: ObjectId,
    },
    StopAttack {
        object_id: ObjectId,
    },
    MoveTo {
        object_id: ObjectId,
        position: Vec3,
    },
    SetAIState {
        object_id: ObjectId,
        state: AIState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSpecialAbility {
    Hijack { target_id: ObjectId },
    Sabotage { target_id: ObjectId },
    CarBomb { target_id: ObjectId },
    SnipeVehicle { target_id: ObjectId },
}

impl PendingSpecialAbility {
    fn target_id(self) -> ObjectId {
        match self {
            PendingSpecialAbility::Hijack { target_id }
            | PendingSpecialAbility::Sabotage { target_id }
            | PendingSpecialAbility::CarBomb { target_id }
            | PendingSpecialAbility::SnipeVehicle { target_id } => target_id,
        }
    }
}

/// Global GameLogic singleton instance
static GAME_LOGIC: OnceLock<Arc<Mutex<GameLogic>>> = OnceLock::new();

/// Audio event request (mirrors C++ AudioEventRTS pattern)
/// These events are queued each frame and processed by the audio system
#[derive(Debug, Clone)]
pub struct AudioEventRequest {
    pub event_type: String,          // e.g., "WeaponFire", "UnitDie", "Explosion"
    pub object_id: Option<ObjectId>, // Source object
    pub position: Option<Vec3>,      // 3D world position
    pub priority: u8,                // 0-255 (higher = more important)
    pub is_looping: bool,            // false = fire-and-forget, true = continuous
}

impl AudioEventRequest {
    pub fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            object_id: None,
            position: None,
            priority: 128,
            is_looping: false,
        }
    }

    pub fn with_object(mut self, object_id: ObjectId) -> Self {
        self.object_id = Some(object_id);
        self
    }

    pub fn with_position(mut self, position: Vec3) -> Self {
        self.position = Some(position);
        self
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn looping(mut self) -> Self {
        self.is_looping = true;
        self
    }
}

/// Game mode types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    SinglePlayer,
    Skirmish,
    Multiplayer,
    Replay,
    Internet,
    Lan,
    Shell,
    None,
}

/// Fixed-step loop diagnostics used for shell/menu stall investigations.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixedStepDiagnostics {
    pub steps_run: usize,
    pub budget_hit: bool,
    pub accumulated_time_seconds: f32,
}

/// Aggregate player statistics for victory screen reporting.
#[derive(Debug, Clone, Default)]
pub struct PlayerStatistics {
    pub units_destroyed: u32,
    pub units_lost: u32,
    pub units_built: u32,
    pub structures_destroyed: u32,
    pub structures_lost: u32,
    pub structures_built: u32,
    pub resources_collected: u32,
    pub resources_spent: u32,
}

/// Player structure
#[derive(Debug, Clone)]
pub struct Player {
    pub id: u32,
    pub team: Team,
    pub name: String,
    pub resources: Resources,
    pub power_available: i32,
    /// Total power produced by this player's power plants (for energy ratio).
    pub power_produced: i32,
    /// Total power consumed by this player's buildings (for energy ratio).
    pub power_consumed: i32,
    pub income_accumulator: f32,
    pub selected_objects: Vec<ObjectId>,
    pub unlocked_sciences: HashSet<String>,
    pub queued_upgrades: HashSet<String>,
    pub is_local: bool,
    pub is_alive: bool,
    pub statistics: PlayerStatistics,
    /// Frame at which power sabotage expires (0 = not sabotaged).
    /// Matches C++ Player::m_powerSabotagedUntilFrame.
    pub power_sabotaged_till_frame: u32,
}

impl Player {
    /// C&C Generals default starting money is $10,000 (Normal difficulty).
    /// Matches the `StartingMoney::Normal` variant from the LAN API game-info crate.
    pub const DEFAULT_STARTING_MONEY: u32 = 10_000;

    pub fn new(id: u32, team: Team, name: &str, is_local: bool) -> Self {
        Self {
            id,
            team,
            name: name.to_string(),
            resources: Resources {
                supplies: Self::DEFAULT_STARTING_MONEY,
                power: 0,
            },
            power_available: 0,
            power_produced: 0,
            power_consumed: 0,
            income_accumulator: 0.0,
            selected_objects: Vec::new(),
            unlocked_sciences: HashSet::new(),
            queued_upgrades: HashSet::new(),
            is_local,
            is_alive: true,
            statistics: PlayerStatistics::default(),
            power_sabotaged_till_frame: 0,
        }
    }

    pub fn can_afford(&self, cost: &Resources) -> bool {
        self.resources.supplies >= cost.supplies && self.power_available + cost.power >= 0
    }

    pub fn spend_resources(&mut self, cost: &Resources) -> bool {
        if self.can_afford(cost) {
            self.resources.supplies -= cost.supplies;
            self.power_available += cost.power; // Negative for consumption
            if cost.supplies > 0 {
                self.record_resources_spent(cost.supplies);
            }
            true
        } else {
            false
        }
    }

    pub fn add_resources(&mut self, amount: &Resources) {
        self.resources.supplies += amount.supplies;
        // Power is calculated from buildings, not directly added
        if amount.supplies > 0 {
            self.statistics.resources_collected = self
                .statistics
                .resources_collected
                .saturating_add(amount.supplies);
        }
    }

    /// Queue an upgrade for this player when not already queued/completed and affordable.
    pub fn queue_upgrade(&mut self, upgrade_name: &str, cost: &Resources) -> bool {
        if self.unlocked_sciences.contains(upgrade_name)
            || self.queued_upgrades.contains(upgrade_name)
        {
            return false;
        }
        if !self.spend_resources(cost) {
            return false;
        }
        self.queued_upgrades.insert(upgrade_name.to_string());
        true
    }

    /// Cancel a queued upgrade and refund the requested resources.
    pub fn cancel_queued_upgrade(&mut self, upgrade_name: &str, refund: &Resources) -> bool {
        if !self.queued_upgrades.remove(upgrade_name) {
            return false;
        }
        self.resources.supplies = self.resources.supplies.saturating_add(refund.supplies);
        self.power_available -= refund.power;
        true
    }

    pub fn record_unit_destroyed(&mut self) {
        self.statistics.units_destroyed = self.statistics.units_destroyed.saturating_add(1);
    }

    pub fn record_unit_lost(&mut self) {
        self.statistics.units_lost = self.statistics.units_lost.saturating_add(1);
    }

    pub fn record_unit_produced(&mut self) {
        self.statistics.units_built = self.statistics.units_built.saturating_add(1);
    }

    pub fn record_structure_built(&mut self) {
        self.statistics.structures_built = self.statistics.structures_built.saturating_add(1);
    }

    pub fn record_structure_destroyed(&mut self) {
        self.statistics.structures_destroyed =
            self.statistics.structures_destroyed.saturating_add(1);
    }

    pub fn record_structure_lost(&mut self) {
        self.statistics.structures_lost = self.statistics.structures_lost.saturating_add(1);
    }

    pub fn record_resources_spent(&mut self, amount: u32) {
        self.statistics.resources_spent = self.statistics.resources_spent.saturating_add(amount);
    }
}

/// Main GameLogic system
pub struct GameLogic {
    /// Objects in the world
    pub objects: HashMap<ObjectId, Object>,

    /// Players in the game
    players: HashMap<u32, Player>,

    /// Object ID counter
    next_object_id: ObjectId,

    /// Simulation frame counter
    frame: u32,

    /// Game mode
    game_mode: GameMode,

    /// Game world dimensions
    world_width: f32,
    world_height: f32,
    world_min: Vec3,
    world_max: Vec3,

    /// Victory conditions subsystem (mirrors SAGE VictoryConditions)
    victory_conditions: VictoryConditions,

    /// Objects to destroy at end of frame
    objects_to_destroy: VecDeque<DestructionEvent>,

    /// Game paused state
    is_paused: bool,

    /// Time tracking
    sim_time_seconds: f32,
    accumulated_time: f32,
    last_fixed_step_diagnostics: FixedStepDiagnostics,

    /// Thing templates registry
    pub templates: HashMap<String, ThingTemplate>,

    /// Map data
    map_name: String,
    map_loaded: bool,

    /// Combat system for parallel projectile processing
    combat_system: CombatSystem,

    /// Pathfinding system for parallel path computation
    pathfinding_system: PathfindingSystem,

    /// AI Management System
    ai_manager: AIManager,

    /// Script execution tracking
    pub scripts_loaded: bool,
    pub mission_script_counter: u32,

    /// Audio events queued this frame (mirrors C++ TheAudio pattern)
    /// In production, these would be sent to the audio engine
    pub queued_audio_events: Vec<AudioEventRequest>,

    /// Command queue for UI-generated commands
    pub command_queue: VecDeque<crate::command_system::GameCommand>,
    pending_special_abilities: HashMap<ObjectId, PendingSpecialAbility>,

    /// Currently selected objects (used by UI)
    pub selected_objects: Vec<ObjectId>,

    partition_manager: PartitionManager,
    radar_notifications: &'static RadarNotifications,
    last_radar_kind_time: [f32; 3],
    last_radar_audio_time: f32,
    last_radar_event: Option<RadarEntry>,
    pending_camera_focus: Option<Vec3>,
    script_camera_focus_estimate: Vec3,
    script_camera_move_to: Option<ScriptCameraMoveTo>,
    script_camera_path: Option<ScriptCameraPathMove>,
    camera_follow_target: Option<ObjectId>,
    script_default_camera_pitch: f32,
    script_default_camera_angle: f32,
    script_default_camera_max_height: f32,
    script_camera_freeze_time_armed: bool,
    script_camera_freeze_angle_armed: bool,
    script_camera_pending_final_speed_multiplier: Option<f32>,
    script_camera_pending_rolling_average_frames: Option<i32>,
    visual_speed_multiplier: f32,
    script_time_frozen_by_script: bool,
    pending_script_fps_limit: Option<i32>,
    pending_camera_zoom_reset: bool,
    pending_camera_zoom: Option<CameraZoomRequest>,
    pending_camera_pitch: Option<CameraPitchRequest>,
    pending_camera_rotate: Option<CameraRotateRequest>,
    pending_camera_look_toward: Option<CameraLookTowardWaypointRequest>,
    pending_camera_slave_mode_enable: Option<CameraSlaveModeRequest>,
    pending_camera_slave_mode_disable: bool,
    pending_screen_shakes: Vec<ScreenShakeRequest>,
    pending_camera_add_shakers: Vec<CameraAddShakerRequest>,
    pending_popup_messages: Vec<ScriptPopupMessageRequest>,
    pending_view_guardband: Option<ViewGuardbandRequest>,
    pending_camera_bw_mode: Option<CameraBwModeRequest>,
    pending_camera_motion_blur: Vec<CameraMotionBlurRequest>,
    script_skybox_enabled: bool,
    script_cameo_flash_count: HashMap<String, i32>,
    script_named_timers: HashMap<String, (String, bool)>,
    script_named_timer_display_shown: bool,
    script_superweapon_display_enabled: bool,
    script_superweapon_hidden_objects: HashSet<ObjectId>,
    /// Beacon locations created this frame for HUD highlighting/bloom.
    recent_beacons: Vec<Vec3>,
    script_engine: Option<Arc<ScriptingEngine>>,
    script_event_pump_in_flight: Arc<AtomicBool>,
    script_event_pump_busy_frames: u32,
    loaded_script_lists: Vec<ScriptList>,
    script_source_path: Option<PathBuf>,
    mission_scripts: Arc<MissionScriptHooks>,
    script_broadcasts: Vec<ScriptBroadcast>,
    new_script_messages: Vec<String>,
    cinematic_letterbox: bool,
    cinematic_text: Option<(String, f32)>,
    military_caption: Option<(String, f32)>,
    radar_enabled: bool,
    pending_music_stop: bool,
    pending_movie: Option<String>,
    pending_radar_movie: Option<String>,
    mission_objectives: Vec<ObjectiveDisplay>,
    objective_lookup: HashMap<String, usize>,
    campaign_manager: Option<Arc<Mutex<CampaignManager>>>,
    last_map_settings: Option<super::script_loader::MapMetadata>,
    spawned_map_object_ids: Vec<(ObjectId, usize)>,
    terrain: Option<super::terrain::TerrainData>,
    pathfinding_height_samples: Option<PathfindingHeightSamples>,
    weather_state: RuntimeWeatherState,
}

#[derive(Debug, Clone)]
struct PathfindingHeightSamples {
    width: u32,
    height: u32,
    values: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct RuntimeWeatherState {
    pub current_weather: String,
    pub intensity: f32,
    pub duration_remaining: f32,
    pub next_change_time: f32,
    pub visible: bool,
}

impl Default for RuntimeWeatherState {
    fn default() -> Self {
        Self {
            current_weather: "clear".to_string(),
            intensity: 0.0,
            duration_remaining: 0.0,
            next_change_time: 0.0,
            visible: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ParabolicEase {
    in_t: f32,
    out_t: f32,
}

impl ParabolicEase {
    fn new(ease_in_time: f32, ease_out_time: f32) -> Self {
        let mut in_t = ease_in_time.clamp(0.0, 1.0);
        let out_t = 1.0 - ease_out_time.clamp(0.0, 1.0);
        if in_t > out_t {
            in_t = out_t;
        }
        Self { in_t, out_t }
    }

    fn eval(self, param: f32) -> f32 {
        let param = param.clamp(0.0, 1.0);
        let v0 = 1.0 + self.out_t - self.in_t;
        if param < self.in_t {
            if self.in_t <= 0.0 {
                0.0
            } else {
                param * param / (v0 * self.in_t)
            }
        } else if param <= self.out_t {
            (self.in_t + 2.0 * (param - self.in_t)) / v0
        } else {
            let denom = (1.0 - self.out_t).max(f32::EPSILON);
            (self.in_t
                + 2.0 * (self.out_t - self.in_t)
                + (2.0 * (param - self.out_t) + self.out_t * self.out_t - param * param) / denom)
                / v0
        }
    }
}

#[derive(Debug, Clone)]
struct ScriptCameraMoveTo {
    start: Vec3,
    target: Vec3,
    ease: ParabolicEase,
    total_time_seconds: f32,
    elapsed_seconds: f32,
    shutter_frames: u32,
    cur_shutter: u32,
    last_ease: f32,
    freeze_time: bool,
    freeze_angle: bool,
    speed_ramp_start_t: f32,
    speed_ramp_start_multiplier: f32,
    speed_ramp_final_multiplier: f32,
}

impl ScriptCameraMoveTo {
    fn new(start: Vec3, request: &CameraMoveToRequest) -> Self {
        let total_time_seconds = request.seconds.max(0.001);
        let ease_in = (request.ease_in_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease_out = (request.ease_out_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease = ParabolicEase::new(ease_in, ease_out);
        let shutter_frames = (request.camera_stutter_seconds * LOGIC_FRAMES_PER_SECOND).round() as u32;
        let shutter_frames = shutter_frames.max(1);
        Self {
            start,
            target: request.position,
            ease,
            total_time_seconds,
            elapsed_seconds: 0.0,
            shutter_frames,
            cur_shutter: shutter_frames,
            last_ease: 0.0,
            freeze_time: false,
            freeze_angle: false,
            speed_ramp_start_t: 0.0,
            speed_ramp_start_multiplier: 1.0,
            speed_ramp_final_multiplier: 1.0,
        }
    }

    fn is_finished(&self) -> bool {
        self.elapsed_seconds >= self.total_time_seconds
    }

    fn final_focus(&self) -> Vec3 {
        self.target
    }

    fn remaining_time_seconds(&self) -> f32 {
        (self.total_time_seconds - self.elapsed_seconds).max(0.0)
    }

    fn set_freeze_time(&mut self, freeze: bool) {
        self.freeze_time = freeze;
    }

    fn freeze_time(&self) -> bool {
        self.freeze_time
    }

    fn set_freeze_angle(&mut self, freeze: bool) {
        self.freeze_angle = freeze;
    }

    fn freeze_angle(&self) -> bool {
        self.freeze_angle
    }

    fn current_speed_multiplier(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= self.speed_ramp_start_t {
            return self.speed_ramp_start_multiplier;
        }
        let span = (1.0 - self.speed_ramp_start_t).max(f32::EPSILON);
        let t = ((progress - self.speed_ramp_start_t) / span).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier
            + (self.speed_ramp_final_multiplier - self.speed_ramp_start_multiplier) * t
    }

    fn set_final_speed_multiplier(&mut self, multiplier: f32) {
        if !multiplier.is_finite() {
            return;
        }
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier = self.current_speed_multiplier(progress);
        self.speed_ramp_start_t = progress;
        self.speed_ramp_final_multiplier = multiplier.max(0.0);
    }

    fn advance(&mut self, dt: f32) -> Option<Vec3> {
        let prev_ease = self.last_ease;
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let speed_multiplier = self.current_speed_multiplier(progress).max(0.0);
        self.elapsed_seconds =
            (self.elapsed_seconds + dt.max(0.0) * speed_multiplier).min(self.total_time_seconds);
        let t = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let next_ease = self.ease.eval(t);
        self.last_ease = next_ease;

        self.cur_shutter = self.cur_shutter.saturating_sub(1);
        if self.cur_shutter > 0 && next_ease > prev_ease {
            return None;
        }
        self.cur_shutter = self.shutter_frames;

        Some(self.start.lerp(self.target, next_ease))
    }
}

#[derive(Debug, Clone)]
struct ScriptCameraPathMove {
    points: Vec<Vec3>,
    segment_length: Vec<f32>,
    total_distance: f32,
    ease: ParabolicEase,
    total_time_seconds: f32,
    elapsed_seconds: f32,
    cur_segment: usize,
    cur_seg_distance: f32,
    shutter_frames: u32,
    cur_shutter: u32,
    last_ease: f32,
    freeze_time: bool,
    freeze_angle: bool,
    rolling_average_frames: i32,
    smoothed_focus: Option<Vec3>,
    speed_ramp_start_t: f32,
    speed_ramp_start_multiplier: f32,
    speed_ramp_final_multiplier: f32,
}

impl ScriptCameraPathMove {
    fn new(start_focus: Vec3, request: &CameraPathRequest) -> Option<Self> {
        let waypoint_name = gamelogic::common::AsciiString::from(&request.waypoint);
        let chain: Vec<Vec3> = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                let mut points = Vec::new();
                let mut current = terrain.get_waypoint_by_name(&waypoint_name)?;
                points.push(Vec3::new(
                    current.get_location().x,
                    0.0,
                    current.get_location().y,
                ));
                while let Some(next_id) = current.get_link(0) {
                    let next = terrain.get_waypoint_by_id(next_id)?;
                    points.push(Vec3::new(next.get_location().x, 0.0, next.get_location().y));
                    current = next;
                }
                Some(points)
            })
            .unwrap_or_default();

        if chain.is_empty() {
            return None;
        }

        let min_delta = gamelogic::common::MAP_XY_FACTOR;
        let mut points: Vec<Vec3> = Vec::with_capacity(chain.len() + 4);
        points.push(start_focus);
        points.push(start_focus);

        for p in chain {
            if let Some(last) = points.last().copied() {
                if Vec2::new(p.x - last.x, p.z - last.z).length() < min_delta {
                    continue;
                }
            }
            points.push(p);
        }

        if points.len() < 3 {
            return None;
        }

        // Pad start to allow spline interpolation like the original W3D view.
        let first = points[1];
        let second = points[2];
        points[0] = Vec3::new(
            first.x - (second.x - first.x),
            0.0,
            first.z - (second.z - first.z),
        );

        // Pad end one segment beyond last to keep interpolation stable.
        let last = *points.last().unwrap();
        let prev = points[points.len() - 2];
        points.push(Vec3::new(
            last.x + (last.x - prev.x),
            0.0,
            last.z + (last.z - prev.z),
        ));

        let last_meaningful = points.len() - 2;
        let mut segment_length = vec![0.0f32; points.len()];
        let mut total_distance = 0.0f32;

        for i in 1..last_meaningful {
            let a = points[i];
            let b = points[i + 1];
            let len = Vec2::new(b.x - a.x, b.z - a.z).length();
            segment_length[i] = len;
            total_distance += len;
        }

        if total_distance < 1.0 && last_meaningful >= 2 {
            let idx = last_meaningful - 1;
            segment_length[idx] += 1.0 - total_distance;
            total_distance = 1.0;
        }

        if last_meaningful >= 2 {
            segment_length[last_meaningful] = segment_length[last_meaningful - 1];
        }

        let total_time_seconds = request.seconds.max(0.001);
        let ease_in = (request.ease_in_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease_out = (request.ease_out_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease = ParabolicEase::new(ease_in, ease_out);

        let shutter_frames = (request.camera_stutter_seconds * LOGIC_FRAMES_PER_SECOND).round() as u32;
        let shutter_frames = shutter_frames.max(1);

        Some(Self {
            points,
            segment_length,
            total_distance,
            ease,
            total_time_seconds,
            elapsed_seconds: 0.0,
            cur_segment: 1,
            cur_seg_distance: 0.0,
            shutter_frames,
            cur_shutter: shutter_frames,
            last_ease: 0.0,
            freeze_time: false,
            freeze_angle: false,
            rolling_average_frames: 1,
            smoothed_focus: None,
            speed_ramp_start_t: 0.0,
            speed_ramp_start_multiplier: 1.0,
            speed_ramp_final_multiplier: 1.0,
        })
    }

    fn is_finished(&self) -> bool {
        self.elapsed_seconds >= self.total_time_seconds
    }

    fn final_focus(&self) -> Vec3 {
        let idx = self.points.len().saturating_sub(2);
        self.points.get(idx).copied().unwrap_or(Vec3::ZERO)
    }

    fn remaining_time_seconds(&self) -> f32 {
        (self.total_time_seconds - self.elapsed_seconds).max(0.0)
    }

    fn set_freeze_time(&mut self, freeze: bool) {
        self.freeze_time = freeze;
    }

    fn freeze_time(&self) -> bool {
        self.freeze_time
    }

    fn set_freeze_angle(&mut self, freeze: bool) {
        self.freeze_angle = freeze;
    }

    fn freeze_angle(&self) -> bool {
        self.freeze_angle
    }

    fn set_rolling_average_frames(&mut self, frames: i32) {
        self.rolling_average_frames = frames.max(1);
    }

    fn current_speed_multiplier(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= self.speed_ramp_start_t {
            return self.speed_ramp_start_multiplier;
        }
        let span = (1.0 - self.speed_ramp_start_t).max(f32::EPSILON);
        let t = ((progress - self.speed_ramp_start_t) / span).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier
            + (self.speed_ramp_final_multiplier - self.speed_ramp_start_multiplier) * t
    }

    fn set_final_speed_multiplier(&mut self, multiplier: f32) {
        if !multiplier.is_finite() {
            return;
        }
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier = self.current_speed_multiplier(progress);
        self.speed_ramp_start_t = progress;
        self.speed_ramp_final_multiplier = multiplier.max(0.0);
    }

    fn advance(&mut self, dt: f32) -> Option<Vec3> {
        let last_meaningful = self.points.len().saturating_sub(2);
        if last_meaningful <= 1 {
            return None;
        }

        let prev_ease = self.last_ease;
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let speed_multiplier = self.current_speed_multiplier(progress).max(0.0);
        self.elapsed_seconds =
            (self.elapsed_seconds + dt.max(0.0) * speed_multiplier).min(self.total_time_seconds);
        let t = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let next_ease = self.ease.eval(t);
        self.last_ease = next_ease;

        let delta = next_ease - prev_ease;
        self.cur_seg_distance += delta * self.total_distance;

        while self.cur_segment < last_meaningful
            && self.cur_seg_distance >= self.segment_length[self.cur_segment]
        {
            self.cur_seg_distance -= self.segment_length[self.cur_segment];
            self.cur_segment += 1;
            if self.cur_segment >= last_meaningful {
                return None;
            }
        }

        self.cur_shutter = self.cur_shutter.saturating_sub(1);
        if self.cur_shutter > 0 {
            return None;
        }
        self.cur_shutter = self.shutter_frames;

        let seg_len = self.segment_length[self.cur_segment].max(f32::EPSILON);
        let mut factor = (self.cur_seg_distance / seg_len).clamp(0.0, 1.0);

        let (start, mid, end) = if factor < 0.5 {
            let start = (self.points[self.cur_segment - 1] + self.points[self.cur_segment]) * 0.5;
            let mid = self.points[self.cur_segment];
            let end = (self.points[self.cur_segment] + self.points[self.cur_segment + 1]) * 0.5;
            factor += 0.5;
            (start, mid, end)
        } else {
            let start = (self.points[self.cur_segment] + self.points[self.cur_segment + 1]) * 0.5;
            let mid = self.points[self.cur_segment + 1];
            let end = (self.points[self.cur_segment + 1] + self.points[self.cur_segment + 2]) * 0.5;
            factor -= 0.5;
            (start, mid, end)
        };

        let p =
            start + (end - start) * factor + (mid - end + mid - start) * (1.0 - factor) * factor;
        let focus = Vec3::new(p.x, 0.0, p.z);
        let average_factor = 1.0 / self.rolling_average_frames.max(1) as f32;
        let smoothed = if let Some(previous) = self.smoothed_focus {
            previous + (focus - previous) * average_factor
        } else {
            focus
        };
        self.smoothed_focus = Some(smoothed);
        Some(smoothed)
    }
}

struct ScriptBroadcast {
    text: String,
    expires_at: f32,
}

fn localized_objective_string(id: &str, suffix: &str, fallback: &str) -> String {
    if id.is_empty() {
        return fallback.to_string();
    }
    let normalized = id.replace(' ', "_").to_ascii_lowercase();
    let key = format!("mission.objective.{normalized}.{suffix}");
    localization::localize(&key, fallback)
}

fn derive_objective_status(obj: &MissionObjective) -> (ObjectiveStatus, Option<(u32, u32)>) {
    if let Some(total) = obj.required_count {
        let current = obj.current_count.min(total);
        let status = if current >= total {
            ObjectiveStatus::Completed
        } else {
            ObjectiveStatus::Active
        };
        (status, Some((current, total)))
    } else {
        (ObjectiveStatus::Active, None)
    }
}

fn mission_objective_to_display(
    obj: &MissionObjective,
    category: ObjectiveCategory,
) -> ObjectiveDisplay {
    let id = obj.id.clone();
    let fallback_title = if obj.description.is_empty() {
        id.clone()
    } else {
        obj.description.clone()
    };
    let title = localized_objective_string(&id, "title", &fallback_title);
    let description = localized_objective_string(&id, "desc", "");
    let (status, progress) = derive_objective_status(obj);
    ObjectiveDisplay {
        id: if id.is_empty() { None } else { Some(id) },
        title,
        description,
        status,
        progress,
        category,
    }
}

impl GameLogic {
    fn seed_sample_objectives() -> Vec<ObjectiveDisplay> {
        vec![
            ObjectiveDisplay {
                id: Some("sample_primary".to_string()),
                title: localization::localize("objectives.primary.sample.title", "Secure the Area"),
                description: localization::localize(
                    "objectives.primary.sample.desc",
                    "Capture all nearby resource points.",
                ),
                status: ObjectiveStatus::Active,
                progress: Some((0, 3)),
                category: ObjectiveCategory::Primary,
            },
            ObjectiveDisplay {
                id: Some("sample_secondary".to_string()),
                title: localization::localize(
                    "objectives.secondary.sample.title",
                    "Bonus: Destroy Radar",
                ),
                description: localization::localize(
                    "objectives.secondary.sample.desc",
                    "Take out the enemy radar installation.",
                ),
                status: ObjectiveStatus::Completed,
                progress: None,
                category: ObjectiveCategory::Secondary,
            },
        ]
    }
}

#[derive(Debug)]
struct DestructionEvent {
    id: ObjectId,
    killer: Option<Team>,
}

impl Default for GameLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLogic {
    fn load_campaign_objectives(&self, map_name: &str) -> Vec<ObjectiveDisplay> {
        let Some(manager) = &self.campaign_manager else {
            return Self::seed_sample_objectives();
        };

        let Ok(guard) = manager.lock() else {
            log::warn!(
                "Campaign manager unavailable while loading objectives for '{}'",
                map_name
            );
            return Self::seed_sample_objectives();
        };

        let Some(mission) = guard
            .mission_definitions
            .values()
            .find(|info| info.map_name.eq_ignore_ascii_case(map_name))
        else {
            log::info!(
                "No campaign mission metadata found for map '{}'; using sample objectives",
                map_name
            );
            return Self::seed_sample_objectives();
        };

        let mut displays = Vec::new();
        for (category, list) in [
            (ObjectiveCategory::Primary, &mission.primary_objectives),
            (ObjectiveCategory::Secondary, &mission.secondary_objectives),
            (ObjectiveCategory::Bonus, &mission.bonus_objectives),
        ] {
            for obj in list.iter() {
                displays.push(mission_objective_to_display(obj, category));
            }
        }

        if displays.is_empty() {
            log::warn!(
                "Mission '{}' ({}) does not define objectives; falling back to samples",
                mission.name,
                mission.id
            );
            Self::seed_sample_objectives()
        } else {
            log::info!(
                "Loaded {} mission objectives for '{}' ({})",
                displays.len(),
                mission.name,
                mission.id
            );
            displays
        }
    }

    fn rebuild_objective_lookup(&mut self) {
        self.objective_lookup.clear();
        for (idx, objective) in self.mission_objectives.iter().enumerate() {
            if let Some(id) = &objective.id {
                self.objective_lookup.insert(id.to_ascii_lowercase(), idx);
            }
        }
    }

    fn with_objective_mut<F>(&mut self, objective_id: &str, mut f: F) -> bool
    where
        F: FnMut(&mut ObjectiveDisplay),
    {
        let key = objective_id.to_ascii_lowercase();
        if let Some(&index) = self.objective_lookup.get(&key) {
            if let Some(objective) = self.mission_objectives.get_mut(index) {
                f(objective);
                return true;
            }
        } else {
            log::debug!("Objective '{}' not found in current mission", objective_id);
        }
        false
    }

    pub fn set_objective_status(&mut self, objective_id: &str, status: ObjectiveStatus) -> bool {
        self.with_objective_mut(objective_id, |objective| objective.status = status)
    }

    pub fn set_objective_progress(
        &mut self,
        objective_id: &str,
        current: u32,
        total: Option<u32>,
    ) -> bool {
        self.with_objective_mut(objective_id, |objective| {
            objective.progress = total.map(|goal| (current.min(goal), goal));
        })
    }

    pub fn mark_objective_completed(&mut self, objective_id: &str) -> bool {
        self.set_objective_status(objective_id, ObjectiveStatus::Completed)
    }

    pub fn mark_objective_failed(&mut self, objective_id: &str) -> bool {
        self.set_objective_status(objective_id, ObjectiveStatus::Failed)
    }
}

impl GameLogic {
    fn script_engine_handle(&self) -> Option<Arc<ScriptingEngine>> {
        self.script_engine.as_ref().map(Arc::clone)
    }

    fn forward_event_to_scripts(&self, event: &ScriptEvent) {
        let engine = match self.script_engine_handle() {
            Some(engine) => engine,
            None => return,
        };

        let mission_event = match self.convert_script_event(event) {
            Some(evt) => evt,
            None => return,
        };

        if let Err(err) = engine.fire_event_sync(mission_event) {
            log::error!("Scripting engine failed to accept event: {}", err);
        }
    }

    pub fn new() -> Self {
        log::debug!("GameLogic::new() - creating new GameLogic instance");
        let world_width = 512.0;
        let world_height = 512.0;
        let world_min = Vec3::new(-world_width * 0.5, 0.0, -world_height * 0.5);
        let world_max = Vec3::new(world_width * 0.5, 0.0, world_height * 0.5);

        let mission_hooks = MissionScriptHooks::new().expect("Mission script runtime init failed");

        let mut instance = Self {
            objects: HashMap::new(),
            players: HashMap::new(),
            next_object_id: ObjectId(1), // Start at 1, 0 is invalid
            frame: 0,
            game_mode: GameMode::None,
            world_width,
            world_height,
            world_min,
            world_max,
            victory_conditions: VictoryConditions::new(),
            objects_to_destroy: VecDeque::new(),
            is_paused: false,
            sim_time_seconds: 0.0,
            accumulated_time: 0.0,
            last_fixed_step_diagnostics: FixedStepDiagnostics::default(),
            templates: HashMap::new(),
            map_name: String::new(),
            map_loaded: false,
            combat_system: CombatSystem::new(),
            pathfinding_system: PathfindingSystem::new_with_origin(
                world_min,
                world_width,
                world_height,
            ),
            ai_manager: AIManager::new(),
            scripts_loaded: false,
            mission_script_counter: 0,
            queued_audio_events: Vec::new(),
            command_queue: VecDeque::new(),
            pending_special_abilities: HashMap::new(),
            selected_objects: Vec::new(),
            partition_manager: PartitionManager::new(),
            radar_notifications: radar_notifications::global_radar_notifications(),
            last_radar_kind_time: [-10.0; 3],
            last_radar_audio_time: -10.0,
            last_radar_event: None,
            pending_camera_focus: None,
            script_camera_focus_estimate: Vec3::ZERO,
            script_camera_move_to: None,
            script_camera_path: None,
            camera_follow_target: None,
            script_default_camera_pitch: 1.0,
            script_default_camera_angle: 0.0,
            script_default_camera_max_height: 1.0,
            script_camera_freeze_time_armed: false,
            script_camera_freeze_angle_armed: false,
            script_camera_pending_final_speed_multiplier: None,
            script_camera_pending_rolling_average_frames: None,
            visual_speed_multiplier: 1.0,
            script_time_frozen_by_script: false,
            pending_script_fps_limit: None,
            pending_camera_zoom_reset: false,
            pending_camera_zoom: None,
            pending_camera_pitch: None,
            pending_camera_rotate: None,
            pending_camera_look_toward: None,
            pending_camera_slave_mode_enable: None,
            pending_camera_slave_mode_disable: false,
            pending_screen_shakes: Vec::new(),
            pending_camera_add_shakers: Vec::new(),
            pending_popup_messages: Vec::new(),
            pending_view_guardband: None,
            pending_camera_bw_mode: None,
            pending_camera_motion_blur: Vec::new(),
            script_skybox_enabled: true,
            script_cameo_flash_count: HashMap::new(),
            script_named_timers: HashMap::new(),
            script_named_timer_display_shown: true,
            script_superweapon_display_enabled: true,
            script_superweapon_hidden_objects: HashSet::new(),
            recent_beacons: Vec::new(),
            script_engine: None,
            script_event_pump_in_flight: Arc::new(AtomicBool::new(false)),
            script_event_pump_busy_frames: 0,
            loaded_script_lists: Vec::new(),
            script_source_path: None,
            mission_scripts: mission_hooks,
            script_broadcasts: Vec::new(),
            new_script_messages: Vec::new(),
            cinematic_letterbox: false,
            cinematic_text: None,
            military_caption: None,
            radar_enabled: true,
            pending_music_stop: false,
            pending_movie: None,
            pending_radar_movie: None,
            mission_objectives: Self::seed_sample_objectives(),
            objective_lookup: HashMap::new(),
            campaign_manager: global_campaign_manager().ok(),
            last_map_settings: None,
            spawned_map_object_ids: Vec::new(),
            terrain: None,
            pathfinding_height_samples: None,
            weather_state: RuntimeWeatherState::default(),
        };
        instance.rebuild_objective_lookup();
        instance
    }

    /// World bounds used for minimap/FOW projections.
    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        (self.world_min, self.world_max)
    }

    pub fn fixed_step_diagnostics(&self) -> FixedStepDiagnostics {
        self.last_fixed_step_diagnostics
    }

    /// Override world dimensions when terrain provides authoritative size.
    pub fn override_world_size(&mut self, width: f32, height: f32) {
        self.world_width = width;
        self.world_height = height;
        self.world_min = Vec3::new(-width * 0.5, 0.0, -height * 0.5);
        self.world_max = Vec3::new(width * 0.5, 0.0, height * 0.5);
        self.pathfinding_system = PathfindingSystem::new_with_origin(self.world_min, width, height);
    }

    /// Reset method - matching C++ GameLogic interface
    pub fn reset(&mut self) {
        log::debug!("GameLogic::reset() - resetting game state");
        self.objects.clear();
        self.players.clear();
        self.next_object_id = ObjectId(1);
        self.frame = 0;
        self.objects_to_destroy.clear();
        self.is_paused = false;
        self.sim_time_seconds = 0.0;
        self.accumulated_time = 0.0;
        self.last_fixed_step_diagnostics = FixedStepDiagnostics::default();
        self.map_loaded = false;
        self.victory_conditions.reset();
        self.scripts_loaded = false;
        self.script_event_pump_in_flight
            .store(false, Ordering::Release);
        self.script_event_pump_busy_frames = 0;
        self.loaded_script_lists.clear();
        self.script_source_path = None;
        self.mission_scripts.install_lists(&[]);
        self.script_broadcasts.clear();
        self.new_script_messages.clear();
        self.cinematic_letterbox = false;
        self.cinematic_text = None;
        self.military_caption = None;
        self.radar_enabled = true;
        self.pending_music_stop = false;
        self.pending_movie = None;
        self.pending_radar_movie = None;
        self.spawned_map_object_ids.clear();
        self.pending_special_abilities.clear();
        self.mission_objectives = Self::seed_sample_objectives();
        self.rebuild_objective_lookup();
        self.last_radar_event = None;
        self.last_radar_audio_time = -10.0;
        self.last_radar_kind_time = [-10.0; 3];
        self.pending_camera_focus = None;
        self.script_camera_focus_estimate = Vec3::ZERO;
        self.script_camera_move_to = None;
        self.script_camera_path = None;
        self.camera_follow_target = None;
        self.script_default_camera_pitch = 1.0;
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = 1.0;
        self.script_camera_freeze_time_armed = false;
        self.script_camera_freeze_angle_armed = false;
        self.script_camera_pending_final_speed_multiplier = None;
        self.script_camera_pending_rolling_average_frames = None;
        self.visual_speed_multiplier = 1.0;
        self.script_time_frozen_by_script = false;
        self.pending_script_fps_limit = None;
        self.pending_camera_zoom_reset = false;
        self.pending_camera_zoom = None;
        self.pending_camera_pitch = None;
        self.pending_camera_rotate = None;
        self.pending_camera_look_toward = None;
        self.pending_camera_slave_mode_enable = None;
        self.pending_camera_slave_mode_disable = false;
        self.pending_screen_shakes.clear();
        self.pending_camera_add_shakers.clear();
        self.pending_popup_messages.clear();
        self.pending_view_guardband = None;
        self.pending_camera_bw_mode = None;
        self.pending_camera_motion_blur.clear();
        self.script_skybox_enabled = true;
        self.script_cameo_flash_count.clear();
        self.script_named_timers.clear();
        self.script_named_timer_display_shown = true;
        self.script_superweapon_display_enabled = true;
        self.script_superweapon_hidden_objects.clear();
        self.recent_beacons.clear();
        self.terrain = None;
        self.pathfinding_height_samples = None;
        self.weather_state = RuntimeWeatherState::default();
        log::debug!("GameLogic::reset() complete");
    }

    pub fn weather_state(&self) -> &RuntimeWeatherState {
        &self.weather_state
    }

    pub fn set_weather_state(
        &mut self,
        current_weather: impl Into<String>,
        intensity: f32,
        duration_remaining: f32,
        next_change_time: f32,
    ) {
        let mut weather = current_weather.into();
        weather = weather.trim().to_string();
        if weather.is_empty() {
            weather = "clear".to_string();
        }

        self.weather_state.current_weather = weather;
        self.weather_state.intensity = intensity.clamp(0.0, 1.0);
        self.weather_state.duration_remaining = duration_remaining.max(0.0);
        self.weather_state.next_change_time = next_change_time.max(0.0);
    }

    pub fn set_weather_visible(&mut self, visible: bool) {
        self.weather_state.visible = visible;
    }

    pub fn queue_pending_special_ability(
        &mut self,
        object_id: ObjectId,
        ability: PendingSpecialAbility,
    ) {
        self.pending_special_abilities.insert(object_id, ability);
    }

    pub fn clear_pending_special_ability(&mut self, object_id: ObjectId) {
        self.pending_special_abilities.remove(&object_id);
    }

    pub fn terrain_height_at(&self, world_pos: Vec3) -> Option<f32> {
        #[cfg(feature = "game_client")]
        {
            return self.terrain.as_ref().map(|t| t.height_at_world(world_pos));
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = world_pos;
            None
        }
    }

    #[cfg(feature = "game_client")]
    pub fn terrain_heightmap_snapshot(&self) -> Option<game_client::terrain::height_map::HeightMap> {
        self.terrain.as_ref().map(|terrain| terrain.heightmap_clone())
    }

    /// Export terrain/pathing passability as a compact grid mask for save/load parity.
    pub fn snapshot_pathfinding_passability(&self) -> (u32, u32, Vec<bool>) {
        let width = self.pathfinding_system.grid.width().max(0) as u32;
        let height = self.pathfinding_system.grid.height().max(0) as u32;
        let mask = self.pathfinding_system.grid.export_static_block_mask();
        (width, height, mask)
    }

    /// Restore terrain/pathing passability from a saved grid mask.
    pub fn restore_pathfinding_passability(
        &mut self,
        width: u32,
        height: u32,
        mask: &[bool],
    ) -> bool {
        if width == 0 || height == 0 {
            return false;
        }

        self.pathfinding_system
            .grid
            .import_static_block_mask(width as i32, height as i32, mask)
    }

    /// Sample terrain heights into the current pathfinding grid resolution for save/load parity.
    pub fn snapshot_terrain_heights_for_path_grid(&self) -> Option<Vec<f32>> {
        #[cfg(feature = "game_client")]
        {
            let terrain = self.terrain.as_ref()?;
            let width = self.pathfinding_system.grid.width().max(0);
            let height = self.pathfinding_system.grid.height().max(0);
            if width == 0 || height == 0 {
                return None;
            }

            let grid_size = self.pathfinding_system.grid.grid_size();
            let origin = self.pathfinding_system.grid.origin();
            let mut samples = Vec::with_capacity((width * height) as usize);
            for y in 0..height {
                for x in 0..width {
                    let pos = Vec3::new(
                        origin.x + (x as f32 + 0.5) * grid_size,
                        0.0,
                        origin.z + (y as f32 + 0.5) * grid_size,
                    );
                    samples.push(terrain.height_at_world(pos));
                }
            }
            Some(samples)
        }
        #[cfg(not(feature = "game_client"))]
        {
            let cache = self.pathfinding_height_samples.as_ref()?;
            let width = self.pathfinding_system.grid.width().max(0) as u32;
            let height = self.pathfinding_system.grid.height().max(0) as u32;

            (cache.width == width && cache.height == height).then_some(cache.values.clone())
        }
    }

    /// Restore coarse terrain heights from a grid snapshot (used to recover post-load height queries).
    pub fn restore_terrain_heights_from_grid(
        &mut self,
        width: u32,
        height: u32,
        heights: &[f32],
    ) -> bool {
        let expected_len = (width as usize).saturating_mul(height as usize);
        if width == 0 || height == 0 || heights.len() != expected_len {
            return false;
        }

        self.pathfinding_height_samples = Some(PathfindingHeightSamples {
            width,
            height,
            values: heights.to_vec(),
        });

        #[cfg(feature = "game_client")]
        {
            let max_height = heights.iter().copied().fold(0.0_f32, f32::max).max(1.0_f32);
            let mut heightmap =
                game_client::terrain::height_map::HeightMap::new(width, height, max_height, 1.0);

            for (dst, src) in heightmap.heights.iter_mut().zip(heights.iter().copied()) {
                *dst = (src / max_height).clamp(0.0, 1.0);
            }

            let terrain = super::terrain::TerrainData::from_heightmap(
                heightmap,
                self.world_min,
                self.world_max,
                0,
            );
            self.terrain = Some(terrain);
            self.seed_pathfinding_from_terrain();
            true
        }
        #[cfg(not(feature = "game_client"))]
        {
            true
        }
    }

    pub fn set_pathfinding_static_block(&mut self, x: i32, y: i32, blocked: bool) {
        self.pathfinding_system
            .grid
            .set_blocked(super::pathfinding::GridPos::new(x, y), blocked);
    }

    pub fn is_pathfinding_static_blocked(&self, x: i32, y: i32) -> bool {
        self.pathfinding_system
            .grid
            .is_static_blocked(super::pathfinding::GridPos::new(x, y))
    }

    fn seed_pathfinding_from_terrain(&mut self) {
        #[cfg(feature = "game_client")]
        {
            let Some(terrain) = self.terrain.as_ref() else {
                return;
            };

            // Reset static blocks to the terrain-derived mask each map load.
            self.pathfinding_system.clear_static_blocks();

            // Coarse impassability heuristic:
            // - Keep units inside map bounds
            // - Treat very steep slopes as impassable
            //
            // This is intentionally conservative until the real SAGE passability layers (roads/water/cliffs)
            // are decoded from map + terrain INIs.
            const MAX_SLOPE: f32 = 1.0; // tan(theta) gradient threshold
            let grid_size = self.pathfinding_system.grid.grid_size();
            let grid_origin = self.pathfinding_system.grid.origin();

            let (min, max) = terrain.world_bounds();
            let min_x = min.x;
            let min_z = min.z;
            let max_x = max.x;
            let max_z = max.z;

            let width = self.pathfinding_system.grid.width();
            let height = self.pathfinding_system.grid.height();
            for y in 0..height {
                for x in 0..width {
                    let center = Vec3::new(
                        grid_origin.x + (x as f32 + 0.5) * grid_size,
                        0.0,
                        grid_origin.z + (y as f32 + 0.5) * grid_size,
                    );

                    if center.x < min_x || center.x > max_x || center.z < min_z || center.z > max_z
                    {
                        self.pathfinding_system
                            .grid
                            .set_blocked(super::pathfinding::GridPos::new(x, y), true);
                        continue;
                    }

                    let slope = terrain.slope_at_world(center);
                    if slope > MAX_SLOPE {
                        self.pathfinding_system
                            .grid
                            .set_blocked(super::pathfinding::GridPos::new(x, y), true);
                    }
                }
            }
        }
    }

    pub fn assign_unit_path(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> bool {
        let (start, can_move) = match self.objects.get(&unit_id) {
            Some(unit) => (unit.get_position(), unit.can_move()),
            None => return false,
        };
        if !can_move {
            return false;
        }

        let mut goals: Vec<Vec3> = waypoints.to_vec();
        goals.push(destination);

        let mut full_path: Vec<Vec3> = Vec::new();
        let mut segment_start = start;
        for goal in goals {
            if segment_start.distance(goal) < 0.1 {
                segment_start = goal;
                continue;
            }

            let segment = self
                .pathfinding_system
                .find_path(segment_start, goal, &self.objects);

            match segment {
                Some(mut segment_path) => {
                    if let Some(first) = segment_path.first_mut() {
                        *first = segment_start;
                    }
                    if !full_path.is_empty()
                        && !segment_path.is_empty()
                        && full_path
                            .last()
                            .is_some_and(|prev| prev.distance(segment_path[0]) < 0.01)
                    {
                        segment_path.remove(0);
                    }
                    full_path.extend(segment_path);
                }
                None => {
                    log::debug!(
                        "No path found for unit {:?} from {:?} to {:?}; falling back to direct segment",
                        unit_id,
                        segment_start,
                        goal
                    );
                    if full_path.is_empty() {
                        full_path.push(segment_start);
                    }
                    full_path.push(goal);
                }
            }

            segment_start = goal;
        }

        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        unit.movement.path = full_path;
        unit.movement.current_path_index = 0;
        unit.movement.target_position = Some(destination);
        unit.ai_state = AIState::Moving;
        unit.status.moving = true;
        true
    }

    pub fn append_unit_waypoint(&mut self, unit_id: ObjectId, waypoint: Vec3) -> bool {
        let (unit_pos, current_path, can_move) = match self.objects.get(&unit_id) {
            Some(unit) => (
                unit.get_position(),
                unit.movement.path.clone(),
                unit.can_move(),
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }

        let last_goal = current_path.last().copied().unwrap_or(unit_pos);

        let segment = self
            .pathfinding_system
            .find_path(last_goal, waypoint, &self.objects);

        let mut appended = current_path;
        match segment {
            Some(mut segment_path) => {
                if let Some(first) = segment_path.first_mut() {
                    *first = last_goal;
                }
                if !appended.is_empty()
                    && !segment_path.is_empty()
                    && appended
                        .last()
                        .is_some_and(|prev| prev.distance(segment_path[0]) < 0.01)
                {
                    segment_path.remove(0);
                }
                appended.extend(segment_path);
            }
            None => {
                log::debug!(
                    "No path found for unit {:?} from {:?} to {:?}; falling back to direct segment",
                    unit_id,
                    last_goal,
                    waypoint
                );
                if appended.is_empty() {
                    appended.push(last_goal);
                }
                appended.push(waypoint);
            }
        }

        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        unit.movement.path = appended;
        unit.movement.target_position = Some(waypoint);
        unit.ai_state = AIState::Moving;
        unit.status.moving = true;
        true
    }

    /// Update method - matching C++ GameLogic interface
    pub fn update(&mut self) {
        self.step_simulation(LOGIC_FRAME_TIMESTEP, None);
    }

    /// C++ interface methods
    pub fn isInGame(&self) -> bool {
        self.game_mode != GameMode::None && self.map_loaded
    }

    pub fn isInShellGame(&self) -> bool {
        self.game_mode == GameMode::Shell
    }

    pub fn isInReplayGame(&self) -> bool {
        self.game_mode == GameMode::Replay
    }

    pub fn isInMultiplayerGame(&self) -> bool {
        self.game_mode == GameMode::Multiplayer
    }

    pub fn isGamePaused(&self) -> bool {
        self.is_paused
    }

    pub fn clearGameData(&mut self) {
        log::debug!("GameLogic::clearGameData() - clearing all game data");
        self.objects.clear();
        self.players.clear();
        self.frame = 0;
        self.game_mode = GameMode::None;
        self.map_loaded = false;
        self.objects_to_destroy.clear();
    }

    pub fn getFrame(&self) -> u32 {
        self.frame
    }

    pub fn last_parsed_map_settings(&self) -> Option<super::script_loader::MapMetadata> {
        self.last_map_settings.clone()
    }

    pub fn is_skybox_enabled(&self) -> bool {
        self.script_skybox_enabled
    }

    /// Convenience accessor for any heightmap path hint parsed from the map.
    pub fn heightmap_hint(&self) -> Option<PathBuf> {
        self.last_map_settings
            .as_ref()
            .and_then(|m| m.heightmap_path.clone())
    }

    /// Return a representative base position for the given team (e.g., command center/structure).
    pub fn team_base_position(&self, team: Team) -> Option<Vec3> {
        // Prefer structures that look like command centers.
        for obj in self.objects.values() {
            if obj.team != team {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure)
                && obj.name.to_ascii_lowercase().contains("commandcenter")
            {
                return Some(obj.get_position().clone());
            }
        }
        // Fallback to any structure.
        for obj in self.objects.values() {
            if obj.team == team && obj.is_kind_of(KindOf::Structure) {
                return Some(obj.get_position().clone());
            }
        }
        // Finally, any object owned by the team.
        self.objects
            .values()
            .find(|o| o.team == team)
            .map(|o| o.get_position().clone())
    }

    /// Initialize the GameLogic singleton
    pub fn initialize() -> GameLogic {
        // For the engine, return a new instance as requested by the original code
        GameLogic::new()
    }

    /// Get reference to the GameLogic singleton
    pub fn instance() -> Arc<Mutex<GameLogic>> {
        GAME_LOGIC
            .get_or_init(|| Arc::new(Mutex::new(GameLogic::new())))
            .clone()
    }

    /// Initialize the global GameLogic singleton
    pub fn init_global() {
        let _ = GAME_LOGIC.get_or_init(|| Arc::new(Mutex::new(GameLogic::new())));
    }

    /// Start a new game with specified mode
    pub fn start_new_game(&mut self, mode: GameMode) {
        log::info!("Starting new game: {:?}", mode);
        self.reset();
        self.game_mode = mode;
        self.setup_templates();
        self.create_default_players();
        log::info!("New game started successfully");
    }

    pub fn game_mode(&self) -> GameMode {
        self.game_mode
    }

    fn team_from_string(name: &str) -> Option<Team> {
        let normalized = name.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "usa" | "us" | "america" => Some(Team::USA),
            "gla" => Some(Team::GLA),
            "china" => Some(Team::China),
            "neutral" => Some(Team::Neutral),
            _ if normalized.contains("usa") || normalized.contains("america") => Some(Team::USA),
            _ if normalized.contains("gla") => Some(Team::GLA),
            _ if normalized.contains("china") => Some(Team::China),
            _ if normalized.contains("neutral") || normalized.contains("civilian") => {
                Some(Team::Neutral)
            }
            _ => None,
        }
    }

    fn sync_legacy_runtime_from_chunky(&mut self, map_path: &Path, map_bytes: &[u8]) {
        let sync_started = Instant::now();
        let mut loader = LogicMapLoader::new();
        log::info!("Legacy runtime sync started for '{}'", map_path.display());
        if loader.load_runtime_support_from_bytes(map_bytes).is_err() {
            log::warn!(
                "Legacy GameLogic map load failed for '{}'",
                map_path.display()
            );
            return;
        }
        log::info!(
            "Legacy runtime support parse finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );

        let map_data = loader.to_map_data();

        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            terrain.reset();
            terrain.load_map_data(map_data);
        }
        log::info!(
            "Legacy terrain sync finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );

        self.sync_legacy_player_list_from_sides();
        log::info!(
            "Legacy player-list sync finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );
        self.sync_legacy_team_factory_from_sides();
        log::info!(
            "Legacy team-factory sync finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );

        let waypoint_count = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|terrain| {
                let mut count = 0usize;
                let mut current = terrain.get_first_waypoint();
                while let Some(waypoint) = current {
                    count += 1;
                    current = waypoint.get_next();
                }
                count
            })
            .unwrap_or(0);
        let team_count = get_team_factory()
            .lock()
            .map(|factory| factory.get_all_teams().len())
            .unwrap_or(0);

        log::info!(
            "Legacy runtime sync complete for '{}': waypoints={}, live_teams={}",
            map_path.display(),
            waypoint_count,
            team_count
        );
    }

    fn sync_legacy_runtime_from_fast_chunky(
        &mut self,
        map_path: &Path,
        chunky: &super::script_loader::ChunkyMap,
    ) {
        let sync_started = Instant::now();
        log::info!(
            "Fast legacy runtime sync started for '{}'",
            map_path.display()
        );

        let heightmap = match super::script_loader::parse_heightmap_data_from_chunky(chunky) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Fast legacy runtime sync heightmap parse failed for '{}': {}",
                    map_path.display(),
                    err
                );
                None
            }
        };
        let (waypoints, waypoint_links) =
            match super::script_loader::parse_runtime_waypoints_from_chunky(chunky) {
                Ok(value) => value,
                Err(err) => {
                    log::warn!(
                        "Fast legacy runtime sync waypoint parse failed for '{}': {}",
                        map_path.display(),
                        err
                    );
                    (Vec::new(), Vec::new())
                }
            };
        let sides_data = match super::script_loader::parse_runtime_sides_from_chunky(chunky) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Fast legacy runtime sync sides parse failed for '{}': {}",
                    map_path.display(),
                    err
                );
                super::script_loader::RuntimeSidesData::default()
            }
        };

        if let Some(heightmap) = heightmap {
            let map_data = gamelogic::system::map_loader::MapData {
                width: heightmap.width.max(0) as u32,
                height: heightmap.height.max(0) as u32,
                heightmap: heightmap.data,
                water_height: None,
                bridges: Vec::new(),
                texture_tiles: Vec::new(),
                boundaries: heightmap
                    .boundaries
                    .into_iter()
                    .map(|(x, y)| gamelogic::common::ICoord2D::new(x, y))
                    .collect(),
                border_size: heightmap.border_size,
                polygon_triggers: Vec::new(),
                waypoints: waypoints
                    .iter()
                    .map(|waypoint| gamelogic::system::map_loader::MapWaypoint {
                        id: waypoint.id,
                        name: waypoint.name.clone(),
                        location: gamelogic::system::map_loader::Coord3D::new(
                            waypoint.location.x,
                            waypoint.location.y,
                            waypoint.location.z,
                        ),
                        path_label1: waypoint.path_label1.clone(),
                        path_label2: waypoint.path_label2.clone(),
                        path_label3: waypoint.path_label3.clone(),
                        bi_directional: waypoint.bi_directional,
                    })
                    .collect(),
                waypoint_links,
            };

            if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
                terrain.reset();
                terrain.load_map_data(map_data);
            }
        }

        self.sync_legacy_player_list_from_side_dicts(&sides_data.side_dicts);
        self.sync_legacy_team_factory_from_team_dicts(&sides_data.team_dicts);

        let waypoint_count = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|terrain| {
                let mut count = 0usize;
                let mut current = terrain.get_first_waypoint();
                while let Some(waypoint) = current {
                    count += 1;
                    current = waypoint.get_next();
                }
                count
            })
            .unwrap_or(0);
        let team_count = get_team_factory()
            .lock()
            .map(|factory| factory.get_all_teams().len())
            .unwrap_or(0);

        log::info!(
            "Fast legacy runtime sync complete for '{}': waypoints={}, live_teams={}, elapsed={:.2}s",
            map_path.display(),
            waypoint_count,
            team_count,
            sync_started.elapsed().as_secs_f32()
        );
    }

    fn sync_legacy_player_list_from_side_dicts(&self, side_dicts: &[Dict]) {
        let mut logic_list = LogicPlayerList::new();

        for (index, dict) in side_dicts.iter().enumerate() {
            let player_name = dict.get_ascii_string(key_player_name());
            let faction = dict.get_ascii_string(key_player_faction());
            let display_name = dict.get_unicode_string(key_player_display_name());
            let is_human = dict.get_bool(key_player_is_human());

            // Keep player-template store locking narrow so Player::init can lazily hydrate
            // templates without deadlocking on the same global RwLock.
            let template_from_store = {
                let store = get_player_template_store();
                store
                    .find_template(&faction)
                    .map(LogicPlayerTemplate::from_common)
            };
            let template = template_from_store.unwrap_or_else(|| {
                let mut template = LogicPlayerTemplate::new(player_name.clone());
                template.side = faction.clone();
                template.base_side = faction.clone();
                template.display_name = if display_name.is_empty() {
                    player_name.clone()
                } else {
                    display_name.clone()
                };
                template
            });

            let mut player = LogicPlayer::new(index as i32);
            if !player_name.is_empty() {
                player.set_player_name_key(NameKeyGenerator::name_to_key(&player_name));
            }
            player.set_display_name(if display_name.is_empty() {
                if player_name.is_empty() {
                    "Neutral".to_string()
                } else {
                    player_name.clone()
                }
            } else {
                display_name
            });
            player.set_side(faction.clone());
            player.set_base_side(faction);
            player.set_difficulty(LogicGameDifficulty::Normal);

            let player_type = if player_name.is_empty() {
                LogicPlayerType::Neutral
            } else if is_human {
                LogicPlayerType::Human
            } else {
                LogicPlayerType::Computer
            };
            player.set_player_type(player_type, false);
            player.init(Arc::new(template));
            player.init_from_dict_defaults();

            logic_list.add_player(Arc::new(RwLock::new(player)));

            if is_human && logic_list.get_local_player_index() < 0 {
                logic_list.set_local_player_index(index as i32);
            }
        }

        if let Ok(mut guard) = ThePlayerList().write() {
            *guard = logic_list;
        }
    }

    fn sync_legacy_player_list_from_sides(&self) {
        let sides_list = get_sides_list();
        let Ok(sides_guard) = sides_list.read() else {
            return;
        };

        let side_dicts: Vec<Dict> = (0..sides_guard.get_num_sides())
            .filter_map(|index| {
                sides_guard
                    .get_side_info(index)
                    .map(|side| side.get_dict().clone())
            })
            .collect();
        self.sync_legacy_player_list_from_side_dicts(&side_dicts);
    }

    fn sync_legacy_team_factory_from_team_dicts(&self, team_dicts: &[Dict]) {
        let Ok(mut team_factory) = get_team_factory().lock() else {
            return;
        };
        team_factory.reset();

        for dict in team_dicts {
            let team_name =
                dict.get_ascii_string(game_engine::common::well_known_keys::key_team_name());
            if team_name.is_empty() {
                continue;
            }

            let owner =
                dict.get_ascii_string(game_engine::common::well_known_keys::key_team_owner());
            let singleton =
                dict.get_bool(game_engine::common::well_known_keys::key_team_is_singleton());

            let _ = team_factory.init_team(
                team_name.clone().into(),
                owner.clone().into(),
                singleton,
                Some(dict),
            );

            let team = team_factory
                .find_team(&team_name)
                .or_else(|| team_factory.create_team(&team_name));

            let Some(team_arc) = team else {
                log::warn!("Failed to instantiate legacy team '{}'", team_name);
                continue;
            };

            if let Ok(mut team_guard) = team_arc.write() {
                if !owner.is_empty() {
                    if let Ok(player_list) = ThePlayerList().read() {
                        if let Some(player_arc) = player_list.find_player_by_name(&owner) {
                            if let Ok(player_guard) = player_arc.read() {
                                team_guard.set_controlling_player_id(Some(
                                    player_guard.get_player_index() as u32,
                                ));
                            }
                        }
                    }
                }
                if singleton {
                    team_guard.set_active();
                }
            };
        }
    }

    fn sync_legacy_team_factory_from_sides(&self) {
        let sides_list = get_sides_list();
        let Ok(sides_guard) = sides_list.read() else {
            return;
        };

        let team_dicts: Vec<Dict> = (0..sides_guard.get_num_teams())
            .filter_map(|index| {
                sides_guard
                    .get_team_info(index)
                    .map(|team| team.get_dict().clone())
            })
            .collect();
        self.sync_legacy_team_factory_from_team_dicts(&team_dicts);
    }

    fn sync_named_shell_object_into_legacy_runtime(
        &self,
        object: &super::script_loader::PlacedObject,
    ) {
        if self.game_mode != GameMode::Shell {
            return;
        }

        let Some(name) = object
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            return;
        };

        let tracker = gamelogic::scripting::engine::get_named_object_tracker();
        if tracker.get_object_id(name).ok().flatten().is_some() {
            return;
        }

        let team_arc = object.team_name.as_deref().and_then(|team_name| {
            gamelogic::team::get_team_factory()
                .lock()
                .ok()
                .and_then(|mut factory| factory.find_team(team_name))
        });

        let terrain_height = self
            .terrain_height_at(Vec3::new(object.position.x, 0.0, object.position.y))
            .unwrap_or(0.0);
        let position = gamelogic::common::Coord3D::new(
            object.position.x,
            object.position.y,
            object.position.z + terrain_height,
        );

        let object_id = match gamelogic::object_manager::get_object_manager().write() {
            Ok(mut manager) => match manager.create_object(
                object.template.as_str(),
                position,
                team_arc,
                gamelogic::object_manager::ObjectCreationFlags::from_template(),
            ) {
                Ok(id) => id,
                Err(err) => {
                    log::warn!(
                        "Failed to mirror named shell object '{}' ({}) into legacy runtime: {}",
                        name,
                        object.template,
                        err
                    );
                    return;
                }
            },
            Err(_) => {
                log::warn!(
                    "Failed to lock GameLogic object manager while mirroring named shell object '{}'",
                    name
                );
                return;
            }
        };

        if let Some(obj_arc) = gamelogic::helpers::TheGameLogic::find_object_by_id(object_id) {
            if let Ok(mut obj) = obj_arc.write() {
                obj.set_name(gamelogic::common::AsciiString::from(name));
            }
        }

        if let Err(err) = tracker.register_named_object(name.to_string(), object_id) {
            log::warn!(
                "Failed to register mirrored shell object '{}' -> {}: {}",
                name,
                object_id,
                err
            );
        }
    }

    fn ground_loaded_map_objects_to_terrain(
        &mut self,
        objects: &[super::script_loader::PlacedObject],
        spawned_object_ids: &[(ObjectId, usize)],
    ) {
        if self.terrain.is_none() || spawned_object_ids.is_empty() {
            return;
        }

        let mut grounded_positions = Vec::with_capacity(spawned_object_ids.len());
        for &(_, index) in spawned_object_ids {
            let object = &objects[index];
            let ground_height = self
                .terrain_height_at(Vec3::new(object.position.x, 0.0, object.position.y))
                .unwrap_or(0.0);
            grounded_positions.push((
                index,
                object.position.x,
                object.position.z + ground_height,
                object.position.y,
            ));
        }

        for ((object_id, _), (_, x, y, z)) in spawned_object_ids
            .iter()
            .copied()
            .zip(grounded_positions.into_iter())
        {
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.set_position(Vec3::new(x, y, z));
            }
        }

        if self.game_mode != GameMode::Shell {
            return;
        }

        let tracker = gamelogic::scripting::engine::get_named_object_tracker();
        for &(_, index) in spawned_object_ids {
            let object = &objects[index];
            let Some(name) = object
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
            else {
                continue;
            };
            let ground_height = self
                .terrain_height_at(Vec3::new(object.position.x, 0.0, object.position.y))
                .unwrap_or(0.0);
            let grounded_position = gamelogic::common::Coord3D::new(
                object.position.x,
                object.position.y,
                object.position.z + ground_height,
            );
            let Some(object_id) = tracker.get_object_id(name).ok().flatten() else {
                continue;
            };
            let Some(object_arc) =
                gamelogic::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let write_result = object_arc.write();
            if let Ok(mut object_guard) = write_result {
                let _ = object_guard.set_position(&grounded_position);
            }
        }
    }

    /// Load a map with optional milestone progress reporting.
    pub fn load_map_with_progress<F>(&mut self, map_name: &str, mut report_progress: F) -> bool
    where
        F: FnMut(f32, &str),
    {
        report_progress(0.26, "Preparing map data");
        log::info!("Loading map: {}", map_name);
        let load_started = Instant::now();
        self.map_name = map_name.to_string();
        self.pathfinding_height_samples = None;
        self.configure_victory_rules_for_map(map_name);
        self.scripts_loaded = false;
        self.script_event_pump_in_flight
            .store(false, Ordering::Release);
        self.script_event_pump_busy_frames = 0;
        self.loaded_script_lists.clear();
        self.script_source_path = None;
        self.mission_scripts.install_lists(&[]);
        self.script_broadcasts.clear();
        self.new_script_messages.clear();
        self.pending_popup_messages.clear();
        self.pending_view_guardband = None;
        self.pending_camera_bw_mode = None;
        self.pending_camera_motion_blur.clear();
        self.script_skybox_enabled = true;
        self.script_cameo_flash_count.clear();
        self.script_named_timers.clear();
        self.script_named_timer_display_shown = true;
        self.script_superweapon_display_enabled = true;
        self.script_superweapon_hidden_objects.clear();
        self.mission_objectives = self.load_campaign_objectives(map_name);
        self.rebuild_objective_lookup();

        // Try to locate the real map file so scripts and future terrain loaders have a source.
        report_progress(0.30, "Resolving map resources");
        let resolved_map = super::script_loader::find_map_file(map_name);
        if let Some(path) = &resolved_map {
            log::info!("Resolved map '{}' to '{}'", map_name, path.display());
            if let Some(chunks) = super::script_loader::inspect_map_chunks(map_name) {
                log::debug!(
                    "Map '{}' contains chunky sections: {}",
                    map_name,
                    chunks.join(", ")
                );
            }
            if let Ok(Some(chunky)) = super::script_loader::load_chunky_map(map_name) {
                report_progress(0.34, "Parsing map chunks");
                log::info!(
                    "Map '{}' parsed: {} TOC entries, body offset {} bytes",
                    map_name,
                    chunky.toc.len(),
                    chunky.body_offset
                );
                if self.game_mode != GameMode::Shell {
                    report_progress(0.40, "Syncing runtime objects");
                } else {
                    report_progress(0.40, "Syncing shell runtime");
                }
                let sync_started = Instant::now();
                self.sync_legacy_runtime_from_fast_chunky(path, &chunky);
                log::info!(
                    "Map '{}' legacy runtime sync finished in {:.2}s (fast path)",
                    map_name,
                    sync_started.elapsed().as_secs_f32()
                );

                let heightmap_started = Instant::now();
                report_progress(0.46, "Parsing terrain heightmap");
                let heightmap_data =
                    super::script_loader::parse_heightmap_data_from_chunky(&chunky)
                        .ok()
                        .flatten();
                log::info!(
                    "Map '{}' heightmap parse finished in {:.2}s (present={})",
                    map_name,
                    heightmap_started.elapsed().as_secs_f32(),
                    heightmap_data.is_some()
                );

                // Replace the test map with parsed object placements for basic fidelity.
                let settings_started = Instant::now();
                report_progress(0.52, "Reading map settings");
                let parsed = super::script_loader::parse_map_settings(map_name);
                let parsed_settings = parsed.ok();
                log::info!(
                    "Map '{}' settings parse finished in {:.2}s (present={})",
                    map_name,
                    settings_started.elapsed().as_secs_f32(),
                    parsed_settings.is_some()
                );
                if let Some(meta) = parsed_settings.as_ref() {
                    log::info!(
                        "Map '{}' metadata: objects={}, heightmap_hint={:?}, world_min={:?}, world_max={:?}",
                        map_name,
                        meta.objects.len(),
                        meta.heightmap_path,
                        meta.world_min,
                        meta.world_max
                    );
                    let objects = &meta.objects;
                    if !objects.is_empty() {
                        let named_count = objects.iter().filter(|obj| obj.name.is_some()).count();
                        if named_count > 0 {
                            log::info!(
                                "Map '{}' contains {} named object placements",
                                map_name,
                                named_count
                            );
                        }
                        let object_spawn_started = Instant::now();
                        report_progress(0.58, "Spawning world objects");
                        self.objects.clear();
                        // Build a mapping from map-defined player IDs to teams.
                        let mut map_player_to_team: HashMap<u32, Team> = HashMap::new();
                        for obj in objects {
                            if let Some(pid) = obj.player_id {
                                if let Some(team) =
                                    obj.team_name.as_deref().and_then(Self::team_from_string)
                                {
                                    map_player_to_team.entry(pid).or_insert(team);
                                }
                            }
                        }
                        // Seed players from map ownership if present.
                        if !map_player_to_team.is_empty() {
                            self.players.clear();
                            for (&pid, &team) in &map_player_to_team {
                                let is_local = pid == 0;
                                let name = format!("Player{}", pid + 1);
                                self.players
                                    .insert(pid, Player::new(pid, team, &name, is_local));
                            }
                        }

                        let mut spawned_object_ids: Vec<(ObjectId, usize)> = Vec::new();
                        let total_objects = objects.len().max(1) as f32;
                        for (index, obj) in objects.iter().enumerate() {
                            if index % 4 == 0 {
                                let t = (index as f32 / total_objects).clamp(0.0, 1.0);
                                report_progress(0.58 + t * 0.20, "Spawning world objects");
                            }
                            let team = obj
                                .team_name
                                .as_deref()
                                .and_then(Self::team_from_string)
                                .unwrap_or_else(|| {
                                    obj.player_id
                                        .and_then(|pid| map_player_to_team.get(&pid).cloned())
                                        .unwrap_or(Team::Neutral)
                                });
                            let mut spawn_position =
                                Vec3::new(obj.position.x, obj.position.z, obj.position.y);
                            if let Some(ground_height) = self.terrain_height_at(Vec3::new(
                                spawn_position.x,
                                0.0,
                                spawn_position.z,
                            )) {
                                // Match C++ map-object placement: map z-offset sits on top of terrain.
                                spawn_position.y += ground_height;
                            }
                            if let Some(id) =
                                self.create_object(obj.template.as_str(), team, spawn_position)
                            {
                                spawned_object_ids.push((id, index));
                                self.sync_named_shell_object_into_legacy_runtime(obj);
                                if let Some(rot) = obj.rotation {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_orientation(rot);
                                    }
                                }
                                if let Some(upgrade) = obj.upgrade.as_deref() {
                                    // ObjectCreationList encodes upgrade/facing hints in a freeform string.
                                    // Apply all upgrades separated by commas/semicolons and treat a numeric-only
                                    // token as a facing override if the chunk omitted rotation.
                                    let mut applied_facing = false;
                                    for token in upgrade.split(&[',', ';'][..]) {
                                        let trimmed = token.trim();
                                        if trimmed.is_empty() {
                                            continue;
                                        }
                                        if !applied_facing && obj.rotation.is_none() {
                                            if let Ok(angle) = trimmed.parse::<f32>() {
                                                if let Some(created) = self.objects.get_mut(&id) {
                                                    created.set_orientation(angle);
                                                }
                                                applied_facing = true;
                                                continue;
                                            }
                                        }
                                        self.apply_upgrade_to_object(id, trimmed);
                                    }
                                }
                            }
                        }
                        report_progress(0.80, "World objects spawned");
                        self.spawned_map_object_ids = spawned_object_ids;
                        report_progress(0.82, "Finalizing world objects");
                        self.ensure_non_shell_player_presence(parsed_settings.as_ref());
                        log::info!(
                            "Spawned {} objects from map placement data for '{}' in {:.2}s",
                            self.objects.len(),
                            map_name,
                            object_spawn_started.elapsed().as_secs_f32()
                        );
                    }
                    self.last_map_settings = Some(meta.clone());
                }
                let bounds_started = Instant::now();
                report_progress(0.84, "Building world bounds");
                let mut bounds_override = parsed_settings.as_ref().and_then(|m| {
                    m.world_min.zip(m.world_max).map(|(min, max)| {
                        (
                            Vec3::new(min.x, min.y, min.z),
                            Vec3::new(max.x, max.y, max.z),
                        )
                    })
                });
                if let Some((min, max)) = bounds_override {
                    let extent_x = (max.x - min.x).abs();
                    let extent_z = (max.z - min.z).abs();
                    if extent_x < 1.0 || extent_z < 1.0 {
                        log::warn!(
                            "Map '{}' reported degenerate bounds ({:.2}x{:.2}); deriving bounds from terrain/object data",
                            map_name,
                            extent_x,
                            extent_z
                        );
                        bounds_override = None;
                    }
                }
                if bounds_override.is_none() {
                    if let Some(hm) = heightmap_data.as_ref() {
                        use gamelogic::common::MAP_XY_FACTOR;
                        let playable_w = (hm.width - 2 * hm.border_size).max(1) as f32;
                        let playable_h = (hm.height - 2 * hm.border_size).max(1) as f32;
                        bounds_override = Some((
                            Vec3::new(0.0, 0.0, 0.0),
                            Vec3::new(playable_w * MAP_XY_FACTOR, 0.0, playable_h * MAP_XY_FACTOR),
                        ));
                    }
                }
                if bounds_override.is_none() && !self.objects.is_empty() {
                    // Derive bounds from placed objects when map metadata is missing.
                    let mut min = Vec3::splat(f32::MAX);
                    let mut max = Vec3::splat(f32::MIN);
                    for obj in self.objects.values() {
                        let pos = obj.get_position();
                        min.x = min.x.min(pos.x);
                        min.y = min.y.min(pos.y);
                        min.z = min.z.min(pos.z);
                        max.x = max.x.max(pos.x);
                        max.y = max.y.max(pos.y);
                        max.z = max.z.max(pos.z);
                    }
                    // Add a small margin to keep camera from clipping edges.
                    let margin = 50.0;
                    min -= Vec3::splat(margin);
                    max += Vec3::splat(margin);
                    bounds_override = Some((min, max));
                }

                if let Some((min, max)) = bounds_override {
                    self.world_min = min;
                    self.world_max = max;
                    self.world_width = (self.world_max.x - self.world_min.x).max(1.0);
                    self.world_height = (self.world_max.z - self.world_min.z).max(1.0);
                    self.pathfinding_system = PathfindingSystem::new_with_origin(
                        self.world_min,
                        self.world_width,
                        self.world_height,
                    );
                    log::info!(
                        "Map '{}' bounds set to min({:.1},{:.1},{:.1}) max({:.1},{:.1},{:.1})",
                        map_name,
                        self.world_min.x,
                        self.world_min.y,
                        self.world_min.z,
                        self.world_max.x,
                        self.world_max.y,
                        self.world_max.z
                    );

                    #[cfg(feature = "game_client")]
                    if let Some(hm) = heightmap_data.as_ref() {
                        use gamelogic::common::MAP_HEIGHT_SCALE;
                        let width = hm.width.max(1) as u32;
                        let height = hm.height.max(1) as u32;
                        if hm.data.len() == (width * height) as usize {
                            let max_height = 255.0 * MAP_HEIGHT_SCALE;
                            let mut heightmap = game_client::terrain::height_map::HeightMap::new(
                                width, height, max_height, 1.0,
                            );
                            heightmap.heights = hm.data.iter().map(|h| *h as f32 / 255.0).collect();
                            self.terrain = Some(super::terrain::TerrainData::from_heightmap(
                                heightmap,
                                self.world_min,
                                self.world_max,
                                hm.border_size.max(0) as u32,
                            ));
                            if let Some(meta) = self.last_map_settings.clone() {
                                let spawned_map_object_ids = self.spawned_map_object_ids.clone();
                                self.ground_loaded_map_objects_to_terrain(
                                    &meta.objects,
                                    &spawned_map_object_ids,
                                );
                            }
                            self.seed_pathfinding_from_terrain();
                        }
                    }
                } else {
                    // Default symmetrical bounds based on current width/height.
                    self.world_min =
                        Vec3::new(-self.world_width * 0.5, 0.0, -self.world_height * 0.5);
                    self.world_max =
                        Vec3::new(self.world_width * 0.5, 0.0, self.world_height * 0.5);
                    self.pathfinding_system = PathfindingSystem::new_with_origin(
                        self.world_min,
                        self.world_width,
                        self.world_height,
                    );
                }

                if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                    shroud_mgr.init_shroud_grid(self.world_width, self.world_height);
                }
                report_progress(0.88, "Initializing shroud and pathfinding");
                log::info!(
                    "Map '{}' bounds/terrain/shroud hookup finished in {:.2}s",
                    map_name,
                    bounds_started.elapsed().as_secs_f32()
                );
            } else {
                log::error!(
                    "Map '{}' was found at '{}' but could not be decoded as a chunky map",
                    map_name,
                    path.display()
                );
                return false;
            }
        } else {
            // Development-only fallback maps: keep the legacy test layout for demos.
            if matches!(map_name, "TestMap" | "demo_map") {
                log::warn!(
                    "Map '{}' not found on disk; using built-in test layout",
                    map_name
                );
                self.create_test_map();
            } else {
                log::warn!("Map '{}' not found on disk", map_name);
                return false;
            }
        }

        // Terrain hookup: if a heightmap path was discovered next to the map, load it for height
        // queries and derive a first-pass impassability mask for the pathfinding grid.
        #[cfg(feature = "game_client")]
        {
            if self.terrain.is_none() {
                if let Some(heightmap_path) = self.heightmap_hint() {
                    if let Some(path_str) = heightmap_path.to_str() {
                        let loaded = if path_str.ends_with(".hmp") {
                            game_client::terrain::height_map::HeightMap::load_hmp(path_str).ok()
                        } else if path_str.ends_with(".tga") {
                            game_client::terrain::height_map::HeightMap::load_tga(path_str).ok()
                        } else if path_str.ends_with(".raw") {
                            game_client::terrain::height_map::HeightMap::load_raw(path_str).ok()
                        } else {
                            None
                        };

                        if let Some(heightmap) = loaded {
                            let terrain = super::terrain::TerrainData::from_heightmap(
                                heightmap,
                                self.world_min,
                                self.world_max,
                                0,
                            );
                            self.terrain = Some(terrain);
                            if let Some(meta) = self.last_map_settings.clone() {
                                let spawned_map_object_ids = self.spawned_map_object_ids.clone();
                                self.ground_loaded_map_objects_to_terrain(
                                    &meta.objects,
                                    &spawned_map_object_ids,
                                );
                            }
                            self.seed_pathfinding_from_terrain();
                        } else {
                            log::warn!("Failed to load heightmap '{}'", path_str);
                        }
                    }
                }
            }
        }

        let scripts_started = Instant::now();
        report_progress(0.92, "Initializing mission scripts");
        self.initialize_scripts(map_name);
        log::info!(
            "Map '{}' script init finished in {:.2}s",
            map_name,
            scripts_started.elapsed().as_secs_f32()
        );

        self.map_loaded = true;
        report_progress(0.96, "Map load complete");
        log::info!(
            "Map loaded successfully in {:.2}s",
            load_started.elapsed().as_secs_f32()
        );
        true
    }

    /// Load a map without external progress reporting.
    pub fn load_map(&mut self, map_name: &str) -> bool {
        self.load_map_with_progress(map_name, |_progress, _phase| {})
    }

    /// Main update loop with delta time
    pub fn update_with_dt(&mut self, dt: f32) {
        self.step_simulation(dt, None);
    }

    pub fn update_with_timing(&mut self, timing: &FrameTiming) {
        self.step_simulation(timing.delta_seconds(), Some(timing.total_seconds()));
    }

    /// Menu/shell update path that bounds fixed-step catch-up work per frame.
    /// This prevents multi-second UI stalls after startup while still advancing shell scripts.
    pub fn update_shell_with_budget(&mut self, dt: f32, max_fixed_steps: usize) {
        self.step_simulation_with_budget(dt, None, Some(max_fixed_steps.max(1)));
    }

    fn step_simulation(&mut self, delta_time: f32, absolute_time: Option<f32>) {
        self.step_simulation_with_budget(delta_time, absolute_time, None);
    }

    fn step_simulation_with_budget(
        &mut self,
        delta_time: f32,
        absolute_time: Option<f32>,
        max_fixed_steps: Option<usize>,
    ) {
        if self.is_paused {
            return;
        }

        self.accumulated_time += delta_time;

        const FIXED_TIMESTEP: f32 = LOGIC_FRAME_TIMESTEP;

        let mut steps_run = 0usize;
        let mut budget_hit = false;
        while self.accumulated_time >= FIXED_TIMESTEP {
            if let Some(step_budget) = max_fixed_steps {
                if steps_run >= step_budget {
                    budget_hit = true;
                    break;
                }
            }
            self.update_simulation(FIXED_TIMESTEP);
            self.accumulated_time -= FIXED_TIMESTEP;
            self.frame += 1;
            self.sim_time_seconds += FIXED_TIMESTEP;
            steps_run += 1;
        }

        if let Some(total_seconds) = absolute_time {
            self.sim_time_seconds = total_seconds.max(self.sim_time_seconds);
        }

        self.last_fixed_step_diagnostics = FixedStepDiagnostics {
            steps_run,
            budget_hit,
            accumulated_time_seconds: self.accumulated_time,
        };

        self.process_destroy_list();
    }

    /// Execute one simulation step.
    ///
    /// Phase ordering follows C++ GameLogic::update() (GameLogic.cpp lines 3548-3803)
    /// as documented in gamelogic::system::game_logic::GameLogic::update():
    ///
    /// ```text
    /// Line 3595: setFrame / sync to GameClient       [frame setup]
    /// Line 3600: TheScriptEngine->UPDATE()            [early scripting]
    /// Line 3603: freezeTime check
    /// Line 3622: TheTerrainLogic->UPDATE()            [terrain/bridges]
    /// Line 3669: processCommandList                   [command processing]
    /// Line 3672: ALLOW_NONSLEEPY_UPDATES loop         [normal modules]
    /// Line 3697: sleepy updates loop                  [sleepy modules]
    /// Line 3743: TheAI->UPDATE()                      [AI]
    /// Line 3748: TheBuildAssistant->UPDATE()          [production]
    /// Line 3753: ThePartitionManager->UPDATE()        [spatial]
    /// Line 3762: processDestroyList()                 [death/cleanup]
    /// Line 3765: TheCommandList->reset()
    /// Line 3767: TheWeaponStore->UPDATE()             [weapons]
    /// Line 3768: TheLocomotorStore->UPDATE()          [locomotors]
    /// Line 3769: TheVictoryConditions->UPDATE()       [victory]
    /// Line 3783: disabled status check                [re-enable]
    /// Line 3799: m_frame++                            [increment]
    /// ```
    fn update_simulation(&mut self, dt: f32) {
        // -----------------------------------------------------------------------
        // Phase 1: Early Scripting (C++ line 3600)
        // -----------------------------------------------------------------------
        // C++: TheScriptEngine->UPDATE();
        // Scripts run BEFORE everything else so they can react to the previous
        // frame's state and issue commands for this frame.
        self.evaluate_and_execute_scripts(dt);

        // -----------------------------------------------------------------------
        // Phase 2: Time Freeze Check (C++ lines 3603-3617)
        // -----------------------------------------------------------------------
        // C++: if (freezeTime) { ... return; }
        // When time is frozen, only scripts evaluated above are allowed to run.
        if self.is_time_frozen_for_simulation() {
            return;
        }

        // -----------------------------------------------------------------------
        // Phase 3: Terrain Update (C++ line 3622)
        // -----------------------------------------------------------------------
        // C++: TheTerrainLogic->UPDATE();
        // Terrain (bridges, dynamic water, trigger areas) updates BEFORE objects
        // so bridge state changes from scripts are reflected during the object pass.
        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            terrain.update();
        }

        // -----------------------------------------------------------------------
        // Phase 4: Pre-Update / Collect object IDs
        // -----------------------------------------------------------------------
        let object_ids: Vec<ObjectId> = self.objects.keys().copied().collect();

        // -----------------------------------------------------------------------
        // Phase 5: Command Processing (C++ line 3669)
        // -----------------------------------------------------------------------
        // C++: processCommandList( TheCommandList );
        // Process queued player commands BEFORE object updates so movement/attack
        // orders are in effect when objects run their updates.
        self.process_commands();

        // -----------------------------------------------------------------------
        // Phase 6: Object Updates -- Normal + Sleepy Modules (C++ lines 3672-3738)
        // -----------------------------------------------------------------------
        // C++: ALLOW_NONSLEEPY_UPDATES loop, then sleepy updates loop.
        // These include construction, movement, and the simplified per-object AI
        // decision logic. Stealth modules also live in the sleepy update queue.
        self.update_construction(&object_ids, dt);
        self.update_movement(&object_ids, dt);

        // Projectile and DoT updates -- these are part of the object update phase
        // in C++ (processed within the normal/sleepy update module loops).
        if let Err(e) = update_projectiles(dt) {
            log::warn!("Projectile update failed: {}", e);
        }
        if let Err(e) = update_dot_effects(self.frame) {
            log::warn!("DoT effects update failed: {}", e);
        }

        // Special power cooldown/timer updates
        update_special_powers();

        // -----------------------------------------------------------------------
        // Phase 7: Combat Resolution (within object updates)
        // -----------------------------------------------------------------------
        // Weapon fire and damage application as part of the object update pass.
        self.update_combat(&object_ids, dt);

        // -----------------------------------------------------------------------
        // Phase 7b: Building Body Damage State Checks (C++ BodyModule update)
        // -----------------------------------------------------------------------
        // C++ parity (GarrisonContain::onBodyDamageStateChange): when a garrisoned
        // building drops to ReallyDamaged health (<= 30%), all occupants are
        // force-ejected. This runs after combat so the health state is current.
        self.check_building_damage_states(&object_ids);

        // -----------------------------------------------------------------------
        // Phase 8: AI Update (C++ line 3743)
        // -----------------------------------------------------------------------
        // C++: TheAI->UPDATE();
        // AI runs AFTER object updates so AI decisions are based on the latest
        // world state (objects have moved, combat resolved). This ordering is
        // critical: objects update first, then AI observes new positions and
        // issues commands for the next frame.
        {
            // 1. Update the legacy THE_AI singleton (pathfinder queue, groups).
            if let Ok(mut ai) = THE_AI.write() {
                if let Err(e) = ai.update(self.frame) {
                    log::warn!("THE_AI update failed at frame {}: {:?}", self.frame, e);
                }
            }

            // 2. Update the AiIntegrationManager (per-player AIPlayer / SkirmishPlayer
            //    updates including economy, construction, military decisions).
            if let Some(result) = with_ai_integration_mut(|mgr| mgr.update_ai_players_only()) {
                if let Err(e) = result {
                    log::warn!("AiIntegrationManager update failed at frame {}: {:?}", self.frame, e);
                }
            }
        }

        // Main crate simplified per-object AI decisions (scan for enemies, retreat, etc.)
        self.update_ai(&object_ids, dt);

        // -----------------------------------------------------------------------
        // Phase 9: Production / Build Assistant (C++ line 3748)
        // -----------------------------------------------------------------------
        // C++: TheBuildAssistant->UPDATE();
        // Production queues update after AI so build orders issued by AI this
        // frame can be immediately reflected.
        self.update_production(dt);
        if let Some(mut build_assistant) = get_build_assistant() {
            build_assistant.update(self.frame);
        }

        // -----------------------------------------------------------------------
        // Phase 10: Player Resources
        // -----------------------------------------------------------------------
        self.update_player_resources(dt);
        self.update_power_disabled_state();

        // -----------------------------------------------------------------------
        // Phase 11: Damage/Physics Resolution
        // -----------------------------------------------------------------------
        // Deferred damage and collision resolution after all objects have moved.
        // (Covered above in update_combat; kept as a documentation marker.)

        // -----------------------------------------------------------------------
        // Phase 12: Partition Manager Update (C++ line 3753)
        // -----------------------------------------------------------------------
        // C++: ThePartitionManager->UPDATE();
        // Spatial partition updated AFTER all objects moved and BEFORE death
        // cleanup so spatial queries during cleanup use correct positions.
        // Note: The gamelogic crate's full update_pipeline also runs its own
        // partition manager update (tick_gamelogic_crate in cnc_game_engine.rs).

        // -----------------------------------------------------------------------
        // Phase 13: Death/Cleanup (C++ line 3762)
        // -----------------------------------------------------------------------
        // C++: processDestroyList();
        // Destroyed objects removed from world. Note: the actual destroy list
        // processing happens in step_simulation_with_budget() after this method
        // returns, so objects marked for destruction this frame are cleaned up
        // after the frame is complete.

        // -----------------------------------------------------------------------
        // Phase 14: Weapon Store Update (C++ line 3767)
        // -----------------------------------------------------------------------
        // C++: TheWeaponStore->UPDATE();
        // Process delayed weapon damage that is now ready.
        if let Err(e) = with_weapon_store_mut(|store| store.update()) {
            // "not initialized" is expected before map load; skip silently
            let err_str = e.to_string();
            if !err_str.contains("not initialized") {
                log::warn!("Weapon store update failed: {}", e);
            }
        }

        // -----------------------------------------------------------------------
        // Phase 14b: Locomotor Store Update (C++ line 3768)
        // -----------------------------------------------------------------------
        // C++: TheLocomotorStore->UPDATE();
        // The Rust locomotor store is a template registry without per-frame
        // update logic yet, but we keep the call site for C++ parity.
        // (Will become a real call once the locomotor store gains an update method.)

        // -----------------------------------------------------------------------
        // Phase 15: Victory Conditions (C++ line 3769)
        // -----------------------------------------------------------------------
        // C++: TheVictoryConditions->UPDATE();
        // Handled by the gamelogic crate's update_pipeline (tick_gamelogic_crate).

        // -----------------------------------------------------------------------
        // Phase 16: Disabled Status Check (C++ lines 3783-3792)
        // -----------------------------------------------------------------------
        // C++: for( Object *obj = m_objList; obj; obj = obj->getNextObject() )
        // C++:   if( obj->isDisabled() ) obj->checkDisabledStatus();
        //
        // Check timer-based disabled states and re-enable objects whose disable
        // duration has expired. The Main crate's ObjectStatus does not yet have
        // a disabled/disabled_timer field, so this is a no-op placeholder that
        // will become active once the disabled-status tracking is implemented.
        // The gamelogic crate's update_pipeline handles this for its objects.

        // -----------------------------------------------------------------------
        // Phase 17: Vision/Shroud Update
        // -----------------------------------------------------------------------
        // The gamelogic crate's ShroudManager only sees objects registered in the
        // gamelogic OBJECT_REGISTRY.  Main-crate objects live in a separate
        // HashMap, so we feed their vision ranges directly into the shroud grid
        // here so fog-of-war actually works for the playable game.
        self.update_main_crate_vision();

        // -----------------------------------------------------------------------
        // Phase 18: Team Events Flush
        // -----------------------------------------------------------------------
        // Handled by the gamelogic crate's update_pipeline.

        // -----------------------------------------------------------------------
        // Post-phase: Audio events
        // -----------------------------------------------------------------------
        // Process queued audio events after all simulation phases.
        // Mirrors C++ TheAudio->UPDATE() call.
        self.process_audio_events();
    }

    /// Update construction progress.
    /// C++ parity: buildings only progress when a worker/dozer is nearby.
    /// Multiple dozers stack their build rate (C++ BuildAssistant).
    fn update_construction(&mut self, object_ids: &[ObjectId], dt: f32) {
        const BUILDER_RANGE: f32 = 30.0; // Max distance for a dozer to contribute.

        // C++ parity: calcTimeToBuild applies the same power penalty to dozer
        // construction as to production queue speed.
        let team_power_factor = self.compute_team_power_factors();

        // Pre-scan all dozer positions/teams so we don't borrow-conflict.
        let dozer_info: Vec<(Vec3, Team)> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive() && obj.can_construct())
            .map(|obj| (obj.get_position(), obj.team))
            .collect();

        let mut completed_structures: Vec<ObjectId> = Vec::new();
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if obj.status.under_construction {
                    let build_pos = obj.get_position();
                    let build_team = obj.team;
                    let dozer_count = dozer_info
                        .iter()
                        .filter(|(pos, t)| *t == build_team && pos.distance(build_pos) <= BUILDER_RANGE)
                        .count()
                        .max(1); // At least 1 so AI-built structures still progress.

                    let power_factor = team_power_factor.get(&build_team).copied().unwrap_or(1.0);
                    let base_rate = 1.0 / obj.thing.template.build_time;
                    let effective_rate = base_rate * dozer_count as f32 * power_factor;
                    obj.construction_percent += effective_rate * dt;

                    if obj.construction_percent >= 1.0 {
                        obj.construction_percent = 1.0;
                        obj.status.under_construction = false;
                        obj.health.current = obj.health.maximum;
                        completed_structures.push(id);
                    } else {
                        obj.health.current =
                            obj.health.maximum * (0.1 + 0.9 * obj.construction_percent);
                    }
                }
                obj.tick_timers(dt);
            }
        }

        // C++ parity: when a structure finishes construction, release any dozers
        // that were constructing it — set them to Idle.
        for &completed_id in &completed_structures {
            for obj in self.objects.values_mut() {
                if obj.ai_state == AIState::Constructing
                    && obj.target == Some(completed_id)
                    && obj.is_alive()
                {
                    obj.set_target(None);
                    obj.stop_moving();
                    obj.ai_state = AIState::Idle;
                }
            }
            if let Some(team) = self.objects.get(&completed_id).map(|o| o.team) {
                self.record_structure_completion(team);
            }
        }
    }

    fn update_production(&mut self, dt: f32) {
        // C++ parity: pre-compute per-team power factor so we don't borrow
        // self.players while self.objects is mutably borrowed.
        // Formula matches ThingTemplate::calcTimeToBuild():
        //   energy_ratio = produced / max(consumed, produced) clamped to [0,1]
        //   energy_short = (1.0 - ratio) * penalty_modifier
        //   rate = max(1.0 - energy_short, 0.5)
        //   if ratio < 1.0: rate = min(rate, 0.8)
        let team_power_factor = self.compute_team_power_factors();

        let mut completions: Vec<(Team, String, Vec3, Option<Vec3>)> = Vec::new();

        for (_id, obj) in self.objects.iter_mut() {
            if !obj.is_constructed() || !obj.is_alive() {
                continue;
            }
            if let Some(building) = obj.building_data.as_mut() {
                let pf = team_power_factor.get(&obj.team).copied().unwrap_or(1.0);
                if let Some(completed) = building.update_production(dt, pf) {
                    let rally = building.rally_point;
                    // Spawn slightly offset from the building facing to reduce clumping.
                    let forward = obj.thing.get_direction_vector();
                    let base = obj.get_position() + forward * obj.selection_radius.max(10.0);
                    // Deterministic jitter based on template bytes (simple FNV-1a).
                    let mut hash: u32 = 0x811c9dc5;
                    for &b in completed.as_bytes() {
                        hash ^= b as u32;
                        hash = hash.wrapping_mul(0x01000193);
                    }
                    let angle = (hash as f32) * 0.001;
                    let radius = 3.0 + (hash as f32 % 5.0);
                    let jitter = Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
                    let spawn_pos = base + jitter;
                    completions.push((obj.team, completed, spawn_pos, rally));
                }
            }
        }

        for (team, template, mut spawn_pos, rally) in completions {
            // Push spawn a bit off the footprint center to reduce stacking.
            let jitter_dir = Vec3::new(
                (spawn_pos.x * 17.0 + spawn_pos.z).sin(),
                0.0,
                (spawn_pos.z * 31.0 + spawn_pos.x).cos(),
            )
            .normalize_or_zero();
            // Use template selection heuristic later once the object is created.
            if let Some(new_id) = self.create_object(&template, team, spawn_pos) {
                if let Some(unit) = self.objects.get(&new_id) {
                    let selection_radius = unit.selection_radius.max(4.0);
                    spawn_pos += jitter_dir * selection_radius;
                }
                if let Some(unit) = self.objects.get_mut(&new_id) {
                    unit.set_position(spawn_pos);
                    if let Some(rally_point) = rally {
                        // Mirror C++: set destination toward rally and kick movement state.
                        unit.set_destination(rally_point);
                        unit.ai_state = AIState::Moving;
                    }
                }
            }
        }
    }

    fn ensure_non_shell_player_presence(
        &mut self,
        parsed_settings: Option<&super::script_loader::MapMetadata>,
    ) {
        if self.game_mode == GameMode::Shell {
            return;
        }

        let mut team_order = Vec::new();
        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for player_id in player_ids {
            let Some(player) = self.players.get(&player_id) else {
                continue;
            };
            if player.team == Team::Neutral || team_order.contains(&player.team) {
                continue;
            }
            team_order.push(player.team);
        }
        if team_order.is_empty() {
            return;
        }

        let default_bounds_min = Vec3::new(-300.0, 0.0, -300.0);
        let default_bounds_max = Vec3::new(300.0, 0.0, 300.0);
        let (bounds_min, bounds_max) = parsed_settings
            .and_then(|meta| {
                meta.world_min.zip(meta.world_max).map(|(min, max)| {
                    (
                        Vec3::new(min.x, min.y, min.z),
                        Vec3::new(max.x, max.y, max.z),
                    )
                })
            })
            .filter(|(min, max)| (max.x - min.x).abs() >= 1.0 && (max.z - min.z).abs() >= 1.0)
            .unwrap_or((default_bounds_min, default_bounds_max));

        let span = bounds_max - bounds_min;
        let spawn_positions = [
            Vec3::new(bounds_min.x + span.x * 0.20, 0.0, bounds_min.z + span.z * 0.20),
            Vec3::new(bounds_max.x - span.x * 0.20, 0.0, bounds_max.z - span.z * 0.20),
            Vec3::new(bounds_max.x - span.x * 0.20, 0.0, bounds_min.z + span.z * 0.20),
            Vec3::new(bounds_min.x + span.x * 0.20, 0.0, bounds_max.z - span.z * 0.20),
            Vec3::new(bounds_min.x + span.x * 0.50, 0.0, bounds_min.z + span.z * 0.15),
            Vec3::new(bounds_min.x + span.x * 0.50, 0.0, bounds_max.z - span.z * 0.15),
        ];

        let mut spawned_count = 0usize;
        for (index, team) in team_order.into_iter().enumerate() {
            let has_presence = self
                .objects
                .values()
                .any(|object| object.team == team && object.is_alive());
            if has_presence {
                continue;
            }

            let mut spawn_position = spawn_positions[index % spawn_positions.len()];
            if let Some(ground_height) =
                self.terrain_height_at(Vec3::new(spawn_position.x, 0.0, spawn_position.z))
            {
                spawn_position.y = ground_height;
            }

            let primary_template = match team {
                Team::USA => "CommandCenter",
                Team::GLA => "GLA_CommandCenter",
                Team::China => "China_CommandCenter",
                Team::Neutral => "CommandCenter",
            };

            if self
                .create_object(primary_template, team, spawn_position)
                .is_none()
                && primary_template != "CommandCenter"
            {
                let _ = self.create_object("CommandCenter", team, spawn_position);
            }
            spawned_count += 1;
        }

        if spawned_count > 0 {
            log::info!(
                "Seeded {} fallback player start structures for non-shell map '{}'",
                spawned_count,
                self.map_name
            );
        }
    }

    fn configure_victory_rules_for_map(&mut self, map_name: &str) {
        let rules = victory_rules_for_map(map_name);
        self.victory_conditions.set_victory_conditions(rules);
        log::info!(
            "Configured victory rules for map '{}': require units = {}, require buildings = {}",
            map_name,
            rules.requires_units(),
            rules.requires_buildings()
        );
    }

    fn convert_script_event(&self, event: &ScriptEvent) -> Option<MissionScriptEvent> {
        use ScriptValue as Val;
        let mut params = HashMap::new();
        let event_type = match event {
            ScriptEvent::PlayerDefeated { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "player_defeated"
            }
            ScriptEvent::AllianceStateChanged { player_id, state } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                params.insert(
                    "state".into(),
                    Val::String(format!("{:?}", state).to_lowercase()),
                );
                "alliance_state_changed"
            }
            ScriptEvent::RevealMapForPlayer { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "reveal_map_for_player"
            }
        };

        Some(MissionScriptEvent {
            event_type: event_type.to_string(),
            parameters: params,
            timestamp: std::time::Instant::now(),
            priority: ScriptPriority::Normal,
        })
    }

    /// Move an object to a target position using pathfinding.
    /// Falls back to direct movement if no path is found.
    /// If `ai_state_override` is provided, sets that AI state after moving.
    fn move_object_with_pathfinding(
        &mut self,
        object_id: ObjectId,
        target_position: Vec3,
        ai_state_override: Option<AIState>,
    ) {
        let start_pos = self
            .objects
            .get(&object_id)
            .map(|obj| obj.get_position());

        let start_pos = match start_pos {
            Some(pos) => pos,
            None => return,
        };

        // Short distance — skip pathfinding overhead and go direct.
        if start_pos.distance(target_position) < 20.0 {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.move_to(target_position);
                if let Some(state) = ai_state_override {
                    obj.ai_state = state;
                }
            }
            return;
        }

        // Attempt A* pathfinding.
        let path = self
            .pathfinding_system
            .find_path(start_pos, target_position, &self.objects);

        if let Some(obj) = self.objects.get_mut(&object_id) {
            if let Some(waypoints) = path {
                if waypoints.len() >= 2 {
                    obj.movement.path = waypoints;
                    obj.movement.current_path_index = 1; // skip start node
                    // target_position will be set to path[1] by update_movement
                    obj.movement.target_position = Some(obj.movement.path[1]);
                    obj.ai_state = ai_state_override.unwrap_or(AIState::Moving);
                    obj.status.moving = true;
                } else {
                    obj.move_to(target_position);
                    if let Some(state) = ai_state_override {
                        obj.ai_state = state;
                    }
                }
            } else {
                // No path found — fall back to direct movement.
                obj.move_to(target_position);
                if let Some(state) = ai_state_override {
                    obj.ai_state = state;
                }
            }
        }
    }

    /// Update movement for all objects
    fn update_movement(&mut self, object_ids: &[ObjectId], dt: f32) {
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if !obj.movement.path.is_empty()
                    && obj.movement.current_path_index < obj.movement.path.len()
                {
                    let current_pos = obj.get_position();
                    let waypoint = obj.movement.path[obj.movement.current_path_index];
                    if current_pos.distance(waypoint) < 5.0 {
                        obj.movement.current_path_index += 1;
                        if obj.movement.current_path_index >= obj.movement.path.len() {
                            obj.stop_moving();
                            continue;
                        }
                    }

                    let waypoint = obj.movement.path[obj.movement.current_path_index];
                    obj.movement.target_position = Some(waypoint);
                }

                if let Some(target_pos) = obj.movement.target_position {
                    let current_pos = obj.get_position();
                    let direction = (target_pos - current_pos).normalize_or_zero();

                    if direction.length() > 0.0 {
                        // Calculate new position and orientation
                        let target_velocity = direction * obj.movement.max_speed;
                        let velocity_diff = target_velocity - obj.movement.velocity;
                        let max_accel = obj.movement.acceleration * dt;

                        let new_velocity = if velocity_diff.length() <= max_accel {
                            target_velocity
                        } else {
                            obj.movement.velocity + velocity_diff.normalize() * max_accel
                        };

                        let new_position = current_pos + new_velocity * dt;
                        let desired_angle = (-new_velocity.z).atan2(new_velocity.x);
                        let reached_target = current_pos.distance(target_pos) < 2.0;

                        obj.set_position(new_position);
                        obj.set_orientation(desired_angle);
                        if reached_target {
                            obj.stop_moving();
                        }
                    }
                }
            }
        }
    }

    /// Update AI behavior for all objects
    /// Enhanced with AI decision system for intelligent behavior
    fn update_ai(&mut self, object_ids: &[ObjectId], dt: f32) {
        use crate::ai_decisions::*;

        let mut ai_commands = Vec::new();
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP; // Convert frame to seconds
        let game_phase = GamePhase::from_time(current_time);

        // First pass: Dispatch object AI through the existing state machine.
        for &object_id in object_ids {
            if let Some(obj) = self.objects.get(&object_id) {
                let position = obj.get_position();
                let team = obj.team;
                let ai_state = obj.ai_state.clone();
                let current_target = obj.target;
                let can_attack = obj.can_attack();
                if let Some(command) = self.process_ai_behavior(
                    object_id,
                    ai_state,
                    current_target,
                    position,
                    team,
                    can_attack,
                    self.frame,
                    dt,
                ) {
                    ai_commands.push(command);
                }
            }
        }

        // Second pass: Handle production buildings
        for &object_id in object_ids {
            let (team, spawn_origin, is_production_building) = match self.objects.get(&object_id) {
                Some(obj)
                    if obj.is_kind_of(KindOf::Structure)
                        && obj.is_constructed()
                        && obj.is_alive() =>
                {
                    let is_production_building = obj.template_name.contains("Barracks")
                        || obj.template_name.contains("WarFactory")
                        || obj.template_name.contains("ArmsDealer");
                    (obj.team, obj.get_position(), is_production_building)
                }
                _ => continue,
            };

            if !is_production_building {
                continue;
            }

            // Find which player owns this building.
            let player_id = self
                .players
                .iter()
                .find_map(|(pid, player)| (player.team == team).then_some(*pid));

            let Some(pid) = player_id else {
                continue;
            };

            // Check if should produce units (every 10 seconds).
            if self.frame % 600 != 0 {
                continue;
            }

            if let Some(unit_to_produce) =
                AIDecisionSystem::select_production_unit(self, team, game_phase, pid)
            {
                // Queue unit production (in a full implementation)
                log::trace!(
                    "AI Building {} queuing production of {}",
                    object_id, unit_to_produce
                );

                // Mirror production parity: charge cost before spawning.
                let Some(build_cost) = self.templates.get(&unit_to_produce).map(|t| t.build_cost)
                else {
                    continue;
                };

                let can_pay = self
                    .get_player_mut(pid)
                    .map(|player| player.spend_resources(&build_cost))
                    .unwrap_or(false);
                if !can_pay {
                    continue;
                }

                // Actually create the unit near the building.
                let spawn_pos = spawn_origin + Vec3::new(20.0, 0.0, 20.0);
                if self
                    .create_object(&unit_to_produce, team, spawn_pos)
                    .is_none()
                {
                    // Refund if the spawn failed after charging resources.
                    if let Some(player) = self.get_player_mut(pid) {
                        player.resources.supplies = player
                            .resources
                            .supplies
                            .saturating_add(build_cost.supplies);
                        player.power_available -= build_cost.power;
                    }
                }
            }
        }

        // Apply all AI commands
        for command in ai_commands {
            self.apply_ai_command(command);
        }

        // Resolve command-driven support states (guard/repair/docking/garrison) after AI decisions.
        self.update_support_states(object_ids, dt);
    }

    /// Update combat for all objects
    fn update_combat(&mut self, object_ids: &[ObjectId], _dt: f32) {
        for &attacker_id in object_ids {
            let Some(attacker) = self.objects.get(&attacker_id) else {
                continue;
            };
            let Some(weapon) = attacker.weapon.as_ref() else {
                continue;
            };
            let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
            if current_time - weapon.last_fire_time < weapon.reload_time {
                continue;
            }

            let attacker_team = attacker.team;
            let target_id = attacker.target;
            let target_location = attacker.target_location;
            let mut weapon_damage = weapon.damage;
            if attacker.overcharge_enabled {
                weapon_damage *= 1.1;
            }
            if attacker.active_weapon_slot == 1 {
                weapon_damage *= 1.15;
            }

            let mut fired = false;

            // Standard object-to-object attack.
            if let Some(target_id) = target_id {
                let target_status = self
                    .objects
                    .get(&target_id)
                    .map(|target| (target.is_alive(), target.get_position()));

                let Some((target_alive, target_position)) = target_status else {
                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        attacker.stop_attack();
                    }
                    continue;
                };

                if !target_alive {
                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        attacker.stop_attack();
                    }
                    continue;
                }

                let can_target = {
                    if let (Some(attacker), Some(target)) =
                        (self.objects.get(&attacker_id), self.objects.get(&target_id))
                    {
                        let enemy_or_forced = attacker.force_attack || attacker.team != target.team;
                        attacker.can_target(target) && enemy_or_forced
                    } else {
                        false
                    }
                };

                if can_target {
                    fired = true;
                    if let Some(target) = self.objects.get_mut(&target_id) {
                        let destroyed = target.take_damage(weapon_damage);
                        if destroyed {
                            // C++ parity: XP is based on victim's ExperienceValue.
                            let kill_xp = target.thing.template.experience_value
                                * Self::veterancy_xp_multiplier(target.experience.level);
                            self.mark_object_for_destruction(target_id, Some(attacker_team));
                            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                attacker.gain_experience(kill_xp);
                                attacker.stop_attack();
                            }
                        }
                    }
                } else if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    // Match classic behavior: an attack order should chase a valid target until
                    // the weapon can fire (unless the unit is immobile).
                    if attacker.can_move() {
                        attacker.movement.path.clear();
                        attacker.movement.current_path_index = 0;
                        attacker.movement.target_position = Some(target_position);
                        attacker.status.moving = true;
                        attacker.ai_state = AIState::Attacking;
                        attacker.status.attacking = true;
                    }
                }
            } else if let Some(target_location) = target_location {
                // Force-attack-ground: consume a shot when the location is in range and apply damage
                // to the nearest hittable object around the designated impact point.
                let can_fire_at_location = {
                    if let Some(attacker) = self.objects.get(&attacker_id) {
                        attacker
                            .weapon
                            .as_ref()
                            .map(|w| {
                                w.can_target_ground
                                    && attacker.position.distance(target_location) <= w.range
                            })
                            .unwrap_or(false)
                    } else {
                        false
                    }
                };

                if can_fire_at_location {
                    fired = true;
                    if let Some(ground_target_id) =
                        self.find_ground_attack_victim(attacker_id, target_location)
                    {
                        if let Some(target) = self.objects.get_mut(&ground_target_id) {
                            let destroyed = target.take_damage(weapon_damage);
                            if destroyed {
                                let kill_xp = target.thing.template.experience_value
                                    * Self::veterancy_xp_multiplier(target.experience.level);
                                self.mark_object_for_destruction(
                                    ground_target_id,
                                    Some(attacker_team),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    attacker.gain_experience(kill_xp);
                                }
                            }
                        }
                    }
                }
            }

            if fired {
                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    if let Some(weapon) = &mut attacker.weapon {
                        weapon.last_fire_time = current_time;
                    }
                }
            }
        }
    }

    fn find_ground_attack_victim(
        &self,
        attacker_id: ObjectId,
        target_location: Vec3,
    ) -> Option<ObjectId> {
        const GROUND_IMPACT_RADIUS: f32 = 12.0;

        let attacker = self.objects.get(&attacker_id)?;
        let mut best: Option<(ObjectId, f32)> = None;

        for (&candidate_id, candidate) in self.objects.iter() {
            if candidate_id == attacker_id || !candidate.is_alive() || !candidate.is_attackable() {
                continue;
            }

            if !attacker.force_attack && candidate.team == attacker.team {
                continue;
            }

            let impact_distance = candidate.get_position().distance(target_location);
            if impact_distance > GROUND_IMPACT_RADIUS {
                continue;
            }

            if !attacker.can_target(candidate) {
                continue;
            }

            match best {
                Some((_, best_distance)) if impact_distance >= best_distance => {}
                _ => best = Some((candidate_id, impact_distance)),
            }
        }

        best.map(|(id, _)| id)
    }

    /// Process AI behavior for a single object
    /// Enhanced with proper enemy detection, attack decisions, and movement
    fn process_ai_behavior(
        &self,
        object_id: ObjectId,
        ai_state: AIState,
        target_id: Option<ObjectId>,
        position: Vec3,
        team: Team,
        can_attack: bool,
        frame: u32,
        _dt: f32,
    ) -> Option<AICommand> {
        let should_scan = |interval: u32| -> bool { interval > 0 && frame % interval == 0 };
        let retreat_from = |threat_id: ObjectId| -> AICommand {
            let direction = self
                .objects
                .get(&threat_id)
                .map(|enemy| position - enemy.get_position())
                .and_then(|delta| {
                    if delta.length_squared() > f32::EPSILON {
                        Some(delta.normalize())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| Vec3::new(1.0, 0.0, 0.0));
            AICommand::MoveTo {
                object_id,
                position: position + direction * 90.0,
            }
        };
        let evaluate_enemy = |enemy_id: ObjectId, search_radius: f32| -> Option<AICommand> {
            use crate::ai_decisions::{AIDecisionSystem, AttackDecision};

            match AIDecisionSystem::should_attack(self, object_id, enemy_id) {
                AttackDecision::Attack => Some(AICommand::AttackTarget {
                    object_id,
                    target_id: enemy_id,
                }),
                AttackDecision::Retreat => Some(retreat_from(enemy_id)),
                AttackDecision::FindNewTarget => AIDecisionSystem::find_best_target(
                    self,
                    object_id,
                    position,
                    team,
                    search_radius,
                    true,
                    true,
                    false,
                )
                .map(|better_target| AICommand::AttackTarget {
                    object_id,
                    target_id: better_target,
                }),
                AttackDecision::Hold => None,
            }
        };

        match ai_state {
            AIState::Idle => {
                if can_attack && should_scan(30) {
                    let search_radius = 200.0;
                    if let Some((enemy_id, _)) =
                        crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                            self,
                            position,
                            team,
                            search_radius,
                        )
                    {
                        return evaluate_enemy(enemy_id, search_radius);
                    }
                }
                if frame % 300 == object_id.0 % 300 {
                    Some(AICommand::SetAIState {
                        object_id,
                        state: AIState::Patrolling,
                    })
                } else {
                    None
                }
            }

            AIState::Attacking => {
                use crate::ai_decisions::{AIDecisionSystem, AttackDecision};

                let Some(current_target_id) = target_id else {
                    return Some(AICommand::StopAttack { object_id });
                };

                match AIDecisionSystem::should_attack(self, object_id, current_target_id) {
                    AttackDecision::Attack | AttackDecision::Hold => None,
                    AttackDecision::Retreat => Some(retreat_from(current_target_id)),
                    AttackDecision::FindNewTarget => {
                        if !can_attack {
                            return Some(AICommand::StopAttack { object_id });
                        }
                        AIDecisionSystem::find_best_target(
                            self, object_id, position, team, 220.0, true, true, false,
                        )
                        .map(|target_id| AICommand::AttackTarget {
                            object_id,
                            target_id,
                        })
                        .or(Some(AICommand::StopAttack { object_id }))
                    }
                }
            }

            AIState::AttackMoving => {
                if can_attack && should_scan(20) {
                    let search_radius = 220.0;
                    if let Some((enemy_id, _)) =
                        crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                            self,
                            position,
                            team,
                            search_radius,
                        )
                    {
                        return evaluate_enemy(enemy_id, search_radius);
                    }
                }
                None
            }

            AIState::Moving => {
                // While moving, check if we're under attack
                // Could transition to defensive behavior if needed
                None
            }

            AIState::Patrolling => {
                if can_attack && should_scan(25) {
                    let search_radius = 200.0;
                    if let Some((enemy_id, _)) =
                        crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                            self,
                            position,
                            team,
                            search_radius,
                        )
                    {
                        return evaluate_enemy(enemy_id, search_radius);
                    }
                }

                if frame % 180 == object_id.0 % 180 {
                    let patrol_radius = 100.0;
                    let random_angle = (((object_id.0 as u64 * 1103515245 + frame as u64) % 360)
                        as f32)
                        .to_radians();
                    let patrol_pos = Vec3::new(
                        position.x + patrol_radius * random_angle.cos(),
                        position.y,
                        position.z + patrol_radius * random_angle.sin(),
                    );
                    Some(AICommand::MoveTo {
                        object_id,
                        position: patrol_pos,
                    })
                } else {
                    None
                }
            }

            AIState::GuardingArea | AIState::GuardingObject => {
                // Guarding states are resolved in update_support_states() where guard anchors/radii
                // and target legality checks are available.
                None
            }

            AIState::Gathering => {
                // Resource gathering behavior: move to supply pile, collect, return to refinery.
                // This autonomous behavior just monitors state — actual resource accumulation
                // happens in the update loop via a separate phase.
                let gather_target_id = target_id;

                if let Some(source_id) = gather_target_id {
                    if let Some(source_obj) = self.objects.get(&source_id) {
                        let dist_to_source = position.distance(source_obj.get_position());
                        if dist_to_source > 15.0 {
                            // Still moving toward the resource — keep going
                            return Some(AICommand::MoveTo {
                                object_id,
                                position: source_obj.get_position(),
                            });
                        }
                        // Close enough — the update loop handles accumulation.
                        // Check if full (stored_resources checked in update phase).
                        None
                    } else {
                        // Resource source no longer exists — go idle
                        Some(AICommand::SetAIState {
                            object_id,
                            state: AIState::Idle,
                        })
                    }
                } else {
                    Some(AICommand::SetAIState {
                        object_id,
                        state: AIState::Idle,
                    })
                }
            }

            AIState::Constructing | AIState::Repairing => {
                // Building or repairing - continue current task
                None
            }

            AIState::Docked | AIState::Garrisoned => {
                // Unit is inside another structure - no autonomous behavior
                None
            }

            AIState::AttackingGround => {
                // Artillery-style ground attack
                // Continue until command is cancelled
                None
            }

            AIState::SpecialAbility => {
                // Unit is using special ability
                // Continue until ability completes
                None
            }

            AIState::SeekingRepair => {
                // Unit is looking for repair facility
                // Would pathfind to nearest repair bay
                None
            }

            AIState::SeekingHealing => {
                // Unit is looking for medical facility
                // Would pathfind to nearest medical center
                None
            }

            AIState::Entering => {
                // Unit is entering a transport or garrison
                None
            }

            AIState::Docking => {
                // Unit is docking with a structure (harvester to refinery, etc)
                None
            }

            AIState::ReturningResources => {
                // Worker heading back to supply center to deposit resources.
                // The actual deposit happens in the update loop when close enough.
                if let Some(refinery_id) = self.find_nearest_supply_center(team, position) {
                    if let Some(refinery) = self.objects.get(&refinery_id) {
                        let dist_to_refinery = position.distance(refinery.get_position());
                        if dist_to_refinery > 20.0 {
                            // Still heading to refinery
                            return Some(AICommand::MoveTo {
                                object_id,
                                position: refinery.get_position(),
                            });
                        }
                    }
                }
                None
            }

            AIState::Capturing => {
                // Unit is capturing enemy structure
                // Continue until capture completes
                None
            }
        }
    }

    /// Apply AI command to the game state
    fn apply_ai_command(&mut self, command: AICommand) {
        match command {
            AICommand::AttackTarget {
                object_id,
                target_id,
            } => {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.attack_target(target_id);
                }
            }
            AICommand::StopAttack { object_id } => {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_attack();
                }
            }
            AICommand::MoveTo {
                object_id,
                position,
            } => {
                self.move_object_with_pathfinding(object_id, position, None);
            }
            AICommand::SetAIState { object_id, state } => {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.ai_state = state;
                }
            }
        }
    }

    fn update_support_states(&mut self, object_ids: &[ObjectId], dt: f32) {
        const GUARD_MIN_RADIUS: f32 = 80.0;
        const INTERACT_RANGE: f32 = 14.0;
        const CAPTURE_RANGE_PADDING: f32 = 4.0;
        const SPECIAL_ABILITY_RANGE_PADDING: f32 = 4.0;
        const REPAIR_RATE: f32 = 35.0;
        const HEAL_RATE: f32 = 25.0;

        for &object_id in object_ids {
            let snapshot = match self.objects.get(&object_id) {
                Some(obj) => (
                    obj.ai_state.clone(),
                    obj.team,
                    obj.get_position(),
                    obj.target,
                    obj.guard_position,
                    obj.guard_target,
                    obj.guard_radius,
                    obj.can_move(),
                    obj.can_attack(),
                    obj.health.current,
                    obj.health.maximum,
                    obj.selection_radius,
                    obj.is_alive(),
                ),
                None => continue,
            };

            let (
                ai_state,
                team,
                position,
                target_id,
                guard_position,
                guard_target,
                guard_radius,
                can_move,
                can_attack,
                health_current,
                health_maximum,
                selection_radius,
                is_alive,
            ) = snapshot;

            if !is_alive {
                continue;
            }

            if ai_state != AIState::SpecialAbility {
                self.pending_special_abilities.remove(&object_id);
            }

            match ai_state {
                AIState::GuardingArea => {
                    let anchor = guard_position.unwrap_or(position);
                    let radius = guard_radius.max(GUARD_MIN_RADIUS);

                    if can_attack {
                        if let Some((enemy_id, _)) =
                            crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                                self, anchor, team, radius,
                            )
                        {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_force_attack(false);
                                obj.attack_target(enemy_id);
                            }
                            continue;
                        }
                    }

                    if can_move && position.distance(anchor) > radius * 0.6 {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(anchor);
                            obj.ai_state = AIState::GuardingArea;
                        }
                    }
                }
                AIState::GuardingObject => {
                    let guard_target_id = match guard_target {
                        Some(id) => id,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    };

                    let Some(guard_anchor) = self
                        .objects
                        .get(&guard_target_id)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_guard_target(None);
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let radius = guard_radius.max(GUARD_MIN_RADIUS);
                    if can_attack {
                        if let Some((enemy_id, _)) =
                            crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                                self,
                                guard_anchor,
                                team,
                                radius,
                            )
                        {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_force_attack(false);
                                obj.attack_target(enemy_id);
                            }
                            continue;
                        }
                    }

                    if can_move && position.distance(guard_anchor) > radius * 0.6 {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(guard_anchor);
                            obj.ai_state = AIState::GuardingObject;
                        }
                    }
                }
                AIState::Repairing => {
                    let Some(repair_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let actor_can_repair = self
                        .objects
                        .get(&object_id)
                        .map(|obj| obj.can_repair())
                        .unwrap_or(false);
                    if !actor_can_repair {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let Some((
                        repair_target_pos,
                        repair_target_team,
                        repair_target_alive,
                        repair_target_is_structure,
                        repair_target_under_construction,
                    )) = self.objects.get(&repair_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure),
                            target.status.under_construction,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !repair_target_alive
                        || !repair_target_is_structure
                        || repair_target_under_construction
                        || (repair_target_team != team && repair_target_team != Team::Neutral)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if can_move && position.distance(repair_target_pos) > INTERACT_RANGE {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(repair_target_pos);
                            obj.ai_state = AIState::Repairing;
                        }
                        continue;
                    }

                    let target_full = if let Some(target) = self.objects.get_mut(&repair_target_id)
                    {
                        target.heal(REPAIR_RATE * dt);
                        target.health.current >= target.health.maximum - 0.01
                    } else {
                        true
                    };
                    if target_full {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                    }
                }
                state @ (AIState::SeekingRepair | AIState::SeekingHealing) => {
                    if health_current >= health_maximum - 0.01 {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let Some(support_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((
                        support_target_pos,
                        support_target_team,
                        support_target_alive,
                        support_target_under_construction,
                        support_building_type,
                    )) = self.objects.get(&support_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.team,
                            target.is_alive(),
                            target.status.under_construction,
                            target
                                .building_data
                                .as_ref()
                                .map(|b| b.building_type)
                                .unwrap_or(BuildingType::CommandCenter),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !support_target_alive
                        || support_target_under_construction
                        || support_target_team != team
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let source_can_use_support = self
                        .objects
                        .get(&object_id)
                        .map(|obj| match state {
                            AIState::SeekingRepair => {
                                if obj.is_kind_of(KindOf::Aircraft) {
                                    support_building_type == BuildingType::Airfield
                                } else if obj.is_kind_of(KindOf::Vehicle) {
                                    support_building_type == BuildingType::RepairPad
                                } else {
                                    false
                                }
                            }
                            AIState::SeekingHealing => {
                                obj.is_kind_of(KindOf::Infantry)
                                    && support_building_type == BuildingType::HealPad
                            }
                            _ => false,
                        })
                        .unwrap_or(false);
                    if !source_can_use_support {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    if can_move && position.distance(support_target_pos) > INTERACT_RANGE {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(support_target_pos);
                            obj.ai_state = state.clone();
                        }
                        continue;
                    }

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        let rate = match state {
                            AIState::SeekingRepair => REPAIR_RATE,
                            AIState::SeekingHealing => HEAL_RATE,
                            _ => 0.0,
                        };
                        obj.heal(rate * dt);
                        if obj.health.current >= obj.health.maximum - 0.01 {
                            obj.set_target(None);
                        } else {
                            obj.ai_state = state;
                        }
                    }
                }
                state @ (AIState::Entering | AIState::Docking) => {
                    let Some(container_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((
                        container_pos,
                        container_radius,
                        container_team,
                        container_is_structure,
                        container_is_alive,
                        container_under_construction,
                        container_can_contain,
                        container_has_space,
                        container_has_unit,
                        container_occupant_count,
                    )) = self.objects.get(&container_id).map(|container| {
                        (
                            container.get_position(),
                            container.selection_radius,
                            container.team,
                            container.is_kind_of(KindOf::Structure),
                            container.is_alive(),
                            container.status.under_construction,
                            container.can_contain(),
                            container.has_capacity_for(1),
                            container.contained_units().contains(&object_id),
                            container.contained_units().len(),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !can_move
                        || !container_is_alive
                        || container_under_construction
                        || !container_can_contain
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // C++ allows some hostile edge cases (empty non-faction containers). In Main,
                    // approximate this by only allowing hostile enter/dock on empty non-structures.
                    if container_team != team
                        && container_team != Team::Neutral
                        && (container_is_structure || container_occupant_count > 0)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let enter_range = selection_radius + container_radius + 4.0;
                    if can_move && position.distance(container_pos) > enter_range {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(container_pos);
                            obj.ai_state = state;
                        }
                        continue;
                    }

                    let can_enter = container_has_unit || container_has_space;
                    if !can_enter {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let entered = if container_has_unit {
                        true
                    } else {
                        self.objects
                            .get_mut(&container_id)
                            .map(|container| container.add_occupant(object_id))
                            .unwrap_or(false)
                    };
                    if !entered {
                        continue;
                    }

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.status.attacking = false;
                        obj.target_location = None;
                        obj.force_attack = false;
                        obj.target = Some(container_id);
                        obj.contained_by = Some(container_id);
                        obj.set_position(container_pos);
                        obj.ai_state = if container_is_structure {
                            AIState::Garrisoned
                        } else {
                            AIState::Docked
                        };
                        obj.status.moving = false;
                    }
                }
                AIState::Capturing => {
                    let Some(capture_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let can_capture_buildings = self
                        .objects
                        .get(&object_id)
                        .map(|obj| obj.is_kind_of(KindOf::Infantry) || obj.is_hero())
                        .unwrap_or(false);
                    if !can_capture_buildings {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let Some((
                        target_position,
                        target_radius,
                        target_team,
                        target_alive,
                        target_is_structure,
                        target_under_construction,
                    )) = self.objects.get(&capture_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure),
                            target.status.under_construction,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !target_alive || !target_is_structure || target_under_construction {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if target_team == team {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let capture_range = selection_radius + target_radius + CAPTURE_RANGE_PADDING;
                    if can_move && position.distance(target_position) > capture_range {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(target_position);
                            obj.ai_state = AIState::Capturing;
                        }
                        continue;
                    }

                    let did_capture = if let Some(target) = self.objects.get_mut(&capture_target_id)
                    {
                        if target.is_alive()
                            && target.is_kind_of(KindOf::Structure)
                            && !target.status.under_construction
                            && target.team != team
                        {
                            target.set_team(team);
                            target.health.heal(target.max_health);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_target(None);
                    }

                    if did_capture {
                        let msg =
                            localization::localize("hud.capture.complete", "Building captured");
                        self.queue_radar_message_for_team(team, msg);
                    }
                }
                AIState::SpecialAbility => {
                    let Some(ability) = self.pending_special_abilities.get(&object_id).copied()
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };
                    let special_target_id = ability.target_id();

                    let Some((
                        target_position,
                        target_radius,
                        target_team,
                        target_alive,
                        target_is_vehicle,
                        target_is_structure,
                        target_is_airborne,
                    )) = self.objects.get(&special_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Vehicle),
                            target.is_kind_of(KindOf::Structure),
                            target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                        )
                    })
                    else {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let requires_enemy_target =
                        !matches!(ability, PendingSpecialAbility::CarBomb { .. });
                    if !target_alive
                        || (requires_enemy_target
                            && (target_team == team || target_team == Team::Neutral))
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(
                        ability,
                        PendingSpecialAbility::SnipeVehicle { .. }
                            | PendingSpecialAbility::Hijack { .. }
                    ) && (!target_is_vehicle || target_is_airborne)
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(ability, PendingSpecialAbility::Sabotage { .. })
                        && !target_is_structure
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let interact_range =
                        selection_radius + target_radius + SPECIAL_ABILITY_RANGE_PADDING;
                    if can_move && position.distance(target_position) > interact_range {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(target_position);
                            obj.ai_state = AIState::SpecialAbility;
                        }
                        continue;
                    }

                    match ability {
                        PendingSpecialAbility::Hijack { .. } => {
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.set_team(team);
                            }
                            // Hijacker is consumed by the ability.
                            if let Some(hijacker) = self.objects.get_mut(&object_id) {
                                hijacker.status.destroyed = true;
                            }
                            self.mark_object_for_destruction(object_id, Some(team));
                        }
                        PendingSpecialAbility::Sabotage { .. } => {
                            let destroyed = self
                                .objects
                                .get_mut(&special_target_id)
                                .map(|target| target.take_damage(target.max_health * 0.5))
                                .unwrap_or(false);
                            if destroyed {
                                self.mark_object_for_destruction(special_target_id, Some(team));
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::SnipeVehicle { .. } => {
                            let destroyed = self
                                .objects
                                .get_mut(&special_target_id)
                                .map(|target| target.take_damage(target.max_health * 0.25))
                                .unwrap_or(false);
                            if destroyed {
                                self.mark_object_for_destruction(special_target_id, Some(team));
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::CarBomb { .. } => {
                            let destroyed = self
                                .objects
                                .get_mut(&special_target_id)
                                .map(|target| target.take_damage(target.max_health))
                                .unwrap_or(false);
                            if destroyed {
                                self.mark_object_for_destruction(special_target_id, Some(team));
                            }
                            // CarBomb is a suicide ability — mark the bomber immediately.
                            if let Some(bomber) = self.objects.get_mut(&object_id) {
                                bomber.status.destroyed = true;
                            }
                            self.mark_object_for_destruction(object_id, Some(team));
                        }
                    }

                    self.pending_special_abilities.remove(&object_id);
                }
                AIState::Gathering => {
                    // Accumulate resources when close to the supply source.
                    const GATHER_RATE: f32 = 100.0;
                    const MAX_CARRY: u32 = 1000;

                    let Some(source_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_ai_state(AIState::Idle);
                        }
                        continue;
                    };

                    // Extract source state before any mutations.
                    let (source_alive, source_pos) = self
                        .objects
                        .get(&source_id)
                        .map(|s| (s.is_alive(), s.get_position()))
                        .unwrap_or((false, position));

                    if !source_alive {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.set_ai_state(AIState::Idle);
                        }
                        continue;
                    }

                    if can_move && position.distance(source_pos) > INTERACT_RANGE {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(source_pos);
                            obj.ai_state = AIState::Gathering;
                        }
                        continue;
                    }

                    // In range — gather resources.
                    // C++ parity (SupplyWarehouseDockUpdate): gathering depletes
                    // the supply source.  The source is destroyed when empty.
                    let gather_amount = (GATHER_RATE * dt) as u32;
                    let is_full = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.stored_resources.supplies)
                        .unwrap_or(0)
                        + gather_amount
                        >= MAX_CARRY;

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stored_resources.supplies = obj
                            .stored_resources
                            .supplies
                            .saturating_add(gather_amount)
                            .min(MAX_CARRY);
                    }

                    // Deplete the supply source.
                    if let Some(source) = self.objects.get_mut(&source_id) {
                        let taken = gather_amount.min(source.stored_resources.supplies);
                        source.stored_resources.supplies =
                            source.stored_resources.supplies.saturating_sub(taken);
                        if source.stored_resources.supplies == 0 {
                            source.status.destroyed = true;
                            self.mark_object_for_destruction(source_id, None);
                        }
                    }

                    if is_full {
                        // Full — head to nearest supply center.
                        let refinery_dest = self
                            .find_nearest_supply_center(team, position)
                            .and_then(|rid| {
                                self.objects.get(&rid).map(|r| r.get_position())
                            });
                        if let Some(dest) = refinery_dest {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_destination(dest);
                                obj.set_ai_state(AIState::ReturningResources);
                            }
                        }
                    }
                }
                AIState::ReturningResources => {
                    // Deposit resources when close to a supply center.
                    let (refinery_id, refinery_pos) = self
                        .find_nearest_supply_center(team, position)
                        .and_then(|rid| {
                            self.objects
                                .get(&rid)
                                .map(|r| (Some(rid), r.get_position()))
                        })
                        .unwrap_or((None, position));

                    let at_refinery = refinery_id
                        .is_some()
                        && position.distance(refinery_pos) <= INTERACT_RANGE;

                    if at_refinery {
                        // Deposit.
                        let deposit_amount = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.stored_resources.supplies)
                            .unwrap_or(0);

                        if deposit_amount > 0 {
                            // Clear carried resources.
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stored_resources.supplies = 0;
                            }
                            // Credit the player.
                            if let Some(player) = self.get_player_mut_by_team(team) {
                                player.resources.supplies = player
                                    .resources
                                    .supplies
                                    .saturating_add(deposit_amount);
                            }
                            // Head back to gather more from the original source.
                            let source_dest = target_id.and_then(|sid| {
                                self.objects.get(&sid).filter(|s| s.is_alive()).map(|s| s.get_position())
                            });
                            if let Some(dest) = source_dest {
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_destination(dest);
                                    obj.set_ai_state(AIState::Gathering);
                                }
                            }
                        }
                    } else if can_move {
                        // Still heading to refinery.
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(refinery_pos);
                            obj.ai_state = AIState::ReturningResources;
                        }
                    }
                }
                AIState::Docked | AIState::Garrisoned => {
                    let Some(container_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((container_pos, container_alive, container_has_unit)) =
                        self.objects.get(&container_id).map(|container| {
                            (
                                container.get_position(),
                                container.is_alive(),
                                container.contained_units().contains(&object_id),
                            )
                        })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !container_alive || !container_has_unit {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_position(container_pos);
                        obj.stop_moving();
                        obj.status.moving = false;
                    }
                }
                _ => {}
            }
        }
    }

    fn update_object_ai(&mut self, object_id: ObjectId, _dt: f32) {
        // Get object state for AI processing
        let (ai_state, target_id, _position) = {
            if let Some(obj) = self.objects.get(&object_id) {
                (obj.ai_state.clone(), obj.target, obj.get_position())
            } else {
                return;
            }
        };

        if ai_state == AIState::Attacking {
            if let Some(target_id) = target_id {
                // Check if target still exists and is in range
                if let Some(target) = self.objects.get(&target_id) {
                    if let Some(attacker) = self.objects.get(&object_id) {
                        if attacker.can_target(target) {
                            // Try to fire
                            let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                            if let Some(attacker) = self.objects.get_mut(&object_id) {
                                if attacker.can_fire(current_time) {
                                    attacker.fire_at(target_id, current_time);
                                }
                            }
                        } else {
                            // Target out of range or invalid, stop attacking
                            if let Some(attacker) = self.objects.get_mut(&object_id) {
                                attacker.stop_attack();
                            }
                        }
                    }
                } else {
                    // Target no longer exists
                    if let Some(attacker) = self.objects.get_mut(&object_id) {
                        attacker.stop_attack();
                    }
                }
            }
        }

        // Handle AttackingGround: fire at target_location.
        if ai_state == AIState::AttackingGround {
            let can_fire_ground = self.objects.get(&object_id).map(|attacker| {
                attacker.can_attack()
                    && attacker.can_fire(self.frame as f32 * LOGIC_FRAME_TIMESTEP)
                    && attacker.target_location.is_some()
            }).unwrap_or(false);

            if can_fire_ground {
                if let Some(attacker) = self.objects.get(&object_id) {
                    let shooter_pos = attacker.get_position();
                    let weapon_damage = attacker
                        .weapon
                        .as_ref()
                        .map(|w| w.damage)
                        .unwrap_or(25.0);
                    let target_loc = attacker.target_location.unwrap();
                    super::combat::queue_projectile(super::combat::PendingProjectile {
                        shooter_id: object_id,
                        shooter_pos,
                        target_id: None,
                        target_pos: target_loc,
                        damage: weapon_damage,
                        speed: 200.0,
                    });
                }
                if let Some(attacker) = self.objects.get_mut(&object_id) {
                    if let Some(w) = attacker.weapon.as_mut() {
                        w.last_fire_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                    }
                }
            }
        }
    }

    fn update_object_combat(&mut self, attacker_id: ObjectId, _dt: f32) {
        // Get attacker and target info
        let (weapon_damage, target_id, attacker_team) = {
            if let Some(attacker) = self.objects.get(&attacker_id) {
                if let (Some(weapon), Some(target_id)) = (&attacker.weapon, attacker.target) {
                    (weapon.damage, target_id, attacker.team)
                } else {
                    return;
                }
            } else {
                return;
            }
        };

        // Apply damage to target
        if let Some(target) = self.objects.get_mut(&target_id) {
            let destroyed = target.take_damage(weapon_damage);
            if destroyed {
                log::debug!("Object {} destroyed object {}", attacker_id, target_id);
                // C++ parity: XP based on victim's ExperienceValue.
                let kill_xp = target.thing.template.experience_value
                    * Self::veterancy_xp_multiplier(target.experience.level);
                self.mark_object_for_destruction(target_id, Some(attacker_team));

                // Give experience to attacker
                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    attacker.gain_experience(kill_xp);
                    attacker.stop_attack();
                }
            }
        }
    }

    fn update_player_resources(&mut self, dt: f32) {
        // Calculate power and resource generation for each player
        for (_, player) in self.players.iter_mut() {
            let (power_produced, power_consumed) =
                super::buildings::BuildingBehavior::calculate_power_for_team(
                    player.team,
                    &self.objects,
                );

            let mut income_per_second = 0.0f32;

            // Base passive income -- every player earns a small trickle so they are
            // never completely stuck even before building a supply center.
            // In the full C++ game this comes from supply-truck harvesting; here we
            // provide a simplified equivalent so the economy always moves forward.
            income_per_second += 5.0; // $5/sec base passive income

            // Calculate from buildings
            for (_, obj) in self.objects.iter() {
                if obj.team == player.team && obj.is_constructed() && obj.is_alive() {
                    // Supply centers generate resources
                    if obj.is_kind_of(KindOf::SupplyCenter) {
                        // $25/sec per supply center approximates a single supply
                        // truck's delivery rate (full Chinook ~= $600 / 25s).
                        income_per_second += 25.0;
                    }
                }
            }

            player.power_available = power_produced - power_consumed;
            player.power_produced = power_produced;
            player.power_consumed = power_consumed;

            // C++ parity: check if power sabotage timer has expired and clear it
            // Matches C++ Player::update() sabotage recovery logic
            if player.power_sabotaged_till_frame > 0
                && self.frame > player.power_sabotaged_till_frame
            {
                player.power_sabotaged_till_frame = 0;
            }
            // If power is sabotaged, zero out power production
            if player.power_sabotaged_till_frame > 0 {
                player.power_available = -power_consumed;
            }

            if income_per_second > 0.0 {
                player.income_accumulator += income_per_second * dt;
                let whole = player.income_accumulator.floor() as u32;
                player.income_accumulator -= whole as f32;
                if whole > 0 {
                    player.resources.supplies = player.resources.supplies.saturating_add(whole);
                }
                player.statistics.resources_collected =
                    player.statistics.resources_collected.saturating_add(whole);
            }
        }
    }

    /// C++ parity (Player::update → doPowerDisable): set/clear
    /// `disabled_underpowered` on all KINDOF_POWERED objects depending on
    /// whether their owning player has sufficient power.
    /// C++ parity (ThingTemplate::calcTimeToBuild): compute per-team power
    /// production speed factor based on the energy supply ratio.
    ///
    ///   energy_ratio = produced / max(consumed, 1) clamped to [0,1]
    ///   energy_short = (1.0 - ratio) * LowEnergyPenaltyModifier (0.4)
    ///   rate = max(1.0 - energy_short, MinLowEnergyProductionSpeed (0.5))
    ///   if ratio < 1.0: rate = min(rate, MaxLowEnergyProductionSpeed (0.8))
    fn compute_team_power_factors(&self) -> std::collections::HashMap<Team, f32> {
        const LOW_ENERGY_PENALTY_MODIFIER: f32 = 0.4;
        const MIN_LOW_ENERGY_PRODUCTION_SPEED: f32 = 0.5;
        const MAX_LOW_ENERGY_PRODUCTION_SPEED: f32 = 0.8;

        let mut factors = std::collections::HashMap::new();
        for player in self.players.values() {
            let factor = if player.power_consumed <= 0 {
                1.0
            } else {
                let energy_ratio =
                    (player.power_produced as f32 / player.power_consumed as f32).min(1.0);
                if energy_ratio >= 1.0 {
                    1.0
                } else {
                    let energy_short = (1.0 - energy_ratio) * LOW_ENERGY_PENALTY_MODIFIER;
                    let mut rate = (1.0 - energy_short).max(MIN_LOW_ENERGY_PRODUCTION_SPEED);
                    rate = rate.min(MAX_LOW_ENERGY_PRODUCTION_SPEED);
                    rate
                }
            };
            factors.insert(player.team, factor);
        }
        factors
    }

    /// C++ parity (GarrisonContain::onBodyDamageStateChange): when a garrisoned
    /// building drops below the ReallyDamaged threshold (30% health), all
    /// occupants are force-ejected.  Buildings with `KINDOF_GARRISONABLE_UNTIL_DESTROYED`
    /// are exempt from this evacuation.
    fn check_building_damage_states(&mut self, object_ids: &[ObjectId]) {
        const REALLY_DAMAGED_THRESHOLD: f32 = 0.3;

        // Collect buildings that need evacuation to avoid borrow conflicts.
        let mut evacuate_from: Vec<(ObjectId, Vec3)> = Vec::new();

        for &obj_id in object_ids {
            let Some(obj) = self.objects.get(&obj_id) else {
                continue;
            };
            if !obj.is_alive()
                || !obj.is_constructed()
                || !obj.is_kind_of(KindOf::Structure)
            {
                continue;
            }
            // Skip buildings that are garrisonable until destroyed.
            if obj.is_kind_of(KindOf::Harvestable) {
                continue;
            }
            let Some(building_data) = &obj.building_data else {
                continue;
            };
            if building_data.garrisoned_units.is_empty() {
                continue;
            }
            let health_pct = obj.health.percentage();
            if health_pct > REALLY_DAMAGED_THRESHOLD {
                continue;
            }

            // Only evacuate once: mark as already-evacuated by clearing the
            // garrison list.  We collect positions first to avoid mut borrows.
            let pos = obj.get_position();
            let occupants: Vec<ObjectId> = building_data.garrisoned_units.clone();
            for &occ_id in &occupants {
                evacuate_from.push((occ_id, pos));
            }
        }

        // Eject occupants.
        for (occ_id, building_pos) in evacuate_from {
            // Remove from container first.
            let container_id = self
                .objects
                .values()
                .find(|o| o.contained_units().contains(&occ_id))
                .map(|o| o.id);

            if let Some(cid) = container_id {
                if let Some(container) = self.objects.get_mut(&cid) {
                    container.remove_occupant(occ_id);
                }
            }

            // Move occupant out.
            if let Some(unit) = self.objects.get_mut(&occ_id) {
                let angle = (occ_id.0 as f32).sin().atan2(1.0);
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 8.0;
                unit.stop_moving();
                unit.set_position(building_pos + offset);
                unit.set_target(None);
                unit.contained_by = None;
                unit.ai_state = AIState::Idle;
                unit.status.moving = false;
                unit.status.attacking = false;
            }
        }
    }

    fn update_power_disabled_state(&mut self) {
        // Build a set of teams that are underpowered.
        let mut underpowered_teams: std::collections::HashSet<Team> =
            std::collections::HashSet::new();
        for player in self.players.values() {
            if player.power_available < 0 {
                underpowered_teams.insert(player.team);
            }
        }

        for obj in self.objects.values_mut() {
            if !obj.is_kind_of(KindOf::Powered) {
                continue;
            }
            let should_disable = underpowered_teams.contains(&obj.team)
                && obj.is_alive()
                && obj.is_constructed();
            obj.status.disabled_underpowered = should_disable;
        }
    }

    /// Create a new object
    pub fn create_object(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        if Self::should_skip_map_object_template(template_name) {
            return None;
        }

        if !self.templates.contains_key(template_name) {
            let mut injected = false;
            let should_spawn_fallback = Self::should_spawn_fallback_template(template_name);

            if let Some(template) = Self::build_template_from_asset_definition(template_name) {
                let missing_model = template
                    .model_name
                    .as_deref()
                    .filter(|model| !Self::is_model_asset_available(model))
                    .map(|model| model.to_string());

                if missing_model.is_none() || should_spawn_fallback {
                    self.templates.insert(template_name.to_string(), template);
                    injected = true;
                    log::debug!(
                        "Synthesized template for '{}' from WW3D object definitions",
                        template_name
                    );
                } else if let Some(model) = missing_model {
                    log::debug!(
                        "Falling back for decorative map object template '{}' after unavailable definition model '{}'",
                        template_name,
                        model
                    );
                }
            }

            if !injected {
                if let Some(fallback_template) = Self::build_visual_fallback_template(template_name)
                {
                    let model_name = fallback_template
                        .model_name
                        .clone()
                        .unwrap_or_else(|| template_name.to_string());
                    self.templates
                        .insert(template_name.to_string(), fallback_template);
                    if should_spawn_fallback {
                        log::warn!(
                            "Injected fallback template for unresolved object '{}' using model '{}'",
                            template_name,
                            model_name
                        );
                    } else {
                        log::debug!(
                            "Injected visual-only fallback template for decorative object '{}' using model '{}'",
                            template_name,
                            model_name
                        );
                    }
                } else if !should_spawn_fallback {
                    log::debug!(
                        "Skipping unsupported decorative map object template '{}'",
                        template_name
                    );
                    return None;
                } else {
                    let fallback_template = Self::build_fallback_template(template_name);
                    self.templates
                        .insert(template_name.to_string(), fallback_template);
                    log::warn!(
                        "Injected fallback template for unresolved object '{}'",
                        template_name
                    );
                }
            }
        }

        if let Some(template) = self.templates.get(template_name).cloned() {
            let is_structure = template.is_kind_of(KindOf::Structure);
            let counts_as_unit = Self::template_counts_as_unit(&template);
            let id = self.allocate_object_id();
            let mut object = Object::new(template, id, team);
            object.set_position(position);
            let starts_under_construction = object.status.under_construction;

            // Set up weapon for combat units
            if object.is_kind_of(KindOf::Infantry) || object.is_kind_of(KindOf::Vehicle) {
                object.weapon = Some(Weapon::default());
            }

            self.objects.insert(id, object);
            if counts_as_unit {
                self.record_unit_production(team);
            } else if is_structure && !starts_under_construction {
                self.record_structure_completion(team);
            }
            log::debug!(
                "Created object {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Create object under construction (for buildings)
    pub fn create_object_under_construction(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        if let Some(template) = self.templates.get(template_name).cloned() {
            let id = self.allocate_object_id();
            let mut object = Object::new_under_construction(template, id, team);
            object.set_position(position);

            self.objects.insert(id, object);
            log::debug!(
                "Started construction of {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Destroy an object
    pub fn destroy_object(&mut self, id: ObjectId) {
        self.mark_object_for_destruction(id, None);
    }

    fn mark_object_for_destruction(&mut self, id: ObjectId, killer: Option<Team>) {
        self.objects_to_destroy
            .push_back(DestructionEvent { id, killer });
    }

    /// Find object by ID
    pub fn find_object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    /// Find mutable object by ID
    pub fn find_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(&id)
    }

    /// Find the nearest supply center (refinery/supply dropzone) for a team.
    fn find_nearest_supply_center(
        &self,
        team: Team,
        from_position: Vec3,
    ) -> Option<ObjectId> {
        let mut nearest_id: Option<ObjectId> = None;
        let mut nearest_dist = f32::MAX;

        for (&obj_id, obj) in &self.objects {
            if obj.team != team
                || !obj.is_alive()
                || !obj.is_constructed()
                || !obj.is_kind_of(KindOf::SupplyCenter)
            {
                continue;
            }
            let dist = from_position.distance(obj.get_position());
            if dist < nearest_dist {
                nearest_dist = dist;
                nearest_id = Some(obj_id);
            }
        }
        nearest_id
    }

    /// Get all objects
    pub fn get_objects(&self) -> &HashMap<ObjectId, Object> {
        &self.objects
    }

    /// Get mutable objects
    pub fn get_objects_mut(&mut self) -> &mut HashMap<ObjectId, Object> {
        &mut self.objects
    }

    /// Get all players (for snapshot/save system)
    pub fn get_players(&self) -> &HashMap<u32, Player> {
        &self.players
    }

    /// Get mutable players (for snapshot restoration)
    pub fn get_players_mut(&mut self) -> &mut HashMap<u32, Player> {
        &mut self.players
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> u64 {
        self.frame as u64
    }

    /// Set current frame number (for snapshot restoration)
    pub fn set_current_frame(&mut self, frame: u64) {
        self.frame = frame as u32;
    }

    /// Clear all objects (for snapshot restoration)
    pub fn clear_all_objects(&mut self) {
        self.objects.clear();
        self.next_object_id = ObjectId(1);
    }

    /// Set the next object ID counter (for snapshot restoration).
    pub fn set_next_object_id_for_restore(&mut self, next_object_id: ObjectId) {
        self.next_object_id = next_object_id;
    }

    /// Clear all players (for snapshot restoration)
    pub fn clear_all_players(&mut self) {
        self.players.clear();
    }

    /// Add a player directly (for snapshot restoration)
    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.id, player);
    }

    pub fn command_center_position(&self, team: Team) -> Option<Vec3> {
        let mut fallback = None;
        let mut highest_cost = i32::MIN;

        for obj in self.objects.values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }

            if obj.is_kind_of(KindOf::CommandCenter) {
                return Some(obj.get_position());
            }

            if obj.is_kind_of(KindOf::Structure) {
                let cost = obj.thing.template.build_cost.supplies as i32;
                if cost > highest_cost {
                    highest_cost = cost;
                    fallback = Some(obj.get_position());
                }
            }
        }

        fallback
    }

    /// Get player by ID
    pub fn get_player(&self, player_id: u32) -> Option<&Player> {
        self.players.get(&player_id)
    }

    /// Get mutable player by ID
    pub fn get_player_mut(&mut self, player_id: u32) -> Option<&mut Player> {
        self.players.get_mut(&player_id)
    }

    pub fn get_player_mut_by_team(&mut self, team: Team) -> Option<&mut Player> {
        let key = self
            .players
            .iter()
            .find_map(|(id, p)| if p.team == team { Some(*id) } else { None })?;
        self.players.get_mut(&key)
    }

    pub fn local_player_id(&self) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.is_local)
            .map(|player| player.id)
    }

    pub fn is_local_player(&self, player_id: u32) -> bool {
        self.players
            .get(&player_id)
            .map(|player| player.is_local)
            .unwrap_or(false)
    }

    /// Override a player's display name (used by CLI / networking parity).
    pub fn set_player_name(&mut self, player_id: u32, name: &str) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.name = name.to_string();
            true
        } else {
            false
        }
    }

    /// Override a player's team/faction at runtime (used by menu selection).
    pub fn set_player_team(&mut self, player_id: u32, team: Team) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.team = team;
            true
        } else {
            false
        }
    }

    /// Apply an upgrade tag to an object.
    /// Mirrors C++ behavior where upgrades are persistent object state, not display-name edits.
    pub fn apply_upgrade_to_object(&mut self, object_id: ObjectId, upgrade: &str) {
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.apply_upgrade_tag(upgrade);
        }
    }

    /// Select objects for a player
    pub fn select_objects(&mut self, player_id: u32, object_ids: Vec<ObjectId>) {
        if let Some(player) = self.players.get_mut(&player_id) {
            // Deselect previously selected objects
            for &old_id in &player.selected_objects {
                if let Some(obj) = self.objects.get_mut(&old_id) {
                    obj.deselect();
                }
            }

            // Select new objects
            player.selected_objects.clear();
            for &object_id in &object_ids {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    if obj.team == player.team && obj.is_selectable() {
                        obj.select();
                        player.selected_objects.push(object_id);
                    }
                }
            }

            log::debug!(
                "Player {} selected {} objects",
                player_id,
                player.selected_objects.len()
            );
        }
    }

    /// Issue move command to selected objects (with pathfinding)
    pub fn command_move(&mut self, player_id: u32, target_position: Vec3) {
        if let Some(player) = self.players.get(&player_id) {
            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                let is_mobile = self
                    .objects
                    .get(&object_id)
                    .map(|obj| obj.is_mobile())
                    .unwrap_or(false);
                if is_mobile {
                    self.move_object_with_pathfinding(object_id, target_position, None);
                }
            }
            log::trace!(
                "Player {} commanded {} units to move to {:?}",
                player_id,
                selected.len(),
                target_position
            );
        }
    }

    /// Issue attack command to selected objects
    pub fn command_attack(&mut self, player_id: u32, target_id: ObjectId) {
        if let Some(player) = self.players.get(&player_id) {
            let Some(target_team) = self.objects.get(&target_id).map(|target| target.team) else {
                return;
            };
            if target_team == player.team {
                return;
            }

            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    if obj.can_attack() && obj.team != target_team {
                        obj.set_force_attack(false);
                        obj.attack_target(target_id);
                    }
                }
            }
            log::trace!(
                "Player {} commanded {} units to attack object {}",
                player_id,
                selected.len(),
                target_id
            );
        }
    }

    fn allocate_object_id(&mut self) -> ObjectId {
        let id = self.next_object_id;
        self.next_object_id = ObjectId(self.next_object_id.0 + 1);
        id
    }

    fn process_destroy_list(&mut self) {
        while let Some(event) = self.objects_to_destroy.pop_front() {
            self.pending_special_abilities.remove(&event.id);
            self.pending_special_abilities
                .retain(|_, ability| ability.target_id() != event.id);

            if let Some(obj) = self.objects.remove(&event.id) {
                let eject_origin = obj.get_position();

                // C++ parity (OpenContain::onDie): if DamagePercentToUnits > 0,
                // apply damage to contained units based on their max health.
                let damage_pct = obj
                    .building_data
                    .as_ref()
                    .map(|bd| bd.damage_percent_to_units)
                    .unwrap_or(0.0);

                for (i, contained_id) in obj.contained_units().into_iter().enumerate() {
                    if let Some(unit) = self.objects.get_mut(&contained_id) {
                        // Apply damage before ejection if configured.
                        if damage_pct > 0.0 {
                            let dmg = unit.max_health * damage_pct;
                            let destroyed = unit.take_damage(dmg);
                            if destroyed {
                                unit.status.destroyed = true;
                                self.mark_object_for_destruction(contained_id, event.killer);
                                continue;
                            }
                        }

                        let angle = (contained_id.0 as f32 + i as f32 * 1.11).sin().atan2(1.0)
                            + i as f32 * 0.73;
                        let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 8.0;
                        unit.stop_moving();
                        unit.set_position(eject_origin + offset);
                        unit.set_target(None);
                        unit.contained_by = None;
                        unit.ai_state = AIState::Idle;
                        unit.status.moving = false;
                        unit.status.attacking = false;
                    }
                }

                log::debug!(
                    "Destroyed object {} ({})",
                    event.id,
                    obj.get_template().name
                );
                self.record_destruction(&obj, event.killer);

                // Remove from player selections
                for (_, player) in self.players.iter_mut() {
                    player.selected_objects.retain(|&x| x != event.id);
                }

                // C++ parity: clear stale target references from all other objects.
                // When an object is destroyed, anything targeting it should stop.
                let destroyed_id = event.id;
                for (_, other_obj) in self.objects.iter_mut() {
                    if other_obj.target == Some(destroyed_id) {
                        other_obj.stop_attack();
                    }
                    if other_obj.guard_target == Some(destroyed_id) {
                        other_obj.guard_target = None;
                        if other_obj.ai_state == AIState::GuardingObject {
                            other_obj.ai_state = AIState::Idle;
                        }
                    }
                }
            }
        }
    }

    fn record_destruction(&mut self, destroyed_object: &Object, killer: Option<Team>) {
        let destroyed_is_structure = destroyed_object.is_kind_of(KindOf::Structure);

        if let Some(team) = killer {
            if let Some(player_id) = self.player_id_for_team(team) {
                if let Some(player) = self.players.get_mut(&player_id) {
                    if destroyed_is_structure {
                        player.record_structure_destroyed();
                    } else {
                        player.record_unit_destroyed();
                    }
                }
            }
        }

        if let Some(player_id) = self.player_id_for_team(destroyed_object.team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                if destroyed_is_structure {
                    player.record_structure_lost();
                } else {
                    player.record_unit_lost();
                }
            }
        }
    }

    /// C++ parity: veterancy-level XP multiplier. In C++ each template
    /// defines per-level ExperienceValue; we approximate by scaling the
    /// base value.  C++ values are modest multipliers, not large ones.
    fn veterancy_xp_multiplier(level: VeterancyLevel) -> f32 {
        match level {
            VeterancyLevel::Rookie => 1.0,
            VeterancyLevel::Veteran => 1.25,
            VeterancyLevel::Elite => 1.5,
            VeterancyLevel::Heroic => 2.0,
        }
    }

    fn should_track_player_stats(&self) -> bool {
        self.sim_time_seconds > 0.0 || self.frame > 0
    }

    fn record_unit_production(&mut self, team: Team) {
        if !self.should_track_player_stats() {
            return;
        }
        if let Some(player_id) = self.player_id_for_team(team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_unit_produced();
            }
        }
    }

    fn record_structure_completion(&mut self, team: Team) {
        if !self.should_track_player_stats() {
            return;
        }
        if let Some(player_id) = self.player_id_for_team(team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_structure_built();
            }
        }
    }

    fn template_counts_as_unit(template: &ThingTemplate) -> bool {
        !template.is_kind_of(KindOf::Structure)
            && (template.is_kind_of(KindOf::Infantry)
                || template.is_kind_of(KindOf::Vehicle)
                || template.is_kind_of(KindOf::Aircraft))
    }

    fn should_skip_map_object_template(template_name: &str) -> bool {
        const ILLEGAL_TEMPLATE_NAMES: &[&str] = &[
            "EMPPulseBomb",
            "GLAAngryMobRockProjectileObject",
            "ClusterMinesBomb",
            "BlackNapalmFirestormSmall",
            "CabooseFullOfTerrorists",
            "GLAAngryMobMolotovCocktailProjectileObject",
            "Firestorm",
            "Avalanche",
            "InfernoTankShell",
            "ChinaArtilleryBarrageShell",
            "ChinaTankOverlordBattleBunker",
            "ChinaTankOverlordPropagandaTower",
            "ChinaTankOverlordGattlingCannon",
            "CINE",
            "GLAInfantryAngryMobNexus",
            "AircraftCarrier",
            "GermanMuseum",
            "Cin_",
            "Amb_",
            "Ambient",
            "GC_",
            "SpecialEffectsTrainCrashObject",
            "Scorch",
        ];

        ILLEGAL_TEMPLATE_NAMES.iter().any(|illegal| {
            template_name.starts_with(illegal)
                || template_name.ends_with(illegal)
                || template_name == *illegal
        })
    }

    fn should_spawn_fallback_template(template_name: &str) -> bool {
        if Self::should_skip_map_object_template(template_name) {
            return false;
        }

        let lower = template_name.to_ascii_lowercase();
        lower.contains("tech")
            || lower.contains("supply")
            || lower.contains("oil")
            || lower.contains("bunker")
            || lower.contains("guardtower")
            || lower.contains("tower")
            || lower.contains("commandcenter")
            || lower.contains("refinery")
            || lower.contains("crate")
    }

    fn build_template_from_asset_definition(template_name: &str) -> Option<ThingTemplate> {
        let manager_arc = get_asset_manager()?;
        let remapped_model = Self::remap_known_model_alias(template_name);
        let (definition, texture_hint) = {
            let manager = manager_arc.lock().ok()?;
            let definition = manager
                .resolve_object_definition(template_name, Some(remapped_model.as_str()))
                .or_else(|| manager.resolve_object_definition(template_name, None))
                .cloned()?;
            let texture_hint = manager
                .get_texture_for_object(template_name)
                .or_else(|| manager.get_texture_for_object(remapped_model.as_str()));
            (definition, texture_hint)
        };

        // C++ data includes audio-only ambient map objects with Draw blocks that contain no model.
        // Keep them out of visual spawn synthesis to avoid bogus model fallback loads.
        if definition.model_name.is_none()
            && Self::object_definition_attr(&definition, "soundambient").is_some()
        {
            return None;
        }

        Some(Self::build_template_from_object_definition(
            template_name,
            &definition,
            texture_hint.as_deref(),
        ))
    }

    fn build_template_from_object_definition(
        template_name: &str,
        definition: &ObjectDefinition,
        texture_hint: Option<&str>,
    ) -> ThingTemplate {
        let mut template = ThingTemplate::new(template_name);
        let lower = template_name.to_ascii_lowercase();
        let kind_of = Self::object_definition_attr(definition, "kindof")
            .unwrap_or_default()
            .to_ascii_lowercase();

        if !definition.display_name.is_empty() {
            template.display_name = definition.display_name.clone();
        }

        if let Some(hit_points) = definition.hit_points {
            if hit_points > 0 {
                template.set_health(hit_points as f32);
            }
        }

        if let Some(model_name) = definition.model_name.as_deref() {
            let model_name = model_name.trim();
            if !model_name.is_empty() && !model_name.eq_ignore_ascii_case("none") {
                let resolved_model_name = Self::resolve_spawn_model_name(model_name)
                    .unwrap_or_else(|| Self::remap_known_model_alias(model_name));
                template.set_model(&resolved_model_name);
            }
        }

        let primary_texture = texture_hint.or_else(|| definition.get_primary_texture());
        if let Some(texture_name) = primary_texture {
            let texture_name = texture_name.trim();
            if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                template.texture_name = Some(texture_name.to_string());
            }
        }

        let is_resource = lower.contains("supplypile")
            || lower.contains("crate")
            || kind_of.contains("resource")
            || kind_of.contains("harvest");
        let is_structure = kind_of.contains("structure")
            || kind_of.contains("immobile")
            || (Self::should_spawn_fallback_template(template_name) && !is_resource);

        if is_resource {
            template
                .add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Harvestable);
        }
        if is_structure {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Attackable);
        }
        if kind_of.contains("selectable") || is_structure {
            template.add_kind_of(KindOf::Selectable);
        }
        if kind_of.contains("powered") {
            template.add_kind_of(KindOf::Powered);
        }

        if lower.contains("commandcenter") {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter);
        }
        if lower.contains("supply") && is_structure {
            template.add_kind_of(KindOf::SupplyCenter);
        }

        if template.max_health <= 1.0 {
            template.set_health(if is_structure { 1200.0 } else { 250.0 });
        }

        // C++ parity: parse ExperienceValue from INI (first value = Rookie level).
        // If not set, use a default based on the object type.
        let xp_val = Self::object_definition_attr(definition, "experiencevalue")
            .and_then(|s| s.split_whitespace().next()?.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if is_structure { 100.0 } else { 50.0 }
            });
        template.experience_value = xp_val;

        // C++ parity: parse Armor from INI (default 0).
        if let Some(armor_val) = Self::object_definition_attr(definition, "armor")
            .and_then(|s| s.trim().parse::<f32>().ok())
        {
            template.armor = armor_val;
        }

        // C++ parity: parse VisionRange from INI.
        if let Some(sight) = Self::object_definition_attr(definition, "visionrange")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|&v| v > 0.0)
        {
            template.sight_range = sight;
        }

        // C++ parity: parse BuildCost from INI.
        if let Some(cost) = Self::object_definition_attr(definition, "buildcost")
            .and_then(|s| s.trim().parse::<u32>().ok())
            .filter(|&v| v > 0)
        {
            template.build_cost.supplies = cost;
        }

        template
    }

    fn object_definition_attr(definition: &ObjectDefinition, key: &str) -> Option<String> {
        definition
            .attributes
            .iter()
            .find_map(|(attr, value)| attr.eq_ignore_ascii_case(key).then(|| value.clone()))
    }

    fn remap_known_model_alias(model_name: &str) -> String {
        let model_name_lower = model_name.to_ascii_lowercase();
        if let Some(alias) = Self::remap_pt_vegetation_alias(&model_name_lower) {
            return alias.to_string();
        }

        match model_name_lower.as_str() {
            // Defcon6 / neutral civilian model aliases that do not exist under their INI base id
            // in the mounted archive set, but have shipped equivalents.
            "cbnukebunk2" => "CBNukeBunk".to_string(),
            "pmcrates01" => "PMWldCrate".to_string(),
            "pmcrates03" => "PMWldCrate".to_string(),
            "pmcrat01" => "PMWldCrate".to_string(),
            "pmcrat02" => "PMWldCrate".to_string(),
            "zbsmalpile" => "ZBSmalPile_S".to_string(),
            "cbbunker01" => "CBBunker01_SN".to_string(),
            "cbtower2" => "CBTower2_SN".to_string(),
            "cbtower" => "CBTower01".to_string(),
            "cbtower02" => "CBTower02_SN".to_string(),
            "cbtower03" => "CBTower03_SN".to_string(),
            "cbtower04" => "CBTower03_SN".to_string(),
            "cbtower05" => "CBTower05_N".to_string(),
            "cbtaltower" => "CBTalTower_N".to_string(),
            "cbtaltower_tr" => "CBTalTower_N".to_string(),
            "cbtower01_tr" => "CBTower02_TR".to_string(),
            "cbtower04_tr" => "CBTower03_SN".to_string(),
            "cbtower05_tr" => "CBTower05_N".to_string(),
            "cbtoildepo" => "CBOilRefny".to_string(),
            "cbtoiltnk1" => "CBOilRefny".to_string(),
            "cbtoiltnk2" => "CBOilRefny".to_string(),
            "cboilrfny" => "CBOilRfny_SN".to_string(),
            "cbchembunk" => "CBChemBunk_SN".to_string(),
            "pmwtrtwr" => "PMTower".to_string(),
            "pmwtrtwr02" => "PMTower2".to_string(),
            "pmctrslpy" => "PMDock08".to_string(),
            // ZH-only archive set in this workspace ships ABSupplyCT as the _A2* family.
            // Use a mesh-root variant instead of the animation-root ABSupplyCT_A2 file.
            "absupplyct" => "ABSupplyCT_A2U".to_string(),
            "absupplyct_a2" => "ABSupplyCT_A2U".to_string(),
            "ubsupply" => "UBSupplyF".to_string(),
            "ubcmdhq" => "UBCmdHQ_FA".to_string(),
            "absupdrop" => "PMWldCrate".to_string(),
            "nbsupcent" => "ABSupplyCT_A2U".to_string(),
            "nbconyard" => "NBConYard_FA".to_string(),
            "uvtechjeep" => "UVTechJeep_d4".to_string(),
            "uvtechvan" => "UVTechVan_d1".to_string(),
            "uvtechtrck" => "UVTechTrck_D4".to_string(),
            "nvssupplytk" => "NVSSupplyTk_B".to_string(),
            "nbptower" => "NBPwrPti".to_string(),
            "nbbunker" => "NBBunkerI".to_string(),
            "zbhospibib" => "ZBHospibib_S".to_string(),
            "cbnfcitych" => "CBCityBlok".to_string(),
            "salvagecrate" => "PMWldCrate".to_string(),
            "smalllevelupcrate" => "PMWldCrate".to_string(),
            "mediumlevelupcrate" => "PMWldCrate".to_string(),
            "2freecrusaderscrate" => "PMWldCrate".to_string(),
            "100dollarcrate" => "PMWldCrate".to_string(),
            "200dollarcrate" => "PMWldCrate".to_string(),
            "1000dollarcrate" => "PMWldCrate".to_string(),
            "1500dollarcrate" => "PMWldCrate".to_string(),
            "2500dollarcrate" => "PMWldCrate".to_string(),
            "zzsupplydock" => "PMWldCrate".to_string(),
            "zbsupplydk" => "PMWldCrate".to_string(),
            // Decorative map-object aliases observed in challenge/skirmish maps.
            "pmboulders" => "PMBoulders_D".to_string(),
            "pmlclusters" => "PMLClusters_D".to_string(),
            "pmmcluster" => "PMMCluster_D".to_string(),
            "pmcluster" => "PMCluster_D".to_string(),
            "pmrocks02" | "pmrocks03" | "pmrocks05" | "pmrocks06" | "pmrocks07" => {
                "PMBoulders_D".to_string()
            }
            "pmrocks01b" | "pmrocks02b" => "PMBoulders_D".to_string(),
            // Zero Hour INIs reference a few decorative props whose exact W3D ids are absent from
            // the mounted archive set in this workspace. Route them to the closest shipped props
            // so challenge/shell maps keep their background dressing instead of dropping objects.
            "ptcypress01" => "PTXARBVT01".to_string(),
            "ptxpine03" => "PTXFIR07".to_string(),
            "pmswing" => "PMBikeRack".to_string(),
            "pmplygdst" => "PMPavilion".to_string(),
            // AVChinook_A2 is an animation-root file; route model fallback to renderable mesh.
            "avamphib" | "avamphib_a" | "avamphib_a1" => "AVChinook".to_string(),
            "avchinook_a2" => "AVChinook_A2MSH".to_string(),
            "avpaladin" => "AVCrusader_A".to_string(),
            "avpaladin_d" => "avcrusader_d".to_string(),
            "avpaladin_d1" | "avpaladin_d2" | "avpaladin_d3" => "avcrusader_d1".to_string(),
            "pmtrshpp03" | "pmtrshpl02" => "PMBrnTrshPl_D".to_string(),
            "pmpump" => "PMWldCrate".to_string(),
            "pmcrates" => "PMWldCrate".to_string(),
            "cbsandbw2" => "CBSandBWY1".to_string(),
            "cbsandbw4c" => "CBSandBWX".to_string(),
            "cvtruck" => "CVTruck_D1".to_string(),
            "cbnshack" => "CBNShack_S".to_string(),
            "cbtraintnl" => "UIRTunnel".to_string(),
            _ => model_name.to_string(),
        }
    }

    fn pt_vegetation_alias_mode() -> &'static str {
        static MODE: OnceLock<String> = OnceLock::new();
        MODE.get_or_init(|| {
            std::env::var("GENERALS_PT_VEGETATION_ALIAS_MODE")
                .unwrap_or_else(|_| "all_fir".to_string())
                .to_ascii_lowercase()
        })
        .as_str()
    }

    fn remap_pt_vegetation_alias(model_name_lower: &str) -> Option<&'static str> {
        let tree_target = match Self::pt_vegetation_alias_mode() {
            "trees_birch" | "all_birch" => Some("PTXBirch06"),
            "trees_oak" | "all_oak" => Some("PTXOak06"),
            "trees_palm" | "all_palm" => Some("PTPalm01"),
            "trees_maple" | "all_maple" => Some("PTMaple02"),
            "trees" | "trees_fir" | "all" | "all_fir" | "tree_pine1" | "tree_pine2"
            | "tree_spruce2" | "tree_spruce05" | "trees_pines" | "trees_spruces"
            | "trees_three" | "bushes_pines" | "bushes_spruces" => Some("PTXFir07"),
            _ => None,
        };

        match Self::pt_vegetation_alias_mode() {
            "bushes" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                _ => None,
            },
            "trees" | "trees_fir" | "trees_birch" | "trees_oak" | "trees_palm" | "trees_maple" => {
                match model_name_lower {
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            "tree_pine1" => match model_name_lower {
                "ptpine01" => tree_target,
                _ => None,
            },
            "tree_pine2" => match model_name_lower {
                "ptpine02" => tree_target,
                _ => None,
            },
            "tree_spruce2" => match model_name_lower {
                "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "tree_spruce05" => match model_name_lower {
                "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_pines" => match model_name_lower {
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "trees_spruces" => match model_name_lower {
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_three" => match model_name_lower {
                "ptpine01" | "ptpine02" | "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "bushes_pines" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "bushes_spruces" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "all" | "all_fir" | "all_birch" | "all_oak" | "all_palm" | "all_maple" => {
                match model_name_lower {
                    "ptbush02" => Some("PTBush17"),
                    "ptbush03" => Some("PTBush18"),
                    "ptbush08" => Some("PTBush20"),
                    "ptbush11" => Some("PTBush21"),
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn is_model_asset_available(model_name: &str) -> bool {
        let model_name = model_name.trim();
        if model_name.is_empty() {
            return false;
        }

        let Some(manager_arc) = get_asset_manager() else {
            // Keep gameplay path permissive during early startup or in tests
            // where the asset manager may not be initialized.
            return true;
        };
        let Ok(mut manager) = manager_arc.lock() else {
            return true;
        };

        let w3d_filename = if model_name.to_ascii_lowercase().ends_with(".w3d") {
            model_name.to_string()
        } else {
            format!("{model_name}.w3d")
        };

        let mut candidates = vec![
            format!("art/w3d/{w3d_filename}"),
            format!("Art/W3D/{w3d_filename}"),
            w3d_filename.clone(),
            format!("data/w3d/{w3d_filename}"),
            format!("models/{w3d_filename}"),
        ];
        candidates.push(candidates[0].to_ascii_uppercase());
        candidates.push(candidates[0].to_ascii_lowercase());

        candidates
            .into_iter()
            .any(|candidate| manager.can_open_file_sync(&candidate))
    }

    fn resolve_spawn_model_name(model_name: &str) -> Option<String> {
        static MODEL_RESOLUTION_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> =
            OnceLock::new();

        let remapped = Self::remap_known_model_alias(model_name);
        if Self::is_model_asset_available(&remapped) {
            return Some(remapped);
        }

        let requested_key = Self::normalize_model_lookup_key(&remapped);
        let cache = MODEL_RESOLUTION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(cache) = cache.lock() {
            if let Some(cached) = cache.get(&requested_key) {
                return cached.clone();
            }
        }

        let resolved = {
            let manager_arc = get_asset_manager()?;
            let manager = manager_arc.lock().ok()?;
            let available_models = manager.list_available_models();
            Self::best_available_model_match(&requested_key, available_models.into_iter())
        };

        if let Ok(mut cache) = cache.lock() {
            cache.insert(requested_key, resolved.clone());
        }

        resolved
    }

    fn best_available_model_match<I>(requested_key: &str, available_models: I) -> Option<String>
    where
        I: Iterator<Item = String>,
    {
        let requested_trimmed = Self::trim_model_variant_suffixes(requested_key);
        let requested_signature = Self::compact_model_signature(&requested_trimmed);
        let mut best_match: Option<(i32, String)> = None;

        for available_model in available_models {
            let candidate_key = Self::normalize_model_lookup_key(&available_model);
            let candidate_trimmed = Self::trim_model_variant_suffixes(&candidate_key);
            let candidate_signature = Self::compact_model_signature(&candidate_trimmed);
            let score = if candidate_key == requested_key {
                10_000
            } else if candidate_key.starts_with(requested_key) {
                9_000 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if requested_key.starts_with(&candidate_key) {
                8_800 - (requested_key.len() as i32 - candidate_key.len() as i32).abs()
            } else if candidate_trimmed == requested_trimmed {
                8_400 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if candidate_trimmed.starts_with(&requested_trimmed)
                || requested_trimmed.starts_with(&candidate_trimmed)
            {
                8_000 - (candidate_trimmed.len() as i32 - requested_trimmed.len() as i32).abs()
            } else if !requested_signature.is_empty() && candidate_signature == requested_signature
            {
                7_600 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if !requested_signature.is_empty()
                && candidate_signature.contains(&requested_signature)
            {
                7_200 - (candidate_signature.len() as i32 - requested_signature.len() as i32).abs()
            } else {
                let distance =
                    Self::levenshtein_distance(&requested_signature, &candidate_signature);
                if distance <= 2 {
                    6_000 - distance as i32 * 100
                } else {
                    continue;
                }
            };

            match &best_match {
                Some((best_score, _)) if *best_score >= score => {}
                _ => {
                    let canonical = available_model
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&available_model)
                        .trim_end_matches(".w3d")
                        .trim_end_matches(".W3D")
                        .to_string();
                    best_match = Some((score, canonical));
                }
            }
        }

        best_match.map(|(_, model)| model)
    }

    fn normalize_model_lookup_key(model_name: &str) -> String {
        model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D")
            .to_ascii_lowercase()
    }

    fn trim_model_variant_suffixes(model_key: &str) -> String {
        let mut trimmed = model_key
            .trim_end_matches(|ch: char| ch.is_ascii_digit())
            .to_string();
        for suffix in [
            "_dsng", "_esn", "_rsn", "_dsn", "_sng", "_dsg", "_sg", "_sn", "_dn", "_en", "_rn",
            "_ds", "_es", "_rs", "_ng", "_dg", "_ns", "_s", "_n", "_d", "_e", "_r", "_g", "_a",
            "_b", "_c",
        ] {
            if let Some(stripped) = trimmed.strip_suffix(suffix) {
                trimmed = stripped.to_string();
                break;
            }
        }
        trimmed
    }

    fn compact_model_signature(model_key: &str) -> String {
        model_key
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase()
    }

    fn levenshtein_distance(left: &str, right: &str) -> usize {
        if left == right {
            return 0;
        }
        if left.is_empty() {
            return right.len();
        }
        if right.is_empty() {
            return left.len();
        }

        let left_chars: Vec<char> = left.chars().collect();
        let right_chars: Vec<char> = right.chars().collect();
        let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
        let mut current = vec![0usize; right_chars.len() + 1];

        for (i, left_char) in left_chars.iter().enumerate() {
            current[0] = i + 1;
            for (j, right_char) in right_chars.iter().enumerate() {
                let substitution_cost = usize::from(left_char != right_char);
                current[j + 1] = (previous[j + 1] + 1)
                    .min(current[j] + 1)
                    .min(previous[j] + substitution_cost);
            }
            previous.clone_from_slice(&current);
        }

        previous[right_chars.len()]
    }

    fn build_fallback_template(template_name: &str) -> ThingTemplate {
        let lower = template_name.to_ascii_lowercase();
        let mut template = ThingTemplate::new(template_name);
        template.set_health(250.0);
        let fallback_model_name = Self::resolve_spawn_model_name(template_name)
            .unwrap_or_else(|| Self::remap_known_model_alias(template_name));
        template.set_model(&fallback_model_name);

        if let Some(manager_arc) = get_asset_manager() {
            if let Ok(manager) = manager_arc.lock() {
                let remapped_model = Self::remap_known_model_alias(template_name);
                if let Some(texture_name) = manager
                    .get_texture_for_object(template_name)
                    .or_else(|| manager.get_texture_for_object(remapped_model.as_str()))
                {
                    if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                        template.texture_name = Some(texture_name);
                    }
                }
            }
        }

        let is_resource = lower.contains("supplypile") || lower.contains("crate");
        let is_structure = Self::should_spawn_fallback_template(template_name) && !is_resource;

        if is_resource {
            template
                .add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Harvestable);
        } else if is_structure {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Attackable);
        }

        if lower.contains("commandcenter") {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter);
        }
        if lower.contains("supply") && is_structure {
            template.add_kind_of(KindOf::SupplyCenter);
        }

        template
    }

    fn build_visual_fallback_template(template_name: &str) -> Option<ThingTemplate> {
        let template = Self::build_fallback_template(template_name);
        let model_name = template.model_name.as_deref()?.trim();
        if model_name.is_empty() || !Self::is_model_asset_available(model_name) {
            return None;
        }
        Some(template)
    }

    fn player_id_for_team(&self, team: Team) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.team == team)
            .map(|player| player.id)
    }

    /// Feed Main-crate object positions and sight ranges into the
    /// gamelogic ShroudManager so that fog-of-war reveals around
    /// player-owned units and structures.
    ///
    /// The gamelogic ShroudManager's own `update()` only iterates
    /// objects in the gamelogic OBJECT_REGISTRY; Main-crate objects
    /// are not registered there, so we must push vision directly.
    fn update_main_crate_vision(&self) {
        use gamelogic::common::Coord3D;

        let shroud = get_shroud_manager();
        let mut shroud_mgr = match shroud.lock() {
            Ok(mgr) => mgr,
            Err(_) => return,
        };

        // Build a player_id → bit-mask mapping for do_shroud_reveal.
        let player_ids: Vec<u32> = self.players.keys().copied().collect();

        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }

            let vision_range = obj.get_template().sight_range;
            if vision_range <= 0.0 {
                continue;
            }

            // Find the player_id for this object's team.
            let player_id = match self.player_id_for_team(obj.team) {
                Some(id) => id,
                None => continue,
            };

            let center = Coord3D::new(
                obj.get_position().x,
                obj.get_position().y,
                obj.get_position().z,
            );

            // C++ parity: reveal shroud for all players on the same team
            // (allies share vision).
            let mut player_mask = 0u32;
            for (&pid, player) in &self.players {
                if player.team == obj.team {
                    player_mask |= 1u32 << pid.min(31);
                }
            }
            if player_mask != 0 {
                shroud_mgr.do_shroud_reveal(&center, vision_range, player_mask);
            }
        }
    }

    fn shroud_visibility_snapshot_for_team(
        &self,
        viewing_team: Team,
    ) -> Option<ShroudVisibilitySnapshot> {
        let player_id = self.player_id_for_team(viewing_team)?;
        let shroud_mgr = get_shroud_manager().lock().ok()?;
        let raw_visible_objects = shroud_mgr.get_visible_objects(player_id);

        // Match existing fail-open behavior while shroud has not produced runtime visibility yet.
        let runtime_active =
            shroud_mgr.get_last_update_frame() > 0 || !raw_visible_objects.is_empty();
        if !runtime_active {
            return None;
        }

        // Apply stealth-aware visibility to currently visible objects.
        let mut visible_objects = HashSet::with_capacity(raw_visible_objects.len());
        for object_id in raw_visible_objects {
            if shroud_mgr
                .can_see_object_with_stealth(player_id, object_id)
                .unwrap_or(true)
            {
                visible_objects.insert(object_id);
            }
        }

        Some(ShroudVisibilitySnapshot {
            visible_objects,
            explored_objects: shroud_mgr
                .get_explored_objects(player_id)
                .into_iter()
                .collect(),
        })
    }

    fn is_object_visible_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            snapshot.visible_objects.contains(&id) || snapshot.explored_objects.contains(&id)
        } else {
            true
        }
    }

    fn is_object_visible_on_minimap_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if object.team == viewing_team {
            return true;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            if snapshot.visible_objects.contains(&id) {
                return true;
            }
            // Keep explored structures on minimap for strategic continuity.
            return object.is_kind_of(KindOf::Structure) && snapshot.explored_objects.contains(&id);
        }

        true
    }

    pub fn first_opponent_id(&self, player_id: u32) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.id != player_id)
            .map(|player| player.id)
    }

    pub fn build_victory_summary(&self, winner_id: Option<u32>) -> VictorySummary {
        let mission_name = if self.map_loaded {
            Some(self.map_name.clone())
        } else {
            None
        };

        let duration = if self.sim_time_seconds > 0.0 {
            Some(Duration::from_secs_f32(self.sim_time_seconds))
        } else {
            None
        };

        let mut player_results = Vec::new();
        for player in self.players.values() {
            let outcome = match winner_id {
                Some(id) if id == player.id => PlayerOutcome::Won,
                Some(_) => PlayerOutcome::Lost,
                None => PlayerOutcome::Draw,
            };

            player_results.push(PlayerResult {
                player_id: player.id,
                player_name: player.name.clone(),
                faction: player.team,
                units_built: player.statistics.units_built,
                units_destroyed: player.statistics.units_destroyed,
                units_lost: player.statistics.units_lost,
                structures_built: player.statistics.structures_built,
                structures_destroyed: player.statistics.structures_destroyed,
                structures_lost: player.statistics.structures_lost,
                resources_collected: player.statistics.resources_collected,
                resources_spent: player.statistics.resources_spent,
                outcome,
            });
        }

        VictorySummary {
            mission_name,
            duration,
            player_results,
        }
    }

    fn setup_templates(&mut self) {
        log::debug!("Setting up comprehensive RTS unit templates");

        // ====== USA FACTION UNITS ======

        // USA Infantry
        let mut usa_ranger = ThingTemplate::new("USA_Ranger");
        usa_ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(60.0)
            .set_cost(80, 0)
            .set_model("airanger_s"); // USA Ranger infantry model
        self.templates.insert("USA_Ranger".to_string(), usa_ranger);

        let mut usa_missile_defender = ThingTemplate::new("USA_MissileDefender");
        usa_missile_defender
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0)
            .set_cost(120, 0)
            .set_model("aimissletm"); // USA Missile Defender
        self.templates
            .insert("USA_MissileDefender".to_string(), usa_missile_defender);

        // USA Vehicles
        let mut usa_humvee = ThingTemplate::new("USA_Humvee");
        usa_humvee
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0)
            .set_cost(600, 0)
            .set_model("avhummer"); // USA Humvee vehicle model
        self.templates.insert("USA_Humvee".to_string(), usa_humvee);

        let mut usa_crusader = ThingTemplate::new("USA_CrusaderTank");
        usa_crusader
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(400.0)
            .set_cost(1200, 0)
            .set_model("avcrusader"); // USA Crusader tank
        self.templates
            .insert("USA_CrusaderTank".to_string(), usa_crusader);

        let mut usa_paladin = ThingTemplate::new("USA_PaladinTank");
        usa_paladin
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(600.0)
            .set_cost(1800, 0)
            .set_model("avcrusader"); // USA Paladin tank (using Crusader model since avpaldin doesn't exist)
        self.templates
            .insert("USA_PaladinTank".to_string(), usa_paladin);

        // USA Aircraft
        let mut usa_raptor = ThingTemplate::new("USA_Raptor");
        usa_raptor
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(180.0)
            .set_cost(1000, 0)
            .set_model("avraptorag"); // USA F-22 Raptor
        self.templates.insert("USA_Raptor".to_string(), usa_raptor);

        // ====== GLA FACTION UNITS ======

        // GLA Infantry
        let mut gla_soldier = ThingTemplate::new("GLA_Soldier");
        gla_soldier
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0)
            .set_cost(60, 0)
            .set_model("uirebel"); // GLA Rebel infantry model
        self.templates
            .insert("GLA_Soldier".to_string(), gla_soldier);

        let mut gla_rpg = ThingTemplate::new("GLA_RPGTrooper");
        gla_rpg
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(60.0)
            .set_cost(100, 0)
            .set_model("uirguard02"); // GLA RPG Trooper (using guard model since uirpgtrp doesn't exist)
        self.templates.insert("GLA_RPGTrooper".to_string(), gla_rpg);

        // GLA Vehicles
        let mut gla_technical = ThingTemplate::new("GLA_Technical");
        gla_technical
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_cost(400, 0)
            .set_model("uvtechvan_d1"); // GLA Technical vehicle model
        self.templates
            .insert("GLA_Technical".to_string(), gla_technical);

        let mut gla_scorpion = ThingTemplate::new("GLA_ScorpionTank");
        gla_scorpion
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_cost(900, 0)
            .set_model("uvscorpion"); // GLA Scorpion tank
        self.templates
            .insert("GLA_ScorpionTank".to_string(), gla_scorpion);

        let mut gla_marauder = ThingTemplate::new("GLA_MarauderTank");
        gla_marauder
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(450.0)
            .set_cost(1400, 0)
            .set_model("uvlitetank"); // GLA Marauder tank (using lite tank model since uvmarudr doesn't exist)
        self.templates
            .insert("GLA_MarauderTank".to_string(), gla_marauder);

        // C++ shell scripts and map logic still reference original INI object names.
        // Keep those aliases live so the simplified template table does not change behavior.
        if let Some(base) = self.templates.get("GLA_Soldier").cloned() {
            for alias in ["GLAInfantryRebel", "GLAInfantryTerrorist"] {
                let mut template = base.clone();
                template.name = alias.to_string();
                template.display_name = alias.to_string();
                self.templates.insert(alias.to_string(), template);
            }
        }
        if let Some(base) = self.templates.get("GLA_RPGTrooper").cloned() {
            let mut template = base.clone();
            template.name = "GLAInfantryTunnelDefender".to_string();
            template.display_name = "GLAInfantryTunnelDefender".to_string();
            self.templates
                .insert("GLAInfantryTunnelDefender".to_string(), template);
        }
        if let Some(base) = self.templates.get("GLA_Technical").cloned() {
            let mut template = base;
            template.name = "GLAVehicleCombatBike".to_string();
            template.display_name = "GLAVehicleCombatBike".to_string();
            self.templates
                .insert("GLAVehicleCombatBike".to_string(), template);
        }

        // ====== CHINA FACTION UNITS ======

        // China Infantry
        let mut china_infantry = ThingTemplate::new("China_RedGuard");
        china_infantry
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(55.0)
            .set_cost(70, 0)
            .set_model("uirebel"); // China Red Guard (using rebel model since ciredgrd doesn't exist)
        self.templates
            .insert("China_RedGuard".to_string(), china_infantry);

        let mut china_tank_hunter = ThingTemplate::new("China_TankHunter");
        china_tank_hunter
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(70.0)
            .set_cost(110, 0)
            .set_model("uirguard02"); // China Tank Hunter (using guard model since citankht doesn't exist)
        self.templates
            .insert("China_TankHunter".to_string(), china_tank_hunter);

        // China Vehicles
        let mut china_battlemaster = ThingTemplate::new("China_BattlemasterTank");
        china_battlemaster
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(360.0)
            .set_cost(1100, 0)
            .set_model("uvscorpion"); // China Battlemaster tank (using scorpion model since cvbtlmst doesn't exist)
        self.templates
            .insert("China_BattlemasterTank".to_string(), china_battlemaster);

        let mut china_overlord = ThingTemplate::new("China_OverlordTank");
        china_overlord
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(700.0)
            .set_cost(2000, 0)
            .set_model("nvovrlrdt"); // China Overlord tank (using correct nv pattern model)
        self.templates
            .insert("China_OverlordTank".to_string(), china_overlord);

        // China Aircraft
        let mut china_mig = ThingTemplate::new("China_MiG");
        china_mig
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_cost(900, 0)
            .set_model("nvmign"); // China MiG (using correct nv pattern model)
        self.templates.insert("China_MiG".to_string(), china_mig);

        let mut china_helix = ThingTemplate::new("China_Helix");
        china_helix
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(220.0)
            .set_cost(1200, 0)
            .set_model("avhummer"); // China Helix helicopter (using humvee model since cahelix doesn't exist)
        self.templates
            .insert("China_Helix".to_string(), china_helix);

        // ====== BUILDINGS (SHARED) ======

        let mut command_center = ThingTemplate::new("CommandCenter");
        command_center
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(2000.0)
            .set_cost(2000, 0)
            .set_model("abbtcmdhq"); // USA Command Center model - correct model name
        self.templates
            .insert("CommandCenter".to_string(), command_center);

        let mut supply_center = ThingTemplate::new("SupplyCenter");
        supply_center
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::SupplyCenter)
            .set_health(1000.0)
            .set_cost(1000, 0)
            .set_model("absupplyct_a2"); // USA supply center model
        self.templates
            .insert("SupplyCenter".to_string(), supply_center);

        let mut power_plant = ThingTemplate::new("PowerPlant");
        power_plant
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::PowerPlant)
            .set_health(800.0)
            .set_cost(800, 0)
            .set_model("abpwrplant_d06"); // USA power plant model
        self.templates.insert("PowerPlant".to_string(), power_plant);

        // CRITICAL: Add missing generic building templates that are referenced in the code
        // These templates ensure perfect alignment with C++ implementation expectations

        // Generic Barracks template (matches what's expected by the engine)
        let mut barracks = ThingTemplate::new("Barracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0)
            .set_cost(600, -1)
            .set_model("abbarracks_fa"); // USA barracks model
        self.templates.insert("Barracks".to_string(), barracks);

        // Generic WarFactory template (matches what's expected by the engine)
        let mut war_factory = ThingTemplate::new("WarFactory");
        war_factory
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1500.0)
            .set_cost(1000, -2)
            .set_model("abwarfact_e"); // USA war factory model
        self.templates.insert("WarFactory".to_string(), war_factory);

        // Add faction-specific building templates for complete C++ alignment
        self.add_faction_building_templates();

        log::info!(
            "Set up {} comprehensive RTS unit templates covering all factions",
            self.templates.len()
        );
    }

    fn create_default_players(&mut self) {
        // If map-defined players already exist, keep them; otherwise seed defaults.
        if !self.players.is_empty() {
            return;
        }
        let player1 = Player::new(0, Team::USA, "USA Commander", true);
        let player2 = Player::new(1, Team::GLA, "GLA General", false);
        let player3 = Player::new(2, Team::China, "China Commander", false);

        self.players.insert(0, player1);
        self.players.insert(1, player2);
        self.players.insert(2, player3);

        log::info!(
            "Created {} default players for shell/skirmish bootstrap",
            self.players.len()
        );
    }

    fn create_test_map(&mut self) {
        println!("🗺️ Creating comprehensive RTS test map with faction-aware bases...");

        let mut player_ids: Vec<u32> = self.players.keys().cloned().collect();
        player_ids.sort_unstable();
        let spawn_positions = [
            Vec3::new(-200.0, 0.0, -200.0),
            Vec3::new(200.0, 0.0, 200.0),
            Vec3::new(200.0, 0.0, -200.0),
            Vec3::new(-200.0, 0.0, 200.0),
        ];

        for (idx, player_id) in player_ids.iter().enumerate() {
            let team = self
                .players
                .get(player_id)
                .map(|p| p.team)
                .unwrap_or(Team::Neutral);
            let origin = spawn_positions.get(idx).cloned().unwrap_or(Vec3::ZERO);
            self.spawn_faction_base(team, origin);
        }

        // Neutral center props to mimic tech buildings and abandoned vehicles.
        println!("Adding neutral objectives in center...");
        self.create_object("OilDerrick", Team::Neutral, Vec3::new(0.0, 0.0, 0.0));
        self.create_object("OilRefinery", Team::Neutral, Vec3::new(50.0, 0.0, 0.0));
        self.create_object("TechHospital", Team::Neutral, Vec3::new(-50.0, 0.0, 50.0));
        self.create_object("USA_Humvee", Team::Neutral, Vec3::new(0.0, 0.0, 0.0));
        self.create_object("GLA_Technical", Team::Neutral, Vec3::new(20.0, 0.0, 20.0));

        println!(
            "✅ Comprehensive RTS test map created with {} objects across all factions!",
            self.objects.len()
        );

        // Demonstrate the RTS functionality
        self.demonstrate_rts_features();

        // Set up AI opponents for a proper skirmish match
        self.setup_skirmish_ai(0);

        // Demonstrate AI functionality
        self.demonstrate_ai_functionality();
    }

    fn spawn_faction_base(&mut self, team: Team, origin: Vec3) {
        println!("Creating {:?} base at {:?}", team, origin);
        match team {
            Team::USA => {
                self.create_object("CommandCenter", team, origin);
                self.create_object("SupplyCenter", team, origin + Vec3::new(50.0, 0.0, 50.0));
                self.create_object("PowerPlant", team, origin + Vec3::new(80.0, 0.0, 20.0));

                self.create_object("USA_Ranger", team, origin + Vec3::new(100.0, 0.0, 100.0));
                self.create_object("USA_Ranger", team, origin + Vec3::new(110.0, 0.0, 100.0));
                self.create_object("USA_Ranger", team, origin + Vec3::new(120.0, 0.0, 100.0));
                self.create_object(
                    "USA_MissileDefender",
                    team,
                    origin + Vec3::new(100.0, 0.0, 90.0),
                );
                self.create_object(
                    "USA_MissileDefender",
                    team,
                    origin + Vec3::new(110.0, 0.0, 90.0),
                );

                self.create_object("USA_Humvee", team, origin + Vec3::new(120.0, 0.0, 80.0));
                self.create_object("USA_Humvee", team, origin + Vec3::new(110.0, 0.0, 70.0));
                self.create_object(
                    "USA_CrusaderTank",
                    team,
                    origin + Vec3::new(140.0, 0.0, 60.0),
                );
                self.create_object(
                    "USA_PaladinTank",
                    team,
                    origin + Vec3::new(160.0, 0.0, 50.0),
                );

                self.create_object("USA_Raptor", team, origin + Vec3::new(180.0, 20.0, 40.0));
            }
            Team::GLA => {
                self.create_object("GLA_CommandCenter", team, origin);
                self.create_object("GLA_SupplyStash", team, origin + Vec3::new(0.0, 0.0, 50.0));
                self.create_object("GLA_ArmsDealer", team, origin + Vec3::new(30.0, 0.0, 20.0));

                self.create_object("GLA_Rebel", team, origin + Vec3::new(-10.0, 0.0, -10.0));
                self.create_object("GLA_Rebel", team, origin + Vec3::new(-20.0, 0.0, -10.0));
                self.create_object("GLA_Rebel", team, origin + Vec3::new(-30.0, 0.0, -10.0));
                self.create_object(
                    "GLA_RPGTrooper",
                    team,
                    origin + Vec3::new(-10.0, 0.0, -20.0),
                );
                self.create_object(
                    "GLA_RPGTrooper",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -20.0),
                );

                self.create_object("GLA_Technical", team, origin + Vec3::new(10.0, 0.0, -40.0));
                self.create_object("GLA_Technical", team, origin + Vec3::new(20.0, 0.0, -50.0));
                self.create_object(
                    "GLA_ScorpionTank",
                    team,
                    origin + Vec3::new(0.0, 0.0, -60.0),
                );
                self.create_object(
                    "GLA_MarauderTank",
                    team,
                    origin + Vec3::new(-10.0, 0.0, -60.0),
                );

                self.create_object(
                    "GLA_ScudLauncher",
                    team,
                    origin + Vec3::new(10.0, 0.0, 10.0),
                );
                self.create_object("GLA_Worker", team, origin + Vec3::new(-15.0, 0.0, -15.0));
                self.create_object("GLA_Worker", team, origin + Vec3::new(5.0, 0.0, -10.0));
            }
            Team::China => {
                self.create_object("China_CommandCenter", team, origin);
                self.create_object(
                    "China_SupplyCenter",
                    team,
                    origin + Vec3::new(30.0, 0.0, 30.0),
                );
                self.create_object(
                    "China_NuclearReactor",
                    team,
                    origin + Vec3::new(50.0, 0.0, 10.0),
                );

                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-30.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-40.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_TankHunter",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -30.0),
                );
                self.create_object(
                    "China_TankHunter",
                    team,
                    origin + Vec3::new(-30.0, 0.0, -30.0),
                );

                self.create_object(
                    "China_BattlemasterTank",
                    team,
                    origin + Vec3::new(20.0, 0.0, -20.0),
                );
                self.create_object(
                    "China_BattlemasterTank",
                    team,
                    origin + Vec3::new(10.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_OverlordTank",
                    team,
                    origin + Vec3::new(40.0, 0.0, -40.0),
                );
                self.create_object(
                    "China_DragonTank",
                    team,
                    origin + Vec3::new(30.0, 0.0, -50.0),
                );
                self.create_object(
                    "China_GatlingTank",
                    team,
                    origin + Vec3::new(20.0, 0.0, -60.0),
                );

                self.create_object("China_MiG", team, origin + Vec3::new(60.0, 20.0, -30.0));
                self.create_object("China_Helix", team, origin + Vec3::new(40.0, 25.0, -20.0));
            }
            Team::Neutral => {
                self.create_object("CommandCenter", team, origin);
            }
        }
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.is_paused = paused;
        log::debug!("Game {}", if paused { "paused" } else { "unpaused" });
    }

    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    pub fn world_dimensions(&self) -> (f32, f32) {
        (self.world_width, self.world_height)
    }

    /// Get the current map name
    pub fn get_current_map_name(&self) -> &str {
        &self.map_name
    }

    /// Get total play time for this game session
    pub fn get_total_play_time(&self) -> f32 {
        self.sim_time_seconds
    }

    /// Get the current difficulty setting (based on AI difficulty)
    pub fn get_difficulty(&self) -> AIDifficulty {
        self.ai_manager
            .dominant_difficulty()
            .unwrap_or(AIDifficulty::Medium)
    }

    /// Check if the game is currently in battle
    pub fn is_in_battle(&self) -> bool {
        // Check if any objects are currently in combat
        self.objects
            .values()
            .any(|obj| obj.status.attacking || obj.ai_state == AIState::Attacking)
    }

    pub fn get_world_dimensions(&self) -> (f32, f32) {
        (self.world_width, self.world_height)
    }

    // Command system compatibility methods

    /// Get object by ID
    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    /// Get mutable object by ID
    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(&id)
    }

    /// Add object to the game world
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        let id = object.id;
        self.objects.insert(id, object);
        id
    }

    // ====== ENHANCED RTS COMMAND SYSTEM ======

    /// Get all objects visible to a specific team (for rendering and UI)
    pub fn get_visible_objects(&self, viewing_team: Team) -> Vec<ObjectId> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);
        self.objects
            .iter()
            .filter_map(|(id, obj)| {
                if Self::is_object_visible_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                ) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get visual information for all visible objects
    pub fn get_visual_object_info(
        &self,
        viewing_team: Team,
    ) -> Vec<(ObjectId, super::ObjectVisualInfo)> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);
        self.objects
            .iter()
            .filter_map(|(id, obj)| {
                if Self::is_object_visible_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                ) {
                    Some((*id, obj.get_visual_info()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Select objects within a rectangular area
    pub fn select_objects_in_area(
        &mut self,
        player_id: u32,
        min_pos: Vec3,
        max_pos: Vec3,
        add_to_selection: bool,
    ) -> Vec<ObjectId> {
        if let Some(player) = self.players.get_mut(&player_id) {
            let mut selected_objects = Vec::new();

            // Clear previous selection if not adding
            if !add_to_selection {
                for &old_id in &player.selected_objects {
                    if let Some(obj) = self.objects.get_mut(&old_id) {
                        obj.deselect();
                    }
                }
                player.selected_objects.clear();
            }

            // Find objects in the selection area.
            // C++ parity: uses bounding-circle intersection with the selection
            // rectangle, not just center-point containment.  This allows selecting
            // large objects whose center is outside the box but whose radius
            // overlaps it.
            for (id, obj) in &mut self.objects {
                if obj.team == player.team && obj.is_selectable() {
                    let pos = obj.get_position();
                    let r = obj.selection_radius;
                    // Circle-vs-AABB intersection test.
                    let closest_x = pos.x.clamp(min_pos.x, max_pos.x);
                    let closest_z = pos.z.clamp(min_pos.z, max_pos.z);
                    let dist_sq = (pos.x - closest_x).powi(2) + (pos.z - closest_z).powi(2);
                    if dist_sq <= r * r {
                        obj.select();
                        selected_objects.push(*id);
                        if !player.selected_objects.contains(id) {
                            player.selected_objects.push(*id);
                        }
                    }
                }
            }

            log::trace!(
                "Player {} selected {} objects in area",
                player_id,
                selected_objects.len()
            );
            selected_objects
        } else {
            Vec::new()
        }
    }

    /// Select a single object by click
    pub fn select_object_at_position(
        &mut self,
        player_id: u32,
        position: Vec3,
        selection_radius: f32,
        add_to_selection: bool,
    ) -> Option<ObjectId> {
        if let Some(player) = self.players.get_mut(&player_id) {
            // Find closest selectable object within radius
            let mut closest_object: Option<(ObjectId, f32)> = None;

            for (id, obj) in &self.objects {
                if obj.team == player.team && obj.is_selectable() {
                    let distance = obj.get_position().distance(position);
                    if distance <= selection_radius.max(obj.selection_radius) {
                        if let Some((_, closest_distance)) = closest_object {
                            if distance < closest_distance {
                                closest_object = Some((*id, distance));
                            }
                        } else {
                            closest_object = Some((*id, distance));
                        }
                    }
                }
            }

            if let Some((selected_id, _)) = closest_object {
                // Clear previous selection if not adding
                if !add_to_selection {
                    for &old_id in &player.selected_objects {
                        if let Some(obj) = self.objects.get_mut(&old_id) {
                            obj.deselect();
                        }
                    }
                    player.selected_objects.clear();
                }

                // Select the new object
                if let Some(obj) = self.objects.get_mut(&selected_id) {
                    obj.select();
                    if !player.selected_objects.contains(&selected_id) {
                        player.selected_objects.push(selected_id);
                    }
                }

                log::trace!("Player {} selected object {}", player_id, selected_id);
                Some(selected_id)
            } else {
                // Clear selection if clicking on empty space and not adding
                if !add_to_selection {
                    for &old_id in &player.selected_objects {
                        if let Some(obj) = self.objects.get_mut(&old_id) {
                            obj.deselect();
                        }
                    }
                    player.selected_objects.clear();
                    log::trace!("Player {} cleared selection", player_id);
                }
                None
            }
        } else {
            None
        }
    }

    /// Command selected units to stop all actions
    pub fn command_stop(&mut self, player_id: u32) {
        if let Some(player) = self.players.get(&player_id) {
            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_moving();
                    obj.stop_attack();
                    obj.ai_state = AIState::Idle;
                }
            }
            log::trace!(
                "Player {} commanded {} units to stop",
                player_id,
                selected.len()
            );
        }
    }

    /// Command selected units to attack-move to a position (with pathfinding)
    pub fn command_attack_move(&mut self, player_id: u32, target_position: Vec3) {
        if let Some(player) = self.players.get(&player_id) {
            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                let is_mobile = self
                    .objects
                    .get(&object_id)
                    .map(|obj| obj.is_mobile())
                    .unwrap_or(false);
                if is_mobile {
                    self.move_object_with_pathfinding(
                        object_id,
                        target_position,
                        Some(AIState::AttackMoving),
                    );
                }
            }
            log::trace!(
                "Player {} commanded {} units to attack-move to {:?}",
                player_id,
                selected.len(),
                target_position
            );
        }
    }

    /// Get detailed information about an object (for UI display)
    pub fn get_object_info(&self, object_id: ObjectId) -> Option<ObjectInfo> {
        self.objects.get(&object_id).map(|obj| ObjectInfo {
            id: object_id,
            name: obj.get_display_name(),
            team: obj.team,
            object_type: obj.object_type,
            health: obj.health.clone(),
            max_health: obj.max_health,
            position: obj.get_position(),
            is_selected: obj.selected,
            is_moving: obj.status.moving,
            is_attacking: obj.status.attacking,
            under_construction: obj.status.under_construction,
            construction_percent: obj.construction_percent,
            experience_level: obj.experience.level,
            ai_state: obj.ai_state.clone(),
            can_attack: obj.can_attack(),
            can_move: obj.is_mobile(),
        })
    }

    /// Spawn a unit at the specified position (for testing/cheats)
    pub fn spawn_unit(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        self.create_object(template_name, team, position)
    }

    fn template_team_hint(name: &str) -> Option<Team> {
        let upper = name.to_ascii_uppercase();
        if upper.starts_with("USA_") || upper.starts_with("AMERICA_") {
            Some(Team::USA)
        } else if upper.starts_with("CHINA_") {
            Some(Team::China)
        } else if upper.starts_with("GLA_") {
            Some(Team::GLA)
        } else if upper.starts_with("NEUTRAL_") || upper.starts_with("CIVILIAN_") {
            Some(Team::Neutral)
        } else {
            None
        }
    }

    /// Get available unit/building templates for a team.
    ///
    /// This keeps a broad fallback for generic templates while avoiding obvious
    /// cross-faction leakage for names with clear faction prefixes.
    pub fn get_available_templates(&self, team: Team) -> Vec<String> {
        let mut templates = self
            .templates
            .iter()
            .filter(|(name, template)| {
                // Exclude non-interactive map/decorative templates.
                let is_interactive = template.is_kind_of(KindOf::Selectable)
                    || template.is_kind_of(KindOf::Infantry)
                    || template.is_kind_of(KindOf::Vehicle)
                    || template.is_kind_of(KindOf::Aircraft)
                    || template.is_kind_of(KindOf::Structure)
                    || template.is_kind_of(KindOf::Worker)
                    || template.is_kind_of(KindOf::SupplyCenter)
                    || template.is_kind_of(KindOf::CommandCenter);
                if !is_interactive {
                    return false;
                }

                // Keep generic templates for all teams; faction-tagged names are filtered.
                match Self::template_team_hint(name.as_str()) {
                    Some(hinted_team) => hinted_team == team || team == Team::Neutral,
                    None => true,
                }
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        templates.sort();
        templates
    }

    /// Get templates registry (immutable access)
    pub fn get_templates(&self) -> &HashMap<String, ThingTemplate> {
        &self.templates
    }

    /// Get templates registry (mutable access)
    pub fn get_templates_mut(&mut self) -> &mut HashMap<String, ThingTemplate> {
        &mut self.templates
    }

    /// Demonstrate RTS functionality (for testing)
    pub fn demonstrate_rts_features(&mut self) {
        println!("\n🎮 DEMONSTRATING RTS FUNCTIONALITY:");

        // Show all objects and their status
        println!("\n📊 CURRENT GAME STATE:");
        println!("   Total Objects: {}", self.objects.len());
        println!("   Players: {}", self.players.len());

        // Show objects by team
        for team in [Team::USA, Team::China, Team::GLA, Team::Neutral] {
            let team_objects: Vec<_> = self
                .objects
                .iter()
                .filter(|(_, obj)| obj.team == team && obj.is_alive())
                .collect();

            if !team_objects.is_empty() {
                println!(
                    "\n   {} Team Objects ({}): ",
                    team.get_name(),
                    team_objects.len()
                );
                for (id, obj) in team_objects.iter().take(5) {
                    // Show first 5
                    let health_percent = (obj.health.percentage() * 100.0) as u32;
                    let pos = obj.get_position();
                    println!(
                        "      {} - {} [{}% HP] at ({:.0}, {:.0}, {:.0})",
                        id,
                        obj.get_display_name(),
                        health_percent,
                        pos.x,
                        pos.y,
                        pos.z
                    );
                }
                if team_objects.len() > 5 {
                    println!("      ... and {} more", team_objects.len() - 5);
                }
            }
        }

        // Demonstrate selection
        println!("\n🖱️ TESTING SELECTION SYSTEM:");
        let usa_objects: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if obj.team == Team::USA && obj.is_alive() && obj.is_selectable() {
                    Some(*id)
                } else {
                    None
                }
            })
            .take(3)
            .collect();

        if !usa_objects.is_empty() {
            let local_player = self.local_player_id().unwrap_or(0);
            self.select_objects(local_player, usa_objects.clone());
            println!("   Selected {} USA units", usa_objects.len());
        }

        // Demonstrate movement command
        println!("\n⚡ TESTING MOVEMENT COMMANDS:");
        if let Some(player) = self.players.get(&0) {
            if !player.selected_objects.is_empty() {
                let target_pos = Vec3::new(50.0, 0.0, 50.0);
                self.command_move(0, target_pos);
                println!(
                    "   Commanded selected units to move to ({}, {}, {})",
                    target_pos.x, target_pos.y, target_pos.z
                );
            }
        }

        // Show visual info for rendering
        println!("\n🎨 VISUAL INFORMATION:");
        let visual_info = self.get_visual_object_info(Team::USA);
        println!("   {} objects visible to USA team", visual_info.len());

        for (id, info) in visual_info.iter().take(3) {
            println!(
                "      {} - {} {} [Selected: {}, Health: {:.0}%]",
                id,
                info.team.get_name(),
                if let Some(ref model) = info.model_name {
                    model
                } else {
                    "Unknown"
                },
                info.is_selected,
                info.health_percentage * 100.0
            );
        }

        // Show available templates
        println!("\n🏭 AVAILABLE UNIT TEMPLATES:");
        let templates = self.get_available_templates(Team::USA);
        println!("   {} unit templates available:", templates.len());
        for template in templates.iter().take(8) {
            println!("      - {}", template);
        }
        if templates.len() > 8 {
            println!("      ... and {} more", templates.len() - 8);
        }

        println!("\n✅ RTS FUNCTIONALITY DEMONSTRATION COMPLETE!\n");
    }

    /// Set up AI opponents for skirmish matches
    pub fn setup_skirmish_ai(&mut self, human_player_id: u32) {
        println!("🤖 Setting up AI opponents for skirmish match...");

        // --- Initialize the gamelogic crate AI subsystem ---
        // THE_AI singleton (pathfinder, groups) and the AiIntegrationManager
        // must be initialized before any AI player updates run.
        if let Ok(mut ai) = THE_AI.write() {
            ai.init();
            log::info!("THE_AI singleton initialized for skirmish");
        }
        if let Err(e) = initialize_ai_integration() {
            log::warn!("AiIntegrationManager init failed (non-fatal): {:?}", e);
        }

        // Add AI players for non-human players
        for player_id in 0..4 {
            if player_id == human_player_id {
                continue;
            }

            if let Some(player) = self.players.get(&player_id) {
                // Determine AI difficulty based on player ID (for testing)
                let difficulty = match player_id {
                    1 => AIDifficulty::Medium,
                    2 => AIDifficulty::Hard,
                    3 => AIDifficulty::Easy,
                    _ => AIDifficulty::Medium,
                };

                self.ai_manager
                    .add_ai_player(player_id, player.team, difficulty);
                println!(
                    "  Added AI player {} ({}) with {:?} difficulty",
                    player_id,
                    player.team.get_name(),
                    difficulty
                );
            }
        }

        println!("✅ AI opponents configured for challenging gameplay!");
    }

    /// Enable/disable AI for specific player
    pub fn set_ai_active(&mut self, player_id: u32, active: bool) {
        self.ai_manager.set_ai_active(player_id, active);
    }

    /// Set AI difficulty for a player
    pub fn set_ai_difficulty(&mut self, player_id: u32, difficulty: AIDifficulty) {
        self.ai_manager.set_difficulty(player_id, difficulty);
    }

    /// Get AI status information
    pub fn get_ai_status(&self, player_id: u32) -> Option<String> {
        self.ai_manager.get_ai_info(player_id)
    }

    /// Start skirmish match with AI opponents
    pub fn start_skirmish_match(&mut self, human_team: Team, map_name: &str) {
        println!(
            "🎮 Starting skirmish match: {} vs AI",
            human_team.get_name()
        );

        // Start new game
        self.start_new_game(GameMode::Skirmish);

        // Load map
        self.load_map(map_name);

        // Create human player
        let human_player = Player::new(0, human_team, "Human Player", true);
        self.players.insert(0, human_player);

        // Create AI players with different teams
        let ai_teams = match human_team {
            Team::USA => vec![Team::China, Team::GLA],
            Team::China => vec![Team::USA, Team::GLA],
            Team::GLA => vec![Team::USA, Team::China],
            _ => vec![Team::USA, Team::China, Team::GLA],
        };

        for (i, &team) in ai_teams.iter().enumerate() {
            let ai_player_id = (i + 1) as u32;
            let ai_player = Player::new(
                ai_player_id,
                team,
                &format!("{} AI", team.get_name()),
                false,
            );
            self.players.insert(ai_player_id, ai_player);
        }

        // Set up AI opponents
        self.setup_skirmish_ai(0);

        println!(
            "✅ Skirmish match started with {} AI opponents!",
            ai_teams.len()
        );
    }

    /// Demonstrate AI capabilities
    pub fn demonstrate_ai_functionality(&mut self) {
        println!("\n🤖 DEMONSTRATING AI FUNCTIONALITY:");

        // Show AI status for each AI player
        for player_id in 1..4 {
            if let Some(status) = self.get_ai_status(player_id) {
                println!("\n{}", status);
            }
        }

        // Show AI decision making
        println!("\n🧠 AI DECISION MAKING:");
        println!("   - Economic management: Resource optimization and base construction");
        println!("   - Military strategy: Unit production and attack coordination");
        println!("   - Intelligence gathering: Enemy assessment and reconnaissance");
        println!("   - Base defense: Defensive positioning and threat response");
        println!("   - Advanced tactics: Combined arms and veteran unit management");

        println!("\n✅ AI SYSTEM FULLY OPERATIONAL!\n");
    }

    /// Add comprehensive faction-specific building templates
    /// This ensures perfect alignment with C++ template expectations
    fn add_faction_building_templates(&mut self) {
        log::debug!("Adding faction-specific building templates for C++ alignment");

        // Integrate the comprehensive building templates from buildings.rs
        let building_templates = create_building_templates();
        let template_count = building_templates.len();

        for (name, template) in building_templates {
            self.templates.insert(name, template);
        }

        log::info!(
            "Added {} faction-specific building templates",
            template_count
        );
    }

    /// Initialize script system for mission/level scripting
    /// Called once per map load to set up script engine and load mission scripts
    pub fn initialize_scripts(&mut self, map_name: &str) {
        if self.scripts_loaded {
            return; // Already initialized
        }

        if self.script_engine.is_none() {
            log::debug!("Initializing script system");
            match ScriptingEngine::new() {
                Ok(mut engine) => {
                    let handler: Arc<dyn ScriptActionHandler> = Arc::new(
                        MissionScriptActionHandler::new(self.mission_scripts.clone()),
                    );

                    engine.set_action_handler(Some(Arc::clone(&handler)));
                    let _ = engine.set_game_state_context(self.build_script_game_state_context());
                    self.script_engine = Some(Arc::new(engine));

                    // Also install the handler into the legacy ScriptEngine pipeline that runs INI
                    // mission scripts, so ScriptActions like DISPLAY_TEXT, MOVE_CAMERA_TO, etc. are
                    // delivered to the main runtime.
                    let _ = gamelogic::scripting::engine::initialize_script_engine();
                    if let Ok(mut legacy_guard) =
                        gamelogic::scripting::engine::get_script_engine().write()
                    {
                        if let Some(legacy) = legacy_guard.as_mut() {
                            legacy.set_action_handler(Some(handler));
                        }
                    }

                    log::info!("Scripting engine initialized");
                }
                Err(err) => {
                    log::error!("Failed to initialize scripting engine: {}", err);
                    return;
                }
            }
        }

        match super::script_loader::load_map_scripts(map_name) {
            Ok(Some(result)) => {
                self.loaded_script_lists = result.script_lists;
                self.script_source_path = Some(result.source_path);
                self.scripts_loaded = true;
                self.mission_scripts
                    .install_lists(&self.loaded_script_lists);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();

                // Feed the decoded per-player ScriptLists into the legacy ScriptEngine
                // implementation (gamelogic::scripting::engine) so that `ScriptEngine::update()`
                // runs real mission scripts each frame.
                let _ = gamelogic::scripting::engine::initialize_script_engine();
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        // C++ parity: ScriptEngine::newMap() resets transient script runtime state
                        // on every map load before installing map-owned script lists.
                        engine.reset();
                        for (idx, list) in self.loaded_script_lists.iter().enumerate() {
                            let _ = engine
                                .set_script_list_for_player(idx, Some(Box::new(list.clone())));
                        }
                    }
                }

                log::info!(
                    "Loaded {} mission scripts for '{}'",
                    result.total_scripts,
                    map_name
                );

            }
            Ok(None) => {
                self.loaded_script_lists.clear();
                self.script_source_path = None;
                self.scripts_loaded = true;
                self.mission_scripts.install_lists(&[]);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();

                // Ensure the legacy ScriptEngine doesn't keep running scripts from a previous map.
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        engine.reset();
                    }
                }

                log::warn!("No mission scripts found for '{}'", map_name);
            }
            Err(err) => {
                log::error!(
                    "Failed to decode mission scripts for '{}': {}",
                    map_name,
                    err
                );
                self.mission_scripts.install_lists(&[]);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();

                // On load failures, clear any previously loaded scripts for safety.
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        engine.reset();
                    }
                }
            }
        }
    }

    fn build_script_game_state_context(&self) -> gamelogic::scripting::GameStateContext {
        let players = self
            .players
            .values()
            .map(|player| {
                let color = color_for_player(player.id as u8);
                gamelogic::scripting::PlayerInfo {
                    id: player.id,
                    name: player.name.clone(),
                    team: player.team as u32,
                    color: format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b),
                    is_human: player.is_local,
                    is_alive: player.is_alive,
                    score: 0,
                }
            })
            .collect();

        gamelogic::scripting::GameStateContext {
            map_name: self.map_name.clone(),
            game_mode: format!("{:?}", self.game_mode),
            players,
            objectives: Vec::new(),
        }
    }

    /// Queue an audio event to be processed by the audio system
    /// Mirrors C++ TheAudio->addAudioEvent() pattern
    pub fn queue_audio_event(&mut self, event: AudioEventRequest) {
        self.queued_audio_events.push(event);
    }

    pub fn play_ui_sound(&mut self, event_type: &str) {
        let translated = translate_audio_event(event_type);
        self.queue_audio_event(AudioEventRequest::new(translated));
    }

    /// Process all queued audio events (called once per frame)
    /// In production, this would send events to the audio manager/engine
    fn process_audio_events(&mut self) {
        for event in self.queued_audio_events.drain(..) {
            // Log audio events for debugging
            if let Some(obj_id) = event.object_id {
                if let Some(pos) = event.position {
                    log::trace!(
                        "🔊 Audio: {} at {:?} from object {}",
                        event.event_type, pos, obj_id
                    );
                } else {
                    log::trace!("🔊 Audio: {} from object {}", event.event_type, obj_id);
                }
            } else if let Some(pos) = event.position {
                log::trace!("🔊 Audio: {} at {:?}", event.event_type, pos);
            } else {
                log::trace!("🔊 Audio: {}", event.event_type);
            }

            // Forward to the audio subsystem so events are not lost.
            let _ = crate::subsystem_manager::with_subsystem_mut::<
                crate::subsystem_manager::AudioManagerSubsystem,
                _,
            >(|audio| audio.queue_event(event.clone()));
        }
    }

    /// Evaluate and execute scripts each frame
    /// This is called from the main game loop (update_simulation)
    /// Phase 8 of game loop update sequence (C++ Generals compatibility)
    fn evaluate_and_execute_scripts(&mut self, dt: f32) {
        if !self.scripts_loaded {
            return;
        }

        self.update_script_camera(dt * self.visual_speed_multiplier.max(0.0));

        // Increment script frame counter
        self.mission_script_counter += 1;

        for event in script_events::drain_events() {
            match event {
                ScriptEvent::PlayerDefeated { player_id } => {
                    log::debug!(
                        "📜 Script event: player {} defeated (frame {})",
                        player_id, self.frame
                    );
                    self.partition_manager.reveal_map_for_player(player_id);
                }
                ScriptEvent::RevealMapForPlayer { player_id } => {
                    log::debug!("📜 Script event: reveal map for player {}", player_id);
                    self.partition_manager.reveal_map_for_player(player_id);
                }
                ScriptEvent::AllianceStateChanged { player_id, state } => {
                    log::debug!(
                        "📜 Script event: alliance state {:?} for player {}",
                        state, player_id
                    );
                }
            }

            self.forward_event_to_scripts(&event);
        }

        if let Some(engine) = self.script_engine_handle() {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let in_flight = Arc::clone(&self.script_event_pump_in_flight);
                if !in_flight.swap(true, Ordering::AcqRel) {
                    self.script_event_pump_busy_frames = 0;
                    handle.spawn(async move {
                        if let Err(err) = engine.process_events().await {
                            log::error!("Scripting engine event processing failed: {}", err);
                        }
                        in_flight.store(false, Ordering::Release);
                    });
                } else {
                    self.script_event_pump_busy_frames =
                        self.script_event_pump_busy_frames.saturating_add(1);
                    if self.script_event_pump_busy_frames % 90 == 0 {
                        let pending_events = engine.pending_event_count();
                        log::warn!(
                            "Script event pump busy for {} frames (pending_events={})",
                            self.script_event_pump_busy_frames,
                            pending_events
                        );
                    }
                }
            }
        }

        let mission_runtime_started = Instant::now();
        let mission_update_result = if self.isInShellGame() {
            // Shell/menu mode already has chunked heavy-script evaluation; cap how many
            // scripts we touch per frame so the UI thread cannot stall on long script lists.
            self.mission_scripts
                .update_shell_budgeted(self.frame as u64, Some(SHELL_MISSION_SCRIPT_BUDGET))
        } else {
            self.mission_scripts.update(self.frame as u64)
        };
        if let Err(err) = mission_update_result {
            log::error!("Mission script runtime update failed: {}", err);
        }
        let mission_runtime_elapsed = mission_runtime_started.elapsed();
        if mission_runtime_elapsed >= Duration::from_millis(120) {
            log::warn!(
                "Slow mission script update: {:?} (frame={}, mode={:?})",
                mission_runtime_elapsed,
                self.frame,
                self.game_mode
            );
        }

        self.script_broadcasts
            .retain(|msg| self.sim_time_seconds <= msg.expires_at);

        if self
            .cinematic_text
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.cinematic_text = None;
        }

        if self
            .military_caption
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.military_caption = None;
        }

        for msg in self.mission_scripts.drain_messages() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: msg.clone(),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
            self.new_script_messages.push(msg);
        }

        for sound in self.mission_scripts.drain_sounds() {
            self.play_ui_sound(&sound);
        }

        for sound in self.mission_scripts.drain_sound_events() {
            let translated = translate_audio_event(&sound.sound_name);
            let mut event = AudioEventRequest::new(translated);
            if let Some(pos) = sound.position {
                event = event.with_position(pos);
            }
            self.queue_audio_event(event);
        }

        for camera_target in self.mission_scripts.drain_camera_moves() {
            self.request_camera_focus(camera_target);
        }

        if !self
            .mission_scripts
            .drain_camera_move_to_selection_requests()
            .is_empty()
        {
            if let Some(center) = self.selected_objects_center_for_local_player() {
                self.camera_follow_target = None;
                self.request_camera_focus(center);
            }
        }

        if !self
            .mission_scripts
            .drain_camera_move_home_requests()
            .is_empty()
        {
            if let Some(home) = self.local_player_camera_home_position() {
                self.camera_follow_target = None;
                self.request_camera_focus(home);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_follows()
            .into_iter()
            .last()
        {
            if last.object_id == 0 {
                self.camera_follow_target = None;
            } else {
                self.script_camera_move_to = None;
                self.script_camera_path = None;
                self.camera_follow_target = Some(ObjectId(last.object_id));
                if last.snap_to_unit {
                    if let Some(obj) = self.objects.get(&ObjectId(last.object_id)) {
                        self.request_camera_focus(obj.get_position());
                    }
                }
            }
        }

        if !self
            .mission_scripts
            .drain_camera_mod_freeze_time_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_time();
        }

        if !self
            .mission_scripts
            .drain_camera_mod_freeze_angle_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_angle();
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_final_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_rolling_average_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_rolling_average(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_visual_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_visual_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_script_freeze_time_requests()
            .into_iter()
            .last()
        {
            self.script_time_frozen_by_script = last;
        }

        if let Some(last) = self
            .mission_scripts
            .drain_set_fps_limit_requests()
            .into_iter()
            .last()
        {
            self.apply_set_fps_limit(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_move_to()
            .into_iter()
            .last()
        {
            self.start_camera_move_to(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_path_moves()
            .into_iter()
            .last()
        {
            self.start_camera_path_move(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_set_default_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_default(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_slave_mode_enable_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_slave_mode_enable = Some(last);
            self.pending_camera_slave_mode_disable = false;
        }

        if !self
            .mission_scripts
            .drain_camera_slave_mode_disable_requests()
            .is_empty()
        {
            self.pending_camera_slave_mode_enable = None;
            self.pending_camera_slave_mode_disable = true;
        }

        let screen_shakes = self.mission_scripts.drain_screen_shake_requests();
        if !screen_shakes.is_empty() {
            self.pending_screen_shakes.extend(screen_shakes);
        }

        let camera_shakers = self.mission_scripts.drain_camera_add_shaker_requests();
        if !camera_shakers.is_empty() {
            self.pending_camera_add_shakers.extend(camera_shakers);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_resets()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            self.pending_camera_zoom_reset = true;
            let request = CameraMoveToRequest {
                position: last.position,
                seconds: last.duration_seconds,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            };
            self.start_camera_move_to(request);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_zoom_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_zoom = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_pitch_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_pitch = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_rotate_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = Some(last);
            } else {
                log::debug!("Camera rotate ignored due to active CAMERA_MOD_FREEZE_ANGLE");
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_zoom_requests()
            .into_iter()
            .last()
        {
            let remaining = self.script_camera_remaining_seconds();
            self.pending_camera_zoom = Some(CameraZoomRequest {
                zoom: last.zoom,
                duration_seconds: remaining,
                ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
            });
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_pitch_requests()
            .into_iter()
            .last()
        {
            let remaining = self.script_camera_remaining_seconds();
            self.pending_camera_pitch = Some(CameraPitchRequest {
                pitch: last.pitch,
                duration_seconds: remaining,
                ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
            });
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_setup_requests()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            self.request_camera_focus(last.position);
            self.pending_camera_zoom = Some(CameraZoomRequest {
                zoom: last.zoom,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            self.pending_camera_pitch = Some(CameraPitchRequest {
                pitch: last.pitch,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: last.look_toward,
                    duration_seconds: 0.0,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_waypoint_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(last);
            } else {
                log::debug!(
                    "Camera look toward waypoint ignored due to active CAMERA_MOD_FREEZE_ANGLE"
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_object_requests()
            .into_iter()
            .last()
        {
            if self.is_script_camera_angle_frozen() {
                log::debug!(
                    "Camera look toward object ignored due to active CAMERA_MOD_FREEZE_ANGLE"
                );
            } else if let Some(obj) = self.objects.get(&ObjectId(last.object_id)) {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: obj.get_position(),
                    duration_seconds: last.duration_seconds,
                    ease_in_seconds: last.ease_in_seconds,
                    ease_out_seconds: last.ease_out_seconds,
                    reverse_rotation: false,
                });
            } else {
                log::warn!(
                    "Camera look toward object request ignored; object {} not found",
                    last.object_id
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_look_toward_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: last.position,
                    duration_seconds: 0.0,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            } else {
                log::debug!("Camera mod look toward ignored due to active CAMERA_MOD_FREEZE_ANGLE");
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_look_toward_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                let remaining = self.script_camera_remaining_seconds();
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: last.position,
                    duration_seconds: remaining,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            } else {
                log::debug!(
                    "Camera mod final look toward ignored due to active CAMERA_MOD_FREEZE_ANGLE"
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_letterbox_events()
            .last()
            .copied()
        {
            self.cinematic_letterbox = last;
        }

        if let Some((text, _font, duration_seconds)) = self
            .mission_scripts
            .drain_cinematic_text()
            .into_iter()
            .last()
        {
            let duration = (duration_seconds as f32).max(0.0);
            self.cinematic_text = Some((text, self.sim_time_seconds + duration));
        }

        if let Some(last) = self
            .mission_scripts
            .drain_military_captions()
            .into_iter()
            .last()
        {
            let duration = (last.duration_frames as f32 / 30.0).max(0.0);
            self.military_caption = Some((last.text, self.sim_time_seconds + duration));
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_movie = Some(movie.clone());
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Movie requested: {}", movie),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_radar_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_radar_movie = Some(movie);
        }

        let objective_updates = self.mission_scripts.drain_objective_updates();
        if !objective_updates.is_empty() {
            for update in objective_updates {
                let status = if update.completed {
                    ObjectiveStatus::Completed
                } else {
                    ObjectiveStatus::Active
                };

                let updated_existing = self.with_objective_mut(&update.name, |objective| {
                    objective.title = update.name.clone();
                    objective.description = update.description.clone();
                    objective.status = status;
                });

                if !updated_existing {
                    self.mission_objectives.push(ObjectiveDisplay::new(
                        Some(update.name.clone()),
                        update.name.clone(),
                        update.description.clone(),
                        ObjectiveCategory::Primary,
                    ));
                    let idx = self.mission_objectives.len().saturating_sub(1);
                    self.objective_lookup
                        .insert(update.name.to_ascii_lowercase(), idx);
                }
            }
        }

        for effect in self.mission_scripts.drain_effect_requests() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!(
                    "Effect '{}' at ({:.0}, {:.0}, {:.0})",
                    effect.effect_type, effect.position.x, effect.position.y, effect.position.z
                ),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_radar_enabled_updates()
            .into_iter()
            .last()
        {
            self.radar_enabled = enabled;
        }

        if let Some(visible) = self
            .mission_scripts
            .drain_weather_visibility_updates()
            .into_iter()
            .last()
        {
            self.set_weather_visible(visible);
        }

        let popup_messages = self.mission_scripts.drain_popup_message_requests();
        if !popup_messages.is_empty() {
            #[cfg(feature = "game_client")]
            for popup in &popup_messages {
                game_client::core::script_action_handler::script_popup_message(
                    &popup.message,
                    popup.x_percent,
                    popup.y_percent,
                    popup.width,
                    popup.pause,
                    popup.pause_music,
                );
            }

            for popup in popup_messages {
                if popup.pause {
                    self.set_paused(true);
                }
                if popup.pause_music {
                    self.pending_music_stop = true;
                }
                self.script_broadcasts.push(ScriptBroadcast {
                    text: popup.message.clone(),
                    expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
                });
                self.new_script_messages.push(popup.message.clone());
                self.pending_popup_messages.push(popup);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_view_guardband_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_resize_view_guardband(
                last.x_bias,
                last.y_bias,
            );
            self.pending_view_guardband = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_bw_mode_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_camera_bw_mode(
                last.enabled,
                last.frames,
            );
            self.pending_camera_bw_mode = Some(last);
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_skybox_enabled_updates()
            .into_iter()
            .last()
        {
            self.script_skybox_enabled = enabled;
            {
                let mut global = game_engine::common::global_data::write();
                global.draw_sky_box = enabled;
            }
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_skybox_enabled(enabled);
        }

        for request in self.mission_scripts.drain_camera_motion_blur_requests() {
            #[cfg(feature = "game_client")]
            match &request {
                CameraMotionBlurRequest::Basic { zoom_in, saturate } => {
                    game_client::core::script_action_handler::script_camera_motion_blur(
                        *zoom_in, *saturate,
                    );
                }
                CameraMotionBlurRequest::Jump { position, saturate } => {
                    game_client::core::script_action_handler::script_camera_motion_blur_jump(
                        position.x, position.z, position.y, *saturate,
                    );
                }
                CameraMotionBlurRequest::Follow { amount } => {
                    game_client::core::script_action_handler::script_camera_motion_blur_follow(
                        *amount,
                    );
                }
                CameraMotionBlurRequest::EndFollow => {
                    game_client::core::script_action_handler::script_camera_motion_blur_end_follow(
                    );
                }
            }

            if let CameraMotionBlurRequest::Jump { position, .. } = &request {
                self.camera_follow_target = None;
                self.request_camera_focus(*position);
            }
            self.pending_camera_motion_blur.push(request);
        }

        for flash in self.mission_scripts.drain_cameo_flash_requests() {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_cameo_flash(
                &flash.command_button_name,
                flash.flash_count,
            );
            self.script_cameo_flash_count
                .insert(flash.command_button_name, flash.flash_count);
        }

        for mutation in self.mission_scripts.drain_named_timer_mutations() {
            match mutation {
                NamedTimerMutation::Add {
                    name,
                    text,
                    countdown,
                } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_add_named_timer(
                        &name, &text, countdown,
                    );
                    self.script_named_timers.insert(name, (text, countdown));
                }
                NamedTimerMutation::Remove { name } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_remove_named_timer(&name);
                    self.script_named_timers.remove(&name);
                }
            }
        }

        if let Some(show) = self
            .mission_scripts
            .drain_named_timer_display_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_show_named_timer_display(show);
            self.script_named_timer_display_shown = show;
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_superweapon_display_enabled_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_superweapon_display_enabled(
                enabled,
            );
            self.script_superweapon_display_enabled = enabled;
        }

        for mutation in self
            .mission_scripts
            .drain_superweapon_object_display_mutations()
        {
            match mutation {
                SuperweaponObjectDisplayMutation::Hide { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_hide_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .insert(ObjectId(object_id));
                }
                SuperweaponObjectDisplayMutation::Show { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_show_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .remove(&ObjectId(object_id));
                }
            }
        }

        if !self.mission_scripts.drain_music_stop_requests().is_empty() {
            self.pending_music_stop = true;
        }

        #[cfg(feature = "game_client")]
        {
            if let Some(amount) = self
                .mission_scripts
                .drain_oversize_terrain_requests()
                .into_iter()
                .last()
            {
                if let Ok(mut terrain_guard) =
                    game_client::terrain::terrain_visual::get_terrain_visual()
                {
                    if let Some(visual) = terrain_guard.as_mut() {
                        visual.oversize_terrain(amount);
                    }
                }
            }

            if let Some(level) = self
                .mission_scripts
                .drain_border_shroud_levels()
                .into_iter()
                .last()
            {
                if !game_client::core::script_action_handler::set_script_display_border_shroud_level(
                    level,
                ) {
                    log::warn!(
                        "Border shroud level script request not applied: display bridge unavailable"
                    );
                }
            }
        }
    }

    fn start_camera_path_move(&mut self, request: CameraPathRequest) {
        self.script_camera_move_to = None;
        if let Some(move_state) =
            ScriptCameraPathMove::new(self.script_camera_focus_estimate, &request)
        {
            let mut move_state = move_state;
            if self.script_camera_freeze_time_armed {
                move_state.set_freeze_time(true);
                self.script_camera_freeze_time_armed = false;
            }
            if self.script_camera_freeze_angle_armed {
                move_state.set_freeze_angle(true);
                self.script_camera_freeze_angle_armed = false;
            }
            if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
                move_state.set_final_speed_multiplier(multiplier);
            }
            if let Some(frames) = self.script_camera_pending_rolling_average_frames.take() {
                move_state.set_rolling_average_frames(frames);
            }
            self.mission_scripts.set_camera_movement_finished(false);
            self.script_camera_path = Some(move_state);
        } else {
            self.mission_scripts.set_camera_movement_finished(true);
            self.script_camera_path = None;
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Camera path '{}' not found", request.waypoint),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }
    }

    fn start_camera_move_to(&mut self, request: CameraMoveToRequest) {
        self.mission_scripts.set_camera_movement_finished(false);
        self.script_camera_path = None;
        let mut move_state = ScriptCameraMoveTo::new(self.script_camera_focus_estimate, &request);
        if self.script_camera_freeze_time_armed {
            move_state.set_freeze_time(true);
            self.script_camera_freeze_time_armed = false;
        }
        if self.script_camera_freeze_angle_armed {
            move_state.set_freeze_angle(true);
            self.script_camera_freeze_angle_armed = false;
        }
        if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
            move_state.set_final_speed_multiplier(multiplier);
        }
        self.script_camera_move_to = Some(move_state);
    }

    fn script_camera_remaining_seconds(&self) -> f32 {
        if let Some(move_to) = self.script_camera_move_to.as_ref() {
            return move_to.remaining_time_seconds();
        }
        if let Some(path) = self.script_camera_path.as_ref() {
            return path.remaining_time_seconds();
        }
        0.0
    }

    fn is_script_camera_angle_frozen(&self) -> bool {
        self.script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_angle())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_angle())
                .unwrap_or(false)
    }

    fn apply_script_camera_mod_freeze_time(&mut self) {
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_time(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_freeze_time(true);
            applied = true;
        }
        if !applied {
            self.script_camera_freeze_time_armed = true;
        }
    }

    fn apply_script_camera_mod_freeze_angle(&mut self) {
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_angle(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_freeze_angle(true);
            applied = true;
        }
        if !applied {
            self.script_camera_freeze_angle_armed = true;
        }
    }

    fn apply_script_camera_mod_final_speed_multiplier(
        &mut self,
        request: &CameraModFinalSpeedMultiplierRequest,
    ) {
        let multiplier = request.multiplier as f32;
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if !applied {
            self.script_camera_pending_final_speed_multiplier = Some(multiplier.max(0.0));
        }
    }

    fn apply_script_camera_mod_rolling_average(
        &mut self,
        request: &CameraModRollingAverageRequest,
    ) {
        let frames = request.frames.max(1);
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_rolling_average_frames(frames);
        } else {
            self.script_camera_pending_rolling_average_frames = Some(frames);
        }
    }

    fn apply_visual_speed_multiplier(&mut self, request: &VisualSpeedMultiplierRequest) {
        let multiplier = request.multiplier.max(1) as f32;
        if multiplier.is_finite() {
            self.visual_speed_multiplier = multiplier;
        }
    }

    fn apply_set_fps_limit(&mut self, request: &SetFpsLimitRequest) {
        self.pending_script_fps_limit = Some(request.fps);
    }

    fn apply_script_camera_default(&mut self, request: CameraSetDefaultRequest) {
        self.script_default_camera_pitch = request.pitch;
        // Match C++ W3DView::setDefaultView(): angle is ignored for the active 3D path.
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = if request.max_height.is_finite() {
            request.max_height.max(0.0)
        } else {
            1.0
        };
    }

    fn update_script_camera(&mut self, dt: f32) {
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            self.mission_scripts.set_camera_movement_finished(false);

            if move_to.is_finished() {
                let focus = move_to.final_focus();
                self.request_camera_focus(focus);
                self.script_camera_move_to = None;
                self.mission_scripts.set_camera_movement_finished(true);
                return;
            }

            if let Some(focus) = move_to.advance(dt) {
                self.request_camera_focus(focus);
            }
            return;
        }

        let Some(path_move) = self.script_camera_path.as_mut() else {
            self.mission_scripts.set_camera_movement_finished(true);
            return;
        };

        self.mission_scripts.set_camera_movement_finished(false);

        if path_move.is_finished() {
            let focus = path_move.final_focus();
            self.request_camera_focus(focus);
            self.script_camera_path = None;
            self.mission_scripts.set_camera_movement_finished(true);
            return;
        }

        if let Some(focus) = path_move.advance(dt) {
            self.request_camera_focus(focus);
        }
    }

    /// Update UI state from game logic
    /// This method extracts all data needed for UI rendering each frame
    /// Matches pattern from C++ InGameUI::preDraw() (InGameUI.h line 466)
    pub fn update_ui_state(&mut self, player_id: u32) -> crate::ui::GameUIState {
        use crate::ui::{
            BuildQueueEntry, GameUIState, MinimapDot, RadarMessageEntry, RadarPing,
            RadarPingKind, UnitDisplayInfo,
        };

        // Get player associated with the current viewport/camera
        let player = self.players.get(&player_id);

        let (credits, power_generated, power_used, max_power, credits_per_second) = if let Some(p) =
            player
        {
            let (produced, consumed) =
                super::buildings::BuildingBehavior::calculate_power_for_team(p.team, &self.objects);
            let supply_centers = self
                .objects
                .values()
                .filter(|obj| {
                    obj.team == p.team
                        && obj.is_constructed()
                        && obj.is_alive()
                        && obj.is_kind_of(KindOf::SupplyCenter)
                })
                .count();
            let income = 5.0 + supply_centers as f32 * 25.0;
            (
                p.resources.supplies as i32,
                produced,
                consumed,
                produced,
                income,
            )
        } else {
            (10000, 100, 60, 100, 5.0)
        };

        // Get selected units
        let mut selected_units = Vec::new();
        let mut selected_unit_infos = Vec::new();

        if let Some(player) = player {
            for &object_id in &player.selected_objects {
                selected_units.push(object_id);

                if let Some(obj) = self.objects.get(&object_id) {
                    selected_unit_infos.push(UnitDisplayInfo {
                        object_id,
                        name: obj.name.clone(),
                        health_current: obj.health.current,
                        health_maximum: obj.health.maximum,
                        unit_type: format!("{:?}", obj.object_type),
                        current_order: if obj.target.is_some() {
                            "Attacking".to_string()
                        } else if obj.movement.target_position.is_some() {
                            "Moving".to_string()
                        } else {
                            "Idle".to_string()
                        },
                    });
                }
            }
        }

        // Get build queues (from all constructing buildings)
        let mut build_queue = Vec::new();
        for (_id, obj) in &self.objects {
            if obj.status.under_construction {
                // Estimate time remaining based on construction percent (assuming 30 second build time)
                let estimated_total_time = 30.0;
                let time_remaining = estimated_total_time * (1.0 - obj.construction_percent);

                build_queue.push(BuildQueueEntry {
                    template_name: obj.name.clone(),
                    percent_complete: obj.construction_percent,
                    time_remaining,
                });
            }
        }

        // Generate minimap dots for all units
        let mut minimap_unit_dots = Vec::new();
        let (world_min, world_max) = self.world_bounds();
        let world_span_x = (world_max.x - world_min.x).max(1.0);
        let world_span_z = (world_max.z - world_min.z).max(1.0);
        let viewing_team = player.map(|p| p.team).unwrap_or(Team::Neutral);
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);

        for (id, obj) in &self.objects {
            if obj.is_alive()
                && (obj.is_kind_of(KindOf::Selectable) || obj.is_kind_of(KindOf::Structure))
                && Self::is_object_visible_on_minimap_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                )
            {
                // Normalize position to 0.0-1.0 range based on world dimensions
                let normalized_x = ((obj.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
                let normalized_y = ((obj.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);

                let color = match obj.team {
                    Team::USA => color_for_player(1),
                    Team::China => color_for_player(0),
                    Team::GLA => color_for_player(4),
                    Team::Neutral => color_for_player(7),
                };

                let size = if obj.is_kind_of(KindOf::Structure) {
                    4.0
                } else {
                    2.0
                };

                minimap_unit_dots.push(MinimapDot::normalized(normalized_x, normalized_y, color, size));
            }
        }

        let mut minimap_beacons = Vec::new();
        for beacon in snapshot_beacons() {
            let normalized_x = ((beacon.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
            let normalized_y = ((beacon.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);
            minimap_beacons.push(MinimapDot::normalized(
                normalized_x,
                normalized_y,
                color_for_player(beacon.player_id as u8),
                4.0,
            ));
        }

        // Use WW3D-synchronized time
        let game_time = self.sim_time_seconds;

        let player_name = player
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("Commander {}", player_id + 1));

        let mut ui_state = GameUIState::default();
        ui_state.credits = credits;
        ui_state.power_generated = power_generated;
        ui_state.power_used = power_used;
        ui_state.max_power = max_power;
        ui_state.credits_per_second = credits_per_second;
        ui_state.player_id = player_id;
        ui_state.player_name = player_name;
        ui_state.selected_units = selected_units;
        ui_state.selected_unit_infos = selected_unit_infos;
        ui_state.build_queue = build_queue;
        ui_state.is_game_paused = self.is_paused;
        ui_state.current_game_time = game_time;
        ui_state.fps = LOGIC_FRAMES_PER_SECOND;
        ui_state.frame_time_ms = 1000.0 / LOGIC_FRAMES_PER_SECOND;
        ui_state.performance_score = 1.0;
        ui_state.minimap_unit_dots = minimap_unit_dots;
        ui_state.minimap_beacons = minimap_beacons.clone();
        ui_state.new_beacons = std::mem::take(&mut self.recent_beacons);
        ui_state.minimap_viewport = crate::ui::default_minimap_viewport();
        ui_state.minimap_texture_id = None;
        ui_state.minimap_coordinates = Some(crate::graphics::MinimapCoordinates {
            minimap_width: 1.0,
            minimap_height: 1.0,
            world_min,
            world_max,
            screen_pos: Vec2::ZERO,
        });

        // Pull fresh radar updates from GameLogic (typed) and turn them into HUD/radar pings.
        for update in radar_notifier::drain() {
            let pos_world = Vec3::new(update.position.0, 0.0, update.position.1);
            match update.event_type {
                RadarEventType::BaseAttacked => {
                    self.queue_radar_attack_at("Base under attack", pos_world);
                }
                RadarEventType::EnemyDetected => {
                    self.queue_radar_message_at(
                        "Enemy detected",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::UnitCreated => {
                    self.queue_radar_message_at(
                        "Unit ready",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::UnitDestroyed => {
                    self.queue_radar_message_at(
                        "Unit lost",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::BeaconPlaced | RadarEventType::BeaconRemoved => {
                    // Beacon events are already handled via beacon manager; skip to avoid duplicates.
                }
            }
        }

        let radar_entries = self.radar_notifications.drain();
        const RADAR_PING_LIFETIME: f32 = 6.0;
        let mut latest_by_kind: [Option<RadarEntry>; 3] = [None, None, None];
        ui_state.radar_messages = radar_entries
            .iter()
            .map(|entry| entry.text.clone())
            .collect();
        ui_state.radar_events = radar_entries
            .iter()
            .map(|entry| RadarMessageEntry {
                text: entry.text.clone(),
                position: Some(entry.position),
                kind: match entry.kind {
                    radar_notifications::RadarKind::Generic => RadarPingKind::Generic,
                    radar_notifications::RadarKind::Attack => RadarPingKind::Attack,
                    radar_notifications::RadarKind::Ally => RadarPingKind::Ally,
                },
            })
            .collect();
        ui_state.radar_pings = radar_entries
            .iter()
            .filter_map(|entry| {
                let age = (self.sim_time_seconds - entry.timestamp).max(0.0);
                if age > RADAR_PING_LIFETIME {
                    return None;
                }
                // Fade out linearly and add a soft pulse to mimic C++ radar blips.
                let normalized = (1.0 - age / RADAR_PING_LIFETIME).clamp(0.0, 1.0);
                let pulse = 0.5 * (1.0 + (age * std::f32::consts::TAU).cos());
                let intensity = (normalized * 0.6 + pulse * 0.4).clamp(0.0, 1.0);
                Some(RadarPing {
                    position: entry.position,
                    intensity,
                    age_seconds: age,
                    kind: match entry.kind {
                        radar_notifications::RadarKind::Generic => RadarPingKind::Generic,
                        radar_notifications::RadarKind::Attack => RadarPingKind::Attack,
                        radar_notifications::RadarKind::Ally => RadarPingKind::Ally,
                    },
                })
            })
            .collect();
        for entry in radar_entries {
            let idx = match entry.kind {
                radar_notifications::RadarKind::Generic => 0,
                radar_notifications::RadarKind::Attack => 1,
                radar_notifications::RadarKind::Ally => 2,
            };
            let slot = &mut latest_by_kind[idx];
            if slot
                .as_ref()
                .map(|e| entry.timestamp >= e.timestamp)
                .unwrap_or(true)
            {
                *slot = Some(entry);
            }
        }
        if let Some(entry) = latest_by_kind
            .iter()
            .filter_map(|e| e.as_ref())
            .max_by(|a, b| {
                a.timestamp
                    .partial_cmp(&b.timestamp)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            self.last_radar_event = Some(entry.clone());
        }
        ui_state.last_radar_ping = self.last_radar_event.as_ref().map(|e| e.position);
        ui_state.script_messages = self
            .script_broadcasts
            .iter()
            .map(|msg| msg.text.clone())
            .collect();
        ui_state.cinematic_letterbox = self.cinematic_letterbox;
        ui_state.cinematic_text = self.cinematic_text.as_ref().map(|(text, _)| text.clone());
        ui_state.military_caption = self.military_caption.as_ref().map(|(text, _)| text.clone());
        ui_state.radar_enabled = self.radar_enabled;
        ui_state.objectives = self.mission_objectives.clone();
        ui_state
    }

    pub fn take_new_script_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.new_script_messages)
    }

    /// Queue a command from the UI
    pub fn queue_command(&mut self, command: crate::command_system::GameCommand) {
        log::trace!("Queuing command: {:?}", command.command_type);
        self.command_queue.push_back(command);
    }

    /// Process queued commands
    pub fn process_commands(&mut self) {
        // Process all queued commands
        while let Some(command) = self.command_queue.pop_front() {
            self.execute_command(command);
        }
    }

    /// Snapshot number of active beacons (used by HUD to clear highlights).
    pub fn beacon_count(&self) -> usize {
        snapshot_beacons().len()
    }

    /// Enqueue unit production on a building if permitted.
    pub fn enqueue_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        let template = match self.templates.get(&template_name) {
            Some(t) => t.clone(),
            None => return false,
        };
        if let Some(producer) = self.objects.get(&producer_id) {
            let team = producer.team;
            // Validate the producer can build this template before charging resources.
            if let Some(building) = &producer.building_data {
                if !building.can_produce(&template) {
                    return false;
                }
            } else {
                return false;
            }
            if let Some(player) = self.get_player_mut_by_team(team) {
                if !player.spend_resources(&template.build_cost) {
                    return false;
                }
            }
        }

        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                return building.add_to_queue(template_name, &template);
            }
        }
        false
    }

    /// Cancel a queued production item by template name (first match).
    pub fn cancel_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        let mut refund: Option<Resources> = None;
        let team = self.objects.get(&producer_id).map(|p| p.team);
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if let Some(pos) = building
                    .production_queue
                    .iter()
                    .position(|item| item.template_name == template_name)
                {
                    refund = building.cancel_production(pos).map(|item| item.cost);
                }
            }
        }

        if let (Some(cost), Some(team)) = (refund, team) {
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.resources.supplies += cost.supplies;
                player.power_available -= cost.power;
            }
            return true;
        }

        false
    }

    pub fn queue_radar_message<S: Into<String>>(&mut self, message: S) {
        self.queue_radar_message_at(message, Vec3::ZERO, radar_notifications::RadarKind::Generic);
    }

    pub fn queue_radar_message_at<S: Into<String>>(
        &mut self,
        message: S,
        position: Vec3,
        kind: radar_notifications::RadarKind,
    ) {
        let kind_index = match kind {
            radar_notifications::RadarKind::Generic => 0,
            radar_notifications::RadarKind::Attack => 1,
            radar_notifications::RadarKind::Ally => 2,
        };
        const RADAR_DEDUP_WINDOW: f32 = 0.5;
        if self.sim_time_seconds - self.last_radar_kind_time[kind_index] < RADAR_DEDUP_WINDOW {
            // Drop duplicate of same kind emitted too fast.
            return;
        }
        let entry = RadarEntry {
            text: message.into(),
            position,
            timestamp: self.sim_time_seconds,
            kind,
        };
        self.radar_notifications.push(entry.clone());
        self.last_radar_event = Some(entry);
        self.last_radar_kind_time[kind_index] = self.sim_time_seconds;

        // Trigger the classic radar/EVA audio cue to mirror the C++ client feedback.
        self.maybe_play_radar_audio("Radar_Event");
    }

    /// Radar attack warning at a location (plays distinct EVA cue).
    pub fn queue_radar_attack_at<S: Into<String>>(&mut self, message: S, position: Vec3) {
        self.queue_radar_message_at(message, position, radar_notifications::RadarKind::Attack);
        self.maybe_play_radar_audio("Radar_Attack");
    }

    /// Radar ally request cue.
    pub fn queue_radar_ally<S: Into<String>>(&mut self, message: S) {
        self.queue_radar_message_at(message, Vec3::ZERO, radar_notifications::RadarKind::Ally);
        self.maybe_play_radar_audio("Radar_Ally");
    }

    pub fn queue_radar_message_for_team<S: Into<String>>(&mut self, team: Team, message: S) {
        if let Some(position) = self.command_center_position(team) {
            self.queue_radar_message_at(message, position, radar_notifications::RadarKind::Generic);
        } else {
            self.queue_radar_message(message);
        }
    }

    /// Track a newly placed beacon so the UI can bloom/highlight it this frame.
    pub fn note_beacon_placed(&mut self, position: Vec3) {
        self.recent_beacons.push(position);
    }

    /// Play radar audio with a short cooldown to avoid stacking duplicates if many events fire simultaneously.
    fn maybe_play_radar_audio(&mut self, cue: &str) {
        const RADAR_AUDIO_COOLDOWN: f32 = 1.0;
        if self.sim_time_seconds - self.last_radar_audio_time >= RADAR_AUDIO_COOLDOWN {
            self.queue_audio_event(AudioEventRequest::new(translate_audio_event(cue)));
            self.last_radar_audio_time = self.sim_time_seconds;
        }
    }

    pub fn last_radar_event_position(&self) -> Option<Vec3> {
        self.last_radar_event.as_ref().map(|entry| entry.position)
    }

    pub fn request_camera_focus(&mut self, position: Vec3) {
        static DEBUG_CAMERA_FOCUS_LOGS: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        if DEBUG_CAMERA_FOCUS_LOGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 24 {
            log::trace!(
                "DEBUG_SHELL_CAMERA_BRIDGE: request_camera_focus position={position:?}"
            );
        }
        self.pending_camera_focus = Some(position);
        self.script_camera_focus_estimate = position;
    }

    fn selected_objects_center_for_local_player(&self) -> Option<Vec3> {
        let local_player_id = self.local_player_id()?;
        let player = self.players.get(&local_player_id)?;
        if player.selected_objects.is_empty() {
            return None;
        }

        let mut count = 0usize;
        let mut sum = Vec3::ZERO;
        for object_id in &player.selected_objects {
            let Some(obj) = self.objects.get(object_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            sum += obj.get_position();
            count += 1;
        }

        if count == 0 {
            None
        } else {
            Some(sum / count as f32)
        }
    }

    fn local_player_camera_home_position(&self) -> Option<Vec3> {
        let local_player_id = self.local_player_id()?;
        let team = self.players.get(&local_player_id)?.team;
        self.command_center_position(team)
            .or_else(|| self.team_base_position(team))
    }

    pub fn take_camera_focus_request(&mut self) -> Option<Vec3> {
        self.pending_camera_focus.take()
    }

    pub fn script_default_camera_pitch(&self) -> f32 {
        self.script_default_camera_pitch
    }

    pub fn script_default_camera_max_height(&self) -> f32 {
        self.script_default_camera_max_height
    }

    pub fn visual_speed_multiplier(&self) -> f32 {
        self.visual_speed_multiplier
    }

    pub fn is_script_camera_time_frozen(&self) -> bool {
        self.script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_time())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_time())
                .unwrap_or(false)
    }

    pub fn take_camera_zoom_reset(&mut self) -> bool {
        std::mem::take(&mut self.pending_camera_zoom_reset)
    }

    pub fn take_camera_zoom_request(&mut self) -> Option<CameraZoomRequest> {
        self.pending_camera_zoom.take()
    }

    pub fn take_camera_pitch_request(&mut self) -> Option<CameraPitchRequest> {
        self.pending_camera_pitch.take()
    }

    pub fn take_camera_rotate_request(&mut self) -> Option<CameraRotateRequest> {
        self.pending_camera_rotate.take()
    }

    pub fn take_camera_look_toward_request(&mut self) -> Option<CameraLookTowardWaypointRequest> {
        self.pending_camera_look_toward.take()
    }

    pub fn take_camera_slave_mode_enable_request(&mut self) -> Option<CameraSlaveModeRequest> {
        self.pending_camera_slave_mode_enable.take()
    }

    pub fn take_camera_slave_mode_disable_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_camera_slave_mode_disable)
    }

    pub fn take_screen_shake_requests(&mut self) -> Vec<ScreenShakeRequest> {
        std::mem::take(&mut self.pending_screen_shakes)
    }

    pub fn take_camera_add_shaker_requests(&mut self) -> Vec<CameraAddShakerRequest> {
        std::mem::take(&mut self.pending_camera_add_shakers)
    }

    pub fn take_popup_message_requests(&mut self) -> Vec<ScriptPopupMessageRequest> {
        std::mem::take(&mut self.pending_popup_messages)
    }

    pub fn take_view_guardband_request(&mut self) -> Option<ViewGuardbandRequest> {
        self.pending_view_guardband.take()
    }

    pub fn take_camera_bw_mode_request(&mut self) -> Option<CameraBwModeRequest> {
        self.pending_camera_bw_mode.take()
    }

    pub fn take_camera_motion_blur_requests(&mut self) -> Vec<CameraMotionBlurRequest> {
        std::mem::take(&mut self.pending_camera_motion_blur)
    }

    pub fn take_music_stop_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_music_stop)
    }

    pub fn take_script_fps_limit_request(&mut self) -> Option<i32> {
        self.pending_script_fps_limit.take()
    }

    pub fn is_script_time_frozen(&self) -> bool {
        self.script_time_frozen_by_script
    }

    pub fn is_time_frozen_for_simulation(&self) -> bool {
        self.is_script_time_frozen() || self.is_script_camera_time_frozen()
    }

    pub fn camera_follow_target_position(&mut self) -> Option<Vec3> {
        let target = self.camera_follow_target?;
        let Some(obj) = self.objects.get(&target) else {
            self.camera_follow_target = None;
            return None;
        };
        if !obj.is_alive() {
            self.camera_follow_target = None;
            return None;
        }
        Some(obj.get_position())
    }

    /// Execute a single command
    fn execute_command(&mut self, command: crate::command_system::GameCommand) {
        let command_type = command.command_type.clone();
        let mut executor = crate::command_executor::CommandExecutor::new(self, command.player_id);

        match executor.execute_command(command) {
            Ok(crate::command_system::CommandResult::Success) => {}
            Ok(result) => {
                log::debug!(
                    "[GameLogic] Command {:?} completed with {:?}",
                    command_type,
                    result
                );
            }
            Err(err) => {
                log::warn!(
                    "[GameLogic] Failed to execute command {:?}: {}",
                    command_type,
                    err
                );
            }
        }
    }
}

impl GameLogic {
    fn update_player_alive_state(&mut self) {
        for player in self.players.values_mut() {
            player.is_alive = self
                .objects
                .values()
                .any(|obj| obj.team == player.team && obj.is_alive());
        }
    }

    pub fn evaluate_victory_condition(&mut self) -> Option<VictoryCondition> {
        self.update_player_alive_state();
        self.victory_conditions
            .evaluate(&self.players, &self.objects, self.frame)
    }

    pub fn take_defeat_events(&mut self) -> Vec<u32> {
        self.victory_conditions.take_defeat_events()
    }

    pub fn take_alliance_events(&mut self) -> Vec<AllianceNotification> {
        self.victory_conditions.take_alliance_events()
    }
}

/// Detailed object information for UI display
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub id: ObjectId,
    pub name: String,
    pub team: Team,
    pub object_type: ObjectType,
    pub health: Health,
    pub max_health: f32,
    pub position: Vec3,
    pub is_selected: bool,
    pub is_moving: bool,
    pub is_attacking: bool,
    pub under_construction: bool,
    pub construction_percent: f32,
    pub experience_level: VeterancyLevel,
    pub ai_state: AIState,
    pub can_attack: bool,
    pub can_move: bool,
}

#[derive(Clone)]
struct ShroudVisibilitySnapshot {
    visible_objects: HashSet<u32>,
    explored_objects: HashSet<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure_test_tank_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestTank") {
            return;
        }

        let mut test_tank = ThingTemplate::new("TestTank");
        test_tank
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0)
            .set_cost(600, 0);
        game_logic
            .templates
            .insert("TestTank".to_string(), test_tank);
    }

    fn ensure_test_dozer_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestDozer") {
            return;
        }

        let mut test_dozer = ThingTemplate::new("TestDozer");
        test_dozer
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0)
            .set_cost(1000, 0);
        game_logic
            .templates
            .insert("TestDozer".to_string(), test_dozer);
    }

    fn ensure_test_infantry_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestInfantry") {
            return;
        }

        let mut test_infantry = ThingTemplate::new("TestInfantry");
        test_infantry
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(80.0)
            .set_cost(100, 0);
        game_logic
            .templates
            .insert("TestInfantry".to_string(), test_infantry);
    }

    fn ensure_test_aircraft_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestAircraft") {
            return;
        }

        let mut test_aircraft = ThingTemplate::new("TestAircraft");
        test_aircraft
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(220.0)
            .set_cost(1200, 0);
        game_logic
            .templates
            .insert("TestAircraft".to_string(), test_aircraft);
    }

    fn ensure_test_structure_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestBuilding") {
            return;
        }

        let mut test_building = ThingTemplate::new("TestBuilding");
        test_building
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1200.0)
            .set_cost(500, -1);
        game_logic
            .templates
            .insert("TestBuilding".to_string(), test_building);
    }

    fn ensure_test_command_center_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestCommandCenter") {
            return;
        }

        let mut command_center = ThingTemplate::new("TestCommandCenter");
        command_center
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1800.0)
            .set_cost(2000, -10);
        game_logic
            .templates
            .insert("TestCommandCenter".to_string(), command_center);
    }

    fn ensure_test_barracks_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestBarracks") {
            return;
        }

        let mut barracks = ThingTemplate::new("TestBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_cost(600, -1);
        game_logic
            .templates
            .insert("TestBarracks".to_string(), barracks);
    }

    fn ensure_test_repair_pad_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestRepairPad") {
            return;
        }

        let mut repair_pad = ThingTemplate::new("TestRepairPad");
        repair_pad
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_cost(500, -1);
        game_logic
            .templates
            .insert("TestRepairPad".to_string(), repair_pad);
    }

    fn ensure_test_heal_pad_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestHealPad") {
            return;
        }

        let mut heal_pad = ThingTemplate::new("TestHealPad");
        heal_pad
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(900.0)
            .set_cost(400, -1);
        game_logic
            .templates
            .insert("TestHealPad".to_string(), heal_pad);
    }

    fn ensure_test_airfield_template(game_logic: &mut GameLogic) {
        if game_logic.templates.contains_key("TestAirfield") {
            return;
        }

        let mut airfield = ThingTemplate::new("TestAirfield");
        airfield
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1200.0)
            .set_cost(1000, -2);
        game_logic
            .templates
            .insert("TestAirfield".to_string(), airfield);
    }

    fn ensure_test_player_for_team(game_logic: &mut GameLogic, team: Team) {
        let player_id = match team {
            Team::USA => 0,
            Team::China => 1,
            Team::GLA => 2,
            Team::Neutral => 3,
        };

        if game_logic.get_player(player_id).is_none() {
            let mut player = Player::new(player_id, team, "TestPlayer", true);
            player.resources.supplies = 100_000;
            player.power_available = 100;
            player.resources.power = 100;
            game_logic.add_player(player);
        }
    }

    #[test]
    fn shell_game_state_tracks_in_game_status() {
        let mut game_logic = GameLogic::new();
        assert!(!game_logic.isInGame());
        assert!(!game_logic.isInShellGame());

        game_logic.start_new_game(GameMode::Shell);
        assert!(
            game_logic.isInShellGame(),
            "GAME_SHELL should report shell state before the map is marked loaded"
        );

        game_logic.map_loaded = true;
        assert!(game_logic.isInShellGame());

        game_logic.start_new_game(GameMode::Skirmish);
        assert!(!game_logic.isInShellGame());
    }

    #[test]
    fn remap_known_model_alias_covers_shell_map_missing_models() {
        assert_eq!(
            GameLogic::remap_known_model_alias("PMRocks01b"),
            "PMBoulders_D"
        );
        assert_eq!(
            GameLogic::remap_known_model_alias("PMRocks02b"),
            "PMBoulders_D"
        );
        assert_eq!(
            GameLogic::remap_known_model_alias("PTCypress01"),
            "PTXARBVT01"
        );
        assert_eq!(GameLogic::remap_known_model_alias("PTXPine03"), "PTXFIR07");
        assert_eq!(GameLogic::remap_known_model_alias("PMSwing"), "PMBikeRack");
        assert_eq!(
            GameLogic::remap_known_model_alias("PMPlygdSt"),
            "PMPavilion"
        );
        assert_eq!(
            GameLogic::remap_known_model_alias("AVAMPHIB"),
            "AVChinook"
        );
        assert_eq!(
            GameLogic::remap_known_model_alias("AVChinook_A2"),
            "AVChinook_A2MSH"
        );
        assert_eq!(
            GameLogic::remap_known_model_alias("ABSupplyCT_A2"),
            "ABSupplyCT_A2U"
        );
        assert_eq!(
            GameLogic::remap_known_model_alias("AVPaladin"),
            "AVCrusader_A"
        );
    }

    #[test]
    fn get_available_templates_filters_faction_prefixed_templates() {
        let mut game_logic = GameLogic::new();
        game_logic.templates.clear();

        let mut usa = ThingTemplate::new("USA_Tank");
        usa.add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Vehicle);
        game_logic.templates.insert(usa.name.clone(), usa);

        let mut china = ThingTemplate::new("China_Tank");
        china
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Vehicle);
        game_logic.templates.insert(china.name.clone(), china);

        let mut gla = ThingTemplate::new("GLA_Tank");
        gla.add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Vehicle);
        game_logic.templates.insert(gla.name.clone(), gla);

        let mut shared = ThingTemplate::new("TestScout");
        shared
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Infantry);
        game_logic.templates.insert(shared.name.clone(), shared);

        let available = game_logic.get_available_templates(Team::USA);
        assert!(available.contains(&"USA_Tank".to_string()));
        assert!(available.contains(&"TestScout".to_string()));
        assert!(!available.contains(&"China_Tank".to_string()));
        assert!(!available.contains(&"GLA_Tank".to_string()));
    }

    #[test]
    fn visibility_filter_allows_object_when_shroud_snapshot_missing() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        let object_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
            .expect("object should be created");
        let object = game_logic
            .find_object(object_id)
            .expect("object should exist");

        assert!(GameLogic::is_object_visible_for_team(
            object_id,
            object,
            Team::USA,
            None
        ));
    }

    #[test]
    fn visibility_filter_requires_visible_or_explored_membership_with_shroud_snapshot() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        let object_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
            .expect("object should be created");
        let object = game_logic
            .find_object(object_id)
            .expect("object should exist");

        let mut visible_only = ShroudVisibilitySnapshot {
            visible_objects: HashSet::new(),
            explored_objects: HashSet::new(),
        };
        visible_only.visible_objects.insert(object_id.0);
        assert!(GameLogic::is_object_visible_for_team(
            object_id,
            object,
            Team::USA,
            Some(&visible_only)
        ));

        let mut explored_only = ShroudVisibilitySnapshot {
            visible_objects: HashSet::new(),
            explored_objects: HashSet::new(),
        };
        explored_only.explored_objects.insert(object_id.0);
        assert!(GameLogic::is_object_visible_for_team(
            object_id,
            object,
            Team::USA,
            Some(&explored_only)
        ));

        let hidden = ShroudVisibilitySnapshot {
            visible_objects: HashSet::new(),
            explored_objects: HashSet::new(),
        };
        assert!(!GameLogic::is_object_visible_for_team(
            object_id,
            object,
            Team::USA,
            Some(&hidden)
        ));
    }

    #[test]
    fn minimap_visibility_filter_requires_live_visibility_for_non_structures() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        let object_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
            .expect("object should be created");
        let object = game_logic
            .find_object(object_id)
            .expect("object should exist");

        let mut explored_only = ShroudVisibilitySnapshot {
            visible_objects: HashSet::new(),
            explored_objects: HashSet::new(),
        };
        explored_only.explored_objects.insert(object_id.0);

        assert!(!GameLogic::is_object_visible_on_minimap_for_team(
            object_id,
            object,
            Team::USA,
            Some(&explored_only),
        ));
    }

    #[test]
    fn minimap_visibility_filter_keeps_explored_structures() {
        let mut game_logic = GameLogic::new();
        let mut structure_template = ThingTemplate::new("TestStructure");
        structure_template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable);
        game_logic
            .templates
            .insert("TestStructure".to_string(), structure_template);
        let object_id = game_logic
            .create_object("TestStructure", Team::GLA, Vec3::new(20.0, 0.0, 20.0))
            .expect("structure should be created");
        let object = game_logic
            .find_object(object_id)
            .expect("structure should exist");

        let mut explored_only = ShroudVisibilitySnapshot {
            visible_objects: HashSet::new(),
            explored_objects: HashSet::new(),
        };
        explored_only.explored_objects.insert(object_id.0);

        assert!(GameLogic::is_object_visible_on_minimap_for_team(
            object_id,
            object,
            Team::USA,
            Some(&explored_only),
        ));
    }

    fn setup_ground_attacker(
        game_logic: &mut GameLogic,
        position: Vec3,
        target_location: Vec3,
    ) -> ObjectId {
        ensure_test_tank_template(game_logic);
        let attacker_id = game_logic
            .create_object("TestTank", Team::USA, position)
            .expect("attacker should be created from template");

        let attacker = game_logic
            .find_object_mut(attacker_id)
            .expect("attacker should exist");
        attacker.set_force_attack(true);
        attacker.set_target_location(Some(target_location));
        attacker.ai_state = AIState::AttackingGround;
        attacker.status.attacking = true;
        if let Some(weapon) = attacker.weapon.as_mut() {
            weapon.damage = 40.0;
            weapon.range = 150.0;
            weapon.reload_time = 0.25;
            weapon.last_fire_time = 0.0;
        }

        attacker_id
    }

    #[test]
    fn entering_state_docks_unit_into_transport_when_close() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let transport_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("transport should be created");
        let unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .expect("unit should be created");

        {
            let unit = game_logic
                .find_object_mut(unit_id)
                .expect("unit should exist");
            unit.target = Some(transport_id);
            unit.ai_state = AIState::Entering;
            unit.status.moving = true;
        }

        game_logic.update_ai(&[transport_id, unit_id], 1.0 / 60.0);

        let transport = game_logic
            .find_object(transport_id)
            .expect("transport should exist");
        assert!(
            transport.contained_units().contains(&unit_id),
            "entering unit should be registered as transport occupant"
        );

        let unit = game_logic.find_object(unit_id).expect("unit should exist");
        assert_eq!(unit.ai_state, AIState::Docked);
        assert_eq!(unit.target, Some(transport_id));
        assert!(!unit.can_move(), "docked units should not be movable");
        assert!(
            !unit.can_attack(),
            "docked units should not be independently attackable"
        );
    }

    #[test]
    fn docking_state_moves_unit_toward_transport_when_far() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let transport_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("transport should be created");
        let unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(120.0, 0.0, 0.0))
            .expect("unit should be created");

        {
            let unit = game_logic
                .find_object_mut(unit_id)
                .expect("unit should exist");
            unit.target = Some(transport_id);
            unit.ai_state = AIState::Docking;
        }

        game_logic.update_ai(&[transport_id, unit_id], 1.0 / 60.0);

        let unit = game_logic.find_object(unit_id).expect("unit should exist");
        let destination = unit
            .movement
            .target_position
            .expect("docking unit should move toward transport");
        assert!(destination.distance(Vec3::new(0.0, 0.0, 0.0)) < 0.01);
        assert_eq!(unit.ai_state, AIState::Docking);
    }

    #[test]
    fn enter_command_rejects_enemy_occupied_transport() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let friendly_unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(-10.0, 0.0, 0.0))
            .expect("friendly unit should be created");
        let enemy_transport_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("enemy transport should be created");
        let enemy_occupant_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("enemy occupant should be created");

        {
            let enemy_transport = game_logic
                .find_object_mut(enemy_transport_id)
                .expect("enemy transport should exist");
            assert!(
                enemy_transport.add_occupant(enemy_occupant_id),
                "enemy transport should hold an occupant for legality test"
            );
        }
        {
            let enemy_occupant = game_logic
                .find_object_mut(enemy_occupant_id)
                .expect("enemy occupant should exist");
            enemy_occupant.target = Some(enemy_transport_id);
            enemy_occupant.ai_state = AIState::Docked;
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Enter {
                target_id: enemy_transport_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![friendly_unit_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let friendly = game_logic
            .find_object(friendly_unit_id)
            .expect("friendly unit should exist");
        assert_ne!(
            friendly.target,
            Some(enemy_transport_id),
            "enter command should not target occupied enemy transport"
        );
        assert_ne!(
            friendly.ai_state,
            AIState::Entering,
            "unit should not start entering an occupied enemy transport"
        );
    }

    #[test]
    fn entering_state_clears_enemy_structure_target() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_barracks_template(&mut game_logic);

        let unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .expect("unit should be created");
        let enemy_barracks_id = game_logic
            .create_object("TestBarracks", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("enemy barracks should be created");

        {
            let unit = game_logic
                .find_object_mut(unit_id)
                .expect("unit should exist");
            unit.target = Some(enemy_barracks_id);
            unit.ai_state = AIState::Entering;
            unit.status.moving = true;
        }

        game_logic.update_ai(&[unit_id, enemy_barracks_id], 1.0 / 60.0);

        let unit = game_logic.find_object(unit_id).expect("unit should exist");
        assert!(
            unit.target.is_none(),
            "entering should clear enemy structure targets"
        );
        assert_eq!(
            unit.ai_state,
            AIState::Idle,
            "unit should return to idle when enter legality fails"
        );
    }

    #[test]
    fn guard_state_engages_nearby_enemy() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let guard_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("guard should be created");
        let enemy_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(25.0, 0.0, 0.0))
            .expect("enemy should be created");

        {
            let guard = game_logic
                .find_object_mut(guard_id)
                .expect("guard should exist");
            guard.ai_state = AIState::GuardingArea;
            guard.guard_position = Some(Vec3::new(0.0, 0.0, 0.0));
            guard.guard_radius = 100.0;
        }

        game_logic.update_ai(&[guard_id, enemy_id], 1.0 / 60.0);

        let guard = game_logic
            .find_object(guard_id)
            .expect("guard should exist");
        assert_eq!(guard.ai_state, AIState::Attacking);
        assert_eq!(guard.target, Some(enemy_id));
    }

    #[test]
    fn process_ai_behavior_idle_fallback_engages_nearby_enemy() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let attacker_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("attacker should be created");
        let enemy_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
            .expect("enemy should be created");
        {
            let attacker = game_logic
                .find_object_mut(attacker_id)
                .expect("attacker should exist");
            attacker.weapon = Some(Weapon {
                range: 150.0,
                ..Weapon::default()
            });
        }

        let attacker = game_logic
            .find_object(attacker_id)
            .expect("attacker should exist");
        let command = game_logic.process_ai_behavior(
            attacker_id,
            AIState::Idle,
            None,
            attacker.get_position(),
            attacker.team,
            attacker.can_attack(),
            30,
            1.0 / 60.0,
        );

        match command {
            Some(AICommand::AttackTarget {
                object_id,
                target_id,
            }) => {
                assert_eq!(object_id, attacker_id);
                assert_eq!(target_id, enemy_id);
            }
            other => panic!("expected idle fallback to attack enemy, got {other:?}"),
        }
    }

    #[test]
    fn process_ai_behavior_attacking_fallback_stops_without_target() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let attacker_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("attacker should be created");
        let attacker = game_logic
            .find_object(attacker_id)
            .expect("attacker should exist");

        let command = game_logic.process_ai_behavior(
            attacker_id,
            AIState::Attacking,
            None,
            attacker.get_position(),
            attacker.team,
            attacker.can_attack(),
            0,
            1.0 / 60.0,
        );

        match command {
            Some(AICommand::StopAttack { object_id }) => assert_eq!(object_id, attacker_id),
            other => panic!("expected attacking fallback to stop attack, got {other:?}"),
        }
    }

    #[test]
    fn process_ai_behavior_patrolling_fallback_moves_deterministically() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, -20.0))
            .expect("unit should be created");
        let unit = game_logic.find_object(unit_id).expect("unit should exist");
        let start = unit.get_position();
        let frame = unit_id.0;

        let command = game_logic.process_ai_behavior(
            unit_id,
            AIState::Patrolling,
            None,
            start,
            unit.team,
            unit.can_attack(),
            frame,
            1.0 / 60.0,
        );

        match command {
            Some(AICommand::MoveTo {
                object_id,
                position,
            }) => {
                assert_eq!(object_id, unit_id);
                let distance = start.distance(position);
                assert!(
                    (distance - 100.0).abs() < 0.001,
                    "patrol destination should keep 100 world-units radius"
                );
            }
            other => panic!("expected patrol fallback to emit movement, got {other:?}"),
        }
    }

    #[test]
    fn repairing_state_heals_target_in_range() {
        let mut game_logic = GameLogic::new();
        ensure_test_dozer_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let repairer_id = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repairer should be created");
        let damaged_id = game_logic
            .create_object("TestBuilding", Team::USA, Vec3::new(5.0, 0.0, 0.0))
            .expect("target should be created");

        {
            let damaged = game_logic
                .find_object_mut(damaged_id)
                .expect("damaged unit should exist");
            let _ = damaged.take_damage(80.0);
        }
        {
            let repairer = game_logic
                .find_object_mut(repairer_id)
                .expect("repairer should exist");
            repairer.target = Some(damaged_id);
            repairer.ai_state = AIState::Repairing;
        }
        let before = game_logic
            .find_object(damaged_id)
            .expect("damaged unit should exist")
            .health
            .current;

        game_logic.update_ai(&[repairer_id, damaged_id], 1.0);

        let after = game_logic
            .find_object(damaged_id)
            .expect("damaged unit should exist")
            .health
            .current;
        assert!(
            after > before,
            "repairing state should restore target health"
        );
    }

    #[test]
    fn seeking_repair_state_heals_self_in_range() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_repair_pad_template(&mut game_logic);

        let repair_bay_id = game_logic
            .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repair source should be created");
        let unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
            .expect("unit should be created");

        {
            let unit = game_logic
                .find_object_mut(unit_id)
                .expect("unit should exist");
            let _ = unit.take_damage(90.0);
            unit.target = Some(repair_bay_id);
            unit.ai_state = AIState::SeekingRepair;
        }
        let before = game_logic
            .find_object(unit_id)
            .expect("unit should exist")
            .health
            .current;

        game_logic.update_ai(&[repair_bay_id, unit_id], 1.0);

        let after = game_logic
            .find_object(unit_id)
            .expect("unit should exist")
            .health
            .current;
        assert!(
            after > before,
            "seeking repair should heal the damaged unit"
        );
    }

    #[test]
    fn seeking_repair_state_clears_under_construction_destination() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_repair_pad_template(&mut game_logic);

        let repair_bay_id = game_logic
            .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repair source should be created");
        let unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
            .expect("unit should be created");

        {
            let repair_bay = game_logic
                .find_object_mut(repair_bay_id)
                .expect("repair source should exist");
            repair_bay.status.under_construction = true;
        }
        {
            let unit = game_logic
                .find_object_mut(unit_id)
                .expect("unit should exist");
            let _ = unit.take_damage(90.0);
            unit.target = Some(repair_bay_id);
            unit.ai_state = AIState::SeekingRepair;
            unit.status.moving = true;
        }

        game_logic.update_ai(&[repair_bay_id, unit_id], 1.0 / 60.0);

        let unit = game_logic.find_object(unit_id).expect("unit should exist");
        assert!(
            unit.target.is_none(),
            "seeking repair should clear under-construction destinations"
        );
        assert_eq!(unit.ai_state, AIState::Idle);
    }

    #[test]
    fn evacuate_command_unloads_selected_transport_occupants() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let transport_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("transport should be created");
        let unit_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .expect("unit should be created");

        {
            let transport = game_logic
                .find_object_mut(transport_id)
                .expect("transport should exist");
            assert!(transport.add_occupant(unit_id));
        }
        {
            let unit = game_logic
                .find_object_mut(unit_id)
                .expect("unit should exist");
            unit.target = Some(transport_id);
            unit.ai_state = AIState::Docked;
            unit.set_position(Vec3::new(0.0, 0.0, 0.0));
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Evacuate,
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![transport_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let transport = game_logic
            .find_object(transport_id)
            .expect("transport should exist");
        assert!(
            !transport.contained_units().contains(&unit_id),
            "evacuate should remove occupants from selected transport"
        );
        let unit = game_logic.find_object(unit_id).expect("unit should exist");
        assert_eq!(unit.ai_state, AIState::Idle);
        assert!(unit.target.is_none());
        assert!(unit.can_move());
    }

    #[test]
    fn capture_command_does_not_instantly_flip_building_owner() {
        let mut game_logic = GameLogic::new();
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let captor_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(120.0, 0.0, 0.0))
            .expect("captor should be created");
        let building_id = game_logic
            .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("building should be created");

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::CaptureBuilding {
                target_id: building_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![captor_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let building = game_logic
            .find_object(building_id)
            .expect("building should exist");
        assert_eq!(
            building.team,
            Team::GLA,
            "capture command should not instantly transfer ownership"
        );

        let captor = game_logic
            .find_object(captor_id)
            .expect("captor should exist");
        assert_eq!(captor.ai_state, AIState::Capturing);
        assert_eq!(captor.target, Some(building_id));
    }

    #[test]
    fn capturing_state_transfers_building_when_in_range() {
        let mut game_logic = GameLogic::new();
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let captor_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
            .expect("captor should be created");
        let building_id = game_logic
            .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("building should be created");

        {
            let captor = game_logic
                .find_object_mut(captor_id)
                .expect("captor should exist");
            captor.target = Some(building_id);
            captor.ai_state = AIState::Capturing;
        }

        game_logic.update_ai(&[captor_id, building_id], 1.0 / 60.0);

        let building = game_logic
            .find_object(building_id)
            .expect("building should exist");
        assert_eq!(
            building.team,
            Team::USA,
            "capturing state should transfer structure to captor team once in range"
        );

        let captor = game_logic
            .find_object(captor_id)
            .expect("captor should exist");
        assert_eq!(captor.ai_state, AIState::Idle);
        assert!(captor.target.is_none());
    }

    #[test]
    fn capture_command_rejects_under_construction_building() {
        let mut game_logic = GameLogic::new();
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let captor_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(6.0, 0.0, 0.0))
            .expect("captor should be created");
        let building_id = game_logic
            .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("building should be created");
        {
            let building = game_logic
                .find_object_mut(building_id)
                .expect("building should exist");
            building.status.under_construction = true;
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::CaptureBuilding {
                target_id: building_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![captor_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let captor = game_logic
            .find_object(captor_id)
            .expect("captor should exist");
        assert_ne!(captor.ai_state, AIState::Capturing);
        assert_ne!(captor.target, Some(building_id));

        let building = game_logic
            .find_object(building_id)
            .expect("building should exist");
        assert_eq!(building.team, Team::GLA);
    }

    #[test]
    fn capturing_state_does_not_transfer_under_construction_building() {
        let mut game_logic = GameLogic::new();
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let captor_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
            .expect("captor should be created");
        let building_id = game_logic
            .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("building should be created");
        {
            let building = game_logic
                .find_object_mut(building_id)
                .expect("building should exist");
            building.status.under_construction = true;
        }
        {
            let captor = game_logic
                .find_object_mut(captor_id)
                .expect("captor should exist");
            captor.target = Some(building_id);
            captor.ai_state = AIState::Capturing;
        }

        game_logic.update_ai(&[captor_id, building_id], 1.0 / 60.0);

        let building = game_logic
            .find_object(building_id)
            .expect("building should exist");
        assert_eq!(building.team, Team::GLA);

        let captor = game_logic
            .find_object(captor_id)
            .expect("captor should exist");
        assert_eq!(captor.ai_state, AIState::Idle);
        assert!(captor.target.is_none());
    }

    #[test]
    fn capture_command_rejects_non_infantry_units() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let tank_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
            .expect("tank should be created");
        let building_id = game_logic
            .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("building should be created");

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::CaptureBuilding {
                target_id: building_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![tank_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let tank = game_logic.find_object(tank_id).expect("tank should exist");
        assert_ne!(tank.ai_state, AIState::Capturing);
        assert_ne!(tank.target, Some(building_id));

        let building = game_logic
            .find_object(building_id)
            .expect("building should exist");
        assert_eq!(building.team, Team::GLA);
    }

    #[test]
    fn repair_command_sets_all_selected_repairers_to_repairing() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_dozer_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let repairer_a = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repairer A should be created");
        let repairer_b = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(4.0, 0.0, 0.0))
            .expect("repairer B should be created");
        let target_id = game_logic
            .create_object("TestBuilding", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("repair target should be created");

        {
            let target = game_logic
                .find_object_mut(target_id)
                .expect("target should exist");
            let _ = target.take_damage(50.0);
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Repair { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![repairer_a, repairer_b],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let a = game_logic
            .find_object(repairer_a)
            .expect("repairer A should exist");
        let b = game_logic
            .find_object(repairer_b)
            .expect("repairer B should exist");

        assert_eq!(a.ai_state, AIState::Repairing);
        assert_eq!(b.ai_state, AIState::Repairing);
        assert_eq!(a.target, Some(target_id));
        assert_eq!(b.target, Some(target_id));
    }

    #[test]
    fn repair_command_ignores_non_worker_units() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let tank_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("tank should be created");
        let target_id = game_logic
            .create_object("TestBuilding", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("repair target should be created");

        {
            let target = game_logic
                .find_object_mut(target_id)
                .expect("target should exist");
            let _ = target.take_damage(75.0);
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Repair { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![tank_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let tank = game_logic.find_object(tank_id).expect("tank should exist");
        assert_ne!(
            tank.ai_state,
            AIState::Repairing,
            "non-worker units should not enter repairing state from repair commands"
        );
    }

    #[test]
    fn repair_command_allows_repairing_neutral_structures() {
        let mut game_logic = GameLogic::new();
        ensure_test_dozer_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let repairer_id = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repairer should be created");
        let target_id = game_logic
            .create_object("TestBuilding", Team::Neutral, Vec3::new(6.0, 0.0, 0.0))
            .expect("neutral target should be created");

        {
            let target = game_logic
                .find_object_mut(target_id)
                .expect("target should exist");
            let _ = target.take_damage(60.0);
        }

        let before = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Repair { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![repairer_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();
        game_logic.update_ai(&[repairer_id, target_id], 1.0 / 60.0);

        let repairer = game_logic
            .find_object(repairer_id)
            .expect("repairer should exist");
        assert_eq!(repairer.ai_state, AIState::Repairing);
        assert_eq!(repairer.target, Some(target_id));

        let after = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert!(after > before);
    }

    #[test]
    fn get_repaired_command_targets_only_damaged_vehicles() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_repair_pad_template(&mut game_logic);

        let repair_bay_id = game_logic
            .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repair bay should be created");
        let vehicle_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("vehicle should be created");
        let infantry_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(9.0, 0.0, 0.0))
            .expect("infantry should be created");

        {
            let vehicle = game_logic
                .find_object_mut(vehicle_id)
                .expect("vehicle should exist");
            let _ = vehicle.take_damage(80.0);
        }
        {
            let infantry = game_logic
                .find_object_mut(infantry_id)
                .expect("infantry should exist");
            let _ = infantry.take_damage(20.0);
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetRepaired {
                target_id: repair_bay_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![vehicle_id, infantry_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let vehicle = game_logic
            .find_object(vehicle_id)
            .expect("vehicle should exist");
        let infantry = game_logic
            .find_object(infantry_id)
            .expect("infantry should exist");
        assert_eq!(vehicle.ai_state, AIState::SeekingRepair);
        assert_ne!(infantry.ai_state, AIState::SeekingRepair);
    }

    #[test]
    fn get_repaired_command_requires_repair_destination_type() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let non_repair_structure = game_logic
            .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("support structure should be created");
        let vehicle_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("vehicle should be created");
        {
            let vehicle = game_logic
                .find_object_mut(vehicle_id)
                .expect("vehicle should exist");
            let _ = vehicle.take_damage(80.0);
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetRepaired {
                target_id: non_repair_structure,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![vehicle_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let vehicle = game_logic
            .find_object(vehicle_id)
            .expect("vehicle should exist");
        assert_ne!(vehicle.ai_state, AIState::SeekingRepair);
        assert_ne!(vehicle.target, Some(non_repair_structure));
    }

    #[test]
    fn get_repaired_command_rejects_under_construction_destination() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_repair_pad_template(&mut game_logic);

        let repair_pad_id = game_logic
            .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repair pad should be created");
        let vehicle_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("vehicle should be created");
        {
            let vehicle = game_logic
                .find_object_mut(vehicle_id)
                .expect("vehicle should exist");
            let _ = vehicle.take_damage(80.0);
        }
        {
            let repair_pad = game_logic
                .find_object_mut(repair_pad_id)
                .expect("repair pad should exist");
            repair_pad.status.under_construction = true;
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetRepaired {
                target_id: repair_pad_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![vehicle_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let vehicle = game_logic
            .find_object(vehicle_id)
            .expect("vehicle should exist");
        assert_ne!(vehicle.ai_state, AIState::SeekingRepair);
        assert_ne!(vehicle.target, Some(repair_pad_id));
    }

    #[test]
    fn get_repaired_command_aircraft_requires_airfield() {
        let mut game_logic = GameLogic::new();
        ensure_test_aircraft_template(&mut game_logic);
        ensure_test_repair_pad_template(&mut game_logic);
        ensure_test_airfield_template(&mut game_logic);

        let repair_pad_id = game_logic
            .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repair pad should be created");
        let airfield_id = game_logic
            .create_object("TestAirfield", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("airfield should be created");
        let aircraft_id = game_logic
            .create_object("TestAircraft", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("aircraft should be created");
        {
            let aircraft = game_logic
                .find_object_mut(aircraft_id)
                .expect("aircraft should exist");
            let _ = aircraft.take_damage(100.0);
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetRepaired {
                target_id: repair_pad_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![aircraft_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();
        let aircraft = game_logic
            .find_object(aircraft_id)
            .expect("aircraft should exist");
        assert_ne!(aircraft.ai_state, AIState::SeekingRepair);

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetRepaired {
                target_id: airfield_id,
            },
            player_id: 0,
            command_id: 2,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![aircraft_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();
        let aircraft = game_logic
            .find_object(aircraft_id)
            .expect("aircraft should exist");
        assert_eq!(aircraft.ai_state, AIState::SeekingRepair);
        assert_eq!(aircraft.target, Some(airfield_id));
    }

    #[test]
    fn get_healed_command_targets_only_injured_infantry() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_heal_pad_template(&mut game_logic);

        let heal_pad_id = game_logic
            .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("heal pad should be created");
        let infantry_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("infantry should be created");
        let vehicle_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(9.0, 0.0, 0.0))
            .expect("vehicle should be created");

        {
            let infantry = game_logic
                .find_object_mut(infantry_id)
                .expect("infantry should exist");
            let _ = infantry.take_damage(20.0);
        }
        {
            let vehicle = game_logic
                .find_object_mut(vehicle_id)
                .expect("vehicle should exist");
            let _ = vehicle.take_damage(80.0);
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetHealed {
                target_id: heal_pad_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![infantry_id, vehicle_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let infantry = game_logic
            .find_object(infantry_id)
            .expect("infantry should exist");
        let vehicle = game_logic
            .find_object(vehicle_id)
            .expect("vehicle should exist");
        assert_eq!(infantry.ai_state, AIState::SeekingHealing);
        assert_ne!(vehicle.ai_state, AIState::SeekingHealing);
    }

    #[test]
    fn get_healed_command_requires_heal_destination_type() {
        let mut game_logic = GameLogic::new();
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let non_heal_structure = game_logic
            .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("non-heal destination should be created");
        let infantry_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("infantry should be created");
        {
            let infantry = game_logic
                .find_object_mut(infantry_id)
                .expect("infantry should exist");
            let _ = infantry.take_damage(20.0);
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetHealed {
                target_id: non_heal_structure,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![infantry_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let infantry = game_logic
            .find_object(infantry_id)
            .expect("infantry should exist");
        assert_ne!(infantry.ai_state, AIState::SeekingHealing);
        assert_ne!(infantry.target, Some(non_heal_structure));
    }

    #[test]
    fn get_healed_command_rejects_under_construction_destination() {
        let mut game_logic = GameLogic::new();
        ensure_test_infantry_template(&mut game_logic);
        ensure_test_heal_pad_template(&mut game_logic);

        let heal_pad_id = game_logic
            .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("heal pad should be created");
        let infantry_id = game_logic
            .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
            .expect("infantry should be created");
        {
            let infantry = game_logic
                .find_object_mut(infantry_id)
                .expect("infantry should exist");
            let _ = infantry.take_damage(20.0);
        }
        {
            let heal_pad = game_logic
                .find_object_mut(heal_pad_id)
                .expect("heal pad should exist");
            heal_pad.status.under_construction = true;
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::GetHealed {
                target_id: heal_pad_id,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![infantry_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let infantry = game_logic
            .find_object(infantry_id)
            .expect("infantry should exist");
        assert_ne!(infantry.ai_state, AIState::SeekingHealing);
        assert_ne!(infantry.target, Some(heal_pad_id));
    }

    #[test]
    fn special_ability_state_without_pending_order_resets_to_idle() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let actor_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("actor should be created");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(3.0, 0.0, 0.0))
            .expect("target should be created");

        {
            let actor = game_logic
                .find_object_mut(actor_id)
                .expect("actor should exist");
            actor.target = Some(target_id);
            actor.ai_state = AIState::SpecialAbility;
        }

        game_logic.update_ai(&[actor_id, target_id], 1.0 / 60.0);

        let actor = game_logic
            .find_object(actor_id)
            .expect("actor should exist");
        assert_eq!(actor.ai_state, AIState::Idle);
        assert!(actor.target.is_none());
    }

    #[test]
    fn build_command_rejects_non_worker_constructor() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);
        ensure_test_player_for_team(&mut game_logic, Team::USA);

        let tank_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("tank should be created");

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DozerConstruct {
                template_name: "TestBuilding".to_string(),
                location: Vec3::new(20.0, 0.0, 20.0),
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![tank_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let created_structures = game_logic
            .get_objects()
            .values()
            .filter(|o| o.template_name == "TestBuilding")
            .count();
        assert_eq!(created_structures, 0);

        let tank = game_logic.find_object(tank_id).expect("tank should exist");
        assert_ne!(tank.ai_state, AIState::Constructing);
    }

    #[test]
    fn dozer_line_assigns_each_worker_a_segment() {
        let mut game_logic = GameLogic::new();
        ensure_test_dozer_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);
        ensure_test_player_for_team(&mut game_logic, Team::USA);

        let dozer_a = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("dozer A should be created");
        let dozer_b = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .expect("dozer B should be created");

        let start = Vec3::new(10.0, 0.0, 10.0);
        let end = Vec3::new(30.0, 0.0, 10.0);
        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DozerConstructLine {
                template_name: "TestBuilding".to_string(),
                start,
                end,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![dozer_a, dozer_b],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let dozer_a_state = game_logic
            .find_object(dozer_a)
            .expect("dozer A should exist");
        let dozer_b_state = game_logic
            .find_object(dozer_b)
            .expect("dozer B should exist");
        assert_eq!(dozer_a_state.ai_state, AIState::Constructing);
        assert_eq!(dozer_b_state.ai_state, AIState::Constructing);

        let a_dest = dozer_a_state
            .movement
            .target_position
            .expect("dozer A should receive a line segment destination");
        let b_dest = dozer_b_state
            .movement
            .target_position
            .expect("dozer B should receive a line segment destination");
        assert!(a_dest.distance(start) < 0.01);
        assert!(b_dest.distance(end) < 0.01);

        let created_structures = game_logic
            .get_objects()
            .values()
            .filter(|o| o.template_name == "TestBuilding")
            .count();
        assert_eq!(created_structures, 2);
    }

    #[test]
    fn hijack_transfers_vehicle_and_updates_team_color() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let hijacker_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("hijacker should be created");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
            .expect("target should be created");

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Hijack { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![hijacker_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();
        game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

        let target = game_logic
            .find_object(target_id)
            .expect("target should exist");
        assert_eq!(target.team, Team::USA);
        assert_eq!(target.team_color, Team::USA.get_color());

        let hijacker = game_logic
            .find_object(hijacker_id)
            .expect("hijacker should exist");
        assert!(hijacker.status.destroyed);
    }

    #[test]
    fn hijack_command_applies_only_after_unit_reaches_target() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let hijacker_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
            .expect("hijacker should be created");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("target should be created");

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Hijack { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![hijacker_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let target_after_command = game_logic
            .find_object(target_id)
            .expect("target should exist");
        assert_eq!(
            target_after_command.team,
            Team::GLA,
            "hijack should not transfer target immediately on command issue"
        );

        game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);
        let target_after_far_update = game_logic
            .find_object(target_id)
            .expect("target should exist");
        assert_eq!(
            target_after_far_update.team,
            Team::GLA,
            "hijack should stay pending while hijacker is out of range"
        );

        {
            let hijacker = game_logic
                .find_object_mut(hijacker_id)
                .expect("hijacker should exist");
            hijacker.set_position(Vec3::new(2.0, 0.0, 0.0));
            hijacker.ai_state = AIState::SpecialAbility;
            hijacker.target = Some(target_id);
        }
        game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

        let target_after_contact = game_logic
            .find_object(target_id)
            .expect("target should exist");
        assert_eq!(target_after_contact.team, Team::USA);

        let hijacker = game_logic
            .find_object(hijacker_id)
            .expect("hijacker should exist");
        assert!(hijacker.status.destroyed);
    }

    #[test]
    fn sabotage_command_applies_only_after_unit_reaches_target() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        ensure_test_structure_template(&mut game_logic);

        let saboteur_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
            .expect("saboteur should be created");
        let target_id = game_logic
            .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("target should be created");

        let initial_health = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Sabotage { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![saboteur_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let target_after_command = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert_eq!(
            target_after_command, initial_health,
            "sabotage should not damage immediately on command issue"
        );

        game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);
        let target_after_far_update = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert_eq!(
            target_after_far_update, initial_health,
            "sabotage should still be pending while saboteur is out of range"
        );

        {
            let saboteur = game_logic
                .find_object_mut(saboteur_id)
                .expect("saboteur should exist");
            saboteur.set_position(Vec3::new(2.0, 0.0, 0.0));
            saboteur.ai_state = AIState::SpecialAbility;
            saboteur.target = Some(target_id);
        }
        game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);

        let target_after_contact = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert!(
            target_after_contact < initial_health,
            "sabotage should apply once saboteur reaches target"
        );
    }

    #[test]
    fn sabotage_command_rejects_non_structure_targets() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let saboteur_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("saboteur should be created");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("target should be created");

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Sabotage { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![saboteur_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let saboteur = game_logic
            .find_object(saboteur_id)
            .expect("saboteur should exist");
        assert_ne!(saboteur.ai_state, AIState::SpecialAbility);
        assert_ne!(saboteur.target, Some(target_id));
    }

    #[test]
    fn snipe_vehicle_command_applies_only_after_unit_reaches_target() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let sniper_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(160.0, 0.0, 0.0))
            .expect("sniper should be created");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("target should be created");

        let initial_health = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::SnipeVehicle { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![sniper_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let target_after_command = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert_eq!(
            target_after_command, initial_health,
            "snipe should not apply immediately on command issue"
        );

        game_logic.update_ai(&[sniper_id, target_id], 1.0 / 60.0);
        let target_after_far_update = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert_eq!(
            target_after_far_update, initial_health,
            "snipe should be pending while sniper is out of range"
        );

        {
            let sniper = game_logic
                .find_object_mut(sniper_id)
                .expect("sniper should exist");
            sniper.set_position(Vec3::new(2.0, 0.0, 0.0));
            sniper.ai_state = AIState::SpecialAbility;
            sniper.target = Some(target_id);
        }
        game_logic.update_ai(&[sniper_id, target_id], 1.0 / 60.0);

        let target_after_contact = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert!(
            target_after_contact < initial_health,
            "snipe should apply once sniper reaches target"
        );
    }

    #[test]
    fn carbomb_command_applies_only_after_unit_reaches_target() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let bomber_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(170.0, 0.0, 0.0))
            .expect("bomber should be created");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("target should be created");

        let initial_health = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::ConvertToCarbomb { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![bomber_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        let target_after_command = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert_eq!(
            target_after_command, initial_health,
            "carbomb should not apply immediately on command issue"
        );

        game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);
        let target_after_far_update = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert_eq!(
            target_after_far_update, initial_health,
            "carbomb should be pending while bomber is out of range"
        );

        {
            let bomber = game_logic
                .find_object_mut(bomber_id)
                .expect("bomber should exist");
            bomber.set_position(Vec3::new(2.0, 0.0, 0.0));
            bomber.ai_state = AIState::SpecialAbility;
            bomber.target = Some(target_id);
        }
        game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);

        let target_after_contact = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert!(
            target_after_contact < initial_health,
            "carbomb should damage target once bomber reaches it"
        );

        let bomber = game_logic
            .find_object(bomber_id)
            .expect("bomber should exist");
        assert!(
            bomber.status.destroyed,
            "carbomb unit should be destroyed when the attack resolves"
        );
    }

    #[test]
    fn carbomb_command_allows_neutral_targets() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let bomber_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
            .expect("bomber should be created");
        let target_id = game_logic
            .create_object("TestTank", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
            .expect("neutral target should be created");

        let initial_health = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::ConvertToCarbomb { target_id },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![bomber_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        game_logic.process_commands();

        {
            let bomber = game_logic
                .find_object(bomber_id)
                .expect("bomber should exist");
            assert_eq!(bomber.ai_state, AIState::SpecialAbility);
            assert_eq!(bomber.target, Some(target_id));
        }

        {
            let bomber = game_logic
                .find_object_mut(bomber_id)
                .expect("bomber should exist");
            bomber.set_position(Vec3::new(2.0, 0.0, 0.0));
            bomber.ai_state = AIState::SpecialAbility;
            bomber.target = Some(target_id);
        }
        game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);

        let target_after_contact = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert!(
            target_after_contact < initial_health,
            "carbomb should resolve against neutral targets"
        );

        let bomber = game_logic
            .find_object(bomber_id)
            .expect("bomber should exist");
        assert!(bomber.status.destroyed);
    }

    #[test]
    fn attack_order_chases_target_when_out_of_range() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let attacker_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("attacker should be created from template");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(300.0, 0.0, 0.0))
            .expect("target should be created from template");

        {
            let attacker = game_logic
                .find_object_mut(attacker_id)
                .expect("attacker should exist");
            attacker.attack_target(target_id);
            if let Some(weapon) = attacker.weapon.as_mut() {
                weapon.range = 50.0;
                weapon.reload_time = 0.0;
                weapon.last_fire_time = 0.0;
            }
        }

        game_logic.frame = 60;
        game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

        let attacker = game_logic
            .find_object(attacker_id)
            .expect("attacker should exist");
        let chase_target = attacker
            .movement
            .target_position
            .expect("attacker should chase out-of-range target");
        assert!(
            chase_target.distance(Vec3::new(300.0, 0.0, 0.0)) < 0.01,
            "attacker should chase the current target position"
        );
        assert_eq!(attacker.ai_state, AIState::Attacking);
        assert!(attacker.status.moving);
    }

    #[test]
    fn attack_order_clears_dead_target() {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);

        let attacker_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("attacker should be created from template");
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
            .expect("target should be created from template");

        {
            let attacker = game_logic
                .find_object_mut(attacker_id)
                .expect("attacker should exist");
            attacker.attack_target(target_id);
        }
        {
            let target = game_logic
                .find_object_mut(target_id)
                .expect("target should exist");
            target.status.destroyed = true;
        }

        game_logic.frame = 60;
        game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

        let attacker = game_logic
            .find_object(attacker_id)
            .expect("attacker should exist");
        assert!(attacker.target.is_none(), "dead targets should be cleared");
        assert_eq!(attacker.ai_state, AIState::Idle);
        assert!(!attacker.status.attacking);
    }

    #[test]
    fn ai_production_does_not_spawn_when_player_cannot_afford_unit() {
        let mut game_logic = GameLogic::new();

        let mut war_factory = ThingTemplate::new("WarFactory");
        war_factory
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1500.0)
            .set_cost(1000, -2);
        game_logic
            .templates
            .insert("WarFactory".to_string(), war_factory);

        let mut humvee = ThingTemplate::new("USA_Humvee");
        humvee
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0)
            .set_cost(500, 0);
        game_logic
            .templates
            .insert("USA_Humvee".to_string(), humvee);

        let mut player = Player::new(0, Team::USA, "AI", false);
        player.resources.supplies = 250;
        game_logic.add_player(player);

        let factory_id = game_logic
            .create_object("WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("war factory should be created");

        game_logic.frame = 600; // AI production pulse
        game_logic.update_ai(&[factory_id], 1.0 / 60.0);

        assert_eq!(
            game_logic.objects.len(),
            1,
            "AI should not spawn units for free when resources are insufficient"
        );
        assert_eq!(
            game_logic
                .get_player(0)
                .expect("player should exist")
                .resources
                .supplies,
            250,
            "resources should remain unchanged when production cannot be afforded"
        );
    }

    #[test]
    fn attack_ground_damages_enemy_near_impact_point() {
        let mut game_logic = GameLogic::new();
        let attacker_id = setup_ground_attacker(
            &mut game_logic,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(15.0, 0.0, 0.0),
        );
        let target_id = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
            .expect("target should be created from template");

        game_logic.frame = 60; // t=1s, enough for first shot with reload_time 0.25
        let health_before = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;

        game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

        let health_after = game_logic
            .find_object(target_id)
            .expect("target should exist")
            .health
            .current;
        assert!(
            health_after < health_before,
            "ground attack should damage units near impact point"
        );
    }

    #[test]
    fn attack_ground_advances_reload_without_victim() {
        let mut game_logic = GameLogic::new();
        let attacker_id = setup_ground_attacker(
            &mut game_logic,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
        );

        game_logic.frame = 60; // t=1s
        let last_fire_before = game_logic
            .find_object(attacker_id)
            .and_then(|obj| obj.weapon.as_ref())
            .map(|weapon| weapon.last_fire_time)
            .unwrap_or_default();

        game_logic.update_combat(&[attacker_id], 1.0 / 60.0);

        let last_fire_after = game_logic
            .find_object(attacker_id)
            .and_then(|obj| obj.weapon.as_ref())
            .map(|weapon| weapon.last_fire_time)
            .unwrap_or_default();
        assert!(
            last_fire_after > last_fire_before,
            "ground attack should consume a shot even when no unit is hit"
        );
    }

    #[test]
    fn force_attack_ground_can_damage_friendlies() {
        let mut game_logic = GameLogic::new();
        let attacker_id = setup_ground_attacker(
            &mut game_logic,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(15.0, 0.0, 0.0),
        );
        let friendly_id = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(15.0, 0.0, 0.0))
            .expect("friendly should be created from template");

        game_logic.frame = 60; // t=1s
        let health_before = game_logic
            .find_object(friendly_id)
            .expect("friendly should exist")
            .health
            .current;

        game_logic.update_combat(&[attacker_id, friendly_id], 1.0 / 60.0);

        let health_after = game_logic
            .find_object(friendly_id)
            .expect("friendly should exist")
            .health
            .current;
        assert!(
            health_after < health_before,
            "forced ground attack should allow friendly fire like classic force-fire behavior"
        );
    }

    #[test]
    fn camera_mod_final_look_toward_uses_remaining_script_camera_time() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic.start_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(200.0, 0.0, 120.0),
            seconds: 4.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
        game_logic
            .mission_scripts
            .push_camera_mod_final_look_toward(CameraModFinalLookTowardRequest {
                position: Vec3::new(300.0, 0.0, 220.0),
            });

        game_logic.evaluate_and_execute_scripts(0.0);

        let look = game_logic
            .take_camera_look_toward_request()
            .expect("mod final look toward should enqueue a look request");
        assert_eq!(look.position, Vec3::new(300.0, 0.0, 220.0));
        assert!(
            (look.duration_seconds - 4.0).abs() < 0.001,
            "mod final look should use remaining camera movement time"
        );
        assert_eq!(look.ease_in_seconds, 0.0);
        assert_eq!(look.ease_out_seconds, 0.0);
    }

    #[test]
    fn camera_mod_look_toward_is_immediate_request() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_mod_look_toward(CameraModLookTowardRequest {
                position: Vec3::new(150.0, 0.0, 50.0),
            });

        game_logic.evaluate_and_execute_scripts(0.0);

        let look = game_logic
            .take_camera_look_toward_request()
            .expect("mod look toward should enqueue look request");
        assert_eq!(look.position, Vec3::new(150.0, 0.0, 50.0));
        assert_eq!(look.duration_seconds, 0.0);
        assert!(!look.reverse_rotation);
    }

    #[test]
    fn camera_mod_freeze_time_applies_to_next_script_camera_move() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic.mission_scripts.push_camera_mod_freeze_time();
        game_logic.evaluate_and_execute_scripts(0.0);
        assert!(
            !game_logic.is_script_camera_time_frozen(),
            "freeze time should arm until a scripted camera move starts"
        );

        game_logic
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(200.0, 0.0, 120.0),
                seconds: 3.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        game_logic.evaluate_and_execute_scripts(0.0);
        assert!(
            game_logic.is_script_camera_time_frozen(),
            "freeze time should be active during scripted camera movement"
        );

        for _ in 0..240 {
            game_logic.update_script_camera(1.0 / 60.0);
        }
        assert!(
            !game_logic.is_script_camera_time_frozen(),
            "freeze time should clear once scripted camera movement ends"
        );
    }

    #[test]
    fn camera_mod_freeze_time_marks_simulation_as_frozen() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        assert!(!game_logic.is_time_frozen_for_simulation());

        game_logic.mission_scripts.push_camera_mod_freeze_time();
        game_logic
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(120.0, 0.0, 60.0),
                seconds: 2.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!(game_logic.is_script_camera_time_frozen());
        assert!(game_logic.is_time_frozen_for_simulation());
    }

    #[test]
    fn camera_mod_freeze_time_blocks_simulation_movement_updates() {
        let mut baseline = GameLogic::new();
        ensure_test_tank_template(&mut baseline);
        let baseline_unit = baseline
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("baseline unit should be created");
        {
            let obj = baseline
                .find_object_mut(baseline_unit)
                .expect("baseline unit should exist");
            obj.move_to(Vec3::new(120.0, 0.0, 0.0));
            obj.movement.max_speed = 60.0;
            obj.movement.acceleration = 3600.0;
        }
        let baseline_before = baseline
            .find_object(baseline_unit)
            .expect("baseline unit should exist")
            .get_position();
        baseline.update_with_dt(1.0 / 30.0);
        let baseline_after = baseline
            .find_object(baseline_unit)
            .expect("baseline unit should exist")
            .get_position();
        assert!(
            baseline_after.distance(baseline_before) > 0.5,
            "baseline simulation should advance movement when not frozen"
        );

        let mut frozen = GameLogic::new();
        frozen.scripts_loaded = true;
        ensure_test_tank_template(&mut frozen);
        let frozen_unit = frozen
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("frozen unit should be created");
        {
            let obj = frozen
                .find_object_mut(frozen_unit)
                .expect("frozen unit should exist");
            obj.move_to(Vec3::new(120.0, 0.0, 0.0));
            obj.movement.max_speed = 60.0;
            obj.movement.acceleration = 3600.0;
        }

        frozen.mission_scripts.push_camera_mod_freeze_time();
        frozen
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(220.0, 0.0, 120.0),
                seconds: 2.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        frozen.evaluate_and_execute_scripts(0.0);
        assert!(frozen.is_time_frozen_for_simulation());

        let frozen_before = frozen
            .find_object(frozen_unit)
            .expect("frozen unit should exist")
            .get_position();
        frozen.update_with_dt(1.0 / 60.0);
        let frozen_after = frozen
            .find_object(frozen_unit)
            .expect("frozen unit should exist")
            .get_position();
        assert!(
            frozen_after.distance(frozen_before) < 0.001,
            "movement should not advance while camera freeze-time is active"
        );
    }

    #[test]
    fn camera_mod_freeze_angle_blocks_look_toward_until_move_finishes() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(180.0, 0.0, 90.0),
                seconds: 2.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        game_logic.mission_scripts.push_camera_mod_freeze_angle();
        game_logic
            .mission_scripts
            .push_camera_mod_look_toward(CameraModLookTowardRequest {
                position: Vec3::new(400.0, 0.0, 300.0),
            });
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!(
            game_logic.take_camera_look_toward_request().is_none(),
            "freeze angle should suppress scripted look-toward while move is active"
        );

        for _ in 0..180 {
            game_logic.update_script_camera(1.0 / 60.0);
        }

        game_logic
            .mission_scripts
            .push_camera_mod_look_toward(CameraModLookTowardRequest {
                position: Vec3::new(410.0, 0.0, 310.0),
            });
        game_logic.evaluate_and_execute_scripts(0.0);
        assert!(
            game_logic.take_camera_look_toward_request().is_some(),
            "look-toward should resume after scripted movement completes"
        );
    }

    #[test]
    fn camera_mod_final_speed_multiplier_applies_to_next_script_camera_move() {
        let mut baseline = GameLogic::new();
        baseline.scripts_loaded = true;
        baseline
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(300.0, 0.0, 200.0),
                seconds: 6.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        baseline.evaluate_and_execute_scripts(0.0);
        for _ in 0..120 {
            baseline.update_script_camera(1.0 / 60.0);
        }
        let baseline_remaining = baseline.script_camera_remaining_seconds();

        let mut modified = GameLogic::new();
        modified.scripts_loaded = true;
        modified
            .mission_scripts
            .push_camera_mod_final_speed_multiplier(CameraModFinalSpeedMultiplierRequest {
                multiplier: 4,
            });
        modified.evaluate_and_execute_scripts(0.0);
        modified
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(300.0, 0.0, 200.0),
                seconds: 6.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        modified.evaluate_and_execute_scripts(0.0);
        for _ in 0..120 {
            modified.update_script_camera(1.0 / 60.0);
        }
        let modified_remaining = modified.script_camera_remaining_seconds();

        assert!(
            modified_remaining + 0.05 < baseline_remaining,
            "final speed multiplier should accelerate scripted camera progression"
        );
    }

    #[test]
    fn camera_mod_rolling_average_arms_for_next_camera_path() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_mod_rolling_average(CameraModRollingAverageRequest { frames: 7 });
        game_logic.evaluate_and_execute_scripts(0.0);

        assert_eq!(
            game_logic.script_camera_pending_rolling_average_frames,
            Some(7)
        );
    }

    #[test]
    fn visual_speed_multiplier_scales_script_camera_update_rate() {
        let mut baseline = GameLogic::new();
        baseline.scripts_loaded = true;
        baseline
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(300.0, 0.0, 200.0),
                seconds: 6.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        baseline.evaluate_and_execute_scripts(0.0);
        baseline.evaluate_and_execute_scripts(1.0 / 60.0);
        let baseline_remaining = baseline.script_camera_remaining_seconds();

        let mut accelerated = GameLogic::new();
        accelerated.scripts_loaded = true;
        accelerated
            .mission_scripts
            .push_visual_speed_multiplier(VisualSpeedMultiplierRequest { multiplier: 3 });
        accelerated.evaluate_and_execute_scripts(0.0);
        accelerated
            .mission_scripts
            .push_camera_move_to(CameraMoveToRequest {
                position: Vec3::new(300.0, 0.0, 200.0),
                seconds: 6.0,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        accelerated.evaluate_and_execute_scripts(0.0);
        accelerated.evaluate_and_execute_scripts(1.0 / 60.0);
        let accelerated_remaining = accelerated.script_camera_remaining_seconds();

        assert_eq!(accelerated.visual_speed_multiplier(), 3.0);
        assert!(
            accelerated_remaining + 0.01 < baseline_remaining,
            "visual speed multiplier should speed up scripted camera updates"
        );
    }

    #[test]
    fn freeze_and_unfreeze_time_toggle_script_freeze_state() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic.mission_scripts.push_script_freeze_time(true);
        game_logic.evaluate_and_execute_scripts(0.0);
        assert!(game_logic.script_time_frozen_by_script);
        assert!(game_logic.is_script_time_frozen());

        game_logic.mission_scripts.push_script_freeze_time(false);
        game_logic.evaluate_and_execute_scripts(0.0);
        assert!(!game_logic.script_time_frozen_by_script);
        assert!(!game_logic.is_script_time_frozen());
    }

    #[test]
    fn set_fps_limit_request_is_forwarded_to_engine() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_set_fps_limit(SetFpsLimitRequest { fps: 90 });
        game_logic.evaluate_and_execute_scripts(0.0);

        assert_eq!(game_logic.take_script_fps_limit_request(), Some(90));
        assert_eq!(game_logic.take_script_fps_limit_request(), None);
    }

    #[test]
    fn move_camera_to_selection_uses_local_player_selection_center() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;
        ensure_test_player_for_team(&mut game_logic, Team::USA);
        ensure_test_tank_template(&mut game_logic);

        let first = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 200.0))
            .expect("first selected object should exist");
        let second = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(160.0, 0.0, 260.0))
            .expect("second selected object should exist");
        game_logic.select_objects(0, vec![first, second]);

        game_logic.mission_scripts.push_camera_move_to_selection();
        game_logic.evaluate_and_execute_scripts(0.0);

        let focus = game_logic
            .take_camera_focus_request()
            .expect("move camera to selection should produce focus request");
        assert_eq!(focus, Vec3::new(130.0, 0.0, 230.0));
    }

    #[test]
    fn move_camera_to_selection_with_empty_selection_is_noop() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;
        ensure_test_player_for_team(&mut game_logic, Team::USA);

        game_logic.select_objects(0, Vec::new());
        game_logic.mission_scripts.push_camera_move_to_selection();
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!(
            game_logic.take_camera_focus_request().is_none(),
            "empty selection should not emit camera focus request"
        );
    }

    #[test]
    fn camera_set_default_updates_script_camera_defaults() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_set_default(CameraSetDefaultRequest {
                pitch: 0.8,
                angle: 35.0,
                max_height: 2.0,
            });
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!((game_logic.script_default_camera_pitch - 0.8).abs() < f32::EPSILON);
        assert!(
            game_logic.script_default_camera_angle.abs() < f32::EPSILON,
            "C++ W3DView::setDefaultView ignores the angle parameter"
        );
        assert!((game_logic.script_default_camera_max_height - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn camera_set_default_sanitizes_non_finite_max_height() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_set_default(CameraSetDefaultRequest {
                pitch: 0.9,
                angle: 0.0,
                max_height: f32::NAN,
            });
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!((game_logic.script_default_camera_max_height - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn camera_set_default_allows_zero_max_height_scale_like_cpp() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_set_default(CameraSetDefaultRequest {
                pitch: 1.0,
                angle: 15.0,
                max_height: 0.0,
            });
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!(game_logic.script_default_camera_angle.abs() < f32::EPSILON);
        assert!(game_logic.script_default_camera_max_height.abs() < f32::EPSILON);
    }

    #[test]
    fn script_screen_shake_requests_are_drained_for_engine() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_screen_shake(ScreenShakeRequest { intensity: 2 });
        game_logic
            .mission_scripts
            .push_screen_shake(ScreenShakeRequest { intensity: 5 });
        game_logic.evaluate_and_execute_scripts(0.0);

        let shakes = game_logic.take_screen_shake_requests();
        assert_eq!(shakes.len(), 2);
        assert_eq!(shakes[0].intensity, 2);
        assert_eq!(shakes[1].intensity, 5);
        assert!(
            game_logic.take_screen_shake_requests().is_empty(),
            "screen shake queue should be drained after take"
        );
    }

    #[test]
    fn camera_add_shaker_requests_are_drained_for_engine() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_add_shaker(CameraAddShakerRequest {
                position: Vec3::new(10.0, 4.0, 20.0),
                amplitude: 3.5,
                duration_seconds: 2.0,
                radius: 120.0,
            });
        game_logic.evaluate_and_execute_scripts(0.0);

        let shakers = game_logic.take_camera_add_shaker_requests();
        assert_eq!(shakers.len(), 1);
        assert_eq!(shakers[0].position, Vec3::new(10.0, 4.0, 20.0));
        assert!((shakers[0].amplitude - 3.5).abs() < f32::EPSILON);
        assert!((shakers[0].duration_seconds - 2.0).abs() < f32::EPSILON);
        assert!((shakers[0].radius - 120.0).abs() < f32::EPSILON);
        assert!(
            game_logic.take_camera_add_shaker_requests().is_empty(),
            "camera shaker queue should be drained after take"
        );
    }

    #[test]
    fn camera_slave_mode_enable_and_disable_requests_are_drained_for_engine() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_camera_slave_mode_enable(CameraSlaveModeRequest {
                thing_template_name: "CineRig".to_string(),
                bone_name: "Bone01".to_string(),
            });
        game_logic.evaluate_and_execute_scripts(0.0);

        let enable = game_logic
            .take_camera_slave_mode_enable_request()
            .expect("slave mode enable should be forwarded");
        assert_eq!(enable.thing_template_name, "CineRig");
        assert_eq!(enable.bone_name, "Bone01");
        assert!(
            !game_logic.take_camera_slave_mode_disable_request(),
            "enable should not set disable flag"
        );

        game_logic.mission_scripts.push_camera_slave_mode_disable();
        game_logic.evaluate_and_execute_scripts(0.0);
        assert!(
            game_logic.take_camera_slave_mode_disable_request(),
            "disable should set disable flag"
        );
        assert!(
            game_logic.take_camera_slave_mode_enable_request().is_none(),
            "disable should clear pending enable request"
        );
    }

    #[test]
    fn camera_move_home_prefers_local_command_center() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;
        ensure_test_player_for_team(&mut game_logic, Team::USA);
        ensure_test_structure_template(&mut game_logic);
        ensure_test_command_center_template(&mut game_logic);
        game_logic.objects.clear();

        game_logic
            .create_object("TestBuilding", Team::USA, Vec3::new(80.0, 0.0, 90.0))
            .expect("fallback structure should exist");
        game_logic
            .create_object("TestCommandCenter", Team::USA, Vec3::new(320.0, 0.0, 410.0))
            .expect("command center should exist");

        game_logic.mission_scripts.push_camera_move_home();
        game_logic.evaluate_and_execute_scripts(0.0);

        let focus = game_logic
            .take_camera_focus_request()
            .expect("camera move home should produce focus request");
        assert_eq!(focus, Vec3::new(320.0, 0.0, 410.0));
    }

    #[test]
    fn camera_move_home_falls_back_to_local_team_base() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;
        ensure_test_player_for_team(&mut game_logic, Team::USA);
        ensure_test_structure_template(&mut game_logic);
        game_logic.objects.clear();

        game_logic
            .create_object("TestBuilding", Team::USA, Vec3::new(190.0, 0.0, 260.0))
            .expect("team base structure should exist");

        game_logic.mission_scripts.push_camera_move_home();
        game_logic.evaluate_and_execute_scripts(0.0);

        let focus = game_logic
            .take_camera_focus_request()
            .expect("camera move home should focus local team base");
        assert_eq!(focus, Vec3::new(190.0, 0.0, 260.0));
    }

    #[test]
    fn camera_move_home_without_local_player_is_noop() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;
        game_logic.players.clear();

        game_logic.mission_scripts.push_camera_move_home();
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!(
            game_logic.take_camera_focus_request().is_none(),
            "camera move home should no-op when no local player exists"
        );
    }

    #[test]
    fn weather_visibility_script_requests_apply_last_value() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        assert!(game_logic.weather_state().visible);

        game_logic.mission_scripts.push_weather_visible(false);
        game_logic.mission_scripts.push_weather_visible(true);
        game_logic.mission_scripts.push_weather_visible(false);
        game_logic.evaluate_and_execute_scripts(0.0);

        assert!(
            !game_logic.weather_state().visible,
            "weather visibility should follow the final script request"
        );

        game_logic.mission_scripts.push_weather_visible(true);
        game_logic.evaluate_and_execute_scripts(0.0);
        assert!(game_logic.weather_state().visible);
    }

    #[test]
    fn popup_and_script_ui_requests_are_forwarded_into_runtime_state() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_popup_message(ScriptPopupMessageRequest {
                message: "Script popup".to_string(),
                x_percent: 45,
                y_percent: 60,
                width: 420,
                pause: true,
                pause_music: true,
            });
        game_logic
            .mission_scripts
            .push_view_guardband(ViewGuardbandRequest {
                x_bias: 1.4,
                y_bias: 0.6,
            });
        game_logic
            .mission_scripts
            .push_camera_bw_mode(CameraBwModeRequest {
                enabled: true,
                frames: 30,
            });
        game_logic.mission_scripts.push_skybox_enabled(false);
        game_logic
            .mission_scripts
            .push_camera_motion_blur(CameraMotionBlurRequest::Basic {
                zoom_in: true,
                saturate: false,
            });
        game_logic
            .mission_scripts
            .push_camera_motion_blur(CameraMotionBlurRequest::Jump {
                position: Vec3::new(120.0, 20.0, 260.0),
                saturate: true,
            });
        game_logic
            .mission_scripts
            .push_named_timer_mutation(NamedTimerMutation::Add {
                name: "LaunchClock".to_string(),
                text: "Launch in".to_string(),
                countdown: true,
            });
        game_logic.mission_scripts.push_named_timer_display(false);
        game_logic
            .mission_scripts
            .push_superweapon_display_enabled(false);
        game_logic
            .mission_scripts
            .push_superweapon_object_display_mutation(SuperweaponObjectDisplayMutation::Hide {
                object_id: 88,
            });
        game_logic
            .mission_scripts
            .push_cameo_flash(CameoFlashRequest {
                command_button_name: "CommandButtonA".to_string(),
                flash_count: 6,
            });

        game_logic.evaluate_and_execute_scripts(0.0);

        assert!(game_logic.is_paused, "popup pause should pause simulation");
        assert!(
            game_logic.take_music_stop_request(),
            "popup pause_music should request music stop"
        );

        let popups = game_logic.take_popup_message_requests();
        assert_eq!(popups.len(), 1);
        assert_eq!(popups[0].message, "Script popup");
        assert_eq!(popups[0].x_percent, 45);
        assert_eq!(popups[0].y_percent, 60);
        assert_eq!(popups[0].width, 420);
        assert!(popups[0].pause);
        assert!(popups[0].pause_music);

        let guardband = game_logic
            .take_view_guardband_request()
            .expect("view guardband request should be pending");
        assert!((guardband.x_bias - 1.4).abs() < f32::EPSILON);
        assert!((guardband.y_bias - 0.6).abs() < f32::EPSILON);

        let bw = game_logic
            .take_camera_bw_mode_request()
            .expect("camera bw request should be pending");
        assert!(bw.enabled);
        assert_eq!(bw.frames, 30);

        assert!(
            !game_logic.script_skybox_enabled,
            "skybox flag should reflect latest script update"
        );
        assert_eq!(
            game_logic
                .script_cameo_flash_count
                .get("CommandButtonA")
                .copied(),
            Some(6)
        );
        assert_eq!(
            game_logic.script_named_timers.get("LaunchClock"),
            Some(&("Launch in".to_string(), true))
        );
        assert!(
            !game_logic.script_named_timer_display_shown,
            "named timer display should be disabled by script"
        );
        assert!(
            !game_logic.script_superweapon_display_enabled,
            "superweapon display should be disabled by script"
        );
        assert!(
            game_logic
                .script_superweapon_hidden_objects
                .contains(&ObjectId(88)),
            "hidden object list should include scripted object id"
        );

        let blur_requests = game_logic.take_camera_motion_blur_requests();
        assert_eq!(blur_requests.len(), 2);
        assert!(matches!(
            blur_requests[0],
            CameraMotionBlurRequest::Basic {
                zoom_in: true,
                saturate: false
            }
        ));
        assert!(matches!(
            blur_requests[1],
            CameraMotionBlurRequest::Jump {
                position,
                saturate: true
            } if position == Vec3::new(120.0, 20.0, 260.0)
        ));

        let jump_focus = game_logic
            .take_camera_focus_request()
            .expect("motion blur jump should emit a camera focus fallback");
        assert_eq!(jump_focus, Vec3::new(120.0, 20.0, 260.0));
    }

    #[test]
    fn script_named_timer_and_superweapon_mutations_respect_order() {
        let mut game_logic = GameLogic::new();
        game_logic.scripts_loaded = true;

        game_logic
            .mission_scripts
            .push_named_timer_mutation(NamedTimerMutation::Add {
                name: "TimerA".to_string(),
                text: "Phase 1".to_string(),
                countdown: true,
            });
        game_logic
            .mission_scripts
            .push_named_timer_mutation(NamedTimerMutation::Remove {
                name: "TimerA".to_string(),
            });
        game_logic
            .mission_scripts
            .push_named_timer_mutation(NamedTimerMutation::Add {
                name: "TimerA".to_string(),
                text: "Phase 2".to_string(),
                countdown: false,
            });
        game_logic
            .mission_scripts
            .push_superweapon_object_display_mutation(SuperweaponObjectDisplayMutation::Hide {
                object_id: 123,
            });
        game_logic
            .mission_scripts
            .push_superweapon_object_display_mutation(SuperweaponObjectDisplayMutation::Show {
                object_id: 123,
            });

        game_logic.evaluate_and_execute_scripts(0.0);

        assert_eq!(
            game_logic.script_named_timers.get("TimerA"),
            Some(&("Phase 2".to_string(), false)),
            "later timer mutation should win"
        );
        assert!(
            !game_logic
                .script_superweapon_hidden_objects
                .contains(&ObjectId(123)),
            "show mutation should undo prior hide mutation for the same object"
        );
    }
}
