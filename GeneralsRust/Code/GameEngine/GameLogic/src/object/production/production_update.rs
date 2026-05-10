//! Production update module
//!
//! Main production system for buildings that produce units.
//! Manages the build queue, cost deduction, and unit spawning.

use super::build_cost_calculator::{
    BuildCostCalculator, BuildFacilityContext, PlayerBuildModifiers,
};
use super::exit_strategies::ProductionExitStrategy;
use super::prerequisite_checker::{
    CanMakeType, PlayerBuildState, Prerequisite, PrerequisiteChecker,
};
use super::queue::{BuildQueue, BuildQueueEntry, ProductionType};
use super::rally_point::RallyPointManager;
use crate::common::*;
use crate::economy::{EconomyManager, IncomeSource};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{
    BehaviorModule, BehaviorModuleInterface, ProductionUpdateInterface, UpdateModuleInterface,
    UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::Object;
use crate::player::{player_list, PlayerIndex};
use crate::system::game_logic;
use std::sync::{Arc, Mutex, RwLock};

/// Module data for ProductionUpdate (configuration)
#[derive(Debug, Clone)]
pub struct ProductionUpdateData {
    /// Maximum queue size (0 = unlimited)
    pub max_queue_size: usize,
    /// Base production speed modifier (1.0 = normal)
    pub production_modifier: f32,
    /// Number of exit doors/spawns
    pub num_doors: usize,
    /// Door open time in frames
    pub door_open_time: u32,
    /// Door wait open time (keeps door open)
    pub door_wait_open_time: u32,
    /// Door wait time (before opening)
    pub door_wait_time: u32,
    /// Whether the facility gives units for free (super weapon)
    pub give_no_exp: bool,
    /// Whether to disable production when disabled
    pub disable_production_toggle: bool,
    /// Production animation name
    pub production_anim: Option<String>,
    /// Build complete sound
    pub complete_sound: Option<String>,
}

impl Default for ProductionUpdateData {
    fn default() -> Self {
        Self {
            max_queue_size: 0, // Unlimited
            production_modifier: 1.0,
            num_doors: 1,
            door_open_time: 15,      // ~0.5 seconds at 30 FPS
            door_wait_open_time: 30, // ~1 second
            door_wait_time: 0,
            give_no_exp: false,
            disable_production_toggle: false,
            production_anim: None,
            complete_sound: None,
        }
    }
}

/// Production state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionState {
    /// Idle, nothing in queue
    Idle,
    /// Actively producing
    Producing,
    /// Waiting for door to open
    WaitingForDoor,
    /// Spawning the unit
    Spawning,
    /// Production paused/disabled
    Paused,
}

/// Main production update module
#[derive(Debug)]
pub struct ProductionUpdate {
    /// Module configuration
    data: ProductionUpdateData,
    /// Build queue
    queue: BuildQueue,
    /// Rally point manager
    rally_points: RallyPointManager,
    /// Current production state
    state: ProductionState,
    /// Door timer (for animations)
    door_timer: u32,
    /// Currently active door index
    current_door: usize,
    /// Track which doors are held open
    door_hold_open: Vec<bool>,
    /// Reference to the owning object
    owner_id: ObjectID,
    /// Exit strategy for spawned units
    exit_strategy: Option<Arc<Mutex<dyn ProductionExitStrategy>>>,
    /// Whether production is currently enabled
    production_enabled: bool,
    /// Last frame production was updated
    last_update_frame: u32,
    /// Build cost calculator (matches C++ calcCostToBuild/calcTimeToBuild)
    cost_calculator: BuildCostCalculator,
    /// Prerequisite checker
    prerequisite_checker: PrerequisiteChecker,
    /// Economy manager reference (for money deduction)
    /// Matches C++ ProductionUpdate.cpp lines 281-282, 405-406
    economy_manager: Option<Arc<Mutex<EconomyManager>>>,
}

impl ProductionUpdate {
    /// Create a new production update module
    pub fn new(data: ProductionUpdateData, owner_id: ObjectID) -> Self {
        let queue = BuildQueue::new(data.max_queue_size);
        let num_doors = data.num_doors.max(1);

        Self {
            data,
            queue,
            rally_points: RallyPointManager::new(),
            state: ProductionState::Idle,
            door_timer: 0,
            current_door: 0,
            door_hold_open: vec![false; num_doors],
            owner_id,
            exit_strategy: None,
            production_enabled: true,
            last_update_frame: 0,
            cost_calculator: BuildCostCalculator::new(),
            prerequisite_checker: PrerequisiteChecker::new(),
            economy_manager: None,
        }
    }

    /// Set the economy manager reference
    /// Must be called after construction to enable cost deduction
    /// Matches C++ pattern of injecting dependencies
    fn build_player_modifiers_for_template(
        &self,
        player_id: ObjectID,
        template_name: &str,
    ) -> PlayerBuildModifiers {
        let mut mods = PlayerBuildModifiers::default();

        let player_guard = match player_list().read() {
            Ok(list) => list.get_player(player_id as PlayerIndex).cloned(),
            Err(_) => None,
        };

        let Some(player_arc) = player_guard else {
            return mods;
        };

        let player = match player_arc.read() {
            Ok(guard) => guard,
            Err(_) => return mods,
        };

        if let Some(template) = TheThingFactory::find_template(template_name) {
            mods.handicap_cost_multiplier = player
                .get_handicap()
                .get_cost_multiplier_for_template(&template);
            mods.handicap_time_multiplier = player
                .get_handicap()
                .get_build_time_multiplier_for_template(&template);
        } else {
            mods.handicap_cost_multiplier = player.get_handicap().get_cost_multiplier();
            mods.handicap_time_multiplier = player.get_handicap().get_build_time_multiplier();
        }
        mods.energy_supply_ratio = player.get_energy().supply_ratio();

        let mut kind_of_mask: KindOfMaskType = KIND_OF_MASK_NONE;
        if let Some(template) = TheThingFactory::find_template(template_name) {
            for &kind in ALL_KIND_OF {
                if template.is_kind_of(kind) {
                    kind_of_mask |= 1u64 << (kind as u32);
                }
            }
        }

        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(kind_of_mask);

        mods
    }

    pub fn set_economy_manager(&mut self, economy: Arc<Mutex<EconomyManager>>) {
        self.economy_manager = Some(economy);
    }

    /// Set the exit strategy
    pub fn set_exit_strategy(&mut self, strategy: Arc<Mutex<dyn ProductionExitStrategy>>) {
        self.exit_strategy = Some(strategy);
    }

    /// Add an item to the production queue with full C++ compatibility
    /// Matches C++ ProductionUpdate::queueCreateUnit lines 363-437
    pub fn enqueue_production(
        &mut self,
        template_name: String,
        production_type: ProductionType,
        cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> Result<(), String> {
        // Check if production is enabled
        if !self.production_enabled && self.data.disable_production_toggle {
            return Err("Production is currently disabled".to_string());
        }

        // Check queue capacity (matches C++ line 397-401)
        if self.queue.is_full() {
            return Err(format!(
                "Production queue is full (max: {})",
                self.data.max_queue_size
            ));
        }

        // Deduct cost from player (matches C++ lines 403-406)
        if let Some(economy) = &self.economy_manager {
            let mut economy_guard = economy
                .lock()
                .map_err(|_| "Failed to lock economy manager".to_string())?;

            let player = PlayerId::new(player_id as u8)
                .ok_or_else(|| format!("Invalid player ID: {}", player_id))?;

            // Withdraw money (matches C++ Money::withdraw)
            economy_guard
                .add_credits(
                    player,
                    -cost,
                    IncomeSource::Unknown,
                    Some(format!("Production cost: {}", template_name)),
                )
                .map_err(|e| format!("Failed to deduct cost: {}", e))?;

            log::debug!(
                "Deducted {} credits from player {} for {}",
                cost,
                player_id,
                template_name
            );
        } else {
            log::warn!("No economy manager - cost not deducted for production");
        }

        // Create queue entry
        let entry =
            BuildQueueEntry::new(template_name, production_type, cost, build_time, player_id);

        // Add to queue (matches C++ line 434)
        self.queue.enqueue(entry)?;

        // If we were idle, start producing
        if self.state == ProductionState::Idle {
            self.start_production()?;
        }

        Ok(())
    }

    /// Cancel a queue entry with refund
    /// Matches C++ ProductionUpdate::cancelUnitCreate lines 443-472
    pub fn cancel_production(&mut self, index: usize) -> Result<(), String> {
        if let Some(entry) = self.queue.cancel(index) {
            // C++ refunds the full calcCostToBuild for the canceled production entry.
            let refund = entry.cost;

            // Return money to player (matches C++ lines 456-458)
            if refund > 0 {
                if let Some(economy) = &self.economy_manager {
                    let mut economy_guard = economy
                        .lock()
                        .map_err(|_| "Failed to lock economy manager".to_string())?;

                    let player_id = PlayerId::new(entry.player_id as u8)
                        .ok_or_else(|| format!("Invalid player id {}", entry.player_id))?;

                    economy_guard
                        .add_credits(player_id, refund, IncomeSource::Refund, None)
                        .map_err(|err| err.to_string())?;

                    log::debug!(
                        "Refunded {} credits to player {} for cancelled {}",
                        refund,
                        entry.player_id,
                        entry.template_name
                    );
                } else {
                    log::warn!("No economy manager - refund not processed");
                }
            }

            // If we cancelled the current item, start next
            if index == 0 && !self.queue.is_empty() {
                self.start_production()?;
            } else if self.queue.is_empty() {
                self.state = ProductionState::Idle;
            }

            Ok(())
        } else {
            Err("Invalid queue index".to_string())
        }
    }

    pub fn cancel_upgrade_by_name(&mut self, upgrade_name: &str) -> Result<(), String> {
        let Some(index) = self
            .queue
            .find_by_template_and_type(ProductionType::Upgrade, upgrade_name)
        else {
            return Err("Upgrade not in queue".to_string());
        };

        self.cancel_production(index)
    }

    /// Start producing the current queue item
    fn start_production(&mut self) -> Result<(), String> {
        if let Some(_entry) = self.queue.current() {
            self.state = ProductionState::Producing;

            // Play production animation if configured
            if let Some(anim) = &self.data.production_anim {
                // Trigger animation on owner object
                // Matches C++ ProductionUpdate setting MODELCONDITION flags
                // In full implementation, would:
                // 1. Get owner object's Drawable module
                // 2. Set model condition flag for production (e.g., MODELCONDITION_ACTIVELY_CONSTRUCTING)
                // 3. Drawable triggers the animation specified in INI
                log::debug!("Starting production animation: {}", anim);
                // Would call: owner.drawable().setModelConditionFlags(MODELCONDITION_ACTIVELY_CONSTRUCTING)
            }

            Ok(())
        } else {
            self.state = ProductionState::Idle;
            Err("No items in queue to produce".to_string())
        }
    }

    /// Update production progress
    fn update_production(&mut self, delta_frames: u32) -> Result<bool, String> {
        // Apply production modifier
        let effective_frames = ((delta_frames as f32) * self.data.production_modifier) as u32;

        // Update current queue item
        if !self.queue.update_current(effective_frames) {
            return Ok(false);
        }

        let is_complete = self
            .queue
            .current()
            .is_some_and(|entry| entry.is_complete());

        if is_complete {
            self.state = ProductionState::WaitingForDoor;
            self.door_timer = self.data.door_wait_time;
        }

        Ok(is_complete)
    }

    /// Handle door opening animation
    fn update_door(&mut self, delta_frames: u32) -> Result<bool, String> {
        if self
            .door_hold_open
            .get(self.current_door)
            .copied()
            .unwrap_or(false)
        {
            self.state = ProductionState::Spawning;
            return Ok(true);
        }
        if self.door_timer > 0 {
            self.door_timer = self.door_timer.saturating_sub(delta_frames);
            Ok(false)
        } else {
            // Door is open, ready to spawn
            self.state = ProductionState::Spawning;
            Ok(true)
        }
    }

    /// Spawn the completed unit
    fn spawn_unit(&mut self) -> Result<(), String> {
        if let Some(entry) = self.queue.current().cloned() {
            // Play completion sound
            // Matches C++ ProductionUpdate sound playing on unit complete
            if let Some(sound) = &self.data.complete_sound {
                log::debug!("Playing completion sound: {}", sound);
                // Play sound via audio system
                // In full implementation, would:
                // 1. Get sound template from TheAudioManager
                // 2. Play sound at building position
                // 3. Announce to player via EVA system if configured
                // Would call: TheAudioManager->playSound(sound, owner_position)
            }

            // Get rally point for this unit type
            let rally = self.rally_points.get_rally(Some(&entry.template_name));

            // Spawn the unit using exit strategy
            if let Some(strategy) = &self.exit_strategy {
                let mut strategy_guard = strategy
                    .lock()
                    .map_err(|_| "Failed to lock exit strategy".to_string())?;

                strategy_guard.spawn_unit(
                    &entry.template_name,
                    self.owner_id,
                    self.current_door,
                    rally.clone(),
                )?;
            } else {
                return Err("No exit strategy configured".to_string());
            }

            // Only dequeue after successful spawn so failed exits keep the item queued.
            let _ = self.queue.dequeue();

            // Rotate door index for next spawn
            self.current_door = (self.current_door + 1) % self.data.num_doors;

            // Start next item if queue has more
            if !self.queue.is_empty() {
                self.start_production()?;
            } else {
                self.state = ProductionState::Idle;
            }

            Ok(())
        } else {
            Err("No completed unit to spawn".to_string())
        }
    }

    /// Get the build queue (read-only)
    pub fn queue(&self) -> &BuildQueue {
        &self.queue
    }

    /// Get the rally point manager
    pub fn rally_points(&self) -> &RallyPointManager {
        &self.rally_points
    }

    /// Get mutable rally point manager
    pub fn rally_points_mut(&mut self) -> &mut RallyPointManager {
        &mut self.rally_points
    }

    /// Pause production
    pub fn pause(&mut self) {
        self.queue.pause();
        self.state = ProductionState::Paused;
    }

    /// Resume production
    pub fn resume(&mut self) {
        self.queue.resume();
        if !self.queue.is_empty() {
            self.state = ProductionState::Producing;
        } else {
            self.state = ProductionState::Idle;
        }
    }

    /// Enable/disable production
    pub fn set_production_enabled(&mut self, enabled: bool) {
        self.production_enabled = enabled;
        if !enabled {
            self.pause();
        }
    }

    /// Check if production is enabled
    pub fn is_production_enabled(&self) -> bool {
        self.production_enabled
    }

    /// Get current production state
    pub fn state(&self) -> ProductionState {
        self.state
    }

    /// Get production progress (0.0 to 1.0)
    pub fn current_progress(&self) -> f32 {
        self.queue
            .current()
            .map(|entry| entry.progress())
            .unwrap_or(0.0)
    }

    /// Get current production template name
    pub fn current_production(&self) -> Option<&str> {
        self.queue
            .current()
            .map(|entry| entry.template_name.as_str())
    }

    /// Get template data (cost and build time) for a unit
    /// Matches C++ ThingTemplate::calcCostToBuild and calcTimeToBuild
    fn get_template_data(&self, template_name: &str) -> (i32, u32) {
        if let Some(template) = TheThingFactory::find_template(template_name) {
            let cost = template.get_build_cost();
            let time = template.calc_time_to_build(None).max(1) as u32;
            return (cost, time);
        }

        log::warn!(
            "Unknown template '{}', using default cost/time",
            template_name
        );
        (1000, 300)
    }

    /// Calculate actual build cost with player modifiers
    /// Matches C++ ThingTemplate::calcCostToBuild
    pub fn calculate_build_cost(
        &self,
        base_cost: i32,
        player_modifiers: &PlayerBuildModifiers,
    ) -> i32 {
        self.cost_calculator
            .calc_cost_to_build(base_cost, player_modifiers)
    }

    /// Calculate actual build time with player modifiers
    /// Matches C++ ThingTemplate::calcTimeToBuild
    pub fn calculate_build_time(
        &self,
        base_time_seconds: f32,
        player_modifiers: &PlayerBuildModifiers,
        facility_context: Option<&BuildFacilityContext>,
    ) -> u32 {
        self.cost_calculator.calc_time_to_build(
            base_time_seconds,
            player_modifiers,
            facility_context,
        )
    }

    /// Check if player can queue a unit (with prerequisites)
    /// Matches C++ ProductionUpdate::canQueueCreateUnit and BuildAssistant::canMakeUnit
    pub fn can_queue_unit(
        &self,
        template_name: &str,
        player_state: &PlayerBuildState,
        prerequisites: &[Prerequisite],
    ) -> CanMakeType {
        // Check queue full
        if self.queue.is_full() {
            return CanMakeType::QueueFull;
        }

        // Get cost
        let (cost, _) = self.get_template_data(template_name);

        // Check prerequisites and funds
        self.prerequisite_checker.can_make_unit(
            template_name,
            cost,
            prerequisites,
            false, // is_unique
            player_state,
        )
    }

    /// Check if any upgrade is queued or currently producing.
    pub fn has_any_upgrade_in_queue(&self) -> bool {
        if let Some(current) = self.queue.current() {
            if current.production_type == ProductionType::Upgrade {
                return true;
            }
        }

        self.queue.contains_type(ProductionType::Upgrade)
    }
}

impl BehaviorModuleInterface for ProductionUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get current frame from the global GameLogic singleton to mirror C++ scheduling.
        let current_frame = game_logic::current_frame();
        let delta_frames = if self.last_update_frame == 0 {
            1
        } else {
            current_frame.saturating_sub(self.last_update_frame)
        };
        self.last_update_frame = current_frame;

        // Skip if paused or disabled
        if self.state == ProductionState::Paused || !self.production_enabled {
            return Ok(());
        }

        // State machine update
        match self.state {
            ProductionState::Idle => {
                // Check if we have queued items
                if !self.queue.is_empty() {
                    self.start_production()?;
                }
            }
            ProductionState::Producing => {
                self.update_production(delta_frames)?;
            }
            ProductionState::WaitingForDoor => {
                self.update_door(delta_frames)?;
            }
            ProductionState::Spawning => {
                // Retry spawning until exit succeeds, matching C++ behavior where
                // completed production stays pending if it cannot exit this frame.
                if let Err(err) = self.spawn_unit() {
                    log::warn!(
                        "ProductionUpdate failed to spawn pending unit for object {}: {}",
                        self.owner_id,
                        err
                    );
                    self.state = ProductionState::WaitingForDoor;
                    self.door_timer = self.data.door_wait_time.max(1);
                }
            }
            ProductionState::Paused => {
                // Do nothing while paused
            }
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

impl BehaviorModule for ProductionUpdate {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!(
            "ProductionUpdate module initialized for object {}",
            self.owner_id
        );
        Ok(())
    }

    fn on_destroy(&mut self) {
        log::info!(
            "ProductionUpdate module destroyed for object {}",
            self.owner_id
        );
        self.queue.clear();
    }
}

impl UpdateModuleInterface for ProductionUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        BehaviorModuleInterface::update(self)?;
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl ProductionUpdateInterface for ProductionUpdate {
    fn can_produce(&self, template_name: &str) -> bool {
        if self.queue.is_full() {
            return false;
        }

        let Some(template) = TheThingFactory::find_template(template_name) else {
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

        log::debug!("Checking if can produce: {}", template_name);
        true
    }

    fn start_production(
        &mut self,
        template_name: String,
        player_id: ObjectID,
    ) -> Result<(), String> {
        // Get cost and build time from template
        let (mut cost, mut build_time_frames) = self.get_template_data(&template_name);

        if let Some(template) = TheThingFactory::find_template(template_name.as_str()) {
            cost = template.get_build_cost();
            build_time_frames = template.calc_time_to_build(None).max(1) as u32;
        }

        let player_mods =
            self.build_player_modifiers_for_template(player_id, template_name.as_str());
        let cost = self.cost_calculator.calc_cost_to_build(cost, &player_mods);
        let base_time_seconds = (build_time_frames as f32) / (LOGICFRAMES_PER_SECOND as f32);
        let build_time =
            self.cost_calculator
                .calc_time_to_build(base_time_seconds, &player_mods, None);

        self.enqueue_production(
            template_name,
            ProductionType::Unit,
            cost,
            build_time,
            player_id,
        )
    }

    fn cancel_production(&mut self, index: usize) -> Result<(), String> {
        // Delegate to the updated cancel_production method
        ProductionUpdate::cancel_production(self, index)
    }

    fn get_queue_size(&self) -> usize {
        self.queue.len()
    }

    fn get_queue_entries(&self) -> Vec<BuildQueueEntry> {
        self.queue.entries().iter().cloned().collect()
    }

    fn get_production_progress(&self) -> f32 {
        self.current_progress()
    }

    fn is_producing(&self) -> bool {
        matches!(
            self.state,
            ProductionState::Producing
                | ProductionState::WaitingForDoor
                | ProductionState::Spawning
        )
    }

    fn pause_production(&mut self) {
        self.pause();
    }

    fn resume_production(&mut self) {
        self.resume();
    }

    fn set_hold_door_open(&mut self, exit_door: usize, hold_it: bool) {
        if exit_door < self.door_hold_open.len() {
            self.door_hold_open[exit_door] = hold_it;
            if hold_it
                && exit_door == self.current_door
                && self.state == ProductionState::WaitingForDoor
            {
                // C++ parity intent: a held-open door should be immediately usable.
                self.door_timer = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_creation() {
        let data = ProductionUpdateData::default();
        let production = ProductionUpdate::new(data, 1);

        assert_eq!(production.state(), ProductionState::Idle);
        assert_eq!(production.queue().len(), 0);
        assert!(production.is_production_enabled());
    }

    #[test]
    fn test_enqueue_production() {
        let data = ProductionUpdateData::default();
        let mut production = ProductionUpdate::new(data, 1);

        let result =
            production.enqueue_production("Tank".to_string(), ProductionType::Unit, 1000, 300, 1);

        assert!(result.is_ok());
        assert_eq!(production.queue().len(), 1);
        assert_eq!(production.state(), ProductionState::Producing);
    }

    #[test]
    fn test_production_disabled() {
        let mut data = ProductionUpdateData::default();
        data.disable_production_toggle = true;
        let mut production = ProductionUpdate::new(data, 1);

        production.set_production_enabled(false);

        let result =
            production.enqueue_production("Tank".to_string(), ProductionType::Unit, 1000, 300, 1);

        assert!(result.is_err());
    }

    #[test]
    fn test_pause_resume() {
        let data = ProductionUpdateData::default();
        let mut production = ProductionUpdate::new(data, 1);

        production
            .enqueue_production("Tank".to_string(), ProductionType::Unit, 1000, 300, 1)
            .unwrap();

        assert_eq!(production.state(), ProductionState::Producing);

        production.pause();
        assert_eq!(production.state(), ProductionState::Paused);
        assert!(production.queue().is_paused());

        production.resume();
        assert_eq!(production.state(), ProductionState::Producing);
        assert!(!production.queue().is_paused());
    }

    #[test]
    fn test_queue_limit() {
        let mut data = ProductionUpdateData::default();
        data.max_queue_size = 2;
        let mut production = ProductionUpdate::new(data, 1);

        // First two should succeed
        assert!(production
            .enqueue_production("Tank1".to_string(), ProductionType::Unit, 1000, 300, 1,)
            .is_ok());

        assert!(production
            .enqueue_production("Tank2".to_string(), ProductionType::Unit, 1000, 300, 1,)
            .is_ok());

        // Third should fail
        assert!(production
            .enqueue_production("Tank3".to_string(), ProductionType::Unit, 1000, 300, 1,)
            .is_err());
    }

    #[test]
    fn cancel_production_refunds_full_cost_after_progress() {
        use crate::economy::{EconomyManager, ResourceType};
        use std::collections::HashMap;

        let economy = Arc::new(Mutex::new(EconomyManager::new()));
        {
            let mut economy_guard = economy.lock().expect("economy lock should be available");
            let mut resources = HashMap::new();
            resources.insert(ResourceType::Money, 2_000);
            economy_guard
                .initialize_player_economy(1, resources)
                .expect("player economy should initialize");
        }

        let mut production = ProductionUpdate::new(ProductionUpdateData::default(), 1);
        production.set_economy_manager(economy.clone());
        production
            .enqueue_production("Tank".to_string(), ProductionType::Unit, 1_000, 100, 1)
            .expect("production should enqueue");
        production
            .queue
            .current_mut()
            .expect("queued production should exist")
            .time_spent = 75;

        production
            .cancel_production(0)
            .expect("production cancel should succeed");

        let money = economy
            .lock()
            .expect("economy lock should be available")
            .get_player_resources(1)
            .expect("player resources should exist")
            .get(&ResourceType::Money)
            .copied()
            .unwrap_or_default();
        assert_eq!(
            money, 2_000,
            "C++ ProductionUpdate::cancelUnitCreate refunds full queued cost"
        );
    }
}
