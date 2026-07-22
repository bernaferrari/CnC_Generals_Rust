//! AIBuildList - Construction and build order management
//!
//! This module implements intelligent construction planning and build order
//! management for AI players. It handles base construction, economic buildings,
//! military structures, and defensive installations with strategic timing
//! and resource management.
//!
//! Author: Converted from C++ original

use std::collections::{HashMap, VecDeque, BTreeMap, HashSet};
use std::sync::{Arc, RwLock};
use glam::Vec3;
use crate::common::*;
use crate::ai::*;
use crate::player::{player_list, PlayerType};
use crate::object::registry::OBJECT_REGISTRY;
use crate::upgrade::center::get_upgrade_center;

/// Build order priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuildPriority {
    Emergency = 0,   // Critical structures (under attack, power failure)
    Critical = 1,    // Essential buildings (command center, power)
    High = 2,        // Important structures (barracks, factories)
    Normal = 3,      // Standard buildings (upgrades, secondary)
    Low = 4,         // Optional structures (decorative, excess)
    Deferred = 5,    // Postponed until resources available
}

impl Default for BuildPriority {
    fn default() -> Self {
        BuildPriority::Normal
    }
}

/// Types of structures in build planning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureCategory {
    Command,         // Command centers, headquarters
    Economic,        // Resource generation, supply centers
    Power,          // Power plants, energy generation
    Military,       // Barracks, factories, training facilities
    Defense,        // Defensive structures, walls, bunkers
    Tech,           // Research facilities, tech centers
    Support,        // Repair depots, hospitals, airfields
    Special,        // Superweapon facilities, special buildings
    Expansion,      // Forward bases, outposts
}

/// Build requirements and constraints
#[derive(Debug, Clone)]
pub struct BuildRequirements {
    pub prerequisite_buildings: Vec<String>,    // Required buildings
    pub prerequisite_upgrades: Vec<String>,     // Required research/upgrades
    pub required_resources: HashMap<String, i32>, // Resource costs
    pub power_requirement: i32,                 // Power needed to operate
    pub build_time: u32,                        // Time to construct (in frames)
    pub builder_type: Option<String>,           // Specific builder required (e.g., "Dozer")
    pub placement_restrictions: PlacementRestrictions, // Where it can be built
    pub max_instances: Option<i32>,             // Maximum number allowed
    pub tech_level_required: i32,               // Technology level needed
}

impl Default for BuildRequirements {
    fn default() -> Self {
        Self {
            prerequisite_buildings: Vec::new(),
            prerequisite_upgrades: Vec::new(),
            required_resources: HashMap::new(),
            power_requirement: 0,
            build_time: 300, // 10 seconds at 30 FPS
            builder_type: None,
            placement_restrictions: PlacementRestrictions::default(),
            max_instances: None,
            tech_level_required: 1,
        }
    }
}

/// Placement restrictions for buildings
#[derive(Debug, Clone)]
pub struct PlacementRestrictions {
    pub near_command_center: Option<f32>,       // Must be within X distance of CC
    pub near_power_source: Option<f32>,         // Must be within X distance of power
    pub near_resource_source: Option<f32>,      // Must be within X distance of resources
    pub requires_clear_terrain: bool,           // Needs flat, clear ground
    pub requires_water_access: bool,            // Needs water access (naval yards)
    pub requires_cliff_face: bool,              // Needs to be built into cliff
    pub minimum_spacing: f32,                   // Minimum distance from other buildings
    pub exclusion_zones: Vec<Area>,             // Areas where it cannot be built
    pub preferred_zones: Vec<Area>,             // Preferred construction areas
    pub defensive_line_placement: bool,         // Should be placed in defensive lines
}

impl Default for PlacementRestrictions {
    fn default() -> Self {
        Self {
            near_command_center: None,
            near_power_source: None,
            near_resource_source: None,
            requires_clear_terrain: true,
            requires_water_access: false,
            requires_cliff_face: false,
            minimum_spacing: 50.0,
            exclusion_zones: Vec::new(),
            preferred_zones: Vec::new(),
            defensive_line_placement: false,
        }
    }
}

/// Individual build list item
#[derive(Debug, Clone)]
pub struct BuildListItem {
    pub building_template: String,              // Template name of building
    pub category: StructureCategory,            // Category classification
    pub priority: BuildPriority,               // Build priority
    pub requirements: BuildRequirements,       // Build requirements
    pub strategic_value: f32,                  // Strategic importance (0.0 to 1.0)
    pub economic_value: f32,                   // Economic contribution (0.0 to 1.0)
    pub military_value: f32,                   // Military contribution (0.0 to 1.0)
    pub dependency_weight: i32,                 // How many other buildings depend on this
    pub construction_order: i32,                // Suggested construction order
    pub conditions: Vec<BuildCondition>,        // Additional conditions for building
    pub alternatives: Vec<String>,              // Alternative buildings that serve similar purpose
}

/// Conditions that must be met before building
#[derive(Debug, Clone)]
pub enum BuildCondition {
    MinimumResources(HashMap<String, i32>),     // Have at least X resources
    MaximumResources(HashMap<String, i32>),     // Have at most X resources
    EnemyPresence(bool),                        // Enemy units detected (true/false)
    ThreatLevel(f32),                           // Minimum threat level
    TimeElapsed(u32),                           // Minimum time since game start
    UnitCount(String, i32, bool),               // Have at least/most X units of type
    BuildingCount(String, i32, bool),           // Have at least/most X buildings of type
    TechLevel(i32),                             // Minimum technology level
    MapControl(f32),                            // Minimum map control percentage
    BaseUnderAttack(bool),                      // Base is/isn't under attack
    ResourceShortage(String),                   // Specific resource shortage
    PowerShortage(bool),                        // Power shortage condition
}

/// Build queue entry
#[derive(Debug, Clone)]
pub struct BuildQueueEntry {
    pub item: BuildListItem,
    pub queued_time: u32,                       // Frame when queued
    pub assigned_builder: Option<ObjectID>,     // Builder assigned to construct
    pub construction_site: Option<Coord3D>,     // Where to build
    pub status: BuildStatus,                    // Current status
    pub retries: u32,                           // Number of failed attempts
    pub estimated_completion: Option<u32>,      // Expected completion frame
}

/// Status of build queue entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStatus {
    Queued,          // Waiting in queue
    Planning,        // Finding location and builder
    Assigned,        // Builder assigned, moving to site
    InProgress,      // Construction in progress
    Paused,          // Construction paused (resources, attack)
    Completed,       // Construction completed
    Cancelled,       // Construction cancelled
    Failed,          // Construction failed
}

/// Build list configuration for different strategies
#[derive(Debug, Clone)]
pub struct BuildListConfig {
    pub faction: String,                        // Faction name (USA, China, GLA)
    pub strategy: String,                       // Strategy name (Turtle, Rush, etc.)
    pub difficulty: GameDifficulty,             // AI difficulty level
    pub build_order: Vec<BuildListItem>,        // Ordered list of buildings
    pub economic_focus: f32,                    // Economic vs military focus (0.0 to 1.0)
    pub defensive_ratio: f32,                   // Percentage of buildings for defense
    pub expansion_threshold: f32,               // When to start expanding (map control %)
    pub rush_timing: Option<u32>,               // When to execute rush (frames)
    pub tech_priorities: Vec<String>,           // Technology research priorities
}

/// Resource tracking for build planning
#[derive(Debug, Clone, Default)]
pub struct ResourcePlanning {
    pub current_resources: HashMap<String, i32>,        // Current available resources
    pub resource_income: HashMap<String, f32>,          // Income per second
    pub resource_expenses: HashMap<String, f32>,        // Expenses per second
    pub projected_resources: HashMap<String, i32>,      // Projected resources in 30 seconds
    pub resource_priorities: Vec<String>,               // Resource priority order
    pub shortage_warnings: HashSet<String>,             // Resources running low
    pub income_efficiency: f32,                         // Overall income efficiency (0.0 to 1.0)
}

/// Construction site evaluation
#[derive(Debug, Clone)]
pub struct ConstructionSite {
    pub position: Coord3D,
    pub suitability_score: f32,                 // How suitable (0.0 to 1.0)
    pub terrain_factors: TerrainFactors,        // Terrain considerations
    pub strategic_factors: StrategicFactors,    // Strategic positioning
    pub risk_assessment: RiskAssessment,        // Safety evaluation
    pub accessibility: f32,                     // How accessible to builders (0.0 to 1.0)
}

/// Terrain factors for site evaluation
#[derive(Debug, Clone, Default)]
pub struct TerrainFactors {
    pub elevation: f32,                         // Height above sea level
    pub slope: f32,                             // Ground slope (0.0 = flat, 1.0 = steep)
    pub terrain_type: String,                   // Terrain classification
    pub buildable_area: f32,                    // Available building space
    pub natural_defenses: f32,                  // Natural defensive advantages
    pub resource_access: f32,                   // Access to resource deposits
    pub water_access: bool,                     // Access to water
}

/// Strategic factors for site evaluation
#[derive(Debug, Clone, Default)]
pub struct StrategicFactors {
    pub distance_to_base: f32,                  // Distance from main base
    pub distance_to_enemy: f32,                 // Distance to nearest enemy
    pub control_value: f32,                     // Strategic control value
    pub defensive_value: f32,                   // Defensive positioning value
    pub expansion_potential: f32,               // Future expansion opportunities
    pub supply_line_security: f32,              // Security of supply lines
}

/// Risk assessment for construction sites
#[derive(Debug, Clone, Default)]
pub struct RiskAssessment {
    pub enemy_threat_level: f32,                // Immediate enemy threat (0.0 to 1.0)
    pub vulnerability_score: f32,               // How vulnerable to attack
    pub escape_routes: u32,                     // Number of escape routes
    pub defensive_support: f32,                 // Nearby defensive coverage
    pub intel_confidence: f32,                  // Confidence in threat assessment
}

/// Main AI build list manager
#[derive(Debug)]
pub struct AIBuildList {
    /// Build configurations by faction and strategy
    build_configs: HashMap<String, BuildListConfig>, // Key: "faction_strategy"
    
    /// Active build queues by player
    build_queues: HashMap<u32, VecDeque<BuildQueueEntry>>, // PlayerID -> Queue
    
    /// Resource planning by player
    resource_planning: HashMap<u32, ResourcePlanning>, // PlayerID -> Resources
    
    /// Construction site database
    evaluated_sites: HashMap<String, Vec<ConstructionSite>>, // BuildingType -> Sites
    
    /// Builder management
    available_builders: HashMap<u32, Vec<ObjectID>>, // PlayerID -> Builder IDs
    busy_builders: HashMap<ObjectID, BuildQueueEntry>, // Builder -> Current task
    
    /// Build statistics
    construction_stats: HashMap<u32, ConstructionStats>, // PlayerID -> Stats
    
    /// Timing and scheduling
    last_queue_update: HashMap<u32, u32>,           // PlayerID -> Frame
    last_site_evaluation: HashMap<String, u32>,     // BuildingType -> Frame
    
    /// Configuration
    update_frequency: u32,                          // Frames between updates
    site_cache_duration: u32,                       // Frames to cache site evaluations
    max_concurrent_builds: HashMap<u32, u32>,       // PlayerID -> Max builds
}

/// Construction statistics
#[derive(Debug, Clone, Default)]
pub struct ConstructionStats {
    pub total_buildings_built: u32,
    pub buildings_by_category: HashMap<StructureCategory, u32>,
    pub construction_time_total: u32,               // Total time spent building
    pub construction_failures: u32,                // Failed construction attempts
    pub average_build_time: f32,                   // Average time per building
    pub resource_efficiency: f32,                  // Resource usage efficiency
    pub site_selection_accuracy: f32,              // How often chosen sites work out
}

impl AIBuildList {
    /// Create new AI build list manager
    pub fn new() -> Self {
        let mut manager = Self {
            build_configs: HashMap::new(),
            build_queues: HashMap::new(),
            resource_planning: HashMap::new(),
            evaluated_sites: HashMap::new(),
            available_builders: HashMap::new(),
            busy_builders: HashMap::new(),
            construction_stats: HashMap::new(),
            last_queue_update: HashMap::new(),
            last_site_evaluation: HashMap::new(),
            update_frequency: 30, // Update once per second
            site_cache_duration: 300, // Cache for 10 seconds
            max_concurrent_builds: HashMap::new(),
        };
        
        // Initialize with default build configurations
        manager.initialize_default_configs();
        manager
    }

    /// Initialize default build configurations
    fn initialize_default_configs(&mut self) {
        // USA Turtle Strategy
        let usa_turtle = self.create_usa_turtle_config();
        self.build_configs.insert("USA_Turtle".to_string(), usa_turtle);
        
        // USA Rush Strategy
        let usa_rush = self.create_usa_rush_config();
        self.build_configs.insert("USA_Rush".to_string(), usa_rush);
        
        // China Economic Strategy
        let china_economic = self.create_china_economic_config();
        self.build_configs.insert("China_Economic".to_string(), china_economic);
        
        // GLA Harassment Strategy
        let gla_harassment = self.create_gla_harassment_config();
        self.build_configs.insert("GLA_Harassment".to_string(), gla_harassment);
    }

    /// Create USA turtle strategy build configuration
    fn create_usa_turtle_config(&self) -> BuildListConfig {
        let mut build_order = Vec::new();
        
        // Command Center (already exists)
        // Power Plant
        build_order.push(BuildListItem {
            building_template: "PowerPlant".to_string(),
            category: StructureCategory::Power,
            priority: BuildPriority::Critical,
            requirements: BuildRequirements::default(),
            strategic_value: 0.9,
            economic_value: 0.8,
            military_value: 0.1,
            dependency_weight: 10,
            construction_order: 1,
            conditions: vec![],
            alternatives: vec!["ColdFusionReactor".to_string()],
        });
        
        // Supply Center
        build_order.push(BuildListItem {
            building_template: "SupplyCenter".to_string(),
            category: StructureCategory::Economic,
            priority: BuildPriority::Critical,
            requirements: BuildRequirements::default(),
            strategic_value: 0.9,
            economic_value: 1.0,
            military_value: 0.0,
            dependency_weight: 8,
            construction_order: 2,
            conditions: vec![],
            alternatives: vec![],
        });
        
        // Barracks
        build_order.push(BuildListItem {
            building_template: "Barracks".to_string(),
            category: StructureCategory::Military,
            priority: BuildPriority::High,
            requirements: BuildRequirements::default(),
            strategic_value: 0.8,
            economic_value: 0.0,
            military_value: 0.9,
            dependency_weight: 5,
            construction_order: 3,
            conditions: vec![
                BuildCondition::MinimumResources(
                    [("Money".to_string(), 500)].iter().cloned().collect()
                )
            ],
            alternatives: vec![],
        });
        
        // Defensive structures
        build_order.push(BuildListItem {
            building_template: "PatriotMissileBattery".to_string(),
            category: StructureCategory::Defense,
            priority: BuildPriority::High,
            requirements: BuildRequirements::default(),
            strategic_value: 0.7,
            economic_value: 0.0,
            military_value: 0.8,
            dependency_weight: 0,
            construction_order: 4,
            conditions: vec![
                BuildCondition::ThreatLevel(0.3)
            ],
            alternatives: vec!["FirebasePatriotMissileBattery".to_string()],
        });

        BuildListConfig {
            faction: "USA".to_string(),
            strategy: "Turtle".to_string(),
            difficulty: GameDifficulty::Normal,
            build_order,
            economic_focus: 0.4,
            defensive_ratio: 0.4,
            expansion_threshold: 0.7,
            rush_timing: None,
            tech_priorities: vec!["AdvancedTraining".to_string(), "Composite Armor".to_string()],
        }
    }

    /// Create USA rush strategy build configuration
    fn create_usa_rush_config(&self) -> BuildListConfig {
        let mut build_order = Vec::new();
        
        // Fast military build-up
        build_order.push(BuildListItem {
            building_template: "PowerPlant".to_string(),
            category: StructureCategory::Power,
            priority: BuildPriority::Critical,
            requirements: BuildRequirements::default(),
            strategic_value: 0.8,
            economic_value: 0.9,
            military_value: 0.1,
            dependency_weight: 10,
            construction_order: 1,
            conditions: vec![],
            alternatives: vec![],
        });
        
        build_order.push(BuildListItem {
            building_template: "Barracks".to_string(),
            category: StructureCategory::Military,
            priority: BuildPriority::Critical,
            requirements: BuildRequirements::default(),
            strategic_value: 0.9,
            economic_value: 0.0,
            military_value: 1.0,
            dependency_weight: 5,
            construction_order: 2,
            conditions: vec![],
            alternatives: vec![],
        });
        
        build_order.push(BuildListItem {
            building_template: "WarFactory".to_string(),
            category: StructureCategory::Military,
            priority: BuildPriority::High,
            requirements: BuildRequirements::default(),
            strategic_value: 0.8,
            economic_value: 0.0,
            military_value: 0.9,
            dependency_weight: 3,
            construction_order: 3,
            conditions: vec![
                BuildCondition::TimeElapsed(900) // 30 seconds
            ],
            alternatives: vec![],
        });

        BuildListConfig {
            faction: "USA".to_string(),
            strategy: "Rush".to_string(),
            difficulty: GameDifficulty::Normal,
            build_order,
            economic_focus: 0.2,
            defensive_ratio: 0.1,
            expansion_threshold: 0.3,
            rush_timing: Some(1800), // 60 seconds
            tech_priorities: vec!["RangerCaptureBuilding".to_string()],
        }
    }

    /// Create China economic strategy build configuration
    fn create_china_economic_config(&self) -> BuildListConfig {
        let mut build_order = Vec::new();
        
        // Economic focus
        build_order.push(BuildListItem {
            building_template: "PowerPlant".to_string(),
            category: StructureCategory::Power,
            priority: BuildPriority::Critical,
            requirements: BuildRequirements::default(),
            strategic_value: 0.9,
            economic_value: 1.0,
            military_value: 0.0,
            dependency_weight: 10,
            construction_order: 1,
            conditions: vec![],
            alternatives: vec!["NuclearReactor".to_string()],
        });
        
        build_order.push(BuildListItem {
            building_template: "SupplyCenter".to_string(),
            category: StructureCategory::Economic,
            priority: BuildPriority::Critical,
            requirements: BuildRequirements::default(),
            strategic_value: 0.9,
            economic_value: 1.0,
            military_value: 0.0,
            dependency_weight: 8,
            construction_order: 2,
            conditions: vec![],
            alternatives: vec![],
        });
        
        // Multiple supply centers for economic advantage
        build_order.push(BuildListItem {
            building_template: "SupplyCenter".to_string(),
            category: StructureCategory::Economic,
            priority: BuildPriority::High,
            requirements: BuildRequirements::default(),
            strategic_value: 0.7,
            economic_value: 0.9,
            military_value: 0.0,
            dependency_weight: 5,
            construction_order: 3,
            conditions: vec![
                BuildCondition::MinimumResources(
                    [("Money".to_string(), 1000)].iter().cloned().collect()
                )
            ],
            alternatives: vec![],
        });

        BuildListConfig {
            faction: "China".to_string(),
            strategy: "Economic".to_string(),
            difficulty: GameDifficulty::Normal,
            build_order,
            economic_focus: 0.8,
            defensive_ratio: 0.2,
            expansion_threshold: 0.6,
            rush_timing: None,
            tech_priorities: vec!["CashHack".to_string(), "OverlordBattleBunker".to_string()],
        }
    }

    /// Create GLA harassment strategy build configuration
    fn create_gla_harassment_config(&self) -> BuildListConfig {
        let mut build_order = Vec::new();
        
        // Mobile, harassment-focused build
        build_order.push(BuildListItem {
            building_template: "GLASupplyStash".to_string(),
            category: StructureCategory::Economic,
            priority: BuildPriority::Critical,
            requirements: BuildRequirements::default(),
            strategic_value: 0.9,
            economic_value: 1.0,
            military_value: 0.0,
            dependency_weight: 8,
            construction_order: 1,
            conditions: vec![],
            alternatives: vec![],
        });
        
        build_order.push(BuildListItem {
            building_template: "GLABarracks".to_string(),
            category: StructureCategory::Military,
            priority: BuildPriority::High,
            requirements: BuildRequirements::default(),
            strategic_value: 0.8,
            economic_value: 0.0,
            military_value: 0.9,
            dependency_weight: 5,
            construction_order: 2,
            conditions: vec![],
            alternatives: vec![],
        });
        
        build_order.push(BuildListItem {
            building_template: "GLAArmsDealer".to_string(),
            category: StructureCategory::Military,
            priority: BuildPriority::High,
            requirements: BuildRequirements::default(),
            strategic_value: 0.7,
            economic_value: 0.0,
            military_value: 0.8,
            dependency_weight: 3,
            construction_order: 3,
            conditions: vec![
                BuildCondition::TimeElapsed(600) // 20 seconds
            ],
            alternatives: vec![],
        });

        BuildListConfig {
            faction: "GLA".to_string(),
            strategy: "Harassment".to_string(),
            difficulty: GameDifficulty::Normal,
            build_order,
            economic_focus: 0.3,
            defensive_ratio: 0.1,
            expansion_threshold: 0.4,
            rush_timing: Some(1200), // 40 seconds
            tech_priorities: vec!["ScrapMetal".to_string(), "APRockets".to_string()],
        }
    }

    /// Update build queue for a player
    pub fn update_build_queue(&mut self, player_id: u32, current_frame: u32) -> Result<(), AiError> {
        // Check if it's time to update
        if let Some(&last_update) = self.last_queue_update.get(&player_id) {
            if current_frame - last_update < self.update_frequency {
                return Ok(());
            }
        }
        
        self.last_queue_update.insert(player_id, current_frame);
        
        // Update resource planning
        self.update_resource_planning(player_id, current_frame)?;
        
        // Process current build queue
        self.process_build_queue(player_id, current_frame)?;
        
        // Evaluate new builds to queue
        self.evaluate_new_builds(player_id, current_frame)?;
        
        // Update builder assignments
        self.update_builder_assignments(player_id)?;
        
        Ok(())
    }

    /// Update resource planning for a player
    fn update_resource_planning(&mut self, player_id: u32, current_frame: u32) -> Result<(), AiError> {
        let planning = self.resource_planning.entry(player_id).or_default();
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(player_id as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    planning.current_resources.insert(
                        "Money".to_string(),
                        player_guard.get_money().get_money() as i32,
                    );
                    planning.current_resources.insert(
                        "Power".to_string(),
                        player_guard.get_energy().get_power() as i32,
                    );
                    planning.resource_income.insert(
                        "Money".to_string(),
                        player_guard.get_money().get_income_rate(),
                    );
                    planning.resource_expenses.insert("Money".to_string(), 0.0);
                }
            }
        }
        
        // Calculate projected resources (30 seconds ahead)
        for (resource, &current) in &planning.current_resources {
            let income = planning.resource_income.get(resource).unwrap_or(&0.0);
            let expense = planning.resource_expenses.get(resource).unwrap_or(&0.0);
            let net_income = income - expense;
            let projected = current + (net_income * 30.0) as i32;
            planning.projected_resources.insert(resource.clone(), projected);
        }
        
        // Check for shortages
        planning.shortage_warnings.clear();
        for (resource, &current) in &planning.current_resources {
            if current < 200 { // Low threshold
                planning.shortage_warnings.insert(resource.clone());
            }
        }
        
        Ok(())
    }

    /// Process current build queue entries
    fn process_build_queue(&mut self, player_id: u32, current_frame: u32) -> Result<(), AiError> {
        let queue = self.build_queues.entry(player_id).or_default();
        let mut completed_builds = Vec::new();
        
        for (i, entry) in queue.iter_mut().enumerate() {
            match entry.status {
                BuildStatus::Queued => {
                    // Try to start planning
                    if self.can_afford_build(&entry.item, player_id)? {
                        entry.status = BuildStatus::Planning;
                    }
                }
                BuildStatus::Planning => {
                    // Find location and assign builder
                    if let Some(site) = self.find_construction_site(&entry.item, player_id)? {
                        entry.construction_site = Some(site.position);
                        
                        if let Some(builder) = self.assign_builder(&entry.item, site.position, player_id)? {
                            entry.assigned_builder = Some(builder);
                            entry.status = BuildStatus::Assigned;
                            entry.estimated_completion = Some(current_frame + entry.item.requirements.build_time);
                        }
                    }
                }
                BuildStatus::Assigned => {
                    // Check if construction has started
                    // This would check with the game's construction system
                    // For now, assume it transitions to InProgress
                    entry.status = BuildStatus::InProgress;
                }
                BuildStatus::InProgress => {
                    // Check if construction is complete
                    if let Some(completion_frame) = entry.estimated_completion {
                        if current_frame >= completion_frame {
                            entry.status = BuildStatus::Completed;
                            completed_builds.push(i);
                        }
                    }
                }
                BuildStatus::Failed => {
                    // Retry or remove failed builds
                    if entry.retries < 3 {
                        entry.retries += 1;
                        entry.status = BuildStatus::Queued;
                        entry.assigned_builder = None;
                        entry.construction_site = None;
                    } else {
                        completed_builds.push(i); // Remove after too many failures
                    }
                }
                _ => {} // Other statuses don't need processing
            }
        }
        
        // Remove completed builds
        for &i in completed_builds.iter().rev() {
            if let Some(entry) = queue.remove(i) {
                // Update statistics
                let stats = self.construction_stats.entry(player_id).or_default();
                if entry.status == BuildStatus::Completed {
                    stats.total_buildings_built += 1;
                    *stats.buildings_by_category.entry(entry.item.category).or_insert(0) += 1;
                } else {
                    stats.construction_failures += 1;
                }
                
                // Release builder
                if let Some(builder_id) = entry.assigned_builder {
                    self.busy_builders.remove(&builder_id);
                    self.available_builders.entry(player_id).or_default().push(builder_id);
                }
            }
        }
        
        Ok(())
    }

    /// Evaluate new builds to add to queue
    fn evaluate_new_builds(&mut self, player_id: u32, current_frame: u32) -> Result<(), AiError> {
        // Get build configuration for this player
        let config = self.get_build_config_for_player(player_id)?;
        let resource_planning = self.resource_planning.get(&player_id).unwrap();
        
        // Check each item in build order
        for item in &config.build_order {
            // Check if we already have this building or it's already queued
            if self.already_have_or_queued(&item.building_template, player_id)? {
                continue;
            }
            
            // Check conditions
            if !self.check_build_conditions(&item.conditions, player_id, current_frame)? {
                continue;
            }
            
            // Check if we can afford it (or will be able to soon)
            if !self.can_afford_build_soon(item, resource_planning)? {
                continue;
            }
            
            // Check prerequisites
            if !self.check_prerequisites(&item.requirements, player_id)? {
                continue;
            }
            
            // Add to queue
            let queue_entry = BuildQueueEntry {
                item: item.clone(),
                queued_time: current_frame,
                assigned_builder: None,
                construction_site: None,
                status: BuildStatus::Queued,
                retries: 0,
                estimated_completion: None,
            };
            
            self.build_queues.entry(player_id).or_default().push_back(queue_entry);
            break; // Only queue one building per update
        }
        
        Ok(())
    }

    /// Check if build conditions are met
    fn check_build_conditions(&self, conditions: &[BuildCondition], player_id: u32, current_frame: u32) -> Result<bool, AiError> {
        for condition in conditions {
            match condition {
                BuildCondition::MinimumResources(required) => {
                    if let Some(resources) = self.resource_planning.get(&player_id) {
                        for (resource, &amount) in required {
                            let current = resources.current_resources.get(resource).unwrap_or(&0);
                            if *current < amount {
                                return Ok(false);
                            }
                        }
                    } else {
                        return Ok(false);
                    }
                }
                BuildCondition::TimeElapsed(min_time) => {
                    if current_frame < *min_time {
                        return Ok(false);
                    }
                }
                BuildCondition::ThreatLevel(min_threat) => {
                    let threat_level = self.compute_threat_level(player_id)?;
                    if threat_level < *min_threat {
                        return Ok(false);
                    }
                }
                BuildCondition::EnemyPresence(needed) => {
                    let threat_level = self.compute_threat_level(player_id)?;
                    let enemy_present = threat_level > 0.01;
                    if enemy_present != *needed {
                        return Ok(false);
                    }
                }
                BuildCondition::UnitCount(template, count, at_least) => {
                    let unit_count = self.count_player_units(player_id, template, false)?;
                    if *at_least {
                        if unit_count < *count {
                            return Ok(false);
                        }
                    } else if unit_count > *count {
                        return Ok(false);
                    }
                }
                BuildCondition::BuildingCount(template, count, at_least) => {
                    let building_count = self.count_player_units(player_id, template, true)?;
                    if *at_least {
                        if building_count < *count {
                            return Ok(false);
                        }
                    } else if building_count > *count {
                        return Ok(false);
                    }
                }
                BuildCondition::TechLevel(required) => {
                    let level = self.get_player_tech_level(player_id)?;
                    if level < *required {
                        return Ok(false);
                    }
                }
                BuildCondition::MapControl(min_control) => {
                    let control = self.compute_map_control(player_id)?;
                    if control < *min_control {
                        return Ok(false);
                    }
                }
                BuildCondition::BaseUnderAttack(needed) => {
                    let threat_level = self.compute_threat_level(player_id)?;
                    let under_attack = threat_level > 0.3;
                    if under_attack != *needed {
                        return Ok(false);
                    }
                }
                BuildCondition::ResourceShortage(resource) => {
                    if let Some(resources) = self.resource_planning.get(&player_id) {
                        if !resources.shortage_warnings.contains(resource) {
                            return Ok(false);
                        }
                    } else {
                        return Ok(false);
                    }
                }
                BuildCondition::PowerShortage(needed) => {
                    let shortage = player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(player_id as i32).cloned())
                        .and_then(|player| player.read().ok().map(|guard| guard.get_energy().is_low_power()))
                        .unwrap_or(false);
                    if shortage != *needed {
                        return Ok(false);
                    }
                }
                BuildCondition::MaximumResources(maximum) => {
                    if let Some(resources) = self.resource_planning.get(&player_id) {
                        for (resource, &amount) in maximum {
                            let current = resources.current_resources.get(resource).unwrap_or(&0);
                            if *current > amount {
                                return Ok(false);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(true)
    }

    /// Check if player can afford a build
    fn can_afford_build(&self, item: &BuildListItem, player_id: u32) -> Result<bool, AiError> {
        if let Some(resources) = self.resource_planning.get(&player_id) {
            for (resource, &cost) in &item.requirements.required_resources {
                let available = resources.current_resources.get(resource).unwrap_or(&0);
                if *available < cost {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if player can afford a build soon (within reasonable time)
    fn can_afford_build_soon(&self, item: &BuildListItem, resources: &ResourcePlanning) -> Result<bool, AiError> {
        for (resource, &cost) in &item.requirements.required_resources {
            let current = resources.current_resources.get(resource).unwrap_or(&0);
            let projected = resources.projected_resources.get(resource).unwrap_or(&0);
            
            if *current < cost && *projected < cost {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Check if prerequisites are met
    fn check_prerequisites(&self, requirements: &BuildRequirements, player_id: u32) -> Result<bool, AiError> {
        // Check prerequisite buildings
        for building in &requirements.prerequisite_buildings {
            if !self.player_has_building(building, player_id)? {
                return Ok(false);
            }
        }
        
        // Check prerequisite upgrades
        for upgrade in &requirements.prerequisite_upgrades {
            if !self.player_has_upgrade(upgrade, player_id)? {
                return Ok(false);
            }
        }
        
        // Check tech level
        let player_tech_level = self.get_player_tech_level(player_id)?;
        if player_tech_level < requirements.tech_level_required {
            return Ok(false);
        }
        
        Ok(true)
    }

    /// Find suitable construction site for building
    fn find_construction_site(&mut self, item: &BuildListItem, player_id: u32) -> Result<Option<ConstructionSite>, AiError> {
        let cache_key = format!("{}_{}", item.building_template, player_id);
        
        // Check cache
        if let Some(sites) = self.evaluated_sites.get(&cache_key) {
            if let Some(best_site) = sites.first() {
                return Ok(Some(best_site.clone()));
            }
        }
        
        // Evaluate new sites
        let mut candidate_sites = Vec::new();
        
        // Generate candidate positions (this would use actual map data)
        let base_position = self.get_player_base_center(player_id)?;
        for i in 0..10 {
            let angle = (i as f32) * 0.628; // ~36 degrees apart
            let distance = 100.0 + (i as f32) * 50.0;
            let x = base_position[0] + distance * angle.cos();
            let y = base_position[1] + distance * angle.sin();
            let position = [x, y, base_position[2]];
            
            let site = self.evaluate_construction_site(position, item, player_id)?;
            candidate_sites.push(site);
        }
        
        // Sort by suitability
        candidate_sites.sort_by(|a, b| b.suitability_score.partial_cmp(&a.suitability_score).unwrap());
        
        // Cache results
        self.evaluated_sites.insert(cache_key, candidate_sites.clone());
        
        Ok(candidate_sites.first().cloned())
    }

    /// Evaluate a construction site
    fn evaluate_construction_site(&self, position: Coord3D, item: &BuildListItem, player_id: u32) -> Result<ConstructionSite, AiError> {
        let mut site = ConstructionSite {
            position,
            suitability_score: 0.0,
            terrain_factors: TerrainFactors::default(),
            strategic_factors: StrategicFactors::default(),
            risk_assessment: RiskAssessment::default(),
            accessibility: 1.0,
        };
        
        // Evaluate terrain factors
        site.terrain_factors.buildable_area = 1.0; // Assume buildable
        site.terrain_factors.slope = 0.1; // Assume mostly flat
        
        // Evaluate strategic factors
        let base_center = self.get_player_base_center(player_id)?;
        site.strategic_factors.distance_to_base = self.calculate_distance(position, base_center);
        
        // Evaluate placement restrictions
        let mut score = 1.0;
        
        // Distance from command center
        if let Some(max_distance) = item.requirements.placement_restrictions.near_command_center {
            if site.strategic_factors.distance_to_base > max_distance {
                score *= 0.1; // Heavy penalty
            }
        }
        
        // Minimum spacing
        let min_spacing = item.requirements.placement_restrictions.minimum_spacing;
        // This would check actual building positions
        let nearby_buildings = self.count_nearby_buildings(player_id, position, min_spacing)?;
        if nearby_buildings > 0 {
            score *= 0.8;
        }
        
        // Category-specific scoring
        match item.category {
            StructureCategory::Defense => {
                // Defensive buildings prefer perimeter locations
                score += 0.2;
            }
            StructureCategory::Economic => {
                // Economic buildings prefer safe, central locations
                score += (1.0 - (site.strategic_factors.distance_to_base / 200.0)).max(0.0);
            }
            StructureCategory::Power => {
                // Power plants prefer central locations for efficient distribution
                score += (1.0 - (site.strategic_factors.distance_to_base / 150.0)).max(0.0);
            }
            _ => {}
        }
        
        site.suitability_score = score.clamp(0.0, 1.0);
        Ok(site)
    }

    /// Assign builder to construction task
    fn assign_builder(&mut self, item: &BuildListItem, position: Coord3D, player_id: u32) -> Result<Option<ObjectID>, AiError> {
        let available = self.available_builders.entry(player_id).or_default();
        
        if available.is_empty() {
            return Ok(None);
        }
        
        // Find closest available builder
        let mut best_builder = None;
        let mut best_distance = f32::INFINITY;
        
        for &builder_id in available.iter() {
            let builder_pos = self.get_builder_position(builder_id)?;
            let distance = self.calculate_distance(builder_pos, position);
            
            if distance < best_distance {
                best_distance = distance;
                best_builder = Some(builder_id);
            }
        }
        
        if let Some(builder_id) = best_builder {
            // Remove from available and add to busy
            available.retain(|&id| id != builder_id);
            Ok(Some(builder_id))
        } else {
            Ok(None)
        }
    }

    /// Update builder assignments and availability
    fn update_builder_assignments(&mut self, player_id: u32) -> Result<(), AiError> {
        // Find idle builders and add them to available list
        let idle_builders = self.find_idle_builders(player_id)?;
        let available = self.available_builders.entry(player_id).or_default();
        
        for builder in idle_builders {
            if !available.contains(&builder) {
                available.push(builder);
            }
        }
        
        Ok(())
    }

    // Helper methods (these would interface with the actual game systems)

    fn get_build_config_for_player(&self, player_id: u32) -> Result<&BuildListConfig, AiError> {
        let side = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_side().clone()))
            .unwrap_or_else(|| "USA".to_string());
        let candidates = [
            format!("{}_Turtle", side),
            format!("{}_Balanced", side),
            "USA_Turtle".to_string(),
        ];
        for key in &candidates {
            if let Some(config) = self.build_configs.get(key) {
                return Ok(config);
            }
        }
        self.build_configs.values().next().ok_or(AiError::InvalidObject)
    }

    fn already_have_or_queued(&self, building_template: &str, player_id: u32) -> Result<bool, AiError> {
        // Check if building is already built or in queue
        if let Some(queue) = self.build_queues.get(&player_id) {
            for entry in queue {
                if entry.item.building_template == building_template {
                    return Ok(true);
                }
            }
        }
        
        self.player_has_building(building_template, player_id)
    }

    fn player_has_building(&self, building: &str, player_id: u32) -> Result<bool, AiError> {
        let Some(player_arc) = player_list().read().ok().and_then(|list| list.get_player(player_id as i32).cloned()) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        for obj_id in player_guard.get_all_objects() {
            let matches = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    (obj_guard.is_kind_of(KindOf::Structure)
                        || obj_guard.is_kind_of(KindOf::Building))
                        && obj_guard.get_template_name() == building
                })
                .unwrap_or(false);
            if matches {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn player_has_upgrade(&self, upgrade: &str, player_id: u32) -> Result<bool, AiError> {
        let Some(player_arc) = player_list().read().ok().and_then(|list| list.get_player(player_id as i32).cloned()) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let center = get_upgrade_center();
        let Ok(center_guard) = center.read() else {
            return Ok(false);
        };
        let Some(template) = center_guard.find_upgrade(upgrade) else {
            return Ok(false);
        };
        Ok(player_guard.has_upgrade_complete(&template))
    }

    fn get_player_tech_level(&self, player_id: u32) -> Result<i32, AiError> {
        let Some(player_arc) = player_list().read().ok().and_then(|list| list.get_player(player_id as i32).cloned()) else {
            return Ok(1);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(1);
        };
        let rank = player_guard.get_rank_level();
        Ok(rank.max(1))
    }

    fn get_player_base_center(&self, player_id: u32) -> Result<Coord3D, AiError> {
        let Some(player_arc) = player_list().read().ok().and_then(|list| list.get_player(player_id as i32).cloned()) else {
            return Ok(Vec3::new(0.0, 0.0, 0.0));
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(Vec3::new(0.0, 0.0, 0.0));
        };
        let mut sum = Coord3D::new(0.0, 0.0, 0.0);
        let mut count = 0.0;
        for obj_id in player_guard.get_all_objects() {
            let Some(pos) = OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                if !obj_guard.is_kind_of(KindOf::Structure)
                    && !obj_guard.is_kind_of(KindOf::Building)
                {
                    return None;
                }
                Some(*obj_guard.get_position())
            }).flatten() else {
                continue;
            };
            sum.x += pos.x;
            sum.y += pos.y;
            sum.z += pos.z;
            count += 1.0;
        }
        if count > 0.0 {
            Ok(Vec3::new(sum.x / count, sum.y / count, sum.z / count))
        } else {
            Ok(Vec3::new(0.0, 0.0, 0.0))
        }
    }

    fn calculate_distance(&self, pos1: Coord3D, pos2: Coord3D) -> f32 {
        // Use Vec3's built-in distance method for efficiency
        pos1.distance(pos2)
    }

    fn get_builder_position(&self, builder_id: ObjectID) -> Result<Coord3D, AiError> {
        Ok(OBJECT_REGISTRY
            .with_object(builder_id, |obj_guard| {
                let pos = obj_guard.get_position();
                Vec3::new(pos.x, pos.y, pos.z)
            })
            .unwrap_or(Vec3::new(0.0, 0.0, 0.0)))
    }

    fn find_idle_builders(&self, player_id: u32) -> Result<Vec<ObjectID>, AiError> {
        let Some(player_arc) = player_list().read().ok().and_then(|list| list.get_player(player_id as i32).cloned()) else {
            return Ok(Vec::new());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(Vec::new());
        };
        let mut builders = Vec::new();
        for obj_id in player_guard.get_all_objects() {
            let idle_dozer = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if !obj_guard.is_kind_of(KindOf::Dozer) {
                        return false;
                    }
                    let Some(ai) = obj_guard.get_ai_update_interface() else {
                        return false;
                    };
                    ai.lock()
                        .ok()
                        .map(|ai_guard| ai_guard.is_idle())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if idle_dozer {
                builders.push(obj_id);
            }
        }
        Ok(builders)
    }

    fn compute_threat_level(&self, player_id: u32) -> Result<f32, AiError> {
        let base_center = self.get_player_base_center(player_id)?;
        let mut threat = 0.0;
        let mut total = 0.0;
        // Host path: empty dual-world registry residual.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(0.0);
        }
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                continue;
            };
            if owner_id as u32 == player_id {
                continue;
            }
            if let Some(player) = player_list().read().ok().and_then(|list| list.get_player(owner_id as i32).cloned()) {
                if let Ok(enemy_guard) = player.read() {
                    if enemy_guard.get_player_type() == PlayerType::Neutral {
                        continue;
                    }
                }
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
            let dist_sq = dx * dx + dy * dy;
            let cost = obj_guard.get_template().calc_cost_to_build(None).max(1) as f32;
            total += cost;
            if dist_sq < (200.0 * 200.0) {
                threat += cost;
            }
        }
        if total > 0.0 {
            Ok((threat / total).clamp(0.0, 1.0))
        } else {
            Ok(0.0)
        }
    }

    fn compute_map_control(&self, player_id: u32) -> Result<f32, AiError> {
        let mut mine = 0.0;
        let mut total = 0.0;
        // Host path: empty dual-world registry residual.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(0.0);
        }
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                continue;
            };
            if let Some(player) = player_list().read().ok().and_then(|list| list.get_player(owner_id as i32).cloned()) {
                if let Ok(owner_guard) = player.read() {
                    if owner_guard.get_player_type() == PlayerType::Neutral {
                        continue;
                    }
                }
            }
            total += 1.0;
            if owner_id as u32 == player_id {
                mine += 1.0;
            }
        }
        if total > 0.0 {
            Ok((mine / total).clamp(0.0, 1.0))
        } else {
            Ok(0.0)
        }
    }

    fn count_player_units(&self, player_id: u32, template: &str, building_only: bool) -> Result<i32, AiError> {
        let Some(player_arc) = player_list().read().ok().and_then(|list| list.get_player(player_id as i32).cloned()) else {
            return Ok(0);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(0);
        };
        let mut count = 0;
        for obj_id in player_guard.get_all_objects() {
            let matches = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if building_only {
                        if !obj_guard.is_kind_of(KindOf::Structure)
                            && !obj_guard.is_kind_of(KindOf::Building)
                        {
                            return false;
                        }
                    }
                    obj_guard.get_template_name() == template
                })
                .unwrap_or(false);
            if matches {
                count += 1;
            }
        }
        Ok(count)
    }

    fn count_nearby_buildings(
        &self,
        player_id: u32,
        position: Coord3D,
        min_spacing: f32,
    ) -> Result<i32, AiError> {
        let Some(player_arc) = player_list().read().ok().and_then(|list| list.get_player(player_id as i32).cloned()) else {
            return Ok(0);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(0);
        };
        let mut count = 0;
        for obj_id in player_guard.get_all_objects() {
            let near = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if !obj_guard.is_kind_of(KindOf::Structure)
                        && !obj_guard.is_kind_of(KindOf::Building)
                    {
                        return false;
                    }
                    let pos = obj_guard.get_position();
                    let dx = pos.x - position.x;
                    let dy = pos.y - position.y;
                    dx * dx + dy * dy < min_spacing * min_spacing
                })
                .unwrap_or(false);
            if near {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get construction statistics for a player
    pub fn get_construction_stats(&self, player_id: u32) -> Option<&ConstructionStats> {
        self.construction_stats.get(&player_id)
    }

    /// Set maximum concurrent builds for a player
    pub fn set_max_concurrent_builds(&mut self, player_id: u32, max_builds: u32) {
        self.max_concurrent_builds.insert(player_id, max_builds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_priority_ordering() {
        assert!(BuildPriority::Emergency < BuildPriority::Critical);
        assert!(BuildPriority::Critical < BuildPriority::High);
        assert!(BuildPriority::High < BuildPriority::Normal);
        assert!(BuildPriority::Normal < BuildPriority::Low);
        assert!(BuildPriority::Low < BuildPriority::Deferred);
    }

    #[test]
    fn test_ai_build_list_creation() {
        let build_list = AIBuildList::new();
        assert!(!build_list.build_configs.is_empty());
        assert!(build_list.build_configs.contains_key("USA_Turtle"));
        assert!(build_list.build_configs.contains_key("USA_Rush"));
        assert!(build_list.build_configs.contains_key("China_Economic"));
        assert!(build_list.build_configs.contains_key("GLA_Harassment"));
    }

    #[test]
    fn test_build_requirements() {
        let mut requirements = BuildRequirements::default();
        requirements.required_resources.insert("Money".to_string(), 1000);
        requirements.power_requirement = 50;
        requirements.build_time = 600;
        
        assert_eq!(requirements.required_resources.get("Money"), Some(&1000));
        assert_eq!(requirements.power_requirement, 50);
        assert_eq!(requirements.build_time, 600);
    }

    #[test]
    fn test_build_conditions() {
        let condition = BuildCondition::MinimumResources(
            [("Money".to_string(), 500)].iter().cloned().collect()
        );
        
        if let BuildCondition::MinimumResources(resources) = condition {
            assert_eq!(resources.get("Money"), Some(&500));
        } else {
            panic!("Wrong condition type");
        }
    }

    #[test]
    fn test_construction_site_evaluation() {
        let site = ConstructionSite {
            position: [100.0, 200.0, 0.0],
            suitability_score: 0.8,
            terrain_factors: TerrainFactors::default(),
            strategic_factors: StrategicFactors::default(),
            risk_assessment: RiskAssessment::default(),
            accessibility: 1.0,
        };
        
        assert_eq!(site.position, [100.0, 200.0, 0.0]);
        assert_eq!(site.suitability_score, 0.8);
        assert_eq!(site.accessibility, 1.0);
    }

    #[test]
    fn test_build_queue_operations() {
        let mut build_list = AIBuildList::new();
        let player_id = 1;
        
        // Initially, build queue should be empty
        assert!(!build_list.build_queues.contains_key(&player_id));
        
        // After update, queue should exist
        build_list.update_build_queue(player_id, 0).unwrap();
        assert!(build_list.build_queues.contains_key(&player_id));
    }
}
