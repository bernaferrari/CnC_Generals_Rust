//! Enhanced AI Player - Modern strategic decision-making system
//!
//! This module provides an enhanced AI player implementation that combines
//! the original C++ logic with modern Rust patterns and improved decision-making
//! algorithms for build orders, strategy, and unit management.
//!
//! Author: Converted from C++ by Claude, original by Michael S. Booth, January 2002

use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};

use super::{AiError, AttitudeType, Pathfinder};
use crate::common::types::{Coord3D, Real};
use crate::common::KindOf;
use crate::common::ObjectID;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{GameDifficulty, Player};

/// AI Player resource assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceStatus {
    VeryPoor,    // Less than 500
    Poor,        // 500-1000
    Normal,      // 1000-2500
    Wealthy,     // 2500-5000
    VeryWealthy, // 5000+
}

/// AI strategic priorities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrategyPriority {
    EconomyExpansion,     // Focus on resource gathering
    MilitaryBuildup,      // Build army units
    TechnologicalAdvance, // Research upgrades
    BaseDefense,          // Build defensive structures
    SpecialPowers,        // Build superweapons/special powers
    Reconnaissance,       // Scout enemy positions
}

/// AI decision-making factors
#[derive(Debug, Clone)]
pub struct DecisionFactors {
    pub resource_status: ResourceStatus,
    pub threat_level: f32,      // 0.0 = no threat, 1.0 = critical threat
    pub base_security: f32,     // 0.0 = very vulnerable, 1.0 = very secure
    pub enemy_strength: f32,    // Relative enemy military strength
    pub time_in_game: Duration, // How long the game has been running
    pub last_attack_time: Option<Instant>, // When we were last attacked
    pub supply_security: f32,   // How secure our supply lines are
    pub expansion_opportunities: u32, // Number of expansion sites available
}

/// Build order item
#[derive(Debug, Clone)]
pub struct BuildOrderItem {
    pub thing_name: String,
    pub priority: u32,
    pub prerequisites: Vec<String>,
    pub resource_cost: u32,
    pub min_supplies: Option<u32>,
    pub build_near_team: Option<String>,
    pub is_defensive: bool,
    pub required_for_strategy: Option<StrategyPriority>,
}

/// Team production order
#[derive(Debug, Clone)]
pub struct TeamBuildOrder {
    pub team_name: String,
    pub units: Vec<BuildOrderItem>,
    pub priority: u32,
    pub min_resources: u32,
    pub strategy_weight: f32, // How important this team is for current strategy
    pub is_reinforcement: bool,
    pub target_team_id: Option<String>, // Team to reinforce
}

/// Enhanced AI Player implementation
pub struct EnhancedAiPlayer {
    /// Player reference for resource and state queries
    player: Weak<RwLock<Player>>,
    /// Player ID
    player_id: u32,
    /// Difficulty level
    difficulty: GameDifficulty,
    /// Current strategic priorities (weighted)
    strategy_priorities: HashMap<StrategyPriority, f32>,
    /// Decision-making factors
    decision_factors: DecisionFactors,
    /// Build queue for structures
    structure_build_queue: VecDeque<BuildOrderItem>,
    /// Build queue for teams
    team_build_queue: VecDeque<TeamBuildOrder>,
    /// Currently building structures
    active_structures: HashMap<ObjectID, BuildOrderItem>,
    /// Currently building teams
    active_teams: HashMap<String, TeamBuildOrder>,
    /// Base center position
    base_center: Option<Coord3D>,
    /// Base expansion positions
    expansion_sites: Vec<Coord3D>,
    /// Enemy base positions (discovered)
    known_enemy_bases: Vec<Coord3D>,
    /// Supply centers under our control
    controlled_supply_centers: Vec<ObjectID>,
    /// Pathfinder reference for strategic positioning
    pathfinder: Arc<RwLock<Pathfinder>>,
    /// Timers for various activities
    timers: AiTimers,
    /// Statistics for performance monitoring
    stats: AiPlayerStats,
    /// Current skillset selection
    skillset_index: Option<usize>,
    /// Build list information
    build_list: Option<BuildList>,
}

/// AI timing controls
#[derive(Debug)]
struct AiTimers {
    last_structure_build: Option<Instant>,
    last_team_build: Option<Instant>,
    last_strategy_evaluation: Option<Instant>,
    last_threat_assessment: Option<Instant>,
    last_expansion_check: Option<Instant>,
    structure_build_delay: Duration,
    team_build_delay: Duration,
    strategy_evaluation_interval: Duration,
    threat_assessment_interval: Duration,
}

impl Default for AiTimers {
    fn default() -> Self {
        Self {
            last_structure_build: None,
            last_team_build: None,
            last_strategy_evaluation: None,
            last_threat_assessment: None,
            last_expansion_check: None,
            structure_build_delay: Duration::from_secs(5),
            team_build_delay: Duration::from_secs(10),
            strategy_evaluation_interval: Duration::from_secs(15),
            threat_assessment_interval: Duration::from_secs(8),
        }
    }
}

/// AI Player performance statistics
#[derive(Debug, Default)]
struct AiPlayerStats {
    structures_built: u32,
    teams_built: u32,
    attacks_launched: u32,
    defenses_built: u32,
    expansions_established: u32,
    resources_spent: u32,
    decisions_made: u32,
}

/// Build list for the AI player
#[derive(Debug, Clone)]
struct BuildList {
    structures: Vec<BuildOrderItem>,
    teams: Vec<TeamBuildOrder>,
    name: String,
}

impl EnhancedAiPlayer {
    /// Create new enhanced AI player
    pub fn new(
        player: Weak<RwLock<Player>>,
        player_id: u32,
        pathfinder: Arc<RwLock<Pathfinder>>,
    ) -> Self {
        let mut strategy_priorities = HashMap::new();

        // Default strategy weights
        strategy_priorities.insert(StrategyPriority::EconomyExpansion, 1.0);
        strategy_priorities.insert(StrategyPriority::MilitaryBuildup, 0.8);
        strategy_priorities.insert(StrategyPriority::BaseDefense, 0.6);
        strategy_priorities.insert(StrategyPriority::TechnologicalAdvance, 0.4);
        strategy_priorities.insert(StrategyPriority::SpecialPowers, 0.2);
        strategy_priorities.insert(StrategyPriority::Reconnaissance, 0.7);

        Self {
            player,
            player_id,
            difficulty: GameDifficulty::Normal,
            strategy_priorities,
            decision_factors: DecisionFactors {
                resource_status: ResourceStatus::Normal,
                threat_level: 0.0,
                base_security: 0.5,
                enemy_strength: 0.5,
                time_in_game: Duration::ZERO,
                last_attack_time: None,
                supply_security: 1.0,
                expansion_opportunities: 0,
            },
            structure_build_queue: VecDeque::new(),
            team_build_queue: VecDeque::new(),
            active_structures: HashMap::new(),
            active_teams: HashMap::new(),
            base_center: None,
            expansion_sites: Vec::new(),
            known_enemy_bases: Vec::new(),
            controlled_supply_centers: Vec::new(),
            pathfinder,
            timers: AiTimers::default(),
            stats: AiPlayerStats::default(),
            skillset_index: None,
            build_list: None,
        }
    }

    /// Set difficulty level
    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
        self.adjust_timers_for_difficulty();
        self.adjust_strategy_for_difficulty();
    }

    /// Update AI player logic for one frame
    pub fn update(&mut self, frame_time: Instant) -> Result<(), AiError> {
        self.decision_factors.time_in_game = frame_time.duration_since(Instant::now()); // Approximation

        // Update decision factors
        self.update_decision_factors()?;

        // Evaluate strategy periodically
        if self.should_evaluate_strategy(frame_time) {
            self.evaluate_strategy()?;
        }

        // Assess threats periodically
        if self.should_assess_threats(frame_time) {
            self.assess_threats()?;
        }

        // Process structure building
        if self.should_build_structure(frame_time) {
            self.process_structure_building()?;
        }

        // Process team building
        if self.should_build_team(frame_time) {
            self.process_team_building()?;
        }

        // Check for expansion opportunities
        if self.should_check_expansion(frame_time) {
            self.check_expansion_opportunities()?;
        }

        // Update active builds
        self.update_active_builds()?;

        Ok(())
    }

    /// Update decision-making factors
    fn update_decision_factors(&mut self) -> Result<(), AiError> {
        // Update resource status
        let resources = self.get_current_resources()?;
        self.decision_factors.resource_status = match resources {
            r if r < 500 => ResourceStatus::VeryPoor,
            r if r < 1000 => ResourceStatus::Poor,
            r if r < 2500 => ResourceStatus::Normal,
            r if r < 5000 => ResourceStatus::Wealthy,
            _ => ResourceStatus::VeryWealthy,
        };

        // Update base security (simplified)
        self.decision_factors.base_security = self.calculate_base_security();

        // Update supply security
        self.decision_factors.supply_security = self.calculate_supply_security();

        Ok(())
    }

    /// Evaluate and adjust strategic priorities
    fn evaluate_strategy(&mut self) -> Result<(), AiError> {
        let factors = &self.decision_factors;

        // Adjust priorities based on current situation
        let mut new_priorities = self.strategy_priorities.clone();

        // If resources are low, prioritize economy
        match factors.resource_status {
            ResourceStatus::VeryPoor | ResourceStatus::Poor => {
                new_priorities.insert(StrategyPriority::EconomyExpansion, 1.5);
                new_priorities.insert(StrategyPriority::MilitaryBuildup, 0.3);
            }
            ResourceStatus::VeryWealthy => {
                new_priorities.insert(StrategyPriority::MilitaryBuildup, 1.2);
                new_priorities.insert(StrategyPriority::SpecialPowers, 0.8);
            }
            _ => {}
        }

        // If under threat, prioritize defense and military
        if factors.threat_level > 0.6 {
            new_priorities.insert(StrategyPriority::BaseDefense, 1.3);
            new_priorities.insert(StrategyPriority::MilitaryBuildup, 1.1);
            new_priorities.insert(StrategyPriority::EconomyExpansion, 0.4);
        }

        // If base is secure and we have resources, consider expansion
        if factors.base_security > 0.7 && factors.resource_status == ResourceStatus::Wealthy {
            new_priorities.insert(StrategyPriority::TechnologicalAdvance, 0.9);
            new_priorities.insert(StrategyPriority::SpecialPowers, 0.6);
        }

        // Always maintain some reconnaissance
        new_priorities.insert(
            StrategyPriority::Reconnaissance,
            0.5 + (factors.enemy_strength * 0.3),
        );

        self.strategy_priorities = new_priorities;
        self.timers.last_strategy_evaluation = Some(Instant::now());
        self.stats.decisions_made += 1;

        log::debug!(
            "AI {} updated strategy priorities: {:?}",
            self.player_id,
            self.strategy_priorities
        );
        Ok(())
    }

    /// Assess current threat level
    fn assess_threats(&mut self) -> Result<(), AiError> {
        let mut threat_level = 0.0f32;

        // Check for nearby enemies
        threat_level += self.scan_for_nearby_enemies()? * 0.4;

        // Check base defenses
        let defense_strength = self.calculate_defense_strength();
        threat_level += (1.0 - defense_strength) * 0.3;

        // Check recent attack history
        if let Some(last_attack) = self.decision_factors.last_attack_time {
            let time_since_attack = Instant::now().duration_since(last_attack);
            if time_since_attack < Duration::from_secs(300) {
                // 5 minutes
                threat_level += 0.3;
            }
        }

        self.decision_factors.threat_level = threat_level.min(1.0);
        self.timers.last_threat_assessment = Some(Instant::now());

        Ok(())
    }

    /// Process structure building decisions
    fn process_structure_building(&mut self) -> Result<(), AiError> {
        if self.structure_build_queue.is_empty() {
            self.generate_structure_build_orders()?;
        }

        // Try to build next structure in queue
        if let Some(build_order) = self.structure_build_queue.front().cloned() {
            if self.can_afford_structure(&build_order)? && self.meets_prerequisites(&build_order)? {
                if let Some(structure_id) = self.start_building_structure(&build_order)? {
                    self.active_structures
                        .insert(structure_id, build_order.clone());
                    self.structure_build_queue.pop_front();
                    self.stats.structures_built += 1;
                    self.timers.last_structure_build = Some(Instant::now());
                }
            }
        }

        Ok(())
    }

    /// Process team building decisions
    fn process_team_building(&mut self) -> Result<(), AiError> {
        if self.team_build_queue.is_empty() {
            self.generate_team_build_orders()?;
        }

        // Try to build next team in queue
        if let Some(build_order) = self.team_build_queue.front().cloned() {
            if self.can_afford_team(&build_order)? && self.meets_team_prerequisites(&build_order)? {
                if self.start_building_team(&build_order)? {
                    self.active_teams
                        .insert(build_order.team_name.clone(), build_order.clone());
                    self.team_build_queue.pop_front();
                    self.stats.teams_built += 1;
                    self.timers.last_team_build = Some(Instant::now());
                }
            }
        }

        Ok(())
    }

    /// Generate structure build orders based on strategy
    fn generate_structure_build_orders(&mut self) -> Result<(), AiError> {
        let mut orders = Vec::new();

        // Priority-based building
        let mut priority_queue: BTreeMap<u32, Vec<BuildOrderItem>> = BTreeMap::new();

        for (strategy, weight) in &self.strategy_priorities {
            let base_priority = (*weight * 1000.0) as u32;

            match strategy {
                StrategyPriority::EconomyExpansion => {
                    // Add supply buildings
                    orders.extend(self.get_economy_buildings(base_priority));
                }
                StrategyPriority::MilitaryBuildup => {
                    // Add barracks, factories, etc.
                    orders.extend(self.get_military_buildings(base_priority));
                }
                StrategyPriority::BaseDefense => {
                    // Add defensive structures
                    orders.extend(self.get_defensive_buildings(base_priority));
                }
                StrategyPriority::TechnologicalAdvance => {
                    // Add research buildings
                    orders.extend(self.get_tech_buildings(base_priority));
                }
                StrategyPriority::SpecialPowers => {
                    // Add superweapon buildings
                    orders.extend(self.get_special_power_buildings(base_priority));
                }
                _ => {}
            }
        }

        // Sort by priority and add to queue
        orders.sort_by_key(|item| std::cmp::Reverse(item.priority));
        for order in orders.into_iter().take(5) {
            // Limit queue size
            self.structure_build_queue.push_back(order);
        }

        Ok(())
    }

    /// Generate team build orders based on strategy
    fn generate_team_build_orders(&mut self) -> Result<(), AiError> {
        let mut orders = Vec::new();

        for (strategy, weight) in &self.strategy_priorities {
            let base_priority = (*weight * 1000.0) as u32;

            match strategy {
                StrategyPriority::MilitaryBuildup => {
                    orders.extend(self.get_military_teams(base_priority));
                }
                StrategyPriority::Reconnaissance => {
                    orders.extend(self.get_scout_teams(base_priority));
                }
                StrategyPriority::BaseDefense => {
                    orders.extend(self.get_defensive_teams(base_priority));
                }
                _ => {}
            }
        }

        // Sort by priority and add to queue
        orders.sort_by_key(|team| std::cmp::Reverse(team.priority));
        for order in orders.into_iter().take(3) {
            // Limit queue size
            self.team_build_queue.push_back(order);
        }

        Ok(())
    }

    // Helper methods for building generation

    fn get_economy_buildings(&self, base_priority: u32) -> Vec<BuildOrderItem> {
        let mut buildings = Vec::new();

        // Add supply depot/power plant equivalents
        buildings.push(BuildOrderItem {
            thing_name: "SupplyCenter".to_string(),
            priority: base_priority + 100,
            prerequisites: vec![],
            resource_cost: 300,
            min_supplies: Some(50),
            build_near_team: None,
            is_defensive: false,
            required_for_strategy: Some(StrategyPriority::EconomyExpansion),
        });

        buildings
    }

    fn get_military_buildings(&self, base_priority: u32) -> Vec<BuildOrderItem> {
        let mut buildings = Vec::new();

        buildings.push(BuildOrderItem {
            thing_name: "Barracks".to_string(),
            priority: base_priority + 80,
            prerequisites: vec![],
            resource_cost: 600,
            min_supplies: None,
            build_near_team: None,
            is_defensive: false,
            required_for_strategy: Some(StrategyPriority::MilitaryBuildup),
        });

        buildings
    }

    fn get_defensive_buildings(&self, base_priority: u32) -> Vec<BuildOrderItem> {
        let mut buildings = Vec::new();

        buildings.push(BuildOrderItem {
            thing_name: "GuardTower".to_string(),
            priority: base_priority + 60,
            prerequisites: vec![],
            resource_cost: 400,
            min_supplies: None,
            build_near_team: None,
            is_defensive: true,
            required_for_strategy: Some(StrategyPriority::BaseDefense),
        });

        buildings
    }

    fn get_tech_buildings(&self, base_priority: u32) -> Vec<BuildOrderItem> {
        let mut buildings = Vec::new();

        buildings.push(BuildOrderItem {
            thing_name: "TechLab".to_string(),
            priority: base_priority + 70,
            prerequisites: vec!["Barracks".to_string()],
            resource_cost: 800,
            min_supplies: None,
            build_near_team: None,
            is_defensive: false,
            required_for_strategy: Some(StrategyPriority::TechnologicalAdvance),
        });

        buildings
    }

    fn get_special_power_buildings(&self, base_priority: u32) -> Vec<BuildOrderItem> {
        let mut buildings = Vec::new();

        buildings.push(BuildOrderItem {
            thing_name: "SuperweaponFacility".to_string(),
            priority: base_priority + 40,
            prerequisites: vec!["TechLab".to_string()],
            resource_cost: 5000,
            min_supplies: None,
            build_near_team: None,
            is_defensive: false,
            required_for_strategy: Some(StrategyPriority::SpecialPowers),
        });

        buildings
    }

    fn get_military_teams(&self, base_priority: u32) -> Vec<TeamBuildOrder> {
        let mut teams = Vec::new();

        teams.push(TeamBuildOrder {
            team_name: "AttackSquad".to_string(),
            units: vec![BuildOrderItem {
                thing_name: "Infantry".to_string(),
                priority: base_priority,
                prerequisites: vec!["Barracks".to_string()],
                resource_cost: 100,
                min_supplies: None,
                build_near_team: None,
                is_defensive: false,
                required_for_strategy: Some(StrategyPriority::MilitaryBuildup),
            }],
            priority: base_priority,
            min_resources: 500,
            strategy_weight: 1.0,
            is_reinforcement: false,
            target_team_id: None,
        });

        teams
    }

    fn get_scout_teams(&self, base_priority: u32) -> Vec<TeamBuildOrder> {
        let mut teams = Vec::new();

        teams.push(TeamBuildOrder {
            team_name: "ScoutTeam".to_string(),
            units: vec![BuildOrderItem {
                thing_name: "Scout".to_string(),
                priority: base_priority,
                prerequisites: vec![],
                resource_cost: 150,
                min_supplies: None,
                build_near_team: None,
                is_defensive: false,
                required_for_strategy: Some(StrategyPriority::Reconnaissance),
            }],
            priority: base_priority,
            min_resources: 200,
            strategy_weight: 0.5,
            is_reinforcement: false,
            target_team_id: None,
        });

        teams
    }

    fn get_defensive_teams(&self, base_priority: u32) -> Vec<TeamBuildOrder> {
        let mut teams = Vec::new();

        teams.push(TeamBuildOrder {
            team_name: "DefenseTeam".to_string(),
            units: vec![BuildOrderItem {
                thing_name: "TankDefender".to_string(),
                priority: base_priority,
                prerequisites: vec!["Barracks".to_string()],
                resource_cost: 300,
                min_supplies: None,
                build_near_team: None,
                is_defensive: true,
                required_for_strategy: Some(StrategyPriority::BaseDefense),
            }],
            priority: base_priority,
            min_resources: 400,
            strategy_weight: 0.8,
            is_reinforcement: false,
            target_team_id: None,
        });

        teams
    }

    // Utility methods (simplified implementations)

    fn should_evaluate_strategy(&self, now: Instant) -> bool {
        self.timers
            .last_strategy_evaluation
            .map(|last| now.duration_since(last) >= self.timers.strategy_evaluation_interval)
            .unwrap_or(true)
    }

    fn should_assess_threats(&self, now: Instant) -> bool {
        self.timers
            .last_threat_assessment
            .map(|last| now.duration_since(last) >= self.timers.threat_assessment_interval)
            .unwrap_or(true)
    }

    fn should_build_structure(&self, now: Instant) -> bool {
        self.timers
            .last_structure_build
            .map(|last| now.duration_since(last) >= self.timers.structure_build_delay)
            .unwrap_or(true)
    }

    fn should_build_team(&self, now: Instant) -> bool {
        self.timers
            .last_team_build
            .map(|last| now.duration_since(last) >= self.timers.team_build_delay)
            .unwrap_or(true)
    }

    fn should_check_expansion(&self, now: Instant) -> bool {
        self.timers
            .last_expansion_check
            .map(|last| now.duration_since(last) >= Duration::from_secs(30))
            .unwrap_or(true)
    }

    fn get_current_resources(&self) -> Result<u32, AiError> {
        let Some(player_arc) = self.player.upgrade() else {
            return Err(AiError::InvalidObject);
        };
        let guard = player_arc.read().map_err(|_| AiError::LockFailed)?;
        let money = guard.get_money().get_money();
        Ok(money.max(0) as u32)
    }

    fn calculate_base_security(&self) -> f32 {
        let mut owned_count = 0.0f32;
        let mut defense_score = 0.0f32;
        let mut armed_score = 0.0f32;
        let mut damaged_penalty = 0.0f32;
        let mut perimeter_score = 0.0f32;

        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_controlling_player_id() != Some(self.player_id)
                || obj_guard.is_effectively_dead()
            {
                continue;
            }

            owned_count += 1.0;
            let health = obj_guard.get_health_percentage().clamp(0.0, 1.0);
            if health < 0.7 {
                damaged_penalty += (0.7 - health) * 0.5;
            }

            if obj_guard.is_kind_of(KindOf::Defense) {
                defense_score += 1.5;
            } else if obj_guard.is_kind_of(KindOf::Structure)
                || obj_guard.is_kind_of(KindOf::Building)
            {
                defense_score += 0.5;
            }

            if obj_guard.has_any_weapon() {
                armed_score += if obj_guard.is_kind_of(KindOf::Vehicle)
                    || obj_guard.is_kind_of(KindOf::Aircraft)
                {
                    1.0
                } else {
                    0.6
                };
            }

            if let Some(base_center) = self.base_center {
                let pos = obj_guard.get_position();
                let dx = pos.x - base_center.x;
                let dy = pos.y - base_center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 120.0 {
                    perimeter_score += 0.5;
                }
            }
        }

        if owned_count <= 0.0 {
            return 0.0;
        }

        let base = 0.15;
        let defense_component = (defense_score / (owned_count * 0.8)).clamp(0.0, 1.0) * 0.45;
        let armed_component = (armed_score / (owned_count * 0.8)).clamp(0.0, 1.0) * 0.3;
        let perimeter_component = (perimeter_score / owned_count).clamp(0.0, 1.0) * 0.2;
        let damage_component = (damaged_penalty / owned_count).clamp(0.0, 1.0) * 0.35;

        (base + defense_component + armed_component + perimeter_component - damage_component)
            .clamp(0.0, 1.0)
    }

    fn calculate_supply_security(&self) -> f32 {
        if self.controlled_supply_centers.is_empty() {
            return 0.0; // No supply centers = no security
        }

        let mut total_score = 0.0f32;
        let mut counted = 0u32;
        let partition = crate::helpers::ThePartitionManager::get();

        for center_id in &self.controlled_supply_centers {
            let Some(center_arc) = OBJECT_REGISTRY.get_object(*center_id) else {
                continue;
            };
            let Ok(center_guard) = center_arc.read() else {
                continue;
            };
            if center_guard.get_controlling_player_id() != Some(self.player_id)
                || center_guard.is_effectively_dead()
            {
                continue;
            }

            let mut nearby_enemy_threat = 0.0f32;
            let mut nearby_defense = 0.0f32;
            if let Some(partition) = partition {
                for obj_id in partition.get_objects_in_range(center_guard.get_position(), 250.0) {
                    let Some(candidate_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                        continue;
                    };
                    let Ok(candidate_guard) = candidate_arc.read() else {
                        continue;
                    };
                    if candidate_guard.is_effectively_dead()
                        || candidate_guard.get_id() == center_guard.get_id()
                    {
                        continue;
                    }
                    if candidate_guard.get_controlling_player_id() == Some(self.player_id) {
                        if candidate_guard.is_kind_of(KindOf::Defense)
                            || candidate_guard.has_any_weapon()
                        {
                            nearby_defense += 1.0;
                        }
                    } else if candidate_guard.has_any_weapon() {
                        nearby_enemy_threat += if candidate_guard.is_kind_of(KindOf::Vehicle)
                            || candidate_guard.is_kind_of(KindOf::Aircraft)
                        {
                            1.5
                        } else {
                            1.0
                        };
                    }
                }
            }

            let center_score =
                (0.7 + nearby_defense * 0.08 - nearby_enemy_threat * 0.12).clamp(0.0, 1.0);
            total_score += center_score;
            counted += 1;
        }

        if counted == 0 {
            0.0
        } else {
            (total_score / counted as f32).clamp(0.0, 1.0)
        }
    }

    fn scan_for_nearby_enemies(&self) -> Result<f32, AiError> {
        let Some(base_center) = self.base_center else {
            return Ok(0.0);
        };
        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return Ok(0.0);
        };

        let mut threat_points = 0.0f32;
        for obj_id in partition.get_objects_in_range(&base_center, 450.0) {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_effectively_dead() {
                continue;
            }
            if obj_guard.get_controlling_player_id() == Some(self.player_id) {
                continue;
            }

            let pos = obj_guard.get_position();
            let dx = pos.x - base_center.x;
            let dy = pos.y - base_center.y;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let distance_factor = (1.0 - (dist / 450.0)).clamp(0.05, 1.0);

            if obj_guard.has_any_weapon() {
                let unit_weight = if obj_guard.is_kind_of(KindOf::Aircraft) {
                    1.4
                } else if obj_guard.is_kind_of(KindOf::Vehicle) {
                    1.2
                } else if obj_guard.is_kind_of(KindOf::Infantry) {
                    0.8
                } else {
                    0.6
                };
                threat_points += unit_weight * distance_factor;
            }
        }

        Ok((threat_points / 12.0).clamp(0.0, 1.0))
    }

    fn calculate_defense_strength(&self) -> f32 {
        let mut defense_structures = 0.0f32;
        let mut combat_units = 0.0f32;
        let mut health_factor = 0.0f32;
        let mut owned_count = 0.0f32;

        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_controlling_player_id() != Some(self.player_id)
                || obj_guard.is_effectively_dead()
            {
                continue;
            }

            owned_count += 1.0;
            health_factor += obj_guard.get_health_percentage().clamp(0.0, 1.0);

            if obj_guard.is_kind_of(KindOf::Defense) {
                defense_structures += 1.0;
            }

            if obj_guard.has_any_weapon() {
                combat_units += if obj_guard.is_kind_of(KindOf::Vehicle)
                    || obj_guard.is_kind_of(KindOf::Aircraft)
                {
                    1.0
                } else {
                    0.5
                };
            }
        }

        if owned_count <= 0.0 {
            return 0.0;
        }

        let avg_health = (health_factor / owned_count).clamp(0.0, 1.0);
        let structure_component = (defense_structures / 8.0).clamp(0.0, 1.0) * 0.55;
        let unit_component = (combat_units / 14.0).clamp(0.0, 1.0) * 0.35;
        let health_component = avg_health * 0.1;

        (structure_component + unit_component + health_component).clamp(0.0, 1.0)
    }

    fn can_afford_structure(&self, build_order: &BuildOrderItem) -> Result<bool, AiError> {
        // Check if we have enough resources to build this structure
        let current_resources = self.get_current_resources()?;
        Ok(current_resources >= build_order.resource_cost)
    }

    fn meets_prerequisites(&self, build_order: &BuildOrderItem) -> Result<bool, AiError> {
        if build_order.prerequisites.is_empty() {
            return Ok(true);
        }

        let mut owned_templates = HashSet::new();
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_controlling_player_id() != Some(self.player_id)
                || obj_guard.is_effectively_dead()
            {
                continue;
            }
            if !obj_guard.is_kind_of(KindOf::Structure) && !obj_guard.is_kind_of(KindOf::Building) {
                continue;
            }
            owned_templates.insert(obj_guard.get_template_name().to_ascii_lowercase());
        }

        for req in &build_order.prerequisites {
            let req_lower = req.to_ascii_lowercase();
            let matched = owned_templates
                .iter()
                .any(|name| name == &req_lower || name.contains(&req_lower));
            if !matched {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn start_building_structure(
        &self,
        build_order: &BuildOrderItem,
    ) -> Result<Option<ObjectID>, AiError> {
        // Initiate structure construction
        // In full implementation, would:
        // 1. Find suitable dozer/builder unit
        // 2. Calculate construction location near base
        // 3. Issue build command to dozer
        // 4. Return ObjectID of structure being built

        // For now, return None to indicate structure not started yet
        // This requires integration with build system and dozer AI
        Ok(None)
    }

    fn can_afford_team(&self, build_order: &TeamBuildOrder) -> Result<bool, AiError> {
        // Calculate total cost of all units in the team
        let total_cost: u32 = build_order
            .units
            .iter()
            .map(|unit| unit.resource_cost)
            .sum();

        // Check if we have enough resources
        let current_resources = self.get_current_resources()?;
        Ok(current_resources >= total_cost && current_resources >= build_order.min_resources)
    }

    fn meets_team_prerequisites(&self, build_order: &TeamBuildOrder) -> Result<bool, AiError> {
        // Check if all prerequisite buildings exist for all units in team
        for unit in &build_order.units {
            if !self.meets_prerequisites(unit)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn start_building_team(&self, build_order: &TeamBuildOrder) -> Result<bool, AiError> {
        // Queue team units for production
        // In full implementation, would:
        // 1. Find available factories for each unit type
        // 2. Queue production orders at factories
        // 3. Track production progress
        // 4. Assign units to team when completed

        // For now, return false to indicate not started
        // Requires integration with production system and factory management
        Ok(false)
    }

    fn check_expansion_opportunities(&mut self) -> Result<(), AiError> {
        // Scan map for potential expansion locations
        // In full implementation, would:
        // 1. Use pathfinder to scan reachable areas
        // 2. Look for resource-rich locations
        // 3. Check distance from enemy bases
        // 4. Evaluate strategic value (chokepoints, high ground, etc.)
        // 5. Add viable locations to expansion_sites list

        self.timers.last_expansion_check = Some(Instant::now());

        self.expansion_sites.clear();
        let Some(base_center) = self.base_center else {
            return Ok(());
        };
        let min_dist_sq = 200.0 * 200.0;
        let mut candidates = Vec::new();
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !obj_guard.is_kind_of(KindOf::ResourceNode)
                && !obj_guard.is_kind_of(KindOf::SupplySource)
                && !obj_guard.is_kind_of(KindOf::FSSupplyCenter)
                && !obj_guard.is_kind_of(KindOf::FSSupplyDropzone)
            {
                continue;
            }
            let pos = obj_guard.get_position();
            let dx = pos.x - base_center.x;
            let dy = pos.y - base_center.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < min_dist_sq {
                continue;
            }
            candidates.push(*pos);
        }

        for pos in candidates {
            if self.expansion_sites.iter().any(|site| {
                let dx = site.x - pos.x;
                let dy = site.y - pos.y;
                dx * dx + dy * dy < (100.0 * 100.0)
            }) {
                continue;
            }
            self.expansion_sites.push(pos);
            if self.expansion_sites.len() >= 10 {
                break;
            }
        }

        Ok(())
    }

    fn update_active_builds(&mut self) -> Result<(), AiError> {
        // Check status of active structure builds
        // In full implementation, would:
        // 1. Query each structure's build progress
        // 2. Remove completed structures from active_structures
        // 3. Handle construction failures (dozer killed, etc.)
        // 4. Update statistics

        // For now, just maintain the maps without active checking
        // Requires integration with object system to query build status

        Ok(())
    }

    fn adjust_timers_for_difficulty(&mut self) {
        match self.difficulty {
            GameDifficulty::Easy => {
                self.timers.structure_build_delay = Duration::from_secs(8);
                self.timers.team_build_delay = Duration::from_secs(15);
            }
            GameDifficulty::Normal => {
                self.timers.structure_build_delay = Duration::from_secs(5);
                self.timers.team_build_delay = Duration::from_secs(10);
            }
            GameDifficulty::Hard => {
                self.timers.structure_build_delay = Duration::from_secs(3);
                self.timers.team_build_delay = Duration::from_secs(6);
            }
            GameDifficulty::Brutal => {
                self.timers.structure_build_delay = Duration::from_secs(3);
                self.timers.team_build_delay = Duration::from_secs(6);
            }
        }
    }

    fn adjust_strategy_for_difficulty(&mut self) {
        match self.difficulty {
            GameDifficulty::Easy => {
                // More conservative, focus on economy
                self.strategy_priorities
                    .insert(StrategyPriority::EconomyExpansion, 1.2);
                self.strategy_priorities
                    .insert(StrategyPriority::MilitaryBuildup, 0.6);
            }
            GameDifficulty::Hard => {
                // More aggressive, faster military buildup
                self.strategy_priorities
                    .insert(StrategyPriority::MilitaryBuildup, 1.3);
                self.strategy_priorities
                    .insert(StrategyPriority::TechnologicalAdvance, 0.8);
            }
            GameDifficulty::Brutal => {
                self.strategy_priorities
                    .insert(StrategyPriority::MilitaryBuildup, 1.3);
                self.strategy_priorities
                    .insert(StrategyPriority::TechnologicalAdvance, 0.8);
            }
            _ => {} // Medium stays default
        }
    }

    /// Get AI player statistics
    pub fn get_stats(&self) -> &AiPlayerStats {
        &self.stats
    }

    /// Get current strategy priorities
    pub fn get_strategy_priorities(&self) -> &HashMap<StrategyPriority, f32> {
        &self.strategy_priorities
    }

    /// Force strategy evaluation (for debugging)
    pub fn force_strategy_evaluation(&mut self) -> Result<(), AiError> {
        self.evaluate_strategy()
    }

    /// Get decision factors (for debugging)
    pub fn get_decision_factors(&self) -> &DecisionFactors {
        &self.decision_factors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use std::sync::Weak;

    #[test]
    fn test_enhanced_ai_player_creation() {
        let pathfinder = Arc::new(RwLock::new(super::super::Pathfinder::new()));
        let player: Weak<RwLock<Player>> = Weak::new();
        let ai = EnhancedAiPlayer::new(player, 123, pathfinder);

        assert_eq!(ai.player_id, 123);
        assert_eq!(ai.difficulty, GameDifficulty::Normal);
        assert!(ai
            .strategy_priorities
            .contains_key(&StrategyPriority::EconomyExpansion));
    }

    #[test]
    fn test_resource_status_calculation() {
        // Test would require mocking get_current_resources method
        assert_eq!(
            match 500 {
                r if r < 500 => ResourceStatus::VeryPoor,
                r if r < 1000 => ResourceStatus::Poor,
                r if r < 2500 => ResourceStatus::Normal,
                r if r < 5000 => ResourceStatus::Wealthy,
                _ => ResourceStatus::VeryWealthy,
            },
            ResourceStatus::Poor
        );
    }

    #[test]
    fn test_strategy_priority_adjustment() {
        let pathfinder = Arc::new(RwLock::new(super::super::Pathfinder::new()));
        let player: Weak<RwLock<Player>> = Weak::new();
        let mut ai = EnhancedAiPlayer::new(player, 123, pathfinder);

        // Test difficulty adjustment
        ai.set_difficulty(GameDifficulty::Hard);
        assert!(ai.strategy_priorities[&StrategyPriority::MilitaryBuildup] > 1.0);
    }
}
