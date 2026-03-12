//! Economy and Resource Management System
//!
//! This module provides a comprehensive economy system for Command & Conquer Generals Zero Hour,
//! including resource gathering, power management, supply lines, and economic analysis.
//!
//! Integrates with the supply_system module for complete supply collection gameplay.

use crate::common::{Coord3D, KindOf, ObjectID, PlayerId, Relationship, INVALID_ID};
use crate::helpers::ThePartitionManager;
use crate::object::registry::OBJECT_REGISTRY;
use crate::supply_system::{
    AutoDepositUpdate, Money, PlayerSupplyManager, ResourceGatheringManager,
    SupplyCenterDockUpdate, SupplyTruckAIUpdate, SupplyWarehouseDockUpdate,
};
use crate::{GameLogicError, GameLogicResult};

type ObjectId = ObjectID;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

/// Resource types in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// Cash/Money - primary resource
    Money,
    /// Power/Electricity
    Power,
    /// Command points for unit limits
    Command,
    /// Experience points
    Experience,
    /// General's promotion points
    GeneralPoints,
}

/// Represents a continuous resource production source.
#[derive(Debug, Clone, Copy)]
pub struct ResourceProduction {
    resource: ResourceType,
    rate_per_second: f32,
    accumulated: f32,
}

impl ResourceProduction {
    pub fn new(resource: ResourceType, rate_per_second: f32) -> Self {
        Self {
            resource,
            rate_per_second,
            accumulated: 0.0,
        }
    }

    pub fn resource_type(&self) -> ResourceType {
        self.resource
    }

    /// Advance the production timer.
    pub fn update(&mut self, delta_time: f32) {
        self.accumulated += self.rate_per_second * delta_time.max(0.0);
    }

    /// Harvest whole units of the produced resource.
    pub fn harvest(&mut self) -> i32 {
        let produced = self.accumulated.floor() as i32;
        self.accumulated -= produced as f32;
        produced
    }

    /// Adjust the base production rate.
    pub fn set_rate(&mut self, new_rate: f32) {
        self.rate_per_second = new_rate.max(0.0);
    }

    pub fn rate(&self) -> f32 {
        self.rate_per_second
    }
}

/// Resource income source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomeSource {
    /// Unknown or unspecified source
    Unknown,
    /// Supply Center passive income
    SupplyCenter,
    /// Supply Docks passive income
    SupplyDocks,
    /// Captured supply piles
    SupplyPile,
    /// Oil derricks
    OilDerrick,
    /// Black market income
    BlackMarket,
    /// Salvage operations
    Salvage,
    /// Hacker income
    Hacker,
    /// Arms dealer sales
    ArmsDealerSales,
    /// Refunds (canceled production, sellbacks, etc.)
    Refund,
}

/// Power generation source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerSource {
    /// Cold Fusion Reactor
    ColdFusionReactor,
    /// Nuclear Reactor
    NuclearReactor,
    /// Toxin Generator
    ToxinGenerator,
    /// Solar Panel
    SolarPanel,
    /// Wind Generator
    WindGenerator,
}

/// Resource storage for a player
#[derive(Debug, Clone)]
pub struct ResourceStorage {
    /// Current resource amounts
    pub resources: HashMap<ResourceType, i32>,
    /// Maximum storage capacity
    pub storage_capacity: HashMap<ResourceType, i32>,
    /// Income rates per second
    pub income_rates: HashMap<ResourceType, f32>,
    /// Upkeep costs per second
    pub upkeep_costs: HashMap<ResourceType, f32>,
}

/// Economic building information
#[derive(Debug, Clone)]
pub struct EconomicBuilding {
    /// Object ID of the building
    pub object_id: ObjectId,
    /// Building position
    pub position: Coord3D,
    /// Type of income this building provides
    pub income_source: Option<IncomeSource>,
    /// Type of power this building provides
    pub power_source: Option<PowerSource>,
    /// Base income rate
    pub base_income_rate: f32,
    /// Base power generation
    pub base_power_generation: f32,
    /// Current efficiency (0.0 to 1.0)
    pub efficiency: f32,
    /// Whether building is currently functional
    pub is_functional: bool,
    /// Whether building needs power to function
    pub needs_power: bool,
    /// Power consumption when active
    pub power_consumption: f32,
    /// Upgrade level
    pub upgrade_level: u32,
    /// Health percentage affecting efficiency
    pub health_percentage: f32,
}

/// Supply line connection between economic buildings
#[derive(Debug, Clone)]
pub struct SupplyLine {
    /// Source building
    pub source_id: ObjectId,
    /// Destination building
    pub destination_id: ObjectId,
    /// Distance between buildings
    pub distance: f32,
    /// Efficiency of the supply line
    pub efficiency: f32,
    /// Whether the supply line is currently disrupted
    pub is_disrupted: bool,
    /// Security level (affected by enemy presence)
    pub security_level: f32,
}

/// Economic analysis data
#[derive(Debug, Clone)]
pub struct EconomicMetrics {
    /// Total income per minute
    pub income_per_minute: HashMap<ResourceType, f32>,
    /// Total expenses per minute
    pub expenses_per_minute: HashMap<ResourceType, f32>,
    /// Net income per minute
    pub net_income_per_minute: HashMap<ResourceType, f32>,
    /// Resource efficiency rating (0.0 to 1.0)
    pub resource_efficiency: f32,
    /// Economic growth rate
    pub growth_rate: f32,
    /// Time to reach resource target
    pub time_to_target: HashMap<ResourceType, f32>,
    /// Economic vulnerability score
    pub vulnerability_score: f32,
}

/// Economic event for logging and analysis
#[derive(Debug, Clone)]
pub enum EconomicEvent {
    /// Income received
    IncomeReceived {
        resource_type: ResourceType,
        amount: i32,
        source: IncomeSource,
    },
    /// Resource spent
    ResourceSpent {
        resource_type: ResourceType,
        amount: i32,
        purpose: String,
    },
    /// Building constructed
    BuildingConstructed {
        building_id: ObjectId,
        building_type: String,
        cost: HashMap<ResourceType, i32>,
    },
    /// Building destroyed
    BuildingDestroyed {
        building_id: ObjectId,
        lost_income: f32,
        lost_power: f32,
    },
    /// Supply line disrupted
    SupplyLineDisrupted {
        source_id: ObjectId,
        destination_id: ObjectId,
        impact: f32,
    },
    /// Power shortage
    PowerShortage {
        shortage_amount: f32,
        affected_buildings: Vec<ObjectId>,
    },
}

/// Main economy manager
pub struct EconomyManager {
    // Manual Debug impl below due to Mutex<VecDeque<(Instant, EconomicEvent)>>
    /// Player resource storage
    player_resources: HashMap<u32, Arc<RwLock<ResourceStorage>>>,
    /// Economic buildings by player
    player_buildings: HashMap<u32, Vec<EconomicBuilding>>,
    /// Supply lines by player
    player_supply_lines: HashMap<u32, Vec<SupplyLine>>,
    /// Economic metrics by player
    player_metrics: HashMap<u32, EconomicMetrics>,
    /// Economic event history
    event_history: Mutex<VecDeque<(Instant, EconomicEvent)>>,
    /// Global economic modifiers
    global_modifiers: HashMap<String, f32>,
    /// Market prices for trading
    market_prices: HashMap<ResourceType, f32>,
    /// Player supply managers (new supply system integration)
    player_supply_managers: HashMap<u32, Arc<RwLock<PlayerSupplyManager>>>,
}

impl std::fmt::Debug for EconomyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EconomyManager")
            .field("player_resources", &self.player_resources.len())
            .field("player_buildings", &self.player_buildings.len())
            .field("player_supply_lines", &self.player_supply_lines.len())
            .field("player_metrics", &self.player_metrics.len())
            .field("global_modifiers", &self.global_modifiers.len())
            .field("market_prices", &self.market_prices.len())
            .field("player_supply_managers", &self.player_supply_managers.len())
            .finish()
    }
}

impl EconomyManager {
    /// Create a new economy manager
    pub fn new() -> Self {
        Self {
            player_resources: HashMap::new(),
            player_buildings: HashMap::new(),
            player_supply_lines: HashMap::new(),
            player_metrics: HashMap::new(),
            event_history: Mutex::new(VecDeque::new()),
            global_modifiers: HashMap::new(),
            market_prices: HashMap::new(),
            player_supply_managers: HashMap::new(),
        }
    }

    /// Get or create player supply manager
    pub fn get_player_supply_manager(
        &mut self,
        player_id: u32,
    ) -> Arc<RwLock<PlayerSupplyManager>> {
        self.player_supply_managers
            .entry(player_id)
            .or_insert_with(|| Arc::new(RwLock::new(PlayerSupplyManager::new(player_id, 10000))))
            .clone()
    }

    /// Get player's money (new supply system)
    pub fn get_player_money(&mut self, player_id: u32) -> Option<Arc<RwLock<Money>>> {
        if let Some(manager) = self.player_supply_managers.get(&player_id) {
            if let Ok(mgr) = manager.read() {
                return Some(mgr.get_money());
            }
        }
        None
    }

    /// Get player's resource gathering manager (new supply system)
    pub fn get_player_resource_manager(
        &mut self,
        player_id: u32,
    ) -> Option<Arc<RwLock<ResourceGatheringManager>>> {
        if let Some(manager) = self.player_supply_managers.get(&player_id) {
            if let Ok(mgr) = manager.read() {
                return Some(mgr.get_resource_manager());
            }
        }
        None
    }

    /// Initialize a player's economy
    pub fn initialize_player_economy(
        &mut self,
        player_id: u32,
        starting_resources: HashMap<ResourceType, i32>,
    ) -> GameLogicResult<()> {
        let mut resources = HashMap::new();
        let mut storage_capacity = HashMap::new();
        let mut income_rates = HashMap::new();
        let mut upkeep_costs = HashMap::new();

        // Set up starting resources
        for (&resource_type, &amount) in &starting_resources {
            resources.insert(resource_type, amount);

            // Set default storage capacities
            let default_capacity = match resource_type {
                ResourceType::Money => i32::MAX,
                ResourceType::Power => 10000,
                ResourceType::Command => 100,
                ResourceType::Experience => 999999,
                ResourceType::GeneralPoints => 10000,
            };
            storage_capacity.insert(resource_type, default_capacity);

            income_rates.insert(resource_type, 0.0);
            upkeep_costs.insert(resource_type, 0.0);
        }

        let resource_storage = ResourceStorage {
            resources,
            storage_capacity,
            income_rates,
            upkeep_costs,
        };

        self.player_resources
            .insert(player_id, Arc::new(RwLock::new(resource_storage)));
        self.player_buildings.insert(player_id, Vec::new());
        self.player_supply_lines.insert(player_id, Vec::new());
        self.player_metrics
            .insert(player_id, EconomicMetrics::default());

        log::info!("Initialized economy for player {}", player_id);
        Ok(())
    }

    /// Add credits (money) to a player's storage, mirroring the C++ economy manager.
    pub fn add_credits(
        &mut self,
        player_id: PlayerId,
        amount: i32,
        source: IncomeSource,
        spend_purpose: Option<String>,
    ) -> GameLogicResult<()> {
        if amount == 0 {
            return Ok(());
        }

        let player_key = player_id.as_u32();
        let resources_arc = if let Some(existing) = self.player_resources.get(&player_key).cloned()
        {
            existing
        } else {
            let list_guard = crate::player::ThePlayerList()
                .read()
                .map_err(|_| GameLogicError::SystemNotInitialized("PlayerList".to_string()))?;
            let player_arc = list_guard
                .get_player(player_key as i32)
                .ok_or_else(|| GameLogicError::InvalidObject(player_key))?;
            let player_guard = player_arc
                .read()
                .map_err(|_| GameLogicError::Threading("Player lock poisoned".to_string()))?;
            let current_money = player_guard.get_money().get_money();
            let mut starting_resources = HashMap::new();
            starting_resources.insert(
                ResourceType::Money,
                (current_money as i64).min(i32::MAX as i64) as i32,
            );
            self.initialize_player_economy(player_key, starting_resources)?;
            self.player_resources
                .get(&player_key)
                .cloned()
                .ok_or_else(|| GameLogicError::InvalidObject(player_key))?
        };

        let mut storage = resources_arc.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire resource lock: {}", e))
        })?;

        let mut actual_delta = 0;
        if amount > 0 {
            let cap = storage
                .storage_capacity
                .get(&ResourceType::Money)
                .copied()
                .unwrap_or(i32::MAX);
            let entry = storage.resources.entry(ResourceType::Money).or_insert(0);
            let original = *entry;
            let updated = entry.saturating_add(amount).min(cap);
            *entry = updated;
            let actual_added = updated.saturating_sub(original);
            actual_delta = actual_added;
            if actual_added > 0 {
                self.log_economic_event(EconomicEvent::IncomeReceived {
                    resource_type: ResourceType::Money,
                    amount: actual_added,
                    source,
                });
            }
        } else {
            let entry = storage.resources.entry(ResourceType::Money).or_insert(0);
            let spend = amount.abs();
            let actual_spent = (*entry).min(spend);
            *entry = (*entry).saturating_sub(actual_spent);
            actual_delta = -(actual_spent as i32);
            if actual_spent > 0 {
                self.log_economic_event(EconomicEvent::ResourceSpent {
                    resource_type: ResourceType::Money,
                    amount: actual_spent,
                    purpose: spend_purpose.unwrap_or_else(|| "Unspecified spend".to_string()),
                });
            }
        }
        let current_money = *storage.resources.get(&ResourceType::Money).unwrap_or(&0);

        if actual_delta != 0 {
            if let Ok(list) = crate::player::ThePlayerList().read() {
                if let Some(player) = list.get_player(player_key as i32) {
                    if let Ok(mut player_guard) = player.write() {
                        if actual_delta > 0 {
                            let actual_added = actual_delta as u32;
                            let _ = player_guard.get_money_mut().deposit(actual_added);
                            player_guard
                                .get_score_keeper_mut()
                                .add_money_earned(actual_added);
                        } else {
                            let spend = (-actual_delta) as u32;
                            if spend > 0 {
                                let _ = player_guard.get_money_mut().withdraw(spend);
                                player_guard.get_score_keeper_mut().add_money_spent(spend);
                            }
                        }
                        // Keep player money in sync with storage, even if other systems changed it.
                        player_guard.get_money_mut().set_money(current_money);
                    }
                }
            }
        }

        Ok(())
    }

    /// Update the economy system for one frame
    pub fn update(&mut self, delta_time: f32) -> GameLogicResult<()> {
        let player_ids: Vec<u32> = self.player_resources.keys().copied().collect();
        for player_id in player_ids {
            // Update resource income
            self.update_player_income(player_id, delta_time)?;

            // Update power grid
            self.update_power_grid(player_id)?;

            // Update supply lines
            self.update_supply_lines(player_id)?;

            // Calculate economic metrics
            self.calculate_economic_metrics(player_id)?;

            // Process economic events
            self.process_economic_events(player_id)?;
        }

        // Clean old events
        self.cleanup_old_events();

        Ok(())
    }

    /// Update player resource income
    fn update_player_income(&mut self, player_id: u32, delta_time: f32) -> GameLogicResult<()> {
        let resources_arc = self
            .player_resources
            .get(&player_id)
            .ok_or_else(|| GameLogicError::InvalidObject(player_id))?
            .clone();

        // Calculate total income from all sources.
        let empty_buildings = Vec::new();
        let buildings = self
            .player_buildings
            .get(&player_id)
            .unwrap_or(&empty_buildings);
        let mut income_entries: Vec<(ResourceType, IncomeSource, i32)> = Vec::new();
        let mut income_totals: HashMap<ResourceType, f32> = HashMap::new();

        for building in buildings {
            if building.is_functional && building.efficiency > 0.0 {
                let income_amount = building.base_income_rate * building.efficiency * delta_time;

                // Apply power efficiency.
                let power_efficiency = self.calculate_power_efficiency(player_id, building)?;
                let final_income = income_amount * power_efficiency;
                if final_income <= 0.0 {
                    continue;
                }

                if let Some(income_source) = building.income_source {
                    let resource_type = self.get_resource_type_for_income_source(income_source);
                    let amount = final_income as i32;
                    if amount > 0 {
                        income_entries.push((resource_type, income_source, amount));
                    }
                    *income_totals.entry(resource_type).or_insert(0.0) += final_income;
                }
            }
        }

        {
            let mut resources = resources_arc.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire resource lock: {}", e))
            })?;

            // Apply non-money income and check storage limits.
            for (resource_type, source, amount) in income_entries
                .iter()
                .filter(|(resource_type, _, _)| *resource_type != ResourceType::Money)
            {
                let current = resources.resources.get(resource_type).copied().unwrap_or(0);
                let capacity = resources
                    .storage_capacity
                    .get(resource_type)
                    .copied()
                    .unwrap_or(100000);
                let new_amount = (current + *amount).min(capacity);
                let actual_added = new_amount - current;

                resources.resources.insert(*resource_type, new_amount);

                if actual_added > 0 {
                    self.log_economic_event(EconomicEvent::IncomeReceived {
                        resource_type: *resource_type,
                        amount: actual_added,
                        source: *source,
                    });
                }
            }

            for (resource_type, rate) in &income_totals {
                resources
                    .income_rates
                    .insert(*resource_type, (*rate).max(0.0));
            }
            for resource_type in [
                ResourceType::Money,
                ResourceType::Command,
                ResourceType::Experience,
                ResourceType::GeneralPoints,
            ] {
                let rate = income_totals.get(&resource_type).copied().unwrap_or(0.0);
                resources.income_rates.insert(resource_type, rate);
            }

            // Apply upkeep costs.
            for (&resource_type, &upkeep) in &resources.upkeep_costs.clone() {
                if upkeep > 0.0 {
                    let upkeep_amount = (upkeep * delta_time) as i32;
                    let current = resources
                        .resources
                        .get(&resource_type)
                        .copied()
                        .unwrap_or(0);
                    let new_amount = (current - upkeep_amount).max(0);

                    resources.resources.insert(resource_type, new_amount);
                }
            }
        }

        // Apply money income via add_credits to keep player money in sync.
        if !income_entries.is_empty() {
            let player = PlayerId::new(player_id as u8)
                .ok_or_else(|| GameLogicError::InvalidObject(player_id))?;
            for (_, source, amount) in income_entries
                .iter()
                .filter(|(resource_type, _, _)| *resource_type == ResourceType::Money)
            {
                self.add_credits(player, *amount, *source, None)?;
            }
        }

        Ok(())
    }

    /// Update power grid for a player
    fn update_power_grid(&mut self, player_id: u32) -> GameLogicResult<()> {
        let mut total_power_generation = 0.0;
        let mut total_power_consumption = 0.0;
        let mut power_event: Option<(f32, Vec<ObjectId>)> = None;
        {
            let buildings = self
                .player_buildings
                .get_mut(&player_id)
                .ok_or_else(|| GameLogicError::InvalidObject(player_id))?;

            // Calculate total power generation
            for building in buildings.iter() {
                if building.is_functional {
                    // Power generation
                    if building.power_source.is_some() {
                        total_power_generation += building.base_power_generation
                            * building.efficiency
                            * building.health_percentage;
                    }

                    // Power consumption
                    if building.needs_power {
                        total_power_consumption += building.power_consumption;
                    }
                }
            }

            // Check for power shortage
            let power_deficit = total_power_consumption - total_power_generation;
            if power_deficit > 0.0 {
                let affected_buildings: Vec<ObjectId> = buildings
                    .iter()
                    .filter(|b| b.needs_power)
                    .map(|b| b.object_id)
                    .collect();

                // Reduce efficiency of power-dependent buildings
                let efficiency_reduction = 1.0 - (power_deficit / total_power_consumption).min(1.0);
                for building in buildings.iter_mut() {
                    if building.needs_power {
                        building.efficiency *= efficiency_reduction;
                    }
                }

                power_event = Some((power_deficit, affected_buildings));
            }
        }

        if let Some((power_deficit, affected_buildings)) = power_event {
            self.log_economic_event(EconomicEvent::PowerShortage {
                shortage_amount: power_deficit,
                affected_buildings,
            });
        }

        // Update resource storage with power information
        if let Some(resources_arc) = self.player_resources.get(&player_id) {
            let mut resources = resources_arc.write().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire resource lock: {}", e))
            })?;

            resources
                .resources
                .insert(ResourceType::Power, total_power_generation as i32);
            resources.income_rates.insert(
                ResourceType::Power,
                total_power_generation - total_power_consumption,
            );
        }

        Ok(())
    }

    /// Update supply lines for a player
    fn update_supply_lines(&mut self, player_id: u32) -> GameLogicResult<()> {
        let mut supply_lines = self
            .player_supply_lines
            .remove(&player_id)
            .ok_or_else(|| GameLogicError::InvalidObject(player_id))?;

        for line in supply_lines.iter_mut() {
            let source_id = line.source_id;
            let destination_id = line.destination_id;
            let was_disrupted = line.is_disrupted;

            let source_exists = self.building_exists(player_id, source_id);
            let destination_exists = self.building_exists(player_id, destination_id);

            if !source_exists || !destination_exists {
                line.is_disrupted = true;
                continue;
            }

            let efficiency = self.calculate_supply_line_efficiency(line);
            let disrupted = self.check_supply_line_security(line)?;

            line.efficiency = efficiency;

            let event_to_log = if disrupted && !was_disrupted {
                line.is_disrupted = true;
                Some(EconomicEvent::SupplyLineDisrupted {
                    source_id,
                    destination_id,
                    impact: efficiency,
                })
            } else if !disrupted && was_disrupted {
                line.is_disrupted = false;
                None
            } else {
                None
            };

            if let Some(event) = event_to_log {
                self.log_economic_event(event);
            }
        }

        self.player_supply_lines.insert(player_id, supply_lines);

        Ok(())
    }
    /// Calculate economic metrics for a player
    fn calculate_economic_metrics(&mut self, player_id: u32) -> GameLogicResult<()> {
        let resources_arc = self
            .player_resources
            .get(&player_id)
            .ok_or_else(|| GameLogicError::InvalidObject(player_id))?
            .clone();

        let resources = resources_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire resource lock: {}", e))
        })?;

        let mut metrics = EconomicMetrics::default();

        // Calculate income and expenses per minute
        for (&resource_type, &rate) in &resources.income_rates {
            metrics.income_per_minute.insert(resource_type, rate * 60.0);
        }

        for (&resource_type, &rate) in &resources.upkeep_costs {
            metrics
                .expenses_per_minute
                .insert(resource_type, rate * 60.0);
        }

        // Calculate net income
        for (&resource_type, &income) in &metrics.income_per_minute {
            let expense = metrics
                .expenses_per_minute
                .get(&resource_type)
                .copied()
                .unwrap_or(0.0);
            metrics
                .net_income_per_minute
                .insert(resource_type, income - expense);
        }

        // Calculate resource efficiency
        let empty_buildings = Vec::new();
        let buildings = self
            .player_buildings
            .get(&player_id)
            .unwrap_or(&empty_buildings);
        let total_buildings = buildings.len() as f32;
        let functional_buildings = buildings.iter().filter(|b| b.is_functional).count() as f32;
        metrics.resource_efficiency = if total_buildings > 0.0 {
            functional_buildings / total_buildings
        } else {
            1.0
        };

        // Calculate economic vulnerability
        metrics.vulnerability_score = self.calculate_vulnerability_score(player_id);

        self.player_metrics.insert(player_id, metrics);
        Ok(())
    }

    /// Add a new economic building
    pub fn add_economic_building(
        &mut self,
        player_id: u32,
        building: EconomicBuilding,
    ) -> GameLogicResult<()> {
        let mut cost = HashMap::new();
        let mut building_type = "Economic Building".to_string();
        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(building.object_id) {
            if let Ok(obj_guard) = obj_arc.read() {
                building_type = obj_guard.get_template_name().to_string();
                let build_cost = obj_guard.get_template().get_build_cost();
                if build_cost > 0 {
                    cost.insert(ResourceType::Money, build_cost);
                }
            }
        }
        if cost.is_empty() {
            cost.insert(ResourceType::Money, 0);
        }

        let event = EconomicEvent::BuildingConstructed {
            building_id: building.object_id,
            building_type,
            cost,
        };
        self.log_economic_event(event);

        self.player_buildings
            .entry(player_id)
            .or_insert_with(Vec::new)
            .push(building);
        Ok(())
    }

    /// Remove an economic building (when destroyed)
    pub fn remove_economic_building(
        &mut self,
        player_id: u32,
        building_id: ObjectId,
    ) -> GameLogicResult<()> {
        if let Some(buildings) = self.player_buildings.get_mut(&player_id) {
            if let Some(pos) = buildings.iter().position(|b| b.object_id == building_id) {
                let event = {
                    let building = buildings.remove(pos);
                    EconomicEvent::BuildingDestroyed {
                        building_id,
                        lost_income: building.base_income_rate,
                        lost_power: building.base_power_generation,
                    }
                };

                self.log_economic_event(event);
                return Ok(());
            }
        }

        Ok(())
    }

    /// Spend resources for a purchase
    pub fn spend_resources(
        &mut self,
        player_id: u32,
        costs: HashMap<ResourceType, i32>,
        purpose: String,
    ) -> GameLogicResult<bool> {
        let resources_arc = self
            .player_resources
            .get(&player_id)
            .ok_or_else(|| GameLogicError::InvalidObject(player_id))?
            .clone();

        let mut resources = resources_arc.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire resource lock: {}", e))
        })?;

        // Check if player has enough resources
        for (&resource_type, &cost) in &costs {
            let available = resources
                .resources
                .get(&resource_type)
                .copied()
                .unwrap_or(0);
            if available < cost {
                return Ok(false); // Not enough resources
            }
        }

        // Deduct resources
        for (&resource_type, &cost) in &costs {
            let current = resources
                .resources
                .get(&resource_type)
                .copied()
                .unwrap_or(0);
            resources.resources.insert(resource_type, current - cost);

            // Log expense
            self.log_economic_event(EconomicEvent::ResourceSpent {
                resource_type,
                amount: cost,
                purpose: purpose.clone(),
            });
        }

        Ok(true)
    }

    /// Get player resources
    pub fn get_player_resources(
        &self,
        player_id: u32,
    ) -> GameLogicResult<HashMap<ResourceType, i32>> {
        let resources_arc = self
            .player_resources
            .get(&player_id)
            .ok_or_else(|| GameLogicError::InvalidObject(player_id))?;

        let resources = resources_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire resource lock: {}", e))
        })?;

        Ok(resources.resources.clone())
    }

    /// Get economic metrics for a player
    pub fn get_economic_metrics(&self, player_id: u32) -> Option<&EconomicMetrics> {
        self.player_metrics.get(&player_id)
    }

    /// Helper methods

    fn calculate_power_efficiency(
        &self,
        player_id: u32,
        building: &EconomicBuilding,
    ) -> GameLogicResult<f32> {
        if !building.needs_power {
            return Ok(1.0);
        }

        if let Ok(list) = crate::player::ThePlayerList().read() {
            if let Some(player) = list.get_player(player_id as i32) {
                if let Ok(player_guard) = player.read() {
                    let ratio = player_guard.get_energy().supply_ratio() as f32;
                    return Ok(ratio.clamp(0.0, 1.0));
                }
            }
        }

        Ok(1.0)
    }

    fn get_resource_type_for_income_source(&self, source: IncomeSource) -> ResourceType {
        match source {
            IncomeSource::SupplyCenter
            | IncomeSource::SupplyDocks
            | IncomeSource::SupplyPile
            | IncomeSource::OilDerrick
            | IncomeSource::BlackMarket
            | IncomeSource::Salvage
            | IncomeSource::Hacker
            | IncomeSource::ArmsDealerSales
            | IncomeSource::Unknown
            | IncomeSource::Refund => ResourceType::Money,
        }
    }

    fn building_exists(&self, player_id: u32, building_id: ObjectId) -> bool {
        if let Some(buildings) = self.player_buildings.get(&player_id) {
            buildings.iter().any(|b| b.object_id == building_id)
        } else {
            false
        }
    }

    fn calculate_supply_line_efficiency(&self, supply_line: &SupplyLine) -> f32 {
        let distance_factor = (1.0 - (supply_line.distance / 1000.0).min(1.0)).max(0.1);
        let security_factor = supply_line.security_level;

        distance_factor * security_factor
    }

    fn check_supply_line_security(&self, _supply_line: &SupplyLine) -> GameLogicResult<bool> {
        let Some(source_arc) = OBJECT_REGISTRY.get_object(_supply_line.source_id) else {
            return Ok(true);
        };
        let Ok(source_guard) = source_arc.read() else {
            return Ok(true);
        };

        let source_pos = *source_guard.get_position();
        let dest_pos = OBJECT_REGISTRY
            .get_object(_supply_line.destination_id)
            .and_then(|arc| arc.read().ok().map(|g| *g.get_position()))
            .unwrap_or(source_pos);

        let mid = Coord3D::new(
            (source_pos.x + dest_pos.x) * 0.5,
            (source_pos.y + dest_pos.y) * 0.5,
            (source_pos.z + dest_pos.z) * 0.5,
        );

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(true);
        };
        let scan_radius = 200.0;
        for obj_id in partition.get_objects_in_range(&mid, scan_radius) {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_destroyed() {
                continue;
            }
            if !obj_guard.is_any_kind_of(&[
                KindOf::Infantry,
                KindOf::Vehicle,
                KindOf::Aircraft,
                KindOf::Unit,
            ]) {
                continue;
            }
            if source_guard.relationship_to(&*obj_guard) == Relationship::Enemy {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn calculate_vulnerability_score(&self, _player_id: u32) -> f32 {
        let buildings = self
            .player_buildings
            .get(&_player_id)
            .cloned()
            .unwrap_or_default();
        if buildings.is_empty() {
            return 0.0;
        }

        let total = buildings.len() as f32;
        let functional = buildings.iter().filter(|b| b.is_functional).count() as f32;
        let avg_health = buildings.iter().map(|b| b.health_percentage).sum::<f32>() / total;

        let disrupted_ratio = self
            .player_supply_lines
            .get(&_player_id)
            .map(|lines| {
                if lines.is_empty() {
                    0.0
                } else {
                    let disrupted = lines.iter().filter(|l| l.is_disrupted).count() as f32;
                    disrupted / lines.len() as f32
                }
            })
            .unwrap_or(0.0);

        let functional_penalty = 1.0 - (functional / total).clamp(0.0, 1.0);
        let health_penalty = 1.0 - avg_health.clamp(0.0, 1.0);

        ((functional_penalty + health_penalty + disrupted_ratio) / 3.0).clamp(0.0, 1.0)
    }

    fn process_economic_events(&self, _player_id: u32) -> GameLogicResult<()> {
        self.cleanup_old_events();
        Ok(())
    }

    fn cleanup_old_events(&self) {
        // Remove events older than 5 minutes
        if let Ok(mut history) = self.event_history.lock() {
            let cutoff = Instant::now() - std::time::Duration::from_secs(300);
            while let Some((timestamp, _)) = history.front() {
                if *timestamp < cutoff {
                    history.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    fn log_economic_event(&self, event: EconomicEvent) {
        if let Ok(mut history) = self.event_history.lock() {
            history.push_back((Instant::now(), event));

            // Keep history size manageable
            if history.len() > 1000 {
                history.pop_front();
            }
        }
    }
}

impl ResourceStorage {
    /// Create default resource storage
    pub fn new() -> Self {
        let mut resources = HashMap::new();
        let mut storage_capacity = HashMap::new();
        let mut income_rates = HashMap::new();
        let mut upkeep_costs = HashMap::new();

        // Initialize all resource types
        for resource_type in [
            ResourceType::Money,
            ResourceType::Power,
            ResourceType::Command,
            ResourceType::Experience,
            ResourceType::GeneralPoints,
        ] {
            resources.insert(resource_type, 0);
            let capacity = match resource_type {
                ResourceType::Money => i32::MAX,
                _ => 100000,
            };
            storage_capacity.insert(resource_type, capacity);
            income_rates.insert(resource_type, 0.0);
            upkeep_costs.insert(resource_type, 0.0);
        }

        Self {
            resources,
            storage_capacity,
            income_rates,
            upkeep_costs,
        }
    }
}

impl Default for EconomicMetrics {
    fn default() -> Self {
        Self {
            income_per_minute: HashMap::new(),
            expenses_per_minute: HashMap::new(),
            net_income_per_minute: HashMap::new(),
            resource_efficiency: 1.0,
            growth_rate: 0.0,
            time_to_target: HashMap::new(),
            vulnerability_score: 0.0,
        }
    }
}

impl Default for EconomyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economy_initialization() {
        let mut economy = EconomyManager::new();
        let mut starting_resources = HashMap::new();
        starting_resources.insert(ResourceType::Money, 10000);
        starting_resources.insert(ResourceType::Power, 1000);

        let result = economy.initialize_player_economy(1, starting_resources);
        assert!(result.is_ok());

        let resources = economy.get_player_resources(1).unwrap();
        assert_eq!(resources[&ResourceType::Money], 10000);
        assert_eq!(resources[&ResourceType::Power], 1000);
    }

    #[test]
    fn test_resource_spending() {
        let mut economy = EconomyManager::new();
        let mut starting_resources = HashMap::new();
        starting_resources.insert(ResourceType::Money, 10000);

        economy
            .initialize_player_economy(1, starting_resources)
            .unwrap();

        let mut costs = HashMap::new();
        costs.insert(ResourceType::Money, 5000);

        let success = economy
            .spend_resources(1, costs, "Test Purchase".to_string())
            .unwrap();
        assert!(success);

        let resources = economy.get_player_resources(1).unwrap();
        assert_eq!(resources[&ResourceType::Money], 5000);
    }

    #[test]
    fn test_insufficient_resources() {
        let mut economy = EconomyManager::new();
        let mut starting_resources = HashMap::new();
        starting_resources.insert(ResourceType::Money, 1000);

        economy
            .initialize_player_economy(1, starting_resources)
            .unwrap();

        let mut costs = HashMap::new();
        costs.insert(ResourceType::Money, 5000);

        let success = economy
            .spend_resources(1, costs, "Expensive Purchase".to_string())
            .unwrap();
        assert!(!success);

        let resources = economy.get_player_resources(1).unwrap();
        assert_eq!(resources[&ResourceType::Money], 1000); // Should remain unchanged
    }

    #[test]
    fn test_building_management() {
        let mut economy = EconomyManager::new();
        economy
            .initialize_player_economy(1, HashMap::new())
            .unwrap();

        let building = EconomicBuilding {
            object_id: 100,
            position: [0.0, 0.0, 0.0].into(),
            income_source: Some(IncomeSource::SupplyCenter),
            power_source: None,
            base_income_rate: 10.0,
            base_power_generation: 0.0,
            efficiency: 1.0,
            is_functional: true,
            needs_power: false,
            power_consumption: 0.0,
            upgrade_level: 0,
            health_percentage: 1.0,
        };

        economy.add_economic_building(1, building).unwrap();

        let buildings = &economy.player_buildings[&1];
        assert_eq!(buildings.len(), 1);
        assert_eq!(buildings[0].object_id, 100);
    }
}
