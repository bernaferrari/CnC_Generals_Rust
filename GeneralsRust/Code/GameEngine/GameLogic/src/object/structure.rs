//! Structure class - Buildings and static installations
//!
//! Structures are stationary objects that provide various functions like
//! production, research, resource generation, defense, etc.

use crate::build_list_info::BuildListInfo;
use crate::commands::command_processor::PlayerManager;
use crate::commands::commands::move_objects_to_position;
use crate::common::ObjectID;
use crate::common::UpgradeMaskType;
use crate::common::*;
use crate::economy::{ResourceProduction, ResourceType};
use crate::object::object_factory::{get_object_factory, ObjectCreationFlags};
use crate::object::{Object, TriggerInfo};
use crate::player::{player_list, Player, PlayerIndex};
use crate::special_power::{SpecialPowerTemplate, SpecialPowerType};
use crate::system::game_logic::current_frame;
use crate::team::Team;
use crate::upgrade::UpgradeTemplate;
use crate::weapon::{WeaponChoiceCriteria, WeaponSet, WeaponSlotType};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

/// Types of structures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureType {
    CommandCenter,     // Main base building
    PowerPlant,        // Provides power
    Refinery,          // Processes resources
    Factory,           // Produces units
    Research,          // Provides technology upgrades
    Defense,           // Defensive structure
    Support,           // Support building (repair, etc.)
    Super,             // Superweapon
    Wall,              // Defensive barrier
    Gate,              // Defensive barrier with passage
    Bunker,            // Garrison structure
    ResourceExtractor, // Extracts resources from deposits
    Airport,           // Aircraft production/landing
    Naval,             // Naval production/docking
    Civilian,          // Civilian structures
}

/// Construction states for buildings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionState {
    Planning,          // Being placed but not started
    UnderConstruction, // Currently being built
    Complete,          // Fully constructed and operational
    Repairing,         // Being repaired
    Upgrading,         // Being upgraded
    Demolished,        // Being demolished/sold
    Destroyed,         // Destroyed in combat
}

/// Production states for factory buildings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionState {
    Idle,      // Not producing anything
    Producing, // Currently producing an item
    Paused,    // Production paused
    Blocked,   // Cannot continue production (no power, resources, etc.)
    Completed, // Item finished, waiting for delivery
}

/// Structure-specific data and behavior
#[derive(Debug)]
#[allow(dead_code)]
pub struct Structure {
    /// Base object functionality
    base_object: Arc<RwLock<Object>>,

    /// Structure classification
    #[allow(dead_code)]
    structure_type: StructureType,
    is_faction_structure: bool,
    is_key_structure: bool, // Losing this ends the game

    /// Construction and health
    construction_state: ConstructionState,
    build_cost: HashMap<ResourceType, u32>,
    build_time: Real,
    construction_progress: Real,
    repair_rate: Real,

    /// Power and resources
    power_provided: i32, // Positive for generators, negative for consumers
    power_required: i32,
    is_powered: bool,
    resource_production: HashMap<ResourceType, ResourceProduction>,
    resource_storage: HashMap<ResourceType, u32>,
    resource_storage_capacity: HashMap<ResourceType, u32>,

    /// Production capabilities
    production_state: ProductionState,
    production_queue: VecDeque<ProductionItem>,
    max_queue_size: usize,
    production_rate_multiplier: Real,
    can_rally_point: bool,
    rally_point: Option<Coord3D>,

    /// Research and upgrades
    available_upgrades: Vec<Arc<UpgradeTemplate>>,
    research_queue: VecDeque<Arc<UpgradeTemplate>>,
    upgrade_discounts: HashMap<String, Real>,

    /// Special powers
    special_powers: HashMap<SpecialPowerType, SpecialPowerData>,

    /// Defensive capabilities
    garrison_capacity: usize,
    garrisoned_units: Vec<ObjectID>,
    garrison_types_allowed: Vec<KindOf>,
    fire_ports: Vec<FirePort>,
    detection_range: Real,
    reveals_stealth: bool,

    /// Area effects
    provides_healing: bool,
    healing_range: Real,
    healing_rate: Real,
    provides_repairs: bool,
    repair_range: Real,
    aura_effects: Vec<AuraEffect>,

    /// Placement and terrain
    foundation_size: Coord2D,
    placement_restrictions: Vec<PlacementRestriction>,
    requires_flat_terrain: bool,
    can_be_built_on_water: bool,
    build_height_offset: Real,

    /// Connections and dependencies
    connected_structures: Vec<ObjectID>,
    requires_connection: bool,
    connection_range: Real,

    /// Selling and demolition
    can_be_sold: bool,
    sell_refund_percentage: Real,
    demolition_weapon: Option<String>,

    /// Visual and animation
    construction_animations: Vec<String>,
    idle_animations: Vec<String>,
    active_animations: Vec<String>,
    damage_states: Vec<DamageState>,

    /// Worker/Builder management
    builders: Vec<ObjectID>,
    max_builders: usize,
    build_efficiency_per_builder: Real,

    /// Veterancy effects
    veteran_bonuses: HashMap<VeterancyLevel, VeterancyBonus>,

    /// Capturable structures
    can_be_captured: bool,
    capture_resistance: Real,
    capture_progress: Real,
    capturing_player: Option<PlayerId>,
    last_capture_frame: UnsignedInt,
}

/// Production item in the queue
#[derive(Debug, Clone)]
pub struct ProductionItem {
    pub template_name: String,
    pub build_cost: HashMap<ResourceType, u32>,
    pub build_time: Real,
    pub progress: Real,
    pub is_paused: bool,
    pub priority: i32,
}

/// Special power data for structures
#[derive(Debug, Clone)]
pub struct SpecialPowerData {
    pub template: Arc<SpecialPowerTemplate>,
    pub cooldown_remaining: Real,
    pub is_available: bool,
    pub charge_progress: Real,
}

/// Fire port for garrisoned units
#[derive(Debug, Clone)]
pub struct FirePort {
    pub position: Coord3D,
    pub facing: Real,
    pub arc: Real, // Firing arc in radians
    pub occupied_by: Option<ObjectID>,
}

/// Area effect that the structure provides
#[derive(Debug, Clone)]
pub struct AuraEffect {
    pub effect_type: AuraEffectType,
    pub range: Real,
    pub strength: Real,
    pub affects_allies: bool,
    pub affects_enemies: bool,
    pub affects_structures: bool,
    pub affects_units: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraEffectType {
    HealingAura,
    RepairAura,
    AttackSpeedBonus,
    DefenseBonus,
    VisionBonus,
    StealthDetection,
    ResourceBonus,
    ProductionSpeedBonus,
    PowerBonus,
    MoraleBonus,
}

/// Damage state for visual representation
#[derive(Debug, Clone)]
pub struct DamageState {
    pub health_threshold: Real,
    pub model_condition: String,
    pub particle_effects: Vec<String>,
    pub sound_effects: Vec<String>,
}

/// Placement restriction for building
#[derive(Debug, Clone)]
pub enum PlacementRestriction {
    NearWater(Real),                            // Must be within distance of water
    NearResource(ResourceType, Real),           // Must be near resource
    NearFriendlyStructure(StructureType, Real), // Must be near specific structure type
    AwayFromEnemies(Real),                      // Must be away from enemies
    OnSpecificTerrain(TerrainType),             // Must be on specific terrain
    RequiresClearLOS,                           // Requires clear line of sight
}

/// Veterancy bonus for structures
#[derive(Debug, Clone)]
pub struct VeterancyBonus {
    pub health_bonus: Real,
    pub armor_bonus: Real,
    pub weapon_damage_bonus: Real,
    pub production_speed_bonus: Real,
    pub resource_production_bonus: Real,
    pub special_abilities: Vec<String>,
}

impl Structure {
    pub fn base_object(&self) -> Arc<RwLock<Object>> {
        Arc::clone(&self.base_object)
    }

    /// Create a new Structure
    pub fn new(
        base_object: Arc<RwLock<Object>>,
        thing_template: &dyn ThingTemplate,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let structure_type = Self::determine_structure_type(thing_template);
        let is_faction_structure = thing_template.is_kind_of(KindOf::Structure);
        let can_be_captured = thing_template.is_kind_of(KindOf::Capturable)
            && !thing_template.is_kind_of(KindOf::ImmuneToCapture);

        Ok(Structure {
            base_object,
            structure_type,
            is_faction_structure,
            is_key_structure: thing_template.is_kind_of(KindOf::KeyStructure),

            construction_state: ConstructionState::Planning,
            build_cost: HashMap::new(),
            build_time: 0.0,
            construction_progress: 0.0,
            repair_rate: 0.0,

            power_provided: 0,
            power_required: 0,
            is_powered: true,
            resource_production: HashMap::new(),
            resource_storage: HashMap::new(),
            resource_storage_capacity: HashMap::new(),

            production_state: ProductionState::Idle,
            production_queue: VecDeque::new(),
            max_queue_size: 0,
            production_rate_multiplier: 1.0,
            can_rally_point: false,
            rally_point: None,

            available_upgrades: Vec::new(),
            research_queue: VecDeque::new(),
            upgrade_discounts: HashMap::new(),

            special_powers: HashMap::new(),

            garrison_capacity: 0,
            garrisoned_units: Vec::new(),
            garrison_types_allowed: Vec::new(),
            fire_ports: Vec::new(),
            detection_range: 0.0,
            reveals_stealth: false,

            provides_healing: false,
            healing_range: 0.0,
            healing_rate: 0.0,
            provides_repairs: false,
            repair_range: 0.0,
            aura_effects: Vec::new(),

            foundation_size: Coord2D::new(1.0, 1.0),
            placement_restrictions: Vec::new(),
            requires_flat_terrain: false,
            can_be_built_on_water: false,
            build_height_offset: 0.0,

            connected_structures: Vec::new(),
            requires_connection: false,
            connection_range: 0.0,

            can_be_sold: thing_template.is_kind_of(KindOf::Structure),
            sell_refund_percentage: 0.5,
            demolition_weapon: None,

            construction_animations: Vec::new(),
            idle_animations: Vec::new(),
            active_animations: Vec::new(),
            damage_states: Vec::new(),

            builders: Vec::new(),
            max_builders: 0,
            build_efficiency_per_builder: 1.0,

            veteran_bonuses: HashMap::new(),

            can_be_captured,
            capture_resistance: 1.0,
            capture_progress: 0.0,
            capturing_player: None,
            last_capture_frame: 0,
        })
    }

    /// Update structure logic for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update construction
        self.update_construction(delta_time)?;

        // Update power before gated systems consume their frame.
        self.update_power_state()?;

        // Update production
        self.update_production(delta_time)?;

        // Update resource production
        self.update_resource_production(delta_time)?;

        // Update special powers
        self.update_special_powers(delta_time)?;

        // Update aura effects
        self.update_aura_effects(delta_time)?;

        // Update garrison
        self.update_garrison(delta_time)?;

        // Update capture progress
        self.update_capture_progress(delta_time)?;

        Ok(())
    }

    /// Start construction of the structure
    pub fn start_construction(
        &mut self,
        builders: Vec<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.construction_state == ConstructionState::Planning {
            self.construction_state = ConstructionState::UnderConstruction;
            self.builders = builders;
            self.construction_progress = 0.0;

            // Set initial health to a small value
            if let Ok(mut obj_guard) = self.base_object.write() {
                let _ = obj_guard.set_health(1.0);
            }
        }
        Ok(())
    }

    /// Complete construction
    pub fn complete_construction(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.construction_state = ConstructionState::Complete;
        self.construction_progress = 1.0;

        // Set to full health
        if let Ok(mut obj_guard) = self.base_object.write() {
            let _ = obj_guard.heal_completely();
        }

        // Initialize resource production
        self.initialize_resource_production()?;

        // Enable aura effects
        self.activate_aura_effects()?;

        Ok(())
    }

    /// Add item to production queue
    pub fn add_to_production_queue(
        &mut self,
        template_name: String,
        build_cost: HashMap<ResourceType, u32>,
        build_time: Real,
        priority: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if self.production_queue.len() >= self.max_queue_size {
            return Ok(false); // Queue full
        }

        let item = ProductionItem {
            template_name,
            build_cost,
            build_time,
            progress: 0.0,
            is_paused: false,
            priority,
        };

        // Insert based on priority
        for (index, existing_item) in self.production_queue.iter().enumerate() {
            if priority > existing_item.priority {
                self.production_queue.insert(index, item.clone());
                self.debit_production_cost(&item);
                return Ok(true);
            }
        }

        self.production_queue.push_back(item);
        if let Some(back) = self.production_queue.back().cloned() {
            self.debit_production_cost(&back);
        }

        Ok(true)
    }

    /// Cancel production item
    pub fn cancel_production_item(
        &mut self,
        index: usize,
    ) -> Result<Option<ProductionItem>, Box<dyn std::error::Error + Send + Sync>> {
        if index < self.production_queue.len() {
            let item = self.production_queue.remove(index).unwrap();

            // Refund partial resources if production was in progress
            self.refund_production_cost(&item);

            Ok(Some(item))
        } else {
            Ok(None)
        }
    }

    /// Garrison a unit in the structure
    pub fn garrison_unit(
        &mut self,
        unit_id: ObjectID,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if self.garrisoned_units.len() >= self.garrison_capacity {
            return Ok(false); // No space
        }

        // Check if unit type is allowed
        // This would check the actual unit's KindOf flags

        self.garrisoned_units.push(unit_id);
        Ok(true)
    }

    /// Ungarrison a unit from the structure
    pub fn ungarrison_unit(
        &mut self,
        unit_id: ObjectID,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(pos) = self.garrisoned_units.iter().position(|&id| id == unit_id) {
            self.garrisoned_units.remove(pos);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Ungarrison all units
    pub fn ungarrison_all(&mut self) -> Vec<ObjectID> {
        let units = self.garrisoned_units.clone();
        self.garrisoned_units.clear();
        units
    }

    /// Set rally point for produced units
    pub fn set_rally_point(
        &mut self,
        position: Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_rally_point {
            self.rally_point = Some(position);
        }
        Ok(())
    }

    /// Sell the structure
    pub fn sell(
        &mut self,
    ) -> Result<HashMap<ResourceType, u32>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_be_sold {
            return Ok(HashMap::new());
        }

        let mut refund = HashMap::new();

        // C++ parity: sell refund is a flat percentage of build cost,
        // independent of current health. (Default SellPercentage = 0.5)
        let refund_multiplier = self.sell_refund_percentage;

        for (resource_type, cost) in &self.build_cost {
            let refund_amount = (*cost as f32 * refund_multiplier) as u32;
            if refund_amount > 0 {
                refund.insert(*resource_type, refund_amount);
            }
        }

        // Mark for demolition
        self.construction_state = ConstructionState::Demolished;

        Ok(refund)
    }

    /// Check if structure is operational
    pub fn is_operational(&self) -> bool {
        match self.construction_state {
            ConstructionState::Complete => self.is_powered && !self.is_destroyed(),
            _ => false,
        }
    }

    /// Check if structure can produce
    pub fn can_produce(&self) -> bool {
        self.is_operational() && self.production_state != ProductionState::Blocked
    }

    /// Get current health percentage
    pub fn get_health_percentage(&self) -> Real {
        if let Ok(obj_guard) = self.base_object.read() {
            let current = obj_guard.get_health();
            let max = obj_guard.get_max_health();
            if max > 0.0 {
                current / max
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Check if structure is destroyed
    pub fn is_destroyed(&self) -> bool {
        if let Ok(obj_guard) = self.base_object.read() {
            obj_guard.is_destroyed()
        } else {
            true
        }
    }

    /// Private helper methods
    fn determine_structure_type(thing_template: &dyn ThingTemplate) -> StructureType {
        // This would analyze the template's KindOf flags to determine type
        if thing_template.is_kind_of(KindOf::CommandCenter) {
            StructureType::CommandCenter
        } else if thing_template.is_kind_of(KindOf::PowerPlant) {
            StructureType::PowerPlant
        } else if thing_template.is_kind_of(KindOf::Refinery) {
            StructureType::Refinery
        } else if thing_template.is_kind_of(KindOf::Factory) {
            StructureType::Factory
        } else if thing_template.is_kind_of(KindOf::Defense) {
            StructureType::Defense
        } else {
            StructureType::Support
        }
    }

    fn update_construction(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.construction_state == ConstructionState::UnderConstruction {
            let builder_count = self.builders.len().min(self.max_builders);
            if builder_count > 0 {
                let build_rate = builder_count as Real * self.build_efficiency_per_builder;
                self.construction_progress += (build_rate * delta_time) / self.build_time;

                if self.construction_progress >= 1.0 {
                    self.complete_construction()?;
                }

                // Update health based on construction progress
                if let Ok(mut obj_guard) = self.base_object.write() {
                    let max_health = obj_guard.get_max_health();
                    let target_health = max_health * self.construction_progress;
                    let _ = obj_guard.set_health(target_health);
                }
            }
        }
        Ok(())
    }

    fn update_production(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let can_produce = self.can_produce();
        if let Some(current_item) = self.production_queue.front_mut() {
            if !current_item.is_paused && can_produce {
                current_item.progress +=
                    (delta_time * self.production_rate_multiplier) / current_item.build_time;

                if current_item.progress >= 1.0 {
                    // Item completed
                    let completed_item = self.production_queue.pop_front().unwrap();
                    self.spawn_produced_item(completed_item)?;
                }
            }
        }
        Ok(())
    }

    fn update_resource_production(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn update_power_state(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Ok(obj_guard) = self.base_object.read() else {
            self.is_powered = false;
            return Ok(());
        };

        if obj_guard.is_disabled_by_type(DisabledType::DisabledUnderpowered)
            || obj_guard.is_disabled_by_type(DisabledType::DisabledScriptUnderpowered)
        {
            self.is_powered = false;
            return Ok(());
        }

        let template_power = obj_guard.get_template().get_energy_production();
        let template_demand = if template_power < 0 {
            -template_power
        } else {
            0
        };
        let required_power = self.power_required.max(template_demand);
        if required_power <= 0 {
            self.is_powered = true;
            return Ok(());
        }

        self.is_powered = obj_guard
            .get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|player_guard| player_guard.get_energy().has_sufficient_power())
            })
            .unwrap_or(false);
        Ok(())
    }

    fn update_special_powers(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for (_, power_data) in &mut self.special_powers {
            if power_data.cooldown_remaining > 0.0 {
                power_data.cooldown_remaining -= delta_time;
                if power_data.cooldown_remaining <= 0.0 {
                    power_data.is_available = true;
                }
            }
        }
        Ok(())
    }

    fn update_aura_effects(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_operational() {
            // Apply aura effects to nearby objects
            // This would involve querying the spatial partition system
        }
        Ok(())
    }

    fn update_garrison(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update garrisoned units - they might provide firing capability
        // or other benefits to the structure
        Ok(())
    }

    fn update_capture_progress(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_be_captured {
            return Ok(());
        }

        if let Some(player_id) = self.capturing_player {
            let active_frame = self.last_capture_frame == current_frame();
            if active_frame {
                let resistance = self.capture_resistance.max(0.01);
                self.capture_progress += delta_time / resistance;
                if self.capture_progress >= 1.0 {
                    self.complete_capture(player_id)?;
                }
            } else if self.capture_progress > 0.0 {
                // Capture progress would decay over time if not actively being captured
                self.capture_progress -= delta_time * 0.1; // Decay rate
                if self.capture_progress <= 0.0 {
                    self.capture_progress = 0.0;
                    self.capturing_player = None;
                }
            }
        } else if self.capture_progress > 0.0 {
            // Decay any lingering progress if capture stopped.
            self.capture_progress -= delta_time * 0.1;
            if self.capture_progress <= 0.0 {
                self.capture_progress = 0.0;
            }
        }
        Ok(())
    }

    fn initialize_resource_production(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Initialize resource production based on structure type
        // This would be set up based on the template
        Ok(())
    }

    fn activate_aura_effects(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Activate any aura effects this structure provides
        Ok(())
    }

    fn refund_production_cost(&mut self, item: &ProductionItem) {
        // Calculate partial refund based on progress and return resources
        let refund_percentage = item.progress * 0.75; // 75% refund for in-progress items

        if let Some(owner) = self
            .base_object
            .read()
            .ok()
            .and_then(|o| o.get_player_id())
            .map(|pid| pid.get())
        {
            for (_resource_type, cost) in &item.build_cost {
                let refund_amount = (*cost as f32 * refund_percentage) as u32;
                if refund_amount > 0 {
                    if let Some(lock) = crate::player::player_list().write().ok() {
                        let mut manager = lock;
                        manager.modify_player_resources(owner as Int, refund_amount as Int, 0);
                    }
                }
            }
        }
    }

    fn debit_production_cost(&mut self, item: &ProductionItem) {
        if let Some(owner) = self
            .base_object
            .read()
            .ok()
            .and_then(|o| o.get_player_id())
            .map(|pid| pid.get())
        {
            for (_resource_type, cost) in &item.build_cost {
                if let Some(lock) = crate::player::player_list().write().ok() {
                    let mut manager = lock;
                    manager.modify_player_resources(owner as Int, -(*cost as Int), 0);
                }
            }
        }
    }

    fn spawn_produced_item(
        &mut self,
        item: ProductionItem,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Spawn the completed item at the structure's exit point or rally point
        // In full parity this would create a new Object from the template and insert into world.
        let exit_pos = self
            .rally_point
            .or_else(|| self.base_object.read().ok().map(|o| *o.get_position()));

        if let Some(exit_pos) = exit_pos {
            let team = self.base_object.read().ok().and_then(|o| o.get_team());
            let controlling_player = team
                .as_ref()
                .and_then(|t| t.read().ok())
                .and_then(|t| t.get_controlling_player_id());

            let created = get_object_factory().write().ok().and_then(|mut factory| {
                factory
                    .create_object(
                        &item.template_name,
                        exit_pos,
                        team,
                        ObjectCreationFlags::empty(),
                    )
                    .ok()
            });

            if let Some(new_id) = created {
                log::debug!(
                    "Structure completed {} -> spawned object {} toward rally {:?}",
                    item.template_name,
                    new_id,
                    exit_pos
                );
                if let (Some(rally), Some(player_id)) = (self.rally_point, controlling_player) {
                    if let Err(err) = move_objects_to_position(
                        vec![new_id],
                        rally,
                        player_id as i32,
                        current_frame(),
                    ) {
                        log::warn!(
                            "Failed to queue rally move for {} to {:?}: {}",
                            new_id,
                            rally,
                            err
                        );
                    }
                }
            } else {
                log::warn!(
                    "Failed to spawn produced item '{}' at {:?}",
                    item.template_name,
                    exit_pos
                );
            }
        }
        Ok(())
    }

    /// Mark capture activity for this structure by a player (C++ capture progress tick).
    pub fn mark_capture_activity(
        &mut self,
        player_id: PlayerId,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_be_captured {
            return Ok(false);
        }

        if self.capturing_player != Some(player_id) {
            self.capturing_player = Some(player_id);
            self.capture_progress = 0.0;
        }

        self.last_capture_frame = current_frame();
        Ok(true)
    }

    fn complete_capture(
        &mut self,
        player_id: PlayerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id.get() as PlayerIndex).cloned());

        let new_team = player_arc
            .as_ref()
            .and_then(|player| player.read().ok())
            .and_then(|player| player.get_default_team());

        let old_owner = self
            .base_object
            .read()
            .ok()
            .and_then(|obj| obj.get_controlling_player());

        if let Ok(mut obj_guard) = self.base_object.write() {
            let _ = obj_guard.set_team(new_team);
            obj_guard.set_captured(true);
            let new_owner = obj_guard.get_controlling_player();
            obj_guard.on_capture(old_owner, new_owner);
        }

        self.capture_progress = 0.0;
        self.capturing_player = None;
        Ok(())
    }
}

/// Extension trait for Object to provide Structure-specific functionality
pub trait StructureExt {
    /// Get structure-specific data if this object is a structure
    fn as_structure(&self) -> Option<&Structure>;
    fn as_structure_mut(&mut self) -> Option<&mut Structure>;
}

// This would need to be implemented for the actual Object type
// impl StructureExt for Object {
//     fn as_structure(&self) -> Option<&Structure> {
//         // Implementation would check if this object is actually a structure
//         None
//     }
//
//     fn as_structure_mut(&mut self) -> Option<&mut Structure> {
//         // Implementation would check if this object is actually a structure
//         None
//     }
// }

// Additional types required by other modules

/// Technology building bonus information
#[derive(Debug, Clone)]
pub struct TechBuildingBonus {
    pub bonus_type: TechBonusType,
    pub bonus_value: f32,
    pub prerequisite_upgrade: Option<String>,
}

/// Types of technology bonuses provided by buildings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechBonusType {
    /// Increases unit production speed
    ProductionSpeed,
    /// Reduces unit build costs
    BuildCost,
    /// Improves unit armor
    Armor,
    /// Improves weapon damage
    WeaponDamage,
    /// Increases unit sight range
    SightRange,
    /// Improves unit speed
    Speed,
    /// Provides healing capability
    Healing,
}

/// Bridge-specific data for structures that act as bridges
#[derive(Debug, Clone)]
pub struct BridgeData {
    pub start_position: Coord3D,
    pub end_position: Coord3D,
    pub bridge_height: Real,
    pub is_destroyed: bool,
    pub repair_cost: i32,
}

impl BridgeData {
    pub fn new(start: Coord3D, end: Coord3D, height: Real) -> Self {
        Self {
            start_position: start,
            end_position: end,
            bridge_height: height,
            is_destroyed: false,
            repair_cost: 0,
        }
    }
}

/// Civilian-specific data for civilian buildings
#[derive(Debug, Clone)]
pub struct CivilianData {
    pub building_type: String,
    pub population: i32,
    pub max_population: i32,
    pub income_generation: Real,
    pub can_be_captured: bool,
    pub capture_value: i32,
}

impl CivilianData {
    pub fn new(building_type: String, max_pop: i32) -> Self {
        Self {
            building_type,
            population: max_pop,
            max_population: max_pop,
            income_generation: 0.0,
            can_be_captured: true,
            capture_value: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn test_state_lock() -> std::sync::MutexGuard<'static, ()> {
        static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_STATE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap()
    }

    fn owned_test_object(object_id: ObjectID, player_index: PlayerIndex) -> Arc<RwLock<Object>> {
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        let team = Arc::new(RwLock::new(Team::new(
            format!("PowerTeam{player_index}").into(),
            object_id + 1000,
        )));
        team.write()
            .unwrap()
            .set_controlling_player_id(Some(player_index as UnsignedInt));
        object.write().unwrap().set_team(Some(team)).unwrap();
        object
    }

    #[test]
    fn structure_update_refreshes_power_before_production() {
        let _guard = test_state_lock();
        player_list().write().unwrap().clear();

        let player = Arc::new(RwLock::new(Player::new(0)));
        player.write().unwrap().adjust_power(-1, true);
        player_list().write().unwrap().add_player(player);

        let object = owned_test_object(8101, 0);
        let template = object.read().unwrap().get_template().clone();
        let mut structure = Structure::new(object, template.as_ref()).unwrap();
        structure.construction_state = ConstructionState::Complete;
        structure.is_powered = true;
        structure.power_required = 1;
        structure.production_state = ProductionState::Producing;
        structure.production_queue.push_back(ProductionItem {
            template_name: "TestTank".to_string(),
            build_cost: HashMap::new(),
            build_time: 1.0,
            progress: 0.0,
            is_paused: false,
            priority: 0,
        });

        structure.update(0.5).unwrap();

        assert!(!structure.is_powered);
        assert_eq!(structure.production_queue.front().unwrap().progress, 0.0);

        player_list().write().unwrap().clear();
    }

    #[test]
    fn structure_power_tracks_underpowered_disabled_flags() {
        let _guard = test_state_lock();
        player_list().write().unwrap().clear();

        let player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut player_guard = player.write().unwrap();
            player_guard.adjust_power(3, true);
            player_guard.adjust_power(-1, true);
        }
        player_list().write().unwrap().add_player(player);

        let object = owned_test_object(8102, 0);
        let template = object.read().unwrap().get_template().clone();
        let mut structure = Structure::new(Arc::clone(&object), template.as_ref()).unwrap();
        structure.construction_state = ConstructionState::Complete;
        structure.power_required = 1;

        structure.update_power_state().unwrap();
        assert!(structure.is_powered);

        object
            .write()
            .unwrap()
            .set_disabled(DisabledType::DisabledScriptUnderpowered);
        structure.update_power_state().unwrap();
        assert!(!structure.is_powered);

        player_list().write().unwrap().clear();
    }
}
