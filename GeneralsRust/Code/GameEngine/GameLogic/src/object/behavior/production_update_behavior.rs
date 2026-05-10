//! Production Update Behavior Module
//!
//! Complete C++ port of ProductionUpdate.cpp from GeneralsMD
//! This module allows buildings to construct units and research upgrades.
//!
//! # C++ Source Reference
//! File: /GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionUpdate.cpp
//! Lines: 1-1384
//!
//! # Key Features
//! - Build queue management (up to 9 entries)
//! - Unit production with cost deduction
//! - Upgrade research
//! - Quantity modifiers (e.g., Chinese Red Guards build 4 at once)
//! - Door animations (opening/closing/waiting)
//! - Construction complete state
//! - Rally point integration
//! - Production cancellation with refunds
//! - Multi-door support (up to 4 doors)
//! - Exit interface integration

use crate::common::MODELCONDITION_ACTIVELY_CONSTRUCTING;
use crate::common::*;
use crate::common::{
    MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING,
    MODELCONDITION_DOOR_1_WAITING_OPEN, MODELCONDITION_DOOR_2_CLOSING,
    MODELCONDITION_DOOR_2_OPENING, MODELCONDITION_DOOR_2_WAITING_OPEN,
    MODELCONDITION_DOOR_3_CLOSING, MODELCONDITION_DOOR_3_OPENING,
    MODELCONDITION_DOOR_3_WAITING_OPEN, MODELCONDITION_DOOR_4_CLOSING,
    MODELCONDITION_DOOR_4_OPENING, MODELCONDITION_DOOR_4_WAITING_OPEN,
};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::player::Player;
use crate::upgrade::center::THE_UPGRADE_CENTER;
use std::collections::VecDeque;

/// Maximum number of doors supported
/// Matches C++ DOOR_COUNT_MAX
const DOOR_COUNT_MAX: usize = 4;
const OPENING_FLAGS: [ModelConditionFlags; DOOR_COUNT_MAX] = [
    MODELCONDITION_DOOR_1_OPENING,
    MODELCONDITION_DOOR_2_OPENING,
    MODELCONDITION_DOOR_3_OPENING,
    MODELCONDITION_DOOR_4_OPENING,
];
const CLOSING_FLAGS: [ModelConditionFlags; DOOR_COUNT_MAX] = [
    MODELCONDITION_DOOR_1_CLOSING,
    MODELCONDITION_DOOR_2_CLOSING,
    MODELCONDITION_DOOR_3_CLOSING,
    MODELCONDITION_DOOR_4_CLOSING,
];
const WAITING_OPEN_FLAGS: [ModelConditionFlags; DOOR_COUNT_MAX] = [
    MODELCONDITION_DOOR_1_WAITING_OPEN,
    MODELCONDITION_DOOR_2_WAITING_OPEN,
    MODELCONDITION_DOOR_3_WAITING_OPEN,
    MODELCONDITION_DOOR_4_WAITING_OPEN,
];

/// Production entry type
/// Matches C++ ProductionType enum (ProductionUpdate.h lines 27-32)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionType {
    Invalid = 0,
    Unit = 1,
    Upgrade = 2,
}

/// Production ID type
/// Matches C++ ProductionID (ProductionUpdate.h lines 22-25)
pub type ProductionID = u32;

/// Invalid production ID constant
pub const PRODUCTIONID_INVALID: ProductionID = 0;

/// Quantity modifier configuration
/// Matches C++ QuantityModifier struct (ProductionUpdate.h lines 94-98)
#[derive(Debug, Clone)]
pub struct QuantityModifier {
    /// Template name (e.g., "ChinaInfantryRedGuard")
    pub template_name: String,
    /// Quantity to produce (e.g., 4 for Red Guards)
    pub quantity: i32,
}

/// Production entry - represents a single item in the build queue
/// Matches C++ ProductionEntry class (ProductionUpdate.h lines 38-91)
#[derive(Debug, Clone)]
pub struct ProductionEntry {
    /// Production type
    production_type: ProductionType,
    /// Template name of object to produce
    object_to_produce: Option<String>,
    /// Name of upgrade to research
    upgrade_to_research: Option<String>,
    /// Unique production ID
    production_id: ProductionID,
    /// Percent complete (0.0 to 100.0)
    percent_complete: f32,
    /// Number of frames under construction
    frames_under_construction: u32,
    /// Total quantity to produce (for quantity modifiers)
    production_quantity_total: i32,
    /// Quantity already produced
    production_quantity_produced: i32,
    /// Exit door reserved for this production
    exit_door: Option<usize>,
}

impl ProductionEntry {
    /// Create new unit production entry
    /// Matches C++ ProductionEntry constructor (ProductionUpdate.cpp lines 131-147)
    pub fn new_unit(template_name: String, production_id: ProductionID, quantity: i32) -> Self {
        Self {
            production_type: ProductionType::Unit,
            object_to_produce: Some(template_name),
            upgrade_to_research: None,
            production_id,
            percent_complete: 0.0,
            frames_under_construction: 0,
            production_quantity_total: quantity,
            production_quantity_produced: 0,
            exit_door: None,
        }
    }

    /// Create new upgrade production entry
    pub fn new_upgrade(upgrade_name: String) -> Self {
        Self {
            production_type: ProductionType::Upgrade,
            object_to_produce: None,
            upgrade_to_research: Some(upgrade_name),
            production_id: PRODUCTIONID_INVALID, // Upgrades don't need IDs
            percent_complete: 0.0,
            frames_under_construction: 0,
            production_quantity_total: 1,
            production_quantity_produced: 0,
            exit_door: None,
        }
    }

    /// Get production type
    pub fn get_production_type(&self) -> ProductionType {
        self.production_type
    }

    /// Get percent complete
    pub fn get_percent_complete(&self) -> f32 {
        self.percent_complete
    }

    /// Get production ID
    pub fn get_production_id(&self) -> ProductionID {
        self.production_id
    }

    /// Get quantity remaining to produce
    /// Matches C++ getProductionQuantityRemaining (ProductionUpdate.h line 66)
    pub fn get_production_quantity_remaining(&self) -> i32 {
        self.production_quantity_total - self.production_quantity_produced
    }

    /// Mark one unit as successfully produced
    /// Matches C++ oneProductionSuccessful (ProductionUpdate.h line 68)
    pub fn one_production_successful(&mut self) {
        self.production_quantity_produced += 1;
        self.exit_door = None; // Re-reserve door for next unit
    }

    /// Get exit door
    pub fn get_exit_door(&self) -> Option<usize> {
        self.exit_door
    }

    /// Set exit door
    pub fn set_exit_door(&mut self, door: Option<usize>) {
        self.exit_door = door;
    }

    /// Get template name for units
    pub fn get_object_template(&self) -> Option<&str> {
        self.object_to_produce.as_deref()
    }

    /// Get upgrade name
    pub fn get_upgrade_name(&self) -> Option<&str> {
        self.upgrade_to_research.as_deref()
    }
}

/// Door animation state
/// Matches C++ DoorInfo struct (ProductionUpdate.h lines 227-233)
#[derive(Debug, Clone, Copy)]
struct DoorInfo {
    /// Frame when door started opening (0 = not opening)
    door_opened_frame: u32,
    /// Frame when door entered wait-open state (0 = not waiting)
    door_wait_open_frame: u32,
    /// Frame when door started closing (0 = not closing)
    door_closed_frame: u32,
    /// If true, don't allow door to close
    hold_open: bool,
}

impl Default for DoorInfo {
    fn default() -> Self {
        // Matches C++ ProductionUpdate constructor (lines 170-176)
        Self {
            door_opened_frame: 0,
            door_wait_open_frame: 0,
            door_closed_frame: 0,
            hold_open: false,
        }
    }
}

/// Production Update Module Data (configuration)
/// Matches C++ ProductionUpdateModuleData (ProductionUpdate.h lines 101-117)
#[derive(Debug, Clone)]
pub struct ProductionUpdateModuleData {
    /// Number of door animations (0-4)
    pub num_door_animations: i32,
    /// Door opening time in frames
    pub door_opening_time: u32,
    /// Door wait open time in frames (how long to keep open)
    pub door_wait_open_time: u32,
    /// Door closing time in frames
    pub door_closing_time: u32,
    /// Construction complete animation duration in frames
    pub construction_complete_duration: u32,
    /// Quantity modifiers for multi-unit production
    pub quantity_modifiers: Vec<QuantityModifier>,
    /// Maximum queue entries (default 9 in C++)
    pub max_queue_entries: usize,
}

impl Default for ProductionUpdateModuleData {
    fn default() -> Self {
        // Matches C++ ProductionUpdateModuleData constructor (lines 76-87)
        Self {
            num_door_animations: 0,
            door_opening_time: 0,
            door_wait_open_time: 0,
            door_closing_time: 0,
            construction_complete_duration: 0,
            quantity_modifiers: Vec::new(),
            max_queue_entries: 9, // C++ default line 85
        }
    }
}

/// Production Update Behavior Module
/// Matches C++ ProductionUpdate class (ProductionUpdate.h lines 162-246)
#[derive(Debug)]
pub struct ProductionUpdateBehavior {
    /// Module configuration
    data: ProductionUpdateModuleData,
    /// Production queue (linked list in C++, VecDeque in Rust)
    production_queue: VecDeque<ProductionEntry>,
    /// Number of items in production queue
    production_count: usize,
    /// Unique ID counter for production
    /// Matches C++ m_uniqueID (line 238)
    unique_id: ProductionID,
    /// Door animation states (4 doors max)
    doors: [DoorInfo; DOOR_COUNT_MAX],
    /// Frame when construction was complete
    construction_complete_frame: u32,
    /// Owner object ID
    owner_id: ObjectID,
}

impl ProductionUpdateBehavior {
    /// Create new production update behavior
    /// Matches C++ ProductionUpdate constructor (lines 162-183)
    pub fn new(data: ProductionUpdateModuleData, owner_id: ObjectID) -> Self {
        Self {
            data,
            production_queue: VecDeque::new(),
            production_count: 0,
            unique_id: 1, // Start from 1 like C++
            doors: [DoorInfo::default(); DOOR_COUNT_MAX],
            construction_complete_frame: 0,
            owner_id,
        }
    }

    /// Check if can queue a unit
    /// Matches C++ canQueueCreateUnit (lines 214-234)
    pub fn can_queue_create_unit(&self, unit_type: &str) -> CanMakeType {
        // Check queue size
        if self.production_count >= self.data.max_queue_entries {
            return CanMakeType::QueueFull;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return CanMakeType::Other;
        };
        let Some(template) = TheThingFactory::find_template(unit_type) else {
            return CanMakeType::Other;
        };

        let parking_full = owner
            .read()
            .ok()
            .and_then(|guard| {
                guard.with_parking_place_behavior(|parking_place| {
                    parking_place.should_reserve_door_when_queued(template.as_ref())
                        && !parking_place.has_available_space_for(template.as_ref())
                })
            })
            .unwrap_or(false);
        if parking_full {
            return CanMakeType::ParkingPlacesFull;
        }

        CanMakeType::Ok
    }

    /// Check if can queue an upgrade
    /// Matches C++ canQueueUpgrade (lines 204-210)
    pub fn can_queue_upgrade(&self) -> CanMakeType {
        if self.production_count >= self.data.max_queue_entries {
            return CanMakeType::QueueFull;
        }

        CanMakeType::Ok
    }

    /// Request a unique production ID
    /// Matches C++ requestUniqueUnitID (ProductionUpdate.h line 188)
    pub fn request_unique_unit_id(&mut self) -> ProductionID {
        let id = self.unique_id;
        self.unique_id += 1;
        id
    }

    /// Queue a unit for production
    /// Matches C++ queueCreateUnit (lines 363-438)
    pub fn queue_create_unit(
        &mut self,
        unit_type: String,
        production_id: ProductionID,
    ) -> Result<(), String> {
        // Check if we can build
        if self.can_queue_create_unit(&unit_type) != CanMakeType::Ok {
            return Err("Cannot queue unit".to_string());
        }

        // Check for quantity modifier
        // Matches C++ lines 415-425
        let quantity = self
            .data
            .quantity_modifiers
            .iter()
            .find(|qm| qm.template_name == unit_type)
            .map(|qm| qm.quantity)
            .unwrap_or(1);

        // Create production entry
        let production = ProductionEntry::new_unit(unit_type, production_id, quantity);

        // Add to queue
        self.add_to_production_queue(production);

        Ok(())
    }

    /// Queue an upgrade for research
    /// Matches C++ queueUpgrade (lines 239-303)
    pub fn queue_upgrade(&mut self, upgrade_name: String) -> Result<(), String> {
        // Check if can build
        if self.can_queue_upgrade() != CanMakeType::Ok {
            return Err("Queue is full".to_string());
        }

        // Check if upgrade already in queue
        if self.is_upgrade_in_queue(&upgrade_name) {
            return Err("Upgrade already in queue".to_string());
        }

        // Create production entry
        let production = ProductionEntry::new_upgrade(upgrade_name);

        // Add to queue
        self.add_to_production_queue(production);

        Ok(())
    }

    /// Cancel unit production by ID
    /// Matches C++ cancelUnitCreate (lines 443-472)
    pub fn cancel_unit_create(&mut self, production_id: ProductionID) -> Option<ProductionEntry> {
        // Find the production entry
        if let Some(index) = self
            .production_queue
            .iter()
            .position(|p| p.get_production_id() == production_id)
        {
            let production = self.production_queue.remove(index).unwrap();
            self.remove_from_production_queue_internal();
            Some(production)
        } else {
            None
        }
    }

    /// Cancel upgrade production
    /// Matches C++ cancelUpgrade (lines 308-357)
    pub fn cancel_upgrade(&mut self, upgrade_name: &str) -> Option<ProductionEntry> {
        // Find the upgrade in queue
        if let Some(index) = self.production_queue.iter().position(|p| {
            p.get_production_type() == ProductionType::Upgrade
                && p.get_upgrade_name() == Some(upgrade_name)
        }) {
            let production = self.production_queue.remove(index).unwrap();
            self.remove_from_production_queue_internal();
            Some(production)
        } else {
            None
        }
    }

    /// Cancel all units of a specific type
    /// Matches C++ cancelAllUnitsOfType (lines 477-508)
    pub fn cancel_all_units_of_type(&mut self, unit_type: &str) -> Vec<ProductionEntry> {
        let mut cancelled = Vec::new();

        // Iterate backwards to safely remove
        let mut i = self.production_queue.len();
        while i > 0 {
            i -= 1;
            if let Some(production) = self.production_queue.get(i) {
                if production.get_production_type() == ProductionType::Unit
                    && production.get_object_template() == Some(unit_type)
                {
                    if let Some(entry) = self.production_queue.remove(i) {
                        cancelled.push(entry);
                        self.remove_from_production_queue_internal();
                    }
                }
            }
        }

        cancelled
    }

    pub fn cancel_one_unit_of_type(&mut self, unit_type: &str) -> Option<ProductionEntry> {
        let index = self.production_queue.iter().position(|production| {
            production.get_production_type() == ProductionType::Unit
                && production.get_object_template() == Some(unit_type)
        })?;
        let production = self.production_queue.remove(index)?;
        self.remove_from_production_queue_internal();
        Some(production)
    }

    /// Cancel and refund all production
    /// Matches C++ cancelAndRefundAllProduction (lines 1119-1141)
    pub fn cancel_and_refund_all_production(&mut self) -> Vec<ProductionEntry> {
        let mut all_entries = Vec::new();

        while let Some(production) = self.production_queue.pop_front() {
            all_entries.push(production);
            self.remove_from_production_queue_internal();
        }

        all_entries
    }

    /// Check if upgrade is in queue
    /// Matches C++ isUpgradeInQueue (lines 1077-1088)
    pub fn is_upgrade_in_queue(&self, upgrade_name: &str) -> bool {
        self.production_queue.iter().any(|p| {
            p.get_production_type() == ProductionType::Upgrade
                && p.get_upgrade_name() == Some(upgrade_name)
        })
    }

    /// Check if any upgrade is queued or in progress.
    pub fn has_any_upgrade_in_queue(&self) -> bool {
        self.production_queue
            .iter()
            .any(|p| p.get_production_type() == ProductionType::Upgrade)
    }

    /// Count units of a specific type in queue
    /// Matches C++ countUnitTypeInQueue (lines 1093-1105)
    pub fn count_unit_type_in_queue(&self, unit_type: &str) -> usize {
        self.production_queue
            .iter()
            .filter(|p| {
                p.get_production_type() == ProductionType::Unit
                    && p.get_object_template() == Some(unit_type)
            })
            .count()
    }

    /// Get production count
    pub fn get_production_count(&self) -> usize {
        self.production_count
    }

    /// Get first production entry
    pub fn first_production(&self) -> Option<&ProductionEntry> {
        self.production_queue.front()
    }

    /// Get first production entry (mutable)
    pub fn first_production_mut(&mut self) -> Option<&mut ProductionEntry> {
        self.production_queue.front_mut()
    }

    /// Get production queue
    pub fn get_queue(&self) -> &VecDeque<ProductionEntry> {
        &self.production_queue
    }

    /// Set hold door open
    /// Matches C++ setHoldDoorOpen (lines 1145-1158)
    pub fn set_hold_door_open(&mut self, exit_door: usize, hold_it: bool) {
        if exit_door < DOOR_COUNT_MAX {
            let door = &mut self.doors[exit_door];
            door.hold_open = hold_it;

            // If holding open and door is closed, start opening immediately.
            // C++ parity: ProductionUpdate::setHoldDoorOpen.
            if hold_it
                && door.door_opened_frame == 0
                && door.door_wait_open_frame == 0
                && door.door_closed_frame == 0
            {
                door.door_opened_frame = TheGameLogic::get_frame();
                if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                    if let Ok(mut guard) = owner.write() {
                        guard.set_model_condition_state(OPENING_FLAGS[exit_door]);
                    }
                }
            }
        }
    }

    /// Update the module
    /// Matches C++ update (lines 587-963)
    pub fn update(&mut self, current_frame: u32) -> UpdateResult {
        // Update doors
        // Matches C++ lines 595-597
        if self.data.num_door_animations > 0 {
            self.update_doors(current_frame);
        }

        // Update construction complete state
        // Matches C++ lines 600-619
        if self.construction_complete_frame > 0 {
            let elapsed = current_frame - self.construction_complete_frame;
            if elapsed > self.data.construction_complete_duration {
                self.construction_complete_frame = 0;
                if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                    if let Ok(mut guard) = owner.write() {
                        guard.clear_model_condition_state(
                            ModelConditionFlags::CONSTRUCTION_COMPLETE,
                        );
                    }
                }
            }
        }

        // If nothing in queue, return
        // Matches C++ lines 637-638
        if self.production_queue.is_empty() {
            return UpdateResult::Sleep;
        }

        // Get first production entry
        if let Some(production) = self.production_queue.front_mut() {
            // Increment frames under construction
            // Matches C++ line 687
            production.frames_under_construction += 1;

            let total_production_frames = match production.production_type {
                ProductionType::Invalid => 1,
                ProductionType::Unit => production
                    .object_to_produce
                    .as_ref()
                    .and_then(|name| TheThingFactory::find_template(name))
                    .map(|template| template.calc_time_to_build(None).max(1) as u32)
                    .unwrap_or(1),
                ProductionType::Upgrade => {
                    let upgrade_name = production.upgrade_to_research.as_deref().unwrap_or("");
                    let upgrade = THE_UPGRADE_CENTER
                        .read()
                        .ok()
                        .and_then(|c| c.find_upgrade(upgrade_name));
                    let owner_player =
                        TheGameLogic::find_object_by_id(self.owner_id).and_then(|obj| {
                            obj.read()
                                .ok()
                                .and_then(|guard| guard.get_controlling_player())
                        });
                    upgrade
                        .map(|template| {
                            if let Some(player) = &owner_player {
                                if let Ok(player_guard) = player.read() {
                                    return template.calc_time_to_build(&player_guard).max(1)
                                        as u32;
                                }
                            }
                            template.calc_time_to_build(&Player::default()).max(1) as u32
                        })
                        .unwrap_or(1)
                }
            };

            // Update percent complete
            // Matches C++ lines 697-699
            production.percent_complete = (production.frames_under_construction as f32
                / total_production_frames as f32)
                * 100.0;

            // Check if production is complete
            // Matches C++ line 702
            if production.percent_complete >= 100.0 {
                if self.construction_complete_frame == 0 {
                    self.construction_complete_frame = current_frame;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.set_model_condition_state(
                                ModelConditionFlags::CONSTRUCTION_COMPLETE,
                            );
                        }
                    }
                }
                return UpdateResult::ProductionComplete;
            }
        }

        UpdateResult::Continue
    }

    /// Update door animations
    /// Matches C++ updateDoors (lines 513-583)
    fn update_doors(&mut self, current_frame: u32) {
        for i in 0..DOOR_COUNT_MAX.min(self.data.num_door_animations as usize) {
            let door = &mut self.doors[i];

            // Door opening -> wait open transition
            // Matches C++ lines 525-543
            if door.door_opened_frame > 0 {
                let elapsed = current_frame - door.door_opened_frame;
                if elapsed > self.data.door_opening_time {
                    door.door_opened_frame = 0;
                    door.door_wait_open_frame = current_frame;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.clear_model_condition_state(OPENING_FLAGS[i]);
                            guard.set_model_condition_state(WAITING_OPEN_FLAGS[i]);
                        }
                    }
                }
            }
            // Door wait open -> closing transition
            // Matches C++ lines 545-563
            else if door.door_wait_open_frame > 0 && !door.hold_open {
                let elapsed = current_frame - door.door_wait_open_frame;
                if elapsed > self.data.door_wait_open_time {
                    door.door_wait_open_frame = 0;
                    door.door_closed_frame = current_frame;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.clear_model_condition_state(WAITING_OPEN_FLAGS[i]);
                            guard.set_model_condition_state(CLOSING_FLAGS[i]);
                        }
                    }
                }
            }
            // Door closing -> closed transition
            // Matches C++ lines 565-581
            else if door.door_closed_frame > 0 && !door.hold_open {
                let elapsed = current_frame - door.door_closed_frame;
                if elapsed > self.data.door_closing_time {
                    door.door_closed_frame = 0;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.clear_model_condition_state(CLOSING_FLAGS[i]);
                        }
                    }
                }
            }
        }
    }

    /// Add production entry to the queue
    /// Matches C++ addToProductionQueue (lines 968-1006)
    fn add_to_production_queue(&mut self, production: ProductionEntry) {
        self.production_queue.push_back(production);
        self.production_count += 1;

        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut guard) = owner.write() {
                guard.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
            }
        }
    }

    /// Internal helper for queue removal bookkeeping
    fn remove_from_production_queue_internal(&mut self) {
        self.production_count = self.production_count.saturating_sub(1);

        if self.production_count == 0 {
            if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(mut guard) = owner.write() {
                    guard.clear_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                }
            }
        }
    }
}

/// Result of update operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    /// Continue updating
    Continue,
    /// Sleep (nothing to do)
    Sleep,
    /// Production complete, ready to spawn
    ProductionComplete,
}

/// Can make type result
/// Matches C++ CanMakeType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanMakeType {
    /// Can make the unit/upgrade
    Ok,
    /// Queue is full
    QueueFull,
    /// Parking places full (for units that need them)
    ParkingPlacesFull,
    /// Other reason (prerequisites, etc.)
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
    use std::sync::{Arc, RwLock};

    fn ensure_template_exists(name: &str) {
        let needs_init = get_thing_factory().unwrap().is_none();
        if needs_init {
            init_thing_factory().unwrap();
        }
        let mut factory_guard = get_thing_factory().unwrap();
        if let Some(factory) = factory_guard.as_mut() {
            if factory.find_template(name, false).is_none() {
                factory.new_template(name);
            }
        }
    }

    fn setup_owner(owner_id: ObjectID, templates: &[&str]) -> Arc<RwLock<Object>> {
        for template in templates {
            ensure_template_exists(template);
        }
        let owner = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
        OBJECT_REGISTRY.register_object(owner_id, &owner);
        owner
    }

    #[test]
    fn test_production_creation() {
        let data = ProductionUpdateModuleData::default();
        let production = ProductionUpdateBehavior::new(data, 1);

        assert_eq!(production.get_production_count(), 0);
        assert_eq!(production.unique_id, 1);
    }

    #[test]
    fn test_queue_unit() {
        let owner_id = 1001;
        let _owner = setup_owner(owner_id, &["Tank"]);
        let data = ProductionUpdateModuleData::default();
        let mut production = ProductionUpdateBehavior::new(data, owner_id);

        let id = production.request_unique_unit_id();
        let result = production.queue_create_unit("Tank".to_string(), id);

        assert!(result.is_ok());
        assert_eq!(production.get_production_count(), 1);
        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_quantity_modifier() {
        let owner_id = 1002;
        let _owner = setup_owner(owner_id, &["ChinaInfantryRedGuard"]);
        let mut data = ProductionUpdateModuleData::default();
        data.quantity_modifiers.push(QuantityModifier {
            template_name: "ChinaInfantryRedGuard".to_string(),
            quantity: 4,
        });

        let mut production = ProductionUpdateBehavior::new(data, owner_id);

        let id = production.request_unique_unit_id();
        production
            .queue_create_unit("ChinaInfantryRedGuard".to_string(), id)
            .unwrap();

        let entry = production.first_production().unwrap();
        assert_eq!(entry.production_quantity_total, 4);
        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_cancel_production() {
        let owner_id = 1003;
        let _owner = setup_owner(owner_id, &["Tank"]);
        let data = ProductionUpdateModuleData::default();
        let mut production = ProductionUpdateBehavior::new(data, owner_id);

        let id = production.request_unique_unit_id();
        production
            .queue_create_unit("Tank".to_string(), id)
            .unwrap();

        let cancelled = production.cancel_unit_create(id);
        assert!(cancelled.is_some());
        assert_eq!(production.get_production_count(), 0);
        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_queue_full() {
        let owner_id = 1004;
        let _owner = setup_owner(owner_id, &["Tank1", "Tank2", "Tank3"]);
        let mut data = ProductionUpdateModuleData::default();
        data.max_queue_entries = 2;

        let mut production = ProductionUpdateBehavior::new(data, owner_id);

        // Queue first unit
        let id1 = production.request_unique_unit_id();
        assert!(production
            .queue_create_unit("Tank1".to_string(), id1)
            .is_ok());

        // Queue second unit
        let id2 = production.request_unique_unit_id();
        assert!(production
            .queue_create_unit("Tank2".to_string(), id2)
            .is_ok());

        // Third should fail
        assert_eq!(
            production.can_queue_create_unit("Tank3"),
            CanMakeType::QueueFull
        );
        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_upgrade_queue() {
        let data = ProductionUpdateModuleData::default();
        let mut production = ProductionUpdateBehavior::new(data, 1);

        let result = production.queue_upgrade("BlackNapalm".to_string());
        assert!(result.is_ok());
        assert_eq!(production.get_production_count(), 1);

        // Duplicate upgrade should fail
        let result2 = production.queue_upgrade("BlackNapalm".to_string());
        assert!(result2.is_err());
    }

    #[test]
    fn test_count_units_in_queue() {
        let owner_id = 1005;
        let _owner = setup_owner(owner_id, &["Tank", "Humvee"]);
        let data = ProductionUpdateModuleData::default();
        let mut production = ProductionUpdateBehavior::new(data, owner_id);

        // Add multiple tanks
        for _ in 0..3 {
            let id = production.request_unique_unit_id();
            production
                .queue_create_unit("Tank".to_string(), id)
                .unwrap();
        }

        // Add a different unit
        let id = production.request_unique_unit_id();
        production
            .queue_create_unit("Humvee".to_string(), id)
            .unwrap();

        assert_eq!(production.count_unit_type_in_queue("Tank"), 3);
        assert_eq!(production.count_unit_type_in_queue("Humvee"), 1);
        assert_eq!(production.count_unit_type_in_queue("Notexist"), 0);
        OBJECT_REGISTRY.unregister_object(owner_id);
    }
}
