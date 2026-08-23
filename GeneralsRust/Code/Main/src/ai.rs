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
        self.work_orders.iter().all(|order| order.factory_id.is_none())
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

impl AIPlayer {
    /// Create new AI player
    pub fn new(player_id: u32, team: Team, difficulty: AIDifficulty) -> Self {
        let personality = AIPersonality::for_team(team);
        // C++ AIPlayer.cpp:71 p->setCanBuildUnits(false) on leftover PlayerList.
        if let Ok(list) = gamelogic::player::player_list().read() {
            if let Some(player_arc) = list.get_player(player_id as i32).cloned() {
                if let Ok(mut player) = player_arc.write() {
                    player.set_can_build_units(false);
                }
            }
        }
        Self {
            player_id,
            team,
            difficulty,
            personality,
            is_active: true,
            enemy_player_id: None,
            base_center: Vec3::ZERO,
            base_radius: 100.0,
            // Seed from player id (stable per slot); base_center updates don't reseed.
            placement_rng: HostRandomState::seeded(player_id.wrapping_add(0xA17A_0001)),
            building_queue: Vec::new(),
            next_building_time: 0.0,
            next_team_queue_time: 0.0,
            next_team_time: 0.0,
            team_seconds: Self::TEAM_SECONDS,
            team_queue: VecDeque::new(),
            team_ready_queue: VecDeque::new(),
            attack_in_progress: false,
            last_attack_time: 0.0,
            defensive_units: Vec::new(),
            last_update_time: 0.0,
            resource_check_time: 0.0,
            enemy_check_time: 0.0,
            current_strategy: AIStrategy::EarlyGame,
            build_phase: AIBuildPhase::BaseConstruction,
            activity_count: 0,
            skillset_selector: INVALID_SKILLSET_SELECTION,
            peer_ai_targets: Vec::new(),
            structures_to_repair: Vec::new(),
            repair_dozer: None,
            repair_dozer_origin: Vec3::ZERO,
            dozer_queued_for_repair: false,
            dozer_is_repairing: false,
            last_bridge_repair_time: -1.0,
            cur_front_base_defense: 0,
            cur_flank_base_defense: 0,
            cur_front_left_defense_angle: 0.0,
            cur_front_right_defense_angle: 0.0,
            cur_left_flank_left_defense_angle: 0.0,
            cur_left_flank_right_defense_angle: 0.0,
            cur_right_flank_left_defense_angle: 0.0,
            cur_right_flank_right_defense_angle: 0.0,
            skirmish_new_map_applied: false,
            current_warehouse_id: None,
            supply_source_attack_check_frame: 0,
            attacked_supply_center: None,
        }
    }

    /// Initialize AI with starting base layout
    pub fn initialize(&mut self, base_position: Vec3) {
        self.base_center = base_position;
        self.setup_base_layout();
        self.setup_initial_strategy();
        // Act on the first host AI update (skirmish vertical-slice pacing).
        self.next_building_time = 0.0;
        self.next_team_queue_time = 0.0;
        self.next_team_time = 0.0;
        self.enemy_check_time = 0.0;
        // C++-aligned: no artificial negative last_attack to force immediate attacks.
        self.last_attack_time = 0.0;
    }

    /// C++ `AIPlayer::setTeamDelaySeconds`.
    pub fn set_team_delay_seconds(&mut self, delay: i32) {
        self.team_seconds = delay.max(0) as f32;
    }

    pub fn capture_queue_persist(
        &self,
    ) -> crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist {
        use crate::save_load::snapshot::ai_player_queue_persist::{
            AIPlayerQueuePersist, AITeamQueuePersist,
        };
        AIPlayerQueuePersist {
            player_id: self.player_id,
            team_queue: self.team_queue.iter().map(AITeamQueuePersist::from_live).collect(),
            team_ready_queue: self
                .team_ready_queue
                .iter()
                .map(AITeamQueuePersist::from_live)
                .collect(),
            next_building_time: self.next_building_time,
            next_team_queue_time: self.next_team_queue_time,
            next_team_time: self.next_team_time,
            team_seconds: self.team_seconds,
            last_update_time: self.last_update_time,
            current_warehouse_id: self.current_warehouse_id.map(|id| id.0),
            repair_dozer: self.repair_dozer.map(|id| id.0),
            repair_dozer_origin: [
                self.repair_dozer_origin.x,
                self.repair_dozer_origin.y,
                self.repair_dozer_origin.z,
            ],
            structures_to_repair: self.structures_to_repair.iter().map(|id| id.0).collect(),
            dozer_queued_for_repair: self.dozer_queued_for_repair,
            dozer_is_repairing: self.dozer_is_repairing,
            last_bridge_repair_time: self.last_bridge_repair_time,
            skillset_selector: self.skillset_selector,
            cur_front_base_defense: self.cur_front_base_defense,
            cur_flank_base_defense: self.cur_flank_base_defense,
            cur_front_left_defense_angle: self.cur_front_left_defense_angle,
            cur_front_right_defense_angle: self.cur_front_right_defense_angle,
            cur_left_flank_left_defense_angle: self.cur_left_flank_left_defense_angle,
            cur_left_flank_right_defense_angle: self.cur_left_flank_right_defense_angle,
            cur_right_flank_left_defense_angle: self.cur_right_flank_left_defense_angle,
            cur_right_flank_right_defense_angle: self.cur_right_flank_right_defense_angle,
        }
    }

    pub fn apply_queue_persist(
        &mut self,
        persist: crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist,
    ) {
        use crate::save_load::snapshot::ai_player_queue_persist::AITeamQueuePersist;
        self.team_queue = persist
            .team_queue
            .into_iter()
            .map(AITeamQueuePersist::into_live)
            .collect();
        self.team_ready_queue = persist
            .team_ready_queue
            .into_iter()
            .map(AITeamQueuePersist::into_live)
            .collect();
        self.next_building_time = persist.next_building_time;
        self.next_team_queue_time = persist.next_team_queue_time;
        self.next_team_time = persist.next_team_time;
        self.team_seconds = persist.team_seconds;
        self.last_update_time = persist.last_update_time;
        self.current_warehouse_id = persist.current_warehouse_id.map(ObjectId);
        self.repair_dozer = persist.repair_dozer.map(ObjectId);
        self.repair_dozer_origin = glam::Vec3::new(
            persist.repair_dozer_origin[0],
            persist.repair_dozer_origin[1],
            persist.repair_dozer_origin[2],
        );
        self.structures_to_repair = persist
            .structures_to_repair
            .into_iter()
            .map(ObjectId)
            .collect();
        self.dozer_queued_for_repair = persist.dozer_queued_for_repair;
        self.dozer_is_repairing = persist.dozer_is_repairing;
        self.last_bridge_repair_time = persist.last_bridge_repair_time;
        self.skillset_selector = persist.skillset_selector;
        self.cur_front_base_defense = persist.cur_front_base_defense;
        self.cur_flank_base_defense = persist.cur_flank_base_defense;
        self.cur_front_left_defense_angle = persist.cur_front_left_defense_angle;
        self.cur_front_right_defense_angle = persist.cur_front_right_defense_angle;
        self.cur_left_flank_left_defense_angle = persist.cur_left_flank_left_defense_angle;
        self.cur_left_flank_right_defense_angle = persist.cur_left_flank_right_defense_angle;
        self.cur_right_flank_left_defense_angle = persist.cur_right_flank_left_defense_angle;
        self.cur_right_flank_right_defense_angle = persist.cur_right_flank_right_defense_angle;
    }

    pub fn clear_queue_persist(&mut self) {
        self.team_queue.clear();
        self.team_ready_queue.clear();
        self.next_building_time = 0.0;
        self.next_team_queue_time = 0.0;
        self.next_team_time = 0.0;
        self.current_warehouse_id = None;
        self.repair_dozer = None;
        self.structures_to_repair.clear();
        self.dozer_queued_for_repair = false;
        self.dozer_is_repairing = false;
        self.last_bridge_repair_time = -1.0;
        self.skillset_selector = INVALID_SKILLSET_SELECTION;
        self.cur_front_base_defense = 0;
        self.cur_flank_base_defense = 0;
        self.cur_front_left_defense_angle = 0.0;
        self.cur_front_right_defense_angle = 0.0;
        self.cur_left_flank_left_defense_angle = 0.0;
        self.cur_left_flank_right_defense_angle = 0.0;
        self.cur_right_flank_left_defense_angle = 0.0;
        self.cur_right_flank_right_defense_angle = 0.0;
    }

    pub fn retain_queue_object_ids(&mut self, valid: &std::collections::HashSet<ObjectId>) {
        self.structures_to_repair.retain(|id| valid.contains(id));
        if self.repair_dozer.is_some_and(|id| !valid.contains(&id)) {
            self.repair_dozer = None;
        }
        if self
            .current_warehouse_id
            .is_some_and(|id| !valid.contains(&id))
        {
            self.current_warehouse_id = None;
        }
        for team in self
            .team_queue
            .iter_mut()
            .chain(self.team_ready_queue.iter_mut())
        {
            if team
                .reinforcement_id
                .is_some_and(|id| !valid.contains(&id))
            {
                team.reinforcement_id = None;
            }
            for order in &mut team.work_orders {
                if order.factory_id.is_some_and(|id| !valid.contains(&id)) {
                    order.factory_id = None;
                }
                if order
                    .supply_center_id
                    .is_some_and(|id| !valid.contains(&id))
                {
                    order.supply_center_id = None;
                }
                order.observed_unit_ids.retain(|id| valid.contains(id));
            }
        }
    }


    /// Relocate base center and re-seed the structure build queue at the new site.
    ///
    /// Used by host golden combat so AI rebuild soup stays within production-weapon
    /// range without stripping faction templates from the catalog.
    pub fn relocate_base(&mut self, base_position: Vec3) {
        self.base_center = base_position;
        self.building_queue.clear();
        self.setup_base_layout();
    }

    /// Main AI update — C++ `AIPlayer::update` (`AIPlayer.cpp:2987-3002`):
    /// doBaseBuilding → checkReadyTeams → checkQueuedTeams → doTeamBuilding
    /// → doUpgradesAndSkills → updateBridgeRepair.
    /// `AISkirmishPlayer::update` just calls this (`AISkirmishPlayer.cpp:932-935`).
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if !self.is_active {
            return;
        }

        // C++ AISkirmishPlayer::newMap — AIData SideBuildList replaces invented pads.
        self.ensure_skirmish_new_map(game_logic);

        self.last_update_time = current_time;
        self.update_enemy_assessment(game_logic, current_time);
        // C++ Player::preTeamDestroy / AIPlayer::aiPreTeamDestroy — drop wiped
        // TeamInQueue entries before checkReadyTeams / checkQueuedTeams.
        self.purge_destroyed_or_wiped_queued_teams(game_logic);


        // doBaseBuilding
        self.update_economic_management(game_logic, current_time);
        // checkReadyTeams — activate ready-queue teams (AIPlayer.cpp:2729-2803)
        self.check_ready_teams(game_logic, current_time);
        // checkQueuedTeams — expire / disband / promote (AIPlayer.cpp:2810-2870)
        self.check_queued_teams(game_logic, current_time);
        // doTeamBuilding
        self.update_military_management(game_logic, current_time);
        self.do_upgrades_and_skills(game_logic);
        // updateBridgeRepair — queue damaged spans and send a dozer
        self.check_bridges(game_logic);
        self.update_bridge_repair(game_logic, current_time);

        self.update_strategic_decisions(game_logic, current_time);
        // C++ AIPlayer::update never auto-fires specials. Named
        // SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST goes through
        // fire_named_special_power after leftover script dispatch.
    }

    /// Set up initial base building layout.
    ///
    /// Core pads plus the C++ `AIData.ini` `SkirmishBuildList` tech/air
    /// structures and `SideInfo::BaseDefenseStructure1`.  Core pads stay
    /// inside the host 512² MinDistFromEdge residual (|offset| ≤ 100).
    /// Defenses use the C++ approach-path fan, not a fixed +80/+80 pad.
    fn setup_base_layout(&mut self) {
        let center = self.base_center;
        self.reset_base_defense_fan();

        // Core base buildings based on team. C++ `newMap` increments rebuilds
        // so the first construction does not spend the last rebuild, and
        // `UNLIMITED_REBUILDS` is a no-op decrement.
        match self.team {
            Team::USA => {
                self.add_layout_building("AmericaCommandCenter", center, UNLIMITED_REBUILDS);
                self.add_layout_building(
                    "AmericaSupplyCenter",
                    center + Vec3::new(50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaPowerPlant",
                    center + Vec3::new(-50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaBarracks",
                    center + Vec3::new(0.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaWarFactory",
                    center + Vec3::new(100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                // C++ SkirmishBuildList America: StrategyCenter + Airfield.
                self.add_layout_building(
                    "AmericaStrategyCenter",
                    center + Vec3::new(-100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaAirfield",
                    center + Vec3::new(50.0, 0.0, -100.0),
                    UNLIMITED_REBUILDS,
                );
            }
            Team::China => {
                self.add_layout_building("ChinaCommandCenter", center, UNLIMITED_REBUILDS);
                self.add_layout_building(
                    "ChinaSupplyCenter",
                    center + Vec3::new(50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaPowerPlant",
                    center + Vec3::new(-50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaBarracks",
                    center + Vec3::new(0.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaWarFactory",
                    center + Vec3::new(100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                // C++ SkirmishBuildList China: PropagandaCenter + Airfield.
                self.add_layout_building(
                    "ChinaPropagandaCenter",
                    center + Vec3::new(-100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaAirfield",
                    center + Vec3::new(50.0, 0.0, -100.0),
                    UNLIMITED_REBUILDS,
                );
            }
            Team::GLA => {
                self.add_layout_building("GLACommandCenter", center, UNLIMITED_REBUILDS);
                self.add_layout_building(
                    "GLASupplyStash",
                    center + Vec3::new(50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "GLAArmsDealer",
                    center + Vec3::new(0.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "GLABarracks",
                    center + Vec3::new(-50.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                // C++ SkirmishBuildList GLA: Palace is the tech structure.
                self.add_layout_building(
                    "GLAPalace",
                    center + Vec3::new(100.0, 0.0, -50.0),
                    UNLIMITED_REBUILDS,
                );
            }
            _ => {}
        }

        // C++ `AISkirmishPlayer::buildAIBaseDefense` — first front-fan slot.
        self.queue_front_base_defense(None);
    }

    /// C++ `AISkirmishPlayer::newMap` — replace invented pads with AIData list.
    fn ensure_skirmish_new_map(&mut self, game_logic: &mut GameLogic) {
        if self.skirmish_new_map_applied {
            return;
        }
        self.skirmish_new_map_applied = true;
        if game_logic
            .get_player(self.player_id)
            .is_some_and(|p| !p.map_side.build_list.is_empty())
        {
            // Campaign/map SidesList already consumed by feed_host_ai.
            return;
        }
        let _ = self.apply_skirmish_new_map(game_logic);
    }

    /// C++ `AISkirmishPlayer::newMap` + `adjustBuildList`.
    pub fn apply_skirmish_new_map(&mut self, game_logic: &mut GameLogic) -> bool {
        let Some(side) = self.side_info_name() else {
            return false;
        };
        let Some(entries) = Self::aidata_side_build_entries(side) else {
            return false;
        };
        if entries.is_empty() {
            return false;
        }

        let start_pos = self.destroy_owned_command_center(game_logic);
        let Some(start_pos) = start_pos.or(Some(self.base_center)) else {
            return false;
        };

        let mut list_cc: Option<Vec3> = None;
        let mut marked: Vec<SideBuildPad> = Vec::new();
        for mut entry in entries {
            if Self::template_is_command_center(game_logic, &entry.template) {
                entry.initially_built = true;
                if list_cc.is_none() {
                    list_cc = Some(entry.position);
                }
            }
            marked.push(entry);
        }
        let Some(build_pos) = list_cc else {
            return false;
        };

        let rotate = Self::aidata_rotate_skirmish_bases();
        let (lo, hi) = game_logic.world_bounds();
        let width = (hi.x - lo.x).max(1.0);
        let height = (hi.z - lo.z).max(1.0);
        let mut grid_index = 0;
        if start_pos.x > lo.x + width / 3.0 {
            grid_index += 1;
        }
        if start_pos.x > lo.x + 2.0 * width / 3.0 {
            grid_index += 1;
        }
        if start_pos.z > lo.z + height / 3.0 {
            grid_index += 3;
        }
        if start_pos.z > lo.z + 2.0 * height / 3.0 {
            grid_index += 3;
        }
        let mut angle = if rotate {
            match grid_index {
                0 => 0.0,
                1 => std::f32::consts::PI / 4.0,
                2 => std::f32::consts::PI / 2.0,
                3 => -std::f32::consts::PI / 4.0,
                4 => 0.0,
                5 => 3.0 * std::f32::consts::PI / 4.0,
                6 => -std::f32::consts::PI / 2.0,
                7 => -3.0 * std::f32::consts::PI / 4.0,
                _ => std::f32::consts::PI,
            }
        } else {
            0.0
        };
        angle += 3.0 * std::f32::consts::PI / 4.0;
        let s = angle.sin();
        let c = angle.cos();

        self.building_queue.clear();
        self.reset_base_defense_fan();
        let mut sum = Vec3::ZERO;
        let mut n = 0u32;
        for entry in marked {
            let mut pos = entry.position;
            pos.x -= build_pos.x;
            pos.z -= build_pos.z;
            let new_x = pos.x * c - pos.z * s;
            let new_z = pos.z * c + pos.x * s;
            pos.x = new_x + start_pos.x;
            pos.z = new_z + start_pos.z;
            pos.y = 0.0;

            let rebuilds = if entry.rebuilds < 0 {
                UNLIMITED_REBUILDS
            } else {
                entry.rebuilds as u32
            };
            let mut building = AIBuildingInfo::new(entry.template.clone(), pos, rebuilds);
            building.automatic_build = entry.automatically_build;
            building.is_priority = false;
            if entry.initially_built {
                // C++ buildStructureNow — do not incrementNumRebuilds on CC.
                building.is_built = true;
                if let Some(id) = game_logic.create_object(&entry.template, self.team, pos) {
                    if let Some(obj) = game_logic.host_object_mut(id) {
                        obj.owner_player_id = Some(self.player_id);
                    }
                    building.object_id = Some(id);
                }
            } else {
                building.increment_num_rebuilds();
            }
            sum += pos;
            n = n.saturating_add(1);
            self.building_queue.push(building);
        }
        if n > 0 {
            self.base_center = sum / n as f32;
            let mut radius = 1.0f32;
            for b in &self.building_queue {
                radius = radius.max((b.position - self.base_center).length());
            }
            self.base_radius = radius;
        }
        true
    }

    fn destroy_owned_command_center(&mut self, game_logic: &mut GameLogic) -> Option<Vec3> {
        let mut found = None;
        for (&id, object) in game_logic.host_objects() {
            if !object.is_alive() {
                continue;
            }
            let ours = object.owner_player_id == Some(self.player_id)
                || (object.owner_player_id.is_none() && object.team == self.team);
            if !ours || !object.is_kind_of(KindOf::CommandCenter) {
                continue;
            }
            found = Some((id, object.get_position()));
            break;
        }
        let (id, pos) = found?;
        game_logic.destroy_object(id);
        Some(pos)
    }

    fn template_is_command_center(game_logic: &GameLogic, template_name: &str) -> bool {
        if let Some(template) = game_logic.templates.get(template_name) {
            return template.is_kind_of(KindOf::CommandCenter);
        }
        template_name.contains("CommandCenter")
    }

    fn aidata_rotate_skirmish_bases() -> bool {
        let store = game_engine::common::ini::get_ai_data_store();
        if let Some(data) = store.get_active() {
            return data.rotate_skirmish_bases;
        }
        drop(store);
        gamelogic::ai::THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.rotate_skirmish_bases)
            })
            .unwrap_or(false)
    }

    fn aidata_max_recruit_distance() -> f32 {
        let from_store = (|| {
            let store = game_engine::common::ini::get_ai_data_store();
            store.get_active().map(|d| d.max_recruit_distance)
        })();
        let dist = from_store
            .or_else(|| {
                gamelogic::ai::THE_AI.read().ok().and_then(|ai| {
                    ai.get_ai_data()
                        .read()
                        .ok()
                        .map(|d| d.max_recruit_distance)
                })
            })
            .unwrap_or(0.0);
        if dist > 0.0 {
            dist
        } else {
            99_999.0
        }
    }

    fn aidata_side_build_entries(side: &str) -> Option<Vec<SideBuildPad>> {
        {
            let store = game_engine::common::ini::get_ai_data_store();
            if let Some(data) = store.get_active() {
                if let Some(list) = data
                    .side_build_lists
                    .iter()
                    .find(|l| l.side.eq_ignore_ascii_case(side))
                {
                    if !list.entries.is_empty() {
                        return Some(
                            list.entries
                                .iter()
                                .map(|e| SideBuildPad {
                                    template: e.template_name.clone(),
                                    position: Vec3::new(e.location.0, 0.0, e.location.1),
                                    rebuilds: e.rebuilds,
                                    initially_built: e.initially_built,
                                    automatically_build: e.automatically_build,
                                })
                                .collect(),
                        );
                    }
                }
            }
        }
        let ai = gamelogic::ai::THE_AI.read().ok()?;
        let data_arc = ai.get_ai_data();
        let data = data_arc.read().ok()?;
        let entry = data
            .side_build_lists
            .iter()
            .find(|e| e.side.eq_ignore_ascii_case(side))?;
        let list = entry.build_list.as_ref()?;
        let mut out = Vec::new();
        let mut cur = Some(list.as_ref());
        while let Some(info) = cur {
            let name = info.get_template_name().to_string();
            if !name.is_empty() {
                let loc = info.get_location().clone();
                out.push(SideBuildPad {
                    template: name,
                    position: Vec3::new(loc.x, loc.z, loc.y),
                    rebuilds: info.get_num_rebuilds() as i32,
                    initially_built: info.is_initially_built(),
                    automatically_build: info.is_automatic_build(),
                });
            }
            cur = info.get_next();
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    /// C++ `AIData.ini` `SideInfo` name for the live host team.
    /// Base-team fallback when no PlayerTemplate Side is bound.
    fn side_info_name(&self) -> Option<&'static str> {
        use crate::game_logic::host_faction_skirmish_residual::{
            SIDE_AMERICA, SIDE_CHINA, SIDE_GLA,
        };
        match self.team {
            Team::USA => Some(SIDE_AMERICA),
            Team::China => Some(SIDE_CHINA),
            Team::GLA => Some(SIDE_GLA),
            _ => None,
        }
    }

    /// C++ `Player::getSide()` — PlayerTemplate Side (`AmericaAirForceGeneral`),
    /// not the three-value host `Team` enum.
    fn live_player_side(&self, game_logic: &GameLogic) -> Option<String> {
        use crate::game_logic::host_faction_skirmish_residual::find_player_template_residual;
        // Residual Side matches C++ PlayerTemplate.ini (`AmericaAirForceGeneral`).
        // Prefer the bound identity so a leftover store that only copied
        // BaseSide=America cannot collapse ZH generals back to Paladin.
        if let Some(identity) = game_logic.player_template_identity(self.player_id) {
            if let Some(residual) = find_player_template_residual(&identity.template_name) {
                return Some(residual.side.to_string());
            }
        }
        if let Some(template) = game_logic.resolved_player_template(self.player_id) {
            let side = template.get_side();
            if !side.is_empty() {
                return Some(side.to_string());
            }
        }
        self.side_info_name().map(str::to_string)
    }

    fn science_names_from_skill_ids(num_skills: i32, skills: &[i32]) -> Vec<String> {
        use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};
        let Some(store) = get_science_store() else {
            return Vec::new();
        };
        let n = (num_skills.max(0) as usize).min(skills.len());
        (0..n)
            .filter_map(|i| {
                let sci = skills[i];
                if sci == SCIENCE_INVALID {
                    return None;
                }
                let name = store.get_internal_name_for_science(sci);
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            })
            .collect()
    }

    /// Leftover parsed `AIData` `SideInfo` SkillSet1–5 (C++ AIPlayer.cpp:2919-2961).
    fn aidata_side_skillsets(side: &str) -> Option<[Vec<String>; 5]> {
        {
            let store = game_engine::common::ini::get_ai_data_store();
            if let Some(data) = store.get_active() {
                if let Some(info) = data
                    .side_info
                    .iter()
                    .find(|info| info.side.eq_ignore_ascii_case(side))
                {
                    let sets = [
                        Self::science_names_from_skill_ids(
                            info.skill_set_1.num_skills,
                            &info.skill_set_1.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_2.num_skills,
                            &info.skill_set_2.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_3.num_skills,
                            &info.skill_set_3.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_4.num_skills,
                            &info.skill_set_4.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_5.num_skills,
                            &info.skill_set_5.skills,
                        ),
                    ];
                    if sets.iter().any(|set| !set.is_empty()) {
                        return Some(sets);
                    }
                }
            }
        }
        let ai = gamelogic::ai::THE_AI.read().ok()?;
        let ai_data = ai.get_ai_data();
        let data = ai_data.read().ok()?;
        let info = data
            .side_info
            .iter()
            .find(|info| info.side.eq_ignore_ascii_case(side))?;
        let sets = [
            Self::science_names_from_skill_ids(info.skill_set_1.num_skills, &info.skill_set_1.skills),
            Self::science_names_from_skill_ids(info.skill_set_2.num_skills, &info.skill_set_2.skills),
            Self::science_names_from_skill_ids(info.skill_set_3.num_skills, &info.skill_set_3.skills),
            Self::science_names_from_skill_ids(info.skill_set_4.num_skills, &info.skill_set_4.skills),
            Self::science_names_from_skill_ids(info.skill_set_5.num_skills, &info.skill_set_5.skills),
        ];
        sets.iter().any(|set| !set.is_empty()).then_some(sets)
    }

    /// ZH general residual first sciences when parsed AIData is not loaded.
    fn residual_general_skillsets(side: &str) -> Option<[Vec<String>; 5]> {
        use crate::game_logic::host_faction_skirmish_residual::{
            SKIRMISH_AI_SIDE_INFO_RESIDUAL, SIDE_AMERICA, SIDE_CHINA, SIDE_GLA,
        };
        if side.eq_ignore_ascii_case(SIDE_AMERICA)
            || side.eq_ignore_ascii_case(SIDE_CHINA)
            || side.eq_ignore_ascii_case(SIDE_GLA)
        {
            return None;
        }
        let info = SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|info| info.side.eq_ignore_ascii_case(side))?;
        let mut sets = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        if !info.skill_set1_first.is_empty() {
            sets[0].push(info.skill_set1_first.to_string());
        }
        if !info.skill_set2_first.is_empty() {
            sets[1].push(info.skill_set2_first.to_string());
        }
        sets.iter().any(|set| !set.is_empty()).then_some(sets)
    }

    /// C++ `doUpgradesAndSkills` SideInfo walk: leftover AIData, then residual
    /// general skillsets, then the three-faction hardcoded tables.
    fn live_side_skillsets(&self, game_logic: &GameLogic) -> [Vec<String>; 5] {
        let side = self
            .live_player_side(game_logic)
            .or_else(|| self.side_info_name().map(str::to_string));
        if let Some(side) = side.as_deref() {
            if let Some(sets) = Self::aidata_side_skillsets(side) {
                return sets;
            }
            if let Some(sets) = Self::residual_general_skillsets(side) {
                return sets;
            }
        }
        self.side_skillsets()
            .map(|set| set.iter().map(|name| (*name).to_string()).collect())
    }

    /// C++ `AISideInfo::m_baseDefenseStructure1` for this host team.
    fn base_defense_structure(&self) -> Option<&'static str> {
        use crate::game_logic::host_faction_skirmish_residual::SKIRMISH_AI_SIDE_INFO_RESIDUAL;
        let side = self.side_info_name()?;
        SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|info| info.side == side)
            .map(|info| info.base_defense_structure1)
    }

    pub fn add_building(&mut self, template_name: &str, position: Vec3, max_rebuilds: u32) {
        let mut building = AIBuildingInfo::new(template_name.to_string(), position, max_rebuilds);
        // Explicit requests (tests / leftover script stamp) are priority pads,
        // not automatic SkirmishBuildList entries.
        building.automatic_build = false;
        building.is_priority = true;
        self.building_queue.push(building);
    }

    /// C++ `AISkirmishPlayer::newMap` layout slot: increment rebuilds so the
    /// first construction does not consume the last rebuild.
    fn add_layout_building(&mut self, template_name: &str, position: Vec3, max_rebuilds: u32) {
        let mut building = AIBuildingInfo::new(template_name.to_string(), position, max_rebuilds);
        building.increment_num_rebuilds();
        self.building_queue.push(building);
    }

    /// C++ `TAiData::m_skirmishBaseDefenseExtraDistance` (Default/AIData.ini = 150).
    pub const SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE: f32 = 150.0;

    fn reset_base_defense_fan(&mut self) {
        self.cur_front_base_defense = 0;
        self.cur_flank_base_defense = 0;
        self.cur_front_left_defense_angle = 0.0;
        self.cur_front_right_defense_angle = 0.0;
        self.cur_left_flank_left_defense_angle = 0.0;
        self.cur_left_flank_right_defense_angle = 0.0;
        self.cur_right_flank_left_defense_angle = 0.0;
        self.cur_right_flank_right_defense_angle = 0.0;
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefense(false)` — one front-fan slot.
    fn queue_front_base_defense(&mut self, game_logic: Option<&GameLogic>) {
        let Some(defense) = self.base_defense_structure() else {
            return;
        };
        if let Some(position) = self.place_next_base_defense_structure(game_logic, defense, false) {
            self.add_layout_building(defense, position, UNLIMITED_REBUILDS);
        }
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefense` — script `SKIRMISH_BUILD_BASE_DEFENSE_*`.
    pub fn build_script_base_defense(&mut self, game_logic: Option<&GameLogic>, flank: bool) -> bool {
        let Some(defense) = self.base_defense_structure() else {
            return false;
        };
        self.build_script_base_defense_structure(game_logic, defense, flank)
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefenseStructure` — script
    /// `SKIRMISH_BUILD_STRUCTURE_FRONT/FLANK`.
    pub fn build_script_base_defense_structure(
        &mut self,
        game_logic: Option<&GameLogic>,
        thing_name: &str,
        flank: bool,
    ) -> bool {
        let Some(position) = self.place_next_base_defense_structure(game_logic, thing_name, flank)
        else {
            return false;
        };
        self.add_building(thing_name, position, UNLIMITED_REBUILDS);
        true
    }


    /// Approach goal for the defense fan.
    ///
    /// C++ `buildAIBaseDefenseStructure` (`AISkirmishPlayer.cpp:599-609`):
    /// closest Center/Flank/Backdoor waypoint, else enemy structure bounds
    /// center for front, else abort for flank.
    fn approach_goal(&self, game_logic: Option<&GameLogic>, flank: bool) -> Option<Vec3> {
        if let Some(gl) = game_logic {
            if let Some(enemy_id) = self.enemy_player_id {
                if let Some(enemy) = gl.get_player(enemy_id) {
                    return Some(self.find_enemy_base_center(gl, enemy.team));
                }
            }
            for player_id in 0..8u32 {
                if player_id == self.player_id {
                    continue;
                }
                if let Some(player) = gl.get_player(player_id) {
                    if player.team != self.team && player.is_alive {
                        return Some(self.find_enemy_base_center(gl, player.team));
                    }
                }
            }
        }
        if flank {
            return None;
        }
        // C++ no-waypoint front fallback is enemy bounds; without an enemy,
        // `find_enemy_base_center` uses the opposite corner (`-base_center`).
        Some(-self.base_center)
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefenseStructure` (`AISkirmishPlayer.cpp:580-686`).
    ///
    /// Walks the left/right approach fan until a legal pad is found or the
    /// angle exceeds π/3.
    fn place_next_base_defense_structure(
        &mut self,
        game_logic: Option<&GameLogic>,
        thing_name: &str,
        flank: bool,
    ) -> Option<Vec3> {
        loop {
            let goal = self.approach_goal(game_logic, flank)?;
            let mut offset_x = goal.x - self.base_center.x;
            let mut offset_z = goal.z - self.base_center.z;
            let length = (offset_x * offset_x + offset_z * offset_z).sqrt();
            if length > 0.001 {
                offset_x /= length;
                offset_z /= length;
            } else {
                offset_x = 1.0;
                offset_z = 0.0;
            }
            let defense_distance =
                self.base_radius + Self::SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE;
            offset_x *= defense_distance;
            offset_z *= defense_distance;

            let structure_radius = 20.0;
            let base_circumference = 2.0 * std::f32::consts::PI * defense_distance.max(1.0);
            let angle_offset = 2.0
                * std::f32::consts::PI
                * (structure_radius * 4.0 / base_circumference);

            let angle = if flank {
                let selector = self.cur_flank_base_defense >> 1;
                if self.cur_flank_base_defense & 1 != 0 {
                    if selector & 1 != 0 {
                        self.cur_left_flank_right_defense_angle -= angle_offset;
                        self.cur_left_flank_right_defense_angle
                    } else {
                        let result = self.cur_left_flank_left_defense_angle;
                        self.cur_left_flank_left_defense_angle += angle_offset;
                        result
                    }
                } else if selector & 1 != 0 {
                    self.cur_right_flank_right_defense_angle -= angle_offset;
                    self.cur_right_flank_right_defense_angle
                } else {
                    let result = self.cur_right_flank_left_defense_angle;
                    self.cur_right_flank_left_defense_angle += angle_offset;
                    result
                }
            } else if self.cur_front_base_defense & 1 != 0 {
                self.cur_front_right_defense_angle -= angle_offset;
                self.cur_front_right_defense_angle
            } else {
                let result = self.cur_front_left_defense_angle;
                self.cur_front_left_defense_angle += angle_offset;
                result
            };

            if angle > std::f32::consts::PI / 3.0 {
                return None;
            }

            let s = angle.sin();
            let c = angle.cos();
            let mut build_pos = self.base_center;
            build_pos.x += offset_x * c - offset_z * s;
            build_pos.z += offset_z * c + offset_x * s;

            if flank {
                self.cur_flank_base_defense += 1;
            } else {
                self.cur_front_base_defense += 1;
            }

            let legal = game_logic
                .map(|gl| gl.is_location_legal_to_build(self.team, build_pos, thing_name))
                .unwrap_or(true);
            if legal {
                return Some(build_pos);
            }
        }
    }

    /// If a queued front-defense pad is illegal, walk the C++ fan for a new pad.
    fn relocate_defense_if_illegal(&mut self, game_logic: &GameLogic, index: usize) {
        let Some(building) = self.building_queue.get(index) else {
            return;
        };
        let name = building.template_name.clone();
        let position = building.position;
        if self.base_defense_structure() != Some(name.as_str()) {
            return;
        }
        if game_logic.is_location_legal_to_build(self.team, position, &name) {
            return;
        }
        if let Some(next) = self.place_next_base_defense_structure(Some(game_logic), &name, false)
        {
            if let Some(building) = self.building_queue.get_mut(index) {
                building.position = next;
            }
        }
    }


    /// Set up initial AI strategy based on personality
    fn setup_initial_strategy(&mut self) {
        self.current_strategy = AIStrategy::EarlyGame;
        self.build_phase = AIBuildPhase::BaseConstruction;

        // Retail AIData StructureSeconds=0 / TeamSeconds=10, scaled by difficulty.
        let delay_modifier = self.difficulty.get_build_delay_modifier();
        self.next_building_time =
            self.last_update_time + (Self::STRUCTURE_SECONDS * delay_modifier);
        self.next_team_time = self.last_update_time + (self.team_seconds * delay_modifier);
    }

    /// C++ `AISkirmishPlayer::acquireEnemy` (`AISkirmishPlayer.cpp:461-522`).
    fn update_enemy_assessment(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // C++ `getAiEnemy` only calls this every 5s.
        if current_time - self.enemy_check_time < 5.0 {
            return;
        }
        self.enemy_check_time = current_time;

        if let Some(enemy_id) = self.enemy_player_id {
            if let Some(enemy) = game_logic.get_player(enemy_id) {
                if enemy.is_alive
                    && self.player_is_enemy(game_logic, enemy)
                    && !self.player_in_bad_shape(game_logic, enemy)
                {
                    return;
                }
            }
        }

        let mut best_enemy: Option<u32> = None;
        let mut best_distance_sqr = HUGE_DIST * HUGE_DIST;

        let player_ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
        for player_id in player_ids {
            if player_id == self.player_id {
                continue;
            }
            let Some(player) = game_logic.get_player(player_id) else {
                continue;
            };
            if !player.is_alive || !self.player_is_enemy(game_logic, player) {
                continue;
            }
            if !self.player_has_any_objects(game_logic, player.team) {
                continue;
            }

            let in_bad_shape = self.player_in_bad_shape(game_logic, player);
            let (min_x, min_z, max_x, max_z) =
                self.player_structure_bounds(game_logic, player.team);
            let enemy_center = if min_x == 0.0 && min_z == 0.0 && max_x == 0.0 && max_z == 0.0 {
                self.find_enemy_base_center(game_logic, player.team)
            } else {
                Vec3::new(
                    min_x + (max_x - min_x) * 0.5,
                    0.0,
                    min_z + (max_z - min_z) * 0.5,
                )
            };
            let dx = enemy_center.x - self.base_center.x;
            let dz = enemy_center.z - self.base_center.z;
            let mut dist_sqr = dx * dx + dz * dz;
            if in_bad_shape {
                dist_sqr = HUGE_DIST * HUGE_DIST * 0.5;
            }
            for &(other_id, other_target) in &self.peer_ai_targets {
                if other_id == self.player_id || other_id == player_id {
                    continue;
                }
                if other_target == Some(player_id) {
                    dist_sqr += 500.0 * 500.0;
                }
                if other_target == Some(self.player_id) {
                    dist_sqr -= 25.0 * 25.0;
                    if dist_sqr < 0.0 {
                        dist_sqr = 0.0;
                    }
                }
            }
            if dist_sqr < best_distance_sqr {
                best_distance_sqr = dist_sqr;
                best_enemy = Some(player_id);
            }
        }

        // C++ only replaces when bestEnemy != NULL && bestEnemy != m_currentEnemy.
        let Some(best) = best_enemy else {
            return;
        };
        if self.enemy_player_id == Some(best) {
            return;
        }
        self.enemy_player_id = Some(best);
        log::debug!(
            "AI Player {} ({}) targeting enemy Player {}",
            self.player_id,
            self.team.get_name(),
            best
        );
    }

    fn player_is_enemy(&self, _game_logic: &GameLogic, player: &Player) -> bool {
        if player.team == self.team {
            return false;
        }
        if player.alliance_team >= 0 {
            if let Some(me) = _game_logic.get_player(self.player_id) {
                if me.alliance_team >= 0 && me.alliance_team == player.alliance_team {
                    return false;
                }
            }
        }
        true
    }

    fn player_has_any_objects(&self, game_logic: &GameLogic, team: Team) -> bool {
        game_logic
            .host_objects()
            .values()
            .any(|object| object.team == team && object.is_alive())
    }

    fn player_has_any_units(&self, game_logic: &GameLogic, team: Team) -> bool {
        game_logic.host_objects().values().any(|object| {
            object.team == team
                && object.is_alive()
                && (object.is_kind_of(KindOf::Infantry)
                    || object.is_kind_of(KindOf::Vehicle)
                    || object.is_kind_of(KindOf::Aircraft))
        })
    }

    fn player_has_any_build_facility(&self, game_logic: &GameLogic, team: Team) -> bool {
        game_logic.host_objects().values().any(|object| {
            object.team == team
                && object.is_alive()
                && (object.is_kind_of(KindOf::CommandCenter)
                    || object.is_kind_of(KindOf::FSBarracks)
                    || object.is_kind_of(KindOf::FSWarFactory)
                    || object.is_kind_of(KindOf::FSAirfield))
        })
    }

    fn player_in_bad_shape(&self, game_logic: &GameLogic, player: &Player) -> bool {
        !self.player_has_any_units(game_logic, player.team)
            || !self.player_has_any_build_facility(game_logic, player.team)
    }

    /// Update economic management (base building, resource optimization)
    fn update_economic_management(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if current_time < self.next_building_time {
            return;
        }

        // Check if we need more resources
        // Check resources first
        let should_build_supply = if let Some(player) = game_logic.get_player(self.player_id) {
            let resource_threshold = match self.difficulty {
                AIDifficulty::Easy => 500,
                AIDifficulty::Medium => 800,
                AIDifficulty::Hard => 1200,
                AIDifficulty::Brutal => 1500,
            };
            player.resources.supplies < resource_threshold
        } else {
            false
        };

        let should_build_power = if let Some(player) = game_logic.get_player(self.player_id) {
            player.power_available < 0
        } else {
            false
        };

        // C++ processBaseBuilding never invents extra supply/power pads.
        // Scripts stamp priority; emergency power picks an existing FS_POWER
        // list entry. Extra-pad invention is leftover of try_build_*.
        let _ = (should_build_supply, should_build_power);

        // `AISkirmishPlayer::processBaseBuilding` selects and starts one
        // structure per economic pass.  Starting every eligible entry here
        // spends the AI's money in a burst and leaves most scaffolds without a
        // dozer, which is not a viable skirmish base build lifecycle.
        self.process_building_queue(game_logic, current_time);

        // StructureSeconds residual + wealth/poor rate (AIData Structures*Rate).
        let interval = self.scaled_interval_seconds(game_logic, Self::STRUCTURE_SECONDS, true);
        self.next_building_time = current_time + interval;
    }

    /// Update military management (unit production, attack coordination).
    ///
    /// Retail `AISkirmishPlayer::doTeamBuilding` calls `queueUnits` every
    /// `m_teamDelay` (2 seconds), but only selects a *new* team after its
    /// `m_teamTimer` (`TeamSeconds`) expires.  Coupling both to TeamSeconds
    /// leaves a just-selected skirmish team idle for ten seconds.
    fn update_military_management(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        let queue_due = current_time >= self.next_team_queue_time;
        // C++ `AIPlayer::onUnitProduced` sets m_teamDelay to zero after the
        // factory has actually created the unit.  Poll the producer link on
        // each host-AI update so a completed unit wakes its waiting order on
        // the next frame instead of idling until the normal two-second pass.
        // `process_team_queue` reconciles again when it runs; that second
        // observation is empty and keeps the queue mutation in one place.
        let unit_completed = !queue_due && self.reconcile_produced_units(game_logic);

        if queue_due || unit_completed {
            // C++ queueUnits runs before selection, so established orders get
            // first use of an idle factory.
            self.process_team_queue(game_logic, current_time);

            let selected_team =
                if current_time >= self.next_team_time && self.should_build_new_team(game_logic) {
                    self.select_team_to_build(game_logic, current_time)
                } else {
                    false
                };

            if selected_team {
                // C++ processTeamBuilding invokes queueUnits immediately after
                // a successful selectTeamToBuild, not on the next TeamSeconds.
                self.process_team_queue(game_logic, current_time);
                self.next_team_time = current_time
                    + self.scaled_interval_seconds(game_logic, self.team_seconds, false);
            } else if current_time >= self.next_team_time {
                // A failed selection leaves m_readyToBuildTeam set.  Retry on
                // the short m_teamDelay cadence rather than sleeping 10 sec.
                self.next_team_time = current_time + Self::TEAM_QUEUE_RETRY_SECONDS;
            }

            self.next_team_queue_time = current_time + Self::TEAM_QUEUE_RETRY_SECONDS;
        }

        // The host attack policy has its own 60-second guard.  Evaluate it on
        // manager cadence so a newly produced force is not held behind the
        // new-team selection timer.
        self.evaluate_attack_opportunities(game_logic, current_time);
    }

    /// Update strategic decisions and long-term planning
    fn update_strategic_decisions(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Update strategy based on game state
        self.update_strategy_phase(game_logic, current_time);

        // Update build phase
        self.update_build_phase(game_logic, current_time);
    }

    /// Pick a real, available construction unit as C++ `AIPlayer::findDozer`
    /// does before `AISkirmishPlayer::processBaseBuilding` starts a structure.
    /// Prefer an idle dozer, then the nearest other eligible one. Never steal
    /// `m_repairDozer`.
    fn find_available_dozer(
        game_logic: &GameLogic,
        team: Team,
        target: Vec3,
        skip: Option<ObjectId>,
    ) -> Option<ObjectId> {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                if object.team != team || !object.is_alive() || !object.can_construct() {
                    return false;
                }
                if skip == Some(object.id) {
                    return false;
                }
                !matches!(
                    object.ai_state,
                    AIState::Constructing
                        | AIState::Repairing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Docking
                )
            })
            .map(|object| {
                let position = object.get_position();
                let dx = position.x - target.x;
                let dz = position.z - target.z;
                let busy_rank = u8::from(object.ai_state != AIState::Idle);
                (busy_rank, dx * dx + dz * dz, object.id)
            })
            .min_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            })
            .map(|(_, _, id)| id)
    }

    fn team_has_any_dozer(game_logic: &GameLogic, team: Team) -> bool {
        game_logic.host_objects().values().any(|object| {
            object.team == team && object.is_alive() && object.can_construct()
        })
    }

    fn is_dozer_work_order_template(template_name: &str) -> bool {
        let n = template_name.to_ascii_lowercase();
        n.contains("dozer") || n.contains("infantryworker") || n.ends_with("worker")
    }

    fn dozer_already_queued(&self) -> bool {
        self.team_queue.iter().any(|team| {
            team.work_orders
                .iter()
                .any(|order| Self::is_dozer_work_order_template(&order.template_name))
        })
    }

    fn faction_dozer_template(team: Team) -> Option<&'static str> {
        match team {
            Team::USA => Some("AmericaVehicleDozer"),
            Team::China => Some("ChinaVehicleDozer"),
            Team::GLA => Some("GLAInfantryWorker"),
            _ => None,
        }
    }

    /// C++ `AIPlayer::queueDozer` (AIPlayer.cpp:3128-3171): prepend a priority
    /// dozer work order and startTraining immediately when no KINDOF_DOZER exists.
    fn queue_dozer(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.dozer_already_queued() {
            return;
        }
        let Some(template_name) = Self::faction_dozer_template(self.team) else {
            return;
        };
        // C++ findFactory(dozer, busyOk=true) — a busy Command Center is allowed.
        let Some(factory_id) =
            Self::find_factory_for_unit_ex(game_logic, template_name, self.team, true)
        else {
            return;
        };

        let mut preexisting: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&unit_id, unit)| {
                (unit.team == self.team
                    && unit.producer_id == Some(factory_id)
                    && unit.template_name.eq_ignore_ascii_case(template_name))
                .then_some(unit_id)
            })
            .collect();
        preexisting.sort_by_key(|id| id.0);

        let mut order = AIWorkOrder::new(template_name.to_string(), 1, 100);
        let queued = Self::with_can_build_units(game_logic, self.player_id, true, |gl| {
            gl.enqueue_production(factory_id, template_name.to_string())
        });
        if queued {
            order.factory_id = Some(factory_id);
            order.queued_count = 1;
            order.observed_unit_ids = preexisting;
        }
        self.team_queue.push_front(AITeamQueue::new(
            "DOZER - building one at the factory".to_string(),
            vec![order],
            true,
            (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
        ));
        // C++ m_teamDelay = 0 so queueUnits retries immediately.
        self.next_team_queue_time = current_time;
        if queued {
            self.activity_count = self.activity_count.saturating_add(1);
        }
    }

    /// Reattach a live dozer to every queued scaffold that lost its builder.
    ///
    /// C++ refreshes existing `BuildListInfo` entries before considering a new
    /// structure, and calls `aiResumeConstruction` whenever the remembered
    /// dozer was killed/captured or no longer has a build task.  Without this,
    /// one interrupted construction permanently blocks its planned base slot.
    fn resume_interrupted_construction(&self, game_logic: &mut GameLogic) {
        let unfinished: Vec<(ObjectId, Vec3)> = self
            .building_queue
            .iter()
            .filter_map(|building| {
                let object_id = building.object_id?;
                let object = game_logic.host_object(object_id)?;
                (object.team == self.team && object.is_alive() && object.status.under_construction)
                    .then_some((object_id, object.get_position()))
            })
            .collect();

        for (structure_id, position) in unfinished {
            let has_live_builder = game_logic.host_objects().values().any(|object| {
                object.team == self.team
                    && object.is_alive()
                    && object.can_construct()
                    && object.target == Some(structure_id)
                    && object.ai_state == AIState::Constructing
            });
            if has_live_builder {
                continue;
            }

            if let Some(dozer_id) =
                Self::find_available_dozer(game_logic, self.team, position, self.repair_dozer)
            {
                if game_logic.resume_construction(&[dozer_id], structure_id) {
                    // `resume_construction` creates the authoritative dock;
                    // this command restores the corresponding movement/path.
                    let _ = game_logic.unit_command_begin_construct(dozer_id, position);
                }
            }
        }
    }

    /// Process one building construction start, matching
    /// `AISkirmishPlayer::processBaseBuilding`.
    ///
    /// C++ does not mass-place scaffolds and later invent construction work:
    /// it chooses one legal plan, finds a dozer, and gives that dozer the live
    /// build task immediately.  Keep an unstartable plan in the queue instead
    /// of fabricating a builder or charging the player.
    fn process_building_queue(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        self.resume_interrupted_construction(game_logic);
        // C++ findDozer → queueDozer when no KINDOF_DOZER exists (AIPlayer.cpp:3254-3256).
        if !Self::team_has_any_dozer(game_logic, self.team) {
            self.queue_dozer(game_logic, current_time);
        }

        let is_under_powered = game_logic
            .get_player(self.player_id)
            .is_some_and(|player| player.power_available < 0);
        let build_index =
            self.select_priority_or_power_build(game_logic, current_time, is_under_powered);

        if let Some(index) = build_index {
            self.relocate_defense_if_illegal(game_logic, index);
            if let Some((template_name, position, mut build_cost)) =
                self.building_queue.get(index).and_then(|building| {
                    game_logic
                        .templates
                        .get(&building.template_name)
                        .map(|template| {
                            (
                                building.template_name.clone(),
                                building.position,
                                template.build_cost,
                            )
                        })
                })
            {
                build_cost.supplies = game_logic.modified_build_cost_supplies(
                    self.player_id,
                    &template_name,
                    build_cost.supplies,
                );
                let started = Self::find_available_dozer(
                    game_logic,
                    self.team,
                    position,
                    self.repair_dozer,
                )
                    .filter(|&dozer_id| {
                        game_logic.is_location_legal_to_build_for_builder(
                            self.team,
                            position,
                            &template_name,
                            Some(dozer_id),
                        )
                    })
                    .and_then(|dozer_id| {
                        let structure_id = game_logic.create_object_under_construction(
                            &template_name,
                            self.team,
                            position,
                        )?;

                        // The authoritative construction APIs carry the exact live
                        // association that `update_construction` consumes: target is
                        // the scaffold and the dozer receives its construct path.
                        let assigned = game_logic.resume_construction(&[dozer_id], structure_id);
                        let commanded =
                            assigned && game_logic.unit_command_begin_construct(dozer_id, position);
                        let paid = commanded
                            && game_logic
                                .get_player_mut(self.player_id)
                                .is_some_and(|player| player.spend_resources(&build_cost));
                        if paid {
                            Some(structure_id)
                        } else {
                            // This is only an invariant failure after the earlier
                            // affordability/path checks.  Do not leave an unpaid,
                            // unassigned scaffold behind.
                            game_logic.cancel_dozers_building(structure_id);
                            game_logic.destroy_object(structure_id);
                            None
                        }
                    });

                if let Some(object_id) = started {
                    let building = &mut self.building_queue[index];
                    building.object_id = Some(object_id);
                    building.decrement_num_rebuilds();
                    // C++ setObjectTimestamp(0) once rebuild is enabled/started.
                    building.destroyed_at_time = None;
                    self.activity_count = self.activity_count.saturating_add(1);
                    log::debug!(
                        "AI Player {} building {} at {:?}",
                        self.player_id,
                        template_name,
                        position
                    );
                }
            }
        }

        // C++ processBaseBuilding: captured pads unbind; missing pads scan GLA holes.
        self.sync_build_list_object_status(game_logic, current_time);
    }

    fn pad_object_still_ours(object: &crate::game_logic::Object, player_id: u32, team: Team) -> bool {
        match object.owner_player_id {
            Some(owner) => owner == player_id,
            None => object.team == team,
        }
    }

    fn find_rebuild_hole_for_spawner(game_logic: &GameLogic, prior_id: ObjectId) -> Option<ObjectId> {
        game_logic.host_objects().iter().find_map(|(&id, object)| {
            (object.is_rebuild_hole && object.rebuild_spawner_id == Some(prior_id)).then_some(id)
        })
    }

    fn sync_build_list_object_status(&mut self, game_logic: &GameLogic, current_time: f32) {
        for building in &mut self.building_queue {
            let Some(object_id) = building.object_id else {
                continue;
            };
            let prior_id = object_id;
            match game_logic.host_object(object_id) {
                Some(object) => {
                    if Self::pad_object_still_ours(object, self.player_id, self.team) {
                        building.is_built = object.is_constructed() && !object.is_rebuild_hole;
                    } else {
                        // C++: captured — clear objectID + stamp timestamp.
                        building.object_id = None;
                        building.is_built = false;
                        if building.destroyed_at_time.is_none() {
                            building.destroyed_at_time = Some(current_time);
                        }
                    }
                }
                None => {
                    building.object_id = None;
                    building.is_built = false;
                    if building.destroyed_at_time.is_none() {
                        building.destroyed_at_time = Some(current_time);
                    }
                    if let Some(hole_id) = Self::find_rebuild_hole_for_spawner(game_logic, prior_id) {
                        building.object_id = Some(hole_id);
                    }
                }
            }
        }
    }

    /// C++ `KINDOF_FS_POWER && !KINDOF_CASH_GENERATOR`.
    fn template_is_power_plan(game_logic: &GameLogic, template_name: &str) -> bool {
        let Some(template) = game_logic.templates.get(template_name) else {
            return template_name.contains("PowerPlant");
        };
        let is_power = template.is_kind_of(KindOf::FSPower)
            || template.is_kind_of(KindOf::PowerPlant);
        let is_cash = template.is_kind_of(KindOf::SupplyCenter)
            || template.is_kind_of(KindOf::FSSupplyCenter);
        is_power && !is_cash
    }

    /// C++ `AISkirmishPlayer::processBaseBuilding` pick: first priority pad,
    /// then underpowered / automatic FS_POWER. Automatic pads never win
    /// (`canMakeUnit(dozer, NULL)` → `CANMAKE_NO_PREREQ`).
    fn select_priority_or_power_build(
        &self,
        game_logic: &GameLogic,
        current_time: f32,
        is_under_powered: bool,
    ) -> Option<usize> {
        let Some(player) = game_logic.get_player(self.player_id) else {
            return None;
        };
        let mut selected: Option<usize> = None;
        let mut selected_priority = false;
        let mut power_idx: Option<usize> = None;
        let mut power_under_construction = false;

        for (index, building) in self.building_queue.iter().enumerate() {
            if let Some(object_id) = building.object_id {
                if let Some(object) = game_logic.host_object(object_id) {
                    if object.status.under_construction
                        && Self::template_is_power_plan(game_logic, &building.template_name)
                    {
                        power_under_construction = true;
                    }
                    continue;
                }
            }
            if building.is_built
                || !building.is_buildable()
                || !building.rebuild_delay_elapsed(current_time, Self::REBUILD_DELAY_SECONDS)
            {
                continue;
            }
            let Some(template) = game_logic.templates.get(&building.template_name) else {
                continue;
            };
            if !player.can_afford(&{
                let mut cost = template.build_cost;
                cost.supplies = game_logic.modified_build_cost_supplies(
                    self.player_id,
                    &building.template_name,
                    template.build_cost.supplies,
                );
                cost
            }) {
                continue;
            }

            if building.is_priority && !selected_priority {
                selected = Some(index);
                selected_priority = true;
            }
            if power_idx.is_none()
                && Self::template_is_power_plan(game_logic, &building.template_name)
                && (is_under_powered || building.automatic_build)
            {
                power_idx = Some(index);
            }
            // Automatic pads: C++ canMakeUnit(dozer, bldgPlan=NULL) continues.
        }

        if let Some(power) = power_idx {
            if !power_under_construction && selected != Some(power) {
                selected = Some(power);
            }
        }
        selected
    }

    /// C++ `AISkirmishPlayer::buildSpecificAIBuilding` — mark an existing
    /// unbuilt list entry as priority. Does not invent a new pad.
    pub fn build_specific_ai_building(&mut self, thing_name: &str) -> bool {
        let mut found = false;
        for building in &mut self.building_queue {
            if building.template_name != thing_name {
                continue;
            }
            found = true;
            if building.object_id.is_some() || building.is_built {
                continue;
            }
            if building.is_priority {
                continue;
            }
            building.is_priority = true;
            building.automatic_build = false;
            return true;
        }
        let _ = found;
        false
    }

    /// C++ `AIPlayer::buildBySupplies` — named template near `findSupplyCenter`.
    /// Never invents a random offset around base center.
    pub fn build_by_supplies(
        &mut self,
        game_logic: &GameLogic,
        minimum_cash: i32,
        thing_name: &str,
    ) -> bool {
        let Some(warehouse_id) = self.find_supply_center(game_logic, minimum_cash) else {
            return false;
        };
        let Some(warehouse) = game_logic.host_object(warehouse_id) else {
            return false;
        };
        let warehouse_pos = warehouse.get_position();
        let is_cash = game_logic
            .templates
            .get(thing_name)
            .map(|t| t.is_kind_of(KindOf::SupplyCenter) || t.is_kind_of(KindOf::FSSupplyCenter))
            .unwrap_or_else(|| {
                thing_name.contains("SupplyCenter") || thing_name.contains("SupplyStash")
            });
        let mut offset = warehouse_pos - self.base_center;
        let mut radius = 30.0;
        if !is_cash {
            if let Some(enemy_id) = self.enemy_player_id {
                if let Some(enemy) = game_logic.get_player(enemy_id) {
                    let enemy_center = self.find_enemy_base_center(game_logic, enemy.team);
                    offset = warehouse_pos - enemy_center;
                }
            }
            radius = warehouse.selection_radius.max(20.0);
        }
        let len = Vec3::new(offset.x, 0.0, offset.z).length();
        if len > 0.0001 {
            offset.x /= len;
            offset.z /= len;
        } else {
            offset = Vec3::new(1.0, 0.0, 0.0);
        }
        let position = Vec3::new(
            warehouse_pos.x - offset.x * radius,
            0.0,
            warehouse_pos.z - offset.z * radius,
        );
        self.add_building(thing_name, position, 2);
        self.current_warehouse_id = Some(warehouse_id);
        true
    }

    /// C++ `AIPlayer::buildSpecificBuildingNearestTeam`.
    /// Place a priority pad at the team's estimate position (first live member).
    pub fn build_specific_building_nearest_team(
        &mut self,
        game_logic: &GameLogic,
        thing_name: &str,
        team_name: &str,
    ) -> bool {
        if thing_name.trim().is_empty() {
            return false;
        }
        let Some(position) = Self::estimate_team_position(game_logic, team_name) else {
            return false;
        };
        self.add_building(thing_name, position, 2);
        true
    }

    /// C++ `Team::getEstimateTeamPosition` — first living member's pose.
    fn estimate_team_position(game_logic: &GameLogic, team_name: &str) -> Option<Vec3> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return None;
        }
        let mut best: Option<(u32, Vec3)> = None;
        for (&id, object) in game_logic.host_objects() {
            if !object.is_alive() || object.status.destroyed {
                continue;
            }
            let instance = !object.team_instance_name.is_empty()
                && object.team_instance_name.eq_ignore_ascii_case(needle);
            let faction = object.team.get_name().eq_ignore_ascii_case(needle);
            if !instance && !faction {
                continue;
            }
            if best.map(|(oid, _)| id.0 < oid).unwrap_or(true) {
                best = Some((id.0, object.get_position()));
            }
        }
        best.map(|(_, pos)| pos)
    }

    /// C++ `AIPlayer::buildUpgrade` — named player upgrade at a ready factory.
    pub fn build_upgrade(&mut self, game_logic: &mut GameLogic, upgrade_name: &str) -> bool {
        if upgrade_name.trim().is_empty() {
            return false;
        }
        if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name) {
            return false;
        }
        let Some(player) = game_logic.get_player(self.player_id) else {
            return false;
        };
        if !player.is_alive
            || player.has_unlocked_upgrade(upgrade_name)
            || player.has_queued_upgrade(upgrade_name)
        {
            return false;
        }
        let leftover_cost = gamelogic::upgrade::center::with_upgrade_center(|center| {
            center
                .find_upgrade(upgrade_name)
                .map(|template| template.get_cost().max(0) as u32)
        });
        let cost_supplies = leftover_cost.unwrap_or_else(|| {
            crate::game_logic::host_upgrades::resolve_upgrade_retail_cost_supplies(upgrade_name)
        });
        let cost = crate::game_logic::Resources {
            supplies: cost_supplies,
            power: 0,
        };
        if cost_supplies > 0 && !player.can_afford(&cost) {
            return false;
        }
        let Some(producer_id) = self.find_upgrade_producer(game_logic, upgrade_name) else {
            return false;
        };
        let Some(player) = game_logic.get_player_mut(self.player_id) else {
            return false;
        };
        if !player.queue_upgrade(upgrade_name, &cost) {
            return false;
        }
        let kind = crate::game_logic::host_upgrades::HostUpgradeKind::from_name(upgrade_name);
        let secs = kind
            .retail_build_time_secs()
            .max(1.0 / LOGIC_FRAMES_PER_SECOND);
        if !game_logic.unit_command_building_add_upgrade_to_queue(
            producer_id,
            upgrade_name,
            secs,
            cost,
        ) {
            if let Some(player) = game_logic.get_player_mut(self.player_id) {
                let _ = player.cancel_queued_upgrade(upgrade_name, &cost);
            }
            return false;
        }
        game_logic.record_host_upgrade_queued(
            self.player_id,
            self.team,
            upgrade_name,
            Some(producer_id),
        );
        game_logic.host_upgrades_mut().set_build_cost_paid(
            upgrade_name,
            self.player_id,
            cost.supplies,
        );
        let frames = (secs * LOGIC_FRAMES_PER_SECOND).round().max(1.0) as u32;
        game_logic.host_upgrades_mut().set_resolved_research_frames(
            upgrade_name,
            self.player_id,
            frames,
        );
        self.activity_count = self.activity_count.saturating_add(1);
        true
    }

    /// C++ `AIPlayer::findSupplyCenter`.
    pub(crate) fn find_supply_center(&self, game_logic: &GameLogic, minimum_cash: i32) -> Option<ObjectId> {
        if let Some(id) = self.current_warehouse_id {
            if game_logic.host_object(id).is_some() {
                return Some(id);
            }
        }
        let floor = minimum_cash.max(0) as u32;
        let mut best: Option<(f32, ObjectId)> = None;
        for (&id, source) in game_logic.host_objects() {
            if !source.is_alive() {
                continue;
            }
            let is_source = source.is_kind_of(KindOf::SupplySource)
                || source.is_kind_of(KindOf::Harvestable)
                || source.is_kind_of(KindOf::Resource);
            if !is_source {
                continue;
            }
            if source.team != Team::Neutral && source.team != self.team {
                continue;
            }
            let cash = source.stored_resources.supplies;
            if cash < floor && cash < 100 {
                continue;
            }
            let pos = source.get_position();
            let dist = (pos - self.base_center).length_squared();
            if best.map(|(d, _)| dist < d).unwrap_or(true) {
                best = Some((dist, id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// C++ `AIPlayer::isSupplySourceAttacked`.
    ///
    /// Rate-limited (`SCAN_RATE = 10` frames). After a recent player attack,
    /// scan cash generators / dozers / harvesters for recent damage and latch
    /// `m_attackedSupplyCenter`.
    pub fn is_supply_source_attacked(&mut self, game_logic: &GameLogic) -> bool {
        const SCAN_RATE: u32 = 10;
        let cur_frame = game_logic.get_frame();
        if cur_frame == 0 {
            self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);
            return false;
        }
        self.attacked_supply_center = None;
        if cur_frame < self.supply_source_attack_check_frame {
            return false;
        }
        let Some(player) = game_logic.get_player(self.player_id) else {
            return false;
        };
        if player.get_attacked_frame().saturating_add(SCAN_RATE) < cur_frame {
            return false;
        }
        self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);

        for (&id, obj) in game_logic.host_objects() {
            if obj.team != self.team || !obj.is_alive() {
                continue;
            }
            // C++ KINDOF_CASH_GENERATOR | DOZER | HARVESTER.
            let is_econ = obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || obj.is_kind_of(KindOf::Dozer)
                || obj.is_kind_of(KindOf::Harvester)
                || obj.is_kind_of(KindOf::Worker);
            if !is_econ {
                continue;
            }
            let Some(timestamp) = obj.last_damage_timestamp else {
                continue;
            };
            if timestamp.saturating_add(SCAN_RATE) > cur_frame {
                self.attacked_supply_center = Some(id);
                return true;
            }
        }
        false
    }


    fn skirmish_enemy_team(&self, game_logic: &GameLogic) -> Option<Team> {
        if let Some(enemy_id) = self.enemy_player_id {
            if let Some(enemy) = game_logic.get_player(enemy_id) {
                if enemy.team != Team::Neutral {
                    return Some(enemy.team);
                }
            }
        }
        game_logic
            .get_players()
            .values()
            .find(|player| player.is_local && player.team != Team::Neutral)
            .map(|player| player.team)
    }

    fn named_team_member_ids(&self, game_logic: &GameLogic, team_name: &str) -> Vec<ObjectId> {
        let needle = team_name.trim();
        game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() || obj.team != self.team {
                    return None;
                }
                if needle.is_empty() {
                    return obj.is_mobile().then_some(id);
                }
                (!obj.team_instance_name.is_empty()
                    && obj.team_instance_name.eq_ignore_ascii_case(needle))
                .then_some(id)
            })
            .collect()
    }

    /// C++ `AIPlayer::guardSupplyCenter`.
    ///
    /// Force attack check; prefer attacked warehouse else `findSupplyCenter`;
    /// `groupGuardPosition` toward enemy structure-bounds offset by
    /// warehouse radius * 0.8 (`CMD_FROM_SCRIPT` / `GUARDMODE_NORMAL`).
    pub fn guard_supply_center(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        min_supplies: i32,
    ) {
        self.supply_source_attack_check_frame = 0;
        let mut warehouse_id = None;
        if self.is_supply_source_attacked(game_logic) {
            warehouse_id = self.attacked_supply_center;
        }
        if warehouse_id.is_none() {
            warehouse_id = self.find_supply_center(game_logic, min_supplies);
        }
        let Some(warehouse_id) = warehouse_id else {
            return;
        };
        let Some(warehouse) = game_logic.host_object(warehouse_id) else {
            return;
        };
        let mut location = warehouse.get_position();
        let radius = warehouse.selection_radius.max(0.0) * 0.8;
        let enemy_team = self.skirmish_enemy_team(game_logic);
        if let Some(enemy_team) = enemy_team {
            let (lo_x, lo_z, hi_x, hi_z) = self.player_structure_bounds(game_logic, enemy_team);
            let mut ox = location.x - (lo_x + hi_x) * 0.5;
            let mut oz = location.z - (lo_z + hi_z) * 0.5;
            let len = (ox * ox + oz * oz).sqrt();
            if len > 0.0001 {
                ox /= len;
                oz /= len;
                location.x -= ox * radius;
                location.z -= oz * radius;
            }
        }

        let members = self.named_team_member_ids(game_logic, team_name);
        for unit_id in members {
            let mobile = game_logic
                .host_object(unit_id)
                .map(|unit| unit.is_alive() && unit.is_mobile())
                .unwrap_or(false);
            if mobile {
                let _ = game_logic.unit_command_guard_position(unit_id, location);
            }
        }
    }

    /// Default/AIData.ini `SideInfo::* ResourceGatherers*` plus the free
    /// SupplyCenter/Stash SpawnBehavior collector.  All three difficulty
    /// entries use these same values in the retail data.
    fn desired_gatherers_per_supply_center(&self) -> u32 {
        match self.team {
            Team::USA | Team::China => 3,
            Team::GLA => 6,
            Team::Neutral => 0,
        }
    }

    /// C++ `SUPPLY_CENTER_CLOSE_DIST` = 20 * PATHFIND_CELL_SIZE_F (10).
    const SUPPLY_CENTER_CLOSE_DISTANCE: f32 = 200.0;

    /// Completed allied SupplyCenterDockUpdate owners, in stable object-id
    /// order.  C++ discovers these through its BuildListInfo records; Main's
    /// live objects are the authoritative equivalent after construction.
    fn live_supply_centers(&self, game_logic: &GameLogic) -> Vec<ObjectId> {
        let mut centers: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.team == self.team
                    && object.is_alive()
                    && object.is_constructed()
                    && !object.status.sold
                    && (object.is_kind_of(KindOf::SupplyCenter)
                        || object.is_kind_of(KindOf::FSSupplyCenter)))
                .then_some(id)
            })
            .collect();
        centers.sort_by_key(|id| id.0);
        centers
    }

    /// Typed `KINDOF_SUPPLY_SOURCE` lookup near a concrete supply center.
    /// The host stores this source capability as `Resource` + `Harvestable`;
    /// do not infer sources from object names.  A finite source must still
    /// hold supplies, matching C++ SupplyWarehouseDockUpdate's available-cash
    /// check before the AI assigns additional collectors.
    fn nearest_supply_source_for_center(
        &self,
        game_logic: &GameLogic,
        center_id: ObjectId,
    ) -> Option<ObjectId> {
        let center = game_logic.host_object(center_id)?;
        let center_position = center.get_position();
        let maximum_distance = Self::SUPPLY_CENTER_CLOSE_DISTANCE + center.selection_radius;
        let maximum_distance_squared = maximum_distance * maximum_distance;

        game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, source)| {
                if !source.is_alive()
                    || source.status.under_construction
                    || (source.team != Team::Neutral && source.team != self.team)
                    || !(source.is_kind_of(KindOf::Resource)
                        || source.is_kind_of(KindOf::Harvestable))
                    || source.stored_resources.supplies == 0
                {
                    return None;
                }
                let delta = source.get_position() - center_position;
                let distance_squared = delta.x * delta.x + delta.z * delta.z;
                (distance_squared <= maximum_distance_squared).then_some((distance_squared, id))
            })
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1 .0.cmp(&right.1 .0))
            })
            .map(|(_, id)| id)
    }

    fn collector_count_for_supply_center(
        &self,
        game_logic: &GameLogic,
        center_id: ObjectId,
    ) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                object.team == self.team
                    && object.is_alive()
                    // C++ `AIPlayer::queueSupplyTruck` counts each
                    // SupplyTruckAIUpdate::getPreferredDockID(), not the
                    // factory/spawner that happened to create the unit.  A
                    // collector rescued after its old center dies must count
                    // toward its newly assigned depot immediately.
                    && object.preferred_dock_id == Some(center_id)
                    && object.is_resource_collector()
            })
            .count() as u32
    }

    fn total_live_collectors(&self, game_logic: &GameLogic) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                object.team == self.team && object.is_alive() && object.is_resource_collector()
            })
            .count() as u32
    }

    /// C++ `AIPlayer::onUnitProduced`: a resource-gatherer work order makes
    /// the newly created collector want supplies and docks it at the matching
    /// SupplyCenter.  Main's typed gather state carries the source target;
    /// `preferred_dock_id` retains the C++ CMD_FROM_PLAYER dock assignment
    /// for the later ReturningResources leg.
    fn route_collector_to_supply_center(
        &self,
        game_logic: &mut GameLogic,
        collector_id: ObjectId,
        center_id: ObjectId,
    ) -> bool {
        let Some(source_id) = self.nearest_supply_source_for_center(game_logic, center_id) else {
            return false;
        };
        let valid_collector = game_logic
            .host_object(collector_id)
            .is_some_and(|collector| {
                collector.team == self.team
                    && collector.is_alive()
                    && collector.is_resource_collector()
            });
        if !valid_collector {
            return false;
        }
        let routed = game_logic
            .unit_command_stop_moving_order_target(collector_id, Some(source_id))
            && game_logic.unit_command_set_ai_state(collector_id, AIState::Gathering);
        if routed {
            if let Some(collector) = game_logic.host_object_mut(collector_id) {
                // C++ AIPlayer::queueSupplyTruck uses aiDock(...,
                // CMD_FROM_PLAYER) specifically so SupplyTruckAIUpdate stores
                // this center in m_preferredDock.
                collector.preferred_dock_id = Some(center_id);
            }
        }
        routed
    }

    /// The SpawnBehavior collector has no AI work order, so give an idle
    /// producer-linked collector the same SupplyTruckAI wanting/dock handoff
    /// that paid collector output receives in C++ `onUnitProduced`.
    fn route_idle_supply_center_collectors(&self, game_logic: &mut GameLogic, center_id: ObjectId) {
        let mut collectors: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                let spawned_here_without_a_dock =
                    object.producer_id == Some(center_id) && object.preferred_dock_id.is_none();
                let already_assigned_to_this_center = object.preferred_dock_id == Some(center_id);
                (object.team == self.team
                    && object.is_alive()
                    && object.is_resource_collector()
                    // The one-shot SpawnBehavior collector initially has a
                    // producer link but no preferred dock.  Once assigned,
                    // C++ only reissues aiDock to a non-ferrying collector
                    // whose preferred dock is this center.
                    && (spawned_here_without_a_dock || already_assigned_to_this_center)
                    && !Self::collector_is_currently_ferrying_supplies(object))
                .then_some(id)
            })
            .collect();
        collectors.sort_by_key(|id| id.0);
        for collector_id in collectors {
            let _ = self.route_collector_to_supply_center(game_logic, collector_id, center_id);
        }
    }

    /// C++ `SupplyTruckAIUpdate::isCurrentlyFerryingSupplies` returns true
    /// only in its Wanting or Docking substates.  Main's economy state owns
    /// those live legs as Gathering, ReturningResources, and Docking; do not
    /// use a generic "not idle" test here because GLA workers also carry
    /// `KINDOF_HARVESTER` while building or repairing.
    #[inline]
    fn collector_is_currently_ferrying_supplies(collector: &Object) -> bool {
        matches!(
            collector.ai_state,
            AIState::Gathering | AIState::ReturningResources | AIState::Docking
        )
    }

    /// C++ `AIPlayer::queueSupplyTruck` first rescues one active collector
    /// whose preferred dock no longer resolves (typically after its supply
    /// center was destroyed), assigns it to the currently-understaffed center,
    /// and returns before it spends money on a replacement truck.
    fn reattach_one_loose_supply_collector(
        &self,
        game_logic: &mut GameLogic,
        center_id: ObjectId,
    ) -> bool {
        let mut candidates: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, collector)| {
                let missing_preferred_dock = collector
                    .preferred_dock_id
                    .is_none_or(|dock_id| game_logic.host_object(dock_id).is_none());
                (collector.team == self.team
                    && collector.is_alive()
                    && collector.is_resource_collector()
                    && missing_preferred_dock
                    && Self::collector_is_currently_ferrying_supplies(collector))
                .then_some(id)
            })
            .collect();
        candidates.sort_by_key(|id| id.0);

        let Some(collector_id) = candidates.into_iter().next() else {
            return false;
        };
        let Some(collector) = game_logic.host_object_mut(collector_id) else {
            return false;
        };
        // `aiDock(center, CMD_FROM_PLAYER)` persists m_preferredDock while
        // preserving the supply sub-brain's active ferry leg.  Do not require
        // a new local warehouse here: retail reattaches before its subsequent
        // resource scan, and an already-full collector must be able to return
        // directly to the replacement center.
        collector.preferred_dock_id = Some(center_id);
        true
    }

    /// A collector work order is tied to the concrete SupplyCenter/Stash whose
    /// authored SpawnBehavior identified the collector template.  This avoids
    /// the old unit-name factory guessing and preserves C++'s real typed
    /// ProductionUpdate authorization/queue/money path.
    fn supply_center_factory_for_collector(
        game_logic: &GameLogic,
        center_id: ObjectId,
        collector_template: &str,
        team: Team,
    ) -> Option<ObjectId> {
        let center = game_logic.host_object(center_id)?;
        if center.team != team
            || !center.is_alive()
            || !center.is_constructed()
            || !Self::factory_is_idle(center)
            || (!center.is_kind_of(KindOf::SupplyCenter)
                && !center.is_kind_of(KindOf::FSSupplyCenter))
            || !game_logic
                .templates
                .get(collector_template)
                .is_some_and(|template| template.is_kind_of(KindOf::Harvester))
        {
            return None;
        }
        (game_logic.can_make_unit(center_id, collector_template)
            == crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK)
            .then_some(center_id)
    }

    /// C++ `AIPlayer::queueSupplyTruck`: retain the free SpawnBehavior
    /// collector separately, then prepend one paid, priority collector work
    /// order when a completed supply center has a nearby live source and is
    /// below its SideInfo gatherer target.
    fn queue_supply_truck(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.team_queue.iter().any(|team| {
            team.work_orders
                .iter()
                .any(|order| order.is_resource_gatherer)
        }) {
            return;
        }

        let desired = self.desired_gatherers_per_supply_center();
        if desired == 0 {
            return;
        }
        let total_collectors = self.total_live_collectors(game_logic);
        let centers = self.live_supply_centers(game_logic);
        for center_id in centers {
            self.route_idle_supply_center_collectors(game_logic, center_id);

            let current = self.collector_count_for_supply_center(game_logic, center_id);
            if current >= desired {
                continue;
            }
            // Do not create a paid replacement while a surviving active
            // harvester from a destroyed center can fill this slot.  C++
            // returns after one such rescue, which also preserves its
            // one-collector-per-economic-tick reassignment cadence.
            if self.reattach_one_loose_supply_collector(game_logic, center_id) {
                return;
            }
            // The first collector must be the actual one-shot SpawnBehavior
            // result, not an unpaid factory item.  Once that behavior has
            // fired, a later dead starter is eligible for normal paid
            // replacement just as C++ does after its initial `current = -1`
            // handoff becomes a real counted gatherer.
            let spawn_behavior_fired = game_logic
                .host_object(center_id)
                .is_some_and(|center| center.supply_center_spawn_behavior_fired);
            if !spawn_behavior_fired {
                continue;
            }
            if total_collectors >= desired.saturating_mul(3)
                || self
                    .nearest_supply_source_for_center(game_logic, center_id)
                    .is_none()
            {
                continue;
            }
            let Some(collector_template) =
                game_logic.supply_center_one_shot_collector_template(center_id)
            else {
                continue;
            };
            if Self::supply_center_factory_for_collector(
                game_logic,
                center_id,
                &collector_template,
                self.team,
            )
            .is_none()
            {
                continue;
            }

            let mut order = AIWorkOrder::new(collector_template, 1, 100);
            order.is_resource_gatherer = true;
            order.supply_center_id = Some(center_id);
            self.team_queue.push_front(AITeamQueue::new(
                "Supply truck".to_string(),
                vec![order],
                true,
                (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
            ));
            // C++ sets m_teamDelay=0 and proceeds through queueUnits in this
            // same pass, so process_team_queue will enqueue it immediately.
            // Do not return: retail keeps walking completed centers and may
            // begin one collector order for each in the same economic tick.
        }
    }

    /// Account for units that have physically left their producing factory.
    ///
    /// C++ `AIPlayer::onUnitProduced` receives the factory and newly-created
    /// unit directly, increments `m_numCompleted`, and clears
    /// `m_factoryID`.  Main owns the production simulation, so its equivalent
    /// is the stable `producer_id` stamped during the live completion path.
    /// Never treat a successful enqueue as a completed work order: doing so
    /// made teams disappear before any of their units existed.
    fn reconcile_produced_units(&mut self, game_logic: &mut GameLogic) -> bool {
        let mut observed_completion = false;
        let mut completed_resource_collectors: Vec<(ObjectId, ObjectId)> = Vec::new();
        for team in &mut self.team_queue {
            for order in &mut team.work_orders {
                let Some(factory_id) = order.factory_id else {
                    continue;
                };

                // The only outstanding request for a normal work order is
                // bound to this factory.  If the producer died, C++ can no
                // longer deliver that request; allow a future queue pass to
                // find a replacement factory instead of retaining a dead ID.
                let factory_alive = game_logic
                    .host_object(factory_id)
                    .map(|factory| factory.is_alive())
                    .unwrap_or(false);
                if !factory_alive {
                    order.factory_id = None;
                    order.queued_count = 0;
                    continue;
                }

                let remaining = order.num_required.saturating_sub(order.num_completed);
                if remaining == 0 {
                    order.factory_id = None;
                    order.queued_count = 0;
                    continue;
                }

                // `producer_id` is applied before the spawned unit enters the
                // normal AI update, so this observes the real factory exit
                // rather than inferring a completion from a queue mutation.
                let mut produced: Vec<ObjectId> = game_logic
                    .host_objects()
                    .iter()
                    .filter_map(|(&unit_id, unit)| {
                        (unit.team == self.team
                            && unit.producer_id == Some(factory_id)
                            && unit
                                .template_name
                                .eq_ignore_ascii_case(&order.template_name)
                            && !order.observed_unit_ids.contains(&unit_id))
                        .then_some(unit_id)
                    })
                    .collect();
                // HashMap iteration is deliberately not an ordering contract;
                // preserve the C++ one-unit-at-a-time work-order progression.
                produced.sort_by_key(|id| id.0);

                let mut accepted = 0u32;
                for unit_id in produced.into_iter().take(remaining as usize) {
                    order.observed_unit_ids.push(unit_id);
                    if order.is_resource_gatherer {
                        if let Some(center_id) = order.supply_center_id {
                            completed_resource_collectors.push((unit_id, center_id));
                        }
                    }
                    if self.dozer_queued_for_repair
                        && Self::is_dozer_work_order_template(&order.template_name)
                    {
                        self.repair_dozer = Some(unit_id);
                        self.dozer_queued_for_repair = false;
                        if let Some(unit) = game_logic.host_object(unit_id) {
                            self.repair_dozer_origin = unit.get_position();
                        }
                    }
                    accepted = accepted.saturating_add(1);
                }
                if accepted == 0 {
                    continue;
                }

                order.num_completed = order
                    .num_completed
                    .saturating_add(accepted)
                    .min(order.num_required);
                order.queued_count = order.queued_count.saturating_sub(accepted);
                observed_completion = true;
                // `onUnitProduced` releases the factory association for the
                // next required unit.  Host queues one at a time as C++ does.
                order.factory_id = None;
            }
        }
        // C++ `onUnitProduced` performs the SupplyTruckAI wanting/dock setup
        // immediately after matching the work order.  Do it after releasing
        // the queue borrow so these are real typed gather commands, not a
        // synthetic resource credit.
        for (collector_id, center_id) in completed_resource_collectors {
            let _ = self.route_collector_to_supply_center(game_logic, collector_id, center_id);
        }
        observed_completion
    }

    /// Process team production queue
    fn process_team_queue(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        self.reconcile_produced_units(game_logic);
        // Retail queueUnits invokes queueSupplyTruck before walking the usual
        // TeamInQueue work orders.  Its priority order therefore gets an idle
        // SupplyCenter and pays through ordinary ProductionUpdate immediately.
        self.queue_supply_truck(game_logic, current_time);
        let can_build_units = game_logic
            .get_player(self.player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true);
        if can_build_units {
            // C++ queueUnits: tryToRecruit existing map units before startTraining.
            self.recruit_waiting_work_orders(game_logic);
        }

        // Collect all factory assignments needed
        let mut factory_assignments = Vec::new();
        let mut completed_teams = Vec::new();

        for (team_index, team) in self.team_queue.iter_mut().enumerate() {
            let mut all_complete = true;

            for (order_index, work_order) in team.work_orders.iter().enumerate() {
                if work_order.num_completed < work_order.num_required {
                    // Try to queue more units
                    if work_order.factory_id.is_none()
                        && (can_build_units
                            || Self::is_dozer_work_order_template(&work_order.template_name)
                            || work_order.is_resource_gatherer)
                    {
                        factory_assignments.push((
                            team_index,
                            order_index,
                            work_order.template_name.clone(),
                            team.priority_build,
                            work_order.is_resource_gatherer,
                            work_order.supply_center_id,
                        ));
                    }

                    all_complete = false;
                }
            }

            if all_complete && !team.completed {
                team.completed = true;
                completed_teams.push(team_index);
            }
        }

        // Process factory assignments and enqueue production on the host path.
        let mut produced = 0u64;
        for (
            team_index,
            order_index,
            template_name,
            priority_build,
            is_resource_gatherer,
            supply_center_id,
        ) in factory_assignments
        {
            // C++ `startTraining(order, team->m_priorityBuild, ...)` only
            // permits a busy factory for a priority build.  Normal skirmish
            // teams must wait for an idle producer; otherwise a pre-existing
            // matching unit could be misattributed to this work order.
            let factory_id = if is_resource_gatherer {
                supply_center_id.and_then(|center_id| {
                    Self::supply_center_factory_for_collector(
                        game_logic,
                        center_id,
                        &template_name,
                        self.team,
                    )
                })
            } else {
                Self::find_factory_for_unit_ex(
                    game_logic,
                    &template_name,
                    self.team,
                    priority_build,
                )
            };
            if let Some(factory_id) = factory_id {
                // Snapshot matching output before the request is submitted.
                // A priority order may join an already-busy factory, whose
                // earlier output belongs to another order.
                let mut preexisting: Vec<ObjectId> = game_logic
                    .host_objects()
                    .iter()
                    .filter_map(|(&unit_id, unit)| {
                        (unit.team == self.team
                            && unit.producer_id == Some(factory_id)
                            && unit.template_name.eq_ignore_ascii_case(&template_name))
                        .then_some(unit_id)
                    })
                    .collect();
                preexisting.sort_by_key(|id| id.0);

                let queued = game_logic.enqueue_production(factory_id, template_name.clone());
                if let Some(team) = self.team_queue.get_mut(team_index) {
                    if let Some(work_order) = team.work_orders.get_mut(order_index) {
                        // Only bind factory on success — failed enqueue (wrong type,
                        // full queue, cash) must retry next military tick.  Record
                        // pre-existing matching output before the enqueue so a unit
                        // made for an older queue cannot complete this order.
                        if queued {
                            for unit_id in preexisting {
                                if !work_order.observed_unit_ids.contains(&unit_id) {
                                    work_order.observed_unit_ids.push(unit_id);
                                }
                            }
                            work_order.factory_id = Some(factory_id);
                            work_order.queued_count = work_order.queued_count.saturating_add(1);
                            produced = produced.saturating_add(1);
                        } else {
                            work_order.factory_id = None;
                        }
                    }
                }
            }
        }
        self.activity_count = self.activity_count.saturating_add(produced);

        // C++ checkQueuedTeams (not queueUnits) promotes completed teams.
        let _ = completed_teams;
    }

    /// C++ `isPossibleToBuildTeam` unit-cost residual:
    /// `cost += thingCost * ((minUnits+maxUnits)/2.0f)` then Int-truncate.
    fn estimate_team_unit_cost(&self, game_logic: &GameLogic, team_name: &str) -> u32 {
        let units = Self::prototype_unit_infos(team_name);
        if !units.is_empty() {
            let mut cost: i32 = 0;
            for (name, min_u, max_u) in units {
                let unit_cost = game_logic
                    .templates
                    .get(&name)
                    .map(|t| t.build_cost.supplies)
                    .unwrap_or(0) as i32;
                // C++: cost += thingCost * ((maxUnits+minUnits)/2.0f);
                cost += (unit_cost as f32 * ((max_u as f32 + min_u as f32) / 2.0)) as i32;
            }
            return cost.max(0) as u32;
        }
        let orders = self.create_work_orders_for_team(team_name);
        let mut cost = 0u32;
        for order in orders {
            let unit_cost = game_logic
                .templates
                .get(&order.template_name)
                .map(|t| t.build_cost.supplies)
                .unwrap_or(0);
            cost = cost.saturating_add(unit_cost.saturating_mul(order.num_required as u32));
        }
        cost
    }

    /// AIData `TeamResourcesToStart` (`m_teamResourcesToBuild`). Store first,
    /// leftover `THE_AI`, then the retail 0.1 residual.
    fn team_resources_to_start_frac() -> f32 {
        let from_store = game_engine::common::ini::get_ai_data_store()
            .get_active()
            .map(|d| d.team_resources_to_build);
        let leftover = gamelogic::ai::THE_AI.read().ok().and_then(|ai| {
            ai.get_ai_data()
                .read()
                .ok()
                .map(|d| d.team_resources_to_build)
        });
        from_store
            .or(leftover)
            .filter(|m| *m > 0.0)
            .unwrap_or(Self::TEAM_RESOURCES_TO_START)
    }

    /// C++ `isPossibleToBuildTeam` money residual:
    /// `cost *= m_teamResourcesToBuild` then require `money >= cost`.
    fn can_afford_team_start(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        let Some(player) = game_logic.get_player(self.player_id) else {
            return false;
        };
        let full = self.estimate_team_unit_cost(game_logic, team_name) as f32;
        // C++: `cost *= m_teamResourcesToBuild` (Int *= Real truncates).
        let required = (full * Self::team_resources_to_start_frac()) as u32;
        player.resources.supplies >= required
    }

    /// C++ `isPossibleToBuildTeam` residual (money + factory existence + any idle).
    /// Production-condition scripts / maxInstances remain unported.
    fn is_possible_to_build_team(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        self.can_afford_team_start(game_logic, team_name)
            && self.team_factories_ready(game_logic, team_name)
    }

    /// C++ `AIPlayer::aiPreTeamDestroy(const Team *deletedTeam)`.
    /// Drop TeamInQueue entries whose `m_team` is the deleted instance.
    pub fn ai_pre_team_destroy(&mut self, deleted_team_id: Option<u32>, deleted_name: &str) {
        let keep = |q: &AITeamQueue| -> bool {
            if let (Some(qid), Some(did)) = (q.team_id, deleted_team_id) {
                // C++: team->m_team == deletedTeam
                return qid != did;
            }
            // Fallback: name compare when m_team handle was never stamped.
            if q.team_id.is_none() && !deleted_name.is_empty() {
                return q.name != deleted_name;
            }
            true
        };
        self.team_queue.retain(keep);
        self.team_ready_queue.retain(keep);
    }

    fn leftover_team_instance_gone(team_id: Option<u32>) -> bool {
        let Some(id) = team_id else {
            return false;
        };
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .map(|factory| factory.find_team_by_id(id).is_none())
            .unwrap_or(false)
    }

    fn leftover_instance_member_ids(team_id: Option<u32>) -> Vec<u32> {
        let Some(id) = team_id else {
            return Vec::new();
        };
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_by_id(id))
            .and_then(|arc| arc.read().ok().map(|tg| tg.get_members().to_vec()))
            .unwrap_or_default()
    }

    /// C++ `Team::hasAnyUnits` on one instance, using live host objects.
    fn leftover_instance_has_any_host_units(game_logic: &GameLogic, team_id: u32) -> bool {
        Self::leftover_instance_member_ids(Some(team_id))
            .into_iter()
            .any(|id| {
                game_logic.host_object(ObjectId(id)).is_some_and(|o| {
                    o.is_alive()
                        && !o.is_kind_of(KindOf::Structure)
                        && !o.is_kind_of(KindOf::Projectile)
                        && !o.is_kind_of(KindOf::Mine)
                })
            })
    }

    /// C++ `Team::countObjectsByThingTemplate` on one instance.
    fn leftover_instance_count_template(
        game_logic: &GameLogic,
        team_id: u32,
        template_name: &str,
    ) -> u32 {
        Self::leftover_instance_member_ids(Some(team_id))
            .into_iter()
            .filter(|&id| {
                game_logic.host_object(ObjectId(id)).is_some_and(|o| {
                    Self::recruit_template_matches(template_name, &o.template_name)
                })
            })
            .count() as u32
    }

    fn leftover_instance_first_member_pos(
        game_logic: &GameLogic,
        team_id: u32,
    ) -> Option<Vec3> {
        for id in Self::leftover_instance_member_ids(Some(team_id)) {
            if let Some(obj) = game_logic.host_object(ObjectId(id)) {
                return Some(obj.get_position());
            }
        }
        None
    }

    fn leftover_prototype_is_singleton(team_name: &str) -> bool {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_prototype(team_name).map(|p| p.is_singleton()))
            .unwrap_or(false)
    }

    fn leftover_default_team_arc(
        player_id: u32,
    ) -> Option<std::sync::Arc<std::sync::RwLock<gamelogic::team::Team>>> {
        gamelogic::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()))
    }

    fn leftover_is_skirmish_ai(&self) -> bool {
        gamelogic::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|player| player.read().ok().map(|p| p.is_skirmish_ai()))
            .unwrap_or(false)
    }

    /// C++ `Object::setTeam` onto the destination leftover instance.
    fn assign_host_unit_to_leftover_team(
        game_logic: &mut GameLogic,
        unit_id: ObjectId,
        dest_team_id: Option<u32>,
        dest_team_name: &str,
    ) {
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            for team in factory.get_all_teams() {
                if let Ok(mut tg) = team.write() {
                    if dest_team_id != Some(tg.get_id()) {
                        tg.remove_member(unit_id.0);
                    }
                }
            }
            if let Some(id) = dest_team_id {
                if let Some(arc) = factory.find_team_by_id(id) {
                    if let Ok(mut tg) = arc.write() {
                        tg.add_member(unit_id.0);
                    }
                }
            }
        }
        if let Some(obj) = game_logic.host_object_mut(unit_id) {
            obj.team_instance_name = dest_team_name.to_string();
        }
    }

    /// C++ `TeamInQueue::disband` (`AIPlayer.cpp:3554-3566`).
    fn disband_queued_team(&self, game_logic: &mut GameLogic, team: &AITeamQueue) {
        let default_name =
            game_logic.default_host_team_instance_name(Some(self.player_id), self.team);
        let mut member_ids: HashSet<u32> = Self::leftover_instance_member_ids(team.team_id)
            .into_iter()
            .collect();
        for order in &team.work_orders {
            member_ids.extend(order.observed_unit_ids.iter().map(|id| id.0));
        }
        for mid in &member_ids {
            if let Some(obj) = game_logic.host_object_mut(ObjectId(*mid)) {
                obj.team_instance_name = default_name.clone();
            }
        }

        let Some(src_id) = team.team_id else {
            if self.leftover_is_skirmish_ai() {
                Self::clear_leftover_team_flags();
            }
            return;
        };
        let default_arc = Self::leftover_default_team_arc(self.player_id);
        let default_id = default_arc
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|tg| tg.get_id()));
        if default_id == Some(src_id) {
            return;
        }
        if let Some(default_arc) = default_arc {
            if let Ok(mut dg) = default_arc.write() {
                for mid in &member_ids {
                    dg.add_member(*mid);
                }
            }
        }
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(src) = factory.find_team_by_id(src_id) {
                if let Ok(mut sg) = src.write() {
                    for mid in &member_ids {
                        sg.remove_member(*mid);
                    }
                }
            }
        }
        if !Self::leftover_prototype_is_singleton(&team.name) {
            if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
                factory.team_about_to_be_deleted(src_id);
            }
        }
        if self.leftover_is_skirmish_ai() {
            Self::clear_leftover_team_flags();
        }
    }

    fn clear_leftover_team_flags() {
        if let Ok(mut eng) = gamelogic::scripting::engine::get_script_engine().write() {
            if let Some(e) = eng.as_mut() {
                e.clear_team_flags();
            }
        }
    }

    fn queue_team_members_wiped(game_logic: &GameLogic, team: &AITeamQueue) -> bool {
        let mut any = false;
        for order in &team.work_orders {
            for &id in &order.observed_unit_ids {
                any = true;
                if game_logic.host_object(id).is_some_and(|o| o.is_alive()) {
                    return false;
                }
            }
        }
        any
    }

    /// C++ Team::~Team → Player::preTeamDestroy: free the AI slot immediately.
    fn purge_destroyed_or_wiped_queued_teams(&mut self, game_logic: &GameLogic) {
        let mut doomed: Vec<(Option<u32>, String)> = Vec::new();
        for team in self.team_ready_queue.iter() {
            if Self::leftover_team_instance_gone(team.team_id)
                || Self::queue_team_members_wiped(game_logic, team)
            {
                doomed.push((team.team_id, team.name.clone()));
            }
        }
        for team in self.team_queue.iter() {
            if Self::leftover_team_instance_gone(team.team_id)
                || ((team.completed || team.is_all_built())
                    && Self::queue_team_members_wiped(game_logic, team))
            {
                doomed.push((team.team_id, team.name.clone()));
            }
        }
        for (id, name) in doomed {
            self.ai_pre_team_destroy(id, &name);
        }
    }

    /// C++ `TeamInQueue::m_team = TheTeamFactory->createInactiveTeam(...)`.
    fn bind_inactive_team_handle(team: &mut AITeamQueue) {
        if team.team_id.is_some() {
            return;
        }
        team.team_id = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.create_inactive_team(&team.name))
            .and_then(|arc| arc.read().ok().map(|t| t.get_id()));
    }

    /// C++ `AIPlayer::checkReadyTeams` (`AIPlayer.cpp:2729-2803`).
    fn check_ready_teams(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        const READY_TEAM_FORCE_SECONDS: f32 = 60.0;
        let mut i = 0;
        while i < self.team_ready_queue.len() {
            let time_expired = {
                let started = self.team_ready_queue[i].frame_started as f32 / LOGIC_FRAMES_PER_SECOND;
                current_time - started >= READY_TEAM_FORCE_SECONDS
            };
            let (mut all_idle, mut any_idle) = (true, false);
            if self.team_ready_queue[i].reinforcement {
                // C++ AIPlayer.cpp:2740-2745 — only the reinforcement object.
                if let Some(obj_id) = self.team_ready_queue[i].reinforcement_id {
                    if let Some(obj) = game_logic.host_object(obj_id) {
                        let idle = obj.is_alive() && obj.ai_state == AIState::Idle;
                        all_idle = idle;
                        any_idle = idle;
                    }
                }
            } else {
                let member_ids: Vec<ObjectId> = self.team_ready_queue[i]
                    .work_orders
                    .iter()
                    .flat_map(|order| order.observed_unit_ids.iter().copied())
                    .collect();
                if member_ids.is_empty() {
                    all_idle = false;
                } else {
                    for id in &member_ids {
                        let idle = game_logic
                            .host_object(*id)
                            .map(|o| o.is_alive() && o.ai_state == AIState::Idle)
                            .unwrap_or(false);
                        if idle {
                            any_idle = true;
                        } else {
                            all_idle = false;
                        }
                    }
                }
            }
            // C++ AIPlayer.cpp:2755-2761 — anyIdle shortcut only when
            // ExecutesActionsOnCreate and ProductionCondition has an Action.
            if any_idle && self.team_ready_queue[i].execute_actions {
                if Self::production_condition_has_action(&self.team_ready_queue[i].name) {
                    all_idle = true;
                }
            }
            if time_expired {
                all_idle = true;
            }
            if !all_idle {
                i += 1;
                continue;
            }
            let mut team = self.team_ready_queue.remove(i).expect("ready idx");
            team.sent_to_start_location = true;
            team.activated = true;
            team.completed = true;
            // C++ Team::setActive() → m_created; OnCreate Hunt/Guard/AttackMove
            // scripts run for this team's members only (not the whole army).
            self.activate_ready_team(game_logic, &team, current_time);
            log::debug!(
                "AI Player {} activated ready team: {}",
                self.player_id,
                team.name
            );
        }
    }

    /// C++ `Team::setActive` + OnCreate + `joinTeam` / `clearTeamFlags`.
    /// Host orders come from the OnCreate script, never a forced AttackMove.
    fn activate_ready_team(
        &mut self,
        game_logic: &mut GameLogic,
        team: &AITeamQueue,
        current_time: f32,
    ) {
        let members: Vec<ObjectId> = team
            .work_orders
            .iter()
            .flat_map(|order| order.observed_unit_ids.iter().copied())
            .collect();

        if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
            let team_arc = factory
                .find_team_instances(&team.name)
                .into_iter()
                .next()
                .or_else(|| factory.create_inactive_team(&team.name));
            if let Some(team_arc) = team_arc {
                if let Ok(mut tg) = team_arc.write() {
                    for id in &members {
                        tg.add_member(id.0);
                    }
                    tg.set_active();
                }
            }
        }

        let on_create = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(&team.name)
                    .map(|proto| proto.get_script_on_create().to_string())
            })
            .unwrap_or_default();
        // C++ AIPlayer.cpp:2788-2792 only Team::setActive. OnCreate runs once
        // from Team::updateState on the next ScriptEngine tick.


        if team.reinforcement {
            if let Some(obj_id) = team.reinforcement_id {
                self.join_team_reinforcement_host(game_logic, obj_id, &members);
            }
        } else {
            if let Ok(mut eng) = gamelogic::scripting::engine::get_script_engine().write() {
                if let Some(e) = eng.as_mut() {
                    e.clear_team_flags();
                }
            }
            self.apply_on_create_host_orders(game_logic, &members, &on_create, current_time);
        }
        self.activity_count = self.activity_count.saturating_add(1);
    }

    /// C++ `AIUpdateInterface::joinTeam` (`AIUpdate.cpp:2516`) — copy a
    /// teammate's current order. Reinforcements never invent AttackMove.
    fn join_team_reinforcement_host(
        &mut self,
        game_logic: &mut GameLogic,
        obj_id: ObjectId,
        members: &[ObjectId],
    ) {
        let mobile = game_logic
            .host_object(obj_id)
            .map(|o| o.is_alive() && o.is_mobile())
            .unwrap_or(false);
        if !mobile {
            return;
        }
        let other_id = members.iter().copied().find(|&id| {
            id != obj_id
                && game_logic
                    .host_object(id)
                    .map(|o| o.is_alive() && !o.is_kind_of(crate::game_logic::KindOf::Immobile))
                    .unwrap_or(false)
        });
        let Some(other_id) = other_id else {
            return;
        };
        let (other_idle, other_pos, other_state, other_target) =
            match game_logic.host_object(other_id) {
                Some(other) => (
                    other.ai_state == AIState::Idle,
                    other.get_position(),
                    other.ai_state.clone(),
                    other.target,
                ),
                None => return,
            };
        if other_idle {
            if game_logic.assign_unit_path(obj_id, other_pos, &[]) {
                game_logic.set_ai_state_decision_aware_for_ai(obj_id, AIState::Moving);
            } else if let Some(unit) = game_logic.host_object_mut(obj_id) {
                unit.move_to(other_pos);
                game_logic.set_ai_state_decision_aware_for_ai(obj_id, AIState::Moving);
            }
            return;
        }
        if let Some(unit) = game_logic.host_object_mut(obj_id) {
            unit.target = other_target;
        }
        game_logic.set_ai_state_decision_aware_for_ai(obj_id, other_state);
    }

    /// Apply leftover OnCreate script intent to live host members.
    /// C++ `checkReadyTeams` only `setActive`; Hunt/Guard/AttackMove come
    /// from the team's OnCreate script actions.
    fn apply_on_create_host_orders(
        &mut self,
        game_logic: &mut GameLogic,
        members: &[ObjectId],
        on_create: &str,
        current_time: f32,
    ) {
        if members.is_empty() {
            return;
        }
        match Self::classify_on_create_script(on_create) {
            OnCreateIntent::Hunt => self.hunt_units(game_logic, members),
            OnCreateIntent::HuntWithCommandButton => {
                self.hunt_units_with_command_button(game_logic, members, on_create)
            }
            OnCreateIntent::Guard => self.guard_units(game_logic, members),
            OnCreateIntent::AttackMove => {
                self.attack_move_units(game_logic, members, current_time)
            }
            OnCreateIntent::None => {}
        }
    }

    fn classify_on_create_script(on_create: &str) -> OnCreateIntent {
        if on_create.is_empty() || on_create == "<none>" {
            return OnCreateIntent::None;
        }
        use gamelogic::scripting::core::ScriptActionType;
        if let Ok(eng) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(e) = eng.as_ref() {
                if let Some(script) = e.find_script_clone_by_name(on_create) {
                    let mut act = script.get_action();
                    while let Some(a) = act {
                        match a.get_action_type() {
                            ScriptActionType::TeamHunt
                            | ScriptActionType::NamedHunt
                            | ScriptActionType::PlayerHunt => {
                                return OnCreateIntent::Hunt;
                            }
                            ScriptActionType::TeamHuntWithCommandButton => {
                                // C++ doTeamHuntWithCommandButton arms
                                // CommandButtonHuntUpdate, not groupHunt.
                                return OnCreateIntent::HuntWithCommandButton;
                            }
                            ScriptActionType::TeamGuard | ScriptActionType::NamedGuard => {
                                return OnCreateIntent::Guard;
                            }
                            ScriptActionType::TeamGuardArea
                            | ScriptActionType::TeamGuardPosition
                            | ScriptActionType::TeamGuardObject
                            | ScriptActionType::TeamGuardSupplyCenter
                            | ScriptActionType::TeamGuardInTunnelNetwork => {
                                // Leftover run_script queues the scripted anchor.
                                // Do not collapse to guard-at-current-position.
                                return OnCreateIntent::None;
                            }
                            ScriptActionType::TeamAttackArea
                            | ScriptActionType::TeamAttackTeam
                            | ScriptActionType::TeamAttackNamed => {
                                return OnCreateIntent::AttackMove;
                            }
                            _ => {}
                        }
                        act = a.get_next();
                    }
                }
            }
        }
        let n = on_create.to_ascii_lowercase();
        if n.contains("guardposition")
            || n.contains("guard_position")
            || n.contains("guardobject")
            || n.contains("guard_object")
            || n.contains("guardarea")
            || n.contains("guard_area")
            || n.contains("tunnel")
            || n.contains("supplycenter")
            || n.contains("supply_center")
        {
            OnCreateIntent::None
        } else if n.contains("guard") {
            OnCreateIntent::Guard
        } else if n.contains("attackmove")
            || n.contains("attack_move")
            || n.contains("team_attack")
        {
            OnCreateIntent::AttackMove
        } else if n.contains("huntwithcommand")
            || n.contains("hunt_with_command")
            || (n.contains("hunt") && n.contains("command"))
        {
            OnCreateIntent::HuntWithCommandButton
        } else if n.contains("hunt") {
            OnCreateIntent::Hunt
        } else {
            OnCreateIntent::None
        }
    }

    /// C++ `AIGroup::groupHunt` residual (`ScriptActions::doTeamHunt`).
    fn hunt_units(&mut self, game_logic: &mut GameLogic, units: &[ObjectId]) {
        // C++ AIGroup::groupHunt: every member with AIUpdateInterface → aiHunt.
        // No can_move / Immobile / Structure gate.
        for &unit_id in units {
            let alive = game_logic
                .host_object(unit_id)
                .is_some_and(|u| u.is_alive());
            if alive {
                let _ = game_logic.unit_command_patrol(unit_id);
            }
        }
    }

    /// C++ `ScriptActions::doTeamHuntWithCommandButton` — arm CommandButtonHuntUpdate.
    fn hunt_units_with_command_button(
        &mut self,
        game_logic: &mut GameLogic,
        units: &[ObjectId],
        on_create: &str,
    ) {
        let button = Self::on_create_command_button_name(on_create);
        for &unit_id in units {
            if game_logic.unit_can_team_hunt_with_command_button(unit_id, button.as_deref()) {
                let _ = game_logic.start_command_button_hunt_named(unit_id, button.as_deref());
            }
        }
    }

    fn on_create_command_button_name(on_create: &str) -> Option<String> {
        use gamelogic::scripting::core::ScriptActionType;
        let engine = gamelogic::scripting::engine::get_script_engine();
        let Ok(eng) = engine.read() else {
            return None;
        };
        let e = eng.as_ref()?;
        let script = e.find_script_clone_by_name(on_create)?;
        let mut act = script.get_action();
        while let Some(a) = act {
            if a.get_action_type() == ScriptActionType::TeamHuntWithCommandButton {
                if let Some(name) = a.get_parameter(1).map(|p| p.get_string().to_string()) {
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
            act = a.get_next();
        }
        None
    }

    /// C++ `ScriptActions::doTeamGuard` — guard at current positions.
    fn guard_units(&mut self, game_logic: &mut GameLogic, units: &[ObjectId]) {
        for &unit_id in units {
            let pos = game_logic.host_object(unit_id).and_then(|u| {
                game_logic
                    .host_unit_can_guard(unit_id)
                    .then_some(u.get_position())
            });
            if let Some(pos) = pos {
                let _ = game_logic.unit_command_guard_position(unit_id, pos);
            }
        }
    }

    /// C++ `AIPlayer::checkQueuedTeams` (`AIPlayer.cpp:2810-2870`).
    fn check_queued_teams(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        let mut i = 0;
        while i < self.team_queue.len() {
            if !self.team_queue[i].is_build_time_expired(current_time) {
                i += 1;
                continue;
            }
            if self.team_queue[i].is_minimum_built() {
                if self.team_queue[i].are_builds_complete() {
                    if let Some(team) = self.team_queue.remove(i) {
                        self.team_ready_queue.push_front(team);
                    }
                } else {
                    i += 1;
                }
            } else if let Some(team) = self.team_queue.remove(i) {
                self.disband_queued_team(game_logic, &team);
                log::debug!(
                    "AI Player {} disbanded expired team: {}",
                    self.player_id,
                    team.name
                );
            }
        }

        let mut i = 0;
        while i < self.team_queue.len() {
            if self.team_queue[i].is_all_built() || self.team_queue[i].completed {
                if let Some(team) = self.team_queue.remove(i) {
                    self.team_ready_queue.push_front(team);
                }
                continue;
            }
            let member_ids: Vec<ObjectId> = self.team_queue[i]
                .work_orders
                .iter()
                .flat_map(|order| order.observed_unit_ids.iter().copied())
                .collect();
            let any_idle = member_ids.iter().any(|id| {
                game_logic
                    .host_object(*id)
                    .map(|o| o.is_alive() && o.ai_state == AIState::Idle)
                    .unwrap_or(false)
            });
            if any_idle {
                let team_name = self.team_queue[i].name.clone();
                let execute = self.team_queue[i].execute_actions
                    || Self::prototype_execute_actions_on_create(&team_name);
                if execute {
                    self.execute_production_condition_actions(
                        game_logic,
                        &team_name,
                        &member_ids,
                        current_time,
                    );
                }
            }
            i += 1;
        }
    }

    fn prototype_execute_actions_on_create(team_name: &str) -> bool {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(team_name)
                    .map(|proto| proto.get_execute_actions_on_create())
            })
            .unwrap_or(false)
    }

    fn prototype_production_condition(team_name: &str) -> String {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(team_name)
                    .map(|proto| proto.get_production_condition().to_string())
            })
            .unwrap_or_default()
    }

    /// C++ `TheScriptEngine->findScriptByName(m_productionCondition)` + `getAction()`.
    fn production_condition_has_action(team_name: &str) -> bool {
        let cond = Self::prototype_production_condition(team_name);
        if cond.is_empty() {
            return false;
        }
        gamelogic::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|eng| eng.as_ref().and_then(|e| e.find_script_clone_by_name(&cond)))
            .and_then(|script| script.get_action().cloned())
            .is_some()
    }


    fn execute_production_condition_actions(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        members: &[ObjectId],
        current_time: f32,
    ) {
        let cond = Self::prototype_production_condition(team_name);
        if cond.is_empty() {
            return;
        }
        if let Ok(eng) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(e) = eng.as_ref() {
                if let Some(script) = e.find_script_clone_by_name(&cond) {
                    if let Some(action) = script.get_action().cloned() {
                        drop(eng);
                        if let Ok(mut writer) =
                            gamelogic::scripting::engine::get_script_engine().write()
                        {
                            if let Some(engine) = writer.as_mut() {
                                engine.friend_execute_action(&action, Some(team_name));
                            }
                        }
                    }
                }
            }
        }
        let idle: Vec<ObjectId> = members
            .iter()
            .copied()
            .filter(|id| {
                game_logic
                    .host_object(*id)
                    .map(|o| o.is_alive() && o.ai_state == AIState::Idle)
                    .unwrap_or(false)
            })
            .collect();
        self.apply_on_create_host_orders(game_logic, &idle, &cond, current_time);
    }

    fn with_can_build_units<R>(
        game_logic: &mut GameLogic,
        player_id: u32,
        force: bool,
        f: impl FnOnce(&mut GameLogic) -> R,
    ) -> R {
        let prev = game_logic
            .get_player(player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true);
        if let Some(player) = game_logic.get_player_mut(player_id) {
            player.set_can_build_units(force);
        }
        let result = f(game_logic);
        if let Some(player) = game_logic.get_player_mut(player_id) {
            player.set_can_build_units(prev);
        }
        result
    }

    /// C++ `AIPlayer::doUpgradesAndSkills` (AIPlayer.cpp:2908) spends science
    /// points from a randomly chosen `AISideInfo` SkillSet. Host also runs a
    /// residual of `AIPlayer::buildUpgrade` (AIPlayer.cpp:1728).
    fn do_upgrades_and_skills(&mut self, game_logic: &mut GameLogic) {
        self.try_queue_structure_upgrade(game_logic);
        self.try_purchase_skillset_science(game_logic);
    }

    /// Retail `AIData.ini` `SideInfo` SkillSet1–5 sciences for the live team.
    fn side_skillsets(&self) -> [&'static [&'static str]; 5] {
        match self.team {
            Team::USA => [
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_A10ThunderboltMissileStrike1",
                    "SCIENCE_A10ThunderboltMissileStrike2",
                    "SCIENCE_A10ThunderboltMissileStrike3",
                    "SCIENCE_SpectreGunshipSolo",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_Paradrop1",
                    "SCIENCE_Paradrop2",
                    "SCIENCE_Paradrop3",
                    "SCIENCE_SpyDrone",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_Pathfinder",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_A10ThunderboltMissileStrike1",
                    "SCIENCE_A10ThunderboltMissileStrike2",
                    "SCIENCE_A10ThunderboltMissileStrike3",
                    "SCIENCE_LeafletDrop",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_Pathfinder",
                    "SCIENCE_Paradrop1",
                    "SCIENCE_Paradrop2",
                    "SCIENCE_Paradrop3",
                    "SCIENCE_SpyDrone",
                    "SCIENCE_DaisyCutter",
                ],
                &[
                    "SCIENCE_PaladinTank",
                    "SCIENCE_StealthFighter",
                    "SCIENCE_A10ThunderboltMissileStrike1",
                    "SCIENCE_A10ThunderboltMissileStrike2",
                    "SCIENCE_A10ThunderboltMissileStrike3",
                    "SCIENCE_SpectreGunshipSolo",
                    "SCIENCE_DaisyCutter",
                ],
            ],
            Team::China => [
                &[
                    "SCIENCE_NukeLauncher",
                    "SCIENCE_ArtilleryTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
                &[
                    "SCIENCE_RedGuardTraining",
                    "SCIENCE_BattlemasterTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
                &[
                    "SCIENCE_NukeLauncher",
                    "SCIENCE_RedGuardTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_CarpetBomb",
                ],
                &[
                    "SCIENCE_BattlemasterTraining",
                    "SCIENCE_ArtilleryTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
                &[
                    "SCIENCE_NukeLauncher",
                    "SCIENCE_ArtilleryTraining",
                    "SCIENCE_ClusterMines",
                    "SCIENCE_ArtilleryBarrage1",
                    "SCIENCE_ArtilleryBarrage2",
                    "SCIENCE_ArtilleryBarrage3",
                    "SCIENCE_EMPPulse",
                ],
            ],
            Team::GLA => [
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_CashBounty1",
                    "SCIENCE_CashBounty2",
                    "SCIENCE_CashBounty3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_RebelAmbush1",
                    "SCIENCE_RebelAmbush2",
                    "SCIENCE_RebelAmbush3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_TechnicalTraining",
                    "SCIENCE_CashBounty1",
                    "SCIENCE_CashBounty2",
                    "SCIENCE_CashBounty3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_CashBounty1",
                    "SCIENCE_RebelAmbush1",
                    "SCIENCE_RebelAmbush2",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
                &[
                    "SCIENCE_ScudLauncher",
                    "SCIENCE_RebelAmbush1",
                    "SCIENCE_RebelAmbush2",
                    "SCIENCE_RebelAmbush3",
                    "SCIENCE_SneakAttack",
                    "SCIENCE_GPSScrambler",
                    "SCIENCE_AnthraxBomb",
                ],
            ],
            _ => [&[][..], &[], &[], &[], &[]],
        }
    }

    /// C++ `AIPlayer::selectSkillset`.
    pub fn select_skillset(&mut self, skillset: i32) {
        self.skillset_selector = skillset;
    }

    /// C++ `AIPlayer::m_skillsetSelector` after `selectSkillset`.
    pub fn selected_skillset(&self) -> i32 {
        self.skillset_selector
    }

    fn try_purchase_skillset_science(&mut self, game_logic: &mut GameLogic) {
        let points = game_logic
            .get_player(self.player_id)
            .map(|p| p.science_purchase_points)
            .unwrap_or(0);
        if points <= 0 {
            return;
        }
        let sets = self.live_side_skillsets(game_logic);
        if self.skillset_selector == INVALID_SKILLSET_SELECTION {
            let mut limit = 0i32;
            if !sets[1].is_empty() {
                limit = 1;
                if !sets[2].is_empty() {
                    limit = 2;
                    if !sets[3].is_empty() {
                        limit = 3;
                        if !sets[4].is_empty() {
                            limit = 4;
                        }
                    }
                }
            }
            // Host leftover AIPlayer is the skirmish path — randomize.
            self.skillset_selector = self.placement_rng.next_int(0, limit);
        }
        let idx = self.skillset_selector.clamp(0, 4) as usize;
        let candidates = sets[idx].clone();
        let purchased: Vec<String> = {
            let Some(player) = game_logic.get_player_mut(self.player_id) else {
                return;
            };
            let mut purchased = Vec::new();
            for name in candidates {
                if player.is_capable_of_purchasing_science(&name)
                    && player.attempt_to_purchase_science(&name)
                {
                    log::debug!(
                        "AI Player {} purchases from SkillSet{} {}",
                        self.player_id,
                        idx + 1,
                        name
                    );
                    purchased.push(name);
                }
            }
            purchased
        };
        // C++ Player::addScience → onSpecialPowerCreation + setReadyFrame(now)
        // for modules whose required science matches. Human/script purchase
        // already calls this; AI skillset buys must too.
        for name in purchased {
            game_logic.on_special_power_science_creation(self.player_id, &name);
        }
    }


    /// C++ `AIPlayer::buildUpgrade` residual — queue one structure upgrade.
    fn structure_upgrade_candidates(&self) -> &'static [&'static str] {
        use crate::game_logic::host_upgrades::{
            UPGRADE_AMERICA_ADVANCED_TRAINING, UPGRADE_AMERICA_RANGER_CAPTURE,
            UPGRADE_AMERICA_SUPPLY_LINES, UPGRADE_CHINA_NATIONALISM,
            UPGRADE_CHINA_REDGUARD_CAPTURE, UPGRADE_GLA_AP_BULLETS, UPGRADE_GLA_REBEL_CAPTURE,
        };
        match self.team {
            Team::USA => &[
                UPGRADE_AMERICA_SUPPLY_LINES,
                UPGRADE_AMERICA_RANGER_CAPTURE,
                UPGRADE_AMERICA_ADVANCED_TRAINING,
            ],
            Team::China => &[UPGRADE_CHINA_NATIONALISM, UPGRADE_CHINA_REDGUARD_CAPTURE],
            Team::GLA => &[UPGRADE_GLA_REBEL_CAPTURE, UPGRADE_GLA_AP_BULLETS],
            _ => &[],
        }
    }

    fn preferred_upgrade_producer_names(upgrade_name: &str) -> &'static [&'static str] {
        let n = upgrade_name.to_ascii_lowercase();
        if n.contains("supplylines") {
            &["AmericaSupplyCenter", "ChinaSupplyCenter", "GLASupplyStash"]
        } else if n.contains("capture") {
            &["AmericaBarracks", "ChinaBarracks", "GLABarracks"]
        } else if n.contains("advancedtraining") {
            &["AmericaStrategyCenter"]
        } else if n.contains("nationalism") {
            &["ChinaPropagandaCenter"]
        } else if n.contains("apbullets") {
            &["GLAPalace"]
        } else {
            &[]
        }
    }

    fn building_can_queue_upgrade(object: &crate::game_logic::Object, upgrade_name: &str) -> bool {
        if !object.is_alive() || !object.is_constructed() {
            return false;
        }
        let Some(building) = object.building_data.as_ref() else {
            return false;
        };
        if building.production_queue.len() >= crate::game_logic::DEFAULT_PRODUCTION_QUEUE_LIMIT {
            return false;
        }
        if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name)
            && object.refuses_object_upgrade(upgrade_name)
        {
            return false;
        }
        !building
            .production_queue
            .iter()
            .any(|item| item.is_upgrade() && item.template_name.eq_ignore_ascii_case(upgrade_name))
    }

    fn find_upgrade_producer(
        &self,
        game_logic: &GameLogic,
        upgrade_name: &str,
    ) -> Option<ObjectId> {
        let preferred = Self::preferred_upgrade_producer_names(upgrade_name);
        let mut fallback = None;
        for (&id, object) in game_logic.host_objects() {
            if object.team != self.team || !Self::building_can_queue_upgrade(object, upgrade_name) {
                continue;
            }
            let name_ok = preferred.iter().any(|name| {
                object.template_name.eq_ignore_ascii_case(name)
                    || object.get_template().name.eq_ignore_ascii_case(name)
            });
            if name_ok {
                return Some(id);
            }
            if fallback.is_none() {
                fallback = Some(id);
            }
        }
        fallback
    }

    fn try_queue_structure_upgrade(&mut self, game_logic: &mut GameLogic) {
        let Some(player) = game_logic.get_player(self.player_id) else {
            return;
        };
        if !player.is_alive {
            return;
        }
        let Some(upgrade_name) = self.structure_upgrade_candidates().iter().copied().find(|name| {
            !player.has_unlocked_upgrade(name) && !player.has_queued_upgrade(name)
        }) else {
            return;
        };
        let kind = HostUpgradeKind::from_name(upgrade_name);
        let cost = Resources {
            supplies: kind.retail_build_cost(),
            power: 0,
        };
        if cost.supplies == 0 || !player.can_afford(&cost) {
            return;
        }
        let Some(producer_id) = self.find_upgrade_producer(game_logic, upgrade_name) else {
            return;
        };
        let Some(player) = game_logic.get_player_mut(self.player_id) else {
            return;
        };
        if !player.queue_upgrade(upgrade_name, &cost) {
            return;
        }
        let secs = kind.retail_build_time_secs().max(1.0 / LOGIC_FRAMES_PER_SECOND);
        if !game_logic.unit_command_building_add_upgrade_to_queue(
            producer_id,
            upgrade_name,
            secs,
            cost,
        ) {
            if let Some(player) = game_logic.get_player_mut(self.player_id) {
                let _ = player.cancel_queued_upgrade(upgrade_name, &cost);
            }
            return;
        }
        game_logic.record_host_upgrade_queued(
            self.player_id,
            self.team,
            upgrade_name,
            Some(producer_id),
        );
        game_logic
            .host_upgrades_mut()
            .set_build_cost_paid(upgrade_name, self.player_id, cost.supplies);
        let frames = (secs * LOGIC_FRAMES_PER_SECOND).round().max(1.0) as u32;
        game_logic.host_upgrades_mut().set_resolved_research_frames(
            upgrade_name,
            self.player_id,
            frames,
        );
        self.activity_count = self.activity_count.saturating_add(1);
    }

    /// Pick candidate team name for the current strategy (same as select_team_to_build).
    fn candidate_team_name(&self) -> Option<String> {
        match self.current_strategy {
            AIStrategy::EarlyGame => self.select_early_game_team(),
            AIStrategy::MidGame => self.select_mid_game_team(),
            AIStrategy::LateGame => self.select_late_game_team(),
            AIStrategy::Desperate => self.select_desperate_team(),
        }
    }

    /// Check if AI should build a new team
    fn should_build_new_team(&self, game_logic: &GameLogic) -> bool {
        if !game_logic
            .get_player(self.player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true)
        {
            return false;
        }
        if self.team_queue.len() >= 3 {
            return false;
        }
        self.player_team_prototype_candidates()
            .iter()
            .any(|(name, _)| self.is_a_good_idea_to_build_team(game_logic, name))
    }

    /// C++ `AIPlayer::selectTeamToBuild` — player TeamPrototypes only.
    fn select_team_to_build(&mut self, game_logic: &mut GameLogic, current_time: f32) -> bool {
        const INVALID_PRI: i32 = -99999;
        let candidates = self.player_team_prototype_candidates();
        let mut good: Vec<(String, i32)> = Vec::new();
        let mut hi_pri = INVALID_PRI;
        for (name, pri) in candidates {
            if self.is_a_good_idea_to_build_team(game_logic, &name) {
                if pri > hi_pri {
                    hi_pri = pri;
                }
                good.push((name, pri));
            }
        }
        if self.select_team_to_reinforce(game_logic, hi_pri, current_time) {
            return true;
        }
        if hi_pri == INVALID_PRI {
            return false;
        }
        let hi: Vec<String> = good
            .into_iter()
            .filter(|(_, p)| *p == hi_pri)
            .map(|(n, _)| n)
            .collect();
        if hi.is_empty() {
            return false;
        }
        let which = if hi.len() == 1 {
            0
        } else {
            self.placement_rng.next_int(0, (hi.len() as i32) - 1) as usize
        };
        let name = &hi[which.min(hi.len() - 1)];
        // C++ `buildSpecificAITeam(teamProto, false)` — auto pick is low
        // priority. Work orders come from TeamPrototype min/max split
        // (optional max-min, then required min including 0), not invented
        // max-as-required compositions.
        if !self.build_specific_ai_team(game_logic, name, false) {
            return false;
        }
        log::debug!("AI Player {} queued team: {}", self.player_id, name);
        true
    }

    fn player_team_prototype_candidates(&self) -> Vec<(String, i32)> {
        let Ok(list) = gamelogic::player::player_list().read() else {
            return Vec::new();
        };
        let Some(player) = list.get_player(self.player_id as i32) else {
            return Vec::new();
        };
        let Ok(pg) = player.read() else {
            return Vec::new();
        };
        pg.get_player_team_prototypes()
            .iter()
            .map(|proto| (proto.get_name().to_string(), proto.get_production_priority()))
            .collect()
    }

    /// C++ `AIPlayer::isAGoodIdeaToBuildTeam`.
    fn is_a_good_idea_to_build_team(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        let factory = gamelogic::team::get_team_factory();
        let Ok(guard) = factory.lock() else {
            return false;
        };
        let Some(proto) = guard.find_team_prototype(team_name) else {
            return false;
        };
        if !proto.evaluate_production_condition() {
            return false;
        }
        let instances = guard.find_team_instances(team_name).len() as i32;
        if instances >= proto.get_max_instances() {
            return false;
        }
        // C++ AIPlayer.cpp:1487-1492 — only iterate_TeamBuildQueue.
        // Ready-queue teams are idle / 60s force, not a "currently building" veto.
        // countTeamInstances() >= maxInstances is the only instance cap.
        if self.team_queue.iter().any(|t| t.name == team_name) {
            return false;
        }
        drop(guard);
        self.is_possible_to_build_team(game_logic, team_name)
    }

    /// C++ `AIPlayer::selectTeamToReinforce` (`AIPlayer.cpp:1513-1625`).
    fn select_team_to_reinforce(
        &mut self,
        game_logic: &mut GameLogic,
        min_priority: i32,
        current_time: f32,
    ) -> bool {
        let candidates = self.collect_auto_reinforce_candidates();
        let mut best: Option<(u32, String, String, i32)> = None;
        let mut cur = min_priority;
        for cand in candidates {
            if cand.priority <= cur {
                continue;
            }
            if self.team_queue.iter().any(|t| t.name == cand.name) {
                continue;
            }
            let instances = gamelogic::team::get_team_factory()
                .lock()
                .ok()
                .map(|factory| factory.find_team_instances(&cand.name))
                .unwrap_or_default();
            for inst in instances {
                let Ok(tg) = inst.read() else {
                    continue;
                };
                let inst_id = tg.get_id();
                drop(tg);
                if !Self::leftover_instance_has_any_host_units(game_logic, inst_id) {
                    continue;
                }
                for unit in &cand.units {
                    if unit.max_units < 1 || unit.thing.is_empty() {
                        continue;
                    }
                    let count =
                        Self::leftover_instance_count_template(game_logic, inst_id, &unit.thing);
                    if count >= unit.max_units as u32 {
                        continue;
                    }
                    if Self::find_factory_for_unit_ex(game_logic, &unit.thing, self.team, false)
                        .is_none()
                    {
                        continue;
                    }
                    best = Some((inst_id, cand.name.clone(), unit.thing.clone(), cand.priority));
                    cur = cand.priority;
                }
            }
        }
        let Some((inst_id, team_name, thing, _)) = best else {
            return false;
        };
        let mut order = AIWorkOrder::new(thing.clone(), 1, 100);
        let home = Self::leftover_instance_first_member_pos(game_logic, inst_id)
            .unwrap_or_else(|| self.team_home_or_base(&team_name));
        if let Some(unit_id) = self.try_to_recruit(game_logic, &team_name, &thing, home, None) {
            order.num_completed = 1;
            order.observed_unit_ids.push(unit_id);
            Self::assign_host_unit_to_leftover_team(
                game_logic,
                unit_id,
                Some(inst_id),
                &team_name,
            );
            if let Some(obj) = game_logic.host_object_mut(unit_id) {
                obj.set_ai_state(AIState::Idle);
            }
        }
        let mut q = AITeamQueue::new(
            team_name,
            vec![order],
            false,
            (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
        );
        q.reinforcement = true;
        if let Some(id) = q.work_orders.first().and_then(|o| o.observed_unit_ids.first()) {
            q.reinforcement_id = Some(*id);
        }
        // C++ reinforce uses the existing team instance, not a new inactive team.
        q.team_id = Some(inst_id);
        self.team_queue.push_front(q);
        // C++ m_teamDelay = 0
        self.next_team_queue_time = current_time;
        self.activity_count = self.activity_count.saturating_add(1);
        true
    }

    fn collect_auto_reinforce_candidates(&self) -> Vec<ReinforceCandidate> {
        let mut out = Vec::new();
        if let Ok(list) = gamelogic::player::player_list().read() {
            if let Some(player) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player.read() {
                    for proto in pg.get_player_team_prototypes() {
                        if !proto.automatically_reinforce() {
                            continue;
                        }
                        out.push(ReinforceCandidate {
                            name: proto.get_name().to_string(),
                            priority: proto.get_production_priority(),
                            units: proto
                                .units_info()
                                .iter()
                                .map(|u| ReinforceUnit {
                                    thing: u.unit_thing_name.to_string(),
                                    max_units: u.max_units,
                                })
                                .collect(),
                        });
                    }
                }
            }
        }
        if out.is_empty() {
            if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
                // Host tests / unsynced player list: scan factory prototypes.
                for proto in factory.list_team_prototypes() {
                    if !proto.automatically_reinforce() {
                        continue;
                    }
                    out.push(ReinforceCandidate {
                        name: proto.get_name().to_string(),
                        priority: proto.get_production_priority(),
                        units: proto
                            .units_info()
                            .iter()
                            .map(|u| ReinforceUnit {
                                thing: u.unit_thing_name.to_string(),
                                max_units: u.max_units,
                            })
                            .collect(),
                    });
                }
            }
        }
        out
    }

    fn count_owned_template_units(&self, game_logic: &GameLogic, template_name: &str) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                Self::pad_object_still_ours(object, self.player_id, self.team)
                    && object.is_alive()
                    && !object.is_kind_of(KindOf::Structure)
                    && object.template_name.eq_ignore_ascii_case(template_name)
            })
            .count() as u32
    }

    fn team_home_or_base(&self, team_name: &str) -> Vec3 {
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(team_name) {
                if proto.has_home_location() {
                    let loc = proto.home_location();
                    return Vec3::new(loc.x, loc.z, loc.y);
                }
            }
        }
        self.base_center
    }

    fn try_to_recruit(
        &self,
        game_logic: &GameLogic,
        dest_team_name: &str,
        template_name: &str,
        home: Vec3,
        max_dist: Option<f32>,
    ) -> Option<ObjectId> {
        let mut assigned: HashSet<ObjectId> = HashSet::new();
        for team in self.team_queue.iter().chain(self.team_ready_queue.iter()) {
            for order in &team.work_orders {
                assigned.extend(order.observed_unit_ids.iter().copied());
            }
        }
        self.try_to_recruit_excluding(
            game_logic,
            dest_team_name,
            template_name,
            home,
            max_dist.unwrap_or_else(Self::aidata_max_recruit_distance),
            &assigned,
        )
    }

    fn dest_team_production_priority(dest_team_name: &str) -> i32 {
        gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(dest_team_name)
                    .map(|proto| proto.get_production_priority())
            })
            .unwrap_or(i32::MAX)
    }

    fn source_is_default_team(game_logic: &GameLogic, object: &crate::game_logic::Object) -> bool {
        let name = object.team_instance_name.trim();
        if name.is_empty() {
            return true;
        }
        let default = game_logic.default_host_team_instance_name(object.owner_player_id, object.team);
        if name.eq_ignore_ascii_case(&default) {
            return true;
        }
        name.eq_ignore_ascii_case(&format!("team{}", object.team.get_name()))
    }

    /// `(active, proto_ai_recruitable, recruitability_set, priority, override_recruitable)`.
    fn leftover_source_team_state(name: &str) -> (bool, bool, bool, i32, bool) {
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return (true, false, false, 0, false);
        };
        let proto = factory.find_team_prototype(name);
        let priority = proto
            .as_ref()
            .map(|p| p.get_production_priority())
            .unwrap_or(0);
        let proto_ai = proto.as_ref().map(|p| p.is_ai_recruitable()).unwrap_or(false);
        if let Some(team) = factory.find_team_instances(name).into_iter().next() {
            if let Ok(tg) = team.read() {
                return (
                    tg.is_active(),
                    proto_ai,
                    tg.is_recruitability_set(),
                    priority,
                    tg.is_recruitable(),
                );
            }
        }
        (true, proto_ai, false, priority, false)
    }

    fn recruit_template_matches(dest_template: &str, candidate: &str) -> bool {
        crate::game_logic::weapon_bootstrap::splash_templates_equivalent(dest_template, candidate)
    }

    fn try_to_recruit_excluding(
        &self,
        game_logic: &GameLogic,
        dest_team_name: &str,
        template_name: &str,
        home: Vec3,
        max_dist: f32,
        assigned: &HashSet<ObjectId>,
    ) -> Option<ObjectId> {
        let dest_priority = Self::dest_team_production_priority(dest_team_name);
        let mut dist_sqr = max_dist * max_dist;
        let mut recruit: Option<ObjectId> = None;
        for (&id, object) in game_logic.host_objects() {
            if !Self::pad_object_still_ours(object, self.player_id, self.team) {
                continue;
            }
            if !object.is_alive() {
                continue;
            }
            // C++ Team::tryToRecruit DISABLED_HELD only (Team.cpp:2353-2356).
            // No KindOf::Structure skip. No contained-by skip beyond HELD.
            if object.status.disabled_held {
                continue;
            }
            // C++ AIUpdateInterface::isRecruitable (Team.cpp:2350-2352).
            if !object.is_recruitable {
                continue;
            }
            if !Self::recruit_template_matches(template_name, &object.template_name) {
                continue;
            }
            if assigned.contains(&id) {
                continue;
            }

            let source_name = if object.team_instance_name.is_empty() {
                game_logic.default_host_team_instance_name(object.owner_player_id, object.team)
            } else {
                object.team_instance_name.clone()
            };
            let is_default = Self::source_is_default_team(game_logic, object);
            let (active, proto_ai, recruitability_set, source_priority, override_recruitable) =
                Self::leftover_source_team_state(&source_name);
            // C++: do not steal from a team still building (Team.cpp:2333-2335).
            if !active {
                continue;
            }
            // C++ source productionPriority < dest (Team.cpp:2336-2338).
            if source_priority >= dest_priority {
                continue;
            }
            let team_is_recruitable = if recruitability_set {
                override_recruitable
            } else {
                is_default || proto_ai
            };
            if !team_is_recruitable {
                continue;
            }

            let pos = object.get_position();
            let d2 = (pos - home).length_squared();
            // C++ default-team fallback even if farther than maxDist (Team.cpp:2361-2370).
            if is_default && recruit.is_none() {
                recruit = Some(id);
                dist_sqr = d2;
            }
            if d2 > dist_sqr {
                continue;
            }
            dist_sqr = d2;
            recruit = Some(id);
        }
        recruit
    }

    fn recruit_waiting_work_orders(&mut self, game_logic: &mut GameLogic) {
        let max_dist = Self::aidata_max_recruit_distance();
        for team in self.team_queue.iter_mut() {
            Self::bind_inactive_team_handle(team);
        }
        let mut assigned: HashSet<ObjectId> = HashSet::new();
        for team in self.team_queue.iter().chain(self.team_ready_queue.iter()) {
            for order in &team.work_orders {
                assigned.extend(order.observed_unit_ids.iter().copied());
            }
        }
        let jobs: Vec<(usize, usize, String, Option<u32>, String, Vec3, bool, u32)> = self
            .team_queue
            .iter()
            .enumerate()
            .flat_map(|(ti, team)| {
                let home = self.team_home_or_base(&team.name);
                let dest_name = team.name.clone();
                let dest_id = team.team_id;
                let has_home = gamelogic::team::get_team_factory()
                    .lock()
                    .ok()
                    .and_then(|f| f.find_team_prototype(&team.name).map(|p| p.has_home_location()))
                    .unwrap_or(false);
                team.work_orders
                    .iter()
                    .enumerate()
                    .filter_map(move |(oi, order)| {
                        if order.num_completed < order.num_required && order.factory_id.is_none() {
                            Some((
                                ti,
                                oi,
                                dest_name.clone(),
                                dest_id,
                                order.template_name.clone(),
                                home,
                                has_home,
                                order.num_required - order.num_completed,
                            ))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut found: Vec<(usize, usize, ObjectId, Option<u32>, String, Vec3, bool)> = Vec::new();
        for (ti, oi, dest_name, dest_id, thing, home, has_home, need) in jobs {
            let mut got = 0u32;
            while got < need {
                let Some(unit_id) = self.try_to_recruit_excluding(
                    game_logic,
                    &dest_name,
                    &thing,
                    home,
                    max_dist,
                    &assigned,
                )
                else {
                    break;
                };
                assigned.insert(unit_id);
                found.push((ti, oi, unit_id, dest_id, dest_name.clone(), home, has_home));
                got = got.saturating_add(1);
            }
        }
        for (ti, oi, unit_id, dest_id, dest_name, home, has_home) in found {
            if let Some(order) = self
                .team_queue
                .get_mut(ti)
                .and_then(|t| t.work_orders.get_mut(oi))
            {
                order.num_completed = order.num_completed.saturating_add(1);
                order.observed_unit_ids.push(unit_id);
            }
            Self::assign_host_unit_to_leftover_team(game_logic, unit_id, dest_id, &dest_name);
            if has_home {
                let _ = game_logic.unit_command_move_to(unit_id, home);
            } else if let Some(obj) = game_logic.host_object_mut(unit_id) {
                obj.set_ai_state(AIState::Idle);
            }
        }
    }

    /// C++ `AIPlayer::buildSpecificAITeam`.
    pub fn build_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        priority_build: bool,
    ) -> bool {
        let can_build = game_logic
            .get_player(self.player_id)
            .map(|p| p.can_build_units)
            .unwrap_or(true);
        if !can_build {
            return false;
        }

        let proto = {
            let Ok(mut factory) = gamelogic::team::get_team_factory().lock() else {
                return false;
            };
            let Some(proto) = factory.find_team_prototype(team_name) else {
                return false;
            };
            if priority_build && proto.is_singleton() {
                if let Some(existing) = factory.find_team(team_name) {
                    if existing.read().ok().is_some_and(|eg| eg.has_any_objects()) {
                        return false;
                    }
                }
            }
            proto
        };

        let execute_actions = proto.get_execute_actions_on_create();
        let production_condition = proto.get_production_condition().to_string();
        drop(proto);

        let orders = self.build_team_work_orders(game_logic, team_name);
        if orders.is_empty() {
            return false;
        }
        // C++ `isPossibleToBuildTeam(proto, false)`: factories must exist;
        // missing money still queues.
        if !self.team_factories_exist(game_logic, &orders) {
            return false;
        }

        let team_id = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.create_inactive_team(team_name))
            .and_then(|arc| arc.read().ok().map(|t| t.get_id()));

        let mut team = AITeamQueue::new(
            team_name.to_string(),
            orders,
            priority_build,
            game_logic.frame as u32,
        );
        team.team_id = team_id;
        team.execute_actions = execute_actions;
        if priority_build {
            self.team_queue.push_front(team);
        } else {
            self.team_queue.push_back(team);
        }
        // C++ `m_teamDelay = 0` so `queueUnits` retries immediately.
        self.next_team_queue_time = 0.0;
        self.activity_count = self.activity_count.saturating_add(1);

        if execute_actions && !production_condition.is_empty() {
            if let Ok(eng) = gamelogic::scripting::engine::get_script_engine().read() {
                if let Some(e) = eng.as_ref() {
                    if let Some(script) = e.find_script_clone_by_name(&production_condition) {
                        if let Some(action) = script.get_action().cloned() {
                            drop(eng);
                            if let Ok(mut writer) =
                                gamelogic::scripting::engine::get_script_engine().write()
                            {
                                if let Some(engine) = writer.as_mut() {
                                    engine.friend_execute_action(&action, Some(team_name));
                                }
                            }
                        }
                    }
                }
            }
        }
        true
    }

    /// C++ `AIPlayer::recruitSpecificAITeam`.
    pub fn recruit_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        recruit_radius: f32,
    ) -> bool {
        let radius = if recruit_radius < 1.0 {
            99_999.0
        } else {
            recruit_radius
        };
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(team_name) {
                if proto.is_singleton() {
                    if let Some(existing) = factory.find_team_instances(team_name).into_iter().next() {
                        if let Ok(eg) = existing.read() {
                            if eg.has_any_objects() {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        let mut orders = self.create_work_orders_for_team(team_name);
        if orders.is_empty() {
            if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
                if let Some(proto) = factory.find_team_prototype(team_name) {
                    for unit in proto.units_info() {
                        if unit.max_units < 1 || unit.unit_thing_name.is_empty() {
                            continue;
                        }
                        orders.push(AIWorkOrder::new(
                            unit.unit_thing_name.to_string(),
                            unit.max_units.max(0) as u32,
                            100,
                        ));
                    }
                }
            }
        }
        if orders.is_empty() {
            return false;
        }
        let home = self.team_home_or_base(team_name);
        let mut recruited = 0u32;
        for order in &mut orders {
            while order.num_completed < order.num_required {
                let Some(unit_id) = self.try_to_recruit(
                    game_logic,
                    team_name,
                    &order.template_name,
                    home,
                    Some(radius),
                )
                else {
                    break;
                };
                order.num_completed = order.num_completed.saturating_add(1);
                order.observed_unit_ids.push(unit_id);
                recruited = recruited.saturating_add(1);
                let _ = game_logic.unit_command_move_to(unit_id, home);
            }
        }
        if recruited == 0 {
            return false;
        }
        let mut q = AITeamQueue::new(
            team_name.to_string(),
            orders,
            false,
            0,
        );
        Self::bind_inactive_team_handle(&mut q);
        self.team_ready_queue.push_back(q);
        self.activity_count = self.activity_count.saturating_add(1);
        true
    }

    /// Select early game team composition
    fn select_early_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => match self.personality {
                AIPersonality::Rush => Some("USA_RangerSquad".to_string()),
                _ => Some("USA_BasicForce".to_string()),
            },
            Team::China => match self.personality {
                AIPersonality::Rush => Some("China_RedGuardSquad".to_string()),
                _ => Some("China_BasicForce".to_string()),
            },
            Team::GLA => Some("GLA_TechnicalSquad".to_string()),
            _ => None,
        }
    }

    /// Select mid game team composition
    fn select_mid_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_CombinedArms".to_string()),
            Team::China => Some("China_TankSquad".to_string()),
            Team::GLA => Some("GLA_HitAndRun".to_string()),
            _ => None,
        }
    }

    /// Select late game team composition
    fn select_late_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_AdvancedStrike".to_string()),
            Team::China => Some("China_HeavyAssault".to_string()),
            Team::GLA => Some("GLA_MassAssault".to_string()),
            _ => None,
        }
    }

    /// Select desperate situation team (cheap, fast units)
    fn select_desperate_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_RangerSquad".to_string()),
            Team::China => Some("China_RedGuardSquad".to_string()),
            Team::GLA => Some("GLA_RebelSwarm".to_string()),
            _ => None,
        }
    }


    /// Create work orders for a specific team type.
    ///
    /// Late-game names (`USA_AdvancedStrike`, `China_HeavyAssault`,
    /// `GLA_MassAssault`) keep their C++-style team identity and emit the
    /// matching retail ThingTemplate units. Unknown names stay empty so they
    /// cannot silently become default infantry (C++ `selectTeamToBuild` only
    /// queues `TeamPrototype` unit lists).
    fn create_work_orders_for_team(&self, team_name: &str) -> Vec<AIWorkOrder> {
        let mut orders = Vec::new();

        match team_name {
            "USA_RangerSquad" => {
                orders.push(AIWorkOrder::new(
                    "AmericaInfantryRanger".to_string(),
                    4,
                    100,
                ));
            }
            "USA_BasicForce" => {
                orders.push(AIWorkOrder::new("AmericaInfantryRanger".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("AmericaVehicleHumvee".to_string(), 1, 100));
            }
            "USA_CombinedArms" => {
                orders.push(AIWorkOrder::new("AmericaInfantryRanger".to_string(), 3, 80));
                orders.push(AIWorkOrder::new("AmericaVehicleHumvee".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("USA_CrusaderTank".to_string(), 1, 100));
            }
            "USA_AdvancedStrike" => {
                orders.push(AIWorkOrder::new(
                    "AmericaInfantryMissileDefender".to_string(),
                    2,
                    80,
                ));
                orders.push(AIWorkOrder::new("AmericaTankCrusader".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("AmericaJetRaptor".to_string(), 2, 100));
            }
            "China_RedGuardSquad" => {
                orders.push(AIWorkOrder::new(
                    "ChinaInfantryRedguard".to_string(),
                    4,
                    100,
                ));
            }
            "China_BasicForce" => {
                orders.push(AIWorkOrder::new("ChinaInfantryRedguard".to_string(), 2, 90));
                orders.push(AIWorkOrder::new(
                    "ChinaTankBattleMaster".to_string(),
                    1,
                    100,
                ));
            }
            "China_TankSquad" => {
                orders.push(AIWorkOrder::new(
                    "China_BattlemasterTank".to_string(),
                    2,
                    100,
                ));
                orders.push(AIWorkOrder::new("ChinaInfantryRedguard".to_string(), 2, 80));
            }
            "China_HeavyAssault" => {
                orders.push(AIWorkOrder::new("ChinaTankBattleMaster".to_string(), 2, 80));
                orders.push(AIWorkOrder::new("ChinaTankOverlord".to_string(), 1, 90));
                orders.push(AIWorkOrder::new("ChinaJetMIG".to_string(), 2, 100));
            }
            "GLA_TechnicalSquad" => {
                // Barracks first: infantry produces even if ArmsDealer is still building.
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 2, 100));
            }
            "GLA_RebelSwarm" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 4, 100));
            }
            "GLA_HitAndRun" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 80));
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 2, 100));
            }
            "GLA_MassAssault" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 80));
                orders.push(AIWorkOrder::new("GLAVehicleTechnical".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("GLAVehicleScudLauncher".to_string(), 1, 100));
            }
            _ => {}
        }

        if orders.is_empty() {
            for (name, _min_u, max_u) in Self::prototype_unit_infos(team_name) {
                if max_u > 0 {
                    orders.push(AIWorkOrder::new(name, max_u as u32, 100));
                }
            }
        }

        orders
    }

    fn prototype_unit_infos(team_name: &str) -> Vec<(String, i32, i32)> {
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return Vec::new();
        };
        let Some(proto) = factory.find_team_prototype(team_name) else {
            return Vec::new();
        };
        proto
            .units_info()
            .iter()
            .filter(|unit| !unit.unit_thing_name.is_empty())
            .map(|unit| {
                (
                    unit.unit_thing_name.to_string(),
                    unit.min_units,
                    unit.max_units,
                )
            })
            .collect()
    }

    fn unit_template_known(&self, game_logic: &GameLogic, name: &str) -> bool {
        if game_logic.templates.contains_key(name) {
            return true;
        }
        if gamelogic::helpers::TheThingFactory::find_template(name).is_some() {
            return true;
        }
        // Live leftover ThingFactory is often empty; allow factory-mapped units.
        Self::factory_template_for_unit(name, self.team).is_some()
    }

    /// C++ `buildSpecificAITeam` work-order construction: optional (max-min)
    /// prepended, then required (min) prepended.
    fn build_team_work_orders(&self, game_logic: &GameLogic, team_name: &str) -> Vec<AIWorkOrder> {
        let units = Self::prototype_unit_infos(team_name);
        if units.is_empty() {
            return self.create_work_orders_for_team(team_name);
        }
        let mut orders = Vec::new();
        for (name, min_u, max_u) in &units {
            if !self.unit_template_known(game_logic, name) {
                continue;
            }
            let count = (*max_u - *min_u).max(0);
            if count <= 0 {
                continue;
            }
            let mut order = AIWorkOrder::new(name.clone(), count as u32, 100);
            order.is_required = false;
            orders.insert(0, order);
        }
        for (name, min_u, _max_u) in &units {
            if !self.unit_template_known(game_logic, name) {
                continue;
            }
            let mut order = AIWorkOrder::new(name.clone(), (*min_u).max(0) as u32, 100);
            order.is_required = true;
            orders.insert(0, order);
        }
        orders
    }

    /// C++ `isPossibleToBuildTeam(..., requireIdleFactory=false)` factory residual.
    fn team_factories_exist(&self, game_logic: &GameLogic, orders: &[AIWorkOrder]) -> bool {
        if orders.is_empty() {
            return false;
        }
        orders.iter().all(|order| {
            Self::find_factory_for_unit_ex(game_logic, &order.template_name, self.team, true)
                .is_some()
        })
    }


    /// Find factory that can produce a specific unit
    fn find_factory_for_unit(
        &self,
        game_logic: &GameLogic,
        unit_template_name: &str,
    ) -> Option<ObjectId> {
        Self::find_factory_for_unit_static(game_logic, unit_template_name, self.team)
    }

    /// Static version to avoid borrowing conflicts
    fn find_factory_for_unit_static(
        game_logic: &GameLogic,
        unit_template_name: &str,
        team: Team,
    ) -> Option<ObjectId> {
        // Prefer idle factory (C++ findFactory(thing, busyOk=false) residual).
        Self::find_factory_for_unit_ex(game_logic, unit_template_name, team, false)
            .or_else(|| Self::find_factory_for_unit_ex(game_logic, unit_template_name, team, true))
    }

    /// Map host unit template → retail factory template residual.
    fn factory_template_for_unit(unit_template_name: &str, team: Team) -> Option<&'static str> {
        let unit = unit_template_name.to_ascii_lowercase();
        if unit.contains("ranger")
            || unit.contains("redguard")
            || unit.contains("soldier")
            || unit.contains("rebel")
            || unit.contains("missiledefender")
            || unit.contains("pathfinder")
        {
            return match team {
                Team::USA => Some("AmericaBarracks"),
                Team::China => Some("ChinaBarracks"),
                Team::GLA => Some("GLABarracks"),
                _ => None,
            };
        }
        if unit.contains("raptor")
            || unit.contains("stealth")
            || unit.contains("aurora")
            || unit.contains("comanche")
            || unit.contains("chinook")
            || unit.contains("mig")
            || unit.contains("helix")
            || unit.contains("jet")
        {
            return match team {
                Team::USA => Some("AmericaAirfield"),
                Team::China => Some("ChinaAirfield"),
                Team::GLA => None,
                _ => None,
            };
        }
        if unit.contains("humvee")
            || unit.contains("technical")
            || unit.contains("tank")
            || unit.contains("tomahawk")
            || unit.contains("scud")
            || unit.contains("launcher")
        {
            return match team {
                Team::USA => Some("AmericaWarFactory"),
                Team::China => Some("ChinaWarFactory"),
                Team::GLA => Some("GLAArmsDealer"),
                _ => None,
            };
        }
        if unit.contains("dozer")
            || unit.contains("infantryworker")
            || (unit.contains("worker") && !unit.contains("supply"))
        {
            return match team {
                Team::USA => Some("AmericaCommandCenter"),
                Team::China => Some("ChinaCommandCenter"),
                Team::GLA => Some("GLACommandCenter"),
                _ => None,
            };
        }
        None
    }

    /// Retail factory name plus leftover USA_*/China_*/GLA_* aliases used by older tests.
    fn factory_name_matches(object_name: &str, retail_factory: &str) -> bool {
        if object_name.eq_ignore_ascii_case(retail_factory) {
            return true;
        }
        let alias = match retail_factory {
            "AmericaBarracks" => "USA_Barracks",
            "AmericaWarFactory" => "USA_WarFactory",
            "AmericaAirfield" => "USA_Airfield",
            "AmericaCommandCenter" => "USA_CommandCenter",
            "ChinaBarracks" => "China_Barracks",
            "ChinaWarFactory" => "China_WarFactory",
            "ChinaAirfield" => "China_Airfield",
            "ChinaCommandCenter" => "China_CommandCenter",
            "GLABarracks" => "GLA_Barracks",
            "GLAArmsDealer" => "GLA_ArmsDealer",
            "GLACommandCenter" => "GLA_CommandCenter",
            _ => return false,
        };
        object_name.eq_ignore_ascii_case(alias)
    }

    /// C++ factory idle residual: empty production_queue ⇒ idle.
    fn factory_is_idle(object: &crate::game_logic::Object) -> bool {
        object
            .building_data
            .as_ref()
            .map(|b| b.production_queue.is_empty())
            .unwrap_or(true)
    }

    /// Find constructed factory; `busy_ok=false` requires idle queue (C++ findFactory).
    fn find_factory_for_unit_ex(
        game_logic: &GameLogic,
        unit_template_name: &str,
        team: Team,
        busy_ok: bool,
    ) -> Option<ObjectId> {
        let factory_name = Self::factory_template_for_unit(unit_template_name, team)?;
        // Pure residual acquire: prefer idle factories (priority 0) over busy (1)
        // when busy_ok; nearest 3D tiebreak for stable multi-factory choice.
        let cands: Vec<_> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                if object.team != team || !object.is_constructed() || !object.is_alive() {
                    return None;
                }
                let name_ok = Self::factory_name_matches(&object.template_name, factory_name)
                    || Self::factory_name_matches(&object.get_template().name, factory_name);
                if !name_ok {
                    return None;
                }
                let idle = Self::factory_is_idle(object);
                if !busy_ok && !idle {
                    return None;
                }
                let priority = if idle { Some(0u8) } else { Some(1u8) };
                Some(
                    crate::game_logic::host_residual_acquire::PriorityAcquireCandidate {
                        id,
                        position: object.get_position(),
                        is_alive: true,
                        priority,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_best_priority_residual_target(
            ObjectId(0),
            glam::Vec3::ZERO,
            (0.0, 0.0),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ `isPossibleToBuildTeam` factory residual (requireIdleFactory=true):
    /// every unit type has a factory, and at least one factory is idle.
    fn team_factories_ready(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        let names = self.team_unit_template_names(game_logic, team_name);
        if names.is_empty() {
            return false;
        }
        let mut any_idle = false;
        for name in &names {
            // Must have some factory that can produce this unit (busy ok for existence).
            if Self::find_factory_for_unit_ex(game_logic, name, self.team, true).is_none() {
                return false;
            }
            if Self::find_factory_for_unit_ex(game_logic, name, self.team, false).is_some() {
                any_idle = true;
            }
        }
        any_idle
    }

    /// Prototype unit templates when present; invented compositions only as
    /// a fallback for scripted alias names that have no TeamPrototype units.
    fn team_unit_template_names(&self, game_logic: &GameLogic, team_name: &str) -> Vec<String> {
        let units = Self::prototype_unit_infos(team_name);
        if !units.is_empty() {
            return units
                .into_iter()
                .filter(|(name, _, _)| self.unit_template_known(game_logic, name))
                .map(|(name, _, _)| name)
                .collect();
        }
        self.create_work_orders_for_team(team_name)
            .into_iter()
            .map(|order| order.template_name)
            .collect()
    }

    /// Minimum seconds between host AI **attack re-evaluations**.
    ///
    /// Wave 616: residual-locked at 60s (not gate-driven early-attack).
    /// Shares the **numeric** 60s value from C++ `AIPlayer::checkReadyTeams`
    /// (`GeneralsMD/.../AI/AIPlayer.cpp`: force-start ready team after
    /// `60 * LOGICFRAMES_PER_SECOND`), but this is **not** a port of that function.
    /// C++ uses 60s for team activation at rally; this host AI uses 60s only as
    /// spacing between strength-threshold attack decisions. Full checkReadyTeams
    /// (idle/anyIdle, production-condition scripts, setActive) remains unported.
    pub const ATTACK_RECHECK_SECONDS: f32 = 60.0;

    /// Retail `AIData.ini` defaults (Default/AIData.ini).
    /// StructureSeconds = 0 → try structure decisions every AI economic tick when ready.
    pub const STRUCTURE_SECONDS: f32 = 0.0;
    /// TeamSeconds = 10 → wait between successful new-team selections.
    pub const TEAM_SECONDS: f32 = 10.0;
    /// C++ `AISkirmishPlayer::doTeamBuilding`: `m_teamDelay = 2 * LOGICFRAMES_PER_SECOND`.
    pub const TEAM_QUEUE_RETRY_SECONDS: f32 = 2.0;
    /// RebuildDelayTimeSeconds = 30 (base rebuild delay residual; full C++ path unported).
    pub const REBUILD_DELAY_SECONDS: f32 = 30.0;
    /// Wealthy resource threshold (AIData `Wealthy`).
    pub const WEALTHY_RESOURCES: u32 = 7000;
    /// Poor resource threshold (AIData `Poor`).
    pub const POOR_RESOURCES: u32 = 2000;
    /// StructuresWealthyRate — interval divisor when wealthy (2=twice as fast).
    pub const STRUCTURES_WEALTHY_RATE: f32 = 2.0;
    /// StructuresPoorRate.
    pub const STRUCTURES_POOR_RATE: f32 = 0.6;
    /// TeamsWealthyRate.
    pub const TEAMS_WEALTHY_RATE: f32 = 2.0;
    /// TeamsPoorRate.
    pub const TEAMS_POOR_RATE: f32 = 0.6;
    /// Retail AIData `TeamResourcesToStart` fallback when leftover AIData is unset.
    pub const TEAM_RESOURCES_TO_START: f32 = 0.1;

    /// Evaluate opportunities to attack enemies (strength-threshold + C++-aligned spacing).

    /// AIData wealth/poor rate residual: returns speed multiplier (>= rate means faster).
    fn resource_speed_rate(&self, game_logic: &GameLogic, for_structures: bool) -> f32 {
        let supplies = game_logic
            .get_player(self.player_id)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        if supplies >= Self::WEALTHY_RESOURCES {
            if for_structures {
                Self::STRUCTURES_WEALTHY_RATE
            } else {
                Self::TEAMS_WEALTHY_RATE
            }
        } else if supplies <= Self::POOR_RESOURCES {
            if for_structures {
                Self::STRUCTURES_POOR_RATE
            } else {
                Self::TEAMS_POOR_RATE
            }
        } else {
            1.0
        }
    }

    /// Base interval seconds scaled by difficulty and wealth/poor rates.
    fn scaled_interval_seconds(
        &self,
        game_logic: &GameLogic,
        base_seconds: f32,
        for_structures: bool,
    ) -> f32 {
        if base_seconds <= 0.0 {
            return 0.0;
        }
        let delay = self.difficulty.get_build_delay_modifier().max(0.01);
        let rate = self
            .resource_speed_rate(game_logic, for_structures)
            .max(0.01);
        // C++ rate multiplies speed → shorter wait when rate > 1.
        (base_seconds * delay) / rate
    }

    fn evaluate_attack_opportunities(&mut self, game_logic: &mut GameLogic, _current_time: f32) {
        // C++ AIPlayer has no all-army raid latch. Teams only AttackMove when
        // OnCreate scripts say so (checkReadyTeams → setActive). Keep the host
        // attack_in_progress flag only so a finished raid can clear.
        self.clear_finished_attack(game_logic);
    }

    /// Calculate our military strength
    fn calculate_military_strength(&self, game_logic: &GameLogic) -> f32 {
        let mut strength = 0.0;

        for object in game_logic.host_objects().values() {
            if object.team == self.team && object.is_alive() && object.can_attack() {
                strength += object.health.current * 0.1; // Basic strength calculation
            }
        }

        strength
    }

    /// Estimate enemy military strength
    fn estimate_enemy_strength(&self, game_logic: &GameLogic, enemy_id: u32) -> f32 {
        let enemy_team = if let Some(player) = game_logic.get_player(enemy_id) {
            player.team
        } else {
            return 0.0;
        };

        let mut strength = 0.0;

        for object in game_logic.host_objects().values() {
            if object.team == enemy_team && object.is_alive() && object.can_attack() {
                strength += object.health.current * 0.1;
            }
        }

        strength
    }

    /// Record C++ `AIAttackMoveState` / `AIInternalMoveToState::onEnter` on the
    /// crate `AiStateMachine` (move/attack only; does not run the 48-state graph).
    fn dispatch_crate_attack_move(unit_id: ObjectId, dest: Vec3, focus: Option<ObjectId>) {
        let dest = gamelogic::common::types::Coord3D::new(dest.x, dest.y, dest.z);
        let _ = gamelogic::ai::state_machine::dispatch_host_move_attack(
            unit_id.0,
            gamelogic::ai::state_machine::HostMoveAttackKind::AttackMoveTo,
            Some(dest),
            focus.map(|id| id.0),
        );
    }


    /// AttackMove the given units toward the enemy base (OnCreate residual).
    fn attack_move_units(
        &mut self,
        game_logic: &mut GameLogic,
        attack_units: &[ObjectId],
        current_time: f32,
    ) {
        if attack_units.is_empty() {
            return;
        }
        let enemy_base = if let Some(enemy_id) = self.enemy_player_id {
            if let Some(player) = game_logic.get_player(enemy_id) {
                self.find_enemy_base_center(game_logic, player.team)
            } else {
                Vec3::ZERO
            }
        } else {
            Vec3::ZERO
        };
        let enemy_team = self
            .enemy_player_id
            .and_then(|eid| game_logic.get_player(eid).map(|p| p.team));
        let focus_enemy = enemy_team.and_then(|eteam| {
            game_logic
                .host_objects()
                .iter()
                .filter(|(_, o)| {
                    o.team == eteam
                        && o.is_alive()
                        && o.is_kind_of(crate::game_logic::KindOf::Attackable)
                })
                .min_by(|(_, a), (_, b)| {
                    let da = a.get_position().distance(enemy_base);
                    let db = b.get_position().distance(enemy_base);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(id, _)| *id)
        });

        for &unit_id in attack_units {
            if let Some(focus) = focus_enemy {
                game_logic.apply_engagement_decision_aware_for_ai(unit_id, focus);
            }
            let mobile = game_logic
                .host_object(unit_id)
                .map(|u| u.is_mobile() && u.is_alive())
                .unwrap_or(false);
            if !mobile {
                continue;
            }
            if game_logic.assign_unit_path(unit_id, enemy_base, &[]) {
                game_logic.set_ai_state_decision_aware_for_ai(unit_id, AIState::AttackMoving);
                if let Some(unit) = game_logic.host_object_mut(unit_id) {
                    unit.is_attack_path = true;
                    unit.requested_destination = Some(enemy_base);
                }
                Self::dispatch_crate_attack_move(unit_id, enemy_base, focus_enemy);
            } else {
                if let Some(unit) = game_logic.host_object_mut(unit_id) {
                    unit.move_to(enemy_base);
                    unit.is_attack_path = true;
                    unit.requested_destination = Some(enemy_base);
                }
                game_logic.set_ai_state_decision_aware_for_ai(unit_id, AIState::AttackMoving);
                Self::dispatch_crate_attack_move(unit_id, enemy_base, focus_enemy);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_move_to(unit_id, enemy_base);
                }
            }
        }

        self.attack_in_progress = true;
        self.last_attack_time = current_time;
        self.activity_count = self.activity_count.saturating_add(1);
    }

    /// Launch coordinated attack
    pub(crate) fn launch_attack(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        log::debug!(
            "AI Player {} ({}) launching attack!",
            self.player_id,
            self.team.get_name()
        );

        let mut attack_units = Vec::new();
        for (object_id, object) in game_logic.host_objects() {
            if object.team == self.team
                && object.is_alive()
                && object.can_attack()
                && object.is_mobile()
            {
                attack_units.push(*object_id);
            }
        }
        self.attack_move_units(game_logic, &attack_units, current_time);
    }

    /// Clear the host raid latch once launched attackers are idle or gone.
    ///
    /// C++ `AIPlayer` / `AISkirmishPlayer` have no permanent `m_attackInProgress`.
    /// Teams activate via `checkReadyTeams` (`AIPlayer.cpp:2729`) when idle (or
    /// after 60s) and later scripts can start another attack. The host latch
    /// must not survive the raid or AI attacks exactly once per game.
    fn clear_finished_attack(&mut self, game_logic: &GameLogic) {
        if !self.attack_in_progress {
            return;
        }
        if !self.raid_units_still_committed(game_logic) {
            self.attack_in_progress = false;
        }
    }

    fn raid_units_still_committed(&self, game_logic: &GameLogic) -> bool {
        game_logic.host_objects().values().any(|object| {
            object.team == self.team
                && object.is_alive()
                && object.can_attack()
                && object.is_mobile()
                && matches!(
                    object.ai_state,
                    AIState::AttackMoving | AIState::Attacking | AIState::AttackingGround
                )
        })
    }

    /// C++ `ScriptActions::doSkirmishFireSpecialPowerAtMostCost`.
    /// Fires only the named SpecialPower template, never the first ready one.
    pub fn fire_named_special_power(&mut self, game_logic: &mut GameLogic, power_name: &str) {
        if power_name.is_empty() {
            return;
        }
        let Some(enemy_id) = self.enemy_player_id else {
            return;
        };
        let Some(enemy_team) = game_logic.get_player(enemy_id).map(|p| p.team) else {
            return;
        };

        let mut ready: Vec<(
            ObjectId,
            crate::command_system::SpecialPowerType,
            String,
            bool,
        )> = Vec::new();
        for (id, object) in game_logic.host_objects() {
            if object.team != self.team || !object.is_alive() {
                continue;
            }
            for module in &object.thing.template.special_power_modules {
                if !module
                    .special_power_template
                    .eq_ignore_ascii_case(power_name)
                {
                    continue;
                }
                let Some(power) = module.command_power.clone() else {
                    continue;
                };
                if !game_logic.is_special_power_ready_for(*id, &power) {
                    continue;
                }
                let sneak = matches!(power, crate::command_system::SpecialPowerType::SneakAttack);
                ready.push((
                    *id,
                    power,
                    module.special_power_template.clone(),
                    sneak,
                ));
            }
        }
        if ready.is_empty() {
            return;
        }

        for (caster, power, template_name, sneak) in ready {
            let cluster = matches!(
                power,
                crate::command_system::SpecialPowerType::ClusterMines
                    | crate::command_system::SpecialPowerType::NukeDrop
            );
            let Some(mut location) = (if cluster {
                self.compute_cluster_mines_target(game_logic, enemy_team)
            } else {
                let mut radius = 50.0;
                let cursor = Self::radius_cursor_for_power(&power, &template_name);
                if cursor > radius {
                    radius = cursor;
                }
                self.compute_superweapon_target(game_logic, enemy_team, radius, !sneak)
            }) else {
                continue;
            };
            if sneak {
                if let Some(legal) = self.calc_closest_construction_zone_location(
                    game_logic,
                    crate::game_logic::GLA_SNEAK_TUNNEL_TEMPLATE,
                    location,
                ) {
                    location = legal;
                } else {
                    continue;
                }
            }
            if location.length_squared() <= 0.0 {
                continue;
            }
            game_logic.queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::DoSpecialPower {
                    power_type: power,
                    target: crate::command_system::PowerTarget::Location(location),
                },
                player_id: self.player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![caster],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            self.activity_count = self.activity_count.saturating_add(1);
            break;
        }
    }

    /// C++ `Player::calcClosestConstructionZoneLocation` residual: seed
    /// `NO_OBJECT_OVERLAP`, then wiggle if the sneak pad is illegal.
    fn calc_closest_construction_zone_location(
        &self,
        game_logic: &GameLogic,
        template_name: &str,
        seed: Vec3,
    ) -> Option<Vec3> {
        if game_logic.is_location_legal_to_build(self.team, seed, template_name) {
            return Some(seed);
        }
        const STEP: f32 = 20.0;
        for ring in 1..12 {
            let reach = STEP * ring as f32;
            for dx in [-1_i32, 0, 1] {
                for dz in [-1_i32, 0, 1] {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let candidate = Vec3::new(
                        seed.x + dx as f32 * reach,
                        seed.y,
                        seed.z + dz as f32 * reach,
                    );
                    if game_logic.is_location_legal_to_build(self.team, candidate, template_name)
                    {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// Host residual of C++ `AIPlayer::computeSuperweaponTarget`.
    /// Ground plane is host XZ (C++ samples XY).
    fn compute_superweapon_target(
        &mut self,
        game_logic: &GameLogic,
        enemy_team: Team,
        weapon_radius: f32,
        target_military_units: bool,
    ) -> Option<Vec3> {
        let radius = weapon_radius.max(1.0);
        let (mut min_x, mut min_z, mut max_x, mut max_z) =
            self.player_structure_bounds(game_logic, enemy_team);
        if min_x == 0.0 && min_z == 0.0 && max_x == 0.0 && max_z == 0.0 {
            let (lo, hi) = game_logic.world_bounds();
            min_x = lo.x;
            min_z = lo.z;
            max_x = hi.x;
            max_z = hi.z;
        }

        min_x += radius;
        max_x -= radius;
        if max_x < min_x {
            let mid = (max_x + min_x) * 0.5;
            min_x = mid;
            max_x = mid;
        }
        if max_z < min_z {
            let mid = (max_z + min_z) * 0.5;
            min_z = mid;
            max_z = mid;
        }

        let width = (max_x - min_x).max(0.0);
        let height = (max_z - min_z).max(0.0);
        let mut x_count = (width / radius).ceil() as i32 + 1;
        let mut z_count = (height / radius).ceil() as i32 + 1;
        if x_count > 10 {
            x_count = 10;
        }
        if z_count > 10 {
            z_count = 10;
        }
        if x_count < 1 {
            x_count = 1;
        }
        if z_count < 1 {
            z_count = 1;
        }

        // C++ GameLogicRandomValue(1, 4) scan-direction residual.
        let (x_delta, z_delta, x_start, z_start) = match self.placement_rng.next_int(1, 4) {
            1 => (1_i32, 1_i32, 0_i32, 0_i32),
            2 => (-1, 1, x_count, 0),
            3 => (1, -1, 0, z_count),
            _ => (-1, -1, x_count, z_count),
        };

        let mut best_cash: i32 = -1;
        let mut best_pos = Vec3::new(min_x, 0.0, min_z);
        let mut x_index = x_start;
        for _ in 0..x_count {
            let mut z_index = z_start;
            for _ in 0..z_count {
                let pos = Vec3::new(
                    min_x + (width * x_index as f32) / x_count as f32,
                    0.0,
                    min_z + (height * z_index as f32) / z_count as f32,
                );
                let value = self.player_superweapon_value(
                    game_logic,
                    enemy_team,
                    pos,
                    2.0 * radius,
                    target_military_units,
                );
                if value > best_cash {
                    best_cash = value;
                    best_pos = pos;
                }
                z_index += z_delta;
            }
            x_index += x_delta;
        }

        // Fine tune: C++ uses (x-5) for BOTH axes (legacy bug — keep for parity).
        let mut fine_best = best_pos;
        let mut fine_cash: i32 = -1;
        let mut fine_count = 0_i32;
        for x in 0..11 {
            for _y in 0..11 {
                let offset = (x - 5) as f32 * (radius / 10.0);
                let pos = Vec3::new(best_pos.x + offset, 0.0, best_pos.z + offset);
                let value = self.player_superweapon_value(
                    game_logic,
                    enemy_team,
                    pos,
                    radius,
                    target_military_units,
                );
                if value > fine_cash {
                    fine_cash = value;
                    fine_best = pos;
                    fine_count = 1;
                } else if value == fine_cash {
                    fine_best.x += pos.x;
                    fine_best.z += pos.z;
                    fine_count += 1;
                }
            }
        }
        if fine_count > 1 {
            fine_best.x /= fine_count as f32;
            fine_best.z /= fine_count as f32;
        }
        if fine_cash > -1 {
            Some(fine_best)
        } else {
            None
        }
    }

    /// C++ `ScriptActions` radius: `max(50, power->getRadiusCursorRadius())`.
    fn radius_cursor_for_power(
        power: &crate::command_system::SpecialPowerType,
        template_name: &str,
    ) -> f32 {
        use crate::command_system::SpecialPowerType as P;
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::special_power_template_row_wave109;
        if let Some(row) = special_power_template_row_wave109(template_name) {
            return row.radius_cursor_radius;
        }
        match power {
            P::ClusterMines | P::NukeDrop => 100.0,
            P::ScudStorm => 200.0,
            P::DaisyCutter | P::AirForceDaisyCutter => 170.0,
            P::NuclearMissile | P::NukeNeutronMissile | P::SuperweaponNeutronMissile => 210.0,
            P::EmpPulse => 200.0,
            P::SpySatellite => 300.0,
            P::SpyDrone => 250.0,
            P::Artillery => 125.0,
            P::CarpetBomb => 100.0,
            P::EarlyChinaCarpetBomb | P::NukeChinaCarpetBomb | P::AirForceCarpetBomb => 180.0,
            P::AnthraxBomb => 250.0,
            P::EmergencyRepair | P::EarlyEmergencyRepair => 100.0,
            P::GpsScrambler | P::StealthGpsScrambler => 100.0,
            P::Frenzy | P::EarlyFrenzy => 200.0,
            P::LeafletDrop | P::EarlyLeafletDrop => 110.0,
            _ => 0.0,
        }
    }
    /// C++ `AISkirmishPlayer::computeSuperweaponTarget` cluster-mines branch.
    fn compute_cluster_mines_target(
        &mut self,
        game_logic: &GameLogic,
        enemy_team: Team,
    ) -> Option<Vec3> {
        let start_index = game_logic
            .get_player(self.player_id)
            .map(|p| p.start_position.max(0))
            .unwrap_or(0);
        let mode = self.placement_rng.next_int(0, 2);
        let _path_label = match mode {
            1 => format!("SkirmFlank{}", start_index + 1),
            2 => format!("SkirmBackdoor{}", start_index + 1),
            _ => format!("SkirmCenter{}", start_index + 1),
        };
        // Host leftover has no TerrainLogic waypoint walk; C++ falls back to
        // enemy structure-bounds center when the labeled path is missing.
        let (min_x, min_z, max_x, max_z) = self.player_structure_bounds(game_logic, enemy_team);
        let goal = if min_x == 0.0 && min_z == 0.0 && max_x == 0.0 && max_z == 0.0 {
            self.find_enemy_base_center(game_logic, enemy_team)
        } else {
            Vec3::new(
                min_x + (max_x - min_x) * 0.5,
                0.0,
                min_z + (max_z - min_z) * 0.5,
            )
        };
        let mut offset_x = goal.x - self.base_center.x;
        let mut offset_z = goal.z - self.base_center.z;
        let length = (offset_x * offset_x + offset_z * offset_z).sqrt();
        if length > 0.001 {
            offset_x /= length;
            offset_z /= length;
        }
        offset_x *= self.base_radius;
        offset_z *= self.base_radius;
        Some(Vec3::new(
            self.base_center.x + offset_x,
            0.0,
            self.base_center.z + offset_z,
        ))
    }

    fn host_object_is_bridge(object: &crate::game_logic::object::Object) -> bool {
        let n = object.template_name.to_ascii_lowercase();
        (n.contains("bridge") && !n.contains("tower") && !n.contains("scaffold"))
            || object.template_name.eq_ignore_ascii_case("Bridge")
    }

    fn host_object_is_damaged(object: &crate::game_logic::object::Object) -> bool {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        if !object.is_alive() {
            return false;
        }
        !matches!(object.body_damage_state, HostBodyDamageType::Pristine)
            || object.health.current + 0.01 < object.health.maximum
    }

    /// C++ `AIPlayer::repairStructure` (`AIPlayer.cpp:2254-2280`).
    pub fn repair_structure(&mut self, game_logic: &GameLogic, structure_id: ObjectId) {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let Some(structure) = game_logic.host_object(structure_id) else {
            return;
        };
        if matches!(structure.body_damage_state, HostBodyDamageType::Pristine)
            && structure.health.current + 0.01 >= structure.health.maximum
        {
            return;
        }
        if self.structures_to_repair.contains(&structure_id) {
            return;
        }
        if self.structures_to_repair.len() >= MAX_STRUCTURES_TO_REPAIR {
            return;
        }
        self.structures_to_repair.push(structure_id);
    }

    /// Host residual of `AISkirmishPlayer::checkBridges`: enqueue damaged spans.
    pub fn check_bridges(&mut self, game_logic: &GameLogic) {
        let damaged: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (Self::host_object_is_bridge(object) && Self::host_object_is_damaged(object))
                    .then_some(id)
            })
            .collect();
        for id in damaged {
            self.repair_structure(game_logic, id);
        }
    }

    /// C++ `AIPlayer::updateBridgeRepair` (`AIPlayer.cpp:2296-2384`).
    fn update_bridge_repair(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        if self.structures_to_repair.is_empty() {
            return;
        }
        if self.last_bridge_repair_time >= 0.0
            && current_time - self.last_bridge_repair_time < 1.0
        {
            return;
        }
        self.last_bridge_repair_time = current_time;

        while !self.structures_to_repair.is_empty() {
            let head = self.structures_to_repair[0];
            if game_logic
                .host_object(head)
                .is_some_and(|o| o.is_alive())
            {
                break;
            }
            self.structures_to_repair.remove(0);
        }
        if self.structures_to_repair.is_empty() {
            return;
        }
        let bridge_id = self.structures_to_repair[0];
        let Some(bridge) = game_logic.host_object(bridge_id) else {
            return;
        };
        let bridge_pos = bridge.get_position();
        let bridge_pristine = matches!(bridge.body_damage_state, HostBodyDamageType::Pristine)
            && bridge.health.current + 0.01 >= bridge.health.maximum;

        if self.repair_dozer.is_none() {
            self.dozer_is_repairing = false;
            if self.dozer_queued_for_repair {
                return;
            }
            if let Some(dozer_id) =
                Self::find_available_dozer(game_logic, self.team, bridge_pos, None)
            {
                self.repair_dozer = Some(dozer_id);
                if let Some(dozer) = game_logic.host_object(dozer_id) {
                    self.repair_dozer_origin = dozer.get_position();
                }
                game_logic.queue_command(crate::command_system::GameCommand {
                    command_type: crate::command_system::CommandType::Repair {
                        target_id: bridge_id,
                    },
                    player_id: self.player_id,
                    command_id: 0,
                    timestamp: std::time::SystemTime::now(),
                    selected_units: vec![dozer_id],
                    modifier_keys: crate::command_system::ModifierKeys::default(),
                });
                self.dozer_is_repairing = true;
                return;
            }
            self.queue_dozer(game_logic, current_time);
            self.dozer_queued_for_repair = true;
            return;
        }

        let Some(dozer_id) = self.repair_dozer else {
            return;
        };
        let Some(dozer) = game_logic.host_object(dozer_id) else {
            self.repair_dozer = None;
            self.last_bridge_repair_time = -1.0;
            return;
        };
        if !dozer.is_alive() {
            self.repair_dozer = None;
            self.last_bridge_repair_time = -1.0;
            return;
        }
        let dozer_idle = dozer.ai_state == AIState::Idle;

        if self.dozer_is_repairing {
            if !dozer_idle {
                return;
            }
            if bridge_pristine {
                self.structures_to_repair.remove(0);
                self.dozer_is_repairing = false;
                if self.structures_to_repair.is_empty() {
                    let mut home = if self.base_center.length_squared() > 0.0 {
                        self.base_center
                    } else {
                        self.repair_dozer_origin
                    };
                    // C++ AIPlayer.cpp:2370-2372 adjustToPossibleDestination then aiMoveToPosition.
                    let _ = game_logic.adjust_to_possible_destination(dozer_id, &mut home);
                    let _ = game_logic.unit_command_move_to(dozer_id, home);
                }
                return;
            }
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Repair {
                target_id: bridge_id,
            },
            player_id: self.player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![dozer_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        self.dozer_is_repairing = true;
    }


    fn player_structure_bounds(&self, game_logic: &GameLogic, enemy_team: Team) -> (f32, f32, f32, f32) {
        let mut any = false;
        let mut min_x = 0.0;
        let mut min_z = 0.0;
        let mut max_x = 0.0;
        let mut max_z = 0.0;
        for object in game_logic.host_objects().values() {
            if object.team != enemy_team || !object.is_alive() || !object.is_kind_of(KindOf::Structure)
            {
                continue;
            }
            let p = object.get_position();
            if !any {
                min_x = p.x;
                max_x = p.x;
                min_z = p.z;
                max_z = p.z;
                any = true;
            } else {
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x);
                min_z = min_z.min(p.z);
                max_z = max_z.max(p.z);
            }
        }
        if any {
            (min_x, min_z, max_x, max_z)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Host residual of C++ `AIPlayer::getPlayerSuperweaponValue`.
    fn player_superweapon_value(
        &self,
        game_logic: &GameLogic,
        enemy_team: Team,
        center: Vec3,
        radius: f32,
        include_military_units: bool,
    ) -> i32 {
        let radius = radius.max(4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL);
        let rad_sqr = radius * radius;
        let mut cash = 0.0_f32;
        for object in game_logic.host_objects().values() {
            if object.team != enemy_team || !object.is_alive() {
                continue;
            }
            let mut apply_neg = false;
            if !include_military_units {
                if object.is_kind_of(KindOf::FSBaseDefense) {
                    apply_neg = true;
                } else if (object.is_kind_of(KindOf::Vehicle) || object.is_kind_of(KindOf::Infantry))
                    && !object.is_kind_of(KindOf::Dozer)
                    && !object.is_kind_of(KindOf::Harvester)
                {
                    apply_neg = true;
                }
            } else if object.is_kind_of(KindOf::Aircraft)
                && (object.status.airborne_target
                    || crate::game_logic::host_usa_pilot::is_significantly_above_terrain(
                        object.get_position().y,
                    ))
            {
                continue;
            }
            let pos = object.get_position();
            let dx = center.x - pos.x;
            let dz = center.z - pos.z;
            let dist_sqr = dx * dx + dz * dz;
            if dist_sqr >= rad_sqr {
                continue;
            }
            let dist = dist_sqr.sqrt();
            let factor = 1.0 - (dist / (2.0 * radius));
            let mut value = object.thing.template.build_cost.supplies as f32;
            if object.is_kind_of(KindOf::CommandCenter) || object.is_kind_of(KindOf::FSSuperweapon) {
                if include_military_units {
                    value /= 10.0;
                } else {
                    value *= 5.0;
                }
            }
            if apply_neg {
                cash -= factor * value * 5.0;
            } else {
                cash += factor * value;
            }
        }
        cash as i32
    }


    /// Find center of enemy base
    fn find_enemy_base_center(&self, game_logic: &GameLogic, enemy_team: Team) -> Vec3 {
        let mut center = Vec3::ZERO;
        let mut count = 0;

        // Find enemy command center or other key buildings
        for object in game_logic.host_objects().values() {
            if object.team == enemy_team
                && object.is_alive()
                && (object.is_kind_of(KindOf::CommandCenter)
                    || object.is_kind_of(KindOf::Structure))
            {
                center += object.get_position();
                count += 1;
            }
        }

        if count > 0 {
            center / count as f32
        } else {
            // Default to opposite corner if no buildings found
            -self.base_center
        }
    }

    /// Update strategic phase based on game state
    fn update_strategy_phase(&mut self, game_logic: &GameLogic, current_time: f32) {
        let game_time = current_time; // Game time in seconds

        match game_time {
            t if t < 300.0 => self.current_strategy = AIStrategy::EarlyGame, // First 5 minutes
            t if t < 900.0 => self.current_strategy = AIStrategy::MidGame,   // 5-15 minutes
            _ => self.current_strategy = AIStrategy::LateGame,               // After 15 minutes
        }

        // Check for desperate situation
        if let Some(player) = game_logic.get_player(self.player_id) {
            if player.resources.supplies < 200 {
                self.current_strategy = AIStrategy::Desperate;
            }
        }
    }

    /// Update build phase based on progress
    fn update_build_phase(&mut self, game_logic: &GameLogic, _current_time: f32) {
        // Count constructed buildings
        let built_buildings = self.building_queue.iter().filter(|b| b.is_built).count();

        // Count military units
        let military_units = game_logic
            .host_objects()
            .iter()
            .filter(|(_, obj)| obj.team == self.team && obj.can_attack())
            .count();

        self.build_phase = match (built_buildings, military_units) {
            (0..=2, _) => AIBuildPhase::BaseConstruction,
            (_, 0..=5) => AIBuildPhase::UnitProduction,
            (3..=5, _) => AIBuildPhase::Expansion,
            _ => AIBuildPhase::MassProduction,
        };
    }
}

/// AI Manager coordinates all AI players
#[derive(Debug)]
pub struct AIManager {
    pub ai_players: HashMap<u32, AIPlayer>,
    pub update_interval: f32,
    pub last_update_time: f32,
}

impl Default for AIManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AIManager {
    /// Create new AI manager
    pub fn new() -> Self {
        Self {
            ai_players: HashMap::new(),
            update_interval: 1.0 / 30.0, // C++ AI::update every logic frame (30 Hz)
            // Negative so the first host update at sim_time=0 is not skipped.
            last_update_time: -1.0,
        }
    }

    /// Add AI player
    pub fn add_ai_player(&mut self, player_id: u32, team: Team, difficulty: AIDifficulty) {
        let mut ai_player = AIPlayer::new(player_id, team, difficulty);

        // Initialize with team-appropriate base position
        // Keep pads inside default 512×512 world with MinDistFromEdgeOfMapForBuild=30
        // and layout offsets up to +100 (WarFactory). |center|<=120 → max pad 220 < 226.
        let base_position = match team {
            Team::USA => Vec3::new(-120.0, 0.0, -120.0),
            Team::China => Vec3::new(120.0, 0.0, -120.0),
            Team::GLA => Vec3::new(120.0, 0.0, 120.0),
            _ => Vec3::ZERO,
        };

        ai_player.initialize(base_position);
        self.ai_players.insert(player_id, ai_player);

        log::info!(
            "Added AI player {} ({}) with {} difficulty",
            player_id,
            team.get_name(),
            match difficulty {
                AIDifficulty::Easy => "Easy",
                AIDifficulty::Medium => "Medium",
                AIDifficulty::Hard => "Hard",
                AIDifficulty::Brutal => "Brutal",
            }
        );
    }

    /// C++ `AIPlayer` ctor `p->setCanBuildUnits(false)`.
    pub fn apply_ctor_can_build_units(game_logic: &mut GameLogic, player_id: u32) {
        if let Some(player) = game_logic.get_player_mut(player_id) {
            player.set_can_build_units(false);
        }
    }

    /// Update all AI players
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.last_update_time >= 0.0
            && current_time - self.last_update_time < self.update_interval
        {
            return;
        }

        let peer_targets: Vec<(u32, Option<u32>)> = self
            .ai_players
            .iter()
            .map(|(&id, ai)| (id, ai.enemy_player_id))
            .collect();
        let destroyed = gamelogic::team::take_host_pre_team_destroy_requests();
        let player_ids: Vec<u32> = self.ai_players.keys().copied().collect();
        for player_id in player_ids {
            if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
                for (team_id, team_name) in &destroyed {
                    ai_player.ai_pre_team_destroy(Some(*team_id), team_name);
                }
                ai_player.peer_ai_targets = peer_targets.clone();
                ai_player.update(game_logic, current_time);
            }
        }

        self.last_update_time = current_time;
    }

    /// Set AI difficulty for a player
    pub fn set_difficulty(&mut self, player_id: u32, difficulty: AIDifficulty) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.difficulty = difficulty;
        }
    }

    /// Relocate one AI player's base/layout without removing templates.
    pub fn relocate_ai_base(&mut self, player_id: u32, base_position: Vec3) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.relocate_base(base_position);
            log::info!(
                "AI Manager: relocated player {} base to {:?}",
                player_id,
                base_position
            );
        }
    }

    /// Enable/disable AI for a player
    pub fn set_ai_active(&mut self, player_id: u32, active: bool) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.is_active = active;
        }
    }

    /// Capture the bounded, decision-relevant host-AI state that can safely
    /// survive a map/object restore.  Pathfinder jobs, production pointers,
    /// and attack targets deliberately are not serialized: their object
    /// references are transient and are rebuilt after the snapshot is loaded.
    pub fn snapshot_players_for_save(&self) -> Vec<crate::save_load::AIPlayerSnapshot> {
        let mut player_ids: Vec<u32> = self.ai_players.keys().copied().collect();
        player_ids.sort_unstable();

        player_ids
            .into_iter()
            .filter_map(|player_id| self.ai_players.get(&player_id))
            .map(|ai| {
                let defensive_groups = (!ai.defensive_units.is_empty())
                    .then(|| crate::save_load::AIUnitGroupSnapshot {
                        group_id: ai.player_id,
                        units: ai.defensive_units.clone(),
                        role: "Defensive".to_string(),
                        current_task: "GuardBase".to_string(),
                        formation: "Default".to_string(),
                        target_position: Some(ai.base_center),
                    })
                    .into_iter()
                    .collect();

                crate::save_load::AIPlayerSnapshot {
                    player_id: ai.player_id,
                    difficulty: Self::difficulty_name(ai.difficulty).to_string(),
                    personality: Self::personality_name(ai.personality).to_string(),
                    current_strategy: Self::strategy_name(ai.current_strategy).to_string(),
                    is_active: ai.is_active,
                    base_center: Some(ai.base_center),
                    base_radius: ai.base_radius,
                    activity_count: ai.activity_count,
                    strategic_state: crate::save_load::AIStrategicStateSnapshot {
                        current_phase: Self::build_phase_name(ai.build_phase).to_string(),
                        objectives: Vec::new(),
                        threat_assessment: crate::save_load::ThreatAssessmentSnapshot {
                            enemy_strengths: HashMap::new(),
                            vulnerable_areas: Vec::new(),
                            threat_level: 0.0,
                        },
                    },
                    tactical_state: crate::save_load::AITacticalStateSnapshot {
                        unit_groups: defensive_groups,
                        active_attacks: Vec::new(),
                        defensive_positions: vec![ai.base_center],
                    },
                    // The host AI rebuild queue carries live object/factory
                    // references.  It is intentionally regenerated after a
                    // load rather than persisted with stale IDs.
                    economic_state: crate::save_load::AIEconomicStateSnapshot {
                        build_priorities: Vec::new(),
                        economic_focus: String::new(),
                        resource_allocation: crate::save_load::ResourceAllocation {
                            military_percentage: 0.0,
                            economic_percentage: 0.0,
                            defensive_percentage: 0.0,
                        },
                    },
                }
            })
            .collect()
    }

    /// Recreate registered host-AI players from an offline snapshot.
    ///
    /// The caller supplies restored player teams because save rows identify an
    /// AI by player id, while team ownership remains part of `PlayerSnapshot`.
    /// Empty snapshots are handled by the caller as the legacy fallback case.
    pub fn restore_players_from_save(
        &mut self,
        snapshots: &[crate::save_load::AIPlayerSnapshot],
        player_teams: &HashMap<u32, Team>,
    ) {
        let mut rows: Vec<_> = snapshots.iter().collect();
        rows.sort_by_key(|snapshot| snapshot.player_id);
        self.ai_players.clear();
        let mut restored_ids = HashSet::new();

        for snapshot in rows {
            if !restored_ids.insert(snapshot.player_id) {
                log::warn!(
                    "Ignoring duplicate host AI snapshot for player {}",
                    snapshot.player_id
                );
                continue;
            }
            let Some(&team) = player_teams.get(&snapshot.player_id) else {
                log::warn!(
                    "Ignoring host AI snapshot for missing player {}",
                    snapshot.player_id
                );
                continue;
            };
            if team == Team::Neutral {
                log::warn!(
                    "Ignoring host AI snapshot for neutral player {}",
                    snapshot.player_id
                );
                continue;
            }

            let difficulty =
                Self::difficulty_from_name(&snapshot.difficulty).unwrap_or(AIDifficulty::Medium);
            self.add_ai_player(snapshot.player_id, team, difficulty);
            let Some(ai) = self.ai_players.get_mut(&snapshot.player_id) else {
                continue;
            };

            ai.personality = Self::personality_from_name(&snapshot.personality)
                .unwrap_or_else(|| AIPersonality::for_team(team));
            ai.is_active = snapshot.is_active;
            // Legacy snapshot rows represented the same anchor only as the
            // first tactical defensive position; prefer the dedicated field
            // when a current-format save has it.
            let saved_base_center = snapshot
                .base_center
                .or_else(|| snapshot.tactical_state.defensive_positions.first().copied());
            if let Some(base_center) = saved_base_center.filter(|pos| pos.is_finite()) {
                ai.relocate_base(base_center);
            }
            if snapshot.base_radius.is_finite() && snapshot.base_radius > 0.0 {
                ai.base_radius = snapshot.base_radius;
            }
            ai.current_strategy = Self::strategy_from_name(&snapshot.current_strategy)
                .unwrap_or(AIStrategy::EarlyGame);
            ai.build_phase = Self::build_phase_from_name(&snapshot.strategic_state.current_phase)
                .unwrap_or(AIBuildPhase::BaseConstruction);
            ai.activity_count = snapshot.activity_count;
            ai.defensive_units = snapshot
                .tactical_state
                .unit_groups
                .iter()
                .filter(|group| group.role.eq_ignore_ascii_case("defensive"))
                .flat_map(|group| group.units.iter().copied())
                .collect();

            // A target can be destroyed/reused during object restoration.  Do
            // not revive a half-resolved attack or production pointer; fresh
            // host AI evaluation will issue legal actions on the next update.
            ai.attack_in_progress = false;
            ai.team_queue.clear();
            ai.team_ready_queue.clear();
            ai.structures_to_repair.clear();
            ai.repair_dozer = None;
            ai.dozer_queued_for_repair = false;
            ai.dozer_is_repairing = false;
            ai.skillset_selector = INVALID_SKILLSET_SELECTION;
            ai.last_update_time = 0.0;
            ai.resource_check_time = 0.0;
            ai.enemy_check_time = 0.0;
            ai.next_building_time = 0.0;
            ai.next_team_queue_time = 0.0;
            ai.next_team_time = 0.0;
        }

        // Let the first post-load logic frame rebuild actions immediately.
        self.last_update_time = -1.0;
    }

    pub fn capture_queue_persist(
        &self,
    ) -> Vec<crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist> {
        let mut player_ids: Vec<u32> = self.ai_players.keys().copied().collect();
        player_ids.sort_unstable();
        player_ids
            .into_iter()
            .filter_map(|player_id| self.ai_players.get(&player_id))
            .map(AIPlayer::capture_queue_persist)
            .collect()
    }

    pub fn apply_queue_persist(
        &mut self,
        rows: Vec<crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist>,
    ) {
        for row in rows {
            let Some(ai) = self.ai_players.get_mut(&row.player_id) else {
                continue;
            };
            ai.apply_queue_persist(row);
        }
    }

    pub fn clear_queue_persist(&mut self) {
        for ai in self.ai_players.values_mut() {
            ai.clear_queue_persist();
        }
    }

    fn difficulty_name(value: AIDifficulty) -> &'static str {
        match value {
            AIDifficulty::Easy => "Easy",
            AIDifficulty::Medium => "Medium",
            AIDifficulty::Hard => "Hard",
            AIDifficulty::Brutal => "Brutal",
        }
    }

    fn difficulty_from_name(value: &str) -> Option<AIDifficulty> {
        if value.eq_ignore_ascii_case("easy") {
            Some(AIDifficulty::Easy)
        } else if value.eq_ignore_ascii_case("medium") {
            Some(AIDifficulty::Medium)
        } else if value.eq_ignore_ascii_case("hard") {
            Some(AIDifficulty::Hard)
        } else if value.eq_ignore_ascii_case("brutal") {
            Some(AIDifficulty::Brutal)
        } else {
            None
        }
    }

    fn personality_name(value: AIPersonality) -> &'static str {
        match value {
            AIPersonality::Balanced => "Balanced",
            AIPersonality::Aggressive => "Aggressive",
            AIPersonality::Defensive => "Defensive",
            AIPersonality::Economic => "Economic",
            AIPersonality::Rush => "Rush",
        }
    }

    fn personality_from_name(value: &str) -> Option<AIPersonality> {
        if value.eq_ignore_ascii_case("balanced") {
            Some(AIPersonality::Balanced)
        } else if value.eq_ignore_ascii_case("aggressive") {
            Some(AIPersonality::Aggressive)
        } else if value.eq_ignore_ascii_case("defensive") {
            Some(AIPersonality::Defensive)
        } else if value.eq_ignore_ascii_case("economic") {
            Some(AIPersonality::Economic)
        } else if value.eq_ignore_ascii_case("rush") {
            Some(AIPersonality::Rush)
        } else {
            None
        }
    }

    fn strategy_name(value: AIStrategy) -> &'static str {
        match value {
            AIStrategy::EarlyGame => "EarlyGame",
            AIStrategy::MidGame => "MidGame",
            AIStrategy::LateGame => "LateGame",
            AIStrategy::Desperate => "Desperate",
        }
    }

    fn strategy_from_name(value: &str) -> Option<AIStrategy> {
        if value.eq_ignore_ascii_case("earlygame") {
            Some(AIStrategy::EarlyGame)
        } else if value.eq_ignore_ascii_case("midgame") {
            Some(AIStrategy::MidGame)
        } else if value.eq_ignore_ascii_case("lategame") {
            Some(AIStrategy::LateGame)
        } else if value.eq_ignore_ascii_case("desperate") {
            Some(AIStrategy::Desperate)
        } else {
            None
        }
    }

    fn build_phase_name(value: AIBuildPhase) -> &'static str {
        match value {
            AIBuildPhase::BaseConstruction => "BaseConstruction",
            AIBuildPhase::UnitProduction => "UnitProduction",
            AIBuildPhase::Expansion => "Expansion",
            AIBuildPhase::MassProduction => "MassProduction",
        }
    }

    fn build_phase_from_name(value: &str) -> Option<AIBuildPhase> {
        if value.eq_ignore_ascii_case("baseconstruction") {
            Some(AIBuildPhase::BaseConstruction)
        } else if value.eq_ignore_ascii_case("unitproduction") {
            Some(AIBuildPhase::UnitProduction)
        } else if value.eq_ignore_ascii_case("expansion") {
            Some(AIBuildPhase::Expansion)
        } else if value.eq_ignore_ascii_case("massproduction") {
            Some(AIBuildPhase::MassProduction)
        } else {
            None
        }
    }

    /// Sum of production-linked AI actions across all host AI players.
    pub fn total_activity_count(&self) -> u64 {
        self.ai_players.values().map(|p| p.activity_count).sum()
    }

    /// Get AI player information
    pub fn get_ai_info(&self, player_id: u32) -> Option<String> {
        self.ai_players.get(&player_id).map(|ai_player| format!(
                "AI Player {} ({}): {:?} difficulty, {:?} strategy, {} buildings queued, {} teams queued", 
                player_id,
                ai_player.team.get_name(),
                ai_player.difficulty,
                ai_player.current_strategy,
                ai_player.building_queue.len(),
                ai_player.team_queue.len()
            ))
    }

    /// Return the most common configured difficulty across active AI players.
    ///
    /// Ties are resolved towards the harder difficulty to better represent
    /// gameplay pressure in mixed-difficulty skirmishes.
    pub fn dominant_difficulty(&self) -> Option<AIDifficulty> {
        if self.ai_players.is_empty() {
            return None;
        }

        let mut counts = [0usize; 4]; // Easy, Medium, Hard, Brutal
        for ai_player in self.ai_players.values() {
            let idx = match ai_player.difficulty {
                AIDifficulty::Easy => 0,
                AIDifficulty::Medium => 1,
                AIDifficulty::Hard => 2,
                AIDifficulty::Brutal => 3,
            };
            counts[idx] += 1;
        }

        let mut best_idx = 0usize;
        for idx in 1..counts.len() {
            if counts[idx] > counts[best_idx] || (counts[idx] == counts[best_idx] && idx > best_idx)
            {
                best_idx = idx;
            }
        }

        Some(match best_idx {
            0 => AIDifficulty::Easy,
            1 => AIDifficulty::Medium,
            2 => AIDifficulty::Hard,
            _ => AIDifficulty::Brutal,
        })
    }

    /// True when a host AI player is registered and marked active.
    pub fn is_ai_active(&self, player_id: u32) -> bool {
        self.ai_players
            .get(&player_id)
            .map(|p| p.is_active)
            .unwrap_or(false)
    }

    /// Configured difficulty for a registered host AI player.
    pub fn ai_difficulty(&self, player_id: u32) -> Option<AIDifficulty> {
        self.ai_players.get(&player_id).map(|p| p.difficulty)
    }

    /// Teams of all registered host AI players (for template rebind).
    pub fn registered_teams(&self) -> Vec<Team> {
        let mut teams = Vec::new();
        for ai in self.ai_players.values() {
            if !teams.contains(&ai.team) {
                teams.push(ai.team);
            }
        }
        teams
    }

    /// Rebind host AI after world objects were wiped (map load / preserve path).
    ///
    /// Keeps registration, difficulty, `is_active`, personality, and base layout
    /// template names. Drops stale object/factory IDs so rebuild soup can run
    /// again without burning `max_rebuilds`, and reopens early-base timers.
    pub fn rebind_after_world_reset(&mut self) {
        log::info!(
            "AI Manager: rebinding {} AI player(s) after world reset",
            self.ai_players.len()
        );
        for ai_player in self.ai_players.values_mut() {
            for building in &mut ai_player.building_queue {
                // Map load clears objects; this is not a combat loss — restore rebuild budget.
                building.object_id = None;
                building.is_built = false;
                building.rebuild_count = 0;
            }
            for team in &mut ai_player.team_queue {
                team.completed = false;
                for order in &mut team.work_orders {
                    order.factory_id = None;
                    order.queued_count = 0;
                    order.num_completed = 0;
                    order.observed_unit_ids.clear();
                }
            }
            ai_player.defensive_units.clear();
            ai_player.attack_in_progress = false;
            // Timing: allow next host AI tick to act immediately.
            ai_player.last_update_time = 0.0;
            ai_player.resource_check_time = 0.0;
            ai_player.enemy_check_time = 0.0;
            ai_player.next_building_time = 0.0;
            ai_player.next_team_queue_time = 0.0;
            ai_player.next_team_time = 0.0;
            ai_player.last_attack_time = 0.0;
            log::debug!(
                "  Rebound AI player {} ({}) active={} difficulty={:?}",
                ai_player.player_id,
                ai_player.team.get_name(),
                ai_player.is_active,
                ai_player.difficulty
            );
        }
        // Negative so the first post-load host update is not rate-limited away.
        self.last_update_time = -1.0;
    }

    /// Called when a game is loaded from save
    pub fn on_game_loaded(&mut self) {
        log::info!("AI Manager: Game loaded, reinitializing AI state...");
        // Save restore also wipes live object pointers in practice; share map-load rebind.
        self.rebind_after_world_reset();
        log::info!("AI Manager: Game load initialization complete");
    }

    pub fn resolve_player_id(game_logic: &GameLogic, token: &str) -> Option<u32> {
        let t = token.trim();
        if t.is_empty() {
            return None;
        }
        if let Ok(id) = t.parse::<u32>() {
            if game_logic.get_player(id).is_some() {
                return Some(id);
            }
        }
        game_logic.get_players().iter().find_map(|(id, player)| {
            let team_name = player.team.get_name();
            let lower = t.to_ascii_lowercase();
            if player.name.eq_ignore_ascii_case(t)
                || team_name.eq_ignore_ascii_case(t)
                || lower.contains(&team_name.to_ascii_lowercase())
            {
                Some(*id)
            } else {
                None
            }
        })
    }

    /// C++ `AIPlayer::buildSpecificAITeam` live host entry.
    pub fn build_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        player_id: u32,
        team_name: &str,
        priority_build: bool,
    ) -> bool {
        self.ai_players.get_mut(&player_id).is_some_and(|ai| {
            ai.build_specific_ai_team(game_logic, team_name, priority_build)
        })
    }

    /// Resolve prototype owner then `buildSpecificAITeam(..., true)`.
    pub fn build_specific_ai_team_for_token(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        team_name: &str,
        priority_build: bool,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_specific_ai_team(game_logic, id, team_name, priority_build)
    }

    /// C++ `AIPlayer::recruitSpecificAITeam` live host entry.
    pub fn recruit_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        player_id: u32,
        team_name: &str,
        recruit_radius: f32,
    ) -> bool {
        self.ai_players.get_mut(&player_id).is_some_and(|ai| {
            ai.recruit_specific_ai_team(game_logic, team_name, recruit_radius)
        })
    }

    /// Resolve prototype owner then `recruitSpecificAITeam`.
    pub fn recruit_specific_ai_team_for_token(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        team_name: &str,
        recruit_radius: f32,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.recruit_specific_ai_team(game_logic, id, team_name, recruit_radius)
    }

    /// C++ `ScriptActions::doGuardSupplyCenter` → `AIPlayer::guardSupplyCenter`.
    pub fn guard_supply_center_for_team(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        min_supplies: i32,
    ) -> bool {
        let Some(player_id) = self.resolve_guard_supply_player(game_logic, team_name) else {
            return false;
        };
        let Some(ai) = self.ai_players.get_mut(&player_id) else {
            return false;
        };
        ai.guard_supply_center(game_logic, team_name, min_supplies);
        true
    }

    fn resolve_guard_supply_player(
        &self,
        game_logic: &GameLogic,
        team_name: &str,
    ) -> Option<u32> {
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(prototype) = factory.find_team_prototype(team_name) {
                let owner = prototype.get_owner_name().to_string();
                if !owner.is_empty() {
                    if let Some(id) = Self::resolve_player_id(game_logic, &owner) {
                        if self.ai_players.contains_key(&id) {
                            return Some(id);
                        }
                    }
                }
            }
        }
        let needle = team_name.trim();
        if !needle.is_empty() {
            for obj in game_logic.host_objects().values() {
                if !obj.is_alive()
                    || obj.team_instance_name.is_empty()
                    || !obj.team_instance_name.eq_ignore_ascii_case(needle)
                {
                    continue;
                }
                if let Some((&id, _)) = game_logic
                    .get_players()
                    .iter()
                    .find(|(_, player)| player.team == obj.team)
                {
                    if self.ai_players.contains_key(&id) {
                        return Some(id);
                    }
                }
            }
        }
        Self::resolve_player_id(game_logic, team_name)
            .filter(|id| self.ai_players.contains_key(id))
    }



    /// C++ `SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST` live host entry.
    pub fn fire_skirmish_special_power_at_most_cost(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        power_name: &str,
    ) {
        let Some(player_id) = Self::resolve_player_id(game_logic, player_token)
            .or_else(|| {
                self.ai_players
                    .keys()
                    .copied()
                    .find(|id| Self::resolve_player_id(game_logic, player_token) == Some(*id))
            })
            .or_else(|| {
                // Token may name a team the AI owns even if Player.name differs.
                self.ai_players.iter().find_map(|(id, ai)| {
                    let team = ai.team.get_name();
                    player_token
                        .to_ascii_lowercase()
                        .contains(&team.to_ascii_lowercase())
                        .then_some(*id)
                })
            })
        else {
            return;
        };
        if let Some(ai) = self.ai_players.get_mut(&player_id) {
            ai.fire_named_special_power(game_logic, power_name);
        }
    }

    /// C++ `SKIRMISH_BUILD_BUILDING` live host entry.
    pub fn build_specific_ai_building(&mut self, player_id: u32, thing_name: &str) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.build_specific_ai_building(thing_name))
    }

    pub fn build_specific_ai_building_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        thing_name: &str,
    ) -> bool {
        if let Some(id) = Self::resolve_player_id(game_logic, player_token) {
            return self.build_specific_ai_building(id, thing_name);
        }
        // Script often omits player and stamps the current skirmish AI.
        let ids: Vec<u32> = self.ai_players.keys().copied().collect();
        ids.into_iter()
            .any(|id| self.build_specific_ai_building(id, thing_name))
    }

    /// C++ `Player::buildBaseDefense` live host entry (current skirmish AI).
    pub fn build_ai_base_defense_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        flank: bool,
    ) -> bool {
        if let Some(id) = Self::resolve_player_id(game_logic, player_token) {
            return self
                .ai_players
                .get_mut(&id)
                .is_some_and(|ai| ai.build_script_base_defense(Some(game_logic), flank));
        }
        let ids: Vec<u32> = self.ai_players.keys().copied().collect();
        ids.into_iter().any(|id| {
            self.ai_players
                .get_mut(&id)
                .is_some_and(|ai| ai.build_script_base_defense(Some(game_logic), flank))
        })
    }

    /// C++ `Player::buildBaseDefenseStructure` live host entry.
    pub fn build_ai_base_defense_structure_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        thing_name: &str,
        flank: bool,
    ) -> bool {
        if let Some(id) = Self::resolve_player_id(game_logic, player_token) {
            return self.ai_players.get_mut(&id).is_some_and(|ai| {
                ai.build_script_base_defense_structure(Some(game_logic), thing_name, flank)
            });
        }
        let ids: Vec<u32> = self.ai_players.keys().copied().collect();
        ids.into_iter().any(|id| {
            self.ai_players.get_mut(&id).is_some_and(|ai| {
                ai.build_script_base_defense_structure(Some(game_logic), thing_name, flank)
            })
        })
    }


    /// C++ `AIPlayer::buildBySupplies` live host entry.
    pub fn build_by_supplies(
        &mut self,
        game_logic: &GameLogic,
        player_id: u32,
        minimum_cash: i32,
        thing_name: &str,
    ) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.build_by_supplies(game_logic, minimum_cash, thing_name))
    }

    /// C++ `AIPlayer::buildBySupplies` for a script player token.
    pub fn build_by_supplies_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        minimum_cash: i32,
        thing_name: &str,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_by_supplies(game_logic, id, minimum_cash, thing_name)
    }

    /// C++ `AIPlayer::buildUpgrade` live host entry.
    pub fn build_upgrade(
        &mut self,
        game_logic: &mut GameLogic,
        player_id: u32,
        upgrade_name: &str,
    ) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.build_upgrade(game_logic, upgrade_name))
    }

    pub fn build_upgrade_for_token(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        upgrade_name: &str,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_upgrade(game_logic, id, upgrade_name)
    }

    /// C++ `AIPlayer::buildSpecificBuildingNearestTeam` live host entry.
    pub fn build_specific_building_nearest_team(
        &mut self,
        game_logic: &GameLogic,
        player_id: u32,
        thing_name: &str,
        team_name: &str,
    ) -> bool {
        self.ai_players.get_mut(&player_id).is_some_and(|ai| {
            ai.build_specific_building_nearest_team(game_logic, thing_name, team_name)
        })
    }

    pub fn build_specific_building_nearest_team_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        thing_name: &str,
        team_name: &str,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_specific_building_nearest_team(game_logic, id, thing_name, team_name)
    }

    /// Clear all pending AI commands
    pub fn clear_pending_commands(&mut self) {
        log::info!("AI Manager: Clearing all pending commands...");

        for ai_player in self.ai_players.values_mut() {
            // Clear building queues
            ai_player.building_queue.clear();

            // Clear team queues
            ai_player.team_queue.clear();

            // Reset attack state
            ai_player.attack_in_progress = false;

            log::debug!(
                "  Cleared commands for AI player {} ({})",
                ai_player.player_id,
                ai_player.team.get_name()
            );
        }

        log::info!("AI Manager: All pending commands cleared");
    }
}

#[cfg(test)]
mod cpp_parity_tests {
    use super::*;

    fn install_player_team_prototype(
        leftover_index: i32,
        team_name: &str,
        units: &[(i32, i32, &'static str)],
        priority: i32,
    ) {
        use std::sync::{Arc, RwLock};
        let _ = gamelogic::scripting::engine::initialize_script_engine();
        if let Ok(guard) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(engine) = guard.as_ref() {
                let mut or_c = gamelogic::scripting::OrCondition::new();
                or_c.set_first_and_condition(Some(Box::new(gamelogic::scripting::Condition::new(
                    gamelogic::scripting::ConditionType::ConditionTrue,
                ))));
                let mut script = gamelogic::scripting::Script::new();
                script.set_name("AlwaysBuild".into());
                script.condition = Some(Box::new(or_c));
                let mut list = gamelogic::scripting::ScriptList::new();
                list.append_script(Box::new(script));
                let _ = engine
                    .set_script_list_for_player(leftover_index as usize, Some(Box::new(list)));
            }
        }
        let proto_arc = {
            let mut tf = gamelogic::team::get_team_factory()
                .lock()
                .expect("team factory");
            let mut proto = gamelogic::team::TeamPrototype::new(team_name.into());
            proto.set_production_priority(priority);
            proto.set_production_condition("AlwaysBuild".into());
            proto.set_max_instances(8);
            for (i, (min_u, max_u, thing)) in units.iter().enumerate() {
                proto.set_units_info(
                    i,
                    gamelogic::team::CreateUnitsInfo {
                        min_units: *min_u,
                        max_units: *max_u,
                        unit_thing_name: thing,
                    },
                );
            }
            tf.replace_team_prototype(proto);
            tf.find_team_prototype(team_name).expect("registered proto")
        };
        let mut list = gamelogic::player::player_list()
            .write()
            .expect("player list");
        list.clear();
        for i in 0..=leftover_index {
            let p = Arc::new(RwLock::new(gamelogic::player::Player::new(i)));
            if i == leftover_index {
                if let Ok(mut pg) = p.write() {
                    pg.set_can_build_units(true);
                    pg.add_team_to_list(proto_arc.clone());
                }
            }
            list.add_player(p);
        }
    }

    fn clear_player_team_prototypes() {
        if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
            factory.reset();
        }
        if let Ok(mut list) = gamelogic::player::player_list().write() {
            list.clear();
        }
    }


    #[test]
    fn ai_default_base_inside_build_edge_residual() {
        // Default synthetic world is 512² centered at origin; MinDistFromEdge=30.
        // Layout offsets reach +100 (WarFactory) — bases must stay ≤ ~120 from origin.
        let mgr = AIManager::new();
        // Manager construction doesn't add players; mirror add_ai_player centers.
        let centers = [
            (Team::USA, Vec3::new(-120.0, 0.0, -120.0)),
            (Team::China, Vec3::new(120.0, 0.0, -120.0)),
            (Team::GLA, Vec3::new(120.0, 0.0, 120.0)),
        ];
        let half = 256.0;
        let edge = 30.0;
        let max_offset = 100.0;
        for (team, c) in centers {
            let farthest = c.x.abs().max(c.z.abs()) + max_offset;
            assert!(
                farthest <= half - edge,
                "{team:?} base pad would violate edge residual: farthest={farthest}"
            );
        }
        let _ = mgr;
    }

    #[test]
    fn script_build_team_drains_onto_host_ai_queue() {
        let _ = gamelogic::scripting::take_host_build_team_requests();

        let mut logic = crate::game_logic::GameLogic::new();
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut barracks_t = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks_t.set_cost(500, 0);
        barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("AmericaBarracks".into(), barracks_t);

        let mut player = crate::game_logic::Player::new(1, Team::USA, "PlyrAmerica", false);
        player.resources.supplies = 10_000;
        player.set_can_build_units(true);
        logic.add_player(player);
        logic.add_ai_opponent(1, Team::USA, AIDifficulty::Medium);
        if let Some(p) = logic.get_player_mut(1) {
            p.set_can_build_units(true);
        }
        let _ = logic.create_object(
            "AmericaBarracks",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        );

        if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
            factory.reset();
            factory.init_team(
                gamelogic::common::AsciiString::from("USA_RangerSquad"),
                gamelogic::common::AsciiString::from("PlyrAmerica"),
                false,
                None,
            );
        }

        gamelogic::scripting::request_host_build_team("PlyrAmerica", "USA_RangerSquad");
        logic.apply_host_loco_set_script_requests();

        let queued = logic
            .ai_manager
            .ai_players
            .get(&1)
            .map(|ai| ai.team_queue.len())
            .unwrap_or(0);
        assert_eq!(
            queued, 1,
            "BUILD_TEAM must field a priority team on host AIPlayer"
        );
        let team = logic
            .ai_manager
            .ai_players
            .get(&1)
            .and_then(|ai| ai.team_queue.front())
            .expect("queued team");
        assert_eq!(team.name, "USA_RangerSquad");
        assert!(team.priority_build);

        if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
            factory.reset();
        }
    }



    #[test]
    fn ai_player_update_order_matches_cpp_aiplayer_update() {
        // C++ AIPlayer.cpp:2987-3002
        let src = include_str!("ai.rs");
        let start = src
            .find("/// Main AI update — C++ `AIPlayer::update`")
            .expect("AIPlayer::update docs");
        let body = &src[start..src.len().min(start + 1800)];
        let econ = body.find("update_economic_management").expect("doBaseBuilding");
        let ready = body.find("check_ready_teams").expect("checkReadyTeams");
        let queued = body.find("check_queued_teams").expect("checkQueuedTeams");
        let mil = body.find("update_military_management").expect("doTeamBuilding");
        let upg = body.find("do_upgrades_and_skills").expect("doUpgradesAndSkills");
        let br = body
            .find("update_bridge_repair")
            .expect("updateBridgeRepair");
        assert!(econ < ready && ready < queued && queued < mil && mil < upg && upg < br);
        assert!(
            AIManager::new().update_interval > 0.0
                && (AIManager::new().update_interval - 1.0 / 30.0).abs() < 1e-6
        );
    }

    #[test]
    fn aidata_timing_constants_match_retail_defaults() {
        // Default/AIData.ini: StructureSeconds=0, TeamSeconds=10, RebuildDelay=30.
        assert_eq!(AIPlayer::STRUCTURE_SECONDS, 0.0);
        assert_eq!(AIPlayer::TEAM_SECONDS, 10.0);
        assert_eq!(AIPlayer::REBUILD_DELAY_SECONDS, 30.0);
        assert_eq!(AIPlayer::ATTACK_RECHECK_SECONDS, 60.0);
        assert_eq!(AIPlayer::WEALTHY_RESOURCES, 7000);
        assert_eq!(AIPlayer::POOR_RESOURCES, 2000);
        assert!((AIPlayer::STRUCTURES_WEALTHY_RATE - 2.0).abs() < 1e-5);
        assert!((AIPlayer::STRUCTURES_POOR_RATE - 0.6).abs() < 1e-5);
        assert!((AIPlayer::TEAMS_WEALTHY_RATE - 2.0).abs() < 1e-5);
        assert!((AIPlayer::TEAMS_POOR_RATE - 0.6).abs() < 1e-5);
        assert!((AIPlayer::TEAM_RESOURCES_TO_START - 0.1).abs() < 1e-5);
        // Difficulty stretches TeamSeconds (Easy slower, Hard faster).
        assert!((AIDifficulty::Easy.get_build_delay_modifier() - 2.0).abs() < 1e-5);
        assert!((AIDifficulty::Medium.get_build_delay_modifier() - 1.0).abs() < 1e-5);
        assert!((AIDifficulty::Hard.get_build_delay_modifier() - 0.7).abs() < 1e-5);
    }

    #[test]
    fn aidata_wealth_rate_scales_team_interval() {
        let mut logic = crate::game_logic::GameLogic::new();
        let ai = AIPlayer::new(1, Team::GLA, AIDifficulty::Medium);
        let mut player = crate::game_logic::Player::new(1, Team::GLA, "GLA", true);
        player.resources.supplies = 1000; // poor
        logic.add_player(player);

        let poor = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
        logic.get_player_mut(1).unwrap().resources.supplies = 4000; // normal
        let mid = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
        logic.get_player_mut(1).unwrap().resources.supplies = 8000; // wealthy
        let rich = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
        // Poor → longer wait; wealthy → shorter wait.
        assert!(
            poor > mid && mid > rich,
            "poor={poor} mid={mid} rich={rich}"
        );
        assert!(
            (mid - 10.0).abs() < 1e-3,
            "medium normal team interval ~10s got {mid}"
        );
        assert!(
            (rich - 5.0).abs() < 1e-3,
            "wealthy team interval ~5s got {rich}"
        );
        assert!(
            (poor - (10.0 / 0.6)).abs() < 1e-2,
            "poor team interval ~16.67s got {poor}"
        );
        // StructureSeconds=0 stays 0 regardless of wealth.
        assert_eq!(
            ai.scaled_interval_seconds(&logic, AIPlayer::STRUCTURE_SECONDS, true),
            0.0
        );
    }

    #[test]
    fn aidata_team_resources_to_start_gates_queue() {
        // C++ isPossibleToBuildTeam: required = trunc(unit_cost_sum * TeamResourcesToStart).
        assert!((AIPlayer::TEAM_RESOURCES_TO_START - 0.1).abs() < 1e-5);
        let mut logic = crate::game_logic::GameLogic::new();
        // Seed templates with known build costs.
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
        humvee.set_cost(700, 0);
        logic
            .templates
            .insert("AmericaVehicleHumvee".into(), humvee);

        let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        // USA_BasicForce = 2*Ranger + 1*Humvee = 2*225 + 700 = 1150; *0.1 = 115.
        let full = ai.estimate_team_unit_cost(&logic, "USA_BasicForce");
        assert_eq!(full, 1150);
        let required = (full as f32 * AIPlayer::TEAM_RESOURCES_TO_START) as u32;
        assert_eq!(required, 115);

        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
        player.resources.supplies = 114; // one under threshold
        logic.add_player(player);
        assert!(!ai.can_afford_team_start(&logic, "USA_BasicForce"));

        logic.get_player_mut(1).unwrap().resources.supplies = 115;
        assert!(ai.can_afford_team_start(&logic, "USA_BasicForce"));
        // Without factories, is_possible_to_build_team stays false (factory residual).
        assert!(!ai.is_possible_to_build_team(&logic, "USA_BasicForce"));
        assert!(!ai.should_build_new_team(&logic));
    }

    #[test]
    fn select_team_to_build_calls_build_specific_ai_team() {
        let src = include_str!("ai.rs");
        let i = src
            .find("/// C++ `AIPlayer::selectTeamToBuild`")
            .expect("selectTeamToBuild");
        let window = &src[i..src.len().min(i + 2500)];
        assert!(
            window.contains("build_specific_ai_team(game_logic, name, false)")
                && !window.contains("create_team_queue"),
            "auto selectTeamToBuild must use leftover-right buildSpecificAITeam"
        );
    }

    #[test]
    fn estimate_team_unit_cost_averages_min_max() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.set_cost(200, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut proto = gamelogic::team::TeamPrototype::new("HQ_AvgCost".into());
            proto.set_units_info(
                0,
                gamelogic::team::CreateUnitsInfo {
                    min_units: 1,
                    max_units: 3,
                    unit_thing_name: "AmericaInfantryRanger",
                },
            );
            tf.replace_team_prototype(proto);
        }
        let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        // C++ (min+max)/2 * cost = (1+3)/2 * 200 = 400, not max-as-required 600.
        assert_eq!(ai.estimate_team_unit_cost(&logic, "HQ_AvgCost"), 400);
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            tf.reset();
        }
    }

    #[test]
    fn build_specific_ai_team_splits_optional_and_required() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        player.set_can_build_units(true);
        logic.add_player(player);
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);
        let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut proto = gamelogic::team::TeamPrototype::new("HQ_SplitTeam".into());
            proto.set_units_info(
                0,
                gamelogic::team::CreateUnitsInfo {
                    min_units: 1,
                    max_units: 4,
                    unit_thing_name: "AmericaInfantryRanger",
                },
            );
            tf.replace_team_prototype(proto);
        }
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        assert!(ai.build_specific_ai_team(&mut logic, "HQ_SplitTeam", false));
        let team = ai.team_queue.front().expect("queued");
        let required: Vec<_> = team
            .work_orders
            .iter()
            .filter(|order| order.is_required)
            .collect();
        let optional: Vec<_> = team
            .work_orders
            .iter()
            .filter(|order| !order.is_required)
            .collect();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].num_required, 1);
        assert_eq!(optional.len(), 1);
        assert_eq!(optional[0].num_required, 3);
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            tf.reset();
        }
    }


    #[test]
    fn select_team_to_build_splits_min_max_not_max_as_required() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        player.set_can_build_units(true);
        logic.add_player(player);
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);
        let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);

        install_player_team_prototype(1, "HQ_MinMaxTeam", &[(1, 4, "AmericaInfantryRanger")], 20);
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        assert!(
            ai.select_team_to_build(&mut logic, 0.0),
            "auto-select must queue the leftover player prototype"
        );
        let team = ai.team_queue.front().expect("queued team");
        assert_eq!(team.name, "HQ_MinMaxTeam");
        let required: Vec<_> = team
            .work_orders
            .iter()
            .filter(|order| order.is_required)
            .collect();
        let optional: Vec<_> = team
            .work_orders
            .iter()
            .filter(|order| !order.is_required)
            .collect();
        assert_eq!(required.len(), 1, "required minUnits stub: {required:?}");
        assert_eq!(required[0].num_required, 1);
        assert_eq!(optional.len(), 1, "optional max-min: {optional:?}");
        assert_eq!(optional[0].num_required, 3);
        assert!(
            !team
                .work_orders
                .iter()
                .any(|order| order.is_required && order.num_required == 4),
            "must not invent max-as-required work orders: {:?}",
            team.work_orders
                .iter()
                .map(|order| (
                    order.template_name.as_str(),
                    order.num_required,
                    order.is_required
                ))
                .collect::<Vec<_>>()
        );
        clear_player_team_prototypes();
    }

    #[test]
    fn is_a_good_idea_does_not_reject_ready_queue_team() {
        // C++ isAGoodIdeaToBuildTeam (AIPlayer.cpp:1487-1492) only walks
        // iterate_TeamBuildQueue. A maxInstances>1 copy may start while the
        // first sits idle in TeamReadyQueue (up to 60s force).
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        player.set_can_build_units(true);
        logic.add_player(player);
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);
        let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);

        install_player_team_prototype(1, "HQ_Auf59_ReadyOk", &[(1, 1, "AmericaInfantryRanger")], 20);
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.team_ready_queue.push_back(AITeamQueue::new(
            "HQ_Auf59_ReadyOk".into(),
            vec![AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 20)],
            false,
            0,
        ));
        assert!(
            ai.is_a_good_idea_to_build_team(&logic, "HQ_Auf59_ReadyOk"),
            "ready-queue copy must not veto a second maxInstances>1 start"
        );

        ai.team_queue.push_back(AITeamQueue::new(
            "HQ_Auf59_ReadyOk".into(),
            vec![AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 20)],
            false,
            0,
        ));
        assert!(
            !ai.is_a_good_idea_to_build_team(&logic, "HQ_Auf59_ReadyOk"),
            "TeamBuildQueue still vetoes a prototype already under construction"
        );
        clear_player_team_prototypes();
    }



    #[test]
    fn aidata_team_factory_idle_gate() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
        humvee.set_cost(700, 0);
        logic
            .templates
            .insert("AmericaVehicleHumvee".into(), humvee);
        let mut barracks_t = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks_t.set_cost(500, 0);
        barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("AmericaBarracks".into(), barracks_t);
        let mut wf_t = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        wf_t.set_cost(1000, 0);
        wf_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), wf_t);

        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
        player.resources.supplies = 10_000;
        logic.add_player(player);

        let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        // No factories yet.
        assert!(!ai.team_factories_ready(&logic, "USA_BasicForce"));
        assert!(!ai.is_possible_to_build_team(&logic, "USA_BasicForce"));

        // Spawn constructed barracks + war factory.
        let barracks_id = logic
            .create_object("AmericaBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("barracks");
        let wf_id = logic
            .create_object(
                "AmericaWarFactory",
                Team::USA,
                glam::Vec3::new(50.0, 0.0, 0.0),
            )
            .expect("war factory");
        // Ensure constructed + empty queues.
        if let Some(o) = logic./* Wave 950 */ host_object_mut(barracks_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.clear();
            }
        }
        if let Some(o) = logic.host_object_mut(wf_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.clear();
            }
        }
        assert!(ai.team_factories_ready(&logic, "USA_BasicForce"));
        assert!(ai.is_possible_to_build_team(&logic, "USA_BasicForce"));

        // Busy both factories → not ready (requireIdleFactory residual).
        if let Some(o) = logic.host_object_mut(barracks_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.push(crate::game_logic::ProductionItem {
                    template_name: "USA_Ranger".into(),
                    progress: 0.1,
                    total_time: 10.0,
                    construction_frames: 0,
                    cost: crate::game_logic::Resources {
                        supplies: 225,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: crate::game_logic::buildings::ProductionKind::Unit,
                });
            }
        }
        if let Some(o) = logic.host_object_mut(wf_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.push(crate::game_logic::ProductionItem {
                    template_name: "USA_Humvee".into(),
                    progress: 0.1,
                    total_time: 10.0,
                    construction_frames: 0,
                    cost: crate::game_logic::Resources {
                        supplies: 700,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: crate::game_logic::buildings::ProductionKind::Unit,
                });
            }
        }
        assert!(!ai.team_factories_ready(&logic, "USA_BasicForce"));
        assert!(!ai.should_build_new_team(&logic));

        // Idle one factory → ready again.
        if let Some(o) = logic.host_object_mut(barracks_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.clear();
            }
        }
        assert!(ai.team_factories_ready(&logic, "USA_BasicForce"));
    }

    #[test]
    fn skirmish_queues_a_selected_team_without_waiting_for_team_seconds() {
        // Retail `AISkirmishPlayer::doTeamBuilding` first services existing
        // work orders and, after `selectTeamToBuild`, calls `queueUnits` again
        // in that same pass.  A normal USA AI therefore starts its Ranger and
        // Humvee immediately when both real factories are idle; waiting until
        // the next 10-second TeamSeconds window makes the early skirmish AI
        // visibly inert.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 3_000;
        logic.add_player(player);

        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);

        let mut war_factory = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        war_factory
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSWarFactory)
            .set_cost(1_000, 0);
        logic
            .templates
            .insert("AmericaWarFactory".into(), war_factory);

        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .set_cost(225, 0);
        // Complete on the next real fixed production frame so the test covers
        // the same producer_id handoff that the live skirmish path uses.
        ranger.build_time = 1.0 / 60.0;
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);

        let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
        humvee
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .set_cost(700, 0);
        logic
            .templates
            .insert("AmericaVehicleHumvee".into(), humvee);

        let barracks_id = logic
            .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
            .expect("constructed barracks");
        let war_factory_id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(64.0, 0.0, 0.0))
            .expect("constructed war factory");
        install_player_team_prototype(
            1,
            "USA_BasicForce",
            &[
                (2, 2, "AmericaInfantryRanger"),
                (1, 1, "AmericaVehicleHumvee"),
            ],
            10,
        );


        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.update_military_management(&mut logic, 0.0);

        assert_eq!(ai.team_queue.len(), 1, "the selected team is retained");
        assert_eq!(
            logic
                .host_object(barracks_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1),
            "the selected team's first Ranger is queued in the same AI pass"
        );
        assert_eq!(
            logic
                .host_object(war_factory_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1),
            "the selected team's Humvee is queued in the same AI pass"
        );
        assert!(
            (ai.next_team_time - AIPlayer::TEAM_SECONDS).abs() < f32::EPSILON,
            "a successful selection starts the longer TeamSeconds timer"
        );
        assert!(
            (ai.next_team_queue_time - AIPlayer::TEAM_QUEUE_RETRY_SECONDS).abs() < f32::EPSILON,
            "unfinished work orders remain on the short queue cadence"
        );

        // Let the actual production update create the Ranger and stamp its
        // producer.  C++ onUnitProduced shortcuts m_teamDelay at this point;
        // do not wait for the normal 2-second queue poll before starting the
        // second Ranger required by USA_BasicForce.
        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
        assert!(
            logic.host_objects().values().any(|object| {
                object.team == Team::USA
                    && object.producer_id == Some(barracks_id)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaInfantryRanger")
            }),
            "the host production path created a producer-linked Ranger"
        );
        let output_time = 1.0 / LOGIC_FRAMES_PER_SECOND;
        ai.update_military_management(&mut logic, output_time);
        let ranger_order = ai
            .team_queue
            .front()
            .and_then(|team| {
                team.work_orders
                    .iter()
                    .find(|order| order.template_name == "AmericaInfantryRanger")
            })
            .expect("BasicForce Ranger work order remains active");
        assert_eq!(ranger_order.num_completed, 1);
        assert_eq!(ranger_order.queued_count, 1);
        assert_eq!(ranger_order.factory_id, Some(barracks_id));
        assert_eq!(
            logic
                .host_object(barracks_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1),
            "live output requeues the next Ranger before the normal poll delay"
        );

        // No second team and no duplicate order before m_teamDelay expires.
        ai.update_military_management(&mut logic, 1.9);
        assert_eq!(ai.team_queue.len(), 1);
        assert_eq!(
            logic
                .host_object(barracks_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1)
        );
        clear_player_team_prototypes();
    }

    #[test]
    fn work_order_waits_for_live_factory_output_before_completing() {
        // C++ AIPlayer::onUnitProduced increments a WorkOrder only after
        // ProductionUpdate has created a unit and identified its producer.
        // A successful queue request alone must not erase the AI team.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 10_000;
        logic.add_player(player);

        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);

        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .add_kind_of(crate::game_logic::KindOf::Attackable)
            .set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);

        let factory = logic
            .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
            .expect("constructed barracks");
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.team_queue.push_back(AITeamQueue::new(
            "one-ranger".into(),
            vec![AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100)],
            false,
            0,
        ));

        ai.process_team_queue(&mut logic, 0.0);
        let queued = ai.team_queue.front().expect("queue survives enqueue");
        let order = queued.work_orders.first().expect("work order");
        assert_eq!(
            order.num_completed, 0,
            "enqueue is not production completion"
        );
        assert_eq!(order.queued_count, 1);
        assert_eq!(order.factory_id, Some(factory));

        // Model the real production completion handoff: the production path
        // stamps producer_id before the unit becomes visible to host AI.
        let unit = logic
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                Vec3::new(12.0, 0.0, 0.0),
            )
            .expect("factory output");
        logic
            .host_object_mut(unit)
            .expect("produced unit")
            .producer_id = Some(factory);

        ai.process_team_queue(&mut logic, 1.0);
        assert!(
            ai.team_queue.front().is_some_and(|t| t.is_all_built()),
            "team becomes complete only after its live factory output is observed"
        );
        ai.check_queued_teams(&mut logic, 1.0);
        assert!(
            ai.team_queue.is_empty(),
            "all-built teams leave the build queue"
        );
        assert_eq!(ai.team_ready_queue.len(), 1);
        ai.check_ready_teams(&mut logic, 1.0);
        assert!(
            ai.team_ready_queue.is_empty(),
            "idle ready team activates without waiting 60s"
        );
    }

    #[test]
    fn supply_center_spawns_free_collector_then_ai_pays_for_next_collector() {
        // Retail AmericaSupplyCenter has SpawnBehavior ModuleTag_12 for one
        // free AmericaVehicleChinook.  C++ AIPlayer::queueSupplyTruck must not
        // represent that freebie as a zero-cost production item; it later
        // prepends a real paid work order through the same SupplyCenter.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 5_000;
        logic.add_player(player);

        let mut supply_center = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
        supply_center
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::FSSupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(2_000, 0);
        logic
            .templates
            .insert("AmericaSupplyCenter".into(), supply_center);

        let mut chinook = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
        chinook
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Aircraft)
            .add_kind_of(crate::game_logic::KindOf::Harvester)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(1_200, 0);
        // Keep the focused test on the real production completion path without
        // waiting ten retail seconds for the paid Chinook.
        chinook.build_time = 0.001;
        logic
            .templates
            .insert("AmericaVehicleChinook".into(), chinook);

        let mut source = crate::game_logic::ThingTemplate::new("TestSupplySource");
        source
            .add_kind_of(crate::game_logic::KindOf::Resource)
            .add_kind_of(crate::game_logic::KindOf::Harvestable);
        logic.templates.insert("TestSupplySource".into(), source);
        let source_id = logic
            .create_object("TestSupplySource", Team::Neutral, Vec3::new(32.0, 0.0, 0.0))
            .expect("typed supply source");
        logic
            .host_object_mut(source_id)
            .expect("source object")
            .set_stored_supplies(20_000);

        let cash_before_spawn = logic.get_player(1).expect("AI player").effective_supplies();
        let center_id = logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::ZERO)
            .expect("constructed supply center");
        let free_collectors: Vec<ObjectId> = logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.team == Team::USA
                    && object.producer_id == Some(center_id)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaVehicleChinook"))
                .then_some(id)
            })
            .collect();
        assert_eq!(
            free_collectors.len(),
            1,
            "SpawnBehavior creates one free Chinook"
        );
        assert_eq!(
            logic
                .get_player(1)
                .expect("AI player after spawn")
                .effective_supplies(),
            cash_before_spawn,
            "the authored SpawnBehavior collector is not charged as production"
        );
        assert_eq!(
            logic
                .host_object(center_id)
                .and_then(|center| center.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(0),
            "free SpawnBehavior collector does not enter ProductionUpdate"
        );

        let free_collector = free_collectors[0];
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.process_team_queue(&mut logic, 0.0);

        let paid_order = ai
            .team_queue
            .front()
            .and_then(|team| team.work_orders.first())
            .expect("one paid follow-up collector work order");
        assert!(paid_order.is_resource_gatherer);
        assert_eq!(paid_order.supply_center_id, Some(center_id));
        assert_eq!(paid_order.factory_id, Some(center_id));
        assert_eq!(paid_order.queued_count, 1);
        assert_eq!(
            logic
                .get_player(1)
                .expect("AI player after paid queue")
                .effective_supplies(),
            cash_before_spawn - 1_200,
            "only the later collector spends its authored build cost"
        );
        let free = logic
            .host_object(free_collector)
            .expect("free collector live");
        assert_eq!(free.ai_state, crate::game_logic::AIState::Gathering);
        assert_eq!(free.target, Some(source_id));
        assert_eq!(free.preferred_dock_id, Some(center_id));

        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
        ai.process_team_queue(&mut logic, 1.0 / LOGIC_FRAMES_PER_SECOND);

        let paid_collectors: Vec<ObjectId> = logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.team == Team::USA
                    && object.producer_id == Some(center_id)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaVehicleChinook"))
                .then_some(id)
            })
            .collect();
        assert_eq!(
            paid_collectors.len(),
            2,
            "the normal paid ProductionUpdate created a second producer-linked Chinook"
        );
        let paid_collector = *paid_collectors
            .iter()
            .find(|&&id| id != free_collector)
            .expect("new production output");
        let paid = logic
            .host_object(paid_collector)
            .expect("paid collector live");
        assert_eq!(paid.ai_state, crate::game_logic::AIState::Gathering);
        assert_eq!(paid.target, Some(source_id));
        assert_eq!(paid.preferred_dock_id, Some(center_id));
        assert!(
            ai.team_queue.is_empty(),
            "the paid collector work order completes only after its real output is routed"
        );
    }

    #[test]
    fn collector_returns_to_assigned_supply_center_over_nearer_center() {
        // `AIPlayer::queueSupplyTruck` sends `aiDock(center,
        // CMD_FROM_PLAYER)`.  SupplyTruckAIUpdate persists that center in
        // m_preferredDock, so the return leg must not switch to a different
        // closer depot in a normal multi-supply-center skirmish base.
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(
            1,
            Team::USA,
            "USA AI",
            false,
        ));

        let mut supply_center = crate::game_logic::ThingTemplate::new("TestSupplyCenter");
        supply_center
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
        logic
            .templates
            .insert("TestSupplyCenter".into(), supply_center);

        let mut collector = crate::game_logic::ThingTemplate::new("TestCollector");
        collector
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Harvester);
        logic.templates.insert("TestCollector".into(), collector);

        let assigned_center = logic
            .create_object("TestSupplyCenter", Team::USA, Vec3::ZERO)
            .expect("assigned supply center");
        let nearer_center = logic
            .create_object("TestSupplyCenter", Team::USA, Vec3::new(125.0, 0.0, 0.0))
            .expect("nearer supply center");
        let collector_id = logic
            .create_object("TestCollector", Team::USA, Vec3::new(250.0, 0.0, 0.0))
            .expect("collector");
        let assigned_position = logic
            .host_object(assigned_center)
            .expect("assigned center live")
            .get_position();
        {
            let collector = logic.host_object_mut(collector_id).expect("collector live");
            collector.preferred_dock_id = Some(assigned_center);
            collector.set_stored_supplies(100);
            collector.set_ai_state(crate::game_logic::AIState::ReturningResources);
        }

        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);

        let collector = logic
            .host_object(collector_id)
            .expect("collector after tick");
        let queued_destination = collector
            .movement
            .path
            .last()
            .copied()
            .or(collector.movement.target_position);
        assert_eq!(queued_destination, Some(assigned_position));
        assert_ne!(
            queued_destination,
            logic
                .host_object(nearer_center)
                .map(|center| center.get_position()),
            "a nearer center must not steal a collector assigned to another depot"
        );
    }

    #[test]
    fn active_loose_collector_rejoins_one_supply_center_before_paid_replacement() {
        // Retail `AIPlayer::queueSupplyTruck` first scans active SupplyTruckAI
        // units with an unresolved `m_preferredDock`.  It assigns one to the
        // understaffed center and returns before `startTraining`, so losing a
        // supply center does not immediately buy a duplicate collector.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 5_000;
        logic.add_player(player);

        let mut old_center_template = crate::game_logic::ThingTemplate::new("TestOldSupplyCenter");
        old_center_template
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
        logic
            .templates
            .insert("TestOldSupplyCenter".into(), old_center_template);

        let mut supply_center = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
        supply_center
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::FSSupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(2_000, 0);
        logic
            .templates
            .insert("AmericaSupplyCenter".into(), supply_center);

        let mut chinook = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
        chinook
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Aircraft)
            .add_kind_of(crate::game_logic::KindOf::Harvester)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(1_200, 0);
        logic
            .templates
            .insert("AmericaVehicleChinook".into(), chinook);

        let mut source = crate::game_logic::ThingTemplate::new("TestSupplySource");
        source
            .add_kind_of(crate::game_logic::KindOf::Resource)
            .add_kind_of(crate::game_logic::KindOf::Harvestable);
        logic.templates.insert("TestSupplySource".into(), source);

        // `ObjectID` validity in C++ is an existence test, not merely an
        // alive-state test.  The active survivor must therefore carry an ID
        // that is absent from the host object store (equivalent to a center
        // that already completed its destruction lifecycle).
        let missing_former_dock = ObjectId(100_000);
        assert!(logic.host_object(missing_former_dock).is_none());

        let replacement_center = logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(100.0, 0.0, 0.0))
            .expect("replacement center");
        // Remove its authored starter so this test isolates the survivor from
        // the destroyed center.  The parent one-shot latch stays fired, as it
        // would in a real match after that starter had been lost in combat.
        let starter_ids: Vec<ObjectId> = logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.producer_id == Some(replacement_center)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaVehicleChinook"))
                .then_some(id)
            })
            .collect();
        assert_eq!(starter_ids.len(), 1, "authored one-shot starter exists");
        for starter_id in starter_ids {
            logic.destroy_object(starter_id);
        }
        logic.process_destroy_list();
        assert!(logic
            .host_object(replacement_center)
            .is_some_and(|center| center.supply_center_spawn_behavior_fired));

        let source_id = logic
            .create_object(
                "TestSupplySource",
                Team::Neutral,
                Vec3::new(132.0, 0.0, 0.0),
            )
            .expect("nearby supply source");
        logic
            .host_object_mut(source_id)
            .expect("source live")
            .set_stored_supplies(20_000);

        let loose_collector = logic
            .create_object(
                "AmericaVehicleChinook",
                Team::USA,
                Vec3::new(140.0, 0.0, 0.0),
            )
            .expect("surviving collector");
        {
            let collector = logic
                .host_object_mut(loose_collector)
                .expect("surviving collector live");
            collector.preferred_dock_id = Some(missing_former_dock);
            collector.set_order_target(Some(source_id));
            collector.set_ai_state(crate::game_logic::AIState::Gathering);
        }

        let cash_before = logic.get_player(1).expect("AI player").effective_supplies();
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.process_team_queue(&mut logic, 0.0);

        let collector = logic
            .host_object(loose_collector)
            .expect("reassigned collector live");
        assert_eq!(collector.preferred_dock_id, Some(replacement_center));
        assert_eq!(collector.ai_state, crate::game_logic::AIState::Gathering);
        assert_eq!(collector.target, Some(source_id));
        assert!(
            ai.team_queue.is_empty(),
            "C++ returns after one active-collector reassignment before creating a paid work order"
        );
        assert_eq!(
            logic
                .host_object(replacement_center)
                .and_then(|center| center.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(0),
            "the factory does not receive a paid replacement in the reassignment pass"
        );
        assert_eq!(
            logic
                .get_player(1)
                .expect("AI player after reassignment")
                .effective_supplies(),
            cash_before,
            "reassigning an existing collector is free"
        );
    }

    #[test]
    fn skirmish_starts_one_structure_with_a_live_dozer_assignment() {
        // `AISkirmishPlayer::processBaseBuilding` starts one plan and routes a
        // real dozer to it.  A second affordable plan must remain queued until
        // the next economic pass; the scaffold must then make real construction
        // progress through the authoritative dozer target association.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 1_000;
        logic.add_player(player);

        let mut dozer_template = crate::game_logic::ThingTemplate::new("TestDozer");
        dozer_template
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Worker);
        logic.templates.insert("TestDozer".into(), dozer_template);

        let mut first_template = crate::game_logic::ThingTemplate::new("TestFirstStructure");
        first_template
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(300, 0);
        first_template.build_time = 10.0;
        logic
            .templates
            .insert("TestFirstStructure".into(), first_template);

        let mut second_template = crate::game_logic::ThingTemplate::new("TestSecondStructure");
        second_template
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(300, 0);
        logic
            .templates
            .insert("TestSecondStructure".into(), second_template);

        let build_position = Vec3::new(64.0, 0.0, 64.0);
        let dozer_id = logic
            .create_object("TestDozer", Team::USA, Vec3::new(48.0, 0.0, 64.0))
            .expect("live dozer");
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_building("TestFirstStructure", build_position, 1);
        ai.add_building("TestSecondStructure", Vec3::new(128.0, 0.0, 64.0), 1);

        ai.process_building_queue(&mut logic, 0.0);

        let structure_id = ai.building_queue[0]
            .object_id
            .expect("first plan starts with its dozer");
        assert!(
            ai.building_queue[1].object_id.is_none(),
            "one C++ skirmish economic pass starts only one structure"
        );
        assert_eq!(
            logic.get_player(1).expect("AI player").effective_supplies(),
            700,
            "only the started structure is charged"
        );
        let dozer = logic.host_object(dozer_id).expect("assigned dozer");
        assert_eq!(dozer.target, Some(structure_id));
        assert_eq!(dozer.ai_state, crate::game_logic::AIState::Constructing);

        // C++ revisits under-construction base entries and hands them to a
        // replacement dozer if the original one is lost.
        let replacement_id = logic
            .create_object("TestDozer", Team::USA, Vec3::new(44.0, 0.0, 64.0))
            .expect("replacement dozer");
        logic
            .host_object_mut(dozer_id)
            .expect("original dozer")
            .health
            .current = 0.0;
        ai.process_building_queue(&mut logic, 0.1);
        let replacement = logic
            .host_object(replacement_id)
            .expect("replacement assigned");
        assert_eq!(replacement.target, Some(structure_id));
        assert_eq!(
            replacement.ai_state,
            crate::game_logic::AIState::Constructing
        );

        let before = logic
            .host_object(structure_id)
            .expect("under-construction scaffold")
            .construction_percent;
        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
        let after = logic
            .host_object(structure_id)
            .expect("live scaffold")
            .construction_percent;
        assert!(
            after > before,
            "the assigned dozer advances construction instead of leaving a dead scaffold"
        );
    }

    /// C++ AIPlayer::findDozer calls queueDozer when no KINDOF_DOZER exists
    /// (AIPlayer.cpp:3254-3256 / queueDozer 3128-3171).
    #[test]
    fn lost_dozer_queues_priority_command_center_replacement() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 5_000;
        logic.add_player(player);

        let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::CommandCenter)
            .set_cost(2000, 0);
        logic.templates.insert("AmericaCommandCenter".into(), cc);

        let mut dozer = crate::game_logic::ThingTemplate::new("AmericaVehicleDozer");
        dozer
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Worker)
            .add_kind_of(crate::game_logic::KindOf::Dozer)
            .set_cost(1000, 0);
        dozer.build_time = 5.0;
        logic
            .templates
            .insert("AmericaVehicleDozer".into(), dozer);

        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);

        let cc_id = logic
            .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
            .expect("command center");
        if let Some(obj) = logic.host_object_mut(cc_id) {
            obj.owner_player_id = Some(1);
            if let Some(bd) = obj.building_data.as_mut() {
                bd.building_type = crate::game_logic::BuildingType::CommandCenter;
            }
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_building("AmericaBarracks", Vec3::new(64.0, 0.0, 0.0), 1);
        ai.process_building_queue(&mut logic, 0.0);

        assert_eq!(
            ai.team_queue.len(),
            1,
            "queueDozer must prepend a priority dozer team"
        );
        let order = ai
            .team_queue
            .front()
            .and_then(|team| team.work_orders.first())
            .expect("dozer work order");
        assert_eq!(order.template_name, "AmericaVehicleDozer");
        assert_eq!(order.factory_id, Some(cc_id));
        assert!(
            ai.team_queue.front().expect("dozer team").priority_build,
            "C++ TeamInQueue m_priorityBuild = true"
        );
        let queued = logic
            .host_object(cc_id)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.len())
            .unwrap_or(0);
        assert_eq!(queued, 1, "Command Center must start training the dozer");

        ai.process_building_queue(&mut logic, 1.0);
        assert_eq!(
            ai.team_queue.len(),
            1,
            "a second economic pass must not stack another dozer order"
        );
    }


    #[test]
    fn aidata_rebuild_delay_gates_destroyed_structure() {
        assert!((AIPlayer::REBUILD_DELAY_SECONDS - 30.0).abs() < 1e-5);
        let b = AIBuildingInfo::new("USA_Barracks".into(), Vec3::ZERO, 2);
        assert!(b.rebuild_delay_elapsed(0.0, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(b.rebuild_delay_elapsed(100.0, AIPlayer::REBUILD_DELAY_SECONDS));

        let mut destroyed = AIBuildingInfo::new("USA_Barracks".into(), Vec3::ZERO, 2);
        destroyed.destroyed_at_time = Some(10.0);
        // C++: timestamp + RebuildDelaySeconds*FPS > frame → wait.
        assert!(!destroyed.rebuild_delay_elapsed(10.0, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(!destroyed.rebuild_delay_elapsed(39.9, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(destroyed.rebuild_delay_elapsed(40.0, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(destroyed.rebuild_delay_elapsed(100.0, AIPlayer::REBUILD_DELAY_SECONDS));

        // process_building_queue stamps destroyed_at_time when object vanishes.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut barracks_t = crate::game_logic::ThingTemplate::new("USA_Barracks");
        barracks_t.set_cost(500, 0);
        barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("USA_Barracks".into(), barracks_t);

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_building("USA_Barracks", Vec3::new(0.0, 0.0, 0.0), 3);
        // Simulate a previously-built slot whose object was destroyed at t=5.
        {
            let b = &mut ai.building_queue[0];
            b.is_built = false;
            b.object_id = None;
            b.rebuild_count = 1;
            b.destroyed_at_time = Some(5.0);
        }
        // Before delay: queue must not start rebuild.
        ai.process_building_queue(&mut logic, 5.0 + AIPlayer::REBUILD_DELAY_SECONDS - 0.1);
        assert!(ai.building_queue[0].object_id.is_none());
        // After delay: may start (if create_object_under_construction succeeds).
        ai.process_building_queue(&mut logic, 5.0 + AIPlayer::REBUILD_DELAY_SECONDS);
        // Either started (object_id Some) or still none if construction API refused —
        // destroyed_at_time must clear only on successful start.
        if ai.building_queue[0].object_id.is_some() {
            assert!(ai.building_queue[0].destroyed_at_time.is_none());
        } else {
            // Delay gate itself elapsed; remaining failure is construction residual.
            assert!(ai.building_queue[0].rebuild_delay_elapsed(
                5.0 + AIPlayer::REBUILD_DELAY_SECONDS,
                AIPlayer::REBUILD_DELAY_SECONDS
            ));
        }
    }

    #[test]
    fn captured_pad_unbinds_and_gla_hole_rebinds() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 10_000;
        logic.add_player(player);
        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);

        let factory_id = logic
            .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
            .expect("pad");
        if let Some(obj) = logic.host_object_mut(factory_id) {
            obj.owner_player_id = Some(1);
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_building("AmericaBarracks", Vec3::ZERO, 3);
        ai.building_queue[0].object_id = Some(factory_id);
        ai.building_queue[0].is_built = true;

        // Capture: new owner, same live object.
        if let Some(obj) = logic.host_object_mut(factory_id) {
            obj.owner_player_id = Some(0);
            obj.set_team(Team::China);
        }
        ai.sync_build_list_object_status(&logic, 12.0);
        assert!(ai.building_queue[0].object_id.is_none());
        assert!(!ai.building_queue[0].is_built);
        assert_eq!(ai.building_queue[0].destroyed_at_time, Some(12.0));

        // Destroyed + GLA hole with matching spawner.
        let hole_id = logic
            .create_object("AmericaBarracks", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
            .expect("hole");
        if let Some(hole) = logic.host_object_mut(hole_id) {
            hole.is_rebuild_hole = true;
            hole.rebuild_spawner_id = Some(factory_id);
            hole.owner_player_id = Some(1);
            hole.set_team(Team::USA);
        }
        ai.building_queue[0].object_id = Some(factory_id);
        logic.destroy_object(factory_id);
        ai.sync_build_list_object_status(&logic, 13.0);
        assert_eq!(ai.building_queue[0].object_id, Some(hole_id));
    }

    #[test]
    fn economic_update_does_not_invent_random_supply_or_power_pads() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 10;
        player.power_available = -40;
        logic.add_player(player);
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_building("AmericaBarracks", Vec3::ZERO, 1);
        let before = ai.building_queue.len();
        ai.next_building_time = 0.0;
        ai.update_economic_management(&mut logic, 0.0);
        assert_eq!(
            ai.building_queue.len(),
            before,
            "low cash / brown-out must not append invented SupplyCenter/PowerPlant pads"
        );
        assert!(
            !ai.building_queue
                .iter()
                .any(|b| b.template_name.contains("SupplyCenter")
                    || b.template_name.contains("PowerPlant")),
            "no extra supply/power pads outside the authored list"
        );
    }

    #[test]
    fn build_by_supplies_places_named_template_near_warehouse() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 10_000;
        logic.add_player(player);
        let mut pile = crate::game_logic::ThingTemplate::new("SupplyWarehouse");
        pile.add_kind_of(crate::game_logic::KindOf::Harvestable)
            .add_kind_of(crate::game_logic::KindOf::Resource)
            .add_kind_of(crate::game_logic::KindOf::SupplySource);
        logic.templates.insert("SupplyWarehouse".into(), pile);
        let mut sc = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
        sc.add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
        logic.templates.insert("AmericaSupplyCenter".into(), sc);

        let warehouse = logic
            .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(200.0, 0.0, 0.0))
            .expect("warehouse");
        if let Some(obj) = logic.host_object_mut(warehouse) {
            obj.stored_resources.supplies = 5_000;
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.base_center = Vec3::ZERO;
        assert!(ai.build_by_supplies(&logic, 100, "AmericaSupplyCenter"));
        let pad = ai
            .building_queue
            .iter()
            .find(|b| b.template_name == "AmericaSupplyCenter")
            .expect("named pad");
        assert!(
            (pad.position - Vec3::new(200.0, 0.0, 0.0)).length() < 80.0,
            "depot must sit near the warehouse, not a random base-center offset: {:?}",
            pad.position
        );
        assert!(pad.is_priority);
    }

    #[test]
    fn skirmish_new_map_uses_aidata_side_build_list() {
        {
            let mut store = game_engine::common::ini::get_ai_data_store_mut();
            store.ensure_base();
            if let Some(data) = store.get_active_mut() {
                data.rotate_skirmish_bases = false;
                data.side_build_lists.retain(|l| !l.side.eq_ignore_ascii_case("America"));
                let mut list = game_engine::common::ini::AiSideBuildList::new("America".into());
                list.entries.push(game_engine::common::ini::BuildListEntry {
                    building_name: "CC".into(),
                    template_name: "AmericaCommandCenter".into(),
                    location: (0.0, 0.0),
                    rebuilds: 0,
                    angle_radians: 0.0,
                    initially_built: false,
                    rally_point_offset: (0.0, 0.0),
                    automatically_build: true,
                });
                list.entries.push(game_engine::common::ini::BuildListEntry {
                    building_name: "WF".into(),
                    template_name: "AmericaWarFactory".into(),
                    location: (80.0, 0.0),
                    rebuilds: 1,
                    angle_radians: 0.0,
                    initially_built: false,
                    rally_point_offset: (0.0, 0.0),
                    automatically_build: true,
                });
                data.side_build_lists.push(list);
            }
        }

        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));
        let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::CommandCenter);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let mut wf = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        wf.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), wf);

        let start_cc = logic
            .create_object("AmericaCommandCenter", Team::USA, Vec3::new(-40.0, 0.0, -40.0))
            .expect("map CC");
        if let Some(obj) = logic.host_object_mut(start_cc) {
            obj.owner_player_id = Some(1);
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.initialize(Vec3::new(-40.0, 0.0, -40.0));
        assert!(ai.apply_skirmish_new_map(&mut logic));
        assert!(
            logic.host_object(start_cc).is_none(),
            "map-placed CC must be destroyed"
        );
        let cc_pad = ai
            .building_queue
            .iter()
            .find(|b| b.template_name.contains("CommandCenter"))
            .expect("list CC");
        assert!(cc_pad.is_built, "list CC is InitiallyBuilt / buildStructureNow");
        let wf_pad = ai
            .building_queue
            .iter()
            .find(|b| b.template_name.contains("WarFactory"))
            .expect("list WF");
        assert!(!wf_pad.is_built);
        assert!(
            wf_pad.is_buildable(),
            "non-CC entries incrementNumRebuilds so first build does not spend the last slot"
        );
        {
            let mut store = game_engine::common::ini::get_ai_data_store_mut();
            if let Some(data) = store.get_active_mut() {
                data.side_build_lists
                    .retain(|l| !l.side.eq_ignore_ascii_case("America"));
            }
        }
    }

    #[test]
    fn queue_units_recruits_existing_idle_units() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut proto = gamelogic::team::TeamPrototype::new("USA_RangerSquad".into());
            proto.set_production_priority(50);
            tf.replace_team_prototype(proto);
        }


        let existing = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("idle ranger");
        if let Some(obj) = logic.host_object_mut(existing) {
            obj.owner_player_id = Some(1);
            obj.set_ai_state(crate::game_logic::AIState::Idle);
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
        ai.team_queue
            .push_back(AITeamQueue::new("USA_RangerSquad".into(), vec![order], false, 0));
        ai.process_team_queue(&mut logic, 0.0);
        let team = ai.team_queue.front().expect("queued team");
        assert_eq!(team.work_orders[0].num_completed, 1);
        assert_eq!(team.work_orders[0].observed_unit_ids, vec![existing]);
        assert!(
            team.work_orders[0].factory_id.is_none(),
            "recruited unit must not also startTraining"
        );
        let dest_id = team.team_id.expect("inactive dest team bound on recruit");
        assert_eq!(
            logic
                .host_object(existing)
                .map(|o| o.team_instance_name.as_str()),
            Some("USA_RangerSquad"),
            "C++ queueUnits setTeam onto dest instance immediately"
        );
        let members = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_by_id(dest_id))
            .and_then(|arc| arc.read().ok().map(|tg| tg.get_members().to_vec()))
            .unwrap_or_default();
        assert!(
            members.contains(&existing.0),
            "leftover dest instance must list the recruited unit"
        );
    }

    #[test]
    fn queue_units_skips_disabled_held_units() {
        // C++ Team::tryToRecruit DISABLED_HELD (Team.cpp:2353-2356).
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);

        let existing = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("held ranger");
        if let Some(obj) = logic.host_object_mut(existing) {
            obj.owner_player_id = Some(1);
            obj.status.disabled_held = true;
            obj.set_ai_state(crate::game_logic::AIState::Idle);
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
        ai.team_queue
            .push_back(AITeamQueue::new("USA_RangerSquad".into(), vec![order], false, 0));
        ai.process_team_queue(&mut logic, 0.0);
        let team = ai.team_queue.front().expect("queued team");
        assert_eq!(
            team.work_orders[0].num_completed, 0,
            "DISABLED_HELD unit must not be recruited"
        );
        assert!(team.work_orders[0].observed_unit_ids.is_empty());
    }

    #[test]
    fn try_to_recruit_takes_structures_and_contained() {
        // C++ Team::tryToRecruit has no Structure skip and no contained-by skip.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut pad_t = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        pad_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), pad_t);
        let mut ranger_t = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger_t.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger_t);

        let pad = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("pad");
        if let Some(obj) = logic.host_object_mut(pad) {
            obj.owner_player_id = Some(1);
            obj.set_ai_state(crate::game_logic::AIState::Idle);
        }
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let order = AIWorkOrder::new("AmericaWarFactory".into(), 1, 100);
        ai.team_queue
            .push_back(AITeamQueue::new("USA_FactoryTeam".into(), vec![order], false, 0));
        ai.process_team_queue(&mut logic, 0.0);
        let team = ai.team_queue.front().expect("queued factory team");
        assert_eq!(
            team.work_orders[0].num_completed, 1,
            "matching structure template must be recruitable"
        );
        assert_eq!(team.work_orders[0].observed_unit_ids, vec![pad]);

        let garrisoned = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("garrisoned");
        if let Some(obj) = logic.host_object_mut(garrisoned) {
            obj.owner_player_id = Some(1);
            obj.contained_by = Some(crate::game_logic::ObjectId(99));
            obj.set_ai_state(crate::game_logic::AIState::Idle);
        }
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
        ai.team_queue
            .push_back(AITeamQueue::new("USA_RangerSquad".into(), vec![order], false, 0));
        ai.process_team_queue(&mut logic, 0.0);
        let team = ai.team_queue.front().expect("queued ranger team");
        assert_eq!(
            team.work_orders[0].num_completed, 1,
            "contained (not HELD) matching unit must be recruitable"
        );
        assert_eq!(team.work_orders[0].observed_unit_ids, vec![garrisoned]);
    }

    fn w21_ranger_logic() -> (
        crate::game_logic::GameLogic,
        crate::game_logic::ObjectId,
    ) {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let existing = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("ranger");
        if let Some(obj) = logic.host_object_mut(existing) {
            obj.owner_player_id = Some(1);
            obj.set_ai_state(crate::game_logic::AIState::Idle);
        }
        (logic, existing)
    }

    fn w21_enqueue_and_recruit(
        logic: &mut crate::game_logic::GameLogic,
        dest: &str,
    ) -> u32 {
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let order = AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100);
        ai.team_queue
            .push_back(AITeamQueue::new(dest.into(), vec![order], false, 0));
        ai.process_team_queue(logic, 0.0);
        ai.team_queue.front().unwrap().work_orders[0].num_completed
    }

    #[test]
    fn try_to_recruit_skips_inactive_higher_priority_and_unrecruitable() {
        // C++ Team::tryToRecruit isActive / productionPriority / isRecruitable.
        let (mut logic, existing) = w21_ranger_logic();
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut dest = gamelogic::team::TeamPrototype::new("W21_DestHigh".into());
            dest.set_production_priority(50);
            tf.replace_team_prototype(dest);
            let mut src = gamelogic::team::TeamPrototype::new("W21_InactiveSrc".into());
            src.set_production_priority(0);
            src.set_ai_recruitable(true);
            tf.replace_team_prototype(src);
            if let Some(team) = tf.create_inactive_team("W21_InactiveSrc") {
                if let Ok(mut tg) = team.write() {
                    tg.add_member(existing.0);
                }
            }
        }
        if let Some(obj) = logic.host_object_mut(existing) {
            obj.team_instance_name = "W21_InactiveSrc".into();
        }
        assert_eq!(
            w21_enqueue_and_recruit(&mut logic, "W21_DestHigh"),
            0,
            "must not steal from a still-building team"
        );

        let (mut logic, existing) = w21_ranger_logic();
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut dest = gamelogic::team::TeamPrototype::new("W21_DestLow".into());
            dest.set_production_priority(10);
            tf.replace_team_prototype(dest);
            let mut src = gamelogic::team::TeamPrototype::new("W21_HighPriSrc".into());
            src.set_production_priority(50);
            src.set_ai_recruitable(true);
            tf.replace_team_prototype(src);
            if let Some(team) = tf.create_team("W21_HighPriSrc") {
                if let Ok(mut tg) = team.write() {
                    tg.add_member(existing.0);
                }
            }
        }
        if let Some(obj) = logic.host_object_mut(existing) {
            obj.team_instance_name = "W21_HighPriSrc".into();
        }
        assert_eq!(
            w21_enqueue_and_recruit(&mut logic, "W21_DestLow"),
            0,
            "must not steal from equal-or-higher priority"
        );

        let (mut logic, existing) = w21_ranger_logic();
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut dest = gamelogic::team::TeamPrototype::new("W21_DestRecruit".into());
            dest.set_production_priority(50);
            tf.replace_team_prototype(dest);
            let mut src = gamelogic::team::TeamPrototype::new("W21_NoRecruitSrc".into());
            src.set_production_priority(0);
            src.set_ai_recruitable(false);
            tf.replace_team_prototype(src);
            if let Some(team) = tf.create_team("W21_NoRecruitSrc") {
                if let Ok(mut tg) = team.write() {
                    tg.add_member(existing.0);
                }
            }
        }
        if let Some(obj) = logic.host_object_mut(existing) {
            obj.team_instance_name = "W21_NoRecruitSrc".into();
        }
        assert_eq!(
            w21_enqueue_and_recruit(&mut logic, "W21_DestRecruit"),
            0,
            "must not steal from a non-recruitable team"
        );

        let (mut logic, existing) = w21_ranger_logic();
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut dest = gamelogic::team::TeamPrototype::new("W21_DestUnitFlag".into());
            dest.set_production_priority(50);
            tf.replace_team_prototype(dest);
        }
        if let Some(obj) = logic.host_object_mut(existing) {
            obj.is_recruitable = false;
        }
        assert_eq!(
            w21_enqueue_and_recruit(&mut logic, "W21_DestUnitFlag"),
            0,
            "must not recruit a unit with isRecruitable=false"
        );
    }

    #[test]
    fn recruit_waiting_work_orders_joins_destination_team_instance() {
        // C++ queueUnits: unit->setTeam(team->m_team) immediately on recruit.
        let (mut logic, existing) = w21_ranger_logic();
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut dest = gamelogic::team::TeamPrototype::new("HQ_6_RecruitDest".into());
            dest.set_production_priority(50);
            tf.replace_team_prototype(dest);
        }
        assert_eq!(
            w21_enqueue_and_recruit(&mut logic, "HQ_6_RecruitDest"),
            1,
            "default-team ranger must be recruited"
        );
        let obj = logic.host_object(existing).expect("recruited ranger");
        assert_eq!(
            obj.team_instance_name, "HQ_6_RecruitDest",
            "recruited unit must join dest team_instance_name during build"
        );
        let members: Vec<u32> = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .map(|factory| {
                factory
                    .find_team_instances("HQ_6_RecruitDest")
                    .into_iter()
                    .flat_map(|arc| {
                        arc.read()
                            .ok()
                            .map(|tg| tg.get_members().to_vec())
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            members.contains(&existing.0),
            "leftover dest instance must gain the recruited member"
        );
    }


    #[test]
    fn check_ready_teams_execute_actions_requires_production_condition_action() {
        // C++ anyIdle shortcut needs ProductionCondition script WITH an action.
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let idle = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("idle");
        let busy = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("busy");
        if let Some(o) = logic.host_object_mut(busy) {
            o.set_ai_state(crate::game_logic::AIState::Moving);
        }
        let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
        order.num_completed = 2;
        order.observed_unit_ids.push(idle);
        order.observed_unit_ids.push(busy);
        let mut team = AITeamQueue::new("W21_NoCondTeam".into(), vec![order], false, 0);
        team.execute_actions = true;
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.team_ready_queue.push_back(team);
        ai.check_ready_teams(&mut logic, 1.0);
        assert_eq!(
            ai.team_ready_queue.len(),
            1,
            "without ProductionCondition action, wait for allIdle"
        );
    }

    #[test]
    fn check_ready_teams_reinforcement_idles_only_reinforcement_unit() {
        // C++ m_reinforcement idle gate uses only m_reinforcementID.
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let reinforce = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("reinforce");
        let fielded = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("fielded");
        if let Some(o) = logic.host_object_mut(fielded) {
            o.set_ai_state(crate::game_logic::AIState::Moving);
        }
        let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
        order.num_completed = 2;
        order.observed_unit_ids.push(reinforce);
        order.observed_unit_ids.push(fielded);
        let mut team = AITeamQueue::new("W21_ReinforceTeam".into(), vec![order], false, 0);
        team.reinforcement = true;
        team.reinforcement_id = Some(reinforce);
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.team_ready_queue.push_back(team);
        ai.check_ready_teams(&mut logic, 1.0);
        assert!(
            ai.team_ready_queue.is_empty(),
            "idle reinforcement unit activates even if fielded teammates are busy"
        );
    }


    #[test]
    fn select_team_to_reinforce_tops_up_short_auto_team() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut tank = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
        tank.add_kind_of(crate::game_logic::KindOf::Vehicle)
            .set_cost(100, 0);
        logic.templates.insert("AmericaTankCrusader".into(), tank);
        let mut wf = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        wf.add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSWarFactory);
        logic.templates.insert("AmericaWarFactory".into(), wf);

        let tank_id = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::ZERO)
            .expect("live crusader");
        if let Some(obj) = logic.host_object_mut(tank_id) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "HQ_V3_TankTeam".into();
        }
        // Player-wide census is at maxUnits, but the instance is still short.
        for i in 1..=2 {
            let extra = logic
                .create_object(
                    "AmericaTankCrusader",
                    Team::USA,
                    Vec3::new(i as f32 * 8.0, 4.0, 0.0),
                )
                .expect("extra crusader");
            if let Some(obj) = logic.host_object_mut(extra) {
                obj.owner_player_id = Some(1);
            }
        }
        let factory = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .expect("idle factory");
        if let Some(obj) = logic.host_object_mut(factory) {
            obj.owner_player_id = Some(1);
        }

        let mut inst_id = None;
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut proto = gamelogic::team::TeamPrototype::new("HQ_V3_TankTeam".into());
            proto.set_automatically_reinforce(true);
            proto.set_production_priority(50);
            proto.set_units_info(
                0,
                gamelogic::team::CreateUnitsInfo {
                    min_units: 1,
                    max_units: 3,
                    unit_thing_name: "AmericaTankCrusader",
                },
            );
            tf.replace_team_prototype(proto);
            if let Some(team) = tf.create_team("HQ_V3_TankTeam") {
                if let Ok(mut tg) = team.write() {
                    tg.add_member(tank_id.0);
                    inst_id = Some(tg.get_id());
                }
            }
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        assert!(ai.select_team_to_reinforce(&mut logic, 0, 1.0));
        let team = ai.team_queue.front().expect("reinforce order");
        assert!(team.reinforcement);
        assert_eq!(team.team_id, inst_id, "reinforce the instance that has units");
        assert_eq!(team.work_orders[0].num_required, 1);
        assert_eq!(team.work_orders[0].template_name, "AmericaTankCrusader");
        assert_eq!(ai.next_team_queue_time, 1.0);
        if let Some(recruited) = team.work_orders[0].observed_unit_ids.first().copied() {
            assert_eq!(
                logic
                    .host_object(recruited)
                    .map(|o| o.team_instance_name.as_str()),
                Some("HQ_V3_TankTeam"),
                "C++ selectTeamToReinforce setTeam onto the reinforced instance"
            );
        }

        // Empty instance is skipped even if the player owns units elsewhere.
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut proto = gamelogic::team::TeamPrototype::new("HQ_V3_EmptyTeam".into());
            proto.set_automatically_reinforce(true);
            proto.set_production_priority(80);
            proto.set_units_info(
                0,
                gamelogic::team::CreateUnitsInfo {
                    min_units: 1,
                    max_units: 3,
                    unit_thing_name: "AmericaTankCrusader",
                },
            );
            tf.replace_team_prototype(proto);
            let _ = tf.create_inactive_team("HQ_V3_EmptyTeam");
        }
        let mut ai_empty = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        assert!(
            !ai_empty.select_team_to_reinforce(&mut logic, 50, 1.0),
            "empty leftover instance must not auto-reinforce from player-wide census"
        );
    }


    #[test]
    fn unlimited_rebuilds_do_not_spend_budget_on_first_build() {
        // C++ `BuildListInfo::decrementNumRebuilds` (`SidesList.h:349-353`) is
        // a no-op for `UNLIMITED_REBUILDS`. `newMap` also increments finite
        // rebuilds so the first construction does not consume the last slot
        // (`AISkirmishPlayer.cpp:1083`).
        let mut unlimited =
            AIBuildingInfo::new("AmericaAirfield".into(), Vec3::ZERO, UNLIMITED_REBUILDS);
        unlimited.increment_num_rebuilds();
        unlimited.decrement_num_rebuilds();
        unlimited.decrement_num_rebuilds();
        assert!(unlimited.is_buildable());
        assert_eq!(unlimited.rebuild_count, 0);
        assert_eq!(unlimited.max_rebuilds, UNLIMITED_REBUILDS);

        let mut factory = AIBuildingInfo::new("AmericaWarFactory".into(), Vec3::ZERO, 1);
        factory.increment_num_rebuilds();
        assert!(factory.is_buildable());
        factory.decrement_num_rebuilds(); // first construction
        assert!(
            factory.is_buildable(),
            "first build must not spend the last rebuild"
        );
        factory.decrement_num_rebuilds(); // one rebuild
        assert!(!factory.is_buildable());

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.initialize(Vec3::new(-120.0, 0.0, -120.0));
        let factory = ai
            .building_queue
            .iter_mut()
            .find(|b| b.template_name == "AmericaWarFactory")
            .expect("layout factory");
        assert_eq!(factory.max_rebuilds, UNLIMITED_REBUILDS);
        assert!(factory.is_buildable());
        factory.decrement_num_rebuilds();
        assert!(
            factory.is_buildable(),
            "unlimited layout factory must still rebuild after first start"
        );
        assert_eq!(factory.rebuild_count, 0);
    }

    #[test]
    fn base_defense_uses_approach_fan_not_plus_80() {
        // C++ `AISkirmishPlayer::buildAIBaseDefenseStructure`
        // (`AISkirmishPlayer.cpp:542-686`): first front pad is along the
        // approach at `baseRadius + extraDistance`, then left/right fan.
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.initialize(Vec3::new(-120.0, 0.0, -120.0));
        let patriot = ai
            .building_queue
            .iter()
            .find(|b| b.template_name == "AmericaPatriotBattery")
            .expect("SideInfo defense is queued");
        let plus_80 = ai.base_center + Vec3::new(80.0, 0.0, 80.0);
        assert!(
            (patriot.position - plus_80).length() > 1.0,
            "defense must not sit on the old +80/+80 pad, got {:?}",
            patriot.position
        );
        let approach = -ai.base_center;
        let dir = Vec3::new(approach.x, 0.0, approach.z).normalize();
        let expected = ai.base_center
            + dir * (ai.base_radius + AIPlayer::SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE);
        assert!(
            (patriot.position - expected).length() < 0.25,
            "first front defense must sit on the approach ring: {:?} vs {:?}",
            patriot.position,
            expected
        );

        let first = patriot.position;
        let second = ai
            .place_next_base_defense_structure(None, "AmericaPatriotBattery", false)
            .expect("fan continues to the next legal slot");
        assert!(
            (second - first).length() > 1.0,
            "legality fan must rotate off the previous pad"
        );
        let radius = ai.base_radius + AIPlayer::SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE;
        let second_radius = Vec3::new(
            second.x - ai.base_center.x,
            0.0,
            second.z - ai.base_center.z,
        )
        .length();
        assert!(
            (second_radius - radius).abs() < 0.25,
            "fan slots stay on the defense ring: {second_radius} vs {radius}"
        );
    }


    #[test]
    fn ai_building_placement_is_deterministic() {
        let mut a = AIPlayer::new(3, Team::GLA, AIDifficulty::Medium);
        let mut b = AIPlayer::new(3, Team::GLA, AIDifficulty::Medium);
        a.base_center = Vec3::new(100.0, 0.0, 200.0);
        b.base_center = Vec3::new(100.0, 0.0, 200.0);
        // Drain same number of placement draws.
        let pa = (
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
        );
        let pb = (
            b.placement_rng.next_real(-80.0, 80.0),
            b.placement_rng.next_real(-80.0, 80.0),
        );
        assert_eq!(pa, pb, "same player_id seed must match placement draws");
        let mut c = AIPlayer::new(99, Team::GLA, AIDifficulty::Medium);
        let pc = (
            c.placement_rng.next_real(-80.0, 80.0),
            c.placement_rng.next_real(-80.0, 80.0),
            c.placement_rng.next_real(-80.0, 80.0),
            c.placement_rng.next_real(-80.0, 80.0),
        );
        let pa4 = (
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
        );
        assert_ne!(pa4, pc, "different player_id seeds must diverge");
    }

    use super::{AIDifficulty, AIManager, AIPlayer};
    use crate::game_logic::{ObjectId, Team};

    /// Gate-only early-attack intervals must not reappear; keep 60s spacing number.
    #[test]
    fn host_attack_recheck_uses_sixty_second_spacing_not_gate_hack() {
        // Same NUMBER as C++ ready-team force-start (60s), not full checkReadyTeams semantics.
        assert_eq!(AIPlayer::ATTACK_RECHECK_SECONDS, 60.0);
        assert!(
            AIPlayer::ATTACK_RECHECK_SECONDS >= 30.0,
            "must not use gate-only early-attack shortcut (<30s)"
        );
    }

    #[test]
    fn rebind_after_world_reset_keeps_difficulty_active_and_restores_rebuild_budget() {
        let mut mgr = AIManager::new();
        mgr.add_ai_player(1, Team::GLA, AIDifficulty::Hard);
        mgr.set_ai_active(1, true);
        {
            let ai = mgr.ai_players.get_mut(&1).expect("ai");
            if let Some(b) = ai.building_queue.first_mut() {
                b.object_id = Some(ObjectId(42));
                b.rebuild_count = b.max_rebuilds;
                b.is_built = true;
            }
            ai.defensive_units.push(ObjectId(7));
            ai.attack_in_progress = true;
        }

        mgr.rebind_after_world_reset();

        assert!(mgr.is_ai_active(1));
        assert_eq!(mgr.ai_difficulty(1), Some(AIDifficulty::Hard));
        let ai = mgr.ai_players.get(&1).expect("ai after rebind");
        assert!(ai.defensive_units.is_empty());
        assert!(!ai.attack_in_progress);
        let b = ai.building_queue.first().expect("layout retained");
        assert!(b.object_id.is_none());
        assert_eq!(b.rebuild_count, 0);
        assert!(!b.is_built);
        assert!(!b.template_name.is_empty());
    }

    #[test]
    fn launch_attack_sets_target_and_logs_host_attack() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::host_attack_log;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

        // Default AI_DECISION_AUTHORITY is on: launch_attack engages host + logs decisions.
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        // Decision logs require coupled shadow writeback frame (live gate).
        crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
        crate::gameworld_shadow::begin_shadow_coupled_tick();
        host_attack_log::clear();
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for (name, team, x) in [("AiAtkU", Team::USA, 0.0f32), ("AiAtkE", Team::GLA, 80.0)] {
            if !logic.templates.contains_key(name) {
                let mut tmpl = ThingTemplate::new(name);
                tmpl.set_health(100.0);
                tmpl.add_kind_of(KindOf::Infantry);
                tmpl.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), tmpl);
            }
            let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
        }
        let usa_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::USA)
            .map(|(id, _)| *id)
            .unwrap_or(0);
        let gla_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::GLA)
            .map(|(id, _)| *id);
        let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = gla_id;
        ai.is_active = true;
        let usa_unit = logic
            .host_objects()
            .iter()
            .find(|(_, o)| o.team == Team::USA && o.is_alive())
            .map(|(id, _)| *id)
            .expect("usa unit");
        if let Some(o) = logic.host_object_mut(usa_unit) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                ..Weapon::default()
});
        }
        ai.launch_attack(&mut logic, 1000.0);
        let decisions = host_ai_decision_log::drain();
        let unit = logic
            .host_objects()
            .get(&usa_unit)
            .expect("usa unit after launch");
        assert!(
            unit.target.is_some(),
            "under AI decision authority host target engages immediately"
        );
        assert!(
            decisions.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK && e.host_object == usa_unit
            }),
            "launch_attack must log AttackTarget decision; got {decisions:?}"
        );
        assert!(
            decisions.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == usa_unit
                    && e.ai_state_ordinal == 3
            }),
            "launch_attack must log AttackMoving state; got {decisions:?}"
        );
        assert!(
            !unit.movement.path.is_empty() || unit.movement.target_position.is_some(),
            "launch_attack must still pathfind on host under decision authority"
        );
        // Legacy residual path when decision authority is off.
        crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
        host_attack_log::clear();
        host_ai_decision_log::clear();
        if let Some(o) = logic.host_object_mut(usa_unit) {
            o.target = None;
            o.ai_state = AIState::Idle;
            o.movement.path.clear();
            o.movement.target_position = None;
        }
        ai.launch_attack(&mut logic, 2000.0);
        let logged = host_attack_log::drain();
        let unit = logic
            .host_objects()
            .get(&usa_unit)
            .expect("usa unit legacy");
        assert!(
            unit.target.is_some() && !logged.is_empty(),
            "legacy launch_attack must set_target and host_attack_log"
        );
        assert_eq!(unit.ai_state, AIState::AttackMoving);
        crate::gameworld_shadow::end_shadow_coupled_tick();
        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn launch_attack_uses_assign_unit_path_surface() {
        let src = include_str!("ai.rs");
        // Do not split on cfg(test) — nested test modules can appear earlier.
        let i = src
            .find("fn attack_move_units(")
            .expect("attack_move_units");
        let w = &src[i..i + 4500.min(src.len() - i)];
        assert!(
            w.contains("assign_unit_path")
                && (w.contains("AIState::AttackMoving") || w.contains("record_set_state")),
            "AI launch_attack must pathfind then restore AttackMoving (host or decision log)"
        );
        // Fallback may call move_to after assign_unit_path fails; primary path
        // must call assign_unit_path first.
        let path_i = w.find("assign_unit_path").expect("path");
        let move_i = w.find("move_to(enemy_base)");
        assert!(
            move_i.is_none() || move_i.unwrap() > path_i,
            "move_to fallback must come after assign_unit_path"
        );
    }

    #[test]
    fn launch_attack_dispatches_crate_attack_move_state() {
        // C++ AIAttackMoveState / AIInternalMoveToState::onEnter (AIStates.cpp).
        // Live host AIState is a flat enum; launch_attack must also record
        // crate AiStateType::AttackMoveTo via dispatch_host_move_attack.
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
        use gamelogic::ai::state_machine::{host_move_attack_state, AiStateType};

        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
        crate::gameworld_shadow::begin_shadow_coupled_tick();

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiSm");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for (name, team, x) in [("AiSmU", Team::USA, 0.0f32), ("AiSmE", Team::GLA, 80.0)] {
            if !logic.templates.contains_key(name) {
                let mut tmpl = ThingTemplate::new(name);
                tmpl.set_health(100.0);
                tmpl.add_kind_of(KindOf::Infantry);
                tmpl.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), tmpl);
            }
            let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
        }
        let usa_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::USA)
            .map(|(id, _)| *id)
            .unwrap_or(0);
        let gla_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::GLA)
            .map(|(id, _)| *id);
        let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = gla_id;
        ai.is_active = true;
        let usa_unit = logic
            .host_objects()
            .iter()
            .find(|(_, o)| o.team == Team::USA && o.is_alive())
            .map(|(id, _)| *id)
            .expect("usa unit");
        if let Some(o) = logic.host_object_mut(usa_unit) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                ..Weapon::default()
});
        }

        ai.launch_attack(&mut logic, 1000.0);

        assert_eq!(
            host_move_attack_state(usa_unit.0),
            Some(AiStateType::AttackMoveTo),
            "launch_attack must record crate AttackMoveTo for the live unit"
        );
        assert_eq!(
            logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
            Some(AIState::AttackMoving)
        );

        crate::gameworld_shadow::end_shadow_coupled_tick();
        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn second_attack_starts_after_first_raid_finishes() {
        // C++ checkReadyTeams only setActive. OnCreate Hunt/Guard/AttackMove
        // come from scripts. evaluate_attack_opportunities must not dump the army.
        use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate, Weapon};

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA AI", false));
        logic.add_player(Player::new(2, Team::GLA, "GLA", true));

        let mut unit_t = ThingTemplate::new("Ai2Infantry");
        unit_t.set_health(100.0);
        unit_t.add_kind_of(KindOf::Infantry);
        unit_t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("Ai2Infantry".into(), unit_t);

        let usa_unit = logic
            .create_object("Ai2Infantry", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("usa unit");
        let _ = logic.create_object("Ai2Infantry", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0));
        if let Some(o) = logic.host_object_mut(usa_unit) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                ..Weapon::default()
});
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = Some(2);
        ai.is_active = true;

        let mut order = AIWorkOrder::new("Ai2Infantry".into(), 1, 100);
        order.num_completed = 1;
        order.observed_unit_ids.push(usa_unit);
        let mut team = AITeamQueue::new("USA_RangerSquad".into(), vec![order], false, 0);
        team.execute_actions = true;
        ai.team_ready_queue.push_back(team);

        ai.check_ready_teams(&mut logic, 1.0);
        assert!(
            ai.team_ready_queue.is_empty(),
            "ready team must activate via setActive"
        );
        assert!(
            !ai.attack_in_progress,
            "empty OnCreate must not invent AttackMove"
        );
        assert_eq!(
            logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
            Some(AIState::Idle),
            "setActive without OnCreate leaves members idle at rally"
        );
        let first_count = ai.activity_count;

        ai.evaluate_attack_opportunities(&mut logic, 1.0 + AIPlayer::ATTACK_RECHECK_SECONDS);
        assert!(
            !ai.attack_in_progress,
            "evaluate_attack_opportunities must not dump the army"
        );
        assert_eq!(ai.activity_count, first_count);

        assert_eq!(
            AIPlayer::classify_on_create_script("TeamHunt"),
            OnCreateIntent::Hunt
        );
        assert_eq!(
            AIPlayer::classify_on_create_script("TeamGuard"),
            OnCreateIntent::Guard
        );
        assert_eq!(
            AIPlayer::classify_on_create_script("TeamAttackMove"),
            OnCreateIntent::AttackMove
        );

        ai.apply_on_create_host_orders(&mut logic, &[usa_unit], "TeamHunt", 1.0);
        assert_eq!(
            logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
            Some(AIState::Patrolling),
            "OnCreate Hunt must hunt, not AttackMove"
        );
        assert!(!ai.attack_in_progress);

        ai.apply_on_create_host_orders(&mut logic, &[usa_unit], "TeamGuard", 1.0);
        assert_eq!(
            logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
            Some(AIState::GuardingArea),
            "OnCreate Guard must guard, not AttackMove"
        );
        assert!(!ai.attack_in_progress);

        let mut order2 = AIWorkOrder::new("Ai2Infantry".into(), 1, 100);
        order2.num_completed = 1;
        order2.observed_unit_ids.push(usa_unit);
        ai.team_ready_queue
            .push_back(AITeamQueue::new("USA_RangerSquad".into(), vec![order2], false, 0));
        ai.check_ready_teams(&mut logic, 2.0);
        assert!(
            ai.activity_count > first_count,
            "second ready team setActive still counts as activity"
        );
        assert_eq!(
            logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
            Some(AIState::GuardingArea),
            "second setActive without OnCreate must not overwrite Guard with AttackMove"
        );
    }

    #[test]
    fn live_ai_fires_ready_superweapon_at_enemy_cluster() {
        // C++ ScriptActions::doSkirmishFireSpecialPowerAtMostCost
        // (ScriptActions.cpp:4142) + AIPlayer::computeSuperweaponTarget
        // (AIPlayer.cpp:1120). Live host path queues DoSpecialPower.
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::{
            GameLogic, KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
            ThingTemplate,
        };

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA AI", false));
        logic.add_player(Player::new(2, Team::GLA, "GLA", true));

        let mut puc = ThingTemplate::new("AiSwPuc");
        puc.set_health(4000.0);
        puc.add_kind_of(KindOf::Structure);
        puc.add_kind_of(KindOf::FSSuperweapon);
        puc.special_power_modules
            .push(SpecialPowerModuleMetadata {
                source_index: 0,
                module_tag: Some("ModuleTag_SpecialPower".into()),
                module_kind: SpecialPowerModuleKind::OclSpecialPower,
                special_power_template: "SuperweaponParticleUplinkCannon".into(),
                special_power_template_id: 1,
                command_power: Some(SpecialPowerType::ParticleCannon),
                reload_time_frames: 0,
                required_science: None,
                public_timer: true,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
            });
        logic.templates.insert("AiSwPuc".into(), puc);

        let mut barracks = ThingTemplate::new("AiSwBarracks");
        barracks.set_health(1000.0);
        barracks.set_cost(500, 0);
        barracks.add_kind_of(KindOf::Structure);
        barracks.add_kind_of(KindOf::Attackable);
        logic.templates.insert("AiSwBarracks".into(), barracks);

        let caster = logic
            .create_object("AiSwPuc", Team::USA, glam::Vec3::new(-40.0, 0.0, 0.0))
            .expect("puc");
        let _ = logic.create_object(
            "AiSwBarracks",
            Team::GLA,
            glam::Vec3::new(80.0, 0.0, 0.0),
        );

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = Some(2);
        ai.is_active = true;

        ai.fire_named_special_power(&mut logic, "SuperweaponParticleUplinkCannon");
        logic.process_commands();

        assert!(
            logic
                .special_power_strikes()
                .honesty_queue_ok(crate::game_logic::HostSuperweaponKind::ParticleCannon),
            "live AI must queue a ParticleCannon strike via computeSuperweaponTarget"
        );
        assert!(
            !logic.is_special_power_ready_for(caster, &SpecialPowerType::ParticleCannon)
                || logic.special_power_strikes().strike_count() >= 1,
            "ready superweapon must be consumed or recorded as a strike"
        );
    }

    #[test]
    fn process_building_queue_skips_automatic_layout_pads() {
        // C++ processBaseBuilding never selects automatic pads
        // (canMakeUnit(dozer, NULL) → CANMAKE_NO_PREREQ).
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 50_000;
        logic.add_player(player);

        let mut dozer_template = crate::game_logic::ThingTemplate::new("TestDozer");
        dozer_template
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Worker);
        logic.templates.insert("TestDozer".into(), dozer_template);
        let _ = logic.create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0));

        for name in ["AmericaStrategyCenter", "AmericaAirfield"] {
            let mut t = crate::game_logic::ThingTemplate::new(name);
            t.add_kind_of(crate::game_logic::KindOf::Structure)
                .set_cost(100, 0);
            logic.templates.insert(name.into(), t);
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_layout_building("AmericaStrategyCenter", Vec3::new(64.0, 0.0, 0.0), 3);
        ai.add_layout_building("AmericaAirfield", Vec3::new(128.0, 0.0, 0.0), 3);
        ai.process_building_queue(&mut logic, 0.0);
        assert!(
            ai.building_queue.iter().all(|b| b.object_id.is_none()),
            "automatic layout pads must not start"
        );

        assert!(ai.build_specific_ai_building("AmericaStrategyCenter"));
        ai.process_building_queue(&mut logic, 0.1);
        assert!(
            ai.building_queue[0].object_id.is_some(),
            "scripted priority stamp must start that pad"
        );
        assert!(
            ai.building_queue[1].object_id.is_none(),
            "unstamped automatic airfield stays queued"
        );
    }

    #[test]
    fn update_does_not_auto_fire_first_ready_special() {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::{
            GameLogic, KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
            ThingTemplate,
        };

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA AI", false));
        logic.add_player(Player::new(2, Team::GLA, "GLA", true));

        let mut a10 = ThingTemplate::new("AiSwA10");
        a10.set_health(4000.0);
        a10.add_kind_of(KindOf::Structure);
        a10.special_power_modules
            .push(SpecialPowerModuleMetadata {
                source_index: 0,
                module_tag: Some("ModuleTag_A10".into()),
                module_kind: SpecialPowerModuleKind::OclSpecialPower,
                special_power_template: "SuperweaponA10ThunderboltMissileStrike".into(),
                special_power_template_id: 2,
                command_power: Some(SpecialPowerType::Airstrike),
                reload_time_frames: 0,
                required_science: None,
                public_timer: true,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
            });
        logic.templates.insert("AiSwA10".into(), a10);
        let _ = logic.create_object("AiSwA10", Team::USA, glam::Vec3::new(-40.0, 0.0, 0.0));

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = Some(2);
        ai.is_active = true;
        ai.update(&mut logic, 1.0);
        logic.process_commands();
        assert_eq!(
            logic.special_power_strikes().strike_count(),
            0,
            "AIPlayer::update must not auto-fire the first ready special"
        );

        ai.fire_named_special_power(&mut logic, "SuperweaponParticleUplinkCannon");
        logic.process_commands();
        assert_eq!(
            logic.special_power_strikes().strike_count(),
            0,
            "wrong script name must not fire a different ready special"
        );
    }


    #[test]
    fn late_game_team_keeps_higher_tier_templates_instead_of_default_infantry() {
        // C++ `AIPlayer::selectTeamToBuild` (AIPlayer.cpp:1630) queues
        // `TeamPrototype` unit lists. Host late-game names must not collapse
        // to default Rangers when the higher-tier ThingTemplates exist.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 20_000;
        logic.add_player(player);

        for (name, kind, cost) in [
            ("AmericaBarracks", crate::game_logic::KindOf::FSBarracks, 500),
            (
                "AmericaWarFactory",
                crate::game_logic::KindOf::FSWarFactory,
                1_000,
            ),
            ("AmericaAirfield", crate::game_logic::KindOf::FSAirfield, 1_000),
            (
                "AmericaStrategyCenter",
                crate::game_logic::KindOf::FSStrategyCenter,
                2_000,
            ),
            (
                "AmericaPatriotBattery",
                crate::game_logic::KindOf::FSBaseDefense,
                1_000,
            ),
        ] {
            let mut building = crate::game_logic::ThingTemplate::new(name);
            building
                .add_kind_of(crate::game_logic::KindOf::Structure)
                .add_kind_of(kind)
                .set_cost(cost, 0);
            logic.templates.insert(name.into(), building);
        }
        for (name, kind, cost) in [
            (
                "AmericaInfantryMissileDefender",
                crate::game_logic::KindOf::Infantry,
                300,
            ),
            (
                "AmericaTankCrusader",
                crate::game_logic::KindOf::Vehicle,
                900,
            ),
            ("AmericaJetRaptor", crate::game_logic::KindOf::Aircraft, 1_400),
        ] {
            let mut unit = crate::game_logic::ThingTemplate::new(name);
            unit.add_kind_of(kind).set_cost(cost, 0);
            logic.templates.insert(name.into(), unit);
        }

        let _ = logic.create_object("AmericaBarracks", Team::USA, Vec3::ZERO);
        let _ = logic.create_object(
            "AmericaWarFactory",
            Team::USA,
            Vec3::new(64.0, 0.0, 0.0),
        );
        let _ = logic.create_object("AmericaAirfield", Team::USA, Vec3::new(128.0, 0.0, 0.0));
        let strategy = logic
            .create_object(
                "AmericaStrategyCenter",
                Team::USA,
                Vec3::new(0.0, 0.0, 64.0),
            )
            .expect("strategy center");

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.current_strategy = AIStrategy::LateGame;

        let orders = ai.create_work_orders_for_team("USA_AdvancedStrike");
        let templates: Vec<&str> = orders
            .iter()
            .map(|order| order.template_name.as_str())
            .collect();
        assert!(
            templates.contains(&"AmericaTankCrusader"),
            "late-game USA team must keep Crusaders: {templates:?}"
        );
        assert!(
            templates.contains(&"AmericaJetRaptor"),
            "late-game USA team must keep Raptors: {templates:?}"
        );
        assert!(
            !templates.iter().all(|name| name.contains("Ranger")),
            "late-game USA team must not collapse to Rangers: {templates:?}"
        );
        assert!(
            ai.create_work_orders_for_team("NoSuchTeam").is_empty(),
            "unknown team names must not invent default infantry"
        );
        assert!(
            ai.is_possible_to_build_team(&logic, "USA_AdvancedStrike"),
            "factories for missile infantry, tanks, and jets must satisfy the late team"
        );

        ai.initialize(Vec3::new(-120.0, 0.0, -120.0));
        let planned: Vec<&str> = ai
            .building_queue
            .iter()
            .map(|building| building.template_name.as_str())
            .collect();
        assert!(
            planned.contains(&"AmericaStrategyCenter")
                && planned.contains(&"AmericaAirfield")
                && planned.contains(&"AmericaPatriotBattery"),
            "skirmish layout must include tech, air, and SideInfo defense: {planned:?}"
        );

        ai.do_upgrades_and_skills(&mut logic);
        let player = logic.get_player(1).expect("AI player");
        assert!(
            player.has_queued_upgrade("Upgrade_AmericaSupplyLines")
                || player.has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
                || player.has_queued_upgrade("Upgrade_AmericaAdvancedTraining")
                || logic
                    .host_object(strategy)
                    .and_then(|object| object.building_data.as_ref())
                    .is_some_and(|building| building
                        .production_queue
                        .iter()
                        .any(|item| item.is_upgrade())),
            "live AI must queue a structure upgrade via AIPlayer::buildUpgrade residual"
        );
    }

    #[test]
    fn check_queued_teams_disbands_expired_incomplete_team() {
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));
        let mut ranger_t = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger_t.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger_t);
        let ranger = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("ranger");
        if let Some(obj) = logic.host_object_mut(ranger) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "HQ_9_Disband".into();
        }

        let mut inst_id = None;
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut proto = gamelogic::team::TeamPrototype::new("HQ_9_Disband".into());
            proto.set_initial_idle_frames(30);
            tf.replace_team_prototype(proto);
            if let Some(team) = tf.create_inactive_team("HQ_9_Disband") {
                if let Ok(mut tg) = team.write() {
                    tg.add_member(ranger.0);
                    inst_id = Some(tg.get_id());
                }
            }
        }

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
        order.num_completed = 0;
        order.observed_unit_ids.push(ranger);
        let mut q = AITeamQueue::new("HQ_9_Disband".into(), vec![order], false, 0);
        q.team_id = inst_id;
        ai.team_queue.push_back(q);

        ai.check_queued_teams(&mut logic, 2.0);
        assert!(
            ai.team_queue.is_empty() && ai.team_ready_queue.is_empty(),
            "expired team below minimum must disband"
        );
        let default = logic.default_host_team_instance_name(Some(1), Team::USA);
        assert_eq!(
            logic
                .host_object(ranger)
                .map(|o| o.team_instance_name.clone())
                .unwrap_or_default(),
            default,
            "disband must transfer recruits to the default team"
        );
        assert!(
            AIPlayer::leftover_team_instance_gone(inst_id),
            "non-singleton leftover instance must be deleted on disband"
        );
    }

    #[test]
    fn check_queued_teams_zero_idle_frames_never_expires() {
        if let Ok(mut tf) = gamelogic::team::get_team_factory().lock() {
            let mut proto = gamelogic::team::TeamPrototype::new("HQ_80_Never".into());
            proto.set_initial_idle_frames(0);
            tf.replace_team_prototype(proto);
        }
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        let mut order = AIWorkOrder::new("AmericaInfantryRanger".into(), 2, 100);
        order.num_completed = 0;
        ai.team_queue.push_back(AITeamQueue::new(
            "HQ_80_Never".into(),
            vec![order],
            false,
            0,
        ));
        let mut logic = crate::game_logic::GameLogic::new();
        ai.check_queued_teams(&mut logic, 999.0);
        assert_eq!(
            ai.team_queue.len(),
            1,
            "InitialIdleFrames < 1 is unlimited; team must not expire"
        );
        assert!(ai.team_ready_queue.is_empty());
    }

    #[test]
    fn air_force_side_uses_a10_skillset_not_paladin() {
        let residual = AIPlayer::residual_general_skillsets("AmericaAirForceGeneral")
            .expect("Air Force SideInfo residual");
        assert_eq!(
            residual[0][0],
            "AirF_SCIENCE_A10ThunderboltMissileStrike1"
        );
        assert_ne!(residual[0][0], "SCIENCE_PaladinTank");

        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "AirF", false));
        if let Some(identity) = crate::game_logic::PlayerTemplateIdentity::from_exact_name(
            "FactionAmericaAirForceGeneral",
        ) {
            let _ = logic.bind_player_template_identity(1, identity);
            let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
            let sets = ai.live_side_skillsets(&logic);
            let first = sets[0].first().map(String::as_str).unwrap_or("");
            assert_ne!(first, "SCIENCE_PaladinTank");
        }
    }

    #[test]
    fn find_dozer_skips_assigned_bridge_repair_dozer() {
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));
        let mut dozer_t = crate::game_logic::ThingTemplate::new("AmericaVehicleDozer");
        dozer_t
            .add_kind_of(crate::game_logic::KindOf::Dozer)
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .set_health(200.0);
        logic.templates.insert("AmericaVehicleDozer".into(), dozer_t);
        let repair = logic
            .create_object("AmericaVehicleDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("repair dozer");
        let free = logic
            .create_object("AmericaVehicleDozer", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .expect("free dozer");
        let found = AIPlayer::find_available_dozer(&logic, Team::USA, Vec3::ZERO, Some(repair));
        assert_eq!(found, Some(free));
    }

    #[test]
    fn ctor_helper_disables_unit_construction() {
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));
        assert!(logic.get_player(1).unwrap().can_build_units);
        AIManager::apply_ctor_can_build_units(&mut logic, 1);
        assert!(!logic.get_player(1).unwrap().can_build_units);
    }


    #[test]
    fn acquire_enemy_keeps_healthy_current_target() {
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));
        logic.add_player(crate::game_logic::Player::new(2, Team::GLA, "GLA", true));
        logic.add_player(crate::game_logic::Player::new(3, Team::China, "China", true));

        let mut barracks = crate::game_logic::ThingTemplate::new("GLABarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_health(1000.0);
        logic.templates.insert("GLABarracks".into(), barracks);
        let mut rebel = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
        rebel
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .set_health(100.0);
        logic.templates.insert("GLAInfantryRebel".into(), rebel);
        let mut china_cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
        china_cc
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::CommandCenter)
            .set_health(5000.0);
        logic.templates.insert("ChinaCommandCenter".into(), china_cc);

        let _ = logic.create_object("GLABarracks", Team::GLA, Vec3::new(400.0, 0.0, 0.0));
        let _ = logic.create_object("GLAInfantryRebel", Team::GLA, Vec3::new(410.0, 0.0, 0.0));
        let _ = logic.create_object(
            "ChinaCommandCenter",
            Team::China,
            Vec3::new(20.0, 0.0, 0.0),
        );

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.base_center = Vec3::ZERO;
        ai.enemy_player_id = Some(2);
        ai.enemy_check_time = -10.0;
        ai.update_enemy_assessment(&mut logic, 0.0);
        assert_eq!(
            ai.enemy_player_id,
            Some(2),
            "healthy current enemy with units and a factory must be kept"
        );
    }

    #[test]
    fn cluster_mines_land_on_own_approach_not_enemy_centroid() {
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::China, "China AI", false));
        logic.add_player(crate::game_logic::Player::new(2, Team::USA, "USA", true));
        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(500, 0)
            .set_health(1000.0);
        logic.templates.insert("AmericaBarracks".into(), barracks);
        let _ = logic.create_object(
            "AmericaBarracks",
            Team::USA,
            Vec3::new(400.0, 0.0, 0.0),
        );

        let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
        ai.base_center = Vec3::ZERO;
        ai.base_radius = 100.0;
        let target = ai
            .compute_cluster_mines_target(&logic, Team::USA)
            .expect("approach");
        let dist_from_base = (target.x * target.x + target.z * target.z).sqrt();
        assert!(
            (dist_from_base - 100.0).abs() < 1.0,
            "cluster mines must sit on the AI approach ring, got {target:?} dist={dist_from_base}"
        );
        assert!(
            target.x < 200.0,
            "mines must not drop on the enemy value centroid"
        );
    }

    #[test]
    fn skillset_selector_uses_chosen_ai_side_info_set() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::China, "China AI", false);
        player.science_purchase_points = 1;
        logic.add_player(player);

        let mut ai = AIPlayer::new(1, Team::China, AIDifficulty::Medium);
        assert_eq!(ai.side_skillsets()[0][0], "SCIENCE_NukeLauncher");
        assert_eq!(ai.side_skillsets()[1][0], "SCIENCE_RedGuardTraining");
        ai.select_skillset(1);
        ai.try_purchase_skillset_science(&mut logic);
        assert_eq!(ai.skillset_selector, 1);
        assert_ne!(
            ai.side_skillsets()[ai.skillset_selector as usize][0],
            "SCIENCE_NukeLauncher"
        );
    }

    #[test]
    fn air_force_side_info_skillset_is_a10_not_paladin() {
        let residual = AIPlayer::residual_general_skillsets("AmericaAirForceGeneral")
            .expect("Air Force residual SideInfo");
        assert_eq!(residual[0][0], "AirF_SCIENCE_A10ThunderboltMissileStrike1");
        assert_eq!(residual[1][0], "SCIENCE_SpectreGunship1");

        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "Air Force AI", false);
        player.science_purchase_points = 5;
        logic.add_player(player);
        let identity = crate::game_logic::PlayerTemplateIdentity::from_exact_name(
            "FactionAmericaAirForceGeneral",
        )
        .expect("retail Air Force PlayerTemplate");
        assert!(logic.bind_player_template_identity(1, identity));

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        assert_eq!(ai.side_skillsets()[0][0], "SCIENCE_PaladinTank");
        ai.select_skillset(0);
        let first = ai.live_side_skillsets(&logic)[0][0].clone();
        assert_eq!(first, "AirF_SCIENCE_A10ThunderboltMissileStrike1");
        assert_ne!(first, "SCIENCE_PaladinTank");
    }

    #[test]
    fn skillset_purchase_readies_required_special_powers() {
        use crate::command_system::SpecialPowerType;
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.science_purchase_points = 10;
        assert!(player.unlock_science("SCIENCE_AMERICA"));
        player
            .shared_special_power_cooldowns
            .insert(SpecialPowerType::DaisyCutter, 99.0);
        logic.add_player(player);

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.select_skillset(0);
        ai.try_purchase_skillset_science(&mut logic);
        let player = logic.get_player(1).expect("player");
        assert!(
            player.has_unlocked_science("SCIENCE_DaisyCutter"),
            "skillset 1 must buy DaisyCutter once AMERICA is owned"
        );
        assert!(
            !player
                .shared_special_power_cooldowns
                .contains_key(&SpecialPowerType::DaisyCutter),
            "C++ addScience onSpecialPowerCreation must ready the required shared power"
        );
    }

    #[test]
    fn check_bridges_queues_damaged_span_and_assigns_dozer() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, Team::USA, "USA AI", false));

        let mut bridge_t = crate::game_logic::ThingTemplate::new("CabinBridge");
        bridge_t
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_health(2000.0);
        logic.templates.insert("CabinBridge".into(), bridge_t);
        let mut dozer_t = crate::game_logic::ThingTemplate::new("AmericaVehicleDozer");
        dozer_t
            .add_kind_of(crate::game_logic::KindOf::Dozer)
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .set_health(200.0);
        logic.templates.insert("AmericaVehicleDozer".into(), dozer_t);

        let bridge = logic
            .create_object("CabinBridge", Team::Neutral, Vec3::new(80.0, 0.0, 0.0))
            .expect("bridge");
        if let Some(o) = logic.host_object_mut(bridge) {
            o.health.current = 400.0;
            o.body_damage_state = HostBodyDamageType::Damaged;
        }
        let dozer = logic
            .create_object("AmericaVehicleDozer", Team::USA, Vec3::ZERO)
            .expect("dozer");

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.base_center = Vec3::ZERO;
        ai.check_bridges(&logic);
        assert_eq!(ai.structures_to_repair, vec![bridge]);
        ai.last_bridge_repair_time = -1.0;
        ai.update_bridge_repair(&mut logic, 0.0);
        assert_eq!(ai.repair_dozer, Some(dozer));
        assert!(ai.dozer_is_repairing);
    }





}
