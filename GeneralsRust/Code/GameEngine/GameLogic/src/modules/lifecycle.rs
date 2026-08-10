// Update/damage/create/die/dock/production interfaces
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Matches C++ FAST_AS_POSSIBLE constant in AIUpdate.h.
pub const FAST_AS_POSSIBLE: Real = 999_999.0;

/// Update module trait for per-frame updates
pub trait UpdateModule: Send + Sync + std::fmt::Debug {
    fn update(&mut self, object_id: ObjectID, delta_time: Real);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
}

/// Damage module interface
pub trait DamageModule: Send + Sync {
    fn process_damage(&mut self, object_id: ObjectID, damage: &DamageInfo) -> Real;
}

/// Upgrade module interface
pub trait UpgradeModuleInterface: Send + Sync {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        let _ = upgrade_mask;
        true
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        let _ = upgrade_mask;
        false
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        let _ = upgrade_mask;
    }

    fn force_refresh_upgrade(&mut self) {}

    /// Notify module that its owning object is being deleted.
    fn on_delete(&mut self, object: &mut Object) {
        let _ = object;
    }

    /// Notify module that its owning object was captured by another player.
    fn on_capture(
        &mut self,
        _object: &mut Object,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        _new_owner: Option<&Arc<RwLock<Player>>>,
    ) {
    }
}

/// Collision module interface
pub trait CollideModuleInterface: Send + Sync {
    fn on_collision(&mut self, object_id: ObjectID, other_id: ObjectID);

    /// Railroad collision identification (matches C++ CollideModuleInterface::isRailroad).
    fn is_railroad(&self) -> bool {
        false
    }
}

/// Create module interface for object creation
pub trait CreateModuleInterface: Send + Sync {
    fn on_create(&mut self, object_id: ObjectID);
}

/// Die module interface for object destruction
pub trait DieModuleInterface: Send + Sync {
    fn on_die(
        &mut self,
        damage: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called when the object is explicitly destroyed (mirrors C++ DestroyModuleInterface bridge)
    fn on_destroy(
        &mut self,
        _reason: DestroyReason,
        _object_id: ObjectID,
        _killer: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Optional creator metadata hook used by SpecialPowerCompletionDie.
    fn set_creator(&mut self, creator_id: ObjectID) {
        let _ = creator_id;
    }

    /// Optional script-engine notification hook used by SpecialPowerCompletionDie.
    /// Returns true when this module handled the notification.
    fn notify_script_engine_with_player_index(&self, _player_index: Option<usize>) -> bool {
        false
    }
}

/// Destroy module interface
pub trait DestroyModuleInterface: Send + Sync {
    fn on_destroy(&mut self, object_id: ObjectID);
}

/// Dock update interface for docking behavior
pub trait DockUpdateInterface: Send + Sync {
    /// Check if the dock is open for business
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Supply-warehouse contents, when this dock represents a supply warehouse.
    fn supply_warehouse_boxes_stored(&self) -> Option<i32> {
        None
    }

    /// Set whether the dock is open (matches C++ DockUpdateInterface::setDockOpen).
    fn set_dock_open(&mut self, open: Bool);

    /// Check if it is clear to approach this dock.
    /// Matches C++ DockUpdateInterface::isClearToApproach, defaulting to open state.
    fn is_clear_to_approach(
        &self,
        _obj_id: ObjectID,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.is_dock_open()
    }

    /// Cancel dock operation for an object
    fn cancel_dock(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Reserve an approach position in the queue
    fn reserve_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Advance to the next approach position
    fn advance_approach_position(
        &mut self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Check if clear to advance in queue
    fn is_clear_to_advance(
        &self,
        obj_id: ObjectID,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Called when approach position reached
    fn on_approach_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Check if clear to enter the dock
    fn is_clear_to_enter(
        &self,
        obj_id: ObjectID,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Get entry position coordinates
    fn get_enter_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called when entry position reached
    fn on_enter_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get actual dock position coordinates
    fn get_dock_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called when dock position reached
    fn on_dock_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Perform dock action (repair, supply, etc.)
    /// Returns true when action is complete
    fn action(
        &mut self,
        obj_id: ObjectID,
        drone_id: Option<ObjectID>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Get exit position coordinates
    fn get_exit_position(
        &self,
        obj_id: ObjectID,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called when exit position reached
    fn on_exit_reached(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Check if this is a passthrough type dock
    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Check if units should go to rally point after docking
    fn is_rally_point_after_dock_type(
        &self,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Set dock crippled state (optional)
    fn set_dock_crippled(
        &mut self,
        crippled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Production update interface for building units
pub trait ProductionUpdateInterface: Send + Sync {
    /// Check if can produce a specific unit/upgrade
    fn can_produce(&self, template_name: &str) -> bool;

    /// Start production of a unit/upgrade
    fn start_production(
        &mut self,
        template_name: String,
        player_id: ObjectID,
    ) -> Result<(), String>;

    /// Cancel production at queue index
    fn cancel_production(&mut self, index: usize) -> Result<(), String>;

    /// Get current queue size
    fn get_queue_size(&self) -> usize;

    /// Snapshot queue entries for UI/debug consumers.
    ///
    /// Default returns an empty list for implementations that do not expose
    /// queue internals.
    fn get_queue_entries(&self) -> Vec<crate::object::production::queue::BuildQueueEntry> {
        Vec::new()
    }

    /// Check if any queued or active production entry is an upgrade.
    fn has_any_upgrade_in_queue(&self) -> bool {
        false
    }

    /// Get production progress (0.0 to 1.0)
    fn get_production_progress(&self) -> f32;

    /// Check if currently producing
    fn is_producing(&self) -> bool;

    /// Pause production
    fn pause_production(&mut self);

    /// Resume production
    fn resume_production(&mut self);

    /// Hold or release a production door open (matches C++ setHoldDoorOpen).
    fn set_hold_door_open(&mut self, _exit_door: usize, _hold_it: bool) {}
}

/// Projectile update interface for projectiles
pub trait ProjectileUpdateInterface {
    fn projectile_update(&mut self, object_id: ObjectID, delta_time: Real);

    /// Return the launcher credited for projectile damage.
    fn projectile_get_launcher_id(&self) -> ObjectID {
        INVALID_ID
    }

    /// Notify projectile it has been jammed (matches C++ ProjectileUpdateInterface::projectileNowJammed).
    fn projectile_now_jammed(&mut self) {
        let _ = self;
    }

    /// Schedule missile diversion after countermeasure decoy delay.
    fn set_frames_till_countermeasure_diversion_occurs(
        &mut self,
        _frames: UnsignedInt,
        _current_frame: UnsignedInt,
    ) {
    }
}

/// Update module interface for general updates (matching C++ UpdateModuleInterface)
pub trait UpdateModuleInterface: Send + Sync {
    /// Update the module
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(UPDATE_SLEEP_NONE)
    }
    /// Simplified update hook most modules implement
    fn update_simple(&mut self) -> UpdateSleepTime {
        match self.update() {
            Ok(sleep) => sleep,
            Err(_) => UPDATE_SLEEP_NONE,
        }
    }
    /// Get disabled types to process
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::empty() // Default: process no disabled types
    }
    /// Phase hint executed after this module wakes.
    fn get_update_phase(&self) -> SleepyUpdatePhase {
        SleepyUpdatePhase::Normal
    }

    /// Lifecycle hook when object is created (matches C++ Module::OnObjectCreated).
    fn on_object_created(&mut self) {
        let _ = self;
    }
}

