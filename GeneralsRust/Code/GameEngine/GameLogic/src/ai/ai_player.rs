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
use crate::common::{
    AsciiString, ControlBarInterface, Coord2D, Coord3D, KindOf, ObjectID, ObjectStatusMaskType,
    ObjectStatusTypes, PlayerId, Real, Relationship, TeamId, ThingTemplate, UnsignedInt,
};
use crate::control_bar::get_control_bar_bridge;
use crate::helpers::{
    game_logic_random_value, TheGameLogic, ThePartitionManager, TheTerrainLogic, TheThingFactory,
};
use crate::modules::ProductionUpdateInterface;
use crate::object::production::build_cost_calculator::{BuildCostCalculator, PlayerBuildModifiers};
use crate::object::production::construction::FoundationValidator;
use crate::object::production::supply_warehouse_dock::{
    SupplyWarehouseDockUpdate, SupplyWarehouseDockUpdateModule,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::{player_list, GameDifficulty, Player, PlayerType};
use crate::supply_system::BASE_VALUE_PER_SUPPLY_BOX;
use crate::team::get_team_factory;
use crate::upgrade::center::with_upgrade_center;
use crate::upgrade::template::UpgradeType;
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

/// Default delay between team production in seconds (C++ m_teamSeconds)
pub const DEFAULT_TEAM_SECONDS: f32 = 2.0;

/// Default delay between structure production in seconds (C++ m_structureSeconds)
pub const DEFAULT_STRUCTURE_SECONDS: f32 = 10.0;

/// Resource threshold for "poor" AI (C++ m_resourcesPoor)
pub const RESOURCES_POOR: i32 = 2000;

/// Resource threshold for "wealthy" AI (C++ m_resourcesWealthy)
pub const RESOURCES_WEALTHY: i32 = 5000;

/// Build speed multiplier when poor (C++ m_structuresPoorMod)
pub const STRUCTURES_POOR_MODIFIER: f32 = 2.0;

/// Build speed multiplier when wealthy (C++ m_structuresWealthyMod)
pub const STRUCTURES_WEALTHY_MODIFIER: f32 = 2.0;

/// Team build speed multiplier when poor (C++ m_teamsPoorMod)
pub const TEAMS_POOR_MODIFIER: f32 = 2.0;

/// Team build speed multiplier when wealthy (C++ m_teamsWealthyMod)
pub const TEAMS_WEALTHY_MODIFIER: f32 = 2.0;

/// Delay before rebuilding destroyed structure in seconds (C++ m_rebuildDelaySeconds)
pub const REBUILD_DELAY_SECONDS: u32 = 5;

/// Team resource multiplier for affordability check (C++ m_teamResourcesToBuild)
pub const TEAM_RESOURCES_TO_BUILD: f32 = 0.5;

/// Supply center safe radius in units (C++ m_supplyCenterSafeRadius)
pub const SUPPLY_CENTER_SAFE_RADIUS: f32 = 100.0;

/// Skirmish base defense extra distance (C++ m_skirmishBaseDefenseExtraDistance)
pub const SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE: f32 = 50.0;

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

    /// Validate that factory ID still refers to an active object
    pub fn validate_factory(&mut self, _player_id: u32) -> Result<(), AiError> {
        if let Some(_factory_id) = self.factory_id {
            // Check if factory still exists and is owned by player
            // This would use your object management system
            // For now, we'll assume it's valid
        }
        Ok(())
    }
}

/// Team in the build/ready queue
#[derive(Debug)]
pub struct TeamInQueue {
    pub work_orders: Vec<WorkOrder>,  // List of work orders for this team
    pub priority_build: bool,         // True if specifically requested
    pub team_name: Option<String>,    // Team that units go into
    pub frame_started: u32,           // Frame we started building
    pub sent_to_start_location: bool, // Has team been sent to start location
    pub stop_queueing: bool,          // True to stop building new units
    pub reinforcement: bool,          // True if reinforcing existing team
    pub reinforcement_id: Option<ObjectID>, // Object being reinforced
}

impl TeamInQueue {
    pub fn new() -> Self {
        Self {
            work_orders: Vec::new(),
            priority_build: false,
            team_name: None,
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

    /// Returns true if minimum required units have been built
    pub fn is_minimum_built(&self) -> bool {
        self.work_orders
            .iter()
            .filter(|order| order.required)
            .all(|order| order.num_completed >= order.num_required)
    }

    /// Returns true if team includes a dozer unit
    pub fn includes_a_dozer(&self) -> bool {
        self.work_orders.iter().any(|order| {
            order.thing_template.contains("dozer") || order.thing_template.contains("worker")
        })
    }

    /// Returns true if all factory builds are complete
    pub fn are_builds_complete(&self) -> bool {
        self.work_orders
            .iter()
            .all(|order| order.factory_id.is_none() || order.num_completed >= order.num_required)
    }

    /// Disband the team (move units to default team)
    pub fn disband(&mut self) -> Result<(), AiError> {
        // Implementation would move all completed units to default team
        self.work_orders.clear();
        Ok(())
    }

    /// Stop queueing new units, just finish current ones
    pub fn stop_queueing(&mut self) {
        self.stop_queueing = true;
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
        let (team_seconds, structure_seconds) = if let Ok(ai) = THE_AI.read() {
            if let Ok(data) = ai.get_ai_data().read() {
                let team = if data.team_seconds > 0.0 {
                    data.team_seconds
                } else {
                    DEFAULT_TEAM_SECONDS
                };
                let structure = if data.structure_seconds > 0.0 {
                    data.structure_seconds
                } else {
                    DEFAULT_STRUCTURE_SECONDS
                };
                (team, structure)
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
        let scan_radius = 200.0;
        let player_id = player_guard.get_id() as UnsignedInt;

        for obj_id in partition.get_objects_in_range(pos, scan_radius) {
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
            if owner_id == player_id {
                continue;
            }
            let Some(owner_arc) = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(owner_id as i32).cloned())
            else {
                continue;
            };
            let Ok(owner_guard) = owner_arc.read() else {
                continue;
            };
            if owner_guard.get_player_type() == PlayerType::Neutral {
                continue;
            }
            if !thing.is_kind_of(KindOf::Structure)
                && !thing.is_kind_of(KindOf::SupplySource)
                && !thing.is_kind_of(KindOf::CashGenerator)
            {
                return false;
            }
            if let Some(team_arc) = obj_guard.get_team() {
                if let Ok(team) = team_arc.read() {
                    if player_guard.get_relationship_with_team(&team) == Relationship::Enemies {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Update loop variant for skirmish AI that supplies its own base-building logic.
    pub fn update_without_base_building(&mut self) -> Result<(), AiError> {
        if self.team_timer > 0 {
            self.team_timer -= 1;
        } else {
            self.ready_to_build_team = true;
            self.team_delay = 0;
        }

        if self.structure_timer > 0 {
            self.structure_timer -= 1;
        } else {
            self.ready_to_build_structure = true;
            self.build_delay = 0;
        }

        if self.build_delay > 0 {
            self.build_delay -= 1;
        }

        if self.team_delay > 0 {
            self.team_delay -= 1;
        }

        self.check_ready_teams()?;
        self.check_queued_teams()?;
        self.do_team_building()?;
        self.do_upgrades_and_skills()?;
        self.update_bridge_repair()?;

        Ok(())
    }

    /// Called when new map is loaded.
    pub fn new_map(&mut self) {
        self.base_center_set = false;
        self.base_radius = 0.0;
        self.team_build_queue.clear();
        self.team_ready_queue.clear();
        self.structures_to_repair = [None; MAX_STRUCTURES_TO_REPAIR];
        self.repair_dozer = None;
        self.dozer_queued_for_repair = false;
        self.dozer_is_repairing = false;
        self.frame_last_building_built = TheGameLogic::get_frame();
    }

    /// Start training for a work order with factory management.
    pub fn start_training_for_order(&mut self, order: &mut WorkOrder, busy_ok: bool) -> bool {
        self.start_training_internal(order, busy_ok, "default")
            .unwrap_or(false)
    }

    pub fn queue_units(&mut self) -> bool {
        let _ = self.queue_supply_truck();

        let mut rebuilt_queue = VecDeque::with_capacity(self.team_build_queue.len());
        while let Some(mut team) = self.team_build_queue.pop_front() {
            let busy_ok = team.priority_build;
            let team_name = team
                .team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            for order in &mut team.work_orders {
                if order.is_waiting_to_build() {
                    let _ = self.start_training_internal(order, busy_ok, team_name.as_str());
                } else {
                    let _ = order.validate_factory(self.player_id);
                }
            }
            rebuilt_queue.push_back(team);
        }
        self.team_build_queue = rebuilt_queue;

        true
    }

    pub fn select_team_to_build_ai(&mut self) -> bool {
        self.select_team_to_build().unwrap_or(false)
    }

    /// Set AI difficulty level
    /// Matches C++ AIPlayer.cpp - affects build speed, reaction time, aggression
    pub fn set_ai_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;

        // Update difficulty handler with new difficulty
        // Note: Faction should be determined from player's side/faction when available
        let faction = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_side().to_string()))
            .unwrap_or_else(|| "USA".to_string());
        self.difficulty_handler
            .set_difficulty(to_ai_difficulty(difficulty), &faction);

        // Apply difficulty-specific behavior modifiers
        match difficulty {
            GameDifficulty::Easy => {
                // Easy AI: Slower, less aggressive, weaker economy
                self.team_seconds = 3.0;
                self.strategic_decision_maker.difficulty_factor = 0.7;
            }
            GameDifficulty::Normal => {
                // Normal AI: Standard behavior
                self.team_seconds = 2.0;
                self.strategic_decision_maker.difficulty_factor = 1.0;
            }
            GameDifficulty::Hard => {
                // Hard AI: Faster, more aggressive, better economy
                self.team_seconds = 1.5;
                self.strategic_decision_maker.difficulty_factor = 1.3;
            }
            GameDifficulty::Brutal => {
                // Brutal AI: Maximum aggression and speed
                self.team_seconds = 1.0;
                self.strategic_decision_maker.difficulty_factor = 1.5;
            }
        }
    }

    /// Select skill set for this AI
    pub fn select_skillset(&mut self, skillset: i32) {
        self.skillset_selector = skillset;
    }

    /// Check if we have a supply source that's safe
    pub fn is_supply_source_safe(&self, min_supplies: i32) -> bool {
        let Ok(list) = player_list().read() else {
            return false;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return false;
        };
        let Ok(player_guard) = player_arc.read() else {
            return false;
        };
        if player_guard.get_money().get_money() < min_supplies {
            return false;
        }
        let Some(partition) = ThePartitionManager::get() else {
            return true;
        };
        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !(obj_guard.is_kind_of(KindOf::SupplySource)
                || obj_guard.is_kind_of(KindOf::ResourceNode)
                || obj_guard.is_kind_of(KindOf::FSSupplyCenter)
                || obj_guard.is_kind_of(KindOf::FSSupplyDropzone)
                || obj_guard.is_kind_of(KindOf::Refinery))
            {
                continue;
            }
            for candidate_id in
                partition.get_objects_in_range(obj_guard.get_position(), SUPPLY_CENTER_SAFE_RADIUS)
            {
                let Some(candidate_arc) = OBJECT_REGISTRY.get_object(candidate_id) else {
                    continue;
                };
                let Ok(candidate_guard) = candidate_arc.read() else {
                    continue;
                };
                if candidate_guard.is_destroyed() {
                    continue;
                }
                let Some(candidate_player_id) = candidate_guard.get_controlling_player_id() else {
                    continue;
                };
                if candidate_player_id as u32 == self.player_id {
                    continue;
                }
                if let Some(candidate_player) = list.get_player(candidate_player_id as i32) {
                    if let Ok(candidate_player_guard) = candidate_player.read() {
                        if candidate_player_guard.get_player_type() == PlayerType::Neutral {
                            continue;
                        }
                    }
                }
                if candidate_guard.is_kind_of(KindOf::Unit)
                    || candidate_guard.is_kind_of(KindOf::Vehicle)
                    || candidate_guard.is_kind_of(KindOf::Infantry)
                    || candidate_guard.is_kind_of(KindOf::Aircraft)
                {
                    return false;
                }
            }
        }
        true
    }

    /// Check if any supply source is under attack
    pub fn is_supply_source_attacked(&self) -> bool {
        self.attacked_supply_center.is_some()
    }

    /// Build a specific AI team immediately
    pub fn build_specific_ai_team(
        &mut self,
        team_name: &str,
        priority_build: bool,
    ) -> Result<(), AiError> {
        let mut team = TeamInQueue::new();
        team.team_name = Some(team_name.to_string());
        team.priority_build = priority_build;
        team.frame_started = 0; // Will be set when we start building

        // Add work orders based on team composition
        // This would normally come from team templates
        self.add_work_orders_for_team(&mut team, team_name)?;

        if priority_build {
            self.team_build_queue.push_front(team);
        } else {
            self.team_build_queue.push_back(team);
        }

        Ok(())
    }

    /// Build AI base defense
    pub fn build_ai_base_defense(&mut self, flank: bool) -> Result<(), AiError> {
        // Determine defense structure type based on faction and strategy
        let defense_structure = self.determine_base_defense_structure(flank)?;
        self.build_ai_base_defense_structure(&defense_structure, flank)
    }

    /// Build specific base defense structure
    pub fn build_ai_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), AiError> {
        // Find suitable location for defense
        let location = self.find_defense_location(flank)?;

        // Queue structure for construction
        self.queue_structure_construction(structure_name, location, 0.0)?;

        Ok(())
    }

    /// Build specific building as soon as possible
    pub fn build_specific_ai_building(&mut self, building_name: &str) -> Result<(), AiError> {
        let priority = ConstructionPriority {
            building_type: building_name.to_string(),
            priority: 0,             // Highest priority
            prerequisites_met: true, // Assume met for immediate building
            max_count: Some(1),
            current_count: 0,
            desired_location: None,
            desired_angle: None,
        };

        self.construction_priorities.insert(0, priority);
        Ok(())
    }

    /// Recruit specific AI team from existing units
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

        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        let origin = self
            .get_base_center()
            .or_else(|| {
                player_guard
                    .get_all_objects()
                    .first()
                    .and_then(|id| OBJECT_REGISTRY.get_object(*id))
                    .and_then(|obj_arc| obj_arc.read().ok().map(|obj| *obj.get_position()))
            })
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        let default_team_id = player_guard.get_default_team_id();
        let player_object_ids = player_guard.get_all_objects();

        let (prototype, team_arc, target_priority, source_priorities) = {
            let Ok(mut factory_guard) = get_team_factory().lock() else {
                return Ok(());
            };
            let Some(prototype) = factory_guard.find_team_prototype(team_name) else {
                return Ok(());
            };

            if prototype.is_singleton() {
                if let Some(existing) = factory_guard.find_team(team_name) {
                    if let Ok(existing_guard) = existing.read() {
                        if existing_guard.has_any_objects() {
                            return Ok(());
                        }
                    }
                }
            }

            let Some(team_arc) = factory_guard.create_inactive_team(team_name) else {
                return Ok(());
            };

            let mut source_priorities = HashMap::new();
            for proto in factory_guard.list_team_prototypes() {
                source_priorities.insert(
                    proto.get_name().to_string(),
                    proto.get_production_priority(),
                );
            }

            (
                prototype.clone(),
                team_arc,
                prototype.get_production_priority(),
                source_priorities,
            )
        };

        if let Ok(mut team_guard) = team_arc.write() {
            team_guard.set_controlling_player_id(Some(self.player_id as UnsignedInt));
        }

        let radius_sqr = radius * radius;
        let mut claimed: HashSet<ObjectID> = HashSet::new();
        let mut units_recruited = 0;

        for unit_info in prototype.units_info() {
            if unit_info.unit_thing_name.is_empty() {
                continue;
            }

            let Some(target_template) = TheThingFactory::find_template(unit_info.unit_thing_name)
            else {
                continue;
            };

            let mut remaining = unit_info.max_units.max(0);
            while remaining > 0 {
                let mut best: Option<(Arc<RwLock<Object>>, ObjectID, Real)> = None;

                for &object_id in &player_object_ids {
                    if claimed.contains(&object_id) {
                        continue;
                    }

                    let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                        continue;
                    };
                    let Ok(object_guard) = object_arc.read() else {
                        continue;
                    };

                    if object_guard.is_destroyed()
                        || object_guard.is_effectively_dead()
                        || object_guard.is_disabled_by_type(crate::common::DisabledType::Held)
                    {
                        continue;
                    }

                    if !target_template.is_equivalent_to(object_guard.get_template().as_ref()) {
                        continue;
                    }

                    let Some(source_team_arc) = object_guard.get_team() else {
                        continue;
                    };
                    let Ok(source_team_guard) = source_team_arc.read() else {
                        continue;
                    };
                    if !source_team_guard.is_active() {
                        continue;
                    }

                    let source_team_id = source_team_guard.get_id();
                    let source_team_name = source_team_guard.get_name().to_string();
                    let source_priority = source_priorities
                        .get(&source_team_name)
                        .copied()
                        .unwrap_or(i32::MAX);
                    if source_priority >= target_priority {
                        continue;
                    }

                    let source_recruitable = if source_team_guard.is_recruitability_set() {
                        source_team_guard.is_recruitable()
                    } else if default_team_id == Some(source_team_id) {
                        true
                    } else {
                        source_team_guard.is_recruitable()
                    };
                    if !source_recruitable {
                        continue;
                    }

                    let pos = object_guard.get_position();
                    let dx = origin.x - pos.x;
                    let dy = origin.y - pos.y;
                    let dist_sqr = dx * dx + dy * dy;
                    if dist_sqr > radius_sqr {
                        continue;
                    }

                    if best
                        .as_ref()
                        .map(|(_, _, best_dist)| dist_sqr < *best_dist)
                        .unwrap_or(true)
                    {
                        best = Some((object_arc.clone(), object_id, dist_sqr));
                    }
                }

                let Some((candidate_arc, candidate_id, _)) = best else {
                    break;
                };

                if let Ok(mut candidate_guard) = candidate_arc.write() {
                    let _ = candidate_guard.set_team(Some(team_arc.clone()));
                }

                if let Ok(mut team_guard) = team_arc.write() {
                    team_guard.add_member(candidate_id);
                }

                claimed.insert(candidate_id);
                units_recruited += 1;
                remaining -= 1;
            }
        }

        if units_recruited > 0 {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.set_active();
            }
        } else if !prototype.is_singleton() {
            let team_id = team_arc.read().ok().map(|team| team.get_id());
            if let (Some(team_id), Ok(mut factory_guard)) = (team_id, get_team_factory().lock()) {
                factory_guard.team_about_to_be_deleted(team_id);
            }
        }

        Ok(())
    }

    /// Build an upgrade (player upgrades only).
    pub fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError> {
        let upgrade = with_upgrade_center(|center| center.find_upgrade(upgrade_name));
        let Some(upgrade) = upgrade else {
            log::warn!("AIPlayer: upgrade '{}' not found", upgrade_name);
            return Ok(());
        };

        if upgrade.get_upgrade_type() == UpgradeType::Object {
            log::warn!(
                "AIPlayer: upgrade '{}' is object-only, skipping",
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

        if player_guard.has_upgrade_in_production(upgrade.as_ref())
            || player_guard.has_upgrade_complete(upgrade.as_ref())
        {
            return Ok(());
        }

        let can_afford = with_upgrade_center(|center| {
            center.can_afford_upgrade(&player_guard, upgrade.as_ref(), false)
        });
        if !can_afford {
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };

        let mut queued = false;
        for object_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            if obj_guard.is_under_construction()
                || obj_guard.test_status(ObjectStatusTypes::Sold)
                || !obj_guard.can_produce_upgrade(upgrade.as_ref())
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

            if obj_guard.queue_upgrade(&upgrade) {
                queued = true;
                break;
            }
        }

        if !queued {
            log::debug!(
                "AIPlayer: no factory available to build upgrade '{}'",
                upgrade_name
            );
        }

        Ok(())
    }

    /// Build a supply center or defense by available supplies near a supply source.
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

        if !self.base_center_set {
            let _ = self.compute_center_and_radius_of_base();
        }
        let base_center = self
            .get_base_center()
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        let is_cash_generator = template.is_kind_of(KindOf::CashGenerator);
        let mut best_supply = None;

        if !is_cash_generator {
            if let Some(warehouse_id) = self.current_warehouse_id {
                if let Some(warehouse_arc) = OBJECT_REGISTRY.get_object(warehouse_id) {
                    best_supply = Some(warehouse_arc);
                }
            }
        }

        if best_supply.is_none() {
            best_supply = self.find_supply_center(minimum_cash);
        }

        let Some(warehouse_arc) = best_supply else {
            return Ok(());
        };
        let Ok(warehouse_guard) = warehouse_arc.read() else {
            return Ok(());
        };
        let mut location = *warehouse_guard.get_position();

        let mut offset = Coord2D::new(location.x - base_center.x, location.y - base_center.y);
        if !is_cash_generator {
            if let Ok(Some((enemy_player, enemy_index))) = self.select_current_enemy_player() {
                let (min_bounds, max_bounds) = self.get_player_structure_bounds(enemy_index)?;
                if !(min_bounds.x == 0.0
                    && min_bounds.y == 0.0
                    && max_bounds.x == 0.0
                    && max_bounds.y == 0.0)
                {
                    let enemy_center = Coord3D::new(
                        (min_bounds.x + max_bounds.x) * 0.5,
                        (min_bounds.y + max_bounds.y) * 0.5,
                        0.0,
                    );
                    offset = Coord2D::new(location.x - enemy_center.x, location.y - enemy_center.y);
                    drop(enemy_player);
                }
            }
        }

        if offset.x != 0.0 || offset.y != 0.0 {
            let len = (offset.x * offset.x + offset.y * offset.y).sqrt();
            offset.x /= len;
            offset.y /= len;
        }

        let radius = if is_cash_generator {
            3.0 * PATHFIND_CELL_SIZE_F
        } else {
            warehouse_guard
                .get_geometry_info()
                .get_bounding_circle_radius()
        };

        location.x -= offset.x * radius;
        location.y -= offset.y * radius;

        if let Some(valid) =
            self.find_valid_build_location(&location, template.get_name().as_str(), 0.0)
        {
            self.queue_structure_construction(thing_name, valid, 0.0)?;
            self.current_warehouse_id = Some(warehouse_guard.get_id());
        }

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

        if !self.base_center_set {
            let _ = self.compute_center_and_radius_of_base();
        }
        let _base_center = self
            .get_base_center()
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        let mut build_location = location;
        if let Some(valid) =
            self.find_valid_build_location(&build_location, template.get_name().as_str(), 0.0)
        {
            build_location = valid;
            self.queue_structure_construction(thing_name, build_location, 0.0)?;
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

    fn find_supply_center(&self, minimum_cash: i32) -> Option<Arc<RwLock<Object>>> {
        let player_arc = self.get_player_arc()?;
        let player_guard = player_arc.read().ok()?;
        let base_center = self
            .get_base_center()
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

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
                    if player_guard.get_relationship_with_team(&team)
                        == crate::common::Relationship::Enemies
                    {
                        continue;
                    }
                }
            }

            let Some(module) = obj_guard.find_update_module("SupplyWarehouseDockUpdate") else {
                continue;
            };
            let mut boxes = None;
            module.with_module_downcast::<SupplyWarehouseDockUpdateModule, _, _>(|module| {
                boxes = Some(module.behavior().get_boxes_stored());
            });
            let Some(boxes) = boxes else {
                continue;
            };

            let available_cash = boxes * BASE_VALUE_PER_SUPPLY_BOX;
            if available_cash < minimum_cash {
                continue;
            }

            let pos = obj_guard.get_position();
            let dist_sq = (pos.x - base_center.x).powi(2) + (pos.y - base_center.y).powi(2);
            if best
                .as_ref()
                .map_or(true, |(best_dist, _)| dist_sq < *best_dist)
            {
                best = Some((dist_sq, obj.clone()));
            }
        }

        best.map(|(_, obj)| obj)
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
    pub fn compute_superweapon_target(
        &self,
        power_template: &str,
        weapon_radius: Real,
    ) -> Result<Option<Coord3D>, AiError> {
        let (_, enemy_index) = match self.select_current_enemy_player() {
            Ok(Some(enemy)) => enemy,
            _ => return Ok(None),
        };

        let radius = weapon_radius.max(1.0);
        let (mut min_bounds, mut max_bounds) = self.get_player_structure_bounds(enemy_index)?;

        if min_bounds.x == 0.0 && min_bounds.y == 0.0 && max_bounds.x == 0.0 && max_bounds.y == 0.0
        {
            if let Some(terrain) = TheTerrainLogic::get() {
                let extent = terrain.get_maximum_pathfind_extent();
                min_bounds = extent.lo;
                max_bounds = extent.hi;
            }
        }

        min_bounds.x += radius;
        min_bounds.y += radius;
        max_bounds.x -= radius;
        max_bounds.y -= radius;
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
        let mut x_count = (width / radius).ceil() as i32 + 1;
        let mut y_count = (height / radius).ceil() as i32 + 1;
        if x_count > 10 {
            x_count = 10;
        }
        if y_count > 10 {
            y_count = 10;
        }

        let power = find_or_create_special_power_template(&AsciiString::from(power_template));
        let target_military_units = power.get_special_power_type()
            != crate::object::special_power_types::SpecialPowerType::SneakAttack;

        let (x_delta, y_delta, x_start, y_start) = match game_logic_random_value(1, 4) {
            1 => (1, 1, 0, 0),
            2 => (-1, 1, x_count - 1, 0),
            3 => (1, -1, 0, y_count - 1),
            _ => (-1, -1, x_count - 1, y_count - 1),
        };

        let mut best_cash = -1.0;
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

        let mut fine_best = best_pos;
        let mut fine_cash = -1.0;
        let mut fine_count = 0;
        let fine_steps = 11;
        for x in 0..fine_steps {
            for y in 0..fine_steps {
                let pos = Coord3D::new(
                    best_pos.x + (x - 5) as f32 * (radius / 10.0),
                    best_pos.y + (y - 5) as f32 * (radius / 10.0),
                    0.0,
                );
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
                } else if (value - fine_cash).abs() < f32::EPSILON {
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

        if fine_cash > -1.0 {
            Ok(Some(fine_best))
        } else {
            Ok(None)
        }
    }

    /// Called when a unit we're training comes into existence
    pub fn on_unit_produced(
        &mut self,
        factory_id: ObjectID,
        _unit_id: ObjectID,
    ) -> Result<(), AiError> {
        // Find the work order that produced this unit
        for team in &mut self.team_build_queue {
            for order in &mut team.work_orders {
                if order.factory_id == Some(factory_id) && order.num_completed < order.num_required
                {
                    order.num_completed += 1;

                    // If this completes the order, clear factory assignment
                    if order.num_completed >= order.num_required {
                        order.factory_id = None;
                    }

                    // Check if team is complete and move to ready queue
                    if team.is_all_built() {
                        // Move team to ready queue
                        // This would be handled by the team management system
                    }

                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Called when a structure we're building comes into existence
    pub fn on_structure_produced(
        &mut self,
        _factory_id: ObjectID,
        structure_id: ObjectID,
    ) -> Result<(), AiError> {
        self.frame_last_building_built = TheGameLogic::get_frame();
        self.team_delay = 0;
        self.build_delay = 0;

        let Some(structure_arc) = OBJECT_REGISTRY.get_object(structure_id) else {
            return Ok(());
        };
        let Ok(structure_guard) = structure_arc.read() else {
            return Ok(());
        };

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return Ok(());
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return Ok(());
        };

        let player_side = player_guard.get_side().to_string();
        if let Some(info) = player_guard.get_build_list_mut() {
            let mut current = Some(&mut *info);
            while let Some(node) = current {
                if node.get_object_id() == structure_id {
                    node.set_under_construction(false);
                    node.set_object_timestamp(TheGameLogic::get_frame());
                    if structure_guard
                        .find_update_module("SupplyCenterDockUpdate")
                        .is_some()
                    {
                        node.set_supply_building(true);
                        node.set_current_gatherers(-1);
                        let mut desired = 0;
                        if let Ok(ai_guard) = THE_AI.read() {
                            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                                for info in &ai_data.side_info {
                                    if info.side == player_side {
                                        desired = match self.difficulty {
                                            GameDifficulty::Easy => info.easy,
                                            GameDifficulty::Normal => info.normal,
                                            GameDifficulty::Hard => info.hard,
                                            GameDifficulty::Brutal => info.hard,
                                        };
                                        break;
                                    }
                                }
                            }
                        }
                        node.set_desired_gatherers(desired + 1);
                    }
                    break;
                }
                current = node.get_next_mut();
            }
        }

        if let Ok(mut structure_write) = structure_arc.write() {
            let mask = ObjectStatusMaskType::from_status(ObjectStatusTypes::UnderConstruction)
                | ObjectStatusMaskType::from_status(ObjectStatusTypes::Reconstructing);
            structure_write.clear_status(mask);
        }

        // Update construction priorities and supply tracking
        self.update_construction_priorities()?;

        Ok(())
    }

    /// Set team delay in seconds
    pub fn set_team_delay_seconds(&mut self, delay: Real) {
        self.team_seconds = delay.max(0.0);
    }

    /// Calculate closest construction zone location
    pub fn calc_closest_construction_zone_location(
        &self,
        template_name: &str,
    ) -> Result<Option<Coord3D>, AiError> {
        if !self.base_center_set {
            return Ok(None);
        }

        let validator = FoundationValidator::new_ai();
        let player_id = self.player_id as ObjectID;
        let base_center = self.base_center;
        let mut radius = 0.0;

        while radius <= SUPPLY_CENTER_CLOSE_DIST {
            let mut angle = 0.0;
            while angle < std::f32::consts::TAU {
                let mut candidate = Coord3D::new(
                    base_center.x + radius * angle.cos(),
                    base_center.y + radius * angle.sin(),
                    base_center.z,
                );
                if let Some(terrain) = TheTerrainLogic::get() {
                    candidate.z = terrain.get_ground_height(candidate.x, candidate.y, None);
                }
                if validator
                    .validate_placement(&candidate, template_name, 0.0, player_id)
                    .is_ok()
                {
                    return Ok(Some(candidate));
                }
                angle += std::f32::consts::FRAC_PI_4;
            }
            radius += 20.0;
        }

        Ok(None)
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

        // C++ AIPlayer uses AIData constants for thresholds
        // m_resourcesWealthy = 5000, m_resourcesPoor = 2000 (from AI.cpp)
        let wealthy_threshold = RESOURCES_WEALTHY;
        let poor_threshold = RESOURCES_POOR;

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

    /// Analyze military situation
    fn analyze_military_situation(&mut self) -> Result<(), AiError> {
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

        self.military_state.unit_counts_by_type.clear();
        let mut total_strength = 0.0;
        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !(obj_guard.is_kind_of(KindOf::Vehicle)
                || obj_guard.is_kind_of(KindOf::Infantry)
                || obj_guard.is_kind_of(KindOf::Aircraft))
            {
                continue;
            }
            let name = obj_guard.get_template_name().to_string();
            *self
                .military_state
                .unit_counts_by_type
                .entry(name)
                .or_insert(0) += 1;
            let cost = obj_guard.get_template().calc_cost_to_build(None).max(1) as f32;
            total_strength += cost;
        }
        self.military_state.total_military_strength = total_strength;

        let mut enemy_strength = 0.0;
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                continue;
            };
            if owner_id as u32 == self.player_id {
                continue;
            }
            if let Some(player) = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(owner_id as i32).cloned())
            {
                if let Ok(enemy_guard) = player.read() {
                    if enemy_guard.get_player_type() == PlayerType::Neutral {
                        continue;
                    }
                }
            }
            if !(obj_guard.is_kind_of(KindOf::Vehicle)
                || obj_guard.is_kind_of(KindOf::Infantry)
                || obj_guard.is_kind_of(KindOf::Aircraft))
            {
                continue;
            }
            let cost = obj_guard.get_template().calc_cost_to_build(None).max(1) as f32;
            enemy_strength += cost;
        }
        self.military_state.enemy_strength_estimate = enemy_strength;

        Ok(())
    }

    /// Analyze current threats
    fn analyze_threats(&mut self) -> Result<(), AiError> {
        // Clear old threats
        self.threat_assessment.immediate_threats.clear();
        self.threat_assessment.potential_threats.clear();

        let _ = self.compute_center_and_radius_of_base();
        let base_center = self.base_center;
        let scan_radius = self.base_radius + SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE;
        let mut threat_score = 0.0;
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                continue;
            };
            if owner_id as u32 == self.player_id {
                continue;
            }
            if !(obj_guard.is_kind_of(KindOf::Vehicle)
                || obj_guard.is_kind_of(KindOf::Infantry)
                || obj_guard.is_kind_of(KindOf::Aircraft)
                || obj_guard.is_kind_of(KindOf::Defense))
            {
                continue;
            }
            let pos = obj_guard.get_position();
            let dx = pos.x - base_center.x;
            let dy = pos.y - base_center.y;
            if dx * dx + dy * dy > scan_radius * scan_radius {
                continue;
            }
            let cost = obj_guard.get_template().calc_cost_to_build(None).max(1) as f32;
            threat_score += cost;
        }

        let denom = (self.military_state.total_military_strength + 1.0).max(1.0);
        let threat_level = (threat_score / denom).min(1.0);
        self.threat_assessment.overall_threat_level = threat_level;
        self.threat_assessment.recommended_response = if threat_level > 0.8 {
            ThreatResponse::Emergency
        } else if threat_level > 0.5 {
            ThreatResponse::Defend
        } else if threat_level > 0.2 {
            ThreatResponse::Monitor
        } else {
            ThreatResponse::None
        };

        Ok(())
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
            for unit in proto.units_info() {
                if unit.unit_thing_name.is_empty() {
                    continue;
                }
                let mut order = WorkOrder::new(unit.unit_thing_name.to_string());
                order.num_required = unit.max_units.max(1);
                order.required = unit.min_units > 0;
                team.work_orders.push(order);
            }
            return Ok(());
        }

        // Fallback: basic units when no prototype exists yet.
        let mut order = WorkOrder::new("Ranger".to_string());
        order.num_required = 1;
        team.work_orders.push(order);

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
        // Main AI player update
        self.update_strategy()?;
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
    /// Process base building logic
    fn do_base_building(&mut self) -> Result<(), AiError> {
        if !self.ready_to_build_structure {
            // Decrement structure timer with difficulty modifier
            if self.structure_timer > 0 {
                self.structure_timer -= 1;
            }

            if self.structure_timer == 0 {
                self.ready_to_build_structure = true;
            }
            return Ok(());
        }

        // Get build speed modifier based on resources and difficulty
        let resource_modifier = self
            .strategic_decision_maker
            .resources
            .get_build_speed_modifier();
        let _difficulty_modifier = self.difficulty_handler.modifiers.structure_speed_modifier;

        // Select structure to build based on priorities
        if let Some(priority) = self.construction_priorities.first().cloned() {
            self.build_structure_now(&priority)?;

            // Reset structure timer with modifiers
            // Base from C++ AIData: m_structureSeconds
            let base_seconds = self.structure_seconds;
            let modified_frames = self
                .difficulty_handler
                .modifiers
                .get_structure_timer_frames(base_seconds);
            self.structure_timer = (modified_frames as f32 / resource_modifier) as u32;
            self.ready_to_build_structure = false;
        }

        Ok(())
    }

    /// Check teams in ready queue
    fn check_ready_teams(&mut self) -> Result<(), AiError> {
        // Move completed teams from build queue to ready queue
        let mut completed_teams = Vec::new();

        for (i, team) in self.team_build_queue.iter().enumerate() {
            if team.is_all_built() {
                completed_teams.push(i);
            }
        }

        // Move completed teams
        for &i in completed_teams.iter().rev() {
            if let Some(team) = self.team_build_queue.remove(i) {
                self.team_ready_queue.push_back(team);
            }
        }

        Ok(())
    }

    /// Check teams in build queue
    /// Matches C++ AIPlayer.cpp checkQueuedTeams logic
    /// Processes work orders and assigns them to available factories
    fn check_queued_teams(&mut self) -> Result<(), AiError> {
        // Collect (team_idx, order_idx, thing_template) for orders waiting to build
        let mut orders_to_process: Vec<(usize, usize, String)> = Vec::new();
        for (team_idx, team) in self.team_build_queue.iter().enumerate() {
            for (order_idx, order) in team.work_orders.iter().enumerate() {
                if order.is_waiting_to_build() {
                    orders_to_process.push((team_idx, order_idx, order.thing_template.clone()));
                }
            }
        }

        // Process collected orders - try to assign factories
        for (team_idx, order_idx, thing_template) in orders_to_process {
            // Find factory for this unit type (prefer idle factories)
            let factory_id = self.find_factory_internal(&thing_template, false)?;

            // Update the order with factory assignment
            if let Some(team) = self.team_build_queue.get_mut(team_idx) {
                if let Some(order) = team.work_orders.get_mut(order_idx) {
                    if factory_id.is_some() {
                        order.factory_id = factory_id;
                        // Training will be started when the factory processes the order
                    }
                }
            }
        }

        // Clean up completed teams
        self.team_build_queue
            .retain(|team| !team.is_all_built() || team.priority_build);

        Ok(())
    }

    /// Process team building logic
    /// Matches C++ AIPlayer.cpp doTeamBuilding
    /// Handles team production timing, resource checks, and reinforcement
    fn do_team_building(&mut self) -> Result<(), AiError> {
        if !self.ready_to_build_team {
            // Decrement team timer with difficulty modifier
            if self.team_timer > 0 {
                self.team_timer -= 1;
            }

            if self.team_timer == 0 {
                self.ready_to_build_team = true;
            }
            return Ok(());
        }

        // Get build speed modifier based on resources and difficulty
        let resource_modifier = self
            .strategic_decision_maker
            .resources
            .get_build_speed_modifier();
        let _difficulty_modifier = self.difficulty_handler.modifiers.team_speed_modifier;

        // C++ checks if we can afford teams based on current resources
        let current_money = self
            .economic_state
            .current_resources
            .get("money")
            .copied()
            .unwrap_or(0);
        let can_afford_team = current_money > 500; // Minimum threshold for team building

        // Select team to build based on strategic priorities
        if can_afford_team && self.select_team_to_build()? {
            // Team selected and queued

            // Reset team timer with modifiers
            // Base from C++ AIData: m_teamSeconds (seconds)
            // Apply difficulty modifier: Easy=slower, Hard=faster
            let base_seconds = self.team_seconds;
            let modified_frames = self
                .difficulty_handler
                .modifiers
                .get_team_timer_frames(base_seconds);

            // Apply resource modifier: Poor=slower, Wealthy=faster
            self.team_timer = (modified_frames as f32 / resource_modifier) as u32;
            self.ready_to_build_team = false;
        }

        // Check for reinforcements needed (minimum priority 5)
        // C++ checks existing teams and determines if they need reinforcement
        if self.select_team_to_reinforce(5)? {
            // Reinforcement team queued
            let base_seconds = self.team_seconds;
            let modified_frames = self
                .difficulty_handler
                .modifiers
                .get_team_timer_frames(base_seconds);
            self.team_timer = (modified_frames as f32 / resource_modifier) as u32;
            self.ready_to_build_team = false;
        }

        Ok(())
    }

    /// Process upgrades and skill purchases.
    /// Matches C++ AIPlayer::doUpgradesAndSkills() from AIPlayer.cpp:2906-2980.
    ///
    /// On first call, selects a skillset randomly from the available ones for the
    /// player's side. Then, if the player has science purchase points, iterates
    /// through the selected skillset and purchases each science that is affordable.
    fn do_upgrades_and_skills(&mut self) -> Result<(), AiError> {
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
        // C++ AIPlayer.cpp:2928-2948
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
            let is_skirmish = self
                .get_player()
                .and_then(|p| p.read().ok().map(|g| g.is_skirmish_ai()))
                .unwrap_or(false);
            if is_skirmish {
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

    /// Update bridge repair system
    fn update_bridge_repair(&mut self) -> Result<(), AiError> {
        if self.bridge_timer > 0 {
            self.bridge_timer = self.bridge_timer.saturating_sub(1);
            return Ok(());
        }

        let Some(structure_id) = self.structures_to_repair.iter().flatten().next().copied() else {
            return Ok(());
        };
        let Some(structure_arc) = OBJECT_REGISTRY.get_object(structure_id) else {
            return Ok(());
        };
        let Ok(structure_guard) = structure_arc.read() else {
            return Ok(());
        };
        if structure_guard.is_destroyed() {
            self.structures_to_repair.iter_mut().for_each(|slot| {
                if slot.as_ref() == Some(&structure_id) {
                    *slot = None;
                }
            });
            return Ok(());
        }

        let target_pos = *structure_guard.get_position();

        if self.repair_dozer.is_none() {
            self.repair_dozer = self.find_dozer(&target_pos)?;
            if let Some(dozer_id) = self.repair_dozer {
                if let Some(dozer_arc) = OBJECT_REGISTRY.get_object(dozer_id) {
                    if let Ok(dozer_guard) = dozer_arc.read() {
                        self.repair_dozer_origin = *dozer_guard.get_position();
                    }
                }
            }
        }

        if let Some(dozer_id) = self.repair_dozer {
            if let Some(dozer_arc) = OBJECT_REGISTRY.get_object(dozer_id) {
                if let Ok(dozer_guard) = dozer_arc.read() {
                    if let Some(ai) = dozer_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let _ = ai_guard.set_movement_target(&target_pos);
                            self.dozer_is_repairing = true;
                        }
                    }
                }
            } else {
                self.repair_dozer = None;
                self.dozer_is_repairing = false;
            }
        }

        Ok(())
    }

    /// Build structure immediately
    fn build_structure_now(&mut self, priority: &ConstructionPriority) -> Result<(), AiError> {
        // Find suitable dozer and location
        let location = if let Some(loc) = priority.desired_location {
            loc
        } else {
            self.calc_closest_construction_zone_location(&priority.building_type)?
                .unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
        };
        let angle = priority.desired_angle.unwrap_or(0.0);

        // Queue for construction
        self.queue_structure_construction(&priority.building_type, location, angle)?;

        Ok(())
    }

    /// Start training a unit at available factory
    /// Matches C++ AIPlayer.cpp:1360 startTraining
    fn start_training_internal(
        &mut self,
        order: &mut WorkOrder,
        busy_ok: bool,
        _team_name: &str,
    ) -> Result<bool, AiError> {
        // From C++ AIPlayer.cpp lines 1360-1381:
        // 1. Find factory that can build this unit
        // 2. Check if factory has production capability
        // 3. Queue unit for production
        // 4. Assign factory ID to work order
        // 5. Return true if successful

        // Find suitable factory (allows busy if requested)
        if let Some(factory_id) = self.find_factory_internal(&order.thing_template, busy_ok)? {
            order.factory_id = Some(factory_id);

            // Full implementation requires:
            // 1. Get factory object via TheGameLogic::findObjectByID
            // 2. Get ProductionUpdateInterface from factory module
            // 3. Call queueCreateUnit with thing template and unique ID
            // 4. Track production in work order
            // 5. Log debug message if AI debugging enabled

            // For now, just assign factory - actual production queuing
            // will be handled when production system is integrated

            log::debug!(
                "AI player {} assigned factory {} for unit {}",
                self.player_id,
                factory_id,
                order.thing_template
            );

            return Ok(true);
        }

        Ok(false)
    }

    #[allow(dead_code)] // C++ parity: default wrapper for start_training_internal
    fn start_training(&mut self, order: &mut WorkOrder) -> Result<(), AiError> {
        // Default: don't use busy factories
        self.start_training_internal(order, false, "default")?;
        Ok(())
    }

    /// Find factory that can build the specified unit
    /// Matches C++ AIPlayer.cpp:1388 findFactory logic
    /// If busyOK is false, only returns idle factories
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

        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_destroyed()
                || obj_guard.is_under_construction()
                || obj_guard.test_status(ObjectStatusTypes::Sold)
            {
                continue;
            }

            let mut checked = false;
            for module_handle in obj_guard.behavior_modules() {
                let mut can_produce = false;
                let mut is_busy = false;
                let matched = module_handle.with_module_downcast::<
                    crate::object::production::production_update_complete::ProductionUpdateCompleteModule,
                    _,
                    _,
                >(|prod_module| {
                    let prod = prod_module.behavior_mut();
                    if prod.can_produce(thing_template) {
                        can_produce = true;
                        is_busy = prod.is_producing() || prod.get_queue_size() > 0;
                    }
                });
                if matched.is_some() {
                    checked = true;
                    if !can_produce {
                        continue;
                    }
                    if !is_busy {
                        return Ok(Some(obj_id));
                    }
                    if busy_ok && busy_factory.is_none() {
                        busy_factory = Some(obj_id);
                    }
                    break;
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
                        busy_factory = Some(obj_id);
                    }
                    break;
                }
            }
        }

        Ok(busy_factory)
    }

    fn find_factory(&self, thing_template: &str) -> Result<Option<ObjectID>, AiError> {
        self.find_factory_internal(thing_template, false)
    }

    /// Select team to build based on priorities and production conditions
    /// Matches C++ AIPlayer.cpp selectTeamToBuild logic
    fn select_team_to_build(&mut self) -> Result<bool, AiError> {
        // This method scans all available team prototypes and selects the best one to build
        // Based on:
        // 1. Production conditions (evaluateProductionCondition)
        // 2. Build limits (maxInstances)
        // 3. Resource availability
        // 4. Factory availability
        // 5. Current team instances
        // 6. Strategic priorities

        // Full implementation (from C++ AIPlayer.cpp):
        // 1. Get all team prototypes for this player's side via TheAI->getTeamPrototypes()
        // 2. For each team prototype:
        //    a. Check production condition via evaluateProductionCondition()
        //    b. Count instances via countTeamInstances(), check against maxInstances
        //    c. Check if already building this team type
        //    d. Call isPossibleToBuildTeam() to check factories and resources
        //    e. Calculate priority based on strategic value
        // 3. Select highest priority team that meets all criteria
        // 4. Create TeamInQueue and add to build queue
        // 5. For each unit in team, create WorkOrder and queue for production

        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok(false);
        };

        let mut best_team: Option<String> = None;
        let mut best_priority = i32::MIN;

        for proto in factory_guard.list_team_prototypes() {
            if !proto.is_ai_recruitable() {
                continue;
            }
            let name = proto.get_name().as_str().to_string();
            if !self.is_a_good_idea_to_build_team(&name)? {
                continue;
            }
            let priority = proto.get_production_priority();
            if priority > best_priority {
                best_priority = priority;
                best_team = Some(name);
            }
        }

        let Some(team_name) = best_team else {
            return Ok(false);
        };

        let mut team = TeamInQueue::new();
        team.team_name = Some(team_name.clone());
        team.frame_started = TheGameLogic::get_frame();
        self.add_work_orders_for_team(&mut team, &team_name)?;
        self.team_build_queue.push_back(team);

        Ok(true)
    }

    /// Select team to reinforce
    fn select_team_to_reinforce(&mut self, min_priority: i32) -> Result<bool, AiError> {
        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok(false);
        };

        let mut best_team: Option<(String, ObjectID)> = None;
        let mut best_priority = i32::MIN;

        for proto in factory_guard.list_team_prototypes() {
            if proto.get_production_priority() < min_priority {
                continue;
            }
            let name = proto.get_name().as_str().to_string();
            let instances = factory_guard.find_team_instances(&name);
            if instances.is_empty() {
                continue;
            }
            if !self.is_a_good_idea_to_build_team(&name)? {
                continue;
            }
            let priority = proto.get_production_priority();
            if priority > best_priority {
                let team_id = instances
                    .iter()
                    .filter_map(|team| team.read().ok().map(|guard| guard.get_id() as ObjectID))
                    .next()
                    .unwrap_or(0);
                best_priority = priority;
                best_team = Some((name, team_id));
            }
        }

        let Some((team_name, team_id)) = best_team else {
            return Ok(false);
        };

        let mut team = TeamInQueue::new();
        team.team_name = Some(team_name.clone());
        team.reinforcement = true;
        team.reinforcement_id = Some(team_id);
        team.frame_started = TheGameLogic::get_frame();
        self.add_work_orders_for_team(&mut team, &team_name)?;
        self.team_build_queue.push_back(team);

        Ok(true)
    }

    /// Queue supply truck
    fn queue_supply_truck(&mut self) -> Result<(), AiError> {
        let mut order = WorkOrder::new("SupplyTruck".to_string());
        order.is_resource_gatherer = true;

        let mut team = TeamInQueue::new();
        team.work_orders.push(order);
        team.priority_build = true;

        self.team_build_queue.push_front(team);
        Ok(())
    }

    /// Process base building requirements
    fn process_base_building(&mut self) -> Result<(), AiError> {
        // Analyze what buildings we need
        self.analyze_building_needs()?;

        // Update construction priorities
        self.update_construction_priorities()?;

        Ok(())
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

    /// Check if team can be considered for building
    /// Matches C++ AIPlayer.cpp:1428 isPossibleToBuildTeam
    fn is_possible_to_build_team(
        &self,
        team_name: &str,
        require_idle_factory: bool,
    ) -> Result<(bool, bool), AiError> {
        // Returns (is_possible, not_enough_money)

        // From C++ AIPlayer.cpp:1428-1469:
        // 1. Get team prototype by name
        // 2. For each unit in team's composition:
        //    - Find factory that can build it via findFactory()
        //    - If require_idle_factory and no idle factory, return (false, false)
        //    - Sum up unit costs from ThingTemplate->calcCostToBuild()
        // 3. Apply team resource multiplier from AIData (m_teamResourcesToBuild = 0.5)
        //    Required resources = total_cost * multiplier
        // 4. Check if player has enough money
        // 5. Return (has_factories, !has_enough_money)

        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok((false, false));
        };
        let Some(proto) = factory_guard.find_team_prototype(team_name) else {
            return Ok((false, false));
        };

        let Ok(list) = player_list().read() else {
            return Ok((false, false));
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok((false, false));
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok((false, false));
        };

        let mut total_cost: i32 = 0;
        let mut has_factories = true;
        let calc = BuildCostCalculator::new();
        let mut modifiers = PlayerBuildModifiers::default();
        modifiers.handicap_cost_multiplier = player_guard.get_handicap().get_cost_multiplier();

        for unit in proto.units_info() {
            if unit.unit_thing_name.is_empty() {
                continue;
            }
            let Some(template) = TheThingFactory::find_template(unit.unit_thing_name) else {
                has_factories = false;
                continue;
            };
            let count = unit.max_units.max(unit.min_units).max(1);
            let base_cost = template.get_build_cost();
            let cost = calc.calc_cost_to_build(base_cost, &modifiers);
            total_cost += cost.saturating_mul(count);

            let factory_id =
                self.find_factory_internal(unit.unit_thing_name, !require_idle_factory)?;
            if factory_id.is_none() {
                has_factories = false;
            }
        }

        let required = ((total_cost as f32) * TEAM_RESOURCES_TO_BUILD).round() as i32;
        let not_enough_money = player_guard.get_money().get_money() < required;

        Ok((has_factories && !not_enough_money, not_enough_money))
    }

    /// Check if team is a good idea to build right now
    /// Matches C++ AIPlayer.cpp:1471 isAGoodIdeaToBuildTeam
    fn is_a_good_idea_to_build_team(&self, team_name: &str) -> Result<bool, AiError> {
        // Evaluation criteria from C++ AIPlayer.cpp:1471-1518:
        // 1. Production condition met via evaluateProductionCondition()
        // 2. Not at max instances: countTeamInstances() < prototype->maxInstances
        // 3. Not already building same team in build queue
        // 4. Can afford and has factories via isPossibleToBuildTeam()

        // Full implementation steps:
        // 1. Get team prototype by name from TheAI->getTeamPrototypes()
        // 2. Call evaluateProductionCondition() with current game state
        // 3. Call countTeamInstances() to count active instances
        // 4. Scan team_build_queue for duplicate team names
        // 5. Call isPossibleToBuildTeam() to verify resources and factories
        // 6. Return true only if all checks pass

        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok(false);
        };
        let Some(proto) = factory_guard.find_team_prototype(team_name) else {
            return Ok(false);
        };

        let instances = factory_guard.find_team_instances(team_name).len() as i32;
        let max_instances = proto.get_max_instances();
        if proto.is_singleton() && instances > 0 {
            return Ok(false);
        }
        if max_instances > 0 && instances >= max_instances {
            return Ok(false);
        }

        if self.team_build_queue.iter().any(|team| {
            team.team_name
                .as_deref()
                .map(|name| name == team_name)
                .unwrap_or(false)
        }) {
            return Ok(false);
        }

        let (possible, _) = self.is_possible_to_build_team(team_name, false)?;
        Ok(possible)
    }

    /// Find dozer for construction
    /// Matches C++ AIPlayer findDozer logic
    fn find_dozer(&self, location: &Coord3D) -> Result<Option<ObjectID>, AiError> {
        // Finds closest idle dozer to the given location
        // Prefers dozers that are:
        // 1. Not building
        // 2. Not collecting resources (for GLA workers)
        // 3. Closest to target location
        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(None);
        };

        let mut best: Option<ObjectID> = None;
        let mut best_dist = f32::MAX;

        for obj_id in player_guard.get_all_objects() {
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
            let Ok(ai_guard) = ai.lock() else {
                continue;
            };
            if !ai_guard.is_idle() {
                continue;
            }
            let pos = obj_guard.get_position();
            let dx = pos.x - location.x;
            let dy = pos.y - location.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best = Some(obj_id);
            }
        }

        Ok(best)
    }

    /// Queue a dozer for construction/repair
    /// Matches C++ AIPlayer queueDozer logic
    pub(crate) fn queue_dozer(&mut self) -> Result<(), AiError> {
        // Creates a high-priority work order for a dozer
        // Adds to front of build queue

        let mut order = WorkOrder::new("Dozer".to_string());
        order.num_required = 1;

        let mut team = TeamInQueue::new();
        team.work_orders.push(order);
        team.priority_build = true;

        self.team_build_queue.push_front(team);
        self.dozer_queued_for_repair = true;

        Ok(())
    }

    /// Repair a structure by sending dozer
    /// Matches C++ AIPlayer repairStructure logic
    pub(crate) fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        // Find available repair slot
        for slot in &mut self.structures_to_repair {
            if slot.is_none() {
                *slot = Some(structure_id);

                // Queue dozer if we don't have one assigned
                if self.repair_dozer.is_none() && !self.dozer_queued_for_repair {
                    self.queue_dozer()?;
                }

                return Ok(());
            }
        }

        Ok(())
    }

    /// Get player structure bounds for targeting
    /// Matches C++ AIPlayer getPlayerStructureBounds logic
    fn get_player_structure_bounds(
        &self,
        player_index: i32,
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

        let mut found = false;
        let mut min = Coord3D::new(0.0, 0.0, 0.0);
        let mut max = Coord3D::new(0.0, 0.0, 0.0);
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
            let pos = obj_guard.get_position();
            if !found {
                min = Coord3D::new(pos.x, pos.y, pos.z);
                max = Coord3D::new(pos.x, pos.y, pos.z);
                found = true;
            } else {
                min.x = min.x.min(pos.x);
                min.y = min.y.min(pos.y);
                max.x = max.x.max(pos.x);
                max.y = max.y.max(pos.y);
            }
        }

        if !found {
            Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)))
        } else {
            Ok((min, max))
        }
    }

    /// Calculate center and radius of AI base
    /// Matches C++ AIPlayer computeCenterAndRadiusOfBase logic
    fn compute_center_and_radius_of_base(&mut self) -> Result<(), AiError> {
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
        let mut sum = Coord3D::new(0.0, 0.0, 0.0);
        let mut count = 0.0;
        let mut positions = Vec::new();
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
            let pos = obj_guard.get_position();
            sum.x += pos.x;
            sum.y += pos.y;
            sum.z += pos.z;
            count += 1.0;
            positions.push(*pos);
        }
        if count > 0.0 {
            self.base_center = Coord3D::new(sum.x / count, sum.y / count, sum.z / count);
            let mut radius = 0.0;
            for pos in positions {
                let dx = pos.x - self.base_center.x;
                let dy = pos.y - self.base_center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > radius {
                    radius = dist;
                }
            }
            self.base_radius = radius;
            self.base_center_set = true;
        }
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

    fn get_player_superweapon_value(
        &self,
        center: &Coord3D,
        player_index: i32,
        radius: Real,
        include_military_units: bool,
    ) -> Result<f32, AiError> {
        let radius = radius.max(4.0 * PATHFIND_CELL_SIZE_F);
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned())
        else {
            return Ok(0.0);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(0.0);
        };

        let mut cash = 0.0;
        let rad_sqr = radius * radius;
        for obj_id in player_guard.get_all_objects() {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_kind_of(KindOf::Aircraft) && obj_guard.is_significantly_above_terrain()
            {
                continue;
            }

            let mut apply_neg_value = false;
            if !include_military_units {
                if obj_guard.is_kind_of(KindOf::Defense) {
                    apply_neg_value = true;
                } else if obj_guard.is_kind_of(KindOf::Vehicle)
                    || obj_guard.is_kind_of(KindOf::Infantry)
                    || obj_guard.is_kind_of(KindOf::Aircraft)
                {
                    if !obj_guard.is_kind_of(KindOf::Dozer)
                        && !obj_guard.is_kind_of(KindOf::Harvester)
                    {
                        apply_neg_value = true;
                    }
                }
            }

            let pos = obj_guard.get_position();
            let dx = center.x - pos.x;
            let dy = center.y - pos.y;
            if dx * dx + dy * dy >= rad_sqr {
                continue;
            }
            let dist = (dx * dx + dy * dy).sqrt();
            let factor = 1.0 - (dist / (2.0 * radius));
            let mut value = obj_guard.get_template().calc_cost_to_build(None).max(1) as f32;
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
        Ok(cash)
    }
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
    fn test_strategy_state() {
        let mut strategy_state = AiStrategyState::default();
        assert_eq!(strategy_state.current_strategy, AiStrategy::Balanced);
        assert_eq!(strategy_state.strategy_confidence, 0.0);
    }
}
