//! Wave 956: host_object/host_objects authority dual-read seal.
//! Wave 958: host_object dual-read seal (tests + residual).
use crate::game_logic::host_rng_residual::HostRandomState;
use crate::game_logic::*;
use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};

const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

/// C++ `BuildListInfo::UNLIMITED_REBUILDS` (`SidesList.h:240`).
pub const UNLIMITED_REBUILDS: u32 = u32::MAX;

/// C++ `INVALID_SKILLSET_SELECTION` (`AIPlayer.h:13`).
const INVALID_SKILLSET_SELECTION: i32 = -1;
/// C++ `AIPlayer::MAX_STRUCTURES_TO_REPAIR` (`AIPlayer.h:262`).
const MAX_STRUCTURES_TO_REPAIR: usize = 2;
/// C++ `HUGE_DIST` (`PartitionManager.h:45`).
const HUGE_DIST: f32 = 1_000_000.0;
/// C++ `TeamInQueue::isBuildTimeExpired` uses prototype `m_initialIdleFrames`.
/// `< 1` means unlimited (never expires). No prototype → unlimited.

/// AI difficulty levels affecting decision making and timing
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AIDifficulty {
    Easy,
    Medium,
    Hard,
    Brutal,
}

impl AIDifficulty {
    /// Get build delay modifier for this difficulty
    pub fn get_build_delay_modifier(&self) -> f32 {
        match self {
            AIDifficulty::Easy => 2.0,   // 2x slower building
            AIDifficulty::Medium => 1.0, // Normal speed
            AIDifficulty::Hard => 0.7,   // 30% faster
            AIDifficulty::Brutal => 0.5, // 50% faster
        }
    }

    /// Get resource bonus for this difficulty
    pub fn get_resource_bonus(&self) -> f32 {
        match self {
            AIDifficulty::Easy => 0.8,   // 20% less resources
            AIDifficulty::Medium => 1.0, // Normal resources
            AIDifficulty::Hard => 1.2,   // 20% bonus
            AIDifficulty::Brutal => 1.5, // 50% bonus
        }
    }

    /// Get aggressive behavior factor
    pub fn get_aggression_factor(&self) -> f32 {
        match self {
            AIDifficulty::Easy => 0.6,   // Less aggressive
            AIDifficulty::Medium => 1.0, // Normal aggression
            AIDifficulty::Hard => 1.4,   // More aggressive
            AIDifficulty::Brutal => 1.8, // Very aggressive
        }
    }
}

/// AI personality types for different playstyles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIPersonality {
    Balanced,   // Mix of offense and defense
    Aggressive, // Fast attacks, less defense
    Defensive,  // Strong defense, slower to attack
    Economic,   // Focus on economy first
    Rush,       // Early, fast attacks
}

impl AIPersonality {
    /// Get personality for a team
    pub fn for_team(team: Team) -> Self {
        match team {
            Team::USA => AIPersonality::Aggressive, // USA is aggressive with advanced tech
            Team::China => AIPersonality::Defensive, // China builds strong defenses
            Team::GLA => AIPersonality::Rush,       // GLA rushes with cheap units
            Team::Neutral => AIPersonality::Balanced,
        }
    }
}

/// AI work order for unit production
#[derive(Debug, Clone)]
pub struct AIWorkOrder {
    pub template_name: String,
    pub factory_id: Option<ObjectId>,
    /// Units actually enqueued for this work order but not yet observed at a
    /// factory exit.  This is intentionally separate from `num_completed`:
    /// C++ `AIPlayer::onUnitProduced` increments completion only after the
    /// `ProductionUpdate` has created the unit.
    pub queued_count: u32,
    pub num_completed: u32,
    pub num_required: u32,
    pub is_required: bool,
    pub priority: u32,
    /// Producer-linked units already accounted for by this order.  A factory
    /// can have produced matching units before this order was queued, so those
    /// must not be mistaken for this team's completion when the host polls the
    /// authoritative production result.
    pub observed_unit_ids: Vec<ObjectId>,
    /// C++ `WorkOrder::m_isResourceGatherer`.  This is only set for the paid
    /// follow-up collector that `AIPlayer::queueSupplyTruck` prepends; the
    /// SupplyCenter's free SpawnBehavior collector has no work order.
    pub is_resource_gatherer: bool,
    /// SupplyCenter/Stash this collector is being assigned to.  Keeping the
    /// concrete producer avoids template/name inference for general-specific
    /// collectors and lets `onUnitProduced` route it to the nearby source.
    pub supply_center_id: Option<ObjectId>,
}

impl AIWorkOrder {
    pub fn new(template_name: String, count: u32, priority: u32) -> Self {
        Self {
            template_name,
            factory_id: None,
            queued_count: 0,
            num_completed: 0,
            num_required: count,
            is_required: true,
            priority,
            observed_unit_ids: Vec::new(),
            is_resource_gatherer: false,
            supply_center_id: None,
        }
    }
}

/// AI team build / ready queue entry (`TeamInQueue` residual).
#[derive(Debug, Clone)]
pub struct AITeamQueue {
    pub name: String,
    /// C++ `TeamInQueue::m_team` instance id (leftover TeamFactory handle).
    pub team_id: Option<u32>,
    pub work_orders: Vec<AIWorkOrder>,
    pub priority_build: bool,
    pub frame_started: u32,
    pub completed: bool,
    /// C++ `TeamPrototype::m_executeActions` residual.
    pub execute_actions: bool,
    /// C++ `TeamInQueue::m_sentToStartLocation`.
    pub sent_to_start_location: bool,
    /// Host residual of `Team::setActive()` / OnCreate.
    pub activated: bool,
    /// C++ `TeamInQueue::m_reinforcement`.
    pub reinforcement: bool,
    /// C++ `TeamInQueue::m_reinforcementID`.
    pub reinforcement_id: Option<ObjectId>,
}

impl AITeamQueue {
    fn new(
        name: String,
        work_orders: Vec<AIWorkOrder>,
        priority_build: bool,
        frame_started: u32,
    ) -> Self {
        Self {
            name,
            team_id: None,
            work_orders,
            priority_build,
            frame_started,
            completed: false,
            execute_actions: false,
            sent_to_start_location: false,
            activated: false,
            reinforcement: false,
            reinforcement_id: None,
        }
    }

    /// C++ `TeamInQueue::isAllBuilt`.
    fn is_all_built(&self) -> bool {
        self.work_orders
            .iter()
            .all(|order| order.num_completed >= order.num_required)
    }

    /// C++ `TeamInQueue::isMinimumBuilt`: assigned factory counts as +1.
    fn is_minimum_built(&self) -> bool {
        for order in self.work_orders.iter().filter(|o| o.is_required) {
            let mut count = order.num_completed;
            if order.factory_id.is_some() {
                count = count.saturating_add(1);
            }
            if order.num_required > count {
                return false;
            }
        }
        true
    }

    /// C++ `TeamInQueue::areBuildsComplete`: no work order still bound to a factory.
    fn are_builds_complete(&self) -> bool {
        self.work_orders
            .iter()
            .all(|order| order.factory_id.is_none())
    }

    /// C++ `TeamInQueue::isBuildTimeExpired` (`AIPlayer.cpp:3488-3496`).
    fn is_build_time_expired(&self, current_time: f32) -> bool {
        let proto_name = self
            .team_id
            .and_then(Self::leftover_team_proto_name)
            .unwrap_or_else(|| self.name.clone());
        let idle_frames = Self::leftover_initial_idle_frames(&proto_name);
        if idle_frames < 1 {
            return false;
        }
        let now = (current_time * LOGIC_FRAMES_PER_SECOND) as u32;
        now > self.frame_started.saturating_add(idle_frames as u32)
    }

    fn leftover_team_proto_name(team_id: u32) -> Option<String> {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_by_id(team_id)
                    .and_then(|arc| arc.read().ok().map(|t| t.get_name().to_string()))
            })
    }

    fn leftover_initial_idle_frames(team_name: &str) -> i32 {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(team_name)
                    .map(|proto| proto.get_initial_idle_frames())
            })
            .unwrap_or(0)
    }
}

/// Host residual of C++ OnCreate script first action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnCreateIntent {
    None,
    Hunt,
    HuntWithCommandButton,
    Guard,
    AttackMove,
}

/// AI building info for base construction
#[derive(Debug, Clone)]
pub struct AIBuildingInfo {
    pub template_name: String,
    pub position: Vec3,
    pub object_id: Option<ObjectId>,
    pub is_built: bool,
    pub is_priority: bool,
    /// C++ `BuildListInfo::m_automaticallyBuild`. Layout pads are automatic;
    /// `canMakeUnit(dozer, NULL)` never selects them. Scripts stamp priority.
    pub automatic_build: bool,
    pub rebuild_count: u32,
    pub max_rebuilds: u32,
    /// Host residual of C++ BuildListInfo objectTimestamp (seconds).
    /// Set when the live building is destroyed; rebuild waits RebuildDelaySeconds.
    pub destroyed_at_time: Option<f32>,
}

impl AIBuildingInfo {
    pub fn new(template_name: String, position: Vec3, max_rebuilds: u32) -> Self {
        Self {
            template_name,
            position,
            object_id: None,
            is_built: false,
            is_priority: false,
            automatic_build: true,
            rebuild_count: 0,
            max_rebuilds,
            destroyed_at_time: None,
        }
    }

    /// C++ rebuild delay residual: ready when never destroyed or delay elapsed.
    pub fn rebuild_delay_elapsed(&self, current_time: f32, delay_seconds: f32) -> bool {
        match self.destroyed_at_time {
            None => true,
            Some(t0) => current_time >= t0 + delay_seconds,
        }
    }

    /// C++ `BuildListInfo::isBuildable` (`SidesList.h:361-366`).
    pub fn is_buildable(&self) -> bool {
        self.max_rebuilds == UNLIMITED_REBUILDS || self.rebuild_count < self.max_rebuilds
    }

    /// C++ `BuildListInfo::decrementNumRebuilds` — no-op for `UNLIMITED_REBUILDS`.
    pub fn decrement_num_rebuilds(&mut self) {
        if self.max_rebuilds != UNLIMITED_REBUILDS {
            self.rebuild_count = self.rebuild_count.saturating_add(1);
        }
    }

    /// C++ `BuildListInfo::incrementNumRebuilds`. `newMap` adds one because the
    /// first construction consumes a rebuild (`AISkirmishPlayer.cpp:1083`).
    pub fn increment_num_rebuilds(&mut self) {
        if self.max_rebuilds != UNLIMITED_REBUILDS {
            self.max_rebuilds = self.max_rebuilds.saturating_add(1);
        }
    }
}

/// One AIData `SkirmishBuildList` / `SideBuildList` pad.
#[derive(Debug, Clone)]
struct SideBuildPad {
    template: String,
    position: Vec3,
    rebuilds: i32,
    initially_built: bool,
    automatically_build: bool,
}

#[derive(Debug, Clone)]
struct ReinforceUnit {
    thing: String,
    max_units: i32,
}

#[derive(Debug, Clone)]
struct ReinforceCandidate {
    name: String,
    priority: i32,
    units: Vec<ReinforceUnit>,
}

/// Base AI Player implementation
#[derive(Debug)]
pub struct AIPlayer {
    pub player_id: u32,
    pub team: Team,
    pub difficulty: AIDifficulty,
    pub personality: AIPersonality,

    // Core AI State
    pub is_active: bool,
    pub enemy_player_id: Option<u32>,

    // Economic Management
    pub base_center: Vec3,
    pub base_radius: f32,
    /// Deterministic placement scatter (retail ADC RandomValue residual).
    placement_rng: HostRandomState,
    pub building_queue: Vec<AIBuildingInfo>,
    pub next_building_time: f32,
    /// Existing work orders run on the short C++ `m_teamDelay`; choosing a
    /// new team uses the longer AIData `TeamSeconds` timer below.
    pub next_team_queue_time: f32,
    pub next_team_time: f32,
    /// C++ `AIPlayer::m_teamSeconds`. Script `SET_BASE_CONSTRUCTION_SPEED`
    /// writes this via `set_team_delay_seconds`.
    pub team_seconds: f32,

    // Military Management
    pub team_queue: VecDeque<AITeamQueue>,
    /// C++ `m_teamReadyQueue` — teams that finished building and await activation.
    pub team_ready_queue: VecDeque<AITeamQueue>,

    pub attack_in_progress: bool,
    pub last_attack_time: f32,
    pub defensive_units: Vec<ObjectId>,

    // Timing and Decision Making
    pub last_update_time: f32,
    pub resource_check_time: f32,
    pub enemy_check_time: f32,

    // AI Decision State
    pub current_strategy: AIStrategy,
    pub build_phase: AIBuildPhase,

    /// Count of production-linked actions (build/produce/attack) for gates.
    pub activity_count: u64,
    /// C++ `m_skillsetSelector`. `-1` until first `doUpgradesAndSkills`.
    skillset_selector: i32,
    /// Other skirmish AIs' current enemy (set by `AIManager` before `update`).
    peer_ai_targets: Vec<(u32, Option<u32>)>,

    // C++ `AIPlayer` bridge-repair queue (`AIPlayer.h:261-268`).
    structures_to_repair: Vec<ObjectId>,
    repair_dozer: Option<ObjectId>,
    repair_dozer_origin: Vec3,
    dozer_queued_for_repair: bool,
    dozer_is_repairing: bool,
    last_bridge_repair_time: f32,

    // C++ `AISkirmishPlayer` front/flank defense fan (`AISkirmishPlayer.cpp:50-57`).
    cur_front_base_defense: i32,
    cur_flank_base_defense: i32,
    cur_front_left_defense_angle: f32,
    cur_front_right_defense_angle: f32,
    cur_left_flank_left_defense_angle: f32,
    cur_left_flank_right_defense_angle: f32,
    cur_right_flank_left_defense_angle: f32,
    cur_right_flank_right_defense_angle: f32,
    /// C++ `AISkirmishPlayer::newMap` applied once (AIData SideBuildList).
    skirmish_new_map_applied: bool,
    /// C++ `AIPlayer::m_curWarehouseID` for `buildBySupplies`.
    current_warehouse_id: Option<ObjectId>,
    /// C++ `AIPlayer::m_supplySourceAttackCheckFrame`.
    supply_source_attack_check_frame: u32,
    /// C++ `AIPlayer::m_attackedSupplyCenter`.
    attacked_supply_center: Option<ObjectId>,
}

/// AI strategic states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIStrategy {
    EarlyGame, // Focus on base building and early units
    MidGame,   // Balanced expansion and military buildup
    LateGame,  // Advanced units and multiple attack groups
    Desperate, // Low on resources/units, all-in attacks
}

/// AI build phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIBuildPhase {
    BaseConstruction, // Building core base structures
    UnitProduction,   // Building initial army
    Expansion,        // Expanding economy
    MassProduction,   // Building large armies
}

mod combat;
mod destination_clearance;
mod economy;
mod manager;
mod player_core;
mod teams;

#[cfg(test)]
mod cpp_parity_tests;

pub use manager::AIManager;
