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

    fn owner_id(&self) -> ObjectID {
        self.object_id
    }

    fn update_drawable_supply_status(&self) {
        // Wave 298: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.object_id == INVALID_ID {
            return;
        }
        let Some(drawable) = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.object_id, |owner_guard| owner_guard.get_drawable())
            .flatten()
        else {
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
            if self.object_id != INVALID_ID {
                self.state_machine = Some(SupplyTruckStateMachine::new(self.object_id));
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
        // Wave 298: empty dual-world only skips drawable status, not the box
        // increment or SuppliesDepletedVoice (C++ gainOneBox.cpp:132-171).

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
                    TheGameLogic::find_object_by_id(self.object_id).or_else(|| {
                        crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id)
                    }),
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
        dock_id: ObjectID,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        // Wave 298: empty dual-world → Ok(0).
        if dual_world_registry_unavailable() {
            return Ok(0);
        }

        let Some(dock) = crate::helpers::TheGameLogic::find_object_by_id(dock_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(dock_id))
        else {
            return Ok(0);
        };
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
        dock_id: ObjectID,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        // Wave 298: empty dual-world → Ok(0).
        if dual_world_registry_unavailable() {
            return Ok(0);
        }

        let Some(dock) = crate::helpers::TheGameLogic::find_object_by_id(dock_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(dock_id))
        else {
            return Ok(0);
        };
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

