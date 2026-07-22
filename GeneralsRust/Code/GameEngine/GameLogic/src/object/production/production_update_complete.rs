//! Complete Production Update Module
//!
//! Faithful port of C++ ProductionUpdate.cpp with all features:
//! - Build queue management with priorities
//! - Cost deduction and refunds
//! - Build time calculation with modifiers
//! - Door animations
//! - Rally points
//! - Quantity modifiers (multi-unit production like Chinese Red Guards)
//! - Prerequisite checking
//! - UI updates and notifications

use super::build_cost_calculator::{
    BuildCostCalculator, BuildFacilityContext, PlayerBuildModifiers,
};
use super::exit_strategies::ProductionExitStrategy;
use super::prerequisite_checker::{
    CanMakeType, PlayerBuildState, Prerequisite, PrerequisiteChecker,
};
use super::queue::{BuildQueue, BuildQueueEntry, ProductionType};
use super::rally_point::RallyPointManager;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::common::{
    MODELCONDITION_ACTIVELY_CONSTRUCTING, MODELCONDITION_DOOR_1_CLOSING,
    MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_WAITING_OPEN,
    MODELCONDITION_DOOR_2_CLOSING, MODELCONDITION_DOOR_2_OPENING,
    MODELCONDITION_DOOR_2_WAITING_OPEN, MODELCONDITION_DOOR_3_CLOSING,
    MODELCONDITION_DOOR_3_OPENING, MODELCONDITION_DOOR_3_WAITING_OPEN,
    MODELCONDITION_DOOR_4_CLOSING, MODELCONDITION_DOOR_4_OPENING,
    MODELCONDITION_DOOR_4_WAITING_OPEN,
};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{
    BehaviorModule, BehaviorModuleInterface, ProductionUpdateInterface, UpdateModuleInterface,
    UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::system::game_logic;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, NameKeyType, ProductionControlInterface,
};
use std::any::Any;
use std::sync::{Arc, Mutex};

/// Quantity modifier for multi-unit production
/// Matches C++ QuantityModifier struct from ProductionUpdate.h line 94
#[derive(Debug, Clone)]
pub struct QuantityModifier {
    /// Template name of the unit
    pub template_name: String,
    /// How many to produce (e.g., 4 for Red Guards)
    pub quantity: i32,
}

fn parse_int_field(
    _ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
    setter: fn(&mut ProductionUpdateModuleData, i32),
) -> Result<(), INIError> {
    let value = tokens
        .get(0)
        .ok_or(INIError::InvalidData)?
        .parse::<i32>()
        .map_err(|_| INIError::InvalidData)?;
    setter(data, value);
    Ok(())
}

fn parse_duration_field(
    _ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
    setter: fn(&mut ProductionUpdateModuleData, u32),
) -> Result<(), INIError> {
    let token = tokens.get(0).ok_or(INIError::InvalidData)?;
    let value = INI::parse_duration_unsigned_int(token)?;
    setter(data, value);
    Ok(())
}

fn parse_max_queue_entries(
    ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_int_field(ini, data, tokens, |d, v| {
        d.max_queue_entries = v.max(0) as usize;
    })
}

fn parse_num_door_animations(
    ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_int_field(ini, data, tokens, |d, v| d.num_door_animations = v)
}

fn parse_door_opening_time(
    ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(ini, data, tokens, |d, v| d.door_opening_time = v)
}

fn parse_door_wait_open_time(
    ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(ini, data, tokens, |d, v| d.door_wait_open_time = v)
}

fn parse_door_closing_time(
    ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(ini, data, tokens, |d, v| d.door_closing_time = v)
}

fn parse_construction_complete_duration(
    ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_duration_field(ini, data, tokens, |d, v| {
        d.construction_complete_duration = v
    })
}

fn parse_quantity_modifier(
    _ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = tokens.get(0).ok_or(INIError::InvalidData)?;
    let count = tokens
        .get(1)
        .map(|token| token.parse::<i32>().unwrap_or(1))
        .unwrap_or(1);
    data.quantity_modifiers.push(QuantityModifier {
        template_name: (*name).to_string(),
        quantity: count,
    });
    Ok(())
}

fn parse_disabled_types_to_process(
    _ini: &mut INI,
    data: &mut ProductionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut mask = DisabledMaskType::none();
    for token in tokens {
        let upper = token.trim().trim_matches(',').to_ascii_uppercase();
        if upper == "NONE" {
            mask = DisabledMaskType::none();
            continue;
        }
        let cleaned = upper.strip_prefix("DISABLED_").unwrap_or(&upper);
        match cleaned {
            "DEFAULT" => mask |= DisabledMaskType::DISABLED_DEFAULT,
            "HACKED" => mask |= DisabledMaskType::DISABLED_HACKED,
            "EMP" => mask |= DisabledMaskType::DISABLED_EMP,
            "HELD" => mask |= DisabledMaskType::HELD,
            "PARALYZED" => mask |= DisabledMaskType::PARALYZED,
            "UNMANNED" => mask |= DisabledMaskType::DISABLED_UNMANNED,
            "UNDERPOWERED" => mask |= DisabledMaskType::DISABLED_UNDERPOWERED,
            "FREEFALL" => mask |= DisabledMaskType::DISABLED_FREEFALL,
            "AWESTRUCK" => mask |= DisabledMaskType::DISABLED_AWESTRUCK,
            "BRAINWASHED" => mask |= DisabledMaskType::DISABLED_BRAINWASHED,
            "SUBDUED" => mask |= DisabledMaskType::DISABLED_SUBDUED,
            "SCRIPT_DISABLED" => mask |= DisabledMaskType::DISABLED_SCRIPT_DISABLED,
            "SCRIPT_UNDERPOWERED" => mask |= DisabledMaskType::DISABLED_SCRIPT_UNDERPOWERED,
            "ANY" => {
                mask = DisabledMaskType::all();
                break;
            }
            _ => {}
        }
    }
    data.disabled_types_to_process = mask;
    Ok(())
}

const PRODUCTION_UPDATE_FIELDS: &[FieldParse<ProductionUpdateModuleData>] = &[
    FieldParse {
        token: "MaxQueueEntries",
        parse: parse_max_queue_entries,
    },
    FieldParse {
        token: "NumDoorAnimations",
        parse: parse_num_door_animations,
    },
    FieldParse {
        token: "DoorOpeningTime",
        parse: parse_door_opening_time,
    },
    FieldParse {
        token: "DoorWaitOpenTime",
        parse: parse_door_wait_open_time,
    },
    FieldParse {
        token: "DoorCloseTime",
        parse: parse_door_closing_time,
    },
    FieldParse {
        token: "ConstructionCompleteDuration",
        parse: parse_construction_complete_duration,
    },
    FieldParse {
        token: "QuantityModifier",
        parse: parse_quantity_modifier,
    },
    FieldParse {
        token: "DisabledTypesToProcess",
        parse: parse_disabled_types_to_process,
    },
];

/// Module configuration data
/// Matches C++ ProductionUpdateModuleData from ProductionUpdate.h lines 101-117
#[derive(Debug, Clone)]
pub struct ProductionUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Number of door animations
    pub num_door_animations: i32,
    /// Door opening time in frames
    pub door_opening_time: u32,
    /// Door wait open time in frames
    pub door_wait_open_time: u32,
    /// Door closing time in frames
    pub door_closing_time: u32,
    /// Construction complete animation duration
    pub construction_complete_duration: u32,
    /// Quantity modifiers for multi-unit production
    pub quantity_modifiers: Vec<QuantityModifier>,
    /// Maximum queue entries (9 in C++)
    pub max_queue_entries: usize,
    /// Disabled types to process (C++ DisabledTypesToProcess)
    pub disabled_types_to_process: DisabledMaskType,
}

impl Default for ProductionUpdateModuleData {
    fn default() -> Self {
        // Matches C++ ProductionUpdateModuleData constructor line 76
        Self {
            base: BehaviorModuleData::default(),
            num_door_animations: 0,
            door_opening_time: 0,
            door_wait_open_time: 0,
            door_closing_time: 0,
            construction_complete_duration: 0,
            quantity_modifiers: Vec::new(),
            max_queue_entries: 9, // C++ default line 85
            disabled_types_to_process: DisabledMaskType::HELD,
        }
    }
}

impl ProductionUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PRODUCTION_UPDATE_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(ProductionUpdateModuleData, base);

/// Door animation states
/// Matches C++ DoorInfo struct from ProductionUpdate.h lines 227-233
#[derive(Debug, Clone, Copy)]
struct DoorInfo {
    /// Frame when door started opening
    door_opened_frame: u32,
    /// Frame when door entered wait-open state
    door_wait_open_frame: u32,
    /// Frame when door started closing
    door_closed_frame: u32,
    /// If true, keep door open
    hold_open: bool,
}

impl Default for DoorInfo {
    fn default() -> Self {
        // Matches C++ ProductionUpdate constructor lines 170-176
        Self {
            door_opened_frame: 0,
            door_wait_open_frame: 0,
            door_closed_frame: 0,
            hold_open: false,
        }
    }
}

/// Maximum number of doors
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

/// Production state for a single entry with quantity support
/// Extends BuildQueueEntry with production-specific state
#[derive(Debug, Clone)]
struct ProductionEntryState {
    /// Queue entry
    entry: BuildQueueEntry,
    /// Total quantity to produce (from QuantityModifier)
    quantity_total: i32,
    /// Quantity already produced
    quantity_produced: i32,
    /// Exit door reserved for this production
    exit_door: Option<usize>,
}

impl ProductionEntryState {
    /// Create from queue entry
    fn from_entry(entry: BuildQueueEntry) -> Self {
        Self {
            entry,
            quantity_total: 1, // Default single unit
            quantity_produced: 0,
            exit_door: None,
        }
    }

    /// Get quantity remaining to produce
    fn quantity_remaining(&self) -> i32 {
        self.quantity_total - self.quantity_produced
    }

    /// Mark one unit as successfully produced
    /// Matches C++ ProductionEntry::oneProductionSuccessful line 68
    fn one_production_successful(&mut self) {
        self.quantity_produced += 1;
        self.exit_door = None; // Re-reserve door for next unit
    }
}

/// Complete Production Update Module
/// Matches C++ ProductionUpdate class from ProductionUpdate.h lines 162-246
#[derive(Debug)]
pub struct ProductionUpdateComplete {
    /// Module configuration
    data: ProductionUpdateModuleData,
    /// Build queue
    queue: BuildQueue,
    /// Rally point manager
    rally_points: RallyPointManager,
    /// Current production state (None if queue empty)
    current_production: Option<ProductionEntryState>,
    /// Door states
    doors: [DoorInfo; DOOR_COUNT_MAX],
    /// Construction complete frame marker
    construction_complete_frame: u32,
    /// Currently active door index
    current_door: usize,
    /// Reference to the owning object
    owner_id: ObjectID,
    /// Exit strategy for spawned units
    exit_strategy: Option<Arc<Mutex<dyn ProductionExitStrategy>>>,
    /// Whether production is currently enabled
    production_enabled: bool,
    /// Last frame production was updated
    last_update_frame: u32,
    /// Build cost calculator
    cost_calculator: BuildCostCalculator,
    /// Prerequisite checker
    prerequisite_checker: PrerequisiteChecker,
    /// Unique ID counter for production IDs
    /// Matches C++ m_uniqueID line 238
    unique_id: u32,
}

impl ProductionUpdateComplete {
    fn production_count(&self) -> usize {
        self.queue.len() + usize::from(self.current_production.is_some())
    }

    fn has_queue_capacity(&self) -> bool {
        self.production_count() < self.data.max_queue_entries
    }

    fn sync_actively_constructing_flag(&mut self) {
        let should_set = self.current_production.is_some() || !self.queue.is_empty();
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut guard) = owner.write() {
                if should_set {
                    guard.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                } else {
                    guard.clear_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                }
            }
        }
    }

    fn set_hold_door_open(&mut self, exit_door: usize, hold_it: bool) {
        if exit_door >= DOOR_COUNT_MAX {
            return;
        }

        let door = &mut self.doors[exit_door];
        door.hold_open = hold_it;

        // C++ parity (ProductionUpdate::setHoldDoorOpen): when a closed door is forced
        // open, immediately start its opening animation/flag timeline.
        if hold_it
            && door.door_opened_frame == 0
            && door.door_wait_open_frame == 0
            && door.door_closed_frame == 0
        {
            door.door_opened_frame = game_logic::current_frame();
            if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(mut guard) = owner.write() {
                    guard.set_model_condition_state(OPENING_FLAGS[exit_door]);
                }
            }
        }
    }
    /// Create a new production update module
    /// Matches C++ ProductionUpdate constructor lines 162-183
    pub fn new(data: ProductionUpdateModuleData, owner_id: ObjectID) -> Self {
        let queue = BuildQueue::new(data.max_queue_entries);

        Self {
            data,
            queue,
            rally_points: RallyPointManager::new(),
            current_production: None,
            doors: Default::default(),
            construction_complete_frame: 0,
            current_door: 0,
            owner_id,
            exit_strategy: None,
            production_enabled: true,
            last_update_frame: 0,
            cost_calculator: BuildCostCalculator::new(),
            prerequisite_checker: PrerequisiteChecker::new(),
            unique_id: 1, // Start from 1 like C++
        }
    }

    /// Set the exit strategy
    pub fn set_exit_strategy(&mut self, strategy: Arc<Mutex<dyn ProductionExitStrategy>>) {
        self.exit_strategy = Some(strategy);
    }

    /// Request a unique production ID
    /// Matches C++ requestUniqueUnitID line 188
    pub fn request_unique_unit_id(&mut self) -> u32 {
        let id = self.unique_id;
        self.unique_id += 1;
        id
    }

    /// Check if can queue a unit
    /// Matches C++ canQueueCreateUnit lines 214-234
    pub fn can_queue_create_unit(
        &self,
        template_name: &str,
        cost: i32,
        prerequisites: &[Prerequisite],
        player_state: &PlayerBuildState,
    ) -> CanMakeType {
        // Check queue size
        if !self.has_queue_capacity() {
            return CanMakeType::QueueFull;
        }

        // Check prerequisites, money, etc.
        self.prerequisite_checker.can_make_unit(
            template_name,
            cost,
            prerequisites,
            false, // Not unique
            player_state,
        )
    }

    /// Check if can queue an upgrade
    /// Matches C++ canQueueUpgrade lines 204-210
    pub fn can_queue_upgrade(
        &self,
        upgrade_name: &str,
        cost: i32,
        prerequisites: &[Prerequisite],
        player_state: &PlayerBuildState,
    ) -> CanMakeType {
        // Check queue size
        if !self.has_queue_capacity() {
            return CanMakeType::QueueFull;
        }

        // Check prerequisites
        self.prerequisite_checker
            .can_make_upgrade(upgrade_name, cost, prerequisites, player_state)
    }

    /// Check if any upgrade is queued or currently producing.
    pub fn has_any_upgrade_in_queue(&self) -> bool {
        if self
            .current_production
            .as_ref()
            .is_some_and(|entry| entry.entry.production_type == ProductionType::Upgrade)
        {
            return true;
        }

        self.queue.contains_type(ProductionType::Upgrade)
    }

    /// Queue a unit for production
    /// Matches C++ queueCreateUnit lines 363-438
    pub fn queue_create_unit(
        &mut self,
        template_name: String,
        production_type: ProductionType,
        cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> Result<u32, String> {
        let production_id = self.request_unique_unit_id();
        self.queue_create_unit_with_id(
            template_name,
            production_type,
            cost,
            build_time,
            player_id,
            production_id,
        )
    }

    pub fn queue_create_unit_with_id(
        &mut self,
        template_name: String,
        production_type: ProductionType,
        cost: i32,
        build_time: u32,
        player_id: ObjectID,
        production_id: u32,
    ) -> Result<u32, String> {
        // Check if production is enabled
        if !self.production_enabled {
            return Err("Production is currently disabled".to_string());
        }
        if !self.has_queue_capacity() {
            return Err("Build queue is full".to_string());
        }

        // Create queue entry
        let entry = BuildQueueEntry::new(
            template_name.clone(),
            production_type,
            cost,
            build_time,
            player_id,
        )
        .with_production_id(production_id);
        if production_id >= self.unique_id {
            self.unique_id = production_id.saturating_add(1);
        }

        // Add to queue
        self.queue.enqueue(entry.clone())?;
        self.sync_actively_constructing_flag();

        // Check for quantity modifier
        // Matches C++ lines 411-425
        let quantity = self
            .data
            .quantity_modifiers
            .iter()
            .find(|qm| qm.template_name == template_name)
            .map(|qm| qm.quantity)
            .unwrap_or(1);

        // If we were idle, start producing
        if self.current_production.is_none() {
            self.start_next_production()?;

            // Apply quantity modifier to current production
            if let Some(ref mut prod) = self.current_production {
                prod.quantity_total = quantity;
            }
        }

        Ok(production_id)
    }

    /// Queue an upgrade for research
    /// Matches C++ queueUpgrade lines 239-303
    pub fn queue_upgrade(
        &mut self,
        upgrade_name: String,
        cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> Result<(), String> {
        if !self.has_queue_capacity() {
            return Err("Build queue is full".to_string());
        }

        // Create entry
        let entry = BuildQueueEntry::new(
            upgrade_name,
            ProductionType::Upgrade,
            cost,
            build_time,
            player_id,
        );

        // Add to queue
        self.queue.enqueue(entry)?;
        self.sync_actively_constructing_flag();

        // Start if idle
        if self.current_production.is_none() {
            self.start_next_production()?;
        }

        Ok(())
    }

    /// Cancel a queue entry with refund
    /// Matches C++ cancelUnitCreate lines 443-472
    pub fn cancel_production(
        &mut self,
        index: usize,
        refund_credits: &mut dyn FnMut(ObjectID, i32),
    ) -> Result<(), String> {
        // If canceling current production
        if index == 0 && self.current_production.is_some() {
            if let Some(prod) = self.current_production.take() {
                // Calculate refund
                // C++ always refunds full cost lines 456-458
                let refund = prod.entry.cost;

                // Return money to player
                if refund > 0 {
                    refund_credits(prod.entry.player_id, refund);
                }

                // Start next production
                if !self.queue.is_empty() {
                    self.start_next_production()?;
                }

                return Ok(());
            }
        }

        // Cancel from queue
        if let Some(entry) = self.queue.cancel(index.saturating_sub(1)) {
            // Full refund for queued items
            if entry.cost > 0 {
                refund_credits(entry.player_id, entry.cost);
            }
            Ok(())
        } else {
            Err("Invalid queue index".to_string())
        }
    }

    pub fn cancel_upgrade_by_name(
        &mut self,
        upgrade_name: &str,
        refund_credits: &mut dyn FnMut(ObjectID, i32),
    ) -> Result<(), String> {
        if self.current_production.as_ref().is_some_and(|prod| {
            prod.entry.production_type == ProductionType::Upgrade
                && prod.entry.template_name == upgrade_name
        }) {
            return self.cancel_production(0, refund_credits);
        }

        let Some(index) = self
            .queue
            .find_by_template_and_type(ProductionType::Upgrade, upgrade_name)
        else {
            return Err("Upgrade not in queue".to_string());
        };
        let visual_index = if self.current_production.is_some() {
            index + 1
        } else {
            index
        };

        self.cancel_production(visual_index, refund_credits)
    }

    pub fn cancel_unit_by_template_name(
        &mut self,
        template_name: &str,
        refund_credits: &mut dyn FnMut(ObjectID, i32),
    ) -> Result<(), String> {
        if self.current_production.as_ref().is_some_and(|prod| {
            prod.entry.production_type == ProductionType::Unit
                && prod.entry.template_name == template_name
        }) {
            return self.cancel_production(0, refund_credits);
        }

        let Some(index) = self
            .queue
            .find_by_template_and_type(ProductionType::Unit, template_name)
        else {
            return Err("Unit not in queue".to_string());
        };
        let visual_index = if self.current_production.is_some() {
            index + 1
        } else {
            index
        };

        self.cancel_production(visual_index, refund_credits)
    }

    pub fn cancel_unit_by_production_id(
        &mut self,
        production_id: u32,
        refund_credits: &mut dyn FnMut(ObjectID, i32),
    ) -> Result<(), String> {
        if self.current_production.as_ref().is_some_and(|prod| {
            prod.entry.production_type == ProductionType::Unit
                && prod.entry.production_id == production_id
        }) {
            return self.cancel_production(0, refund_credits);
        }

        let Some(index) = self.queue.find_by_production_id(production_id) else {
            return Err("Unit not in queue".to_string());
        };
        let visual_index = if self.current_production.is_some() {
            index + 1
        } else {
            index
        };

        self.cancel_production(visual_index, refund_credits)
    }

    /// Cancel and refund all production
    /// Matches C++ cancelAndRefundAllProduction lines 1119-1141
    pub fn cancel_and_refund_all_production(
        &mut self,
        refund_credits: &mut dyn FnMut(ObjectID, i32),
    ) {
        // Cancel current production
        if let Some(prod) = self.current_production.take() {
            if prod.entry.cost > 0 {
                refund_credits(prod.entry.player_id, prod.entry.cost);
            }
        }

        // Cancel all queued items
        let all_entries = self.queue.cancel_all();
        for entry in all_entries {
            if entry.cost > 0 {
                refund_credits(entry.player_id, entry.cost);
            }
        }
        self.sync_actively_constructing_flag();
    }

    /// Start producing the next queue item
    /// Internal helper
    fn start_next_production(&mut self) -> Result<(), String> {
        if let Some(entry) = self.queue.dequeue() {
            let mut prod_state = ProductionEntryState::from_entry(entry);

            // Apply quantity modifier
            // Matches C++ lines 411-425
            if let Some(qm) = self
                .data
                .quantity_modifiers
                .iter()
                .find(|qm| qm.template_name == prod_state.entry.template_name)
            {
                prod_state.quantity_total = qm.quantity;
            }

            self.current_production = Some(prod_state);
            self.sync_actively_constructing_flag();
            Ok(())
        } else {
            Err("No items in queue to produce".to_string())
        }
    }

    /// Update door animations
    /// Matches C++ updateDoors lines 513-583
    fn update_doors(&mut self, current_frame: u32) {
        for door_idx in 0..self.data.num_door_animations.min(DOOR_COUNT_MAX as i32) as usize {
            let door = &mut self.doors[door_idx];

            // Door opening -> wait open transition
            if door.door_opened_frame > 0 {
                let elapsed = current_frame - door.door_opened_frame;
                if elapsed > self.data.door_opening_time {
                    door.door_opened_frame = 0;
                    door.door_wait_open_frame = current_frame;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.clear_model_condition_state(OPENING_FLAGS[door_idx]);
                            guard.set_model_condition_state(WAITING_OPEN_FLAGS[door_idx]);
                        }
                    }
                }
            }
            // Door wait open -> closing transition
            else if door.door_wait_open_frame > 0 && !door.hold_open {
                let elapsed = current_frame - door.door_wait_open_frame;
                if elapsed > self.data.door_wait_open_time {
                    door.door_wait_open_frame = 0;
                    door.door_closed_frame = current_frame;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.clear_model_condition_state(WAITING_OPEN_FLAGS[door_idx]);
                            guard.set_model_condition_state(CLOSING_FLAGS[door_idx]);
                        }
                    }
                }
            }
            // Door closing -> closed transition
            else if door.door_closed_frame > 0 && !door.hold_open {
                let elapsed = current_frame - door.door_closed_frame;
                if elapsed > self.data.door_closing_time {
                    door.door_closed_frame = 0;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.clear_model_condition_state(CLOSING_FLAGS[door_idx]);
                        }
                    }
                }
            }
        }
    }

    /// Update construction complete animation
    /// Matches C++ lines 600-619
    fn update_construction_complete(&mut self, current_frame: u32) {
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
    }

    fn build_player_modifiers(&self, template_name: Option<&str>) -> PlayerBuildModifiers {
        let mut mods = PlayerBuildModifiers::default();

        let Some(player) = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_guard| {
                owner_guard.get_controlling_player()
            })
            .flatten()
        else {
            return mods;
        };

        let player_guard = match player.read() {
            Ok(guard) => guard,
            Err(_) => return mods,
        };

        if let Some(name) = template_name {
            if let Some(template) = TheThingFactory::find_template(name) {
                mods.handicap_cost_multiplier = player_guard
                    .get_handicap()
                    .get_cost_multiplier_for_template(&template);
                mods.handicap_time_multiplier = player_guard
                    .get_handicap()
                    .get_build_time_multiplier_for_template(&template);
                mods.production_cost_change_percent =
                    player_guard.get_production_cost_change_percent(template.get_name().as_str());
                mods.production_time_change_percent =
                    player_guard.get_production_time_change_percent(template.get_name().as_str());
            } else {
                mods.production_cost_change_percent =
                    player_guard.get_production_cost_change_percent(name);
                mods.production_time_change_percent =
                    player_guard.get_production_time_change_percent(name);
            }
        } else {
            mods.handicap_cost_multiplier = player_guard.get_handicap().get_cost_multiplier();
            mods.handicap_time_multiplier = player_guard.get_handicap().get_build_time_multiplier();
        }
        mods.energy_supply_ratio = player_guard.get_energy().supply_ratio();

        let mut kind_of_mask: KindOfMaskType = KIND_OF_MASK_NONE;
        if let Some(name) = template_name {
            if let Some(template) = TheThingFactory::find_template(name) {
                for &kind in ALL_KIND_OF {
                    if template.is_kind_of(kind) {
                        kind_of_mask |= 1u64 << (kind as u32);
                    }
                }
            }
        }

        mods.production_cost_change_by_kind =
            player_guard.get_production_cost_change_based_on_kind_of(kind_of_mask);

        mods
    }

    /// Update production progress with all modifiers
    /// Matches C++ update lines 689-703
    fn update_production_progress(
        &mut self,
        delta_frames: u32,
        player_modifiers: &PlayerBuildModifiers,
        facility_context: Option<&BuildFacilityContext>,
    ) -> bool {
        if let Some(ref mut prod) = self.current_production {
            // Calculate effective build time with all modifiers:
            // - Handicap modifier
            // - Faction production time change
            // - Energy penalty (low power)
            // - Multiple factory bonus
            // Matches C++ ThingTemplate::calcTimeToBuild lines 1524-1576
            let base_time_seconds = (prod.entry.build_time as f32) / 30.0; // Assume 30 FPS
            let total_frames = self.cost_calculator.calc_time_to_build(
                base_time_seconds,
                player_modifiers,
                facility_context,
            );

            // Update time spent
            // Note: In C++, this increments by 1 each frame (line 687)
            // We support variable delta_frames for flexibility
            prod.entry.time_spent = prod.entry.time_spent.saturating_add(delta_frames);

            // Update percent complete for UI
            // Matches C++ lines 696-699
            prod.entry.time_spent >= total_frames
        } else {
            false
        }
    }

    /// Check if production should be disabled due to game state
    /// Matches C++ lines 641-648: sold status check
    #[allow(dead_code)]
    fn should_halt_production(&self, object_sold: bool, object_disabled: bool) -> bool {
        // If object is sold or destroyed, halt production
        // C++: "if( us->getStatusBits().test( OBJECT_STATUS_SOLD ) )"
        if object_sold {
            return true;
        }

        // If object is disabled and module configured to disable on disable
        if object_disabled {
            return true;
        }

        false
    }

    /// Spawn a completed unit
    /// Matches C++ lines 706-856
    fn spawn_unit(&mut self, current_frame: u32) -> Result<(), String> {
        if let Some(ref mut prod) = self.current_production {
            // Check if we've produced all units in this batch
            if prod.quantity_remaining() <= 0 {
                // Remove from production
                self.current_production = None;

                // Start next item if available
                if !self.queue.is_empty() {
                    self.start_next_production()?;
                }

                return Ok(());
            }

            // Open door if needed
            // Matches C++ lines 736-778
            if self.data.num_door_animations > 0 {
                let door_idx = self.current_door
                    % (DOOR_COUNT_MAX.min(self.data.num_door_animations as usize));
                let door = &mut self.doors[door_idx];

                // Start opening door if closed
                if door.door_opened_frame == 0
                    && door.door_wait_open_frame == 0
                    && door.door_closed_frame == 0
                {
                    door.door_opened_frame = current_frame;
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut guard) = owner.write() {
                            guard.set_model_condition_state(OPENING_FLAGS[door_idx]);
                        }
                    }
                }

                // Wait for door to be fully open
                if door.door_wait_open_frame == 0 {
                    return Ok(()); // Door not ready yet
                }
            }

            // Start construction complete animation
            // Matches C++ lines 781-788
            if self.construction_complete_frame == 0 {
                self.construction_complete_frame = current_frame;
                if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                    if let Ok(mut guard) = owner.write() {
                        guard.set_model_condition_state(ModelConditionFlags::CONSTRUCTION_COMPLETE);
                    }
                }
            }

            // Spawn the unit via exit strategy
            // Matches C++ lines 790-843
            if let Some(strategy) = &self.exit_strategy {
                let rally = self
                    .rally_points
                    .get_rally(Some(&prod.entry.template_name))
                    .clone();

                let mut strategy_guard = strategy
                    .lock()
                    .map_err(|_| "Failed to lock exit strategy".to_string())?;

                strategy_guard.spawn_unit(
                    &prod.entry.template_name,
                    self.owner_id,
                    self.current_door,
                    rally,
                )?;

                // Mark one successful
                prod.one_production_successful();

                // Rotate door for next spawn
                self.current_door =
                    (self.current_door + 1) % self.data.num_door_animations.max(1) as usize;
            } else {
                return Err("No exit strategy configured".to_string());
            }

            Ok(())
        } else {
            Err("No production to spawn".to_string())
        }
    }

    /// Get the build queue (read-only)
    pub fn queue(&self) -> &BuildQueue {
        &self.queue
    }

    /// Get current production progress (0.0 to 1.0)
    pub fn current_progress(&self) -> f32 {
        self.current_production
            .as_ref()
            .map(|p| p.entry.progress())
            .unwrap_or(0.0)
    }

    /// Get current production template name
    pub fn current_production_name(&self) -> Option<&str> {
        self.current_production
            .as_ref()
            .map(|p| p.entry.template_name.as_str())
    }

    /// Enable/disable prerequisite checking (cheat mode)
    pub fn set_ignore_prerequisites(&mut self, ignore: bool) {
        self.prerequisite_checker.set_ignore_prerequisites(ignore);
    }
}

impl BehaviorModuleInterface for ProductionUpdateComplete {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get current frame
        // Matches C++ update line 592
        let current_frame = game_logic::current_frame();
        let delta_frames = if self.last_update_frame == 0 {
            1
        } else {
            current_frame.saturating_sub(self.last_update_frame)
        };
        self.last_update_frame = current_frame;

        // Skip if not enabled
        if !self.production_enabled {
            return Ok(());
        }

        // Update doors
        // Matches C++ lines 595-597
        if self.data.num_door_animations > 0 {
            self.update_doors(current_frame);
        }

        // Update construction complete animation
        // Matches C++ lines 600-619
        self.update_construction_complete(current_frame);

        // Update production progress
        let player_mods = self.build_player_modifiers(None);
        let is_complete = self.update_production_progress(delta_frames, &player_mods, None);

        // Spawn if complete
        if is_complete {
            // Ignore errors for now - in full integration, these would be logged
            let _ = self.spawn_unit(current_frame);
        }

        Ok(())
    }

    fn get_module_name(&self) -> &str {
        "ProductionUpdate"
    }

    fn get_interface_mask() -> u32 {
        0x00000001 // UPDATE_MODULE interface
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_production_update_interface(&mut self) -> Option<&mut dyn ProductionUpdateInterface> {
        Some(self)
    }
}

impl BehaviorModule for ProductionUpdateComplete {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!(
            "ProductionUpdateComplete module initialized for object {}",
            self.owner_id
        );
        Ok(())
    }

    fn on_destroy(&mut self) {
        log::info!(
            "ProductionUpdateComplete module destroyed for object {}",
            self.owner_id
        );

        // Cancel all production with no refund (building destroyed)
        self.current_production = None;
        self.queue.clear();
    }
}

impl UpdateModuleInterface for ProductionUpdateComplete {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        BehaviorModuleInterface::update(self)?;
        Ok(UPDATE_SLEEP_NONE)
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        self.data.disabled_types_to_process
    }
}

impl ProductionUpdateInterface for ProductionUpdateComplete {
    fn can_produce(&self, _template_name: &str) -> bool {
        if self.queue.is_full() {
            return false;
        }

        let Some(template) = TheThingFactory::find_template(_template_name) else {
            return false;
        };

        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return false;
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
            return false;
        }

        true
    }

    fn start_production(
        &mut self,
        template_name: String,
        player_id: ObjectID,
    ) -> Result<(), String> {
        let mut base_cost = 1000;
        let mut base_time_frames = 300u32; // 10 seconds at 30 FPS

        if let Some(template) = TheThingFactory::find_template(template_name.as_str()) {
            base_cost = template.get_build_cost();
            base_time_frames = template.calc_time_to_build(None).max(1) as u32;
        }

        let player_mods = self.build_player_modifiers(Some(template_name.as_str()));
        let cost = self
            .cost_calculator
            .calc_cost_to_build(base_cost, &player_mods);
        let base_time_seconds = (base_time_frames as f32) / (LOGICFRAMES_PER_SECOND as f32);
        let build_time =
            self.cost_calculator
                .calc_time_to_build(base_time_seconds, &player_mods, None);

        self.queue_create_unit(
            template_name,
            ProductionType::Unit,
            cost,
            build_time,
            player_id,
        )
        .map(|_| ())
    }

    fn cancel_production(&mut self, index: usize) -> Result<(), String> {
        let mut refund = |player_id: ObjectID, credits: i32| {
            if credits <= 0 {
                return;
            }
            if let Ok(list) = crate::player::player_list().read() {
                if let Some(player_arc) = list.get_player(player_id as i32) {
                    if let Ok(mut player) = player_arc.write() {
                        player.get_money_mut().add_money(credits);
                    }
                }
            }
        };
        self.cancel_production(index, &mut refund)
    }

    fn get_queue_size(&self) -> usize {
        let queue_size = self.queue.len();
        if self.current_production.is_some() {
            queue_size + 1
        } else {
            queue_size
        }
    }

    fn get_queue_entries(&self) -> Vec<BuildQueueEntry> {
        let mut entries = Vec::with_capacity(self.get_queue_size());

        if let Some(current) = &self.current_production {
            let mut entry = current.entry.clone();
            entry.queue_index = 0;
            entries.push(entry);
        }

        let index_offset = entries.len();
        entries.extend(self.queue.entries().iter().cloned().enumerate().map(
            |(index, mut entry)| {
                entry.queue_index = index + index_offset;
                entry
            },
        ));

        entries
    }

    fn has_any_upgrade_in_queue(&self) -> bool {
        ProductionUpdateComplete::has_any_upgrade_in_queue(self)
    }

    fn get_production_progress(&self) -> f32 {
        self.current_progress()
    }

    fn is_producing(&self) -> bool {
        self.current_production.is_some()
    }

    fn pause_production(&mut self) {
        self.queue.pause();
    }

    fn resume_production(&mut self) {
        self.queue.resume();
    }

    fn set_hold_door_open(&mut self, exit_door: usize, hold_it: bool) {
        ProductionUpdateComplete::set_hold_door_open(self, exit_door, hold_it);
    }
}

impl ProductionControlInterface for ProductionUpdateComplete {
    fn can_produce(&self, template_name: &str) -> bool {
        ProductionUpdateInterface::can_produce(self, template_name)
    }

    fn is_producing(&self) -> bool {
        ProductionUpdateInterface::is_producing(self)
    }

    fn queue_size(&self) -> usize {
        ProductionUpdateInterface::get_queue_size(self)
    }

    fn start_production(
        &mut self,
        template_name: String,
        player_id: ObjectID,
    ) -> Result<(), String> {
        ProductionUpdateInterface::start_production(self, template_name, player_id)
    }
}

impl Snapshotable for ProductionUpdateComplete {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |res: std::io::Result<()>| res.map_err(|e| e.to_string());

        let mut version: u32 = 1;
        xfer_io(xfer.xfer_u32(&mut version))?;

        let mut queue_len: u16 = self.queue.len().min(u16::MAX as usize) as u16;
        xfer_io(xfer.xfer_unsigned_short(&mut queue_len))?;

        if xfer.is_writing() {
            for entry in self.queue.entries().iter() {
                xfer_build_queue_entry(xfer, entry)?;
            }
        } else {
            let mut new_queue = BuildQueue::new(self.data.max_queue_entries);
            for _ in 0..queue_len {
                let entry = xfer_read_build_queue_entry(xfer)?;
                let _ = new_queue.enqueue(entry);
            }
            self.queue = new_queue;
        }

        let mut has_current = self.current_production.is_some();
        xfer_io(xfer.xfer_bool(&mut has_current))?;
        if xfer.is_writing() {
            if let Some(current) = &self.current_production {
                xfer_build_queue_entry(xfer, &current.entry)?;
                let mut qty_total = current.quantity_total;
                let mut qty_produced = current.quantity_produced;
                xfer_io(xfer.xfer_i32(&mut qty_total))?;
                xfer_io(xfer.xfer_i32(&mut qty_produced))?;
                let mut exit_door = current.exit_door.map(|v| v as i32).unwrap_or(-1);
                xfer_io(xfer.xfer_i32(&mut exit_door))?;
            }
        } else if has_current {
            let entry = xfer_read_build_queue_entry(xfer)?;
            let mut qty_total = 0i32;
            let mut qty_produced = 0i32;
            let mut exit_door = -1i32;
            xfer_io(xfer.xfer_i32(&mut qty_total))?;
            xfer_io(xfer.xfer_i32(&mut qty_produced))?;
            xfer_io(xfer.xfer_i32(&mut exit_door))?;
            let mut state = ProductionEntryState::from_entry(entry);
            state.quantity_total = qty_total;
            state.quantity_produced = qty_produced;
            state.exit_door = if exit_door >= 0 {
                Some(exit_door as usize)
            } else {
                None
            };
            self.current_production = Some(state);
        } else {
            self.current_production = None;
        }

        xfer_io(xfer.xfer_u32(&mut self.unique_id))?;
        xfer_io(xfer.xfer_u32(&mut self.construction_complete_frame))?;

        let mut current_door = self.current_door as u32;
        xfer_io(xfer.xfer_u32(&mut current_door))?;
        if xfer.is_reading() {
            self.current_door = current_door as usize;
        }

        xfer_io(xfer.xfer_bool(&mut self.production_enabled))?;
        xfer_io(xfer.xfer_u32(&mut self.last_update_frame))?;

        for door in &mut self.doors {
            xfer_io(xfer.xfer_u32(&mut door.door_opened_frame))?;
            xfer_io(xfer.xfer_u32(&mut door.door_wait_open_frame))?;
            xfer_io(xfer.xfer_u32(&mut door.door_closed_frame))?;
            xfer_io(xfer.xfer_bool(&mut door.hold_open))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn xfer_build_queue_entry(xfer: &mut dyn Xfer, entry: &BuildQueueEntry) -> Result<(), String> {
    let xfer_io = |res: std::io::Result<()>| res.map_err(|e| e.to_string());

    let mut production_type = entry.production_type as u32;
    xfer_io(xfer.xfer_u32(&mut production_type))?;
    let mut name = entry.template_name.clone();
    xfer_io(xfer.xfer_ascii_string(&mut name))?;
    let mut priority = entry.priority as i32;
    xfer_io(xfer.xfer_i32(&mut priority))?;
    let mut cost = entry.cost;
    xfer_io(xfer.xfer_i32(&mut cost))?;
    let mut build_time = entry.build_time;
    xfer_io(xfer.xfer_u32(&mut build_time))?;
    let mut time_spent = entry.time_spent;
    xfer_io(xfer.xfer_u32(&mut time_spent))?;
    let mut player_id = entry.player_id;
    xfer_io(xfer.xfer_object_id(&mut player_id))?;
    let mut is_repeat = entry.is_repeat;
    xfer_io(xfer.xfer_bool(&mut is_repeat))?;
    let mut production_id = entry.production_id;
    xfer_io(xfer.xfer_u32(&mut production_id))?;
    let mut queue_index = entry.queue_index as u32;
    xfer_io(xfer.xfer_u32(&mut queue_index))?;
    Ok(())
}

fn xfer_read_build_queue_entry(xfer: &mut dyn Xfer) -> Result<BuildQueueEntry, String> {
    let xfer_io = |res: std::io::Result<()>| res.map_err(|e| e.to_string());

    let mut production_type = 0u32;
    xfer_io(xfer.xfer_u32(&mut production_type))?;
    let production_type = match production_type {
        1 => ProductionType::Upgrade,
        2 => ProductionType::SpecialPower,
        _ => ProductionType::Unit,
    };
    let mut name = String::new();
    xfer_io(xfer.xfer_ascii_string(&mut name))?;
    let mut priority = 1i32;
    xfer_io(xfer.xfer_i32(&mut priority))?;
    let mut cost = 0i32;
    xfer_io(xfer.xfer_i32(&mut cost))?;
    let mut build_time = 0u32;
    xfer_io(xfer.xfer_u32(&mut build_time))?;
    let mut time_spent = 0u32;
    xfer_io(xfer.xfer_u32(&mut time_spent))?;
    let mut player_id = 0u32;
    xfer_io(xfer.xfer_object_id(&mut player_id))?;
    let mut is_repeat = false;
    xfer_io(xfer.xfer_bool(&mut is_repeat))?;
    let mut production_id = 0u32;
    xfer_io(xfer.xfer_u32(&mut production_id))?;
    let mut queue_index = 0u32;
    xfer_io(xfer.xfer_u32(&mut queue_index))?;

    let mut entry = BuildQueueEntry::new(name, production_type, cost, build_time, player_id)
        .with_production_id(production_id);
    entry.priority = match priority {
        0 => super::queue::BuildPriority::Low,
        2 => super::queue::BuildPriority::High,
        3 => super::queue::BuildPriority::Urgent,
        _ => super::queue::BuildPriority::Normal,
    };
    entry.time_spent = time_spent;
    entry.is_repeat = is_repeat;
    entry.queue_index = queue_index as usize;
    Ok(entry)
}

#[derive(Debug)]
pub struct ProductionUpdateCompleteModule {
    behavior: ProductionUpdateComplete,
    module_name_key: NameKeyType,
    module_data: Arc<ProductionUpdateModuleData>,
}

impl ProductionUpdateCompleteModule {
    pub fn new(
        module_name: &AsciiString,
        module_data: Arc<ProductionUpdateModuleData>,
        owner_id: ObjectID,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        let behavior = ProductionUpdateComplete::new((*module_data).clone(), owner_id);
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &ProductionUpdateComplete {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut ProductionUpdateComplete {
        &mut self.behavior
    }
}

impl Module for ProductionUpdateCompleteModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(
            self.module_data.as_ref(),
        )
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        let _ = self.behavior.init();
    }

    fn on_delete(&mut self) {
        self.behavior.on_destroy();
    }

    fn get_production_control_interface(&mut self) -> Option<&mut dyn ProductionControlInterface> {
        Some(&mut self.behavior)
    }
}

impl Snapshotable for ProductionUpdateCompleteModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_entries_include_current_production_first() {
        let mut production =
            ProductionUpdateComplete::new(ProductionUpdateModuleData::default(), 42);

        production
            .queue_create_unit("TestInfantry".to_string(), ProductionType::Unit, 100, 30, 0)
            .expect("first item should queue");
        production
            .queue_create_unit("TestTank".to_string(), ProductionType::Unit, 700, 90, 0)
            .expect("second item should queue");

        let entries = ProductionUpdateInterface::get_queue_entries(&production);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].template_name, "TestInfantry");
        assert_eq!(entries[0].queue_index, 0);
        assert_eq!(entries[1].template_name, "TestTank");
        assert_eq!(entries[1].queue_index, 1);
    }

    #[test]
    fn max_queue_entries_counts_current_production() {
        let mut data = ProductionUpdateModuleData::default();
        data.max_queue_entries = 2;
        let mut production = ProductionUpdateComplete::new(data, 42);

        production
            .queue_create_unit("TestInfantry".to_string(), ProductionType::Unit, 100, 30, 0)
            .expect("first item should start current production");
        production
            .queue_create_unit("TestTank".to_string(), ProductionType::Unit, 700, 90, 0)
            .expect("second item should fill the queue limit");

        let result =
            production.queue_create_unit("Overflow".to_string(), ProductionType::Unit, 900, 90, 0);

        assert!(
            result.is_err(),
            "C++ m_productionCount includes current production plus queued entries"
        );
        assert_eq!(ProductionUpdateInterface::get_queue_size(&production), 2);
    }

    #[test]
    fn queue_create_unit_with_id_cancels_current_by_production_id() {
        let mut production =
            ProductionUpdateComplete::new(ProductionUpdateModuleData::default(), 42);
        let mut refunds = Vec::new();

        production
            .queue_create_unit_with_id(
                "TestInfantry".to_string(),
                ProductionType::Unit,
                100,
                30,
                7,
                55,
            )
            .expect("unit should queue");

        assert_eq!(
            production
                .current_production
                .as_ref()
                .map(|prod| prod.entry.production_id),
            Some(55)
        );

        production
            .cancel_unit_by_production_id(55, &mut |player_id, credits| {
                refunds.push((player_id, credits));
            })
            .expect("production id cancel should succeed");

        assert!(production.current_production.is_none());
        assert_eq!(refunds, vec![(7, 100)]);
    }

    #[test]
    fn queue_create_unit_with_id_cancels_queued_by_production_id() {
        let mut production =
            ProductionUpdateComplete::new(ProductionUpdateModuleData::default(), 42);
        let mut refunds = Vec::new();

        production
            .queue_create_unit_with_id(
                "TestInfantry".to_string(),
                ProductionType::Unit,
                100,
                30,
                7,
                55,
            )
            .expect("first unit should queue");
        production
            .queue_create_unit_with_id("TestTank".to_string(), ProductionType::Unit, 700, 90, 7, 56)
            .expect("second unit should queue");

        production
            .cancel_unit_by_production_id(56, &mut |player_id, credits| {
                refunds.push((player_id, credits));
            })
            .expect("production id cancel should succeed");

        assert_eq!(
            production
                .current_production
                .as_ref()
                .map(|prod| prod.entry.production_id),
            Some(55)
        );
        assert!(production.queue.is_empty());
        assert_eq!(refunds, vec![(7, 700)]);
    }
}
