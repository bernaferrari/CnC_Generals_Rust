//! Supply and Economy System
//!
//! Complete implementation of the C&C Generals supply collection and economy system.
//! Ports the C++ system from:
//! - SupplyCenterDockUpdate.cpp
//! - SupplyWarehouseDockUpdate.cpp
//! - SupplyTruckAIUpdate.cpp
//! - ResourceGatheringManager.cpp
//! - AutoDepositUpdate.cpp
//! - Player.cpp (money management)

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

use crate::action_manager::ActionManager;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, Coord3D as LogicCoord3D, KindOf, ModelConditionFlags, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::compat::{register_classic_state, ClassicState};
use crate::helpers::{
    FindPositionOptions, TheAudio, TheGameLogic, TheGameText, TheInGameUI, ThePartitionManager,
};
use crate::modules::{
    AIUpdateInterface, BodyModuleInterfaceExt, SupplyTruckAIInterface, WorkerAIUpdateInterface,
};
use crate::object::drawable::DrawableExt;
use crate::object::production::get_construction_manager;
use crate::object::Object;
use crate::player::player_list;
use crate::resource;
use crate::state_machine::{
    State, StateConditionInfo, StateExitType, StateImplementation, StateMachine, StateReturnType,
    StateTransitionUserData,
};
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer::Xfer;

pub type ObjectID = u32;
pub type PlayerIndex = u32;
pub type Real = f32;
pub type Color = u32;

pub const INVALID_ID: ObjectID = 0;
pub const BASE_VALUE_PER_SUPPLY_BOX: i32 = 100; // Matches C++ GlobalData::m_baseValuePerSupplyBox

// Faction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Faction {
    USA,
    China,
    GLA,
}

// ============================================================================
// EXTERNAL SYSTEM INTERFACES
// ============================================================================

/// Audio system interface
/// In production this would connect to the real audio system
pub trait AudioSystem: Send + Sync {
    /// Play money withdraw sound
    /// Matches C++ Money::withdraw() - TheAudio->addAudioEvent(&event)
    fn play_money_withdraw_sound(&self, player_index: PlayerIndex);

    /// Play money deposit sound
    /// Matches C++ Money::deposit() - TheAudio->addAudioEvent(&event)
    fn play_money_deposit_sound(&self, player_index: PlayerIndex);

    /// Play voice event
    /// Matches C++ SupplyTruckAIUpdate::gainOneBox() - TheAudio->addAudioEvent(&m_suppliesDepletedVoice)
    fn play_voice_event(&self, event_name: &str, object_id: ObjectID);
}

/// UI system interface for floating text
/// In production this would connect to InGameUI
pub trait UISystem: Send + Sync {
    /// Add floating text at position
    /// Matches C++ TheInGameUI->addFloatingText(moneys, &pos, color)
    /// From SupplyCenterDockUpdate.cpp:136 and AutoDepositUpdate.cpp:186
    fn add_floating_text(&self, text: &str, position: &Coord3D, color: Color);
}

/// Academy stats tracking interface
/// Matches C++ Player::getAcademyStats()->recordIncome()
pub trait AcademyStats: Send + Sync {
    /// Record income for statistics
    /// From Money.cpp:65
    fn record_income(&self);
}

/// Stealth system interface
/// Matches C++ StealthUpdate from SupplyCenterDockUpdate.cpp:92-108
pub trait StealthSystem: Send + Sync {
    /// Grant temporary stealth to an object
    /// Matches C++ stealth->receiveGrant(TRUE, frames)
    fn grant_temporary_stealth(&self, object_id: ObjectID, frames: u32);
}

/// Upgrade system interface
/// Matches C++ Player::hasUpgradeComplete(upgradeTemplate)
pub trait UpgradeSystem: Send + Sync {
    /// Check if player has a specific upgrade
    /// Returns bonus amount if upgrade is present, 0 otherwise
    /// Matches C++ WorkerAIUpdate::getUpgradedSupplyBoost() - WorkerAIUpdate.cpp:1376
    fn get_supply_boost(&self, player_index: PlayerIndex) -> u32;
}

/// 3D coordinate
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Coord3D {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn distance_to(&self, other: &Coord3D) -> Real {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn distance_squared_to(&self, other: &Coord3D) -> Real {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }
}

// ============================================================================
// MONEY SYSTEM
// ============================================================================

/// Player's money account
/// Matches C++ Money.cpp
pub struct Money {
    /// Current money amount
    money: u32,
    /// Player index
    player_index: PlayerIndex,
    /// Audio system reference (optional)
    audio_system: Option<Arc<dyn AudioSystem>>,
    /// Academy stats reference (optional)
    academy_stats: Option<Arc<dyn AcademyStats>>,
    /// Total money earned (for statistics)
    total_earned: u32,
    /// Total money spent (for statistics)
    total_spent: u32,
    /// Bounty from destroyed enemy units
    bounty_earned: u32,
    /// Salvage from crates and pickups
    salvage_earned: u32,
}

impl std::fmt::Debug for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Money")
            .field("money", &self.money)
            .field("player_index", &self.player_index)
            .field("total_earned", &self.total_earned)
            .field("total_spent", &self.total_spent)
            .field("bounty_earned", &self.bounty_earned)
            .field("salvage_earned", &self.salvage_earned)
            .field(
                "audio_system",
                &self.audio_system.as_ref().map(|_| "AudioSystem"),
            )
            .field(
                "academy_stats",
                &self.academy_stats.as_ref().map(|_| "AcademyStats"),
            )
            .finish()
    }
}

impl Money {
    pub fn new(player_index: PlayerIndex, starting_money: u32) -> Self {
        Self {
            money: starting_money,
            player_index,
            audio_system: None,
            academy_stats: None,
            total_earned: starting_money,
            total_spent: 0,
            bounty_earned: 0,
            salvage_earned: 0,
        }
    }

    /// Set audio system for sound playback
    pub fn set_audio_system(&mut self, audio_system: Arc<dyn AudioSystem>) {
        self.audio_system = Some(audio_system);
    }

    /// Set academy stats for income tracking
    pub fn set_academy_stats(&mut self, academy_stats: Arc<dyn AcademyStats>) {
        self.academy_stats = Some(academy_stats);
    }

    /// Withdraw money from account
    /// Matches C++ Money::withdraw() - Money.cpp:23
    pub fn withdraw(&mut self, amount_to_withdraw: u32, play_sound: bool) -> u32 {
        let actual_amount = if amount_to_withdraw > self.money {
            self.money
        } else {
            amount_to_withdraw
        };

        if actual_amount == 0 {
            return 0;
        }

        // Play sound if enabled
        // Matches C++ Money.cpp:32-37
        if play_sound {
            if let Some(audio) = &self.audio_system {
                audio.play_money_withdraw_sound(self.player_index);
            }
        }

        self.money -= actual_amount;
        self.total_spent += actual_amount;
        actual_amount
    }

    /// Deposit money into account
    /// Matches C++ Money::deposit() - Money.cpp:45
    pub fn deposit(&mut self, amount_to_deposit: u32, play_sound: bool) {
        if amount_to_deposit == 0 {
            return;
        }

        // Play sound if enabled
        // Matches C++ Money.cpp:51-56
        if play_sound {
            if let Some(audio) = &self.audio_system {
                audio.play_money_deposit_sound(self.player_index);
            }
        }

        self.money += amount_to_deposit;
        self.total_earned += amount_to_deposit;

        // Record income for academy stats
        // Matches C++ Money.cpp:60-67
        if amount_to_deposit > 0 {
            if let Some(stats) = &self.academy_stats {
                stats.record_income();
            }
        }
    }

    pub fn get_money(&self) -> u32 {
        self.money
    }

    pub fn set_money(&mut self, amount: u32) {
        self.money = amount;
    }

    pub fn can_afford(&self, cost: u32) -> bool {
        self.money >= cost
    }

    /// Award bounty for destroying enemy unit
    /// Bounty system matches C++ kill/bounty mechanics
    pub fn award_bounty(&mut self, bounty_amount: u32) {
        if bounty_amount > 0 {
            self.deposit(bounty_amount, false);
            self.bounty_earned += bounty_amount;
        }
    }

    /// Award salvage from crate pickup
    /// Crate system matches C++ MoneyCrateCollide.cpp
    pub fn award_salvage(&mut self, salvage_amount: u32) {
        if salvage_amount > 0 {
            self.deposit(salvage_amount, true);
            self.salvage_earned += salvage_amount;
        }
    }

    pub fn get_total_earned(&self) -> u32 {
        self.total_earned
    }

    pub fn get_total_spent(&self) -> u32 {
        self.total_spent
    }

    pub fn get_bounty_earned(&self) -> u32 {
        self.bounty_earned
    }

    pub fn get_salvage_earned(&self) -> u32 {
        self.salvage_earned
    }
}

// ============================================================================
// RESOURCE GATHERING MANAGER
// ============================================================================

/// Manages supply warehouses and centers for a player
/// Matches C++ ResourceGatheringManager.cpp
#[derive(Debug)]
pub struct ResourceGatheringManager {
    /// List of supply warehouse IDs
    supply_warehouses: Vec<ObjectID>,
    /// List of supply center IDs
    supply_centers: Vec<ObjectID>,
}

impl ResourceGatheringManager {
    pub fn new() -> Self {
        Self {
            supply_warehouses: Vec::new(),
            supply_centers: Vec::new(),
        }
    }

    /// Add a supply center
    /// Matches C++ ResourceGatheringManager::addSupplyCenter()
    pub fn add_supply_center(&mut self, center_id: ObjectID) {
        if !self.supply_centers.contains(&center_id) {
            self.supply_centers.push(center_id);
        }
    }

    /// Remove a supply center
    /// Matches C++ ResourceGatheringManager::removeSupplyCenter()
    pub fn remove_supply_center(&mut self, center_id: ObjectID) {
        self.supply_centers.retain(|&id| id != center_id);
    }

    /// Add a supply warehouse
    /// Matches C++ ResourceGatheringManager::addSupplyWarehouse()
    pub fn add_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        if !self.supply_warehouses.contains(&warehouse_id) {
            self.supply_warehouses.push(warehouse_id);
        }
    }

    /// Remove a supply warehouse
    /// Matches C++ ResourceGatheringManager::removeSupplyWarehouse()
    pub fn remove_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        self.supply_warehouses.retain(|&id| id != warehouse_id);
    }

    /// Find best supply warehouse for a truck
    /// Matches C++ ResourceGatheringManager::findBestSupplyWarehouse()
    pub fn find_best_supply_warehouse(
        &self,
        truck_position: &Coord3D,
        preferred_dock: Option<ObjectID>,
        max_distance: Real,
        warehouse_positions: &HashMap<ObjectID, Coord3D>,
        warehouse_available: &HashMap<ObjectID, bool>,
    ) -> Option<ObjectID> {
        // Check preferred dock first
        if let Some(preferred) = preferred_dock {
            if self.supply_warehouses.contains(&preferred) {
                if let Some(&available) = warehouse_available.get(&preferred) {
                    if available {
                        return Some(preferred);
                    }
                }
            }
        }

        // Find best warehouse by distance
        let max_distance_squared = max_distance * max_distance;
        let mut best_warehouse = None;
        let mut best_cost = Real::MAX;

        for &warehouse_id in &self.supply_warehouses {
            if let Some(&available) = warehouse_available.get(&warehouse_id) {
                if !available {
                    continue;
                }
            }

            if let Some(warehouse_pos) = warehouse_positions.get(&warehouse_id) {
                let distance_squared = truck_position.distance_squared_to(warehouse_pos);

                if distance_squared < best_cost && distance_squared < max_distance_squared {
                    best_warehouse = Some(warehouse_id);
                    best_cost = distance_squared;
                }
            }
        }

        best_warehouse
    }

    /// Find best supply center for a truck
    /// Matches C++ ResourceGatheringManager::findBestSupplyCenter()
    pub fn find_best_supply_center(
        &self,
        truck_position: &Coord3D,
        preferred_dock: Option<ObjectID>,
        center_positions: &HashMap<ObjectID, Coord3D>,
        center_available: &HashMap<ObjectID, bool>,
    ) -> Option<ObjectID> {
        // Check preferred dock first
        if let Some(preferred) = preferred_dock {
            if self.supply_centers.contains(&preferred) {
                if let Some(&available) = center_available.get(&preferred) {
                    if available {
                        return Some(preferred);
                    }
                }
            }
        }

        // Find best center by distance (no max distance limit for centers)
        let mut best_center = None;
        let mut best_cost = Real::MAX;

        for &center_id in &self.supply_centers {
            if let Some(&available) = center_available.get(&center_id) {
                if !available {
                    continue;
                }
            }

            if let Some(center_pos) = center_positions.get(&center_id) {
                let distance_squared = truck_position.distance_squared_to(center_pos);

                if distance_squared < best_cost {
                    best_center = Some(center_id);
                    best_cost = distance_squared;
                }
            }
        }

        best_center
    }

    pub fn get_supply_warehouses(&self) -> &[ObjectID] {
        &self.supply_warehouses
    }

    pub fn get_supply_centers(&self) -> &[ObjectID] {
        &self.supply_centers
    }
}

impl Default for ResourceGatheringManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SUPPLY WAREHOUSE DOCK UPDATE
// ============================================================================

/// Supply warehouse dock update module data
/// Matches C++ SupplyWarehouseDockUpdateModuleData
#[derive(Debug, Clone)]
pub struct SupplyWarehouseDockUpdateData {
    /// Number of supply boxes to start with
    pub starting_boxes: i32,
    /// Whether to delete the warehouse when empty
    pub delete_when_empty: bool,
    /// Number of approach positions
    pub num_approaches: usize,
    /// Action delay time in frames
    pub action_delay: u32,
}

impl Default for SupplyWarehouseDockUpdateData {
    fn default() -> Self {
        Self {
            starting_boxes: 1,
            delete_when_empty: false,
            num_approaches: 3,
            action_delay: 30,
        }
    }
}

/// Supply warehouse dock update module
/// Matches C++ SupplyWarehouseDockUpdate
#[derive(Debug)]
pub struct SupplyWarehouseDockUpdate {
    /// Configuration data
    data: SupplyWarehouseDockUpdateData,
    /// Current number of boxes stored
    boxes_stored: i32,
    /// Currently docked object
    active_docker: Option<ObjectID>,
    /// Whether docker is inside the warehouse
    docker_inside: bool,
    /// Whether dock is crippled
    is_crippled: bool,
}

impl SupplyWarehouseDockUpdate {
    pub fn new(data: SupplyWarehouseDockUpdateData) -> Self {
        let boxes_stored = data.starting_boxes;
        Self {
            data,
            boxes_stored,
            active_docker: None,
            docker_inside: false,
            is_crippled: false,
        }
    }

    /// Perform dock action - give boxes to truck
    /// Matches C++ SupplyWarehouseDockUpdate::action()
    pub fn action(&mut self, _docker_id: ObjectID) -> Result<bool, String> {
        if self.boxes_stored == 0 {
            return Ok(false);
        }

        // Decrease boxes (docker will see we're shy by one from within gainOneBox)
        self.boxes_stored -= 1;

        // Return true if truck successfully gained the box
        // The truck will call gainOneBox() to actually take it
        Ok(true)
    }

    /// Give one box to a truck
    /// Called by truck AI after action() succeeds
    pub fn give_box(&mut self) -> Option<i32> {
        if self.boxes_stored >= 0 {
            let remaining = self.boxes_stored;
            Some(remaining)
        } else {
            // Take it back if no one gained it
            self.boxes_stored += 1;
            None
        }
    }

    /// Set the cash value and calculate boxes needed
    /// Matches C++ SupplyWarehouseDockUpdate::setCashValue()
    pub fn set_cash_value(&mut self, cash_value: i32) {
        self.boxes_stored = (cash_value as f32 / BASE_VALUE_PER_SUPPLY_BOX as f32).ceil() as i32;
    }

    /// Set dock crippled state
    /// Matches C++ SupplyWarehouseDockUpdate::setDockCrippled()
    pub fn set_dock_crippled(&mut self, crippled: bool) {
        self.is_crippled = crippled;

        if crippled && self.active_docker.is_some() {
            // If docker is inside, kill it (handled by game logic)
            // If between approach and enter, tell it to stop and retry later
            // This is handled by the AI system
        }
    }

    pub fn get_boxes_stored(&self) -> i32 {
        self.boxes_stored
    }

    pub fn is_empty(&self) -> bool {
        self.boxes_stored == 0
    }

    pub fn should_delete_when_empty(&self) -> bool {
        self.data.delete_when_empty
    }

    pub fn set_active_docker(&mut self, docker_id: Option<ObjectID>, inside: bool) {
        self.active_docker = docker_id;
        self.docker_inside = inside;
    }
}

// ============================================================================
// SUPPLY CENTER DOCK UPDATE
// ============================================================================

/// Supply center dock update module data
/// Matches C++ SupplyCenterDockUpdateModuleData
#[derive(Debug, Clone)]
pub struct SupplyCenterDockUpdateData {
    /// Frames to grant temporary stealth (0 = disabled)
    pub grant_temporary_stealth_frames: u32,
    /// Number of approach positions
    pub num_approaches: usize,
    /// Action delay time in frames
    pub action_delay: u32,
}

impl Default for SupplyCenterDockUpdateData {
    fn default() -> Self {
        Self {
            grant_temporary_stealth_frames: 0,
            num_approaches: 3,
            action_delay: 30,
        }
    }
}

/// Supply center dock update module
/// Matches C++ SupplyCenterDockUpdate
pub struct SupplyCenterDockUpdate {
    /// Configuration data
    data: SupplyCenterDockUpdateData,
    /// Currently docked object
    active_docker: Option<ObjectID>,
    /// UI system reference (optional)
    ui_system: Option<Arc<dyn UISystem>>,
    /// Stealth system reference (optional)
    stealth_system: Option<Arc<dyn StealthSystem>>,
}

impl std::fmt::Debug for SupplyCenterDockUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupplyCenterDockUpdate")
            .field("data", &self.data)
            .field("active_docker", &self.active_docker)
            .field("ui_system", &self.ui_system.as_ref().map(|_| "UISystem"))
            .field(
                "stealth_system",
                &self.stealth_system.as_ref().map(|_| "StealthSystem"),
            )
            .finish()
    }
}

impl SupplyCenterDockUpdate {
    pub fn new(data: SupplyCenterDockUpdateData) -> Self {
        Self {
            data,
            active_docker: None,
            ui_system: None,
            stealth_system: None,
        }
    }

    /// Set UI system for floating text
    pub fn set_ui_system(&mut self, ui_system: Arc<dyn UISystem>) {
        self.ui_system = Some(ui_system);
    }

    /// Set stealth system for temporary stealth grants
    pub fn set_stealth_system(&mut self, stealth_system: Arc<dyn StealthSystem>) {
        self.stealth_system = Some(stealth_system);
    }

    /// Perform dock action - deposit money for boxes
    /// Matches C++ SupplyCenterDockUpdate::action() - SupplyCenterDockUpdate.cpp:64
    pub fn action(
        &mut self,
        docker_id: ObjectID,
        docker_position: &Coord3D,
        docker_boxes: &mut i32,
        supply_box_value: u32,
        upgraded_supply_boost: u32,
        player_money: &mut Money,
        player_color: Color,
    ) -> Result<bool, String> {
        let mut value = 0u32;

        // Take all boxes from the truck
        // Matches C++ SupplyCenterDockUpdate.cpp:77-80
        while *docker_boxes > 0 {
            *docker_boxes -= 1;
            value += supply_box_value;
        }

        // Add money boost from upgrades
        // Matches C++ SupplyCenterDockUpdate.cpp:82-83
        value += upgraded_supply_boost;

        if value > 0 {
            // Deposit money to player
            // Matches C++ SupplyCenterDockUpdate.cpp:87-89
            player_money.deposit(value, true);

            // Grant temporary stealth if configured
            // Matches C++ SupplyCenterDockUpdate.cpp:92-108
            if self.data.grant_temporary_stealth_frames > 0 {
                if let Some(stealth) = &self.stealth_system {
                    stealth.grant_temporary_stealth(
                        docker_id,
                        self.data.grant_temporary_stealth_frames,
                    );
                }
            }

            // Display floating text showing money gained
            // Matches C++ SupplyCenterDockUpdate.cpp:112-137
            // Format: "GUI:AddCash" = "+$%d"
            if let Some(ui) = &self.ui_system {
                let text = format!("+${}", value);
                // Color combines player color with alpha=230
                // Matches C++ SupplyCenterDockUpdate.cpp:134
                let color_with_alpha = player_color | 0xE6000000;
                ui.add_floating_text(&text, docker_position, color_with_alpha);
            }
        }

        self.active_docker = Some(docker_id);
        Ok(false) // Always return false (don't stay docked)
    }

    pub fn set_active_docker(&mut self, docker_id: Option<ObjectID>) {
        self.active_docker = docker_id;
    }
}

// ============================================================================
// SUPPLY TRUCK AI
// ============================================================================

/// Supply truck AI state
/// Matches C++ SupplyTruckAIUpdate states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyTruckState {
    /// Not doing anything, should autopilot?
    Idle,
    /// Direct player involvement, off autopilot
    Busy,
    /// Search for warehouse or center and dock with it
    Wanting,
    /// Wanting failed, hang out at base until something changes
    Regrouping,
    /// Docking substates running
    Docking,
}

const ST_IDLE: u32 = 0;
const ST_BUSY: u32 = 1;
const ST_WANTING: u32 = 2;
const ST_REGROUPING: u32 = 3;
const ST_DOCKING: u32 = 4;

const REGROUP_SUCCESS_DISTANCE_SQUARED: Real = 225.0;

fn owner_ai_and_truck(
    state: &State,
) -> Result<(Arc<RwLock<Object>>, Arc<Mutex<dyn AIUpdateInterface>>), String> {
    let owner = state
        .get_machine_owner()
        .ok_or_else(|| "SupplyTruck state missing owner".to_string())?;
    let ai = owner
        .read()
        .map_err(|_| "SupplyTruck owner lock poisoned".to_string())?
        .get_ai_update_interface()
        .ok_or_else(|| "SupplyTruck owner missing AIUpdateInterface".to_string())?;
    Ok((owner, ai))
}

fn with_supply_truck_interface<R>(
    state: &State,
    f: impl FnOnce(&mut dyn SupplyTruckAIInterface) -> R,
) -> Result<R, String> {
    let (_owner, ai) = owner_ai_and_truck(state)?;
    let mut ai_guard = ai
        .lock()
        .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
    let truck = ai_guard
        .get_supply_truck_ai_interface_mut()
        .ok_or_else(|| "SupplyTruck AI interface missing".to_string())?;
    Ok(f(truck))
}

#[derive(Debug)]
struct SupplyTruckBusyState {
    base: State,
}

impl SupplyTruckBusyState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "SupplyTruckBusyState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Err(err) = with_supply_truck_interface(&self.base, |truck| {
            truck.set_force_busy_state(false);
        }) {
            log::debug!("SupplyTruckBusyState::on_enter: {}", err);
        }
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for SupplyTruckBusyState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct SupplyTruckIdleState {
    base: State,
}

impl SupplyTruckIdleState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "SupplyTruckIdleState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for SupplyTruckIdleState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct SupplyTruckWantsToPickUpOrDeliverBoxesState {
    base: State,
}

impl SupplyTruckWantsToPickUpOrDeliverBoxesState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(
                Some(Arc::downgrade(machine)),
                "SupplyTruckWantsToPickUpOrDeliverBoxesState",
            ),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Err(err) = with_supply_truck_interface(&self.base, |truck| {
            truck.set_force_wanting_state(false);
        }) {
            log::debug!(
                "SupplyTruckWantsToPickUpOrDeliverBoxesState::on_enter: {}",
                err
            );
        }
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        let (owner, ai) = owner_ai_and_truck(&self.base)?;
        let owner_id = owner
            .read()
            .map_err(|_| "SupplyTruck owner lock poisoned".to_string())?
            .get_id();

        let mut ai_guard = ai
            .lock()
            .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
        let truck = ai_guard
            .get_supply_truck_ai_interface_mut()
            .ok_or_else(|| "SupplyTruck AI interface missing".to_string())?;

        if !truck.is_available_for_supplying() {
            return Ok(StateReturnType::Failure);
        }

        let num_boxes = truck.get_number_boxes();
        if num_boxes > 0 {
            if let Some(best_center) = resource::find_best_supply_center(owner_id) {
                let mut params =
                    AiCommandParams::new(AiCommandType::Dock, CommandSourceType::FromAi);
                params.obj = Some(best_center);
                if let Err(err) = ai_guard.execute_command(&params) {
                    log::debug!(
                        "SupplyTruckWantsToPickUpOrDeliverBoxesState::update dock(center) failed: {}",
                        err
                    );
                }
                return Ok(StateReturnType::Success);
            }
        } else if let Some(best_warehouse) = resource::find_best_supply_warehouse(owner_id) {
            let mut params = AiCommandParams::new(AiCommandType::Dock, CommandSourceType::FromAi);
            params.obj = Some(best_warehouse);
            if let Err(err) = ai_guard.execute_command(&params) {
                log::debug!(
                    "SupplyTruckWantsToPickUpOrDeliverBoxesState::update dock(warehouse) failed: {}",
                    err
                );
            }
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for SupplyTruckWantsToPickUpOrDeliverBoxesState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct RegroupingState {
    base: State,
}

impl RegroupingState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "RegroupingState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        let (owner, ai) = owner_ai_and_truck(&self.base)?;
        let owner_arc = owner.clone();

        {
            let mut ai_guard = ai
                .lock()
                .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
            if let Err(err) = ai_guard.ignore_obstacle(None) {
                log::debug!("RegroupingState::on_enter ignore_obstacle failed: {}", err);
            }
        }

        let owner_guard = owner_arc
            .read()
            .map_err(|_| "SupplyTruck owner lock poisoned".to_string())?;
        let owner_player_id = owner_guard
            .get_controlling_player_id()
            .ok_or_else(|| "SupplyTruck owner missing player".to_string())?;
        let owner_player = {
            let list_guard = player_list()
                .read()
                .map_err(|_| "Player list lock poisoned".to_string())?;
            list_guard
                .get_player(owner_player_id as i32)
                .cloned()
                .ok_or_else(|| "SupplyTruck owner player missing".to_string())?
        };
        let owner_player_guard = owner_player
            .read()
            .map_err(|_| "Player lock poisoned".to_string())?;

        let destination_object = find_regroup_target(&owner_guard, &owner_player_guard);
        let Some(destination_object) = destination_object else {
            return Ok(StateReturnType::Failure);
        };

        let destination_guard = destination_object
            .read()
            .map_err(|_| "Regroup target lock poisoned".to_string())?;
        let dist_sq = ThePartitionManager::get_distance_squared(
            &owner_guard,
            &destination_guard,
            crate::common::FROM_BOUNDING_SPHERE_2D,
        );
        if dist_sq < REGROUP_SUCCESS_DISTANCE_SQUARED {
            return Ok(StateReturnType::Continue);
        }

        let mut destination = LogicCoord3D::ZERO;
        let mut options = FindPositionOptions::default();
        options.min_radius = 0.0;
        options.max_radius = 100.0;

        let can_find_destination = ThePartitionManager::get()
            .map(|partition| {
                partition.find_position_around_with_options(
                    destination_guard.get_position(),
                    &options,
                    &mut destination,
                )
            })
            .unwrap_or(false);
        if !can_find_destination {
            return Ok(StateReturnType::Failure);
        }

        let mut ai_guard = ai
            .lock()
            .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;
        let mut params =
            AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
        params.pos = destination;
        if let Err(err) = ai_guard.execute_command(&params) {
            log::debug!("RegroupingState::on_enter move command failed: {}", err);
        }

        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        let (_owner, ai) = owner_ai_and_truck(&self.base)?;
        let ai_guard = ai
            .lock()
            .map_err(|_| "SupplyTruck AI lock poisoned".to_string())?;

        if ai_guard.is_idle() {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for RegroupingState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct DockingState {
    base: State,
}

impl DockingState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "DockingState"),
        }
    }

    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Err(err) = with_supply_truck_interface(&self.base, |truck| {
            truck.set_force_wanting_state(false);
        }) {
            log::debug!("DockingState::on_enter: {}", err);
        }
        Ok(StateReturnType::Continue)
    }

    fn update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl ClassicState for DockingState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.on_exit(exit)
    }
}

#[derive(Debug)]
struct SupplyTruckStateMachine {
    machine: Arc<Mutex<StateMachine>>,
}

impl SupplyTruckStateMachine {
    fn new(owner: Arc<RwLock<Object>>) -> Self {
        let owner_weak = Arc::downgrade(&owner);
        let machine = Arc::new(Mutex::new(StateMachine::new(
            Some(owner_weak),
            "SupplyTruckStateMachine",
        )));
        let mut guard = machine
            .lock()
            .expect("SupplyTruckStateMachine lock poisoned");

        let busy_conditions = vec![
            StateConditionInfo::new(
                Self::owner_idle,
                ST_IDLE,
                StateTransitionUserData::new(),
                "owner_idle",
            ),
            StateConditionInfo::new(
                Self::owner_docking,
                ST_DOCKING,
                StateTransitionUserData::new(),
                "owner_docking",
            ),
        ];

        let idle_conditions = vec![
            StateConditionInfo::new(
                Self::is_forced_into_busy_state,
                ST_BUSY,
                StateTransitionUserData::new(),
                "forced_busy",
            ),
            StateConditionInfo::new(
                Self::is_forced_into_wanting_state,
                ST_WANTING,
                StateTransitionUserData::new(),
                "forced_wanting",
            ),
            StateConditionInfo::new(
                Self::owner_docking,
                ST_DOCKING,
                StateTransitionUserData::new(),
                "owner_docking",
            ),
            StateConditionInfo::new(
                Self::owner_not_docking_or_idle,
                ST_BUSY,
                StateTransitionUserData::new(),
                "owner_not_docking_or_idle",
            ),
        ];

        let wanting_conditions = vec![
            StateConditionInfo::new(
                Self::owner_docking,
                ST_DOCKING,
                StateTransitionUserData::new(),
                "owner_docking",
            ),
            StateConditionInfo::new(
                Self::owner_not_docking_or_idle,
                ST_BUSY,
                StateTransitionUserData::new(),
                "owner_not_docking_or_idle",
            ),
        ];

        let regrouping_conditions = vec![StateConditionInfo::new(
            Self::owner_player_commanded,
            ST_BUSY,
            StateTransitionUserData::new(),
            "owner_player_commanded",
        )];

        let docking_conditions = vec![
            StateConditionInfo::new(
                Self::is_forced_into_busy_state,
                ST_BUSY,
                StateTransitionUserData::new(),
                "forced_busy",
            ),
            StateConditionInfo::new(
                Self::owner_available_for_supplying,
                ST_WANTING,
                StateTransitionUserData::new(),
                "owner_available_for_supplying",
            ),
            StateConditionInfo::new(
                Self::owner_not_docking_or_idle,
                ST_BUSY,
                StateTransitionUserData::new(),
                "owner_not_docking_or_idle",
            ),
        ];

        register_classic_state(
            &mut guard,
            ST_BUSY,
            SupplyTruckBusyState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &busy_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_IDLE,
            SupplyTruckIdleState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &idle_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_WANTING,
            SupplyTruckWantsToPickUpOrDeliverBoxesState::new(&machine),
            Some(ST_BUSY),
            Some(ST_REGROUPING),
            &wanting_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_REGROUPING,
            RegroupingState::new(&machine),
            Some(ST_WANTING),
            Some(ST_BUSY),
            &regrouping_conditions,
        );

        register_classic_state(
            &mut guard,
            ST_DOCKING,
            DockingState::new(&machine),
            Some(ST_BUSY),
            Some(ST_BUSY),
            &docking_conditions,
        );

        let _ = guard.init_default_state();
        drop(guard);
        Self { machine }
    }

    fn update(&mut self) -> StateReturnType {
        self.machine
            .lock()
            .map(|mut guard| guard.update())
            .unwrap_or(StateReturnType::Failure)
    }

    fn current_state_id(&self) -> Option<u32> {
        self.machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_current_state_id())
    }

    fn owner_docking(state: &dyn StateImplementation, _data: &StateTransitionUserData) -> bool {
        let owner = match state.get_machine_owner() {
            Ok(owner) => owner,
            Err(_) => return false,
        };
        let ai = match owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        {
            Some(ai) => ai,
            None => return false,
        };
        ai.lock()
            .ok()
            .and_then(|guard| guard.get_current_command())
            .map(|cmd| cmd == AiCommandType::Dock)
            .unwrap_or(false)
    }

    fn owner_idle(state: &dyn StateImplementation, _data: &StateTransitionUserData) -> bool {
        let owner = match state.get_machine_owner() {
            Ok(owner) => owner,
            Err(_) => return false,
        };
        let ai = match owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        {
            Some(ai) => ai,
            None => return false,
        };
        ai.lock().ok().map_or(false, |guard| guard.is_idle())
    }

    fn owner_available_for_supplying(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let owner = match state.get_machine_owner() {
            Ok(owner) => owner,
            Err(_) => return false,
        };
        let ai = match owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        {
            Some(ai) => ai,
            None => return false,
        };
        let mut ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if !ai_guard.is_idle() {
            return false;
        }
        let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() else {
            return false;
        };
        truck.is_available_for_supplying()
    }

    fn owner_not_docking_or_idle(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let owner = match state.get_machine_owner() {
            Ok(owner) => owner,
            Err(_) => return false,
        };
        let ai = match owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        {
            Some(ai) => ai,
            None => return false,
        };
        let ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if ai_guard.is_idle() {
            return false;
        }
        ai_guard
            .get_current_command()
            .map(|cmd| cmd != AiCommandType::Dock)
            .unwrap_or(true)
    }

    fn is_forced_into_wanting_state(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let owner = match state.get_machine_owner() {
            Ok(owner) => owner,
            Err(_) => return false,
        };
        let ai = match owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        {
            Some(ai) => ai,
            None => return false,
        };
        let mut ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() else {
            return false;
        };
        truck.is_forced_into_wanting_state()
    }

    fn is_forced_into_busy_state(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let owner = match state.get_machine_owner() {
            Ok(owner) => owner,
            Err(_) => return false,
        };
        let ai = match owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        {
            Some(ai) => ai,
            None => return false,
        };
        let mut ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let Some(truck) = ai_guard.get_supply_truck_ai_interface_mut() else {
            return false;
        };
        truck.is_forced_into_busy_state()
    }

    fn owner_player_commanded(
        state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        let owner = match state.get_machine_owner() {
            Ok(owner) => owner,
            Err(_) => return false,
        };
        let ai = match owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
        {
            Some(ai) => ai,
            None => return false,
        };
        ai.lock()
            .ok()
            .map(|guard| guard.get_last_command_source() == CommandSourceType::FromPlayer)
            .unwrap_or(false)
    }
}

fn find_regroup_target(
    owner: &Object,
    player: &crate::player::Player,
) -> Option<Arc<RwLock<Object>>> {
    let candidates = [
        KindOf::CashGenerator,
        KindOf::CommandCenter,
        KindOf::Structure,
    ];

    for kindof in candidates {
        let mut best: Option<(Arc<RwLock<Object>>, Real)> = None;
        for object_id in player.get_all_objects() {
            let Some(obj) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };
            if obj_guard.is_destroyed() || !obj_guard.is_kind_of(kindof) {
                continue;
            }
            let dist_sq = ThePartitionManager::get_distance_squared(
                owner,
                &obj_guard,
                crate::common::FROM_BOUNDING_SPHERE_2D,
            );
            if best
                .as_ref()
                .map_or(true, |(_, best_dist)| dist_sq < *best_dist)
            {
                best = Some((obj.clone(), dist_sq));
            }
        }
        if let Some((obj, _)) = best {
            return Some(obj);
        }
    }
    None
}

/// Supply truck AI update module data
#[derive(Debug, Clone)]
pub struct SupplyTruckAIUpdateData {
    /// Maximum number of boxes this truck can carry
    pub max_boxes: i32,
    /// Warehouse scan distance
    pub warehouse_scan_distance: Real,
    /// Delay time at warehouse (in frames)
    pub warehouse_delay: u32,
    /// Delay time at center (in frames)
    pub center_delay: u32,
    /// Supplies depleted voice event name
    pub supplies_depleted_voice: String,
}

impl Default for SupplyTruckAIUpdateData {
    fn default() -> Self {
        Self {
            max_boxes: 0,
            warehouse_scan_distance: 100.0,
            warehouse_delay: 0,
            center_delay: 0,
            supplies_depleted_voice: String::new(),
        }
    }
}

/// Supply truck AI update module
/// Matches C++ SupplyTruckAIUpdate
pub struct SupplyTruckAIUpdate {
    /// Configuration data
    data: SupplyTruckAIUpdateData,
    /// Current AI state
    state: SupplyTruckState,
    /// Current number of boxes carried
    number_boxes: i32,
    /// Preferred dock ID (set by player command)
    preferred_dock: Option<ObjectID>,
    /// Whether to force wanting state
    force_wanting_state: bool,
    /// Whether to force busy state
    force_busy_state: bool,
    /// Supply truck state machine
    state_machine: Option<SupplyTruckStateMachine>,
    /// Object ID of this truck
    object_id: ObjectID,
    /// Player index for upgrade checks
    player_index: PlayerIndex,
    /// Audio system reference (optional)
    audio_system: Option<Arc<dyn AudioSystem>>,
    /// Upgrade system reference (optional)
    upgrade_system: Option<Arc<dyn UpgradeSystem>>,
}

impl std::fmt::Debug for SupplyTruckAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupplyTruckAIUpdate")
            .field("data", &self.data)
            .field("state", &self.state)
            .field("number_boxes", &self.number_boxes)
            .field("preferred_dock", &self.preferred_dock)
            .field("force_wanting_state", &self.force_wanting_state)
            .field("force_busy_state", &self.force_busy_state)
            .field(
                "state_machine",
                &self
                    .state_machine
                    .as_ref()
                    .map(|_| "SupplyTruckStateMachine"),
            )
            .field("object_id", &self.object_id)
            .field("player_index", &self.player_index)
            .field(
                "audio_system",
                &self.audio_system.as_ref().map(|_| "AudioSystem"),
            )
            .field(
                "upgrade_system",
                &self.upgrade_system.as_ref().map(|_| "UpgradeSystem"),
            )
            .finish()
    }
}

impl SupplyTruckAIUpdate {
    pub fn new(
        data: SupplyTruckAIUpdateData,
        object_id: ObjectID,
        player_index: PlayerIndex,
    ) -> Self {
        Self {
            data,
            state: SupplyTruckState::Idle,
            number_boxes: 0,
            preferred_dock: None,
            force_wanting_state: false,
            force_busy_state: false,
            state_machine: None,
            object_id,
            player_index,
            audio_system: None,
            upgrade_system: None,
        }
    }

    /// Set audio system for voice events
    pub fn set_audio_system(&mut self, audio_system: Arc<dyn AudioSystem>) {
        self.audio_system = Some(audio_system);
    }

    /// Set upgrade system for supply boost calculation
    pub fn set_upgrade_system(&mut self, upgrade_system: Arc<dyn UpgradeSystem>) {
        self.upgrade_system = Some(upgrade_system);
    }

    fn owner_object(&self) -> Option<Arc<RwLock<Object>>> {
        TheGameLogic::find_object_by_id(self.object_id)
    }

    fn update_drawable_supply_status(&self) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let drawable = owner_guard.get_drawable();
        drop(owner_guard);
        let Some(drawable) = drawable else {
            return;
        };
        if let Ok(mut draw_guard) = drawable.write() {
            draw_guard.update_supply_status(self.data.max_boxes, self.number_boxes);
        };
    }

    fn sync_state_from_machine(&mut self) {
        let Some(machine) = &self.state_machine else {
            return;
        };
        match machine.current_state_id() {
            Some(ST_IDLE) => self.state = SupplyTruckState::Idle,
            Some(ST_BUSY) => self.state = SupplyTruckState::Busy,
            Some(ST_WANTING) => self.state = SupplyTruckState::Wanting,
            Some(ST_REGROUPING) => self.state = SupplyTruckState::Regrouping,
            Some(ST_DOCKING) => self.state = SupplyTruckState::Docking,
            _ => {}
        }
    }

    /// Update the supply truck AI state machine.
    pub fn update(&mut self) -> StateReturnType {
        if self.state_machine.is_none() {
            if let Some(owner) = self.owner_object() {
                self.state_machine = Some(SupplyTruckStateMachine::new(owner));
            } else {
                return StateReturnType::Failure;
            }
        }

        let status = if let Some(machine) = &mut self.state_machine {
            machine.update()
        } else {
            StateReturnType::Failure
        };
        self.sync_state_from_machine();
        status
    }

    /// Handle idle command (matches C++ SupplyTruckAIUpdate::privateIdle).
    pub fn private_idle(&mut self, cmd_source: CommandSourceType) {
        if cmd_source == CommandSourceType::FromPlayer {
            self.set_force_busy_state(true);
        }
    }

    /// Handle dock command (matches C++ SupplyTruckAIUpdate::privateDock).
    pub fn private_dock(&mut self, dock_id: Option<ObjectID>, cmd_source: CommandSourceType) {
        if cmd_source == CommandSourceType::FromPlayer {
            if let Some(dock_id) = dock_id {
                self.set_preferred_dock(dock_id);
            }
        }
    }

    /// Lose one box (when depositing at supply center)
    /// Matches C++ SupplyTruckAIUpdate::loseOneBox() - SupplyTruckAIUpdate.cpp:116
    pub fn lose_one_box(&mut self) -> bool {
        if self.number_boxes == 0 {
            return false;
        }
        self.number_boxes -= 1;
        self.update_drawable_supply_status();
        true
    }

    /// Gain one box (when collecting from warehouse)
    /// Matches C++ SupplyTruckAIUpdate::gainOneBox() - SupplyTruckAIUpdate.cpp:132
    pub fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        if self.number_boxes >= self.data.max_boxes {
            return false;
        }
        self.number_boxes += 1;

        // If we just took the last box, announce supplies depleted
        // Matches C++ SupplyTruckAIUpdate.cpp:141-161
        if remaining_stock == 0 && !self.data.supplies_depleted_voice.is_empty() {
            let mut play_depleted = true;
            if let Some(best_warehouse) = resource::find_best_supply_warehouse(self.object_id) {
                if let (Some(owner), Some(warehouse)) = (
                    self.owner_object(),
                    TheGameLogic::find_object_by_id(best_warehouse),
                ) {
                    if let (Ok(owner_guard), Ok(warehouse_guard)) = (owner.read(), warehouse.read())
                    {
                        let delta = *owner_guard.get_position() - *warehouse_guard.get_position();
                        let distance =
                            (delta.x * delta.x + delta.y * delta.y + delta.z * delta.z).sqrt();
                        let is_ai_player = owner_guard
                            .get_controlling_player_id()
                            .and_then(|player_id| {
                                let Ok(list) = player_list().read() else {
                                    return None;
                                };
                                list.get_player(player_id as i32).cloned()
                            })
                            .and_then(|player| {
                                player.read().ok().map(|guard| guard.is_skirmish_ai())
                            })
                            .unwrap_or(false);
                        if distance <= self.get_warehouse_scan_distance(is_ai_player) / 4.0 {
                            play_depleted = false;
                        }
                    }
                }
            }

            if play_depleted {
                if let Some(audio) = &self.audio_system {
                    audio.play_voice_event(&self.data.supplies_depleted_voice, self.object_id);
                }
            }
        }

        self.update_drawable_supply_status();
        true
    }

    /// Get upgraded supply boost from upgrades
    /// Matches C++ SupplyTruckAIInterface::getUpgradedSupplyBoost()
    /// Implementation follows WorkerAIUpdate::getUpgradedSupplyBoost() - WorkerAIUpdate.cpp:1376
    pub fn get_upgraded_supply_boost(&self) -> u32 {
        if let Some(upgrade) = &self.upgrade_system {
            upgrade.get_supply_boost(self.player_index)
        } else {
            0
        }
    }

    /// Check if currently ferrying supplies
    /// Matches C++ SupplyTruckAIUpdate::isCurrentlyFerryingSupplies()
    pub fn is_currently_ferrying_supplies(&self) -> bool {
        matches!(
            self.state,
            SupplyTruckState::Wanting | SupplyTruckState::Docking
        )
    }

    /// Check if available for supplying
    pub fn is_available_for_supplying(&self) -> bool {
        true
    }

    /// Set preferred dock (from player command)
    pub fn set_preferred_dock(&mut self, dock_id: ObjectID) {
        self.preferred_dock = Some(dock_id);
    }

    pub fn get_preferred_dock(&self) -> Option<ObjectID> {
        self.preferred_dock
    }

    /// Set force wanting state
    pub fn set_force_wanting_state(&mut self, force: bool) {
        self.force_wanting_state = force;
    }

    /// Set force busy state
    pub fn set_force_busy_state(&mut self, force: bool) {
        self.force_busy_state = force;
    }

    /// Get action delay for a dock
    /// Matches C++ SupplyTruckAIUpdate::getActionDelayForDock()
    pub fn get_action_delay_for_dock(&self, is_warehouse: bool) -> u32 {
        if is_warehouse {
            self.data.warehouse_delay
        } else {
            self.data.center_delay
        }
    }

    /// Get warehouse scan distance
    /// AI players get 2x distance
    pub fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Real {
        if is_ai_player {
            self.data.warehouse_scan_distance * 2.0
        } else {
            self.data.warehouse_scan_distance
        }
    }

    pub fn get_number_boxes(&self) -> i32 {
        self.number_boxes
    }

    pub fn get_max_boxes(&self) -> i32 {
        self.data.max_boxes
    }

    pub fn get_state(&self) -> SupplyTruckState {
        self.state
    }

    pub fn set_state(&mut self, state: SupplyTruckState) {
        self.state = state;
    }
}

impl SupplyTruckAIInterface for SupplyTruckAIUpdate {
    fn get_supplies_count(&self) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.number_boxes)
    }

    fn get_number_boxes(&self) -> i32 {
        self.number_boxes
    }

    fn get_action_delay_for_dock(
        &self,
        dock: &Arc<RwLock<Object>>,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let is_warehouse = dock.read().ok().map_or(false, |obj| {
            obj.find_update_module("SupplyWarehouseDockUpdate")
                .is_some()
                || obj
                    .module_by_name(&AsciiString::from("SupplyWarehouseDockUpdate"))
                    .is_some()
        });
        Ok(self.get_action_delay_for_dock(is_warehouse))
    }

    fn set_force_wanting_state(&mut self, enabled: bool) {
        self.force_wanting_state = enabled;
    }

    fn is_forced_into_wanting_state(&self) -> bool {
        self.force_wanting_state
    }

    fn set_force_busy_state(&mut self, enabled: bool) {
        self.force_busy_state = enabled;
    }

    fn is_forced_into_busy_state(&self) -> bool {
        self.force_busy_state
    }

    fn get_preferred_dock_id(&self) -> Option<ObjectID> {
        self.get_preferred_dock()
    }

    fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Option<Real> {
        Some(self.get_warehouse_scan_distance(is_ai_player))
    }

    fn is_available_for_supplying(&self) -> bool {
        self.is_available_for_supplying()
    }

    fn is_currently_ferrying_supplies(&self) -> bool {
        self.is_currently_ferrying_supplies()
    }

    fn lose_one_box(&mut self) -> bool {
        SupplyTruckAIUpdate::lose_one_box(self)
    }

    fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        SupplyTruckAIUpdate::gain_one_box(self, remaining_stock)
    }

    fn get_upgraded_supply_boost(&self) -> u32 {
        self.get_upgraded_supply_boost()
    }
}

impl SupplyTruckAIInterface for WorkerAIUpdate {
    fn get_supplies_count(&self) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.number_boxes)
    }

    fn get_number_boxes(&self) -> i32 {
        self.number_boxes
    }

    fn get_action_delay_for_dock(
        &self,
        dock: &Arc<RwLock<Object>>,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let is_warehouse = dock.read().ok().map_or(false, |obj| {
            obj.find_update_module("SupplyWarehouseDockUpdate")
                .is_some()
                || obj
                    .module_by_name(&AsciiString::from("SupplyWarehouseDockUpdate"))
                    .is_some()
        });
        Ok(self.get_action_delay_for_dock(is_warehouse))
    }

    fn set_force_wanting_state(&mut self, enabled: bool) {
        WorkerAIUpdate::set_force_wanting_state(self, enabled);
    }

    fn is_forced_into_wanting_state(&self) -> bool {
        WorkerAIUpdate::is_forced_into_wanting_state(self)
    }

    fn set_force_busy_state(&mut self, enabled: bool) {
        WorkerAIUpdate::set_force_busy_state(self, enabled);
    }

    fn is_forced_into_busy_state(&self) -> bool {
        WorkerAIUpdate::is_forced_into_busy_state(self)
    }

    fn get_preferred_dock_id(&self) -> Option<ObjectID> {
        self.get_preferred_dock()
    }

    fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Option<Real> {
        Some(self.get_warehouse_scan_distance(is_ai_player))
    }

    fn is_available_for_supplying(&self) -> bool {
        self.is_available_for_supplying()
    }

    fn is_currently_ferrying_supplies(&self) -> bool {
        self.is_currently_ferrying_supplies()
    }

    fn lose_one_box(&mut self) -> bool {
        WorkerAIUpdate::lose_one_box(self)
    }

    fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        WorkerAIUpdate::gain_one_box(self, remaining_stock)
    }

    fn get_upgraded_supply_boost(&self) -> u32 {
        self.get_upgraded_supply_boost()
    }
}

impl WorkerAIUpdateInterface for WorkerAIUpdate {
    fn set_build_task(
        &mut self,
        building_id: ObjectID,
        total_build_frames: u32,
        max_health: f32,
        is_rebuild: bool,
    ) {
        WorkerAIUpdate::set_build_task(
            self,
            building_id,
            total_build_frames,
            max_health,
            is_rebuild,
        );
    }
}

// ============================================================================
// AUTO DEPOSIT UPDATE
// ============================================================================

/// Auto deposit update module data
/// Matches C++ AutoDepositUpdateModuleData
#[derive(Debug, Clone)]
pub struct AutoDepositUpdateData {
    /// Amount to deposit
    pub deposit_amount: u32,
    /// Frame interval for deposits
    pub deposit_interval: u32,
    /// Initial capture bonus
    pub initial_capture_bonus: u32,
    /// Whether this is actual money (not just score)
    pub is_actual_money: bool,
}

impl Default for AutoDepositUpdateData {
    fn default() -> Self {
        Self {
            deposit_amount: 0,
            deposit_interval: 150, // 5 seconds at 30 FPS
            initial_capture_bonus: 0,
            is_actual_money: true,
        }
    }
}

/// Auto deposit update module (for oil derricks, black market, hackers)
/// Matches C++ AutoDepositUpdate
pub struct AutoDepositUpdate {
    /// Configuration data
    data: AutoDepositUpdateData,
    /// Frame to deposit on next
    deposit_on_frame: u32,
    /// Whether to award initial capture bonus
    award_initial_capture_bonus: bool,
    /// Whether initialized
    initialized: bool,
    /// Player index for upgrade checks
    player_index: PlayerIndex,
    /// UI system reference (optional)
    ui_system: Option<Arc<dyn UISystem>>,
    /// Upgrade system reference (optional)
    upgrade_system: Option<Arc<dyn UpgradeSystem>>,
}

impl std::fmt::Debug for AutoDepositUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoDepositUpdate")
            .field("data", &self.data)
            .field("deposit_on_frame", &self.deposit_on_frame)
            .field(
                "award_initial_capture_bonus",
                &self.award_initial_capture_bonus,
            )
            .field("initialized", &self.initialized)
            .field("player_index", &self.player_index)
            .field("ui_system", &self.ui_system.as_ref().map(|_| "UISystem"))
            .field(
                "upgrade_system",
                &self.upgrade_system.as_ref().map(|_| "UpgradeSystem"),
            )
            .finish()
    }
}

impl AutoDepositUpdate {
    pub fn new(data: AutoDepositUpdateData, current_frame: u32, player_index: PlayerIndex) -> Self {
        Self {
            deposit_on_frame: current_frame + data.deposit_interval,
            data,
            award_initial_capture_bonus: false,
            initialized: false,
            player_index,
            ui_system: None,
            upgrade_system: None,
        }
    }

    /// Set UI system for floating text
    pub fn set_ui_system(&mut self, ui_system: Arc<dyn UISystem>) {
        self.ui_system = Some(ui_system);
    }

    /// Set upgrade system for supply boost calculation
    pub fn set_upgrade_system(&mut self, upgrade_system: Arc<dyn UpgradeSystem>) {
        self.upgrade_system = Some(upgrade_system);
    }

    /// Award initial capture bonus
    /// Matches C++ AutoDepositUpdate::awardInitialCaptureBonus()
    pub fn award_initial_capture_bonus(
        &mut self,
        player_money: &mut Money,
        current_frame: u32,
        building_position: &Coord3D,
        player_color: Color,
    ) {
        self.deposit_on_frame = current_frame + self.data.deposit_interval;

        if !self.award_initial_capture_bonus || self.data.initial_capture_bonus == 0 {
            return;
        }

        player_money.deposit(self.data.initial_capture_bonus, true);

        // Display floating text for initial capture bonus
        if let Some(ui) = &self.ui_system {
            let text = format!("+${}", self.data.initial_capture_bonus);
            let color_with_alpha = player_color | 0xE6000000;
            ui.add_floating_text(&text, building_position, color_with_alpha);
        }

        self.award_initial_capture_bonus = false;
    }

    /// Update the auto deposit (called each frame)
    /// Matches C++ AutoDepositUpdate::update() - AutoDepositUpdate.cpp:126
    pub fn update(
        &mut self,
        current_frame: u32,
        is_neutral: bool,
        construction_complete: bool,
        player_money: &mut Money,
        building_position: &Coord3D,
        player_color: Color,
    ) -> bool {
        if current_frame >= self.deposit_on_frame {
            if !self.initialized {
                // Set on first update, not on load
                self.award_initial_capture_bonus = true;
                self.initialized = true;
            }

            self.deposit_on_frame = current_frame + self.data.deposit_interval;

            if is_neutral || self.data.deposit_amount == 0 {
                return false;
            }

            // Buildings under construction don't get bonuses
            if !construction_complete {
                return false;
            }

            if self.data.is_actual_money {
                // Add upgraded supply boost
                // Matches C++ AutoDepositUpdate.cpp:143
                let upgraded_boost = if let Some(upgrade) = &self.upgrade_system {
                    upgrade.get_supply_boost(self.player_index)
                } else {
                    0
                };

                let total_amount = self.data.deposit_amount + upgraded_boost;
                player_money.deposit(total_amount, true);

                // Display floating text
                // Matches C++ AutoDepositUpdate.cpp:162-187
                if let Some(ui) = &self.ui_system {
                    let text = format!("+${}", total_amount);
                    let color_with_alpha = player_color | 0xE6000000;
                    ui.add_floating_text(&text, building_position, color_with_alpha);
                }

                return true;
            }
        }

        false
    }
}

// ============================================================================
// SUPPLY PILE SYSTEM
// ============================================================================

/// Supply pile with limited resources
/// Matches C++ supply warehouse mechanics with depletion support
#[derive(Debug, Clone)]
pub struct SupplyPile {
    /// Object ID of the pile
    pub pile_id: ObjectID,
    /// Position of the pile
    pub position: Coord3D,
    /// Remaining supply boxes
    pub remaining_boxes: i32,
    /// Maximum supply boxes (for visualization)
    pub max_boxes: i32,
    /// Number of active gatherers
    pub active_gatherers: usize,
    /// Whether pile is depleted
    pub is_depleted: bool,
}

impl SupplyPile {
    pub fn new(pile_id: ObjectID, position: Coord3D, initial_boxes: i32) -> Self {
        Self {
            pile_id,
            position,
            remaining_boxes: initial_boxes,
            max_boxes: initial_boxes,
            active_gatherers: 0,
            is_depleted: false,
        }
    }

    /// Take one box from the pile
    /// Returns true if successful, false if pile is empty
    pub fn take_box(&mut self) -> bool {
        if self.remaining_boxes > 0 {
            self.remaining_boxes -= 1;
            if self.remaining_boxes == 0 {
                self.is_depleted = true;
            }
            true
        } else {
            false
        }
    }

    /// Add a gatherer to this pile
    pub fn add_gatherer(&mut self) {
        self.active_gatherers += 1;
    }

    /// Remove a gatherer from this pile
    pub fn remove_gatherer(&mut self) {
        if self.active_gatherers > 0 {
            self.active_gatherers -= 1;
        }
    }

    /// Get the percentage of resources remaining (0.0 to 1.0)
    pub fn get_remaining_percentage(&self) -> f32 {
        if self.max_boxes > 0 {
            self.remaining_boxes as f32 / self.max_boxes as f32
        } else {
            0.0
        }
    }

    /// Check if pile can support another gatherer
    /// Typically limit to prevent overcrowding
    pub fn can_accept_gatherer(&self, max_gatherers_per_pile: usize) -> bool {
        !self.is_depleted && self.active_gatherers < max_gatherers_per_pile
    }
}

// ============================================================================
// FACTION-SPECIFIC SUPPLY MECHANICS
// ============================================================================

/// USA-specific supply drop zone
/// Implements Chinook supply drop mechanics
#[derive(Debug, Clone)]
pub struct SupplyDropZone {
    /// Zone ID
    pub zone_id: ObjectID,
    /// Drop zone position
    pub position: Coord3D,
    /// Supply value per drop
    pub supply_per_drop: u32,
    /// Cooldown between drops (in frames)
    pub drop_cooldown: u32,
    /// Next frame when drop is available
    pub next_drop_frame: u32,
    /// Whether zone is available
    pub is_available: bool,
}

impl SupplyDropZone {
    pub fn new(zone_id: ObjectID, position: Coord3D, supply_per_drop: u32, cooldown: u32) -> Self {
        Self {
            zone_id,
            position,
            supply_per_drop,
            drop_cooldown: cooldown,
            next_drop_frame: 0,
            is_available: true,
        }
    }

    /// Request a supply drop
    /// Returns the supply amount if available, 0 otherwise
    pub fn request_drop(&mut self, current_frame: u32) -> u32 {
        if self.is_available && current_frame >= self.next_drop_frame {
            self.next_drop_frame = current_frame + self.drop_cooldown;
            self.supply_per_drop
        } else {
            0
        }
    }

    /// Get cooldown remaining in frames
    pub fn get_cooldown_remaining(&self, current_frame: u32) -> u32 {
        if current_frame < self.next_drop_frame {
            self.next_drop_frame - current_frame
        } else {
            0
        }
    }
}

/// China-specific hacker income
/// Implements passive income from hacker units
#[derive(Debug, Clone)]
pub struct HackerIncome {
    /// Hacker object ID
    pub hacker_id: ObjectID,
    /// Income per interval
    pub income_per_interval: u32,
    /// Interval in frames
    pub income_interval: u32,
    /// Next frame to award income
    pub next_income_frame: u32,
    /// Whether hacker is active (not disabled/garrisoned)
    pub is_active: bool,
}

impl HackerIncome {
    pub fn new(
        hacker_id: ObjectID,
        income_per_interval: u32,
        interval: u32,
        current_frame: u32,
    ) -> Self {
        Self {
            hacker_id,
            income_per_interval,
            income_interval: interval,
            next_income_frame: current_frame + interval,
            is_active: true,
        }
    }

    /// Update and award income if ready
    /// Returns the income amount if awarded, 0 otherwise
    pub fn update(&mut self, current_frame: u32) -> u32 {
        if self.is_active && current_frame >= self.next_income_frame {
            self.next_income_frame = current_frame + self.income_interval;
            self.income_per_interval
        } else {
            0
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
}

/// GLA-specific black market income
/// Implements passive income from black market building
#[derive(Debug, Clone)]
pub struct BlackMarketIncome {
    /// Black market object ID
    pub market_id: ObjectID,
    /// Base income per interval
    pub base_income: u32,
    /// Interval in frames
    pub income_interval: u32,
    /// Next frame to award income
    pub next_income_frame: u32,
    /// Number of upgrade levels (increases income)
    pub upgrade_level: u32,
    /// Whether market is functional
    pub is_functional: bool,
}

impl BlackMarketIncome {
    pub fn new(market_id: ObjectID, base_income: u32, interval: u32, current_frame: u32) -> Self {
        Self {
            market_id,
            base_income,
            income_interval: interval,
            next_income_frame: current_frame + interval,
            upgrade_level: 0,
            is_functional: true,
        }
    }

    /// Update and award income if ready
    /// Returns the income amount if awarded, 0 otherwise
    pub fn update(&mut self, current_frame: u32) -> u32 {
        if self.is_functional && current_frame >= self.next_income_frame {
            self.next_income_frame = current_frame + self.income_interval;
            // Income increases with upgrade level
            self.base_income + (self.upgrade_level * 10)
        } else {
            0
        }
    }

    pub fn upgrade(&mut self) {
        self.upgrade_level += 1;
    }

    pub fn set_functional(&mut self, functional: bool) {
        self.is_functional = functional;
    }
}

/// Worker AI update module data (matches C++ WorkerAIUpdateModuleData fields).
#[derive(Debug, Clone)]
pub struct WorkerAIUpdateData {
    /// Maximum number of boxes this worker can carry
    pub max_boxes: i32,
    /// Warehouse scan distance
    pub warehouse_scan_distance: Real,
    /// Delay time at warehouse (in frames)
    pub warehouse_delay: u32,
    /// Delay time at center (in frames)
    pub center_delay: u32,
    /// Supplies depleted voice event name
    pub supplies_depleted_voice: String,
    /// Repair health percent per second
    pub repair_health_percent_per_second: Real,
    /// Bored time (seconds)
    pub bored_time: Real,
    /// Bored range
    pub bored_range: Real,
    /// Supply boost when upgraded (worker shoes)
    pub upgraded_supply_boost: u32,
}

impl Default for WorkerAIUpdateData {
    fn default() -> Self {
        Self {
            max_boxes: 0,
            warehouse_scan_distance: 100.0,
            warehouse_delay: 0,
            center_delay: 0,
            supplies_depleted_voice: String::new(),
            repair_health_percent_per_second: 0.0,
            bored_time: 0.0,
            bored_range: 0.0,
            upgraded_supply_boost: 0,
        }
    }
}

/// GLA worker AI (similar to supply truck but for GLA faction)
/// GLA workers gather supplies on foot from supply piles
pub struct WorkerAIUpdate {
    /// Worker configuration (similar to SupplyTruckAIUpdate)
    data: WorkerAIUpdateData,
    /// Current AI state
    state: SupplyTruckState,
    /// Current number of boxes carried
    number_boxes: i32,
    /// Preferred dock ID (set by player command)
    preferred_dock: Option<ObjectID>,
    /// Whether to force wanting state
    force_wanting_state: bool,
    /// Whether to force busy state
    force_busy_state: bool,
    /// Supply truck state machine (workers reuse supply truck state logic)
    state_machine: Option<SupplyTruckStateMachine>,
    /// Active dozer-style task (repair/resume/build)
    dozer_task: Option<WorkerDozerTask>,
    /// Current dozer action state
    dozer_action_state: WorkerDozerActionState,
    /// Dozer task entries (build/repair/fortify)
    dozer_tasks: [WorkerTaskEntry; WORKER_DOZER_TASK_COUNT],
    /// Dock points per task (start/action/end)
    dozer_dock_points: [[WorkerDockPoint; WORKER_DOCK_POINT_COUNT]; WORKER_DOZER_TASK_COUNT],
    /// Current task slot
    current_task: Option<WorkerDozerTaskSlot>,
    /// Object ID of this worker
    object_id: ObjectID,
    /// Player index
    player_index: PlayerIndex,
    /// Audio system reference
    audio_system: Option<Arc<dyn AudioSystem>>,
    /// Upgrade system reference
    upgrade_system: Option<Arc<dyn UpgradeSystem>>,
}

/// Dozer-style task types for workers (matches C++ WorkerAIUpdate task handling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDozerTaskType {
    Repair,
    ResumeConstruction,
    Build,
    Fortify,
}

/// Current dozer action phase (matches DOZER_ACTION_* flow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDozerActionState {
    PickActionPos,
    MoveToActionPos,
    DoAction,
}

#[derive(Debug, Clone)]
struct WorkerDozerTask {
    task_type: WorkerDozerTaskType,
    target_id: ObjectID,
    dock_point: Option<Coord3D>,
    failed_attempts: u32,
    build_total_frames: u32,
    build_max_health: f32,
    is_rebuild: bool,
    started_construction: bool,
}

#[derive(Debug, Clone, Copy)]
struct WorkerDockPoint {
    valid: bool,
    location: Coord3D,
}

impl Default for WorkerDockPoint {
    fn default() -> Self {
        Self {
            valid: false,
            location: Coord3D::zero(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerDozerTaskSlot {
    Build = 0,
    Repair = 1,
    Fortify = 2,
}

impl WorkerDozerTaskSlot {
    fn as_index(self) -> usize {
        self as usize
    }
}

const WORKER_DOZER_TASK_COUNT: usize = 3;
const WORKER_DOCK_POINT_COUNT: usize = 3;

#[derive(Debug, Clone)]
struct WorkerTaskEntry {
    target_id: ObjectID,
    order_frame: u32,
}

impl Default for WorkerTaskEntry {
    fn default() -> Self {
        Self {
            target_id: INVALID_ID,
            order_frame: 0,
        }
    }
}

impl std::fmt::Debug for WorkerAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerAIUpdate")
            .field("data", &self.data)
            .field("state", &self.state)
            .field("number_boxes", &self.number_boxes)
            .field("preferred_dock", &self.preferred_dock)
            .field("force_wanting_state", &self.force_wanting_state)
            .field("force_busy_state", &self.force_busy_state)
            .field(
                "state_machine",
                &self
                    .state_machine
                    .as_ref()
                    .map(|_| "SupplyTruckStateMachine"),
            )
            .field("dozer_task", &self.dozer_task)
            .field("dozer_action_state", &self.dozer_action_state)
            .field("current_task", &self.current_task)
            .field("object_id", &self.object_id)
            .field("player_index", &self.player_index)
            .field("audio_system", &self.audio_system.is_some())
            .field("upgrade_system", &self.upgrade_system.is_some())
            .finish()
    }
}

impl WorkerAIUpdate {
    pub fn new(data: WorkerAIUpdateData, object_id: ObjectID, player_index: PlayerIndex) -> Self {
        Self {
            data,
            state: SupplyTruckState::Idle,
            number_boxes: 0,
            preferred_dock: None,
            force_wanting_state: false,
            force_busy_state: false,
            state_machine: None,
            dozer_task: None,
            dozer_action_state: WorkerDozerActionState::PickActionPos,
            dozer_tasks: [
                WorkerTaskEntry::default(),
                WorkerTaskEntry::default(),
                WorkerTaskEntry::default(),
            ],
            dozer_dock_points: [
                [
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                ],
                [
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                ],
                [
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                    WorkerDockPoint::default(),
                ],
            ],
            current_task: None,
            object_id,
            player_index,
            audio_system: None,
            upgrade_system: None,
        }
    }

    fn owner_object(&self) -> Option<Arc<RwLock<Object>>> {
        TheGameLogic::find_object_by_id(self.object_id)
    }

    fn update_drawable_supply_status(&self) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let drawable = owner_guard.get_drawable();
        drop(owner_guard);
        let Some(drawable) = drawable else {
            return;
        };
        if let Ok(mut draw_guard) = drawable.write() {
            draw_guard.update_supply_status(self.data.max_boxes, self.number_boxes);
        };
    }

    fn sync_state_from_machine(&mut self) {
        let Some(machine) = &self.state_machine else {
            return;
        };
        match machine.current_state_id() {
            Some(ST_IDLE) => self.state = SupplyTruckState::Idle,
            Some(ST_BUSY) => self.state = SupplyTruckState::Busy,
            Some(ST_WANTING) => self.state = SupplyTruckState::Wanting,
            Some(ST_REGROUPING) => self.state = SupplyTruckState::Regrouping,
            Some(ST_DOCKING) => self.state = SupplyTruckState::Docking,
            _ => {}
        }
    }

    /// Update the worker supply state machine.
    pub fn update(&mut self) -> StateReturnType {
        if self.state_machine.is_none() {
            if let Some(owner) = self.owner_object() {
                self.state_machine = Some(SupplyTruckStateMachine::new(owner));
            } else {
                return StateReturnType::Failure;
            }
        }

        let status = if let Some(machine) = &mut self.state_machine {
            machine.update()
        } else {
            StateReturnType::Failure
        };
        self.sync_state_from_machine();
        status
    }

    /// Handle idle command (matches C++ WorkerAIUpdate::privateIdle).
    pub fn private_idle(&mut self, _cmd_source: CommandSourceType) {
        // Worker does not force busy on player stop (see C++ comment).
    }

    /// Handle dock command (matches C++ WorkerAIUpdate::privateDock).
    pub fn private_dock(&mut self, dock_id: Option<ObjectID>, cmd_source: CommandSourceType) {
        if cmd_source == CommandSourceType::FromPlayer {
            if let Some(dock_id) = dock_id {
                self.preferred_dock = Some(dock_id);
            }
        }
    }

    fn is_task_pending(&self, task: WorkerDozerTaskSlot) -> bool {
        self.dozer_tasks[task.as_index()].target_id != INVALID_ID
    }

    fn get_task_target(&self, task: WorkerDozerTaskSlot) -> Option<ObjectID> {
        let entry = &self.dozer_tasks[task.as_index()];
        if entry.target_id == INVALID_ID {
            None
        } else {
            Some(entry.target_id)
        }
    }

    fn set_current_task(&mut self, task: Option<WorkerDozerTaskSlot>) {
        self.current_task = task;
    }

    fn get_most_recent_task(&self) -> Option<WorkerDozerTaskSlot> {
        let mut most_recent: Option<WorkerDozerTaskSlot> = None;
        let mut most_recent_frame: u32 = 0;
        for slot in [
            WorkerDozerTaskSlot::Build,
            WorkerDozerTaskSlot::Repair,
            WorkerDozerTaskSlot::Fortify,
        ] {
            if self.is_task_pending(slot) {
                let entry = &self.dozer_tasks[slot.as_index()];
                if most_recent.is_none() || entry.order_frame > most_recent_frame {
                    most_recent = Some(slot);
                    most_recent_frame = entry.order_frame;
                }
            }
        }
        most_recent
    }

    fn clear_task(&mut self, task: WorkerDozerTaskSlot) {
        let idx = task.as_index();
        self.dozer_tasks[idx] = WorkerTaskEntry::default();
        for point in &mut self.dozer_dock_points[idx] {
            point.valid = false;
        }
        if self.current_task == Some(task) {
            self.current_task = None;
        }
    }

    fn set_dock_points_for_task(&mut self, task: WorkerDozerTaskSlot, position: Coord3D) {
        let idx = task.as_index();
        self.dozer_dock_points[idx][0] = WorkerDockPoint {
            valid: true,
            location: position,
        };
        self.dozer_dock_points[idx][1] = WorkerDockPoint {
            valid: true,
            location: position,
        };
        self.dozer_dock_points[idx][2] = WorkerDockPoint {
            valid: true,
            location: position,
        };
    }

    fn find_action_position_for_target(&self, owner: &Object, target: &Object) -> Coord3D {
        let radius = target.get_geometry_info().get_bounding_sphere_radius();
        let start_angle = (owner.get_position().y - target.get_position().y)
            .atan2(owner.get_position().x - target.get_position().x);
        let mut options = FindPositionOptions::default();
        options.min_radius = radius;
        options.max_radius = radius;
        options.start_angle = Some(start_angle);

        let mut result = *target.get_position();
        if let Some(partition) = ThePartitionManager::get() {
            if partition.find_position_around_with_options(
                target.get_position(),
                &options,
                &mut result,
            ) {
                return Coord3D::new(result.x, result.y, result.z);
            }
        }
        Coord3D::new(
            target.get_position().x,
            target.get_position().y,
            target.get_position().z,
        )
    }

    fn remove_bridge_scaffolding(bridge_tower_id: ObjectID) {
        let Some(tower_obj) = TheGameLogic::find_object_by_id(bridge_tower_id) else {
            return;
        };
        let Ok(tower_guard) = tower_obj.read() else {
            return;
        };
        let mut bridge_id: Option<ObjectID> = None;
        for behavior in tower_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(tower) = behavior.get_bridge_tower_behavior_interface() else {
                continue;
            };
            bridge_id = Some(tower.get_bridge_id());
            if bridge_id.is_some() {
                break;
            }
        }
        let Some(bridge_id) = bridge_id else {
            return;
        };
        let Some(bridge_obj) = TheGameLogic::find_object_by_id(bridge_id) else {
            return;
        };
        let Ok(bridge_guard) = bridge_obj.read() else {
            return;
        };
        for behavior in bridge_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(bridge) = behavior.get_bridge_behavior_interface() else {
                continue;
            };
            if let Err(err) = bridge.try_remove_scaffolding() {
                log::debug!(
                    "WorkerAIUpdate::remove_bridge_scaffolding failed for bridge {}: {}",
                    bridge_id,
                    err
                );
            }
            break;
        }
    }

    fn new_task(&mut self, task: WorkerDozerTaskSlot, target_id: ObjectID) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };

        self.preferred_dock = None;

        if task == WorkerDozerTaskSlot::Build || task == WorkerDozerTaskSlot::Repair {
            let pos = self.find_action_position_for_target(&owner_guard, &target_guard);
            self.set_dock_points_for_task(task, pos);
        }
        if task == WorkerDozerTaskSlot::Build {
            if let Ok(mut target_write) = target.write() {
                target_write.set_builder(Some(&owner_guard));
            }
        }

        self.dozer_tasks[task.as_index()].target_id = target_id;
        self.dozer_tasks[task.as_index()].order_frame = TheGameLogic::get_frame();
        self.set_current_task(Some(task));
    }

    fn spawn_dozer_task_from_current(&mut self) {
        let Some(current) = self.current_task else {
            return;
        };
        let Some(target_id) = self.get_task_target(current) else {
            return;
        };
        match current {
            WorkerDozerTaskSlot::Build => {
                self.dozer_task = Some(WorkerDozerTask {
                    task_type: WorkerDozerTaskType::Build,
                    target_id,
                    dock_point: None,
                    failed_attempts: 0,
                    build_total_frames: 0,
                    build_max_health: 0.0,
                    is_rebuild: false,
                    started_construction: false,
                });
            }
            WorkerDozerTaskSlot::Repair => {
                self.dozer_task = Some(WorkerDozerTask {
                    task_type: WorkerDozerTaskType::Repair,
                    target_id,
                    dock_point: None,
                    failed_attempts: 0,
                    build_total_frames: 0,
                    build_max_health: 0.0,
                    is_rebuild: false,
                    started_construction: false,
                });
            }
            WorkerDozerTaskSlot::Fortify => {
                self.dozer_task = Some(WorkerDozerTask {
                    task_type: WorkerDozerTaskType::Fortify,
                    target_id,
                    dock_point: None,
                    failed_attempts: 0,
                    build_total_frames: 0,
                    build_max_health: 0.0,
                    is_rebuild: false,
                    started_construction: false,
                });
            }
        }
    }

    /// Issue a repair task to this worker (matches C++ WorkerAIUpdate::privateRepair).
    pub fn set_repair_target(&mut self, target_id: ObjectID, cmd_source: CommandSourceType) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_repair_object(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }

        self.new_task(WorkerDozerTaskSlot::Repair, target_id);
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::Repair,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    /// Issue a resume-construction task (matches C++ WorkerAIUpdate::privateResumeConstruction).
    pub fn set_resume_construction_target(
        &mut self,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
    ) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_resume_construction_of(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }

        self.new_task(WorkerDozerTaskSlot::Build, target_id);
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::ResumeConstruction,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    /// Issue a build task for a newly created construction site.
    pub fn set_build_task(
        &mut self,
        building_id: ObjectID,
        total_build_frames: u32,
        max_health: f32,
        is_rebuild: bool,
    ) {
        self.new_task(WorkerDozerTaskSlot::Build, building_id);
        self.dozer_task = Some(WorkerDozerTask {
            task_type: WorkerDozerTaskType::Build,
            target_id: building_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: total_build_frames.max(1),
            build_max_health: max_health,
            is_rebuild,
            started_construction: false,
        });
        self.dozer_action_state = WorkerDozerActionState::PickActionPos;
    }

    fn update_dozer_task(&mut self) {
        const MIN_ACTION_TOLERANCE: Real = 70.0;
        let repair_rate = self.get_repair_health_per_second();
        let clear_current = |this: &mut WorkerAIUpdate| {
            if let Some(current) = this.current_task {
                this.clear_task(current);
            }
        };
        let Some(owner) = self.owner_object() else {
            self.dozer_task = None;
            clear_current(self);
            return;
        };
        let Some(task) = self.dozer_task.as_mut() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(task.target_id) else {
            self.dozer_task = None;
            clear_current(self);
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        let owner_pos = *owner_guard.get_position();
        let owner_pos_local = Coord3D::new(owner_pos.x, owner_pos.y, owner_pos.z);
        let owner_airborne = owner_guard.is_using_airborne_locomotor();
        let owner_ai_update = owner_guard.get_ai_update_interface();

        let target_pos = *target_guard.get_position();
        let target_pos_local = Coord3D::new(target_pos.x, target_pos.y, target_pos.z);
        let target_radius = target_guard
            .get_geometry_info()
            .get_bounding_sphere_radius();
        let target_builder_id = target_guard.get_builder_id();
        let target_is_bridge_tower = target_guard.is_kind_of(KindOf::BridgeTower);
        drop(target_guard);

        // Determine action position if needed.
        if task.dock_point.is_none()
            && self.dozer_action_state == WorkerDozerActionState::PickActionPos
        {
            if let Some(current) = self.current_task {
                let points = &self.dozer_dock_points[current.as_index()];
                if points[0].valid {
                    task.dock_point = Some(points[0].location);
                }
            }
        }

        if task.dock_point.is_none()
            && self.dozer_action_state == WorkerDozerActionState::PickActionPos
        {
            let start_angle = (owner_pos_local.y - target_pos_local.y)
                .atan2(owner_pos_local.x - target_pos_local.x);
            let mut options = FindPositionOptions::default();
            options.min_radius = target_radius;
            options.max_radius = 100.0;
            options.start_angle = Some(start_angle);
            options.source_to_path_to_dest_id = Some(self.object_id);
            if !owner_airborne {
                options.max_z_delta = 10.0;
            } else {
                options.ignore_object_id = Some(task.target_id);
            }

            let mut dock_pos = target_pos;
            if let Some(partition) = ThePartitionManager::get() {
                if partition.find_position_around_with_options(&target_pos, &options, &mut dock_pos)
                {
                    task.dock_point = Some(Coord3D::new(dock_pos.x, dock_pos.y, dock_pos.z));
                }
            }
            if task.dock_point.is_none() {
                task.dock_point = Some(Coord3D::new(dock_pos.x, dock_pos.y, dock_pos.z));
            }
            self.dozer_action_state = WorkerDozerActionState::MoveToActionPos;
        }

        let dock_pos = task.dock_point.unwrap_or(target_pos_local);
        let delta = Coord3D::new(
            owner_pos_local.x - dock_pos.x,
            owner_pos_local.y - dock_pos.y,
            owner_pos_local.z - dock_pos.z,
        );
        let dist_sq = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
        let mut build_complete_rebuild: Option<bool> = None;

        match self.dozer_action_state {
            WorkerDozerActionState::MoveToActionPos => {
                if dist_sq <= MIN_ACTION_TOLERANCE * MIN_ACTION_TOLERANCE {
                    self.dozer_action_state = WorkerDozerActionState::DoAction;
                } else if let Some(ai) = owner_ai_update.as_ref() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let dock_pos_logic = LogicCoord3D::new(dock_pos.x, dock_pos.y, dock_pos.z);
                        if let Err(err) = ai_guard.set_movement_target(&dock_pos_logic) {
                            log::debug!(
                                "WorkerAIUpdate::update_dozer_task set_movement_target failed: {}",
                                err
                            );
                        }
                    }
                }
            }
            WorkerDozerActionState::DoAction => match task.task_type {
                WorkerDozerTaskType::Repair => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let Ok(target_guard) = target.read() else {
                        self.dozer_task = None;
                        return;
                    };
                    if !ActionManager::can_repair_object(
                        &*owner_guard,
                        &*target_guard,
                        CommandSourceType::FromAi,
                    ) {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    if let Some(body) = target_guard.get_body_module() {
                        let max_health = body.get_max_health();
                        let current = body.get_health();
                        if max_health > 0.0 {
                            let delta = max_health * repair_rate * SECONDS_PER_LOGICFRAME_REAL;
                            let new_health = (current + delta).min(max_health);
                            body.set_health(new_health);
                            if new_health >= max_health {
                                if target_is_bridge_tower {
                                    Self::remove_bridge_scaffolding(task.target_id);
                                }
                                self.dozer_task = None;
                                self.clear_task(WorkerDozerTaskSlot::Repair);
                            }
                        }
                    } else {
                        self.dozer_task = None;
                        self.clear_task(WorkerDozerTaskSlot::Repair);
                    }
                }
                WorkerDozerTaskType::ResumeConstruction => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let Ok(target_guard) = target.read() else {
                        self.dozer_task = None;
                        return;
                    };
                    if !ActionManager::can_resume_construction_of(
                        &*owner_guard,
                        &*target_guard,
                        CommandSourceType::FromAi,
                    ) {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let current_percent = target_guard.get_construction_percent() as Real;
                    if current_percent >= 100.0 {
                        self.dozer_task = None;
                        self.clear_task(WorkerDozerTaskSlot::Build);
                        return;
                    }
                    drop(target_guard);
                    let new_percent = (current_percent
                        + repair_rate * 100.0 * SECONDS_PER_LOGICFRAME_REAL)
                        .min(100.0);
                    if let Ok(mut target_write) = target.write() {
                        target_write.set_construction_percent(new_percent);
                    }
                    if new_percent >= 100.0 {
                        build_complete_rebuild = Some(task.is_rebuild);
                    }
                }
                WorkerDozerTaskType::Build => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.dozer_task = None;
                        clear_current(self);
                        return;
                    }
                    let construction_manager = get_construction_manager();
                    let mut manager = match construction_manager.write() {
                        Ok(manager) => manager,
                        Err(_) => {
                            self.dozer_task = None;
                            return;
                        }
                    };

                    if !task.started_construction {
                        let max_health = if task.build_max_health > 0.0 {
                            task.build_max_health
                        } else {
                            let Ok(target_guard) = target.read() else {
                                self.dozer_task = None;
                                return;
                            };
                            target_guard
                                .get_body_module()
                                .map(|body| body.get_max_health())
                                .unwrap_or(0.0)
                        };
                        if let Err(err) = manager.start_construction(
                            task.target_id,
                            self.object_id,
                            max_health,
                            task.build_total_frames.max(1),
                            task.is_rebuild,
                        ) {
                            log::debug!(
                                "WorkerAIUpdate::update_dozer_task start_construction failed: {}",
                                err
                            );
                        }
                        task.started_construction = true;
                    }

                    let completed = manager.update_for_dozer(self.object_id);
                    let progress = manager.get_progress(task.target_id).unwrap_or(0.0);
                    let current_health = manager.get_current_health(task.target_id);
                    if let Ok(mut target_write) = target.write() {
                        target_write.set_construction_percent(progress);
                        if let Some(health) = current_health {
                            if let Err(err) = target_write.set_health(health) {
                                log::debug!(
                                    "WorkerAIUpdate::update_dozer_task set_health failed: {}",
                                    err
                                );
                            }
                        }
                    }
                    if completed.contains(&task.target_id) {
                        build_complete_rebuild = Some(task.is_rebuild);
                    }
                }
                WorkerDozerTaskType::Fortify => {
                    // C++ path leaves fortify as a no-op; complete immediately so the worker AI does not stall.
                    self.dozer_task = None;
                    self.clear_task(WorkerDozerTaskSlot::Fortify);
                }
            },
            WorkerDozerActionState::PickActionPos => {}
        }

        if let Some(is_rebuild) = build_complete_rebuild {
            self.handle_build_completion(&owner, &target, is_rebuild);
            self.dozer_task = None;
            self.clear_task(WorkerDozerTaskSlot::Build);
        }
    }

    fn handle_build_completion(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        target: &Arc<RwLock<Object>>,
        is_rebuild: bool,
    ) {
        let mut target_display_name: Option<String> = None;
        let mut target_pos: Option<LogicCoord3D> = None;
        let mut controlling_player: Option<Arc<RwLock<crate::player::Player>>> = None;

        if let Ok(mut target_guard) = target.write() {
            target_guard.clear_status(
                crate::common::ObjectStatusMaskType::from_status(
                    crate::common::ObjectStatusTypes::UnderConstruction,
                ) | crate::common::ObjectStatusMaskType::from_status(
                    crate::common::ObjectStatusTypes::Reconstructing,
                ),
            );

            if let Err(err) = target_guard.clear_model_condition_flags(
                ModelConditionFlags::AWAITING_CONSTRUCTION
                    | ModelConditionFlags::PARTIALLY_CONSTRUCTED
                    | ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
            ) {
                log::debug!(
                    "WorkerAIUpdate::handle_build_completion clear_model_condition_flags failed: {}",
                    err
                );
            }
            target_guard.set_construction_percent(crate::object::CONSTRUCTION_COMPLETE);

            if let Some(body) = target_guard.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    if let Err(err) = body_guard.evaluate_visual_condition() {
                        log::debug!(
                            "WorkerAIUpdate::handle_build_completion evaluate_visual_condition failed: {}",
                            err
                        );
                    }
                }
            }

            target_guard.handle_partition_cell_maintenance();
            target_guard.update_upgrade_modules_from_player();
            target_guard.on_build_complete();

            let template = target_guard.get_template();
            target_display_name = Some(template.get_name().as_str().to_string());
            target_pos = Some(*target_guard.get_position());
            controlling_player = target_guard.get_controlling_player();
        }

        if let Some(player) = controlling_player {
            if let Ok(mut player_guard) = player.write() {
                player_guard.on_structure_construction_complete(Some(owner), target, is_rebuild);
            }
        }

        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_locally_controlled() {
                if let Some(display_name) = target_display_name.as_ref() {
                    let format = crate::helpers::TheGameText::fetch("DOZER:ConstructionComplete");
                    let message = if format.contains("%s") {
                        format.replace("%s", display_name)
                    } else {
                        format!("{} {}", format, display_name)
                    };
                    crate::helpers::TheInGameUI::display_message(&message);
                }

                if let Some(voice) = owner_guard
                    .get_template()
                    .get_per_unit_sound("VoiceTaskComplete")
                {
                    if let Some(audio) = TheAudio::get() {
                        let mut event = voice;
                        event.set_object_id(owner_guard.get_id());
                        audio.add_audio_event(&event);
                    }
                }

                if let (Some(radar), Some(pos)) = (crate::helpers::TheRadar::get(), target_pos) {
                    radar.create_event(
                        &pos,
                        game_engine::common::system::radar::RadarEventType::Construction,
                        4.0,
                    );
                }
            }
        }
    }

    /// Same interface as SupplyTruckAIUpdate for consistency
    pub fn lose_one_box(&mut self) -> bool {
        if self.number_boxes == 0 {
            return false;
        }
        self.number_boxes -= 1;
        self.update_drawable_supply_status();
        true
    }

    pub fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        if self.number_boxes >= self.data.max_boxes {
            return false;
        }
        self.number_boxes += 1;

        // Play depleted voice if took last box
        if remaining_stock == 0 && !self.data.supplies_depleted_voice.is_empty() {
            let mut play_depleted = true;
            if let Some(best_warehouse) = resource::find_best_supply_warehouse(self.object_id) {
                if let (Some(owner), Some(warehouse)) = (
                    self.owner_object(),
                    TheGameLogic::find_object_by_id(best_warehouse),
                ) {
                    if let (Ok(owner_guard), Ok(warehouse_guard)) = (owner.read(), warehouse.read())
                    {
                        let delta = *owner_guard.get_position() - *warehouse_guard.get_position();
                        let distance =
                            (delta.x * delta.x + delta.y * delta.y + delta.z * delta.z).sqrt();
                        let is_ai_player = owner_guard
                            .get_controlling_player_id()
                            .and_then(|player_id| {
                                let Ok(list) = player_list().read() else {
                                    return None;
                                };
                                list.get_player(player_id as i32).cloned()
                            })
                            .and_then(|player| {
                                player.read().ok().map(|guard| guard.is_skirmish_ai())
                            })
                            .unwrap_or(false);
                        if distance <= self.get_warehouse_scan_distance(is_ai_player) / 4.0 {
                            play_depleted = false;
                        }
                    }
                }
            }

            if play_depleted {
                if let Some(audio) = &self.audio_system {
                    audio.play_voice_event(&self.data.supplies_depleted_voice, self.object_id);
                }
            }
        }

        self.update_drawable_supply_status();
        true
    }

    pub fn get_upgraded_supply_boost(&self) -> u32 {
        if let Some(upgrade) = &self.upgrade_system {
            upgrade.get_supply_boost(self.player_index)
        } else {
            self.data.upgraded_supply_boost
        }
    }

    /// Repair health percent per second (matches C++ WorkerAIUpdate::getRepairHealthPerSecond).
    pub fn get_repair_health_per_second(&self) -> Real {
        self.data.repair_health_percent_per_second
    }

    /// Worker bored time (matches C++ WorkerAIUpdate::getBoredTime).
    pub fn get_bored_time(&self) -> Real {
        self.data.bored_time
    }

    /// Worker bored range (matches C++ WorkerAIUpdate::getBoredRange).
    pub fn get_bored_range(&self) -> Real {
        self.data.bored_range
    }

    pub fn get_number_boxes(&self) -> i32 {
        self.number_boxes
    }

    pub fn set_preferred_dock(&mut self, dock_id: ObjectID) {
        self.preferred_dock = Some(dock_id);
    }

    pub fn get_preferred_dock(&self) -> Option<ObjectID> {
        self.preferred_dock
    }

    pub fn set_force_wanting_state(&mut self, force: bool) {
        self.force_wanting_state = force;
    }

    pub fn is_forced_into_wanting_state(&self) -> bool {
        self.force_wanting_state
    }

    pub fn set_force_busy_state(&mut self, force: bool) {
        self.force_busy_state = force;
    }

    pub fn is_forced_into_busy_state(&self) -> bool {
        self.force_busy_state
    }

    pub fn is_available_for_supplying(&self) -> bool {
        true
    }

    pub fn is_currently_ferrying_supplies(&self) -> bool {
        if let Some(machine) = &self.state_machine {
            match machine.current_state_id() {
                Some(ST_IDLE) | Some(ST_BUSY) | Some(ST_REGROUPING) => false,
                Some(ST_WANTING) | Some(ST_DOCKING) => true,
                _ => false,
            }
        } else {
            matches!(
                self.state,
                SupplyTruckState::Wanting | SupplyTruckState::Docking
            )
        }
    }

    /// Get action delay for a dock (matches C++ WorkerAIUpdate::getActionDelayForDock).
    pub fn get_action_delay_for_dock(&self, is_warehouse: bool) -> u32 {
        if is_warehouse {
            self.data.warehouse_delay
        } else {
            self.data.center_delay
        }
    }

    /// Get warehouse scan distance (AI players get 2x distance).
    pub fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Real {
        if is_ai_player {
            self.data.warehouse_scan_distance * 2.0
        } else {
            self.data.warehouse_scan_distance
        }
    }

    pub fn get_state(&self) -> SupplyTruckState {
        self.state
    }

    pub fn set_state(&mut self, state: SupplyTruckState) {
        self.state = state;
    }
}

// ============================================================================
// PLAYER SUPPLY MANAGEMENT
// ============================================================================

/// Player's supply management system
#[derive(Debug)]
pub struct PlayerSupplyManager {
    /// Player's money account
    money: Arc<RwLock<Money>>,
    /// Resource gathering manager
    resource_manager: Arc<RwLock<ResourceGatheringManager>>,
    /// Supply box value (can be modified by upgrades)
    supply_box_value: u32,
    /// Player's faction
    faction: Faction,
    /// Supply piles accessible to this player
    supply_piles: Vec<SupplyPile>,
    /// USA: Supply drop zones
    supply_drop_zones: Vec<SupplyDropZone>,
    /// China: Hacker income sources
    hacker_incomes: Vec<HackerIncome>,
    /// GLA: Black market income
    black_market: Option<BlackMarketIncome>,
}

impl PlayerSupplyManager {
    pub fn new(player_index: PlayerIndex, starting_money: u32) -> Self {
        Self::new_with_faction(player_index, starting_money, Faction::USA)
    }

    pub fn new_with_faction(
        player_index: PlayerIndex,
        starting_money: u32,
        faction: Faction,
    ) -> Self {
        Self {
            money: Arc::new(RwLock::new(Money::new(player_index, starting_money))),
            resource_manager: Arc::new(RwLock::new(ResourceGatheringManager::new())),
            supply_box_value: BASE_VALUE_PER_SUPPLY_BOX as u32,
            faction,
            supply_piles: Vec::new(),
            supply_drop_zones: Vec::new(),
            hacker_incomes: Vec::new(),
            black_market: None,
        }
    }

    pub fn get_money(&self) -> Arc<RwLock<Money>> {
        Arc::clone(&self.money)
    }

    pub fn get_resource_manager(&self) -> Arc<RwLock<ResourceGatheringManager>> {
        Arc::clone(&self.resource_manager)
    }

    pub fn get_supply_box_value(&self) -> u32 {
        self.supply_box_value
    }

    pub fn set_supply_box_value(&mut self, value: u32) {
        self.supply_box_value = value;
    }

    pub fn get_faction(&self) -> Faction {
        self.faction
    }

    // Supply pile management
    pub fn add_supply_pile(&mut self, pile: SupplyPile) {
        self.supply_piles.push(pile);
    }

    pub fn remove_supply_pile(&mut self, pile_id: ObjectID) {
        self.supply_piles.retain(|p| p.pile_id != pile_id);
    }

    pub fn get_supply_piles(&self) -> &[SupplyPile] {
        &self.supply_piles
    }

    pub fn get_supply_piles_mut(&mut self) -> &mut Vec<SupplyPile> {
        &mut self.supply_piles
    }

    // USA: Supply drop zone management
    pub fn add_supply_drop_zone(&mut self, zone: SupplyDropZone) {
        if self.faction == Faction::USA {
            self.supply_drop_zones.push(zone);
        }
    }

    pub fn request_supply_drop(&mut self, zone_id: ObjectID, current_frame: u32) -> u32 {
        if self.faction != Faction::USA {
            return 0;
        }

        for zone in &mut self.supply_drop_zones {
            if zone.zone_id == zone_id {
                return zone.request_drop(current_frame);
            }
        }
        0
    }

    // China: Hacker income management
    pub fn add_hacker(&mut self, hacker: HackerIncome) {
        if self.faction == Faction::China {
            self.hacker_incomes.push(hacker);
        }
    }

    pub fn remove_hacker(&mut self, hacker_id: ObjectID) {
        self.hacker_incomes.retain(|h| h.hacker_id != hacker_id);
    }

    pub fn update_hacker_income(&mut self, current_frame: u32) -> u32 {
        if self.faction != Faction::China {
            return 0;
        }

        let mut total_income = 0;
        for hacker in &mut self.hacker_incomes {
            total_income += hacker.update(current_frame);
        }
        total_income
    }

    // GLA: Black market management
    pub fn set_black_market(&mut self, market: BlackMarketIncome) {
        if self.faction == Faction::GLA {
            self.black_market = Some(market);
        }
    }

    pub fn update_black_market_income(&mut self, current_frame: u32) -> u32 {
        if self.faction != Faction::GLA {
            return 0;
        }

        if let Some(market) = &mut self.black_market {
            market.update(current_frame)
        } else {
            0
        }
    }

    pub fn upgrade_black_market(&mut self) {
        if let Some(market) = &mut self.black_market {
            market.upgrade();
        }
    }
}

// C++ parity: WorkerAIUpdateModuleData and WorkerAIUpdate save/load
impl Snapshotable for WorkerAIUpdateData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        xfer_io(xfer.xfer_int(&mut self.max_boxes))?;
        xfer_io(xfer.xfer_real(&mut self.warehouse_scan_distance))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.warehouse_delay))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.center_delay))?;
        xfer_io(xfer.xfer_real(&mut self.repair_health_percent_per_second))?;
        xfer_io(xfer.xfer_real(&mut self.bored_time))?;
        xfer_io(xfer.xfer_real(&mut self.bored_range))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.upgraded_supply_boost))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for WorkerAIUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());

        self.data.xfer(xfer)?;

        let mut state_disc = self.state as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut state_disc))?;
        self.state = match state_disc {
            0 => SupplyTruckState::Idle,
            1 => SupplyTruckState::Busy,
            2 => SupplyTruckState::Wanting,
            3 => SupplyTruckState::Regrouping,
            _ => SupplyTruckState::Idle,
        };

        xfer_io(xfer.xfer_int(&mut self.number_boxes))?;

        let mut has_dock = self.preferred_dock.is_some() as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut has_dock))?;
        if has_dock != 0 {
            let mut dock_id = self.preferred_dock.unwrap_or(0);
            xfer_io(xfer.xfer_unsigned_int(&mut dock_id))?;
            self.preferred_dock = Some(dock_id);
        } else {
            self.preferred_dock = None;
        }

        xfer_io(xfer.xfer_bool(&mut self.force_wanting_state))?;
        xfer_io(xfer.xfer_bool(&mut self.force_busy_state))?;

        let mut dozer_action_disc = self.dozer_action_state as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut dozer_action_disc))?;
        self.dozer_action_state = match dozer_action_disc {
            0 => WorkerDozerActionState::PickActionPos,
            1 => WorkerDozerActionState::MoveToActionPos,
            2 => WorkerDozerActionState::DoAction,
            _ => WorkerDozerActionState::PickActionPos,
        };

        let mut has_task = self.current_task.is_some() as u32;
        xfer_io(xfer.xfer_unsigned_int(&mut has_task))?;
        if has_task != 0 {
            let mut task_disc = self.current_task.map(|t| t as u32).unwrap_or(0);
            xfer_io(xfer.xfer_unsigned_int(&mut task_disc))?;
            self.current_task = match task_disc {
                0 => Some(WorkerDozerTaskSlot::Build),
                1 => Some(WorkerDozerTaskSlot::Repair),
                2 => Some(WorkerDozerTaskSlot::Fortify),
                _ => None,
            };
        } else {
            self.current_task = None;
        }

        xfer_io(xfer.xfer_unsigned_int(&mut self.object_id))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.player_index))?;

        for entry in &mut self.dozer_tasks {
            xfer_io(xfer.xfer_unsigned_int(&mut entry.target_id))?;
            xfer_io(xfer.xfer_unsigned_int(&mut entry.order_frame))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
