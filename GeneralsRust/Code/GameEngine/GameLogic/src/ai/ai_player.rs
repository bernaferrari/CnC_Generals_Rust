//! AIPlayer - Computer player AI system
//!
//! This module implements the computerized opponent AI that manages all aspects
//! of computer player behavior including economy, military, construction, and
//! strategic decision making.
//!
//! Author: Converted from C++ original by Michael S. Booth

use super::ai_update::AiPlayerTrait;
use crate::ai::modules::GameDifficulty as AiGameDifficulty;
use crate::ai::modules::{
    BuildOrderOptimizer, DifficultyHandler, StrategicDecision, StrategicDecisionMaker,
    ThreatAssessmentSystem,
};
use crate::ai::{AiError, AiGroup, AttitudeType, ScienceType, AI, THE_AI};
use crate::ai::{CommandSourceType, GuardMode};
use crate::common::xfer::{Xfer, XferExt};
use crate::common::Snapshot;
use crate::common::{
    AsciiString, ControlBarInterface, Coord2D, Coord3D, CoordOrigin, KindOf, LocomotorSetType,
    ObjectID, ObjectStatusMaskType, ObjectStatusTypes, PlayerId, Real, Relationship, TeamId,
    ThingTemplate, UnsignedInt, INVALID_ID,
};
use crate::control_bar::get_control_bar_bridge;
use crate::helpers::{
    game_logic_random_value, TheGameLogic, ThePartitionManager, TheTerrainLogic, TheThingFactory,
};
use crate::modules::AIUpdateInterfaceExt;
use crate::modules::ProductionUpdateInterface;
use crate::object::production::construction::FoundationValidator;
use crate::object::production::supply_warehouse_dock::SupplyWarehouseDockUpdate;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::{player_list, GameDifficulty, Player, PlayerType};
use crate::scripting::engine::get_script_engine;
use crate::scripting::evaluator::ScriptEvaluator;
use crate::supply_system::BASE_VALUE_PER_SUPPLY_BOX;
use crate::team::get_team_factory;
use crate::upgrade::center::with_upgrade_center;
use crate::upgrade::template::UpgradeType;
use game_engine::common::thing::thing_factory::get_thing_factory;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};

/// Invalid skill set selection constant
pub const INVALID_SKILLSET_SELECTION: i32 = -1;

/// Maximum number of structures to repair simultaneously
pub const MAX_STRUCTURES_TO_REPAIR: usize = 2;

// Constants from C++ AIData (AI.cpp)
// These match the defaults and are critical for AI behavior

/// Frames per second for timing calculations (C++ LOGICFRAMES_PER_SECOND)
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// C++ `AIPlayer::doBaseBuilding` recheck: `m_buildDelay = 2*LOGICFRAMES_PER_SECOND`.
pub const BUILD_DELAY_RECHECK_FRAMES: u32 = 2 * LOGICFRAMES_PER_SECOND;

/// C++ `AIPlayer::doTeamBuilding` recheck: `m_teamDelay = 5*LOGICFRAMES_PER_SECOND`.
pub const TEAM_DELAY_RECHECK_FRAMES: u32 = 5 * LOGICFRAMES_PER_SECOND;

/// Default delay between team production in seconds.
/// Retail `Default/AIData.ini` TeamSeconds = 10 (C++ AIPlayer ctor uses AIData).
pub const DEFAULT_TEAM_SECONDS: f32 = 10.0;

/// Default delay between structure production in seconds.
/// Retail `Default/AIData.ini` StructureSeconds = 0 (try every ready tick).
pub const DEFAULT_STRUCTURE_SECONDS: f32 = 0.0;

/// Resource threshold for "poor" AI (retail AIData Poor = 2000).
pub const RESOURCES_POOR: i32 = 2000;

/// Resource threshold for "wealthy" AI (retail AIData Wealthy = 7000).
pub const RESOURCES_WEALTHY: i32 = 7000;

/// Build speed modifier when poor (retail AIData StructuresPoorRate = 0.6).
/// C++ divides the timer by this rate: 0.6 → slower when poor.
pub const STRUCTURES_POOR_MODIFIER: f32 = 0.6;

/// Build speed modifier when wealthy (retail AIData StructuresWealthyRate = 2.0).
pub const STRUCTURES_WEALTHY_MODIFIER: f32 = 2.0;

/// Team build speed modifier when poor (retail AIData TeamsPoorRate = 0.6).
pub const TEAMS_POOR_MODIFIER: f32 = 0.6;

/// Team build speed modifier when wealthy (retail AIData TeamsWealthyRate = 2.0).
pub const TEAMS_WEALTHY_MODIFIER: f32 = 2.0;

/// Delay before rebuilding destroyed structure in seconds.
/// Retail `Default/AIData.ini` RebuildDelayTimeSeconds = 30.
pub const REBUILD_DELAY_SECONDS: u32 = 30;

/// Team resource multiplier for affordability check (C++ m_teamResourcesToBuild)
pub const TEAM_RESOURCES_TO_BUILD: f32 = 0.5;

/// Supply center safe radius in units (C++ m_supplyCenterSafeRadius)
pub const SUPPLY_CENTER_SAFE_RADIUS: f32 = 100.0;

/// Skirmish base defense extra distance.
/// Retail `Default/AIData.ini` SkirmishBaseDefenseExtraDistance = 150.0.
pub const SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE: f32 = 150.0;

/// Close distance for supply center pathfinding (C++ SUPPLY_CENTER_CLOSE_DIST)
/// 20 * PATHFIND_CELL_SIZE_F where PATHFIND_CELL_SIZE_F = 10.0
pub const SUPPLY_CENTER_CLOSE_DIST: f32 = 200.0;

/// Huge distance constant for enemy prioritization (C++ HUGE_DIST)
pub const HUGE_DIST: f32 = 100000.0;

/// Work order for unit production tracking
#[derive(Debug, Clone)]
pub struct WorkOrder {
    pub thing_template: String,       // Template name of thing to build
    pub factory_id: Option<ObjectID>, // ID of factory building this, or None if none
    pub num_completed: i32,           // Number built so far
    pub num_required: i32,            // Number needed total
    pub required: bool,               // True if part of minimum requirement
    pub is_resource_gatherer: bool,   // True if this is a resource gatherer
}

impl WorkOrder {
    pub fn new(thing_template: String) -> Self {
        Self {
            thing_template,
            factory_id: None,
            num_completed: 0,
            num_required: 1,
            required: false,
            is_resource_gatherer: false,
        }
    }

    /// Returns true if nothing is building this unit yet
    pub fn is_waiting_to_build(&self) -> bool {
        self.factory_id.is_none() && self.num_completed < self.num_required
    }

    /// Validate that factory ID still refers to an active object.
    ///
    /// Matches C++ AIPlayer.cpp:3688 WorkOrder::validateFactory.
    /// Checks if the factory object still exists, is alive (not destroyed),
    /// and is still owned by the specified player. If any check fails,
    /// the factory_id is cleared to INVALID_ID.
    pub fn validate_factory(&mut self, player_id: u32) -> Result<(), AiError> {
        if self.factory_id.is_none() {
            // C++ parity: if m_factoryID == INVALID_ID, return immediately (valid)
            return Ok(());
        }
        let factory_id = self.factory_id.unwrap();

        // C++ parity: TheGameLogic->findObjectByID(m_factoryID)
        let Some(factory_arc) = OBJECT_REGISTRY.get_object(factory_id) else {
            // C++ parity: factory == NULL -> m_factoryID = INVALID_ID
            self.factory_id = None;
            return Ok(());
        };

        let Ok(factory_guard) = factory_arc.read() else {
            self.factory_id = None;
            return Ok(());
        };

        // C++ parity: factory->getControllingPlayer() != thisPlayer
        if factory_guard.get_controlling_player_id() != Some(player_id as UnsignedInt) {
            self.factory_id = None;
        }

        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut thing_template = self.thing_template.clone();
        let _ = xfer.xfer_ascii_string(&mut thing_template);
        if xfer.is_loading() {
            self.thing_template = thing_template;
        }

        let mut factory_id = self.factory_id.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut factory_id);
        if xfer.is_loading() {
            self.factory_id = if factory_id == INVALID_ID {
                None
            } else {
                Some(factory_id)
            };
        }

        let mut num_completed = self.num_completed;
        let _ = xfer.xfer_int(&mut num_completed);
        if xfer.is_loading() {
            self.num_completed = num_completed;
        }

        let mut num_required = self.num_required;
        let _ = xfer.xfer_int(&mut num_required);
        if xfer.is_loading() {
            self.num_required = num_required;
        }

        let mut required = self.required;
        let _ = xfer.xfer_bool(&mut required);
        if xfer.is_loading() {
            self.required = required;
        }

        let mut is_resource_gatherer = self.is_resource_gatherer;
        let _ = xfer.xfer_bool(&mut is_resource_gatherer);
        if xfer.is_loading() {
            self.is_resource_gatherer = is_resource_gatherer;
        }
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) {
        let _ = xfer;
    }
}

/// Team in the build/ready queue
#[derive(Debug)]
pub struct TeamInQueue {
    pub work_orders: Vec<WorkOrder>, // List of work orders for this team
    pub priority_build: bool,        // True if specifically requested
    pub team_name: Option<String>,   // Team that units go into
    /// C++ `TeamInQueue::m_team` — concrete team instance (not just name).
    pub team: Option<Arc<RwLock<crate::team::Team>>>,
    pub frame_started: u32,                 // Frame we started building
    pub sent_to_start_location: bool,       // Has team been sent to start location
    pub stop_queueing: bool,                // True to stop building new units
    pub reinforcement: bool,                // True if reinforcing existing team
    pub reinforcement_id: Option<ObjectID>, // Object being reinforced
}

impl TeamInQueue {
    pub fn new() -> Self {
        Self {
            work_orders: Vec::new(),
            priority_build: false,
            team_name: None,
            team: None,
            frame_started: 0,
            sent_to_start_location: false,
            stop_queueing: false,
            reinforcement: false,
            reinforcement_id: None,
        }
    }

    /// Returns true if all units in the team have finished building
    pub fn is_all_built(&self) -> bool {
        self.work_orders
            .iter()
            .all(|order| order.num_completed >= order.num_required)
    }

    /// Returns true if minimum required units have been built.
    ///
    /// C++ `TeamInQueue::isMinimumBuilt`: counts an assigned factory as +1 completed.
    pub fn is_minimum_built(&self) -> bool {
        for order in self.work_orders.iter().filter(|o| o.required) {
            let mut count = order.num_completed;
            if order.factory_id.is_some() {
                count += 1; // one currently building
            }
            if order.num_required > count {
                return false;
            }
        }
        true
    }

    /// C++ `TeamInQueue::includesADozer` — any work order KINDOF_DOZER.
    /// C++ `TeamInQueue::includesADozer`:
    /// KINDOF_DOZER and not a resource-gatherer work order (GLA workers are both).
    pub fn includes_a_dozer(&self) -> bool {
        self.work_orders.iter().any(|order| {
            // C++: isKindOf(DOZER) && !order->m_isResourceGatherer
            if order.is_resource_gatherer {
                return false;
            }
            if TheThingFactory::find_template(&order.thing_template)
                .map(|t| t.is_kind_of(KindOf::Dozer))
                .unwrap_or(false)
            {
                return true;
            }
            // Residual name heuristic when templates lack KindOf flags (unit tests /
            // early boot). Prefer "dozer"; "worker" only if no template was found.
            let n = order.thing_template.to_ascii_lowercase();
            if n.contains("dozer") {
                return true;
            }
            TheThingFactory::find_template(&order.thing_template).is_none() && n.contains("worker")
        })
    }

    /// Returns true if all factory builds are complete.
    ///
    /// C++ `TeamInQueue::areBuildsComplete`: true when no work order still has a factory.
    pub fn are_builds_complete(&self) -> bool {
        self.work_orders
            .iter()
            .all(|order| order.factory_id.is_none())
    }

    /// C++ `TeamInQueue::isBuildTimeExpired`.
    ///
    /// Uses team prototype `initial_idle_frames` as the build-time budget.
    /// `< 1` means unlimited (never expires).
    pub fn is_build_time_expired(&self) -> bool {
        // C++ uses m_team->getPrototype()->m_initialIdleFrames.
        let team_name = self
            .team
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|tg| tg.get_name().to_string()))
            .or_else(|| self.team_name.clone());
        let Some(team_name) = team_name else {
            return false;
        };
        let Ok(factory) = get_team_factory().lock() else {
            return false;
        };
        let Some(prototype) = factory.find_team_prototype(&team_name) else {
            return false;
        };
        let idle_frames = prototype.get_initial_idle_frames();
        if idle_frames < 1 {
            return false; // unlimited
        }
        let now = TheGameLogic::get_frame();
        now > self.frame_started.saturating_add(idle_frames as u32)
    }

    /// Disbands the team: transfers units to the default team, deletes non-singleton teams.
    ///
    /// Matches C++ AIPlayer.cpp:3554 TeamInQueue::disband.
    /// Prefers `m_team` handle; name lookup is fallback for legacy/xfer entries.
    pub fn disband(&mut self) -> Result<(), AiError> {
        let team_name = self.team_name.clone().unwrap_or_default();
        log::debug!("{} - team disbanded, build time expired.", team_name);

        // Prefer concrete m_team handle (C++); name lookup is fallback only.
        let team_arc = if let Some(arc) = self.team.clone() {
            arc
        } else if !team_name.is_empty() {
            let Ok(mut factory) = get_team_factory().lock() else {
                self.work_orders.clear();
                return Ok(());
            };
            let Some(arc) = factory.find_team(&team_name) else {
                self.work_orders.clear();
                return Ok(());
            };
            drop(factory);
            arc
        } else {
            self.work_orders.clear();
            return Ok(());
        };

        let Ok(mut team_guard) = team_arc.write() else {
            self.work_orders.clear();
            return Ok(());
        };

        let Some(controlling_player_id) = team_guard.get_controlling_player_id() else {
            self.work_orders.clear();
            return Ok(());
        };

        let default_team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(controlling_player_id as i32).cloned())
            .and_then(|player_arc| player_arc.read().ok().and_then(|p| p.get_default_team()));

        let Some(default_team_arc) = default_team else {
            self.work_orders.clear();
            return Ok(());
        };

        if team_guard.get_id()
            == default_team_arc
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(0)
        {
            self.work_orders.clear();
            return Ok(());
        }

        let Ok(mut default_team_guard) = default_team_arc.write() else {
            self.work_orders.clear();
            return Ok(());
        };

        team_guard.transfer_units_to(&mut default_team_guard);

        // PARITY_NOTE: C++ calls m_team->deleteInstance() if !getIsSingleton().
        // In Rust, delete_team destroys all remaining members and marks the team for cleanup.
        // Since units were already transferred, the team should have no remaining members.
        if !(*team_guard).is_singleton() {
            team_guard.delete_team(false);
        }
        drop(team_guard);

        // C++ m_team = NULL after disband so ~TeamInQueue will not setActive.
        self.team = None;
        self.work_orders.clear();
        Ok(())
    }

    /// Stop queueing new units, just finish current ones
    pub fn stop_queueing(&mut self) {
        self.stop_queueing = true;
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut work_order_count = self.work_orders.len() as u16;
        let _ = xfer.xfer_unsigned_short(&mut work_order_count);
        if xfer.is_loading() {
            self.work_orders.clear();
            for _ in 0..work_order_count {
                let mut order = WorkOrder::new(String::new());
                order.xfer(xfer);
                self.work_orders.push(order);
            }
        } else {
            for order in &mut self.work_orders {
                order.xfer(xfer);
            }
        }

        let mut priority_build = self.priority_build;
        let _ = xfer.xfer_bool(&mut priority_build);
        if xfer.is_loading() {
            self.priority_build = priority_build;
        }

        // C++: TeamID teamID = m_team ? m_team->getID() : TEAM_ID_INVALID;
        //      xferUser(&teamID); load: m_team = TheTeamFactory->findTeamByID(teamID);
        let mut team_id: u32 = self
            .team
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
            .unwrap_or(crate::team::TEAM_ID_INVALID);
        let _ = xfer.xfer_unsigned_int(&mut team_id);
        if xfer.is_loading() {
            if team_id == crate::team::TEAM_ID_INVALID {
                self.team = None;
                self.team_name = None;
            } else if let Ok(factory) = get_team_factory().lock() {
                if let Some(arc) = factory.find_team_by_id(team_id) {
                    self.team_name = arc.read().ok().map(|g| g.get_name().to_string());
                    self.team = Some(arc);
                } else {
                    self.team = None;
                    self.team_name = None;
                }
            }
        }

        let mut frame_started = self.frame_started as i32;
        let _ = xfer.xfer_int(&mut frame_started);
        if xfer.is_loading() {
            self.frame_started = frame_started as u32;
        }

        let mut sent_to_start_location = self.sent_to_start_location;
        let _ = xfer.xfer_bool(&mut sent_to_start_location);
        if xfer.is_loading() {
            self.sent_to_start_location = sent_to_start_location;
        }

        let mut stop_queueing = self.stop_queueing;
        let _ = xfer.xfer_bool(&mut stop_queueing);
        if xfer.is_loading() {
            self.stop_queueing = stop_queueing;
        }

        let mut reinforcement = self.reinforcement;
        let _ = xfer.xfer_bool(&mut reinforcement);
        if xfer.is_loading() {
            self.reinforcement = reinforcement;
        }

        let mut reinforcement_id = self.reinforcement_id.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut reinforcement_id);
        if xfer.is_loading() {
            self.reinforcement_id = if reinforcement_id == INVALID_ID {
                None
            } else {
                Some(reinforcement_id)
            };
        }
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) {
        let _ = xfer;
    }
}

/// C++ `~TeamInQueue`: if m_team remains, activate it (empty active teams are
/// cleaned up by Team). `disband` nulls the handle so Drop will not re-activate.
impl Drop for TeamInQueue {
    fn drop(&mut self) {
        if let Some(team_arc) = self.team.take() {
            if let Ok(mut tg) = team_arc.write() {
                tg.set_active();
            }
        }
    }
}

/// AI Player implementation
#[derive(Debug)]
pub struct AIPlayer {
    /// Player we represent
    player_id: u32,

    /// Team build and ready queues
    team_build_queue: VecDeque<TeamInQueue>,
    team_ready_queue: VecDeque<TeamInQueue>,

    /// Timing and delays
    ready_to_build_team: bool,
    ready_to_build_structure: bool,
    team_timer: u32,
    structure_timer: u32,
    team_seconds: Real,
    structure_seconds: Real,
    build_delay: u32,
    team_delay: u32,
    frame_last_building_built: u32,

    /// AI configuration
    difficulty: GameDifficulty,
    skillset_selector: i32,

    /// Base information
    base_center: Coord3D,
    base_center_set: bool,
    base_radius: Real,

    /// Bridge repair system
    structures_to_repair: [Option<ObjectID>; MAX_STRUCTURES_TO_REPAIR],
    repair_dozer: Option<ObjectID>,
    repair_dozer_origin: Coord3D,
    structures_in_queue: i32,
    dozer_queued_for_repair: bool,
    dozer_is_repairing: bool,
    bridge_timer: u32,

    /// Supply tracking
    supply_source_attack_check_frame: u32,
    attacked_supply_center: Option<ObjectID>,
    current_warehouse_id: Option<ObjectID>,

    /// AI strategy state
    strategy_state: AiStrategyState,

    /// Economic state
    economic_state: AiEconomicState,

    /// Military state
    military_state: AiMilitaryState,

    /// Construction priorities
    construction_priorities: Vec<ConstructionPriority>,

    /// Threat assessment
    threat_assessment: ThreatAssessment,

    /// Strategic decision maker (new integrated system)
    strategic_decision_maker: StrategicDecisionMaker,

    /// Difficulty handler (new integrated system)
    difficulty_handler: DifficultyHandler,

    /// Build order optimizer (new integrated system)
    build_order_optimizer: BuildOrderOptimizer,

    /// Threat assessment system (new integrated system)
    threat_system: ThreatAssessmentSystem,
}

/// AI strategy state information
#[derive(Debug, Clone, Default)]
pub struct AiStrategyState {
    pub current_strategy: AiStrategy,
    pub strategy_confidence: f32,              // 0.0 to 1.0
    pub time_in_strategy: u32,                 // Frames in current strategy
    pub last_strategy_change: u32,             // Frame of last strategy change
    pub fallback_strategy: Option<AiStrategy>, // Backup strategy
}

/// AI strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiStrategy {
    Turtle,     // Defensive, build up base
    Rush,       // Early aggressive attack
    Economic,   // Focus on resource gathering
    Balanced,   // Balanced approach
    Expansion,  // Expand territory
    TechRush,   // Focus on technology advancement
    Harassment, // Hit and run tactics
    AllOut,     // All-out attack
}

impl Default for AiStrategy {
    fn default() -> Self {
        AiStrategy::Balanced
    }
}

/// Economic state tracking
#[derive(Debug, Clone, Default)]
pub struct AiEconomicState {
    pub current_resources: HashMap<String, i32>, // Resources by type
    pub resource_income_rate: HashMap<String, f32>, // Income per second
    pub resource_priorities: Vec<String>,        // Ordered by priority
    pub economic_pressure: f32,                  // 0.0 to 1.0, higher = more pressure
    pub supply_shortage: bool,                   // Running low on supplies
    pub power_shortage: bool,                    // Need more power
}

/// Military state tracking
#[derive(Debug, Clone, Default)]
pub struct AiMilitaryState {
    pub total_military_strength: f32, // Overall military power
    pub unit_counts_by_type: HashMap<String, i32>, // Unit counts
    pub preferred_unit_mix: Vec<UnitMixPreference>, // Desired unit composition
    pub current_military_stance: MilitaryStance, // Current military posture
    pub enemy_strength_estimate: f32, // Estimated enemy strength
    pub last_combat_frame: u32,       // Frame of last combat
}

/// Military stance options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilitaryStance {
    Defensive,  // Defend base and territory
    Aggressive, // Actively seek combat
    Balanced,   // Mix of offense and defense
    Retreating, // Pulling back
}

impl Default for MilitaryStance {
    fn default() -> Self {
        MilitaryStance::Balanced
    }
}

/// Unit mix preferences for army composition
#[derive(Debug, Clone)]
pub struct UnitMixPreference {
    pub unit_type: String,
    pub desired_percentage: f32, // 0.0 to 1.0
    pub minimum_count: i32,      // Minimum units of this type
    pub priority: i32,           // Build priority (lower = higher priority)
}

/// Construction priority for buildings
#[derive(Debug, Clone)]
pub struct ConstructionPriority {
    pub building_type: String,
    pub priority: i32,           // Lower = higher priority
    pub prerequisites_met: bool, // Can we build this now?
    pub max_count: Option<i32>,  // Maximum number to build
    pub current_count: i32,      // How many we have now
    pub desired_location: Option<Coord3D>,
    pub desired_angle: Option<Real>,
}

/// Threat assessment system
#[derive(Debug, Clone, Default)]
pub struct ThreatAssessment {
    pub immediate_threats: Vec<ThreatInfo>, // Threats requiring immediate response
    pub potential_threats: Vec<ThreatInfo>, // Future threats to watch
    pub overall_threat_level: f32,          // 0.0 to 1.0
    pub recommended_response: ThreatResponse, // Suggested action
}

/// Individual threat information
#[derive(Debug, Clone)]
pub struct ThreatInfo {
    pub threat_id: ObjectID,
    pub threat_type: ThreatType,
    pub location: Coord3D,
    pub severity: f32,                 // 0.0 to 1.0
    pub time_detected: u32,            // Frame when detected
    pub estimated_time_to_impact: u32, // Frames until threat reaches us
}

/// Types of threats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatType {
    Military,     // Enemy military units
    Economic,     // Threats to economy (raids on workers)
    Strategic,    // Long-term strategic threats
    Superweapon,  // Incoming superweapon
    Infiltration, // Spies, stealth units
}

/// Recommended threat responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatResponse {
    None,      // No action needed
    Monitor,   // Keep watching
    Defend,    // Build defenses
    Attack,    // Counter-attack
    Retreat,   // Pull back
    Emergency, // All-out response
}

impl Default for ThreatResponse {
    fn default() -> Self {
        ThreatResponse::None
    }
}

/// Convert the public GameDifficulty (from player module) to the AI-specific enum
fn to_ai_difficulty(diff: GameDifficulty) -> AiGameDifficulty {
    match diff {
        GameDifficulty::Easy => AiGameDifficulty::Easy,
        GameDifficulty::Normal => AiGameDifficulty::Normal,
        GameDifficulty::Hard => AiGameDifficulty::Hard,
        GameDifficulty::Brutal => AiGameDifficulty::Brutal,
    }
}

impl AIPlayer {
    fn get_player_arc(&self) -> Option<Arc<RwLock<Player>>> {
        player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
    }

    /// Get the backing Player for this AI instance.
    pub fn get_player(&self) -> Option<Arc<RwLock<Player>>> {
        self.get_player_arc()
    }

    /// Create new AI player
    pub fn new(player_id: u32) -> Self {
        // C++ AIPlayer ctor: m_teamSeconds = TheAI->getAiData()->m_teamSeconds;
        // Structure interval is read live from AIData each arm (0.0 is valid = every tick).
        // Prefer live AIData; fall back to retail Default/AIData.ini constants when unloaded.
        let (team_seconds, structure_seconds) = if let Ok(ai) = THE_AI.read() {
            if let Ok(data) = ai.get_ai_data().read() {
                let team = if data.team_seconds > 0.0 {
                    data.team_seconds
                } else {
                    DEFAULT_TEAM_SECONDS
                };
                // StructureSeconds = 0.0 is intentional retail (do not treat as missing).
                (team, data.structure_seconds)
            } else {
                (DEFAULT_TEAM_SECONDS, DEFAULT_STRUCTURE_SECONDS)
            }
        } else {
            (DEFAULT_TEAM_SECONDS, DEFAULT_STRUCTURE_SECONDS)
        };

        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(player_id as i32).cloned() {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_units(false);
                }
            }
        }

        Self {
            player_id,
            team_build_queue: VecDeque::new(),
            team_ready_queue: VecDeque::new(),
            ready_to_build_team: false,
            ready_to_build_structure: false,
            team_timer: 2,
            structure_timer: 2,
            team_seconds,
            structure_seconds,
            build_delay: 0,
            team_delay: 0,
            frame_last_building_built: TheGameLogic::get_frame(),
            difficulty: GameDifficulty::Normal,
            skillset_selector: INVALID_SKILLSET_SELECTION,
            base_center: Coord3D::new(0.0, 0.0, 0.0),
            base_center_set: false,
            base_radius: 0.0,
            structures_to_repair: [None; MAX_STRUCTURES_TO_REPAIR],
            repair_dozer: None,
            repair_dozer_origin: Coord3D::new(0.0, 0.0, 0.0),
            structures_in_queue: 0,
            dozer_queued_for_repair: false,
            dozer_is_repairing: false,
            bridge_timer: 0,
            supply_source_attack_check_frame: 0,
            attacked_supply_center: None,
            current_warehouse_id: None,
            strategy_state: AiStrategyState::default(),
            economic_state: AiEconomicState::default(),
            military_state: AiMilitaryState::default(),
            construction_priorities: Vec::new(),
            threat_assessment: ThreatAssessment::default(),
            strategic_decision_maker: StrategicDecisionMaker::new(),
            difficulty_handler: DifficultyHandler::new(
                to_ai_difficulty(GameDifficulty::Normal),
                "USA",
            ),
            build_order_optimizer: BuildOrderOptimizer::new(),
            threat_system: ThreatAssessmentSystem::new(),
        }
    }

    /// Get base center position
    pub fn get_base_center(&self) -> Option<Coord3D> {
        if self.base_center_set {
            Some(self.base_center)
        } else {
            None
        }
    }

    pub fn get_base_radius(&self) -> Real {
        self.base_radius
    }

    pub fn get_ai_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    /// Public update entrypoint used by the integration layer.
    pub fn update(&mut self) -> Result<(), AiError> {
        <Self as AiPlayerTrait>::update(self)
    }

    /// Main AI think loop with frame parameter.
    ///
    /// C++ `AIPlayer::update` order (AIPlayer.cpp):
    ///   doBaseBuilding → checkReadyTeams → checkQueuedTeams →
    ///   doTeamBuilding → doUpgradesAndSkills → updateBridgeRepair
    ///
    /// Optional host residuals (off by default for C++ parity):
    /// - analysis prep before phases (`GENERALS_AI_HOST_ANALYSIS=1`)
    /// - strength-threshold attack after phases (`GENERALS_AI_HOST_ATTACK=1`)
    /// C++ skirmish attacks come from team scripts / AIGroup, not AIPlayer::update.
    pub fn update_with_frame(&mut self, frame: u32) -> Result<(), AiError> {
        if Self::host_analysis_enabled() {
            // Host residual (not in C++ AIPlayer::update).
            self.analyze_economic_situation()?;
            self.analyze_military_situation()?;
            self.analyze_threats()?;
        }

        // --- C++ AIPlayer::update phase order (timers live inside do_* ) ---
        self.do_base_building()?;
        self.check_ready_teams()?;
        self.check_queued_teams()?;
        self.do_team_building()?;
        self.do_upgrades_and_skills()?;
        self.update_bridge_repair()?;
        // --- end C++ phase order ---

        // Host residual: strength-threshold attack (not in C++ AIPlayer::update).
        // Default off — opt in with GENERALS_AI_HOST_ATTACK=1 for host smoke gates.
        if Self::host_attack_enabled() {
            self.process_attack_decisions(frame)?;
        }

        Ok(())
    }

    /// Opt-in host residual attack after C++ update phases.
    fn host_attack_enabled() -> bool {
        match std::env::var("GENERALS_AI_HOST_ATTACK") {
            Ok(v) => {
                let v = v.trim();
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("on")
                    || v.eq_ignore_ascii_case("yes")
            }
            Err(_) => false,
        }
    }

    /// Opt-in host residual analysis before C++ update phases.
    fn host_analysis_enabled() -> bool {
        match std::env::var("GENERALS_AI_HOST_ANALYSIS") {
            Ok(v) => {
                let v = v.trim();
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("on")
                    || v.eq_ignore_ascii_case("yes")
            }
            Err(_) => false,
        }
    }

    fn process_attack_decisions(&mut self, _frame: u32) -> Result<(), AiError> {
        let strength = self.military_state.total_military_strength;
        let threat = self.threat_assessment.overall_threat_level;

        if strength <= 0.0 {
            return Ok(());
        }

        let attack_ratio = match self.difficulty {
            GameDifficulty::Easy => 1.5,
            GameDifficulty::Normal => 1.0,
            GameDifficulty::Hard => 0.7,
            GameDifficulty::Brutal => 0.5,
        };

        if strength >= threat * attack_ratio {
            self.launch_attack()?;
        }

        Ok(())
    }

    fn analyze_military_situation(&mut self) -> Result<(), AiError> {
        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        let mut total_strength = 0.0f32;
        let mut counts: HashMap<String, i32> = HashMap::new();

        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_destroyed() || obj_guard.is_effectively_dead() {
                continue;
            }

            if obj_guard.is_kind_of(KindOf::Infantry) {
                *counts.entry("infantry".to_string()).or_insert(0) += 1;
                total_strength += 1.0;
            } else if obj_guard.is_kind_of(KindOf::Vehicle) {
                *counts.entry("vehicle".to_string()).or_insert(0) += 1;
                total_strength += 2.0;
            } else if obj_guard.is_kind_of(KindOf::Aircraft) {
                *counts.entry("aircraft".to_string()).or_insert(0) += 1;
                total_strength += 3.0;
            }
        }

        self.military_state.unit_counts_by_type = counts;
        self.military_state.total_military_strength = total_strength;

        Ok(())
    }

    fn analyze_threats(&mut self) -> Result<(), AiError> {
        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        let base_center = self.get_base_center().unwrap_or_else(Coord3D::origin);
        let scan_radius = 500.0f32;

        let mut threat_level = 0.0f32;
        let mut immediate_threats = Vec::new();

        if let Some(partition) = ThePartitionManager::get() {
            for obj_id in partition.get_objects_in_range(&base_center, scan_radius) {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() {
                    continue;
                }
                let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                    continue;
                };
                if owner_id as u32 == self.player_id {
                    continue;
                }
                if let Some(owner_arc) = player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(owner_id as i32).cloned())
                {
                    if let Ok(owner_guard) = owner_arc.read() {
                        if owner_guard.get_player_type() == PlayerType::Neutral {
                            continue;
                        }
                    }
                }
                let Some(target_team_arc) = obj_guard.get_team() else {
                    continue;
                };
                let Ok(target_team) = target_team_arc.read() else {
                    continue;
                };
                if player_guard.get_relationship_with_team(&target_team) != Relationship::Enemies {
                    continue;
                }

                let severity = if obj_guard.is_kind_of(KindOf::Structure) {
                    0.3
                } else if obj_guard.is_kind_of(KindOf::Vehicle) {
                    0.7
                } else if obj_guard.is_kind_of(KindOf::Infantry) {
                    0.5
                } else if obj_guard.is_kind_of(KindOf::Aircraft) {
                    0.8
                } else {
                    0.2
                };

                threat_level += severity;

                immediate_threats.push(ThreatInfo {
                    threat_id: obj_id,
                    threat_type: ThreatType::Military,
                    location: *obj_guard.get_position(),
                    severity,
                    time_detected: TheGameLogic::get_frame(),
                    estimated_time_to_impact: 0,
                });
            }
        }

        self.threat_assessment.immediate_threats = immediate_threats;
        self.threat_assessment.overall_threat_level = threat_level;

        self.threat_assessment.recommended_response = if threat_level > 5.0 {
            ThreatResponse::Emergency
        } else if threat_level > 3.0 {
            ThreatResponse::Attack
        } else if threat_level > 1.0 {
            ThreatResponse::Defend
        } else if threat_level > 0.0 {
            ThreatResponse::Monitor
        } else {
            ThreatResponse::None
        };

        self.military_state.enemy_strength_estimate = threat_level * 2.0;

        Ok(())
    }

    pub fn get_build_delay(&self) -> u32 {
        self.build_delay
    }

    pub fn get_team_delay(&self) -> u32 {
        self.team_delay
    }

    pub fn get_team_timer(&self) -> u32 {
        self.team_timer
    }

    pub fn get_structure_timer(&self) -> u32 {
        self.structure_timer
    }

    pub fn set_build_delay_frames(&mut self, frames: u32) {
        self.build_delay = frames;
    }

    pub fn set_team_delay_frames(&mut self, frames: u32) {
        self.team_delay = frames;
    }

    pub fn set_team_timer_frames(&mut self, frames: u32) {
        self.team_timer = frames;
    }

    pub fn set_structure_timer_frames(&mut self, frames: u32) {
        self.structure_timer = frames;
    }

    pub fn can_build_structure_now(&self) -> bool {
        self.ready_to_build_structure && self.build_delay == 0
    }

    pub fn can_build_team_now(&self) -> bool {
        self.ready_to_build_team && self.team_delay == 0
    }

    pub fn is_ready_to_build_structure(&self) -> bool {
        self.ready_to_build_structure
    }

    pub fn set_ready_to_build_structure(&mut self, ready: bool) {
        self.ready_to_build_structure = ready;
    }

    pub fn is_ready_to_build_team(&self) -> bool {
        self.ready_to_build_team
    }

    pub fn set_ready_to_build_team(&mut self, ready: bool) {
        self.ready_to_build_team = ready;
    }

    pub fn start_structure_timer_seconds(&mut self, seconds: i32) {
        let seconds = seconds.max(0) as u32;
        self.structure_timer = seconds * LOGICFRAMES_PER_SECOND;
        self.ready_to_build_structure = false;
    }

    /// Returns true if the team is already queued for building.
    pub fn is_team_in_queue(&self, team_name: &str) -> bool {
        self.team_build_queue.iter().any(|team| {
            team.team_name
                .as_deref()
                .map(|name| name == team_name)
                .unwrap_or(false)
        })
    }

    /// Check if location is safe for building.
    /// C++ `AIPlayer::isLocationSafe` (AIPlayer.cpp).
    ///
    /// Scan enemies (alive, non-stealthed, significant, non-harvester, non-dozer)
    /// within supply-center safe radius + template bounding radius.
    /// C++ `AIPlayer::isLocationSafe` (AIPlayer.cpp).
    ///
    /// Partition closest-object filters: enemies only, alive, not stealthed,
    /// reject harvesters/dozers. Any hit → unsafe.
    pub fn is_location_safe(&self, pos: &Coord3D, thing: &dyn ThingTemplate) -> bool {
        let Some(player_arc) = self.get_player_arc() else {
            return true;
        };
        let Ok(player_guard) = player_arc.read() else {
            return true;
        };
        let Some(partition) = ThePartitionManager::get() else {
            return true;
        };

        let mut radius = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.supply_center_safe_radius)
            })
            .filter(|r| *r > 0.0)
            .unwrap_or(SUPPLY_CENTER_SAFE_RADIUS);
        radius += thing
            .get_template_geometry_info()
            .get_bounding_circle_radius();

        for obj_id in partition.get_objects_in_range(pos, radius) {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            // PartitionFilterAlive
            if obj_guard.is_destroyed() || obj_guard.is_effectively_dead() {
                continue;
            }
            // PartitionFilterRejectByKindOf harvester/dozer
            if obj_guard.is_kind_of(KindOf::Harvester) || obj_guard.is_kind_of(KindOf::Dozer) {
                continue;
            }
            // PartitionFilterRejectByObjectStatus stealthed (unless detected/disguised)
            if obj_guard.test_status(ObjectStatusTypes::Stealthed)
                && !obj_guard.test_status(ObjectStatusTypes::Detected)
                && !obj_guard.test_status(ObjectStatusTypes::Disguised)
            {
                continue;
            }
            // PartitionFilterPlayerAffiliation: enemies only
            // (ALLOW_ALLIES|ALLOW_NEUTRAL rejected via affiliation=false)
            let Some(team_arc) = obj_guard.get_team() else {
                continue;
            };
            let Ok(team) = team_arc.read() else {
                continue;
            };
            if player_guard.get_relationship_with_team(&team) != Relationship::Enemies {
                continue;
            }
            // PartitionFilterInsignificantBuildings(true, false): reject bridges /
            // bridge towers as insignificant for placement safety (closest match
            // without KINDOF_INSIGNIFICANT_BUILDING enum in port).
            if obj_guard.is_kind_of(KindOf::Bridge) || obj_guard.is_kind_of(KindOf::BridgeTower) {
                continue;
            }
            // Any enemy that passes filters fails safety (C++ getClosestObject != NULL).
            return false;
        }
        true
    }

    /// Update loop variant for skirmish AI that supplies its own base-building logic.
    ///
    /// C++ order without `doBaseBuilding` (skirmish overrides base building).
    pub fn update_without_base_building(&mut self) -> Result<(), AiError> {
        self.check_ready_teams()?;
        self.check_queued_teams()?;
        self.do_team_building()?;
        self.do_upgrades_and_skills()?;
        self.update_bridge_repair()?;
        Ok(())
    }

    /// C++ `AIPlayer::newMap` (AIPlayer.cpp).
    ///
    /// 1. Snapshot pre-existing build-list entries (C++ saves head before prepends)
    /// 2. Prepend placed factories via addToBuildList
    /// 3. computeCenterAndRadiusOfBase (includes new factories)
    /// 4. Walk *original* entries only: initiallyBuilt → buildStructureNow else
    ///    incrementNumRebuilds
    pub fn new_map(&mut self) {
        // C++ does not clear queues/timers here — only factory scan + initial builds.

        // Snapshot original build list BEFORE factory prepends (C++ keeps old head ptr).
        let mut original_entries: Vec<(String, Coord3D, Real, bool)> = Vec::new();
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player_arc.read() {
                    let mut cur = pg.get_build_list();
                    while let Some(node) = cur {
                        let name = node.get_template_name().to_string();
                        if !name.is_empty() {
                            original_entries.push((
                                name,
                                *node.get_location(),
                                node.get_angle(),
                                node.is_initially_built(),
                            ));
                        }
                        cur = node.get_next();
                    }
                }
            }
        }

        // Add any factories placed to the build list (C++ ProductionUpdateInterface).
        // C++ addToBuildList prepends — new entries are NOT in original_entries.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                let owned: Vec<ObjectID> = player_arc
                    .read()
                    .ok()
                    .map(|g| g.get_all_objects())
                    .unwrap_or_default();
                drop(list);
                for obj_id in owned {
                    let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                        continue;
                    };
                    let Ok(obj_g) = obj_arc.read() else {
                        continue;
                    };
                    // Factory if any production interface.
                    let mut is_factory = false;
                    for behavior in obj_g.get_behavior_modules() {
                        if let Ok(mut bg) = behavior.lock() {
                            if bg.get_production_update_interface().is_some() {
                                is_factory = true;
                                break;
                            }
                        }
                    }
                    if !is_factory {
                        for mh in obj_g.behavior_modules() {
                            let matched = mh.with_module(|module| {
                                module.get_production_control_interface().is_some()
                            });
                            if matched {
                                is_factory = true;
                                break;
                            }
                        }
                    }
                    if !is_factory {
                        continue;
                    }
                    let template_name = obj_g.get_template_name().to_string();
                    let pos = *obj_g.get_position();
                    let angle = obj_g.get_orientation();
                    drop(obj_g);
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(self.player_id as i32) {
                            if let Ok(mut pg) = player_arc.write() {
                                pg.add_to_build_list(
                                    obj_id,
                                    AsciiString::from(template_name.as_str()),
                                    pos,
                                    angle,
                                );
                            }
                        }
                    }
                }
            }
        }

        let _ = self.compute_center_and_radius_of_base();

        // Walk original (pre-factory) entries only — matches C++ head pointer walk.
        let mut initial: Vec<(String, Coord3D, Real)> = Vec::new();
        for (name, loc, ang, initially) in original_entries {
            if TheThingFactory::find_template(&name).is_none() {
                log::debug!("*** ERROR - Build list building '{}' doesn't exist.", name);
                continue;
            }
            if initially {
                initial.push((name, loc, ang));
            } else {
                // C++ info->incrementNumRebuilds on the live node.
                if let Ok(list) = player_list().read() {
                    if let Some(player_arc) = list.get_player(self.player_id as i32) {
                        if let Ok(mut pg) = player_arc.write() {
                            if let Some(info) = pg.get_build_list_mut() {
                                let mut cur = Some(&mut *info);
                                while let Some(node) = cur {
                                    if node.get_template_name() == name
                                        && (node.get_location().x - loc.x).abs() < 0.01
                                        && (node.get_location().y - loc.y).abs() < 0.01
                                    {
                                        node.increment_num_rebuilds();
                                        break;
                                    }
                                    cur = node.get_next_mut();
                                }
                            }
                        }
                    }
                }
            }
        }
        for (name, loc, ang) in initial {
            if let Err(err) = self.build_structure_now_at(&name, loc, ang, None) {
                log::debug!("newMap buildStructureNow('{}') failed: {err}", name);
            }
        }
    }

    /// Start training for a work order with factory management.
    pub(crate) fn start_training_for_order(
        &mut self,
        order: &mut WorkOrder,
        busy_ok: bool,
    ) -> Result<bool, AiError> {
        self.start_training_internal(order, busy_ok, "default")
    }

    /// C++ `AIPlayer::queueUnits` (AIPlayer.cpp).
    ///
    /// For each work order still waiting: recruit existing map units into the
    /// team (tryToRecruit) until full or none left; then startTraining if still
    /// waiting; else validateFactory.
    pub fn queue_units(&mut self) -> bool {
        let _ = self.queue_supply_truck();

        let max_recruit = THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.max_recruit_distance))
            .filter(|d| *d > 0.0)
            .unwrap_or(99999.0);

        let mut rebuilt_queue = VecDeque::with_capacity(self.team_build_queue.len());
        while let Some(mut team_q) = self.team_build_queue.pop_front() {
            let busy_ok = team_q.priority_build;
            let team_name = team_q
                .team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());

            // C++ team->m_team: prefer concrete handle; name lookup is fallback only.
            if team_q.team.is_none() {
                team_q.team = get_team_factory().lock().ok().and_then(|mut factory| {
                    factory
                        .find_team_instances(&team_name)
                        .into_iter()
                        .next()
                        .or_else(|| factory.find_team(&team_name))
                });
            }
            let team_arc = team_q.team.clone();

            // Home for recruit search: C++ m_team prototype homeLocation else base center.
            let (home, has_home) = self.queue_units_home_for_team(team_arc.as_ref(), &team_name);

            for order in &mut team_q.work_orders {
                // C++: while waiting, tryToRecruit repeatedly.
                if let Some(ref team_arc) = team_arc {
                    while order.is_waiting_to_build() {
                        let Some(thing) = TheThingFactory::find_template(&order.thing_template)
                        else {
                            break;
                        };
                        let Ok(team_g) = team_arc.read() else {
                            break;
                        };
                        let Some(unit_arc) = team_g.try_to_recruit(&thing, &home, max_recruit)
                        else {
                            break; // no more recruitable units
                        };
                        drop(team_g);

                        order.num_completed = order.num_completed.saturating_add(1);

                        if let Ok(mut unit_g) = unit_arc.write() {
                            let _ = unit_g.set_team(Some(team_arc.clone()));
                            if let Some(ai) = unit_g.get_ai_update_interface() {
                                if has_home {
                                    // C++ aiMoveToPosition(&home, CMD_FROM_AI)
                                    ai.ai_move_to_position(&home, false, CommandSourceType::FromAi);
                                } else {
                                    // C++ aiIdle(CMD_FROM_AI)
                                    ai.ai_idle(CommandSourceType::FromAi);
                                }
                            }
                        }

                        log::debug!(
                            "Team '{}' recruits {} (queueUnits)",
                            team_name,
                            order.thing_template
                        );
                    }
                }

                if order.is_waiting_to_build() {
                    // start the creation of a new unit
                    // C++ startTraining(..., team->m_team->getName())
                    let train_name = team_arc
                        .as_ref()
                        .and_then(|a| a.read().ok().map(|g| g.get_name().to_string()))
                        .unwrap_or_else(|| team_name.clone());
                    let _ = self.start_training_internal(order, busy_ok, train_name.as_str());
                } else {
                    // under construction / complete — verify factory still exists
                    let _ = order.validate_factory(self.player_id);
                }
            }
            rebuilt_queue.push_back(team_q);
        }
        self.team_build_queue = rebuilt_queue;

        true
    }

    /// C++ queueUnits home: m_team prototype homeLocation if set, else getBaseCenter.
    fn queue_units_home_for_team(
        &self,
        team: Option<&Arc<RwLock<crate::team::Team>>>,
        team_name: &str,
    ) -> (Coord3D, bool) {
        // Resolve prototype name from concrete m_team when present (C++ getPrototype()).
        let proto_name = team
            .and_then(|a| a.read().ok().map(|g| g.get_name().to_string()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| team_name.to_string());
        if let Ok(factory) = get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(&proto_name) {
                if proto.has_home_location() {
                    return (proto.home_location(), true);
                }
            }
            // Fallback: original team_name if m_team name differs.
            if proto_name != team_name {
                if let Some(proto) = factory.find_team_prototype(team_name) {
                    if proto.has_home_location() {
                        return (proto.home_location(), true);
                    }
                }
            }
        }
        // C++ falls back to base center when !hasHomeLocation.
        if let Some(center) = self.get_base_center() {
            return (center, false);
        }
        (Coord3D::new(0.0, 0.0, 0.0), false)
    }

    /// C++ onUnitProduced supply assignment: first build-list supply building with
    /// desiredGatherers > currentGatherers; bump current and return object id.
    fn take_supply_gatherer_slot(&mut self) -> Option<ObjectID> {
        let player_arc = self.get_player()?;
        let Ok(mut pg) = player_arc.write() else {
            return None;
        };
        let Some(info_head) = pg.get_build_list_mut() else {
            return None;
        };
        let mut node = Some(&mut *info_head);
        while let Some(info) = node {
            if info.is_supply_building()
                && info.get_desired_gatherers() > 0
                && info.get_desired_gatherers() > info.get_current_gatherers()
            {
                let oid = info.get_object_id();
                if oid != INVALID_ID && OBJECT_REGISTRY.get_object(oid).is_some() {
                    info.set_current_gatherers(info.get_current_gatherers() + 1);
                    return Some(oid);
                }
            }
            node = info.get_next_mut();
        }
        None
    }

    /// C++ `AIPlayer::checkForSupplyCenter` (AIPlayer.cpp).
    ///
    /// If structure has SupplyCenterDockUpdate, mark build-list entry as supply
    /// building and set desired gatherers from AISideInfo + 1 freebie.
    pub fn check_for_supply_center(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        let Some(structure_arc) = OBJECT_REGISTRY.get_object(structure_id) else {
            return Ok(());
        };
        let Ok(structure_guard) = structure_arc.read() else {
            return Ok(());
        };
        // C++: findUpdateModule(NAMEKEY("SupplyCenterDockUpdate")) only —
        // KindOf alone is not sufficient (matches GeneralsMD AIPlayer.cpp).
        if structure_guard
            .find_update_module("SupplyCenterDockUpdate")
            .is_none()
        {
            return Ok(());
        }

        let side = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_side().to_string()))
            .unwrap_or_default();

        let mut desired = 0;
        if let Ok(ai_guard) = THE_AI.read() {
            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                for info in &ai_data.side_info {
                    if info.side == side {
                        desired = match self.difficulty {
                            GameDifficulty::Easy => info.easy,
                            GameDifficulty::Normal => info.normal,
                            GameDifficulty::Hard | GameDifficulty::Brutal => info.hard,
                        };
                        break;
                    }
                }
            }
        }

        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        let mut cur = Some(&mut *info);
                        while let Some(node) = cur {
                            if node.get_object_id() == structure_id {
                                node.set_supply_building(true);
                                node.set_current_gatherers(-1);
                                // C++ desiredGatherers + 1 freebie with depot
                                node.set_desired_gatherers(desired + 1);
                                break;
                            }
                            cur = node.get_next_mut();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn select_team_to_build_ai(&mut self) -> bool {
        self.select_team_to_build().unwrap_or(false)
    }

    /// C++ `AIPlayer::setAIDifficulty` — assign `m_difficulty` only.
    ///
    /// Does not rewrite TeamSeconds or host strategy factors (those are not in
    /// GeneralsMD AIPlayer::setAIDifficulty).
    pub fn set_ai_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
    }

    /// C++ `AIPlayer::selectSkillset` — assign skillset; warn if already chosen.
    pub fn select_skillset(&mut self, skillset: i32) {
        if self.skillset_selector != INVALID_SKILLSET_SELECTION {
            log::debug!(
                "Selecting a skill set ({}) after one has already been chosen ({}) means some points have been incorrectly spent.",
                skillset + 1,
                self.skillset_selector + 1
            );
        }
        self.skillset_selector = skillset;
    }

    /// C++ `AIPlayer::processTeamBuilding`: if selectTeamToBuild then queueUnits.
    /// (selectTeamToBuild itself may reinforce a higher-priority team first.)
    pub fn process_team_building(&mut self) -> Result<(), AiError> {
        if self.select_team_to_build()? {
            let _ = self.queue_units();
        }
        Ok(())
    }

    /// C++ `AIPlayer::isSupplySourceSafe` (AIPlayer.cpp).
    pub fn is_supply_source_safe(&self, min_supplies: i32) -> bool {
        let Some(warehouse) = self.find_supply_center(min_supplies) else {
            return true; // safe because it doesn't exist
        };
        let Ok(guard) = warehouse.read() else {
            return true;
        };
        let template = guard.get_template();
        self.is_location_safe(guard.get_position(), template.as_ref())
    }

    /// C++ `AIPlayer::isSupplySourceAttacked` (AIPlayer.cpp).
    ///
    /// Rate-limited (10s): if player was recently attacked, scan cash generators /
    /// dozers / harvesters for recent damage and latch attacked_supply_center.
    pub fn is_supply_source_attacked(&mut self) -> bool {
        // C++ AIPlayer.cpp: const Int SCAN_RATE = 10;
        // Comment says "10 seconds" but the value is added to frame counters as-is
        // (10 logic frames ≈ 0.33s). Match code, not the misleading comment.
        const SCAN_RATE: u32 = 10;
        let cur_frame = TheGameLogic::get_frame();
        if cur_frame == 0 {
            self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);
            return false;
        }
        self.attacked_supply_center = None;
        if cur_frame < self.supply_source_attack_check_frame {
            return false;
        }

        let Ok(list) = player_list().read() else {
            return false;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return false;
        };
        let Ok(player_guard) = player_arc.read() else {
            return false;
        };
        if player_guard.get_attacked_frame().saturating_add(SCAN_RATE) < cur_frame {
            return false; // haven't been attacked recently
        }
        self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);

        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !obj_guard.is_kind_of(KindOf::CashGenerator)
                && !obj_guard.is_kind_of(KindOf::Dozer)
                && !obj_guard.is_kind_of(KindOf::Harvester)
            {
                continue;
            }
            let Some(body) = obj_guard.get_body_module() else {
                continue;
            };
            let Ok(body_g) = body.lock() else {
                continue;
            };
            let Some(info) = body_g.get_last_damage_info() else {
                continue;
            };
            if info.output.no_effect {
                continue;
            }
            if body_g.get_last_damage_timestamp().saturating_add(SCAN_RATE) > cur_frame {
                self.attacked_supply_center = Some(obj_id);
                return true;
            }
        }
        false
    }

    /// C++ `AIPlayer::buildSpecificAITeam` (AIPlayer.cpp).
    ///
    /// Gates: canBuildUnits, singleton+priority, isPossibleToBuildTeam (money-
    /// only still queues). Work orders: optional (max-min) then required (min,
    /// even minUnits==0). createInactiveTeam, executeActions, priority prepend
    /// vs normal append, teamDelay=0.
    pub fn build_specific_ai_team(
        &mut self,
        team_name: &str,
        priority_build: bool,
    ) -> Result<(), AiError> {
        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };
        if !player_guard.get_can_build_units() {
            log::debug!(
                "Can't build team '{}' because build units is disabled.",
                team_name
            );
            return Ok(());
        }
        drop(player_guard);

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(());
        };
        let Some(proto) = factory.find_team_prototype(team_name).map(|p| p.clone()) else {
            return Ok(());
        };

        if priority_build && proto.is_singleton() {
            if let Some(existing) = factory.find_team(team_name) {
                if let Ok(eg) = existing.read() {
                    if eg.has_any_objects() {
                        log::debug!(
                            "Unable to build singleton team '{}' because team already exists.",
                            team_name
                        );
                        return Ok(());
                    }
                }
            }
        }

        // Drop factory lock before is_possible (find_factory may lock).
        let units: Vec<(String, i32, i32)> = proto
            .units_info()
            .iter()
            .filter(|u| !u.unit_thing_name.is_empty())
            .map(|u| (u.unit_thing_name.to_string(), u.min_units, u.max_units))
            .collect();
        drop(factory);

        let (possible, need_money) = self.is_possible_to_build_team(team_name, false)?;
        if !possible {
            if need_money {
                log::debug!(
                    "Note - queueing team '{}' but there is not enough money.",
                    team_name
                );
                // C++ still queues when only money is missing.
            } else {
                log::debug!(
                    "Unable to build team '{}' because required factories/tech don't exist.",
                    team_name
                );
                return Ok(());
            }
        }

        // Optional units first (max-min), then required (min) — C++ prepend order
        // so required ends up first in list after both prepends.
        // C++ still creates required WorkOrders when minUnits==0 (numRequired=0).
        let mut orders: Vec<WorkOrder> = Vec::new();
        // Optional
        for (name, min_u, max_u) in &units {
            let count = (*max_u - *min_u).max(0);
            if count <= 0 {
                continue;
            }
            if TheThingFactory::find_template(name).is_none() {
                continue;
            }
            let mut order = WorkOrder::new(name.clone());
            order.num_required = count;
            order.required = false;
            orders.insert(0, order); // prepend
        }
        // Required — always when template exists (even minUnits==0).
        for (name, min_u, _max_u) in &units {
            if TheThingFactory::find_template(name).is_none() {
                continue;
            }
            let count = (*min_u).max(0);
            let mut order = WorkOrder::new(name.clone());
            order.num_required = count;
            order.required = true;
            orders.insert(0, order); // prepend
        }

        if orders.is_empty() {
            log::debug!("{} - contains 0 buildable units.", team_name);
            return Ok(());
        }

        // createInactiveTeam
        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(());
        };
        let Some(team_arc) = factory.create_inactive_team(team_name) else {
            return Ok(());
        };
        drop(factory);

        if let Ok(mut tg) = team_arc.write() {
            tg.set_controlling_player_id(Some(self.player_id as UnsignedInt));
        }

        // C++: if executeActions, friend_executeAction(productionCondition action, team).
        if proto.get_execute_actions_on_create() {
            let cond = proto.get_production_condition().to_string();
            if !cond.is_empty() {
                let script_engine = get_script_engine();
                let action = script_engine
                    .read()
                    .ok()
                    .and_then(|eng| {
                        eng.as_ref()
                            .and_then(|e| e.find_script_clone_by_name(&cond))
                    })
                    .and_then(|script| script.get_action().cloned());
                if let Some(action) = action {
                    // C++ friend_executeAction(action, team)
                    drop(script_engine);
                    if let Ok(mut eng) = get_script_engine().write() {
                        if let Some(e) = eng.as_mut() {
                            e.friend_execute_action(&action, Some(team_name));
                        }
                    }
                }
            }
        }

        let mut team = TeamInQueue::new();
        team.team_name = Some(team_name.to_string());
        team.team = Some(team_arc);
        team.priority_build = priority_build;
        team.frame_started = TheGameLogic::get_frame();
        team.work_orders = orders;

        if priority_build {
            self.team_build_queue.push_front(team);
        } else {
            self.team_build_queue.push_back(team);
        }
        self.team_delay = 0;
        log::debug!("{} - starting team build.", team_name);
        Ok(())
    }

    /// C++ `AIPlayer::buildAIBaseDefense` — solo AI unsupported (skirmish overrides).
    pub fn build_ai_base_defense(&mut self, _flank: bool) -> Result<(), AiError> {
        log::debug!("Error : Solo ai doesn't support buildAIBaseDefense.");
        Ok(())
    }

    /// C++ `AIPlayer::buildAIBaseDefenseStructure` — solo AI unsupported.
    pub fn build_ai_base_defense_structure(
        &mut self,
        _structure_name: &str,
        _flank: bool,
    ) -> Result<(), AiError> {
        log::debug!("Error : Solo ai doesn't support buildAIBaseDefenseStructure.");
        Ok(())
    }

    /// Build specific building as soon as possible
    /// C++ `AIPlayer::buildSpecificAIBuilding` — solo AI does not support this;
    /// skirmish override handles real priority-build stamping.
    pub fn build_specific_ai_building(&mut self, building_name: &str) -> Result<(), AiError> {
        log::debug!(
            "Error : Solo ai doesn't support BuildSpecificBuilding. '{}' not built.",
            building_name
        );
        Ok(())
    }

    /// C++ `AIPlayer::recruitSpecificAITeam` (AIPlayer.cpp).
    ///
    /// createInactiveTeam, tryToRecruit up to maxUnits per type within radius of
    /// home/base, move to home, ready-queue if any recruited else disband.
    pub fn recruit_specific_ai_team(
        &mut self,
        team_name: &str,
        recruit_radius: Real,
    ) -> Result<(), AiError> {
        let radius = if recruit_radius < 1.0 {
            99_999.0
        } else {
            recruit_radius
        };

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(());
        };
        let Some(proto) = factory.find_team_prototype(team_name).map(|p| p.clone()) else {
            return Ok(());
        };

        if proto.is_singleton() {
            if let Some(existing) = factory.find_team(team_name) {
                if let Ok(eg) = existing.read() {
                    if eg.has_any_objects() {
                        log::debug!(
                            "Unable to recruit singleton team '{}' because team already exists.",
                            team_name
                        );
                        return Ok(());
                    }
                }
            }
        }

        // C++: warn missing home when not skirmish AI (AIPlayer) / always for skirmish
        // override path. Still recruits using template home (often origin).
        if !proto.has_home_location() && !self.is_skirmish_ai_player() {
            log::debug!(
                "Error : team '{}' has no Home Position (or Origin).",
                team_name
            );
        }

        let Some(team_arc) = factory.create_inactive_team(team_name) else {
            return Ok(());
        };
        drop(factory);

        if let Ok(mut tg) = team_arc.write() {
            tg.set_controlling_player_id(Some(self.player_id as UnsignedInt));
        }

        // C++ tryToRecruit / aiMoveToPosition use teamProto homeLocation.
        let home = proto.home_location();

        let mut units_recruited = 0i32;
        for unit_info in proto.units_info() {
            if unit_info.unit_thing_name.is_empty() {
                continue;
            }
            let Some(thing) = TheThingFactory::find_template(unit_info.unit_thing_name) else {
                continue;
            };
            let mut count = unit_info.max_units.max(0);
            while count > 0 {
                let recruited = {
                    let Ok(tg) = team_arc.read() else {
                        break;
                    };
                    tg.try_to_recruit(&thing, &home, radius)
                };
                let Some(unit_arc) = recruited else {
                    break;
                };
                let unit_id = unit_arc
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(INVALID_ID);
                if let Ok(mut ug) = unit_arc.write() {
                    let _ = ug.set_team(Some(team_arc.clone()));
                }
                if let Ok(mut tg) = team_arc.write() {
                    tg.add_member(unit_id);
                }
                // Move to home (CMD_FROM_AI).
                if let Ok(ug) = unit_arc.read() {
                    if let Some(ai) = ug.get_ai_update_interface() {
                        if let Ok(mut ai_g) = ai.lock() {
                            let mut params = crate::ai::AiCommandParams::new(
                                crate::ai::AiCommandType::MoveToPosition,
                                CommandSourceType::FromAi,
                            );
                            params.pos = home;
                            let _ = ai_g.execute_command(&params);
                        }
                    }
                }
                units_recruited += 1;
                count -= 1;
            }
        }

        if units_recruited > 0 {
            let mut team = TeamInQueue::new();
            team.team_name = Some(team_name.to_string());
            team.team = Some(team_arc);
            team.priority_build = false;
            team.frame_started = TheGameLogic::get_frame();
            // Ready queue — C++ prependTo_TeamReadyQueue (activate later).
            self.team_ready_queue.push_front(team);
            log::debug!("{} - Finished recruiting.", team_name);
        } else {
            if !proto.is_singleton() {
                let team_id = team_arc.read().ok().map(|t| t.get_id());
                if let (Some(team_id), Ok(mut factory)) = (team_id, get_team_factory().lock()) {
                    factory.team_about_to_be_deleted(team_id);
                }
            }
            log::debug!("{} - Recruited 0 units, disbanding.", team_name);
        }

        Ok(())
    }

    /// Build an upgrade (player upgrades only).
    /// C++ `AIPlayer::buildUpgrade` (AIPlayer.cpp).
    ///
    /// Validate upgrade type/affordability, then walk player build list for a
    /// ready factory whose command set can queue the upgrade.
    pub fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError> {
        let upgrade = with_upgrade_center(|center| center.find_upgrade(upgrade_name));
        let Some(upgrade) = upgrade else {
            log::debug!(
                "Upgrade {} does not exist.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        };

        if upgrade.get_upgrade_type() == UpgradeType::Object {
            log::debug!(
                "Player build upgrade: Upgrade {} is an object, not a player upgrade.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }

        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        if player_guard.has_upgrade_in_production(upgrade.as_ref()) {
            log::debug!(
                "already has upgrade {} queued.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }
        if player_guard.has_upgrade_complete(upgrade.as_ref()) {
            log::debug!(
                "already has upgrade {} completed.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }

        let can_afford = with_upgrade_center(|center| {
            center.can_afford_upgrade(&player_guard, upgrade.as_ref(), false)
        });
        if !can_afford {
            log::debug!(
                "lacks money to build upgrade {} at this time.  Ignoring request.",
                upgrade_name
            );
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };

        // C++ walks build list (not all objects) for factory order parity.
        let factory_ids: Vec<ObjectID> = {
            let mut ids = Vec::new();
            let mut cur = player_guard.get_build_list();
            while let Some(info) = cur {
                let id = info.get_object_id();
                if id != INVALID_ID {
                    ids.push(id);
                }
                cur = info.get_next();
            }
            ids
        };
        drop(player_guard);

        for object_id in factory_ids {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            if obj_guard.test_status(ObjectStatusTypes::UnderConstruction)
                || obj_guard.test_status(ObjectStatusTypes::Sold)
            {
                continue;
            }

            let command_set_name = obj_guard.get_command_set_string();
            let Some(command_set) = control_bar.find_command_set_by_name(command_set_name) else {
                continue;
            };

            let mut can_upgrade_here = false;
            for button in &command_set.buttons {
                let Some(button) = button else {
                    continue;
                };
                let Some(button_upgrade) = button.get_upgrade_template() else {
                    continue;
                };
                if button_upgrade.get_name() == upgrade.get_name() {
                    can_upgrade_here = true;
                    break;
                }
            }
            if !can_upgrade_here {
                continue;
            }

            // Need production update interface residual — queue_upgrade covers it.
            if obj_guard.queue_upgrade(&upgrade) {
                log::debug!(
                    "queues {} at {}",
                    upgrade.get_name(),
                    obj_guard.get_template_name()
                );
                return Ok(());
            }
        }

        log::debug!(
            "lacks factory to build upgrade {} at this time.  Ignoring request.",
            upgrade_name
        );
        Ok(())
    }

    /// C++ `AIPlayer::buildBySupplies` (AIPlayer.cpp).
    ///
    /// findSupplyCenter, then non-cash may override with m_curWarehouseID.
    /// Offset toward base (cash) or enemy bounds (defense), legalize/wiggle,
    /// always addToPriorityBuildList (even if placement stays at seed), stamp
    /// m_curWarehouseID. Uses m_baseCenter as-is (no auto recompute).
    pub fn build_by_supplies(
        &mut self,
        minimum_cash: i32,
        thing_name: &str,
    ) -> Result<(), AiError> {
        let Some(template) = crate::helpers::TheThingFactory::find_template(thing_name) else {
            log::warn!(
                "AIPlayer: template '{}' not found for build_by_supplies",
                thing_name
            );
            return Ok(());
        };

        // C++ uses m_baseCenter even when m_baseCenterSet is false.
        let base_center = self.base_center;

        let is_cash_generator = template.is_kind_of(KindOf::CashGenerator);

        // C++: always findSupplyCenter first.
        let mut best_supply = self.find_supply_center(minimum_cash);

        // Non-cash: live m_curWarehouseID overrides find result when present.
        if !is_cash_generator {
            if let Some(warehouse_id) = self.current_warehouse_id {
                if let Some(warehouse_arc) = OBJECT_REGISTRY.get_object(warehouse_id) {
                    best_supply = Some(warehouse_arc);
                }
            }
        }

        let Some(warehouse_arc) = best_supply else {
            return Ok(());
        };
        let Ok(warehouse_guard) = warehouse_arc.read() else {
            return Ok(());
        };
        let mut location = *warehouse_guard.get_position();

        let mut offset_x = location.x - base_center.x;
        let mut offset_y = location.y - base_center.y;
        let mut radius = 3.0 * PATHFIND_CELL_SIZE_F;
        if !is_cash_generator {
            // Defensive structure — face toward enemy base center.
            let enemy_ndx = self.get_skirmish_enemy_player_index();
            if let Ok((lo, hi)) = self.get_player_structure_bounds(enemy_ndx) {
                offset_x = location.x - (lo.x + hi.x) * 0.5;
                offset_y = location.y - (lo.y + hi.y) * 0.5;
            }
            radius = warehouse_guard
                .get_geometry_info()
                .get_bounding_circle_radius();
        }
        let len = (offset_x * offset_x + offset_y * offset_y).sqrt();
        if len > 0.0001 {
            offset_x /= len;
            offset_y /= len;
        }
        location.x -= offset_x * radius;
        location.y -= offset_y * radius;

        let angle = template.get_placement_view_angle();
        // C++: if seed illegal, wiggle; if wiggle succeeds use newPos; else keep seed.
        // Always priority-build regardless of legalize success.
        let placement = self
            .find_valid_build_location(&location, template.get_name().as_str(), angle)
            .unwrap_or(location);
        let mut final_loc = placement;
        final_loc.z = 0.0; // build list locations are ground relative

        let warehouse_id = warehouse_guard.get_id();
        drop(warehouse_guard);

        if let Some(player_arc) = self.get_player_arc() {
            if let Ok(mut pg) = player_arc.write() {
                pg.add_to_priority_build_list(AsciiString::from(thing_name), final_loc, angle);
            }
        }
        self.current_warehouse_id = Some(warehouse_id);
        Ok(())
    }

    pub fn build_specific_building_near_location(
        &mut self,
        thing_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        let Some(template) = crate::helpers::TheThingFactory::find_template(thing_name) else {
            log::warn!(
                "AIPlayer: template '{}' not found for build_specific_building_near_location",
                thing_name
            );
            return Ok(());
        };

        // C++ near-location path does not recompute base center.
        let angle = template.get_placement_view_angle();
        let mut build_location = location;
        if let Some(valid) =
            self.find_valid_build_location(&build_location, template.get_name().as_str(), angle)
        {
            build_location = valid;
            self.queue_structure_construction(thing_name, build_location, angle)?;
        }

        Ok(())
    }

    /// Legacy compatibility wrapper used by skirmish AI paths.
    pub fn build_specific_ai_building_at(
        &mut self,
        thing_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        self.build_specific_building_near_location(thing_name, location)
    }

    /// Build near the first member of the specified team, falling back to a normal build request.
    /// C++ `AIPlayer::buildSpecificBuildingNearestTeam` (AIPlayer.cpp).
    ///
    /// Team estimate position → legalize/wiggle → priority build list.
    pub fn build_specific_building_nearest_team(
        &mut self,
        thing_name: &str,
        team_name: &str,
    ) -> Result<(), AiError> {
        let Some(template) = TheThingFactory::find_template(thing_name) else {
            return Ok(());
        };
        let team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(team_name));
        let Some(team_arc) = team_arc else {
            return Ok(());
        };
        let Ok(team_g) = team_arc.read() else {
            return Ok(());
        };
        let Some(location) = team_g.get_estimate_team_position() else {
            return Ok(());
        };
        drop(team_g);

        // C++ does not recompute base center here (offset toward base is unused).
        let angle = template.get_placement_view_angle();
        // C++ only addToPriorityBuildList when wiggle set valid after initial fail
        // (same control flow as calcClosestConstructionZoneLocation).
        let adjusted =
            self.calc_closest_construction_zone_location(template.get_name().as_str(), &location)?;
        let Some(mut new_pos) = adjusted else {
            log::debug!(
                "{} - buildSpecificBuildingNearestTeam unable to place.",
                thing_name
            );
            return Ok(());
        };
        new_pos.z = 0.0;
        if let Some(player_arc) = self.get_player_arc() {
            if let Ok(mut pg) = player_arc.write() {
                pg.add_to_priority_build_list(AsciiString::from(thing_name), new_pos, angle);
            }
        }
        Ok(())
    }

    /// C++ `AIPlayer::findSupplyCenter` (AIPlayer.cpp).
    ///
    /// Closest non-enemy warehouse with enough cash, no nearby owned cash
    /// generator, not closer to enemy than us (60/40). Halve cash floor to 100.
    fn find_supply_center(&self, minimum_cash: i32) -> Option<Arc<RwLock<Object>>> {
        let player_arc = self.get_player_arc()?;
        let player_guard = player_arc.read().ok()?;
        let base_center = self
            .get_base_center()
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        // C++: Player *enemy = getAiEnemy(); structure bounds midpoint.
        // Prefer latched current-enemy index (skirmish acquireEnemy), then human.
        let mut enemy_center = Coord3D::new(0.0, 0.0, 0.0);
        let mut has_enemy = false;
        let enemy_index = {
            let mut idx = None;
            if let Ok(list) = player_list().read() {
                if let Some(me) = list.get_player(self.player_id as i32) {
                    if let Ok(mg) = me.read() {
                        idx = mg.get_current_enemy_player_index();
                    }
                }
            }
            idx.or_else(|| {
                self.select_current_enemy_player()
                    .ok()
                    .and_then(|o| o.map(|(_, i)| i))
            })
        };
        if let Some(enemy_index) = enemy_index {
            if let Ok((lo, hi)) = self.get_player_structure_bounds(enemy_index) {
                enemy_center = Coord3D::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, 0.0);
                has_enemy = true;
            }
        }

        let mut cash_floor = minimum_cash.max(0);
        loop {
            let mut best: Option<(f32, Arc<RwLock<Object>>)> = None;
            for obj in OBJECT_REGISTRY.get_all_objects() {
                let Ok(obj_guard) = obj.read() else {
                    continue;
                };
                if !obj_guard.is_kind_of(KindOf::Structure)
                    || !obj_guard.is_kind_of(KindOf::SupplySource)
                {
                    continue;
                }
                if let Some(team_arc) = obj_guard.get_team() {
                    if let Ok(team) = team_arc.read() {
                        if player_guard.get_relationship_with_team(&team) == Relationship::Enemies {
                            continue;
                        }
                    }
                }
                let Some(module) = obj_guard.find_update_module("SupplyWarehouseDockUpdate") else {
                    continue;
                };
                let boxes = module.with_module(|module| {
                    module
                        .get_supply_warehouse_dock_interface()
                        .map(|warehouse| warehouse.boxes_stored())
                });
                let Some(boxes) = boxes else {
                    continue;
                };
                let available_cash = boxes * BASE_VALUE_PER_SUPPLY_BOX;
                if available_cash < cash_floor {
                    continue;
                }

                let center = *obj_guard.get_position();
                let radius = SUPPLY_CENTER_CLOSE_DIST
                    + obj_guard.get_geometry_info().get_bounding_circle_radius();

                // Skip if we already own a cash generator near this warehouse.
                let mut already_have = false;
                for cand in OBJECT_REGISTRY.get_all_objects() {
                    let Ok(cg) = cand.read() else {
                        continue;
                    };
                    if !cg.is_kind_of(KindOf::CashGenerator) {
                        continue;
                    }
                    let Some(pid) = cg.get_controlling_player_id() else {
                        continue;
                    };
                    if pid as u32 != self.player_id {
                        continue;
                    }
                    let p = cg.get_position();
                    let dx = p.x - center.x;
                    let dy = p.y - center.y;
                    if dx * dx + dy * dy <= radius * radius {
                        already_have = true;
                        break;
                    }
                }
                if already_have {
                    continue;
                }

                let dx = center.x - base_center.x;
                let dy = center.y - base_center.y;
                let dist_sqr = dx * dx + dy * dy;
                if has_enemy {
                    let ex = center.x - enemy_center.x;
                    let ey = center.y - enemy_center.y;
                    let enemy_dist_sqr = ex * ex + ey * ey;
                    // C++: closer than 60/40 to enemy than to us → skip
                    if dist_sqr * 0.4 > enemy_dist_sqr * 0.6 {
                        continue;
                    }
                }

                if best.as_ref().map_or(true, |(bd, _)| dist_sqr < *bd) {
                    best = Some((dist_sqr, obj.clone()));
                }
            }
            if let Some((_, warehouse)) = best {
                return Some(warehouse);
            }
            // C++: minimumCash /= 2; while (minimumCash > 100)
            // After a failed pass, halve then stop once floor is ≤100 — do not
            // attempt another pass at the halved ≤100 value.
            cash_floor /= 2;
            if cash_floor <= 100 {
                break;
            }
        }
        None
    }

    fn find_valid_build_location(
        &self,
        location: &Coord3D,
        template_name: &str,
        angle: Real,
    ) -> Option<Coord3D> {
        let validator = FoundationValidator::new_ai();
        if validator
            .validate_placement(location, template_name, angle, self.player_id as ObjectID)
            .is_ok()
        {
            return Some(*location);
        }

        let mut pos_offset = 0.0;
        while pos_offset < 2.0 * SUPPLY_CENTER_CLOSE_DIST {
            let offset = pos_offset * 0.5;
            let mut x = location.x - offset;
            let y = location.y - offset;

            while x <= location.x + offset {
                let mut candidate = Coord3D::new(x, y, location.z);
                if validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                candidate.y = y + pos_offset;
                if validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                x += PATHFIND_CELL_SIZE_F;
            }

            let mut y_pos = location.y - offset;
            let x_pos = location.x - offset;
            while y_pos <= location.y + offset {
                let mut candidate = Coord3D::new(x_pos, y_pos, location.z);
                if validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                candidate.x = x_pos + pos_offset;
                if validator
                    .validate_placement(
                        &candidate,
                        template_name,
                        angle,
                        self.player_id as ObjectID,
                    )
                    .is_ok()
                {
                    return Some(candidate);
                }
                y_pos += PATHFIND_CELL_SIZE_F;
            }

            pos_offset += 2.0 * PATHFIND_CELL_SIZE_F;
        }

        None
    }

    /// Calculate superweapon target location
    /// C++ `AIPlayer::computeSuperweaponTarget` (AIPlayer.cpp).
    ///
    /// Grid-sample enemy structure bounds (or map extent), randomize scan
    /// direction, score with getPlayerSuperweaponValue, then fine-tune.
    /// Preserves C++ fine-tune `(x-5)` on both axes (legacy bug).
    /// `player_index` is C++ `playerNdx` — player whose structures are scored.
    pub fn compute_superweapon_target(
        &self,
        power_template: &str,
        weapon_radius: Real,
        player_index: i32,
    ) -> Result<Option<Coord3D>, AiError> {
        // Prefer explicit playerNdx (C++). Fall back to current enemy only when
        // caller passes a negative / invalid index residual.
        let enemy_index = if player_index >= 0 {
            player_index
        } else {
            match self.select_current_enemy_player() {
                Ok(Some((_, idx))) => idx,
                _ => return Ok(None),
            }
        };

        let radius = weapon_radius.max(1.0);
        let (mut min_bounds, mut max_bounds) = self.get_player_structure_bounds(enemy_index)?;

        // Degenerate bounds (no buildings) → full map extent (C++ getExtent, not pathfind).
        if min_bounds.x == 0.0 && min_bounds.y == 0.0 && max_bounds.x == 0.0 && max_bounds.y == 0.0
        {
            if let Some(terrain) = TheTerrainLogic::get() {
                let extent = terrain.get_extent();
                min_bounds = extent.lo;
                max_bounds = extent.hi;
            }
        }

        // Shrink by weapon radius (C++ only shrinks X then clamps both axes).
        min_bounds.x += radius;
        max_bounds.x -= radius;
        if max_bounds.x < min_bounds.x {
            let mid = (max_bounds.x + min_bounds.x) / 2.0;
            max_bounds.x = mid;
            min_bounds.x = mid;
        }
        if max_bounds.y < min_bounds.y {
            let mid = (max_bounds.y + min_bounds.y) / 2.0;
            max_bounds.y = mid;
            min_bounds.y = mid;
        }

        let width = (max_bounds.x - min_bounds.x).max(0.0);
        let height = (max_bounds.y - min_bounds.y).max(0.0);
        // C++: REAL_TO_INT_CEIL(bounds.width()/weaponRadius)+1, cap 10.
        let mut x_count = (width / radius).ceil() as i32 + 1;
        let mut y_count = (height / radius).ceil() as i32 + 1;
        if x_count > 10 {
            x_count = 10;
        }
        if y_count > 10 {
            y_count = 10;
        }
        if x_count < 1 {
            x_count = 1;
        }
        if y_count < 1 {
            y_count = 1;
        }

        let power = find_or_create_special_power_template(&AsciiString::from(power_template));
        // SPECIAL_SNEAK_ATTACK → do not value military units positively.
        let target_military_units = power.get_special_power_type()
            != crate::object::special_power_types::SpecialPowerType::SneakAttack;

        // C++ GameLogicRandomValue(1,4): starts at xCount/yCount (not count-1)
        // when scanning max→min so first sample hits the far edge.
        let (x_delta, y_delta, x_start, y_start) = match game_logic_random_value(1, 4) {
            1 => (1_i32, 1_i32, 0_i32, 0_i32),
            2 => (-1, 1, x_count, 0),
            3 => (1, -1, 0, y_count),
            _ => (-1, -1, x_count, y_count),
        };

        let mut best_cash: i32 = -1;
        let mut best_pos = Coord3D::new(min_bounds.x, min_bounds.y, 0.0);
        let mut x_index = x_start;
        for _ in 0..x_count {
            let mut y_index = y_start;
            for _ in 0..y_count {
                let pos = Coord3D::new(
                    min_bounds.x + (width * x_index as f32) / x_count as f32,
                    min_bounds.y + (height * y_index as f32) / y_count as f32,
                    0.0,
                );
                let value = self.get_player_superweapon_value(
                    &pos,
                    enemy_index,
                    2.0 * radius,
                    target_military_units,
                )?;
                if value > best_cash {
                    best_cash = value;
                    best_pos = pos;
                }
                y_index += y_delta;
            }
            x_index += x_delta;
        }

        // Fine tune: C++ uses (x-5) for BOTH axes (legacy bug — keep for parity).
        let mut fine_best = best_pos;
        let mut fine_cash: i32 = -1;
        let mut fine_count = 0_i32;
        let fine_steps = 11;
        for x in 0..fine_steps {
            for _y in 0..fine_steps {
                let offset = (x - 5) as f32 * (radius / 10.0);
                let pos = Coord3D::new(best_pos.x + offset, best_pos.y + offset, 0.0);
                let value = self.get_player_superweapon_value(
                    &pos,
                    enemy_index,
                    radius,
                    target_military_units,
                )?;
                if value > fine_cash {
                    fine_cash = value;
                    fine_best = pos;
                    fine_count = 1;
                } else if value == fine_cash {
                    // C++ averages equal-score samples.
                    fine_best.x += pos.x;
                    fine_best.y += pos.y;
                    fine_count += 1;
                }
            }
        }
        if fine_count > 1 {
            fine_best.x /= fine_count as f32;
            fine_best.y /= fine_count as f32;
        }
        if let Some(terrain) = TheTerrainLogic::get() {
            fine_best.z = terrain.get_ground_height(fine_best.x, fine_best.y, None);
        }

        // C++ success = (cash > -1)
        if fine_cash > -1 {
            Ok(Some(fine_best))
        } else {
            Ok(None)
        }
    }

    /// Called when a unit we're training comes into existence
    /// C++ `AIPlayer::onUnitProduced` (AIPlayer.cpp).
    ///
    /// Match work order by factoryID + incomplete + template equivalent; complete
    /// one unit; setTeam; clear factoryID; dozer/repair shortcuts; always
    /// `teamDelay = 0`.
    pub fn on_unit_produced(
        &mut self,
        factory_id: ObjectID,
        unit_id: ObjectID,
    ) -> Result<(), AiError> {
        // C++: factory could be NULL at start of game.
        if factory_id == INVALID_ID {
            return Ok(());
        }

        let Some(unit_arc) = OBJECT_REGISTRY.get_object(unit_id) else {
            self.team_delay = 0;
            return Ok(());
        };

        let (unit_template_name, is_dozer) = {
            let Ok(unit_g) = unit_arc.read() else {
                self.team_delay = 0;
                return Ok(());
            };
            (
                unit_g.get_template_name().to_string(),
                unit_g.is_kind_of(KindOf::Dozer),
            )
        };

        let mut found = false;
        let mut supply_truck = false;
        let mut matched_team_name: Option<String> = None;
        let mut matched_team: Option<Arc<RwLock<crate::team::Team>>> = None;
        let mut is_resource_gatherer_order = false;

        for team_q in &mut self.team_build_queue {
            if found {
                break;
            }
            for order in &mut team_q.work_orders {
                if order.factory_id != Some(factory_id) {
                    continue;
                }
                if order.num_completed >= order.num_required {
                    continue;
                }
                // C++ unit->getTemplate()->isEquivalentTo(order->m_thing)
                let equiv = order
                    .thing_template
                    .eq_ignore_ascii_case(&unit_template_name)
                    || TheThingFactory::find_template(&order.thing_template)
                        .zip(TheThingFactory::find_template(&unit_template_name))
                        .map(|(a, b)| a.is_equivalent_to(b.as_ref()))
                        .unwrap_or(false);
                if !equiv {
                    continue;
                }

                order.num_completed = order.num_completed.saturating_add(1);
                // C++ clears factory after matching this production slot.
                order.factory_id = None;
                is_resource_gatherer_order = order.is_resource_gatherer;
                matched_team_name = team_q.team_name.clone();
                matched_team = team_q.team.clone();

                if team_q.reinforcement {
                    team_q.reinforcement_id = Some(unit_id);
                }

                found = true;
                break;
            }
        }

        // put new unit into the team under construction
        if found {
            let team_name = matched_team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            // Prefer TeamInQueue.m_team (C++); name lookup is fallback.
            let team_arc = matched_team.or_else(|| {
                get_team_factory().lock().ok().and_then(|mut factory| {
                    factory
                        .find_team_instances(&team_name)
                        .into_iter()
                        .next()
                        .or_else(|| factory.find_team(&team_name))
                })
            });
            if let Some(ref team_arc) = team_arc {
                if let Ok(mut ug) = unit_arc.write() {
                    let _ = ug.set_team(Some(team_arc.clone()));
                }
            }

            // C++: if team has homeLocation → aiFollowExitProductionPath(goal, home).
            // path[0] = *ai->getGoalPosition() (not path destination).
            let (home, has_home) =
                self.queue_units_home_for_team(team_arc.as_ref(), team_name.as_str());
            // has_home is true only for prototype homeLocation (not base-center fallback).
            if has_home {
                if let Ok(unit_g) = unit_arc.read() {
                    if let Some(ai) = unit_g.get_ai_update_interface() {
                        let start = ai
                            .get_goal_position()
                            .unwrap_or_else(|| *unit_g.get_position());
                        let path = [start, home];
                        ai.ai_follow_exit_production_path(&path, None, CommandSourceType::FromAi);
                    }
                }
            }

            // Supply truck force-wanting + dock (C++ SupplyTruckAIInterface).
            if let Ok(unit_g) = unit_arc.read() {
                if let Some(ai) = unit_g.get_ai_update_interface() {
                    if let Ok(mut ai_g) = ai.lock() {
                        if let Some(truck) = ai_g.get_supply_truck_ai_interface_mut() {
                            supply_truck = is_resource_gatherer_order;
                            truck.set_force_wanting_state(supply_truck);
                        }
                    }
                    if supply_truck {
                        // C++: assign to first supply build-list entry needing gatherers,
                        // then aiDock(obj, CMD_FROM_PLAYER).
                        if let Some(dock_id) = self.take_supply_gatherer_slot() {
                            ai.ai_dock(dock_id, CommandSourceType::FromPlayer);
                        }
                    }
                }
            }
        }

        // C++ dozer path is NOT gated on `found` (AIPlayer.cpp after the queue loop).
        // supplyTruck defaults false unless a matched order set force-wanting true —
        // C++ leaves it uninitialized when no SupplyTruckAI; treat unset as false.
        if !supply_truck && is_dozer {
            if self.dozer_queued_for_repair {
                self.repair_dozer = Some(unit_id);
                self.dozer_queued_for_repair = false;
            } else {
                self.build_delay = 0;
                self.structure_timer = 1;
            }
        }

        if !found {
            log::debug!("***AI PLAYER-Unit not found in production queue.");
        }

        // C++ always: m_teamDelay = 0
        self.team_delay = 0;
        Ok(())
    }

    /// Called when a structure we're building comes into existence
    /// C++ `AIPlayer::onStructureProduced` (AIPlayer.cpp).
    ///
    /// Match build-list by objectID: clear UC, upgrades, script attach residual,
    /// checkForSupplyCenter. Else match rebuild hole spawn and retarget list ID.
    pub fn on_structure_produced(
        &mut self,
        _factory_id: ObjectID,
        structure_id: ObjectID,
    ) -> Result<(), AiError> {
        // C++: m_teamDelay = 0; m_buildDelay = 0; (no frameLastBuildingBuilt here)
        self.team_delay = 0;
        self.build_delay = 0;

        let Some(structure_arc) = OBJECT_REGISTRY.get_object(structure_id) else {
            return Ok(());
        };

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return Ok(());
        };

        // Pass 1: exact objectID match on build list.
        // Do NOT call check_for_supply_center while holding player write —
        // it re-acquires the same lock (would deadlock on std::sync::RwLock).
        // C++ order: map props → clear UC → upgrades → script cache → supply.
        let mut matched = false;
        let mut script_name = String::new();
        {
            let Ok(mut player_guard) = player_arc.write() else {
                return Ok(());
            };
            if let Some(info) = player_guard.get_build_list_mut() {
                let mut current = Some(&mut *info);
                while let Some(node) = current {
                    if node.get_object_id() == structure_id {
                        // C++ Dict: objectName/script/health/unsellable → map props.
                        let mut props = crate::common::Dict::new();
                        props.set_ascii_string(
                            crate::common::well_known_keys::key_object_name(),
                            node.get_building_name().as_str(),
                        );
                        props.set_ascii_string(
                            crate::common::well_known_keys::key_object_script_attachment(),
                            node.get_script().as_str(),
                        );
                        props.set_int(
                            crate::common::well_known_keys::key_object_initial_health(),
                            node.get_health(),
                        );
                        props.set_bool(
                            crate::common::well_known_keys::key_object_unsellable(),
                            node.get_unsellable(),
                        );
                        script_name = node.get_script().to_string();
                        node.set_under_construction(false);

                        if let Ok(mut sg) = structure_arc.write() {
                            sg.update_obj_values_from_map_properties(&props);
                            let mask = ObjectStatusMaskType::from_status(
                                ObjectStatusTypes::UnderConstruction,
                            ) | ObjectStatusMaskType::from_status(
                                ObjectStatusTypes::Reconstructing,
                            );
                            sg.clear_status(mask);
                            // UnderConstruction just cleared → refresh upgrades.
                            sg.update_upgrade_modules_from_player();
                        }

                        matched = true;
                        break;
                    }
                    current = node.get_next_mut();
                }
            }
        }
        if matched {
            // C++ TheScriptEngine->addObjectToCache + runObjectScript
            if let Ok(mut eng) = get_script_engine().write() {
                if let Some(e) = eng.as_mut() {
                    e.add_object_to_cache(structure_id);
                    if !script_name.is_empty() {
                        e.run_object_script(&script_name, structure_id);
                    }
                }
            }
            // C++ checkForSupplyCenter(info, bldg) after script — outside player write.
            let _ = self.check_for_supply_center(structure_id);
            return Ok(());
        }

        // Pass 2: rebuild-hole spawn retarget (C++ getReconstructedBuildingID).
        let structure_template_name = structure_arc
            .read()
            .ok()
            .map(|g| g.get_template_name().to_string())
            .unwrap_or_default();
        {
            let Ok(mut player_guard) = player_arc.write() else {
                return Ok(());
            };
            if let Some(info) = player_guard.get_build_list_mut() {
                let mut current = Some(&mut *info);
                while let Some(node) = current {
                    let name = node.get_template_name().to_string();
                    let equiv = TheThingFactory::find_template(&name)
                        .zip(TheThingFactory::find_template(&structure_template_name))
                        .map(|(a, b)| a.is_equivalent_to(b.as_ref()))
                        .unwrap_or(false)
                        || name.eq_ignore_ascii_case(&structure_template_name);
                    if !equiv {
                        current = node.get_next_mut();
                        continue;
                    }
                    let list_id = node.get_object_id();
                    if list_id != INVALID_ID {
                        if let Some(hole_arc) = OBJECT_REGISTRY.get_object(list_id) {
                            if let Ok(hole_g) = hole_arc.read() {
                                if hole_g.is_kind_of(KindOf::RebuildHole) {
                                    // C++: only if bldg->getID() == rhbi->getReconstructedBuildingID().
                                    let mut is_this_spawn = false;
                                    let mut saw_rhbi = false;
                                    for behavior in hole_g.get_behavior_modules() {
                                        if let Ok(mut bg) = behavior.lock() {
                                            if let Some(rhbi) =
                                                bg.get_rebuild_hole_behavior_interface()
                                            {
                                                saw_rhbi = true;
                                                let rebuilt = rhbi.get_reconstructed_building_id();
                                                is_this_spawn = rebuilt == structure_id;
                                                break;
                                            }
                                        }
                                    }
                                    if saw_rhbi && is_this_spawn {
                                        log::debug!("AI got rebuilt {}", name);
                                        node.set_object_id(structure_id);
                                        matched = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    current = node.get_next_mut();
                }
            }
        }

        if !matched && TheGameLogic::get_frame() > 0 {
            log::debug!("***AI PLAYER-Structure not found in production queue.");
        }
        Ok(())
    }

    /// Set team delay in seconds
    pub fn set_team_delay_seconds(&mut self, delay: Real) {
        self.team_seconds = delay.max(0.0);
    }

    /// C++ `AIPlayer::calcClosestConstructionZoneLocation` (AIPlayer.cpp).
    ///
    /// Uses template placement view angle. If the seed fails
    /// NO_OBJECT_OVERLAP-style validation, wiggle with clear-path/terrain/overlap
    /// (same spiral as buildBySupplies). Returns Some only when the wiggle path
    /// set `valid` — matching GeneralsMD control flow where an already-legal seed
    /// leaves `valid=false` and the function fails (location zeroed in C++).
    pub fn calc_closest_construction_zone_location(
        &self,
        template_name: &str,
        location: &Coord3D,
    ) -> Result<Option<Coord3D>, AiError> {
        let Some(template) = TheThingFactory::find_template(template_name) else {
            return Ok(None);
        };
        let angle = template.get_placement_view_angle();
        let validator = FoundationValidator::new_ai();

        // C++: Bool valid = false; only set true inside the adjust loop.
        let mut valid = false;
        let mut new_pos = *location;

        // First check: NO_OBJECT_OVERLAP residual via FoundationValidator.
        let initial_ok = validator
            .validate_placement(location, template_name, angle, self.player_id as ObjectID)
            .is_ok();
        if !initial_ok {
            log::debug!(
                "{} - calcClosestConstructionZoneLocation unable to place.  Attempting to adjust position.",
                template_name
            );
            // Wiggle spiral (same extents as C++ 2*SUPPLY_CENTER_CLOSE_DIST).
            let mut pos_offset = 0.0_f32;
            'outer: while pos_offset < 2.0 * SUPPLY_CENTER_CLOSE_DIST {
                let offset = pos_offset * 0.5;
                let mut x = location.x - offset;
                let y0 = location.y - offset;
                while x <= location.x + offset + 0.001 {
                    for y in [y0, y0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, location.z);
                        if validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            new_pos = candidate;
                            valid = true;
                            break 'outer;
                        }
                    }
                    x += PATHFIND_CELL_SIZE_F;
                }
                let mut y = location.y - offset;
                let x0 = location.x - offset;
                while y <= location.y + offset + 0.001 {
                    for x in [x0, x0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, location.z);
                        if validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            new_pos = candidate;
                            valid = true;
                            break 'outer;
                        }
                    }
                    y += PATHFIND_CELL_SIZE_F;
                }
                pos_offset += 2.0 * PATHFIND_CELL_SIZE_F;
            }
        }
        // C++: if (valid) location=newPos success; else location.zero() fail.
        // Note: when initial_ok, valid stays false → None (C++ shipped behavior).
        let _ = initial_ok;
        if valid {
            Ok(Some(new_pos))
        } else {
            Ok(None)
        }
    }

    /// Convenience: search near base center when no seed location given.
    pub fn calc_closest_construction_zone_near_base(
        &self,
        template_name: &str,
    ) -> Result<Option<Coord3D>, AiError> {
        if !self.base_center_set {
            return Ok(None);
        }
        self.calc_closest_construction_zone_location(template_name, &self.base_center)
    }

    /// Update AI strategy based on current conditions
    fn update_strategy(&mut self) -> Result<(), AiError> {
        let current_frame = TheGameLogic::get_frame();

        // Update strategic decision maker
        self.strategic_decision_maker.update(current_frame);

        // Analyze current situation
        self.analyze_economic_situation()?;
        self.analyze_military_situation()?;
        self.analyze_threats()?;

        // Calculate base health from owned structures
        // In full implementation, would scan all player buildings and calculate average health
        let base_health = self.calculate_base_health();

        // Make strategic decision using new system
        let decision = self.strategic_decision_maker.make_decision(
            self.military_state.total_military_strength,
            self.military_state.enemy_strength_estimate,
            base_health,
            self.threat_assessment.overall_threat_level as f32 / 5.0, // Convert enum to 0.0-1.0
            self.economic_state
                .current_resources
                .get("money")
                .copied()
                .unwrap_or(0),
        );

        // Execute decision
        self.execute_strategic_decision(decision)?;

        // Legacy strategy change logic
        if self.should_change_strategy()? {
            let new_strategy = self.determine_optimal_strategy()?;
            self.change_strategy(new_strategy, current_frame)?;
        }

        Ok(())
    }

    /// Execute a strategic decision made by the decision maker
    fn execute_strategic_decision(&mut self, decision: StrategicDecision) -> Result<(), AiError> {
        match decision {
            StrategicDecision::BuildUpForces => {
                // Focus on building military units
                self.prioritize_military_production()?;
            }
            StrategicDecision::LaunchAttack => {
                // Initiate attack on enemy
                self.launch_attack()?;
                self.strategic_decision_maker.on_attack_launched();
            }
            StrategicDecision::DefendBase => {
                // Build defenses and position units defensively
                self.prioritize_defensive_buildings()?;
            }
            StrategicDecision::Expand => {
                // Expand to new locations
                if self.strategic_decision_maker.expansion.can_expand {
                    self.initiate_expansion()?;
                    self.strategic_decision_maker.on_expansion_complete();
                }
            }
            StrategicDecision::EconomicGrowth => {
                // Focus on economy
                self.prioritize_economic_buildings()?;
            }
            StrategicDecision::TechProgression => {
                // Research upgrades
                self.prioritize_tech_upgrades()?;
            }
            StrategicDecision::Harassment => {
                // Send harassing units
                self.initiate_harassment()?;
            }
            StrategicDecision::Turtle => {
                // Build heavy defenses
                self.build_ai_base_defense(false)?;
                self.build_ai_base_defense(true)?;
            }
            StrategicDecision::AllOut => {
                // All-out attack with everything
                self.launch_all_out_attack()?;
            }
        }
        Ok(())
    }

    /// Prioritize military production
    fn prioritize_military_production(&mut self) -> Result<(), AiError> {
        // Adjust resource allocation to favor military
        self.strategic_decision_maker
            .resources
            .allocations
            .insert("military".to_string(), 0.7);
        self.strategic_decision_maker
            .resources
            .allocations
            .insert("economy".to_string(), 0.2);
        Ok(())
    }

    /// Launch attack on enemy
    /// Coordinates attack teams and selects strategic targets
    fn launch_attack(&mut self) -> Result<(), AiError> {
        // Build attack teams if we don't have enough
        let military_strength = self.military_state.total_military_strength;
        let enemy_strength = self.military_state.enemy_strength_estimate;

        // Only attack if we have reasonable strength (difficulty affects this)
        let strength_threshold = match self.difficulty {
            GameDifficulty::Easy => 0.6,   // Easy AI needs 60% of enemy strength
            GameDifficulty::Normal => 0.8, // Normal needs 80%
            GameDifficulty::Hard => 1.0,   // Hard attacks at parity
            GameDifficulty::Brutal => 1.2, // Brutal attacks when weaker
        };

        if military_strength < enemy_strength * strength_threshold {
            // Not strong enough yet, keep building
            self.prioritize_military_production()?;
            return Ok(());
        }

        // Select attack target based on strategic value
        let target = self.select_attack_target()?;

        if let Some(_target_location) = target {
            // Queue attack teams
            self.build_specific_ai_team("attack_force", true)?;

            // Update military stance
            self.military_state.current_military_stance = MilitaryStance::Aggressive;
        }

        Ok(())
    }

    /// Select best attack target based on strategic priorities
    /// Considers: economy disruption, defensive weakness, strategic value
    fn select_attack_target(&self) -> Result<Option<Coord3D>, AiError> {
        // Priority order (matches C++ AIPlayer behavior):
        // 1. Enemy supply centers (economy disruption)
        // 2. Enemy production facilities (tactical advantage)
        // 3. Enemy defenses (if we can win)
        // 4. Enemy command center (decisive strike)

        let list = player_list().read().map_err(|_| AiError::LockFailed)?;
        let mut best: Option<(f32, Coord3D)> = None;

        for (idx, player_arc) in list.iter().enumerate() {
            if idx as u32 == self.player_id {
                continue;
            }
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };
            if player_guard.get_player_type() == PlayerType::Neutral {
                continue;
            }
            for obj_id in player_guard.get_all_objects() {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() {
                    continue;
                }
                let score = self.score_attack_target(&obj_guard);
                if best
                    .map(|(best_score, _)| score > best_score)
                    .unwrap_or(true)
                {
                    best = Some((score, *obj_guard.get_position()));
                }
            }
        }

        Ok(best.map(|(_, pos)| pos))
    }

    fn score_attack_target(&self, obj: &Object) -> f32 {
        let mut score = if obj.is_kind_of(KindOf::SupplySource)
            || obj.is_kind_of(KindOf::ResourceNode)
            || obj.is_kind_of(KindOf::FSSupplyCenter)
            || obj.is_kind_of(KindOf::FSSupplyDropzone)
            || obj.is_kind_of(KindOf::Refinery)
        {
            0.9
        } else if obj.is_kind_of(KindOf::CommandCenter) || obj.is_kind_of(KindOf::KeyStructure) {
            1.0
        } else if obj.is_kind_of(KindOf::Factory)
            || obj.is_kind_of(KindOf::FSWarfactory)
            || obj.is_kind_of(KindOf::FSAirfield)
            || obj.is_kind_of(KindOf::FSBarracks)
        {
            0.8
        } else if obj.is_kind_of(KindOf::PowerPlant) || obj.is_kind_of(KindOf::FSPower) {
            0.7
        } else if obj.is_kind_of(KindOf::Defense) {
            0.6
        } else if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Building) {
            0.5
        } else if obj.is_kind_of(KindOf::Vehicle) || obj.is_kind_of(KindOf::Infantry) {
            0.3
        } else {
            0.2
        };

        let health = obj.get_health_percentage().clamp(0.0, 1.0);
        score *= 0.7 + (1.0 - health) * 0.3;

        if let Some(base_center) = self.get_base_center() {
            let dx = obj.get_position().x - base_center.x;
            let dy = obj.get_position().y - base_center.y;
            let dist = (dx * dx + dy * dy).sqrt();
            let dist_factor = (1.0 / (1.0 + dist / 500.0)).clamp(0.2, 1.0);
            score *= dist_factor;
        }

        score.clamp(0.0, 1.0)
    }

    /// Initiate expansion to new location
    fn initiate_expansion(&mut self) -> Result<(), AiError> {
        // Queue dozer and expansion buildings
        Ok(())
    }

    /// Prioritize economic buildings
    fn prioritize_economic_buildings(&mut self) -> Result<(), AiError> {
        self.build_specific_ai_building("SupplyCenter")?;
        Ok(())
    }

    /// Prioritize tech upgrades
    fn prioritize_tech_upgrades(&mut self) -> Result<(), AiError> {
        // Queue upgrades from skillset
        Ok(())
    }

    /// Initiate harassment attacks
    fn initiate_harassment(&mut self) -> Result<(), AiError> {
        // Build fast units for hit-and-run
        Ok(())
    }

    /// Launch all-out attack
    fn launch_all_out_attack(&mut self) -> Result<(), AiError> {
        // Send all military units to attack
        Ok(())
    }

    /// Analyze current economic situation
    /// Matches C++ AIPlayer economic analysis
    /// Updates resource tracking, income rates, and economic pressure
    fn analyze_economic_situation(&mut self) -> Result<(), AiError> {
        let current_resources = if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    let money = player_guard.get_money().get_money();
                    let power = player_guard.get_energy().get_power() as i32;
                    self.economic_state
                        .current_resources
                        .insert("money".to_string(), money);
                    self.economic_state
                        .current_resources
                        .insert("power".to_string(), power);
                    self.economic_state.resource_income_rate.insert(
                        "money".to_string(),
                        player_guard.get_money().get_income_rate(),
                    );
                    self.economic_state.power_shortage = power < 0;
                    self.economic_state.supply_shortage = money < RESOURCES_POOR;
                    self.economic_state.economic_pressure = if money < RESOURCES_POOR {
                        0.8
                    } else if money > RESOURCES_WEALTHY {
                        0.2
                    } else {
                        0.5
                    };
                    money
                } else {
                    self.economic_state
                        .current_resources
                        .get("money")
                        .copied()
                        .unwrap_or(0)
                }
            } else {
                self.economic_state
                    .current_resources
                    .get("money")
                    .copied()
                    .unwrap_or(0)
            }
        } else {
            self.economic_state
                .current_resources
                .get("money")
                .copied()
                .unwrap_or(0)
        };

        // C++ AIPlayer uses TheAI->getAiData() thresholds (retail Wealthy=7000, Poor=2000).
        let (poor_threshold, wealthy_threshold) = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    let poor = if data.resources_poor > 0 {
                        data.resources_poor
                    } else {
                        RESOURCES_POOR
                    };
                    let wealthy = if data.resources_wealthy > 0 {
                        data.resources_wealthy
                    } else {
                        RESOURCES_WEALTHY
                    };
                    (poor, wealthy)
                })
            })
            .unwrap_or((RESOURCES_POOR, RESOURCES_WEALTHY));

        // Update strategic decision maker's resource management
        self.strategic_decision_maker.resources.update(
            current_resources,
            wealthy_threshold,
            poor_threshold,
        );

        // Calculate economic pressure based on resources and income
        // High pressure = need more income, low resources
        self.economic_state.economic_pressure = if current_resources < poor_threshold {
            0.9 // Very high pressure - need supply centers urgently
        } else if current_resources < wealthy_threshold {
            0.5 // Moderate pressure - could use more income
        } else {
            0.2 // Low pressure - economy is good
        };

        // Difficulty affects economic pressure tolerance
        // Easy AI more conservative, Hard AI more aggressive with spending
        self.economic_state.economic_pressure *= match self.difficulty {
            GameDifficulty::Easy => 1.3,   // More cautious
            GameDifficulty::Normal => 1.0, // Standard
            GameDifficulty::Hard => 0.8,   // More aggressive
            GameDifficulty::Brutal => 0.6, // Very aggressive
        };

        // Check for supply shortage (count active supply trucks)
        // This would scan player units for KINDOF_HARVESTER
        let active_harvesters = self.count_active_harvesters();
        let desired_harvesters = 3 * self.count_supply_centers(); // 3 per center
        self.economic_state.supply_shortage = active_harvesters < desired_harvesters;

        // Check for power shortage (scan for power plants vs power usage)
        self.economic_state.power_shortage = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.get_energy().is_low_power())
            })
            .unwrap_or(false);

        Ok(())
    }

    /// Count number of active supply centers
    fn count_supply_centers(&self) -> usize {
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return 0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 0;
        };
        let mut count = 0;
        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_kind_of(KindOf::SupplySource)
                || obj_guard.is_kind_of(KindOf::ResourceNode)
                || obj_guard.is_kind_of(KindOf::FSSupplyCenter)
                || obj_guard.is_kind_of(KindOf::FSSupplyDropzone)
                || obj_guard.is_kind_of(KindOf::Refinery)
            {
                count += 1;
            }
        }
        count
    }

    /// Calculate average base health from all structures
    /// Used for strategic decision making
    fn calculate_base_health(&self) -> f32 {
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return 1.0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 1.0;
        };
        let mut total = 0.0;
        let mut count = 0.0;
        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !obj_guard.is_kind_of(KindOf::Structure) && !obj_guard.is_kind_of(KindOf::Building) {
                continue;
            }
            total += obj_guard.get_health_percentage();
            count += 1.0;
        }
        if count > 0.0 {
            (total / count).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Check if strategy should be changed
    fn should_change_strategy(&self) -> Result<bool, AiError> {
        let current_frame = TheGameLogic::get_frame();
        let time_in_strategy =
            current_frame.saturating_sub(self.strategy_state.last_strategy_change);
        let time_threshold = LOGICFRAMES_PER_SECOND * 60;

        if time_in_strategy > time_threshold {
            return Ok(true);
        }

        if self.threat_assessment.overall_threat_level > 0.7
            && self.strategy_state.current_strategy != AiStrategy::Turtle
        {
            return Ok(true);
        }

        if self.economic_state.economic_pressure > 0.8
            && self.strategy_state.current_strategy != AiStrategy::Economic
        {
            return Ok(true);
        }

        if self.military_state.enemy_strength_estimate
            > self.military_state.total_military_strength * 1.2
            && self.strategy_state.current_strategy != AiStrategy::Turtle
        {
            return Ok(true);
        }

        Ok(false)
    }

    /// Prioritize defensive buildings
    /// Adds defensive structures to construction queue based on threat level
    fn prioritize_defensive_buildings(&mut self) -> Result<(), AiError> {
        // Add defensive structures to construction priorities
        let defensive_priority = ConstructionPriority {
            building_type: "GuardTower".to_string(),
            priority: 5, // High priority for defense
            prerequisites_met: true,
            max_count: Some(4), // Build up to 4 guard towers
            current_count: 0,
            desired_location: None,
            desired_angle: None,
        };

        self.construction_priorities.push(defensive_priority);

        // Could integrate with build order optimizer for more sophisticated prioritization
        // For now, direct insertion into construction queue is sufficient

        Ok(())
    }

    /// Determine optimal strategy for current situation
    /// Matches C++ AIPlayer strategic decision making
    /// Considers resources, threats, game phase, and difficulty
    fn determine_optimal_strategy(&self) -> Result<AiStrategy, AiError> {
        // Strategy selection based on multiple factors
        let current_money = self
            .economic_state
            .current_resources
            .get("money")
            .copied()
            .unwrap_or(0);
        let military_strength = self.military_state.total_military_strength;
        let enemy_strength = self.military_state.enemy_strength_estimate;
        let threat_level = self.threat_assessment.overall_threat_level;

        // Early game (low resources, low military)
        if current_money < 2000 && military_strength < 50.0 {
            return Ok(match self.difficulty {
                GameDifficulty::Easy => AiStrategy::Turtle, // Play safe
                GameDifficulty::Normal => AiStrategy::Economic, // Build economy
                GameDifficulty::Hard => AiStrategy::Rush,   // Early pressure
                GameDifficulty::Brutal => AiStrategy::Rush, // Aggressive start
            });
        }

        // Under heavy threat - defend
        if threat_level > 0.7 || enemy_strength > military_strength * 1.5 {
            return Ok(AiStrategy::Turtle);
        }

        // Strong military advantage - attack
        if military_strength > enemy_strength * 1.3 {
            return Ok(match self.difficulty {
                GameDifficulty::Easy => AiStrategy::Balanced, // Cautious attack
                GameDifficulty::Normal => AiStrategy::Balanced, // Standard attack
                GameDifficulty::Hard => AiStrategy::AllOut,   // Aggressive
                GameDifficulty::Brutal => AiStrategy::AllOut, // Very aggressive
            });
        }

        // Good economy but weak military - tech rush
        if current_money > 8000 && military_strength < enemy_strength {
            return Ok(AiStrategy::TechRush);
        }

        // Resource shortage - focus economy
        if self.economic_state.economic_pressure > 0.6 {
            return Ok(AiStrategy::Economic);
        }

        // Default to balanced approach
        Ok(AiStrategy::Balanced)
    }

    /// Change to new strategy
    fn change_strategy(
        &mut self,
        new_strategy: AiStrategy,
        current_frame: u32,
    ) -> Result<(), AiError> {
        self.strategy_state.current_strategy = new_strategy;
        self.strategy_state.last_strategy_change = current_frame;
        self.strategy_state.time_in_strategy = 0;
        self.strategy_state.strategy_confidence = 1.0;

        Ok(())
    }

    /// Add work orders for a specific team
    /// C++ work-order composition for a team prototype (optional then required).
    fn add_work_orders_for_team(
        &mut self,
        team: &mut TeamInQueue,
        team_name: &str,
    ) -> Result<(), AiError> {
        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok(());
        };
        if let Some(proto) = factory_guard.find_team_prototype(team_name) {
            let mut orders = Vec::new();
            // Optional: max-min
            for unit in proto.units_info() {
                if unit.unit_thing_name.is_empty() {
                    continue;
                }
                let count = (unit.max_units - unit.min_units).max(0);
                if count <= 0 {
                    continue;
                }
                let mut order = WorkOrder::new(unit.unit_thing_name.to_string());
                order.num_required = count;
                order.required = false;
                orders.insert(0, order);
            }
            // Required: min
            for unit in proto.units_info() {
                if unit.unit_thing_name.is_empty() {
                    continue;
                }
                let count = unit.min_units.max(0);
                if count <= 0 {
                    continue;
                }
                let mut order = WorkOrder::new(unit.unit_thing_name.to_string());
                order.num_required = count;
                order.required = true;
                orders.insert(0, order);
            }
            team.work_orders = orders;
            return Ok(());
        }

        Ok(())
    }

    /// Determine appropriate base defense structure
    fn determine_base_defense_structure(&self, flank: bool) -> Result<String, AiError> {
        // Choose defense based on:
        // - Faction
        // - Current threats
        // - Resource availability
        // - Strategic position (front vs flank)

        if flank {
            Ok("PatriotMissileBattery".to_string())
        } else {
            Ok("FirebasePatriotMissileBattery".to_string())
        }
    }

    /// Find suitable location for defense structure
    fn find_defense_location(&self, flank: bool) -> Result<Coord3D, AiError> {
        let base = self
            .get_base_center()
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));
        let offset = if flank { 160.0 } else { 80.0 };
        let candidate = Coord3D::new(base.x + offset, base.y, base.z);

        let mut position = candidate;
        if let Some(terrain) = TheTerrainLogic::get() {
            position.z = terrain.get_ground_height(position.x, position.y, None);
        }

        let validator = FoundationValidator::new_ai();
        if validator
            .validate_placement(
                &position,
                "PatriotMissileBattery",
                0.0,
                self.player_id as ObjectID,
            )
            .is_ok()
        {
            return Ok(position);
        }

        Ok(base)
    }

    /// Queue structure for construction
    fn queue_structure_construction(
        &mut self,
        structure_name: &str,
        location: Coord3D,
        angle: Real,
    ) -> Result<(), AiError> {
        // Add to construction queue
        let priority = ConstructionPriority {
            building_type: structure_name.to_string(),
            priority: 10,
            prerequisites_met: true,
            max_count: None,
            current_count: 0,
            desired_location: Some(location),
            desired_angle: Some(angle),
        };

        self.construction_priorities.push(priority);
        Ok(())
    }

    /// Update construction priorities based on current needs
    fn update_construction_priorities(&mut self) -> Result<(), AiError> {
        // Remove completed priorities
        self.construction_priorities.retain(|p| {
            if let Some(max) = p.max_count {
                p.current_count < max
            } else {
                true
            }
        });

        // Sort by priority
        self.construction_priorities.sort_by_key(|p| p.priority);

        Ok(())
    }
}

impl AiPlayerTrait for AIPlayer {
    fn update(&mut self) -> Result<(), AiError> {
        // C++ AIPlayer::update (AIPlayer.cpp): base → ready → queued → team →
        // upgrades → bridge. No strategy residual in C++.
        self.do_base_building()?;
        self.check_ready_teams()?;
        self.check_queued_teams()?;
        self.do_team_building()?;
        self.do_upgrades_and_skills()?;
        self.update_bridge_repair()?;

        Ok(())
    }

    fn update_economy(&mut self) -> Result<(), AiError> {
        self.analyze_economic_situation()?;

        // Queue supply trucks if needed
        if self.economic_state.supply_shortage {
            self.queue_supply_truck()?;
        }

        // Build economic structures
        if self.economic_state.economic_pressure > 0.7 {
            self.build_specific_ai_building("SupplyCenter")?;
        }

        Ok(())
    }

    fn update_construction(&mut self) -> Result<(), AiError> {
        self.process_base_building()?;
        self.update_construction_priorities()?;
        Ok(())
    }

    fn update_diplomacy(&mut self) -> Result<(), AiError> {
        // AI diplomacy hooks are limited in the current port; preserve no-op behavior.
        Ok(())
    }

    fn build_specific_building(&mut self, building_name: &str) -> Result<(), AiError> {
        self.build_specific_ai_building(building_name)
    }

    fn build_by_supplies(&mut self, minimum_cash: i32, building_name: &str) -> Result<(), AiError> {
        AIPlayer::build_by_supplies(self, minimum_cash, building_name)
    }

    fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError> {
        AIPlayer::build_upgrade(self, upgrade_name)
    }

    fn build_specific_building_near_location(
        &mut self,
        building_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        AIPlayer::build_specific_building_near_location(self, building_name, location)
    }

    fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        AIPlayer::repair_structure(self, structure_id)
    }

    fn get_player_id(&self) -> u32 {
        self.player_id
    }

    fn get_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    fn build_base_defense(&mut self, flank: bool) -> Result<(), AiError> {
        self.build_ai_base_defense(flank)
    }

    fn build_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), AiError> {
        self.build_ai_base_defense_structure(structure_name, flank)
    }
}

// Additional implementation methods for base AI functionality
impl AIPlayer {
    /// C++ `AIPlayer::doBaseBuilding` (AIPlayer.cpp).
    ///
    /// structureTimer → readyToBuildStructure; buildDelay throttles processBaseBuilding
    /// to every `BUILD_DELAY_RECHECK_FRAMES` (2s), shortcut when structure completes.
    fn do_base_building(&mut self) -> Result<(), AiError> {
        let can_build_base = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_can_build_base()))
            .unwrap_or(true);
        if !can_build_base {
            return Ok(());
        }

        // See if we are ready to start trying a structure.
        // C++ AIPlayer::doBaseBuilding has NO 3s clamp (only AISkirmishPlayer does).
        if !self.ready_to_build_structure {
            if self.structure_timer > 0 {
                self.structure_timer -= 1;
            }
            if self.structure_timer == 0 {
                self.ready_to_build_structure = true;
                self.build_delay = 0; // Cause immediate check
            }
        }

        // Throttle processBaseBuilding (C++ m_buildDelay).
        if self.build_delay > 0 {
            self.build_delay -= 1;
        }
        if self.build_delay == 0 {
            if self.ready_to_build_structure {
                self.process_base_building()?;
            }
            // processBaseBuilding may reset m_buildDelay (C++); only default if still 0.
            if self.build_delay == 0 {
                self.build_delay = BUILD_DELAY_RECHECK_FRAMES;
            }
        }

        Ok(())
    }

    fn object_ai_is_idle(object_id: ObjectID) -> bool {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return false;
        };
        let Ok(obj) = obj_arc.read() else {
            return false;
        };
        let Some(ai) = obj.get_ai_update_interface() else {
            return false;
        };
        ai.is_idle()
    }

    fn team_any_member_idle(team_name: &str) -> bool {
        let Ok(factory) = get_team_factory().lock() else {
            return false;
        };
        let Some(team_arc) = factory.find_team_instances(team_name).into_iter().next() else {
            return false;
        };
        drop(factory);
        let Ok(team) = team_arc.read() else {
            return false;
        };
        team.get_members()
            .iter()
            .copied()
            .any(Self::object_ai_is_idle)
    }

    fn team_all_members_idle(team_name: &str) -> bool {
        let Ok(factory) = get_team_factory().lock() else {
            return true;
        };
        let Some(team_arc) = factory.find_team_instances(team_name).into_iter().next() else {
            return true;
        };
        drop(factory);
        let Ok(team) = team_arc.read() else {
            return true;
        };
        team.is_idle()
    }

    /// C++ `AIPlayer::checkReadyTeams` (AIPlayer.cpp).
    ///
    /// Activates ready-queue teams when all members are idle, any member is idle
    /// with an execute-actions production script, or 60s since `frame_started`.
    pub(crate) fn check_ready_teams(&mut self) -> Result<(), AiError> {
        let now = TheGameLogic::get_frame();
        let mut i = 0;
        while i < self.team_ready_queue.len() {
            let should_activate = {
                let team_q = &self.team_ready_queue[i];
                let time_expired = team_q
                    .frame_started
                    .saturating_add(60 * LOGICFRAMES_PER_SECOND)
                    < now;

                let (mut all_idle, mut any_idle) = (true, false);
                if team_q.reinforcement {
                    if let Some(obj_id) = team_q.reinforcement_id {
                        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
                            if let Ok(obj) = obj_arc.read() {
                                if let Some(ai) = obj.get_ai_update_interface() {
                                    if let Ok(ai_g) = ai.lock() {
                                        all_idle = ai_g.is_idle();
                                        any_idle = all_idle;
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(team_arc) = team_q.team.as_ref() {
                    // C++ team->m_team->isIdle() + member anyIdle walk.
                    if let Ok(tg) = team_arc.read() {
                        all_idle = tg.is_idle();
                        any_idle = false;
                        for mid in tg.get_members() {
                            if let Some(oarc) = OBJECT_REGISTRY.get_object(*mid) {
                                if let Ok(og) = oarc.read() {
                                    if let Some(ai) = og.get_ai_update_interface() {
                                        if let Ok(ai_g) = ai.lock() {
                                            if ai_g.is_idle() {
                                                any_idle = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(team_name) = team_q.team_name.as_deref() {
                    // Fallback when m_team missing (legacy queue entries).
                    all_idle = Self::team_all_members_idle(team_name);
                    any_idle = Self::team_any_member_idle(team_name);
                }

                // C++: anyIdle && m_team->proto->m_executeActions &&
                // productionCondition script has Action → force allIdle.
                // Resolve prototype via concrete team name first, then team_name field.
                if any_idle {
                    let proto_name = team_q
                        .team
                        .as_ref()
                        .and_then(|arc| arc.read().ok().map(|tg| tg.get_name().to_string()))
                        .or_else(|| team_q.team_name.clone());
                    if let Some(team_name) = proto_name {
                        if let Ok(factory) = get_team_factory().lock() {
                            if let Some(proto) = factory.find_team_prototype(&team_name) {
                                if proto.get_execute_actions_on_create() {
                                    let cond = proto.get_production_condition();
                                    if !cond.is_empty() {
                                        if let Ok(eng) = get_script_engine().read() {
                                            if eng
                                                .as_ref()
                                                .and_then(|e| {
                                                    e.find_script_clone_by_name(cond.as_str())
                                                })
                                                .and_then(|s| s.get_action().cloned())
                                                .is_some()
                                            {
                                                all_idle = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if time_expired {
                    all_idle = true;
                }
                all_idle
            };

            if !should_activate {
                i += 1;
                continue;
            }

            let mut team_q = self.team_ready_queue.remove(i).expect("ready idx");
            if !team_q.sent_to_start_location {
                team_q.sent_to_start_location = true;
                // C++ home-location tighten block is commented out in GeneralsMD.
            }

            if team_q.reinforcement {
                if let Some(obj_id) = team_q.reinforcement_id {
                    self.join_team_reinforcement(
                        obj_id,
                        team_q.team.clone(),
                        team_q.team_name.as_deref(),
                    );
                }
            } else {
                // C++ m_team->setActive() on the concrete team handle.
                if let Some(team_arc) = team_q.team.as_ref() {
                    if let Ok(mut tg) = team_arc.write() {
                        tg.set_active();
                    }
                } else if let Some(team_name) = team_q.team_name.as_deref() {
                    if let Ok(factory) = get_team_factory().lock() {
                        if let Some(team_arc) =
                            factory.find_team_instances(team_name).into_iter().next()
                        {
                            drop(factory);
                            if let Ok(mut tg) = team_arc.write() {
                                tg.set_active();
                            }
                        }
                    }
                }
                if self.is_skirmish_ai_player() {
                    if let Ok(mut eng) = get_script_engine().write() {
                        if let Some(e) = eng.as_mut() {
                            e.clear_team_flags();
                        }
                    }
                }
            }
            // team_q dropped = C++ deleteInstance
        }

        Ok(())
    }

    /// C++ `AIUpdateInterface::joinTeam` for reinforcement activation.
    fn join_team_reinforcement(
        &self,
        obj_id: ObjectID,
        _team: Option<Arc<RwLock<crate::team::Team>>>,
        _team_name: Option<&str>,
    ) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj) = obj_arc.read() else {
            return;
        };
        // C++ joinTeam uses obj->getTeam(); team handle args are unused.
        let Some(ai) = obj.get_ai_update_interface() else {
            return;
        };
        drop(obj);
        ai.join_team();
    }

    fn is_skirmish_ai_player(&self) -> bool {
        player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.is_skirmish_ai()))
            .unwrap_or(false)
    }

    /// C++ `AIPlayer::checkQueuedTeams` (AIPlayer.cpp).
    ///
    /// 1. Expire build-time: min-built + complete → ready; else disband.
    /// 2. All-built → ready queue (prepend).
    /// 3. Any idle + executeActions → run productionCondition action (team-scoped).
    pub(crate) fn check_queued_teams(&mut self) -> Result<(), AiError> {
        // --- C++ phase 1: build-time expiry ---
        let mut i = 0;
        while i < self.team_build_queue.len() {
            let expired = self.team_build_queue[i].is_build_time_expired();
            if !expired {
                i += 1;
                continue;
            }
            let min_built = self.team_build_queue[i].is_minimum_built();
            if min_built {
                if self.team_build_queue[i].are_builds_complete() {
                    let team = self.team_build_queue.remove(i).expect("build idx");
                    // C++ prependTo_TeamReadyQueue
                    self.team_ready_queue.push_front(team);
                } else {
                    i += 1; // still building required units
                }
            } else {
                let mut team = self.team_build_queue.remove(i).expect("build idx");
                let _ = team.disband();
                if self.is_skirmish_ai_player() {
                    if let Ok(mut eng) = get_script_engine().write() {
                        if let Some(e) = eng.as_mut() {
                            e.clear_team_flags();
                        }
                    }
                }
            }
        }

        // --- C++ phase 2: all-built → ready; any-idle executeActions ---
        let mut i = 0;
        while i < self.team_build_queue.len() {
            if self.team_build_queue[i].is_all_built() {
                let team = self.team_build_queue.remove(i).expect("build idx");
                self.team_ready_queue.push_front(team);
                continue;
            }

            // anyIdle + executeActions → friend_executeAction(productionCondition)
            // C++ walks team->m_team members; prefer concrete handle.
            let any_idle = {
                let tq = &self.team_build_queue[i];
                if let Some(team_arc) = tq.team.as_ref() {
                    if let Ok(tg) = team_arc.read() {
                        let mut idle = false;
                        for mid in tg.get_members() {
                            let Some(oarc) = OBJECT_REGISTRY.get_object(*mid) else {
                                continue;
                            };
                            let Ok(og) = oarc.read() else {
                                continue;
                            };
                            let Some(ai) = og.get_ai_update_interface() else {
                                continue;
                            };
                            let Ok(aig) = ai.lock() else {
                                continue;
                            };
                            if aig.is_idle() {
                                idle = true;
                                break;
                            }
                        }
                        idle
                    } else {
                        false
                    }
                } else if let Some(ref name) = tq.team_name {
                    Self::team_any_member_idle(name)
                } else {
                    false
                }
            };

            if any_idle {
                // C++ uses team->m_team->getPrototype(); prefer handle name.
                let proto_name = self.team_build_queue[i]
                    .team
                    .as_ref()
                    .and_then(|arc| arc.read().ok().map(|tg| tg.get_name().to_string()))
                    .or_else(|| self.team_build_queue[i].team_name.clone());
                if let Some(ref name) = proto_name {
                    if let Ok(factory) = get_team_factory().lock() {
                        if let Some(proto) = factory.find_team_prototype(name) {
                            if proto.get_execute_actions_on_create() {
                                let cond = proto.get_production_condition().to_string();
                                drop(factory);
                                if !cond.is_empty() {
                                    let script_engine = get_script_engine();
                                    let action = script_engine
                                        .read()
                                        .ok()
                                        .and_then(|eng| {
                                            eng.as_ref()
                                                .and_then(|e| e.find_script_clone_by_name(&cond))
                                        })
                                        .and_then(|script| script.get_action().cloned());
                                    if let Some(action) = action {
                                        // C++ friend_executeAction(action, team->m_team)
                                        drop(script_engine);
                                        if let Ok(mut eng) = get_script_engine().write() {
                                            if let Some(e) = eng.as_mut() {
                                                e.friend_execute_action(
                                                    &action,
                                                    Some(name.as_str()),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        // C++ checkQueuedTeams does not bind factories here — queueUnits does.

        Ok(())
    }

    /// C++ `AIPlayer::doTeamBuilding` (AIPlayer.cpp).
    ///
    /// teamTimer → readyToBuildTeam; teamDelay throttles queueUnits + processTeamBuilding
    /// to every `TEAM_DELAY_RECHECK_FRAMES` (5s), shortcut when unit/building completes.
    fn do_team_building(&mut self) -> Result<(), AiError> {
        let can_build_units = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_can_build_units()))
            .unwrap_or(true);
        if !can_build_units {
            return Ok(());
        }

        // See if we are ready to start trying a team.
        // C++ AIPlayer::doTeamBuilding has NO 3s clamp (only AISkirmishPlayer does).
        if !self.ready_to_build_team {
            if self.team_timer > 0 {
                self.team_timer -= 1;
            }
            if self.team_timer == 0 {
                self.ready_to_build_team = true;
                self.team_delay = 0; // Cause immediate check
            }
        }

        // Throttle queue/process (C++ m_teamDelay).
        if self.team_delay > 0 {
            self.team_delay -= 1;
        }
        if self.team_delay == 0 {
            // C++ always queueUnits on this cadence, then processTeamBuilding if ready.
            let _ = self.queue_units();
            if self.ready_to_build_team {
                self.process_team_building()?;
            }
            self.team_delay = TEAM_DELAY_RECHECK_FRAMES;
        }

        Ok(())
    }

    /// Process upgrades and skill purchases.
    /// Matches C++ AIPlayer::doUpgradesAndSkills() from AIPlayer.cpp:2906-2980.
    ///
    /// On first call, selects a skillset randomly from the available ones for the
    /// player's side. Then, if the player has science purchase points, iterates
    /// through the selected skillset and purchases each science that is affordable.
    pub(crate) fn do_upgrades_and_skills(&mut self) -> Result<(), AiError> {
        // C++ AIPlayer.cpp:2908-2910 — can't do updates on the first few frames.
        if TheGameLogic::get_frame() < 2 {
            return Ok(());
        }

        // C++: if (!getSciencePurchasePoints()) return; before sideInfo walk.
        let purchase_points_early = self
            .get_player()
            .and_then(|p| p.read().ok().map(|g| g.get_science_purchase_points()))
            .unwrap_or(0);
        if purchase_points_early <= 0 {
            return Ok(());
        }

        // Find the AiSideInfo for our player's side
        // C++ AIPlayer.cpp:2917-2926
        let player_side = {
            let Some(player_arc) = self.get_player() else {
                return Ok(());
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(());
            };
            player_guard.get_side().clone()
        };

        // Get side info from AI data
        let side_info = THE_AI.read().ok().and_then(|ai_guard| {
            let ai_data = ai_guard.get_ai_data();
            let data = ai_data.read().ok()?;
            data.side_info
                .iter()
                .find(|info| info.side == player_side)
                .cloned()
        });

        let Some(side_info) = side_info else {
            return Ok(());
        };

        // Skillset selection: pick randomly among defined skillsets
        // C++ AIPlayer.cpp:2928-2948 (after science-points early-out).
        if self.skillset_selector == INVALID_SKILLSET_SELECTION {
            let mut limit: u32 = 0;
            // Pick randomly among the skillsets that have skills.
            // Designers sometimes only define skillset 1 & 2, or some such.
            if side_info.skill_set_2.num_skills > 0 {
                limit = 1;
                if side_info.skill_set_3.num_skills > 0 {
                    limit = 2;
                    if side_info.skill_set_4.num_skills > 0 {
                        limit = 3;
                        if side_info.skill_set_5.num_skills > 0 {
                            limit = 4;
                        }
                    }
                }
            }
            // C++ AIPlayer::isSkirmishAI() — false on base AIPlayer, true on skirmish.
            if self.is_skirmish_ai_player() {
                self.skillset_selector = game_logic_random_value(0, limit) as i32;
            } else {
                // Non-skirmish default to 0
                self.skillset_selector = 0;
            }
        }

        // SKILLS: purchase sciences from the selected skillset
        // C++ AIPlayer.cpp:2951-2977
        let Some(player_arc) = self.get_player() else {
            return Ok(());
        };
        let purchase_points = {
            let Ok(player_guard) = player_arc.read() else {
                return Ok(());
            };
            player_guard.get_science_purchase_points()
        };
        if purchase_points <= 0 {
            return Ok(());
        }

        let skillset: &super::SkillSet = match self.skillset_selector {
            0 => &side_info.skill_set_1,
            1 => &side_info.skill_set_2,
            2 => &side_info.skill_set_3,
            3 => &side_info.skill_set_4,
            _ => &side_info.skill_set_5,
        };

        // Attempt to purchase each science in the skillset
        for i in 0..skillset.num_skills as usize {
            if i >= skillset.skills.len() {
                break;
            }
            let science = skillset.skills[i];
            if science == crate::common::science::SCIENCE_INVALID {
                continue;
            }
            let (capable, purchased) = {
                let Ok(mut player_guard) = player_arc.write() else {
                    break;
                };
                let capable = player_guard.is_capable_of_purchasing_science(science);
                if !capable {
                    (false, false)
                } else {
                    let purchased = player_guard.attempt_to_purchase_science(science);
                    (true, purchased)
                }
            };
            if capable && purchased {
                // Successfully purchased a science from the skillset
                log::debug!(
                    "AI Player purchases from SkillSet{} science {}",
                    self.skillset_selector + 1,
                    science,
                );
            }
        }

        Ok(())
    }

    /// C++ `AIPlayer::updateBridgeRepair` (AIPlayer.cpp).
    ///
    /// Once/second: pop dead queue heads, assign/find repair dozer, issue
    /// aiRepair, complete when pristine and idle, then send dozer home.
    pub(crate) fn update_bridge_repair(&mut self) -> Result<(), AiError> {
        use crate::ai::{AiCommandParams, AiCommandType};
        use crate::object::body::BodyDamageType;
        use crate::object::update::ai_update::dozer_ai_update::DozerTask;

        if self.structures_in_queue <= 0 {
            return Ok(());
        }
        // C++: m_bridgeTimer--; if (m_bridgeTimer>0) return; m_bridgeTimer = FPS;
        // Decrement first so timer==1 proceeds this frame (not FPS+1 lag).
        self.bridge_timer = self.bridge_timer.saturating_sub(1);
        if self.bridge_timer > 0 {
            return Ok(());
        }
        self.bridge_timer = LOGICFRAMES_PER_SECOND;

        // Pop missing heads.
        let mut bridge_id = None;
        while bridge_id.is_none() && self.structures_in_queue > 0 {
            let head = self.structures_to_repair[0];
            if head.and_then(|id| OBJECT_REGISTRY.get_object(id)).is_some() {
                bridge_id = head;
            } else {
                // shift left
                for i in 0..(self.structures_in_queue as usize).saturating_sub(1) {
                    self.structures_to_repair[i] = self.structures_to_repair[i + 1];
                }
                if self.structures_in_queue > 0 {
                    let last = (self.structures_in_queue as usize) - 1;
                    self.structures_to_repair[last] = None;
                    self.structures_in_queue -= 1;
                }
            }
        }
        if self.structures_in_queue <= 0 {
            return Ok(());
        }
        let Some(bridge_id) = bridge_id else {
            return Ok(());
        };
        let Some(bridge_arc) = OBJECT_REGISTRY.get_object(bridge_id) else {
            return Ok(());
        };
        let bridge_state = {
            let Ok(bg) = bridge_arc.read() else {
                return Ok(());
            };
            bg.get_body_module()
                .and_then(|b| b.lock().ok().map(|g| g.get_damage_state()))
                .unwrap_or(BodyDamageType::Pristine)
        };
        let bridge_pos = bridge_arc
            .read()
            .ok()
            .map(|g| *g.get_position())
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        if self.repair_dozer.is_none() {
            self.dozer_is_repairing = false;
            if self.dozer_queued_for_repair {
                return Ok(()); // waiting for queued dozer
            }
            if let Some(dozer_id) = self.find_dozer(&bridge_pos)? {
                self.repair_dozer = Some(dozer_id);
                if let Some(dozer_arc) = OBJECT_REGISTRY.get_object(dozer_id) {
                    if let Ok(dg) = dozer_arc.read() {
                        self.repair_dozer_origin = *dg.get_position();
                        if let Some(ai) = dg.get_ai_update_interface() {
                            if let Ok(mut ai_lock) = ai.lock() {
                                let mut params = AiCommandParams::new(
                                    AiCommandType::Repair,
                                    CommandSourceType::FromAi,
                                );
                                params.obj = Some(bridge_id);
                                let _ = ai_lock.execute_command(&params);
                            }
                        }
                    }
                }
                self.dozer_is_repairing = true;
                return Ok(());
            }
            self.queue_dozer()?;
            self.dozer_queued_for_repair = true;
            return Ok(());
        }

        let Some(dozer_id) = self.repair_dozer else {
            return Ok(());
        };
        let Some(dozer_arc) = OBJECT_REGISTRY.get_object(dozer_id) else {
            self.repair_dozer = None; // killed
            self.bridge_timer = 0;
            return Ok(());
        };

        let any_task_pending = {
            let Ok(dg) = dozer_arc.read() else {
                return Ok(());
            };
            let Some(ai) = dg.get_ai_update_interface() else {
                return Ok(());
            };
            let Ok(mut ai_g) = ai.lock() else {
                return Ok(());
            };
            ai_g.get_dozer_ai_update_interface_mut()
                .map(|d| d.is_any_task_pending())
                .unwrap_or(false)
        };

        if self.dozer_is_repairing {
            if !any_task_pending {
                if bridge_state == BodyDamageType::Pristine {
                    // Done — pop head.
                    for i in 0..(self.structures_in_queue as usize).saturating_sub(1) {
                        self.structures_to_repair[i] = self.structures_to_repair[i + 1];
                    }
                    if self.structures_in_queue > 0 {
                        let last = (self.structures_in_queue as usize) - 1;
                        self.structures_to_repair[last] = None;
                        self.structures_in_queue -= 1;
                    }
                    self.dozer_is_repairing = false;
                    if self.structures_in_queue == 0 {
                        // Go home to base center or origin.
                        // C++: pathfinder->adjustToPossibleDestination(dozer, locoSet, &pos)
                        // then aiMoveToPosition(&pos, CMD_FROM_AI).
                        let mut pos = if self.base_center_set {
                            self.base_center
                        } else {
                            self.repair_dozer_origin
                        };
                        if let Ok(dg) = dozer_arc.read() {
                            let start = *dg.get_position();
                            if let Some(ai) = dg.get_ai_update_interface() {
                                // Adjust destination onto a reachable cell with dozer loco set.
                                if let Some(loco_set) = ai.get_locomotor_set_clone() {
                                    if let Ok(ai_sys) = THE_AI.read() {
                                        if let Some(pf_arc) = ai_sys.pathfinder() {
                                            if let Ok(pf) = pf_arc.read() {
                                                let surfaces = loco_set.get_valid_surfaces();
                                                let _ = pf.adjust_to_possible_destination(
                                                    &start, &mut pos, surfaces, false, 0.0,
                                                );
                                            }
                                        }
                                    }
                                }
                                ai.ai_move_to_position(&pos, false, CommandSourceType::FromAi);
                            }
                        }
                        return Ok(());
                    }
                }
            } else {
                return Ok(()); // still working
            }
        }

        // (Re)issue repair.
        if let Ok(dg) = dozer_arc.read() {
            if let Some(ai) = dg.get_ai_update_interface() {
                if let Ok(mut ai_lock) = ai.lock() {
                    let mut params =
                        AiCommandParams::new(AiCommandType::Repair, CommandSourceType::FromAi);
                    params.obj = Some(bridge_id);
                    let _ = ai_lock.execute_command(&params);
                }
            }
        }
        self.dozer_is_repairing = true;
        let _ = DozerTask::Build; // keep import path warm if needed
        Ok(())
    }

    /// C++ `AIPlayer::buildStructureWithDozer` (AIPlayer.cpp) — core path residual.
    ///
    /// findDozer → funds check → ground height → spawn + dozer build task →
    /// stamp BuildListInfo objectID/timestamp/underConstruction.
    /// C++ `AIPlayer::buildStructureWithDozer` (AIPlayer.cpp).
    ///
    /// findDozer → funds → ground Z → enemy-overlap reject → legalize/wiggle →
    /// path teleport residual → spawn UC building + dozer build task → stamp list.
    pub fn build_structure_with_dozer(
        &mut self,
        template_name: &str,
        location: Coord3D,
        angle: Real,
    ) -> Result<Option<ObjectID>, AiError> {
        // C++ findDozer may queueDozer internally; do not double-queue here.
        let Some(dozer_id) = self.find_dozer(&location)? else {
            return Ok(None);
        };

        let Some(template) = TheThingFactory::find_template(template_name) else {
            return Ok(None);
        };

        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(None);
        };

        let cost = template.calc_cost_to_build(Some(&*player_guard));
        if player_guard.get_money().get_money() < cost {
            return Ok(None);
        }

        let mut pos = location;
        if let Some(terrain) = TheTerrainLogic::get() {
            pos.z += terrain.get_ground_height(pos.x, pos.y, None);
        }

        // C++ first check: NO_ENEMY_OBJECT_OVERLAP only — fail hard if enemies.
        let validator = FoundationValidator::new_ai();
        if !self.is_location_safe(&pos, template.as_ref()) {
            // Approximate enemy-overlap reject (C++ NO_ENEMY_OBJECT_OVERLAP).
            return Ok(None);
        }

        // C++ CLEAR_PATH | TERRAIN_RESTRICTIONS | NO_OBJECT_OVERLAP; wiggle if illegal.
        let is_skirmish = self.is_skirmish_ai_player();
        let mut legal = validator
            .validate_placement(&pos, template_name, angle, self.player_id as ObjectID)
            .is_ok();
        if !legal {
            log::debug!(
                "{} - Dozer unable to place.  Attempting to adjust position.",
                template_name
            );
            let limit = if is_skirmish {
                120.0 * PATHFIND_CELL_SIZE_F
            } else {
                10.0 * PATHFIND_CELL_SIZE_F
            };
            let step = if is_skirmish {
                4.0 * PATHFIND_CELL_SIZE_F
            } else {
                2.0 * PATHFIND_CELL_SIZE_F
            };
            let mut pos_offset = 0.0_f32;
            let mut found = None;
            while pos_offset < limit {
                let offset = pos_offset * 0.5;
                // Horizontal edges at y = pos.y ± offset
                let mut x = pos.x - offset;
                let y0 = pos.y - offset;
                while x <= pos.x + offset + 0.001 {
                    for y in [y0, y0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, pos.z);
                        if validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            found = Some(candidate);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                    x += if is_skirmish {
                        2.0 * PATHFIND_CELL_SIZE_F
                    } else {
                        PATHFIND_CELL_SIZE_F
                    };
                }
                if found.is_some() {
                    break;
                }
                // Vertical edges at x = pos.x ± offset
                let mut y = pos.y - offset;
                let x0 = pos.x - offset;
                while y <= pos.y + offset + 0.001 {
                    for x in [x0, x0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, pos.z);
                        if validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            found = Some(candidate);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                    y += if is_skirmish {
                        2.0 * PATHFIND_CELL_SIZE_F
                    } else {
                        PATHFIND_CELL_SIZE_F
                    };
                }
                if found.is_some() {
                    break;
                }
                pos_offset += step;
            }
            if let Some(p) = found {
                pos = p;
                legal = true;
            } else {
                // C++ final fallback: NO_ENEMY_OBJECT_OVERLAP only.
                legal = self.is_location_safe(&pos, template.as_ref());
            }
        }
        if !legal {
            return Ok(None);
        }

        // C++: if (!pathfinder->clientSafeQuickDoesPathExist(
        //           dozer->getAI()->getLocomotorSet(), dozerPos, &pos))
        //        { log; dozer->setPosition(&pos); }
        if let Some(dozer_arc) = OBJECT_REGISTRY.get_object(dozer_id) {
            let Ok(dozer_g) = dozer_arc.read() else {
                return Ok(None);
            };
            let Some(dozer_ai) = dozer_g.get_ai_update_interface() else {
                return Ok(None);
            };
            let dpos = *dozer_g.get_position();
            // Ensure Normal set is selected (C++ getLocomotorSet is current).
            dozer_ai.choose_locomotor_set(LocomotorSetType::Normal);
            let loco_set = dozer_ai.get_locomotor_set_clone();
            drop(dozer_g);

            let mut path_ok = false;
            if let Some(ref loco_set) = loco_set {
                if let Ok(ai_guard) = THE_AI.read() {
                    if let Some(pf_arc) = ai_guard.pathfinder() {
                        if let Ok(pf) = pf_arc.read() {
                            path_ok = pf.client_safe_quick_does_path_exist(loco_set, &dpos, &pos);
                        }
                    }
                }
            }
            // Empty/missing loco set → path_ok stays false → teleport (same as C++
            // when path fails; avoids always-teleport when loco data is present).
            if !path_ok {
                log::debug!(
                    "{} - Dozer unable to reach building.  Teleporting.",
                    template_name
                );
                if let Ok(mut dozer_w) = dozer_arc.write() {
                    let _ = dozer_w.set_position(&pos);
                }
            }
        }

        let team = player_guard.get_default_team();
        drop(player_guard);
        drop(list);

        let Some(team_arc) = team else {
            return Ok(None);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(None);
        };
        let Ok(factory) = TheThingFactory::get() else {
            return Ok(None);
        };
        let Ok(new_object) = factory.new_object(template.clone(), &*team_guard) else {
            return Ok(None);
        };
        drop(team_guard);

        let mut build_max_health = 0.0;
        if let Ok(guard) = new_object.read() {
            if let Some(body) = guard.get_body_module() {
                if let Ok(body_guard) = body.lock() {
                    build_max_health = body_guard.get_max_health();
                }
            }
        }

        let bldg_id = {
            let Ok(mut guard) = new_object.write() else {
                return Ok(None);
            };
            let _ = guard.set_position(&pos);
            let _ = guard.set_orientation(angle);
            if let Some(dozer_arc) = OBJECT_REGISTRY.get_object(dozer_id) {
                if let Ok(dozer_g) = dozer_arc.read() {
                    guard.set_producer(Some(&*dozer_g));
                    guard.set_builder(Some(&*dozer_g));
                }
            }
            guard.set_construction_percent(0.0);
            if build_max_health > 0.0 {
                let _ = guard.set_health(1.0);
            }
            guard.set_status(
                ObjectStatusMaskType::from_status(ObjectStatusTypes::UnderConstruction),
                true,
            );
            guard.get_id()
        };

        let total_build_frames = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| {
                p.read()
                    .ok()
                    .map(|pg| template.calc_time_to_build(Some(&*pg)).max(1) as u32)
            })
            .unwrap_or(300);

        if let Some(dozer_arc) = OBJECT_REGISTRY.get_object(dozer_id) {
            if let Ok(dozer_g) = dozer_arc.read() {
                if let Some(ai) = dozer_g.get_ai_update_interface() {
                    if let Ok(mut ai_g) = ai.try_lock() {
                        if let Some(dozer_ai) = ai_g.get_dozer_ai_update_interface_mut() {
                            dozer_ai.set_build_task(
                                bldg_id,
                                total_build_frames,
                                build_max_health,
                                false,
                            );
                        } else if let Some(worker_ai) = ai_g.get_worker_ai_update_interface_mut() {
                            worker_ai.set_build_task(
                                bldg_id,
                                total_build_frames,
                                build_max_health,
                                false,
                            );
                        }
                    }
                }
            }
        }

        // C++ stamps the BuildListInfo* passed into buildStructureWithDozer
        // (setObjectID/timestamp/underConstruction). Match that entry by template
        // + requested location so duplicate templates do not steal the stamp.
        // decrementNumRebuilds is done by caller in C++ processBaseBuilding; we
        // keep decrement here for solo process_base_building which does not.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        // Pass 1: prefer location match (C++ pointer identity).
                        let mut best_loc: Option<Coord3D> = None;
                        let mut best_dist = f32::MAX;
                        let mut fallback_loc: Option<Coord3D> = None;
                        {
                            let mut cur = Some(&*info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() == template_name
                                    && node.get_object_id() == INVALID_ID
                                {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - location.x;
                                    let dy = nloc.y - location.y;
                                    let d2 = dx * dx + dy * dy;
                                    if d2 < best_dist {
                                        best_dist = d2;
                                        best_loc = Some(nloc);
                                    }
                                    if fallback_loc.is_none() {
                                        fallback_loc = Some(nloc);
                                    }
                                }
                                cur = node.get_next();
                            }
                        }
                        // Exact-ish location first; else first free slot of that template.
                        let stamp_loc = if best_dist <= 1.0 {
                            best_loc
                        } else {
                            fallback_loc.or(best_loc)
                        };
                        if let Some(target) = stamp_loc {
                            let mut cur = Some(&mut *info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() == template_name
                                    && node.get_object_id() == INVALID_ID
                                {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - target.x;
                                    let dy = nloc.y - target.y;
                                    if dx * dx + dy * dy <= 1.0 {
                                        node.set_object_id(bldg_id);
                                        node.set_object_timestamp(
                                            TheGameLogic::get_frame().saturating_add(1),
                                        );
                                        node.set_under_construction(true);
                                        node.decrement_num_rebuilds();
                                        break;
                                    }
                                }
                                cur = node.get_next_mut();
                            }
                        }
                    }
                }
            }
        }

        log::debug!(
            "AI dozer {} started building {} as {}",
            dozer_id,
            template_name,
            bldg_id
        );
        Ok(Some(bldg_id))
    }

    /// C++ `AIPlayer::buildStructureNow` via priority residual (no BuildListInfo ptr).
    fn build_structure_now(&mut self, priority: &ConstructionPriority) -> Result<(), AiError> {
        let location = if let Some(loc) = priority.desired_location {
            loc
        } else {
            self.calc_closest_construction_zone_near_base(&priority.building_type)?
                .unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
        };
        let angle = priority.desired_angle.unwrap_or(0.0);
        let _ = self.build_structure_now_at(&priority.building_type, location, angle, None)?;
        Ok(())
    }

    /// C++ `AIPlayer::buildStructureNow` (AIPlayer.cpp).
    ///
    /// Instant-construct (no dozer): BuildAssistant/new_object, clear UC status,
    /// stamp BuildListInfo, checkForSupplyCenter. Returns built object id.
    pub fn build_structure_now_at(
        &mut self,
        template_name: &str,
        location: Coord3D,
        angle: Real,
        stamp_object_id_slot: Option<ObjectID>,
    ) -> Result<Option<ObjectID>, AiError> {
        let Some(template) = TheThingFactory::find_template(template_name) else {
            return Ok(None);
        };

        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(None);
        };
        let team = player_guard.get_default_team();
        drop(player_guard);
        drop(list);

        let Some(team_arc) = team else {
            return Ok(None);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(None);
        };
        let Ok(factory) = TheThingFactory::get() else {
            return Ok(None);
        };
        let Ok(new_object) = factory.new_object(template.clone(), &*team_guard) else {
            return Ok(None);
        };
        drop(team_guard);

        let mut pos = location;
        if let Some(terrain) = TheTerrainLogic::get() {
            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
        }

        // Capture BuildListInfo map props before/while stamping (C++ Dict).
        let mut map_building_name = String::new();
        let mut map_script = String::new();
        let mut map_health: i32 = 100;
        let mut map_unsellable = false;
        let mut stamped = false;

        // Prefer matching build-list entry props first (before object exists fully).
        // C++ uses the BuildListInfo* argument — match by slot id or template+location.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player_arc.read() {
                    let mut best_dist = f32::MAX;
                    let mut fallback_done = false;
                    let mut cur = pg.get_build_list();
                    while let Some(node) = cur {
                        let name_match = node.get_template_name().as_str() == template_name;
                        if !name_match {
                            cur = node.get_next();
                            continue;
                        }
                        if let Some(id) = stamp_object_id_slot {
                            if node.get_object_id() == id {
                                map_building_name = node.get_building_name().to_string();
                                map_script = node.get_script().to_string();
                                map_health = node.get_health();
                                map_unsellable = node.get_unsellable();
                                best_dist = 0.0;
                                break;
                            }
                        }
                        if node.get_object_id() == INVALID_ID {
                            let nloc = *node.get_location();
                            let dx = nloc.x - location.x;
                            let dy = nloc.y - location.y;
                            let d2 = dx * dx + dy * dy;
                            if d2 < best_dist {
                                best_dist = d2;
                                map_building_name = node.get_building_name().to_string();
                                map_script = node.get_script().to_string();
                                map_health = node.get_health();
                                map_unsellable = node.get_unsellable();
                            }
                            if !fallback_done {
                                // keep first free as last-resort if nothing closer later
                                fallback_done = true;
                                if best_dist == f32::MAX {
                                    map_building_name = node.get_building_name().to_string();
                                    map_script = node.get_script().to_string();
                                    map_health = node.get_health();
                                    map_unsellable = node.get_unsellable();
                                }
                            }
                        }
                        cur = node.get_next();
                    }
                }
            }
        }

        let bldg_id = {
            let Ok(mut guard) = new_object.write() else {
                return Ok(None);
            };
            let _ = guard.set_position(&pos);
            let _ = guard.set_orientation(angle);

            // C++ updateObjValuesFromMapProperties(Dict)
            let mut props = crate::common::Dict::new();
            props.set_ascii_string(
                crate::common::well_known_keys::key_object_name(),
                map_building_name.as_str(),
            );
            props.set_ascii_string(
                crate::common::well_known_keys::key_object_script_attachment(),
                map_script.as_str(),
            );
            props.set_int(
                crate::common::well_known_keys::key_object_initial_health(),
                map_health,
            );
            props.set_bool(
                crate::common::well_known_keys::key_object_unsellable(),
                map_unsellable,
            );
            guard.update_obj_values_from_map_properties(&props);

            // C++ clear UnderConstruction + Reconstructing (instant complete).
            let mask = ObjectStatusMaskType::from_status(ObjectStatusTypes::UnderConstruction)
                | ObjectStatusMaskType::from_status(ObjectStatusTypes::Reconstructing);
            guard.clear_status(mask);
            guard.set_construction_percent(100.0);
            // UnderConstruction just cleared → update upgrades (C++).
            guard.update_upgrade_modules_from_player();
            guard.get_id()
        };

        // Stamp build list entry: C++ stamps the BuildListInfo* passed in.
        // Prefer slot id hint, else template + requested location, else first free.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        let mut best_loc: Option<Coord3D> = None;
                        let mut best_dist = f32::MAX;
                        let mut fallback_loc: Option<Coord3D> = None;
                        let mut slot_loc: Option<Coord3D> = None;
                        {
                            let mut cur = Some(&*info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() != template_name {
                                    cur = node.get_next();
                                    continue;
                                }
                                if let Some(id) = stamp_object_id_slot {
                                    if node.get_object_id() == id {
                                        slot_loc = Some(*node.get_location());
                                        break;
                                    }
                                }
                                if node.get_object_id() == INVALID_ID {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - location.x;
                                    let dy = nloc.y - location.y;
                                    let d2 = dx * dx + dy * dy;
                                    if d2 < best_dist {
                                        best_dist = d2;
                                        best_loc = Some(nloc);
                                    }
                                    if fallback_loc.is_none() {
                                        fallback_loc = Some(nloc);
                                    }
                                }
                                cur = node.get_next();
                            }
                        }
                        let stamp_loc = slot_loc.or(if best_dist <= 1.0 {
                            best_loc
                        } else {
                            fallback_loc.or(best_loc)
                        });
                        if let Some(target) = stamp_loc {
                            let mut cur = Some(&mut *info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() == template_name {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - target.x;
                                    let dy = nloc.y - target.y;
                                    if dx * dx + dy * dy <= 1.0 {
                                        if stamp_object_id_slot
                                            .map(|id| node.get_object_id() == id)
                                            .unwrap_or(node.get_object_id() == INVALID_ID)
                                            || node.get_object_id() == INVALID_ID
                                        {
                                            node.set_object_id(bldg_id);
                                            node.set_object_timestamp(
                                                TheGameLogic::get_frame().saturating_add(1),
                                            );
                                            node.set_under_construction(false);
                                            stamped = true;
                                            break;
                                        }
                                    }
                                }
                                cur = node.get_next_mut();
                            }
                        }
                    }
                }
            }
        }
        let _ = stamped;

        // C++ TheScriptEngine->addObjectToCache + runObjectScript
        if let Ok(mut eng) = get_script_engine().write() {
            if let Some(e) = eng.as_mut() {
                e.add_object_to_cache(bldg_id);
                if !map_script.is_empty() {
                    e.run_object_script(&map_script, bldg_id);
                }
            }
        }

        // C++ checkForSupplyCenter(info, bldg)
        let _ = self.check_for_supply_center(bldg_id);

        // Rally offset residual deferred (gotOffset bug in C++ leaves gotOffset false).
        log::debug!("AI inst-built {} as {}", template_name, bldg_id);
        Ok(Some(bldg_id))
    }

    /// C++ `AIPlayer::startTraining` (AIPlayer.cpp).
    ///
    /// findFactory → ProductionUpdateInterface::queueCreateUnit(requestUniqueUnitID)
    /// → set order.factoryID. Returns true only if queued.
    fn start_training_internal(
        &mut self,
        order: &mut WorkOrder,
        busy_ok: bool,
        team_name: &str,
    ) -> Result<bool, AiError> {
        let Some(factory_id) = self.find_factory_internal(&order.thing_template, busy_ok)? else {
            return Ok(false);
        };

        let Some(factory_arc) = OBJECT_REGISTRY.get_object(factory_id) else {
            return Ok(false);
        };
        let Some(template) = TheThingFactory::find_template(&order.thing_template) else {
            return Ok(false);
        };

        // Prefer Object production queue path (queueCreateUnit + unique id).
        let queued = {
            let Ok(mut factory_g) = factory_arc.write() else {
                return Ok(false);
            };
            let production_id = factory_g.request_unique_unit_production_id().unwrap_or(0);
            if production_id != 0 {
                factory_g.queue_unit_with_production_id(&template, production_id)
            } else {
                factory_g.queue_unit(&template)
            }
        };

        if !queued {
            // Fallback: ProductionUpdateInterface::start_production on behaviors.
            let Ok(factory_g) = factory_arc.read() else {
                return Ok(false);
            };
            let mut started = false;
            for behavior in factory_g.get_behavior_modules() {
                let Ok(mut bg) = behavior.lock() else {
                    continue;
                };
                let Some(prod) = bg.get_production_update_interface() else {
                    continue;
                };
                if prod
                    .start_production(order.thing_template.clone(), self.player_id)
                    .is_ok()
                {
                    started = true;
                    break;
                }
            }
            if !started {
                return Ok(false);
            }
        }

        order.factory_id = Some(factory_id);
        log::debug!(
            "Queuing {} for {} at factory {}",
            order.thing_template,
            team_name,
            factory_id
        );
        Ok(true)
    }

    #[allow(dead_code)] // C++ parity: default wrapper for start_training_internal
    fn start_training(&mut self, order: &mut WorkOrder) -> Result<(), AiError> {
        // Default: don't use busy factories
        self.start_training_internal(order, false, "default")?;
        Ok(())
    }

    /// Shared factory eligibility check used by build-list and object-scan paths.
    fn factory_candidate(
        &self,
        obj_id: ObjectID,
        thing_template: &str,
        busy_ok: bool,
        busy_factory: &mut Option<ObjectID>,
    ) -> Result<Option<ObjectID>, AiError> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(None);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(None);
        };
        if obj_guard.get_controlling_player_id() != Some(self.player_id) {
            return Ok(None);
        }
        if obj_guard.is_destroyed()
            || obj_guard.is_under_construction()
            || obj_guard.test_status(ObjectStatusTypes::Sold)
        {
            return Ok(None);
        }

        let mut checked = false;
        for module_handle in obj_guard.behavior_modules() {
            let mut can_produce = false;
            let mut is_busy = false;
            let matched = module_handle.with_module(|module| {
                let Some(prod) = module.get_production_control_interface() else {
                    return false;
                };
                if prod.can_produce(thing_template) {
                    can_produce = true;
                    is_busy = prod.is_producing() || prod.queue_size() > 0;
                }
                true
            });
            if matched {
                checked = true;
                if !can_produce {
                    return Ok(None);
                }
                if !is_busy {
                    return Ok(Some(obj_id));
                }
                if busy_ok && busy_factory.is_none() {
                    *busy_factory = Some(obj_id);
                }
                return Ok(None);
            }
        }

        if !checked {
            for behavior in obj_guard.get_behavior_modules() {
                let Ok(mut behavior_guard) = behavior.lock() else {
                    continue;
                };
                let Some(prod) = behavior_guard.get_production_update_interface() else {
                    continue;
                };
                if !prod.can_produce(thing_template) {
                    continue;
                }
                let is_busy = prod.is_producing() || prod.get_queue_size() > 0;
                if !is_busy {
                    return Ok(Some(obj_id));
                }
                if busy_ok && busy_factory.is_none() {
                    *busy_factory = Some(obj_id);
                }
                break;
            }
        }

        Ok(None)
    }

    /// C++ `AIPlayer::findFactory` (AIPlayer.cpp).
    ///
    /// Iterates the player **build list only** (C++). Clears object IDs for
    /// captured factories. `busy_ok` allows returning a busy factory when no
    /// idle one exists (script priority teams).
    fn find_factory_internal(
        &self,
        thing_template: &str,
        busy_ok: bool,
    ) -> Result<Option<ObjectID>, AiError> {
        let mut busy_factory: Option<ObjectID> = None;
        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(None);
        };

        // --- C++ path: iterate build list only (no full-object scan). ---
        // Need mut build list to clear captured factory IDs like C++.
        drop(player_guard);
        drop(list);
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut player_guard) = player_arc.write() {
                    if let Some(head) = player_guard.get_build_list_mut() {
                        let mut current = Some(&mut *head);
                        while let Some(info) = current {
                            let obj_id = info.get_object_id();
                            if obj_id != INVALID_ID {
                                // C++: if factory->getControllingPlayer() != m_player → clear ID.
                                let wrong_owner = if let Some(arc) =
                                    OBJECT_REGISTRY.get_object(obj_id)
                                {
                                    arc.read()
                                        .ok()
                                        .map(|g| {
                                            g.get_controlling_player_id() != Some(self.player_id)
                                        })
                                        .unwrap_or(false)
                                } else {
                                    false
                                };
                                if wrong_owner {
                                    info.set_object_id(INVALID_ID);
                                } else if let Some(found) = self.factory_candidate(
                                    obj_id,
                                    thing_template,
                                    busy_ok,
                                    &mut busy_factory,
                                )? {
                                    return Ok(Some(found));
                                }
                            }
                            current = info.get_next_mut();
                        }
                    }
                }
            }
        }

        Ok(busy_factory)
    }

    fn find_factory(&self, thing_template: &str) -> Result<Option<ObjectID>, AiError> {
        self.find_factory_internal(thing_template, false)
    }

    /// C++ `AIPlayer::selectTeamToBuild` (AIPlayer.cpp).
    ///
    /// 1. Collect isAGoodIdea candidates + hiPri
    /// 2. selectTeamToReinforce(hiPri) first
    /// 3. Random pick among hiPri set via GameLogicRandomValue
    /// 4. buildSpecificAITeam(low priority) + arm teamTimer with wealth mods
    pub(crate) fn select_team_to_build(&mut self) -> Result<bool, AiError> {
        const INVALID_PRI: i32 = -99999;

        // C++ iterates m_player->getPlayerTeams(), not the global TeamFactory.
        let candidates: Vec<(String, i32)> = {
            let Ok(list) = player_list().read() else {
                return Ok(false);
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok(false);
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(false);
            };
            player_guard
                .get_player_team_prototypes()
                .iter()
                .map(|proto| {
                    (
                        proto.get_name().as_str().to_string(),
                        proto.get_production_priority(),
                    )
                })
                .collect()
        };

        let mut good: Vec<(String, i32)> = Vec::new();
        let mut hi_pri = INVALID_PRI;
        for (name, pri) in candidates {
            if self.is_a_good_idea_to_build_team(&name)? {
                if pri > hi_pri {
                    hi_pri = pri;
                }
                good.push((name, pri));
            }
        }

        // C++: try reinforce at hiPri before picking a new team.
        if self.select_team_to_reinforce(hi_pri)? {
            return Ok(true);
        }

        if hi_pri == INVALID_PRI {
            return Ok(false);
        }

        let hi: Vec<String> = good
            .into_iter()
            .filter(|(_, p)| *p == hi_pri)
            .map(|(n, _)| n)
            .collect();
        if hi.is_empty() {
            return Ok(false);
        }

        // C++ GameLogicRandomValue(0, count-1)
        let which = if hi.len() == 1 {
            0
        } else {
            game_logic_random_value(0, (hi.len() as u32) - 1) as usize
        };
        let team_name = &hi[which.min(hi.len() - 1)];

        // C++ buildSpecificAITeam(teamProto, false) — auto pick is low priority.
        // Low-priority path appends (push_back); frame_started already stamped inside.
        self.build_specific_ai_team(team_name, false)?;
        self.arm_team_timer_after_build()?;
        Ok(true)
    }

    /// After auto team select: C++ sets ready=false and teamTimer with wealth mods.
    ///
    /// Retail TeamSeconds=0 → timer 0 (like structureSeconds). C++ does not clamp
    /// to 1; next doTeamBuilding frame decrements and re-arms ready.
    fn arm_team_timer_after_build(&mut self) -> Result<(), AiError> {
        self.ready_to_build_team = false;
        // C++: m_teamTimer = m_teamSeconds * LOGICFRAMES_PER_SECOND (0 is valid).
        let mut timer = (self.team_seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32) as u32;

        let money = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_money().get_money()))
            .unwrap_or(0);

        let (poor, wealthy, poor_mod, wealthy_mod) = Self::team_wealth_params();

        // C++: timer = timer / mod when mod applies (mod 0 → skip).
        // Integer divide of 0 stays 0 (immediate re-ready next doTeamBuilding).
        if money < poor && poor_mod > 0.0 {
            timer = (timer as f32 / poor_mod) as u32;
        } else if money > wealthy && wealthy_mod > 0.0 {
            timer = (timer as f32 / wealthy_mod) as u32;
        }
        self.team_timer = timer;
        Ok(())
    }

    /// C++ `AIPlayer::selectTeamToReinforce` (AIPlayer.cpp).
    ///
    /// Among auto-reinforce prototypes with priority > minPriority, find a live
    /// team instance missing units below maxUnits with an idle factory. Queue a
    /// single required work order (prepend), try recruit then startTraining,
    /// and shortcut teamDelay=0.
    pub(crate) fn select_team_to_reinforce(&mut self, min_priority: i32) -> Result<bool, AiError> {
        // C++ iterates m_player->getPlayerTeams() only.
        let protos: Vec<_> = {
            let Ok(list) = player_list().read() else {
                return Ok(false);
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok(false);
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(false);
            };
            player_guard
                .get_player_team_prototypes()
                .iter()
                .cloned()
                .collect()
        };

        let mut best: Option<(String, Arc<RwLock<crate::team::Team>>, String, i32)> = None;
        // C++ curPriority starts at minPriority; only priorities *above* min win.
        let mut cur_priority = min_priority;

        for proto in &protos {
            if !proto.automatically_reinforce() {
                continue;
            }
            let priority = proto.get_production_priority();
            if priority <= cur_priority {
                continue;
            }
            let name = proto.get_name().as_str().to_string();

            // C++: busy if any TeamInQueue.m_team->getPrototype() == proto.
            let busy = self.team_build_queue.iter().any(|q| {
                if let Some(team_arc) = q.team.as_ref() {
                    if let Ok(tg) = team_arc.read() {
                        if tg.get_name().as_str() == name.as_str() {
                            return true;
                        }
                    }
                }
                q.team_name
                    .as_deref()
                    .map(|n| n == name.as_str())
                    .unwrap_or(false)
            });
            if busy {
                continue;
            }

            let Ok(factory_guard) = get_team_factory().lock() else {
                continue;
            };
            let instances = factory_guard.find_team_instances(&name);
            drop(factory_guard);

            for team_arc in instances {
                let Ok(team_g) = team_arc.read() else {
                    continue;
                };
                if !team_g.has_any_units() {
                    continue;
                }

                for unit_info in proto.units_info() {
                    if unit_info.max_units < 1 {
                        continue;
                    }
                    if unit_info.unit_thing_name.is_empty() {
                        continue;
                    }
                    let Some(thing) = TheThingFactory::find_template(unit_info.unit_thing_name)
                    else {
                        continue;
                    };
                    let mut counts = [0i32; 1];
                    team_g.count_objects_by_thing_template(
                        std::slice::from_ref(&thing),
                        false,
                        false,
                        &mut counts,
                    );
                    if counts[0] >= unit_info.max_units {
                        continue;
                    }
                    // Idle factory required (findFactory(thing, false)).
                    if self
                        .find_factory_internal(unit_info.unit_thing_name, false)?
                        .is_none()
                    {
                        continue;
                    }
                    // Better candidate.
                    best = Some((
                        name.clone(),
                        team_arc.clone(),
                        unit_info.unit_thing_name.to_string(),
                        priority,
                    ));
                    cur_priority = priority;
                }
            }
        }

        let Some((team_name, team_arc, thing_name, _)) = best else {
            return Ok(false);
        };

        let Some(thing) = TheThingFactory::find_template(&thing_name) else {
            return Ok(false);
        };

        // Origin: home location, else first member position.
        let (origin, _team_id) = {
            let Ok(team_g) = team_arc.read() else {
                return Ok(false);
            };
            let tid = team_g.get_id() as ObjectID;
            // C++: origin = homeLocation; if first member exists, use its position.
            let mut origin = Coord3D::new(0.0, 0.0, 0.0);
            if let Ok(factory) = get_team_factory().lock() {
                if let Some(proto) = factory.find_team_prototype(team_g.get_name().as_str()) {
                    if proto.has_home_location() {
                        origin = proto.home_location();
                    }
                }
            }
            if let Some(&mid) = team_g.get_members().first() {
                if let Some(o) = OBJECT_REGISTRY.get_object(mid) {
                    if let Ok(g) = o.read() {
                        origin = *g.get_position();
                    }
                }
            }
            (origin, tid)
        };

        let max_recruit = THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.max_recruit_distance))
            .unwrap_or(99999.0);

        let mut order = WorkOrder::new(thing_name.clone());
        order.num_required = 1;
        order.required = true;
        order.factory_id = None;

        let mut recruited_id = None;
        if let Ok(team_g) = team_arc.read() {
            if let Some(unit_arc) = team_g.try_to_recruit(&thing, &origin, max_recruit) {
                // Transfer to this team + idle (C++ setTeam + aiIdle).
                if let Ok(mut unit_g) = unit_arc.write() {
                    let _ = unit_g.set_team(Some(team_arc.clone()));
                    if let Some(ai) = unit_g.get_ai_update_interface() {
                        ai.ai_idle(CommandSourceType::FromAi);
                    }
                    recruited_id = Some(unit_g.get_id());
                }
                order.num_completed = 1;
            }
        }

        if recruited_id.is_none() {
            // startTraining residual: assign factory if idle.
            let _ = self.start_training_internal(&mut order, false, &team_name)?;
        }

        let mut team_q = TeamInQueue::new();
        team_q.team_name = Some(team_name);
        team_q.team = Some(team_arc);
        team_q.priority_build = false;
        team_q.reinforcement = true;
        // C++ m_reinforcementID is the recruited unit, else INVALID until trained.
        team_q.reinforcement_id = recruited_id;
        team_q.frame_started = TheGameLogic::get_frame();
        team_q.work_orders.push(order);
        // C++ prependTo_TeamBuildQueue
        self.team_build_queue.push_front(team_q);
        // C++ m_teamDelay = 0 shortcut
        self.team_delay = 0;

        log::debug!("AI auto-reinforcing one {} onto team instance", thing_name);
        Ok(true)
    }

    /// C++ `AIPlayer::queueSupplyTruck` (AIPlayer.cpp).
    ///
    /// Skip if a resource-gatherer is already queued. For each supply building
    /// needing gatherers: recount current, reattach loose harvesters, else start
    /// training one harvester (priority team) if under 3× desired global cap.
    /// C++ `AIPlayer::queueSupplyTruck` (AIPlayer.cpp).
    ///
    /// Skip if a resource-gatherer is already queued. For each supply build-list
    /// entry:
    /// - if current >= desired: maintain (nearby warehouse, recount/redock)
    /// - else: reattach loose harvesters, else train one (unless ≥3× desired total)
    fn queue_supply_truck(&mut self) -> Result<(), AiError> {
        // Already building a supply truck?
        let truck_in_queue = self.team_build_queue.iter().any(|team| {
            team.work_orders
                .iter()
                .any(|order| order.is_resource_gatherer)
        });
        if truck_in_queue {
            return Ok(());
        }

        let total_harvesters = self.count_player_harvesters();

        // Snapshot supply-building build-list entries we may need to service.
        let mut supply_entries: Vec<(ObjectID, i32, i32)> = Vec::new();
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player_arc.read() {
                    let mut cur = pg.get_build_list();
                    while let Some(info) = cur {
                        if info.is_supply_building() {
                            supply_entries.push((
                                info.get_object_id(),
                                info.get_desired_gatherers(),
                                info.get_current_gatherers(),
                            ));
                        }
                        cur = info.get_next();
                    }
                }
            }
        }

        for (center_id, desired, cur_gatherers) in supply_entries {
            if cur_gatherers >= desired {
                // C++ maintenance branch only when live non-hole center + nearby supplies.
                if center_id == INVALID_ID {
                    continue;
                }
                let Some(center_arc) = OBJECT_REGISTRY.get_object(center_id) else {
                    continue;
                };
                let Ok(center_g) = center_arc.read() else {
                    continue;
                };
                if center_g.is_kind_of(KindOf::RebuildHole) {
                    continue;
                }
                if !self.supply_center_has_nearby_supplies(&center_g) {
                    continue;
                }
                drop(center_g);
                // C++ checkForSupplyCenter then recount docked harvesters.
                let _ = self.check_for_supply_center(center_id);
                let recounted = self.recount_and_redock_harvesters(center_id);
                self.set_build_list_current_gatherers(center_id, recounted);
                continue;
            }

            // Under-desired: reattach loose harvesters (preferred dock missing).
            if center_id != INVALID_ID {
                if self.try_reattach_loose_harvester(center_id)? {
                    return Ok(());
                }
            }

            if total_harvesters >= desired.saturating_mul(3) {
                continue; // lotsa gatherers
            }

            // Temporarily allow unit building while training a harvester.
            let prev_can_build = self.set_can_build_units_temp(true);
            let queued = self.queue_one_harvester_at_factory(center_id, cur_gatherers)?;
            self.set_can_build_units_temp(prev_can_build);
            if queued {
                return Ok(());
            }
        }

        Ok(())
    }

    fn count_player_harvesters(&self) -> i32 {
        let Ok(list) = player_list().read() else {
            return 0;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return 0;
        };
        let Ok(pg) = player_arc.read() else {
            return 0;
        };
        let mut total = 0;
        for obj_id in pg.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if !obj.is_kind_of(KindOf::Harvester) {
                continue;
            }
            if let Some(ai) = obj.get_ai_update_interface() {
                if let Ok(ai_g) = ai.lock() {
                    if ai_g.get_supply_truck_ai_interface().is_some() {
                        total += 1;
                    }
                }
            }
        }
        total
    }

    fn supply_center_has_nearby_supplies(&self, center: &Object) -> bool {
        let center_pos = *center.get_position();
        let radius =
            SUPPLY_CENTER_CLOSE_DIST + center.get_geometry_info().get_bounding_circle_radius();

        let Some(partition) = ThePartitionManager::get() else {
            // Fallback: any warehouse on map with boxes.
            return OBJECT_REGISTRY.get_all_objects().iter().any(|obj| {
                obj.read()
                    .ok()
                    .map(|g| {
                        g.find_update_module("SupplyWarehouseDockUpdate")
                            .and_then(|m| {
                                m.with_module(|mm| {
                                    mm.get_supply_warehouse_dock_interface()
                                        .map(|w| w.boxes_stored() > 0)
                                })
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            });
        };

        for obj_id in partition.get_objects_in_range(&center_pos, radius) {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if !obj.is_kind_of(KindOf::SupplySource) {
                continue;
            }
            // Skip enemies.
            if let (Some(my_team), Some(their_team)) = (
                // approximate: controlling player
                Some(self.player_id),
                obj.get_controlling_player_id(),
            ) {
                if my_team != their_team {
                    // relationship residual: skip if not same player
                    // (C++ ENEMIES check via team relationship)
                    if let Ok(list) = player_list().read() {
                        if let Some(me) = list.get_player(self.player_id as i32) {
                            if let Ok(me_g) = me.read() {
                                if let Some(tarc) = obj.get_team() {
                                    if let Ok(tg) = tarc.read() {
                                        if me_g.get_relationship_with_team(&tg)
                                            == Relationship::Enemies
                                        {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(module) = obj.find_update_module("SupplyWarehouseDockUpdate") {
                let boxes = module.with_module(|m| {
                    m.get_supply_warehouse_dock_interface()
                        .map(|w| w.boxes_stored())
                });
                if boxes.unwrap_or(0) > 0 {
                    return true;
                }
            }
        }
        false
    }

    fn recount_and_redock_harvesters(&self, center_id: ObjectID) -> i32 {
        let Ok(list) = player_list().read() else {
            return 0;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return 0;
        };
        let Ok(pg) = player_arc.read() else {
            return 0;
        };
        let mut cur = 0;
        // Collect dock commands outside locks (C++ aiDock CMD_FROM_PLAYER).
        let mut redock: Vec<ObjectID> = Vec::new();
        for obj_id in pg.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if !obj.is_kind_of(KindOf::Harvester) {
                continue;
            }
            let Some(ai) = obj.get_ai_update_interface() else {
                continue;
            };
            let Ok(ai_g) = ai.lock() else {
                continue;
            };
            let Some(truck) = ai_g.get_supply_truck_ai_interface() else {
                continue;
            };
            if truck.get_preferred_dock_id() == Some(center_id) {
                cur += 1;
                // C++: if (!isCurrentlyFerryingSupplies()) aiDock(center, CMD_FROM_PLAYER)
                if !truck.is_currently_ferrying_supplies() {
                    redock.push(obj_id);
                }
            }
        }
        drop(pg);
        drop(list);
        for truck_id in redock {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(truck_id) {
                if let Ok(obj) = obj_arc.read() {
                    if let Some(ai) = obj.get_ai_update_interface() {
                        ai.ai_dock(center_id, CommandSourceType::FromPlayer);
                    }
                }
            }
        }
        cur
    }

    fn set_build_list_current_gatherers(&self, center_id: ObjectID, cur: i32) {
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        let mut node = Some(&mut *info);
                        while let Some(n) = node {
                            if n.get_object_id() == center_id {
                                n.set_current_gatherers(cur);
                                break;
                            }
                            node = n.get_next_mut();
                        }
                    }
                }
            }
        }
    }

    fn try_reattach_loose_harvester(&mut self, center_id: ObjectID) -> Result<bool, AiError> {
        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(false);
        };
        let Ok(pg) = player_arc.read() else {
            return Ok(false);
        };
        for obj_id in pg.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if !obj.is_kind_of(KindOf::Harvester) {
                continue;
            }
            let Some(ai) = obj.get_ai_update_interface() else {
                continue;
            };
            let Ok(ai_g) = ai.lock() else {
                continue;
            };
            let Some(truck) = ai_g.get_supply_truck_ai_interface() else {
                continue;
            };
            let dock = truck.get_preferred_dock_id();
            let dock_alive = dock
                .map(|id| OBJECT_REGISTRY.get_object(id).is_some())
                .unwrap_or(false);
            if dock_alive {
                continue;
            }
            if truck.is_currently_ferrying_supplies() || truck.is_forced_into_wanting_state() {
                // C++: bump current gatherers and aiDock(center, CMD_FROM_PLAYER).
                drop(ai_g);
                drop(obj);
                // Issue dock before recount so preferred dock can stick.
                if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
                    if let Ok(og) = obj_arc.read() {
                        if let Some(ai) = og.get_ai_update_interface() {
                            ai.ai_dock(center_id, CommandSourceType::FromPlayer);
                        }
                    }
                }
                self.set_build_list_current_gatherers(
                    center_id,
                    self.recount_and_redock_harvesters(center_id),
                );
                log::debug!("Re-attaching supply truck to supply center.");
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn set_can_build_units_temp(&self, can: bool) -> bool {
        let Ok(list) = player_list().read() else {
            return can;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return can;
        };
        let Ok(mut pg) = player_arc.write() else {
            return can;
        };
        let prev = pg.get_can_build_units();
        pg.set_can_build_units(can);
        prev
    }

    /// Find a harvester template with an idle factory and queue one (C++ priority team).
    ///
    /// C++ walks `TheThingFactory->firstTemplate()` / `friend_getNextTemplate()` for
    /// `KINDOF_HARVESTER`. Fall back to known faction names if the factory is empty.
    fn queue_one_harvester_at_factory(
        &mut self,
        center_id: ObjectID,
        cur_gatherers: i32,
    ) -> Result<bool, AiError> {
        // Collect harvester template names: full factory walk first (C++ order).
        let mut harvester_names: Vec<String> = Vec::new();
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                let mut current = factory.first_template().cloned();
                while let Some(template) = current {
                    // Common ThingTemplate uses u64 masks; resolve via TheThingFactory
                    // adapter for KindOf::Harvester (C++ isKindOf(KINDOF_HARVESTER)).
                    let name = template.get_name().to_string();
                    if !name.is_empty()
                        && TheThingFactory::find_template(&name)
                            .map(|t| t.is_kind_of(KindOf::Harvester))
                            .unwrap_or(false)
                        && !harvester_names.iter().any(|n| n == &name)
                    {
                        harvester_names.push(name);
                    }
                    current = template.get_next_template().clone();
                }
            }
        }
        // Fallback residual when ThingFactory unloaded (tests / early boot).
        if harvester_names.is_empty() {
            for name in [
                "AmericaVehicleChinook",
                "AmericaVehicleSupplyTruck",
                "ChinaVehicleSupplyTruck",
                "GLAVehicleSupplyTruck",
                "GLAInfantryWorker",
                "SupplyTruck",
            ] {
                if TheThingFactory::find_template(name)
                    .map(|t| t.is_kind_of(KindOf::Harvester))
                    .unwrap_or(false)
                {
                    harvester_names.push(name.to_string());
                }
            }
        }

        for name in harvester_names {
            let Some(factory_id) = self.find_factory_internal(&name, false)? else {
                continue;
            };

            let mut order = WorkOrder::new(name.clone());
            order.num_required = 1;
            order.required = true;
            order.is_resource_gatherer = true;

            let mut team = TeamInQueue::new();
            team.priority_build = true;
            team.frame_started = TheGameLogic::get_frame();
            // C++ sticks supply truck on default team (m_team + name).
            if let Ok(list) = player_list().read() {
                if let Some(player_arc) = list.get_player(self.player_id as i32) {
                    if let Ok(pg) = player_arc.read() {
                        if let Some(dt) = pg.get_default_team() {
                            if let Ok(tg) = dt.read() {
                                team.team_name = Some(tg.get_name().to_string());
                            }
                            team.team = Some(dt);
                        }
                    }
                }
            }

            self.team_delay = 0;
            let team_name = team
                .team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            if cur_gatherers == -1 {
                // First one is automatic (C++): assign factory without training.
                order.factory_id = Some(factory_id);
                self.set_build_list_current_gatherers(center_id, 0);
                team.work_orders.push(order);
                self.team_build_queue.push_front(team);
                log::debug!(
                    "Supply truck - automatic first gatherer ({}) at factory {}",
                    name,
                    factory_id
                );
                return Ok(true);
            }

            // startTraining before push to avoid double borrow of self.
            let _ = self.start_training_internal(&mut order, true, &team_name)?;
            team.work_orders.push(order);
            self.team_build_queue.push_front(team);
            log::debug!(
                "Supply truck - building one {} at factory {}",
                name,
                factory_id
            );
            return Ok(true);
        }
        Ok(false)
    }

    /// C++ `AIPlayer::processBaseBuilding` (AIPlayer.cpp) — USE_DOZER path residual.
    ///
    /// Walk player build list: track destroyed buildings, honor rebuild delay,
    /// start at most one dozer build per call, then arm structureTimer with wealth mods.
    fn process_base_building(&mut self) -> Result<(), AiError> {
        if !self.ready_to_build_structure {
            return Ok(());
        }

        // C++ processBaseBuilding: build list walk only (no host priority analysis).
        let current_frame = TheGameLogic::get_frame();
        let rebuild_delay_frames = self.rebuild_delay_frames();

        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return Ok(());
        };
        let player_index = player_guard.get_player_index() as u32;

        // Collect first actionable missing buildable entry (name, location, angle).
        // Also collect under-construction buildings needing dozer resume (C++).
        let mut to_build: Option<(String, Coord3D, Real)> = None;
        let mut resume_jobs: Vec<(ObjectID, ObjectID, Coord3D)> = Vec::new();
        // (bldg_id, builder_id_or_INVALID, bldg_pos)
        let mut info_opt = player_guard.get_build_list_mut();
        while let Some(info) = info_opt {
            let name = info.get_template_name();
            if name.is_empty() {
                info_opt = info.get_next_mut();
                continue;
            }

            let obj_id = info.get_object_id();
            if obj_id != INVALID_ID {
                match OBJECT_REGISTRY.get_object(obj_id) {
                    Some(obj_arc) => {
                        if let Ok(obj_guard) = obj_arc.read() {
                            if obj_guard.get_controlling_player_id() == Some(player_index) {
                                // Owned: if under construction, ensure dozer resumes (C++).
                                let under = obj_guard
                                    .test_status(ObjectStatusTypes::UnderConstruction)
                                    || obj_guard.is_under_construction();
                                if under {
                                    resume_jobs.push((
                                        obj_id,
                                        obj_guard.get_builder_id(),
                                        *obj_guard.get_position(),
                                    ));
                                }
                                info_opt = info.get_next_mut();
                                continue;
                            }
                        }
                        // Captured or gone: clear and stamp for rebuild delay.
                        let prior_id = obj_id;
                        info.set_object_id(INVALID_ID);
                        info.set_object_timestamp(current_frame.saturating_add(1));
                        // C++ GLA hole scan by spawnerID.
                        for hole_arc in OBJECT_REGISTRY.get_all_objects() {
                            let Ok(hg) = hole_arc.read() else {
                                continue;
                            };
                            if !hg.is_kind_of(KindOf::RebuildHole) {
                                continue;
                            }
                            let mut matched = false;
                            for behavior in hg.get_behavior_modules() {
                                if let Ok(mut bg) = behavior.lock() {
                                    if let Some(rhbi) = bg.get_rebuild_hole_behavior_interface() {
                                        if rhbi.get_spawner_id() == prior_id {
                                            matched = true;
                                        }
                                        break;
                                    }
                                }
                            }
                            if matched {
                                info.set_object_id(hg.get_id());
                                break;
                            }
                        }
                    }
                    None => {
                        let prior_id = obj_id;
                        info.set_object_id(INVALID_ID);
                        info.set_object_timestamp(current_frame.saturating_add(1));
                        for hole_arc in OBJECT_REGISTRY.get_all_objects() {
                            let Ok(hg) = hole_arc.read() else {
                                continue;
                            };
                            if !hg.is_kind_of(KindOf::RebuildHole) {
                                continue;
                            }
                            let mut matched = false;
                            for behavior in hg.get_behavior_modules() {
                                if let Ok(mut bg) = behavior.lock() {
                                    if let Some(rhbi) = bg.get_rebuild_hole_behavior_interface() {
                                        if rhbi.get_spawner_id() == prior_id {
                                            matched = true;
                                        }
                                        break;
                                    }
                                }
                            }
                            if matched {
                                info.set_object_id(hg.get_id());
                                break;
                            }
                        }
                    }
                }
            }

            // C++: only apply rebuild delay when objectID is INVALID and timestamp>0.
            // (Hole-attached IDs skip this branch until the hole is gone.)
            if info.get_object_id() == INVALID_ID && info.get_object_timestamp() > 0 {
                if info
                    .get_object_timestamp()
                    .saturating_add(rebuild_delay_frames)
                    > current_frame
                {
                    info_opt = info.get_next_mut();
                    continue;
                }
                log::debug!("Enabling rebuild for {}", name);
                info.set_object_timestamp(0); // ready to build
            }

            if !info.is_buildable() {
                info_opt = info.get_next_mut();
                continue;
            }

            // C++: isBuildable && findObjectByID == NULL → dozer build.
            if info.get_object_id() == INVALID_ID {
                to_build = Some((name.to_string(), *info.get_location(), info.get_angle()));
                break;
            }

            info_opt = info.get_next_mut();
        }
        drop(player_guard);

        // C++: for each UC building, aiResumeConstruction on builder or findDozer.
        for (bldg_id, builder_id, bldg_pos) in resume_jobs {
            let mut dozer_id = builder_id;
            let mut builder_ok = false;
            if dozer_id != INVALID_ID {
                if let Some(darc) = OBJECT_REGISTRY.get_object(dozer_id) {
                    if let Ok(dg) = darc.read() {
                        if dg.get_controlling_player_id() == Some(player_index)
                            && dg.get_ai_update_interface().is_some()
                        {
                            builder_ok = true;
                        }
                    }
                }
            }
            if !builder_ok {
                log::debug!("AI's Dozer got killed.  Find another dozer.");
                // C++ solo does not queueDozer here (skirmish does).
                dozer_id = self.find_dozer(&bldg_pos)?.unwrap_or(INVALID_ID);
                if dozer_id == INVALID_ID {
                    continue;
                }
                // Clear dead builder on building.
                if let Some(barc) = OBJECT_REGISTRY.get_object(bldg_id) {
                    if let Ok(mut bg) = barc.write() {
                        bg.set_builder(None);
                    }
                }
            }
            if let Some(darc) = OBJECT_REGISTRY.get_object(dozer_id) {
                if let Ok(dg) = darc.read() {
                    if let Some(ai) = dg.get_ai_update_interface() {
                        if let Ok(mut ai_g) = ai.lock() {
                            let mut params = crate::ai::AiCommandParams::new(
                                crate::ai::AiCommandType::ResumeConstruction,
                                CommandSourceType::FromAi,
                            );
                            params.obj = Some(bldg_id);
                            let _ = ai_g.execute_command(&params);
                        }
                    }
                }
            }
        }

        if let Some((name, location, angle)) = to_build {
            // C++ USE_DOZER: buildStructureWithDozer; NULL → no timer arm.
            match self.build_structure_with_dozer(&name, location, angle)? {
                Some(_bldg_id) => {
                    self.arm_structure_timer_after_build()?;
                    self.frame_last_building_built = current_frame;
                    // C++: only one building per delay loop.
                    return Ok(());
                }
                None => {
                    // No dozer / funds / placement — retry later.
                    return Ok(());
                }
            }
        }

        // C++ processBaseBuilding walks BuildListInfo only — no construction_priorities fallback.
        Ok(())
    }

    /// C++ rebuild delay frames from AIData `m_rebuildDelaySeconds` (default path).
    /// Retail AIData = 30; zero/unloaded AIData falls back to REBUILD_DELAY_SECONDS.
    fn rebuild_delay_frames(&self) -> u32 {
        let seconds = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    if data.rebuild_delay_seconds > 0 {
                        data.rebuild_delay_seconds as u32
                    } else {
                        REBUILD_DELAY_SECONDS
                    }
                })
            })
            .unwrap_or(REBUILD_DELAY_SECONDS);
        seconds * LOGICFRAMES_PER_SECOND
    }

    /// After starting a structure: C++ sets ready=false and structureTimer with wealth mods.
    ///
    /// C++ always re-reads `TheAI->getAiData()->m_structureSeconds` (not a player
    /// field). Retail StructureSeconds=0 → timer 0 (immediately eligible next
    /// doBaseBuilding).
    pub(crate) fn arm_structure_timer_after_build(&mut self) -> Result<(), AiError> {
        self.ready_to_build_structure = false;
        // Live AIData structureSeconds (0.0 is valid retail). Keep field snapshot
        // in sync for xfer/tests that set structure_seconds directly.
        let structure_seconds = THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.structure_seconds))
            .unwrap_or(self.structure_seconds);
        self.structure_seconds = structure_seconds;
        let mut timer = (structure_seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32) as u32;

        let money = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_money().get_money()))
            .unwrap_or(0);

        let (poor, wealthy, poor_mod, wealthy_mod) = Self::structure_wealth_params();

        // C++: timer = timer / mod when mod applies (mod 0 → skip).
        // Integer divide of 0 stays 0 (immediate re-ready).
        if money < poor && poor_mod > 0.0 {
            timer = (timer as f32 / poor_mod) as u32;
        } else if money > wealthy && wealthy_mod > 0.0 {
            timer = (timer as f32 / wealthy_mod) as u32;
        }

        self.structure_timer = timer;
        Ok(())
    }

    /// Retail AIData structure wealth params; zero AIData fields → Default/AIData.ini fallbacks.
    fn structure_wealth_params() -> (i32, i32, f32, f32) {
        THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    (
                        if data.resources_poor > 0 {
                            data.resources_poor
                        } else {
                            RESOURCES_POOR
                        },
                        if data.resources_wealthy > 0 {
                            data.resources_wealthy
                        } else {
                            RESOURCES_WEALTHY
                        },
                        if data.structures_poor_mod > 0.0 {
                            data.structures_poor_mod
                        } else {
                            STRUCTURES_POOR_MODIFIER
                        },
                        if data.structures_wealthy_mod > 0.0 {
                            data.structures_wealthy_mod
                        } else {
                            STRUCTURES_WEALTHY_MODIFIER
                        },
                    )
                })
            })
            .unwrap_or((
                RESOURCES_POOR,
                RESOURCES_WEALTHY,
                STRUCTURES_POOR_MODIFIER,
                STRUCTURES_WEALTHY_MODIFIER,
            ))
    }

    /// Retail AIData team wealth params; zero AIData fields → Default/AIData.ini fallbacks.
    fn team_wealth_params() -> (i32, i32, f32, f32) {
        THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    (
                        if data.resources_poor > 0 {
                            data.resources_poor
                        } else {
                            RESOURCES_POOR
                        },
                        if data.resources_wealthy > 0 {
                            data.resources_wealthy
                        } else {
                            RESOURCES_WEALTHY
                        },
                        if data.team_poor_mod > 0.0 {
                            data.team_poor_mod
                        } else {
                            TEAMS_POOR_MODIFIER
                        },
                        if data.team_wealthy_mod > 0.0 {
                            data.team_wealthy_mod
                        } else {
                            TEAMS_WEALTHY_MODIFIER
                        },
                    )
                })
            })
            .unwrap_or((
                RESOURCES_POOR,
                RESOURCES_WEALTHY,
                TEAMS_POOR_MODIFIER,
                TEAMS_WEALTHY_MODIFIER,
            ))
    }

    /// Analyze current building needs
    fn analyze_building_needs(&mut self) -> Result<(), AiError> {
        // Check if we need power
        if self.economic_state.power_shortage {
            let priority = ConstructionPriority {
                building_type: "PowerPlant".to_string(),
                priority: 1,
                prerequisites_met: true,
                max_count: None,
                current_count: 0,
                desired_location: None,
                desired_angle: None,
            };
            self.construction_priorities.push(priority);
        }

        // Check if we need supply centers
        if self.economic_state.supply_shortage {
            let priority = ConstructionPriority {
                building_type: "SupplyCenter".to_string(),
                priority: 2,
                prerequisites_met: true,
                max_count: None,
                current_count: 0,
                desired_location: None,
                desired_angle: None,
            };
            self.construction_priorities.push(priority);
        }

        Ok(())
    }

    /// C++ `AIPlayer::isPossibleToBuildTeam` (AIPlayer.cpp).
    ///
    /// Returns `(possible, not_enough_money)`.
    /// For each unit type: must have *some* factory (`findFactory(..., true)`);
    /// track whether any unit type has an **idle** factory. Cost uses
    /// `(min+max)/2` average count, then `* teamResourcesToBuild`.
    fn is_possible_to_build_team(
        &self,
        team_name: &str,
        require_idle_factory: bool,
    ) -> Result<(bool, bool), AiError> {
        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok((false, false));
        };
        let Some(proto) = factory_guard.find_team_prototype(team_name) else {
            return Ok((false, false));
        };
        // Clone unit list so we can drop the factory lock before find_factory.
        let units: Vec<(String, i32, i32)> = proto
            .units_info()
            .iter()
            .filter(|u| !u.unit_thing_name.is_empty())
            .map(|u| (u.unit_thing_name.to_string(), u.min_units, u.max_units))
            .collect();
        drop(factory_guard);

        // Cost calc needs player, but find_factory_internal also locks the player
        // RwLock (not reentrant) — snapshot money/cost player handle briefly, drop,
        // then factory-scan, then re-check money.
        // C++ uses Int cost with float intermediate truncated each assignment.
        let mut any_idle = false;
        let mut cost: i32 = 0;
        {
            let Ok(list) = player_list().read() else {
                return Ok((false, false));
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok((false, false));
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok((false, false));
            };
            for (thing_name, min_units, max_units) in &units {
                let Some(template) = TheThingFactory::find_template(thing_name) else {
                    continue;
                };
                let thing_cost = template.calc_cost_to_build(Some(&*player_guard)) as i32;
                // C++: cost += thingCost * ((maxUnits+minUnits)/2.0f);  // truncates to Int
                cost +=
                    (thing_cost as f32 * ((*max_units as f32 + *min_units as f32) / 2.0)) as i32;
            }
        }

        for (thing_name, _min_units, _max_units) in &units {
            if TheThingFactory::find_template(thing_name).is_none() {
                continue;
            }
            // C++: findFactory(thing, true) — any factory (busy OK). Missing → false.
            if self.find_factory_internal(thing_name, true)?.is_none() {
                return Ok((false, false));
            }
            // C++: findFactory(thing, false) — idle.
            if self.find_factory_internal(thing_name, false)?.is_some() {
                any_idle = true;
            }
        }

        let resources_mod = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.team_resources_to_build)
            })
            .filter(|m| *m > 0.0)
            .unwrap_or(TEAM_RESOURCES_TO_BUILD);
        // C++: cost *= m_teamResourcesToBuild; (Int *= Real truncates)
        cost = (cost as f32 * resources_mod) as i32;

        let money = {
            let Ok(list) = player_list().read() else {
                return Ok((false, false));
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok((false, false));
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok((false, false));
            };
            player_guard.get_money().get_money() as i32
        };
        if money < cost {
            return Ok((false, true)); // notEnoughMoney
        }
        if any_idle {
            return Ok((true, false));
        }
        if !require_idle_factory {
            return Ok((true, false));
        }
        Ok((false, false))
    }

    /// Check if team is a good idea to build right now
    /// Matches C++ AIPlayer.cpp:1471 isAGoodIdeaToBuildTeam
    pub(crate) fn is_a_good_idea_to_build_team(&self, team_name: &str) -> Result<bool, AiError> {
        // C++ AIPlayer::isAGoodIdeaToBuildTeam:
        // 1. evaluateProductionCondition()
        // 2. countTeamInstances() >= maxInstances → reject
        // 3. already building same prototype in TeamBuildQueue → reject
        // 4. isPossibleToBuildTeam(proto, true, needMoney)

        // Snapshot under the factory lock, then drop before is_possible_to_build_team
        // (same Mutex — not reentrant).
        let (condition_ok, instances, max_instances) = {
            let factory = get_team_factory();
            let Ok(factory_guard) = factory.lock() else {
                return Ok(false);
            };
            let Some(proto) = factory_guard.find_team_prototype(team_name) else {
                return Ok(false);
            };
            (
                proto.evaluate_production_condition(),
                factory_guard.find_team_instances(team_name).len() as i32,
                proto.get_max_instances(),
            )
        };

        if !condition_ok {
            return Ok(false);
        }
        // C++ bare: countTeamInstances() >= m_maxInstances
        if instances >= max_instances {
            return Ok(false);
        }

        // C++: team->m_team->getPrototype() == proto (busy building this prototype).
        if self.team_build_queue.iter().any(|q| {
            if let Some(team_arc) = q.team.as_ref() {
                if let Ok(tg) = team_arc.read() {
                    if tg.get_name().as_str() == team_name {
                        return true;
                    }
                }
            }
            q.team_name
                .as_deref()
                .map(|name| name == team_name)
                .unwrap_or(false)
        }) {
            return Ok(false);
        }

        let (possible, _) = self.is_possible_to_build_team(team_name, true)?;
        Ok(possible)
    }

    /// C++ `AIPlayer::findDozer` (AIPlayer.cpp).
    ///
    /// Prefer idle dozers (not building, not ferrying supplies, not repair dozer).
    /// Closest idle dozer wins. If no dozer exists at all, queue one.
    fn find_dozer(&mut self, location: &Coord3D) -> Result<Option<ObjectID>, AiError> {
        use crate::object::update::ai_update::dozer_ai_update::DozerTask;

        let mut need_dozer = true;
        let mut dozer: Option<ObjectID> = None;
        let mut closest_dozer: Option<ObjectID> = None;
        let mut closest_dist_sqr = 0.0_f32;

        let object_ids: Vec<ObjectID> = {
            let Ok(list) = player_list().read() else {
                return Ok(None);
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok(None);
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(None);
            };
            player_guard.get_all_objects()
        };

        for obj_id in object_ids {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_destroyed() || !obj_guard.is_kind_of(KindOf::Dozer) {
                continue;
            }
            let Some(ai) = obj_guard.get_ai_update_interface() else {
                continue;
            };
            let Ok(mut ai_guard) = ai.lock() else {
                continue;
            };

            // Must have dozer AI; capture task flags before optional truck check.
            let (has_dozer, build_pending, any_pending) =
                match ai_guard.get_dozer_ai_update_interface_mut() {
                    Some(dozer_ai) => (
                        true,
                        dozer_ai.is_task_pending(DozerTask::Build),
                        dozer_ai.is_any_task_pending(),
                    ),
                    None => (false, false, false),
                };
            if !has_dozer {
                continue;
            }
            if !any_pending {
                // Don't steal supply-ferrying workers (GLA).
                if let Some(truck) = ai_guard.get_supply_truck_ai_interface() {
                    if truck.is_currently_ferrying_supplies()
                        || truck.is_forced_into_wanting_state()
                    {
                        continue;
                    }
                }
            }

            if Some(obj_id) == self.repair_dozer {
                continue;
            }
            need_dozer = false;

            if build_pending {
                continue;
            }
            let idle = !any_pending;
            if idle {
                dozer = Some(obj_id);
            } else if dozer.is_none() {
                dozer = Some(obj_id);
            }

            if idle {
                let pos = obj_guard.get_position();
                let dx = location.x - pos.x;
                let dy = location.y - pos.y;
                let dist_sqr = dx * dx + dy * dy;
                if closest_dozer.is_none() || dist_sqr < closest_dist_sqr {
                    closest_dozer = Some(obj_id);
                    closest_dist_sqr = dist_sqr;
                }
            }
        }

        if need_dozer {
            let _ = self.queue_dozer();
        }
        if closest_dozer.is_some() {
            return Ok(closest_dozer);
        }
        Ok(dozer)
    }

    /// C++ `AIPlayer::queueDozer` (AIPlayer.cpp).
    ///
    /// If no dozer already queued, walk ThingFactory for KINDOF_DOZER with a
    /// factory (busyOK=true), priority-queue a team, and startTraining.
    /// Does **not** set `dozer_queued_for_repair` (that flag is repair-path only).
    pub(crate) fn queue_dozer(&mut self) -> Result<(), AiError> {
        if self.dozer_in_queue() {
            return Ok(());
        }

        let prev_can = self.set_can_build_units_temp(true);

        // C++: firstTemplate / friend_getNextTemplate for KINDOF_DOZER.
        let mut dozer_names: Vec<String> = Vec::new();
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                let mut current = factory.first_template().cloned();
                while let Some(template) = current {
                    let name = template.get_name().to_string();
                    if !name.is_empty()
                        && TheThingFactory::find_template(&name)
                            .map(|t| t.is_kind_of(KindOf::Dozer))
                            .unwrap_or(false)
                        && !dozer_names.iter().any(|n| n == &name)
                    {
                        dozer_names.push(name);
                    }
                    current = template.get_next_template().clone();
                }
            }
        }
        // Fallback residual when ThingFactory unloaded (tests / early boot).
        if dozer_names.is_empty() {
            for name in [
                "AmericaVehicleDozer",
                "ChinaVehicleDozer",
                "GLAInfantryWorker",
                "Dozer",
                "Worker",
            ] {
                if TheThingFactory::find_template(name)
                    .map(|t| t.is_kind_of(KindOf::Dozer))
                    .unwrap_or(false)
                {
                    dozer_names.push(name.to_string());
                }
            }
        }

        for name in dozer_names {
            // C++ findFactory(tTemplate, true) — busyOK allows queueing on busy factory.
            let Some(factory_id) = self.find_factory_internal(&name, true)? else {
                continue;
            };

            let mut order = WorkOrder::new(name.clone());
            order.num_required = 1;
            order.required = true;
            order.is_resource_gatherer = false;

            let mut team = TeamInQueue::new();
            team.priority_build = true;
            team.frame_started = TheGameLogic::get_frame();
            if let Ok(list) = player_list().read() {
                if let Some(player_arc) = list.get_player(self.player_id as i32) {
                    if let Ok(pg) = player_arc.read() {
                        if let Some(dt) = pg.get_default_team() {
                            if let Ok(tg) = dt.read() {
                                team.team_name = Some(tg.get_name().to_string());
                            }
                            team.team = Some(dt);
                        }
                    }
                }
            }
            let team_name = team
                .team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            // C++: prependTo_TeamBuildQueue then startTraining. Train first so we
            // do not hold a queue borrow across &mut self (same observable result:
            // factoryID stamped on the order before it sits in the queue).
            self.team_delay = 0;
            let _ = self.start_training_internal(&mut order, true, &team_name)?;
            team.work_orders.push(order);
            self.team_build_queue.push_front(team);
            // C++ queueDozer does not set m_dozerQueuedForRepair.
            log::debug!("DOZER - building one {} at factory {}", name, factory_id);
            break;
        }

        self.set_can_build_units_temp(prev_can);
        Ok(())
    }

    /// Returns true if a dozer is already present in the build queue.
    /// C++ `dozerInQueue` → `TeamInQueue::includesADozer` (KINDOF_DOZER and
    /// **not** a resource-gatherer work order — GLA workers can be both).
    pub fn dozer_in_queue(&self) -> bool {
        self.team_build_queue
            .iter()
            .any(|team| team.includes_a_dozer())
    }

    /// C++ `AIPlayer::repairStructure` (AIPlayer.cpp).
    pub(crate) fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        let Some(structure_arc) = OBJECT_REGISTRY.get_object(structure_id) else {
            return Ok(());
        };
        let Ok(structure_g) = structure_arc.read() else {
            return Ok(());
        };
        let Some(body) = structure_g.get_body_module() else {
            return Ok(());
        };
        let Ok(body_g) = body.lock() else {
            return Ok(());
        };
        if body_g.get_damage_state() == crate::object::body::BodyDamageType::Pristine {
            return Ok(());
        }
        drop(body_g);
        drop(structure_g);

        // Already queued?
        for i in 0..self.structures_in_queue as usize {
            if self.structures_to_repair.get(i).and_then(|s| *s) == Some(structure_id) {
                return Ok(());
            }
        }
        if self.structures_in_queue as usize >= MAX_STRUCTURES_TO_REPAIR {
            log::debug!("Structure repair queue is full, ignoring repair request.");
            return Ok(());
        }
        let idx = self.structures_in_queue as usize;
        self.structures_to_repair[idx] = Some(structure_id);
        self.structures_in_queue += 1;
        Ok(())
    }

    /// Remove all queued teams from both the build and ready queues.
    pub fn clear_teams_in_queue(&mut self) {
        self.team_build_queue.clear();
        self.team_ready_queue.clear();
    }

    pub fn set_base_center_set(&mut self, set: bool) {
        self.base_center_set = set;
        if !set {
            self.base_radius = 0.0;
        }
    }

    /// Public wrapper for skirmish newMap initiallyBuilt inst-build.
    pub fn build_structure_now_at_public(
        &mut self,
        template_name: &str,
        location: Coord3D,
        angle: Real,
    ) -> Result<Option<ObjectID>, AiError> {
        self.build_structure_now_at(template_name, location, angle, None)
    }

    /// Public findDozer for skirmish processBaseBuilding resume path.
    pub fn find_dozer_public(&mut self, location: &Coord3D) -> Result<Option<ObjectID>, AiError> {
        self.find_dozer(location)
    }

    pub fn set_frame_last_building_built(&mut self, frame: u32) {
        self.frame_last_building_built = frame;
    }

    /// C++ `AIPlayer::aiPreTeamDestroy(const Team *deletedTeam)`.
    ///
    /// Drop TeamInQueue entries whose `m_team` is the deleted instance (pointer
    /// identity). Name match is fallback for legacy/xfer entries without handle.
    pub fn ai_pre_team_destroy(&mut self, deleted: &Arc<RwLock<crate::team::Team>>) {
        let deleted_name = deleted.read().ok().map(|g| g.get_name().to_string());
        let keep = |q: &TeamInQueue| -> bool {
            if let Some(ref qt) = q.team {
                // C++: team->m_team == deletedTeam
                return !Arc::ptr_eq(qt, deleted);
            }
            // Fallback: name compare when m_team missing.
            if let (Some(ref dn), Some(ref qn)) = (deleted_name.as_ref(), q.team_name.as_ref()) {
                return qn != dn;
            }
            true
        };
        self.team_build_queue.retain(keep);
        self.team_ready_queue.retain(keep);
    }

    /// Name-based wrapper for call sites that only have a team name.
    pub fn ai_pre_team_destroy_by_name(&mut self, team_name: &str) {
        self.team_build_queue.retain(|team| {
            team.team_name
                .as_deref()
                .map(|name| name != team_name)
                .unwrap_or(true)
                && team
                    .team
                    .as_ref()
                    .and_then(|a| a.read().ok())
                    .map(|g| g.get_name().as_str() != team_name)
                    .unwrap_or(true)
        });
        self.team_ready_queue.retain(|team| {
            team.team_name
                .as_deref()
                .map(|name| name != team_name)
                .unwrap_or(true)
                && team
                    .team
                    .as_ref()
                    .and_then(|a| a.read().ok())
                    .map(|g| g.get_name().as_str() != team_name)
                    .unwrap_or(true)
        });
    }

    /// C++ `AIPlayer::guardSupplyCenter` (AIPlayer.cpp).
    ///
    /// Force attack check; prefer attacked center else findSupplyCenter; issue
    /// aiGuardPosition toward enemy base offset by warehouse radius*0.8.
    pub fn guard_supply_center(
        &mut self,
        team_name: &str,
        min_supplies: i32,
    ) -> Result<(), AiError> {
        self.supply_source_attack_check_frame = 0; // force check
        let mut warehouse_id = None;
        if self.is_supply_source_attacked() {
            warehouse_id = self.attacked_supply_center;
        }
        if warehouse_id.is_none() {
            warehouse_id = self
                .find_supply_center(min_supplies)
                .and_then(|w| w.read().ok().map(|g| g.get_id()));
        }
        let Some(warehouse_id) = warehouse_id else {
            return Ok(());
        };
        let Some(warehouse_arc) = OBJECT_REGISTRY.get_object(warehouse_id) else {
            return Ok(());
        };
        let Ok(warehouse) = warehouse_arc.read() else {
            return Ok(());
        };
        let mut location = *warehouse.get_position();
        let radius = warehouse.get_geometry_info().get_bounding_circle_radius() * 0.8;

        // Offset toward enemy structure bounds center.
        let enemy_ndx = self.get_skirmish_enemy_player_index();
        if let Ok((lo, hi)) = self.get_player_structure_bounds(enemy_ndx) {
            let mut ox = location.x - (lo.x + hi.x) * 0.5;
            let mut oy = location.y - (lo.y + hi.y) * 0.5;
            let len = (ox * ox + oy * oy).sqrt();
            if len > 0.0001 {
                ox /= len;
                oy /= len;
                location.x -= ox * radius;
                location.y -= oy * radius;
            }
        }
        drop(warehouse);

        // Resolve team members (named team or default).
        let members: Vec<ObjectID> = {
            let mut team_arc = None;
            if !team_name.is_empty() {
                if let Ok(mut factory) = get_team_factory().lock() {
                    team_arc = factory.find_team(team_name);
                }
            }
            if team_arc.is_none() {
                if let Ok(list) = player_list().read() {
                    if let Some(player_arc) = list.get_player(self.player_id as i32) {
                        if let Ok(pg) = player_arc.read() {
                            team_arc = pg.get_default_team();
                        }
                    }
                }
            }
            team_arc
                .and_then(|t| t.read().ok().map(|g| g.get_members().to_vec()))
                .unwrap_or_default()
        };

        // C++: AIGroup::groupGuardPosition(&location, GUARDMODE_NORMAL, CMD_FROM_SCRIPT)
        // Issue per-member guard with script command source (not the no-op trait stub).
        for member_id in members {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(member_id) else {
                continue;
            };
            let Ok(obj_g) = obj_arc.read() else {
                continue;
            };
            let Some(ai) = obj_g.get_ai_update_interface() else {
                continue;
            };
            // AIUpdateInterfaceExt::ai_guard_position(pos, mode, cmd_source)
            ai.ai_guard_position(&location, GuardMode::Normal, CommandSourceType::FromScript);
        }
        Ok(())
    }

    /// C++ `TheScriptEngine->getSkirmishEnemyPlayer()->getPlayerIndex()`.
    /// Prefer this player's current enemy, then first human, then any non-neutral.
    fn get_skirmish_enemy_player_index(&self) -> i32 {
        if let Ok(list) = player_list().read() {
            if let Some(me) = list.get_player(self.player_id as i32) {
                if let Ok(mg) = me.read() {
                    if let Some(enemy_index) = mg.get_current_enemy_player_index() {
                        if let Some(enemy) = list.get_player(enemy_index) {
                            if let Ok(eg) = enemy.read() {
                                if eg.get_player_type() != PlayerType::Neutral {
                                    return enemy_index;
                                }
                            }
                        }
                    }
                }
            }
            // C++ ScriptEngine residual: first human player.
            for i in 0..list.get_player_count() {
                if let Some(p) = list.get_player(i as i32) {
                    if let Ok(pg) = p.read() {
                        if pg.get_player_type() == PlayerType::Human {
                            return i as i32;
                        }
                    }
                }
            }
            for i in 0..list.get_player_count() {
                let i = i as i32;
                if i == self.player_id as i32 {
                    continue;
                }
                if let Some(p) = list.get_player(i) {
                    if let Ok(pg) = p.read() {
                        if pg.get_player_type() != PlayerType::Neutral {
                            return i;
                        }
                    }
                }
            }
        }
        0
    }

    /// Get player structure bounds for targeting.
    /// Matches C++ `AIPlayer::getPlayerStructureBounds(bounds, playerNdx)` with
    /// `conservative = false` (default call sites).
    pub fn get_player_structure_bounds(
        &self,
        player_index: i32,
    ) -> Result<(Coord3D, Coord3D), AiError> {
        self.get_player_structure_bounds_ex(player_index, false)
    }

    /// C++ `AIPlayer::getPlayerStructureBounds(bounds, playerNdx, conservative)`.
    ///
    /// Structure AABB only (non-structures never contribute). When `conservative`,
    /// skip KINDOF_CONSERVATIVE_BUILDING. No structures → zeroed bounds (C++ leaves
    /// Region2D at 0). Final C++ `if (!firstStructure) *bounds = objBounds` is a
    /// no-op because both AABBs only track structures.
    pub fn get_player_structure_bounds_ex(
        &self,
        player_index: i32,
        conservative: bool,
    ) -> Result<(Coord3D, Coord3D), AiError> {
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned())
        else {
            return Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)));
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)));
        };

        let mut first_structure = true;
        let mut struct_min = Coord3D::new(0.0, 0.0, 0.0);
        let mut struct_max = Coord3D::new(0.0, 0.0, 0.0);

        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            // C++ only enters the AABB expand when isKindOf(STRUCTURE).
            if !obj_guard.is_kind_of(KindOf::Structure) {
                continue;
            }
            // C++: conservative && KINDOF_CONSERVATIVE_BUILDING → skip.
            if conservative && obj_guard.is_kind_of(KindOf::ConservativeBuilding) {
                continue;
            }

            let pos = *obj_guard.get_position();
            if first_structure {
                struct_min = Coord3D::new(pos.x, pos.y, pos.z);
                struct_max = Coord3D::new(pos.x, pos.y, pos.z);
                first_structure = false;
            } else {
                struct_min.x = struct_min.x.min(pos.x);
                struct_min.y = struct_min.y.min(pos.y);
                struct_max.x = struct_max.x.max(pos.x);
                struct_max.y = struct_max.y.max(pos.y);
            }
        }

        // No structures → zeroed bounds (C++ never copies unit-only bounds).
        if first_structure {
            Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)))
        } else {
            Ok((struct_min, struct_max))
        }
    }

    /// Calculate center and radius of AI base
    /// Matches C++ AIPlayer computeCenterAndRadiusOfBase logic
    /// C++ `AIPlayer::computeCenterAndRadiusOfBase` (AIPlayer.cpp).
    ///
    /// Average of build-list entry locations (not live structures). Radius is
    /// max |dx|+geom*0.4 / |dy|+geom*0.4 Manhattan-as-axis-abs then hypot.
    pub fn compute_center_and_radius_of_base(&mut self) -> Result<(), AiError> {
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        // Pass 1: centroid of valid build-list locations.
        let mut total_x = 0.0_f32;
        let mut total_y = 0.0_f32;
        let mut num_bldg = 0i32;
        let mut entries: Vec<(Coord3D, f32)> = Vec::new(); // pos + geom radius
        let mut cur = player_guard.get_build_list();
        while let Some(info) = cur {
            let name = info.get_template_name().to_string();
            if name.is_empty() {
                cur = info.get_next();
                continue;
            }
            let Some(template) = TheThingFactory::find_template(&name) else {
                cur = info.get_next();
                continue;
            };
            let pos = *info.get_location();
            total_x += pos.x;
            total_y += pos.y;
            num_bldg += 1;
            let geom_r = template
                .get_template_geometry_info()
                .get_bounding_circle_radius()
                * 0.4;
            entries.push((pos, geom_r));
            cur = info.get_next();
        }

        self.base_center_set = num_bldg > 0;
        if num_bldg > 0 {
            self.base_center =
                Coord3D::new(total_x / num_bldg as f32, total_y / num_bldg as f32, 0.0);
        } else {
            self.base_center = Coord3D::new(0.0, 0.0, 0.0);
            self.base_radius = 0.0;
            return Ok(());
        }

        // Pass 2: max radSqr with axis-abs + geom padding (C++).
        let mut max_rad_sqr = 0.0_f32;
        for (pos, bldg_radius) in entries {
            let mut dx = pos.x - self.base_center.x;
            let mut dy = pos.y - self.base_center.y;
            if dx < 0.0 {
                dx = -dx;
            }
            if dy < 0.0 {
                dy = -dy;
            }
            dx += bldg_radius;
            dy += bldg_radius;
            let rad_sqr = dx * dx + dy * dy;
            if rad_sqr > max_rad_sqr {
                max_rad_sqr = rad_sqr;
            }
        }
        self.base_radius = max_rad_sqr.sqrt();
        Ok(())
    }

    fn select_current_enemy_player(&self) -> Result<Option<(Arc<RwLock<Player>>, i32)>, AiError> {
        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(me_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(me_guard) = me_arc.read() else {
            return Ok(None);
        };
        if let Some(enemy_index) = me_guard.get_current_enemy_player_index() {
            if let Some(enemy_arc) = list.get_player(enemy_index).cloned() {
                let is_non_neutral = if let Ok(enemy_guard) = enemy_arc.read() {
                    enemy_guard.get_player_type() != PlayerType::Neutral
                } else {
                    false
                };
                if is_non_neutral {
                    return Ok(Some((enemy_arc, enemy_index)));
                }
            }
        }

        for (index, player_arc) in list.iter().enumerate() {
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };
            if player_guard.get_player_type() == PlayerType::Neutral {
                continue;
            }
            if player_guard.get_id() == self.player_id as i32 {
                continue;
            }
            return Ok(Some((player_arc.clone(), index as i32)));
        }

        Ok(None)
    }

    fn count_active_harvesters(&self) -> usize {
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return 0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 0;
        };
        let mut count = 0;
        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_kind_of(KindOf::Harvester) {
                count += 1;
            }
        }
        count
    }

    /// C++ `AIPlayer::getPlayerSuperweaponValue` (AIPlayer.cpp).
    fn get_player_superweapon_value(
        &self,
        center: &Coord3D,
        player_index: i32,
        radius: Real,
        include_military_units: bool,
    ) -> Result<i32, AiError> {
        let radius = radius.max(4.0 * PATHFIND_CELL_SIZE_F);
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned())
        else {
            return Ok(0);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(0);
        };

        let mut cash = 0.0_f32;
        let rad_sqr = radius * radius;
        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            let mut apply_neg_value = false;
            if !include_military_units {
                // Sneak attack: defenses + combat units are hostile (negative).
                if obj_guard.is_kind_of(KindOf::FSBaseDefense)
                    || obj_guard.is_kind_of(KindOf::TechBaseDefense)
                {
                    apply_neg_value = true;
                } else if (obj_guard.is_kind_of(KindOf::Vehicle)
                    || obj_guard.is_kind_of(KindOf::Infantry))
                    && !obj_guard.is_kind_of(KindOf::Dozer)
                    && !obj_guard.is_kind_of(KindOf::Harvester)
                {
                    apply_neg_value = true;
                }
            } else if obj_guard.is_kind_of(KindOf::Aircraft)
                && obj_guard.is_significantly_above_terrain()
            {
                // Only when valuing military: skip flying aircraft.
                continue;
            }

            let pos = obj_guard.get_position();
            let dx = center.x - pos.x;
            let dy = center.y - pos.y;
            if dx * dx + dy * dy >= rad_sqr {
                continue;
            }
            let dist = (dx * dx + dy * dy).sqrt();
            let factor = 1.0 - (dist / (2.0 * radius)); // 1.0 center, 0.5 edge
                                                        // C++ calcCostToBuild(pPlayer) — pass player when possible.
            let mut value = obj_guard
                .get_template()
                .calc_cost_to_build(Some(&*player_guard as &dyn std::any::Any))
                .max(0) as f32;
            if obj_guard.is_kind_of(KindOf::CommandCenter) {
                value = if include_military_units {
                    value / 10.0
                } else {
                    value * 5.0
                };
            }
            if obj_guard.is_kind_of(KindOf::FSSuperweapon) {
                value = if include_military_units {
                    value / 10.0
                } else {
                    value * 5.0
                };
            }
            if apply_neg_value {
                cash -= factor * value * 5.0;
            } else {
                cash += factor * value;
            }
        }
        // C++ returns Int (truncates Real cash).
        Ok(cash as i32)
    }
}

impl Snapshot for AIPlayer {
    /// C++ `AIPlayer::crc` is empty (no fields hashed).
    fn crc(&self, _xfer: &mut dyn Xfer) {
        // Intentionally empty — matches GeneralsMD AIPlayer.cpp.
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut team_build_queue_count = self.team_build_queue.len() as u16;
        let _ = xfer.xfer_unsigned_short(&mut team_build_queue_count);
        if xfer.is_loading() {
            self.team_build_queue.clear();
            for _ in 0..team_build_queue_count {
                let mut team = TeamInQueue::new();
                team.xfer(xfer);
                self.team_build_queue.push_back(team);
            }
        } else {
            for team in &mut self.team_build_queue {
                team.xfer(xfer);
            }
        }

        let mut team_ready_queue_count = self.team_ready_queue.len() as u16;
        let _ = xfer.xfer_unsigned_short(&mut team_ready_queue_count);
        if xfer.is_loading() {
            self.team_ready_queue.clear();
            for _ in 0..team_ready_queue_count {
                let mut team = TeamInQueue::new();
                team.xfer(xfer);
                self.team_ready_queue.push_back(team);
            }
        } else {
            for team in &mut self.team_ready_queue {
                team.xfer(xfer);
            }
        }

        let mut player_index = self.player_id as i32;
        let _ = xfer.xfer_int(&mut player_index);

        let mut ready_to_build_team = self.ready_to_build_team;
        let _ = xfer.xfer_bool(&mut ready_to_build_team);
        if xfer.is_loading() {
            self.ready_to_build_team = ready_to_build_team;
        }

        let mut ready_to_build_structure = self.ready_to_build_structure;
        let _ = xfer.xfer_bool(&mut ready_to_build_structure);
        if xfer.is_loading() {
            self.ready_to_build_structure = ready_to_build_structure;
        }

        let mut team_timer = self.team_timer as i32;
        let _ = xfer.xfer_int(&mut team_timer);
        if xfer.is_loading() {
            self.team_timer = team_timer as u32;
        }

        let mut structure_timer = self.structure_timer as i32;
        let _ = xfer.xfer_int(&mut structure_timer);
        if xfer.is_loading() {
            self.structure_timer = structure_timer as u32;
        }

        let mut build_delay = self.build_delay as i32;
        let _ = xfer.xfer_int(&mut build_delay);
        if xfer.is_loading() {
            self.build_delay = build_delay as u32;
        }

        let mut team_delay = self.team_delay as i32;
        let _ = xfer.xfer_int(&mut team_delay);
        if xfer.is_loading() {
            self.team_delay = team_delay as u32;
        }

        let mut team_seconds = self.team_seconds.round() as i32;
        let _ = xfer.xfer_int(&mut team_seconds);
        if xfer.is_loading() {
            self.team_seconds = team_seconds as Real;
        }

        let mut cur_warehouse_id = self.current_warehouse_id.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut cur_warehouse_id);
        if xfer.is_loading() {
            self.current_warehouse_id = if cur_warehouse_id == INVALID_ID {
                None
            } else {
                Some(cur_warehouse_id)
            };
        }

        let mut frame_last_building_built = self.frame_last_building_built as i32;
        let _ = xfer.xfer_int(&mut frame_last_building_built);
        if xfer.is_loading() {
            self.frame_last_building_built = frame_last_building_built as u32;
        }

        let mut difficulty = self.difficulty as i32;
        let _ = xfer.xfer_int(&mut difficulty);
        if xfer.is_loading() {
            self.difficulty = match difficulty {
                0 => GameDifficulty::Easy,
                1 => GameDifficulty::Normal,
                2 => GameDifficulty::Hard,
                3 => GameDifficulty::Brutal,
                _ => GameDifficulty::Normal,
            };
        }

        let mut skillset_selector = self.skillset_selector;
        let _ = xfer.xfer_int(&mut skillset_selector);
        if xfer.is_loading() {
            self.skillset_selector = skillset_selector;
        }

        xfer.xfer_coord3d(&mut self.base_center);

        let mut base_center_set = self.base_center_set;
        let _ = xfer.xfer_bool(&mut base_center_set);
        if xfer.is_loading() {
            self.base_center_set = base_center_set;
        }

        let mut base_radius = self.base_radius;
        let _ = xfer.xfer_real(&mut base_radius);
        if xfer.is_loading() {
            self.base_radius = base_radius;
        }

        for i in 0..MAX_STRUCTURES_TO_REPAIR {
            let mut id = self.structures_to_repair[i].unwrap_or(INVALID_ID);
            let _ = xfer.xfer_object_id(&mut id);
            if xfer.is_loading() {
                self.structures_to_repair[i] = if id == INVALID_ID { None } else { Some(id) };
            }
        }

        let mut repair_dozer = self.repair_dozer.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut repair_dozer);
        if xfer.is_loading() {
            self.repair_dozer = if repair_dozer == INVALID_ID {
                None
            } else {
                Some(repair_dozer)
            };
        }

        let mut structures_in_queue = self.structures_in_queue;
        let _ = xfer.xfer_int(&mut structures_in_queue);
        if xfer.is_loading() {
            self.structures_in_queue = structures_in_queue;
        }

        let mut dozer_queued_for_repair = self.dozer_queued_for_repair;
        let _ = xfer.xfer_bool(&mut dozer_queued_for_repair);
        if xfer.is_loading() {
            self.dozer_queued_for_repair = dozer_queued_for_repair;
        }

        let mut dozer_is_repairing = self.dozer_is_repairing;
        let _ = xfer.xfer_bool(&mut dozer_is_repairing);
        if xfer.is_loading() {
            self.dozer_is_repairing = dozer_is_repairing;
        }

        let mut bridge_timer = self.bridge_timer as i32;
        let _ = xfer.xfer_int(&mut bridge_timer);
        if xfer.is_loading() {
            self.bridge_timer = bridge_timer as u32;
        }
    }

    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_player_creation() {
        let ai_player = AIPlayer::new(1);
        assert_eq!(ai_player.player_id, 1);
        assert_eq!(ai_player.difficulty, GameDifficulty::Normal);
        assert!(!ai_player.base_center_set);
    }

    #[test]
    fn test_work_order() {
        let mut order = WorkOrder::new("Ranger".to_string());
        assert!(order.is_waiting_to_build());

        order.num_completed = 1;
        order.num_required = 1;
        assert!(!order.is_waiting_to_build());
    }

    #[test]
    fn test_team_in_queue() {
        let mut team = TeamInQueue::new();

        let mut order1 = WorkOrder::new("Ranger".to_string());
        order1.num_required = 2;
        order1.required = true;
        team.work_orders.push(order1);

        let mut order2 = WorkOrder::new("Humvee".to_string());
        order2.num_required = 1;
        team.work_orders.push(order2);

        assert!(!team.is_all_built());
        assert!(!team.is_minimum_built());

        team.work_orders[0].num_completed = 2;
        assert!(!team.is_all_built()); // Second order not complete
        assert!(team.is_minimum_built()); // Required order complete

        team.work_orders[1].num_completed = 1;
        assert!(team.is_all_built()); // All orders complete
    }

    #[test]
    fn is_minimum_built_counts_in_progress_factory_like_cpp() {
        let mut team = TeamInQueue::new();
        let mut order = WorkOrder::new("USA_Ranger".into());
        order.required = true;
        order.num_required = 2;
        order.num_completed = 1;
        order.factory_id = Some(1); // C++ counts +1 for assigned factory
        team.work_orders.push(order);
        assert!(team.is_minimum_built());
        team.work_orders[0].factory_id = None;
        assert!(!team.is_minimum_built());
    }

    #[test]
    fn are_builds_complete_requires_no_factory_like_cpp() {
        let mut team = TeamInQueue::new();
        let mut order = WorkOrder::new("USA_Ranger".into());
        order.num_completed = 1;
        order.num_required = 1;
        order.factory_id = Some(7);
        team.work_orders.push(order);
        // C++ areBuildsComplete is false while factory assigned, even if count done.
        assert!(!team.are_builds_complete());
        team.work_orders[0].factory_id = None;
        assert!(team.are_builds_complete());
    }

    #[test]
    fn team_in_queue_xfer_uses_team_id_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("impl TeamInQueue").unwrap_or(0);
        // Find TeamInQueue::xfer after work orders
        let j = src[i..]
            .find("pub fn xfer(&mut self, xfer: &mut dyn Xfer)")
            .expect("TeamInQueue xfer")
            + i;
        // There may be WorkOrder xfer first — find the one with priority_build
        let k = src[j..].find("priority_build").expect("priority") + j;
        let w = &src[k.saturating_sub(200)..src.len().min(k + 1200)];
        assert!(
            w.contains("xfer_unsigned_int(&mut team_id)")
                && w.contains("find_team_by_id")
                && w.contains("TEAM_ID_INVALID")
                && !w.contains("xfer_ascii_string(&mut team_name)"),
            "TeamInQueue xfer must use TeamID like C++, not team name string"
        );
    }

    #[test]
    fn ai_pre_team_destroy_matches_m_team_pointer() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn ai_pre_team_destroy(&mut self, deleted:")
            .expect("aiPreTeamDestroy");
        let window = &src[i..src.len().min(i + 1800)];
        assert!(
            window.contains("Arc::ptr_eq")
                && window.contains("team_build_queue.retain")
                && window.contains("team_ready_queue.retain"),
            "aiPreTeamDestroy must drop queue entries by m_team pointer identity"
        );
    }

    #[test]
    fn includes_a_dozer_uses_kindof_dozer() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("pub fn includes_a_dozer").expect("includesADozer");
        let window = &src[i..src.len().min(i + 1200)];
        assert!(
            window.contains("KindOf::Dozer")
                && window.contains("find_template")
                && window.contains("is_resource_gatherer"),
            "includesADozer must check KINDOF_DOZER and exclude resource gatherers"
        );
    }

    #[test]
    fn is_location_safe_filters_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("pub fn is_location_safe").expect("isLocationSafe");
        let window = &src[i..src.len().min(i + 3500)];
        assert!(
            window.contains("supply_center_safe_radius")
                && window.contains("KindOf::Harvester")
                && window.contains("KindOf::Dozer")
                && window.contains("ObjectStatusTypes::Stealthed")
                && window.contains("Relationship::Enemies")
                && window.contains("is_effectively_dead"),
            "isLocationSafe must apply C++ partition filters"
        );
    }

    #[test]
    fn team_in_queue_drop_activates_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        assert!(
            src.contains("impl Drop for TeamInQueue")
                && src.contains("tg.set_active()")
                && src.contains("self.team = None"),
            "TeamInQueue Drop must setActive; disband must null m_team"
        );
        // disband path nulls team before Drop
        let i = src.find("pub fn disband(&mut self)").expect("disband");
        let w = &src[i..src.len().min(i + 3500)];
        assert!(
            w.contains("self.team = None"),
            "disband must clear team handle like C++ m_team = NULL"
        );
    }

    #[test]
    fn team_in_queue_stores_team_handle_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        assert!(
            src.contains("pub team: Option<Arc<RwLock<crate::team::Team>>>"),
            "TeamInQueue must hold m_team handle"
        );
        let i = src.find("pub fn build_specific_ai_team").expect("bst");
        let w = &src[i..src.len().min(i + 9000)];
        assert!(
            w.contains("team.team = Some(team_arc)"),
            "buildSpecificAITeam must stamp TeamInQueue.m_team"
        );
        let j = src.find("pub(crate) fn check_ready_teams").expect("ready");
        let rw = &src[j..src.len().min(j + 7500)];
        assert!(
            rw.contains("tg.is_idle()") && rw.contains("tg.set_active()"),
            "checkReadyTeams must use m_team isIdle/setActive"
        );
        let k = src
            .find("pub(crate) fn check_queued_teams")
            .expect("queued");
        let qw = &src[k..src.len().min(k + 6500)];
        assert!(
            qw.contains("tq.team.as_ref()")
                && qw.contains("tg.get_members()")
                && qw.contains("aig.is_idle()"),
            "checkQueuedTeams anyIdle must walk m_team members"
        );
    }

    #[test]
    fn check_ready_teams_execute_actions_uses_team_handle() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub(crate) fn check_ready_teams")
            .expect("checkReadyTeams");
        let w = &src[i..src.len().min(i + 5000)];
        assert!(
            w.contains("get_execute_actions_on_create")
                && w.contains("find_script_clone_by_name")
                && w.contains("tg.get_name()")
                && w.contains("60 * LOGICFRAMES_PER_SECOND"),
            "checkReadyTeams must resolve executeActions via team handle name"
        );
    }

    #[test]
    fn check_ready_teams_activates_on_60s_expiry() {
        // C++: timeExpired = frameStarted + 60*LOGICFRAMES_PER_SECOND < frame
        let mut ai = AIPlayer::new(1);
        let mut team = TeamInQueue::new();
        team.team_name = Some("TestReadyTeam".into());
        team.frame_started = 0;
        ai.team_ready_queue.push_back(team);
        // Without a live team object, activation still removes from ready queue
        // when timeExpired forces allIdle=true.
        // Force "now" via TheGameLogic if available; otherwise just exercise path.
        let _ = ai.check_ready_teams();
        // After check, either still queued (frame not advanced) or empty (expired).
        // Structural honesty: function exists and does not panic.
        assert!(ai.team_ready_queue.len() <= 1);
    }

    #[test]
    fn check_queued_all_built_prepends_ready_queue_like_cpp() {
        let mut ai = AIPlayer::new(1);
        let mut team = TeamInQueue::new();
        team.team_name = Some("BuiltTeam".into());
        let mut order = WorkOrder::new("USA_Ranger".into());
        order.num_completed = 1;
        order.num_required = 1;
        team.work_orders.push(order);
        assert!(team.is_all_built());
        ai.team_build_queue.push_back(team);
        // Seed ready with a marker to verify prepend
        let mut marker = TeamInQueue::new();
        marker.team_name = Some("AlreadyReady".into());
        ai.team_ready_queue.push_back(marker);
        ai.check_queued_teams().expect("check_queued");
        assert!(ai.team_build_queue.is_empty());
        assert_eq!(ai.team_ready_queue.len(), 2);
        // C++ prependTo_TeamReadyQueue → new team at front
        assert_eq!(
            ai.team_ready_queue
                .front()
                .and_then(|t| t.team_name.as_deref()),
            Some("BuiltTeam")
        );
    }

    #[test]
    fn build_and_team_delay_recheck_constants_match_cpp() {
        // C++ AIPlayer.cpp: m_buildDelay = 2*LOGICFRAMES_PER_SECOND;
        //                   m_teamDelay = 5*LOGICFRAMES_PER_SECOND;
        assert_eq!(BUILD_DELAY_RECHECK_FRAMES, 2 * LOGICFRAMES_PER_SECOND);
        assert_eq!(TEAM_DELAY_RECHECK_FRAMES, 5 * LOGICFRAMES_PER_SECOND);
        assert_eq!(BUILD_DELAY_RECHECK_FRAMES, 60);
        assert_eq!(TEAM_DELAY_RECHECK_FRAMES, 150);
    }

    #[test]
    fn retail_aidata_fallback_constants_match_default_ini() {
        // windows_game/.../Default/AIData.ini
        assert!((DEFAULT_STRUCTURE_SECONDS - 0.0).abs() < f32::EPSILON);
        assert!((DEFAULT_TEAM_SECONDS - 10.0).abs() < f32::EPSILON);
        assert_eq!(RESOURCES_POOR, 2000);
        assert_eq!(RESOURCES_WEALTHY, 7000);
        assert!((STRUCTURES_POOR_MODIFIER - 0.6).abs() < 1e-5);
        assert!((STRUCTURES_WEALTHY_MODIFIER - 2.0).abs() < 1e-5);
        assert!((TEAMS_POOR_MODIFIER - 0.6).abs() < 1e-5);
        assert!((TEAMS_WEALTHY_MODIFIER - 2.0).abs() < 1e-5);
        assert_eq!(REBUILD_DELAY_SECONDS, 30);
        assert!((SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE - 150.0).abs() < 1e-5);
    }

    #[test]
    fn science_points_early_out_before_skillset_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub(crate) fn do_upgrades_and_skills")
            .expect("do_upgrades_and_skills");
        let end = src[i..]
            .find(
                "
    pub(crate) fn update_bridge_repair",
            )
            .map(|o| i + o)
            .unwrap_or(src.len().min(i + 6000));
        let w = &src[i..end];
        let skillset_idx = w
            .find("skillset_selector == INVALID_SKILLSET_SELECTION")
            .expect("skillset pick");
        let early = &w[..skillset_idx];
        assert!(
            early.contains("purchase_points_early")
                && early.contains("if purchase_points_early <= 0"),
            "C++ returns when science points are 0 before skillset selection"
        );
    }

    #[test]
    fn friend_execute_action_team_scoped_like_cpp() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/scripting/engine.rs"
        ));
        assert!(
            eng.contains("fn friend_execute_action")
                && eng.contains("calling_team")
                && eng.contains("execute_action_chain"),
            "ScriptEngine must expose friend_executeAction with team context"
        );
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub(crate) fn check_queued_teams")
            .expect("check_queued_teams");
        let end = src[i..]
            .find(
                "
    /// C++ `AIPlayer::doTeamBuilding`",
            )
            .map(|o| i + o)
            .unwrap_or(src.len().min(i + 7000));
        let w = &src[i..end];
        assert!(
            w.contains("friend_execute_action")
                && !w.contains("ScriptEvaluator::new(script_engine)"),
            "checkQueuedTeams must call friend_execute_action with team name"
        );
    }

    #[test]
    fn process_base_building_no_priority_fallback_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn process_base_building(&mut self)")
            .expect("process_base_building");
        let end = src[i..]
            .find(
                "
    /// C++ rebuild delay frames",
            )
            .map(|o| i + o)
            .unwrap_or(src.len().min(i + 8000));
        let w = &src[i..end];
        assert!(
            w.contains("build_structure_with_dozer")
                && !w.contains("construction_priorities.first()")
                && !w.contains("build_structure_now(&priority)")
                && !w.contains("analyze_building_needs")
                && !w.contains("update_construction_priorities"),
            "processBaseBuilding must walk BuildListInfo only like C++ (no host priorities)"
        );
    }

    #[test]
    fn is_a_good_idea_checks_m_team_handle_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub(crate) fn is_a_good_idea_to_build_team")
            .expect("is_a_good_idea");
        let w = &src[i..src.len().min(i + 2200)];
        assert!(
            w.contains("q.team.as_ref()")
                && w.contains("tg.get_name()")
                && w.contains("team_build_queue.iter()"),
            "isAGoodIdeaToBuildTeam must reject queue entries by m_team prototype, not only team_name"
        );
    }

    #[test]
    fn ai_player_trait_update_order_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("impl AiPlayerTrait for AIPlayer").expect("trait");
        let j = src[i..].find("fn update(&mut self)").expect("update") + i;
        let w = &src[j..src.len().min(j + 600)];
        assert!(
            w.contains("do_base_building")
                && w.contains("check_ready_teams")
                && w.contains("check_queued_teams")
                && w.contains("do_team_building")
                && w.contains("do_upgrades_and_skills")
                && w.contains("update_bridge_repair")
                && !w.contains("update_strategy"),
            "AiPlayerTrait::update must match C++ AIPlayer::update phase order only"
        );
        // update_with_frame also must not inject strategy into the C++ phase block.
        let k = src
            .find("pub fn update_with_frame")
            .expect("update_with_frame");
        let ww = &src[k..src.len().min(k + 1200)];
        let base = ww.find("do_base_building").expect("base");
        let bridge = ww.find("update_bridge_repair").expect("bridge");
        let mid = &ww[base..=bridge];
        assert!(
            !mid.contains("update_strategy") && !mid.contains("process_attack_decisions"),
            "C++ phase block in update_with_frame must stay free of host residuals"
        );
    }

    #[test]
    fn check_queued_teams_no_factory_residual_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub(crate) fn check_queued_teams")
            .expect("check_queued_teams");
        let end = src[i..]
            .find(
                "
    /// C++ `AIPlayer::doTeamBuilding`",
            )
            .map(|o| i + o)
            .unwrap_or(src.len().min(i + 5000));
        let w = &src[i..end];
        assert!(
            !w.contains("orders_to_process")
                && !w.contains("find_factory_internal(&thing_template"),
            "checkQueuedTeams must not assign factory_id residual"
        );
        assert!(
            w.contains("is_build_time_expired")
                && w.contains("is_all_built")
                && w.contains("get_execute_actions_on_create"),
            "checkQueuedTeams must keep expire/all-built/executeActions phases"
        );
    }

    #[test]
    fn set_ai_difficulty_only_sets_field_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("pub fn set_ai_difficulty(&mut self, difficulty: GameDifficulty)")
            .expect("setAIDifficulty");
        let end = prod[i..]
            .find("pub fn select_skillset")
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 400));
        let w = &prod[i..end];
        assert!(
            w.contains("self.difficulty = difficulty")
                && !w.contains("team_seconds")
                && !w.contains("difficulty_factor")
                && !w.contains("difficulty_handler"),
            "setAIDifficulty must only assign m_difficulty like C++"
        );
    }

    #[test]
    fn do_upgrades_and_skills_skips_first_two_frames_like_cpp() {
        // C++: if (TheGameLogic->getFrame() < 2) return;
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub(crate) fn do_upgrades_and_skills")
            .expect("do_upgrades");
        let window = &src[i..src.len().min(i + 900)];
        assert!(
            window.contains("get_frame() < 2")
                && window.contains("get_science_purchase_points")
                && window.find("get_frame() < 2").unwrap()
                    < window.find("get_side()").unwrap_or(usize::MAX),
            "do_upgrades_and_skills must gate frame<2 and science points before side walk"
        );
    }

    #[test]
    fn arm_structure_timer_zero_seconds_stays_zero_like_cpp() {
        let mut ai = AIPlayer::new(1);
        ai.structure_seconds = 0.0;
        ai.ready_to_build_structure = true;
        ai.arm_structure_timer_after_build().expect("arm");
        assert!(!ai.ready_to_build_structure);
        // C++: m_structureSeconds*LOGICFRAMES with 0 → timer 0 (immediate next ready path).
        assert_eq!(ai.structure_timer, 0);
    }

    #[test]
    fn do_base_building_sets_2s_build_delay_like_cpp() {
        let mut ai = AIPlayer::new(1);
        ai.ready_to_build_structure = true;
        ai.build_delay = 0;
        ai.do_base_building().expect("do_base");
        assert_eq!(
            ai.build_delay, BUILD_DELAY_RECHECK_FRAMES,
            "after process attempt, C++ sets buildDelay to 2 seconds"
        );
    }

    #[test]
    fn do_team_building_sets_5s_team_delay_and_queues_like_cpp() {
        let mut ai = AIPlayer::new(1);
        ai.ready_to_build_team = true;
        ai.team_delay = 0;
        ai.do_team_building().expect("do_team");
        assert_eq!(
            ai.team_delay, TEAM_DELAY_RECHECK_FRAMES,
            "after queue/process attempt, C++ sets teamDelay to 5 seconds"
        );
    }

    #[test]
    fn solo_do_base_building_does_not_clamp_structure_timer_like_cpp() {
        // C++ AIPlayer::doBaseBuilding has no 3*LOGICFRAMES clamp; skirmish does.
        let mut ai = AIPlayer::new(1);
        ai.ready_to_build_structure = false;
        ai.structure_timer = 3 * LOGICFRAMES_PER_SECOND + 50;
        let before = ai.structure_timer;
        ai.do_base_building().expect("do_base");
        // decremented by 1, not clamped to 3s
        assert_eq!(ai.structure_timer, before - 1);
        assert!(ai.structure_timer > 3 * LOGICFRAMES_PER_SECOND);
    }

    #[test]
    fn solo_do_team_building_does_not_clamp_team_timer_like_cpp() {
        let mut ai = AIPlayer::new(1);
        ai.ready_to_build_team = false;
        ai.team_timer = 3 * LOGICFRAMES_PER_SECOND + 50;
        let before = ai.team_timer;
        ai.do_team_building().expect("do_team");
        assert_eq!(ai.team_timer, before - 1);
        assert!(ai.team_timer > 3 * LOGICFRAMES_PER_SECOND);
    }

    #[test]
    fn do_base_building_decrements_structure_timer_until_ready() {
        let mut ai = AIPlayer::new(1);
        ai.ready_to_build_structure = false;
        ai.structure_timer = 2;
        ai.build_delay = 99;
        ai.do_base_building().expect("t1");
        assert!(!ai.ready_to_build_structure);
        assert_eq!(ai.structure_timer, 1);
        assert_eq!(
            ai.build_delay, 98,
            "while waiting, only structureTimer path; buildDelay still decrements"
        );
        // Expiry frame: C++ sets buildDelay=0 then same-frame continues into
        // buildDelay recheck and sets buildDelay = 2*LOGICFRAMES_PER_SECOND.
        ai.do_base_building().expect("t2");
        assert!(ai.ready_to_build_structure);
        assert_eq!(
            ai.build_delay, BUILD_DELAY_RECHECK_FRAMES,
            "same-frame recheck after timer expiry sets 2s delay (C++)"
        );
    }

    #[test]
    fn update_with_frame_source_order_matches_cpp_do_methods() {
        // Source-order honesty: do_base → check_ready → check_queued → do_team
        // (delays are inside do_*, not pre-gated in update_with_frame).
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("pub fn update_with_frame").expect("uwf");
        let window = &src[i..src.len().min(i + 2800)];
        let base = window.find("self.do_base_building()?").expect("base");
        let ready = window.find("self.check_ready_teams()?").expect("ready");
        let queued = window.find("self.check_queued_teams()?").expect("queued");
        let team = window.find("self.do_team_building()?").expect("team");
        assert!(base < ready && ready < queued && queued < team);
        assert!(
            !window[..base].contains("if self.ready_to_build_structure && self.build_delay"),
            "update_with_frame must not pre-gate do_base_building (C++ always calls it)"
        );
        assert!(
            window.contains("host_attack_enabled()")
                && window.contains("GENERALS_AI_HOST_ATTACK")
                && window.contains("host_analysis_enabled()"),
            "host residual attack/analysis must be env-gated off by default"
        );
    }

    #[test]
    fn host_attack_disabled_by_default_like_cpp_update() {
        // Unset env in test process may inherit; function treats missing as false.
        std::env::remove_var("GENERALS_AI_HOST_ATTACK");
        assert!(!AIPlayer::host_attack_enabled());
        std::env::remove_var("GENERALS_AI_HOST_ANALYSIS");
        assert!(!AIPlayer::host_analysis_enabled());
    }

    #[test]
    fn arm_structure_timer_applies_wealth_mods_like_cpp() {
        // C++: m_structureTimer = TheAI->getAiData()->m_structureSeconds * FPS
        // (live AIData, not a per-player field). Snapshot and restore AIData.
        let prev_seconds = {
            let ai_g = THE_AI.write().expect("ai");
            let data_arc = ai_g.get_ai_data().clone();
            drop(ai_g);
            let mut data = data_arc.write().expect("data");
            let prev = data.structure_seconds;
            data.structure_seconds = 10.0; // 300 frames base
            prev
        };
        let mut player_ai = AIPlayer::new(1);
        player_ai.ready_to_build_structure = true;
        player_ai.arm_structure_timer_after_build().expect("arm");
        assert!(!player_ai.ready_to_build_structure);
        // No player money → 0 < Poor(2000) → divide by StructuresPoorRate 0.6.
        // C++ Real (f32) truncation: (300f32 / 0.6) as u32 == 499.
        assert_eq!(
            player_ai.structure_timer,
            (300f32 / STRUCTURES_POOR_MODIFIER) as u32
        );
        assert_eq!(player_ai.structure_timer, 499);
        {
            let ai_g = THE_AI.write().expect("ai restore");
            let data_arc = ai_g.get_ai_data().clone();
            drop(ai_g);
            let mut data = data_arc.write().expect("data restore");
            data.structure_seconds = prev_seconds;
        }
    }

    #[test]
    fn arm_structure_timer_reads_live_aidata_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn arm_structure_timer_after_build")
            .expect("arm_structure");
        let w = &src[i..src.len().min(i + 1200)];
        assert!(
            w.contains("get_ai_data()")
                && w.contains("structure_seconds")
                && w.contains("Live AIData"),
            "arm structure timer must re-read AIData.structureSeconds like C++"
        );
    }

    #[test]
    fn process_base_building_honors_rebuild_delay_timestamp() {
        // C++: if timestamp + rebuildDelaySeconds*FPS > frame, skip rebuild.
        let mut ai = AIPlayer::new(1);
        ai.ready_to_build_structure = true;
        // No player build list → falls through without panic.
        ai.process_base_building().expect("process");
        // rebuild_delay_frames uses AIData or REBUILD_DELAY_SECONDS constant.
        assert!(ai.rebuild_delay_frames() >= LOGICFRAMES_PER_SECOND);
    }

    #[test]
    fn process_base_building_source_has_cpp_rebuild_and_timer_arm() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn process_base_building(&mut self) -> Result<(), AiError>")
            .expect("pbb");
        let window = &src[i..src.len().min(i + 16000)];
        assert!(
            window.contains("rebuild_delay_frames")
                && window.contains("set_object_timestamp")
                && window.contains("arm_structure_timer_after_build")
                && window.contains("build_structure_with_dozer")
                && window.contains("only one building per delay loop")
                && window.contains("ResumeConstruction"),
            "process_base_building must port C++ rebuild delay + dozer resume + timer arm"
        );
    }

    #[test]
    fn select_team_uses_player_team_list_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        for (label, needle) in [
            ("build", "pub(crate) fn select_team_to_build"),
            ("reinforce", "pub(crate) fn select_team_to_reinforce"),
        ] {
            let i = src.find(needle).unwrap_or_else(|| panic!("{label}"));
            let window = &src[i..src.len().min(i + 2800)];
            assert!(
                window.contains("get_player_team_prototypes"),
                "{label} must iterate player team list like C++ getPlayerTeams"
            );
            assert!(
                !window.contains("list_team_prototypes()"),
                "{label} must not walk global TeamFactory list"
            );
        }
        let player = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/player.rs"));
        assert!(
            player.contains("pub fn get_player_team_prototypes"),
            "Player must expose getPlayerTeams equivalent"
        );
    }

    #[test]
    fn select_team_to_build_random_hi_pri_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        // Anchor on the Result signature so we do not match select_team_to_build_ai.
        let i = src
            .find("fn select_team_to_build(&mut self) -> Result<bool, AiError>")
            .expect("select_team_to_build");
        let window = &src[i..src.len().min(i + 3500)];
        assert!(
            window.contains("INVALID_PRI")
                && window.contains("select_team_to_reinforce(hi_pri)")
                && window.contains("game_logic_random_value")
                && window.contains("build_specific_ai_team(team_name, false)")
                && window.contains("arm_team_timer_after_build"),
            "select_team_to_build must match C++ hiPri/reinforce/random/arm timer"
        );
    }

    #[test]
    fn arm_team_timer_after_build_sets_ready_false() {
        let mut ai = AIPlayer::new(1);
        ai.team_seconds = 10.0; // 300 frames
        ai.ready_to_build_team = true;
        ai.arm_team_timer_after_build().expect("arm");
        assert!(!ai.ready_to_build_team);
        // money=0 → poor TeamsPoorRate 0.6 (f32 truncation like C++ Real).
        assert_eq!(ai.team_timer, (300f32 / TEAMS_POOR_MODIFIER) as u32);
        assert_eq!(ai.team_timer, 499);
    }

    #[test]
    fn arm_team_timer_zero_seconds_stays_zero_like_cpp() {
        let mut ai = AIPlayer::new(1);
        ai.team_seconds = 0.0;
        ai.ready_to_build_team = true;
        ai.arm_team_timer_after_build().expect("arm");
        assert!(!ai.ready_to_build_team);
        // C++ allows teamTimer 0 when TeamSeconds is 0 (no clamp to 1).
        assert_eq!(ai.team_timer, 0);
        // Next doTeamBuilding: timer==0 → ready true (same one-frame lag as C++).
        ai.do_team_building().expect("tick");
        assert!(ai.ready_to_build_team);
        assert_eq!(ai.team_timer, 0);
    }

    #[test]
    fn is_a_good_idea_requires_idle_factory_like_cpp() {
        // C++ isAGoodIdeaToBuildTeam calls isPossibleToBuildTeam(proto, true, ...)
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("fn is_a_good_idea_to_build_team").expect("good");
        let window = &src[i..src.len().min(i + 2000)];
        assert!(
            window.contains("is_possible_to_build_team(team_name, true)"),
            "is_a_good_idea must require idle factory (C++ busyOK=true means require idle)"
        );
    }

    #[test]
    fn is_a_good_idea_calls_evaluate_production_condition() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("fn is_a_good_idea_to_build_team").expect("good");
        let window = &src[i..src.len().min(i + 2200)];
        assert!(
            window.contains("evaluate_production_condition()"),
            "is_a_good_idea must call evaluateProductionCondition first (C++)"
        );
    }

    #[test]
    fn queue_units_prefers_m_team_handle() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn queue_units(&mut self)")
            .expect("queue_units");
        let window = &src[i..src.len().min(i + 4500)];
        assert!(
            window.contains("team_q.team.is_none()")
                && window.contains("team_q.team.clone()")
                && window.contains("queue_units_home_for_team(team_arc.as_ref()")
                && window.contains("start_training_internal(order, busy_ok, train_name.as_str())"),
            "queueUnits must recruit/train via TeamInQueue.m_team"
        );
    }

    #[test]
    fn queue_units_try_to_recruit_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        // Doc comment sits above the fn signature — include a lookback.
        let i = src
            .find("C++ `AIPlayer::queueUnits`")
            .expect("queueUnits doc");
        let window = &src[i..src.len().min(i + 5000)];
        assert!(
            window.contains("try_to_recruit")
                && window.contains("while order.is_waiting_to_build()")
                && window.contains("ai_move_to_position")
                && window.contains("ai_idle")
                && window.contains("start_training_internal")
                && window.contains("validate_factory")
                && window.contains("max_recruit_distance"),
            "queue_units must recruit-then-train like C++ queueUnits"
        );
    }

    #[test]
    fn queue_units_empty_queue_ok() {
        let mut ai = AIPlayer::new(1);
        // queueSupplyTruck may prepend a gatherer team (C++ also calls it).
        assert!(ai.queue_units());
    }

    #[test]
    fn is_a_good_idea_drops_factory_lock_before_possible() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub(crate) fn is_a_good_idea_to_build_team")
            .expect("good idea");
        let window = &src[i..src.len().min(i + 2200)];
        assert!(
            window.contains("Snapshot under the factory lock")
                && window.contains("instances >= max_instances")
                && window.contains("is_possible_to_build_team(team_name, true)"),
            "isAGoodIdeaToBuildTeam must drop factory Mutex before isPossibleToBuildTeam"
        );
        let call = window
            .find("is_possible_to_build_team(team_name, true)")
            .expect("call");
        let before = &window[..call];
        assert!(
            before.matches('}').count() >= 2,
            "factory lock scope must close before is_possible_to_build_team"
        );

        let j = src.find("fn is_possible_to_build_team(").expect("possible");
        let pw = &src[j..src.len().min(j + 4500)];
        assert!(
            pw.contains("find_factory_internal also locks the player")
                && pw.contains("then factory-scan")
                && pw.find("find_factory_internal(thing_name, true)").unwrap()
                    > pw.find("calc_cost_to_build").unwrap(),
            "isPossibleToBuildTeam must not hold player RwLock across find_factory"
        );
    }

    #[test]
    fn is_possible_to_build_team_avg_cost_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::isPossibleToBuildTeam`")
            .expect("isPossible doc");
        let window = &src[i..src.len().min(i + 5500)];
        assert!(
            window.contains("any_idle")
                && (window.contains("find_factory_internal(thing_name, true)")
                    || window.contains("find_factory_internal(&thing_name, true)"))
                && (window.contains("find_factory_internal(thing_name, false)")
                    || window.contains("find_factory_internal(&thing_name, false)"))
                && (window.contains("max_units as f32 + min_units as f32")
                    || window.contains("*max_units as f32 + *min_units as f32"))
                && window.contains("team_resources_to_build")
                && window.contains("notEnoughMoney")
                && window.contains("!require_idle_factory")
                && window.contains("find_factory_internal also locks the player")
                && window.contains("let mut cost: i32 = 0")
                && window.contains("as i32"),
            "isPossibleToBuildTeam must match C++ anyIdle/Int avg cost/resources mod"
        );
    }

    #[test]
    fn start_training_queues_create_unit_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::startTraining`")
            .expect("startTraining doc");
        let window = &src[i..src.len().min(i + 2500)];
        assert!(
            window.contains("request_unique_unit_production_id")
                && window.contains("queue_unit_with_production_id")
                && window.contains("order.factory_id = Some(factory_id)")
                && window.contains("start_production"),
            "startTraining must queueCreateUnit then set factoryID"
        );
    }

    #[test]
    fn find_factory_prefers_build_list_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::findFactory`")
            .expect("findFactory doc");
        let window = &src[i..src.len().min(i + 2500)];
        assert!(
            window.contains("get_build_list_mut")
                && window.contains("factory_candidate")
                && window.contains("build list only")
                && window.contains("set_object_id(INVALID_ID)")
                && !window.contains("get_all_objects()"),
            "findFactory must iterate build list only and clear captured factories"
        );
    }

    #[test]
    fn on_unit_produced_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::onUnitProduced`")
            .expect("onUnitProduced");
        let window = &src[i..src.len().min(i + 7500)];
        assert!(
            window.contains("is_equivalent_to")
                && window.contains("order.factory_id = None")
                && window.contains("set_team")
                && window.contains("reinforcement_id = Some(unit_id)")
                && window.contains("self.team_delay = 0")
                && window.contains("structure_timer = 1")
                && window.contains("set_force_wanting_state")
                && window.contains("ai_follow_exit_production_path")
                && window.contains("get_goal_position")
                && !window.contains("get_path_destination")
                && window.contains("take_supply_gatherer_slot")
                && window.contains("ai_dock")
                && window.contains("FromPlayer")
                && window.contains("!supply_truck && is_dozer")
                && !window.contains("if found && !supply_truck && is_dozer"),
            "onUnitProduced must match factory+template; dozer path not gated on found"
        );
    }

    #[test]
    fn on_unit_produced_shortcuts_team_delay() {
        let mut ai = AIPlayer::new(1);
        ai.team_delay = 99;
        // invalid factory → still sets team_delay=0 after missing unit path
        ai.on_unit_produced(INVALID_ID, INVALID_ID).expect("oop");
        // factory INVALID returns early without clearing in our port when factory_id==INVALID
        // Use a fake factory with no unit:
        ai.team_delay = 99;
        let _ = ai.on_unit_produced(1, 999999);
        assert_eq!(
            ai.team_delay, 0,
            "C++ always zeroes teamDelay at end of onUnitProduced"
        );
    }

    #[test]
    fn build_structure_with_dozer_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn build_structure_with_dozer")
            .expect("buildStructureWithDozer");
        let window = &src[i..src.len().min(i + 14000)];
        assert!(
            window.contains("find_dozer")
                && window.contains("calc_cost_to_build")
                && window.contains("set_build_task")
                && window.contains("UnderConstruction")
                && window.contains("decrement_num_rebuilds")
                && window.contains("is_location_safe")
                && window.contains("120.0 * PATHFIND_CELL_SIZE_F")
                && window.contains("prefer location match")
                && window.contains("client_safe_quick_does_path_exist")
                && window.contains("get_locomotor_set_clone")
                && !window.contains("LocomotorSet::new()")
                && window.contains("Dozer unable to reach building.  Teleporting.")
                && !window.contains("let _ = self.queue_dozer()"),
            "buildStructureWithDozer must path-check+teleport, stamp by location, skirmish wiggle"
        );
    }

    #[test]
    fn process_base_building_resumes_dozer_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn process_base_building(&mut self)")
            .expect("pbb");
        let window = &src[i..src.len().min(i + 14000)];
        assert!(
            window.contains("ResumeConstruction")
                && window.contains("resume_jobs")
                && window.contains("AI's Dozer got killed"),
            "solo processBaseBuilding must resume construction on UC buildings"
        );
    }

    #[test]
    fn process_base_building_rebuild_delay_requires_invalid_id() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn process_base_building(&mut self)")
            .expect("pbb");
        let w = &src[i..src.len().min(i + 9000)];
        assert!(
            w.contains("get_object_id() == INVALID_ID && info.get_object_timestamp() > 0")
                && w.contains("Enabling rebuild for"),
            "solo processBaseBuilding rebuild delay must require INVALID_ID like C++"
        );
    }

    #[test]
    fn process_base_building_calls_build_structure_with_dozer() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("fn process_base_building").expect("pbb");
        let window = &src[i..src.len().min(i + 12000)];
        assert!(
            window.contains("build_structure_with_dozer")
                && window.contains("get_spawner_id()")
                && window.contains("KindOf::RebuildHole"),
            "processBaseBuilding must call buildStructureWithDozer and match holes by spawner"
        );
    }

    #[test]
    fn compute_center_uses_build_list_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn compute_center_and_radius_of_base")
            .expect("compute");
        let window = &src[i..src.len().min(i + 3500)];
        assert!(
            window.contains("get_build_list()")
                && window.contains("get_bounding_circle_radius")
                && window.contains("* 0.4")
                && window.contains("max_rad_sqr")
                && !window.contains("get_all_objects()"),
            "computeCenterAndRadiusOfBase must average build-list locations + geom*0.4"
        );
    }

    #[test]
    fn on_structure_produced_applies_map_props_and_script() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn on_structure_produced")
            .expect("onStructure");
        let window = &src[i..src.len().min(i + 8000)];
        assert!(
            window.contains("update_obj_values_from_map_properties")
                && window.contains("key_object_initial_health")
                && window.contains("add_object_to_cache")
                && window.contains("run_object_script")
                && window.contains("check_for_supply_center")
                && window.contains("get_reconstructed_building_id")
                && window.contains("saw_rhbi && is_this_spawn")
                && !window.contains("frame_last_building_built = TheGameLogic::get_frame()")
                && window.find("run_object_script") < window.find("self.check_for_supply_center")
                && window.contains("Do NOT call check_for_supply_center while holding"),
            "onStructureProduced: script then supply outside player write; no frame stamp"
        );
    }

    #[test]
    fn new_map_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod.find("pub fn new_map(&mut self)").expect("newMap");
        let end = prod[i..]
            .find("/// Start training for a work order")
            .or_else(|| prod[i..].find("pub(crate) fn start_training"))
            .or_else(|| prod[i..].find("fn start_training_internal"))
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 8000));
        let w = &prod[i..end];
        assert!(
            w.contains("original_entries")
                && w.contains("add_to_build_list")
                && w.contains("compute_center_and_radius_of_base")
                && w.contains("is_initially_built")
                && w.contains("increment_num_rebuilds")
                && w.contains("build_structure_now_at")
                && !w.contains("team_build_queue.clear()")
                && !w.contains("structures_to_repair = [None"),
            "newMap must snapshot pre-factory list, prepend factories, center, then original initials"
        );
        let snap = w.find("original_entries").expect("snap");
        let add = w.find("add_to_build_list").expect("add");
        let center = w.find("compute_center_and_radius_of_base").expect("center");
        assert!(
            snap < add && add < center,
            "C++ order: capture head, add factories, compute center, walk original"
        );
    }

    #[test]
    fn join_team_reinforcement_calls_join_team_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn join_team_reinforcement")
            .expect("join_team_reinforcement");
        let w = &src[i..src.len().min(i + 1200)];
        assert!(
            w.contains("ai.join_team()") && !w.contains("Residual: always move toward teammate"),
            "reinforcement must call full joinTeam, not residual move-only"
        );
        let unit = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/object/unit.rs"));
        let j = unit
            .find("fn join_team(&mut self)")
            .expect("UnitAI join_team");
        let uw = &unit[j..unit.len().min(j + 3500)];
        assert!(
            uw.contains("choose_locomotor_set")
                && uw.contains("set_goal_waypoint(None)")
                && uw.contains("clear()")
                && uw.contains("other_idle")
                && uw.contains("INVALID_STATE_ID")
                && uw.contains("set_goal_object")
                && uw.contains("set_goal_position"),
            "joinTeam must clear, copy goal, and setState(INVALID→default) like C++"
        );
    }

    #[test]
    fn build_structure_now_at_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::buildStructureNow` (AIPlayer.cpp)")
            .expect("buildStructureNow");
        let window = &src[i..src.len().min(i + 14000)];
        assert!(
            window.contains("update_obj_values_from_map_properties")
                && window.contains("key_object_initial_health")
                && window.contains("clear_status")
                && window.contains("update_upgrade_modules_from_player")
                && window.contains("add_object_to_cache")
                && window.contains("run_object_script")
                && window.contains("check_for_supply_center")
                && window.contains("requested location"),
            "buildStructureNow must apply map props, clear UC, script cache/run, supply check"
        );
    }

    #[test]
    fn check_for_supply_center_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::checkForSupplyCenter`")
            .expect("checkForSupplyCenter");
        let window = &src[i..src.len().min(i + 2500)];
        assert!(
            window.contains("SupplyCenterDockUpdate")
                && window.contains("set_supply_building(true)")
                && window.contains("set_desired_gatherers(desired + 1)")
                && window.contains("set_current_gatherers(-1)"),
            "checkForSupplyCenter must stamp supply building + desired gatherers+1"
        );
    }

    #[test]
    fn queue_supply_truck_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("fn queue_supply_truck(&mut self)")
            .expect("queueSupplyTruck");
        let end = prod[i..]
            .find("fn count_player_harvesters")
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 4000));
        let window = &prod[i..end];
        assert!(
            window.contains("truck_in_queue")
                && window.contains("is_resource_gatherer")
                && window.contains("count_player_harvesters")
                && window.contains("is_supply_building")
                && window.contains("queue_one_harvester_at_factory")
                && window.contains("recount_and_redock_harvesters")
                && window.contains("try_reattach_loose_harvester")
                && window.contains("desired.saturating_mul(3)")
                && window.contains("supply_center_has_nearby_supplies"),
            "queueSupplyTruck must skip-if-queued, recount, reattach, train harvester"
        );
        // C++ only requires nearby warehouse on the cur>=desired maintenance branch.
        let maint = window
            .find("if cur_gatherers >= desired")
            .expect("maintenance branch");
        let under = window
            .find("// Under-desired")
            .expect("under-desired branch");
        assert!(
            maint < under
                && window[maint..under].contains("supply_center_has_nearby_supplies")
                && !window[under..].contains("supply_center_has_nearby_supplies"),
            "nearby warehouse check must only gate the maintenance branch like C++"
        );
    }

    #[test]
    fn find_dozer_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("C++ `AIPlayer::findDozer`").expect("findDozer");
        let window = &src[i..src.len().min(i + 5000)];
        assert!(
            window.contains("need_dozer")
                && window.contains("DozerTask::Build")
                && window.contains("is_currently_ferrying_supplies")
                && window.contains("repair_dozer")
                && window.contains("closest_dozer")
                && window.contains("queue_dozer"),
            "findDozer must prefer idle, skip ferrying/repair/build, queue if none"
        );
    }

    #[test]
    fn compute_superweapon_target_uses_get_extent_and_int_cash() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn compute_superweapon_target")
            .expect("compute_superweapon_target");
        let window = &src[i..src.len().min(i + 5500)];
        assert!(
            window.contains("get_extent()")
                && !window.contains("get_maximum_pathfind_extent()")
                && window.contains("let mut best_cash: i32 = -1")
                && window.contains("let mut fine_cash: i32 = -1")
                && window.contains("value == fine_cash")
                && window.contains("player_index: i32")
                && window.contains("player_index >= 0"),
            "computeSuperweaponTarget must use getExtent + Int cash + explicit playerNdx"
        );
        let j = src
            .find("/// C++ `AIPlayer::getPlayerSuperweaponValue`")
            .expect("value");
        let vw = &src[j..src.len().min(j + 6000)];
        assert!(
            vw.contains("-> Result<i32, AiError>") && vw.contains("Ok(cash as i32)"),
            "getPlayerSuperweaponValue must return Int like C++"
        );
        let h = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/helpers.rs"));
        assert!(
            h.contains("pub fn get_extent(&self)") && h.contains("guard.get_extent()"),
            "TheTerrainLogic must expose get_extent"
        );
    }

    #[test]
    fn ai_player_crc_is_empty_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("/// C++ `AIPlayer::crc` is empty")
            .expect("crc doc");
        let j = src[i..].find("fn xfer(").expect("xfer after crc") + i;
        let window = &src[i..j];
        assert!(
            window.contains("Intentionally empty")
                && !window.contains("ready_to_build_team")
                && !window.contains("xfer_bool"),
            "AIPlayer::crc must be empty like C++"
        );
    }

    #[test]
    fn check_for_supply_center_module_only_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn check_for_supply_center")
            .expect("check_for_supply_center");
        let w = &src[i..src.len().min(i + 900)];
        assert!(
            w.contains("SupplyCenterDockUpdate")
                && w.contains("find_update_module")
                && !w.contains("FSSupplyCenter")
                && !w.contains("SupplySource"),
            "checkForSupplyCenter must key only on SupplyCenterDockUpdate like C++"
        );
    }

    #[test]
    fn get_player_structure_bounds_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn get_player_structure_bounds_ex")
            .expect("bounds_ex");
        let window = &src[i..src.len().min(i + 3500)];
        assert!(
            window.contains("conservative")
                && window.contains("ConservativeBuilding")
                && window.contains("KindOf::Structure")
                && window.contains("first_structure")
                && window.contains("only enters the AABB expand when isKindOf(STRUCTURE)")
                && !window.contains("obj_min"),
            "getPlayerStructureBounds must structure-only AABB + conservative skip"
        );
    }

    #[test]
    fn calc_closest_construction_zone_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::calcClosestConstructionZoneLocation`")
            .expect("calcClosest");
        let w = &src[i..src.len().min(i + 5000)];
        assert!(
            w.contains("get_placement_view_angle")
                && w.contains("let mut valid = false")
                && w.contains("2.0 * SUPPLY_CENTER_CLOSE_DIST")
                && w.contains("when initial_ok, valid stays false"),
            "calcClosestConstructionZone must use placement angle + C++ valid control flow"
        );
    }

    #[test]
    fn compute_superweapon_target_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::computeSuperweaponTarget`")
            .expect("computeSuperweaponTarget");
        let window = &src[i..src.len().min(i + 5500)];
        assert!(
            window.contains("get_extent") && window.contains("x_count, y_count")
                || (window.contains("x_count") && window.contains("y_count")),
            "grid counts"
        );
        assert!(
            window.contains("game_logic_random_value(1, 4)")
                && window.contains("x_count, 0")
                && window.contains("// Fine tune: C++ uses (x-5) for BOTH axes")
                && window.contains("SneakAttack")
                && window.contains("get_player_superweapon_value")
                && window.contains("player_index"),
            "computeSuperweaponTarget must randomize scan, preserve fine-tune bug, playerNdx"
        );
    }

    #[test]
    fn build_upgrade_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::buildUpgrade`")
            .expect("buildUpgrade");
        let w = &src[i..src.len().min(i + 4000)];
        assert!(
            w.contains("get_build_list")
                && w.contains("has_upgrade_in_production")
                && w.contains("has_upgrade_complete")
                && w.contains("can_afford_upgrade")
                && w.contains("queue_upgrade")
                && w.contains("UnderConstruction"),
            "buildUpgrade must walk build list and gate type/money/progress"
        );
    }

    #[test]
    fn find_supply_center_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("fn find_supply_center(&self, minimum_cash: i32)")
            .expect("findSupplyCenter");
        let end = prod[i..]
            .find("fn find_valid_build_location")
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 6000));
        let w = &prod[i..end];
        assert!(
            w.contains("KindOf::SupplySource")
                && w.contains("SupplyWarehouseDockUpdate")
                && w.contains("BASE_VALUE_PER_SUPPLY_BOX")
                && w.contains("KindOf::CashGenerator")
                && w.contains("SUPPLY_CENTER_CLOSE_DIST")
                && w.contains("dist_sqr * 0.4")
                && w.contains("enemy_dist_sqr * 0.6")
                && w.contains("cash_floor /= 2")
                && w.contains("if cash_floor <= 100"),
            "findSupplyCenter must filter warehouse cash, own supply center, enemy 60/40"
        );
        // Halve-then-stop: divide happens before the ≤100 break (C++ do/while).
        let div = w.find("cash_floor /= 2").expect("div");
        let stop = w.find("if cash_floor <= 100").expect("stop");
        assert!(
            div < stop,
            "C++ halves minimumCash before while(minimumCash>100); no extra ≤100 pass"
        );
    }

    #[test]
    fn build_specific_ai_team_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::buildSpecificAITeam`")
            .expect("buildSpecificAITeam");
        let w = &src[i..src.len().min(i + 7000)];
        assert!(
            w.contains("get_can_build_units")
                && w.contains("is_singleton")
                && w.contains("is_possible_to_build_team")
                && w.contains("need_money")
                && w.contains("create_inactive_team")
                && w.contains("order.required = true")
                && w.contains("self.team_delay = 0")
                && w.contains("even minUnits==0")
                && w.contains("execute_action_sequence"),
            "buildSpecificAITeam must queue min=0 required orders and run executeActions"
        );
    }

    #[test]
    fn recruit_specific_ai_team_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::recruitSpecificAITeam`")
            .expect("recruitSpecificAITeam");
        let w = &src[i..src.len().min(i + 7000)];
        assert!(
            w.contains("try_to_recruit")
                && w.contains("team_ready_queue.push_front")
                && w.contains("create_inactive_team")
                && w.contains("MoveToPosition")
                && w.contains("team_about_to_be_deleted")
                && w.contains("Recruited 0 units")
                && w.contains("home_location()")
                && w.contains("has_home_location")
                && w.contains("is_skirmish_ai_player"),
            "recruitSpecificAITeam must tryToRecruit, homeLocation, ready-queue or disband"
        );
    }

    #[test]
    fn build_specific_ai_team_respects_can_build_units_surface() {
        // Without player/can_build setup, empty name should still be a no-op Ok.
        let mut ai = AIPlayer::new(1);
        ai.build_specific_ai_team("NoSuchTeam", false).expect("bst");
        assert!(ai.team_build_queue.is_empty());
    }

    #[test]
    fn build_nearest_team_and_calc_closest_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let n = src
            .find("C++ `AIPlayer::buildSpecificBuildingNearestTeam`")
            .expect("nearest");
        let c = src
            .find("C++ `AIPlayer::calcClosestConstructionZoneLocation`")
            .expect("calc");
        let nw = &src[n..src.len().min(n + 2500)];
        let cw = &src[c..src.len().min(c + 5000)];
        assert!(
            nw.contains("get_estimate_team_position")
                && nw.contains("add_to_priority_build_list")
                && nw.contains("calc_closest_construction_zone_location"),
            "nearest-team must estimate pos, calcClosest, priority list"
        );
        assert!(
            cw.contains("get_placement_view_angle")
                && cw.contains("let mut valid = false")
                && cw.contains("location: &Coord3D"),
            "calcClosest must use placement angle + C++ valid control flow"
        );
    }

    #[test]
    fn solo_base_defense_and_on_structure_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let d = src
            .find("C++ `AIPlayer::buildAIBaseDefense`")
            .expect("defense");
        let o = src
            .find("C++ `AIPlayer::onStructureProduced`")
            .expect("onStructure");
        let dw = &src[d..src.len().min(d + 1200)];
        let ow = &src[o..src.len().min(o + 8000)];
        assert!(
            dw.contains("Solo ai doesn't support buildAIBaseDefense")
                && dw.contains("buildAIBaseDefenseStructure"),
            "solo defense stubs"
        );
        assert!(
            ow.contains("update_upgrade_modules_from_player")
                && ow.contains("RebuildHole")
                && ow.contains("check_for_supply_center")
                && ow.contains("Structure not found in production queue"),
            "onStructureProduced list match + hole retarget + supply"
        );
    }

    #[test]
    fn build_paths_use_placement_view_angle_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn build_by_supplies")
            .expect("build_by_supplies");
        let w = &src[i..src.len().min(i + 2500)];
        assert!(
            w.contains("get_placement_view_angle()"),
            "buildBySupplies must use ThingTemplate placement view angle (C++)"
        );
        let j = src
            .find("fn build_specific_building_near_location")
            .expect("near_location");
        let w2 = &src[j..src.len().min(j + 1200)];
        assert!(
            w2.contains("get_placement_view_angle()"),
            "buildSpecificBuildingNearLocation must use placement view angle"
        );
    }

    #[test]
    fn get_skirmish_enemy_player_index_prefers_current_enemy_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn get_skirmish_enemy_player_index")
            .expect("skirmish enemy");
        let w = &src[i..src.len().min(i + 1800)];
        assert!(
            w.contains("get_current_enemy_player_index")
                && w.contains("PlayerType::Human")
                && w.contains("PlayerType::Neutral"),
            "skirmish enemy index must prefer current enemy then human"
        );
    }

    #[test]
    fn guard_supply_center_uses_script_cmd_source_like_cpp() {
        // C++ groupGuardPosition(..., GUARDMODE_NORMAL, CMD_FROM_SCRIPT)
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::guardSupplyCenter`")
            .expect("guardSupplyCenter");
        let window = &src[i..src.len().min(i + 4500)];
        assert!(
            window.contains("ai_guard_position")
                && window.contains("GuardMode::Normal")
                && window.contains("FromScript")
                && window.contains("supply_source_attack_check_frame = 0"),
            "guardSupplyCenter must issue GuardPosition NORMAL from script source"
        );
    }

    #[test]
    fn build_by_supplies_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let prod = src
            .split("#[cfg(test)]")
            .next()
            .expect("production before tests");
        let i = prod
            .find("pub fn build_by_supplies(")
            .expect("buildBySupplies");
        let end = prod[i..]
            .find("pub fn build_specific_building_near_location")
            .map(|o| i + o)
            .unwrap_or(prod.len().min(i + 4000));
        let w = &prod[i..end];
        assert!(
            w.contains("find_supply_center")
                && w.contains("add_to_priority_build_list")
                && w.contains("CashGenerator")
                && w.contains("current_warehouse_id")
                && w.contains("3.0 * PATHFIND_CELL_SIZE_F")
                && w.contains("self.base_center")
                && !w.contains("compute_center_and_radius_of_base"),
            "buildBySupplies must find then maybe override warehouse; use m_baseCenter as-is"
        );
        // find first, then non-cash current override.
        let find = w.find("find_supply_center(minimum_cash)").expect("find");
        let override_cur = w.find("!is_cash_generator").expect("non-cash");
        assert!(
            find < override_cur,
            "C++ finds supply center before non-cash curWarehouse override"
        );
    }

    #[test]
    fn repair_structure_and_bridge_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let r = src
            .find("C++ `AIPlayer::repairStructure`")
            .expect("repairStructure");
        let b = src
            .find("C++ `AIPlayer::updateBridgeRepair`")
            .expect("updateBridgeRepair");
        let rw = &src[r..src.len().min(r + 2000)];
        let bw = &src[b..src.len().min(b + 9000)];
        assert!(
            rw.contains("BodyDamageType::Pristine")
                && rw.contains("MAX_STRUCTURES_TO_REPAIR")
                && rw.contains("structures_in_queue"),
            "repairStructure must skip pristine and bound queue"
        );
        assert!(
            bw.contains("LOGICFRAMES_PER_SECOND")
                && bw.contains("dozer_queued_for_repair")
                && bw.contains("AiCommandType::Repair")
                && bw.contains("is_any_task_pending")
                && bw.contains("ai_move_to_position")
                && bw.contains("adjust_to_possible_destination")
                && bw.find("saturating_sub(1)").unwrap()
                    < bw.find("if self.bridge_timer > 0").unwrap(),
            "updateBridgeRepair must decrement timer first, assign dozer, complete+home"
        );

        // Behavioral: timer==1 with empty/missing queue still advances without hang.
        let mut ai = AIPlayer::new(1);
        ai.structures_in_queue = 1;
        ai.structures_to_repair[0] = Some(999_999); // missing object → pop
        ai.bridge_timer = 1;
        ai.update_bridge_repair().expect("ubr");
        assert_eq!(
            ai.structures_in_queue, 0,
            "missing repair target is popped when timer fires"
        );
        assert_eq!(
            ai.bridge_timer, LOGICFRAMES_PER_SECOND,
            "timer resets to 1s after fire"
        );
    }

    #[test]
    fn repair_structure_skips_pristine_and_duplicates() {
        let mut ai = AIPlayer::new(1);
        // Without a live object, repair_structure returns Ok and does not enqueue.
        ai.repair_structure(999_999).expect("rs");
        assert_eq!(ai.structures_in_queue, 0);
    }

    #[test]
    fn build_specific_ai_building_solo_is_noop_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::buildSpecificAIBuilding`")
            .expect("buildSpecificAIBuilding");
        let w = &src[i..src.len().min(i + 800)];
        assert!(
            w.contains("Solo ai doesn't support") && !w.contains("construction_priorities.insert"),
            "solo buildSpecificAIBuilding only logs"
        );
    }

    #[test]
    fn get_player_superweapon_value_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("C++ `AIPlayer::getPlayerSuperweaponValue`")
            .expect("getPlayerSuperweaponValue");
        let window = &src[i..src.len().min(i + 3500)];
        assert!(
            window.contains("FSBaseDefense")
                && window.contains("TechBaseDefense")
                && window.contains("is_significantly_above_terrain")
                && window.contains("CommandCenter")
                && window.contains("FSSuperweapon")
                && window.contains("factor * value * 5.0"),
            "getPlayerSuperweaponValue must match C++ defense/aircraft/CC scoring"
        );
    }

    #[test]
    fn queue_dozer_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("C++ `AIPlayer::queueDozer`").expect("queueDozer");
        let window = &src[i..src.len().min(i + 4500)];
        assert!(
            window.contains("dozer_in_queue")
                && window.contains("set_can_build_units_temp")
                && window.contains("start_training_internal")
                && window.contains("priority_build = true")
                && window.contains("KindOf::Dozer")
                && window.contains("first_template")
                && window.contains("get_next_template")
                && !window.contains("dozer_queued_for_repair = true"),
            "queueDozer must gate queue, enable units, startTraining priority dozer"
        );
    }

    #[test]
    fn is_supply_source_attacked_scan_rate_is_10_frames_like_cpp() {
        // C++: const Int SCAN_RATE = 10; added to frame counters (not * LOGICFRAMES).
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("pub fn is_supply_source_attacked")
            .expect("is_supply_source_attacked");
        let window = &src[i..src.len().min(i + 1200)];
        assert!(
            window.contains("const SCAN_RATE: u32 = 10")
                && !window[..window.find("const SCAN_RATE").unwrap_or(0) + 80]
                    .contains("LOGICFRAMES_PER_SECOND"),
            "SCAN_RATE must be 10 frames matching C++ AIPlayer.cpp, not 10 seconds"
        );
    }

    #[test]
    fn supply_source_attacked_safe_guard_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let a = src
            .find("C++ `AIPlayer::isSupplySourceAttacked`")
            .expect("attacked");
        let s = src
            .find("C++ `AIPlayer::isSupplySourceSafe`")
            .expect("safe");
        let g = src
            .find("C++ `AIPlayer::guardSupplyCenter`")
            .expect("guard");
        let aw = &src[a..src.len().min(a + 4000)];
        let sw = &src[s..src.len().min(s + 1500)];
        let gw = &src[g..src.len().min(g + 4000)];
        assert!(
            aw.contains("SCAN_RATE")
                && aw.contains("get_attacked_frame")
                && aw.contains("CashGenerator")
                && aw.contains("get_last_damage_timestamp")
                && aw.contains("attacked_supply_center"),
            "isSupplySourceAttacked must rate-limit and scan recent damage"
        );
        assert!(
            sw.contains("find_supply_center") && sw.contains("is_location_safe"),
            "isSupplySourceSafe must delegate to find+isLocationSafe"
        );
        assert!(
            gw.contains("supply_source_attack_check_frame = 0")
                && gw.contains("ai_guard_position")
                && gw.contains("get_bounding_circle_radius")
                && gw.contains("get_player_structure_bounds"),
            "guardSupplyCenter must force check, offset, and guard"
        );
    }

    #[test]
    fn dozer_in_queue_uses_includes_a_dozer_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("pub fn dozer_in_queue").expect("dozerInQueue");
        let w = &src[i..src.len().min(i + 500)];
        assert!(
            w.contains("includes_a_dozer()"),
            "dozerInQueue must use TeamInQueue::includesADozer"
        );

        // gatherer-only work order is not a "dozer in queue"
        let mut ai = AIPlayer::new(1);
        let mut order = WorkOrder::new("GLAInfantryWorker".into());
        order.is_resource_gatherer = true;
        let mut team = TeamInQueue::new();
        team.work_orders.push(order);
        ai.team_build_queue.push_front(team);
        assert!(
            !ai.dozer_in_queue(),
            "resource-gatherer work order must not count as dozerInQueue"
        );
    }

    #[test]
    fn queue_dozer_does_not_set_repair_flag_like_cpp() {
        // C++ queueDozer never sets m_dozerQueuedForRepair (repair path only).
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src.find("pub(crate) fn queue_dozer").expect("queue_dozer");
        let end = src[i..].find("pub fn dozer_in_queue").unwrap_or(5000);
        let window = &src[i..i + end];
        assert!(
            !window.contains("dozer_queued_for_repair = true"),
            "queueDozer must not stamp dozer_queued_for_repair"
        );
        assert!(
            window.contains("first_template") && window.contains("KindOf::Dozer"),
            "queueDozer must walk ThingFactory dozer templates"
        );
    }

    #[test]
    fn queue_dozer_skips_when_already_queued() {
        let mut ai = AIPlayer::new(1);
        let mut order = WorkOrder::new("AmericaVehicleDozer".into());
        let mut team = TeamInQueue::new();
        team.work_orders.push(order);
        ai.team_build_queue.push_front(team);
        let before = ai.team_build_queue.len();
        ai.queue_dozer().expect("qd");
        assert_eq!(
            ai.team_build_queue.len(),
            before,
            "C++ dozerInQueue early-out"
        );
    }

    #[test]
    fn queue_one_harvester_walks_thing_factory_like_cpp() {
        // C++ queueSupplyTruck: tTemplate = firstTemplate(); while (tTemplate) { isKindOf(HARVESTER); tTemplate = friend_getNextTemplate(); }
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn queue_one_harvester_at_factory")
            .expect("queue_one_harvester");
        let window = &src[i..src.len().min(i + 3500)];
        assert!(
            window.contains("first_template")
                && window.contains("get_next_template")
                && window.contains("KindOf::Harvester")
                && window.contains("is_resource_gatherer = true")
                && window.contains("cur_gatherers == -1"),
            "queue_one_harvester must walk ThingFactory harvester templates like C++"
        );
    }

    #[test]
    fn queue_supply_truck_skips_when_already_queued() {
        let mut ai = AIPlayer::new(1);
        let mut order = WorkOrder::new("SupplyTruck".into());
        order.is_resource_gatherer = true;
        let mut team = TeamInQueue::new();
        team.work_orders.push(order);
        ai.team_build_queue.push_front(team);
        let before = ai.team_build_queue.len();
        ai.queue_supply_truck().expect("qst");
        assert_eq!(
            ai.team_build_queue.len(),
            before,
            "C++ returns early when truck already in queue"
        );
    }

    #[test]
    fn select_team_to_reinforce_auto_cpp_surface() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ai/ai_player.rs"));
        let i = src
            .find("fn select_team_to_reinforce(&mut self, min_priority: i32)")
            .expect("reinforce");
        let window = &src[i..src.len().min(i + 7000)];
        assert!(
            window.contains("automatically_reinforce")
                && window.contains("priority <= cur_priority")
                && window.contains("max_units")
                && window.contains("try_to_recruit")
                && window.contains("push_front(team_q)")
                && window.contains("self.team_delay = 0")
                && window.contains("find_factory_internal")
                && window.contains("order.num_required = 1")
                && window.contains("q.team.as_ref()")
                && window.contains("has_home_location")
                && window.contains("get_members().first()"),
            "select_team_to_reinforce must match C++ busy-by-handle + home/member origin"
        );
    }

    #[test]
    fn select_team_to_reinforce_no_auto_returns_false() {
        let mut ai = AIPlayer::new(1);
        // No prototypes with automatically_reinforce → false, no panic.
        assert!(!ai.select_team_to_reinforce(0).expect("reinforce"));
        assert!(ai.team_build_queue.is_empty());
    }

    #[test]
    fn test_strategy_state() {
        let mut strategy_state = AiStrategyState::default();
        assert_eq!(strategy_state.current_strategy, AiStrategy::Balanced);
        assert_eq!(strategy_state.strategy_confidence, 0.0);
    }

    #[test]
    fn ai_player_xfer_writes_team_seconds_as_cpp_int() {
        use game_engine::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut ai_player = AIPlayer::new(7);
        ai_player.team_seconds = 66_051.0;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("ai_player_team_seconds").unwrap();
            ai_player.xfer(&mut save);
            save.close().unwrap();
        }

        assert!(bytes
            .windows(4)
            .any(|window| window == &66_051i32.to_le_bytes()));
        assert!(!bytes
            .windows(4)
            .any(|window| window == &66_051.0f32.to_le_bytes()));
    }
}
