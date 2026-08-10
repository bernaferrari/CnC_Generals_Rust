// SupplyTruck, Worker, and Dozer AI interfaces
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

pub trait SupplyTruckAIInterface: Send + Sync {
    /// Get supplies count
    fn get_supplies_count(&self) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(0)
    }
    /// Number of boxes currently carried (matches C++ getNumberBoxes).
    fn get_number_boxes(&self) -> i32 {
        self.get_supplies_count().unwrap_or(0)
    }

    /// Dock action delay (matches C++ SupplyTruckAIInterface::getActionDelayForDock)
    fn get_action_delay_for_dock(
        &self,
        _dock_id: ObjectID,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(0)
    }

    /// Force the supply truck to seek supplies immediately (SupplyCenter exit behavior).
    fn set_force_wanting_state(&mut self, enabled: bool) {
        let _ = enabled;
    }
    /// Query force wanting latch (matches C++ isForcedIntoWantingState).
    fn is_forced_into_wanting_state(&self) -> bool {
        false
    }

    /// Force the supply truck into busy state (stop command).
    fn set_force_busy_state(&mut self, enabled: bool) {
        let _ = enabled;
    }
    /// Query force busy latch.
    fn is_forced_into_busy_state(&self) -> bool {
        false
    }

    /// Preferred dock override (matches C++ SupplyTruckAIInterface::getPreferredDockID).
    fn get_preferred_dock_id(&self) -> Option<ObjectID> {
        None
    }

    /// Warehouse scan distance override (matches C++ SupplyTruckAIInterface::getWarehouseScanDistance).
    fn get_warehouse_scan_distance(&self, _is_ai_player: bool) -> Option<Real> {
        None
    }

    /// Check whether the truck is currently available for supplying.
    fn is_available_for_supplying(&self) -> bool {
        true
    }

    /// Check whether the truck is ferrying supplies (matches C++ isCurrentlyFerryingSupplies).
    fn is_currently_ferrying_supplies(&self) -> bool {
        false
    }

    /// Lose one supply box (delivery).
    fn lose_one_box(&mut self) -> bool {
        false
    }

    /// Gain one supply box (collection).
    fn gain_one_box(&mut self, _remaining_stock: i32) -> bool {
        false
    }

    /// Supply boost from upgrades.
    fn get_upgraded_supply_boost(&self) -> u32;
}

/// Worker AI update interface (build/repair tasks).
pub trait WorkerAIUpdateInterface: Send + Sync {
    /// Assign a build task for a newly created construction site.
    fn set_build_task(
        &mut self,
        _building_id: ObjectID,
        _total_build_frames: u32,
        _max_health: f32,
        _is_rebuild: bool,
    ) {
    }
}

/// Dozer AI update interface (build tasks).
pub trait DozerAIUpdateInterface: Send + Sync {
    /// Assign a build task for a newly created construction site.
    fn set_build_task(
        &mut self,
        _building_id: ObjectID,
        _total_build_frames: u32,
        _max_health: f32,
        _is_rebuild: bool,
    ) {
    }

    /// Cancel an active or pending dozer task.
    fn cancel_task(&mut self, _task: crate::object::update::ai_update::dozer_ai_update::DozerTask) {
    }

    /// C++ DozerAIInterface::isTaskPending
    fn is_task_pending(
        &self,
        _task: crate::object::update::ai_update::dozer_ai_update::DozerTask,
    ) -> bool {
        false
    }

    /// C++ DozerAIInterface::isAnyTaskPending
    fn is_any_task_pending(&self) -> bool {
        false
    }
}
